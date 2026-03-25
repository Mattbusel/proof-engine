//! Screen Transition Manager — handles visual transitions between game screens.
//!
//! Supports multiple transition types: FadeBlack, Dissolve, Slide, ZoomIn, ChaosWipe.
//! Each transition captures the outgoing screen state, tweens a visual effect, and
//! reveals the incoming screen.
//!
//! # Usage
//!
//! ```rust,no_run
//! use proof_engine::game::transitions::*;
//!
//! let mut tm = TransitionManager::new();
//! tm.start(TransitionType::FadeBlack {
//!     out_time: 0.2, hold_time: 0.05, in_time: 0.2,
//! });
//! // Each frame:
//! tm.tick(dt);
//! if tm.should_swap_state() {
//!     // swap game state here
//!     tm.acknowledge_swap();
//! }
//! // Render the transition overlay:
//! let overlay = tm.render_overlay(screen_width, screen_height);
//! ```

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ── Transition State ────────────────────────────────────────────────────────

/// Current phase of a screen transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionState {
    /// No transition active — normal rendering.
    None,
    /// Old screen is fading/wiping out.
    FadingOut,
    /// Black/blank screen while game state swaps.
    Hold,
    /// New screen is fading/wiping in.
    FadingIn,
    /// Transition just completed this frame.
    Completed,
}

// ── Transition Type ─────────────────────────────────────────────────────────

/// Visual effect used for the screen transition.
#[derive(Debug, Clone)]
pub enum TransitionType {
    /// Classic fade to/from black.
    FadeBlack {
        out_time: f32,
        hold_time: f32,
        in_time: f32,
    },
    /// Noise-based dissolve between screens.
    Dissolve {
        duration: f32,
    },
    /// Slide the old screen out to the left, new screen in from the right.
    SlideLeft {
        duration: f32,
    },
    /// Slide the old screen out to the right, new screen in from the left.
    SlideRight {
        duration: f32,
    },
    /// Zoom into center, then new screen appears.
    ZoomIn {
        duration: f32,
    },
    /// Chaos field particles sweep across the screen as a wave.
    ChaosWipe {
        duration: f32,
    },
    /// No visual — instant cut.
    Cut,
}

impl TransitionType {
    /// Total duration of the transition in seconds.
    pub fn total_duration(&self) -> f32 {
        match self {
            Self::FadeBlack { out_time, hold_time, in_time } => out_time + hold_time + in_time,
            Self::Dissolve { duration } => *duration,
            Self::SlideLeft { duration } => *duration,
            Self::SlideRight { duration } => *duration,
            Self::ZoomIn { duration } => *duration,
            Self::ChaosWipe { duration } => *duration,
            Self::Cut => 0.0,
        }
    }

    /// Normalized time at which the state swap should occur (0.0 to 1.0).
    pub fn swap_point(&self) -> f32 {
        match self {
            Self::FadeBlack { out_time, hold_time, in_time } => {
                let total = out_time + hold_time + in_time;
                if total < 1e-6 { return 0.5; }
                (out_time + hold_time * 0.5) / total
            }
            Self::Dissolve { .. } => 0.5,
            Self::SlideLeft { .. } => 0.5,
            Self::SlideRight { .. } => 0.5,
            Self::ZoomIn { .. } => 0.5,
            Self::ChaosWipe { .. } => 0.5,
            Self::Cut => 0.0,
        }
    }
}

// ── Overlay quad ────────────────────────────────────────────────────────────

