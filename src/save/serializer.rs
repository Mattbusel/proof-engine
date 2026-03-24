//! Custom serialization layer for the save system.
//!
//! `SerializedValue` is a JSON-compatible value enum. The `Serialize` and
//! `Deserialize` traits let any game type opt into save/load support without
//! pulling in serde.
//!
//! A simple hand-written JSON encoder/decoder is included so save files are
//! human-readable without any extra dependencies.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use glam::{Vec2, Vec3};

// ─────────────────────────────────────────────
//  DeserializeError
// ─────────────────────────────────────────────

/// Errors that can occur during deserialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeserializeError {
    /// The value was the wrong variant (e.g. expected Int, got Str).
    WrongType { expected: &'static str, got: &'static str },
    /// A required key was missing from a Map.
    MissingKey(String),
    /// A list index was out of bounds.
    IndexOutOfBounds { index: usize, len: usize },
    /// The string could not be parsed as the target type.
    ParseError(String),
    /// A catch-all for structural/logic errors.
    Custom(String),
}

impl std::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeserializeError::WrongType { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            DeserializeError::MissingKey(k) => write!(f, "missing key: '{k}'"),
            DeserializeError::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds (len {len})")
            }
            DeserializeError::ParseError(s) => write!(f, "parse error: {s}"),
            DeserializeError::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for DeserializeError {}

// ─────────────────────────────────────────────
//  SerializedValue
// ─────────────────────────────────────────────

/// A generic serialized value, analogous to `serde_json::Value`.
#[derive(Debug, Clone, PartialEq)]
pub enum SerializedValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<SerializedValue>),
    Map(HashMap<String, SerializedValue>),
}

impl SerializedValue {
    // ── Constructors ───────────────────────────────────────────────────────

    pub fn null() -> Self { SerializedValue::Null }
    pub fn bool(b: bool) -> Self { SerializedValue::Bool(b) }
    pub fn int(i: i64) -> Self { SerializedValue::Int(i) }
    pub fn float(f: f64) -> Self { SerializedValue::Float(f) }
    pub fn str(s: impl Into<String>) -> Self { SerializedValue::Str(s.into()) }
    pub fn bytes(b: Vec<u8>) -> Self { SerializedValue::Bytes(b) }
    pub fn list(v: Vec<SerializedValue>) -> Self { SerializedValue::List(v) }
    pub fn map(m: HashMap<String, SerializedValue>) -> Self { SerializedValue::Map(m) }

    pub fn empty_map() -> Self {
        SerializedValue::Map(HashMap::new())
    }

    pub fn empty_list() -> Self {
        SerializedValue::List(Vec::new())
    }

    // ── Type accessors ─────────────────────────────────────────────────────

    pub fn as_bool(&self) -> Option<bool> {
        if let SerializedValue::Bool(b) = self { Some(*b) } else { None }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            SerializedValue::Int(i) => Some(*i),
            SerializedValue::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            SerializedValue::Float(f) => Some(*f),
            SerializedValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let SerializedValue::Str(s) = self { Some(s.as_str()) } else { None }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let SerializedValue::Bytes(b) = self { Some(b.as_slice()) } else { None }
    }

    pub fn as_list(&self) -> Option<&[SerializedValue]> {
        if let SerializedValue::List(v) = self { Some(v.as_slice()) } else { None }
    }

    pub fn as_map(&self) -> Option<&HashMap<String, SerializedValue>> {
        if let SerializedValue::Map(m) = self { Some(m) } else { None }
    }

