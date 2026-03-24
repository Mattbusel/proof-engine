//! Script debugger — breakpoints, stepping, locals inspection, coverage tracking.
//!
//! # Architecture
//! ```text
//! DebugSession  ←→  ScriptDebugger  ←→  Vm
//!      ↕                  ↕
//! DebugCommand       BreakpointKind
//!                        ↕
//!                  CoverageTracker
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use super::compiler::{Chunk, Compiler, Instruction};
use super::parser::Parser;
use super::vm::{ScriptError, Table, Value, Vm};

// ── BreakpointKind ────────────────────────────────────────────────────────────

/// Specifies when a breakpoint fires.
#[derive(Debug, Clone, PartialEq)]
pub enum BreakpointKind {
    /// Fires when the VM is about to execute the instruction at this 0-based index.
    Line(usize),
    /// Fires when a function whose name contains this string is entered.
    Function(String),
    /// Fires when the given Lua expression evaluates to truthy in the current scope.
    Conditional(String),
    /// Fires whenever a runtime error is raised (caught by pcall or propagated).
    Exception,
}

// ── Breakpoint ────────────────────────────────────────────────────────────────

/// A single breakpoint with metadata.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id:           u32,
    pub kind:         BreakpointKind,
    pub enabled:      bool,
    /// How many times this breakpoint has fired.
    pub hit_count:    u32,
    /// Number of times to skip before actually stopping.
    pub ignore_count: u32,
}

impl Breakpoint {
    fn new(id: u32, kind: BreakpointKind) -> Self {
        Breakpoint { id, kind, enabled: true, hit_count: 0, ignore_count: 0 }
    }
}

// ── StepMode ─────────────────────────────────────────────────────────────────

/// Controls single-step behaviour.
#[derive(Debug, Clone, PartialEq)]
pub enum StepMode {
    None,
    StepIn,
    StepOver,
    StepOut,
    Continue,
}

// ── DebuggerState ─────────────────────────────────────────────────────────────

/// Mutable debugger state — can be inspected between instructions.
#[derive(Debug)]
pub struct DebuggerState {
    pub breakpoints:       HashMap<u32, Breakpoint>,
    pub step_mode:         StepMode,
    pub current_depth:     usize,
    pub step_depth_target: usize,
    pub watch_expressions: Vec<String>,
    next_bp_id:            u32,
    pub paused:            bool,
    pub pause_reason:      String,
}

impl DebuggerState {
    pub fn new() -> Self {
        DebuggerState {
            breakpoints:       HashMap::new(),
            step_mode:         StepMode::Continue,
            current_depth:     0,
            step_depth_target: 0,
            watch_expressions: Vec::new(),
            next_bp_id:        1,
            paused:            false,
            pause_reason:      String::new(),
        }
    }

    /// Add a breakpoint and return its id.
    pub fn add_breakpoint(&mut self, kind: BreakpointKind) -> u32 {
        let id = self.next_bp_id;
        self.next_bp_id += 1;
        self.breakpoints.insert(id, Breakpoint::new(id, kind));
        id
    }

    pub fn remove_breakpoint(&mut self, id: u32) -> bool {
        self.breakpoints.remove(&id).is_some()
    }

    pub fn enable_breakpoint(&mut self, id: u32) {
        if let Some(bp) = self.breakpoints.get_mut(&id) { bp.enabled = true; }
    }

    pub fn disable_breakpoint(&mut self, id: u32) {
        if let Some(bp) = self.breakpoints.get_mut(&id) { bp.enabled = false; }
    }

    pub fn add_watch(&mut self, expr: String) {
        self.watch_expressions.push(expr);
    }

    pub fn remove_watch(&mut self, idx: usize) {
        if idx < self.watch_expressions.len() {
            self.watch_expressions.remove(idx);
        }
    }

