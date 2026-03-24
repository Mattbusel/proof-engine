//! # Dialogue Module
//!
//! Full-featured branching dialogue system for the Proof Engine.
//!
//! ## Architecture
//! - [`tree`] — data model: nodes, conditions, builder, library
//! - [`runner`] — runtime execution: state machine, session, history
//!
//! ## Quick Start
//! ```rust,ignore
//! use proof_engine::dialogue::*;
//!
//! let lib = Arc::new(DialogueLibrary::new());
//! let mut session = DialogueSession::new(lib, SessionConfig::default());
//! session.start_session(DialogueId(1)).unwrap();
//! while let Some(out) = session.process(SessionInput::Advance) {
//!     println!("{:?}", out);
//! }
//! ```

pub mod tree;
pub mod runner;

// ── Re-exports ─────────────────────────────────────────────────────────────

pub use tree::{
    CameraAction, ChoiceOption, Condition, DialogueBuilder, DialogueLibrary, DialogueMeta,
    DialogueNode, DialogueTree,
};
pub use runner::{
    DialogueHistory, DialogueOutput, DialogueRunner, DialogueSession, DialogueState,
    HistoryRecord, RunnerError, RunnerStatus, SessionConfig, SessionInput, VisibleChoice,
};

// ── Primitive ID newtypes ───────────────────────────────────────────────────

/// Opaque identifier for a single dialogue node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const INVALID: NodeId = NodeId(u32::MAX);

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

/// Opaque identifier for a complete dialogue tree / conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DialogueId(pub u32);

impl DialogueId {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for DialogueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dialogue({})", self.0)
    }
}

/// Opaque identifier for a speaker character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpeakerId(pub u32);

impl SpeakerId {
    /// Sentinel value meaning "no specific speaker" (narration, signs, etc.).
    pub const NARRATOR: SpeakerId = SpeakerId(0);

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for SpeakerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Speaker({})", self.0)
    }
}

// ── Speaker ─────────────────────────────────────────────────────────────────

/// A character who can speak in dialogue.
#[derive(Debug, Clone)]
pub struct Speaker {
    /// Unique identifier.
    pub id: SpeakerId,
    /// Display name shown in the UI.
    pub name: String,
    /// Atlas / texture key for the portrait frame, if any.
    pub portrait_key: Option<String>,
    /// Audio engine voice ID for text-to-speech / VO routing, if any.
    pub voice_id: Option<String>,
}

impl Speaker {
    /// Create a basic speaker with just a name.
    pub fn new(id: SpeakerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            portrait_key: None,
            voice_id: None,
        }
    }

    /// Builder-style: set the portrait key.
    pub fn with_portrait(mut self, key: impl Into<String>) -> Self {
        self.portrait_key = Some(key.into());
        self
    }

    /// Builder-style: set the voice ID.
    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice_id = Some(voice.into());
        self
    }
}

// ── SpeakerRegistry ─────────────────────────────────────────────────────────

/// Central registry mapping [`SpeakerId`] → [`Speaker`].
///
/// Typically stored behind an `Arc<RwLock<SpeakerRegistry>>` so the dialogue
/// runner and UI can share it without copying speaker data.
#[derive(Debug, Clone, Default)]
pub struct SpeakerRegistry {
    speakers: std::collections::HashMap<SpeakerId, Speaker>,
}

impl SpeakerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a speaker.  Returns the old value if one existed.
    pub fn register(&mut self, speaker: Speaker) -> Option<Speaker> {
        self.speakers.insert(speaker.id, speaker)
    }

    /// Look up a speaker by its ID.
    pub fn get(&self, id: SpeakerId) -> Option<&Speaker> {
        self.speakers.get(&id)
    }

    /// Look up a speaker by display name (case-sensitive, first match).
    pub fn lookup_by_name(&self, name: &str) -> Option<&Speaker> {
        self.speakers.values().find(|s| s.name == name)
    }

    /// Iterate over all registered speakers in an unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &Speaker> {
        self.speakers.values()
    }

    /// Total number of registered speakers.
    pub fn len(&self) -> usize {
        self.speakers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.speakers.is_empty()
    }

    /// Remove a speaker by ID.  Returns the removed speaker if it existed.
    pub fn remove(&mut self, id: SpeakerId) -> Option<Speaker> {
        self.speakers.remove(&id)
    }
}

// ── Emotion ─────────────────────────────────────────────────────────────────

/// Emotional state affecting portrait expression and voice pitch modifiers.
///
/// Mapped to Ekman's basic emotions plus two extras common in game dialogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Emotion {
    /// Default, no particular expression.
    #[default]
    Neutral,
    Happy,
    Sad,
    Angry,
    Fearful,
    Surprised,
    Disgusted,
    /// Mild disdain / superiority — distinct from `Angry`.
    Contemptuous,
}