    pub fn as_map_mut(&mut self) -> Option<&mut HashMap<String, SerializedValue>> {
        if let SerializedValue::Map(m) = self { Some(m) } else { None }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, SerializedValue::Null)
    }

    // ── Key / index access ─────────────────────────────────────────────────

    /// Get a value from a Map by key.
    pub fn get(&self, key: &str) -> Option<&SerializedValue> {
        self.as_map()?.get(key)
    }

    /// Get a value from a List by index.
    pub fn index(&self, i: usize) -> Option<&SerializedValue> {
        self.as_list()?.get(i)
    }

    /// Insert a key-value pair into a Map. Returns `false` if `self` is not a Map.
    pub fn insert(&mut self, key: impl Into<String>, value: SerializedValue) -> bool {
        if let SerializedValue::Map(m) = self {
            m.insert(key.into(), value);
            true
        } else {
            false
        }
    }

    /// Push a value into a List. Returns `false` if `self` is not a List.
    pub fn push(&mut self, value: SerializedValue) -> bool {
        if let SerializedValue::List(v) = self {
            v.push(value);
            true
        } else {
            false
        }
    }

    /// The variant name as a static string (for error messages).
    pub fn type_name(&self) -> &'static str {
        match self {
            SerializedValue::Null => "Null",
            SerializedValue::Bool(_) => "Bool",
            SerializedValue::Int(_) => "Int",
            SerializedValue::Float(_) => "Float",
            SerializedValue::Str(_) => "Str",
            SerializedValue::Bytes(_) => "Bytes",
            SerializedValue::List(_) => "List",
            SerializedValue::Map(_) => "Map",
        }
    }

    // ── JSON encoding ──────────────────────────────────────────────────────

    /// Encode to a JSON string. Bytes are encoded as a base64-like hex string.
    pub fn to_json_string(&self) -> String {
        let mut buf = String::with_capacity(64);
        self.write_json(&mut buf);
        buf
    }

    fn write_json(&self, buf: &mut String) {
        match self {
            SerializedValue::Null => buf.push_str("null"),
            SerializedValue::Bool(b) => buf.push_str(if *b { "true" } else { "false" }),
            SerializedValue::Int(i) => buf.push_str(&i.to_string()),
            SerializedValue::Float(f) => {
                if f.is_nan() {
                    buf.push_str("null"); // JSON has no NaN
                } else if f.is_infinite() {
                    buf.push_str(if *f > 0.0 { "1e308" } else { "-1e308" });
                } else {
                    buf.push_str(&format!("{f:?}"));
                }
            }
            SerializedValue::Str(s) => {
                buf.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => buf.push_str("\\\""),
                        '\\' => buf.push_str("\\\\"),
                        '\n' => buf.push_str("\\n"),
                        '\r' => buf.push_str("\\r"),
                        '\t' => buf.push_str("\\t"),
                        c if (c as u32) < 0x20 => {
                            buf.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => buf.push(c),
                    }
                }
                buf.push('"');
            }
            SerializedValue::Bytes(bytes) => {
                // Encode as a JSON string containing lowercase hex
                buf.push('"');
                for b in bytes {
                    buf.push_str(&format!("{b:02x}"));
                }
                buf.push('"');
            }
            SerializedValue::List(items) => {
                buf.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        buf.push(',');
                    }
                    item.write_json(buf);
                }
                buf.push(']');
            }
            SerializedValue::Map(m) => {
                buf.push('{');
                let mut first = true;
                // Sort keys for deterministic output
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                for key in keys {
                    if !first {
                        buf.push(',');
                    }
                    first = false;
                    SerializedValue::Str(key.clone()).write_json(buf);
                    buf.push(':');
                    m[key].write_json(buf);
                }
                buf.push('}');
            }
        }
    }

    // ── JSON decoding ──────────────────────────────────────────────────────

    /// Parse a JSON string into a `SerializedValue`.
    pub fn from_json_str(s: &str) -> Result<Self, DeserializeError> {
        let mut parser = JsonParser::new(s.trim());
        let v = parser.parse_value()?;
        parser.skip_whitespace();
        Ok(v)
    }
}

impl Default for SerializedValue {
    fn default() -> Self {
        SerializedValue::Null
    }
}

// ─────────────────────────────────────────────
//  Minimal JSON parser
// ─────────────────────────────────────────────