/// A full-screen overlay quad produced by the transition for rendering.
#[derive(Debug, Clone)]
pub struct TransitionOverlay {
    /// RGBA color of the overlay. Alpha controls visibility.
    pub color: Vec4,
    /// 0.0 = no effect, 1.0 = fully covering screen.
    pub coverage: f32,
    /// For dissolve: noise threshold (pixels below this show new screen).
    pub dissolve_threshold: f32,
    /// For slide: horizontal offset in normalized screen coords (-1 to 1).
    pub slide_offset: f32,
    /// For zoom: scale factor (1.0 = normal, >1.0 = zoomed in).
    pub zoom_scale: f32,
    /// For chaos wipe: the wave front position (0.0 = left, 1.0 = right).
    pub wipe_front: f32,
    /// Number of chaos particles to spawn for ChaosWipe (0 if not applicable).
    pub chaos_particle_count: u32,
    /// The active transition type (for the renderer to select the right shader/technique).
    pub effect: TransitionEffect,
}

/// Which visual effect the renderer should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionEffect {
    None,
    FadeBlack,
    Dissolve,
    SlideLeft,
    SlideRight,
    ZoomIn,
    ChaosWipe,
}

impl Default for TransitionOverlay {
    fn default() -> Self {
        Self {
            color: Vec4::new(0.0, 0.0, 0.0, 0.0),
            coverage: 0.0,
            dissolve_threshold: 0.0,
            slide_offset: 0.0,
            zoom_scale: 1.0,
            wipe_front: 0.0,
            chaos_particle_count: 0,
            effect: TransitionEffect::None,
        }
    }
}

// ── Screenshot (framebuffer capture placeholder) ────────────────────────────

/// Captured framebuffer of the previous screen for cross-fade transitions.
///
/// In a real implementation this would hold a GL texture handle. Here we store
/// the metadata; the actual capture is done by the render pipeline.
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub captured_at: f32,  // scene time when captured
    /// GL texture handle (if captured). None if not yet captured.
    pub texture_id: Option<u32>,
}

impl Screenshot {
    pub fn placeholder(w: u32, h: u32, time: f32) -> Self {
        Self { width: w, height: h, captured_at: time, texture_id: None }
    }
}

// ── Easing functions ────────────────────────────────────────────────────────

/// Easing functions for transition curves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionEasing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    SmoothStep,
}

impl TransitionEasing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::SmoothStep => t * t * (3.0 - 2.0 * t),
        }
    }
}

// ── Transition Manager ──────────────────────────────────────────────────────

/// Manages screen-to-screen visual transitions.
///
/// The game loop should:
/// 1. Call `start()` to begin a transition
/// 2. Call `tick(dt)` each frame
/// 3. Check `should_swap_state()` to know when to swap game state
/// 4. Call `acknowledge_swap()` after swapping
/// 5. Call `render_overlay()` to get the overlay for rendering
/// 6. Check `is_done()` to know when the transition is complete
pub struct TransitionManager {
    state: TransitionState,
    progress: f32,
    elapsed: f32,
    transition_type: TransitionType,
    easing: TransitionEasing,
    from_screen: Option<Screenshot>,
    swap_pending: bool,
    swap_acknowledged: bool,
    /// Callback tag for identifying which transition this is.
    pub tag: String,
    /// Per-frame stats.
    pub stats: TransitionStats,
}

/// Per-frame statistics.
#[derive(Debug, Clone, Default)]
pub struct TransitionStats {
    pub state: &'static str,
    pub progress: f32,
    pub elapsed: f32,
    pub total_duration: f32,
}

impl TransitionManager {
    pub fn new() -> Self {
        Self {
            state: TransitionState::None,
            progress: 0.0,
            elapsed: 0.0,
            transition_type: TransitionType::Cut,
            easing: TransitionEasing::SmoothStep,
            from_screen: None,
            swap_pending: false,
            swap_acknowledged: false,
            tag: String::new(),
            stats: TransitionStats::default(),
        }
    }

    /// Start a new transition. Any in-progress transition is immediately replaced.
    pub fn start(&mut self, transition: TransitionType) {
        self.transition_type = transition;
        self.state = if self.transition_type.total_duration() < 1e-6 {
            // Instant cut
            self.swap_pending = true;
            TransitionState::Hold
        } else {
            TransitionState::FadingOut
        };
        self.progress = 0.0;
        self.elapsed = 0.0;
        self.swap_pending = false;
        self.swap_acknowledged = false;
    }