impl Emotion {
    /// Human-readable label, useful for UI tooltips or debug overlays.
    pub fn label(self) -> &'static str {
        match self {
            Emotion::Neutral      => "Neutral",
            Emotion::Happy        => "Happy",
            Emotion::Sad          => "Sad",
            Emotion::Angry        => "Angry",
            Emotion::Fearful      => "Fearful",
            Emotion::Surprised    => "Surprised",
            Emotion::Disgusted    => "Disgusted",
            Emotion::Contemptuous => "Contemptuous",
        }
    }

    /// Returns a rough pitch-shift multiplier (1.0 = neutral) for VO routing.
    pub fn pitch_bias(self) -> f32 {
        match self {
            Emotion::Neutral      => 1.00,
            Emotion::Happy        => 1.05,
            Emotion::Sad          => 0.92,
            Emotion::Angry        => 0.88,
            Emotion::Fearful      => 1.10,
            Emotion::Surprised    => 1.12,
            Emotion::Disgusted    => 0.95,
            Emotion::Contemptuous => 0.97,
        }
    }

    /// All variants in declaration order, useful for editor dropdowns.
    pub const ALL: &'static [Emotion] = &[
        Emotion::Neutral,
        Emotion::Happy,
        Emotion::Sad,
        Emotion::Angry,
        Emotion::Fearful,
        Emotion::Surprised,
        Emotion::Disgusted,
        Emotion::Contemptuous,
    ];
}

impl std::fmt::Display for Emotion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ── DialogueVar ─────────────────────────────────────────────────────────────

/// A dynamically-typed value stored in the dialogue runtime's variable table.
///
/// Variables drive conditions, script arguments, and can be read back by game
/// systems to reflect choices made during conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogueVar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl DialogueVar {
    /// Coerce to bool.  Falsy: `false`, `0`, `0.0`, `""`.
    pub fn as_bool(&self) -> bool {
        match self {
            DialogueVar::Bool(b)  => *b,
            DialogueVar::Int(n)   => *n != 0,
            DialogueVar::Float(f) => *f != 0.0,
            DialogueVar::Str(s)   => !s.is_empty(),
        }
    }

    /// Coerce to i64.
    pub fn as_int(&self) -> i64 {
        match self {
            DialogueVar::Bool(b)  => if *b { 1 } else { 0 },
            DialogueVar::Int(n)   => *n,
            DialogueVar::Float(f) => *f as i64,
            DialogueVar::Str(s)   => s.parse().unwrap_or(0),
        }
    }

    /// Coerce to f64.
    pub fn as_float(&self) -> f64 {
        match self {
            DialogueVar::Bool(b)  => if *b { 1.0 } else { 0.0 },
            DialogueVar::Int(n)   => *n as f64,
            DialogueVar::Float(f) => *f,
            DialogueVar::Str(s)   => s.parse().unwrap_or(0.0),
        }
    }

    /// Coerce to a string representation.
    pub fn as_str(&self) -> String {
        match self {
            DialogueVar::Bool(b)  => b.to_string(),
            DialogueVar::Int(n)   => n.to_string(),
            DialogueVar::Float(f) => f.to_string(),
            DialogueVar::Str(s)   => s.clone(),
        }
    }

    /// Returns the type name as a static string.
    pub fn type_name(&self) -> &'static str {
        match self {
            DialogueVar::Bool(_)  => "bool",
            DialogueVar::Int(_)   => "int",
            DialogueVar::Float(_) => "float",
            DialogueVar::Str(_)   => "str",
        }
    }

    /// Partial comparison: `<`.  Mixed numeric types are coerced to f64.
    pub fn lt(&self, other: &DialogueVar) -> bool {
        match (self, other) {
            (DialogueVar::Int(a), DialogueVar::Int(b))     => a < b,
            (DialogueVar::Float(a), DialogueVar::Float(b)) => a < b,
            (DialogueVar::Str(a), DialogueVar::Str(b))     => a < b,
            _ => self.as_float() < other.as_float(),
        }
    }

    /// Partial comparison: `>`.
    pub fn gt(&self, other: &DialogueVar) -> bool {
        match (self, other) {
            (DialogueVar::Int(a), DialogueVar::Int(b))     => a > b,
            (DialogueVar::Float(a), DialogueVar::Float(b)) => a > b,
            (DialogueVar::Str(a), DialogueVar::Str(b))     => a > b,
            _ => self.as_float() > other.as_float(),
        }
    }
}

