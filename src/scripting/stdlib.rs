//! Standard library for the scripting engine.
//!
//! Registers built-in functions: print, tostring, tonumber, type,
//! math.*, string.*, table.*, io.*, os.*, pairs, ipairs, etc.

use std::sync::Arc;

use super::vm::{NativeFunc, ScriptError, Table, Value, Vm};

// ── Registration entry point ─────────────────────────────────────────────────

/// Register all standard library functions into the VM.
pub fn register_all(vm: &mut Vm) {
    register_globals(vm);
    register_math(vm);
    register_string(vm);
    register_table(vm);
    register_io(vm);
    register_os(vm);
}

// ── Global functions ──────────────────────────────────────────────────────────

fn register_globals(vm: &mut Vm) {
    vm.register_native("print", |vm, args| {
        let out: Vec<String> = args.iter().map(|v| v.to_string()).collect();
        let line = out.join("\t");
        vm.output.push(line.clone());
        println!("{}", line);
        Ok(vec![])
    });

    vm.register_native("tostring", |_vm, args| {
        let s = args.into_iter().next().unwrap_or(Value::Nil).to_string();
        Ok(vec![Value::Str(Arc::new(s))])
    });

    vm.register_native("tonumber", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        let result = match &v {
            Value::Int(i)   => Value::Int(*i),
            Value::Float(f) => Value::Float(*f),
            Value::Str(s)   => {
                if let Ok(i) = s.parse::<i64>() {
                    Value::Int(i)
                } else if let Ok(f) = s.parse::<f64>() {
                    Value::Float(f)
                } else {
                    Value::Nil
                }
            }
            _ => Value::Nil,
        };
        Ok(vec![result])
    });

    vm.register_native("type", |_vm, args| {
        let t = args.into_iter().next().unwrap_or(Value::Nil).type_name();
        Ok(vec![Value::Str(Arc::new(t.to_string()))])
    });

    vm.register_native("assert", |_vm, args| {
        let cond = args.first().cloned().unwrap_or(Value::Nil);
        if !cond.is_truthy() {
            let msg = args.get(1).map(|v| v.to_string())
                .unwrap_or_else(|| "assertion failed!".to_string());
            return Err(ScriptError::new(msg));
        }
        Ok(args)
    });

    vm.register_native("error", |_vm, args| {
        let msg = args.into_iter().next().unwrap_or(Value::Nil).to_string();
        Err(ScriptError::new(msg))
    });

    vm.register_native("pcall", |vm, mut args| {
        if args.is_empty() { return Ok(vec![Value::Bool(false), Value::Str(Arc::new("no function".to_string()))]); }
        let func = args.remove(0);
        match vm.call(func, args) {
            Ok(mut results) => {
                results.insert(0, Value::Bool(true));
                Ok(results)
            }
            Err(e) => Ok(vec![Value::Bool(false), Value::Str(Arc::new(e.message))]),
        }
    });

    vm.register_native("xpcall", |vm, mut args| {
        if args.len() < 2 { return Ok(vec![Value::Bool(false)]); }
        let func    = args.remove(0);
        let _handler = args.remove(0);
        match vm.call(func, args) {
            Ok(mut results) => {
                results.insert(0, Value::Bool(true));
                Ok(results)
            }
            Err(e) => Ok(vec![Value::Bool(false), Value::Str(Arc::new(e.message))]),
        }
    });

    vm.register_native("ipairs", |_vm, args| {
        let table = args.into_iter().next().unwrap_or(Value::Nil);
        // Returns iterator function, table, 0
        let iter_fn = Arc::new(NativeFunc {
            name: "ipairs_iter".to_string(),
            func: Box::new(|_vm, args| {
                let table = args.first().cloned().unwrap_or(Value::Nil);
                let idx   = args.get(1).and_then(|v| v.to_int()).unwrap_or(0);
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

    vm.register_native("pairs", |_vm, args| {
        let table = args.into_iter().next().unwrap_or(Value::Nil);
        let iter_fn = Arc::new(NativeFunc {
            name: "pairs_iter".to_string(),
            func: Box::new(|_vm, args| {
                let table = args.first().cloned().unwrap_or(Value::Nil);
                let key   = args.get(1).cloned().unwrap_or(Value::Nil);
                match &table {
                    Value::Table(t) => {
                        match t.next(&key) {
                            Some((k, v)) => Ok(vec![k, v]),
                            None         => Ok(vec![Value::Nil]),
                        }
                    }
                    _ => Ok(vec![Value::Nil]),
                }
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn), table, Value::Nil])
    });

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

    vm.register_native("select", |_vm, args| {
        let selector = args.first().cloned().unwrap_or(Value::Nil);
        match selector {
            Value::Str(s) if s.as_ref() == "#" => {
                Ok(vec![Value::Int((args.len() - 1) as i64)])
            }
            Value::Int(i) => {
                let rest: Vec<Value> = args.into_iter().skip(i as usize).collect();
                Ok(rest)
            }
            _ => Err(ScriptError::new("select: invalid index")),
        }
    });

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

    vm.register_native("rawget", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let key   = args.get(1).cloned().unwrap_or(Value::Nil);
        match table {
            Value::Table(t) => Ok(vec![t.get(&key)]),
            _ => Err(ScriptError::new("rawget: not a table")),
        }
    });

    vm.register_native("rawset", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let key   = args.get(1).cloned().unwrap_or(Value::Nil);
        let val   = args.get(2).cloned().unwrap_or(Value::Nil);
        match &table {
            Value::Table(t) => { t.set(key, val); Ok(vec![table]) }
            _ => Err(ScriptError::new("rawset: not a table")),
        }
    });

    vm.register_native("rawequal", |_vm, args| {
        let a = args.first().cloned().unwrap_or(Value::Nil);
        let b = args.get(1).cloned().unwrap_or(Value::Nil);
        Ok(vec![Value::Bool(a == b)])
    });

    vm.register_native("rawlen", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        let n = match v {
            Value::Table(t) => t.length(),
            Value::Str(s)   => s.len() as i64,
            _ => return Err(ScriptError::new("rawlen: not a table or string")),
        };
        Ok(vec![Value::Int(n)])
    });

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

    vm.register_native("require", |_vm, args| {
        // Minimal stub: returns nil (host can override)
        let _path = args.into_iter().next();
        Ok(vec![Value::Nil])
    });

    vm.register_native("collectgarbage", |_vm, _args| {
        Ok(vec![Value::Int(0)])
    });
}

// ── math.* ────────────────────────────────────────────────────────────────────

fn register_math(vm: &mut Vm) {
    let math = Table::new();

    math.rawset_str("pi",  Value::Float(std::f64::consts::PI));
    math.rawset_str("huge", Value::Float(f64::INFINITY));
    math.rawset_str("maxinteger", Value::Int(i64::MAX));
    math.rawset_str("mininteger", Value::Int(i64::MIN));

    math_fn(&math, "abs",   |a: f64| a.abs());
    math_fn(&math, "ceil",  |a: f64| a.ceil());
    math_fn(&math, "floor", |a: f64| a.floor());
    math_fn(&math, "sqrt",  |a: f64| a.sqrt());
    math_fn(&math, "sin",   |a: f64| a.sin());
    math_fn(&math, "cos",   |a: f64| a.cos());
    math_fn(&math, "tan",   |a: f64| a.tan());
    math_fn(&math, "asin",  |a: f64| a.asin());
    math_fn(&math, "acos",  |a: f64| a.acos());
    math_fn(&math, "atan",  |a: f64| a.atan());
    math_fn(&math, "exp",   |a: f64| a.exp());
    math_fn(&math, "log",   |a: f64| a.ln());

    math.rawset_str("log", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.log".to_string(),
        func: Box::new(|_vm, args| {
            let x    = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
            let base = args.get(1).and_then(|v| v.to_float());
            let result = match base {
                Some(b) => x.log(b),
                None    => x.ln(),
            };
            Ok(vec![Value::Float(result)])
        }),
    })));

    math.rawset_str("max", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.max".to_string(),
        func: Box::new(|_vm, args| {
            if args.is_empty() { return Err(ScriptError::new("math.max: no arguments")); }
            let mut best = args[0].clone();
            for a in &args[1..] {
                let bv = best.to_float().unwrap_or(f64::NEG_INFINITY);
                let av = a.to_float().unwrap_or(f64::NEG_INFINITY);
                if av > bv { best = a.clone(); }
            }
            Ok(vec![best])
        }),
    })));

    math.rawset_str("min", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.min".to_string(),
        func: Box::new(|_vm, args| {
            if args.is_empty() { return Err(ScriptError::new("math.min: no arguments")); }
            let mut best = args[0].clone();
            for a in &args[1..] {
                let bv = best.to_float().unwrap_or(f64::INFINITY);
                let av = a.to_float().unwrap_or(f64::INFINITY);
                if av < bv { best = a.clone(); }
            }
            Ok(vec![best])
        }),
    })));

    math.rawset_str("fmod", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.fmod".to_string(),
        func: Box::new(|_vm, args| {
            let a = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
            let b = args.get(1).and_then(|v| v.to_float()).unwrap_or(1.0);
            Ok(vec![Value::Float(a % b)])
        }),
    })));

    math.rawset_str("modf", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.modf".to_string(),
        func: Box::new(|_vm, args| {
            let a = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
            let int_part = a.trunc();
            let frac_part = a.fract();
            Ok(vec![Value::Float(int_part), Value::Float(frac_part)])
        }),
    })));

    math.rawset_str("type", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.type".to_string(),
        func: Box::new(|_vm, args| {
            let v = args.into_iter().next().unwrap_or(Value::Nil);
            let t = match v {
                Value::Int(_)   => "integer",
                Value::Float(_) => "float",
                _               => "other",
            };
            Ok(vec![Value::Str(Arc::new(t.to_string()))])
        }),
    })));

    math.rawset_str("tointeger", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.tointeger".to_string(),
        func: Box::new(|_vm, args| {
            let v = args.into_iter().next().unwrap_or(Value::Nil);
            Ok(vec![v.to_int().map(Value::Int).unwrap_or(Value::Nil)])
        }),
    })));

    // Simple LCG random
    math.rawset_str("random", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.random".to_string(),
        func: Box::new(|_vm, args| {
            // Pseudo-random using time-based seed
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(42);
            let r = (seed.wrapping_mul(1664525).wrapping_add(1013904223)) as f64
                / u32::MAX as f64;
            let result = match args.len() {
                0 => Value::Float(r),
                1 => {
                    let m = args[0].to_int().unwrap_or(1);
                    Value::Int(1 + (r * m as f64) as i64 % m)
                }
                _ => {
                    let lo = args[0].to_int().unwrap_or(1);
                    let hi = args[1].to_int().unwrap_or(1);
                    Value::Int(lo + (r * (hi - lo + 1) as f64) as i64 % (hi - lo + 1).max(1))
                }
            };
            Ok(vec![result])
        }),
    })));

    math.rawset_str("randomseed", Value::NativeFunction(Arc::new(NativeFunc {
        name: "math.randomseed".to_string(),
        func: Box::new(|_vm, _args| Ok(vec![])),
    })));

    vm.set_global("math", Value::Table(math));
}

