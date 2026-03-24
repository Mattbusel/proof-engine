//! Module system — loading, caching, dependency resolution, hot-reload.
//!
//! # Architecture
//! ```text
//! PackageManager
//!   └─ ModuleRegistry
//!        ├─ Module (Unloaded / Loading / Loaded / Error)
//!        └─ StringMapLoader / FileLoader
//! Namespace  — hierarchical name lookup
//! HotReloadWatcher — timestamp-based change detection
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use super::compiler::{Chunk, Compiler};
use super::parser::Parser;
use super::vm::{ScriptError, Table, Value, Vm};

// ── ModuleId ──────────────────────────────────────────────────────────────────

/// FNV-1a hash of a module path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u64);

impl ModuleId {
    pub fn from_path(path: &str) -> Self {
        ModuleId(fnv1a(path.as_bytes()))
    }
}

fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModuleId({:016x})", self.0)
    }
}

// ── LoadStatus ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LoadStatus {
    Unloaded,
    /// Currently being loaded — used to detect circular deps.
    Loading,
    Loaded,
    Error(String),
}

// ── Module ────────────────────────────────────────────────────────────────────

/// A compiled and (optionally) executed script module.
#[derive(Debug, Clone)]
pub struct Module {
    pub id:           ModuleId,
    pub name:         String,
    pub source_path:  String,
    pub chunk:        Option<Arc<Chunk>>,
    pub exports:      HashMap<String, Value>,
    pub dependencies: Vec<ModuleId>,
    pub status:       LoadStatus,
}

impl Module {
    pub fn new(name: impl Into<String>, source_path: impl Into<String>) -> Self {
        let name = name.into();
        let path = source_path.into();
        Module {
            id:           ModuleId::from_path(&path),
            name,
            source_path:  path,
            chunk:        None,
            exports:      HashMap::new(),
            dependencies: Vec::new(),
            status:       LoadStatus::Unloaded,
        }
    }

    /// Build a Value::Table from the exports map.
    pub fn exports_table(&self) -> Table {
        let t = Table::new();
        for (k, v) in &self.exports {
            t.rawset_str(k, v.clone());
        }
        t
    }
}

// ── ModuleLoader trait ────────────────────────────────────────────────────────

/// Pluggable backend for loading module source code.
pub trait ModuleLoader: Send + Sync {
    fn load_source(&self, path: &str) -> Result<String, String>;
}

// ── StringMapLoader ───────────────────────────────────────────────────────────

/// Loads modules from an in-memory map of `path -> source`.
pub struct StringMapLoader {
    pub map: HashMap<String, String>,
}

impl StringMapLoader {
    pub fn new() -> Self { StringMapLoader { map: HashMap::new() } }

    pub fn add(&mut self, path: impl Into<String>, source: impl Into<String>) {
        self.map.insert(path.into(), source.into());
    }
}

impl Default for StringMapLoader {
    fn default() -> Self { Self::new() }
}

impl ModuleLoader for StringMapLoader {
    fn load_source(&self, path: &str) -> Result<String, String> {
        self.map.get(path)
            .cloned()
            .ok_or_else(|| format!("module not found: {}", path))
    }
}

// ── ModuleRegistry ────────────────────────────────────────────────────────────

/// Maintains the full module graph with circular-dependency detection.
pub struct ModuleRegistry {
    modules: HashMap<ModuleId, Module>,
    loader:  Box<dyn ModuleLoader>,
    /// DFS coloring for cycle detection: gray = currently on stack, black = done.
    gray:    std::collections::HashSet<ModuleId>,
    black:   std::collections::HashSet<ModuleId>,
}

impl ModuleRegistry {
    pub fn new(loader: Box<dyn ModuleLoader>) -> Self {
        ModuleRegistry {
            modules: HashMap::new(),
            loader,
            gray:    std::collections::HashSet::new(),
            black:   std::collections::HashSet::new(),
        }
    }

    pub fn with_string_map(map: HashMap<String, String>) -> Self {
        let mut loader = StringMapLoader::new();
        loader.map = map;
        Self::new(Box::new(loader))
    }