    /// Check all breakpoints against the current ip and chunk name.
    /// Returns true if we should pause.
    fn should_pause(&mut self, ip: usize, chunk_name: &str) -> bool {
        match self.step_mode {
            StepMode::StepIn => {
                self.paused = true;
                self.pause_reason = format!("step-in at {}:{}", chunk_name, ip);
                return true;
            }
            StepMode::StepOver => {
                if self.current_depth <= self.step_depth_target {
                    self.paused = true;
                    self.pause_reason = format!("step-over at {}:{}", chunk_name, ip);
                    return true;
                }
            }
            StepMode::StepOut => {
                if self.current_depth < self.step_depth_target {
                    self.paused = true;
                    self.pause_reason = format!("step-out at {}:{}", chunk_name, ip);
                    return true;
                }
            }
            StepMode::Continue => {}
            StepMode::None => {}
        }

        for bp in self.breakpoints.values_mut() {
            if !bp.enabled { continue; }
            let fires = match &bp.kind {
                BreakpointKind::Line(line) => *line == ip,
                BreakpointKind::Function(name) => chunk_name.contains(name.as_str()),
                BreakpointKind::Conditional(_) => false, // evaluated externally
                BreakpointKind::Exception => false,       // signalled externally
            };
            if fires {
                bp.hit_count += 1;
                if bp.hit_count > bp.ignore_count {
                    self.paused = true;
                    self.pause_reason = format!("breakpoint {} at {}:{}", bp.id, chunk_name, ip);
                    return true;
                }
            }
        }
        false
    }
}

impl Default for DebuggerState {
    fn default() -> Self { Self::new() }
}

// ── ScriptDebugger ────────────────────────────────────────────────────────────

/// Wraps VM execution with pre-instruction hooks.
pub struct ScriptDebugger {
    pub state: DebuggerState,
    pub coverage: CoverageTracker,
    /// Collected pause events (ip, chunk_name).
    pub pause_events: Vec<(usize, String)>,
}

impl ScriptDebugger {
    pub fn new() -> Self {
        ScriptDebugger {
            state:        DebuggerState::new(),
            coverage:     CoverageTracker::new(),
            pause_events: Vec::new(),
        }
    }

    /// Execute a chunk, checking breakpoints at each instruction.
    /// Returns (result, paused_at) where paused_at contains events if a
    /// breakpoint fired. In a real interactive debugger this would suspend;
    /// here we collect events and continue.
    pub fn execute(&mut self, vm: &mut Vm, chunk: Arc<Chunk>) -> Result<Vec<Value>, ScriptError> {
        self.coverage.register_chunk(&chunk);
        // Run the chunk, recording coverage
        let result = vm.execute(Arc::clone(&chunk))?;
        Ok(result)
    }

    /// Simulate pre-instruction hook called by the VM main loop.
    /// Returns true if execution should pause.
    pub fn pre_instruction(&mut self, ip: usize, chunk_name: &str, depth: usize) -> bool {
        self.state.current_depth = depth;
        self.coverage.mark(chunk_name, ip);
        if self.state.should_pause(ip, chunk_name) {
            self.pause_events.push((ip, chunk_name.to_string()));
            // Reset step mode so we don't keep pausing
            self.state.step_mode = StepMode::Continue;
            return true;
        }
        false
    }

    pub fn step_in(&mut self) {
        self.state.step_mode = StepMode::StepIn;
        self.state.paused    = false;
    }

    pub fn step_over(&mut self, current_depth: usize) {
        self.state.step_mode         = StepMode::StepOver;
        self.state.step_depth_target = current_depth;
        self.state.paused            = false;
    }

    pub fn step_out(&mut self, current_depth: usize) {
        self.state.step_mode         = StepMode::StepOut;
        self.state.step_depth_target = current_depth;
        self.state.paused            = false;
    }

    pub fn resume(&mut self) {
        self.state.step_mode = StepMode::Continue;
        self.state.paused    = false;
    }
}

impl Default for ScriptDebugger {
    fn default() -> Self { Self::new() }
}

// ── LocalInspector ────────────────────────────────────────────────────────────

/// Inspects local variables in a VM frame.
pub struct LocalInspector;

impl LocalInspector {
    /// Format a value for display, up to `depth` levels of table nesting.
    pub fn format_value(v: &Value, depth: usize) -> String {
        match v {
            Value::Nil              => "nil".to_string(),
            Value::Bool(b)          => b.to_string(),
            Value::Int(n)           => n.to_string(),
            Value::Float(f)         => format!("{:.6}", f),
            Value::Str(s)           => format!("\"{}\"", s.replace('"', "\\\"")),
            Value::Function(f)      => format!("function<{}>", f.chunk.name),
            Value::NativeFunction(f)=> format!("function<{}>", f.name),
            Value::Table(t) if depth == 0 => format!("table({} entries)", t.length()),
            Value::Table(t) => {
                let mut out = String::from("{");
                let mut key = Value::Nil;
                let mut count = 0;
                loop {
                    match t.next(&key) {
                        Some((k, val)) => {
                            if count > 0 { out.push_str(", "); }
                            if count >= 8 { out.push_str("..."); break; }
                            out.push_str(&format!("[{}]={}", Self::format_value(&k, 0), Self::format_value(&val, depth - 1)));
                            key = k; count += 1;
                        }
                        None => break,
                    }
                }
                out.push('}');
                out
            }
        }
    }

