//! Host bridge — exposes Rust functions and objects to the scripting VM.
//!
//! `ScriptHost` wraps a `Vm` and provides a clean API for:
//!   - Registering host functions callable from scripts
//!   - Binding host objects as script tables
//!   - Executing scripts and retrieving results
//!   - Event-driven scripting (hooks / callbacks)

use std::collections::HashMap;
use std::sync::Arc;

use super::compiler::Compiler;
use super::parser::Parser;
use super::stdlib;
use super::vm::{NativeFunc, ScriptError, Table, Value, Vm};

// ── HostFunction type alias ───────────────────────────────────────────────────

/// A Rust function callable from scripts.
pub type HostFunction =
    Arc<dyn Fn(&mut Vm, Vec<Value>) -> Result<Vec<Value>, ScriptError> + Send + Sync>;

// ── ScriptHost ────────────────────────────────────────────────────────────────

/// High-level scripting host. Owns the VM and provides ergonomic registration APIs.
pub struct ScriptHost {
    vm:      Vm,
    modules: HashMap<String, Value>,
}

impl ScriptHost {
    /// Create a new ScriptHost with the standard library pre-registered.
    pub fn new() -> Self {
        let mut vm = Vm::new();
        stdlib::register_all(&mut vm);
        ScriptHost { vm, modules: HashMap::new() }
    }

    /// Create a sandboxed host with no standard library.
    pub fn sandboxed() -> Self {
        ScriptHost { vm: Vm::new(), modules: HashMap::new() }
    }

    // ── Registration API ──────────────────────────────────────────────────

    /// Register a Rust closure as a global script function.
    pub fn register<F>(&mut self, name: &str, f: F)
    where
        F: Fn(&mut Vm, Vec<Value>) -> Result<Vec<Value>, ScriptError> + Send + Sync + 'static,
    {
        self.vm.register_native(name, f);
    }

    /// Register a simple 1-argument Rust function.
    pub fn register_fn<F>(&mut self, name: &str, f: F)
    where
        F: Fn(Vec<Value>) -> Result<Vec<Value>, ScriptError> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        self.vm.register_native(name, move |_vm, args| f(args));
    }

    /// Set a global variable.
    pub fn set(&mut self, name: &str, value: Value) {
        self.vm.set_global(name, value);
    }

    /// Get a global variable.
    pub fn get(&self, name: &str) -> Value {
        self.vm.get_global(name)
    }

    /// Expose a Rust table of functions as a named module.
    ///
    /// ```ignore
    /// host.register_module("engine", vec![
    ///     ("spawn", Arc::new(|_vm, args| { ... })),
    /// ]);
    /// ```
    pub fn register_module(&mut self, name: &str, funcs: Vec<(&str, HostFunction)>) {
        let table = Table::new();
        for (fname, f) in funcs {
            let fname = fname.to_string();
            let func_arc = Arc::clone(&f);
            table.rawset_str(&fname, Value::NativeFunction(Arc::new(NativeFunc {
                name: format!("{}.{}", name, fname),
                func: Box::new(move |vm, args| (func_arc)(vm, args)),
            })));
        }
        let v = Value::Table(table.clone());
        self.vm.set_global(name, v.clone());
        self.modules.insert(name.to_string(), v);
    }

    // ── Execution API ─────────────────────────────────────────────────────

    /// Execute a script string, returning all return values.
    pub fn exec(&mut self, source: &str) -> Result<Vec<Value>, ScriptError> {
        let script = Parser::from_source("<inline>", source)
            .map_err(|e| ScriptError::new(e.to_string()))?;
        let proto = Compiler::compile_script(&script);
        self.vm.execute(proto)
    }

    /// Execute a named script (for better error messages).
    pub fn exec_named(&mut self, name: &str, source: &str) -> Result<Vec<Value>, ScriptError> {
        let script = Parser::from_source(name, source)
            .map_err(|e| ScriptError::new(e.to_string()))?;
        let proto = Compiler::compile_script(&script);
        self.vm.execute(proto)
    }

    /// Call a previously defined script function by name.
    pub fn call(&mut self, func_name: &str, args: Vec<Value>) -> Result<Vec<Value>, ScriptError> {
        let func = self.vm.get_global(func_name);
        self.vm.call(func, args)
    }

    /// Call a method on a table: `table_name.method_name(args)`.
    pub fn call_method(
        &mut self,
        table_name: &str,
        method_name: &str,
        args: Vec<Value>,
    ) -> Result<Vec<Value>, ScriptError> {
        let table = self.vm.get_global(table_name);
        match &table {
            Value::Table(t) => {
                let method = t.rawget_str(method_name);
                self.vm.call(method, args)
            }
            other => Err(ScriptError::new(format!(
                "call_method: {} is not a table (got {})", table_name, other.type_name()
            ))),
        }
    }

    // ── Convenience getters ───────────────────────────────────────────────

    /// Get a global as an integer.
    pub fn get_int(&self, name: &str) -> Option<i64> {
        self.vm.get_global(name).to_int()
    }

    /// Get a global as a float.
    pub fn get_float(&self, name: &str) -> Option<f64> {
        self.vm.get_global(name).to_float()
    }

    /// Get a global as a string.
    pub fn get_string(&self, name: &str) -> Option<String> {
        self.vm.get_global(name).to_str_repr()
    }

    /// Get a global as a boolean.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.vm.get_global(name) {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    // ── Captured output ───────────────────────────────────────────────────

    /// Drain all lines produced by `print()` calls.
    pub fn drain_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.vm.output)
    }

    /// Access the underlying VM directly.
    pub fn vm(&mut self) -> &mut Vm {
        &mut self.vm
    }
}

