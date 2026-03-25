//! Sprite Animation System — frame-by-frame ASCII art animation.
//!
//! Provides a `SpriteAnimator` that drives frame-based animations for entities
//! in the Chaos RPG. Each animation is a sequence of `SpriteFrame`s, where each
//! frame is a collection of positioned, colored glyphs forming the ASCII art.
//!
//! # Architecture
//!
//! ```text
//! SpriteAnimator
//!   ├─ animations: HashMap<String, SpriteAnimation>
//!   ├─ current: String (active animation name)
//!   ├─ frame_index: usize
//!   ├─ frame_timer: f32
//!   └─ state_machine: AnimationStateMachine
//!         ├─ states: HashMap<String, AnimState>
//!         ├─ transitions: Vec<AnimTransition>
//!         └─ params: HashMap<String, f32>
//! ```
//!
//! # Pre-built Animations
//!
//! The module includes ready-to-use animations for:
//! - Player: idle, attack, cast, hurt, defend
//! - Enemy: idle, attack, death
//! - Bosses: unique idle and attack per boss type

use glam::{Vec2, Vec4};
use std::collections::HashMap;

// ── FrameGlyph ──────────────────────────────────────────────────────────────

/// A single glyph within a sprite frame, positioned relative to entity center.
#[derive(Debug, Clone)]
pub struct FrameGlyph {
    pub character: char,
    /// Offset from entity center in world units.
    pub offset: Vec2,
    /// RGBA color.
    pub color: Vec4,
    /// Emission intensity (0 = none, 1+ = glows).
    pub emission: f32,
    /// Scale multiplier (1.0 = normal).
    pub scale: f32,
}

impl FrameGlyph {
    pub fn new(ch: char, offset: Vec2, color: Vec4) -> Self {
        Self { character: ch, offset, color, emission: 0.0, scale: 1.0 }
    }

    pub fn colored(ch: char, x: f32, y: f32, r: f32, g: f32, b: f32) -> Self {
        Self::new(ch, Vec2::new(x, y), Vec4::new(r, g, b, 1.0))
    }

    pub fn white(ch: char, x: f32, y: f32) -> Self {
        Self::colored(ch, x, y, 1.0, 1.0, 1.0)
    }

    pub fn with_emission(mut self, e: f32) -> Self { self.emission = e; self }
    pub fn with_scale(mut self, s: f32) -> Self { self.scale = s; self }
}

// ── SpriteFrame ─────────────────────────────────────────────────────────────

/// A single frame of a sprite animation — the complete ASCII art for one pose.
#[derive(Debug, Clone)]
pub struct SpriteFrame {
    /// All glyphs making up this frame.
    pub glyphs: Vec<FrameGlyph>,
    /// Optional per-frame event tag (e.g. "hit", "cast_release").
    pub event: Option<String>,
    /// Optional per-frame duration override (overrides animation default).
    pub duration_override: Option<f32>,
}

impl SpriteFrame {
    pub fn new(glyphs: Vec<FrameGlyph>) -> Self {
        Self { glyphs, event: None, duration_override: None }
    }

    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    pub fn with_duration(mut self, d: f32) -> Self {
        self.duration_override = Some(d);
        self
    }

    /// Build a frame from a grid string. Each non-space char becomes a glyph.
    /// Lines are centered vertically; chars are spaced 1.0 apart horizontally.
    pub fn from_ascii(art: &str, color: Vec4) -> Self {
        let lines: Vec<&str> = art.lines().collect();
        let height = lines.len() as f32;
        let mut glyphs = Vec::new();

        for (row, line) in lines.iter().enumerate() {
            let width = line.len() as f32;
            for (col, ch) in line.chars().enumerate() {
                if ch == ' ' { continue; }
                let x = col as f32 - width * 0.5;
                let y = -(row as f32 - height * 0.5); // y-up
                glyphs.push(FrameGlyph::new(ch, Vec2::new(x, y), color));
            }
        }

        Self::new(glyphs)
    }
}

// ── LoopMode ────────────────────────────────────────────────────────────────

/// How an animation repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    /// Play once and stop on the last frame.
    Once,
    /// Loop from beginning after reaching the end.
    Loop,
    /// Play forward, then backward, then forward, etc.
    PingPong,
    /// Play once and signal completion (for state machine transitions).
    OnceAndDone,
}

// ── SpriteAnimation ─────────────────────────────────────────────────────────

/// A named animation consisting of ordered frames.
#[derive(Debug, Clone)]
pub struct SpriteAnimation {
    pub name: String,
    pub frames: Vec<SpriteFrame>,
    /// Default seconds per frame.
    pub frame_duration: f32,
    pub loop_mode: LoopMode,
    /// Speed multiplier (1.0 = normal, 2.0 = double speed).
    pub speed: f32,
}

impl SpriteAnimation {
    pub fn new(name: impl Into<String>, frames: Vec<SpriteFrame>, frame_duration: f32, loop_mode: LoopMode) -> Self {
        Self {
            name: name.into(),
            frames,
            frame_duration,
            loop_mode,
            speed: 1.0,
        }
    }

    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }

    /// Total duration of one play-through in seconds.
    pub fn total_duration(&self) -> f32 {
        let mut total = 0.0;
        for frame in &self.frames {
            total += frame.duration_override.unwrap_or(self.frame_duration);
        }
        total / self.speed.max(0.01)
    }

    /// Number of frames.
    pub fn frame_count(&self) -> usize { self.frames.len() }

    /// Duration of a specific frame.
    pub fn frame_time(&self, index: usize) -> f32 {
        self.frames.get(index)
            .and_then(|f| f.duration_override)
            .unwrap_or(self.frame_duration) / self.speed.max(0.01)
    }
}

// ── Animation State Machine ─────────────────────────────────────────────────

/// A state in the animation state machine.
#[derive(Debug, Clone)]
pub struct AnimState {
    pub name: String,
    /// Which animation to play in this state.
    pub animation: String,
    /// Speed multiplier for this state.
    pub speed: f32,
}

impl AnimState {
    pub fn new(name: impl Into<String>, animation: impl Into<String>) -> Self {
        Self { name: name.into(), animation: animation.into(), speed: 1.0 }
    }

    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }
}

/// Condition for transitioning between animation states.
#[derive(Debug, Clone)]
pub enum AnimCondition {
    /// Fires when a named parameter exceeds a threshold.
    ParamGt { param: String, value: f32 },
    /// Fires when a named parameter is below a threshold.
    ParamLt { param: String, value: f32 },
    /// Fires when a named trigger is set (consumed on transition).
    Trigger(String),
    /// Fires when the current animation has completed (LoopMode::Once/OnceAndDone).
    AnimationDone,
    /// Always true.
    Always,
}

impl AnimCondition {
    pub fn trigger(name: impl Into<String>) -> Self { Self::Trigger(name.into()) }
    pub fn param_gt(name: impl Into<String>, v: f32) -> Self {
        Self::ParamGt { param: name.into(), value: v }
    }
    pub fn param_lt(name: impl Into<String>, v: f32) -> Self {
        Self::ParamLt { param: name.into(), value: v }
    }
}

/// A transition between animation states.
#[derive(Debug, Clone)]
pub struct AnimTransition {
    pub from: String,
    pub to: String,
    pub condition: AnimCondition,
    /// Crossfade duration in seconds (0 = instant).
    pub blend_time: f32,
}

impl AnimTransition {
    pub fn new(from: impl Into<String>, to: impl Into<String>, condition: AnimCondition) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            condition,
            blend_time: 0.0,
        }
    }

    pub fn with_blend(mut self, t: f32) -> Self { self.blend_time = t; self }
}

/// Simple animation state machine.
pub struct AnimationStateMachine {
    pub states: HashMap<String, AnimState>,
    pub transitions: Vec<AnimTransition>,
    pub params: HashMap<String, f32>,
    pub triggers: HashMap<String, bool>,
    pub current_state: Option<String>,
}