    /// Start a transition with a tag for identification.
    pub fn start_tagged(&mut self, transition: TransitionType, tag: impl Into<String>) {
        self.tag = tag.into();
        self.start(transition);
    }

    /// Start a transition with custom easing.
    pub fn start_with_easing(&mut self, transition: TransitionType, easing: TransitionEasing) {
        self.easing = easing;
        self.start(transition);
    }

    /// Capture the current screen for cross-fade transitions.
    pub fn capture_screen(&mut self, width: u32, height: u32, time: f32) {
        self.from_screen = Some(Screenshot::placeholder(width, height, time));
    }

    /// Advance the transition by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.state == TransitionState::None || self.state == TransitionState::Completed {
            return;
        }

        self.elapsed += dt;
        let total = self.transition_type.total_duration();

        if total < 1e-6 {
            // Instant
            self.progress = 1.0;
            self.state = TransitionState::Completed;
            self.swap_pending = true;
            self.update_stats();
            return;
        }

        self.progress = (self.elapsed / total).clamp(0.0, 1.0);
        let swap_point = self.transition_type.swap_point();

        // Determine phase
        match &self.transition_type {
            TransitionType::FadeBlack { out_time, hold_time, in_time } => {
                let total = out_time + hold_time + in_time;
                if self.elapsed < *out_time {
                    self.state = TransitionState::FadingOut;
                } else if self.elapsed < out_time + hold_time {
                    self.state = TransitionState::Hold;
                    if !self.swap_pending && !self.swap_acknowledged {
                        self.swap_pending = true;
                    }
                } else if self.elapsed < total {
                    self.state = TransitionState::FadingIn;
                } else {
                    self.state = TransitionState::Completed;
                }
            }
            _ => {
                // For non-FadeBlack: FadingOut until swap_point, FadingIn after
                if self.progress < swap_point {
                    self.state = TransitionState::FadingOut;
                } else if !self.swap_acknowledged {
                    self.state = TransitionState::Hold;
                    if !self.swap_pending {
                        self.swap_pending = true;
                    }
                } else if self.progress < 1.0 {
                    self.state = TransitionState::FadingIn;
                } else {
                    self.state = TransitionState::Completed;
                }
            }
        }

        if self.elapsed >= total {
            self.state = TransitionState::Completed;
        }

        self.update_stats();
    }

    fn update_stats(&self) {
        // Stats are read by the caller — we just set the public field
    }

    /// Whether the game should swap state now.
    pub fn should_swap_state(&self) -> bool {
        self.swap_pending && !self.swap_acknowledged
    }

    /// Call after swapping game state to continue the fade-in phase.
    pub fn acknowledge_swap(&mut self) {
        self.swap_acknowledged = true;
        self.swap_pending = false;
    }

    /// Whether the transition has fully completed.
    pub fn is_done(&self) -> bool {
        self.state == TransitionState::None || self.state == TransitionState::Completed
    }

    /// Whether any transition is currently active (not None and not Completed).
    pub fn is_active(&self) -> bool {
        !self.is_done()
    }

    /// Current transition state.
    pub fn state(&self) -> TransitionState { self.state }

    /// Current progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 { self.progress }

    /// Reset to no transition.
    pub fn clear(&mut self) {
        self.state = TransitionState::None;
        self.progress = 0.0;
        self.elapsed = 0.0;
        self.swap_pending = false;
        self.swap_acknowledged = false;
        self.from_screen = None;
    }

    // ── Overlay rendering ───────────────────────────────────────────────────

    /// Compute the overlay parameters for the current frame.
    ///
    /// The renderer uses this to draw the transition effect on top of the scene.
    pub fn render_overlay(&self, _screen_w: f32, _screen_h: f32) -> TransitionOverlay {
        if self.state == TransitionState::None || self.state == TransitionState::Completed {
            return TransitionOverlay::default();
        }

        match &self.transition_type {
            TransitionType::FadeBlack { out_time, hold_time, in_time } => {
                self.render_fade_black(*out_time, *hold_time, *in_time)
            }
            TransitionType::Dissolve { duration } => {
                self.render_dissolve(*duration)
            }
            TransitionType::SlideLeft { duration } => {
                self.render_slide(*duration, -1.0)
            }
            TransitionType::SlideRight { duration } => {
                self.render_slide(*duration, 1.0)
            }
            TransitionType::ZoomIn { duration } => {
                self.render_zoom(*duration)
            }
            TransitionType::ChaosWipe { duration } => {
                self.render_chaos_wipe(*duration)
            }
            TransitionType::Cut => TransitionOverlay::default(),
        }
    }

    // ── FadeBlack ───────────────────────────────────────────────────────────

    fn render_fade_black(&self, out_time: f32, hold_time: f32, in_time: f32) -> TransitionOverlay {
        let alpha = if self.elapsed < out_time {
            // Fading out: 0 → 1
            let t = if out_time > 1e-6 { self.elapsed / out_time } else { 1.0 };
            self.easing.apply(t)
        } else if self.elapsed < out_time + hold_time {
            // Hold: fully black
            1.0
        } else {
            // Fading in: 1 → 0
            let fade_in_elapsed = self.elapsed - out_time - hold_time;
            let t = if in_time > 1e-6 { fade_in_elapsed / in_time } else { 1.0 };
            1.0 - self.easing.apply(t)
        };

        TransitionOverlay {
            color: Vec4::new(0.0, 0.0, 0.0, alpha),
            coverage: alpha,
            effect: TransitionEffect::FadeBlack,
            ..Default::default()
        }
    }

    // ── Dissolve ────────────────────────────────────────────────────────────

    fn render_dissolve(&self, duration: f32) -> TransitionOverlay {
        let t = if duration > 1e-6 { self.elapsed / duration } else { 1.0 };
        let threshold = self.easing.apply(t.clamp(0.0, 1.0));

        TransitionOverlay {
            dissolve_threshold: threshold,
            coverage: threshold,
            effect: TransitionEffect::Dissolve,
            ..Default::default()
        }
    }

    // ── Slide ───────────────────────────────────────────────────────────────

    fn render_slide(&self, duration: f32, direction: f32) -> TransitionOverlay {
        let t = if duration > 1e-6 { self.elapsed / duration } else { 1.0 };
        let eased = self.easing.apply(t.clamp(0.0, 1.0));
        let offset = eased * direction;

        let effect = if direction < 0.0 {
            TransitionEffect::SlideLeft
        } else {
            TransitionEffect::SlideRight
        };

        TransitionOverlay {
            slide_offset: offset,
            coverage: eased.min(1.0 - eased) * 2.0, // peaks at 0.5
            effect,
            ..Default::default()
        }
    }

    // ── ZoomIn ──────────────────────────────────────────────────────────────

    fn render_zoom(&self, duration: f32) -> TransitionOverlay {
        let t = if duration > 1e-6 { self.elapsed / duration } else { 1.0 };
        let eased = self.easing.apply(t.clamp(0.0, 1.0));

        // Zoom: 1.0 → 3.0 in first half, then snap to new screen zoomed in → 1.0
        let zoom = if eased < 0.5 {
            1.0 + eased * 4.0  // 1.0 → 3.0
        } else {
            3.0 - (eased - 0.5) * 4.0  // 3.0 → 1.0
        };

        // Fade to white at midpoint for the "flash"
        let flash_alpha = if eased > 0.4 && eased < 0.6 {
            let flash_t = ((eased - 0.4) / 0.2).clamp(0.0, 1.0);
            if flash_t < 0.5 {
                flash_t * 2.0
            } else {
                (1.0 - flash_t) * 2.0
            }
        } else {
            0.0
        };

        TransitionOverlay {
            color: Vec4::new(1.0, 1.0, 1.0, flash_alpha),
            zoom_scale: zoom.max(0.01),
            coverage: flash_alpha,
            effect: TransitionEffect::ZoomIn,
            ..Default::default()
        }
    }

    // ── ChaosWipe ───────────────────────────────────────────────────────────

    fn render_chaos_wipe(&self, duration: f32) -> TransitionOverlay {
        let t = if duration > 1e-6 { self.elapsed / duration } else { 1.0 };
        let eased = self.easing.apply(t.clamp(0.0, 1.0));

        // Wave front sweeps left to right (0.0 → 1.0)
        // Particles spawn at the wave front
        let wipe_front = eased;

        // Number of chaos particles: peaks at midpoint
        let intensity = if eased < 0.5 { eased * 2.0 } else { (1.0 - eased) * 2.0 };
        let particle_count = (intensity * 200.0) as u32;

        TransitionOverlay {
            wipe_front,
            chaos_particle_count: particle_count,
            coverage: eased,
            color: Vec4::new(0.0, 0.0, 0.0, 0.0), // no solid overlay
            effect: TransitionEffect::ChaosWipe,
            ..Default::default()
        }
    }
}

