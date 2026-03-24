//! Runtime inspector, variable monitor, and in-game command registry.
//!
//! The inspector provides:
//! - `InspectorValue` — a tagged-union value type for inspector fields
//! - `Inspectable` trait — objects that can expose and receive inspector fields
//! - `RuntimeInspector` — a registry of named inspectable objects
//! - `InspectorWatcher` — polls for field changes between frames
//! - `VariableMonitor` — floating debug table of per-frame named values
//! - `CommandRegistry` — simple in-game console command registry

use std::any::Any;
use std::collections::HashMap;

// ── InspectorValue ────────────────────────────────────────────────────────────

/// A dynamically-typed value for use in the inspector UI.
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    /// RGBA color in 0–1 range.
    Color([f32; 4]),
    List(Vec<InspectorValue>),
    Map(HashMap<String, InspectorValue>),
    Enum { variant: String, variants: Vec<String> },
}

impl InspectorValue {
    /// Returns `true` if this value is a numeric type.
    pub fn is_numeric(&self) -> bool {
        matches!(self, InspectorValue::Int(_) | InspectorValue::Float(_))
    }

    /// Coerce to f64, returning `None` if not numeric.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            InspectorValue::Int(v)   => Some(*v as f64),
            InspectorValue::Float(v) => Some(*v),
            _                        => None,
        }
    }

    /// Coerce to i64, returning `None` if not numeric.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            InspectorValue::Int(v)   => Some(*v),
            InspectorValue::Float(v) => Some(*v as i64),
            _                        => None,
        }
    }

    /// Return a human-readable short representation.
    pub fn display(&self) -> String {
        match self {
            InspectorValue::Bool(v)     => v.to_string(),
            InspectorValue::Int(v)      => v.to_string(),
            InspectorValue::Float(v)    => format!("{:.4}", v),
            InspectorValue::String(v)   => v.clone(),
            InspectorValue::Vec2(v)     => format!("[{:.3}, {:.3}]", v[0], v[1]),
            InspectorValue::Vec3(v)     => format!("[{:.3}, {:.3}, {:.3}]", v[0], v[1], v[2]),
            InspectorValue::Vec4(v)     => format!("[{:.3}, {:.3}, {:.3}, {:.3}]", v[0], v[1], v[2], v[3]),
            InspectorValue::Color(v)    => format!("rgba({:.2},{:.2},{:.2},{:.2})", v[0], v[1], v[2], v[3]),
            InspectorValue::List(v)     => format!("[{}]", v.len()),
            InspectorValue::Map(v)      => format!("{{{}}}", v.len()),
            InspectorValue::Enum { variant, .. } => variant.clone(),
        }
    }

    /// Type name as a &str for display purposes.
    pub fn type_name(&self) -> &'static str {
        match self {
            InspectorValue::Bool(_)    => "bool",
            InspectorValue::Int(_)     => "int",
            InspectorValue::Float(_)   => "float",
            InspectorValue::String(_)  => "string",
            InspectorValue::Vec2(_)    => "vec2",
            InspectorValue::Vec3(_)    => "vec3",
            InspectorValue::Vec4(_)    => "vec4",
            InspectorValue::Color(_)   => "color",
            InspectorValue::List(_)    => "list",
            InspectorValue::Map(_)     => "map",
            InspectorValue::Enum { .. } => "enum",
        }
    }
}

impl Default for InspectorValue {
    fn default() -> Self { InspectorValue::Int(0) }
}

impl From<bool>   for InspectorValue { fn from(v: bool)   -> Self { InspectorValue::Bool(v) } }
impl From<i32>    for InspectorValue { fn from(v: i32)    -> Self { InspectorValue::Int(v as i64) } }
impl From<i64>    for InspectorValue { fn from(v: i64)    -> Self { InspectorValue::Int(v) } }
impl From<f32>    for InspectorValue { fn from(v: f32)    -> Self { InspectorValue::Float(v as f64) } }
impl From<f64>    for InspectorValue { fn from(v: f64)    -> Self { InspectorValue::Float(v) } }
impl From<String> for InspectorValue { fn from(v: String) -> Self { InspectorValue::String(v) } }
impl From<&str>   for InspectorValue { fn from(v: &str)   -> Self { InspectorValue::String(v.to_owned()) } }

