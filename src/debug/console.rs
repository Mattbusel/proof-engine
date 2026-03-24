//! In-engine debug console with command registration, tab completion, and history.
//!
//! The console renders as an overlay on top of the scene using glyph rendering.
//! It supports:
//! - Command registration with argument schemas and help text
//! - Tab completion for commands and arguments
//! - Command history (up/down arrow navigation)
//! - Built-in commands: help, list, set, get, clear, echo, quit, reload, time, fps
//! - Structured output log with severity levels and scroll
//! - Lua-style expression evaluation stubs for future scripting integration
//!
//! ## Quick Start
//! ```rust,no_run
//! use proof_engine::debug::console::Console;
//! let mut console = Console::new();
//! console.register_command(proof_engine::debug::console::Command {
//!     name:     "say".to_owned(),
//!     help:     "Print a message".to_owned(),
//!     args:     vec![proof_engine::debug::console::ArgSpec::required("text")],
//!     handler:  Box::new(|args, out| {
//!         out.push(proof_engine::debug::console::LogLine::info(args.join(" ")));
//!     }),
//! });
//! console.submit("say hello world");
//! ```

use std::collections::VecDeque;
use glam::{Vec3, Vec4};

// ── LogLine ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel { Info, Warn, Error, Success, Debug, Command }

#[derive(Debug, Clone)]
pub struct LogLine {
    pub level:   LogLevel,
    pub text:    String,
    pub color:   Vec4,
}

impl LogLine {
    pub fn new(level: LogLevel, text: impl Into<String>) -> Self {
        let color = match level {
            LogLevel::Info    => Vec4::new(0.9, 0.9, 0.9, 1.0),
            LogLevel::Warn    => Vec4::new(1.0, 0.85, 0.2, 1.0),
            LogLevel::Error   => Vec4::new(1.0, 0.3, 0.3, 1.0),
            LogLevel::Success => Vec4::new(0.3, 1.0, 0.5, 1.0),
            LogLevel::Debug   => Vec4::new(0.6, 0.6, 1.0, 1.0),
            LogLevel::Command => Vec4::new(0.5, 0.8, 1.0, 1.0),
        };
        Self { level, text: text.into(), color }
    }

    pub fn info(text: impl Into<String>)    -> Self { Self::new(LogLevel::Info,    text) }
    pub fn warn(text: impl Into<String>)    -> Self { Self::new(LogLevel::Warn,    text) }
    pub fn error(text: impl Into<String>)   -> Self { Self::new(LogLevel::Error,   text) }
    pub fn success(text: impl Into<String>) -> Self { Self::new(LogLevel::Success, text) }
    pub fn debug(text: impl Into<String>)   -> Self { Self::new(LogLevel::Debug,   text) }
    pub fn command(text: impl Into<String>) -> Self { Self::new(LogLevel::Command, text) }
}

// ── ArgSpec ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub name:        String,
    pub required:    bool,
    pub default:     Option<String>,
    pub completions: Vec<String>,
    pub description: String,
}

impl ArgSpec {
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
            default: None,
            completions: Vec::new(),
            description: String::new(),
        }
    }

    pub fn optional(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: false,
            default: Some(default.into()),
            completions: Vec::new(),
            description: String::new(),
        }
    }

    pub fn with_completions(mut self, opts: Vec<String>) -> Self {
        self.completions = opts;
        self
    }

    pub fn with_description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }
}

// ── Command ───────────────────────────────────────────────────────────────────

pub type HandlerFn = Box<dyn Fn(&[&str], &mut Vec<LogLine>) + Send + Sync>;

pub struct Command {
    pub name:    String,
    pub help:    String,
    pub args:    Vec<ArgSpec>,
    pub handler: HandlerFn,
    pub aliases: Vec<String>,
    pub hidden:  bool,
}

impl Command {
    pub fn new(
        name:    impl Into<String>,
        help:    impl Into<String>,
        handler: HandlerFn,
    ) -> Self {
        Self {
            name: name.into(),
            help: help.into(),
            args: Vec::new(),
            handler,
            aliases: Vec::new(),
            hidden: false,
        }
    }

    pub fn with_args(mut self, args: Vec<ArgSpec>) -> Self { self.args = args; self }
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self { self.aliases.push(alias.into()); self }
    pub fn hidden(mut self) -> Self { self.hidden = true; self }