fn math_fn(table: &Table, name: &'static str, f: fn(f64) -> f64) {
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

    string.rawset_str("len", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.len".to_string(),
        func: Box::new(|_vm, args| {
            let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            Ok(vec![Value::Int(s.len() as i64)])
        }),
    })));

    string.rawset_str("sub", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.sub".to_string(),
        func: Box::new(|_vm, args| {
            let s    = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            let len  = s.len() as i64;
            let i    = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
            let j    = args.get(2).and_then(|v| v.to_int()).unwrap_or(-1);
            let from = if i < 0 { (len + i).max(0) } else { (i - 1).max(0) } as usize;
            let to   = if j < 0 { (len + j + 1).max(0) } else { j.min(len) } as usize;
            let result = if from <= to && from < s.len() {
                s.get(from..to.min(s.len())).unwrap_or("").to_string()
            } else { String::new() };
            Ok(vec![Value::Str(Arc::new(result))])
        }),
    })));

    string.rawset_str("upper", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.upper".to_string(),
        func: Box::new(|_vm, args| {
            let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            Ok(vec![Value::Str(Arc::new(s.to_uppercase()))])
        }),
    })));

    string.rawset_str("lower", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.lower".to_string(),
        func: Box::new(|_vm, args| {
            let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            Ok(vec![Value::Str(Arc::new(s.to_lowercase()))])
        }),
    })));

    string.rawset_str("rep", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.rep".to_string(),
        func: Box::new(|_vm, args| {
            let s   = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            let n   = args.get(1).and_then(|v| v.to_int()).unwrap_or(0).max(0) as usize;
            let sep = args.get(2).and_then(|v| v.to_str_repr()).unwrap_or_default();
            let result = if n == 0 { String::new() }
            else {
                let parts: Vec<&str> = std::iter::repeat(s.as_str()).take(n).collect();
                parts.join(&sep)
            };
            Ok(vec![Value::Str(Arc::new(result))])
        }),
    })));

    string.rawset_str("reverse", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.reverse".to_string(),
        func: Box::new(|_vm, args| {
            let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            Ok(vec![Value::Str(Arc::new(s.chars().rev().collect()))])
        }),
    })));

    string.rawset_str("byte", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.byte".to_string(),
        func: Box::new(|_vm, args| {
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
        }),
    })));

    string.rawset_str("char", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.char".to_string(),
        func: Box::new(|_vm, args| {
            let mut s = String::new();
            for a in &args {
                if let Some(i) = a.to_int() {
                    if let Some(c) = char::from_u32(i as u32) {
                        s.push(c);
                    }
                }
            }
            Ok(vec![Value::Str(Arc::new(s))])
        }),
    })));

    string.rawset_str("format", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.format".to_string(),
        func: Box::new(|_vm, args| {
            let fmt_str = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            let result = simple_format(&fmt_str, &args[1.min(args.len())..]);
            Ok(vec![Value::Str(Arc::new(result))])
        }),
    })));

    string.rawset_str("find", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.find".to_string(),
        func: Box::new(|_vm, args| {
            let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
            let init    = args.get(2).and_then(|v| v.to_int()).unwrap_or(1);
            let start   = (init as usize).saturating_sub(1).min(s.len());
            if let Some(pos) = s[start..].find(&pattern as &str) {
                let abs_start = start + pos + 1;
                let abs_end   = abs_start + pattern.len() - 1;
                Ok(vec![Value::Int(abs_start as i64), Value::Int(abs_end as i64)])
            } else {
                Ok(vec![Value::Nil])
            }
        }),
    })));

    string.rawset_str("gsub", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.gsub".to_string(),
        func: Box::new(|_vm, args| {
            let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
            let repl    = args.get(2).and_then(|v| v.to_str_repr()).unwrap_or_default();
            let _max_n  = args.get(3).and_then(|v| v.to_int());
            let count = s.matches(pattern.as_str()).count();
            let result = s.replace(pattern.as_str(), &repl);
            Ok(vec![Value::Str(Arc::new(result)), Value::Int(count as i64)])
        }),
    })));

    string.rawset_str("match", Value::NativeFunction(Arc::new(NativeFunc {
        name: "string.match".to_string(),
        func: Box::new(|_vm, args| {
            let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
            let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
            if s.contains(pattern.as_str()) {
                Ok(vec![Value::Str(Arc::new(pattern))])
            } else {
                Ok(vec![Value::Nil])
            }
        }),
    })));

    vm.set_global("string", Value::Table(string));
}