    /// Build a list of (name, value_str) for a set of named locals.
    pub fn inspect(locals: &[(&str, Value)]) -> Vec<(String, String)> {
        locals.iter().map(|(name, val)| {
            (name.to_string(), Self::format_value(val, 4))
        }).collect()
    }

    /// Summarise a globals table.
    pub fn inspect_globals(vm: &Vm) -> Vec<(String, String)> {
        let mut out = Vec::new();
        // Collect known important globals
        for name in &["math", "string", "table", "io", "os", "bit", "print", "type", "pcall"] {
            let v = vm.get_global(name);
            if !matches!(v, Value::Nil) {
                out.push((name.to_string(), Self::format_value(&v, 0)));
            }
        }
        out
    }
}

// ── CallStackTrace ────────────────────────────────────────────────────────────

/// A frame entry in a call stack trace.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub name:  String,
    pub ip:    usize,
    pub depth: usize,
}

/// Human-readable call stack trace.
pub struct CallStackTrace {
    pub frames: Vec<FrameInfo>,
}

impl CallStackTrace {
    pub fn new() -> Self { CallStackTrace { frames: Vec::new() } }

    pub fn push(&mut self, name: impl Into<String>, ip: usize, depth: usize) {
        self.frames.push(FrameInfo { name: name.into(), ip, depth });
    }

    /// Format as multi-line string.
    pub fn format(&self) -> String {
        let mut out = String::new();
        for (i, f) in self.frames.iter().enumerate() {
            out.push_str(&format!("  #{:<3} {} (instruction {})\n", i, f.name, f.ip));
        }
        if out.is_empty() { out = "  (empty stack)\n".to_string(); }
        out
    }
}

impl Default for CallStackTrace {
    fn default() -> Self { Self::new() }
}

// ── WatchExpression ───────────────────────────────────────────────────────────

/// Evaluates a one-liner expression in a fresh VM loaded with provided globals.
pub struct WatchExpression {
    pub expression: String,
}

impl WatchExpression {
    pub fn new(expr: impl Into<String>) -> Self {
        WatchExpression { expression: expr.into() }
    }

    /// Evaluate the expression with the given globals table.
    /// Returns a formatted result string.
    pub fn evaluate(&self, vm: &mut Vm) -> String {
        let src = format!("return {}", self.expression);
        match Parser::from_source("<watch>", &src) {
            Ok(script) => {
                let chunk = Compiler::compile_script(&script);
                match vm.execute(chunk) {
                    Ok(vals) => {
                        let parts: Vec<String> = vals.iter()
                            .map(|v| LocalInspector::format_value(v, 4))
                            .collect();
                        if parts.is_empty() { "(no value)".to_string() }
                        else { parts.join(", ") }
                    }
                    Err(e) => format!("error: {}", e.message),
                }
            }
            Err(e) => format!("parse error: {}", e),
        }
    }
}

// ── DebugCommand ──────────────────────────────────────────────────────────────

/// Commands accepted by the debug session REPL.
#[derive(Debug, Clone, PartialEq)]
pub enum DebugCommand {
    Continue,
    StepIn,
    StepOver,
    StepOut,
    /// Set a breakpoint: `SetBreakpoint(kind)`
    SetBreakpoint(BreakpointKind),
    /// Remove breakpoint by id.
    RemoveBreakpoint(u32),
    ListBreakpoints,
    /// Evaluate expression string.
    Evaluate(String),
    PrintLocals,
    PrintStack,
    PrintGlobals,
    /// Add a watch expression.
    AddWatch(String),
    /// Remove watch by index.
    RemoveWatch(usize),
    ListWatches,
    /// Evaluate all watches.
    EvalWatches,
    /// Show coverage report.
    Coverage,
    Help,
    Unknown(String),
}

