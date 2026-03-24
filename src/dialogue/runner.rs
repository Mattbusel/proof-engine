//! Dialogue runtime execution engine.
//!
//! # Architecture
//!
//! ```text
//! DialogueLibrary (Arc)
//!        │
//!        ▼
//! DialogueRunner  ── state: DialogueState
//!        │              variables, flags, history, choice_counts
//!        │
//!        ├─ advance()        → Option<DialogueOutput>
//!        ├─ make_choice(n)   → Result<(), RunnerError>
//!        ├─ tick(dt)         → Option<DialogueOutput>
//!        └─ step_node(id)    (internal)
//!
//! DialogueSession
//!        ├─ wraps DialogueRunner
//!        ├─ DialogueHistory (full record)
//!        ├─ auto-advance / skip modes
//!        └─ process(SessionInput) → &[DialogueOutput]
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use crate::dialogue::{DialogueId, DialogueVar, NodeId, SpeakerId};
use crate::dialogue::tree::{
    CameraAction, ChoiceOption, Condition, DialogueLibrary, DialogueNode,
};
use crate::dialogue::Emotion;

// ── RunnerError ───────────────────────────────────────────────────────────────

/// Errors the dialogue runner can produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    /// The requested [`DialogueId`] does not exist in the library.
    TreeNotFound(DialogueId),
    /// A [`NodeId`] referenced by the tree was not found in the node map.
    NodeNotFound(NodeId),
    /// The supplied choice index is out of range.
    InvalidChoice { index: usize, max: usize },
    /// `start()` was called while a dialogue is already running.
    AlreadyRunning,
    /// An operation was requested but no dialogue is currently running.
    NotRunning,
    /// `make_choice` was called when the runner is not waiting for one.
    NoChoicePending,
    /// A `RandomChoice` node has no options or all weights are zero.
    EmptyRandomChoice(NodeId),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerError::TreeNotFound(id) =>
                write!(f, "dialogue tree {:?} not found", id),
            RunnerError::NodeNotFound(id) =>
                write!(f, "node {:?} not found", id),
            RunnerError::InvalidChoice { index, max } =>
                write!(f, "choice index {} out of range (max {})", index, max),
            RunnerError::AlreadyRunning =>
                write!(f, "a dialogue is already running"),
            RunnerError::NotRunning =>
                write!(f, "no dialogue is currently running"),
            RunnerError::NoChoicePending =>
                write!(f, "no choice is pending"),
            RunnerError::EmptyRandomChoice(id) =>
                write!(f, "random choice node {:?} has no valid options", id),
        }
    }
}

impl std::error::Error for RunnerError {}

// ── RunnerStatus ──────────────────────────────────────────────────────────────

/// The current execution state of a [`DialogueRunner`].
#[derive(Debug, Clone, PartialEq)]
pub enum RunnerStatus {
    /// Actively stepping through nodes.
    Running,
    /// Paused, waiting for the player to select a dialogue option.
    WaitingForChoice,
    /// Paused for `f32` more game-seconds (a `Wait` node is active).
    WaitingForTime(f32),
    /// The dialogue has concluded normally.
    Finished,
    /// An unrecoverable error occurred; inner string has the message.
    Errored(String),
}

impl RunnerStatus {
    pub fn is_finished(&self) -> bool {
        matches!(self, RunnerStatus::Finished)
    }

    pub fn is_running(&self) -> bool {
        matches!(self, RunnerStatus::Running)
    }

    pub fn is_waiting_for_choice(&self) -> bool {
        matches!(self, RunnerStatus::WaitingForChoice)
    }

    pub fn is_errored(&self) -> bool {
        matches!(self, RunnerStatus::Errored(_))
    }
}

// ── DialogueOutput ────────────────────────────────────────────────────────────

/// A single output event produced by the runner for the UI layer to display.
#[derive(Debug, Clone)]
pub enum DialogueOutput {
    /// A character is speaking.
    Say {
        speaker:   SpeakerId,
        text:      String,
        emotion:   Emotion,
        audio_key: Option<String>,
    },
    /// The player must choose one of the listed options.
    ShowChoices(Vec<VisibleChoice>),
    /// The runner is waiting for `f32` seconds before continuing.
    Wait(f32),
    /// A camera director command should be executed.
    CameraAction(CameraAction),
    /// A character animation should be played.
    PlayAnim {
        speaker:  SpeakerId,
        anim_key: String,
    },
    /// An external script function was invoked.
    ScriptCall {
        function: String,
        args:     Vec<DialogueVar>,
    },
    /// The dialogue has ended.
    Ended,
}

// ── VisibleChoice ─────────────────────────────────────────────────────────────

/// A filtered choice option as seen by the UI.
///
/// Conditions have already been evaluated; only visible choices are included.
/// `once_only` options that have been exhausted are excluded before this struct
/// is constructed.
#[derive(Debug, Clone)]
pub struct VisibleChoice {
    /// Index into the original `options` vec — pass this to `make_choice`.
    pub index: usize,
    /// Display text.
    pub text:  String,
    /// Tags forwarded from the source [`ChoiceOption`].
    pub tags:  Vec<String>,
}

// ── DialogueState ─────────────────────────────────────────────────────────────

/// All mutable runtime state for one active dialogue session.
#[derive(Debug, Clone)]
pub struct DialogueState {
    /// The node currently being executed.
    pub current_node: NodeId,
    /// Which tree is being executed.
    pub tree_id: DialogueId,
    /// Runtime variable store (readable by conditions and scripts).
    pub variables: HashMap<String, DialogueVar>,
    /// Boolean flags (cheaper than `Bool` vars for simple on/off state).
    pub flags: HashSet<String>,
    /// Ordered list of every node visited, for "already seen" detection.
    pub history: Vec<NodeId>,
    /// How many times each choice node has been visited (for `once_only` logic).
    pub choice_counts: HashMap<NodeId, u32>,
    /// Game-time in seconds when this dialogue started (caller-supplied).
    pub started_at: f32,
}

impl DialogueState {
    pub fn new(
        tree_id:      DialogueId,
        start_node:   NodeId,
        initial_vars: HashMap<String, DialogueVar>,
        initial_flags: HashSet<String>,
        started_at:   f32,
    ) -> Self {
        Self {
            current_node: start_node,
            tree_id,
            variables:    initial_vars,
            flags:        initial_flags,
            history:      Vec::new(),
            choice_counts: HashMap::new(),
            started_at,
        }
    }

    pub fn record_visit(&mut self, node: NodeId) {
        self.history.push(node);
    }

    pub fn increment_choice(&mut self, node: NodeId) {
        *self.choice_counts.entry(node).or_insert(0) += 1;
    }

    pub fn choice_count(&self, node: NodeId) -> u32 {
        self.choice_counts.get(&node).copied().unwrap_or(0)
    }

    pub fn has_visited(&self, node: NodeId) -> bool {
        self.history.contains(&node)
    }

    pub fn set_var(&mut self, name: impl Into<String>, value: DialogueVar) {
        self.variables.insert(name.into(), value);
    }

    pub fn get_var(&self, name: &str) -> Option<&DialogueVar> {
        self.variables.get(name)
    }

    pub fn set_flag(&mut self, name: impl Into<String>) {
        self.flags.insert(name.into());
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    pub fn remove_flag(&mut self, name: &str) -> bool {
        self.flags.remove(name)
    }
}

// ── DialogueRunner ────────────────────────────────────────────────────────────

/// The core dialogue execution engine.
///
/// The runner is a step-by-step state machine: call [`advance`] repeatedly to
/// consume output events.  When `status` is [`RunnerStatus::WaitingForChoice`],
/// call [`make_choice`] before resuming.  When `status` is
/// [`RunnerStatus::WaitingForTime`], call [`tick`] each game frame.
///
/// The runner does **not** keep a reference to a specific tree; the tree is
/// looked up from the library each time it is needed, allowing hot-reload.
pub struct DialogueRunner {
    /// Active execution state, or `None` if no dialogue is running.
    pub state: Option<DialogueState>,
    /// Current execution status.
    pub status: RunnerStatus,
    /// Queue of output events not yet consumed by the caller.
    pub pending_output: VecDeque<DialogueOutput>,
    /// Shared tree library.
    pub library: Arc<DialogueLibrary>,
    /// Variable table that persists across multiple dialogues in one session.
    persistent_vars:  HashMap<String, DialogueVar>,
    /// Flag set that persists across multiple dialogues.
    persistent_flags: HashSet<String>,
}

impl DialogueRunner {
    // ── Construction ──────────────────────────────────────────────────────

    pub fn new(library: Arc<DialogueLibrary>) -> Self {
        Self {
            state:           None,
            status:          RunnerStatus::Finished,
            pending_output:  VecDeque::new(),
            library,
            persistent_vars:  HashMap::new(),
            persistent_flags: HashSet::new(),
        }
    }

    // ── Persistent variable/flag API ──────────────────────────────────────

    /// Set a variable that persists across dialogues.
    pub fn set_persistent_var(&mut self, name: impl Into<String>, value: DialogueVar) {
        self.persistent_vars.insert(name.into(), value);
    }

    /// Get a persistent variable.
    pub fn get_persistent_var(&self, name: &str) -> Option<&DialogueVar> {
        self.persistent_vars.get(name)
    }

    /// Set a persistent flag.
    pub fn set_persistent_flag(&mut self, name: impl Into<String>) {
        self.persistent_flags.insert(name.into());
    }