    pub fn usage(&self) -> String {
        let mut s = self.name.clone();
        for arg in &self.args {
            if arg.required {
                s.push_str(&format!(" <{}>", arg.name));
            } else {
                let def = arg.default.as_deref().unwrap_or("...");
                s.push_str(&format!(" [{}={}]", arg.name, def));
            }
        }
        s
    }
}

// ── CompletionResult ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub completions:   Vec<String>,
    pub common_prefix: String,
}

impl CompletionResult {
    pub fn empty() -> Self { Self { completions: Vec::new(), common_prefix: String::new() } }

    fn from_list(prefix: &str, candidates: &[impl AsRef<str>]) -> Self {
        let matching: Vec<String> = candidates.iter()
            .filter(|c| c.as_ref().starts_with(prefix))
            .map(|c| c.as_ref().to_owned())
            .collect();
        if matching.is_empty() {
            return Self::empty();
        }
        let common = common_prefix(&matching);
        Self { completions: matching, common_prefix: common }
    }
}

fn common_prefix(strings: &[String]) -> String {
    if strings.is_empty() { return String::new(); }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        len = first.chars().zip(s.chars()).take(len).take_while(|(a, b)| a == b).count();
    }
    first[..len].to_owned()
}

// ── Console ───────────────────────────────────────────────────────────────────

/// Maximum lines kept in the output log.
const MAX_LOG_LINES:     usize = 1024;
/// Maximum history entries.
const MAX_HISTORY:       usize = 200;
/// Number of visible log lines in the console window.
const VISIBLE_LOG_LINES: usize = 20;
/// Maximum input line length.
const MAX_INPUT_LEN:     usize = 512;

pub struct Console {
    /// Whether the console window is visible.
    pub visible:          bool,
    /// Current input buffer.
    pub input:            String,
    /// Cursor position in the input buffer (byte offset).
    pub cursor:           usize,
    /// Output log (newest last).
    pub log:              VecDeque<LogLine>,
    /// Scroll offset: 0 = bottom (newest), N = scroll N lines up.
    pub scroll_offset:    usize,
    /// Command history, oldest first.
    history:              VecDeque<String>,
    /// Current history navigation index. None = not navigating.
    history_index:        Option<usize>,
    /// Saved input line while browsing history.
    history_saved:        String,
    /// Registered commands.
    commands:             Vec<Command>,
    /// Pending tab completion results.
    pending_completion:   Option<CompletionResult>,
    /// Ambient variable store (set/get).
    vars:                 std::collections::HashMap<String, String>,
}

impl Console {
    pub fn new() -> Self {
        let mut console = Self {
            visible:           false,
            input:             String::new(),
            cursor:            0,
            log:               VecDeque::new(),
            scroll_offset:     0,
            history:           VecDeque::new(),
            history_index:     None,
            history_saved:     String::new(),
            commands:          Vec::new(),
            pending_completion: None,
            vars:              std::collections::HashMap::new(),
        };
        console.register_builtins();
        console
    }

    // ── Registration ─────────────────────────────────────────────────────────