// ── InspectorField ────────────────────────────────────────────────────────────

/// A single named field exposed by an `Inspectable` object.
#[derive(Debug, Clone)]
pub struct InspectorField {
    /// Field name / key.
    pub name:     String,
    /// Current value.
    pub value:    InspectorValue,
    /// Whether the field can be modified via `apply_changes`.
    pub editable: bool,
    /// Optional tooltip string.
    pub tooltip:  Option<String>,
    /// Numeric range (min, max) for clamping UI sliders.
    pub range:    Option<(f64, f64)>,
    /// Step size for numeric increment/decrement.
    pub step:     Option<f64>,
}

impl InspectorField {
    pub fn new(name: impl Into<String>, value: InspectorValue) -> Self {
        Self {
            name:     name.into(),
            value,
            editable: false,
            tooltip:  None,
            range:    None,
            step:     None,
        }
    }

    pub fn editable(mut self) -> Self { self.editable = true; self }
    pub fn with_tooltip(mut self, tip: impl Into<String>) -> Self { self.tooltip = Some(tip.into()); self }
    pub fn with_range(mut self, min: f64, max: f64) -> Self { self.range = Some((min, max)); self }
    pub fn with_step(mut self, step: f64) -> Self { self.step = Some(step); self }

    pub fn readonly(name: impl Into<String>, value: InspectorValue) -> Self {
        Self::new(name, value)
    }

    pub fn read_write(name: impl Into<String>, value: InspectorValue) -> Self {
        Self::new(name, value).editable()
    }
}

// ── Inspectable ───────────────────────────────────────────────────────────────

/// Objects that can expose their fields to the runtime inspector.
pub trait Inspectable {
    /// Return a list of inspector fields describing this object's current state.
    fn inspect(&self) -> Vec<InspectorField>;

    /// Apply a list of (name, new_value) changes from the inspector.
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>);
}

// ── Inspectable impls ─────────────────────────────────────────────────────────

impl Inspectable for f32 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Float(*self as f64))
            .with_step(0.01).with_range(f64::NEG_INFINITY, f64::INFINITY)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let Some(v) = val.as_f64() { *self = v as f32; }
            }
        }
    }
}

impl Inspectable for f64 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Float(*self))
            .with_step(0.001)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let Some(v) = val.as_f64() { *self = v; }
            }
        }
    }
}

impl Inspectable for i32 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Int(*self as i64))
            .with_step(1.0)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let Some(v) = val.as_i64() { *self = v as i32; }
            }
        }
    }
}

impl Inspectable for i64 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Int(*self))
            .with_step(1.0)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let Some(v) = val.as_i64() { *self = v; }
            }
        }
    }
}

impl Inspectable for bool {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Bool(*self))]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::Bool(v) = val { *self = v; }
            }
        }
    }
}

impl Inspectable for String {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::String(self.clone()))]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::String(v) = val { *self = v; }
            }
        }
    }
}

/// A 2-component float vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2([f32; 2]);

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self { Self([x, y]) }
    pub fn x(&self) -> f32 { self.0[0] }
    pub fn y(&self) -> f32 { self.0[1] }
}

impl Inspectable for Vec2 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Vec2(self.0)).with_step(0.01)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::Vec2(v) = val { self.0 = v; }
            }
        }
    }
}

/// A 3-component float vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3([f32; 3]);

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self { Self([x, y, z]) }
    pub fn x(&self) -> f32 { self.0[0] }
    pub fn y(&self) -> f32 { self.0[1] }
    pub fn z(&self) -> f32 { self.0[2] }
}

impl Inspectable for Vec3 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Vec3(self.0)).with_step(0.01)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::Vec3(v) = val { self.0 = v; }
            }
        }
    }
}

/// A 4-component float vector.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec4([f32; 4]);

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self { Self([x, y, z, w]) }
}

impl Inspectable for Vec4 {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Vec4(self.0)).with_step(0.01)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::Vec4(v) = val { self.0 = v; }
            }
        }
    }
}

impl Inspectable for [f32; 3] {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Vec3(*self)).with_step(0.01)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::Vec3(v) = val { *self = v; }
            }
        }
    }
}