    /// `require(path, vm)` — load, compile and execute a module; cache result.
    pub fn require(&mut self, path: &str, vm: &mut Vm) -> Result<Value, ScriptError> {
        let id = ModuleId::from_path(path);

        // Already loaded?
        if let Some(m) = self.modules.get(&id) {
            match &m.status {
                LoadStatus::Loaded => {
                    return Ok(Value::Table(m.exports_table()));
                }
                LoadStatus::Loading => {
                    return Err(ScriptError::new(format!(
                        "circular dependency detected for module '{}'", path
                    )));
                }
                LoadStatus::Error(e) => {
                    return Err(ScriptError::new(format!("module '{}' failed: {}", path, e)));
                }
                LoadStatus::Unloaded => {}
            }
        }

        // Cycle check
        if self.gray.contains(&id) {
            return Err(ScriptError::new(format!("circular dependency: '{}'", path)));
        }
        self.gray.insert(id);

        // Load source
        let source = self.loader.load_source(path).map_err(|e| ScriptError::new(e))?;

        // Register module as loading
        let mut module = Module::new(path, path);
        module.status  = LoadStatus::Loading;
        self.modules.insert(id, module);

        // Compile
        let chunk = match Parser::from_source(path, &source) {
            Ok(script) => Compiler::compile_script(&script),
            Err(e) => {
                if let Some(m) = self.modules.get_mut(&id) {
                    m.status = LoadStatus::Error(e.to_string());
                }
                self.gray.remove(&id);
                return Err(ScriptError::new(format!("parse error in '{}': {}", path, e)));
            }
        };

        // Execute
        let result = vm.execute(Arc::clone(&chunk));
        self.gray.remove(&id);
        self.black.insert(id);

        match result {
            Ok(vals) => {
                // The module's "exports" are whatever it returns or sets in globals
                let exports_val = vals.into_iter().next().unwrap_or(Value::Nil);
                let mut exports_map = HashMap::new();
                if let Value::Table(t) = &exports_val {
                    let mut key = Value::Nil;
                    loop {
                        match t.next(&key) {
                            Some((k, v)) => {
                                if let Value::Str(ks) = &k {
                                    exports_map.insert(ks.as_ref().clone(), v);
                                }
                                key = k;
                            }
                            None => break,
                        }
                    }
                }
                if let Some(m) = self.modules.get_mut(&id) {
                    m.chunk   = Some(chunk);
                    m.exports = exports_map;
                    m.status  = LoadStatus::Loaded;
                }
                Ok(exports_val)
            }
            Err(e) => {
                if let Some(m) = self.modules.get_mut(&id) {
                    m.status = LoadStatus::Error(e.message.clone());
                }
                Err(e)
            }
        }
    }

    /// Reload a module by path (re-compile and re-execute).
    pub fn reload(&mut self, path: &str, vm: &mut Vm) -> Result<Value, ScriptError> {
        let id = ModuleId::from_path(path);
        // Reset status to unloaded
        if let Some(m) = self.modules.get_mut(&id) {
            m.status = LoadStatus::Unloaded;
        }
        self.black.remove(&id);
        self.require(path, vm)
    }

    /// Remove a module from the cache.
    pub fn unload(&mut self, path: &str) {
        let id = ModuleId::from_path(path);
        self.modules.remove(&id);
        self.black.remove(&id);
    }

    /// List all loaded module names.
    pub fn loaded_modules(&self) -> Vec<String> {
        self.modules.values()
            .filter(|m| m.status == LoadStatus::Loaded)
            .map(|m| m.name.clone())
            .collect()
    }

    pub fn get_module(&self, path: &str) -> Option<&Module> {
        self.modules.get(&ModuleId::from_path(path))
    }

    /// ASCII-art dependency tree for a module.
    pub fn dependency_tree(&self, path: &str) -> String {
        let mut out = String::new();
        self.dep_tree_inner(path, 0, &mut std::collections::HashSet::new(), &mut out);
        out
    }