impl std::fmt::Display for DialogueVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DialogueVar::Bool(b)  => write!(f, "{}", b),
            DialogueVar::Int(n)   => write!(f, "{}", n),
            DialogueVar::Float(v) => write!(f, "{}", v),
            DialogueVar::Str(s)   => write!(f, "{}", s),
        }
    }
}

impl From<bool>   for DialogueVar { fn from(v: bool)   -> Self { DialogueVar::Bool(v) } }
impl From<i64>    for DialogueVar { fn from(v: i64)    -> Self { DialogueVar::Int(v) } }
impl From<i32>    for DialogueVar { fn from(v: i32)    -> Self { DialogueVar::Int(v as i64) } }
impl From<f64>    for DialogueVar { fn from(v: f64)    -> Self { DialogueVar::Float(v) } }
impl From<f32>    for DialogueVar { fn from(v: f32)    -> Self { DialogueVar::Float(v as f64) } }
impl From<String> for DialogueVar { fn from(v: String) -> Self { DialogueVar::Str(v) } }
impl From<&str>   for DialogueVar { fn from(v: &str)   -> Self { DialogueVar::Str(v.to_string()) } }

// ── DialogueEvent ────────────────────────────────────────────────────────────

/// Observable events emitted by the dialogue runner during execution.
///
/// Game systems (achievements, analytics, cutscene triggers) can subscribe to
/// this event stream to react to dialogue milestones without tight coupling.
#[derive(Debug, Clone)]
pub enum DialogueEvent {
    /// A dialogue tree has begun execution.
    Started(DialogueId),
    /// The runner has entered a new node.
    NodeEntered(NodeId),
    /// The player made a choice at a particular node.
    ChoiceMade {
        node:   NodeId,
        choice: usize,
    },
    /// The dialogue has fully concluded.
    Ended(DialogueId),
    /// A variable was written (old value is not stored to keep events small).
    VariableChanged {
        name:  String,
        value: DialogueVar,
    },
    /// A boolean flag was set.
    FlagSet(String),
    /// A script function was invoked via a `CallScript` node.
    ScriptCalled {
        function: String,
        args:     Vec<DialogueVar>,
    },
}

// ── EventSink ───────────────────────────────────────────────────────────────

/// Collects [`DialogueEvent`]s during a session for deferred processing.
///
/// Replace with your own channel/callback type by wrapping the runner.
#[derive(Debug, Clone, Default)]
pub struct EventSink {
    events: Vec<DialogueEvent>,
}

