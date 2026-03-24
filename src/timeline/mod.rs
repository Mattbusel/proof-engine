//! Cutscene and timeline system — scripted sequences of engine events.
//!
//! A `Timeline` is a sorted list of `CuePoint`s, each holding a `TimelineAction`
//! that fires when the playhead reaches the cue's time.  `TimelinePlayer` drives
//! the playhead and dispatches actions to engine callbacks.
//!
//! # Example
//! ```text
//! let mut tl = Timeline::new();
//! tl.at(0.0,  TimelineAction::FadeIn { duration: 1.0 });
//! tl.at(2.0,  TimelineAction::SpawnEntity { blueprint: "hero".into(), position: Vec3::ZERO });
//! tl.at(5.0,  TimelineAction::Dialogue { speaker: "Hero".into(), text: "We must fight!".into() });
//! tl.at(10.0, TimelineAction::FadeOut { duration: 0.5 });
//! let mut player = TimelinePlayer::new(tl);
//! // Each frame: player.tick(dt, &mut ctx);
//! ```

pub mod script;
pub mod dialogue;

use glam::Vec3;
use std::collections::HashMap;

// ── TimelineAction ────────────────────────────────────────────────────────────

/// An action that fires at a specific time in the timeline.
#[derive(Clone, Debug)]
pub enum TimelineAction {
    // ── Camera ───────────────────────────────────────────────────────────────
    /// Move camera to position over duration.
    CameraMoveTo { target: Vec3, duration: f32 },
    /// Look at a world position.
    CameraLookAt { target: Vec3, duration: f32 },
    /// Shake the camera.
    CameraShake  { intensity: f32, duration: f32, frequency: f32 },
    /// Set camera zoom.
    CameraZoom   { zoom: f32, duration: f32 },

    // ── Visual ───────────────────────────────────────────────────────────────
    /// Fade to black.
    FadeOut { duration: f32, color: [f32; 4] },
    /// Fade in from black.
    FadeIn  { duration: f32 },
    /// Flash screen.
    Flash   { color: [f32; 4], duration: f32, intensity: f32 },
    /// Enable/disable bloom.
    SetBloom { enabled: bool, intensity: f32, duration: f32 },
    /// Set chromatic aberration.
    SetChromaticAberration { amount: f32, duration: f32 },
    /// Enable film grain.
    SetFilmGrain { amount: f32 },
    /// Set vignette.
    SetVignette { radius: f32, softness: f32, intensity: f32 },

    // ── Entities ─────────────────────────────────────────────────────────────
    /// Spawn an entity blueprint at a position.
    SpawnEntity { blueprint: String, position: Vec3, tag: Option<String> },
    /// Despawn entities by tag.
    DespawnTag  { tag: String },
    /// Apply a force to entities with a tag.
    PushTag     { tag: String, force: Vec3, duration: f32 },
    /// Kill all entities with a tag.
    KillTag     { tag: String },

    // ── Audio ─────────────────────────────────────────────────────────────────
    /// Play a named sound effect.
    PlaySfx { name: String, volume: f32, position: Option<Vec3> },
    /// Set music vibe.
    SetMusicVibe { vibe: String },
    /// Set master volume.
    SetMasterVolume { volume: f32, duration: f32 },
    /// Stop all music.
    StopMusic,

    // ── UI ────────────────────────────────────────────────────────────────────
    /// Show a dialogue line.
    Dialogue { speaker: String, text: String, duration: Option<f32> },
    /// Show a title card (big text center screen).
    TitleCard { text: String, subtitle: String, duration: f32 },
    /// Show a HUD notification.
    Notify  { text: String, duration: f32 },
    /// Hide all dialogue.
    HideDialogue,