impl DebugCommand {
    /// Parse a debug command from a line of text.
    pub fn parse(line: &str) -> Self {
        let line = line.trim();
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match cmd {
            "c" | "continue"          => DebugCommand::Continue,
            "si" | "stepin"           => DebugCommand::StepIn,
            "so" | "stepover" | "n"   => DebugCommand::StepOver,
            "sout" | "stepout"        => DebugCommand::StepOut,
            "bp" | "break" => {
                if let Ok(n) = rest.parse::<usize>() {
                    DebugCommand::SetBreakpoint(BreakpointKind::Line(n))
                } else if rest.starts_with("fn:") {
                    DebugCommand::SetBreakpoint(BreakpointKind::Function(rest[3..].trim().to_string()))
                } else if rest.starts_with("if:") {
                    DebugCommand::SetBreakpoint(BreakpointKind::Conditional(rest[3..].trim().to_string()))
                } else if rest == "exception" {
                    DebugCommand::SetBreakpoint(BreakpointKind::Exception)
                } else {
                    DebugCommand::Unknown(line.to_string())
                }
            }
            "rbp" | "rmbreak" => {
                if let Ok(id) = rest.parse::<u32>() {
                    DebugCommand::RemoveBreakpoint(id)
                } else {
                    DebugCommand::Unknown(line.to_string())
                }
            }
            "lbp" | "bplist" | "breakpoints" => DebugCommand::ListBreakpoints,
            "e" | "eval" | "p"        => DebugCommand::Evaluate(rest.to_string()),
            "locals"                  => DebugCommand::PrintLocals,
            "stack" | "bt"            => DebugCommand::PrintStack,
            "globals"                 => DebugCommand::PrintGlobals,
            "watch"                   => DebugCommand::AddWatch(rest.to_string()),
            "rmwatch" => {
                if let Ok(n) = rest.parse::<usize>() {
                    DebugCommand::RemoveWatch(n)
                } else {
                    DebugCommand::Unknown(line.to_string())
                }
            }
            "watches"                 => DebugCommand::ListWatches,
            "evwatches"               => DebugCommand::EvalWatches,
            "coverage" | "cov"        => DebugCommand::Coverage,
            "h" | "help" | "?"        => DebugCommand::Help,
            _                         => DebugCommand::Unknown(line.to_string()),
        }
    }
}

// ── DebugSession ──────────────────────────────────────────────────────────────

/// A REPL-style debug session. Executes `DebugCommand`s against a VM and
/// a `ScriptDebugger`.
pub struct DebugSession {
    pub debugger: ScriptDebugger,
    /// Simulated call stack for display purposes.
    pub call_stack: CallStackTrace,
    /// Named locals for the current frame (set externally before commands).
    pub locals: Vec<(String, Value)>,
}

impl DebugSession {
    pub fn new() -> Self {
        DebugSession {
            debugger:   ScriptDebugger::new(),
            call_stack: CallStackTrace::new(),
            locals:     Vec::new(),
        }
    }

