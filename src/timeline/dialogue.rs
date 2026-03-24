//! Dialogue system — typewriter effect, choice trees, speaker portraits.
//!
//! A `DialogueTree` is a directed graph of `DialogueNode`s connected by `Choice`s.
//! The `DialoguePlayer` renders the current node character-by-character with a
//! configurable typewriter effect and waits for player input to advance.

use std::collections::HashMap;

// ── DialogueNode ──────────────────────────────────────────────────────────────

/// A single node in the dialogue tree.
#[derive(Clone, Debug)]
pub struct DialogueNode {
    pub id:       String,
    pub speaker:  String,
    pub text:     String,
    /// Optional portrait key (maps to an atlas glyph or texture name).
    pub portrait: Option<String>,
    /// Emotion tag for expression/color changes.
    pub emotion:  DialogueEmotion,
    /// What comes next.
    pub next:     DialogueNext,
}

/// How to advance after a node.
#[derive(Clone, Debug)]
pub enum DialogueNext {
    /// Jump to another node by ID.
    Node(String),
    /// Present choices to the player.
    Choice(Vec<Choice>),
    /// The dialogue tree ends.
    End,
    /// Jump to End after a timer (auto-advance).
    Auto { duration: f32, then: Box<DialogueNext> },
}

/// A selectable choice in the dialogue.
#[derive(Clone, Debug)]
pub struct Choice {
    pub text:     String,
    pub next:     String,  // node ID
    /// Condition flag — only shown if this flag is true (or None = always shown).
    pub requires: Option<String>,
    /// Consequence flags to set when chosen.
    pub sets:     Vec<(String, bool)>,
}

impl Choice {
    pub fn new(text: impl Into<String>, next: impl Into<String>) -> Self {
        Self { text: text.into(), next: next.into(), requires: None, sets: Vec::new() }
    }

    pub fn requires(mut self, flag: impl Into<String>) -> Self {
        self.requires = Some(flag.into());
        self
    }

    pub fn sets_flag(mut self, flag: impl Into<String>, value: bool) -> Self {
        self.sets.push((flag.into(), value));
        self
    }
}

/// Speaker emotion — affects text color and portrait expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DialogueEmotion {
    #[default]
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Scared,
    Suspicious,
    Mysterious,
}

impl DialogueEmotion {
    /// RGBA color associated with this emotion (for text tinting).
    pub fn color(self) -> [f32; 4] {
        match self {
            DialogueEmotion::Neutral    => [1.0, 1.0, 1.0, 1.0],
            DialogueEmotion::Happy      => [1.0, 0.95, 0.5, 1.0],
            DialogueEmotion::Sad        => [0.5, 0.6, 0.9, 1.0],
            DialogueEmotion::Angry      => [1.0, 0.3, 0.2, 1.0],
            DialogueEmotion::Surprised  => [0.9, 0.7, 1.0, 1.0],
            DialogueEmotion::Scared     => [0.6, 0.9, 0.7, 1.0],
            DialogueEmotion::Suspicious => [0.8, 0.8, 0.4, 1.0],
            DialogueEmotion::Mysterious => [0.5, 0.4, 0.9, 1.0],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DialogueEmotion::Neutral    => "neutral",
            DialogueEmotion::Happy      => "happy",
            DialogueEmotion::Sad        => "sad",
            DialogueEmotion::Angry      => "angry",
            DialogueEmotion::Surprised  => "surprised",
            DialogueEmotion::Scared     => "scared",
            DialogueEmotion::Suspicious => "suspicious",
            DialogueEmotion::Mysterious => "mysterious",
        }
    }
}

// ── DialogueTree ──────────────────────────────────────────────────────────────

/// A collection of nodes forming a branching conversation.
#[derive(Clone, Debug, Default)]
pub struct DialogueTree {
    pub name:       String,
    pub nodes:      HashMap<String, DialogueNode>,
    pub start_node: String,
}