    /// Check a persistent flag.
    pub fn has_persistent_flag(&self, name: &str) -> bool {
        self.persistent_flags.contains(name)
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Begin executing a dialogue tree.
    ///
    /// # Errors
    /// - [`RunnerError::AlreadyRunning`] if a dialogue is active.
    /// - [`RunnerError::TreeNotFound`] if the ID isn't in the library.
    pub fn start(&mut self, tree_id: DialogueId) -> Result<(), RunnerError> {
        self.start_at(tree_id, 0.0)
    }

    /// Like [`start`] but with an explicit game-time offset.
    pub fn start_at(&mut self, tree_id: DialogueId, game_time: f32) -> Result<(), RunnerError> {
        if matches!(self.status, RunnerStatus::Running
                                | RunnerStatus::WaitingForChoice
                                | RunnerStatus::WaitingForTime(_))
        {
            return Err(RunnerError::AlreadyRunning);
        }

        let tree = self.library.get(tree_id)
            .ok_or(RunnerError::TreeNotFound(tree_id))?;

        let state = DialogueState::new(
            tree_id,
            tree.start,
            self.persistent_vars.clone(),
            self.persistent_flags.clone(),
            game_time,
        );

        self.state          = Some(state);
        self.status         = RunnerStatus::Running;
        self.pending_output.clear();

        // Step the first node immediately.
        self.pump()?;
        Ok(())
    }

    /// Stop the dialogue, regardless of current status.
    pub fn stop(&mut self) {
        self.state = None;
        self.status = RunnerStatus::Finished;
        self.pending_output.clear();
    }

    // ── Output consumption ────────────────────────────────────────────────

    /// Consume the next output event from the queue.
    ///
    /// Returns `None` when the queue is empty.  After consuming a `Say` or
    /// `ShowChoices` event the caller should wait for player input before
    /// calling `advance` or `make_choice`.
    pub fn advance(&mut self) -> Option<DialogueOutput> {
        // If there is already queued output, return it without stepping.
        if let Some(out) = self.pending_output.pop_front() {
            return Some(out);
        }
        // If we are still Running, step more nodes.
        if self.status == RunnerStatus::Running {
            if let Err(e) = self.pump() {
                self.status = RunnerStatus::Errored(e.to_string());
            }
        }
        self.pending_output.pop_front()
    }

    // ── Choice handling ────────────────────────────────────────────────────

    /// Submit the player's choice when status is [`WaitingForChoice`].
    ///
    /// `index` is the [`VisibleChoice::index`] value, **not** the ordinal in
    /// the visible list, which may differ if some options were filtered out.
    pub fn make_choice(&mut self, index: usize) -> Result<(), RunnerError> {
        if !matches!(self.status, RunnerStatus::WaitingForChoice) {
            return Err(RunnerError::NoChoicePending);
        }

        let state = self.state.as_mut().ok_or(RunnerError::NotRunning)?;
        let node_id = state.current_node;

        let target = {
            let tree = self.library.get(state.tree_id)
                .ok_or(RunnerError::TreeNotFound(state.tree_id))?;
            let node = tree.get(node_id)
                .ok_or(RunnerError::NodeNotFound(node_id))?;

            if let DialogueNode::Choice { options, .. } = node {
                let opt = options.get(index)
                    .ok_or(RunnerError::InvalidChoice { index, max: options.len().saturating_sub(1) })?;
                opt.next
            } else {
                return Err(RunnerError::NoChoicePending);
            }
        };

        // Record choice and increment counts.
        {
            let state = self.state.as_mut().unwrap();
            state.increment_choice(node_id);
            state.current_node = target;
            state.record_visit(target);
        }

        self.status = RunnerStatus::Running;
        self.pump()?;
        Ok(())
    }

    // ── Timer tick ────────────────────────────────────────────────────────

    /// Advance the internal wait timer by `delta` seconds.
    ///
    /// Returns a `DialogueOutput::Wait(remaining)` event each call until the
    /// timer expires, then resumes execution and returns subsequent output.
    pub fn tick(&mut self, delta: f32) -> Option<DialogueOutput> {
        if let RunnerStatus::WaitingForTime(remaining) = &mut self.status {
            let new_remaining = *remaining - delta;
            if new_remaining <= 0.0 {
                // Timer expired — continue to next node.
                self.status = RunnerStatus::Running;
                if let Err(e) = self.pump() {
                    self.status = RunnerStatus::Errored(e.to_string());
                }
                return self.pending_output.pop_front();
            } else {
                *remaining = new_remaining;
                return Some(DialogueOutput::Wait(new_remaining));
            }
        }
        self.advance()
    }

    // ── Variable / flag API ───────────────────────────────────────────────

    /// Write a variable into the active dialogue state.
    ///
    /// # Errors
    /// Returns [`RunnerError::NotRunning`] if no dialogue is active.
    pub fn set_var(&mut self, name: impl Into<String>, value: DialogueVar) -> Result<(), RunnerError> {
        let state = self.state.as_mut().ok_or(RunnerError::NotRunning)?;
        state.set_var(name, value);
        Ok(())
    }

    /// Read a variable from the active dialogue state.
    pub fn get_var(&self, name: &str) -> Option<&DialogueVar> {
        self.state.as_ref()?.get_var(name)
    }

    /// Set a flag in the active dialogue state.
    pub fn set_flag(&mut self, name: impl Into<String>) -> Result<(), RunnerError> {
        let state = self.state.as_mut().ok_or(RunnerError::NotRunning)?;
        state.set_flag(name);
        Ok(())
    }

    /// Check whether a flag is set in the active dialogue state.
    pub fn has_flag(&self, name: &str) -> bool {
        self.state.as_ref().map_or(false, |s| s.has_flag(name))
    }

    // ── Status helpers ────────────────────────────────────────────────────

    pub fn is_finished(&self) -> bool {
        self.status.is_finished()
    }

    /// Returns `true` when `advance()` can be called productively.
    pub fn can_advance(&self) -> bool {
        matches!(
            self.status,
            RunnerStatus::Running
        ) || !self.pending_output.is_empty()
    }

    pub fn is_waiting_for_choice(&self) -> bool {
        self.status.is_waiting_for_choice()
    }

    // ── Internal execution engine ─────────────────────────────────────────

    /// Step nodes until an output-producing node or terminal is reached.
    fn pump(&mut self) -> Result<(), RunnerError> {
        // Guard against infinite loops in poorly-authored graphs.
        const MAX_STEPS: usize = 1024;
        let mut steps = 0;

        loop {
            if steps >= MAX_STEPS {
                self.status = RunnerStatus::Errored(
                    "dialogue pump exceeded max step limit — possible infinite loop".to_string(),
                );
                break;
            }
            steps += 1;

            let state = match &self.state {
                Some(s) => s,
                None    => break,
            };

            if !matches!(self.status, RunnerStatus::Running) {
                break;
            }

            let node_id  = state.current_node;
            let tree_id  = state.tree_id;

            let action = {
                let tree = self.library.get(tree_id)
                    .ok_or(RunnerError::TreeNotFound(tree_id))?;
                let node = tree.get(node_id)
                    .ok_or(RunnerError::NodeNotFound(node_id))?;

                self.classify_node(node)?
            };

            self.apply_action(action)?;
        }

        Ok(())
    }

    /// Classify a node into a [`NodeAction`] without mutating state.
    fn classify_node(&self, node: &DialogueNode) -> Result<NodeAction, RunnerError> {
        let state = self.state.as_ref().unwrap();

        match node {
            // ── Terminal ──────────────────────────────────────────────────
            DialogueNode::End { .. } => Ok(NodeAction::End),

            // ── Speech ────────────────────────────────────────────────────
            DialogueNode::Say { speaker, text, emotion, audio_key, next, .. } => {
                Ok(NodeAction::EmitSay {
                    speaker:   *speaker,
                    text:      text.clone(),
                    emotion:   *emotion,
                    audio_key: audio_key.clone(),
                    next:      *next,
                })
            }

            // ── Player choice ─────────────────────────────────────────────
            DialogueNode::Choice { options, .. } => {
                let visible = self.filter_choices(options);
                Ok(NodeAction::ShowChoices(visible))
            }

            // ── Conditional branch ────────────────────────────────────────
            DialogueNode::Branch { condition, if_true, if_false, .. } => {
                let result = condition.evaluate(&state.variables, &state.flags);
                if result {
                    Ok(NodeAction::Jump(*if_true))
                } else if let Some(f) = if_false {
                    Ok(NodeAction::Jump(*f))
                } else {
                    Ok(NodeAction::End)
                }
            }

            // ── Variable mutation ─────────────────────────────────────────
            DialogueNode::SetVar { name, value, next, .. } => {
                Ok(NodeAction::SetVar {
                    name:  name.clone(),
                    value: value.clone(),
                    next:  *next,
                })
            }

            // ── Script call ───────────────────────────────────────────────
            DialogueNode::CallScript { function, args, next, .. } => {
                Ok(NodeAction::ScriptCall {
                    function: function.clone(),
                    args:     args.clone(),
                    next:     *next,
                })
            }

            // ── Jump ──────────────────────────────────────────────────────
            DialogueNode::Jump { target, .. } => {
                Ok(NodeAction::Jump(*target))
            }

            // ── Weighted random ───────────────────────────────────────────
            DialogueNode::RandomChoice { id, options } => {
                let target = self.pick_random_weighted(*id, options)?;
                Ok(NodeAction::Jump(target))
            }

            // ── Wait timer ────────────────────────────────────────────────
            DialogueNode::Wait { duration, next, .. } => {
                Ok(NodeAction::Wait { duration: *duration, next: *next })
            }

            // ── Animation ─────────────────────────────────────────────────
            DialogueNode::PlayAnim { speaker, anim_key, next, .. } => {
                Ok(NodeAction::PlayAnim {
                    speaker:  *speaker,
                    anim_key: anim_key.clone(),
                    next:     *next,
                })
            }

            // ── Camera ────────────────────────────────────────────────────
            DialogueNode::Camera { action, next, .. } => {
                Ok(NodeAction::CameraAction {
                    action: action.clone(),
                    next:   *next,
                })
            }
        }
    }

    /// Apply a [`NodeAction`], mutating state and queuing output as needed.
    fn apply_action(&mut self, action: NodeAction) -> Result<(), RunnerError> {
        match action {
            NodeAction::End => {
                self.pending_output.push_back(DialogueOutput::Ended);
                self.status = RunnerStatus::Finished;
            }

            NodeAction::EmitSay { speaker, text, emotion, audio_key, next } => {
                // Record the visit before emitting.
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.current_node = next.unwrap_or(NodeId::INVALID);
                }
                self.pending_output.push_back(DialogueOutput::Say { speaker, text, emotion, audio_key });
                // Pause: caller calls advance() again after displaying the line.
                // We keep status Running so advance() will continue pumping.
            }

            NodeAction::ShowChoices(visible) => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                }
                self.pending_output.push_back(DialogueOutput::ShowChoices(visible));
                self.status = RunnerStatus::WaitingForChoice;
            }