struct JsonParser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(s: &'a str) -> Self {
        Self { src: s.as_bytes(), pos: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn consume(&mut self) -> Option<u8> {
        if self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), DeserializeError> {
        match self.consume() {
            Some(got) if got == b => Ok(()),
            Some(got) => Err(DeserializeError::ParseError(format!(
                "expected '{}' got '{}'",
                b as char, got as char
            ))),
            None => Err(DeserializeError::ParseError("unexpected EOF".into())),
        }
    }

    fn parse_value(&mut self) -> Result<SerializedValue, DeserializeError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => self.parse_null(),
            Some(b't') | Some(b'f') => self.parse_bool(),
            Some(b'"') => self.parse_string(),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(c) => Err(DeserializeError::ParseError(format!("unexpected char '{}'", c as char))),
            None => Err(DeserializeError::ParseError("unexpected EOF".into())),
        }
    }

    fn parse_null(&mut self) -> Result<SerializedValue, DeserializeError> {
        self.expect(b'n')?;
        self.expect(b'u')?;
        self.expect(b'l')?;
        self.expect(b'l')?;
        Ok(SerializedValue::Null)
    }

    fn parse_bool(&mut self) -> Result<SerializedValue, DeserializeError> {
        if self.peek() == Some(b't') {
            self.expect(b't')?; self.expect(b'r')?; self.expect(b'u')?; self.expect(b'e')?;
            Ok(SerializedValue::Bool(true))
        } else {
            self.expect(b'f')?; self.expect(b'a')?; self.expect(b'l')?; self.expect(b's')?; self.expect(b'e')?;
            Ok(SerializedValue::Bool(false))
        }
    }

    fn parse_string(&mut self) -> Result<SerializedValue, DeserializeError> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            match self.consume() {
                Some(b'"') => break,
                Some(b'\\') => {
                    match self.consume() {
                        Some(b'"') => s.push('"'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'/') => s.push('/'),
                        Some(b'n') => s.push('\n'),
                        Some(b'r') => s.push('\r'),
                        Some(b't') => s.push('\t'),
                        Some(b'u') => {
                            // Read 4 hex digits
                            let mut hex = String::new();
                            for _ in 0..4 {
                                hex.push(self.consume().unwrap_or(b'0') as char);
                            }
                            let code = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
                            s.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        Some(c) => s.push(c as char),
                        None => return Err(DeserializeError::ParseError("unterminated string".into())),
                    }
                }
                Some(c) => s.push(c as char),
                None => return Err(DeserializeError::ParseError("unterminated string".into())),
            }
        }
        Ok(SerializedValue::Str(s))
    }

    fn parse_number(&mut self) -> Result<SerializedValue, DeserializeError> {
        let start = self.pos;
        if self.peek() == Some(b'-') { self.pos += 1; }
        while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
        let is_float = matches!(self.peek(), Some(b'.') | Some(b'e') | Some(b'E'));
        if is_float {
            if self.peek() == Some(b'.') {
                self.pos += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
            }
            if matches!(self.peek(), Some(b'e') | Some(b'E')) {
                self.pos += 1;
                if matches!(self.peek(), Some(b'+') | Some(b'-')) { self.pos += 1; }
                while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
            }
        }
        let slice = std::str::from_utf8(&self.src[start..self.pos])
            .map_err(|e| DeserializeError::ParseError(e.to_string()))?;
        if is_float {
            let f: f64 = slice.parse()
                .map_err(|e: std::num::ParseFloatError| DeserializeError::ParseError(e.to_string()))?;
            Ok(SerializedValue::Float(f))
        } else {
            let i: i64 = slice.parse()
                .map_err(|e: std::num::ParseIntError| DeserializeError::ParseError(e.to_string()))?;
            Ok(SerializedValue::Int(i))
        }
    }

    fn parse_array(&mut self) -> Result<SerializedValue, DeserializeError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(SerializedValue::List(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => { self.pos += 1; }
                Some(b']') => { self.pos += 1; break; }
                Some(c) => return Err(DeserializeError::ParseError(format!("expected ',' or ']' got '{}'", c as char))),
                None => return Err(DeserializeError::ParseError("unterminated array".into())),
            }
        }
        Ok(SerializedValue::List(items))
    }

    fn parse_object(&mut self) -> Result<SerializedValue, DeserializeError> {
        self.expect(b'{')?;
        let mut map = HashMap::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(SerializedValue::Map(map));
        }
        loop {
            self.skip_whitespace();
            let key_val = self.parse_string()?;
            let key = match key_val {
                SerializedValue::Str(s) => s,
                _ => return Err(DeserializeError::ParseError("expected string key".into())),
            };
            self.skip_whitespace();
            self.expect(b':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => { self.pos += 1; }
                Some(b'}') => { self.pos += 1; break; }
                Some(c) => return Err(DeserializeError::ParseError(format!("expected ',' or '}}' got '{}'", c as char))),
                None => return Err(DeserializeError::ParseError("unterminated object".into())),
            }
        }
        Ok(SerializedValue::Map(map))
    }
}

// ─────────────────────────────────────────────
//  Serialize / Deserialize traits
// ─────────────────────────────────────────────

/// Convert a value to a `SerializedValue`.
pub trait Serialize {
    fn serialize(&self) -> SerializedValue;
}

/// Reconstruct a value from a `SerializedValue`.
pub trait Deserialize: Sized {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError>;
}

// ── bool ───────────────────────────────────────────────────────────────────