/// Very minimal printf-style formatter (handles %d %f %s %q %%.)
fn simple_format(fmt: &str, args: &[Value]) -> String {
    let mut result = String::new();
    let mut chars  = fmt.chars().peekable();
    let mut arg_i  = 0usize;
    while let Some(c) = chars.next() {
        if c != '%' { result.push(c); continue; }
        match chars.next() {
            Some('%')  => result.push('%'),
            Some('d') | Some('i') => {
                let v = args.get(arg_i).and_then(|v| v.to_int()).unwrap_or(0);
                result.push_str(&v.to_string());
                arg_i += 1;
            }
            Some('f') => {
                let v = args.get(arg_i).and_then(|v| v.to_float()).unwrap_or(0.0);
                result.push_str(&format!("{:.6}", v));
                arg_i += 1;
            }
            Some('g') => {
                let v = args.get(arg_i).and_then(|v| v.to_float()).unwrap_or(0.0);
                result.push_str(&format!("{}", v));
                arg_i += 1;
            }
            Some('s') => {
                let v = args.get(arg_i).map(|v| v.to_string()).unwrap_or_default();
                result.push_str(&v);
                arg_i += 1;
            }
            Some('q') => {
                let v = args.get(arg_i).and_then(|v| v.to_str_repr()).unwrap_or_default();
                result.push('"');
                for ch in v.chars() {
                    match ch {
                        '"'  => result.push_str("\\\""),
                        '\\' => result.push_str("\\\\"),
                        '\n' => result.push_str("\\n"),
                        c    => result.push(c),
                    }
                }
                result.push('"');
                arg_i += 1;
            }
            Some(x) => { result.push('%'); result.push(x); }
            None    => { result.push('%'); }
        }
    }
    result
}