impl Inspectable for [f32; 4] {
    fn inspect(&self) -> Vec<InspectorField> {
        vec![InspectorField::read_write("value", InspectorValue::Vec4(*self)).with_step(0.01)]
    }
    fn apply_changes(&mut self, changes: Vec<(String, InspectorValue)>) {
        for (name, val) in changes {
            if name == "value" {
                if let InspectorValue::Vec4(v) = val { *self = v; }
            }
        }
    }
}

// ── InspectorWatcher ──────────────────────────────────────────────────────────

/// Stores last-seen field values for a registered object and detects changes.
pub struct InspectorWatcher {
    pub object_name: String,
    last_seen: HashMap<String, InspectorValue>,
}

impl InspectorWatcher {
    pub fn new(object_name: impl Into<String>) -> Self {
        Self { object_name: object_name.into(), last_seen: HashMap::new() }
    }

    /// Update the watcher with the current fields and return a list of
    /// `(field_name, old_value, new_value)` tuples for any changed fields.
    pub fn check_changes(&mut self, current_fields: &[InspectorField])
        -> Vec<(String, InspectorValue, InspectorValue)>
    {
        let mut changes = Vec::new();

        for field in current_fields {
            match self.last_seen.get(&field.name) {
                Some(old) if old != &field.value => {
                    changes.push((field.name.clone(), old.clone(), field.value.clone()));
                    self.last_seen.insert(field.name.clone(), field.value.clone());
                }
                None => {
                    // First observation — not a "change", just record it.
                    self.last_seen.insert(field.name.clone(), field.value.clone());
                }
                _ => {}
            }
        }

        changes
    }

    /// Force-record all current fields without reporting changes.
    pub fn reset(&mut self, fields: &[InspectorField]) {
        self.last_seen.clear();
        for f in fields {
            self.last_seen.insert(f.name.clone(), f.value.clone());
        }
    }

    pub fn field_count(&self) -> usize { self.last_seen.len() }
}

// ── ObjectEntry ───────────────────────────────────────────────────────────────

/// Wraps a boxed `Any` alongside a function that can produce inspector fields
/// from it, since we can't store `dyn Inspectable` directly with `Any`.
struct ObjectEntry {
    data:    Box<dyn Any>,
    inspect: Box<dyn Fn(&dyn Any) -> Vec<InspectorField>>,
    apply:   Box<dyn Fn(&mut dyn Any, Vec<(String, InspectorValue)>)>,
}

// ── RuntimeInspector ──────────────────────────────────────────────────────────

/// Registry of named runtime objects that can be inspected and modified.
///
/// Objects are stored as type-erased `Any`; the inspector functions are
/// stored as closures that know the concrete type at registration time.
pub struct RuntimeInspector {
    objects: HashMap<String, ObjectEntry>,
    /// Insertion order for deterministic display.
    order:   Vec<String>,
}

impl RuntimeInspector {
    pub fn new() -> Self {
        Self { objects: HashMap::new(), order: Vec::new() }
    }

    /// Register an `Inspectable` object under `name`.
    ///
    /// The object is cloned into the registry.  Mutations via `apply` modify
    /// the stored copy; use `get_object::<T>()` to retrieve the current value.
    pub fn register<T>(&mut self, name: impl Into<String>, obj: T)
    where
        T: Inspectable + Any + 'static,
    {
        let name = name.into();
        if !self.objects.contains_key(&name) {
            self.order.push(name.clone());
        }
        let entry = ObjectEntry {
            data: Box::new(obj),
            inspect: Box::new(|any| {
                if let Some(v) = any.downcast_ref::<T>() {
                    v.inspect()
                } else {
                    Vec::new()
                }
            }),
            apply: Box::new(|any, changes| {
                if let Some(v) = any.downcast_mut::<T>() {
                    v.apply_changes(changes);
                }
            }),
        };
        self.objects.insert(name, entry);
    }

    /// Return the inspector fields for the named object, or `None` if not found.
    pub fn inspect(&self, name: &str) -> Option<Vec<InspectorField>> {
        self.objects.get(name).map(|e| (e.inspect)(e.data.as_ref()))
    }

    /// Apply changes to the named object.
    pub fn apply(&mut self, name: &str, changes: Vec<(String, InspectorValue)>) {
        if let Some(e) = self.objects.get_mut(name) {
            (e.apply)(e.data.as_mut(), changes);
        }
    }