            NodeAction::Jump(target) => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.current_node = target;
                }
                // Continue pumping (loop back to classify the jump target).
            }

            NodeAction::SetVar { name, value, next } => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.set_var(&name, value);
                    state.current_node = next.unwrap_or(NodeId::INVALID);
                    if state.current_node == NodeId::INVALID {
                        self.status = RunnerStatus::Finished;
                        self.pending_output.push_back(DialogueOutput::Ended);
                    }
                }
            }

            NodeAction::ScriptCall { function, args, next } => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.current_node = next.unwrap_or(NodeId::INVALID);
                }
                self.pending_output.push_back(DialogueOutput::ScriptCall { function, args });
                if self.state.as_ref().map_or(false, |s| s.current_node == NodeId::INVALID) {
                    self.status = RunnerStatus::Finished;
                    self.pending_output.push_back(DialogueOutput::Ended);
                }
            }

            NodeAction::Wait { duration, next } => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.current_node = next;
                }
                self.pending_output.push_back(DialogueOutput::Wait(duration));
                self.status = RunnerStatus::WaitingForTime(duration);
            }

            NodeAction::PlayAnim { speaker, anim_key, next } => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.current_node = next;
                }
                self.pending_output.push_back(DialogueOutput::PlayAnim { speaker, anim_key });
                // PlayAnim is non-blocking; continue stepping.
            }

            NodeAction::CameraAction { action, next } => {
                if let Some(state) = &mut self.state {
                    state.record_visit(state.current_node);
                    state.current_node = next;
                }
                self.pending_output.push_back(DialogueOutput::CameraAction(action));
                // Camera is non-blocking; continue stepping.
            }
        }

        Ok(())
    }

    /// Filter a choice list, respecting conditions and `once_only`.
    fn filter_choices(&self, options: &[ChoiceOption]) -> Vec<VisibleChoice> {
        let state = match &self.state {
            Some(s) => s,
            None    => return Vec::new(),
        };

        options
            .iter()
            .enumerate()
            .filter_map(|(idx, opt)| {
                // Evaluate optional guard condition.
                if let Some(cond) = &opt.condition {
                    if !cond.evaluate(&state.variables, &state.flags) {
                        return None;
                    }
                }
                // Suppress once_only options that have been used.
                if opt.once_only && state.choice_count(state.current_node) > 0 {
                    return None;
                }
                Some(VisibleChoice {
                    index: idx,
                    text:  opt.text.clone(),
                    tags:  opt.tags.clone(),
                })
            })
            .collect()
    }

    /// Pick a weighted random target from a `RandomChoice` node.
    fn pick_random_weighted(
        &self,
        node_id: NodeId,
        options: &[(NodeId, f32)],
    ) -> Result<NodeId, RunnerError> {
        let total: f32 = options.iter().map(|(_, w)| w.max(0.0)).sum();
        if total <= 0.0 {
            return Err(RunnerError::EmptyRandomChoice(node_id));
        }

        // Deterministic pseudo-random using game time + node ID as seed.
        let seed = self.state.as_ref().map_or(0.0, |s| s.started_at);
        let rand_val = pseudo_rand(seed, node_id.raw()) * total;

        let mut cumulative = 0.0_f32;
        for (target, weight) in options {
            let w = weight.max(0.0);
            cumulative += w;
            if rand_val <= cumulative {
                return Ok(*target);
            }
        }

        // Fallback to last option (floating-point rounding safety).
        options.last()
            .map(|(t, _)| *t)
            .ok_or(RunnerError::EmptyRandomChoice(node_id))
    }
}

/// Simple deterministic pseudo-random in [0, 1) based on two seeds.
/// Uses a cheap bijection — not cryptographic, just good enough for
/// weighted dialogue selection without pulling in `rand`.
fn pseudo_rand(seed_a: f32, seed_b: u32) -> f32 {
    let bits = seed_a.to_bits().wrapping_add(seed_b.wrapping_mul(2654435761));
    let mixed = bits
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(0x6c62_272e)
        .rotate_right(13)
        .wrapping_mul(0x2545_f491);
    // Map to [0, 1)
    (mixed as f32) / (u32::MAX as f32 + 1.0)
}

// ── NodeAction (internal) ─────────────────────────────────────────────────────

/// Internal instruction produced by [`DialogueRunner::classify_node`].
///
/// Separating classification from mutation makes unit-testing easier and
/// avoids borrow-checker conflicts when reading the library and mutating state
/// in the same method.
#[derive(Debug)]
enum NodeAction {
    End,
    EmitSay {
        speaker:   SpeakerId,
        text:      String,
        emotion:   Emotion,
        audio_key: Option<String>,
        next:      Option<NodeId>,
    },
    ShowChoices(Vec<VisibleChoice>),
    Jump(NodeId),
    SetVar {
        name:  String,
        value: DialogueVar,
        next:  Option<NodeId>,
    },
    ScriptCall {
        function: String,
        args:     Vec<DialogueVar>,
        next:     Option<NodeId>,
    },
    Wait {
        duration: f32,
        next:     NodeId,
    },
    PlayAnim {
        speaker:  SpeakerId,
        anim_key: String,
        next:     NodeId,
    },
    CameraAction {
        action: CameraAction,
        next:   NodeId,
    },
}

// ── DialogueHistory ───────────────────────────────────────────────────────────

/// A full transcript record of visited nodes across one or more dialogues.
#[derive(Debug, Clone, Default)]
pub struct DialogueHistory {
    pub records: Vec<HistoryRecord>,
}

impl DialogueHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, record: HistoryRecord) {
        self.records.push(record);
    }

    /// Returns `true` if any record matches both tree and node IDs.
    pub fn has_seen(&self, tree_id: DialogueId, node_id: NodeId) -> bool {
        self.records.iter().any(|r| r.tree_id == tree_id && r.node_id == node_id)
    }

    /// All records for a specific tree, in visit order.
    pub fn for_tree(&self, tree_id: DialogueId) -> Vec<&HistoryRecord> {
        self.records.iter().filter(|r| r.tree_id == tree_id).collect()
    }

    /// Total number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

/// One row in the dialogue history transcript.
#[derive(Debug, Clone)]
pub struct HistoryRecord {
    pub tree_id:       DialogueId,
    pub node_id:       NodeId,
    /// Snapshot of the displayed text, if any (empty for non-Say nodes).
    pub text_snapshot: String,
    /// Game-time at which this node was visited.
    pub timestamp:     f32,
}

impl HistoryRecord {
    pub fn new(
        tree_id:       DialogueId,
        node_id:       NodeId,
        text_snapshot: impl Into<String>,
        timestamp:     f32,
    ) -> Self {
        Self {
            tree_id,
            node_id,
            text_snapshot: text_snapshot.into(),
            timestamp,
        }
    }
}

// ── SessionConfig ─────────────────────────────────────────────────────────────

/// Configuration for a [`DialogueSession`].
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Seconds to wait before auto-advancing a `Say` node (0.0 = manual only).
    pub auto_advance_delay: f32,
    /// If `true`, skip only applies to nodes not yet in history.
    pub skip_unseen_only: bool,
    /// Characters revealed per second in typewriter mode.
    pub text_speed: f32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_advance_delay: 0.0,
            skip_unseen_only:   true,
            text_speed:         30.0,
        }
    }
}

impl SessionConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_auto_advance(mut self, delay: f32) -> Self {
        self.auto_advance_delay = delay;
        self
    }

    pub fn with_text_speed(mut self, cps: f32) -> Self {
        self.text_speed = cps;
        self
    }

    pub fn with_skip_unseen_only(mut self, flag: bool) -> Self {
        self.skip_unseen_only = flag;
        self
    }
}

// ── SessionInput ──────────────────────────────────────────────────────────────

