//! Dialogue tree data model.
//!
//! Provides the full node graph, condition evaluation, a fluent builder API,
//! and a library for registering and querying multiple trees.
//!
//! # Node variants
//! - [`DialogueNode::Say`]         — a line of speech
//! - [`DialogueNode::Choice`]      — player-selected branch
//! - [`DialogueNode::Branch`]      — conditional fork
//! - [`DialogueNode::SetVar`]      — write a variable
//! - [`DialogueNode::CallScript`]  — invoke an external function
//! - [`DialogueNode::Jump`]        — unconditional goto
//! - [`DialogueNode::End`]         — terminate conversation
//! - [`DialogueNode::RandomChoice`]— weighted-random branch
//! - [`DialogueNode::Wait`]        — pause for a duration
//! - [`DialogueNode::PlayAnim`]    — trigger a character animation
//! - [`DialogueNode::Camera`]      — camera director command

use std::collections::{HashMap, HashSet};

use crate::dialogue::{DialogueId, DialogueVar, Emotion, NodeId, SpeakerId};

// ── CameraAction ─────────────────────────────────────────────────────────────

/// Director instruction embedded in a [`DialogueNode::Camera`] node.
#[derive(Debug, Clone, PartialEq)]
pub enum CameraAction {
    /// Zoom / pan the camera to frame a speaker.
    FocusOn(SpeakerId),
    /// Move the camera to world-space coordinates.
    PanTo { x: f32, y: f32, z: f32 },
    /// Return to the default overworld/game camera.
    Restore,
}

impl CameraAction {
    pub fn focus_on(speaker: SpeakerId) -> Self {
        CameraAction::FocusOn(speaker)
    }

    pub fn pan_to(x: f32, y: f32, z: f32) -> Self {
        CameraAction::PanTo { x, y, z }
    }

    pub fn restore() -> Self {
        CameraAction::Restore
    }
}

// ── ChoiceOption ─────────────────────────────────────────────────────────────

/// One selectable option within a [`DialogueNode::Choice`] node.
#[derive(Debug, Clone)]
pub struct ChoiceOption {
    /// Localised display text shown to the player.
    pub text: String,
    /// Node to jump to when this option is selected.
    pub next: NodeId,
    /// If `Some`, this option is only visible when the condition is true.
    pub condition: Option<Condition>,
    /// If `true`, this option disappears after it has been chosen once.
    pub once_only: bool,
    /// Arbitrary string tags (e.g. "aggressive", "quest:main") for filtering.
    pub tags: Vec<String>,
}

impl ChoiceOption {
    /// Minimal constructor.
    pub fn new(text: impl Into<String>, next: NodeId) -> Self {
        Self {
            text:      text.into(),
            next,
            condition: None,
            once_only: false,
            tags:      Vec::new(),
        }
    }

    pub fn with_condition(mut self, cond: Condition) -> Self {
        self.condition = Some(cond);
        self
    }

    pub fn once_only(mut self) -> Self {
        self.once_only = true;
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

// ── Condition ────────────────────────────────────────────────────────────────

/// A boolean expression that can guard branches and choice options.
///
/// Conditions are fully recursive; `And` / `Or` contain `Vec<Condition>` so
/// arbitrarily deep nesting is possible without extra boxing (the `Not` variant
/// uses `Box<Condition>` because a recursive enum variant must be sized).
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    /// Variable equals a specific value (equality comparison).
    VarEquals(String, DialogueVar),
    /// Variable is strictly greater than the given value.
    VarGreater(String, DialogueVar),
    /// Variable is strictly less than the given value.
    VarLess(String, DialogueVar),
    /// A named flag is present in the flags set.
    HasFlag(String),
    /// Logical NOT of the inner condition.
    Not(Box<Condition>),
    /// All child conditions must be true.
    And(Vec<Condition>),
    /// At least one child condition must be true.
    Or(Vec<Condition>),
    /// Always evaluates to `true` (useful for unconditional default branches).
    Always,
    /// Always evaluates to `false` (useful for disabled/stub branches).
    Never,
}

impl Condition {
    /// Evaluate the condition against the current variable table and flag set.
    pub fn evaluate(
        &self,
        vars:  &HashMap<String, DialogueVar>,
        flags: &HashSet<String>,
    ) -> bool {
        match self {
            Condition::Always => true,
            Condition::Never  => false,

            Condition::HasFlag(name) => flags.contains(name),

            Condition::VarEquals(name, expected) => {
                vars.get(name).map_or(false, |v| v == expected)
            }

            Condition::VarGreater(name, threshold) => {
                vars.get(name).map_or(false, |v| v.gt(threshold))
            }

            Condition::VarLess(name, threshold) => {
                vars.get(name).map_or(false, |v| v.lt(threshold))
            }

            Condition::Not(inner) => !inner.evaluate(vars, flags),

            Condition::And(children) => {
                children.iter().all(|c| c.evaluate(vars, flags))
            }

            Condition::Or(children) => {
                children.iter().any(|c| c.evaluate(vars, flags))
            }
        }
    }

    // ── Convenience constructors ──────────────────────────────────────────

    pub fn var_equals(name: impl Into<String>, value: impl Into<DialogueVar>) -> Self {
        Condition::VarEquals(name.into(), value.into())
    }

    pub fn var_greater(name: impl Into<String>, value: impl Into<DialogueVar>) -> Self {
        Condition::VarGreater(name.into(), value.into())
    }

    pub fn var_less(name: impl Into<String>, value: impl Into<DialogueVar>) -> Self {
        Condition::VarLess(name.into(), value.into())
    }

    pub fn has_flag(name: impl Into<String>) -> Self {
        Condition::HasFlag(name.into())
    }

    pub fn not(inner: Condition) -> Self {
        Condition::Not(Box::new(inner))
    }

    pub fn and(conditions: Vec<Condition>) -> Self {
        Condition::And(conditions)
    }

    pub fn or(conditions: Vec<Condition>) -> Self {
        Condition::Or(conditions)
    }
}

// ── DialogueNode ─────────────────────────────────────────────────────────────

/// Every node in a dialogue graph.
///
/// All variants carry an `id` so the graph can be traversed and nodes
/// referenced by [`NodeId`] regardless of variant.
#[derive(Debug, Clone)]
pub enum DialogueNode {
    // ── Speech / narration ────────────────────────────────────────────────

    /// A character speaks a line of text.
    Say {
        id:        NodeId,
        speaker:   SpeakerId,
        text:      String,
        emotion:   Emotion,
        /// Optional audio cue key (VO clip, ambient, etc.)
        audio_key: Option<String>,
        /// Node to advance to after the player confirms the line.
        next:      Option<NodeId>,
    },

    // ── Player agency ─────────────────────────────────────────────────────

    /// Present the player with a list of response options.
    Choice {
        id:      NodeId,
        speaker: SpeakerId,
        /// Optional introductory line before choices are shown.
        prompt:  Option<String>,
        options: Vec<ChoiceOption>,
    },

    // ── Control flow ──────────────────────────────────────────────────────

    /// Evaluate a condition and branch accordingly.
    Branch {
        id:       NodeId,
        condition: Condition,
        if_true:  NodeId,
        /// Jump here if the condition is false; if `None` and condition fails,
        /// the dialogue ends.
        if_false: Option<NodeId>,
    },

    /// Write a value to the runtime variable table and continue.
    SetVar {
        id:   NodeId,
        name: String,
        value: DialogueVar,
        next: Option<NodeId>,
    },

    /// Invoke an external game-side function (quest flags, inventory, etc.)
    CallScript {
        id:       NodeId,
        function: String,
        args:     Vec<DialogueVar>,
        next:     Option<NodeId>,
    },

    /// Unconditional jump to another node.
    Jump {
        id:     NodeId,
        target: NodeId,
    },

    /// Terminate the dialogue.
    End {
        id: NodeId,
    },