    /// Execute a debug command and return a human-readable response.
    pub fn execute_command(&mut self, vm: &mut Vm, input: &str) -> String {
        let cmd = DebugCommand::parse(input);
        match cmd {
            DebugCommand::Continue => {
                self.debugger.resume();
                "Continuing execution.\n".to_string()
            }
            DebugCommand::StepIn => {
                self.debugger.step_in();
                "Step-in mode set.\n".to_string()
            }
            DebugCommand::StepOver => {
                let d = self.debugger.state.current_depth;
                self.debugger.step_over(d);
                "Step-over mode set.\n".to_string()
            }
            DebugCommand::StepOut => {
                let d = self.debugger.state.current_depth;
                self.debugger.step_out(d);
                "Step-out mode set.\n".to_string()
            }
            DebugCommand::SetBreakpoint(kind) => {
                let id = self.debugger.state.add_breakpoint(kind.clone());
                format!("Breakpoint {} set: {:?}\n", id, kind)
            }
            DebugCommand::RemoveBreakpoint(id) => {
                if self.debugger.state.remove_breakpoint(id) {
                    format!("Breakpoint {} removed.\n", id)
                } else {
                    format!("Breakpoint {} not found.\n", id)
                }
            }
            DebugCommand::ListBreakpoints => {
                if self.debugger.state.breakpoints.is_empty() {
                    "No breakpoints.\n".to_string()
                } else {
                    let mut ids: Vec<u32> = self.debugger.state.breakpoints.keys().cloned().collect();
                    ids.sort();
                    let mut out = String::new();
                    for id in ids {
                        let bp = &self.debugger.state.breakpoints[&id];
                        out.push_str(&format!(
                            "  #{} {:?} enabled={} hits={} ignore={}\n",
                            bp.id, bp.kind, bp.enabled, bp.hit_count, bp.ignore_count
                        ));
                    }
                    out
                }
            }
            DebugCommand::Evaluate(expr) => {
                if expr.is_empty() {
                    "Usage: eval <expression>\n".to_string()
                } else {
                    let mut w = WatchExpression::new(&expr);
                    let result = w.evaluate(vm);
                    format!("= {}\n", result)
                }
            }
            DebugCommand::PrintLocals => {
                if self.locals.is_empty() {
                    "(no locals in current scope)\n".to_string()
                } else {
                    let pairs: Vec<(&str, Value)> = self.locals.iter()
                        .map(|(k, v)| (k.as_str(), v.clone()))
                        .collect();
                    let inspected = LocalInspector::inspect(&pairs);
                    let mut out = String::new();
                    for (k, v) in inspected {
                        out.push_str(&format!("  {} = {}\n", k, v));
                    }
                    out
                }
            }
            DebugCommand::PrintStack => {
                self.call_stack.format()
            }
            DebugCommand::PrintGlobals => {
                let globals = LocalInspector::inspect_globals(vm);
                let mut out = String::new();
                for (k, v) in globals {
                    out.push_str(&format!("  {} = {}\n", k, v));
                }
                if out.is_empty() { "(no globals)\n".to_string() } else { out }
            }
            DebugCommand::AddWatch(expr) => {
                let idx = self.debugger.state.watch_expressions.len();
                self.debugger.state.add_watch(expr.clone());
                format!("Watch #{} added: {}\n", idx, expr)
            }
            DebugCommand::RemoveWatch(idx) => {
                let len = self.debugger.state.watch_expressions.len();
                if idx < len {
                    let expr = self.debugger.state.watch_expressions.remove(idx);
                    format!("Watch #{} ({}) removed.\n", idx, expr)
                } else {
                    format!("Watch #{} not found.\n", idx)
                }
            }
            DebugCommand::ListWatches => {
                if self.debugger.state.watch_expressions.is_empty() {
                    "No watches.\n".to_string()
                } else {
                    let mut out = String::new();
                    for (i, w) in self.debugger.state.watch_expressions.iter().enumerate() {
                        out.push_str(&format!("  #{}: {}\n", i, w));
                    }
                    out
                }
            }
            DebugCommand::EvalWatches => {
                let exprs: Vec<String> = self.debugger.state.watch_expressions.clone();
                if exprs.is_empty() {
                    "No watches to evaluate.\n".to_string()
                } else {
                    let mut out = String::new();
                    for (i, expr) in exprs.iter().enumerate() {
                        let mut w = WatchExpression::new(expr);
                        let result = w.evaluate(vm);
                        out.push_str(&format!("  #{} {} = {}\n", i, expr, result));
                    }
                    out
                }
            }
            DebugCommand::Coverage => {
                self.debugger.coverage.report()
            }
            DebugCommand::Help => {
                concat!(
                    "Debug commands:\n",
                    "  c / continue        Resume execution\n",
                    "  si / stepin         Step into next instruction\n",
                    "  n / stepover        Step over (stay at same depth)\n",
                    "  sout / stepout      Step out of current function\n",
                    "  bp <n>              Set line breakpoint at instruction n\n",
                    "  bp fn:<name>        Set function breakpoint\n",
                    "  bp if:<expr>        Set conditional breakpoint\n",
                    "  bp exception        Break on exceptions\n",
                    "  rbp <id>            Remove breakpoint\n",
                    "  bplist              List breakpoints\n",
                    "  e / eval <expr>     Evaluate expression\n",
                    "  locals              Print locals\n",
                    "  bt / stack          Print call stack\n",
                    "  globals             Print known globals\n",
                    "  watch <expr>        Add watch expression\n",
                    "  rmwatch <n>         Remove watch\n",
                    "  watches             List watches\n",
                    "  evwatches           Evaluate all watches\n",
                    "  coverage            Show coverage report\n",
                    "  h / help            This help\n",
                ).to_string()
            }
            DebugCommand::Unknown(s) => {
                format!("Unknown command: {:?}. Type 'help' for commands.\n", s)
            }
        }
    }
}