    fn dep_tree_inner(
        &self,
        path: &str,
        depth: usize,
        visited: &mut std::collections::HashSet<ModuleId>,
        out: &mut String,
    ) {
        let id  = ModuleId::from_path(path);
        let prefix = if depth == 0 { String::new() } else {
            format!("{}{}", "│  ".repeat(depth - 1), "├─ ")
        };
        out.push_str(&format!("{}{}\n", prefix, path));
        if visited.contains(&id) {
            out.push_str(&format!("{}{}  (already shown)\n", "│  ".repeat(depth), "└─"));
            return;
        }
        visited.insert(id);
        if let Some(m) = self.modules.get(&id) {
            let deps: Vec<ModuleId> = m.dependencies.clone();
            for dep_id in deps {
                // Find module name by id
                if let Some(dep_m) = self.modules.values().find(|m2| m2.id == dep_id) {
                    let dep_path = dep_m.source_path.clone();
                    self.dep_tree_inner(&dep_path, depth + 1, visited, out);
                }
            }
        }
    }
}

// ── PackageManager ────────────────────────────────────────────────────────────

/// Tries multiple search paths and file suffixes when loading modules.
pub struct PackageManager {
    pub search_paths: Vec<String>,
    pub suffixes:     Vec<String>,
    pub registry:     ModuleRegistry,
}

impl PackageManager {
    pub fn new(loader: Box<dyn ModuleLoader>) -> Self {
        PackageManager {
            search_paths: vec![String::new()],
            suffixes:     vec![".lua".to_string(), "".to_string()],
            registry:     ModuleRegistry::new(loader),
        }
    }

    /// Add a search path prefix.
    pub fn add_path(&mut self, path: impl Into<String>) {
        self.search_paths.push(path.into());
    }

    /// require(name, vm) — tries each search_path + suffix combination.
    pub fn require(&mut self, name: &str, vm: &mut Vm) -> Result<Value, ScriptError> {
        // Already cached?
        let id = ModuleId::from_path(name);
        if let Some(m) = self.registry.modules.get(&id) {
            if m.status == LoadStatus::Loaded {
                return Ok(Value::Table(m.exports_table()));
            }
        }

        // Try each candidate path
        let paths: Vec<String> = self.search_paths.iter().flat_map(|base| {
            self.suffixes.iter().map(move |suf| {
                if base.is_empty() {
                    format!("{}{}", name, suf)
                } else {
                    format!("{}/{}{}", base, name, suf)
                }
            })
        }).collect();

        for candidate in &paths {
            // Check if loader can find this
            if self.registry.loader.load_source(candidate).is_ok() {
                return self.registry.require(candidate, vm);
            }
        }
        Err(ScriptError::new(format!("module '{}' not found in path", name)))
    }

    pub fn loaded_modules(&self) -> Vec<String> {
        self.registry.loaded_modules()
    }
}

// ── Namespace ─────────────────────────────────────────────────────────────────

/// Hierarchical name lookup.  E.g. `math.sin` → table field.
pub struct Namespace {
    pub name:     String,
    pub children: HashMap<String, Namespace>,
    pub values:   HashMap<String, Value>,
}

impl Namespace {
    pub fn new(name: impl Into<String>) -> Self {
        Namespace {
            name:     name.into(),
            children: HashMap::new(),
            values:   HashMap::new(),
        }
    }

    /// Set a value at a dotted path, e.g. `"math.sin"`.
    pub fn set(&mut self, path: &str, value: Value) {
        let parts: Vec<&str> = path.splitn(2, '.').collect();
        if parts.len() == 1 {
            self.values.insert(parts[0].to_string(), value);
        } else {
            self.children
                .entry(parts[0].to_string())
                .or_insert_with(|| Namespace::new(parts[0]))
                .set(parts[1], value);
        }
    }

    /// Get a value at a dotted path.
    pub fn get(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.splitn(2, '.').collect();
        if parts.len() == 1 {
            self.values.get(parts[0])
        } else {
            self.children.get(parts[0])?.get(parts[1])
        }
    }

    /// Import all values from this namespace into a VM as globals.
    pub fn import_into(&self, vm: &mut Vm, prefix: &str) {
        for (k, v) in &self.values {
            let name = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
            vm.set_global(&name, v.clone());
        }
        for (child_name, child_ns) in &self.children {
            let new_prefix = if prefix.is_empty() {
                child_name.clone()
            } else {
                format!("{}.{}", prefix, child_name)
            };
            child_ns.import_into(vm, &new_prefix);
        }
    }