    // ── Randomness ────────────────────────────────────────────────────────

    /// Pick a branch at random, weighted by the associated `f32` weight.
    ///
    /// Weights do not need to sum to 1; they are normalised at runtime.
    RandomChoice {
        id:      NodeId,
        /// `(target_node, weight)` pairs — weight must be ≥ 0.
        options: Vec<(NodeId, f32)>,
    },

    // ── Timing ────────────────────────────────────────────────────────────

    /// Pause execution for `duration` game-seconds, then continue.
    Wait {
        id:       NodeId,
        duration: f32,
        next:     NodeId,
    },

    // ── Presentation ─────────────────────────────────────────────────────

    /// Trigger a character animation clip.
    PlayAnim {
        id:       NodeId,
        speaker:  SpeakerId,
        anim_key: String,
        next:     NodeId,
    },

    /// Issue a camera director command.
    Camera {
        id:     NodeId,
        action: CameraAction,
        next:   NodeId,
    },
}

impl DialogueNode {
    /// Return the [`NodeId`] common to every variant.
    pub fn id(&self) -> NodeId {
        match self {
            DialogueNode::Say        { id, .. } => *id,
            DialogueNode::Choice     { id, .. } => *id,
            DialogueNode::Branch     { id, .. } => *id,
            DialogueNode::SetVar     { id, .. } => *id,
            DialogueNode::CallScript { id, .. } => *id,
            DialogueNode::Jump       { id, .. } => *id,
            DialogueNode::End        { id }     => *id,
            DialogueNode::RandomChoice { id, .. } => *id,
            DialogueNode::Wait       { id, .. } => *id,
            DialogueNode::PlayAnim   { id, .. } => *id,
            DialogueNode::Camera     { id, .. } => *id,
        }
    }

    /// Human-readable variant name for debug / editor use.
    pub fn kind_name(&self) -> &'static str {
        match self {
            DialogueNode::Say        { .. } => "Say",
            DialogueNode::Choice     { .. } => "Choice",
            DialogueNode::Branch     { .. } => "Branch",
            DialogueNode::SetVar     { .. } => "SetVar",
            DialogueNode::CallScript { .. } => "CallScript",
            DialogueNode::Jump       { .. } => "Jump",
            DialogueNode::End        { .. } => "End",
            DialogueNode::RandomChoice { .. } => "RandomChoice",
            DialogueNode::Wait       { .. } => "Wait",
            DialogueNode::PlayAnim   { .. } => "PlayAnim",
            DialogueNode::Camera     { .. } => "Camera",
        }
    }

    /// Return the "next" node(s) for static graph traversal and validation.
    ///
    /// For `Choice` nodes this includes every option's target; for `Branch` it
    /// includes both branches; for `RandomChoice` it includes every option.
    pub fn successors(&self) -> Vec<NodeId> {
        match self {
            DialogueNode::Say        { next, .. } => next.iter().copied().collect(),
            DialogueNode::SetVar     { next, .. } => next.iter().copied().collect(),
            DialogueNode::CallScript { next, .. } => next.iter().copied().collect(),
            DialogueNode::Jump       { target, .. } => vec![*target],
            DialogueNode::End        { .. }          => vec![],
            DialogueNode::Wait       { next, .. }    => vec![*next],
            DialogueNode::PlayAnim   { next, .. }    => vec![*next],
            DialogueNode::Camera     { next, .. }    => vec![*next],
            DialogueNode::Branch { if_true, if_false, .. } => {
                let mut v = vec![*if_true];
                if let Some(f) = if_false { v.push(*f); }
                v
            }
            DialogueNode::Choice { options, .. } => {
                options.iter().map(|o| o.next).collect()
            }
            DialogueNode::RandomChoice { options, .. } => {
                options.iter().map(|(n, _)| *n).collect()
            }
        }
    }

    /// Returns `true` if this node will pause execution waiting for input.
    pub fn is_blocking(&self) -> bool {
        matches!(self, DialogueNode::Say { .. } | DialogueNode::Choice { .. })
    }

    /// Returns `true` if this node is a terminal node.
    pub fn is_terminal(&self) -> bool {
        matches!(self, DialogueNode::End { .. })
    }
}

// ── DialogueMeta ─────────────────────────────────────────────────────────────

/// Authoring metadata attached to a dialogue tree.
#[derive(Debug, Clone, Default)]
pub struct DialogueMeta {
    /// Human-readable name of the conversation.
    pub title:   String,
    /// Content author or writer credit.
    pub author:  String,
    /// Searchable tags (e.g. "tutorial", "main_quest", "npc_merchant").
    pub tags:    Vec<String>,
    /// Semver-style version string for asset pipeline tracking.
    pub version: String,
    /// BCP 47 locale code (e.g. `"en-US"`, `"ja-JP"`).
    pub locale:  String,
}

impl DialogueMeta {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title:   title.into(),
            author:  String::new(),
            tags:    Vec::new(),
            version: "0.1.0".to_string(),
            locale:  "en-US".to_string(),
        }
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

// ── DialogueTree ─────────────────────────────────────────────────────────────

/// A complete, self-contained dialogue graph.
///
/// The graph is stored as a flat `HashMap<NodeId, DialogueNode>` which allows
/// O(1) random access; references between nodes are expressed as [`NodeId`]
/// values embedded in each node variant.
#[derive(Debug, Clone)]
pub struct DialogueTree {
    /// Unique identifier for this tree within its [`DialogueLibrary`].
    pub id:       DialogueId,
    /// Flat node store.
    pub nodes:    HashMap<NodeId, DialogueNode>,
    /// The first node the runner visits upon starting this tree.
    pub start:    NodeId,
    /// Authoring metadata.
    pub metadata: DialogueMeta,
}

impl DialogueTree {
    /// Create an empty tree; nodes can be inserted via [`nodes`].
    pub fn new(id: DialogueId, start: NodeId, metadata: DialogueMeta) -> Self {
        Self {
            id,
            nodes: HashMap::new(),
            start,
            metadata,
        }
    }

    /// Insert a node, replacing any existing node with the same ID.
    pub fn insert(&mut self, node: DialogueNode) {
        self.nodes.insert(node.id(), node);
    }

    /// Look up a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&DialogueNode> {
        self.nodes.get(&id)
    }

    /// Total node count.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Validate reachability and referential integrity.
    ///
    /// Returns a list of problems found; an empty vec means the tree is clean.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Start node must exist.
        if !self.nodes.contains_key(&self.start) {
            errors.push(ValidationError::MissingStart(self.start));
        }

        for (id, node) in &self.nodes {
            for successor in node.successors() {
                if !self.nodes.contains_key(&successor) {
                    errors.push(ValidationError::DanglingReference {
                        from:    *id,
                        to:      successor,
                        context: node.kind_name(),
                    });
                }
            }

            // RandomChoice: weights must be non-negative.
            if let DialogueNode::RandomChoice { options, .. } = node {
                for (target, weight) in options {
                    if *weight < 0.0 {
                        errors.push(ValidationError::NegativeWeight {
                            node:   *id,
                            target: *target,
                        });
                    }
                }
            }
        }

        errors
    }

    /// Collect all [`NodeId`]s reachable from the start node via BFS.
    pub fn reachable_nodes(&self) -> HashSet<NodeId> {
        let mut visited = HashSet::new();
        let mut queue   = std::collections::VecDeque::new();
        queue.push_back(self.start);

        while let Some(nid) = queue.pop_front() {
            if visited.contains(&nid) {
                continue;
            }
            visited.insert(nid);
            if let Some(node) = self.nodes.get(&nid) {
                for s in node.successors() {
                    if !visited.contains(&s) {
                        queue.push_back(s);
                    }
                }
            }
        }

        visited
    }

    /// Returns `true` if the tree has at least one path from start to an `End` node.
    pub fn has_terminal(&self) -> bool {
        self.reachable_nodes()
            .iter()
            .any(|nid| matches!(self.nodes.get(nid), Some(DialogueNode::End { .. })))
    }
}