impl AnimationStateMachine {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            transitions: Vec::new(),
            params: HashMap::new(),
            triggers: HashMap::new(),
            current_state: None,
        }
    }

    pub fn add_state(&mut self, state: AnimState) {
        self.states.insert(state.name.clone(), state);
    }

    pub fn add_transition(&mut self, transition: AnimTransition) {
        self.transitions.push(transition);
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        self.params.insert(name.to_owned(), value);
    }

    pub fn set_trigger(&mut self, name: &str) {
        self.triggers.insert(name.to_owned(), true);
    }

    pub fn start(&mut self, state: &str) {
        self.current_state = Some(state.to_owned());
    }

    /// Evaluate transitions. Returns the new animation name if state changed.
    pub fn evaluate(&mut self, animation_done: bool) -> Option<String> {
        let current = self.current_state.as_ref()?;
        let current = current.clone();

        for trans in &self.transitions {
            if trans.from != current && trans.from != "*" { continue; }

            let satisfied = match &trans.condition {
                AnimCondition::ParamGt { param, value } => {
                    self.params.get(param).copied().unwrap_or(0.0) > *value
                }
                AnimCondition::ParamLt { param, value } => {
                    self.params.get(param).copied().unwrap_or(0.0) < *value
                }
                AnimCondition::Trigger(name) => {
                    self.triggers.get(name).copied().unwrap_or(false)
                }
                AnimCondition::AnimationDone => animation_done,
                AnimCondition::Always => true,
            };

            if satisfied {
                // Consume trigger
                if let AnimCondition::Trigger(name) = &trans.condition {
                    self.triggers.insert(name.clone(), false);
                }

                let new_state = trans.to.clone();
                let anim = self.states.get(&new_state)
                    .map(|s| s.animation.clone());
                self.current_state = Some(new_state);
                return anim;
            }
        }

        None
    }

    /// Current animation name from the active state.
    pub fn current_animation(&self) -> Option<&str> {
        let state_name = self.current_state.as_ref()?;
        self.states.get(state_name).map(|s| s.animation.as_str())
    }
}

impl Default for AnimationStateMachine {
    fn default() -> Self { Self::new() }
}

// ── SpriteAnimator ──────────────────────────────────────────────────────────

/// Events emitted by the animator during playback.
#[derive(Debug, Clone)]
pub struct AnimEvent {
    pub animation: String,
    pub frame_index: usize,
    pub tag: String,
}

/// Drives frame-by-frame sprite animations with an optional state machine.
pub struct SpriteAnimator {
    pub animations: HashMap<String, SpriteAnimation>,
    current: String,
    frame_index: usize,
    frame_timer: f32,
    playing: bool,
    /// True if the animation finished this frame (for Once/OnceAndDone).
    finished: bool,
    /// PingPong direction: true = forward, false = backward.
    ping_pong_forward: bool,
    /// Optional state machine for automatic transitions.
    pub state_machine: Option<AnimationStateMachine>,
    /// Events fired since last drain.
    pending_events: Vec<AnimEvent>,
}

impl SpriteAnimator {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            current: String::new(),
            frame_index: 0,
            frame_timer: 0.0,
            playing: false,
            finished: false,
            ping_pong_forward: true,
            state_machine: None,
            pending_events: Vec::new(),
        }
    }

    /// Add an animation to the library.
    pub fn add_animation(&mut self, anim: SpriteAnimation) {
        self.animations.insert(anim.name.clone(), anim);
    }

    /// Play a named animation from the beginning.
    pub fn play(&mut self, name: &str) {
        if self.animations.contains_key(name) {
            self.current = name.to_owned();
            self.frame_index = 0;
            self.frame_timer = 0.0;
            self.playing = true;
            self.finished = false;
            self.ping_pong_forward = true;
        }
    }

    /// Play only if not already playing this animation (avoids restart).
    pub fn play_if_different(&mut self, name: &str) {
        if self.current != name {
            self.play(name);
        }
    }

    /// Stop playback (freeze on current frame).
    pub fn stop(&mut self) { self.playing = false; }

    /// Resume playback.
    pub fn resume(&mut self) { self.playing = true; }

    /// Whether the current animation has finished (Once/OnceAndDone).
    pub fn is_finished(&self) -> bool { self.finished }

    /// Whether the animator is currently playing.
    pub fn is_playing(&self) -> bool { self.playing }

    /// Current animation name.
    pub fn current_animation(&self) -> &str { &self.current }

    /// Current frame index.
    pub fn current_frame_index(&self) -> usize { self.frame_index }

    /// Advance the animation by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.finished = false;

        // State machine evaluation
        if let Some(ref mut sm) = self.state_machine {
            if let Some(new_anim) = sm.evaluate(self.finished) {
                self.play(&new_anim);
            }
        }

        if !self.playing { return; }

        let anim = match self.animations.get(&self.current) {
            Some(a) => a.clone(), // clone to avoid borrow issues
            None => return,
        };

        if anim.frames.is_empty() { return; }

        let frame_dur = anim.frame_time(self.frame_index);
        self.frame_timer += dt;

        while self.frame_timer >= frame_dur && frame_dur > 0.0 {
            self.frame_timer -= frame_dur;

            // Fire frame event
            if let Some(ref event) = anim.frames[self.frame_index].event {
                self.pending_events.push(AnimEvent {
                    animation: self.current.clone(),
                    frame_index: self.frame_index,
                    tag: event.clone(),
                });
            }

            // Advance frame
            match anim.loop_mode {
                LoopMode::Loop => {
                    self.frame_index = (self.frame_index + 1) % anim.frames.len();
                }
                LoopMode::Once => {
                    if self.frame_index + 1 < anim.frames.len() {
                        self.frame_index += 1;
                    } else {
                        self.playing = false;
                        self.finished = true;
                    }
                }
                LoopMode::OnceAndDone => {
                    if self.frame_index + 1 < anim.frames.len() {
                        self.frame_index += 1;
                    } else {
                        self.playing = false;
                        self.finished = true;
                    }
                }
                LoopMode::PingPong => {
                    if self.ping_pong_forward {
                        if self.frame_index + 1 < anim.frames.len() {
                            self.frame_index += 1;
                        } else {
                            self.ping_pong_forward = false;
                            if self.frame_index > 0 {
                                self.frame_index -= 1;
                            }
                        }
                    } else {
                        if self.frame_index > 0 {
                            self.frame_index -= 1;
                        } else {
                            self.ping_pong_forward = true;
                            self.frame_index += 1;
                        }
                    }
                }
            }

            // Re-check frame_dur for new frame
            break;
        }
    }

    /// Get the current frame's glyphs. Returns empty slice if no animation.
    pub fn current_glyphs(&self) -> &[FrameGlyph] {
        self.animations.get(&self.current)
            .and_then(|a| a.frames.get(self.frame_index))
            .map(|f| f.glyphs.as_slice())
            .unwrap_or(&[])
    }

    /// Get the current SpriteFrame if one exists.
    pub fn current_frame(&self) -> Option<&SpriteFrame> {
        self.animations.get(&self.current)
            .and_then(|a| a.frames.get(self.frame_index))
    }

    /// Drain all pending events.
    pub fn drain_events(&mut self) -> Vec<AnimEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

impl Default for SpriteAnimator {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════════════════════════
// Pre-built animations for the Chaos RPG
// ═══════════════════════════════════════════════════════════════════════════

/// Pre-built animation library for the Chaos RPG.
pub struct AnimationLibrary;

impl AnimationLibrary {
    // ── Color constants ─────────────────────────────────────────────────