    pub fn register_command(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    fn register_builtins(&mut self) {
        self.commands.push(Command::new("help", "Show help for a command or list all commands",
            Box::new(|args, out| {
                if args.is_empty() {
                    out.push(LogLine::info("Type 'list' to see all commands. 'help <command>' for details."));
                } else {
                    out.push(LogLine::info(format!("Help for '{}': (use 'list' to see all)", args[0])));
                }
            })
        ).with_args(vec![ArgSpec::optional("command", "")]));

        self.commands.push(Command::new("list", "List all registered commands",
            Box::new(|_args, out| {
                out.push(LogLine::info("Available commands: help, list, set, get, clear, echo, history, reload, version"));
            })
        ));

        self.commands.push(Command::new("clear", "Clear the console output log",
            Box::new(|_args, out| {
                out.push(LogLine::new(LogLevel::Debug, "CLEAR"));
            })
        ));

        self.commands.push(Command::new("echo", "Print arguments to the console",
            Box::new(|args, out| {
                out.push(LogLine::info(args.join(" ")));
            })
        ).with_args(vec![ArgSpec::required("text")]));

        self.commands.push(Command::new("set", "Set a console variable: set <name> <value>",
            Box::new(|args, out| {
                if args.len() < 2 {
                    out.push(LogLine::warn("Usage: set <name> <value>"));
                } else {
                    out.push(LogLine::success(format!("SET {} = {}", args[0], args[1..]
                        .join(" "))));
                }
            })
        ).with_args(vec![ArgSpec::required("name"), ArgSpec::required("value")]));

        self.commands.push(Command::new("get", "Get a console variable: get <name>",
            Box::new(|args, out| {
                if args.is_empty() {
                    out.push(LogLine::warn("Usage: get <name>"));
                } else {
                    out.push(LogLine::info(format!("GET {}", args[0])));
                }
            })
        ).with_args(vec![ArgSpec::required("name")]));

        self.commands.push(Command::new("history", "Show command history",
            Box::new(|_args, out| {
                out.push(LogLine::info("--- command history ---"));
            })
        ));

        self.commands.push(Command::new("version", "Show engine version",
            Box::new(|_args, out| {
                out.push(LogLine::info("Proof Engine -- mathematical rendering engine for Rust"));
            })
        ));

        self.commands.push(Command::new("reload", "Reload engine config from disk",
            Box::new(|_args, out| {
                out.push(LogLine::info("Reloading config..."));
            })
        ));

        self.commands.push(Command::new("quit", "Quit the engine",
            Box::new(|_args, out| {
                out.push(LogLine::warn("Quit requested."));
            })
        ).with_alias("exit").with_alias("q"));
    }

    // ── Input handling ────────────────────────────────────────────────────────

    /// Insert a character at the cursor position.
    pub fn type_char(&mut self, c: char) {
        if self.input.len() >= MAX_INPUT_LEN { return; }
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.pending_completion = None;
        self.history_index = None;
    }

    /// Delete the character before the cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor == 0 { return; }
        // Step back one char boundary
        let before = &self.input[..self.cursor];
        if let Some(c) = before.chars().next_back() {
            let len = c.len_utf8();
            self.input.remove(self.cursor - len);
            self.cursor -= len;
        }
        self.pending_completion = None;
    }

    /// Delete the character at the cursor (Delete key).
    pub fn delete_forward(&mut self) {
        if self.cursor >= self.input.len() { return; }
        self.input.remove(self.cursor);
        self.pending_completion = None;
    }

    /// Move cursor left by one character.
    pub fn cursor_left(&mut self) {
        if self.cursor == 0 { return; }
        let before = &self.input[..self.cursor];
        if let Some(c) = before.chars().next_back() {
            self.cursor -= c.len_utf8();
        }
    }

    /// Move cursor right by one character.
    pub fn cursor_right(&mut self) {
        if self.cursor >= self.input.len() { return; }
        let c = self.input[self.cursor..].chars().next().unwrap();
        self.cursor += c.len_utf8();
    }

    /// Move cursor to start of input.
    pub fn cursor_home(&mut self) { self.cursor = 0; }

    /// Move cursor to end of input.
    pub fn cursor_end(&mut self) { self.cursor = self.input.len(); }

    /// Clear the entire input line.
    pub fn clear_input(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.history_index = None;
    }

    // ── History navigation ────────────────────────────────────────────────────