impl EventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: DialogueEvent) {
        self.events.push(event);
    }

    /// Drain all accumulated events, leaving the sink empty.
    pub fn drain(&mut self) -> Vec<DialogueEvent> {
        std::mem::take(&mut self.events)
    }

    /// Peek without draining.
    pub fn pending(&self) -> &[DialogueEvent] {
        &self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

// ── DialogueVarTable ─────────────────────────────────────────────────────────

/// A typed variable store used by the dialogue runtime.
///
/// Wraps `HashMap<String, DialogueVar>` with ergonomic helpers for common
/// patterns: increment, clamp, typed getters, merge.
#[derive(Debug, Clone, Default)]
pub struct DialogueVarTable {
    inner: std::collections::HashMap<String, DialogueVar>,
}

impl DialogueVarTable {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<DialogueVar>) {
        self.inner.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&DialogueVar> {
        self.inner.get(name)
    }

    pub fn get_or(&self, name: &str, default: DialogueVar) -> DialogueVar {
        self.inner.get(name).cloned().unwrap_or(default)
    }

    pub fn get_int(&self, name: &str) -> i64 {
        self.inner.get(name).map_or(0, |v| v.as_int())
    }

    pub fn get_float(&self, name: &str) -> f64 {
        self.inner.get(name).map_or(0.0, |v| v.as_float())
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.inner.get(name).map_or(false, |v| v.as_bool())
    }

    pub fn get_str(&self, name: &str) -> String {
        self.inner.get(name).map_or_else(String::new, |v| v.as_str())
    }

    pub fn remove(&mut self, name: &str) -> Option<DialogueVar> {
        self.inner.remove(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.inner.contains_key(name)
    }

    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &DialogueVar)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Merge `other` into `self`; keys from `other` overwrite.
    pub fn merge(&mut self, other: &DialogueVarTable) {
        for (k, v) in &other.inner {
            self.inner.insert(k.clone(), v.clone());
        }
    }

    pub fn raw(&self) -> &std::collections::HashMap<String, DialogueVar> {
        &self.inner
    }

    /// Increment an integer variable by `delta`.  Initialises to 0 if absent.
    pub fn increment(&mut self, name: &str, delta: i64) {
        let current = self.get_int(name);
        self.set(name, DialogueVar::Int(current + delta));
    }

    /// Clamp an integer variable to `[min, max]`.
    pub fn clamp_int(&mut self, name: &str, min: i64, max: i64) {
        let current = self.get_int(name);
        self.set(name, DialogueVar::Int(current.clamp(min, max)));
    }

    pub fn clear(&mut self) { self.inner.clear(); }
}

impl From<std::collections::HashMap<String, DialogueVar>> for DialogueVarTable {
    fn from(map: std::collections::HashMap<String, DialogueVar>) -> Self {
        Self { inner: map }
    }
}

// ── FlagSet ──────────────────────────────────────────────────────────────────

/// A set of boolean flags used by the dialogue runtime.
///
/// Flags are lighter than `Bool` variables — no value, just presence/absence.
/// Typical uses: `"met_alice"`, `"quest_started"`, `"door_unlocked"`.
#[derive(Debug, Clone, Default)]
pub struct FlagSet {
    inner: std::collections::HashSet<String>,
}

impl FlagSet {
    pub fn new() -> Self { Self::default() }

    pub fn set(&mut self, name: impl Into<String>) {
        self.inner.insert(name.into());
    }

    pub fn clear_flag(&mut self, name: &str) -> bool {
        self.inner.remove(name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.inner.contains(name)
    }

    /// Toggle: set if absent, clear if present.  Returns the new state.
    pub fn toggle(&mut self, name: &str) -> bool {
        if self.inner.contains(name) {
            self.inner.remove(name);
            false
        } else {
            self.inner.insert(name.to_string());
            true
        }
    }

    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.inner.iter().map(|s| s.as_str())
    }

    pub fn raw(&self) -> &std::collections::HashSet<String> {
        &self.inner
    }

    pub fn merge(&mut self, other: &FlagSet) {
        for flag in &other.inner {
            self.inner.insert(flag.clone());
        }
    }

    pub fn clear_all(&mut self) { self.inner.clear(); }
}

impl From<std::collections::HashSet<String>> for FlagSet {
    fn from(set: std::collections::HashSet<String>) -> Self {
        Self { inner: set }
    }
}

// ── LocalisedText ─────────────────────────────────────────────────────────────

/// A piece of text with variants in multiple locales.
///
/// Used by `Say` nodes when you need the same dialogue tree to serve multiple
/// languages without duplicating the graph structure.
#[derive(Debug, Clone, Default)]
pub struct LocalisedText {
    translations:    std::collections::HashMap<String, String>,
    fallback_locale: String,
}

impl LocalisedText {
    pub fn new(locale: impl Into<String>, text: impl Into<String>) -> Self {
        let locale = locale.into();
        let text   = text.into();
        let mut translations = std::collections::HashMap::new();
        translations.insert(locale.clone(), text);
        Self { translations, fallback_locale: locale }
    }

    pub fn add(mut self, locale: impl Into<String>, text: impl Into<String>) -> Self {
        self.translations.insert(locale.into(), text.into());
        self
    }

    pub fn get(&self, locale: &str) -> &str {
        self.translations
            .get(locale)
            .or_else(|| self.translations.get(&self.fallback_locale))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn with_fallback(mut self, locale: impl Into<String>) -> Self {
        self.fallback_locale = locale.into();
        self
    }

    pub fn locales(&self) -> Vec<&str> {
        self.translations.keys().map(|s| s.as_str()).collect()
    }

    pub fn has_locale(&self, locale: &str) -> bool {
        self.translations.contains_key(locale)
    }
}

// ── DialogueVarDiff ───────────────────────────────────────────────────────────

/// Records the difference between two variable table snapshots.
#[derive(Debug, Clone, Default)]
pub struct DialogueVarDiff {
    pub added:   Vec<(String, DialogueVar)>,
    pub removed: Vec<(String, DialogueVar)>,
    pub changed: Vec<(String, DialogueVar, DialogueVar)>,
}

impl DialogueVarDiff {
    pub fn compute(
        before: &std::collections::HashMap<String, DialogueVar>,
        after:  &std::collections::HashMap<String, DialogueVar>,
    ) -> Self {
        let mut diff = DialogueVarDiff::default();
        for (k, new_v) in after {
            match before.get(k) {
                None          => diff.added.push((k.clone(), new_v.clone())),
                Some(old_v) if old_v != new_v => {
                    diff.changed.push((k.clone(), old_v.clone(), new_v.clone()))
                }
                _ => {}
            }
        }
        for (k, old_v) in before {
            if !after.contains_key(k) {
                diff.removed.push((k.clone(), old_v.clone()));
            }
        }
        diff
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

// ── DialogueTag ───────────────────────────────────────────────────────────────

/// A dot-separated hierarchical tag: e.g. `"quest.main.chapter1"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogueTag {
    raw: String,
}

impl DialogueTag {
    pub fn new(tag: impl Into<String>) -> Self { Self { raw: tag.into() } }
    pub fn as_str(&self) -> &str { &self.raw }

    pub fn parts(&self) -> Vec<&str> { self.raw.split('.').collect() }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        if self.raw == prefix { return true; }
        self.raw.starts_with(&format!("{}.", prefix))
    }

    pub fn namespace(&self) -> &str {
        self.raw.split('.').next().unwrap_or(&self.raw)
    }

    pub fn tail(&self) -> &str {
        match self.raw.find('.') {
            Some(pos) => &self.raw[pos + 1..],
            None      => "",
        }
    }
}

impl std::fmt::Display for DialogueTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raw)
    }
}