    fn white() -> Vec4 { Vec4::new(1.0, 1.0, 1.0, 1.0) }
    fn red()   -> Vec4 { Vec4::new(1.0, 0.3, 0.2, 1.0) }
    fn blue()  -> Vec4 { Vec4::new(0.3, 0.5, 1.0, 1.0) }
    fn gold()  -> Vec4 { Vec4::new(1.0, 0.85, 0.3, 1.0) }
    fn green() -> Vec4 { Vec4::new(0.3, 1.0, 0.4, 1.0) }
    fn gray()  -> Vec4 { Vec4::new(0.5, 0.5, 0.5, 1.0) }
    fn dark()  -> Vec4 { Vec4::new(0.2, 0.2, 0.2, 0.5) }
    fn purple() -> Vec4 { Vec4::new(0.7, 0.2, 1.0, 1.0) }
    fn cyan()  -> Vec4 { Vec4::new(0.2, 0.9, 1.0, 1.0) }
    fn orange() -> Vec4 { Vec4::new(1.0, 0.5, 0.1, 1.0) }

    // ── Player Animations ───────────────────────────────────────────────

    /// Player idle: 2-frame breathing cycle.
    pub fn player_idle() -> SpriteAnimation {
        let c = Self::white();
        let frame1 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),       // head
            FrameGlyph::white('|', 0.0, 1.0),        // torso
            FrameGlyph::white('/', -1.0, 1.0),       // left arm
            FrameGlyph::white('\\', 1.0, 1.0),       // right arm
            FrameGlyph::white('|', 0.0, 0.0),        // waist
            FrameGlyph::white('/', -0.5, -1.0),      // left leg
            FrameGlyph::white('\\', 0.5, -1.0),      // right leg
        ]);

        // Slightly expanded (breathing in)
        let frame2 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.1),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::white('/', -1.1, 1.1),
            FrameGlyph::white('\\', 1.1, 1.1),
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        SpriteAnimation::new("player_idle", vec![frame1, frame2], 0.6, LoopMode::PingPong)
    }

    /// Player attack: 4-frame swing animation.
    pub fn player_attack() -> SpriteAnimation {
        // Frame 1: Wind up — arm raised
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::white('/', -1.0, 1.0),
            FrameGlyph::colored('\\', 1.0, 2.0, 1.0, 0.9, 0.3),  // arm raised
            FrameGlyph::colored('/', 1.5, 2.5, 0.8, 0.8, 0.8),   // weapon up
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        // Frame 2: Swing — arm forward, weapon arc
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::white('/', -1.0, 1.0),
            FrameGlyph::colored('-', 1.5, 1.0, 1.0, 0.9, 0.3),   // arm extended
            FrameGlyph::colored('>', 2.5, 1.0, 1.0, 0.7, 0.2).with_emission(0.5), // weapon strike
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]).with_event("hit");

        // Frame 3: Follow through
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::white('/', -1.0, 1.0),
            FrameGlyph::colored('\\', 1.5, 0.0, 1.0, 0.9, 0.3),   // arm down
            FrameGlyph::colored('\\', 2.0, -0.5, 0.8, 0.8, 0.8),  // weapon low
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        // Frame 4: Return to ready
        let f4 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::white('/', -1.0, 1.0),
            FrameGlyph::white('\\', 1.0, 1.0),
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        SpriteAnimation::new("player_attack", vec![f1, f2, f3, f4], 0.08, LoopMode::OnceAndDone)
    }

    /// Player cast: 3-frame spell casting.
    pub fn player_cast() -> SpriteAnimation {
        // Frame 1: Arms raise
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::colored('/', -1.0, 2.0, 0.3, 0.5, 1.0),   // left arm up
            FrameGlyph::colored('\\', 1.0, 2.0, 0.3, 0.5, 1.0),   // right arm up
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        // Frame 2: Glow brightens
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::colored('/', -1.0, 2.0, 0.3, 0.5, 1.0),
            FrameGlyph::colored('\\', 1.0, 2.0, 0.3, 0.5, 1.0),
            FrameGlyph::colored('*', 0.0, 2.5, 0.5, 0.7, 1.0).with_emission(1.0), // glow
            FrameGlyph::colored('·', -0.5, 2.8, 0.4, 0.6, 1.0).with_emission(0.6),
            FrameGlyph::colored('·', 0.5, 2.8, 0.4, 0.6, 1.0).with_emission(0.6),
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        // Frame 3: Release
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::colored('-', -1.5, 1.5, 0.3, 0.5, 1.0),   // arms thrust forward
            FrameGlyph::colored('-', 1.5, 1.5, 0.3, 0.5, 1.0),
            FrameGlyph::colored('★', 0.0, 3.0, 0.5, 0.8, 1.0).with_emission(1.5), // spell release
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]).with_event("cast_release");

        SpriteAnimation::new("player_cast", vec![f1, f2, f3], 0.12, LoopMode::OnceAndDone)
    }

    /// Player hurt: 2-frame recoil.
    pub fn player_hurt() -> SpriteAnimation {
        // Frame 1: Lean back
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('O', -0.3, 2.1, 1.0, 0.5, 0.5),
            FrameGlyph::colored('\\', -0.2, 1.0, 1.0, 0.5, 0.5),
            FrameGlyph::colored('/', -1.3, 0.8, 1.0, 0.5, 0.5),
            FrameGlyph::colored('\\', 0.8, 0.8, 1.0, 0.5, 0.5),
            FrameGlyph::colored('|', -0.1, 0.0, 1.0, 0.5, 0.5),
            FrameGlyph::colored('/', -0.6, -1.0, 1.0, 0.5, 0.5),
            FrameGlyph::colored('\\', 0.4, -1.0, 1.0, 0.5, 0.5),
        ]);

        // Frame 2: Recovery
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::white('/', -1.0, 1.0),
            FrameGlyph::white('\\', 1.0, 1.0),
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        SpriteAnimation::new("player_hurt", vec![f1, f2], 0.15, LoopMode::OnceAndDone)
    }

    /// Player defend: 2-frame block.
    pub fn player_defend() -> SpriteAnimation {
        // Frame 1: Arms cross (blocking)
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::colored('X', 0.0, 1.5, 0.7, 0.9, 1.0),  // crossed arms
            FrameGlyph::colored('[', -0.5, 1.5, 0.5, 0.5, 0.6),  // shield left
            FrameGlyph::colored(']', 0.5, 1.5, 0.5, 0.5, 0.6),   // shield right
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        // Frame 2: Hold (slight shimmer)
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::white('O', 0.0, 2.0),
            FrameGlyph::white('|', 0.0, 1.0),
            FrameGlyph::colored('X', 0.0, 1.5, 0.8, 1.0, 1.0).with_emission(0.3),
            FrameGlyph::colored('[', -0.5, 1.5, 0.6, 0.6, 0.7),
            FrameGlyph::colored(']', 0.5, 1.5, 0.6, 0.6, 0.7),
            FrameGlyph::white('|', 0.0, 0.0),
            FrameGlyph::white('/', -0.5, -1.0),
            FrameGlyph::white('\\', 0.5, -1.0),
        ]);

        SpriteAnimation::new("player_defend", vec![f1, f2], 0.3, LoopMode::Loop)
    }

    // ── Enemy Animations ────────────────────────────────────────────────

    /// Enemy idle: 2-frame shift.
    pub fn enemy_idle() -> SpriteAnimation {
        let c = Self::red();
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 0.0, 2.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('█', 0.0, 1.0, 0.8, 0.2, 0.1),
            FrameGlyph::colored('/', -1.0, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('\\', 1.0, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('▲', -0.5, -0.5, 0.6, 0.15, 0.1),
            FrameGlyph::colored('▲', 0.5, -0.5, 0.6, 0.15, 0.1),
        ]);

        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 0.2, 2.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('█', 0.2, 1.0, 0.8, 0.2, 0.1),
            FrameGlyph::colored('/', -0.8, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('\\', 1.2, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('▲', -0.3, -0.5, 0.6, 0.15, 0.1),
            FrameGlyph::colored('▲', 0.7, -0.5, 0.6, 0.15, 0.1),
        ]);

        SpriteAnimation::new("enemy_idle", vec![f1, f2], 0.5, LoopMode::PingPong)
    }

    /// Enemy attack: 3-frame lunge.
    pub fn enemy_attack() -> SpriteAnimation {
        // Lean forward
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 0.5, 2.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('█', 0.3, 1.0, 0.8, 0.2, 0.1),
            FrameGlyph::colored('/', -0.5, 0.8, 0.8, 0.2, 0.1),
            FrameGlyph::colored('-', 1.5, 1.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('▲', -0.3, -0.5, 0.6, 0.15, 0.1),
            FrameGlyph::colored('▲', 0.5, -0.5, 0.6, 0.15, 0.1),
        ]);

        // Strike
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 1.0, 1.8, 1.0, 0.4, 0.2),
            FrameGlyph::colored('█', 0.5, 1.0, 0.8, 0.2, 0.1),
            FrameGlyph::colored('/', -0.3, 0.8, 0.8, 0.2, 0.1),
            FrameGlyph::colored('>', 2.0, 1.0, 1.0, 0.5, 0.2).with_emission(0.8),
            FrameGlyph::colored('▲', -0.1, -0.5, 0.6, 0.15, 0.1),
            FrameGlyph::colored('▲', 0.7, -0.5, 0.6, 0.15, 0.1),
        ]).with_event("hit");

        // Return
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 0.0, 2.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('█', 0.0, 1.0, 0.8, 0.2, 0.1),
            FrameGlyph::colored('/', -1.0, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('\\', 1.0, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('▲', -0.5, -0.5, 0.6, 0.15, 0.1),
            FrameGlyph::colored('▲', 0.5, -0.5, 0.6, 0.15, 0.1),
        ]);

        SpriteAnimation::new("enemy_attack", vec![f1, f2, f3], 0.1, LoopMode::OnceAndDone)
    }

    /// Enemy death: 4-frame dissolution.
    pub fn enemy_death() -> SpriteAnimation {
        // Intact
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 0.0, 2.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('█', 0.0, 1.0, 0.8, 0.2, 0.1),
            FrameGlyph::colored('/', -1.0, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('\\', 1.0, 0.5, 0.8, 0.2, 0.1),
            FrameGlyph::colored('▲', -0.5, -0.5, 0.6, 0.15, 0.1),
            FrameGlyph::colored('▲', 0.5, -0.5, 0.6, 0.15, 0.1),
        ]);

        // Cracking
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('▼', 0.1, 2.0, 0.8, 0.3, 0.2),
            FrameGlyph::colored('░', 0.0, 1.0, 0.7, 0.2, 0.1),
            FrameGlyph::colored('/', -1.2, 0.3, 0.6, 0.15, 0.1),
            FrameGlyph::colored('\\', 1.2, 0.3, 0.6, 0.15, 0.1),
            FrameGlyph::colored('·', -0.5, -0.5, 0.5, 0.1, 0.1),
            FrameGlyph::colored('·', 0.5, -0.5, 0.5, 0.1, 0.1),
        ]);

        // Scattering
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::colored('·', 0.3, 2.3, 0.5, 0.2, 0.15),
            FrameGlyph::colored('░', -0.2, 1.2, 0.4, 0.15, 0.1),
            FrameGlyph::colored('·', -1.5, 0.1, 0.3, 0.1, 0.05),
            FrameGlyph::colored('·', 1.5, 0.1, 0.3, 0.1, 0.05),
            FrameGlyph::colored('·', 0.0, -0.8, 0.2, 0.05, 0.05),
        ]);

        // Gone
        let f4 = SpriteFrame::new(vec![
            FrameGlyph::colored('·', 0.5, 2.5, 0.2, 0.1, 0.1),
            FrameGlyph::colored('·', -0.5, 0.5, 0.15, 0.05, 0.05),
        ]).with_event("death_complete");

        SpriteAnimation::new("enemy_death", vec![f1, f2, f3, f4], 0.2, LoopMode::OnceAndDone)
    }

    // ── Boss Animations ─────────────────────────────────────────────────

    /// Get idle animation for a boss by name.
    pub fn boss_idle(boss_name: &str) -> SpriteAnimation {
        match boss_name {
            "Mirror" => Self::boss_mirror_idle(),
            "Null" => Self::boss_null_idle(),
            "Committee" => Self::boss_committee_idle(),
            "FibonacciHydra" => Self::boss_hydra_idle(),
            "Eigenstate" => Self::boss_eigenstate_idle(),
            "Ouroboros" => Self::boss_ouroboros_idle(),
            "AlgorithmReborn" => Self::boss_algorithm_idle(),
            "ChaosWeaver" => Self::boss_chaos_weaver_idle(),
            "VoidSerpent" => Self::boss_void_serpent_idle(),
            "PrimeFactorial" => Self::boss_prime_idle(),
            _ => Self::enemy_idle(),
        }
    }

    /// Get attack animation for a boss by name.
    pub fn boss_attack(boss_name: &str) -> SpriteAnimation {
        match boss_name {
            "Mirror" => Self::boss_mirror_attack(),
            "Null" => Self::boss_null_attack(),
            "Committee" => Self::boss_committee_attack(),
            "FibonacciHydra" => Self::boss_hydra_attack(),
            "Eigenstate" => Self::boss_eigenstate_attack(),
            "Ouroboros" => Self::boss_ouroboros_attack(),
            "AlgorithmReborn" => Self::boss_algorithm_attack(),
            "ChaosWeaver" => Self::boss_chaos_weaver_attack(),
            "VoidSerpent" => Self::boss_void_serpent_attack(),
            "PrimeFactorial" => Self::boss_prime_attack(),
            _ => Self::enemy_attack(),
        }
    }

    // ── Individual boss idle animations ─────────────────────────────────

    fn boss_mirror_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◇', 0.0, 3.0, 0.8, 0.9, 1.0).with_emission(0.5),
            FrameGlyph::colored('│', 0.0, 2.0, 0.7, 0.8, 0.9),
            FrameGlyph::colored('◇', -1.0, 1.0, 0.6, 0.7, 0.8),
            FrameGlyph::colored('◇', 1.0, 1.0, 0.6, 0.7, 0.8),
            FrameGlyph::colored('│', 0.0, 0.0, 0.7, 0.8, 0.9),
            FrameGlyph::colored('△', -0.5, -1.0, 0.5, 0.6, 0.7),
            FrameGlyph::colored('△', 0.5, -1.0, 0.5, 0.6, 0.7),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 3.0, 0.9, 1.0, 1.0).with_emission(0.8),
            FrameGlyph::colored('│', 0.0, 2.0, 0.8, 0.9, 1.0),
            FrameGlyph::colored('◆', -1.0, 1.0, 0.7, 0.8, 0.9),
            FrameGlyph::colored('◆', 1.0, 1.0, 0.7, 0.8, 0.9),
            FrameGlyph::colored('│', 0.0, 0.0, 0.8, 0.9, 1.0),
            FrameGlyph::colored('△', -0.5, -1.0, 0.6, 0.7, 0.8),
            FrameGlyph::colored('△', 0.5, -1.0, 0.6, 0.7, 0.8),
        ]);
        SpriteAnimation::new("boss_mirror_idle", vec![f1, f2], 0.7, LoopMode::PingPong)
    }

    fn boss_null_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('∅', 0.0, 3.0, 0.3, 0.3, 0.3).with_emission(0.3),
            FrameGlyph::colored('█', 0.0, 2.0, 0.1, 0.1, 0.1),
            FrameGlyph::colored('░', -1.0, 1.0, 0.2, 0.2, 0.2),
            FrameGlyph::colored('░', 1.0, 1.0, 0.2, 0.2, 0.2),
            FrameGlyph::colored('▓', 0.0, 0.0, 0.15, 0.15, 0.15),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('∅', 0.0, 3.0, 0.2, 0.2, 0.2),
            FrameGlyph::colored('░', 0.0, 2.0, 0.08, 0.08, 0.08),
            FrameGlyph::colored(' ', -1.0, 1.0, 0.0, 0.0, 0.0),
            FrameGlyph::colored('░', 1.0, 1.0, 0.15, 0.15, 0.15),
            FrameGlyph::colored('▒', 0.0, 0.0, 0.1, 0.1, 0.1),
        ]);
        SpriteAnimation::new("boss_null_idle", vec![f1, f2], 0.8, LoopMode::PingPong)
    }

    fn boss_committee_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            // Five heads in a row
            FrameGlyph::colored('☻', -2.0, 2.0, 1.0, 0.8, 0.3),
            FrameGlyph::colored('☻', -1.0, 2.0, 0.3, 1.0, 0.4),
            FrameGlyph::colored('☻', 0.0, 2.5, 1.0, 0.3, 0.3),  // center judge raised
            FrameGlyph::colored('☻', 1.0, 2.0, 0.3, 0.5, 1.0),
            FrameGlyph::colored('☻', 2.0, 2.0, 0.8, 0.3, 1.0),
            FrameGlyph::colored('═', -2.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', -1.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', 0.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', 1.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', 2.0, 1.0, 0.5, 0.4, 0.2),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('☻', -2.0, 2.1, 1.0, 0.8, 0.3),
            FrameGlyph::colored('☻', -1.0, 1.9, 0.3, 1.0, 0.4),
            FrameGlyph::colored('☻', 0.0, 2.5, 1.0, 0.3, 0.3),
            FrameGlyph::colored('☻', 1.0, 2.1, 0.3, 0.5, 1.0),
            FrameGlyph::colored('☻', 2.0, 1.9, 0.8, 0.3, 1.0),
            FrameGlyph::colored('═', -2.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', -1.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', 0.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', 1.0, 1.0, 0.5, 0.4, 0.2),
            FrameGlyph::colored('═', 2.0, 1.0, 0.5, 0.4, 0.2),
        ]);
        SpriteAnimation::new("boss_committee_idle", vec![f1, f2], 0.6, LoopMode::PingPong)
    }

    fn boss_hydra_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', -1.0, 3.0, 0.2, 0.8, 0.3),
            FrameGlyph::colored('◆', 1.0, 3.0, 0.2, 0.8, 0.3),
            FrameGlyph::colored('\\', -0.5, 2.0, 0.15, 0.6, 0.2),
            FrameGlyph::colored('/', 0.5, 2.0, 0.15, 0.6, 0.2),
            FrameGlyph::colored('█', 0.0, 1.0, 0.1, 0.5, 0.15),
            FrameGlyph::colored('▲', 0.0, -0.5, 0.08, 0.4, 0.1),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', -1.2, 3.2, 0.2, 0.8, 0.3),
            FrameGlyph::colored('◆', 1.2, 2.8, 0.2, 0.8, 0.3),
            FrameGlyph::colored('\\', -0.6, 2.1, 0.15, 0.6, 0.2),
            FrameGlyph::colored('/', 0.6, 1.9, 0.15, 0.6, 0.2),
            FrameGlyph::colored('█', 0.0, 1.0, 0.1, 0.5, 0.15),
            FrameGlyph::colored('▲', 0.0, -0.5, 0.08, 0.4, 0.1),
        ]);
        SpriteAnimation::new("boss_hydra_idle", vec![f1, f2], 0.5, LoopMode::PingPong)
    }

    fn boss_eigenstate_idle() -> SpriteAnimation {
        // Quantum superposition: alternates between two different forms
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('ψ', 0.0, 3.0, 0.5, 0.2, 1.0).with_emission(0.6),
            FrameGlyph::colored('|', 0.0, 2.0, 0.4, 0.15, 0.8),
            FrameGlyph::colored('◇', -1.0, 1.5, 0.3, 0.1, 0.7),
            FrameGlyph::colored('◇', 1.0, 1.5, 0.3, 0.1, 0.7),
            FrameGlyph::colored('▽', 0.0, 0.0, 0.2, 0.1, 0.6),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('φ', 0.0, 3.0, 1.0, 0.2, 0.5).with_emission(0.6),
            FrameGlyph::colored('│', 0.0, 2.0, 0.8, 0.15, 0.4),
            FrameGlyph::colored('◆', -1.0, 1.5, 0.7, 0.1, 0.3),
            FrameGlyph::colored('◆', 1.0, 1.5, 0.7, 0.1, 0.3),
            FrameGlyph::colored('△', 0.0, 0.0, 0.6, 0.1, 0.2),
        ]);
        SpriteAnimation::new("boss_eigenstate_idle", vec![f1, f2], 0.3, LoopMode::PingPong)
    }

    fn boss_ouroboros_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 2.0, 0.2, 0.8, 0.5).with_emission(0.4),
            FrameGlyph::colored('~', 1.0, 1.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', 1.5, 0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', 1.0, -0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', 0.0, -1.0, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', -1.0, -0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', -1.5, 0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', -1.0, 1.5, 0.15, 0.6, 0.4),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 1.0, 1.5, 0.2, 0.8, 0.5).with_emission(0.4),
            FrameGlyph::colored('~', 1.5, 0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', 1.0, -0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', 0.0, -1.0, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', -1.0, -0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', -1.5, 0.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', -1.0, 1.5, 0.15, 0.6, 0.4),
            FrameGlyph::colored('~', 0.0, 2.0, 0.15, 0.6, 0.4),
        ]);
        SpriteAnimation::new("boss_ouroboros_idle", vec![f1, f2], 0.4, LoopMode::Loop)
    }

    fn boss_algorithm_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('Σ', 0.0, 3.0, 0.2, 1.0, 0.8).with_emission(0.7),
            FrameGlyph::colored('█', 0.0, 2.0, 0.1, 0.6, 0.5),
            FrameGlyph::colored('0', -1.5, 1.0, 0.0, 0.4, 0.3),
            FrameGlyph::colored('1', 1.5, 1.0, 0.0, 0.4, 0.3),
            FrameGlyph::colored('λ', -0.5, 0.0, 0.0, 0.3, 0.25),
            FrameGlyph::colored('λ', 0.5, 0.0, 0.0, 0.3, 0.25),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('Σ', 0.0, 3.0, 0.3, 1.0, 0.9).with_emission(0.9),
            FrameGlyph::colored('█', 0.0, 2.0, 0.15, 0.7, 0.6),
            FrameGlyph::colored('1', -1.5, 1.0, 0.0, 0.5, 0.4),
            FrameGlyph::colored('0', 1.5, 1.0, 0.0, 0.5, 0.4),
            FrameGlyph::colored('λ', -0.5, 0.0, 0.0, 0.35, 0.3),
            FrameGlyph::colored('λ', 0.5, 0.0, 0.0, 0.35, 0.3),
        ]);
        SpriteAnimation::new("boss_algorithm_idle", vec![f1, f2], 0.5, LoopMode::PingPong)
    }

    fn boss_chaos_weaver_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('∞', 0.0, 3.0, 1.0, 0.2, 0.8).with_emission(0.8),
            FrameGlyph::colored('▓', 0.0, 2.0, 0.8, 0.1, 0.6),
            FrameGlyph::colored('~', -1.5, 1.5, 0.6, 0.1, 0.5),
            FrameGlyph::colored('~', 1.5, 1.5, 0.6, 0.1, 0.5),
            FrameGlyph::colored('▲', -0.5, 0.0, 0.5, 0.05, 0.4),
            FrameGlyph::colored('▲', 0.5, 0.0, 0.5, 0.05, 0.4),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('∞', 0.0, 3.0, 0.8, 0.3, 1.0).with_emission(1.0),
            FrameGlyph::colored('▓', 0.0, 2.0, 0.6, 0.2, 0.8),
            FrameGlyph::colored('~', -1.8, 1.2, 0.5, 0.15, 0.6),
            FrameGlyph::colored('~', 1.8, 1.8, 0.5, 0.15, 0.6),
            FrameGlyph::colored('▲', -0.5, 0.0, 0.4, 0.1, 0.5),
            FrameGlyph::colored('▲', 0.5, 0.0, 0.4, 0.1, 0.5),
        ]);
        SpriteAnimation::new("boss_chaos_weaver_idle", vec![f1, f2], 0.35, LoopMode::PingPong)
    }

    fn boss_void_serpent_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 3.0, 0.1, 0.0, 0.3).with_emission(0.3),
            FrameGlyph::colored('S', 0.5, 2.0, 0.08, 0.0, 0.25),
            FrameGlyph::colored('S', -0.5, 1.0, 0.08, 0.0, 0.25),
            FrameGlyph::colored('S', 0.5, 0.0, 0.08, 0.0, 0.25),
            FrameGlyph::colored('▲', 0.0, -1.0, 0.06, 0.0, 0.2),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.3, 3.0, 0.15, 0.0, 0.4).with_emission(0.4),
            FrameGlyph::colored('S', -0.3, 2.0, 0.1, 0.0, 0.3),
            FrameGlyph::colored('S', 0.3, 1.0, 0.1, 0.0, 0.3),
            FrameGlyph::colored('S', -0.3, 0.0, 0.1, 0.0, 0.3),
            FrameGlyph::colored('▲', 0.0, -1.0, 0.08, 0.0, 0.25),
        ]);
        SpriteAnimation::new("boss_void_serpent_idle", vec![f1, f2], 0.45, LoopMode::PingPong)
    }

    fn boss_prime_idle() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('π', 0.0, 3.0, 1.0, 0.85, 0.3).with_emission(0.5),
            FrameGlyph::colored('█', 0.0, 2.0, 0.8, 0.7, 0.2),
            FrameGlyph::colored('2', -1.5, 1.0, 0.7, 0.6, 0.15),
            FrameGlyph::colored('3', 1.5, 1.0, 0.7, 0.6, 0.15),
            FrameGlyph::colored('▲', -0.5, 0.0, 0.6, 0.5, 0.1),
            FrameGlyph::colored('▲', 0.5, 0.0, 0.6, 0.5, 0.1),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('π', 0.0, 3.0, 1.0, 0.9, 0.4).with_emission(0.7),
            FrameGlyph::colored('█', 0.0, 2.0, 0.85, 0.75, 0.25),
            FrameGlyph::colored('5', -1.5, 1.0, 0.75, 0.65, 0.2),
            FrameGlyph::colored('7', 1.5, 1.0, 0.75, 0.65, 0.2),
            FrameGlyph::colored('▲', -0.5, 0.0, 0.65, 0.55, 0.15),
            FrameGlyph::colored('▲', 0.5, 0.0, 0.65, 0.55, 0.15),
        ]);
        SpriteAnimation::new("boss_prime_idle", vec![f1, f2], 0.6, LoopMode::PingPong)
    }

    // ── Individual boss attack animations ───────────────────────────────

    fn boss_mirror_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◇', 0.0, 3.0, 1.0, 1.0, 1.0).with_emission(1.0),
            FrameGlyph::colored('│', 0.0, 2.0, 0.9, 0.9, 1.0),
            FrameGlyph::colored('>', 2.0, 2.0, 1.0, 1.0, 1.0).with_emission(0.8),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 3.0, 1.0, 1.0, 1.0).with_emission(1.5),
            FrameGlyph::colored('─', 1.0, 2.0, 1.0, 1.0, 1.0),
            FrameGlyph::colored('─', 2.0, 2.0, 1.0, 1.0, 1.0),
            FrameGlyph::colored('★', 3.0, 2.0, 1.0, 1.0, 1.0).with_emission(1.2),
        ]).with_event("hit");
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::colored('◇', 0.0, 3.0, 0.8, 0.9, 1.0).with_emission(0.5),
            FrameGlyph::colored('│', 0.0, 2.0, 0.7, 0.8, 0.9),
            FrameGlyph::colored('◇', -1.0, 1.0, 0.6, 0.7, 0.8),
            FrameGlyph::colored('◇', 1.0, 1.0, 0.6, 0.7, 0.8),
        ]);
        SpriteAnimation::new("boss_mirror_attack", vec![f1, f2, f3], 0.1, LoopMode::OnceAndDone)
    }

    fn boss_null_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('∅', 0.0, 3.0, 0.5, 0.5, 0.5).with_emission(0.8),
            FrameGlyph::colored('█', 0.0, 2.0, 0.2, 0.2, 0.2),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('∅', 0.0, 3.0, 0.1, 0.1, 0.1).with_emission(1.5),
            FrameGlyph::colored(' ', 0.0, 2.0, 0.0, 0.0, 0.0),
            FrameGlyph::colored(' ', 1.0, 2.0, 0.0, 0.0, 0.0),
            FrameGlyph::colored(' ', 2.0, 2.0, 0.0, 0.0, 0.0),
        ]).with_event("erase");
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::colored('∅', 0.0, 3.0, 0.3, 0.3, 0.3).with_emission(0.3),
            FrameGlyph::colored('█', 0.0, 2.0, 0.1, 0.1, 0.1),
            FrameGlyph::colored('░', -1.0, 1.0, 0.2, 0.2, 0.2),
            FrameGlyph::colored('░', 1.0, 1.0, 0.2, 0.2, 0.2),
        ]);
        SpriteAnimation::new("boss_null_attack", vec![f1, f2, f3], 0.12, LoopMode::OnceAndDone)
    }

    fn boss_committee_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('☻', -2.0, 2.0, 1.0, 0.0, 0.0), // voting red
            FrameGlyph::colored('☻', -1.0, 2.0, 1.0, 0.0, 0.0),
            FrameGlyph::colored('☻', 0.0, 2.5, 1.0, 0.0, 0.0).with_emission(0.5),
            FrameGlyph::colored('☻', 1.0, 2.0, 0.0, 1.0, 0.0), // dissent
            FrameGlyph::colored('☻', 2.0, 2.0, 1.0, 0.0, 0.0),
            FrameGlyph::colored('═', -2.0, 1.0, 0.8, 0.2, 0.2),
            FrameGlyph::colored('═', 0.0, 1.0, 0.8, 0.2, 0.2),
            FrameGlyph::colored('═', 2.0, 1.0, 0.8, 0.2, 0.2),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('!', -2.0, 3.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('!', -1.0, 3.0, 1.0, 0.3, 0.2),
            FrameGlyph::colored('!', 0.0, 3.5, 1.0, 0.5, 0.3).with_emission(1.0),
            FrameGlyph::colored('?', 1.0, 3.0, 0.3, 1.0, 0.3),
            FrameGlyph::colored('!', 2.0, 3.0, 1.0, 0.3, 0.2),
        ]).with_event("verdict");
        SpriteAnimation::new("boss_committee_attack", vec![f1, f2], 0.15, LoopMode::OnceAndDone)
    }

    fn boss_hydra_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', -1.5, 3.5, 0.2, 0.9, 0.3),
            FrameGlyph::colored('◆', 1.5, 3.5, 0.2, 0.9, 0.3),
            FrameGlyph::colored('>', -0.5, 3.0, 0.3, 1.0, 0.4).with_emission(0.5),
            FrameGlyph::colored('>', 0.5, 3.0, 0.3, 1.0, 0.4).with_emission(0.5),
            FrameGlyph::colored('█', 0.0, 1.0, 0.1, 0.5, 0.15),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('>', -0.5, 4.0, 0.4, 1.0, 0.5).with_emission(1.0),
            FrameGlyph::colored('>', 0.5, 4.0, 0.4, 1.0, 0.5).with_emission(1.0),
            FrameGlyph::colored('*', 0.0, 4.5, 0.5, 1.0, 0.6).with_emission(1.2),
            FrameGlyph::colored('█', 0.0, 1.0, 0.1, 0.5, 0.15),
        ]).with_event("bite");
        SpriteAnimation::new("boss_hydra_attack", vec![f1, f2], 0.12, LoopMode::OnceAndDone)
    }

    fn boss_eigenstate_attack() -> SpriteAnimation {
        // Collapses into one form then strikes
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('ψ', 0.0, 3.0, 1.0, 0.5, 1.0).with_emission(1.2),
            FrameGlyph::colored('φ', 0.2, 3.0, 0.5, 0.2, 1.0).with_emission(0.8),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('Ψ', 0.0, 3.0, 1.0, 0.2, 1.0).with_emission(2.0),
            FrameGlyph::colored('─', 1.0, 3.0, 0.8, 0.1, 0.8),
            FrameGlyph::colored('─', 2.0, 3.0, 0.6, 0.1, 0.6),
            FrameGlyph::colored('★', 3.0, 3.0, 1.0, 0.3, 1.0).with_emission(1.5),
        ]).with_event("collapse");
        SpriteAnimation::new("boss_eigenstate_attack", vec![f1, f2], 0.12, LoopMode::OnceAndDone)
    }

    fn boss_ouroboros_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 2.0, 0.3, 1.0, 0.6).with_emission(0.8),
            FrameGlyph::colored('O', 0.0, 0.5, 0.2, 0.8, 0.5).with_emission(1.0),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 2.0, 0.5, 1.0, 0.8).with_emission(1.5),
            FrameGlyph::colored('∞', 0.0, 0.5, 0.4, 1.0, 0.7).with_emission(1.5),
            FrameGlyph::colored('~', 2.0, 0.5, 0.3, 0.8, 0.5),
            FrameGlyph::colored('~', -2.0, 0.5, 0.3, 0.8, 0.5),
        ]).with_event("reverse");
        SpriteAnimation::new("boss_ouroboros_attack", vec![f1, f2], 0.15, LoopMode::OnceAndDone)
    }

    fn boss_algorithm_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('Σ', 0.0, 3.0, 0.4, 1.0, 0.9).with_emission(1.0),
            FrameGlyph::colored('█', 0.0, 2.0, 0.2, 0.7, 0.6),
            FrameGlyph::colored('>', 1.0, 2.0, 0.3, 0.9, 0.8),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('Σ', 0.0, 3.0, 0.5, 1.0, 1.0).with_emission(1.5),
            FrameGlyph::colored('>', 1.5, 2.5, 0.4, 1.0, 0.9).with_emission(0.8),
            FrameGlyph::colored('>', 2.5, 2.0, 0.4, 1.0, 0.9).with_emission(0.8),
            FrameGlyph::colored('>', 3.5, 1.5, 0.4, 1.0, 0.9).with_emission(0.8),
        ]).with_event("predict");
        let f3 = SpriteFrame::new(vec![
            FrameGlyph::colored('Σ', 0.0, 3.0, 0.3, 0.8, 0.7).with_emission(0.5),
            FrameGlyph::colored('█', 0.0, 2.0, 0.15, 0.6, 0.5),
        ]);
        SpriteAnimation::new("boss_algorithm_attack", vec![f1, f2, f3], 0.1, LoopMode::OnceAndDone)
    }

    fn boss_chaos_weaver_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('∞', 0.0, 3.0, 1.0, 0.3, 1.0).with_emission(1.5),
            FrameGlyph::colored('~', -1.0, 2.0, 0.8, 0.2, 0.8),
            FrameGlyph::colored('~', 1.0, 2.0, 0.8, 0.2, 0.8),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('∞', 0.0, 3.0, 1.0, 0.5, 1.0).with_emission(2.0),
            FrameGlyph::colored('★', -2.0, 1.0, 1.0, 0.2, 0.8).with_emission(1.0),
            FrameGlyph::colored('★', 2.0, 1.0, 1.0, 0.2, 0.8).with_emission(1.0),
            FrameGlyph::colored('★', 0.0, -1.0, 1.0, 0.2, 0.8).with_emission(1.0),
        ]).with_event("warp");
        SpriteAnimation::new("boss_chaos_weaver_attack", vec![f1, f2], 0.12, LoopMode::OnceAndDone)
    }

    fn boss_void_serpent_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 3.0, 0.2, 0.0, 0.5).with_emission(0.5),
            FrameGlyph::colored('O', 0.0, 4.0, 0.1, 0.0, 0.4).with_emission(0.8), // mouth opens
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('◆', 0.0, 3.0, 0.3, 0.0, 0.6).with_emission(1.0),
            FrameGlyph::colored('O', 0.0, 4.5, 0.0, 0.0, 0.0).with_emission(2.0), // void consume
            FrameGlyph::colored('·', 1.0, 4.0, 0.1, 0.0, 0.3),
            FrameGlyph::colored('·', -1.0, 4.0, 0.1, 0.0, 0.3),
        ]).with_event("consume");
        SpriteAnimation::new("boss_void_serpent_attack", vec![f1, f2], 0.15, LoopMode::OnceAndDone)
    }

    fn boss_prime_attack() -> SpriteAnimation {
        let f1 = SpriteFrame::new(vec![
            FrameGlyph::colored('π', 0.0, 3.0, 1.0, 0.9, 0.5).with_emission(1.0),
            FrameGlyph::colored('!', 0.0, 4.0, 1.0, 0.85, 0.3),
        ]);
        let f2 = SpriteFrame::new(vec![
            FrameGlyph::colored('π', 0.0, 3.0, 1.0, 1.0, 0.6).with_emission(1.5),
            FrameGlyph::colored('1', -1.0, 4.0, 1.0, 0.9, 0.4).with_emission(0.5),
            FrameGlyph::colored('3', 0.0, 4.5, 1.0, 0.9, 0.4).with_emission(0.5),
            FrameGlyph::colored('7', 1.0, 4.0, 1.0, 0.9, 0.4).with_emission(0.5),
        ]).with_event("calculate");
        SpriteAnimation::new("boss_prime_attack", vec![f1, f2], 0.12, LoopMode::OnceAndDone)
    }

    // ── Full animation set builder ──────────────────────────────────────

    /// Build a complete SpriteAnimator with all player animations and state machine.
    pub fn player_animator() -> SpriteAnimator {
        let mut animator = SpriteAnimator::new();
        animator.add_animation(Self::player_idle());
        animator.add_animation(Self::player_attack());
        animator.add_animation(Self::player_cast());
        animator.add_animation(Self::player_hurt());
        animator.add_animation(Self::player_defend());

        // State machine
        let mut sm = AnimationStateMachine::new();
        sm.add_state(AnimState::new("idle", "player_idle"));
        sm.add_state(AnimState::new("attack", "player_attack"));
        sm.add_state(AnimState::new("cast", "player_cast"));
        sm.add_state(AnimState::new("hurt", "player_hurt"));
        sm.add_state(AnimState::new("defend", "player_defend"));

        sm.add_transition(AnimTransition::new("idle", "attack", AnimCondition::trigger("attack")));
        sm.add_transition(AnimTransition::new("idle", "cast", AnimCondition::trigger("cast")));
        sm.add_transition(AnimTransition::new("idle", "defend", AnimCondition::trigger("defend")));
        sm.add_transition(AnimTransition::new("*", "hurt", AnimCondition::trigger("hurt")));
        sm.add_transition(AnimTransition::new("attack", "idle", AnimCondition::AnimationDone));
        sm.add_transition(AnimTransition::new("cast", "idle", AnimCondition::AnimationDone));
        sm.add_transition(AnimTransition::new("hurt", "idle", AnimCondition::AnimationDone));
        sm.add_transition(AnimTransition::new("defend", "idle", AnimCondition::trigger("release_defend")));

        sm.start("idle");
        animator.state_machine = Some(sm);
        animator.play("player_idle");

        animator
    }

    /// Build a standard enemy animator.
    pub fn enemy_animator() -> SpriteAnimator {
        let mut animator = SpriteAnimator::new();
        animator.add_animation(Self::enemy_idle());
        animator.add_animation(Self::enemy_attack());
        animator.add_animation(Self::enemy_death());

        let mut sm = AnimationStateMachine::new();
        sm.add_state(AnimState::new("idle", "enemy_idle"));
        sm.add_state(AnimState::new("attack", "enemy_attack"));
        sm.add_state(AnimState::new("death", "enemy_death"));

        sm.add_transition(AnimTransition::new("idle", "attack", AnimCondition::trigger("attack")));
        sm.add_transition(AnimTransition::new("attack", "idle", AnimCondition::AnimationDone));
        sm.add_transition(AnimTransition::new("*", "death", AnimCondition::trigger("death")));

        sm.start("idle");
        animator.state_machine = Some(sm);
        animator.play("enemy_idle");

        animator
    }

    /// Build a boss animator for a specific boss type.
    pub fn boss_animator(boss_name: &str) -> SpriteAnimator {
        let mut animator = SpriteAnimator::new();
        let idle = Self::boss_idle(boss_name);
        let attack = Self::boss_attack(boss_name);
        let idle_name = idle.name.clone();
        let attack_name = attack.name.clone();

        animator.add_animation(idle);
        animator.add_animation(attack);
        animator.add_animation(Self::enemy_death()); // shared death animation

        let mut sm = AnimationStateMachine::new();
        sm.add_state(AnimState::new("idle", &idle_name));
        sm.add_state(AnimState::new("attack", &attack_name));
        sm.add_state(AnimState::new("death", "enemy_death"));

        sm.add_transition(AnimTransition::new("idle", "attack", AnimCondition::trigger("attack")));
        sm.add_transition(AnimTransition::new("attack", "idle", AnimCondition::AnimationDone));
        sm.add_transition(AnimTransition::new("*", "death", AnimCondition::trigger("death")));

        sm.start("idle");
        animator.state_machine = Some(sm);
        animator.play(&idle_name);

        animator
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_from_ascii() {
        let frame = SpriteFrame::from_ascii("AB\nCD", Vec4::ONE);
        assert_eq!(frame.glyphs.len(), 4);
    }

    #[test]
    fn loop_mode_cycles() {
        let mut animator = SpriteAnimator::new();
        animator.add_animation(SpriteAnimation::new(
            "test",
            vec![
                SpriteFrame::new(vec![FrameGlyph::white('A', 0.0, 0.0)]),
                SpriteFrame::new(vec![FrameGlyph::white('B', 0.0, 0.0)]),
                SpriteFrame::new(vec![FrameGlyph::white('C', 0.0, 0.0)]),
            ],
            0.1,
            LoopMode::Loop,
        ));
        animator.play("test");

        // After 3 frames we should loop back
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 1);
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 2);
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 0); // looped
    }

    #[test]
    fn once_mode_stops() {
        let mut animator = SpriteAnimator::new();
        animator.add_animation(SpriteAnimation::new(
            "test",
            vec![
                SpriteFrame::new(vec![FrameGlyph::white('A', 0.0, 0.0)]),
                SpriteFrame::new(vec![FrameGlyph::white('B', 0.0, 0.0)]),
            ],
            0.1,
            LoopMode::Once,
        ));
        animator.play("test");
        animator.tick(0.1); // frame 1
        animator.tick(0.1); // should stop
        assert!(animator.is_finished());
        assert!(!animator.is_playing());
    }

    #[test]
    fn frame_events_fire() {
        let mut animator = SpriteAnimator::new();
        animator.add_animation(SpriteAnimation::new(
            "test",
            vec![
                SpriteFrame::new(vec![FrameGlyph::white('A', 0.0, 0.0)]).with_event("start"),
                SpriteFrame::new(vec![FrameGlyph::white('B', 0.0, 0.0)]).with_event("hit"),
            ],
            0.1,
            LoopMode::Once,
        ));
        animator.play("test");
        animator.tick(0.1); // advances past frame 0, fires "start"
        let events = animator.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tag, "start");
    }

    #[test]
    fn player_animator_builds() {
        let animator = AnimationLibrary::player_animator();
        assert!(animator.animations.contains_key("player_idle"));
        assert!(animator.animations.contains_key("player_attack"));
        assert!(animator.animations.contains_key("player_cast"));
        assert!(animator.animations.contains_key("player_hurt"));
        assert!(animator.animations.contains_key("player_defend"));
        assert!(animator.is_playing());
    }

    #[test]
    fn enemy_animator_builds() {
        let animator = AnimationLibrary::enemy_animator();
        assert!(animator.animations.contains_key("enemy_idle"));
        assert!(animator.animations.contains_key("enemy_attack"));
        assert!(animator.animations.contains_key("enemy_death"));
    }

    #[test]
    fn all_boss_animators_build() {
        let bosses = [
            "Mirror", "Null", "Committee", "FibonacciHydra", "Eigenstate",
            "Ouroboros", "AlgorithmReborn", "ChaosWeaver", "VoidSerpent", "PrimeFactorial",
        ];
        for boss in &bosses {
            let animator = AnimationLibrary::boss_animator(boss);
            assert!(animator.animations.len() >= 3, "Boss {} has <3 animations", boss);
        }
    }

    #[test]
    fn state_machine_transitions() {
        let mut sm = AnimationStateMachine::new();
        sm.add_state(AnimState::new("idle", "idle_anim"));
        sm.add_state(AnimState::new("attack", "attack_anim"));
        sm.add_transition(AnimTransition::new("idle", "attack", AnimCondition::trigger("attack")));
        sm.add_transition(AnimTransition::new("attack", "idle", AnimCondition::AnimationDone));
        sm.start("idle");

        // No trigger → no transition
        assert!(sm.evaluate(false).is_none());

        // Set trigger → transition fires
        sm.set_trigger("attack");
        let result = sm.evaluate(false);
        assert_eq!(result, Some("attack_anim".to_string()));
        assert_eq!(sm.current_state.as_deref(), Some("attack"));

        // Trigger consumed
        assert!(!sm.triggers.get("attack").copied().unwrap_or(false));

        // AnimationDone → back to idle
        let result = sm.evaluate(true);
        assert_eq!(result, Some("idle_anim".to_string()));
    }

    #[test]
    fn animation_duration() {
        let anim = SpriteAnimation::new(
            "test",
            vec![
                SpriteFrame::new(vec![]).with_duration(0.2),
                SpriteFrame::new(vec![]).with_duration(0.3),
                SpriteFrame::new(vec![]),
            ],
            0.1,
            LoopMode::Loop,
        );
        // 0.2 + 0.3 + 0.1 (default) = 0.6
        assert!((anim.total_duration() - 0.6).abs() < 1e-6);
    }

    #[test]
    fn pingpong_mode() {
        let mut animator = SpriteAnimator::new();
        animator.add_animation(SpriteAnimation::new(
            "pp",
            vec![
                SpriteFrame::new(vec![FrameGlyph::white('A', 0.0, 0.0)]),
                SpriteFrame::new(vec![FrameGlyph::white('B', 0.0, 0.0)]),
                SpriteFrame::new(vec![FrameGlyph::white('C', 0.0, 0.0)]),
            ],
            0.1,
            LoopMode::PingPong,
        ));
        animator.play("pp");

        // Forward: 0 → 1 → 2
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 1);
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 2);
        // Backward: 2 → 1
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 1);
        // Backward: 1 → 0
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 0);
        // Forward again: 0 → 1
        animator.tick(0.1);
        assert_eq!(animator.current_frame_index(), 1);
    }
}