    // ── Control ───────────────────────────────────────────────────────────────
    /// Wait (no-op — used as a marker for script pauses).
    Wait { duration: f32 },
    /// Jump playhead to a named label.
    GotoLabel { label: String },
    /// Set a flag variable.
    SetFlag { name: String, value: bool },
    /// Conditional: only fire `then` if flag is true.
    IfFlag  { name: String, then: Box<TimelineAction> },
    /// Fire multiple actions simultaneously.
    Parallel { actions: Vec<TimelineAction> },
    /// Custom callback by name (engine resolves at runtime).
    Callback { name: String, args: HashMap<String, String> },
    /// End the timeline.
    End,
}

// ── CuePoint ──────────────────────────────────────────────────────────────────

/// A timed entry in the timeline.
#[derive(Clone, Debug)]
pub struct CuePoint {
    pub time:   f32,
    pub action: TimelineAction,
    /// Optional label for GotoLabel.
    pub label:  Option<String>,
    /// Whether this cue can fire multiple times (for looping timelines).
    pub repeat: bool,
    /// Has been fired this playthrough.
    pub fired:  bool,
}

impl CuePoint {
    pub fn new(time: f32, action: TimelineAction) -> Self {
        Self { time, action, label: None, repeat: false, fired: false }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn repeating(mut self) -> Self {
        self.repeat = true;
        self
    }
}

// ── Timeline ──────────────────────────────────────────────────────────────────

/// An ordered sequence of timed cue points.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    pub cues:   Vec<CuePoint>,
    pub name:   String,
    pub looping: bool,
    pub speed:   f32,  // playback speed multiplier
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            cues:    Vec::new(),
            name:    String::new(),
            looping: false,
            speed:   1.0,
        }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    pub fn with_speed(mut self, s: f32) -> Self {
        self.speed = s;
        self
    }

    /// Add a cue at a given time (will be sorted on insertion).
    pub fn at(&mut self, time: f32, action: TimelineAction) -> &mut Self {
        let idx = self.cues.partition_point(|c| c.time <= time);
        self.cues.insert(idx, CuePoint::new(time, action));
        self
    }

    /// Add a labeled cue.
    pub fn at_labeled(&mut self, time: f32, label: impl Into<String>, action: TimelineAction) -> &mut Self {
        let idx = self.cues.partition_point(|c| c.time <= time);
        self.cues.insert(idx, CuePoint::new(time, action).with_label(label));
        self
    }

    /// Total duration (time of last cue).
    pub fn duration(&self) -> f32 {
        self.cues.last().map(|c| c.time).unwrap_or(0.0)
    }

    /// Find the time of a label.
    pub fn label_time(&self, label: &str) -> Option<f32> {
        self.cues.iter().find(|c| c.label.as_deref() == Some(label)).map(|c| c.time)
    }

    /// Reset all fired flags.
    pub fn reset(&mut self) {
        for cue in &mut self.cues { cue.fired = false; }
    }
}

// ── PlaybackState ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Finished,
}

// ── TimelinePlayer ────────────────────────────────────────────────────────────

/// Drives a Timeline forward in time and dispatches actions.
pub struct TimelinePlayer {
    pub timeline: Timeline,
    pub time:     f32,
    pub state:    PlaybackState,
    /// Flags set by SetFlag actions.
    flags:        HashMap<String, bool>,
    /// Pending callbacks waiting for duration to elapse.
    active_waits: Vec<ActiveWait>,
    /// Callbacks registered by name for Callback actions.
    callbacks:    HashMap<String, Box<dyn Fn(&HashMap<String, String>) + Send + Sync>>,
}

struct ActiveWait {
    pub remaining: f32,
    pub on_done:   Box<dyn FnOnce() + Send>,
}