impl Serialize for bool {
    fn serialize(&self) -> SerializedValue { SerializedValue::Bool(*self) }
}

impl Deserialize for bool {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_bool().ok_or(DeserializeError::WrongType { expected: "Bool", got: v.type_name() })
    }
}

// ── i32 ───────────────────────────────────────────────────────────────────

impl Serialize for i32 {
    fn serialize(&self) -> SerializedValue { SerializedValue::Int(*self as i64) }
}

impl Deserialize for i32 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_int().map(|i| i as i32)
            .ok_or(DeserializeError::WrongType { expected: "Int", got: v.type_name() })
    }
}

// ── i64 ───────────────────────────────────────────────────────────────────

impl Serialize for i64 {
    fn serialize(&self) -> SerializedValue { SerializedValue::Int(*self) }
}

impl Deserialize for i64 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_int().ok_or(DeserializeError::WrongType { expected: "Int", got: v.type_name() })
    }
}

// ── u32 ───────────────────────────────────────────────────────────────────

impl Serialize for u32 {
    fn serialize(&self) -> SerializedValue { SerializedValue::Int(*self as i64) }
}

impl Deserialize for u32 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_int().map(|i| i as u32)
            .ok_or(DeserializeError::WrongType { expected: "Int", got: v.type_name() })
    }
}

// ── u64 ───────────────────────────────────────────────────────────────────

impl Serialize for u64 {
    fn serialize(&self) -> SerializedValue { SerializedValue::Int(*self as i64) }
}

impl Deserialize for u64 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_int().map(|i| i as u64)
            .ok_or(DeserializeError::WrongType { expected: "Int", got: v.type_name() })
    }
}

// ── f32 ───────────────────────────────────────────────────────────────────

impl Serialize for f32 {
    fn serialize(&self) -> SerializedValue { SerializedValue::Float(*self as f64) }
}

impl Deserialize for f32 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_float().map(|f| f as f32)
            .ok_or(DeserializeError::WrongType { expected: "Float", got: v.type_name() })
    }
}

// ── f64 ───────────────────────────────────────────────────────────────────

impl Serialize for f64 {
    fn serialize(&self) -> SerializedValue { SerializedValue::Float(*self) }
}

impl Deserialize for f64 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_float().ok_or(DeserializeError::WrongType { expected: "Float", got: v.type_name() })
    }
}

// ── String ─────────────────────────────────────────────────────────────────

impl Serialize for String {
    fn serialize(&self) -> SerializedValue { SerializedValue::Str(self.clone()) }
}

impl Deserialize for String {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        v.as_str().map(|s| s.to_string())
            .ok_or(DeserializeError::WrongType { expected: "Str", got: v.type_name() })
    }
}

// ── &str ──────────────────────────────────────────────────────────────────

impl Serialize for &str {
    fn serialize(&self) -> SerializedValue { SerializedValue::Str(self.to_string()) }
}

// ── Vec<T> ────────────────────────────────────────────────────────────────

impl<T: Serialize> Serialize for Vec<T> {
    fn serialize(&self) -> SerializedValue {
        SerializedValue::List(self.iter().map(|v| v.serialize()).collect())
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        let list = v.as_list()
            .ok_or(DeserializeError::WrongType { expected: "List", got: v.type_name() })?;
        list.iter().map(T::deserialize).collect()
    }
}

// ── HashMap<String, V> ────────────────────────────────────────────────────

impl<V: Serialize> Serialize for HashMap<String, V> {
    fn serialize(&self) -> SerializedValue {
        let mut m = HashMap::new();
        for (k, v) in self {
            m.insert(k.clone(), v.serialize());
        }
        SerializedValue::Map(m)
    }
}

impl<V: Deserialize> Deserialize for HashMap<String, V> {
    fn deserialize(sv: &SerializedValue) -> Result<Self, DeserializeError> {
        let map = sv.as_map()
            .ok_or(DeserializeError::WrongType { expected: "Map", got: sv.type_name() })?;
        let mut out = HashMap::new();
        for (k, v) in map {
            out.insert(k.clone(), V::deserialize(v)?);
        }
        Ok(out)
    }
}

// ── Option<T> ─────────────────────────────────────────────────────────────

impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self) -> SerializedValue {
        match self {
            Some(v) => v.serialize(),
            None => SerializedValue::Null,
        }
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        if v.is_null() {
            Ok(None)
        } else {
            T::deserialize(v).map(Some)
        }
    }
}