// ── ValidationError ──────────────────────────────────────────────────────────

/// A structural problem found during [`DialogueTree::validate`].
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// The `start` node ID does not exist in the node map.
    MissingStart(NodeId),
    /// A node references a target that is not in the node map.
    DanglingReference {
        from:    NodeId,
        to:      NodeId,
        context: &'static str,
    },
    /// A `RandomChoice` option has a negative weight.
    NegativeWeight {
        node:   NodeId,
        target: NodeId,
    },
}

// ── DialogueBuilder ──────────────────────────────────────────────────────────

/// Fluent builder for constructing [`DialogueTree`]s programmatically.
///
/// Node IDs are auto-assigned starting from 1 and incremented with each call.
/// The start node is always the first node added.
///
/// # Example
/// ```rust,ignore
/// let tree = DialogueBuilder::new(DialogueId(1))
///     .say(SpeakerId(1), "Hello, traveller.")
///     .choice(&[
///         ("I need supplies.", NodeId(3)),
///         ("Farewell.",        NodeId(4)),
///     ])
///     .end()
///     .build();
/// ```
pub struct DialogueBuilder {
    id:       DialogueId,
    nodes:    Vec<DialogueNode>,
    next_id:  u32,
    start:    Option<NodeId>,
    meta:     DialogueMeta,
    /// Pending "next" pointer from the last linear node.
    pending_next: Option<usize>, // index into `nodes` whose `next` field should be patched
}

impl DialogueBuilder {
    /// Create a builder for the tree with the given ID.
    pub fn new(id: DialogueId) -> Self {
        Self {
            id,
            nodes:        Vec::new(),
            next_id:      1,
            start:        None,
            meta:         DialogueMeta::new("Untitled"),
            pending_next: None,
        }
    }

    pub fn with_meta(mut self, meta: DialogueMeta) -> Self {
        self.meta = meta;
        self
    }

    // ── ID allocation ─────────────────────────────────────────────────────

    fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    // ── Linear-node plumbing ──────────────────────────────────────────────

    /// Patch the previously recorded "needs-next" node to point at `target`.
    fn wire_pending(&mut self, target: NodeId) {
        if let Some(idx) = self.pending_next.take() {
            let node = &mut self.nodes[idx];
            match node {
                DialogueNode::Say        { next, .. } => *next = Some(target),
                DialogueNode::SetVar     { next, .. } => *next = Some(target),
                DialogueNode::CallScript { next, .. } => *next = Some(target),
                _ => {}
            }
        }
    }

    fn push_node(&mut self, node: DialogueNode) -> NodeId {
        let id = node.id();
        if self.start.is_none() {
            self.start = Some(id);
        }
        self.nodes.push(node);
        id
    }

    // ── Node-adding methods ───────────────────────────────────────────────

    /// Append a `Say` node.  The `next` pointer is patched when the following
    /// node is added.
    pub fn say(mut self, speaker: SpeakerId, text: impl Into<String>) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let idx = self.nodes.len();
        self.push_node(DialogueNode::Say {
            id,
            speaker,
            text:      text.into(),
            emotion:   Emotion::Neutral,
            audio_key: None,
            next:      None,
        });
        self.pending_next = Some(idx);
        self
    }

    /// Append a `Say` node with explicit emotion.
    pub fn say_with_emotion(
        mut self,
        speaker: SpeakerId,
        text:    impl Into<String>,
        emotion: Emotion,
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let idx = self.nodes.len();
        self.push_node(DialogueNode::Say {
            id,
            speaker,
            text:      text.into(),
            emotion,
            audio_key: None,
            next:      None,
        });
        self.pending_next = Some(idx);
        self
    }

    /// Append a `Say` node with an audio key.
    pub fn say_audio(
        mut self,
        speaker:   SpeakerId,
        text:      impl Into<String>,
        audio_key: impl Into<String>,
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let idx = self.nodes.len();
        self.push_node(DialogueNode::Say {
            id,
            speaker,
            text:      text.into(),
            emotion:   Emotion::Neutral,
            audio_key: Some(audio_key.into()),
            next:      None,
        });
        self.pending_next = Some(idx);
        self
    }

    /// Append a `Choice` node.  Each entry is `(display_text, target_node_id)`.
    /// The targets must already exist in an external context; the builder does
    /// not validate them.
    pub fn choice(mut self, options: &[(&str, NodeId)]) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let opts: Vec<ChoiceOption> = options
            .iter()
            .map(|(text, next)| ChoiceOption::new(*text, *next))
            .collect();
        self.push_node(DialogueNode::Choice {
            id,
            speaker: SpeakerId::NARRATOR,
            prompt:  None,
            options: opts,
        });
        // Choice is a fork — no single pending next.
        self.pending_next = None;
        self
    }

    /// Append a `Choice` node with a prompt and speaker.
    pub fn choice_with_prompt(
        mut self,
        speaker: SpeakerId,
        prompt:  impl Into<String>,
        options: &[(&str, NodeId)],
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let opts: Vec<ChoiceOption> = options
            .iter()
            .map(|(text, next)| ChoiceOption::new(*text, *next))
            .collect();
        self.push_node(DialogueNode::Choice {
            id,
            speaker,
            prompt: Some(prompt.into()),
            options: opts,
        });
        self.pending_next = None;
        self
    }

    /// Append a `Branch` node.
    pub fn branch(
        mut self,
        cond:     Condition,
        if_true:  NodeId,
        if_false: Option<NodeId>,
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::Branch {
            id,
            condition: cond,
            if_true,
            if_false,
        });
        self.pending_next = None;
        self
    }

    /// Append a `SetVar` node.
    pub fn set_var(
        mut self,
        name:  impl Into<String>,
        value: impl Into<DialogueVar>,
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let idx = self.nodes.len();
        self.push_node(DialogueNode::SetVar {
            id,
            name:  name.into(),
            value: value.into(),
            next:  None,
        });
        self.pending_next = Some(idx);
        self
    }

    /// Append a `CallScript` node.
    pub fn call_script(
        mut self,
        function: impl Into<String>,
        args:     Vec<DialogueVar>,
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        let idx = self.nodes.len();
        self.push_node(DialogueNode::CallScript {
            id,
            function: function.into(),
            args,
            next:     None,
        });
        self.pending_next = Some(idx);
        self
    }

    /// Append a `Jump` node.
    pub fn jump(mut self, target: NodeId) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::Jump { id, target });
        self.pending_next = None;
        self
    }

    /// Append a `Wait` node.  Requires an explicit continuation node.
    pub fn wait(mut self, duration: f32, next: NodeId) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::Wait { id, duration, next });
        self.pending_next = None;
        self
    }

    /// Append a `PlayAnim` node.
    pub fn play_anim(
        mut self,
        speaker:  SpeakerId,
        anim_key: impl Into<String>,
        next:     NodeId,
    ) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::PlayAnim {
            id,
            speaker,
            anim_key: anim_key.into(),
            next,
        });
        self.pending_next = None;
        self
    }

    /// Append a `Camera` node.
    pub fn camera(mut self, action: CameraAction, next: NodeId) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::Camera { id, action, next });
        self.pending_next = None;
        self
    }

    /// Append a `RandomChoice` node with weighted targets.
    pub fn random_choice(mut self, options: Vec<(NodeId, f32)>) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::RandomChoice { id, options });
        self.pending_next = None;
        self
    }

    /// Append an `End` node.
    pub fn end(mut self) -> Self {
        let id = self.alloc_id();
        self.wire_pending(id);
        self.push_node(DialogueNode::End { id });
        self.pending_next = None;
        self
    }

    /// Peek at the next node ID that will be allocated (useful for forward
    /// references when constructing cross-linked graphs).
    pub fn peek_next_id(&self) -> NodeId {
        NodeId(self.next_id)
    }

    // ── Finalise ──────────────────────────────────────────────────────────

    /// Consume the builder and produce a [`DialogueTree`].
    ///
    /// # Panics
    /// Panics if no nodes were added.
    pub fn build(self) -> DialogueTree {
        let start = self.start.expect("DialogueBuilder: no nodes were added");
        let mut tree = DialogueTree::new(self.id, start, self.meta);
        for node in self.nodes {
            tree.insert(node);
        }
        tree
    }
}

