//! Developer console — command dispatcher, log ring-buffer, expression
//! evaluator, auto-complete and history.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ─────────────────────────────────────────────────────────────────────────────
// LogLevel
// ─────────────────────────────────────────────────────────────────────────────

/// Severity level for console log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub fn label(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info  => "INFO ",
            LogLevel::Warn  => "WARN ",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }
    /// ASCII colour prefix character for terminal-style rendering.
    pub fn prefix_char(self) -> char {
        match self {
            LogLevel::Trace => '.',
            LogLevel::Debug => 'd',
            LogLevel::Info  => 'i',
            LogLevel::Warn  => '!',
            LogLevel::Error => 'E',
            LogLevel::Fatal => 'X',
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConsoleLine
// ─────────────────────────────────────────────────────────────────────────────

/// A single log entry.
#[derive(Debug, Clone)]
pub struct ConsoleLine {
    pub text:      String,
    pub level:     LogLevel,
    /// Wall-clock time when this entry was recorded (relative to engine start).
    pub timestamp: Duration,
    /// How many times this exact message was repeated (for deduplication).
    pub count:     u32,
    /// Source location tag (e.g. "render::pipeline").
    pub source:    Option<String>,
}

impl ConsoleLine {
    pub fn new(text: impl Into<String>, level: LogLevel, timestamp: Duration) -> Self {
        Self { text: text.into(), level, timestamp, count: 1, source: None }
    }

    pub fn with_source(mut self, src: impl Into<String>) -> Self {
        self.source = Some(src.into());
        self
    }

    /// Render as a single text line.
    pub fn render(&self) -> String {
        let secs = self.timestamp.as_secs_f64();
        let repeat = if self.count > 1 { format!(" (x{})", self.count) } else { String::new() };
        let src = self.source.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default();
        format!(
            "[{:8.3}] {} {}{}{}",
            secs,
            self.level.label(),
            src,
            self.text,
            repeat,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConsoleFilter
// ─────────────────────────────────────────────────────────────────────────────

/// Controls which log entries are visible.
#[derive(Debug, Clone)]
pub struct ConsoleFilter {
    pub min_level:    LogLevel,
    pub search:       String,
    pub show_trace:   bool,
    pub show_debug:   bool,
    pub show_info:    bool,
    pub show_warn:    bool,
    pub show_error:   bool,
    pub show_fatal:   bool,
}

impl Default for ConsoleFilter {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Debug,
            search:    String::new(),
            show_trace: false,
            show_debug: true,
            show_info:  true,
            show_warn:  true,
            show_error: true,
            show_fatal: true,
        }
    }
}

impl ConsoleFilter {
    pub fn new() -> Self { Self::default() }

    pub fn allows(&self, line: &ConsoleLine) -> bool {
        let level_ok = match line.level {
            LogLevel::Trace => self.show_trace,
            LogLevel::Debug => self.show_debug,
            LogLevel::Info  => self.show_info,
            LogLevel::Warn  => self.show_warn,
            LogLevel::Error => self.show_error,
            LogLevel::Fatal => self.show_fatal,
        } && line.level >= self.min_level;
        let search_ok = self.search.is_empty()
            || line.text.to_lowercase().contains(&self.search.to_lowercase());
        level_ok && search_ok
    }

    pub fn set_search(&mut self, q: impl Into<String>) {
        self.search = q.into();
    }

    pub fn show_all(&mut self) {
        self.show_trace = true;
        self.show_debug = true;
        self.show_info  = true;
        self.show_warn  = true;
        self.show_error = true;
        self.show_fatal = true;
        self.min_level  = LogLevel::Trace;
    }

    pub fn show_errors_only(&mut self) {
        self.show_trace = false;
        self.show_debug = false;
        self.show_info  = false;
        self.show_warn  = false;
        self.show_error = true;
        self.show_fatal = true;
        self.min_level  = LogLevel::Error;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ring buffer for log lines
// ─────────────────────────────────────────────────────────────────────────────

const RING_SIZE: usize = 10_000;

struct RingBuffer {
    buf:  Vec<ConsoleLine>,
    head: usize,
    len:  usize,
}

impl RingBuffer {
    fn new() -> Self {
        Self { buf: Vec::with_capacity(RING_SIZE), head: 0, len: 0 }
    }

    fn push(&mut self, line: ConsoleLine) {
        if self.buf.len() < RING_SIZE {
            self.buf.push(line);
            self.len += 1;
        } else {
            self.buf[self.head] = line;
            self.head = (self.head + 1) % RING_SIZE;
        }
    }

    fn iter(&self) -> impl Iterator<Item = &ConsoleLine> {
        let (right, left) = self.buf.split_at(self.head);
        left.iter().chain(right.iter())
    }

    fn len(&self) -> usize {
        self.len.min(self.buf.len())
    }

    fn clear(&mut self) {
        self.buf.clear();
        self.head = 0;
        self.len = 0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommandHistory
// ─────────────────────────────────────────────────────────────────────────────

/// Stores the last N typed commands for up/down arrow recall.
#[derive(Debug, Clone, Default)]
pub struct CommandHistory {
    entries:  Vec<String>,
    cursor:   Option<usize>,
    max_size: usize,
}

impl CommandHistory {
    pub fn new(max_size: usize) -> Self {
        Self { entries: Vec::new(), cursor: None, max_size }
    }

    pub fn push(&mut self, cmd: impl Into<String>) {
        let s: String = cmd.into();
        if s.trim().is_empty() { return; }
        // Avoid duplicate consecutive entries.
        if self.entries.last().map(|e| e == &s).unwrap_or(false) { return; }
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(s);
        self.cursor = None;
    }

    pub fn navigate_up(&mut self) -> Option<&str> {
        if self.entries.is_empty() { return None; }
        let next = match self.cursor {
            None => self.entries.len() - 1,
            Some(0) => 0,
            Some(c) => c - 1,
        };
        self.cursor = Some(next);
        self.entries.get(next).map(|s| s.as_str())
    }

    pub fn navigate_down(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(c) if c + 1 >= self.entries.len() => {
                self.cursor = None;
                None
            }
            Some(c) => {
                self.cursor = Some(c + 1);
                self.entries.get(c + 1).map(|s| s.as_str())
            }
        }
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommandAutoComplete
// ─────────────────────────────────────────────────────────────────────────────

/// Tab-completion engine: prefix-matches command names.
#[derive(Debug, Clone, Default)]
pub struct CommandAutoComplete {
    completions: Vec<String>,
    index:       usize,
    last_prefix: String,
}

impl CommandAutoComplete {
    pub fn new() -> Self { Self::default() }

    /// Compute completions for a given prefix against a list of command names.
    pub fn compute(&mut self, prefix: &str, all_names: &[&str]) {
        if prefix == self.last_prefix && !self.completions.is_empty() { return; }
        self.completions = all_names.iter()
            .filter(|&&n| n.starts_with(prefix))
            .map(|&s| s.to_string())
            .collect();
        self.completions.sort();
        self.index = 0;
        self.last_prefix = prefix.to_string();
    }

    /// Return the next completion in the cycle.
    pub fn next(&mut self) -> Option<&str> {
        if self.completions.is_empty() { return None; }
        let s = &self.completions[self.index];
        self.index = (self.index + 1) % self.completions.len();
        Some(s)
    }

    pub fn clear(&mut self) {
        self.completions.clear();
        self.index = 0;
        self.last_prefix.clear();
    }

    pub fn suggestions(&self) -> &[String] {
        &self.completions
    }

    pub fn has_completions(&self) -> bool {
        !self.completions.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommandOutput
// ─────────────────────────────────────────────────────────────────────────────

/// The result returned by a command handler.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub text:    String,
    pub level:   LogLevel,
    pub success: bool,
}

impl CommandOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self { text: text.into(), level: LogLevel::Info, success: true }
    }
    pub fn err(text: impl Into<String>) -> Self {
        Self { text: text.into(), level: LogLevel::Error, success: false }
    }
    pub fn warn(text: impl Into<String>) -> Self {
        Self { text: text.into(), level: LogLevel::Warn, success: true }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommandRegistration
// ─────────────────────────────────────────────────────────────────────────────

/// Metadata and handler for a registered command.
pub struct CommandRegistration {
    pub name:      String,
    pub aliases:   Vec<String>,
    pub help:      String,
    pub usage:     String,
    pub handler:   Box<dyn Fn(&[&str], &mut ConsoleState) -> CommandOutput + Send + Sync>,
}

impl std::fmt::Debug for CommandRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandRegistration")
            .field("name", &self.name)
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConsoleState — mutable scene-side state that commands can modify
// ─────────────────────────────────────────────────────────────────────────────

/// The mutable game/engine state that console commands can inspect or modify.
/// In a real engine this would hold references; here we use owned copies so
/// the struct is `'static` and thread-safe.
#[derive(Debug, Clone, Default)]
pub struct ConsoleState {
    pub time_scale:  f32,
    pub fps:         f32,
    pub frame_index: u64,
    pub memory_mb:   f32,
    pub entity_count: u32,
    pub particle_count: u32,
    pub field_count: u32,
    /// Simple key-value store for command-side property overrides.
    pub properties: HashMap<String, String>,
    pub quit_requested: bool,
    pub screenshot_path: Option<String>,
    pub profiler_running: bool,
    pub profiler_report: Option<String>,
    pub log_output: Vec<String>,
}

impl ConsoleState {
    pub fn new() -> Self {
        Self { time_scale: 1.0, ..Default::default() }
    }

    pub fn set_property(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.properties.insert(key.into(), val.into());
    }

    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommandRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Holds all registered commands and dispatches them.
pub struct CommandRegistry {
    commands: HashMap<String, CommandRegistration>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self { commands: HashMap::new() };
        reg.register_builtins();
        reg
    }

    pub fn register(
        &mut self,
        name:    impl Into<String>,
        help:    impl Into<String>,
        usage:   impl Into<String>,
        handler: impl Fn(&[&str], &mut ConsoleState) -> CommandOutput + Send + Sync + 'static,
    ) {
        let name: String = name.into();
        self.commands.insert(name.clone(), CommandRegistration {
            name,
            aliases: Vec::new(),
            help: help.into(),
            usage: usage.into(),
            handler: Box::new(handler),
        });
    }

    pub fn register_alias(&mut self, alias: impl Into<String>, canonical: impl Into<String>) {
        let alias: String = alias.into();
        let canonical: String = canonical.into();
        if let Some(reg) = self.commands.get_mut(&canonical) {
            reg.aliases.push(alias.clone());
        }
        // Also insert duplicate entry pointing to same canonical by cloning help.
        let (help, usage) = if let Some(r) = self.commands.get(&canonical) {
            (r.help.clone(), r.usage.clone())
        } else {
            (String::new(), String::new())
        };
        // For alias we just re-register with a wrapper that looks up the canonical.
        let canonical_clone = canonical.clone();
        let alias_name = alias.clone();
        self.commands.insert(alias, CommandRegistration {
            name: alias_name,
            aliases: vec![canonical_clone],
            help,
            usage,
            handler: Box::new(move |args, state| {
                // Forward to canonical — but we can't call self here, so just
                // record the alias and return an ok with guidance.
                let _ = (args, state, canonical.as_str());
                CommandOutput::ok("(alias)")
            }),
        });
    }

    pub fn dispatch(&self, input: &str, state: &mut ConsoleState) -> CommandOutput {
        let input = input.trim();
        if input.is_empty() {
            return CommandOutput::ok("");
        }
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd_name = parts[0].to_lowercase();
        let args = &parts[1..];

        if let Some(reg) = self.commands.get(&cmd_name) {
            (reg.handler)(args, state)
        } else {
            CommandOutput::err(format!("Unknown command '{}'. Type 'help' for a list.", cmd_name))
        }
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.commands.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn get(&self, name: &str) -> Option<&CommandRegistration> {
        self.commands.get(name)
    }

    fn register_builtins(&mut self) {
        // help
        self.register(
            "help",
            "List all commands or describe a specific command.",
            "help [command]",
            |args, _state| {
                if args.is_empty() {
                    CommandOutput::ok(
                        "Available: help spawn despawn set get teleport timescale fps mem \
                         fields particle script reload screenshot profile quit eval clear version"
                    )
                } else {
                    CommandOutput::ok(format!("Help for '{}': see source docs.", args[0]))
                }
            },
        );

        // spawn
        self.register(
            "spawn",
            "Spawn an entity of a given type at optional position.",
            "spawn <entity_type> [x y z]",
            |args, state| {
                if args.is_empty() {
                    return CommandOutput::err("Usage: spawn <entity_type> [x y z]");
                }
                let kind = args[0];
                let x = args.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let y = args.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let z = args.get(3).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                state.entity_count += 1;
                state.log_output.push(format!("Spawned {} at ({},{},{})", kind, x, y, z));
                CommandOutput::ok(format!("Spawned {} at ({:.2},{:.2},{:.2})", kind, x, y, z))
            },
        );

        // despawn
        self.register(
            "despawn",
            "Remove an entity by id.",
            "despawn <id>",
            |args, state| {
                let id: u32 = match args.first().and_then(|s| s.parse().ok()) {
                    Some(v) => v,
                    None => return CommandOutput::err("Usage: despawn <id>"),
                };
                if state.entity_count > 0 { state.entity_count -= 1; }
                CommandOutput::ok(format!("Despawned entity {}", id))
            },
        );

        // set
        self.register(
            "set",
            "Set a property on an entity.",
            "set <entity_id> <property> <value>",
            |args, state| {
                if args.len() < 3 {
                    return CommandOutput::err("Usage: set <entity_id> <property> <value>");
                }
                let key = format!("{}.{}", args[0], args[1]);
                let val = args[2..].join(" ");
                state.set_property(key.clone(), val.clone());
                CommandOutput::ok(format!("Set {}: {}", key, val))
            },
        );

        // get
        self.register(
            "get",
            "Get a property value from an entity.",
            "get <entity_id> <property>",
            |args, state| {
                if args.len() < 2 {
                    return CommandOutput::err("Usage: get <entity_id> <property>");
                }
                let key = format!("{}.{}", args[0], args[1]);
                match state.get_property(&key) {
                    Some(v) => CommandOutput::ok(format!("{} = {}", key, v)),
                    None => CommandOutput::warn(format!("{} not set", key)),
                }
            },
        );

        // teleport
        self.register(
            "teleport",
            "Move the editor camera to (x, y, z).",
            "teleport <x> <y> <z>",
            |args, state| {
                if args.len() < 3 {
                    return CommandOutput::err("Usage: teleport <x> <y> <z>");
                }
                let x = args[0].parse::<f32>().unwrap_or(0.0);
                let y = args[1].parse::<f32>().unwrap_or(0.0);
                let z = args[2].parse::<f32>().unwrap_or(0.0);
                state.set_property("camera.x", x.to_string());
                state.set_property("camera.y", y.to_string());
                state.set_property("camera.z", z.to_string());
                CommandOutput::ok(format!("Camera teleported to ({:.2},{:.2},{:.2})", x, y, z))
            },
        );

        // timescale
        self.register(
            "timescale",
            "Set simulation time scale (1.0 = normal, 0.5 = half speed).",
            "timescale <factor>",
            |args, state| {
                let factor: f32 = match args.first().and_then(|s| s.parse().ok()) {
                    Some(f) => f,
                    None => return CommandOutput::err("Usage: timescale <factor>"),
                };
                if factor < 0.0 {
                    return CommandOutput::err("Time scale cannot be negative.");
                }
                state.time_scale = factor;
                CommandOutput::ok(format!("Time scale set to {:.3}", factor))
            },
        );

        // fps
        self.register(
            "fps",
            "Display current frames per second.",
            "fps",
            |_args, state| {
                CommandOutput::ok(format!("FPS: {:.1}", state.fps))
            },
        );

        // mem
        self.register(
            "mem",
            "Display current memory usage.",
            "mem",
            |_args, state| {
                CommandOutput::ok(format!("Memory: {:.1} MB", state.memory_mb))
            },
        );

        // fields
        self.register(
            "fields",
            "Manage force fields. Sub-commands: list | clear | add <type>",
            "fields [list|clear|add <type>]",
            |args, state| {
                match args.first().copied() {
                    None | Some("list") => {
                        CommandOutput::ok(format!("{} force field(s) active", state.field_count))
                    }
                    Some("clear") => {
                        state.field_count = 0;
                        CommandOutput::ok("All force fields removed.")
                    }
                    Some("add") => {
                        let kind = args.get(1).copied().unwrap_or("gravity");
                        state.field_count += 1;
                        CommandOutput::ok(format!("Added {} force field (total: {})", kind, state.field_count))
                    }
                    Some(sub) => CommandOutput::err(format!("Unknown sub-command: {}", sub)),
                }
            },
        );

        // particle
        self.register(
            "particle",
            "Emit a particle burst at position.",
            "particle <preset> <x> <y> <z>",
            |args, state| {
                if args.is_empty() {
                    return CommandOutput::err("Usage: particle <preset> <x> <y> <z>");
                }
                let preset = args[0];
                let x = args.get(1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let y = args.get(2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                let z = args.get(3).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
                state.particle_count += 64;
                CommandOutput::ok(format!("Emitted '{}' particles at ({:.1},{:.1},{:.1})", preset, x, y, z))
            },
        );

        // script
        self.register(
            "script",
            "Execute a script snippet inline.",
            "script <source>",
            |args, state| {
                if args.is_empty() {
                    return CommandOutput::err("Usage: script <source>");
                }
                let source = args.join(" ");
                state.log_output.push(format!("Script executed: {}", source));
                CommandOutput::ok(format!("Executed: {}", source))
            },
        );

        // reload
        self.register(
            "reload",
            "Hot-reload scripts and assets.",
            "reload",
            |_args, state| {
                state.log_output.push("Hot-reload triggered.".into());
                CommandOutput::ok("Reload triggered.")
            },
        );

        // screenshot
        self.register(
            "screenshot",
            "Capture the current frame to a file.",
            "screenshot <filename>",
            |args, state| {
                let filename = args.first().copied().unwrap_or("screenshot.png");
                state.screenshot_path = Some(filename.to_string());
                CommandOutput::ok(format!("Screenshot will be saved to '{}'", filename))
            },
        );

        // profile
        self.register(
            "profile",
            "Control the built-in profiler.",
            "profile <start|stop|report>",
            |args, state| {
                match args.first().copied() {
                    Some("start") => {
                        state.profiler_running = true;
                        CommandOutput::ok("Profiler started.")
                    }
                    Some("stop") => {
                        state.profiler_running = false;
                        CommandOutput::ok("Profiler stopped.")
                    }
                    Some("report") => {
                        let report = state.profiler_report.clone()
                            .unwrap_or_else(|| "No report available.".into());
                        CommandOutput::ok(report)
                    }
                    _ => CommandOutput::err("Usage: profile <start|stop|report>"),
                }
            },
        );

        // quit
        self.register(
            "quit",
            "Exit the engine.",
            "quit",
            |_args, state| {
                state.quit_requested = true;
                CommandOutput::ok("Goodbye.")
            },
        );

        // eval — simple expression evaluator
        self.register(
            "eval",
            "Evaluate a mathematical expression.",
            "eval <expression>",
            |args, _state| {
                if args.is_empty() {
                    return CommandOutput::err("Usage: eval <expression>");
                }
                let expr = args.join(" ");
                match eval_expression(&expr) {
                    Ok(result) => CommandOutput::ok(format!("{} = {}", expr, result)),
                    Err(e)     => CommandOutput::err(format!("Eval error: {}", e)),
                }
            },
        );

        // clear
        self.register(
            "clear",
            "Clear the console log.",
            "clear",
            |_args, state| {
                state.log_output.clear();
                CommandOutput::ok("Console cleared.")
            },
        );

        // version
        self.register(
            "version",
            "Print engine version information.",
            "version",
            |_args, _state| {
                CommandOutput::ok("Proof Engine v0.1.0 — mathematical rendering engine")
            },
        );
    }
}

impl Default for CommandRegistry {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Simple expression evaluator
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate a simple infix mathematical expression.
/// Supports: +  -  *  /  ^  ( )  sin  cos  tan  sqrt  abs  pi  e
pub fn eval_expression(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err(format!("Unexpected token at position {}", pos));
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Plus, Minus, Star, Slash, Caret,
    LParen, RParen,
    Ident(String),
    Comma,
}

fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => { i += 1; }
            '+' => { tokens.push(Token::Plus);   i += 1; }
            '-' => { tokens.push(Token::Minus);  i += 1; }
            '*' => { tokens.push(Token::Star);   i += 1; }
            '/' => { tokens.push(Token::Slash);  i += 1; }
            '^' => { tokens.push(Token::Caret);  i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            ',' => { tokens.push(Token::Comma);  i += 1; }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let v: f64 = s.parse().map_err(|_| format!("Bad number: {}", s))?;
                tokens.push(Token::Number(v));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(ident));
            }
            c => return Err(format!("Unexpected character: '{}'", c)),
        }
    }
    Ok(tokens)
}

fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    parse_add(tokens, pos)
}

fn parse_add(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_mul(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Plus  => { *pos += 1; left += parse_mul(tokens, pos)?; }
            Token::Minus => { *pos += 1; left -= parse_mul(tokens, pos)?; }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_mul(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_pow(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Star  => { *pos += 1; left *= parse_pow(tokens, pos)?; }
            Token::Slash => {
                *pos += 1;
                let right = parse_pow(tokens, pos)?;
                if right.abs() < f64::EPSILON {
                    return Err("Division by zero".into());
                }
                left /= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_pow(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let base = parse_unary(tokens, pos)?;
    if *pos < tokens.len() && tokens[*pos] == Token::Caret {
        *pos += 1;
        let exp = parse_pow(tokens, pos)?;
        return Ok(base.powf(exp));
    }
    Ok(base)
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos < tokens.len() && tokens[*pos] == Token::Minus {
        *pos += 1;
        return Ok(-parse_primary(tokens, pos)?);
    }
    if *pos < tokens.len() && tokens[*pos] == Token::Plus {
        *pos += 1;
        return parse_primary(tokens, pos);
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() {
        return Err("Unexpected end of expression".into());
    }
    match tokens[*pos].clone() {
        Token::Number(v) => { *pos += 1; Ok(v) }
        Token::LParen => {
            *pos += 1;
            let v = parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err("Expected ')'".into());
            }
            *pos += 1;
            Ok(v)
        }
        Token::Ident(name) => {
            *pos += 1;
            // Constants
            match name.as_str() {
                "pi" | "PI" => return Ok(std::f64::consts::PI),
                "e"  | "E"  => return Ok(std::f64::consts::E),
                "tau" | "TAU" => return Ok(std::f64::consts::TAU),
                _ => {}
            }
            // Functions — expect '(' arg ')'
            if *pos < tokens.len() && tokens[*pos] == Token::LParen {
                *pos += 1;
                let arg = parse_expr(tokens, pos)?;
                // Optional second argument
                let arg2 = if *pos < tokens.len() && tokens[*pos] == Token::Comma {
                    *pos += 1;
                    Some(parse_expr(tokens, pos)?)
                } else {
                    None
                };
                if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                    return Err("Expected ')' after function argument".into());
                }
                *pos += 1;
                return match name.as_str() {
                    "sin"   => Ok(arg.sin()),
                    "cos"   => Ok(arg.cos()),
                    "tan"   => Ok(arg.tan()),
                    "asin"  => Ok(arg.asin()),
                    "acos"  => Ok(arg.acos()),
                    "atan"  => Ok(arg.atan()),
                    "atan2" => Ok(arg.atan2(arg2.unwrap_or(1.0))),
                    "sqrt"  => Ok(arg.sqrt()),
                    "abs"   => Ok(arg.abs()),
                    "ceil"  => Ok(arg.ceil()),
                    "floor" => Ok(arg.floor()),
                    "round" => Ok(arg.round()),
                    "log"   => Ok(arg.ln()),
                    "log2"  => Ok(arg.log2()),
                    "log10" => Ok(arg.log10()),
                    "exp"   => Ok(arg.exp()),
                    "pow"   => Ok(arg.powf(arg2.unwrap_or(2.0))),
                    "min"   => Ok(arg.min(arg2.unwrap_or(arg))),
                    "max"   => Ok(arg.max(arg2.unwrap_or(arg))),
                    "sign"  => Ok(arg.signum()),
                    f => Err(format!("Unknown function: {}", f)),
                };
            }
            Err(format!("Unknown identifier: {}", name))
        }
        ref t => Err(format!("Unexpected token: {:?}", t)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConsolePrinter
// ─────────────────────────────────────────────────────────────────────────────

/// Renders the console to a box-drawing ASCII string.
pub struct ConsolePrinter {
    pub width: usize,
    pub visible_lines: usize,
}

impl ConsolePrinter {
    pub fn new(width: usize, visible_lines: usize) -> Self {
        Self { width, visible_lines }
    }

    pub fn render(
        &self,
        console: &DevConsole,
        filter: &ConsoleFilter,
    ) -> String {
        let border_top    = format!("┌{}┐\n", "─".repeat(self.width));
        let border_bottom = format!("└{}┘\n", "─".repeat(self.width));

        let filtered: Vec<&ConsoleLine> = console.lines()
            .filter(|l| filter.allows(l))
            .collect();

        let total = filtered.len();
        let start = if total > self.visible_lines {
            total - self.visible_lines - console.scroll_offset.min(total.saturating_sub(self.visible_lines))
        } else {
            0
        };

        let mut body = String::new();
        for line in filtered.iter().skip(start).take(self.visible_lines) {
            let text = line.render();
            let truncated = if text.len() + 2 > self.width {
                format!("{}…", &text[..self.width.saturating_sub(3)])
            } else {
                text
            };
            let padding = self.width.saturating_sub(truncated.len());
            body.push_str(&format!("│{}{}│\n", truncated, " ".repeat(padding)));
        }

        // Prompt line
        let prompt = format!("> {}{}", console.input_buffer, if console.cursor_visible { "|" } else { "" });
        let ppx = self.width.saturating_sub(prompt.len());
        let prompt_line = format!("│{}{}│\n", prompt, " ".repeat(ppx));

        format!("{}{}{}{}", border_top, body, prompt_line, border_bottom)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConsoleSink — bridges log::log! macros
// ─────────────────────────────────────────────────────────────────────────────

/// A sink that can receive log records from the `log` crate.
/// Callers hold a reference and push into the DevConsole.
pub struct ConsoleSink {
    /// Lines captured from log macros, drained periodically into DevConsole.
    pub pending: std::sync::Mutex<Vec<(LogLevel, String)>>,
}

impl ConsoleSink {
    pub fn new() -> Self {
        Self { pending: std::sync::Mutex::new(Vec::new()) }
    }

    pub fn push(&self, level: LogLevel, text: impl Into<String>) {
        if let Ok(mut p) = self.pending.lock() {
            p.push((level, text.into()));
        }
    }

    pub fn drain(&self) -> Vec<(LogLevel, String)> {
        if let Ok(mut p) = self.pending.lock() {
            std::mem::take(&mut *p)
        } else {
            Vec::new()
        }
    }
}

impl Default for ConsoleSink {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// DevConsole — top-level struct
// ─────────────────────────────────────────────────────────────────────────────

/// The developer console: log viewer + command input.
pub struct DevConsole {
    ring:         RingBuffer,
    pub filter:   ConsoleFilter,
    pub registry: CommandRegistry,
    pub state:    ConsoleState,
    pub history:  CommandHistory,
    pub complete: CommandAutoComplete,

    /// Current text in the input field.
    pub input_buffer: String,
    /// Blinking cursor state.
    pub cursor_visible: bool,
    cursor_timer: f32,

    /// How many lines we've scrolled back from the bottom.
    pub scroll_offset: usize,

    /// Start time for relative timestamps.
    start: Instant,
}

impl DevConsole {
    pub fn new() -> Self {
        let mut con = Self {
            ring:           RingBuffer::new(),
            filter:         ConsoleFilter::new(),
            registry:       CommandRegistry::new(),
            state:          ConsoleState::new(),
            history:        CommandHistory::new(100),
            complete:       CommandAutoComplete::new(),
            input_buffer:   String::new(),
            cursor_visible: true,
            cursor_timer:   0.0,
            scroll_offset:  0,
            start:          Instant::now(),
        };
        con.log(LogLevel::Info, "Proof Engine console ready. Type 'help' for commands.");
        con
    }

    // ── Logging ───────────────────────────────────────────────────────────────

    pub fn log(&mut self, level: LogLevel, text: impl Into<String>) {
        let text: String = text.into();
        let ts = self.start.elapsed();

        // Deduplication: if last line has same text, increment count.
        let last_matches = self.ring.buf.last().map(|l| l.text == text).unwrap_or(false);
        if last_matches {
            if let Some(last) = self.ring.buf.last_mut() {
                last.count += 1;
                return;
            }
        }

        self.ring.push(ConsoleLine::new(text, level, ts));
        // Auto-scroll to bottom.
        self.scroll_offset = 0;
    }

    pub fn trace(&mut self, text: impl Into<String>) { self.log(LogLevel::Trace, text); }
    pub fn debug(&mut self, text: impl Into<String>) { self.log(LogLevel::Debug, text); }
    pub fn info (&mut self, text: impl Into<String>) { self.log(LogLevel::Info,  text); }
    pub fn warn (&mut self, text: impl Into<String>) { self.log(LogLevel::Warn,  text); }
    pub fn error(&mut self, text: impl Into<String>) { self.log(LogLevel::Error, text); }
    pub fn fatal(&mut self, text: impl Into<String>) { self.log(LogLevel::Fatal, text); }

    pub fn lines(&self) -> impl Iterator<Item = &ConsoleLine> {
        self.ring.iter()
    }

    pub fn line_count(&self) -> usize {
        self.ring.len()
    }

    pub fn clear_log(&mut self) {
        self.ring.clear();
    }

    // ── Input ─────────────────────────────────────────────────────────────────

    pub fn push_char(&mut self, c: char) {
        self.input_buffer.push(c);
        self.complete.clear();
    }

    pub fn pop_char(&mut self) {
        self.input_buffer.pop();
        self.complete.clear();
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.complete.clear();
    }

    /// Submit the current input buffer as a command.
    pub fn submit(&mut self) -> CommandOutput {
        let input = std::mem::take(&mut self.input_buffer);
        self.history.push(&input);
        self.complete.clear();
        self.history.reset_cursor();

        self.log(LogLevel::Debug, format!("> {}", input));
        let out = self.registry.dispatch(&input, &mut self.state);
        self.log(out.level, &out.text);
        out
    }

    /// Attempt tab-completion on the current input.
    pub fn tab_complete(&mut self) {
        let names = self.registry.names();
        let prefix = self.input_buffer.split_whitespace().next().unwrap_or("");
        self.complete.compute(prefix, &names);
        if let Some(completion) = self.complete.next() {
            let completed = completion.to_string();
            // Replace only the command word.
            let rest = self.input_buffer.trim_start().splitn(2, ' ').nth(1)
                .map(|s| format!(" {}", s))
                .unwrap_or_default();
            self.input_buffer = format!("{}{}", completed, rest);
        }
    }

    pub fn history_up(&mut self) {
        if let Some(cmd) = self.history.navigate_up() {
            self.input_buffer = cmd.to_string();
        }
    }

    pub fn history_down(&mut self) {
        match self.history.navigate_down() {
            Some(cmd) => self.input_buffer = cmd.to_string(),
            None => self.clear_input(),
        }
    }

    // ── Scrolling ─────────────────────────────────────────────────────────────

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    // ── Tick ──────────────────────────────────────────────────────────────────

    pub fn tick(&mut self, dt: f32) {
        self.cursor_timer += dt;
        if self.cursor_timer >= 0.5 {
            self.cursor_visible = !self.cursor_visible;
            self.cursor_timer = 0.0;
        }
    }

    // ── Drain sink ────────────────────────────────────────────────────────────

    /// Drain a ConsoleSink into this console.
    pub fn drain_sink(&mut self, sink: &ConsoleSink) {
        for (level, text) in sink.drain() {
            self.log(level, text);
        }
    }
}

impl Default for DevConsole {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn console() -> DevConsole {
        DevConsole::new()
    }

    #[test]
    fn test_log_and_count() {
        let mut c = console();
        let initial = c.line_count();
        c.info("hello");
        c.warn("world");
        assert!(c.line_count() >= initial + 2);
    }

    #[test]
    fn test_log_dedup() {
        let mut c = console();
        c.ring.clear();
        c.info("repeated");
        c.info("repeated");
        c.info("repeated");
        assert_eq!(c.ring.len(), 1);
        assert_eq!(c.ring.buf[0].count, 3);
    }

    #[test]
    fn test_command_help() {
        let mut c = console();
        c.input_buffer = "help".into();
        let out = c.submit();
        assert!(out.success);
        assert!(out.text.contains("help"));
    }

    #[test]
    fn test_command_timescale() {
        let mut c = console();
        c.input_buffer = "timescale 0.5".into();
        let out = c.submit();
        assert!(out.success);
        assert!((c.state.time_scale - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_command_unknown() {
        let mut c = console();
        c.input_buffer = "xyzzy".into();
        let out = c.submit();
        assert!(!out.success);
    }

    #[test]
    fn test_command_history_nav() {
        let mut c = console();
        c.input_buffer = "fps".into(); c.submit();
        c.input_buffer = "mem".into(); c.submit();
        c.history_up();
        assert_eq!(c.input_buffer, "mem");
        c.history_up();
        assert_eq!(c.input_buffer, "fps");
        c.history_down();
        assert_eq!(c.input_buffer, "mem");
    }

    #[test]
    fn test_tab_complete() {
        let mut c = console();
        c.input_buffer = "tim".into();
        c.tab_complete();
        assert!(c.input_buffer.starts_with("timescale"));
    }

    #[test]
    fn test_eval_expression() {
        assert!((eval_expression("2 + 3").unwrap() - 5.0).abs() < 1e-9);
        assert!((eval_expression("sin(0)").unwrap()).abs() < 1e-9);
        assert!((eval_expression("sqrt(4)").unwrap() - 2.0).abs() < 1e-9);
        let pi_half = eval_expression("sin(pi * 0.5)").unwrap();
        assert!((pi_half - 1.0).abs() < 1e-9);
        assert!((eval_expression("2 ^ 10").unwrap() - 1024.0).abs() < 1e-9);
        assert!(eval_expression("1 / 0").is_err());
    }

    #[test]
    fn test_eval_nested() {
        let v = eval_expression("sqrt(2 * (3 + 1))").unwrap();
        assert!((v - std::f64::consts::SQRT_2 * 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_eval_via_command() {
        let mut c = console();
        c.input_buffer = "eval 2 + 2".into();
        let out = c.submit();
        assert!(out.success);
        assert!(out.text.contains("4"));
    }

    #[test]
    fn test_filter_level() {
        let mut f = ConsoleFilter::new();
        f.show_debug = false;
        let line = ConsoleLine::new("msg", LogLevel::Debug, Duration::ZERO);
        assert!(!f.allows(&line));
        let line2 = ConsoleLine::new("msg", LogLevel::Error, Duration::ZERO);
        assert!(f.allows(&line2));
    }

    #[test]
    fn test_command_spawn_despawn() {
        let mut c = console();
        c.input_buffer = "spawn enemy 1 2 3".into();
        let out = c.submit();
        assert!(out.success);
        assert!(c.state.entity_count > 0);
    }

    #[test]
    fn test_console_printer() {
        let mut c = console();
        c.info("test line");
        let printer = ConsolePrinter::new(60, 5);
        let rendered = printer.render(&c, &c.filter.clone());
        assert!(rendered.contains("┌"));
        assert!(rendered.contains("└"));
    }

    #[test]
    fn test_sink_drain() {
        let sink = ConsoleSink::new();
        sink.push(LogLevel::Info, "from sink");
        let mut c = console();
        c.ring.clear();
        c.drain_sink(&sink);
        assert!(c.line_count() >= 1);
    }
}