// ── Vec2 (glam) ───────────────────────────────────────────────────────────

impl Serialize for Vec2 {
    fn serialize(&self) -> SerializedValue {
        let mut m = HashMap::new();
        m.insert("x".to_string(), SerializedValue::Float(self.x as f64));
        m.insert("y".to_string(), SerializedValue::Float(self.y as f64));
        SerializedValue::Map(m)
    }
}

impl Deserialize for Vec2 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        let x = v.get("x")
            .and_then(|v| v.as_float())
            .ok_or_else(|| DeserializeError::MissingKey("x".into()))? as f32;
        let y = v.get("y")
            .and_then(|v| v.as_float())
            .ok_or_else(|| DeserializeError::MissingKey("y".into()))? as f32;
        Ok(Vec2::new(x, y))
    }
}

// ── Vec3 (glam) ───────────────────────────────────────────────────────────

impl Serialize for Vec3 {
    fn serialize(&self) -> SerializedValue {
        let mut m = HashMap::new();
        m.insert("x".to_string(), SerializedValue::Float(self.x as f64));
        m.insert("y".to_string(), SerializedValue::Float(self.y as f64));
        m.insert("z".to_string(), SerializedValue::Float(self.z as f64));
        SerializedValue::Map(m)
    }
}

impl Deserialize for Vec3 {
    fn deserialize(v: &SerializedValue) -> Result<Self, DeserializeError> {
        let x = v.get("x").and_then(|v| v.as_float())
            .ok_or_else(|| DeserializeError::MissingKey("x".into()))? as f32;
        let y = v.get("y").and_then(|v| v.as_float())
            .ok_or_else(|| DeserializeError::MissingKey("y".into()))? as f32;
        let z = v.get("z").and_then(|v| v.as_float())
            .ok_or_else(|| DeserializeError::MissingKey("z".into()))? as f32;
        Ok(Vec3::new(x, y, z))
    }
}

// ─────────────────────────────────────────────
//  ComponentSerializer
// ─────────────────────────────────────────────

type SerializeFn = Box<dyn Fn(*const u8) -> SerializedValue + Send + Sync>;
type DeserializeFn = Box<dyn Fn(&SerializedValue) -> Box<dyn Any + Send + Sync> + Send + Sync>;

struct ComponentEntry {
    type_id: TypeId,
    name: String,
    serialize: SerializeFn,
    deserialize: DeserializeFn,
}

/// Registry mapping component type names to their serialize/deserialize functions.
///
/// Used by the snapshot system to serialize arbitrary component types by name.
pub struct ComponentSerializer {
    by_name: HashMap<String, usize>,
    by_type: HashMap<TypeId, usize>,
    entries: Vec<ComponentEntry>,
}