    /// Navigate to the previous command in history (up arrow).
    pub fn history_prev(&mut self) {
        if self.history.is_empty() { return; }
        match self.history_index {
            None => {
                self.history_saved = self.input.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => return,
            Some(ref mut i) => *i -= 1,
        }
        if let Some(idx) = self.history_index {
            self.input  = self.history[idx].clone();
            self.cursor = self.input.len();
        }
    }

    /// Navigate to the next command in history (down arrow).
    pub fn history_next(&mut self) {
        match self.history_index {
            None => return,
            Some(i) if i + 1 >= self.history.len() => {
                self.history_index = None;
                self.input  = self.history_saved.clone();
                self.cursor = self.input.len();
            }
            Some(ref mut i) => {
                *i += 1;
                let idx = *i;
                self.input  = self.history[idx].clone();
                self.cursor = self.input.len();
            }
        }
    }

    // ── Tab completion ────────────────────────────────────────────────────────

    /// Attempt tab completion on the current input.
    pub fn tab_complete(&mut self) {
        let input = self.input.trim_start().to_owned();
        if input.is_empty() {
            // Show all commands
            let names: Vec<String> = self.commands.iter()
                .filter(|c| !c.hidden)
                .map(|c| c.name.clone())
                .collect();
            for name in &names { self.log.push_back(LogLine::debug(name.clone())); }
            self.trim_log();
            return;
        }

        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command_word = parts[0];

        if parts.len() == 1 {
            // Complete command name
            let all_names: Vec<String> = self.commands.iter()
                .flat_map(|c| std::iter::once(c.name.clone()).chain(c.aliases.iter().cloned()))
                .filter(|n| !n.is_empty())
                .collect();
            let result = CompletionResult::from_list(command_word, &all_names);
            if result.completions.len() == 1 {
                self.input  = result.completions[0].clone() + " ";
                self.cursor = self.input.len();
            } else if result.completions.len() > 1 {
                if result.common_prefix.len() > command_word.len() {
                    self.input  = result.common_prefix.clone();
                    self.cursor = self.input.len();
                }
                for c in &result.completions {
                    self.log.push_back(LogLine::debug(c.clone()));
                }
                self.trim_log();
            }
            self.pending_completion = Some(result);
        } else {
            // Complete argument
            let partial_arg = parts[1];
            if let Some(cmd) = self.commands.iter().find(|c| c.name == command_word || c.aliases.contains(&command_word.to_owned())) {
                let completions: Vec<String> = cmd.args.iter()
                    .flat_map(|a| a.completions.iter().cloned())
                    .collect();
                let result = CompletionResult::from_list(partial_arg, &completions);
                if result.completions.len() == 1 {
                    self.input  = format!("{} {}", command_word, result.completions[0]);
                    self.cursor = self.input.len();
                } else if result.completions.len() > 1 {
                    for c in &result.completions {
                        self.log.push_back(LogLine::debug(c.clone()));
                    }
                    self.trim_log();
                }
                self.pending_completion = Some(result);
            }
        }
    }

    // ── Submission ────────────────────────────────────────────────────────────

    /// Submit the current input line. Returns any special engine actions.
    pub fn submit(&mut self) -> ConsoleAction {
        let line = self.input.trim().to_owned();
        if line.is_empty() { return ConsoleAction::None; }

        // Push to history
        if self.history.back().map(|l| l.as_str()) != Some(&line) {
            self.history.push_back(line.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.pop_front();
            }
        }
        self.history_index = None;
        self.history_saved.clear();

        self.log.push_back(LogLine::command(format!("> {}", line)));
        self.input.clear();
        self.cursor = 0;
        self.scroll_offset = 0;

        self.execute_line(&line)
    }

    fn execute_line(&mut self, line: &str) -> ConsoleAction {
        let mut parts = tokenize(line);
        if parts.is_empty() { return ConsoleAction::None; }

        let cmd_name = parts.remove(0);
        let args: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

        // Special built-in actions that need ConsoleAction return
        match cmd_name.as_str() {
            "quit" | "exit" | "q" => {
                self.log.push_back(LogLine::warn("Quitting..."));
                self.trim_log();
                return ConsoleAction::Quit;
            }
            "clear" => {
                self.log.clear();
                self.scroll_offset = 0;
                return ConsoleAction::None;
            }
            "history" => {
                for (i, h) in self.history.iter().enumerate() {
                    self.log.push_back(LogLine::debug(format!("{:4}: {}", i + 1, h)));
                }
                self.trim_log();
                return ConsoleAction::None;
            }
            "set" if args.len() >= 2 => {
                let val = args[1..].join(" ");
                self.vars.insert(args[0].to_owned(), val.clone());
                self.log.push_back(LogLine::success(format!("{} = {}", args[0], val)));
                self.trim_log();
                return ConsoleAction::None;
            }
            "get" if !args.is_empty() => {
                let val = self.vars.get(args[0]).cloned().unwrap_or_else(|| "<undefined>".into());
                self.log.push_back(LogLine::info(format!("{} = {}", args[0], val)));
                self.trim_log();
                return ConsoleAction::None;
            }
            _ => {}
        }

        // Find and execute registered command
        let found = self.commands.iter().any(|c| {
            c.name == cmd_name || c.aliases.contains(&cmd_name)
        });

        if found {
            let mut output: Vec<LogLine> = Vec::new();
            // Invoke handler (borrow-safe: collect output then push)
            for cmd in &self.commands {
                if cmd.name == cmd_name || cmd.aliases.contains(&cmd_name) {
                    (cmd.handler)(&args, &mut output);
                    break;
                }
            }
            for line in output {
                self.log.push_back(line);
            }
            // Check for "help" command result containing a request to show help
            if cmd_name == "help" && !args.is_empty() {
                let target = args[0];
                if let Some(cmd) = self.commands.iter().find(|c| c.name == target) {
                    self.log.push_back(LogLine::info(format!("  {}", cmd.usage())));
                    self.log.push_back(LogLine::info(format!("  {}", cmd.help)));
                } else {
                    self.log.push_back(LogLine::warn(format!("Unknown command: '{}'", target)));
                }
            }
        } else {
            self.log.push_back(LogLine::error(format!("Unknown command: '{}'. Type 'list' for help.", cmd_name)));
        }

        self.trim_log();
        ConsoleAction::None
    }

    fn trim_log(&mut self) {
        while self.log.len() > MAX_LOG_LINES {
            self.log.pop_front();
            if self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
    }

    // ── Scrolling ─────────────────────────────────────────────────────────────

    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.log.len().saturating_sub(VISIBLE_LOG_LINES);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_to_bottom(&mut self) { self.scroll_offset = 0; }
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.log.len().saturating_sub(VISIBLE_LOG_LINES);
    }

    // ── Rendering helpers ─────────────────────────────────────────────────────

    /// Get the slice of log lines currently visible in the console window.
    pub fn visible_lines(&self) -> impl Iterator<Item = &LogLine> {
        let total = self.log.len();
        let start = if self.scroll_offset + VISIBLE_LOG_LINES > total {
            0
        } else {
            total - VISIBLE_LOG_LINES - self.scroll_offset
        };
        let end = (total - self.scroll_offset).min(total);
        self.log.range(start..end)
    }

    /// The input line split at the cursor for rendering a blinking cursor.
    pub fn input_before_cursor(&self) -> &str { &self.input[..self.cursor] }
    pub fn input_after_cursor(&self)  -> &str { &self.input[self.cursor..] }

    /// Toggle console visibility.
    pub fn toggle(&mut self) { self.visible = !self.visible; }

    /// Push a log line from external code.
    pub fn print(&mut self, line: LogLine) {
        self.log.push_back(line);
        self.trim_log();
    }

    pub fn println(&mut self, text: impl Into<String>) {
        self.print(LogLine::info(text));
    }

    pub fn print_warn(&mut self, text: impl Into<String>) {
        self.print(LogLine::warn(text));
    }

    pub fn print_error(&mut self, text: impl Into<String>) {
        self.print(LogLine::error(text));
    }

    pub fn print_success(&mut self, text: impl Into<String>) {
        self.print(LogLine::success(text));
    }
}

impl Default for Console {
    fn default() -> Self { Self::new() }
}

// ── ConsoleAction ─────────────────────────────────────────────────────────────

/// Returned by submit() to signal special engine actions.
#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleAction {
    None,
    Quit,
    Reload,
    RunScript(String),
    SetVar { name: String, value: String },
}