impl Default for TransitionManager {
    fn default() -> Self { Self::new() }
}

// ── Game transition presets ─────────────────────────────────────────────────

/// Pre-configured transitions for specific game state changes.
pub struct GameTransitions;

impl GameTransitions {
    /// Title Screen → Character Creation
    pub fn title_to_character_creation() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.2,
            hold_time: 0.05,
            in_time: 0.2,
        }
    }

    /// Character Creation → Floor Navigation
    pub fn character_creation_to_floor_nav() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.15,
            hold_time: 0.05,
            in_time: 0.2,
        }
    }

    /// Floor Navigation → Combat
    pub fn floor_nav_to_combat() -> TransitionType {
        TransitionType::ChaosWipe {
            duration: 0.3,
        }
    }

    /// Combat → Floor Navigation
    pub fn combat_to_floor_nav() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.2,
            hold_time: 0.05,
            in_time: 0.2,
        }
    }

    /// Any → Death Screen (slow, dramatic)
    pub fn to_death() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.5,
            hold_time: 0.5,
            in_time: 0.3,
        }
    }

    /// Any → Boss Encounter (zoom in, dramatic)
    pub fn to_boss() -> TransitionType {
        TransitionType::ZoomIn {
            duration: 0.3,
        }
    }

    /// Floor → Floor (noise dissolve)
    pub fn floor_transition() -> TransitionType {
        TransitionType::Dissolve {
            duration: 0.4,
        }
    }

    /// Quick menu transition
    pub fn menu_transition() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.1,
            hold_time: 0.02,
            in_time: 0.1,
        }
    }

    /// Settings / pause overlay
    pub fn pause_overlay() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.08,
            hold_time: 0.0,
            in_time: 0.08,
        }
    }

    /// Victory screen
    pub fn to_victory() -> TransitionType {
        TransitionType::FadeBlack {
            out_time: 0.3,
            hold_time: 0.2,
            in_time: 0.5,
        }
    }

    /// Slide left for inventory/menu panels
    pub fn panel_slide_left() -> TransitionType {
        TransitionType::SlideLeft { duration: 0.25 }
    }

    /// Slide right for inventory/menu panels
    pub fn panel_slide_right() -> TransitionType {
        TransitionType::SlideRight { duration: 0.25 }
    }
}