    /// Create an `InspectorWatcher` for the named object.
    ///
    /// The watcher is seeded with the current field values.
    pub fn watch(&self, name: &str) -> Option<InspectorWatcher> {
        let fields = self.inspect(name)?;
        let mut watcher = InspectorWatcher::new(name);
        watcher.reset(&fields);
        Some(watcher)
    }

    /// Get a reference to the stored object as type `T`.
    pub fn get_object<T: Any>(&self, name: &str) -> Option<&T> {
        self.objects.get(name)?.data.downcast_ref::<T>()
    }

    /// Get a mutable reference to the stored object as type `T`.
    pub fn get_object_mut<T: Any>(&mut self, name: &str) -> Option<&mut T> {
        self.objects.get_mut(name)?.data.downcast_mut::<T>()
    }

    /// Remove an object from the registry.
    pub fn unregister(&mut self, name: &str) {
        self.objects.remove(name);
        self.order.retain(|n| n != name);
    }

    /// Iterate over all registered object names in insertion order.
    pub fn names(&self) -> &[String] { &self.order }

    /// Total number of registered objects.
    pub fn len(&self) -> usize { self.objects.len() }

    pub fn is_empty(&self) -> bool { self.objects.is_empty() }

    /// Render a simple text table of all registered objects and their fields.
    pub fn format_table(&self) -> String {
        let mut lines = vec!["=== Runtime Inspector ===".to_owned()];
        for name in &self.order {
            lines.push(format!("  [{}]", name));
            if let Some(fields) = self.inspect(name) {
                for f in fields {
                    let rw = if f.editable { "rw" } else { "ro" };
                    lines.push(format!("    {:20} ({:3}) = {}", f.name, rw, f.value.display()));
                }
            }
        }
        lines.join("\n")
    }
}

impl Default for RuntimeInspector {
    fn default() -> Self { Self::new() }
}

// ── VariableMonitor ───────────────────────────────────────────────────────────

/// A lightweight per-frame named-variable display.
///
/// Call `set(name, value)` each frame; `render()` produces a formatted
/// table string showing the current values.
pub struct VariableMonitor {
    vars:    HashMap<String, MonitorVar>,
    order:   Vec<String>,
    title:   String,
    max_key_width: usize,
}

#[derive(Debug, Clone)]
struct MonitorVar {
    value:    InspectorValue,
    category: Option<String>,
    color:    Option<String>,
}

impl VariableMonitor {
    pub fn new() -> Self {
        Self {
            vars:          HashMap::new(),
            order:         Vec::new(),
            title:         "Variables".to_owned(),
            max_key_width: 0,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set or update the named variable.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<InspectorValue>) {
        let name = name.into();
        let value = value.into();
        if !self.vars.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.max_key_width = self.max_key_width.max(name.len());
        self.vars.insert(name, MonitorVar { value, category: None, color: None });
    }

    /// Set a variable with a category tag.
    pub fn set_categorized(&mut self, name: impl Into<String>, value: impl Into<InspectorValue>, category: impl Into<String>) {
        let name     = name.into();
        let value    = value.into();
        let category = category.into();
        if !self.vars.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.max_key_width = self.max_key_width.max(name.len());
        self.vars.insert(name, MonitorVar { value, category: Some(category), color: None });
    }

    /// Remove a named variable.
    pub fn remove(&mut self, name: &str) {
        self.vars.remove(name);
        self.order.retain(|n| n != name);
        self.max_key_width = self.order.iter()
            .map(|n| n.len())
            .max()
            .unwrap_or(0);
    }

    /// Clear all variables.
    pub fn clear(&mut self) {
        self.vars.clear();
        self.order.clear();
        self.max_key_width = 0;
    }

    /// Format all variables as a display table.
    pub fn render(&self) -> String {
        if self.vars.is_empty() { return format!("[ {} — (empty) ]", self.title); }

        let kw = self.max_key_width.max(4);
        let total = kw + 3 + 24; // key + " │ " + value
        let border = format!("┌{}┐", "─".repeat(total + 2));
        let title  = format!("│ {:<width$} │", self.title, width = total);
        let sep    = format!("├{}┤", "─".repeat(total + 2));
        let bottom = format!("└{}┘", "─".repeat(total + 2));

        let mut lines = vec![border, title, sep];

        // Group by category
        let mut last_cat: Option<String> = None;
        for name in &self.order {
            if let Some(var) = self.vars.get(name) {
                let cat = var.category.as_deref();
                if cat != last_cat.as_deref() {
                    if let Some(c) = cat {
                        let cat_line = format!("│ {:─<width$} │", format!("─ {} ", c), width = total);
                        lines.push(cat_line);
                    }
                    last_cat = cat.map(str::to_owned);
                }
                let val_str = var.value.display();
                lines.push(format!("│ {:<kw$} │ {:<24} │", name, val_str, kw = kw));
            }
        }

        lines.push(bottom);
        lines.join("\n")
    }

    /// Number of variables registered.
    pub fn len(&self) -> usize { self.vars.len() }
    pub fn is_empty(&self) -> bool { self.vars.is_empty() }

    /// Get the current value of a named variable.
    pub fn get(&self, name: &str) -> Option<&InspectorValue> {
        self.vars.get(name).map(|v| &v.value)
    }
}

impl Default for VariableMonitor {
    fn default() -> Self { Self::new() }
}

// ── CommandRegistry ───────────────────────────────────────────────────────────

/// A simple in-game console command registry.
///
/// Commands are registered with a name, description, and a handler function
/// that accepts `Vec<String>` args and returns a `String` result (or an error
/// string).
pub struct CommandRegistry {
    commands: Vec<RegisteredCommand>,
}

struct RegisteredCommand {
    name:        String,
    description: String,
    aliases:     Vec<String>,
    handler:     Box<dyn Fn(Vec<String>) -> String + Send + Sync>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self { commands: Vec::new() };
        reg.register_builtins();
        reg
    }