impl Default for ScriptHost {
    fn default() -> Self {
        ScriptHost::new()
    }
}

// ── EventBus ──────────────────────────────────────────────────────────────────

/// Simple event system for triggering script callbacks.
pub struct EventBus {
    handlers: HashMap<String, Vec<String>>, // event_name -> [script function names]
}

impl EventBus {
    pub fn new() -> Self {
        EventBus { handlers: HashMap::new() }
    }

    /// Register a script function to handle a named event.
    pub fn on(&mut self, event: &str, func_name: &str) {
        self.handlers
            .entry(event.to_string())
            .or_default()
            .push(func_name.to_string());
    }

    /// Fire an event, calling all registered handlers.
    pub fn emit(
        &self,
        event: &str,
        host: &mut ScriptHost,
        args: Vec<Value>,
    ) -> Result<(), ScriptError> {
        if let Some(handlers) = self.handlers.get(event) {
            for func_name in handlers {
                host.call(func_name, args.clone())?;
            }
        }
        Ok(())
    }

    /// Remove all handlers for an event.
    pub fn clear(&mut self, event: &str) {
        self.handlers.remove(event);
    }
}

// ── ScriptObject ──────────────────────────────────────────────────────────────

/// A Rust-owned object exposed to scripts via a table interface.
pub trait ScriptObject: Send + Sync {
    fn script_type_name(&self) -> &'static str;
    fn get_field(&self, name: &str) -> Value;
    fn set_field(&mut self, name: &str, value: Value);
    fn call_method(&mut self, name: &str, args: Vec<Value>) -> Result<Vec<Value>, ScriptError>;
}

/// Wrap a `ScriptObject` as a script table. Changes to the table are NOT
/// reflected back to the Rust object — use `bind_object` for two-way binding.
pub fn object_to_table<O: ScriptObject>(obj: &O) -> Table {
    let t = Table::new();
    // Expose type name
    t.rawset_str("__type", Value::Str(Arc::new(obj.script_type_name().to_string())));
    t
}

// ── ScriptComponent ──────────────────────────────────────────────────────────

/// A script component: holds a pre-compiled chunk + per-instance table.
pub struct ScriptComponent {
    source:  String,
    globals: Table,
}

impl ScriptComponent {
    pub fn new(source: impl Into<String>) -> Self {
        ScriptComponent {
            source:  source.into(),
            globals: Table::new(),
        }
    }

    /// Run the script body, populating the component's globals table.
    pub fn init(&mut self, host: &mut ScriptHost) -> Result<(), ScriptError> {
        // Expose 'self' table to the script
        host.set("self", Value::Table(self.globals.clone()));
        host.exec_named("<component>", &self.source)?;
        Ok(())
    }

    /// Call a method defined by this component's script.
    pub fn call(&mut self, host: &mut ScriptHost, method: &str, args: Vec<Value>) -> Result<Vec<Value>, ScriptError> {
        let func = self.globals.rawget_str(method);
        if matches!(func, Value::Nil) {
            return Ok(vec![]);
        }
        host.vm().call(func, args)
    }

    pub fn table(&self) -> &Table { &self.globals }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_exec() {
        let mut host = ScriptHost::new();
        let result = host.exec("return 1 + 2").unwrap();
        assert_eq!(result[0], Value::Int(3));
    }

    #[test]
    fn test_host_set_get() {
        let mut host = ScriptHost::new();
        host.set("x", Value::Int(99));
        let result = host.exec("return x").unwrap();
        assert_eq!(result[0], Value::Int(99));
    }

    #[test]
    fn test_host_register_fn() {
        let mut host = ScriptHost::new();
        host.register("double", |_vm, args| {
            let n = args.first().and_then(|v| v.to_int()).unwrap_or(0);
            Ok(vec![Value::Int(n * 2)])
        });
        let result = host.exec("return double(21)").unwrap();
        assert_eq!(result[0], Value::Int(42));
    }

    #[test]
    fn test_host_call_function() {
        let mut host = ScriptHost::new();
        host.exec("function greet(name) return \"Hello, \" .. name end").unwrap();
        let result = host.call("greet", vec![Value::Str(Arc::new("World".to_string()))]).unwrap();
        assert!(matches!(&result[0], Value::Str(s) if s.as_ref() == "Hello, World"));
    }

    #[test]
    fn test_host_print_capture() {
        let mut host = ScriptHost::new();
        host.exec("print(\"hello\")").unwrap();
        let out = host.drain_output();
        assert_eq!(out, vec!["hello"]);
    }

    #[test]
    fn test_event_bus() {
        let mut host = ScriptHost::new();
        let mut bus  = EventBus::new();
        host.exec("function on_tick() end").unwrap();
        bus.on("tick", "on_tick");
        assert!(bus.emit("tick", &mut host, vec![]).is_ok());
    }

    #[test]
    fn test_register_module() {
        let mut host = ScriptHost::new();
        let add_fn: HostFunction = Arc::new(|_vm, args| {
            let a = args.first().and_then(|v| v.to_int()).unwrap_or(0);
            let b = args.get(1).and_then(|v| v.to_int()).unwrap_or(0);
            Ok(vec![Value::Int(a + b)])
        });
        host.register_module("mymod", vec![("add", add_fn)]);
        let result = host.exec("return mymod.add(3, 4)").unwrap();
        assert_eq!(result[0], Value::Int(7));
    }

    #[test]
    fn test_sandboxed_no_stdlib() {
        let mut host = ScriptHost::sandboxed();
        // math shouldn't be available
        let result = host.exec("return type(math)");
        // Either an error or nil is acceptable
        match result {
            Ok(r)  => assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "nil")),
            Err(_) => {} // also fine
        }
    }
}