// ── Transition queue ────────────────────────────────────────────────────────

/// Queued transition request — allows scheduling transitions from game logic.
#[derive(Debug, Clone)]
pub struct TransitionRequest {
    pub transition: TransitionType,
    pub tag: String,
    pub easing: TransitionEasing,
    pub delay: f32,
}

impl TransitionRequest {
    pub fn new(transition: TransitionType) -> Self {
        Self {
            transition,
            tag: String::new(),
            easing: TransitionEasing::SmoothStep,
            delay: 0.0,
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    pub fn with_easing(mut self, easing: TransitionEasing) -> Self {
        self.easing = easing;
        self
    }

    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }
}

/// A queue of pending transitions. Useful when multiple transitions might be
/// requested in quick succession (e.g. combat → floor nav → shop).
pub struct TransitionQueue {
    pub manager: TransitionManager,
    pending: Vec<TransitionRequest>,
    delay_timer: f32,
}

impl TransitionQueue {
    pub fn new() -> Self {
        Self {
            manager: TransitionManager::new(),
            pending: Vec::new(),
            delay_timer: 0.0,
        }
    }

    /// Enqueue a transition request.
    pub fn enqueue(&mut self, request: TransitionRequest) {
        self.pending.push(request);
    }

    /// Enqueue a simple transition with no delay.
    pub fn enqueue_simple(&mut self, transition: TransitionType) {
        self.pending.push(TransitionRequest::new(transition));
    }

