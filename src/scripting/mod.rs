//! Proof Engine scripting system — pure-Rust Lua-like language engine.
//!
//! Implements a complete stack-based bytecode VM for a dynamically-typed
//! scripting language with closures, tables, first-class functions, and
//! a host API bridge.
//!
//! # Architecture
//! ```text
//! Source → Lexer → Parser → AST → Compiler → Bytecode → VM → Value
//!                                                          ↑
//!                                              HostFunctions (Rust callbacks)
//! ```
//!
//! # Language features
//! - Dynamic typing: nil, bool, int, float, string, table, function
//! - First-class functions and closures
//! - Tables (hash maps + arrays)
//! - For/while/if/else control flow
//! - Multiple return values
//! - String interpolation
//! - Metatables for operator overloading
//! - `require` for multi-file scripts

pub mod lexer;
pub mod ast;
pub mod parser;
pub mod compiler;
pub mod vm;
pub mod stdlib;
pub mod host;

pub use vm::{Vm, Value, ScriptError};
pub use host::{ScriptHost, HostFunction};