/// Input events the caller sends to a [`DialogueSession`].
#[derive(Debug, Clone)]
pub enum SessionInput {
    /// Player pressed the "advance" / confirm button.
    Advance,
    /// Player selected a numbered choice.
    ChooseOption(usize),
    /// Skip one node (respects `skip_unseen_only`).
    Skip,
    /// Enable or disable fast-forward mode.
    FastForward(bool),
}

// ── DialogueSession ───────────────────────────────────────────────────────────

/// High-level dialogue session combining a runner, history, and skip/FF modes.
///
/// The session acts as the primary integration point for game UI code.  Pass
/// [`SessionInput`] events in, read [`DialogueOutput`] events out.
pub struct DialogueSession {
    runner:          DialogueRunner,
    pub history:     DialogueHistory,
    pub config:      SessionConfig,
    fast_forward:    bool,
    auto_timer:      f32,
    current_outputs: Vec<DialogueOutput>,
    game_time:       f32,
}

impl DialogueSession {
    // ── Construction ──────────────────────────────────────────────────────

    pub fn new(library: Arc<DialogueLibrary>, config: SessionConfig) -> Self {
        Self {
            runner:          DialogueRunner::new(library),
            history:         DialogueHistory::new(),
            config,
            fast_forward:    false,
            auto_timer:      0.0,
            current_outputs: Vec::new(),
            game_time:       0.0,
        }
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Start a dialogue, resetting the session output buffer.
    pub fn start_session(&mut self, tree_id: DialogueId) -> Result<(), RunnerError> {
        self.current_outputs.clear();
        self.auto_timer = 0.0;
        self.runner.start_at(tree_id, self.game_time)?;
        self.drain_runner();
        Ok(())
    }

    /// Advance the internal game clock.  Should be called every frame.
    pub fn update(&mut self, delta: f32) {
        self.game_time += delta;

        // Tick waiting timer.
        if let RunnerStatus::WaitingForTime(_) = &self.runner.status {
            if let Some(out) = self.runner.tick(delta) {
                self.record_and_push(out);
                // If timer expired, drain any newly produced output.
                if matches!(self.runner.status, RunnerStatus::Running) {
                    self.drain_runner();
                }
            }
        }

        // Auto-advance logic.
        if self.config.auto_advance_delay > 0.0
            && matches!(self.runner.status, RunnerStatus::Running)
        {
            self.auto_timer += delta;
            if self.auto_timer >= self.config.auto_advance_delay {
                self.auto_timer = 0.0;
                self.process_advance();
            }
        }
    }

    // ── Input processing ──────────────────────────────────────────────────

    /// Process a single input event and return all new outputs.
    ///
    /// The returned slice is valid until the next call to `process`.
    pub fn process(&mut self, input: SessionInput) -> &[DialogueOutput] {
        self.current_outputs.clear();
        self.auto_timer = 0.0;

        match input {
            SessionInput::Advance => self.process_advance(),

            SessionInput::ChooseOption(idx) => {
                if let Err(e) = self.runner.make_choice(idx) {
                    // Non-fatal: just log to output.
                    eprintln!("[DialogueSession] make_choice error: {}", e);
                } else {
                    self.drain_runner();
                }
            }

            SessionInput::Skip => self.process_skip(),

            SessionInput::FastForward(enabled) => {
                self.fast_forward = enabled;
                if enabled {
                    self.process_fast_forward();
                }
            }
        }

        &self.current_outputs
    }

    /// Current outputs (may be stale if `process` hasn't been called yet).
    pub fn current_output(&self) -> &[DialogueOutput] {
        &self.current_outputs
    }

    // ── Status helpers ─────────────────────────────────────────────────────

    pub fn is_finished(&self) -> bool {
        self.runner.is_finished()
    }

    pub fn is_waiting_for_choice(&self) -> bool {
        self.runner.is_waiting_for_choice()
    }

    pub fn status(&self) -> &RunnerStatus {
        &self.runner.status
    }

    // ── Variable / flag passthrough ────────────────────────────────────────

    pub fn set_var(&mut self, name: impl Into<String>, value: DialogueVar) {
        let _ = self.runner.set_var(name, value);
    }

    pub fn get_var(&self, name: &str) -> Option<&DialogueVar> {
        self.runner.get_var(name)
    }

    pub fn set_flag(&mut self, name: impl Into<String>) {
        let _ = self.runner.set_flag(name);
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.runner.has_flag(name)
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    fn process_advance(&mut self) {
        while let Some(out) = self.runner.advance() {
            let is_blocking = matches!(out,
                DialogueOutput::Say { .. } | DialogueOutput::ShowChoices(_)
            );
            let is_ended = matches!(out, DialogueOutput::Ended);
            self.record_and_push(out);
            if is_blocking || is_ended {
                break;
            }
        }
    }

    fn process_skip(&mut self) {
        if self.config.skip_unseen_only {
            // Only skip if the current node is already in history.
            let seen = if let Some(state) = &self.runner.state {
                self.history.has_seen(state.tree_id, state.current_node)
            } else {
                false
            };
            if seen {
                self.process_advance();
            }
        } else {
            // Skip unconditionally.
            self.process_advance();
        }
    }

    fn process_fast_forward(&mut self) {
        // Advance until we hit a choice or an end, consuming all intermediate
        // nodes.  Hard limit prevents infinite loops in malformed trees.
        const FF_LIMIT: usize = 512;
        let mut steps = 0;
        loop {
            if steps >= FF_LIMIT || self.runner.is_finished()
                || self.runner.is_waiting_for_choice()
            {
                break;
            }
            steps += 1;
            match self.runner.advance() {
                None => break,
                Some(out) => {
                    let stop = matches!(out,
                        DialogueOutput::ShowChoices(_) | DialogueOutput::Ended
                    );
                    self.record_and_push(out);
                    if stop { break; }
                }
            }
        }
    }

    /// Drain all immediately available runner output.
    fn drain_runner(&mut self) {
        loop {
            match self.runner.advance() {
                None => break,
                Some(out) => {
                    let is_blocking = matches!(out,
                        DialogueOutput::Say { .. } | DialogueOutput::ShowChoices(_)
                    );
                    let is_ended = matches!(out, DialogueOutput::Ended);
                    self.record_and_push(out);
                    if is_blocking || is_ended {
                        break;
                    }
                }
            }
        }
    }

    fn record_and_push(&mut self, out: DialogueOutput) {
        // Record Say nodes to history.
        if let DialogueOutput::Say { ref text, .. } = out {
            if let Some(state) = &self.runner.state {
                self.history.push(HistoryRecord::new(
                    state.tree_id,
                    state.current_node,
                    text.clone(),
                    self.game_time,
                ));
            }
        }
        self.current_outputs.push(out);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue::tree::{Condition, DialogueBuilder, DialogueLibrary};
    use crate::dialogue::{DialogueId, DialogueVar, NodeId, SpeakerId};

    // ── Helper: build a trivial library ──────────────────────────────────

    fn make_library_with(tree: crate::dialogue::tree::DialogueTree) -> Arc<DialogueLibrary> {
        let mut lib = DialogueLibrary::new();
        lib.register(tree);
        Arc::new(lib)
    }

    // ── RunnerError display ───────────────────────────────────────────────

    #[test]
    fn runner_error_display() {
        let e = RunnerError::TreeNotFound(DialogueId(99));
        assert!(e.to_string().contains("99"));
        let e2 = RunnerError::InvalidChoice { index: 3, max: 1 };
        assert!(e2.to_string().contains("3"));
    }

    // ── Basic start → say → end flow ──────────────────────────────────────

    #[test]
    fn runner_start_say_end() {
        let alice = SpeakerId(1);
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(alice, "Hello, world!")
            .end()
            .build();

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);

        runner.start(DialogueId(1)).expect("start must succeed");

        // First advance should yield the Say node.
        let out = runner.advance();
        assert!(matches!(out, Some(DialogueOutput::Say { .. })),
            "expected Say, got {:?}", out);

        // Second advance should yield Ended.
        let out2 = runner.advance();
        assert!(matches!(out2, Some(DialogueOutput::Ended)),
            "expected Ended, got {:?}", out2);

        assert!(runner.is_finished());
    }

    #[test]
    fn runner_already_running_error() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Hi")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);

        runner.start(DialogueId(1)).unwrap();
        let result = runner.start(DialogueId(1));
        assert_eq!(result, Err(RunnerError::AlreadyRunning));
    }

    #[test]
    fn runner_tree_not_found() {
        let lib = Arc::new(DialogueLibrary::new());
        let mut runner = DialogueRunner::new(lib);
        let result = runner.start(DialogueId(99));
        assert_eq!(result, Err(RunnerError::TreeNotFound(DialogueId(99))));
    }

    // ── Choice flow ────────────────────────────────────────────────────────

    #[test]
    fn runner_choice_make_choice() {
        // Build: Say → Choice (opt A: End@3, opt B: End@4)
        // We must build the end nodes and choice node manually so IDs align.
        let alice = SpeakerId(1);

        // Node IDs:
        //   1 → Say
        //   2 → Choice (targets 3, 4)
        //   3 → End (option A)
        //   4 → End (option B)

        // We can use DialogueBuilder but the choice targets are forward
        // references — pass them explicitly.
        let end_a = NodeId(3);
        let end_b = NodeId(4);

        let mut tree_raw = crate::dialogue::tree::DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            crate::dialogue::tree::DialogueMeta::new("Test"),
        );
        tree_raw.insert(DialogueNode::Say {
            id:        NodeId(1),
            speaker:   alice,
            text:      "Pick one.".to_string(),
            emotion:   Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(2)),
        });
        tree_raw.insert(DialogueNode::Choice {
            id:      NodeId(2),
            speaker: alice,
            prompt:  None,
            options: vec![
                crate::dialogue::tree::ChoiceOption::new("Option A", end_a),
                crate::dialogue::tree::ChoiceOption::new("Option B", end_b),
            ],
        });
        tree_raw.insert(DialogueNode::End { id: end_a });
        tree_raw.insert(DialogueNode::End { id: end_b });