impl DialogueTree {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), nodes: HashMap::new(), start_node: String::new() }
    }

    pub fn add_node(mut self, node: DialogueNode) -> Self {
        if self.start_node.is_empty() {
            self.start_node = node.id.clone();
        }
        self.nodes.insert(node.id.clone(), node);
        self
    }

    pub fn with_start(mut self, id: impl Into<String>) -> Self {
        self.start_node = id.into();
        self
    }

    pub fn get(&self, id: &str) -> Option<&DialogueNode> {
        self.nodes.get(id)
    }
}

// ── NodeBuilder ───────────────────────────────────────────────────────────────

/// Fluent builder for DialogueNode.
pub struct NodeBuilder {
    id:       String,
    speaker:  String,
    text:     String,
    portrait: Option<String>,
    emotion:  DialogueEmotion,
}

impl NodeBuilder {
    pub fn new(id: impl Into<String>, speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id:       id.into(),
            speaker:  speaker.into(),
            text:     text.into(),
            portrait: None,
            emotion:  DialogueEmotion::Neutral,
        }
    }

    pub fn portrait(mut self, p: impl Into<String>) -> Self { self.portrait = Some(p.into()); self }
    pub fn emotion(mut self, e: DialogueEmotion) -> Self { self.emotion = e; self }

    pub fn end(self) -> DialogueNode {
        DialogueNode { id: self.id, speaker: self.speaker, text: self.text,
                       portrait: self.portrait, emotion: self.emotion,
                       next: DialogueNext::End }
    }

    pub fn then(self, next_id: impl Into<String>) -> DialogueNode {
        DialogueNode { id: self.id, speaker: self.speaker, text: self.text,
                       portrait: self.portrait, emotion: self.emotion,
                       next: DialogueNext::Node(next_id.into()) }
    }

    pub fn choices(self, choices: Vec<Choice>) -> DialogueNode {
        DialogueNode { id: self.id, speaker: self.speaker, text: self.text,
                       portrait: self.portrait, emotion: self.emotion,
                       next: DialogueNext::Choice(choices) }
    }

    pub fn auto(self, duration: f32, then: DialogueNext) -> DialogueNode {
        DialogueNode { id: self.id, speaker: self.speaker, text: self.text,
                       portrait: self.portrait, emotion: self.emotion,
                       next: DialogueNext::Auto { duration, then: Box::new(then) } }
    }
}

// ── Typewriter state ──────────────────────────────────────────────────────────

/// Typewriter render state for a line of text.
#[derive(Clone, Debug)]
pub struct TypewriterState {
    pub full_text:    String,
    pub chars_shown:  usize,  // how many chars have been revealed
    pub chars_per_sec: f32,
    pub accumulator:  f32,    // fractional char accumulator
    pub complete:     bool,
    /// Pause accumulator for punctuation delays.
    pub pause_timer:  f32,
}

impl TypewriterState {
    pub fn new(text: impl Into<String>, chars_per_sec: f32) -> Self {
        let text = text.into();
        let complete = text.is_empty();
        Self {
            full_text: text,
            chars_shown: 0,
            chars_per_sec,
            accumulator: 0.0,
            complete,
            pause_timer: 0.0,
        }
    }

    /// Advance by dt seconds. Returns true if newly completed.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.complete { return false; }

        // Punctuation pause
        if self.pause_timer > 0.0 {
            self.pause_timer -= dt;
            return false;
        }

        self.accumulator += dt * self.chars_per_sec;
        let new_chars = self.accumulator as usize;
        self.accumulator -= new_chars as f32;

