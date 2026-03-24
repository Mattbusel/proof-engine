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
        // Just confirm it clones without panic.
        let _cloned = evt.clone();
    }
}