    fn register_builtins(&mut self) {
        self.register(
            "help",
            "List all available commands",
            Box::new(|_args| "Available commands: help, echo, version, clear".to_owned()),
        );
        self.register(
            "echo",
            "Echo arguments back",
            Box::new(|args| args.join(" ")),
        );
        self.register(
            "version",
            "Print engine version",
            Box::new(|_args| "Proof Engine v0.1.0".to_owned()),
        );
    }

    /// Register a command with a name, description, and handler.
    pub fn register(
        &mut self,
        name:        impl Into<String>,
        description: impl Into<String>,
        handler:     Box<dyn Fn(Vec<String>) -> String + Send + Sync>,
    ) {
        let name = name.into();
        // Replace if already exists
        self.commands.retain(|c| c.name != name);
        self.commands.push(RegisteredCommand {
            name,
            description: description.into(),
            aliases: Vec::new(),
            handler,
        });
    }

    /// Add an alias for an existing command.
    pub fn add_alias(&mut self, name: &str, alias: impl Into<String>) {
        if let Some(cmd) = self.commands.iter_mut().find(|c| c.name == name) {
            cmd.aliases.push(alias.into());
        }
    }

    /// Unregister a command by name.
    pub fn unregister(&mut self, name: &str) {
        self.commands.retain(|c| c.name != name);
    }

    /// Execute a raw input string.  Parses the first token as the command name
    /// and the remainder as space-separated arguments.
    pub fn execute(&self, input: &str) -> Result<String, String> {
        let tokens = tokenize(input);
        if tokens.is_empty() { return Ok(String::new()); }

        let cmd_name = &tokens[0];
        let args: Vec<String> = tokens[1..].to_vec();

        // Match by name or alias
        let cmd = self.commands.iter().find(|c| {
            c.name == *cmd_name || c.aliases.iter().any(|a| a == cmd_name)
        });

        match cmd {
            Some(c) => Ok((c.handler)(args)),
            None    => Err(format!("Unknown command: '{}'. Type 'help' for a list.", cmd_name)),
        }
    }

    /// Return all command names that start with `prefix`, sorted.
    pub fn completions(&self, prefix: &str) -> Vec<String> {
        let mut matches: Vec<String> = self.commands.iter()
            .flat_map(|c| {
                let mut names: Vec<String> = std::iter::once(c.name.clone())
                    .chain(c.aliases.iter().cloned())
                    .collect();
                names.retain(|n| n.starts_with(prefix));
                names
            })
            .collect();
        matches.sort();
        matches.dedup();
        matches
    }