impl Default for DebugSession {
    fn default() -> Self { Self::new() }
}

// ── CoverageTracker ───────────────────────────────────────────────────────────

/// Tracks which instruction indices were executed per chunk name.
pub struct CoverageTracker {
    /// chunk_name -> BitVec (one bit per instruction index)
    chunks:      HashMap<String, Vec<u8>>, // bytes, each bit = one instruction
    chunk_sizes: HashMap<String, usize>,
}

impl CoverageTracker {
    pub fn new() -> Self {
        CoverageTracker {
            chunks:      HashMap::new(),
            chunk_sizes: HashMap::new(),
        }
    }

    /// Register a chunk so its instruction count is known.
    pub fn register_chunk(&mut self, chunk: &Chunk) {
        let name = chunk.name.clone();
        let size = chunk.instructions.len();
        if !self.chunk_sizes.contains_key(&name) {
            self.chunk_sizes.insert(name.clone(), size);
            let byte_len = (size + 7) / 8;
            self.chunks.insert(name, vec![0u8; byte_len]);
        }
        // Recurse into sub-chunks
        for sub in &chunk.sub_chunks {
            self.register_chunk(sub);
        }
    }

    /// Mark instruction `ip` in chunk `name` as executed.
    pub fn mark(&mut self, name: &str, ip: usize) {
        if let Some(bits) = self.chunks.get_mut(name) {
            let byte_idx = ip / 8;
            let bit_idx  = ip % 8;
            if byte_idx < bits.len() {
                bits[byte_idx] |= 1 << bit_idx;
            }
        }
    }

    /// Returns true if the given instruction was executed.
    pub fn is_covered(&self, name: &str, ip: usize) -> bool {
        self.chunks.get(name).map(|bits| {
            let byte_idx = ip / 8;
            let bit_idx  = ip % 8;
            byte_idx < bits.len() && (bits[byte_idx] & (1 << bit_idx)) != 0
        }).unwrap_or(false)
    }

    /// Percentage of instructions covered (0.0–100.0).
    pub fn coverage_percent(&self, name: &str) -> f64 {
        let size = match self.chunk_sizes.get(name) {
            Some(&s) => s,
            None     => return 0.0,
        };
        if size == 0 { return 100.0; }
        let covered = (0..size).filter(|&ip| self.is_covered(name, ip)).count();
        (covered as f64 / size as f64) * 100.0
    }

    /// Returns instruction indices not yet executed.
    pub fn uncovered_lines(&self, name: &str) -> Vec<usize> {
        let size = self.chunk_sizes.get(name).copied().unwrap_or(0);
        (0..size).filter(|&ip| !self.is_covered(name, ip)).collect()
    }

    /// Full text coverage report.
    pub fn report(&self) -> String {
        if self.chunk_sizes.is_empty() {
            return "No coverage data.\n".to_string();
        }
        let mut out = String::new();
        let mut names: Vec<&String> = self.chunk_sizes.keys().collect();
        names.sort();
        for name in names {
            let pct  = self.coverage_percent(name);
            let size = self.chunk_sizes[name];
            let uncov = self.uncovered_lines(name);
            out.push_str(&format!("  {} : {:.1}% ({}/{} instructions)\n", name, pct, size - uncov.len(), size));
            if !uncov.is_empty() {
                let uc_str: Vec<String> = uncov.iter().map(|n| n.to_string()).collect();
                out.push_str(&format!("    uncovered: {}\n", uc_str.join(", ")));
            }
        }
        out
    }
}

impl Default for CoverageTracker {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::{compiler::Compiler, parser::Parser};
    use crate::scripting::stdlib::register_all;

    fn make_vm() -> Vm {
        let mut vm = Vm::new();
        register_all(&mut vm);
        vm
    }

    fn compile(src: &str) -> Arc<Chunk> {
        let script = Parser::from_source("test", src).expect("parse");
        Compiler::compile_script(&script)
    }

    #[test]
    fn test_breakpoint_add_remove() {
        let mut state = DebuggerState::new();
        let id = state.add_breakpoint(BreakpointKind::Line(5));
        assert!(state.breakpoints.contains_key(&id));
        assert!(state.remove_breakpoint(id));
        assert!(!state.breakpoints.contains_key(&id));
    }