impl From<&str>   for DialogueTag { fn from(s: &str)   -> Self { DialogueTag::new(s) } }
impl From<String> for DialogueTag { fn from(s: String) -> Self { DialogueTag::new(s) } }

// ── PortraitSpec ──────────────────────────────────────────────────────────────

/// Describes a portrait frame to display during a `Say` node.
#[derive(Debug, Clone)]
pub struct PortraitSpec {
    pub key:     String,
    pub flipped: bool,
    pub tint:    [f32; 4],
    pub scale:   f32,
    pub offset:  [f32; 2],
}

impl PortraitSpec {
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into(), flipped: false, tint: [1.0; 4], scale: 1.0, offset: [0.0; 2] }
    }

    pub fn flipped(mut self) -> Self { self.flipped = true; self }

    pub fn with_tint(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.tint = [r, g, b, a]; self
    }

    pub fn with_scale(mut self, scale: f32) -> Self { self.scale = scale; self }

    pub fn with_offset(mut self, x: f32, y: f32) -> Self { self.offset = [x, y]; self }
}

impl Default for PortraitSpec {
    fn default() -> Self { Self::new("") }
}

// ── TypewriterState ───────────────────────────────────────────────────────────

/// Tracks the typewriter-reveal animation for a single line of dialogue.
#[derive(Debug, Clone)]
pub struct TypewriterState {
    full_text:      String,
    chars_revealed: f32,
    chars_per_sec:  f32,
    finished:       bool,
}

impl TypewriterState {
    pub fn new(text: impl Into<String>, chars_per_sec: f32) -> Self {
        let full_text = text.into();
        let finished  = full_text.is_empty();
        Self { full_text, chars_revealed: 0.0, chars_per_sec: chars_per_sec.max(1.0), finished }
    }

    pub fn update(&mut self, delta: f32) {
        if self.finished { return; }
        self.chars_revealed += self.chars_per_sec * delta;
        let total = self.full_text.chars().count() as f32;
        if self.chars_revealed >= total {
            self.chars_revealed = total;
            self.finished = true;
        }
    }

    pub fn visible_text(&self) -> &str {
        if self.finished { return &self.full_text; }
        let n = self.chars_revealed as usize;
        let byte_idx = self.full_text
            .char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(self.full_text.len());
        &self.full_text[..byte_idx]
    }

    pub fn skip(&mut self) {
        self.chars_revealed = self.full_text.chars().count() as f32;
        self.finished = true;
    }

    pub fn is_finished(&self) -> bool { self.finished }

    pub fn progress(&self) -> f32 {
        if self.full_text.is_empty() { return 1.0; }
        let total = self.full_text.chars().count() as f32;
        (self.chars_revealed / total).clamp(0.0, 1.0)
    }

    pub fn full_text(&self) -> &str { &self.full_text }
}

// ── DialogueError ─────────────────────────────────────────────────────────────

/// Top-level error type wrapping both runner and validation failures.
#[derive(Debug, Clone)]
pub enum DialogueError {
    Runner(runner::RunnerError),
    Validation(tree::ValidationError),
    MissingLocale { key: String, locale: String },
    Other(String),
}

impl std::fmt::Display for DialogueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DialogueError::Runner(e)     => write!(f, "runner error: {}", e),
            DialogueError::Validation(e) => write!(f, "validation error: {:?}", e),
            DialogueError::MissingLocale { key, locale } =>
                write!(f, "missing locale '{}' for key '{}'", locale, key),
            DialogueError::Other(msg)    => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for DialogueError {}

impl From<runner::RunnerError> for DialogueError {
    fn from(e: runner::RunnerError) -> Self { DialogueError::Runner(e) }
}