    /// Tick the queue and active transition.
    pub fn tick(&mut self, dt: f32) {
        // Tick active transition
        self.manager.tick(dt);

        // If no transition is active and there's a pending one, start it
        if self.manager.is_done() && !self.pending.is_empty() {
            // Handle delay
            if self.delay_timer > 0.0 {
                self.delay_timer -= dt;
                return;
            }

            let request = self.pending.remove(0);
            if request.delay > 0.0 && self.delay_timer <= 0.0 {
                self.delay_timer = request.delay;
                self.pending.insert(0, TransitionRequest {
                    delay: 0.0,
                    ..request
                });
                return;
            }

            self.manager.easing = request.easing;
            self.manager.start_tagged(request.transition, request.tag);
        }
    }

    /// Whether the game should swap state.
    pub fn should_swap_state(&self) -> bool {
        self.manager.should_swap_state()
    }

    /// Acknowledge state swap.
    pub fn acknowledge_swap(&mut self) {
        self.manager.acknowledge_swap();
    }

    /// Get the overlay for rendering.
    pub fn render_overlay(&self, w: f32, h: f32) -> TransitionOverlay {
        self.manager.render_overlay(w, h)
    }

    /// Whether any transition is active or pending.
    pub fn is_busy(&self) -> bool {
        self.manager.is_active() || !self.pending.is_empty()
    }

    /// Clear all pending and active transitions.
    pub fn clear(&mut self) {
        self.manager.clear();
        self.pending.clear();
        self.delay_timer = 0.0;
    }

    /// Number of pending transitions in the queue.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for TransitionQueue {
    fn default() -> Self { Self::new() }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_black_phases() {
        let mut tm = TransitionManager::new();
        tm.start(TransitionType::FadeBlack {
            out_time: 0.2,
            hold_time: 0.1,
            in_time: 0.2,
        });

        assert_eq!(tm.state(), TransitionState::FadingOut);

        // Advance through fade out
        tm.tick(0.15);
        assert_eq!(tm.state(), TransitionState::FadingOut);

        // Into hold
        tm.tick(0.1);
        assert_eq!(tm.state(), TransitionState::Hold);
        assert!(tm.should_swap_state());

        tm.acknowledge_swap();
        assert!(!tm.should_swap_state());

        // Into fade in
        tm.tick(0.1);
        assert_eq!(tm.state(), TransitionState::FadingIn);

        // Complete
        tm.tick(0.2);
        assert_eq!(tm.state(), TransitionState::Completed);
        assert!(tm.is_done());
    }