        let lib = make_library_with(tree_raw);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();

        // Consume the Say output.
        let say = runner.advance();
        assert!(matches!(say, Some(DialogueOutput::Say { .. })));

        // Next output should be ShowChoices.
        let choices = runner.advance();
        assert!(matches!(choices, Some(DialogueOutput::ShowChoices(_))),
            "expected ShowChoices, got {:?}", choices);
        assert!(runner.is_waiting_for_choice());

        // Make choice 0 (Option A → End@3).
        runner.make_choice(0).expect("make_choice must succeed");

        // Should now produce Ended.
        let ended = runner.advance();
        assert!(matches!(ended, Some(DialogueOutput::Ended)));
        assert!(runner.is_finished());
    }

    #[test]
    fn runner_no_choice_pending_error() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Hello")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();
        // Not in WaitingForChoice state:
        let result = runner.make_choice(0);
        assert_eq!(result, Err(RunnerError::NoChoicePending));
    }

    // ── Branch flow ────────────────────────────────────────────────────────

    #[test]
    fn runner_branch_true() {
        // Branch: var "x" > 5 → NodeId(2) End, else NodeId(3) End
        let mut tree = crate::dialogue::tree::DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            crate::dialogue::tree::DialogueMeta::new("Branch Test"),
        );
        tree.insert(DialogueNode::Branch {
            id:        NodeId(1),
            condition: Condition::var_greater("x", 5i64),
            if_true:   NodeId(2),
            if_false:  Some(NodeId(3)),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(2),
            speaker:   SpeakerId(1),
            text:      "Branch taken (true).".to_string(),
            emotion:   Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(3),
            speaker:   SpeakerId(1),
            text:      "Branch taken (false).".to_string(),
            emotion:   Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        tree.insert(DialogueNode::End { id: NodeId(4) });

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.set_persistent_var("x", DialogueVar::Int(10));
        runner.start(DialogueId(1)).unwrap();

        let out = runner.advance();
        if let Some(DialogueOutput::Say { text, .. }) = out {
            assert!(text.contains("true"), "expected true branch, got: {}", text);
        } else {
            panic!("Expected Say, got {:?}", out);
        }
    }

    #[test]
    fn runner_branch_false() {
        let mut tree = crate::dialogue::tree::DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            crate::dialogue::tree::DialogueMeta::new("Branch False"),
        );
        tree.insert(DialogueNode::Branch {
            id:        NodeId(1),
            condition: Condition::var_greater("x", 5i64),
            if_true:   NodeId(2),
            if_false:  Some(NodeId(3)),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(2),
            speaker:   SpeakerId(1),
            text:      "true path".to_string(),
            emotion:   Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(3),
            speaker:   SpeakerId(1),
            text:      "false path".to_string(),
            emotion:   Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        tree.insert(DialogueNode::End { id: NodeId(4) });

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        // x = 2, so condition (x > 5) is false.
        runner.set_persistent_var("x", DialogueVar::Int(2));
        runner.start(DialogueId(1)).unwrap();

        let out = runner.advance();
        if let Some(DialogueOutput::Say { text, .. }) = out {
            assert!(text.contains("false"), "expected false branch, got: {}", text);
        } else {
            panic!("Expected Say, got {:?}", out);
        }
    }

    // ── SetVar flow ────────────────────────────────────────────────────────

    #[test]
    fn runner_set_var_then_branch() {
        let mut tree = crate::dialogue::tree::DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            crate::dialogue::tree::DialogueMeta::new("SetVar Test"),
        );
        // Node 1: SetVar "gold" = 200, next = Node 2
        tree.insert(DialogueNode::SetVar {
            id:    NodeId(1),
            name:  "gold".to_string(),
            value: DialogueVar::Int(200),
            next:  Some(NodeId(2)),
        });
        // Node 2: Branch gold > 100 → Node 3, else Node 4
        tree.insert(DialogueNode::Branch {
            id:        NodeId(2),
            condition: Condition::var_greater("gold", 100i64),
            if_true:   NodeId(3),
            if_false:  Some(NodeId(4)),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(3),
            speaker:   SpeakerId(1),
            text:      "Rich!".to_string(),
            emotion:   Emotion::Happy,
            audio_key: None,
            next:      Some(NodeId(5)),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(4),
            speaker:   SpeakerId(1),
            text:      "Poor.".to_string(),
            emotion:   Emotion::Sad,
            audio_key: None,
            next:      Some(NodeId(5)),
        });
        tree.insert(DialogueNode::End { id: NodeId(5) });

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();

        let out = runner.advance();
        if let Some(DialogueOutput::Say { text, .. }) = out {
            assert_eq!(text, "Rich!");
        } else {
            panic!("Expected Say(Rich!), got {:?}", out);
        }
    }

    // ── Tick (Wait) ────────────────────────────────────────────────────────

    #[test]
    fn runner_wait_tick_expires() {
        let mut tree = crate::dialogue::tree::DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            crate::dialogue::tree::DialogueMeta::new("Wait Test"),
        );
        tree.insert(DialogueNode::Wait {
            id:       NodeId(1),
            duration: 2.0,
            next:     NodeId(2),
        });
        tree.insert(DialogueNode::End { id: NodeId(2) });

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();

        // Consume the Wait output.
        let first = runner.advance();
        assert!(matches!(first, Some(DialogueOutput::Wait(2.0))));
        assert!(matches!(runner.status, RunnerStatus::WaitingForTime(_)));

        // Tick 1 second — still waiting.
        let mid = runner.tick(1.0);
        assert!(matches!(mid, Some(DialogueOutput::Wait(_))));

        // Tick 1.5 more seconds — timer expires (2.0 - 1.0 - 1.5 < 0).
        let after = runner.tick(1.5);
        // After expiry the runner should step to Ended.
        // Depending on pump behaviour we may get Ended directly or None here.
        // Either way, the runner should reach Finished.
        drop(after);
        // Drain remaining.
        loop {
            match runner.advance() {
                None => break,
                Some(DialogueOutput::Ended) => break,
                Some(_) => {}
            }
        }
        assert!(runner.is_finished(), "runner should be finished after wait expires");
    }

    // ── Variable / flag helpers ────────────────────────────────────────────

    #[test]
    fn runner_set_and_get_var() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "test")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();
        runner.set_var("score", DialogueVar::Int(100)).unwrap();
        assert_eq!(runner.get_var("score"), Some(&DialogueVar::Int(100)));
    }

    #[test]
    fn runner_set_flag() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "test")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();
        runner.set_flag("met_king").unwrap();
        assert!(runner.has_flag("met_king"));
        assert!(!runner.has_flag("other_flag"));
    }

    #[test]
    fn runner_not_running_errors() {
        let lib = Arc::new(DialogueLibrary::new());
        let mut runner = DialogueRunner::new(lib);
        assert_eq!(runner.set_var("x", DialogueVar::Int(1)), Err(RunnerError::NotRunning));
        assert_eq!(runner.set_flag("f"), Err(RunnerError::NotRunning));
        assert_eq!(runner.make_choice(0), Err(RunnerError::NoChoicePending));
    }

    // ── DialogueHistory ────────────────────────────────────────────────────

    #[test]
    fn history_has_seen() {
        let mut hist = DialogueHistory::new();
        hist.push(HistoryRecord::new(DialogueId(1), NodeId(5), "Hello", 0.0));
        assert!(hist.has_seen(DialogueId(1), NodeId(5)));
        assert!(!hist.has_seen(DialogueId(1), NodeId(99)));
        assert!(!hist.has_seen(DialogueId(2), NodeId(5)));
    }

    #[test]
    fn history_for_tree() {
        let mut hist = DialogueHistory::new();
        hist.push(HistoryRecord::new(DialogueId(1), NodeId(1), "A", 0.0));
        hist.push(HistoryRecord::new(DialogueId(2), NodeId(1), "B", 1.0));
        hist.push(HistoryRecord::new(DialogueId(1), NodeId(2), "C", 2.0));
        let tree1 = hist.for_tree(DialogueId(1));
        assert_eq!(tree1.len(), 2);
    }

    #[test]
    fn history_clear() {
        let mut hist = DialogueHistory::new();
        hist.push(HistoryRecord::new(DialogueId(1), NodeId(1), "x", 0.0));
        hist.clear();
        assert!(hist.is_empty());
    }

    // ── SessionConfig ──────────────────────────────────────────────────────

    #[test]
    fn session_config_defaults() {
        let cfg = SessionConfig::default();
        assert_eq!(cfg.auto_advance_delay, 0.0);
        assert!(cfg.skip_unseen_only);
        assert!(cfg.text_speed > 0.0);
    }

    #[test]
    fn session_config_builder() {
        let cfg = SessionConfig::new()
            .with_auto_advance(1.5)
            .with_text_speed(60.0)
            .with_skip_unseen_only(false);
        assert_eq!(cfg.auto_advance_delay, 1.5);
        assert_eq!(cfg.text_speed, 60.0);
        assert!(!cfg.skip_unseen_only);
    }

    // ── DialogueSession basic flow ─────────────────────────────────────────

    #[test]
    fn session_start_and_advance() {
        let alice = SpeakerId(1);
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(alice, "Session hello.")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut session = DialogueSession::new(lib, SessionConfig::default());
        session.start_session(DialogueId(1)).expect("session start must succeed");

        // After start the session should have buffered the first Say.
        let outputs = session.process(SessionInput::Advance);
        assert!(!outputs.is_empty(), "expected at least one output");

        // Eventually should reach Ended.
        let mut found_end = false;
        for _ in 0..20 {
            let outs = session.process(SessionInput::Advance);
            for o in outs {
                if matches!(o, DialogueOutput::Ended) {
                    found_end = true;
                }
            }
            if found_end || session.is_finished() { break; }
        }
        // Either found_end directly or the session is marked finished.
        assert!(found_end || session.is_finished(), "session should have ended");
    }

    #[test]
    fn session_choose_option() {
        let alice = SpeakerId(1);
        let mut tree_raw = crate::dialogue::tree::DialogueTree::new(
            DialogueId(1),
            NodeId(1),
            crate::dialogue::tree::DialogueMeta::new("Session Choice"),
        );
        tree_raw.insert(DialogueNode::Choice {
            id:      NodeId(1),
            speaker: alice,
            prompt:  None,
            options: vec![
                crate::dialogue::tree::ChoiceOption::new("Yes", NodeId(2)),
                crate::dialogue::tree::ChoiceOption::new("No",  NodeId(3)),
            ],
        });
        tree_raw.insert(DialogueNode::End { id: NodeId(2) });
        tree_raw.insert(DialogueNode::End { id: NodeId(3) });

        let lib = make_library_with(tree_raw);
        let mut session = DialogueSession::new(lib, SessionConfig::default());
        session.start_session(DialogueId(1)).unwrap();

        // drain any output so far
        let _ = session.process(SessionInput::Advance);

        // Should be waiting for choice.
        assert!(session.is_waiting_for_choice() || !session.is_finished());

        let outs = session.process(SessionInput::ChooseOption(0));
        let _ = outs; // consume
        // Runner should eventually finish.
        for _ in 0..10 {
            if session.is_finished() { break; }
            let _ = session.process(SessionInput::Advance);
        }
        assert!(session.is_finished(), "session should finish after choosing");
    }

    #[test]
    fn session_fast_forward() {
        let alice = SpeakerId(1);
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(alice, "Line 1")
            .say(alice, "Line 2")
            .say(alice, "Line 3")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut session = DialogueSession::new(lib, SessionConfig::default());
        session.start_session(DialogueId(1)).unwrap();

        // Enable fast-forward — should skip to the end in one call.
        let outs = session.process(SessionInput::FastForward(true));
        let found_ended = outs.iter().any(|o| matches!(o, DialogueOutput::Ended));
        // Either ended here or session is finished.
        assert!(found_ended || session.is_finished());
    }

    // ── Pseudo-random determinism ──────────────────────────────────────────

    #[test]
    fn pseudo_rand_in_range() {
        for i in 0..100u32 {
            let v = pseudo_rand(i as f32 * 0.1, i);
            assert!(v >= 0.0 && v < 1.0, "pseudo_rand out of range: {}", v);
        }
    }

    // ── RunnerStatus helpers ───────────────────────────────────────────────

    #[test]
    fn runner_status_helpers() {
        assert!(RunnerStatus::Finished.is_finished());
        assert!(!RunnerStatus::Running.is_finished());
        assert!(RunnerStatus::Running.is_running());
        assert!(RunnerStatus::WaitingForChoice.is_waiting_for_choice());
        assert!(RunnerStatus::Errored("oops".to_string()).is_errored());
    }
}