    /// Export this namespace as a table value.
    pub fn export_table(&self) -> Value {
        let t = Table::new();
        for (k, v) in &self.values {
            t.rawset_str(k, v.clone());
        }
        for (child_name, child_ns) in &self.children {
            t.rawset_str(child_name, child_ns.export_table());
        }
        Value::Table(t)
    }

    /// Merge another namespace into this one (other takes precedence).
    pub fn merge_namespaces(&mut self, other: &Namespace) {
        for (k, v) in &other.values {
            self.values.insert(k.clone(), v.clone());
        }
        for (k, child) in &other.children {
            self.children
                .entry(k.clone())
                .or_insert_with(|| Namespace::new(k))
                .merge_namespaces(child);
        }
    }
}

// ── HotReloadWatcher ─────────────────────────────────────────────────────────

/// Tracks file modification timestamps and detects changes.
/// Uses a `HashMap<String, u64>` for timestamps (simulated or real).
pub struct HotReloadWatcher {
    /// path -> last seen timestamp (or content hash)
    timestamps: HashMap<String, u64>,
    /// The registry to reload from.
    loader_snapshots: HashMap<String, String>,
}

impl HotReloadWatcher {
    pub fn new() -> Self {
        HotReloadWatcher {
            timestamps:       HashMap::new(),
            loader_snapshots: HashMap::new(),
        }
    }

    /// Register a file with its current timestamp.
    pub fn watch(&mut self, path: impl Into<String>, timestamp: u64) {
        let p = path.into();
        self.timestamps.insert(p, timestamp);
    }

    /// Update the watcher's snapshot of a file's source content.
    pub fn snapshot_source(&mut self, path: impl Into<String>, source: impl Into<String>) {
        let p = path.into();
        let s = source.into();
        // Use a simple hash as a proxy timestamp
        let ts = fnv1a(s.as_bytes());
        self.timestamps.insert(p.clone(), ts);
        self.loader_snapshots.insert(p, s);
    }

    /// Set timestamp for a path (simulates an mtime update).
    pub fn set_timestamp(&mut self, path: &str, ts: u64) {
        self.timestamps.insert(path.to_string(), ts);
    }

    /// Check which files have changed since last snapshot.
    /// Returns paths whose current timestamp differs from registered.
    pub fn check_changes(&self, current_timestamps: &HashMap<String, u64>) -> Vec<String> {
        let mut changed = Vec::new();
        for (path, &last_ts) in &self.timestamps {
            if let Some(&cur_ts) = current_timestamps.get(path) {
                if cur_ts != last_ts {
                    changed.push(path.clone());
                }
            }
        }
        changed
    }

    /// Reload any changed modules detected by comparing the given current timestamps.
    pub fn reload_changed(
        &mut self,
        current_timestamps: &HashMap<String, u64>,
        registry: &mut ModuleRegistry,
        vm: &mut Vm,
    ) -> Vec<(String, Result<(), String>)> {
        let changed = self.check_changes(current_timestamps);
        let mut results = Vec::new();
        for path in &changed {
            let res = registry.reload(path, vm)
                .map(|_| ())
                .map_err(|e| e.message);
            if res.is_ok() {
                // Update timestamp
                if let Some(&ts) = current_timestamps.get(path) {
                    self.timestamps.insert(path.clone(), ts);
                }
            }
            results.push((path.clone(), res));
        }
        results
    }

    pub fn watched_paths(&self) -> Vec<String> {
        self.timestamps.keys().cloned().collect()
    }
}

impl Default for HotReloadWatcher {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::stdlib::register_all;
    use crate::scripting::vm::Vm;

    fn make_vm() -> Vm {
        let mut vm = Vm::new();
        register_all(&mut vm);
        vm
    }

    fn string_registry(entries: &[(&str, &str)]) -> ModuleRegistry {
        let mut loader = StringMapLoader::new();
        for (k, v) in entries {
            loader.add(*k, *v);
        }
        ModuleRegistry::new(Box::new(loader))
    }