// ── DialogueLibrary ──────────────────────────────────────────────────────────

/// A registry of [`DialogueTree`]s keyed by [`DialogueId`].
///
/// Typically wrapped in `Arc<DialogueLibrary>` and shared with runner instances.
#[derive(Debug, Clone, Default)]
pub struct DialogueLibrary {
    trees: HashMap<DialogueId, DialogueTree>,
}

impl DialogueLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tree, replacing any existing entry with the same ID.
    pub fn register(&mut self, tree: DialogueTree) {
        self.trees.insert(tree.id, tree);
    }

    /// Look up a tree by ID.
    pub fn get(&self, id: DialogueId) -> Option<&DialogueTree> {
        self.trees.get(&id)
    }

    /// Iterate over all trees whose metadata includes the given tag.
    pub fn list_by_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a DialogueTree> {
        self.trees.values().filter(move |t| t.metadata.has_tag(tag))
    }

    /// Iterate over all registered trees.
    pub fn iter(&self) -> impl Iterator<Item = &DialogueTree> {
        self.trees.values()
    }

    /// Number of trees registered.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }

    /// Remove a tree by ID.
    pub fn remove(&mut self, id: DialogueId) -> Option<DialogueTree> {
        self.trees.remove(&id)
    }

    /// Return all [`DialogueId`]s present in the library.
    pub fn ids(&self) -> Vec<DialogueId> {
        self.trees.keys().copied().collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue::{DialogueId, DialogueVar, NodeId, SpeakerId};

    // ── Condition tests ───────────────────────────────────────────────────

    fn make_vars(pairs: &[(&str, DialogueVar)]) -> HashMap<String, DialogueVar> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    fn make_flags(flags: &[&str]) -> HashSet<String> {
        flags.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn condition_always_never() {
        let vars  = HashMap::new();
        let flags = HashSet::new();
        assert!(Condition::Always.evaluate(&vars, &flags));
        assert!(!Condition::Never.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_has_flag() {
        let vars  = HashMap::new();
        let flags = make_flags(&["quest_started", "met_alice"]);
        assert!(Condition::HasFlag("quest_started".to_string()).evaluate(&vars, &flags));
        assert!(!Condition::HasFlag("not_present".to_string()).evaluate(&vars, &flags));
    }

    #[test]
    fn condition_var_equals() {
        let vars = make_vars(&[("gold", DialogueVar::Int(100))]);
        let flags = HashSet::new();
        let cond = Condition::var_equals("gold", 100i64);
        assert!(cond.evaluate(&vars, &flags));
        let cond_false = Condition::var_equals("gold", 50i64);
        assert!(!cond_false.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_var_greater() {
        let vars = make_vars(&[("level", DialogueVar::Int(5))]);
        let flags = HashSet::new();
        assert!(Condition::var_greater("level", 3i64).evaluate(&vars, &flags));
        assert!(!Condition::var_greater("level", 5i64).evaluate(&vars, &flags));
        assert!(!Condition::var_greater("level", 10i64).evaluate(&vars, &flags));
    }

    #[test]
    fn condition_var_less() {
        let vars = make_vars(&[("hp", DialogueVar::Int(30))]);
        let flags = HashSet::new();
        assert!(Condition::var_less("hp", 50i64).evaluate(&vars, &flags));
        assert!(!Condition::var_less("hp", 30i64).evaluate(&vars, &flags));
    }

    #[test]
    fn condition_not() {
        let vars  = HashMap::new();
        let flags = make_flags(&["flag_a"]);
        let cond = Condition::not(Condition::HasFlag("flag_a".to_string()));
        assert!(!cond.evaluate(&vars, &flags));
        let cond2 = Condition::not(Condition::HasFlag("flag_b".to_string()));
        assert!(cond2.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_and() {
        let vars  = make_vars(&[("x", DialogueVar::Int(10))]);
        let flags = make_flags(&["ready"]);
        let cond = Condition::and(vec![
            Condition::HasFlag("ready".to_string()),
            Condition::var_greater("x", 5i64),
        ]);
        assert!(cond.evaluate(&vars, &flags));
        let cond_fail = Condition::and(vec![
            Condition::HasFlag("ready".to_string()),
            Condition::var_greater("x", 20i64),
        ]);
        assert!(!cond_fail.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_or() {
        let vars  = HashMap::new();
        let flags = make_flags(&["a"]);
        let cond = Condition::or(vec![
            Condition::HasFlag("a".to_string()),
            Condition::HasFlag("b".to_string()),
        ]);
        assert!(cond.evaluate(&vars, &flags));
        let cond_fail = Condition::or(vec![
            Condition::HasFlag("c".to_string()),
            Condition::HasFlag("d".to_string()),
        ]);
        assert!(!cond_fail.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_nested() {
        // (x > 5 AND flag_a) OR (x < 2)
        let vars  = make_vars(&[("x", DialogueVar::Int(8))]);
        let flags = make_flags(&["flag_a"]);
        let cond = Condition::or(vec![
            Condition::and(vec![
                Condition::var_greater("x", 5i64),
                Condition::HasFlag("flag_a".to_string()),
            ]),
            Condition::var_less("x", 2i64),
        ]);
        assert!(cond.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_missing_var_is_false() {
        let vars  = HashMap::new();
        let flags = HashSet::new();
        // A variable that doesn't exist should not panic — just return false.
        assert!(!Condition::var_equals("missing", 0i64).evaluate(&vars, &flags));
        assert!(!Condition::var_greater("missing", 0i64).evaluate(&vars, &flags));
        assert!(!Condition::var_less("missing", 0i64).evaluate(&vars, &flags));
    }

    // ── Builder tests ─────────────────────────────────────────────────────

    #[test]
    fn builder_linear_chain() {
        let alice = SpeakerId(1);
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(alice, "Hello.")
            .say(alice, "How are you?")
            .end()
            .build();

        assert_eq!(tree.node_count(), 3);
        assert!(tree.validate().is_empty(), "linear chain should validate clean");

        // First node should be NodeId(1)
        assert_eq!(tree.start, NodeId(1));

        // NodeId(1) should wire to NodeId(2)
        if let Some(DialogueNode::Say { next, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*next, Some(NodeId(2)));
        } else {
            panic!("Node 1 should be Say");
        }

        // NodeId(2) should wire to NodeId(3)
        if let Some(DialogueNode::Say { next, .. }) = tree.get(NodeId(2)) {
            assert_eq!(*next, Some(NodeId(3)));
        } else {
            panic!("Node 2 should be Say");
        }
    }

    #[test]
    fn builder_choice_fork() {
        let narrator = SpeakerId::NARRATOR;
        // Build target nodes manually first using raw NodeId references.
        // The builder allocates IDs sequentially, so we need to know what
        // IDs will be used for the choice targets.
        let mut b = DialogueBuilder::new(DialogueId(2));

        // Node 1: Say (greeter)
        // Node 2: Choice
        // Targets: NodeId(3) and NodeId(4) — we add them after via jump nodes
        let target_a = NodeId(3);
        let target_b = NodeId(4);

        let tree = b
            .say(narrator, "What do you want?")
            .choice(&[("Buy", target_a), ("Leave", target_b)])
            .build();

        assert_eq!(tree.start, NodeId(1));
        // The choice node should have 2 options
        if let Some(DialogueNode::Choice { options, .. }) = tree.get(NodeId(2)) {
            assert_eq!(options.len(), 2);
        } else {
            panic!("Node 2 should be Choice");
        }
    }

    #[test]
    fn builder_set_var_wires_next() {
        let tree = DialogueBuilder::new(DialogueId(3))
            .set_var("reputation", 10i64)
            .end()
            .build();

        if let Some(DialogueNode::SetVar { next, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*next, Some(NodeId(2)));
        } else {
            panic!("Node 1 should be SetVar");
        }
    }

    #[test]
    fn builder_branch_node() {
        let tree = DialogueBuilder::new(DialogueId(4))
            .branch(
                Condition::var_greater("gold", 50i64),
                NodeId(10),
                Some(NodeId(11)),
            )
            .build();

        if let Some(DialogueNode::Branch { if_true, if_false, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*if_true, NodeId(10));
            assert_eq!(*if_false, Some(NodeId(11)));
        } else {
            panic!("Node 1 should be Branch");
        }
    }

    #[test]
    fn builder_with_meta() {
        let meta = DialogueMeta::new("Intro Convo")
            .with_author("Alice Writer")
            .with_tag("tutorial")
            .with_version("1.0.0");
        let tree = DialogueBuilder::new(DialogueId(5))
            .with_meta(meta)
            .end()
            .build();
        assert_eq!(tree.metadata.title, "Intro Convo");
        assert_eq!(tree.metadata.author, "Alice Writer");
        assert!(tree.metadata.has_tag("tutorial"));
        assert_eq!(tree.metadata.version, "1.0.0");
    }

    // ── Library tests ─────────────────────────────────────────────────────

    #[test]
    fn library_register_and_get() {
        let mut lib = DialogueLibrary::new();
        let tree = DialogueBuilder::new(DialogueId(1))
            .end()
            .build();
        lib.register(tree);
        assert!(lib.get(DialogueId(1)).is_some());
        assert!(lib.get(DialogueId(99)).is_none());
    }

    #[test]
    fn library_list_by_tag() {
        let mut lib = DialogueLibrary::new();

        let meta_a = DialogueMeta::new("A").with_tag("tutorial");
        let tree_a = DialogueBuilder::new(DialogueId(1))
            .with_meta(meta_a)
            .end()
            .build();

        let meta_b = DialogueMeta::new("B").with_tag("combat");
        let tree_b = DialogueBuilder::new(DialogueId(2))
            .with_meta(meta_b)
            .end()
            .build();

        lib.register(tree_a);
        lib.register(tree_b);

        let tutorials: Vec<_> = lib.list_by_tag("tutorial").collect();
        assert_eq!(tutorials.len(), 1);
        assert_eq!(tutorials[0].metadata.title, "A");

        let combat: Vec<_> = lib.list_by_tag("combat").collect();
        assert_eq!(combat.len(), 1);

        let none: Vec<_> = lib.list_by_tag("no_such_tag").collect();
        assert!(none.is_empty());
    }

    #[test]
    fn library_remove() {
        let mut lib = DialogueLibrary::new();
        lib.register(DialogueBuilder::new(DialogueId(1)).end().build());
        assert_eq!(lib.len(), 1);
        lib.remove(DialogueId(1));
        assert!(lib.is_empty());
    }

    // ── Tree validation tests ─────────────────────────────────────────────

    #[test]
    fn tree_validate_clean() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Hi")
            .end()
            .build();
        assert!(tree.validate().is_empty());
    }

    #[test]
    fn tree_validate_missing_start() {
        let tree = DialogueTree::new(
            DialogueId(1),
            NodeId(99), // doesn't exist
            DialogueMeta::new("Bad"),
        );
        let errs = tree.validate();
        assert!(!errs.is_empty());
        assert!(matches!(errs[0], ValidationError::MissingStart(NodeId(99))));
    }

    #[test]
    fn tree_reachable_nodes_linear() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "A")
            .say(SpeakerId(1), "B")
            .end()
            .build();
        let reachable = tree.reachable_nodes();
        assert_eq!(reachable.len(), 3);
        assert!(reachable.contains(&NodeId(1)));
        assert!(reachable.contains(&NodeId(2)));
        assert!(reachable.contains(&NodeId(3)));
    }

    #[test]
    fn tree_has_terminal() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Hello")
            .end()
            .build();
        assert!(tree.has_terminal());
    }

    #[test]
    fn camera_action_constructors() {
        let _ = CameraAction::focus_on(SpeakerId(1));
        let _ = CameraAction::pan_to(1.0, 2.0, 3.0);
        let _ = CameraAction::restore();
    }

    #[test]
    fn choice_option_builder() {
        let opt = ChoiceOption::new("Fight!", NodeId(5))
            .with_condition(Condition::var_greater("strength", 10i64))
            .once_only()
            .with_tag("aggressive");
        assert!(opt.condition.is_some());
        assert!(opt.once_only);
        assert_eq!(opt.tags, vec!["aggressive"]);
    }

    #[test]
    fn dialogue_node_id_accessor() {
        let node = DialogueNode::End { id: NodeId(42) };
        assert_eq!(node.id(), NodeId(42));
        assert_eq!(node.kind_name(), "End");
        assert!(node.is_terminal());
        assert!(!node.is_blocking());
    }

    #[test]
    fn dialogue_node_say_is_blocking() {
        let node = DialogueNode::Say {
            id:        NodeId(1),
            speaker:   SpeakerId(1),
            text:      "Hi".to_string(),
            emotion:   Emotion::Happy,
            audio_key: None,
            next:      None,
        };
        assert!(node.is_blocking());
        assert!(!node.is_terminal());
    }

    #[test]
    fn dialogue_node_successors_branch() {
        let node = DialogueNode::Branch {
            id:        NodeId(1),
            condition: Condition::Always,
            if_true:   NodeId(2),
            if_false:  Some(NodeId(3)),
        };
        let s = node.successors();
        assert_eq!(s.len(), 2);
        assert!(s.contains(&NodeId(2)));
        assert!(s.contains(&NodeId(3)));
    }

    #[test]
    fn builder_peek_next_id() {
        let mut b = DialogueBuilder::new(DialogueId(1));
        assert_eq!(b.peek_next_id(), NodeId(1));
        b = b.say(SpeakerId(1), "test");
        assert_eq!(b.peek_next_id(), NodeId(2));
    }
}

// ── GraphAnalyser ─────────────────────────────────────────────────────────────

/// Static analysis utilities for a [`DialogueTree`].
///
/// These are separate from the runtime runner — they operate on the immutable
/// graph structure and are used by the editor, importers, and test suites.
pub struct GraphAnalyser<'a> {
    tree: &'a DialogueTree,
}

impl<'a> GraphAnalyser<'a> {
    pub fn new(tree: &'a DialogueTree) -> Self {
        Self { tree }
    }

    /// Collect all nodes that are never referenced as a successor of any other
    /// node (other than the start node).  These are "orphaned" — the runner
    /// can never reach them.
    pub fn orphaned_nodes(&self) -> Vec<NodeId> {
        let mut referenced: HashSet<NodeId> = HashSet::new();
        referenced.insert(self.tree.start);

        for node in self.tree.nodes.values() {
            for s in node.successors() {
                referenced.insert(s);
            }
        }

        self.tree.nodes
            .keys()
            .copied()
            .filter(|id| !referenced.contains(id))
            .collect()
    }

    /// Collect all `End` nodes reachable from the start.
    pub fn terminal_nodes(&self) -> Vec<NodeId> {
        self.tree.reachable_nodes()
            .into_iter()
            .filter(|id| matches!(self.tree.nodes.get(id), Some(DialogueNode::End { .. })))
            .collect()
    }

    /// Collect all `Choice` nodes reachable from the start.
    pub fn choice_nodes(&self) -> Vec<NodeId> {
        self.tree.reachable_nodes()
            .into_iter()
            .filter(|id| matches!(self.tree.nodes.get(id), Some(DialogueNode::Choice { .. })))
            .collect()
    }

    /// Collect all `Branch` nodes reachable from the start.
    pub fn branch_nodes(&self) -> Vec<NodeId> {
        self.tree.reachable_nodes()
            .into_iter()
            .filter(|id| matches!(self.tree.nodes.get(id), Some(DialogueNode::Branch { .. })))
            .collect()
    }

    /// Collect all `Say` nodes reachable from the start.
    pub fn say_nodes(&self) -> Vec<NodeId> {
        self.tree.reachable_nodes()
            .into_iter()
            .filter(|id| matches!(self.tree.nodes.get(id), Some(DialogueNode::Say { .. })))
            .collect()
    }

    /// Count all unique variable names read or written by reachable nodes.
    pub fn used_variable_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        for nid in self.tree.reachable_nodes() {
            if let Some(node) = self.tree.nodes.get(&nid) {
                match node {
                    DialogueNode::SetVar { name, .. } => { names.insert(name.clone()); }
                    DialogueNode::Branch { condition, .. } => {
                        collect_condition_vars(condition, &mut names);
                    }
                    DialogueNode::Choice { options, .. } => {
                        for opt in options {
                            if let Some(cond) = &opt.condition {
                                collect_condition_vars(cond, &mut names);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        names
    }

    /// Count all unique flag names referenced in reachable conditions.
    pub fn used_flag_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        for nid in self.tree.reachable_nodes() {
            if let Some(node) = self.tree.nodes.get(&nid) {
                match node {
                    DialogueNode::Branch { condition, .. } => {
                        collect_condition_flags(condition, &mut names);
                    }
                    DialogueNode::Choice { options, .. } => {
                        for opt in options {
                            if let Some(cond) = &opt.condition {
                                collect_condition_flags(cond, &mut names);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        names
    }

    /// Count the total number of reachable `Say` lines (the "word count").
    pub fn line_count(&self) -> usize {
        self.say_nodes().len()
    }

    /// Return the depth (longest path in terms of node hops) from start to any
    /// `End` node via BFS level tracking.
    ///
    /// Returns `None` if no `End` node is reachable.
    pub fn max_depth(&self) -> Option<usize> {
        let mut visited: HashMap<NodeId, usize> = HashMap::new();
        let mut queue: std::collections::VecDeque<(NodeId, usize)> = std::collections::VecDeque::new();
        queue.push_back((self.tree.start, 0));
        let mut max_end_depth: Option<usize> = None;

        while let Some((nid, depth)) = queue.pop_front() {
            if visited.contains_key(&nid) { continue; }
            visited.insert(nid, depth);

            if let Some(node) = self.tree.nodes.get(&nid) {
                if node.is_terminal() {
                    max_end_depth = Some(max_end_depth.map_or(depth, |d: usize| d.max(depth)));
                }
                for s in node.successors() {
                    if !visited.contains_key(&s) {
                        queue.push_back((s, depth + 1));
                    }
                }
            }
        }

        max_end_depth
    }

    /// Detect simple cycles (nodes that appear in their own successor chain).
    ///
    /// Returns the set of node IDs that are part of at least one cycle.
    /// This uses a colour-based DFS.
    pub fn detect_cycles(&self) -> HashSet<NodeId> {
        #[derive(PartialEq)]
        enum Colour { White, Grey, Black }

        let mut colour: HashMap<NodeId, Colour> = HashMap::new();
        let mut in_cycle: HashSet<NodeId> = HashSet::new();

        fn dfs(
            nid:     NodeId,
            tree:    &DialogueTree,
            colour:  &mut HashMap<NodeId, Colour>,
            in_cycle: &mut HashSet<NodeId>,
            path:    &mut Vec<NodeId>,
        ) {
            colour.insert(nid, Colour::Grey);
            path.push(nid);

            if let Some(node) = tree.nodes.get(&nid) {
                for s in node.successors() {
                    match colour.get(&s) {
                        Some(Colour::Grey) => {
                            // Found a back-edge — everything in path from s is a cycle.
                            if let Some(pos) = path.iter().position(|&n| n == s) {
                                for &cn in &path[pos..] {
                                    in_cycle.insert(cn);
                                }
                            }
                        }
                        Some(Colour::Black) => {}
                        _ => {
                            dfs(s, tree, colour, in_cycle, path);
                        }
                    }
                }
            }

            path.pop();
            colour.insert(nid, Colour::Black);
        }

        let mut path = Vec::new();
        dfs(self.tree.start, self.tree, &mut colour, &mut in_cycle, &mut path);
        in_cycle
    }
}

/// Recursively collect all variable names referenced in a condition.
fn collect_condition_vars(cond: &Condition, names: &mut HashSet<String>) {
    match cond {
        Condition::VarEquals(n, _)  |
        Condition::VarGreater(n, _) |
        Condition::VarLess(n, _)    => { names.insert(n.clone()); }
        Condition::Not(inner)       => collect_condition_vars(inner, names),
        Condition::And(children) |
        Condition::Or(children)     => {
            for c in children { collect_condition_vars(c, names); }
        }
        _ => {}
    }
}

/// Recursively collect all flag names referenced in a condition.
fn collect_condition_flags(cond: &Condition, names: &mut HashSet<String>) {
    match cond {
        Condition::HasFlag(n)   => { names.insert(n.clone()); }
        Condition::Not(inner)   => collect_condition_flags(inner, names),
        Condition::And(children) |
        Condition::Or(children) => {
            for c in children { collect_condition_flags(c, names); }
        }
        _ => {}
    }
}

// ── NodePatch ─────────────────────────────────────────────────────────────────

/// A minimal patch operation for updating a single field of a node in place.
///
/// Used by the editor to make targeted mutations without rebuilding the whole
/// tree.  The runner is not affected — patches take effect on the next call to
/// `tree.get(id)`.
#[derive(Debug, Clone)]
pub enum NodePatch {
    /// Change the text of a `Say` node.
    SetText { node: NodeId, text: String },
    /// Change the emotion of a `Say` node.
    SetEmotion { node: NodeId, emotion: crate::dialogue::Emotion },
    /// Add a tag to a `ChoiceOption`.
    AddChoiceTag { node: NodeId, option_index: usize, tag: String },
    /// Remove a tag from a `ChoiceOption`.
    RemoveChoiceTag { node: NodeId, option_index: usize, tag: String },
    /// Change the `once_only` flag on a `ChoiceOption`.
    SetOnceOnly { node: NodeId, option_index: usize, once_only: bool },
    /// Replace the condition on a `Branch` node.
    SetBranchCondition { node: NodeId, condition: Condition },
    /// Change the `if_false` arm of a `Branch` node.
    SetBranchFalse { node: NodeId, target: Option<NodeId> },
}

impl NodePatch {
    /// Apply this patch to a mutable [`DialogueTree`].
    ///
    /// Returns `Ok(())` on success or an error description if the patch could
    /// not be applied (wrong node kind, out-of-range index, etc.).
    pub fn apply(&self, tree: &mut DialogueTree) -> Result<(), String> {
        match self {
            NodePatch::SetText { node, text } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Say { text: t, .. }) => { *t = text.clone(); Ok(()) }
                    Some(n) => Err(format!("NodePatch::SetText: node {:?} is {}, not Say", node, n.kind_name())),
                    None    => Err(format!("NodePatch::SetText: node {:?} not found", node)),
                }
            }

            NodePatch::SetEmotion { node, emotion } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Say { emotion: e, .. }) => { *e = *emotion; Ok(()) }
                    Some(n) => Err(format!("NodePatch::SetEmotion: node {:?} is {}", node, n.kind_name())),
                    None    => Err(format!("NodePatch::SetEmotion: node {:?} not found", node)),
                }
            }

            NodePatch::AddChoiceTag { node, option_index, tag } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Choice { options, .. }) => {
                        let opt = options.get_mut(*option_index)
                            .ok_or_else(|| format!("option index {} out of range", option_index))?;
                        if !opt.tags.contains(tag) { opt.tags.push(tag.clone()); }
                        Ok(())
                    }
                    Some(n) => Err(format!("AddChoiceTag: node {:?} is {}", node, n.kind_name())),
                    None    => Err(format!("AddChoiceTag: node {:?} not found", node)),
                }
            }

            NodePatch::RemoveChoiceTag { node, option_index, tag } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Choice { options, .. }) => {
                        let opt = options.get_mut(*option_index)
                            .ok_or_else(|| format!("option index {} out of range", option_index))?;
                        opt.tags.retain(|t| t != tag);
                        Ok(())
                    }
                    Some(n) => Err(format!("RemoveChoiceTag: node {:?} is {}", node, n.kind_name())),
                    None    => Err(format!("RemoveChoiceTag: node {:?} not found", node)),
                }
            }

            NodePatch::SetOnceOnly { node, option_index, once_only } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Choice { options, .. }) => {
                        let opt = options.get_mut(*option_index)
                            .ok_or_else(|| format!("option index {} out of range", option_index))?;
                        opt.once_only = *once_only;
                        Ok(())
                    }
                    Some(n) => Err(format!("SetOnceOnly: node {:?} is {}", node, n.kind_name())),
                    None    => Err(format!("SetOnceOnly: node {:?} not found", node)),
                }
            }

            NodePatch::SetBranchCondition { node, condition } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Branch { condition: c, .. }) => { *c = condition.clone(); Ok(()) }
                    Some(n) => Err(format!("SetBranchCondition: node {:?} is {}", node, n.kind_name())),
                    None    => Err(format!("SetBranchCondition: node {:?} not found", node)),
                }
            }

            NodePatch::SetBranchFalse { node, target } => {
                match tree.nodes.get_mut(node) {
                    Some(DialogueNode::Branch { if_false, .. }) => { *if_false = *target; Ok(()) }
                    Some(n) => Err(format!("SetBranchFalse: node {:?} is {}", node, n.kind_name())),
                    None    => Err(format!("SetBranchFalse: node {:?} not found", node)),
                }
            }
        }
    }
}