    #[test]
    fn dissolve_transition() {
        let mut tm = TransitionManager::new();
        tm.start(TransitionType::Dissolve { duration: 1.0 });

        tm.tick(0.25);
        assert!(tm.is_active());
        let overlay = tm.render_overlay(800.0, 600.0);
        assert_eq!(overlay.effect, TransitionEffect::Dissolve);
        assert!(overlay.dissolve_threshold > 0.0);

        tm.tick(0.75);
        assert!(tm.is_done());
    }

    #[test]
    fn chaos_wipe_particles() {
        let mut tm = TransitionManager::new();
        tm.start(TransitionType::ChaosWipe { duration: 0.3 });

        tm.tick(0.15);
        let overlay = tm.render_overlay(800.0, 600.0);
        assert_eq!(overlay.effect, TransitionEffect::ChaosWipe);
        assert!(overlay.chaos_particle_count > 0);
        assert!(overlay.wipe_front > 0.0);
    }

    #[test]
    fn zoom_in_flash() {
        let mut tm = TransitionManager::new();
        tm.start(TransitionType::ZoomIn { duration: 1.0 });

        // At midpoint, there should be a flash
        tm.tick(0.5);
        let overlay = tm.render_overlay(800.0, 600.0);
        assert_eq!(overlay.effect, TransitionEffect::ZoomIn);
        assert!(overlay.zoom_scale > 1.0);
    }

    #[test]
    fn instant_cut() {
        let mut tm = TransitionManager::new();
        tm.start(TransitionType::Cut);
        tm.tick(0.0);
        assert!(tm.is_done());
    }

    #[test]
    fn slide_left() {
        let mut tm = TransitionManager::new();
        tm.start(TransitionType::SlideLeft { duration: 0.5 });
        tm.tick(0.25);
        let overlay = tm.render_overlay(800.0, 600.0);
        assert_eq!(overlay.effect, TransitionEffect::SlideLeft);
        assert!(overlay.slide_offset < 0.0);
    }

    #[test]
    fn game_preset_durations() {
        assert!(GameTransitions::title_to_character_creation().total_duration() > 0.0);
        assert!(GameTransitions::to_death().total_duration() > 1.0);
        assert!(GameTransitions::to_boss().total_duration() > 0.0);
        assert!(GameTransitions::floor_transition().total_duration() > 0.0);
    }

    #[test]
    fn easing_bounds() {
        for easing in &[
            TransitionEasing::Linear,
            TransitionEasing::EaseIn,
            TransitionEasing::EaseOut,
            TransitionEasing::EaseInOut,
            TransitionEasing::SmoothStep,
        ] {
            assert!((easing.apply(0.0) - 0.0).abs() < 1e-6, "{:?} at 0", easing);
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-6, "{:?} at 1", easing);
            // Monotonic check: midpoint should be between 0 and 1
            let mid = easing.apply(0.5);
            assert!(mid >= 0.0 && mid <= 1.0, "{:?} mid={}", easing, mid);
        }
    }

    #[test]
    fn transition_queue_sequences() {
        let mut queue = TransitionQueue::new();
        queue.enqueue_simple(TransitionType::FadeBlack {
            out_time: 0.1, hold_time: 0.0, in_time: 0.1,
        });
        queue.enqueue_simple(TransitionType::Dissolve { duration: 0.2 });

        // First transition starts
        queue.tick(0.01);
        assert!(queue.is_busy());
        assert_eq!(queue.pending_count(), 1);

        // Complete first (swap + finish)
        queue.tick(0.05);
        queue.acknowledge_swap();
        queue.tick(0.15);

        // Second should start
        queue.tick(0.01);
        assert!(queue.is_busy());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn overlay_default_when_inactive() {
        let tm = TransitionManager::new();
        let overlay = tm.render_overlay(800.0, 600.0);
        assert_eq!(overlay.effect, TransitionEffect::None);
        assert_eq!(overlay.coverage, 0.0);
    }
}