impl TimelinePlayer {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            time:         0.0,
            state:        PlaybackState::Stopped,
            flags:        HashMap::new(),
            active_waits: Vec::new(),
            callbacks:    HashMap::new(),
        }
    }

    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == PlaybackState::Paused {
            self.state = PlaybackState::Playing;
        }
    }

    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.time  = 0.0;
        self.timeline.reset();
    }

    pub fn seek(&mut self, time: f32) {
        self.time = time.clamp(0.0, self.timeline.duration());
        // Re-arm all cues at or after the seek point
        for cue in &mut self.timeline.cues {
            if cue.time >= self.time { cue.fired = false; }
        }
    }

    pub fn is_playing(&self) -> bool { self.state == PlaybackState::Playing }
    pub fn is_finished(&self) -> bool { self.state == PlaybackState::Finished }

    /// Register a named callback.
    pub fn register_callback(
        &mut self,
        name: impl Into<String>,
        f: impl Fn(&HashMap<String, String>) + Send + Sync + 'static,
    ) {
        self.callbacks.insert(name.into(), Box::new(f));
    }

    /// Advance the timeline by `dt` seconds and fire pending cues.
    /// Returns a list of actions that fired this tick.
    pub fn tick(&mut self, dt: f32) -> Vec<TimelineAction> {
        if self.state != PlaybackState::Playing { return Vec::new(); }

        let effective_dt = dt * self.timeline.speed;
        self.time += effective_dt;

        // Tick active waits
        self.active_waits.retain_mut(|w| {
            w.remaining -= effective_dt;
            w.remaining > 0.0
        });

        let duration = self.timeline.duration();
        if self.time > duration {
            if self.timeline.looping {
                self.time -= duration;
                self.timeline.reset();
            } else {
                self.time  = duration;
                self.state = PlaybackState::Finished;
            }
        }

        let current_time = self.time;
        let mut fired = Vec::new();

        for cue in &mut self.timeline.cues {
            if cue.fired { continue; }
            if cue.time > current_time { break; }

            cue.fired = true;
            fired.push(cue.action.clone());
        }

        // Process GotoLabel actions
        let mut goto: Option<String> = None;
        for action in &fired {
            if let TimelineAction::GotoLabel { label } = action {
                goto = Some(label.clone());
            }
        }
        if let Some(label) = goto {
            if let Some(t) = self.timeline.label_time(&label) {
                self.seek(t);
            }
        }

        // Process SetFlag actions
        for action in &fired {
            if let TimelineAction::SetFlag { name, value } = action {
                self.flags.insert(name.clone(), *value);
            }
        }

        // Process Callback actions
        for action in &fired {
            if let TimelineAction::Callback { name, args } = action {
                if let Some(cb) = self.callbacks.get(name.as_str()) {
                    cb(args);
                }
            }
        }

        fired
    }

    pub fn get_flag(&self, name: &str) -> bool {
        self.flags.get(name).copied().unwrap_or(false)
    }

    pub fn set_flag(&mut self, name: impl Into<String>, value: bool) {
        self.flags.insert(name.into(), value);
    }

    /// Progress [0, 1] through the timeline.
    pub fn progress(&self) -> f32 {
        let d = self.timeline.duration();
        if d < f32::EPSILON { 1.0 } else { (self.time / d).clamp(0.0, 1.0) }
    }
}

// ── CutsceneLibrary ───────────────────────────────────────────────────────────

/// Manages a collection of named timelines.
pub struct CutsceneLibrary {
    timelines: HashMap<String, Timeline>,
    pub active: Option<TimelinePlayer>,
}

impl CutsceneLibrary {
    pub fn new() -> Self {
        Self { timelines: HashMap::new(), active: None }
    }

    pub fn register(&mut self, timeline: Timeline) {
        self.timelines.insert(timeline.name.clone(), timeline);
    }

    /// Start playing a named cutscene. Returns false if not found.
    pub fn play(&mut self, name: &str) -> bool {
        if let Some(tl) = self.timelines.get(name).cloned() {
            let mut player = TimelinePlayer::new(tl);
            player.play();
            self.active = Some(player);
            true
        } else {
            false
        }
    }

    /// Stop the active cutscene.
    pub fn stop(&mut self) {
        if let Some(p) = &mut self.active { p.stop(); }
        self.active = None;
    }

    /// Tick the active player, returning fired actions.
    pub fn tick(&mut self, dt: f32) -> Vec<TimelineAction> {
        if let Some(player) = &mut self.active {
            let actions = player.tick(dt);
            if player.is_finished() { self.active = None; }
            actions
        } else {
            Vec::new()
        }
    }

