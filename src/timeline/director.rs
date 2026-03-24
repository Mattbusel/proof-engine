//! Cinematic director system — high-level cutscene orchestration.
//!
//! `CinematicDirector` manages the lifecycle of cutscenes: playing, pausing,
//! skipping, looping, and dispatching cinematic events.  It integrates camera
//! shake, letterboxing / pillar-boxing, fade overlays, and screen overlays.
//!
//! # Example
//! ```text
//! let mut lib = CutsceneLibrary::new();
//! lib.register(Cutscene::new("intro").skippable(true).duration(12.0));
//!
//! let mut director = CinematicDirector::new(lib);
//! director.command(DirectorCommand::Play("intro".into()));
//!
//! // each frame:
//! let events = director.tick(dt);
//! for ev in events { ... }
//! ```

use std::collections::HashMap;
use glam::{Vec3, Vec4};

// ── ShakeProfile ──────────────────────────────────────────────────────────────

/// Defines a camera-shake waveform.
#[derive(Debug, Clone)]
pub struct ShakeProfile {
    /// Oscillations per second.
    pub frequency: f32,
    /// Peak displacement in world units.
    pub amplitude: f32,
    /// Exponential decay rate (per second).  Higher = faster settle.
    pub decay:     f32,
    /// Directional bias [0,1] — 0 = omnidirectional, 1 = pure horizontal.
    pub horizontal_bias: f32,
    /// Total duration of the shake in seconds (f32::MAX = indefinite).
    pub duration:  f32,
}

impl ShakeProfile {
    pub fn new(frequency: f32, amplitude: f32, decay: f32) -> Self {
        Self {
            frequency,
            amplitude,
            decay,
            horizontal_bias: 0.5,
            duration: f32::MAX,
        }
    }

    pub fn with_duration(mut self, d: f32) -> Self { self.duration = d; self }
    pub fn with_bias(mut self, b: f32) -> Self { self.horizontal_bias = b.clamp(0.0, 1.0); self }

    /// Evaluate offset at time `t` (seconds since shake began).
    /// Returns a Vec3 displacement.
    pub fn evaluate(&self, t: f32, seed: f32) -> Vec3 {
        let decay_factor = (-self.decay * t).exp();
        let envelope     = self.amplitude * decay_factor;
        // Use two sine waves at different phases for pseudo-random feel.
        let phase_x = t * self.frequency * std::f32::consts::TAU + seed;
        let phase_y = t * self.frequency * std::f32::consts::TAU + seed + 1.3;
        let phase_z = t * self.frequency * std::f32::consts::TAU * 0.7 + seed + 2.7;
        let hb = self.horizontal_bias;
        Vec3::new(
            phase_x.sin() * envelope * (0.5 + hb * 0.5),
            phase_y.sin() * envelope * (0.5 - hb * 0.5 + 0.1),
            phase_z.sin() * envelope * 0.15,
        )
    }
}

impl Default for ShakeProfile {
    fn default() -> Self { Self::new(12.0, 0.1, 3.0) }
}

// ── ActiveShake ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ActiveShake {
    profile: ShakeProfile,
    elapsed: f32,
    seed:    f32,
}

impl ActiveShake {
    fn new(profile: ShakeProfile, seed: f32) -> Self {
        Self { profile, elapsed: 0.0, seed }
    }

    fn tick(&mut self, dt: f32) -> Vec3 {
        let offset = self.profile.evaluate(self.elapsed, self.seed);
        self.elapsed += dt;
        offset
    }

    fn is_done(&self) -> bool {
        self.elapsed >= self.profile.duration
    }
}

// ── FadeState ─────────────────────────────────────────────────────────────────

/// Describes an ongoing screen fade.
#[derive(Debug, Clone)]
pub struct FadeState {
    pub from_alpha: f32,
    pub to_alpha:   f32,
    pub color:      [f32; 4],
    pub elapsed:    f32,
    pub duration:   f32,
}

impl FadeState {
    pub fn fade_in(duration: f32) -> Self {
        Self { from_alpha: 1.0, to_alpha: 0.0, color: [0.0, 0.0, 0.0, 1.0], elapsed: 0.0, duration }
    }

    pub fn fade_out(color: [f32; 4], duration: f32) -> Self {
        Self { from_alpha: 0.0, to_alpha: 1.0, color, elapsed: 0.0, duration }
    }

    pub fn fade_to(from: f32, to: f32, color: [f32; 4], duration: f32) -> Self {
        Self { from_alpha: from, to_alpha: to, color, elapsed: 0.0, duration }
    }

    /// Current alpha [0,1].
    pub fn current_alpha(&self) -> f32 {
        let t = if self.duration < f32::EPSILON {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        };
        self.from_alpha + t * (self.to_alpha - self.from_alpha)
    }