    #[test]
    fn test_module_id_stable() {
        let a = ModuleId::from_path("math.utils");
        let b = ModuleId::from_path("math.utils");
        assert_eq!(a, b);
    }

    #[test]
    fn test_module_id_different() {
        let a = ModuleId::from_path("a");
        let b = ModuleId::from_path("b");
        assert_ne!(a, b);
    }

    #[test]
    fn test_string_map_loader() {
        let mut loader = StringMapLoader::new();
        loader.add("foo", "return 42");
        assert_eq!(loader.load_source("foo").unwrap(), "return 42");
        assert!(loader.load_source("bar").is_err());
    }

    #[test]
    fn test_registry_require_simple() {
        let mut vm  = make_vm();
        let mut reg = string_registry(&[("mod", "return 99")]);
        let v = reg.require("mod", &mut vm).unwrap();
        assert_eq!(v, Value::Int(99));
    }

    #[test]
    fn test_registry_require_cached() {
        let mut vm  = make_vm();
        let mut reg = string_registry(&[("mod", "return {x=1}")]);
        let _  = reg.require("mod", &mut vm).unwrap();
        let v2 = reg.require("mod", &mut vm).unwrap();
        assert!(matches!(v2, Value::Table(_)));
    }

    #[test]
    fn test_registry_unload() {
        let mut vm  = make_vm();
        let mut reg = string_registry(&[("mod", "return 1")]);
        reg.require("mod", &mut vm).unwrap();
        reg.unload("mod");
        assert!(reg.get_module("mod").is_none());
    }

    #[test]
    fn test_registry_loaded_modules() {
        let mut vm  = make_vm();
        let mut reg = string_registry(&[("a", "return 1"), ("b", "return 2")]);
        reg.require("a", &mut vm).unwrap();
        reg.require("b", &mut vm).unwrap();
        let mods = reg.loaded_modules();
        assert_eq!(mods.len(), 2);
    }

    #[test]
    fn test_namespace_set_get() {
        let mut ns = Namespace::new("root");
        ns.set("x", Value::Int(10));
        ns.set("math.pi", Value::Float(3.14));
        assert_eq!(ns.get("x"), Some(&Value::Int(10)));
        assert!(ns.get("math.pi").is_some());
    }

    #[test]
    fn test_namespace_export_table() {
        let mut ns = Namespace::new("root");
        ns.set("a", Value::Int(1));
        ns.set("b", Value::Int(2));
        let t = ns.export_table();
        if let Value::Table(tbl) = &t {
            assert_eq!(tbl.rawget_str("a"), Value::Int(1));
        } else {
            panic!("expected table");
        }
    }

    #[test]
    fn test_namespace_merge() {
        let mut a = Namespace::new("a");
        a.set("x", Value::Int(1));
        let mut b = Namespace::new("b");
        b.set("x", Value::Int(99));
        b.set("y", Value::Int(2));
        a.merge_namespaces(&b);
        assert_eq!(a.get("x"), Some(&Value::Int(99)));
        assert_eq!(a.get("y"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_hot_reload_detect_change() {
        let mut watcher = HotReloadWatcher::new();
        watcher.watch("file.lua", 100);
        let mut current = HashMap::new();
        current.insert("file.lua".to_string(), 100u64);
        assert!(watcher.check_changes(&current).is_empty());
        current.insert("file.lua".to_string(), 200u64);
        let changed = watcher.check_changes(&current);
        assert_eq!(changed, vec!["file.lua".to_string()]);
    }

    #[test]
    fn test_hot_reload_no_change() {
        let mut watcher = HotReloadWatcher::new();
        watcher.watch("a.lua", 42);
        watcher.watch("b.lua", 43);
        let mut current = HashMap::new();
        current.insert("a.lua".to_string(), 42u64);
        current.insert("b.lua".to_string(), 43u64);
        assert!(watcher.check_changes(&current).is_empty());
    }

    #[test]
    fn test_fnv1a_hash() {
        // Verify determinism
        assert_eq!(fnv1a(b"hello"), fnv1a(b"hello"));
        assert_ne!(fnv1a(b"hello"), fnv1a(b"world"));
    }
}