        for _ in 0..new_chars {
            if self.chars_shown < self.full_text.len() {
                // Pause after sentence-ending punctuation
                let ch = self.full_text.chars().nth(self.chars_shown).unwrap_or(' ');
                self.chars_shown += 1;
                match ch {
                    '.' | '!' | '?' => self.pause_timer = 0.25,
                    ',' | ';'       => self.pause_timer = 0.1,
                    _ => {}
                }
                if self.chars_shown >= self.full_text.chars().count() {
                    self.complete = true;
                    return true;
                }
            }
        }
        false
    }

    /// Skip to end immediately.
    pub fn skip(&mut self) {
        self.chars_shown = self.full_text.chars().count();
        self.complete    = true;
        self.pause_timer = 0.0;
    }

    /// The currently visible portion of the text.
    pub fn visible_text(&self) -> &str {
        if self.chars_shown >= self.full_text.len() {
            &self.full_text
        } else {
            &self.full_text[..self.char_byte_offset(self.chars_shown)]
        }
    }

    fn char_byte_offset(&self, n: usize) -> usize {
        self.full_text.char_indices().nth(n).map(|(i, _)| i).unwrap_or(self.full_text.len())
    }

    /// Progress [0, 1].
    pub fn progress(&self) -> f32 {
        let total = self.full_text.chars().count();
        if total == 0 { 1.0 } else { self.chars_shown as f32 / total as f32 }
    }
}

// ── DialoguePlayer ────────────────────────────────────────────────────────────

/// Drives a DialogueTree.
pub struct DialoguePlayer {
    pub tree:         DialogueTree,
    pub current_node: Option<String>,
    pub typewriter:   Option<TypewriterState>,
    pub state:        DialogueState,
    pub flags:        HashMap<String, bool>,
    pub history:      Vec<String>,          // node ids visited in order
    pub chars_per_sec: f32,
    auto_timer:       Option<f32>,
    /// Available choices (after typewriter completes on a Choice node).
    pub choices:      Vec<Choice>,
    pub selected_choice: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogueState {
    Idle,
    /// Typewriter is running.
    Typing,
    /// Typewriter done, waiting for advance input.
    Waiting,
    /// Showing choices.
    Choosing,
    /// Auto-advance timer running.
    AutoTimer,
    /// Done.
    Finished,
}

impl DialoguePlayer {
    pub fn new(tree: DialogueTree) -> Self {
        Self {
            tree,
            current_node:    None,
            typewriter:      None,
            state:           DialogueState::Idle,
            flags:           HashMap::new(),
            history:         Vec::new(),
            chars_per_sec:   28.0,
            auto_timer:      None,
            choices:         Vec::new(),
            selected_choice: 0,
        }
    }

    pub fn with_speed(mut self, chars_per_sec: f32) -> Self {
        self.chars_per_sec = chars_per_sec;
        self
    }

    /// Start the dialogue from its start node.
    pub fn start(&mut self) {
        let id = self.tree.start_node.clone();
        self.goto(&id);
    }

    /// Jump to a specific node by ID.
    pub fn goto(&mut self, id: &str) {
        if let Some(node) = self.tree.get(id).cloned() {
            self.history.push(id.to_string());
            self.current_node = Some(id.to_string());
            self.typewriter   = Some(TypewriterState::new(&node.text, self.chars_per_sec));
            self.state        = DialogueState::Typing;
            self.choices.clear();
            self.selected_choice = 0;
            self.auto_timer = None;
        }
    }

    pub fn is_finished(&self) -> bool { self.state == DialogueState::Finished }
    pub fn is_typing(&self) -> bool   { self.state == DialogueState::Typing  }

    /// Current visible text (typewriter output).
    pub fn visible_text(&self) -> &str {
        self.typewriter.as_ref().map(|tw| tw.visible_text()).unwrap_or("")
    }

    /// The current node (for speaker/portrait/emotion access).
    pub fn current(&self) -> Option<&DialogueNode> {
        self.current_node.as_deref().and_then(|id| self.tree.get(id))
    }