    pub fn is_complete(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
    }
}

// ── LetterboxState ────────────────────────────────────────────────────────────

/// Animated letterbox / pillar-box bars.
#[derive(Debug, Clone)]
pub struct LetterboxState {
    /// Target bar fraction (0 = none, 0.1 = 10% bars).
    pub target_fraction: f32,
    /// Current fraction (animated toward target).
    pub current_fraction: f32,
    /// Transition speed (fraction per second).
    pub speed: f32,
    /// Bar color.
    pub color: [f32; 4],
    /// True = horizontal bars (letterbox), false = vertical (pillar-box).
    pub horizontal: bool,
}

impl LetterboxState {
    pub fn new() -> Self {
        Self {
            target_fraction:  0.0,
            current_fraction: 0.0,
            speed:            2.0,
            color:            [0.0, 0.0, 0.0, 1.0],
            horizontal:       true,
        }
    }

    pub fn show(mut self, fraction: f32) -> Self {
        self.target_fraction = fraction.clamp(0.0, 0.45);
        self
    }

    pub fn cinematic() -> Self {
        // 2.35:1 anamorphic equivalent at 16:9 base
        Self {
            target_fraction:  0.1056,
            current_fraction: 0.0,
            speed:            3.0,
            color:            [0.0, 0.0, 0.0, 1.0],
            horizontal:       true,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let diff = self.target_fraction - self.current_fraction;
        let step = self.speed * dt;
        if diff.abs() <= step {
            self.current_fraction = self.target_fraction;
        } else {
            self.current_fraction += step * diff.signum();
        }
    }

    pub fn top_bar(&self) -> f32 { self.current_fraction }
    pub fn bottom_bar(&self) -> f32 { self.current_fraction }
    pub fn is_settled(&self) -> bool { (self.current_fraction - self.target_fraction).abs() < 0.001 }
}

impl Default for LetterboxState {
    fn default() -> Self { Self::new() }
}

// ── ScreenOverlay ─────────────────────────────────────────────────────────────

/// A full-screen texture overlay (e.g., vignette, film burn, HUD flash).
#[derive(Debug, Clone)]
pub struct ScreenOverlay {
    /// Asset handle name for the texture.
    pub texture: String,
    /// Current alpha [0,1].
    pub alpha:   f32,
    /// Target alpha.
    pub target_alpha: f32,
    /// Fade speed.
    pub fade_speed: f32,
    /// Blend mode string — "alpha", "additive", "multiply".
    pub blend_mode: String,
    /// Tint color multiplied with texture.
    pub tint: [f32; 4],
    /// Whether this overlay should be removed when alpha reaches 0.
    pub auto_remove: bool,
}

impl ScreenOverlay {
    pub fn new(texture: impl Into<String>) -> Self {
        Self {
            texture:      texture.into(),
            alpha:        0.0,
            target_alpha: 1.0,
            fade_speed:   4.0,
            blend_mode:   "alpha".into(),
            tint:         [1.0, 1.0, 1.0, 1.0],
            auto_remove:  false,
        }
    }

    pub fn with_alpha(mut self, a: f32) -> Self { self.alpha = a; self.target_alpha = a; self }
    pub fn with_blend(mut self, mode: impl Into<String>) -> Self { self.blend_mode = mode.into(); self }
    pub fn with_tint(mut self, tint: [f32; 4]) -> Self { self.tint = tint; self }
    pub fn with_fade_speed(mut self, s: f32) -> Self { self.fade_speed = s; self }
    pub fn auto_remove(mut self) -> Self { self.auto_remove = true; self }

    pub fn fade_in(&mut self, target: f32) { self.target_alpha = target.clamp(0.0, 1.0); }
    pub fn fade_out(&mut self)             { self.target_alpha = 0.0; }

    pub fn tick(&mut self, dt: f32) {
        let diff = self.target_alpha - self.alpha;
        let step = self.fade_speed * dt;
        if diff.abs() <= step {
            self.alpha = self.target_alpha;
        } else {
            self.alpha += step * diff.signum();
        }
    }