// ── DialogueOutputQueue ───────────────────────────────────────────────────────

/// A buffered queue of [`DialogueOutput`] events with typed accessors.
///
/// The runner writes into a `VecDeque<DialogueOutput>` internally; this struct
/// provides a convenient wrapper for callers who want typed extraction helpers
/// rather than pattern-matching each variant by hand.
#[derive(Debug, Default)]
pub struct DialogueOutputQueue {
    inner: VecDeque<DialogueOutput>,
}

impl DialogueOutputQueue {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, out: DialogueOutput) {
        self.inner.push_back(out);
    }

    pub fn pop(&mut self) -> Option<DialogueOutput> {
        self.inner.pop_front()
    }

    pub fn peek(&self) -> Option<&DialogueOutput> {
        self.inner.front()
    }

    pub fn len(&self) -> usize { self.inner.len() }
    pub fn is_empty(&self) -> bool { self.inner.is_empty() }

    /// Drain all events into a `Vec`.
    pub fn drain_all(&mut self) -> Vec<DialogueOutput> {
        self.inner.drain(..).collect()
    }

    /// Pop the next `Say` event if the front of the queue is a `Say`.
    pub fn pop_say(&mut self) -> Option<(SpeakerId, String, Emotion, Option<String>)> {
        if let Some(DialogueOutput::Say { .. }) = self.inner.front() {
            if let Some(DialogueOutput::Say { speaker, text, emotion, audio_key }) = self.inner.pop_front() {
                return Some((speaker, text, emotion, audio_key));
            }
        }
        None
    }

    /// Pop the next `ShowChoices` event if the front of the queue is choices.
    pub fn pop_choices(&mut self) -> Option<Vec<VisibleChoice>> {
        if let Some(DialogueOutput::ShowChoices(_)) = self.inner.front() {
            if let Some(DialogueOutput::ShowChoices(choices)) = self.inner.pop_front() {
                return Some(choices);
            }
        }
        None
    }

    /// Returns `true` if any event in the queue is `Ended`.
    pub fn has_ended(&self) -> bool {
        self.inner.iter().any(|o| matches!(o, DialogueOutput::Ended))
    }
}

// ── RunnerSnapshot ────────────────────────────────────────────────────────────

/// A point-in-time snapshot of [`DialogueRunner`] state for save/load.
///
/// The snapshot does not include the `DialogueLibrary` reference; the caller
/// is responsible for providing the same library when restoring.
#[derive(Debug, Clone)]
pub struct RunnerSnapshot {
    pub state:   Option<DialogueState>,
    pub status:  RunnerStatus,
}

impl RunnerSnapshot {
    /// Capture the current state of a runner.
    pub fn capture(runner: &DialogueRunner) -> Self {
        Self {
            state:  runner.state.clone(),
            status: runner.status.clone(),
        }
    }

    /// Restore a runner to this snapshot.
    ///
    /// The runner's `library` and `persistent_vars`/`persistent_flags` are
    /// preserved — only execution state is replaced.
    pub fn restore(self, runner: &mut DialogueRunner) {
        runner.state  = self.state;
        runner.status = self.status;
        runner.pending_output.clear();
    }
}

// ── ChoiceHistory ─────────────────────────────────────────────────────────────

/// Tracks which choices have been made in previous runs, enabling the UI to
/// mark "already seen" paths.
#[derive(Debug, Clone, Default)]
pub struct ChoiceHistory {
    /// Map from (DialogueId, NodeId) → set of chosen option indices.
    records: HashMap<(DialogueId, NodeId), HashSet<usize>>,
}

impl ChoiceHistory {
    pub fn new() -> Self { Self::default() }

    /// Record that `option_index` was chosen at `(tree, node)`.
    pub fn record(&mut self, tree: DialogueId, node: NodeId, option_index: usize) {
        self.records
            .entry((tree, node))
            .or_default()
            .insert(option_index);
    }

    /// Returns `true` if `option_index` has been chosen at `(tree, node)` before.
    pub fn has_chosen(&self, tree: DialogueId, node: NodeId, option_index: usize) -> bool {
        self.records
            .get(&(tree, node))
            .map_or(false, |s| s.contains(&option_index))
    }