    /// Advance by dt.  Returns an event if something notable happened.
    pub fn tick(&mut self, dt: f32) -> Option<DialogueEvent> {
        match self.state {
            DialogueState::Typing => {
                let done = self.typewriter.as_mut().map(|tw| tw.tick(dt)).unwrap_or(false);
                if done {
                    let node = self.current_node.as_deref()
                        .and_then(|id| self.tree.get(id))
                        .cloned();
                    if let Some(node) = node {
                        match &node.next {
                            DialogueNext::End => {
                                self.state = DialogueState::Waiting;
                            }
                            DialogueNext::Node(_) => {
                                self.state = DialogueState::Waiting;
                            }
                            DialogueNext::Choice(choices) => {
                                let visible: Vec<Choice> = choices.iter()
                                    .filter(|c| {
                                        c.requires.as_ref()
                                            .map(|f| self.flags.get(f.as_str()).copied().unwrap_or(false))
                                            .unwrap_or(true)
                                    })
                                    .cloned()
                                    .collect();
                                self.choices = visible;
                                self.state   = DialogueState::Choosing;
                                return Some(DialogueEvent::ShowChoices(self.choices.clone()));
                            }
                            DialogueNext::Auto { duration, .. } => {
                                self.auto_timer = Some(*duration);
                                self.state      = DialogueState::AutoTimer;
                            }
                        }
                    }
                    return Some(DialogueEvent::TypewriterDone);
                }
            }
            DialogueState::AutoTimer => {
                if let Some(ref mut timer) = self.auto_timer {
                    *timer -= dt;
                    if *timer <= 0.0 {
                        self.auto_timer = None;
                        return self.advance_auto();
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn advance_auto(&mut self) -> Option<DialogueEvent> {
        let next = self.current_node.as_deref()
            .and_then(|id| self.tree.get(id))
            .map(|n| n.next.clone())?;

        if let DialogueNext::Auto { then, .. } = next {
            match *then {
                DialogueNext::Node(id) => { self.goto(&id); Some(DialogueEvent::NodeChanged(id)) }
                DialogueNext::End => { self.state = DialogueState::Finished; Some(DialogueEvent::Finished) }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Player pressed "advance" (confirm/space).
    pub fn advance(&mut self) -> Option<DialogueEvent> {
        match self.state {
            DialogueState::Typing => {
                // Skip typewriter to end
                if let Some(tw) = &mut self.typewriter { tw.skip(); }
                self.state = DialogueState::Waiting;
                Some(DialogueEvent::TypewriterDone)
            }
            DialogueState::Waiting => {
                let next = self.current_node.as_deref()
                    .and_then(|id| self.tree.get(id))
                    .map(|n| n.next.clone());
                match next {
                    Some(DialogueNext::Node(id)) => {
                        self.goto(&id);
                        Some(DialogueEvent::NodeChanged(id))
                    }
                    Some(DialogueNext::End) | None => {
                        self.state = DialogueState::Finished;
                        Some(DialogueEvent::Finished)
                    }
                    _ => None,
                }
            }
            DialogueState::Choosing => {
                if self.choices.is_empty() {
                    self.state = DialogueState::Finished;
                    return Some(DialogueEvent::Finished);
                }
                let choice = self.choices[self.selected_choice].clone();
                // Apply flags
                for (flag, val) in &choice.sets {
                    self.flags.insert(flag.clone(), *val);
                }
                let next_id = choice.next.clone();
                self.goto(&next_id);
                Some(DialogueEvent::ChoiceMade { index: self.selected_choice, next: next_id })
            }
            _ => None,
        }
    }

    /// Move selection up/down in choice list.
    pub fn select_prev(&mut self) {
        if !self.choices.is_empty() {
            self.selected_choice = (self.selected_choice + self.choices.len() - 1) % self.choices.len();
        }
    }

    pub fn select_next(&mut self) {
        if !self.choices.is_empty() {
            self.selected_choice = (self.selected_choice + 1) % self.choices.len();
        }
    }

    pub fn select(&mut self, idx: usize) {
        self.selected_choice = idx.min(self.choices.len().saturating_sub(1));
    }

    pub fn get_flag(&self, f: &str) -> bool { self.flags.get(f).copied().unwrap_or(false) }
    pub fn set_flag(&mut self, f: impl Into<String>, v: bool) { self.flags.insert(f.into(), v); }
}

/// Events emitted by the dialogue player.
#[derive(Clone, Debug)]
pub enum DialogueEvent {
    TypewriterDone,
    ShowChoices(Vec<Choice>),
    ChoiceMade { index: usize, next: String },
    NodeChanged(String),
    Finished,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree() -> DialogueTree {
        DialogueTree::new("test")
            .add_node(NodeBuilder::new("intro", "Hero", "Hello there.").then("end"))
            .add_node(NodeBuilder::new("end",   "Hero", "Goodbye.").end())
    }

    #[test]
    fn typewriter_advances() {
        let mut tw = TypewriterState::new("Hello", 100.0);
        tw.tick(0.5);
        assert!(tw.chars_shown > 0);
    }

    #[test]
    fn typewriter_skip() {
        let mut tw = TypewriterState::new("Long text here", 10.0);
        tw.skip();
        assert!(tw.complete);
        assert_eq!(tw.visible_text(), "Long text here");
    }

    #[test]
    fn typewriter_progress() {
        let mut tw = TypewriterState::new("ABCDE", 100.0);
        tw.tick(0.02); // 2 chars
        assert!(tw.progress() > 0.0 && tw.progress() < 1.0);
    }

    #[test]
    fn player_starts_and_types() {
        let tree = make_tree();
        let mut player = DialoguePlayer::new(tree);
        player.start();
        assert_eq!(player.state, DialogueState::Typing);
    }

    #[test]
    fn player_skips_typewriter() {
        let tree = make_tree();
        let mut player = DialoguePlayer::new(tree);
        player.start();
        let ev = player.advance();
        assert!(matches!(ev, Some(DialogueEvent::TypewriterDone)));
        assert_eq!(player.state, DialogueState::Waiting);
    }

    #[test]
    fn player_advances_node() {
        let tree = make_tree();
        let mut player = DialoguePlayer::new(tree);
        player.start();
        player.advance(); // skip typewriter
        let ev = player.advance(); // advance to next node
        assert!(matches!(ev, Some(DialogueEvent::NodeChanged(_))));
    }

    #[test]
    fn player_finishes() {
        let tree = make_tree();
        let mut player = DialoguePlayer::new(tree);
        player.start();
        player.advance(); // skip typewriter on intro
        player.advance(); // advance to end node
        player.advance(); // skip typewriter on end
        let ev = player.advance(); // finish
        assert!(matches!(ev, Some(DialogueEvent::Finished)));
        assert!(player.is_finished());
    }

    #[test]
    fn choice_node() {
        let tree = DialogueTree::new("choices")
            .add_node(NodeBuilder::new("q", "NPC", "What do you want?").choices(vec![
                Choice::new("Fight", "fight_node"),
                Choice::new("Talk",  "talk_node"),
            ]))
            .add_node(NodeBuilder::new("fight_node", "NPC", "Let's fight!").end())
            .add_node(NodeBuilder::new("talk_node",  "NPC", "Let's talk!").end());

        let mut player = DialoguePlayer::new(tree);
        player.start();
        player.advance(); // skip typewriter
        assert_eq!(player.state, DialogueState::Choosing);
        assert_eq!(player.choices.len(), 2);

        player.select(1); // pick "Talk"
        let ev = player.advance();
        assert!(matches!(ev, Some(DialogueEvent::ChoiceMade { index: 1, .. })));
    }

    #[test]
    fn emotion_colors_defined() {
        use DialogueEmotion::*;
        for em in [Neutral, Happy, Sad, Angry, Surprised, Scared, Suspicious, Mysterious] {
            let c = em.color();
            assert!(c[3] > 0.0); // must be non-transparent
        }
    }
}