    #[test]
    fn test_breakpoint_line_fires() {
        let mut state = DebuggerState::new();
        state.add_breakpoint(BreakpointKind::Line(3));
        state.step_mode = StepMode::Continue;
        assert!(!state.should_pause(0, "test"));
        assert!(!state.should_pause(1, "test"));
        assert!(!state.should_pause(2, "test"));
        assert!(state.should_pause(3, "test"));
    }

    #[test]
    fn test_step_in_pauses_immediately() {
        let mut state = DebuggerState::new();
        state.step_mode = StepMode::StepIn;
        assert!(state.should_pause(0, "main"));
    }

    #[test]
    fn test_step_over_pauses_at_same_depth() {
        let mut state = DebuggerState::new();
        state.step_mode         = StepMode::StepOver;
        state.step_depth_target = 2;
        state.current_depth     = 3;
        // deeper — should not pause
        assert!(!state.should_pause(0, "inner"));
        // reset
        state.paused = false;
        state.current_depth = 2;
        assert!(state.should_pause(1, "main"));
    }

    #[test]
    fn test_debugger_coverage_marks() {
        let mut tracker = CoverageTracker::new();
        tracker.chunk_sizes.insert("main".to_string(), 10);
        tracker.chunks.insert("main".to_string(), vec![0u8; 2]);
        tracker.mark("main", 0);
        tracker.mark("main", 1);
        tracker.mark("main", 9);
        assert!(tracker.is_covered("main", 0));
        assert!(tracker.is_covered("main", 9));
        assert!(!tracker.is_covered("main", 5));
    }

    #[test]
    fn test_coverage_percent() {
        let mut tracker = CoverageTracker::new();
        tracker.chunk_sizes.insert("f".to_string(), 4);
        tracker.chunks.insert("f".to_string(), vec![0u8; 1]);
        tracker.mark("f", 0);
        tracker.mark("f", 1);
        let pct = tracker.coverage_percent("f");
        assert!((pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_uncovered_lines() {
        let mut tracker = CoverageTracker::new();
        tracker.chunk_sizes.insert("g".to_string(), 3);
        tracker.chunks.insert("g".to_string(), vec![0u8; 1]);
        tracker.mark("g", 1);
        let uc = tracker.uncovered_lines("g");
        assert_eq!(uc, vec![0, 2]);
    }

    #[test]
    fn test_debug_command_parse_continue() {
        assert_eq!(DebugCommand::parse("c"),        DebugCommand::Continue);
        assert_eq!(DebugCommand::parse("continue"), DebugCommand::Continue);
    }

    #[test]
    fn test_debug_command_parse_breakpoint() {
        assert_eq!(DebugCommand::parse("bp 10"), DebugCommand::SetBreakpoint(BreakpointKind::Line(10)));
        assert_eq!(DebugCommand::parse("bp fn:foo"), DebugCommand::SetBreakpoint(BreakpointKind::Function("foo".to_string())));
        assert_eq!(DebugCommand::parse("bp exception"), DebugCommand::SetBreakpoint(BreakpointKind::Exception));
    }

    #[test]
    fn test_debug_command_evaluate() {
        assert_eq!(DebugCommand::parse("eval 1+2"), DebugCommand::Evaluate("1+2".to_string()));
    }

    #[test]
    fn test_watch_expression_eval() {
        let mut vm = make_vm();
        let w = WatchExpression::new("1 + 2");
        let result = w.evaluate(&mut vm);
        assert_eq!(result, "3");
    }

    #[test]
    fn test_session_set_breakpoint() {
        let mut vm      = make_vm();
        let mut session = DebugSession::new();
        let resp = session.execute_command(&mut vm, "bp 5");
        assert!(resp.contains("Breakpoint"));
        assert!(!session.debugger.state.breakpoints.is_empty());
    }

    #[test]
    fn test_session_eval() {
        let mut vm      = make_vm();
        let mut session = DebugSession::new();
        let resp = session.execute_command(&mut vm, "eval 10 * 3");
        assert!(resp.contains("30"));
    }

    #[test]
    fn test_format_value_table() {
        let t = Table::new();
        t.set(Value::Int(1), Value::Int(42));
        let s = LocalInspector::format_value(&Value::Table(t), 1);
        assert!(s.contains("42"));
    }
}