// ── TreeDiff ──────────────────────────────────────────────────────────────────

/// The difference between two versions of a [`DialogueTree`].
#[derive(Debug, Clone, Default)]
pub struct TreeDiff {
    /// Node IDs present in `after` but absent in `before`.
    pub added_nodes:   Vec<NodeId>,
    /// Node IDs present in `before` but absent in `after`.
    pub removed_nodes: Vec<NodeId>,
    /// Node IDs present in both but with different `kind_name` (type changed).
    pub changed_kind:  Vec<NodeId>,
}

impl TreeDiff {
    /// Compute the structural diff between two trees.
    ///
    /// This compares only which nodes exist and their variant type; it does not
    /// do a deep field comparison (use [`NodePatch`] for that).
    pub fn compute(before: &DialogueTree, after: &DialogueTree) -> Self {
        let mut diff = TreeDiff::default();

        for (&id, after_node) in &after.nodes {
            match before.nodes.get(&id) {
                None => diff.added_nodes.push(id),
                Some(before_node) if before_node.kind_name() != after_node.kind_name() => {
                    diff.changed_kind.push(id);
                }
                _ => {}
            }
        }

        for &id in before.nodes.keys() {
            if !after.nodes.contains_key(&id) {
                diff.removed_nodes.push(id);
            }
        }

        diff
    }