impl From<tree::ValidationError> for DialogueError {
    fn from(e: tree::ValidationError) -> Self { DialogueError::Validation(e) }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_display() {
        assert_eq!(NodeId(42).to_string(), "Node(42)");
        assert_eq!(DialogueId(7).to_string(), "Dialogue(7)");
        assert_eq!(SpeakerId(0).to_string(), "Speaker(0)");
    }

    #[test]
    fn node_id_constants() {
        assert_eq!(NodeId::INVALID, NodeId(u32::MAX));
        assert_eq!(SpeakerId::NARRATOR, SpeakerId(0));
    }

    #[test]
    fn speaker_registry_register_and_get() {
        let mut reg = SpeakerRegistry::new();
        let s = Speaker::new(SpeakerId(1), "Alice")
            .with_portrait("alice_neutral")
            .with_voice("voice_alto");
        reg.register(s);
        let found = reg.get(SpeakerId(1)).expect("speaker must be present");
        assert_eq!(found.name, "Alice");
        assert_eq!(found.portrait_key.as_deref(), Some("alice_neutral"));
        assert_eq!(found.voice_id.as_deref(), Some("voice_alto"));
    }

    #[test]
    fn speaker_registry_lookup_by_name() {
        let mut reg = SpeakerRegistry::new();
        reg.register(Speaker::new(SpeakerId(2), "Bob"));
        reg.register(Speaker::new(SpeakerId(3), "Carol"));
        assert!(reg.lookup_by_name("Bob").is_some());
        assert!(reg.lookup_by_name("Nobody").is_none());
    }

    #[test]
    fn speaker_registry_remove() {
        let mut reg = SpeakerRegistry::new();
        reg.register(Speaker::new(SpeakerId(5), "Eve"));
        assert_eq!(reg.len(), 1);
        let removed = reg.remove(SpeakerId(5));
        assert!(removed.is_some());
        assert!(reg.is_empty());
    }