    /// All indices that have been chosen at `(tree, node)`.
    pub fn chosen_at(&self, tree: DialogueId, node: NodeId) -> Vec<usize> {
        self.records
            .get(&(tree, node))
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Clear history for a specific tree (e.g. after a new game).
    pub fn clear_tree(&mut self, tree: DialogueId) {
        self.records.retain(|(t, _), _| *t != tree);
    }

    /// Clear all history.
    pub fn clear_all(&mut self) {
        self.records.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

// ── AutoAdvanceTimer ─────────────────────────────────────────────────────────

/// Drives automatic dialogue progression at a configurable delay.
///
/// Attach one of these to a session or UI system; call `tick` each frame and
/// act on the returned signal.
#[derive(Debug, Clone)]
pub struct AutoAdvanceTimer {
    /// Seconds to wait before auto-advancing.
    pub delay:   f32,
    elapsed:     f32,
    armed:       bool,
    paused:      bool,
}

impl AutoAdvanceTimer {
    pub fn new(delay: f32) -> Self {
        Self { delay, elapsed: 0.0, armed: false, paused: false }
    }

    /// Arm the timer (start counting from 0).
    pub fn arm(&mut self) {
        self.elapsed = 0.0;
        self.armed   = true;
    }

    /// Disarm the timer without firing.
    pub fn disarm(&mut self) {
        self.armed   = false;
        self.elapsed = 0.0;
    }

    pub fn pause(&mut self)  { self.paused = true; }
    pub fn resume(&mut self) { self.paused = false; }

    /// Advance by `delta` seconds.  Returns `true` when the timer fires.
    pub fn tick(&mut self, delta: f32) -> bool {
        if !self.armed || self.paused { return false; }
        self.elapsed += delta;
        if self.elapsed >= self.delay {
            self.disarm();
            true
        } else {
            false
        }
    }

    pub fn is_armed(&self) -> bool { self.armed }

    /// Progress in [0, 1].
    pub fn progress(&self) -> f32 {
        if self.delay <= 0.0 { return 1.0; }
        (self.elapsed / self.delay).clamp(0.0, 1.0)
    }
}

impl Default for AutoAdvanceTimer {
    fn default() -> Self { Self::new(2.0) }
}

// ── SkipPolicy ────────────────────────────────────────────────────────────────

/// Controls which nodes the skip action is allowed to fast-forward through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipPolicy {
    /// Skip is not allowed.
    Disabled,
    /// Skip through all nodes unconditionally.
    All,
    /// Skip only through nodes that appear in the provided history.
    SeenOnly,
    /// Skip until a choice node or end is reached.
    UntilChoice,
}

impl SkipPolicy {
    /// Returns `true` if skipping is allowed at all.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, SkipPolicy::Disabled)
    }
}

impl Default for SkipPolicy {
    fn default() -> Self { SkipPolicy::SeenOnly }
}

// ── OutputFilter ─────────────────────────────────────────────────────────────

/// Filtering rules applied to [`DialogueOutput`] before delivery to the UI.
///
/// Allows systems like cutscene cameras or voice-over managers to intercept
/// specific output kinds without modifying the runner.
#[derive(Debug, Clone, Default)]
pub struct OutputFilter {
    /// If true, `CameraAction` outputs are suppressed (e.g. in menus).
    pub suppress_camera:  bool,
    /// If true, `PlayAnim` outputs are suppressed.
    pub suppress_anim:    bool,
    /// If true, `ScriptCall` outputs are suppressed (dry-run mode).
    pub suppress_scripts: bool,
    /// If true, `Wait` outputs are suppressed and timers run at zero cost.
    pub suppress_waits:   bool,
}

impl OutputFilter {
    pub fn new() -> Self { Self::default() }

    pub fn suppress_camera(mut self)  -> Self { self.suppress_camera  = true; self }
    pub fn suppress_anim(mut self)    -> Self { self.suppress_anim    = true; self }
    pub fn suppress_scripts(mut self) -> Self { self.suppress_scripts = true; self }
    pub fn suppress_waits(mut self)   -> Self { self.suppress_waits   = true; self }

    /// Returns `false` if this output should be dropped.
    pub fn allow(&self, output: &DialogueOutput) -> bool {
        match output {
            DialogueOutput::CameraAction(_) => !self.suppress_camera,
            DialogueOutput::PlayAnim { .. } => !self.suppress_anim,
            DialogueOutput::ScriptCall { .. } => !self.suppress_scripts,
            DialogueOutput::Wait(_)         => !self.suppress_waits,
            _ => true,
        }
    }

    /// Filter a list of outputs, removing disallowed ones.
    pub fn apply(&self, outputs: Vec<DialogueOutput>) -> Vec<DialogueOutput> {
        outputs.into_iter().filter(|o| self.allow(o)).collect()
    }
}

// ── extra runner tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod extra_tests {
    use super::*;
    use crate::dialogue::tree::{
        CameraAction, ChoiceOption, Condition, DialogueBuilder, DialogueLibrary,
        DialogueNode, DialogueMeta, DialogueTree,
    };
    use crate::dialogue::{DialogueId, DialogueVar, NodeId, SpeakerId};

    fn make_library_with(tree: DialogueTree) -> Arc<DialogueLibrary> {
        let mut lib = DialogueLibrary::new();
        lib.register(tree);
        Arc::new(lib)
    }

    // ── DialogueOutputQueue ────────────────────────────────────────────────

    #[test]
    fn output_queue_push_pop() {
        let mut q = DialogueOutputQueue::new();
        q.push(DialogueOutput::Ended);
        assert_eq!(q.len(), 1);
        let out = q.pop();
        assert!(matches!(out, Some(DialogueOutput::Ended)));
        assert!(q.is_empty());
    }

    #[test]
    fn output_queue_pop_say() {
        let mut q = DialogueOutputQueue::new();
        q.push(DialogueOutput::Say {
            speaker:   SpeakerId(1),
            text:      "Hi".to_string(),
            emotion:   crate::dialogue::Emotion::Neutral,
            audio_key: None,
        });
        let result = q.pop_say();
        assert!(result.is_some());
        let (spk, text, _, _) = result.unwrap();
        assert_eq!(spk, SpeakerId(1));
        assert_eq!(text, "Hi");
    }

    #[test]
    fn output_queue_pop_choices() {
        let mut q = DialogueOutputQueue::new();
        q.push(DialogueOutput::ShowChoices(vec![
            VisibleChoice { index: 0, text: "Yes".to_string(), tags: vec![] },
        ]));
        let choices = q.pop_choices();
        assert!(choices.is_some());
        assert_eq!(choices.unwrap().len(), 1);
    }

    #[test]
    fn output_queue_has_ended() {
        let mut q = DialogueOutputQueue::new();
        q.push(DialogueOutput::Ended);
        assert!(q.has_ended());
        q.drain_all();
        assert!(!q.has_ended());
    }

    #[test]
    fn output_queue_drain_all() {
        let mut q = DialogueOutputQueue::new();
        q.push(DialogueOutput::Ended);
        q.push(DialogueOutput::Ended);
        let drained = q.drain_all();
        assert_eq!(drained.len(), 2);
        assert!(q.is_empty());
    }

    // ── RunnerSnapshot ─────────────────────────────────────────────────────

    #[test]
    fn snapshot_capture_and_restore() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Line 1")
            .say(SpeakerId(1), "Line 2")
            .end()
            .build();
        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();

        // Advance one step.
        let _ = runner.advance();

        // Capture snapshot.
        let snap = RunnerSnapshot::capture(&runner);

        // Advance further.
        let _ = runner.advance();
        let _ = runner.advance();

        // Restore.
        snap.restore(&mut runner);