// ── tokenize ──────────────────────────────────────────────────────────────────

/// Split a command line into tokens, respecting quoted strings.
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for c in line.chars() {
        match c {
            '"' | '\'' if !in_quotes => { in_quotes = true;  quote_char = c; }
            c if in_quotes && c == quote_char => { in_quotes = false; }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() { tokens.push(current); }
    tokens
}

// ── ConsoleSink ───────────────────────────────────────────────────────────────

/// A lightweight handle for writing to the console from other systems.
/// Clone it and pass it around; it queues lines to be pushed on the next tick.
#[derive(Debug, Clone, Default)]
pub struct ConsoleSink {
    pub pending: Vec<LogLine>,
}

impl ConsoleSink {
    pub fn new() -> Self { Self::default() }

    pub fn info(&mut self,    text: impl Into<String>) { self.pending.push(LogLine::info(text));    }
    pub fn warn(&mut self,    text: impl Into<String>) { self.pending.push(LogLine::warn(text));    }
    pub fn error(&mut self,   text: impl Into<String>) { self.pending.push(LogLine::error(text));   }
    pub fn success(&mut self, text: impl Into<String>) { self.pending.push(LogLine::success(text)); }

    /// Drain pending lines into the console.
    pub fn flush(&mut self, console: &mut Console) {
        for line in self.pending.drain(..) {
            console.print(line);
        }
    }
}
