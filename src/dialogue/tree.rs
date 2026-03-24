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
        // After consuming via say, ID increments:
        b = b.say(SpeakerId(1), "test");
        assert_eq!(b.peek_next_id(), NodeId(2));
    }
}