    pub fn should_remove(&self) -> bool {
        self.auto_remove && self.alpha < f32::EPSILON
    }
}

// ── Cutscene ──────────────────────────────────────────────────────────────────

/// Metadata for a registered cutscene.
#[derive(Debug, Clone)]
pub struct Cutscene {
    /// Unique identifier.
    pub id:        String,
    /// Human-readable display name.
    pub name:      String,
    /// Whether the player can skip this cutscene.
    pub skippable: bool,
    /// How many times to loop before completing (0 = play once).
    pub loop_count: u32,
    /// Nominal duration in seconds (used for progress computation).
    pub duration:  f32,
    /// Optional tags for grouping.
    pub tags:      Vec<String>,
    /// Whether camera letterbox should be shown during this cutscene.
    pub letterbox: bool,
    /// Whether to disable player input during playback.
    pub lock_input: bool,
    /// Version counter for hot-reload detection.
    pub version:   u32,
}

impl Cutscene {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name:       id.clone(),
            id,
            skippable:  true,
            loop_count: 0,
            duration:   0.0,
            tags:       Vec::new(),
            letterbox:  true,
            lock_input: true,
            version:    1,
        }
    }

    pub fn named(mut self, name: impl Into<String>) -> Self { self.name = name.into(); self }
    pub fn skippable(mut self, v: bool) -> Self { self.skippable = v; self }
    pub fn loops(mut self, n: u32) -> Self { self.loop_count = n; self }
    pub fn duration(mut self, d: f32) -> Self { self.duration = d; self }
    pub fn letterbox(mut self, v: bool) -> Self { self.letterbox = v; self }
    pub fn lock_input(mut self, v: bool) -> Self { self.lock_input = v; self }
    pub fn tag(mut self, t: impl Into<String>) -> Self { self.tags.push(t.into()); self }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

// ── CutsceneState ─────────────────────────────────────────────────────────────

/// Current playback state of the active cutscene.
#[derive(Debug, Clone)]
pub enum CutsceneState {
    /// No cutscene loaded.
    Idle,
    /// Cutscene is actively playing.
    Playing {
        elapsed:    f32,
        /// Playback speed multiplier.
        speed:      f32,
        /// Current loop iteration.
        loop_iter:  u32,
    },
    /// Cutscene is paused.
    Paused {
        elapsed:    f32,
        speed:      f32,
        loop_iter:  u32,
    },
    /// Skip animation in progress (fast-forward to end).
    Skipping {
        elapsed:    f32,
        speed:      f32,
    },
    /// Playback finished (before cleanup).
    Complete {
        total_time: f32,
        skipped:    bool,
    },
}

impl CutsceneState {
    pub fn elapsed(&self) -> f32 {
        match self {
            CutsceneState::Playing  { elapsed, .. } => *elapsed,
            CutsceneState::Paused   { elapsed, .. } => *elapsed,
            CutsceneState::Skipping { elapsed, .. } => *elapsed,
            CutsceneState::Complete { total_time, ..} => *total_time,
            CutsceneState::Idle     => 0.0,
        }
    }

    pub fn is_playing(&self) -> bool { matches!(self, CutsceneState::Playing { .. }) }
    pub fn is_paused(&self)  -> bool { matches!(self, CutsceneState::Paused  { .. }) }
    pub fn is_complete(&self)-> bool { matches!(self, CutsceneState::Complete{ .. }) }
    pub fn is_idle(&self)    -> bool { matches!(self, CutsceneState::Idle     )      }
}

// ── DirectorCommand ────────────────────────────────────────────────────────────

/// Commands sent to the `CinematicDirector`.
#[derive(Debug, Clone)]
pub enum DirectorCommand {
    /// Begin playing the named cutscene.
    Play(String),
    /// Pause the active cutscene.
    Pause,
    /// Resume a paused cutscene.
    Resume,
    /// Skip the active cutscene (if skippable).
    Skip,
    /// Stop and clear the active cutscene immediately.
    Stop,
    /// Set playback speed multiplier.
    SetSpeed(f32),
    /// Jump playhead to a specific time.
    JumpTo(f32),
    /// Trigger a fade to color.
    FadeToColor { color: [f32; 4], duration: f32 },
    /// Trigger a fade in from current overlay.
    FadeIn { duration: f32 },
    /// Start a camera shake.
    StartShake(ShakeProfile),
    /// Stop all active shakes.
    StopShake,
    /// Show/hide letterbox bars.
    SetLetterbox { enabled: bool },
    /// Add a screen overlay.
    AddOverlay(ScreenOverlay),
    /// Remove a screen overlay by texture name.
    RemoveOverlay(String),
}

// ── CinematicEvent ────────────────────────────────────────────────────────────

/// Events dispatched by the director.
#[derive(Debug, Clone)]
pub enum CinematicEvent {
    /// A cutscene began playing.
    Started { id: String },
    /// A cutscene finished naturally.
    Completed { id: String, total_time: f32 },
    /// A cutscene was skipped by the player.
    Skipped { id: String, at_time: f32 },
    /// A looping cutscene completed one iteration.
    Looped { id: String, iteration: u32 },
    /// Playback reached a named marker.
    MarkerReached(String),
    /// Playback was paused.
    Paused { id: String },
    /// Playback was resumed.
    Resumed { id: String },
    /// A fade operation completed.
    FadeComplete,
    /// Shake ended.
    ShakeComplete,
}

// ── CutsceneLibrary ────────────────────────────────────────────────────────────