// ── table.* ───────────────────────────────────────────────────────────────────

fn register_table(vm: &mut Vm) {
    let tbl = Table::new();

    tbl.rawset_str("insert", Value::NativeFunction(Arc::new(NativeFunc {
        name: "table.insert".to_string(),
        func: Box::new(|_vm, args| {
            let table = args.first().cloned().unwrap_or(Value::Nil);
            match table {
                Value::Table(t) => {
                    if args.len() == 2 {
                        // Append
                        t.push(args[1].clone());
                    } else if args.len() >= 3 {
                        // Insert at position
                        let _pos = args[1].to_int().unwrap_or(1);
                        t.push(args[2].clone());
                    }
                    Ok(vec![])
                }
                _ => Err(ScriptError::new("table.insert: not a table")),
            }
        }),
    })));

    tbl.rawset_str("remove", Value::NativeFunction(Arc::new(NativeFunc {
        name: "table.remove".to_string(),
        func: Box::new(|_vm, args| {
            let table = args.first().cloned().unwrap_or(Value::Nil);
            match &table {
                Value::Table(t) => {
                    let len = t.length();
                    let pos = args.get(1).and_then(|v| v.to_int()).unwrap_or(len);
                    if len == 0 { return Ok(vec![Value::Nil]); }
                    let removed = t.get(&Value::Int(pos));
                    // Shift elements down
                    for i in pos..len {
                        let next = t.get(&Value::Int(i + 1));
                        t.set(Value::Int(i), next);
                    }
                    t.set(Value::Int(len), Value::Nil);
                    Ok(vec![removed])
                }
                _ => Err(ScriptError::new("table.remove: not a table")),
            }
        }),
    })));

    tbl.rawset_str("concat", Value::NativeFunction(Arc::new(NativeFunc {
        name: "table.concat".to_string(),
        func: Box::new(|_vm, args| {
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
        }),
    })));

    tbl.rawset_str("sort", Value::NativeFunction(Arc::new(NativeFunc {
        name: "table.sort".to_string(),
        func: Box::new(|_vm, args| {
            let table = args.first().cloned().unwrap_or(Value::Nil);
            match &table {
                Value::Table(t) => {
                    let mut v = t.array_values();
                    v.sort_by(|a, b| {
                        let af = a.to_float().unwrap_or(0.0);
                        let bf = b.to_float().unwrap_or(0.0);
                        af.partial_cmp(&bf).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    for (i, val) in v.into_iter().enumerate() {
                        t.set(Value::Int(i as i64 + 1), val);
                    }
                    Ok(vec![])
                }
                _ => Err(ScriptError::new("table.sort: not a table")),
            }
        }),
    })));

    tbl.rawset_str("unpack", Value::NativeFunction(Arc::new(NativeFunc {
        name: "table.unpack".to_string(),
        func: Box::new(|_vm, args| {
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
                _ => Err(ScriptError::new("table.unpack: not a table")),
            }
        }),
    })));

    tbl.rawset_str("move", Value::NativeFunction(Arc::new(NativeFunc {
        name: "table.move".to_string(),
        func: Box::new(|_vm, args| {
            let a1  = args.first().cloned().unwrap_or(Value::Nil);
            let f   = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
            let e   = args.get(2).and_then(|v| v.to_int()).unwrap_or(0);
            let t_p = args.get(3).and_then(|v| v.to_int()).unwrap_or(1);
            let a2  = args.get(4).cloned().unwrap_or_else(|| a1.clone());
            if let (Value::Table(src), Value::Table(dst)) = (&a1, &a2) {
                let mut vals = Vec::new();
                for i in f..=e {
                    vals.push(src.get(&Value::Int(i)));
                }
                for (offset, val) in vals.into_iter().enumerate() {
                    dst.set(Value::Int(t_p + offset as i64), val);
                }
            }
            Ok(vec![a2])
        }),
    })));

    vm.set_global("table", Value::Table(tbl));
}

// ── io.* ─────────────────────────────────────────────────────────────────────

fn register_io(vm: &mut Vm) {
    let io = Table::new();

    io.rawset_str("write", Value::NativeFunction(Arc::new(NativeFunc {
        name: "io.write".to_string(),
        func: Box::new(|vm, args| {
            let s: String = args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("");
            vm.output.push(s.clone());
            print!("{}", s);
            Ok(vec![])
        }),
    })));

    io.rawset_str("read", Value::NativeFunction(Arc::new(NativeFunc {
        name: "io.read".to_string(),
        func: Box::new(|_vm, _args| {
            // Stub: return nil in sandbox
            Ok(vec![Value::Nil])
        }),
    })));

    vm.set_global("io", Value::Table(io));
}

// ── os.* ─────────────────────────────────────────────────────────────────────

fn register_os(vm: &mut Vm) {
    let os = Table::new();

    os.rawset_str("time", Value::NativeFunction(Arc::new(NativeFunc {
        name: "os.time".to_string(),
        func: Box::new(|_vm, _args| {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            Ok(vec![Value::Int(t)])
        }),
    })));

    os.rawset_str("clock", Value::NativeFunction(Arc::new(NativeFunc {
        name: "os.clock".to_string(),
        func: Box::new(|_vm, _args| {
            Ok(vec![Value::Float(0.0)])
        }),
    })));

    os.rawset_str("date", Value::NativeFunction(Arc::new(NativeFunc {
        name: "os.date".to_string(),
        func: Box::new(|_vm, _args| {
            Ok(vec![Value::Str(Arc::new("01/01/1970 00:00:00".to_string()))])
        }),
    })));

    vm.set_global("os", Value::Table(os));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::{compiler::Compiler, parser::Parser};

    fn run_with_stdlib(src: &str) -> Vec<Value> {
        let script = Parser::from_source("test", src).expect("parse error");
        let chunk  = Compiler::compile_script(&script);
        let mut vm = Vm::new();
        register_all(&mut vm);
        vm.execute(chunk).expect("runtime error")
    }

    #[test]
    fn test_math_abs() {
        let r = run_with_stdlib("return math.abs(-5)");
        assert!(matches!(&r[0], Value::Float(f) if (*f - 5.0).abs() < 1e-6));
    }

    #[test]
    fn test_math_floor() {
        let r = run_with_stdlib("return math.floor(3.7)");
        assert!(matches!(&r[0], Value::Float(f) if (*f - 3.0).abs() < 1e-6));
    }

    #[test]
    fn test_string_upper() {
        let r = run_with_stdlib("return string.upper(\"hello\")");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "HELLO"));
    }

    #[test]
    fn test_string_len() {
        let r = run_with_stdlib("return string.len(\"hello\")");
        assert_eq!(r[0], Value::Int(5));
    }

    #[test]
    fn test_table_insert_concat() {
        let r = run_with_stdlib(
            "local t = {} table.insert(t, \"a\") table.insert(t, \"b\") return table.concat(t, \",\")"
        );
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "a,b"));
    }

    #[test]
    fn test_type_function() {
        let r = run_with_stdlib("return type(42)");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "integer"));
    }

    #[test]
    fn test_tostring() {
        let r = run_with_stdlib("return tostring(3.14)");
        assert!(matches!(&r[0], Value::Str(_)));
    }

    #[test]
    fn test_pcall_success() {
        let r = run_with_stdlib("return pcall(function() return 42 end)");
        assert_eq!(r[0], Value::Bool(true));
    }

    #[test]
    fn test_pcall_error() {
        let r = run_with_stdlib("return pcall(function() error(\"oops\") end)");
        assert_eq!(r[0], Value::Bool(false));
    }
}