    /// Return a formatted help string for all registered commands.
    pub fn help_text(&self) -> String {
        let mut lines = Vec::new();
        for c in &self.commands {
            let aliases = if c.aliases.is_empty() {
                String::new()
            } else {
                format!(" (aliases: {})", c.aliases.join(", "))
            };
            lines.push(format!("  {:16} — {}{}", c.name, c.description, aliases));
        }
        lines.join("\n")
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize { self.commands.len() }
    pub fn is_empty(&self) -> bool { self.commands.is_empty() }
}

impl Default for CommandRegistry {
    fn default() -> Self { Self::new() }
}

/// Tokenize a command line, respecting quoted strings.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens  = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_ch = '"';

    for c in input.chars() {
        match c {
            '"' | '\'' if !in_quote => { in_quote = true; quote_ch = c; }
            c if in_quote && c == quote_ch => { in_quote = false; }
            ' ' | '\t' if !in_quote => {
                if !current.is_empty() { tokens.push(std::mem::take(&mut current)); }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() { tokens.push(current); }
    tokens
}

// ── FieldDiff ─────────────────────────────────────────────────────────────────

/// A compact summary of differences between two field snapshots.
#[derive(Debug, Clone)]
pub struct FieldDiff {
    pub field:     String,
    pub old_value: InspectorValue,
    pub new_value: InspectorValue,
}

impl FieldDiff {
    pub fn compute(old: &[InspectorField], new: &[InspectorField]) -> Vec<FieldDiff> {
        let old_map: HashMap<&str, &InspectorValue> = old.iter()
            .map(|f| (f.name.as_str(), &f.value))
            .collect();
        let mut diffs = Vec::new();
        for nf in new {
            if let Some(&ov) = old_map.get(nf.name.as_str()) {
                if ov != &nf.value {
                    diffs.push(FieldDiff {
                        field:     nf.name.clone(),
                        old_value: ov.clone(),
                        new_value: nf.value.clone(),
                    });
                }
            }
        }
        diffs
    }
}

// ── InspectorHistory ──────────────────────────────────────────────────────────

/// Records inspector field snapshots over time for undo/redo capability.
pub struct InspectorHistory {
    object_name: String,
    undo_stack:  Vec<Vec<InspectorField>>,
    redo_stack:  Vec<Vec<InspectorField>>,
    max_depth:   usize,
}

impl InspectorHistory {
    pub fn new(object_name: impl Into<String>, max_depth: usize) -> Self {
        Self {
            object_name: object_name.into(),
            undo_stack:  Vec::new(),
            redo_stack:  Vec::new(),
            max_depth:   max_depth.max(1),
        }
    }

    /// Push a snapshot before making a change.
    pub fn push_snapshot(&mut self, fields: Vec<InspectorField>) {
        self.redo_stack.clear();
        self.undo_stack.push(fields);
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    /// Pop the last snapshot for undo.
    pub fn undo(&mut self) -> Option<Vec<InspectorField>> {
        let snap = self.undo_stack.pop()?;
        Some(snap)
    }

    /// Return true if undo is available.
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }

    /// Number of undo steps available.
    pub fn undo_depth(&self) -> usize { self.undo_stack.len() }

    pub fn object_name(&self) -> &str { &self.object_name }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspector_value_display() {
        assert_eq!(InspectorValue::Bool(true).display(), "true");
        assert_eq!(InspectorValue::Int(42).display(), "42");
        assert_eq!(InspectorValue::String("hello".into()).display(), "hello");
    }

    #[test]
    fn inspector_value_as_f64() {
        assert_eq!(InspectorValue::Int(5).as_f64(), Some(5.0));
        assert_eq!(InspectorValue::Float(3.14).as_f64(), Some(3.14));
        assert_eq!(InspectorValue::Bool(true).as_f64(), None);
    }

    #[test]
    fn inspectable_f32() {
        let mut v: f32 = 1.0;
        let fields = v.inspect();
        assert_eq!(fields.len(), 1);
        assert!(fields[0].editable);
        v.apply_changes(vec![("value".to_owned(), InspectorValue::Float(2.5))]);
        assert!((v - 2.5_f32).abs() < 1e-5);
    }

    #[test]
    fn inspectable_bool() {
        let mut b = false;
        let fields = b.inspect();
        assert_eq!(fields[0].value, InspectorValue::Bool(false));
        b.apply_changes(vec![("value".to_owned(), InspectorValue::Bool(true))]);
        assert!(b);
    }

    #[test]
    fn inspectable_array3() {
        let mut arr: [f32; 3] = [1.0, 2.0, 3.0];
        let fields = arr.inspect();
        assert_eq!(fields.len(), 1);
        arr.apply_changes(vec![("value".to_owned(), InspectorValue::Vec3([4.0, 5.0, 6.0]))]);
        assert_eq!(arr, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn runtime_inspector_register_inspect() {
        let mut ri = RuntimeInspector::new();
        ri.register("speed", 42.0_f32);
        let fields = ri.inspect("speed").unwrap();
        assert_eq!(fields.len(), 1);
        if let InspectorValue::Float(v) = &fields[0].value {
            assert!((*v - 42.0).abs() < 1e-5);
        } else {
            panic!("expected Float");
        }
    }

    #[test]
    fn runtime_inspector_apply() {
        let mut ri = RuntimeInspector::new();
        ri.register("hp", 100_i32);
        ri.apply("hp", vec![("value".to_owned(), InspectorValue::Int(50))]);
        let v = ri.get_object::<i32>("hp").copied().unwrap();
        assert_eq!(v, 50);
    }

    #[test]
    fn inspector_watcher_detects_changes() {
        let mut watcher = InspectorWatcher::new("obj");
        let fields1 = vec![
            InspectorField::readonly("x", InspectorValue::Float(1.0)),
        ];
        // First call seeds the watcher; no changes returned.
        let changes = watcher.check_changes(&fields1);
        assert!(changes.is_empty(), "first check should not report changes");

        let fields2 = vec![
            InspectorField::readonly("x", InspectorValue::Float(2.0)),
        ];
        let changes = watcher.check_changes(&fields2);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "x");
    }

    #[test]
    fn variable_monitor_set_render() {
        let mut mon = VariableMonitor::new();
        mon.set("fps",    60.0_f64);
        mon.set("frames", 1234_i64);
        mon.set("name",   "test");
        let out = mon.render();
        assert!(out.contains("fps"),    "should contain fps");
        assert!(out.contains("frames"), "should contain frames");
        assert!(out.contains("test"),   "should contain value");
    }

    #[test]
    fn variable_monitor_remove() {
        let mut mon = VariableMonitor::new();
        mon.set("a", 1_i64);
        mon.set("b", 2_i64);
        mon.remove("a");
        assert_eq!(mon.len(), 1);
        assert!(mon.get("a").is_none());
    }

    #[test]
    fn command_registry_execute() {
        let reg = CommandRegistry::new();
        let result = reg.execute("echo hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn command_registry_unknown_command() {
        let reg = CommandRegistry::new();
        let result = reg.execute("unknown_cmd");
        assert!(result.is_err());
    }

    #[test]
    fn command_registry_completions() {
        let reg = CommandRegistry::new();
        let c = reg.completions("he");
        assert!(c.contains(&"help".to_owned()));
    }

    #[test]
    fn command_registry_register_custom() {
        let mut reg = CommandRegistry::new();
        reg.register("greet", "Say hello", Box::new(|args| {
            format!("Hello, {}!", args.first().map(|s| s.as_str()).unwrap_or("World"))
        }));
        let out = reg.execute("greet Alice").unwrap();
        assert_eq!(out, "Hello, Alice!");
    }

    #[test]
    fn field_diff_compute() {
        let old = vec![
            InspectorField::readonly("x", InspectorValue::Float(1.0)),
            InspectorField::readonly("y", InspectorValue::Float(2.0)),
        ];
        let new_fields = vec![
            InspectorField::readonly("x", InspectorValue::Float(3.0)),
            InspectorField::readonly("y", InspectorValue::Float(2.0)),
        ];
        let diffs = FieldDiff::compute(&old, &new_fields);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].field, "x");
    }

    #[test]
    fn inspector_history_undo() {
        let mut hist = InspectorHistory::new("obj", 10);
        let snap = vec![InspectorField::readonly("hp", InspectorValue::Int(100))];
        hist.push_snapshot(snap.clone());
        assert!(hist.can_undo());
        let restored = hist.undo().unwrap();
        assert_eq!(restored[0].name, "hp");
        assert!(!hist.can_undo());
    }
}