    pub fn is_playing(&self) -> bool {
        self.active.as_ref().map(|p| p.is_playing()).unwrap_or(false)
    }

    pub fn names(&self) -> Vec<&str> {
        self.timelines.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for CutsceneLibrary {
    fn default() -> Self { Self::new() }
}

// ── Built-in timeline builders ────────────────────────────────────────────────

/// Factory methods for common cutscene patterns.
pub struct CutsceneTemplates;

impl CutsceneTemplates {
    /// Simple intro: fade in, wait, show title, fade out.
    pub fn intro(title: &str, subtitle: &str, duration: f32) -> Timeline {
        let mut tl = Timeline::new().named("intro");
        tl.at(0.0,        TimelineAction::FadeOut { duration: 0.0, color: [0.0,0.0,0.0,1.0] });
        tl.at(0.5,        TimelineAction::FadeIn  { duration: 1.5 });
        tl.at(2.0,        TimelineAction::TitleCard {
            text:     title.into(),
            subtitle: subtitle.into(),
            duration,
        });
        tl.at(2.0 + duration, TimelineAction::FadeOut { duration: 1.0, color: [0.0,0.0,0.0,1.0] });
        tl.at(3.0 + duration, TimelineAction::End);
        tl
    }

    /// Boss encounter intro: screen flash, camera shake, music sting.
    pub fn boss_intro(boss_name: &str, position: Vec3) -> Timeline {
        let mut tl = Timeline::new().named("boss_intro");
        tl.at(0.0, TimelineAction::SetMusicVibe { vibe: "boss".into() });
        tl.at(0.0, TimelineAction::CameraShake  { intensity: 0.3, duration: 0.5, frequency: 20.0 });
        tl.at(0.0, TimelineAction::Flash        { color: [1.0,0.2,0.0,1.0], duration: 0.3, intensity: 2.0 });
        tl.at(0.5, TimelineAction::SpawnEntity  { blueprint: boss_name.into(), position, tag: Some("boss".into()) });
        tl.at(1.0, TimelineAction::CameraLookAt { target: position, duration: 0.5 });
        tl.at(1.5, TimelineAction::TitleCard {
            text:     boss_name.into(),
            subtitle: "BOSS ENCOUNTER".into(),
            duration: 2.5,
        });
        tl.at(4.0, TimelineAction::SetBloom    { enabled: true, intensity: 1.5, duration: 0.3 });
        tl.at(4.5, TimelineAction::End);
        tl
    }

    /// Victory sequence: music swell, sparkles, score tally.
    pub fn victory() -> Timeline {
        let mut tl = Timeline::new().named("victory");
        tl.at(0.0, TimelineAction::SetMusicVibe { vibe: "victory".into() });
        tl.at(0.0, TimelineAction::SetBloom     { enabled: true, intensity: 2.0, duration: 0.5 });
        tl.at(0.3, TimelineAction::Flash        { color: [1.0,1.0,0.5,1.0], duration: 0.4, intensity: 1.5 });
        tl.at(0.5, TimelineAction::TitleCard    {
            text: "VICTORY".into(), subtitle: "".into(), duration: 3.0,
        });
        tl.at(3.8, TimelineAction::FadeOut      { duration: 1.2, color: [0.0,0.0,0.0,1.0] });
        tl.at(5.0, TimelineAction::End);
        tl
    }

    /// Death / game-over sequence.
    pub fn death() -> Timeline {
        let mut tl = Timeline::new().named("death");
        tl.at(0.0, TimelineAction::CameraShake         { intensity: 0.5, duration: 0.8, frequency: 15.0 });
        tl.at(0.0, TimelineAction::SetMusicVibe         { vibe: "silence".into() });
        tl.at(0.0, TimelineAction::SetChromaticAberration { amount: 0.04, duration: 0.1 });
        tl.at(0.1, TimelineAction::SetFilmGrain         { amount: 0.3 });
        tl.at(0.5, TimelineAction::SetMasterVolume      { volume: 0.0, duration: 0.5 });
        tl.at(0.6, TimelineAction::FadeOut              { duration: 1.5, color: [0.4,0.0,0.0,1.0] });
        tl.at(2.0, TimelineAction::TitleCard            {
            text: "YOU DIED".into(), subtitle: "".into(), duration: 2.5,
        });
        tl.at(4.5, TimelineAction::End);
        tl
    }

    /// Simple level transition.
    pub fn level_transition(level_name: &str) -> Timeline {
        let mut tl = Timeline::new().named("level_transition");
        tl.at(0.0, TimelineAction::FadeOut { duration: 0.5, color: [0.0,0.0,0.0,1.0] });
        tl.at(0.5, TimelineAction::DespawnTag { tag: "level_geometry".into() });
        tl.at(1.0, TimelineAction::Callback { name: "load_level".into(), args: {
            let mut m = HashMap::new(); m.insert("name".into(), level_name.into()); m
        }});
        tl.at(1.5, TimelineAction::FadeIn  { duration: 0.8 });
        tl.at(2.3, TimelineAction::TitleCard {
            text:     level_name.into(),
            subtitle: "".into(),
            duration: 1.5,
        });
        tl.at(3.8, TimelineAction::End);
        tl
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_fires_in_order() {
        let mut tl = Timeline::new();
        tl.at(0.5, TimelineAction::Wait { duration: 0.0 });
        tl.at(1.0, TimelineAction::End);
        tl.at(0.1, TimelineAction::Flash { color: [1.0,0.0,0.0,1.0], duration: 0.1, intensity: 1.0 });

        // Check sorted order
        assert!(tl.cues[0].time <= tl.cues[1].time);
        assert!(tl.cues[1].time <= tl.cues[2].time);
    }

    #[test]
    fn player_fires_actions() {
        let mut tl = Timeline::new();
        tl.at(0.1, TimelineAction::Flash { color: [1.0,0.0,0.0,1.0], duration: 0.1, intensity: 1.0 });
        tl.at(0.5, TimelineAction::End);

        let mut player = TimelinePlayer::new(tl);
        player.play();

        let actions = player.tick(0.2);
        assert!(!actions.is_empty(), "Expected Flash to fire");
    }

    #[test]
    fn player_does_not_fire_future_cues() {
        let mut tl = Timeline::new();
        tl.at(5.0, TimelineAction::End);
        let mut player = TimelinePlayer::new(tl);
        player.play();
        let actions = player.tick(0.1);
        assert!(actions.is_empty());
    }

    #[test]
    fn player_finishes() {
        let mut tl = Timeline::new();
        tl.at(0.1, TimelineAction::End);
        let mut player = TimelinePlayer::new(tl);
        player.play();
        player.tick(1.0);
        assert!(player.is_finished());
    }

    #[test]
    fn flag_set_and_get() {
        let mut player = TimelinePlayer::new(Timeline::new());
        player.set_flag("combat_started", true);
        assert!(player.get_flag("combat_started"));
        assert!(!player.get_flag("other_flag"));
    }

    #[test]
    fn progress_zero_at_start() {
        let mut tl = Timeline::new();
        tl.at(10.0, TimelineAction::End);
        let player = TimelinePlayer::new(tl);
        assert!((player.progress() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn library_play_unknown() {
        let mut lib = CutsceneLibrary::new();
        assert!(!lib.play("nonexistent"));
    }

    #[test]
    fn library_play_registered() {
        let mut lib = CutsceneLibrary::new();
        let tl = CutsceneTemplates::victory();
        lib.register(tl);
        assert!(lib.play("victory"));
        assert!(lib.is_playing());
    }

    #[test]
    fn template_intro_has_cues() {
        let tl = CutsceneTemplates::intro("Test", "Subtitle", 3.0);
        assert!(!tl.cues.is_empty());
        assert!(tl.duration() > 0.0);
    }
}