impl ComponentSerializer {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            by_type: HashMap::new(),
            entries: Vec::new(),
        }
    }

    /// Register a component type `T` under `name`.
    pub fn register<T: Serialize + Deserialize + Any + Send + Sync + 'static>(
        &mut self,
        name: impl Into<String>,
    ) {
        let name = name.into();
        let type_id = TypeId::of::<T>();
        let idx = self.entries.len();
        self.by_name.insert(name.clone(), idx);
        self.by_type.insert(type_id, idx);
        self.entries.push(ComponentEntry {
            type_id,
            name,
            serialize: Box::new(|ptr| {
                // SAFETY: caller must ensure ptr points to a valid T
                let reference = unsafe { &*(ptr as *const T) };
                reference.serialize()
            }),
            deserialize: Box::new(|v| {
                match T::deserialize(v) {
                    Ok(t) => Box::new(t) as Box<dyn Any + Send + Sync>,
                    Err(_) => Box::new(()) as Box<dyn Any + Send + Sync>,
                }
            }),
        });
    }

    /// Serialize a component by TypeId, given a raw pointer to its data.
    ///
    /// # Safety
    /// `ptr` must point to a valid value of the type identified by `type_id`.
    pub unsafe fn serialize_any(&self, type_id: TypeId, ptr: *const u8) -> Option<SerializedValue> {
        let idx = self.by_type.get(&type_id)?;
        Some((self.entries[*idx].serialize)(ptr))
    }

    /// Deserialize a component by name.
    pub fn deserialize_any(&self, name: &str, v: &SerializedValue) -> Option<Box<dyn Any + Send + Sync>> {
        let idx = self.by_name.get(name)?;
        Some((self.entries[*idx].deserialize)(v))
    }

    /// Returns `true` if a type is registered under `name`.
    pub fn has_name(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// All registered component names.
    pub fn registered_names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    /// Number of registered component types.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for ComponentSerializer {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_primitives() {
        assert_eq!(true.serialize(), SerializedValue::Bool(true));
        assert_eq!(42i64.serialize(), SerializedValue::Int(42));
        assert_eq!(3.14f64.serialize(), SerializedValue::Float(3.14));
        assert_eq!("hello".serialize(), SerializedValue::Str("hello".into()));
    }

    #[test]
    fn deserialize_primitives() {
        assert_eq!(bool::deserialize(&SerializedValue::Bool(false)).unwrap(), false);
        assert_eq!(i64::deserialize(&SerializedValue::Int(7)).unwrap(), 7);
        assert_eq!(f64::deserialize(&SerializedValue::Float(1.5)).unwrap(), 1.5);
        assert_eq!(String::deserialize(&SerializedValue::Str("hi".into())).unwrap(), "hi");
    }

    #[test]
    fn serialize_vec2_roundtrip() {
        let v = Vec2::new(1.0, 2.5);
        let sv = v.serialize();
        let v2 = Vec2::deserialize(&sv).unwrap();
        assert!((v.x - v2.x).abs() < 1e-5);
        assert!((v.y - v2.y).abs() < 1e-5);
    }

    #[test]
    fn serialize_vec3_roundtrip() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let sv = v.serialize();
        let v2 = Vec3::deserialize(&sv).unwrap();
        assert!((v - v2).length() < 1e-5);
    }

    #[test]
    fn serialize_vec_of_ints() {
        let v: Vec<i64> = vec![10, 20, 30];
        let sv = v.serialize();
        let v2: Vec<i64> = Vec::deserialize(&sv).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn serialize_option_some_none() {
        let some: Option<i64> = Some(99);
        let none: Option<i64> = None;
        assert_eq!(some.serialize(), SerializedValue::Int(99));
        assert_eq!(none.serialize(), SerializedValue::Null);
        assert_eq!(Option::<i64>::deserialize(&SerializedValue::Null).unwrap(), None);
        assert_eq!(Option::<i64>::deserialize(&SerializedValue::Int(5)).unwrap(), Some(5));
    }

    #[test]
    fn json_roundtrip_simple() {
        let original = SerializedValue::Map({
            let mut m = HashMap::new();
            m.insert("name".into(), SerializedValue::Str("Alice".into()));
            m.insert("score".into(), SerializedValue::Int(1000));
            m.insert("alive".into(), SerializedValue::Bool(true));
            m
        });
        let json = original.to_json_string();
        let parsed = SerializedValue::from_json_str(&json).unwrap();
        assert_eq!(parsed.get("name").and_then(|v| v.as_str()), Some("Alice"));
        assert_eq!(parsed.get("score").and_then(|v| v.as_int()), Some(1000));
        assert_eq!(parsed.get("alive").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn json_roundtrip_nested() {
        let sv = SerializedValue::List(vec![
            SerializedValue::Int(1),
            SerializedValue::Float(2.5),
            SerializedValue::Null,
            SerializedValue::Bool(true),
        ]);
        let json = sv.to_json_string();
        let parsed = SerializedValue::from_json_str(&json).unwrap();
        assert_eq!(parsed.index(0).and_then(|v| v.as_int()), Some(1));
        assert_eq!(parsed.index(2).map(|v| v.is_null()), Some(true));
    }

    #[test]
    fn json_string_escape() {
        let sv = SerializedValue::Str("say \"hello\"\nnewline".into());
        let json = sv.to_json_string();
        let parsed = SerializedValue::from_json_str(&json).unwrap();
        assert_eq!(parsed.as_str(), Some("say \"hello\"\nnewline"));
    }

    #[test]
    fn component_serializer_register_and_use() {
        let mut cs = ComponentSerializer::new();
        cs.register::<i64>("health");
        assert!(cs.has_name("health"));
        assert_eq!(cs.len(), 1);

        let sv = SerializedValue::Int(100);
        let boxed = cs.deserialize_any("health", &sv).unwrap();
        let val = boxed.downcast_ref::<i64>().unwrap();
        assert_eq!(*val, 100);
    }

    #[test]
    fn wrong_type_error() {
        let sv = SerializedValue::Str("not a bool".into());
        let err = bool::deserialize(&sv).unwrap_err();
        assert!(matches!(err, DeserializeError::WrongType { .. }));
    }
}