    #[test]
    fn emotion_pitch_bias_neutral() {
        assert!((Emotion::Neutral.pitch_bias() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn emotion_all_coverage() {
        // Every variant must appear in ALL.
        assert_eq!(Emotion::ALL.len(), 8);
        for e in Emotion::ALL {
            let _ = e.label();
            let _ = e.pitch_bias();
        }
    }

    #[test]
    fn dialogue_var_coercions() {
        assert_eq!(DialogueVar::Bool(true).as_int(), 1);
        assert_eq!(DialogueVar::Bool(false).as_int(), 0);
        assert_eq!(DialogueVar::Int(7).as_float(), 7.0);
        assert_eq!(DialogueVar::Float(3.5).as_bool(), true);
        assert_eq!(DialogueVar::Str("".to_string()).as_bool(), false);
        assert_eq!(DialogueVar::Str("42".to_string()).as_int(), 42);
    }

    #[test]
    fn dialogue_var_comparisons() {
        let a = DialogueVar::Int(3);
        let b = DialogueVar::Int(5);
        assert!(a.lt(&b));
        assert!(b.gt(&a));
        assert!(!a.gt(&b));
    }

    #[test]
    fn dialogue_var_from_impls() {
        let _ = DialogueVar::from(true);
        let _ = DialogueVar::from(42i64);
        let _ = DialogueVar::from(42i32);
        let _ = DialogueVar::from(3.14f64);
        let _ = DialogueVar::from(1.0f32);
        let _ = DialogueVar::from("hello");
        let _ = DialogueVar::from("world".to_string());
    }

    #[test]
    fn event_sink_drain() {
        let mut sink = EventSink::new();
        sink.push(DialogueEvent::Started(DialogueId(1)));
        sink.push(DialogueEvent::Ended(DialogueId(1)));
        assert_eq!(sink.pending().len(), 2);
        let events = sink.drain();
        assert_eq!(events.len(), 2);
        assert!(sink.is_empty());
    }

    #[test]
    fn dialogue_event_variable_changed() {
        let evt = DialogueEvent::VariableChanged {
            name:  "reputation".to_string(),
            value: DialogueVar::Int(10),
        };
        let _cloned = evt.clone();
    }

    // ── DialogueVarTable ──────────────────────────────────────────────────

    #[test]
    fn var_table_set_get() {
        let mut t = DialogueVarTable::new();
        t.set("gold", 50i64);
        assert_eq!(t.get_int("gold"), 50);
        assert!(t.get_bool("gold"));
        assert_eq!(t.get_float("gold"), 50.0);
    }

    #[test]
    fn var_table_get_or_default() {
        let t = DialogueVarTable::new();
        let v = t.get_or("missing", DialogueVar::Int(99));
        assert_eq!(v, DialogueVar::Int(99));
    }

    #[test]
    fn var_table_increment_and_clamp() {
        let mut t = DialogueVarTable::new();
        t.increment("score", 10);
        t.increment("score", 5);
        assert_eq!(t.get_int("score"), 15);
        t.clamp_int("score", 0, 12);
        assert_eq!(t.get_int("score"), 12);
    }

    #[test]
    fn var_table_merge() {
        let mut a = DialogueVarTable::new();
        a.set("x", 1i64);
        let mut b = DialogueVarTable::new();
        b.set("x", 2i64);
        b.set("y", 3i64);
        a.merge(&b);
        assert_eq!(a.get_int("x"), 2);
        assert_eq!(a.get_int("y"), 3);
    }

    #[test]
    fn var_table_remove() {
        let mut t = DialogueVarTable::new();
        t.set("temp", "hello");
        assert!(t.contains("temp"));
        t.remove("temp");
        assert!(!t.contains("temp"));
        assert!(t.is_empty());
    }

    #[test]
    fn var_table_iter() {
        let mut t = DialogueVarTable::new();
        t.set("a", 1i64);
        t.set("b", 2i64);
        assert_eq!(t.iter().count(), 2);
    }

    #[test]
    fn var_table_get_str() {
        let mut t = DialogueVarTable::new();
        t.set("name", "hero");
        assert_eq!(t.get_str("name"), "hero");
        assert_eq!(t.get_str("absent"), "");
    }

    // ── FlagSet ────────────────────────────────────────────────────────────

    #[test]
    fn flag_set_basic() {
        let mut fs = FlagSet::new();
        fs.set("quest_started");
        assert!(fs.has("quest_started"));
        assert!(!fs.has("other"));
        fs.clear_flag("quest_started");
        assert!(!fs.has("quest_started"));
    }

    #[test]
    fn flag_set_toggle() {
        let mut fs = FlagSet::new();
        let s1 = fs.toggle("flag_a");
        assert!(s1);
        let s2 = fs.toggle("flag_a");
        assert!(!s2);
    }

    #[test]
    fn flag_set_merge() {
        let mut a = FlagSet::new();
        a.set("f1");
        let mut b = FlagSet::new();
        b.set("f2");
        a.merge(&b);
        assert!(a.has("f1"));
        assert!(a.has("f2"));
    }

    #[test]
    fn flag_set_clear_all() {
        let mut fs = FlagSet::new();
        fs.set("a");
        fs.set("b");
        fs.clear_all();
        assert!(fs.is_empty());
    }

    #[test]
    fn flag_set_iter() {
        let mut fs = FlagSet::new();
        fs.set("x");
        fs.set("y");
        fs.set("z");
        assert_eq!(fs.iter().count(), 3);
    }

    // ── LocalisedText ─────────────────────────────────────────────────────

    #[test]
    fn localised_text_basic() {
        let lt = LocalisedText::new("en-US", "Hello!")
            .add("ja-JP", "こんにちは！")
            .add("fr-FR", "Bonjour !");
        assert_eq!(lt.get("en-US"), "Hello!");
        assert_eq!(lt.get("ja-JP"), "こんにちは！");
        assert_eq!(lt.get("fr-FR"), "Bonjour !");
    }

    #[test]
    fn localised_text_fallback() {
        let lt = LocalisedText::new("en-US", "Fallback text");
        assert_eq!(lt.get("de-DE"), "Fallback text");
    }

    #[test]
    fn localised_text_locales() {
        let lt = LocalisedText::new("en-US", "Hi").add("es-MX", "Hola");
        assert_eq!(lt.locales().len(), 2);
        assert!(lt.has_locale("en-US"));
        assert!(!lt.has_locale("zh-CN"));
    }

    // ── DialogueVarDiff ────────────────────────────────────────────────────

    #[test]
    fn var_diff_added() {
        let before = std::collections::HashMap::new();
        let mut after = std::collections::HashMap::new();
        after.insert("x".to_string(), DialogueVar::Int(5));
        let diff = DialogueVarDiff::compute(&before, &after);
        assert_eq!(diff.added.len(), 1);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn var_diff_removed() {
        let mut before = std::collections::HashMap::new();
        before.insert("x".to_string(), DialogueVar::Int(5));
        let after = std::collections::HashMap::new();
        let diff = DialogueVarDiff::compute(&before, &after);
        assert_eq!(diff.removed.len(), 1);
    }

    #[test]
    fn var_diff_changed() {
        let mut before = std::collections::HashMap::new();
        before.insert("x".to_string(), DialogueVar::Int(5));
        let mut after = std::collections::HashMap::new();
        after.insert("x".to_string(), DialogueVar::Int(10));
        let diff = DialogueVarDiff::compute(&before, &after);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.total_changes(), 1);
    }

    #[test]
    fn var_diff_no_change() {
        let mut map = std::collections::HashMap::new();
        map.insert("x".to_string(), DialogueVar::Int(5));
        let diff = DialogueVarDiff::compute(&map, &map);
        assert!(diff.is_empty());
    }

    // ── DialogueTag ────────────────────────────────────────────────────────

    #[test]
    fn tag_namespace_and_tail() {
        let tag = DialogueTag::new("quest.main.chapter1");
        assert_eq!(tag.namespace(), "quest");
        assert_eq!(tag.tail(), "main.chapter1");
        assert!(tag.has_prefix("quest"));
        assert!(tag.has_prefix("quest.main"));
        assert!(!tag.has_prefix("ques"));
    }

    #[test]
    fn tag_no_dot() {
        let tag = DialogueTag::new("tutorial");
        assert_eq!(tag.namespace(), "tutorial");
        assert_eq!(tag.tail(), "");
        assert_eq!(tag.parts().len(), 1);
        assert!(tag.has_prefix("tutorial"));
    }

    #[test]
    fn tag_display_and_from() {
        let t1: DialogueTag = "npc.merchant".into();
        let t2: DialogueTag = "npc.merchant".to_string().into();
        assert_eq!(t1, t2);
        assert_eq!(t1.to_string(), "npc.merchant");
    }

    // ── PortraitSpec ───────────────────────────────────────────────────────

    #[test]
    fn portrait_spec_defaults() {
        let p = PortraitSpec::new("alice_neutral");
        assert_eq!(p.key, "alice_neutral");
        assert!(!p.flipped);
        assert_eq!(p.scale, 1.0);
    }

    #[test]
    fn portrait_spec_builder() {
        let p = PortraitSpec::new("bob_angry")
            .flipped()
            .with_tint(1.0, 0.5, 0.5, 1.0)
            .with_scale(1.2)
            .with_offset(-10.0, 5.0);
        assert!(p.flipped);
        assert!((p.scale - 1.2).abs() < f32::EPSILON);
        assert_eq!(p.offset, [-10.0, 5.0]);
    }

    // ── TypewriterState ────────────────────────────────────────────────────

    #[test]
    fn typewriter_reveals_incrementally() {
        let mut tw = TypewriterState::new("Hello", 5.0);
        assert_eq!(tw.visible_text(), "");
        tw.update(0.2);
        assert_eq!(tw.visible_text(), "H");
        tw.update(0.2);
        assert_eq!(tw.visible_text(), "He");
        tw.update(10.0);
        assert!(tw.is_finished());
        assert_eq!(tw.visible_text(), "Hello");
    }

    #[test]
    fn typewriter_skip() {
        let mut tw = TypewriterState::new("Quick brown fox", 1.0);
        assert!(!tw.is_finished());
        tw.skip();
        assert!(tw.is_finished());
        assert_eq!(tw.visible_text(), "Quick brown fox");
    }

    #[test]
    fn typewriter_progress() {
        let mut tw = TypewriterState::new("1234", 4.0);
        tw.update(0.5); // 2 chars revealed = 50%
        let p = tw.progress();
        assert!((p - 0.5).abs() < 0.01, "expected ~0.5, got {}", p);
    }

    #[test]
    fn typewriter_empty_string() {
        let tw = TypewriterState::new("", 10.0);
        assert!(tw.is_finished());
        assert_eq!(tw.progress(), 1.0);
    }

    #[test]
    fn typewriter_unicode_safe() {
        // "こんにちは" = 5 chars, each 3 bytes in UTF-8.
        let mut tw = TypewriterState::new("こんにちは", 5.0);
        tw.update(0.2); // reveal 1 char
        assert_eq!(tw.visible_text(), "こ");
    }

    // ── DialogueError ──────────────────────────────────────────────────────

    #[test]
    fn dialogue_error_display() {
        let e = DialogueError::Other("something went wrong".to_string());
        assert!(e.to_string().contains("something went wrong"));
    }

    #[test]
    fn dialogue_error_from_runner_error() {
        let re = runner::RunnerError::NotRunning;
        let de = DialogueError::from(re);
        assert!(matches!(de, DialogueError::Runner(_)));
    }

    #[test]
    fn dialogue_error_missing_locale() {
        let e = DialogueError::MissingLocale {
            key:    "greeting".to_string(),
            locale: "zh-CN".to_string(),
        };
        let s = e.to_string();
        assert!(s.contains("zh-CN"));
        assert!(s.contains("greeting"));
    }
}