    pub fn is_empty(&self) -> bool {
        self.added_nodes.is_empty()
            && self.removed_nodes.is_empty()
            && self.changed_kind.is_empty()
    }
}

// ── Extra tree.rs tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod extra_tests {
    use super::*;
    use crate::dialogue::{DialogueId, DialogueVar, NodeId, SpeakerId};

    fn simple_tree() -> DialogueTree {
        DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Hello")
            .say(SpeakerId(1), "World")
            .end()
            .build()
    }

    // ── GraphAnalyser ─────────────────────────────────────────────────────

    #[test]
    fn analyser_terminal_nodes() {
        let tree = simple_tree();
        let a = GraphAnalyser::new(&tree);
        let terminals = a.terminal_nodes();
        assert_eq!(terminals.len(), 1);
    }

    #[test]
    fn analyser_say_nodes() {
        let tree = simple_tree();
        let a = GraphAnalyser::new(&tree);
        assert_eq!(a.say_nodes().len(), 2);
        assert_eq!(a.line_count(), 2);
    }

    #[test]
    fn analyser_choice_nodes() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .choice(&[("A", NodeId(2)), ("B", NodeId(3))])
            .build();
        let a = GraphAnalyser::new(&tree);
        assert_eq!(a.choice_nodes().len(), 1);
    }

    #[test]
    fn analyser_branch_nodes() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .branch(Condition::Always, NodeId(2), Some(NodeId(3)))
            .build();
        let a = GraphAnalyser::new(&tree);
        assert_eq!(a.branch_nodes().len(), 1);
    }

    #[test]
    fn analyser_used_variable_names() {
        let mut tree = DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            DialogueMeta::new("VarTest"),
        );
        tree.insert(DialogueNode::SetVar {
            id:    NodeId(1),
            name:  "score".to_string(),
            value: DialogueVar::Int(0),
            next:  Some(NodeId(2)),
        });
        tree.insert(DialogueNode::Branch {
            id:        NodeId(2),
            condition: Condition::var_greater("score", 5i64),
            if_true:   NodeId(3),
            if_false:  Some(NodeId(3)),
        });
        tree.insert(DialogueNode::End { id: NodeId(3) });
        let a = GraphAnalyser::new(&tree);
        let vars = a.used_variable_names();
        assert!(vars.contains("score"), "score must be in used vars");
    }

    #[test]
    fn analyser_used_flag_names() {
        let mut tree = DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            DialogueMeta::new("FlagTest"),
        );
        tree.insert(DialogueNode::Branch {
            id:        NodeId(1),
            condition: Condition::HasFlag("quest_done".to_string()),
            if_true:   NodeId(2),
            if_false:  None,
        });
        tree.insert(DialogueNode::End { id: NodeId(2) });
        let a = GraphAnalyser::new(&tree);
        let flags = a.used_flag_names();
        assert!(flags.contains("quest_done"));
    }

    #[test]
    fn analyser_max_depth_linear() {
        let tree = simple_tree(); // 3 nodes deep
        let a = GraphAnalyser::new(&tree);
        let depth = a.max_depth();
        assert_eq!(depth, Some(2), "linear 3-node tree has max depth 2");
    }

    #[test]
    fn analyser_orphaned_nodes() {
        let mut tree = simple_tree();
        // Insert a node that nothing points to.
        tree.insert(DialogueNode::Say {
            id:        NodeId(99),
            speaker:   SpeakerId(1),
            text:      "orphan".to_string(),
            emotion:   crate::dialogue::Emotion::Neutral,
            audio_key: None,
            next:      None,
        });
        let a = GraphAnalyser::new(&tree);
        let orphans = a.orphaned_nodes();
        assert!(orphans.contains(&NodeId(99)), "NodeId(99) must be orphaned");
    }

    #[test]
    fn analyser_no_cycles_in_linear_tree() {
        let tree = simple_tree();
        let a = GraphAnalyser::new(&tree);
        assert!(a.detect_cycles().is_empty());
    }

    // ── NodePatch ─────────────────────────────────────────────────────────

    #[test]
    fn patch_set_text() {
        let mut tree = simple_tree();
        let patch = NodePatch::SetText {
            node: NodeId(1),
            text: "Updated text".to_string(),
        };
        patch.apply(&mut tree).expect("patch must succeed");
        if let Some(DialogueNode::Say { text, .. }) = tree.get(NodeId(1)) {
            assert_eq!(text, "Updated text");
        } else {
            panic!("Node 1 should be Say");
        }
    }

    #[test]
    fn patch_set_emotion() {
        let mut tree = simple_tree();
        let patch = NodePatch::SetEmotion {
            node:    NodeId(1),
            emotion: crate::dialogue::Emotion::Happy,
        };
        patch.apply(&mut tree).expect("patch must succeed");
        if let Some(DialogueNode::Say { emotion, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*emotion, crate::dialogue::Emotion::Happy);
        }
    }

    #[test]
    fn patch_wrong_node_kind_returns_error() {
        let mut tree = simple_tree();
        // Node 3 is End, not Say.
        let patch = NodePatch::SetText {
            node: NodeId(3),
            text: "nope".to_string(),
        };
        assert!(patch.apply(&mut tree).is_err());
    }

    #[test]
    fn patch_add_choice_tag() {
        let mut tree = DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            DialogueMeta::new("T"),
        );
        tree.insert(DialogueNode::Choice {
            id:      NodeId(1),
            speaker: SpeakerId::NARRATOR,
            prompt:  None,
            options: vec![ChoiceOption::new("Yes", NodeId(2))],
        });
        tree.insert(DialogueNode::End { id: NodeId(2) });

        let patch = NodePatch::AddChoiceTag {
            node:         NodeId(1),
            option_index: 0,
            tag:          "brave".to_string(),
        };
        patch.apply(&mut tree).unwrap();

        if let Some(DialogueNode::Choice { options, .. }) = tree.get(NodeId(1)) {
            assert!(options[0].tags.contains(&"brave".to_string()));
        }
    }

    #[test]
    fn patch_set_branch_condition() {
        let mut tree = DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            DialogueMeta::new("T"),
        );
        tree.insert(DialogueNode::Branch {
            id:        NodeId(1),
            condition: Condition::Always,
            if_true:   NodeId(2),
            if_false:  None,
        });
        tree.insert(DialogueNode::End { id: NodeId(2) });

        let patch = NodePatch::SetBranchCondition {
            node:      NodeId(1),
            condition: Condition::Never,
        };
        patch.apply(&mut tree).unwrap();

        if let Some(DialogueNode::Branch { condition, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*condition, Condition::Never);
        }
    }

    // ── TreeDiff ──────────────────────────────────────────────────────────

    #[test]
    fn tree_diff_no_change() {
        let tree = simple_tree();
        let diff = TreeDiff::compute(&tree, &tree);
        assert!(diff.is_empty());
    }

    #[test]
    fn tree_diff_added_node() {
        let tree_before = simple_tree();
        let mut tree_after = tree_before.clone();
        tree_after.insert(DialogueNode::End { id: NodeId(99) });
        let diff = TreeDiff::compute(&tree_before, &tree_after);
        assert_eq!(diff.added_nodes.len(), 1);
        assert!(diff.added_nodes.contains(&NodeId(99)));
    }

    #[test]
    fn tree_diff_removed_node() {
        let tree_before = simple_tree();
        let mut tree_after = tree_before.clone();
        tree_after.nodes.remove(&NodeId(3));
        let diff = TreeDiff::compute(&tree_before, &tree_after);
        assert_eq!(diff.removed_nodes.len(), 1);
    }

    // ── Condition helper constructors ─────────────────────────────────────

    #[test]
    fn condition_clone_and_eq() {
        let c = Condition::and(vec![
            Condition::var_equals("x", 1i64),
            Condition::not(Condition::HasFlag("f".to_string())),
        ]);
        let c2 = c.clone();
        assert_eq!(c, c2);
    }

    #[test]
    fn condition_or_short_circuits() {
        let vars  = std::collections::HashMap::new();
        let flags = std::collections::HashSet::new();
        // First condition is true → whole Or is true even without evaluating second.
        let c = Condition::Or(vec![Condition::Always, Condition::Never]);
        assert!(c.evaluate(&vars, &flags));
    }

    #[test]
    fn condition_and_short_circuits() {
        let vars  = std::collections::HashMap::new();
        let flags = std::collections::HashSet::new();
        let c = Condition::And(vec![Condition::Never, Condition::Always]);
        assert!(!c.evaluate(&vars, &flags));
    }

    // ── Builder edge cases ────────────────────────────────────────────────

    #[test]
    fn builder_call_script() {
        use crate::dialogue::DialogueVar;
        let tree = DialogueBuilder::new(DialogueId(1))
            .call_script("add_gold", vec![DialogueVar::Int(100)])
            .end()
            .build();
        if let Some(DialogueNode::CallScript { function, args, next, .. }) = tree.get(NodeId(1)) {
            assert_eq!(function, "add_gold");
            assert_eq!(args[0], DialogueVar::Int(100));
            assert_eq!(*next, Some(NodeId(2)));
        } else {
            panic!("Node 1 should be CallScript");
        }
    }

    #[test]
    fn builder_jump() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .jump(NodeId(99))
            .build();
        if let Some(DialogueNode::Jump { target, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*target, NodeId(99));
        }
    }

    #[test]
    fn builder_random_choice() {
        let opts = vec![(NodeId(2), 0.3), (NodeId(3), 0.7)];
        let tree = DialogueBuilder::new(DialogueId(1))
            .random_choice(opts.clone())
            .build();
        if let Some(DialogueNode::RandomChoice { options, .. }) = tree.get(NodeId(1)) {
            assert_eq!(options.len(), 2);
        }
    }

    #[test]
    fn builder_say_with_emotion() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say_with_emotion(SpeakerId(1), "Angry line", crate::dialogue::Emotion::Angry)
            .end()
            .build();
        if let Some(DialogueNode::Say { emotion, .. }) = tree.get(NodeId(1)) {
            assert_eq!(*emotion, crate::dialogue::Emotion::Angry);
        }
    }

    #[test]
    fn builder_say_audio() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say_audio(SpeakerId(1), "Voiced line", "vo_001")
            .end()
            .build();
        if let Some(DialogueNode::Say { audio_key, .. }) = tree.get(NodeId(1)) {
            assert_eq!(audio_key.as_deref(), Some("vo_001"));
        }
    }

    #[test]
    fn builder_choice_with_prompt() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .choice_with_prompt(SpeakerId(1), "What do you do?", &[
                ("Fight", NodeId(2)),
                ("Run",   NodeId(3)),
            ])
            .build();
        if let Some(DialogueNode::Choice { prompt, options, .. }) = tree.get(NodeId(1)) {
            assert_eq!(prompt.as_deref(), Some("What do you do?"));
            assert_eq!(options.len(), 2);
        }
    }

    // ── Library ────────────────────────────────────────────────────────────

    #[test]
    fn library_ids() {
        let mut lib = DialogueLibrary::new();
        lib.register(DialogueBuilder::new(DialogueId(1)).end().build());
        lib.register(DialogueBuilder::new(DialogueId(2)).end().build());
        lib.register(DialogueBuilder::new(DialogueId(3)).end().build());
        let mut ids = lib.ids();
        ids.sort();
        assert_eq!(ids, vec![DialogueId(1), DialogueId(2), DialogueId(3)]);
    }

    #[test]
    fn library_iter() {
        let mut lib = DialogueLibrary::new();
        for i in 1..=5u32 {
            lib.register(DialogueBuilder::new(DialogueId(i)).end().build());
        }
        assert_eq!(lib.iter().count(), 5);
    }

    #[test]
    fn dialogue_meta_has_tag() {
        let meta = DialogueMeta::new("Test")
            .with_tag("tutorial")
            .with_tag("act1");
        assert!(meta.has_tag("tutorial"));
        assert!(meta.has_tag("act1"));
        assert!(!meta.has_tag("boss"));
    }
}