/// Stores and manages registered `Cutscene` definitions.
/// Supports hot-reload via version tracking.
pub struct CutsceneLibrary {
    entries: HashMap<String, Cutscene>,
    /// Last seen version per id (for hot-reload change detection).
    versions: HashMap<String, u32>,
}

impl CutsceneLibrary {
    pub fn new() -> Self {
        Self {
            entries:  HashMap::new(),
            versions: HashMap::new(),
        }
    }

    /// Register or update a cutscene definition.
    pub fn register(&mut self, cutscene: Cutscene) {
        self.versions.insert(cutscene.id.clone(), cutscene.version);
        self.entries.insert(cutscene.id.clone(), cutscene);
    }

    /// Remove a cutscene definition.
    pub fn unregister(&mut self, id: &str) {
        self.entries.remove(id);
        self.versions.remove(id);
    }

    pub fn get(&self, id: &str) -> Option<&Cutscene> {
        self.entries.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    pub fn all_ids(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    pub fn with_tag(&self, tag: &str) -> Vec<&Cutscene> {
        self.entries.values().filter(|c| c.has_tag(tag)).collect()
    }

    /// Hot-reload: update a cutscene definition and bump its version.
    /// Returns true if the definition changed.
    pub fn hot_reload(&mut self, mut cutscene: Cutscene) -> bool {
        let old_version = self.versions.get(&cutscene.id).copied().unwrap_or(0);
        cutscene.version = old_version + 1;
        let changed = self.entries.get(&cutscene.id).map(|old| old.version != cutscene.version).unwrap_or(true);
        if changed {
            self.versions.insert(cutscene.id.clone(), cutscene.version);
            self.entries.insert(cutscene.id.clone(), cutscene);
        }
        changed
    }

    /// Count of registered cutscenes.
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

impl Default for CutsceneLibrary {
    fn default() -> Self { Self::new() }
}

// ── EventCallback ─────────────────────────────────────────────────────────────

type EventCallback = Box<dyn Fn(&CinematicEvent) + Send + Sync>;

// ── CinematicDirector ─────────────────────────────────────────────────────────

/// The top-level cinematic director.
///
/// Owns the cutscene library, playback state, visual effects (shake, letterbox,
/// fades, overlays) and dispatches `CinematicEvent`s each frame via `tick()`.
pub struct CinematicDirector {
    pub library:    CutsceneLibrary,
    /// Currently loaded cutscene id.
    pub active_id:  Option<String>,
    /// Current playback state.
    pub state:      CutsceneState,
    /// Active camera shakes (can be layered).
    shakes:         Vec<ActiveShake>,
    /// Accumulated shake seed counter.
    shake_seed:     f32,
    /// Current fade state (if any).
    pub fade:       Option<FadeState>,
    /// Letterbox bars state.
    pub letterbox:  LetterboxState,
    /// Active screen overlays (keyed by texture name).
    overlays:       Vec<ScreenOverlay>,
    /// Registered event callbacks.
    callbacks:      Vec<EventCallback>,
    /// Marker times for the active cutscene (name -> time).
    markers:        HashMap<String, f32>,
    /// Which markers have fired this playback.
    fired_markers:  Vec<String>,
    /// Internal frame counter for determinism / debugging.
    frame_count:    u64,
    /// Whether player input is currently locked.
    pub input_locked: bool,
}

impl CinematicDirector {
    pub fn new(library: CutsceneLibrary) -> Self {
        Self {
            library,
            active_id:   None,
            state:        CutsceneState::Idle,
            shakes:       Vec::new(),
            shake_seed:   0.0,
            fade:         None,
            letterbox:    LetterboxState::new(),
            overlays:     Vec::new(),
            callbacks:    Vec::new(),
            markers:      HashMap::new(),
            fired_markers: Vec::new(),
            frame_count:  0,
            input_locked: false,
        }
    }

    // ── Registration helpers ──────────────────────────────────────────────────

    /// Register a callback invoked on every `CinematicEvent`.
    pub fn on_event<F>(&mut self, f: F)
    where
        F: Fn(&CinematicEvent) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(f));
    }

    /// Register a named time marker for the active cutscene.
    pub fn add_marker(&mut self, name: impl Into<String>, time: f32) {
        self.markers.insert(name.into(), time);
    }

    /// Clear all markers.
    pub fn clear_markers(&mut self) {
        self.markers.clear();
        self.fired_markers.clear();
    }

    // ── Command dispatch ──────────────────────────────────────────────────────

    /// Process a director command.  Returns immediately-fired events.
    pub fn command(&mut self, cmd: DirectorCommand) -> Vec<CinematicEvent> {
        let mut events = Vec::new();
        match cmd {
            DirectorCommand::Play(id) => {
                if let Some(cutscene) = self.library.get(&id).cloned() {
                    self.active_id    = Some(id.clone());
                    self.state        = CutsceneState::Playing {
                        elapsed:   0.0,
                        speed:     1.0,
                        loop_iter: 0,
                    };
                    self.fired_markers.clear();
                    self.input_locked = cutscene.lock_input;

                    if cutscene.letterbox {
                        self.letterbox = LetterboxState::cinematic();
                    } else {
                        self.letterbox.target_fraction = 0.0;
                    }

                    events.push(CinematicEvent::Started { id });
                }
            }
            DirectorCommand::Pause => {
                if let CutsceneState::Playing { elapsed, speed, loop_iter } = self.state.clone() {
                    let id = self.active_id.clone().unwrap_or_default();
                    self.state = CutsceneState::Paused { elapsed, speed, loop_iter };
                    events.push(CinematicEvent::Paused { id });
                }
            }
            DirectorCommand::Resume => {
                if let CutsceneState::Paused { elapsed, speed, loop_iter } = self.state.clone() {
                    let id = self.active_id.clone().unwrap_or_default();
                    self.state = CutsceneState::Playing { elapsed, speed, loop_iter };
                    events.push(CinematicEvent::Resumed { id });
                }
            }
            DirectorCommand::Skip => {
                let can_skip = self.active_id.as_deref()
                    .and_then(|id| self.library.get(id))
                    .map(|c| c.skippable)
                    .unwrap_or(false);

                if can_skip {
                    let elapsed = self.state.elapsed();
                    let speed   = match &self.state {
                        CutsceneState::Playing { speed, ..} => *speed,
                        CutsceneState::Paused  { speed, ..} => *speed,
                        _ => 1.0,
                    };
                    self.state = CutsceneState::Skipping { elapsed, speed };
                }
            }
            DirectorCommand::Stop => {
                self.state       = CutsceneState::Idle;
                self.active_id   = None;
                self.input_locked = false;
                self.letterbox.target_fraction = 0.0;
                self.fired_markers.clear();
            }
            DirectorCommand::SetSpeed(s) => {
                let clamped = s.clamp(0.01, 10.0);
                match &mut self.state {
                    CutsceneState::Playing { speed, ..} => *speed = clamped,
                    CutsceneState::Paused  { speed, ..} => *speed = clamped,
                    _ => {}
                }
            }
            DirectorCommand::JumpTo(t) => {
                let duration = self.active_id.as_deref()
                    .and_then(|id| self.library.get(id))
                    .map(|c| c.duration)
                    .unwrap_or(0.0);
                let clamped = t.clamp(0.0, duration.max(t));
                match &mut self.state {
                    CutsceneState::Playing { elapsed, ..} => *elapsed = clamped,
                    CutsceneState::Paused  { elapsed, ..} => *elapsed = clamped,
                    _ => {}
                }
                // Re-arm markers after the jumped-to time
                self.fired_markers.retain(|m| {
                    self.markers.get(m.as_str()).map(|&mt| mt < clamped).unwrap_or(false)
                });
            }
            DirectorCommand::FadeToColor { color, duration } => {
                let current = self.current_fade_alpha();
                self.fade = Some(FadeState::fade_to(current, 1.0, color, duration));
            }
            DirectorCommand::FadeIn { duration } => {
                let current = self.current_fade_alpha();
                self.fade = Some(FadeState::fade_to(current, 0.0, [0.0, 0.0, 0.0, 1.0], duration));
            }
            DirectorCommand::StartShake(profile) => {
                self.shake_seed += 1.7;
                self.shakes.push(ActiveShake::new(profile, self.shake_seed));
            }
            DirectorCommand::StopShake => {
                self.shakes.clear();
            }
            DirectorCommand::SetLetterbox { enabled } => {
                self.letterbox.target_fraction = if enabled { 0.1056 } else { 0.0 };
            }
            DirectorCommand::AddOverlay(overlay) => {
                // Remove existing overlay with same texture name
                self.overlays.retain(|o| o.texture != overlay.texture);
                self.overlays.push(overlay);
            }
            DirectorCommand::RemoveOverlay(texture) => {
                for o in &mut self.overlays {
                    if o.texture == texture {
                        o.fade_out();
                        o.auto_remove = true;
                    }
                }
            }
        }
        self.dispatch_events(&events);
        events
    }

    // ── Per-frame tick ────────────────────────────────────────────────────────

    /// Advance the director by `dt` seconds.
    /// Returns a list of `CinematicEvent`s that fired this frame.
    pub fn tick(&mut self, dt: f32) -> Vec<CinematicEvent> {
        self.frame_count += 1;
        let mut events: Vec<CinematicEvent> = Vec::new();

        // --- Tick fade ---
        if let Some(ref mut fade) = self.fade {
            fade.tick(dt);
            if fade.is_complete() {
                events.push(CinematicEvent::FadeComplete);
                self.fade = None;
            }
        }

        // --- Tick letterbox ---
        self.letterbox.tick(dt);

        // --- Tick overlays ---
        for overlay in &mut self.overlays {
            overlay.tick(dt);
        }
        self.overlays.retain(|o| !o.should_remove());

        // --- Tick shakes ---
        self.shakes.retain(|s| !s.is_done());
        for shake in &mut self.shakes {
            shake.tick(dt);
        }
        if self.shakes.is_empty() && self.frame_count > 1 {
            // Only fire ShakeComplete if we had shakes and they just cleared
        }

        // --- Advance playback ---
        self.tick_playback(dt, &mut events);

        // Dispatch collected events to callbacks
        self.dispatch_events(&events);
        events
    }

    fn tick_playback(&mut self, dt: f32, events: &mut Vec<CinematicEvent>) {
        let active_id = match self.active_id.clone() {
            Some(id) => id,
            None     => return,
        };

        match self.state.clone() {
            CutsceneState::Playing { mut elapsed, speed, mut loop_iter } => {
                elapsed += dt * speed;

                // Fire markers
                let mut new_fired = Vec::new();
                for (name, &time) in &self.markers {
                    if elapsed >= time && !self.fired_markers.contains(name) {
                        events.push(CinematicEvent::MarkerReached(name.clone()));
                        new_fired.push(name.clone());
                    }
                }
                self.fired_markers.extend(new_fired);

                let duration = self.library.get(&active_id)
                    .map(|c| c.duration)
                    .unwrap_or(f32::MAX);

                let loop_count = self.library.get(&active_id)
                    .map(|c| c.loop_count)
                    .unwrap_or(0);

                if duration > 0.0 && elapsed >= duration {
                    if loop_iter < loop_count {
                        loop_iter += 1;
                        elapsed -= duration;
                        self.fired_markers.clear();
                        events.push(CinematicEvent::Looped { id: active_id.clone(), iteration: loop_iter });
                        self.state = CutsceneState::Playing { elapsed, speed, loop_iter };
                    } else {
                        let total = elapsed;
                        self.state = CutsceneState::Complete { total_time: total, skipped: false };
                        self.input_locked = false;
                        self.letterbox.target_fraction = 0.0;
                        events.push(CinematicEvent::Completed { id: active_id, total_time: total });
                    }
                } else {
                    self.state = CutsceneState::Playing { elapsed, speed, loop_iter };
                }
            }
            CutsceneState::Skipping { mut elapsed, speed } => {
                // Fast-forward at 8x speed
                elapsed += dt * speed * 8.0;

                let duration = self.library.get(&active_id)
                    .map(|c| c.duration)
                    .unwrap_or(1.0);

                if elapsed >= duration || duration < f32::EPSILON {
                    let at_time = self.state.elapsed();
                    self.state = CutsceneState::Complete { total_time: elapsed, skipped: true };
                    self.input_locked = false;
                    self.letterbox.target_fraction = 0.0;
                    events.push(CinematicEvent::Skipped { id: active_id, at_time });
                } else {
                    self.state = CutsceneState::Skipping { elapsed, speed };
                }
            }
            CutsceneState::Complete { .. } => {
                // Linger one frame in Complete then go Idle
                self.state     = CutsceneState::Idle;
                self.active_id = None;
            }
            _ => {}
        }
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Compute the total camera shake offset this frame.
    pub fn shake_offset(&self) -> Vec3 {
        self.shakes.iter().fold(Vec3::ZERO, |acc, s| {
            acc + s.profile.evaluate(s.elapsed, s.seed)
        })
    }

    /// Current fade overlay color (RGBA) — alpha represents opacity.
    pub fn fade_color(&self) -> [f32; 4] {
        if let Some(ref fade) = self.fade {
            let mut c = fade.color;
            c[3] = fade.current_alpha();
            c
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    }

    fn current_fade_alpha(&self) -> f32 {
        self.fade.as_ref().map(|f| f.current_alpha()).unwrap_or(0.0)
    }

    /// Progress [0, 1] through the active cutscene.
    pub fn progress(&self) -> f32 {
        let elapsed = self.state.elapsed();
        let duration = self.active_id.as_deref()
            .and_then(|id| self.library.get(id))
            .map(|c| c.duration)
            .unwrap_or(0.0);
        if duration < f32::EPSILON { return 0.0; }
        (elapsed / duration).clamp(0.0, 1.0)
    }

    /// Whether a cutscene is currently playing (not paused).
    pub fn is_playing(&self) -> bool { self.state.is_playing() }
    /// Whether a cutscene is paused.
    pub fn is_paused(&self) -> bool  { self.state.is_paused()  }
    /// Whether the director is idle (no active cutscene).
    pub fn is_idle(&self) -> bool    { self.state.is_idle()    }

    /// The active cutscene's metadata (if any).
    pub fn active_cutscene(&self) -> Option<&Cutscene> {
        self.active_id.as_deref().and_then(|id| self.library.get(id))
    }

    /// Current letterbox top/bottom bar fractions.
    pub fn letterbox_bars(&self) -> (f32, f32) {
        (self.letterbox.top_bar(), self.letterbox.bottom_bar())
    }

    /// All active overlays.
    pub fn overlays(&self) -> &[ScreenOverlay] {
        &self.overlays
    }

    /// Total frame count since creation.
    pub fn frame_count(&self) -> u64 { self.frame_count }

    // ── Internal event dispatch ───────────────────────────────────────────────

    fn dispatch_events(&self, events: &[CinematicEvent]) {
        for ev in events {
            for cb in &self.callbacks {
                cb(ev);
            }
        }
    }

    // ── Convenience helpers ───────────────────────────────────────────────────

    /// Start a quick screen flash (adds and auto-removes an overlay).
    pub fn flash(&mut self, color: [f32; 4], duration: f32) {
        let overlay = ScreenOverlay::new("__flash__")
            .with_tint(color)
            .with_alpha(1.0)
            .with_fade_speed(1.0 / duration.max(f32::EPSILON))
            .with_blend("additive".to_string())
            .auto_remove();
        // Immediately start fading out
        let mut o = overlay;
        o.target_alpha = 0.0;
        self.overlays.retain(|ov| ov.texture != "__flash__");
        self.overlays.push(o);
    }

    /// Queue a shake with default profile scaled by intensity.
    pub fn shake(&mut self, intensity: f32, duration: f32) {
        let profile = ShakeProfile::new(14.0, intensity * 0.15, 4.0)
            .with_duration(duration);
        self.command(DirectorCommand::StartShake(profile));
    }

    /// Convenience: fade to black over `duration`.
    pub fn fade_to_black(&mut self, duration: f32) {
        self.command(DirectorCommand::FadeToColor { color: [0.0, 0.0, 0.0, 1.0], duration });
    }

    /// Convenience: fade in from black over `duration`.
    pub fn fade_from_black(&mut self, duration: f32) {
        self.command(DirectorCommand::FadeIn { duration });
    }
}

// ── DirectorBuilder ───────────────────────────────────────────────────────────

/// Fluent builder for `CinematicDirector`.
pub struct DirectorBuilder {
    library: CutsceneLibrary,
    initial_fade_alpha: f32,
}

impl DirectorBuilder {
    pub fn new() -> Self {
        Self {
            library:            CutsceneLibrary::new(),
            initial_fade_alpha: 0.0,
        }
    }

    pub fn register(mut self, cutscene: Cutscene) -> Self {
        self.library.register(cutscene);
        self
    }

    pub fn start_faded(mut self) -> Self {
        self.initial_fade_alpha = 1.0;
        self
    }

    pub fn build(self) -> CinematicDirector {
        let mut dir = CinematicDirector::new(self.library);
        if self.initial_fade_alpha > 0.0 {
            dir.fade = Some(FadeState::fade_to(
                self.initial_fade_alpha,
                self.initial_fade_alpha,
                [0.0, 0.0, 0.0, 1.0],
                0.0,
            ));
        }
        dir
    }
}

impl Default for DirectorBuilder {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_director() -> CinematicDirector {
        let mut lib = CutsceneLibrary::new();
        lib.register(
            Cutscene::new("test_scene")
                .duration(5.0)
                .skippable(true)
                .letterbox(true)
        );
        lib.register(
            Cutscene::new("unskippable")
                .duration(3.0)
                .skippable(false)
        );
        CinematicDirector::new(lib)
    }

    #[test]
    fn director_plays_cutscene() {
        let mut dir = make_director();
        let events = dir.command(DirectorCommand::Play("test_scene".into()));
        assert!(events.iter().any(|e| matches!(e, CinematicEvent::Started { .. })));
        assert!(dir.is_playing());
    }

    #[test]
    fn director_plays_unknown_does_nothing() {
        let mut dir = make_director();
        let events = dir.command(DirectorCommand::Play("nope".into()));
        assert!(events.is_empty());
        assert!(dir.is_idle());
    }

    #[test]
    fn director_pause_resume() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        let ev = dir.command(DirectorCommand::Pause);
        assert!(ev.iter().any(|e| matches!(e, CinematicEvent::Paused { .. })));
        assert!(dir.is_paused());

        let ev2 = dir.command(DirectorCommand::Resume);
        assert!(ev2.iter().any(|e| matches!(e, CinematicEvent::Resumed { .. })));
        assert!(dir.is_playing());
    }

    #[test]
    fn director_skip_skippable() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        dir.command(DirectorCommand::Skip);
        // Tick until complete
        let mut events = Vec::new();
        for _ in 0..20 {
            events.extend(dir.tick(0.1));
        }
        assert!(events.iter().any(|e| matches!(e, CinematicEvent::Skipped { .. })));
    }

    #[test]
    fn director_skip_unskippable_ignored() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("unskippable".into()));
        dir.command(DirectorCommand::Skip);
        // Should still be playing (or paused), not skipping
        assert!(!matches!(dir.state, CutsceneState::Skipping { .. }));
    }

    #[test]
    fn director_completes_naturally() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        let mut completed = false;
        for _ in 0..100 {
            let events = dir.tick(0.1);
            if events.iter().any(|e| matches!(e, CinematicEvent::Completed { .. })) {
                completed = true;
                break;
            }
        }
        assert!(completed);
    }

    #[test]
    fn director_progress() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        dir.tick(1.0);
        let p = dir.progress();
        assert!(p > 0.0 && p <= 1.0, "progress={}", p);
    }

    #[test]
    fn director_marker_fires() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        dir.add_marker("mid", 2.0);
        let mut hit = false;
        for _ in 0..30 {
            let events = dir.tick(0.1);
            if events.iter().any(|e| matches!(e, CinematicEvent::MarkerReached(m) if m == "mid")) {
                hit = true;
                break;
            }
        }
        assert!(hit);
    }

    #[test]
    fn shake_profile_evaluate() {
        let profile = ShakeProfile::new(10.0, 0.5, 2.0);
        let offset = profile.evaluate(0.1, 1.23);
        assert!(offset.length() > 0.0);
    }

    #[test]
    fn shake_decays_to_zero() {
        let profile = ShakeProfile::new(10.0, 1.0, 5.0);
        let early = profile.evaluate(0.01, 0.0).length();
        let late  = profile.evaluate(5.0,  0.0).length();
        assert!(late < early, "early={} late={}", early, late);
    }

    #[test]
    fn fade_state_transitions() {
        let mut fade = FadeState::fade_in(1.0);
        assert!((fade.current_alpha() - 1.0).abs() < f32::EPSILON);
        fade.tick(0.5);
        assert!((fade.current_alpha() - 0.5).abs() < 0.01, "alpha={}", fade.current_alpha());
        fade.tick(0.5);
        assert!(fade.is_complete());
        assert!((fade.current_alpha() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn letterbox_animates() {
        let mut lb = LetterboxState::cinematic();
        lb.tick(1.0);
        assert!(lb.current_fraction > 0.0);
        // After enough ticks it should settle
        for _ in 0..100 { lb.tick(0.1); }
        assert!(lb.is_settled());
    }

    #[test]
    fn screen_overlay_fades() {
        let mut o = ScreenOverlay::new("vignette").with_alpha(1.0);
        o.fade_out();
        o.auto_remove = true;
        for _ in 0..20 { o.tick(0.1); }
        assert!(o.alpha < 0.01 || o.should_remove());
    }

    #[test]
    fn library_hot_reload() {
        let mut lib = CutsceneLibrary::new();
        let c = Cutscene::new("test").duration(3.0);
        lib.register(c.clone());
        let changed = lib.hot_reload(c);
        assert!(changed);
        assert_eq!(lib.len(), 1);
    }

    #[test]
    fn director_set_speed() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        dir.command(DirectorCommand::SetSpeed(2.0));
        if let CutsceneState::Playing { speed, .. } = &dir.state {
            assert!((*speed - 2.0).abs() < f32::EPSILON);
        } else {
            panic!("expected Playing state");
        }
    }

    #[test]
    fn director_jump_to() {
        let mut dir = make_director();
        dir.command(DirectorCommand::Play("test_scene".into()));
        dir.command(DirectorCommand::JumpTo(3.0));
        assert!((dir.state.elapsed() - 3.0).abs() < 0.01);
    }

    #[test]
    fn director_flash_adds_overlay() {
        let mut dir = make_director();
        dir.flash([1.0, 1.0, 1.0, 1.0], 0.3);
        assert!(!dir.overlays().is_empty());
    }

    #[test]
    fn cutscene_tags() {
        let mut lib = CutsceneLibrary::new();
        lib.register(Cutscene::new("a").tag("story"));
        lib.register(Cutscene::new("b").tag("story"));
        lib.register(Cutscene::new("c").tag("tutorial"));
        let story = lib.with_tag("story");
        assert_eq!(story.len(), 2);
    }

    #[test]
    fn builder_creates_director() {
        let dir = DirectorBuilder::new()
            .register(Cutscene::new("intro").duration(5.0))
            .start_faded()
            .build();
        assert!(dir.is_idle());
        assert!(dir.fade.is_some());
    }
}