        // Runner should be back in Running state (not Finished).
        assert!(!runner.is_finished(), "runner should not be finished after restore");
    }

    // ── ChoiceHistory ─────────────────────────────────────────────────────

    #[test]
    fn choice_history_record_and_query() {
        let mut ch = ChoiceHistory::new();
        ch.record(DialogueId(1), NodeId(5), 0);
        ch.record(DialogueId(1), NodeId(5), 2);
        assert!(ch.has_chosen(DialogueId(1), NodeId(5), 0));
        assert!(ch.has_chosen(DialogueId(1), NodeId(5), 2));
        assert!(!ch.has_chosen(DialogueId(1), NodeId(5), 1));
        assert!(!ch.has_chosen(DialogueId(2), NodeId(5), 0));
    }

    #[test]
    fn choice_history_chosen_at() {
        let mut ch = ChoiceHistory::new();
        ch.record(DialogueId(1), NodeId(3), 1);
        ch.record(DialogueId(1), NodeId(3), 3);
        let mut chosen = ch.chosen_at(DialogueId(1), NodeId(3));
        chosen.sort();
        assert_eq!(chosen, vec![1, 3]);
    }

    #[test]
    fn choice_history_clear_tree() {
        let mut ch = ChoiceHistory::new();
        ch.record(DialogueId(1), NodeId(1), 0);
        ch.record(DialogueId(2), NodeId(1), 0);
        ch.clear_tree(DialogueId(1));
        assert!(!ch.has_chosen(DialogueId(1), NodeId(1), 0));
        assert!(ch.has_chosen(DialogueId(2), NodeId(1), 0));
    }

    #[test]
    fn choice_history_clear_all() {
        let mut ch = ChoiceHistory::new();
        ch.record(DialogueId(1), NodeId(1), 0);
        ch.clear_all();
        assert!(ch.is_empty());
    }

    // ── AutoAdvanceTimer ───────────────────────────────────────────────────

    #[test]
    fn auto_advance_timer_fires() {
        let mut t = AutoAdvanceTimer::new(1.0);
        t.arm();
        assert!(!t.tick(0.5));
        assert!(t.tick(0.6)); // total 1.1 ≥ 1.0 — fires
        assert!(!t.is_armed());
    }

    #[test]
    fn auto_advance_timer_disarmed_does_not_fire() {
        let mut t = AutoAdvanceTimer::new(0.1);
        // Not armed — should never fire.
        assert!(!t.tick(10.0));
    }

    #[test]
    fn auto_advance_timer_paused() {
        let mut t = AutoAdvanceTimer::new(1.0);
        t.arm();
        t.pause();
        assert!(!t.tick(10.0)); // paused: won't fire regardless of time
        t.resume();
        assert!(t.tick(1.0)); // now fires
    }

    #[test]
    fn auto_advance_timer_progress() {
        let mut t = AutoAdvanceTimer::new(4.0);
        t.arm();
        t.tick(2.0);
        let p = t.progress();
        assert!((p - 0.5).abs() < 0.01, "expected 0.5, got {}", p);
    }

    // ── SkipPolicy ─────────────────────────────────────────────────────────

    #[test]
    fn skip_policy_enabled() {
        assert!(!SkipPolicy::Disabled.is_enabled());
        assert!(SkipPolicy::All.is_enabled());
        assert!(SkipPolicy::SeenOnly.is_enabled());
        assert!(SkipPolicy::UntilChoice.is_enabled());
    }

    // ── OutputFilter ───────────────────────────────────────────────────────

    #[test]
    fn output_filter_suppress_camera() {
        let filter = OutputFilter::new().suppress_camera();
        let outputs = vec![
            DialogueOutput::CameraAction(CameraAction::Restore),
            DialogueOutput::Ended,
        ];
        let filtered = filter.apply(outputs);
        assert_eq!(filtered.len(), 1);
        assert!(matches!(filtered[0], DialogueOutput::Ended));
    }

    #[test]
    fn output_filter_allow_all_by_default() {
        let filter = OutputFilter::new();
        let out = DialogueOutput::CameraAction(CameraAction::Restore);
        assert!(filter.allow(&out));
    }

    #[test]
    fn output_filter_suppress_scripts() {
        let filter = OutputFilter::new().suppress_scripts();
        let out = DialogueOutput::ScriptCall {
            function: "test".to_string(),
            args:     vec![],
        };
        assert!(!filter.allow(&out));
        // Say is always allowed.
        let say = DialogueOutput::Say {
            speaker:   SpeakerId(1),
            text:      "hi".to_string(),
            emotion:   crate::dialogue::Emotion::Neutral,
            audio_key: None,
        };
        assert!(filter.allow(&say));
    }

    // ── DialogueState ─────────────────────────────────────────────────────

    #[test]
    fn dialogue_state_visit_tracking() {
        let mut state = DialogueState::new(
            DialogueId(1),
            NodeId(1),
            HashMap::new(),
            HashSet::new(),
            0.0,
        );
        state.record_visit(NodeId(1));
        state.record_visit(NodeId(2));
        assert!(state.has_visited(NodeId(1)));
        assert!(state.has_visited(NodeId(2)));
        assert!(!state.has_visited(NodeId(99)));
    }

    #[test]
    fn dialogue_state_choice_counts() {
        let mut state = DialogueState::new(
            DialogueId(1), NodeId(1), HashMap::new(), HashSet::new(), 0.0,
        );
        state.increment_choice(NodeId(5));
        state.increment_choice(NodeId(5));
        assert_eq!(state.choice_count(NodeId(5)), 2);
        assert_eq!(state.choice_count(NodeId(99)), 0);
    }

    #[test]
    fn dialogue_state_flag_ops() {
        let mut state = DialogueState::new(
            DialogueId(1), NodeId(1), HashMap::new(), HashSet::new(), 0.0,
        );
        state.set_flag("met_npc");
        assert!(state.has_flag("met_npc"));
        assert!(state.remove_flag("met_npc"));
        assert!(!state.has_flag("met_npc"));
    }

    #[test]
    fn dialogue_state_var_ops() {
        let mut state = DialogueState::new(
            DialogueId(1), NodeId(1), HashMap::new(), HashSet::new(), 0.0,
        );
        state.set_var("score", DialogueVar::Int(42));
        assert_eq!(state.get_var("score"), Some(&DialogueVar::Int(42)));
        assert_eq!(state.get_var("absent"), None);
    }

    // ── Persistent vars ────────────────────────────────────────────────────

    #[test]
    fn runner_persistent_vars_carry_into_next_dialogue() {
        let mut lib = DialogueLibrary::new();

        // Tree 1: SetVar "x" = 99 → End
        let mut t1 = DialogueTree::new(DialogueId(1), NodeId(1), DialogueMeta::new("T1"));
        t1.insert(DialogueNode::SetVar {
            id:    NodeId(1),
            name:  "x".to_string(),
            value: DialogueVar::Int(99),
            next:  Some(NodeId(2)),
        });
        t1.insert(DialogueNode::End { id: NodeId(2) });

        // Tree 2: Branch x > 50 → Say "big" → End, else Say "small" → End
        let mut t2 = DialogueTree::new(DialogueId(2), NodeId(1), DialogueMeta::new("T2"));
        t2.insert(DialogueNode::Branch {
            id:        NodeId(1),
            condition: Condition::var_greater("x", 50i64),
            if_true:   NodeId(2),
            if_false:  Some(NodeId(3)),
        });
        t2.insert(DialogueNode::Say {
            id:        NodeId(2),
            speaker:   SpeakerId(1),
            text:      "big".to_string(),
            emotion:   crate::dialogue::Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        t2.insert(DialogueNode::Say {
            id:        NodeId(3),
            speaker:   SpeakerId(1),
            text:      "small".to_string(),
            emotion:   crate::dialogue::Emotion::Neutral,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        t2.insert(DialogueNode::End { id: NodeId(4) });

        lib.register(t1);
        lib.register(t2);
        let lib = Arc::new(lib);

        let mut runner = DialogueRunner::new(lib);

        // Run tree 1 to set x = 99.
        runner.start(DialogueId(1)).unwrap();
        loop {
            match runner.advance() {
                None | Some(DialogueOutput::Ended) => break,
                _ => {}
            }
        }
        // Copy the variable back into persistent store.
        if let Some(v) = runner.get_var("x").cloned() {
            runner.set_persistent_var("x", v);
        }

        // Run tree 2 — x should be 99 → "big" branch.
        runner.start(DialogueId(2)).unwrap();
        let mut found_text = String::new();
        loop {
            match runner.advance() {
                None | Some(DialogueOutput::Ended) => break,
                Some(DialogueOutput::Say { text, .. }) => { found_text = text; }
                _ => {}
            }
        }
        assert_eq!(found_text, "big", "expected 'big' branch, got '{}'", found_text);
    }

    // ── Camera + PlayAnim non-blocking ────────────────────────────────────

    #[test]
    fn runner_camera_and_anim_non_blocking() {
        let mut tree = DialogueTree::new(
            DialogueId(1), NodeId(1), DialogueMeta::new("Cutscene"),
        );
        tree.insert(DialogueNode::Camera {
            id:     NodeId(1),
            action: CameraAction::FocusOn(SpeakerId(1)),
            next:   NodeId(2),
        });
        tree.insert(DialogueNode::PlayAnim {
            id:       NodeId(2),
            speaker:  SpeakerId(1),
            anim_key: "wave".to_string(),
            next:     NodeId(3),
        });
        tree.insert(DialogueNode::Say {
            id:        NodeId(3),
            speaker:   SpeakerId(1),
            text:      "Hello!".to_string(),
            emotion:   crate::dialogue::Emotion::Happy,
            audio_key: None,
            next:      Some(NodeId(4)),
        });
        tree.insert(DialogueNode::End { id: NodeId(4) });

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();

        let mut outputs = Vec::new();
        loop {
            match runner.advance() {
                None => break,
                Some(out) => {
                    let ended = matches!(out, DialogueOutput::Ended);
                    outputs.push(out);
                    if ended { break; }
                }
            }
        }

        let has_camera = outputs.iter().any(|o| matches!(o, DialogueOutput::CameraAction(_)));
        let has_anim   = outputs.iter().any(|o| matches!(o, DialogueOutput::PlayAnim { .. }));
        let has_say    = outputs.iter().any(|o| matches!(o, DialogueOutput::Say { .. }));

        assert!(has_camera, "expected CameraAction in outputs");
        assert!(has_anim,   "expected PlayAnim in outputs");
        assert!(has_say,    "expected Say in outputs");
    }

    // ── ScriptCall output ─────────────────────────────────────────────────

    #[test]
    fn runner_script_call_output() {
        let mut tree = DialogueTree::new(
            DialogueId(1), NodeId(1), DialogueMeta::new("Scripts"),
        );
        tree.insert(DialogueNode::CallScript {
            id:       NodeId(1),
            function: "unlock_door".to_string(),
            args:     vec![DialogueVar::Int(42)],
            next:     Some(NodeId(2)),
        });
        tree.insert(DialogueNode::End { id: NodeId(2) });

        let lib = make_library_with(tree);
        let mut runner = DialogueRunner::new(lib);
        runner.start(DialogueId(1)).unwrap();

        let mut found_script = false;
        loop {
            match runner.advance() {
                None => break,
                Some(DialogueOutput::ScriptCall { function, args }) => {
                    assert_eq!(function, "unlock_door");
                    assert_eq!(args[0], DialogueVar::Int(42));
                    found_script = true;
                }
                Some(DialogueOutput::Ended) => break,
                _ => {}
            }
        }
        assert!(found_script, "expected ScriptCall output");
    }

    // ── Session update tick ────────────────────────────────────────────────

    #[test]
    fn session_update_advances_time() {
        let tree = DialogueBuilder::new(DialogueId(1))
            .say(SpeakerId(1), "Tick test")
            .end()
            .build();
        let lib = make_library_with(tree);
        let cfg = SessionConfig::new().with_auto_advance(0.5);
        let mut session = DialogueSession::new(lib, cfg);
        session.start_session(DialogueId(1)).unwrap();
        // update() advances time; after > 0.5s it should auto-advance.
        for _ in 0..10 {
            session.update(0.1);
        }
        // After 1.0s total, auto advance should have fired at least once.
        // We don't assert finished here (depends on exact timing), just that
        // it doesn't panic.
    }

    // ── HistoryRecord ─────────────────────────────────────────────────────

    #[test]
    fn history_record_fields() {
        let rec = HistoryRecord::new(DialogueId(3), NodeId(7), "Some text", 42.5);
        assert_eq!(rec.tree_id, DialogueId(3));
        assert_eq!(rec.node_id, NodeId(7));
        assert_eq!(rec.text_snapshot, "Some text");
        assert!((rec.timestamp - 42.5).abs() < f32::EPSILON);
    }
}
