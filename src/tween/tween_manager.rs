//! TweenManager — central hub for managing active tweens across the entire game.
//!
//! The TweenManager owns a pool of `ActiveTween`s, each targeting a specific
//! property in the scene (glyph position, camera FOV, bar fill, etc.).
//! Every frame, `tick(dt)` advances all tweens and applies their values.
//!
//! Features:
//!   - Start / start_delayed / cancel by ID
//!   - TweenTarget enum covering glyphs, camera, screen, bars, and custom lambdas
//!   - Chaining: on_complete callbacks that can start new tweens
//!   - Group cancellation by tag
//!   - Automatic cleanup of completed tweens

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

use super::easing::Easing;
use super::{Tween, TweenState, Lerp};
use crate::glyph::GlyphId;

// ── TweenId ─────────────────────────────────────────────────────────────────

/// Opaque handle to a running tween.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TweenId(pub u64);

// ── BarId ───────────────────────────────────────────────────────────────────

/// Opaque handle to a UI bar (HP, MP, XP, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BarId(pub u32);

// ── TweenTarget ─────────────────────────────────────────────────────────────

/// What property an active tween drives.
pub enum TweenTarget {
    // ── Glyph properties ────────────────────────────────────────────────
    GlyphPositionX(GlyphId),
    GlyphPositionY(GlyphId),
    GlyphPositionZ(GlyphId),
    GlyphScale(GlyphId),
    GlyphScaleX(GlyphId),
    GlyphScaleY(GlyphId),
    GlyphAlpha(GlyphId),
    GlyphEmission(GlyphId),
    GlyphRotation(GlyphId),
    GlyphColorR(GlyphId),
    GlyphColorG(GlyphId),
    GlyphColorB(GlyphId),
    GlyphGlowRadius(GlyphId),
    GlyphTemperature(GlyphId),
    GlyphEntropy(GlyphId),

    // ── Camera ──────────────────────────────────────────────────────────
    CameraFov,
    CameraPositionX,
    CameraPositionY,
    CameraPositionZ,
    CameraTargetX,
    CameraTargetY,
    CameraTargetZ,
    CameraTrauma,

    // ── Screen ──────────────────────────────────────────────────────────
    ScreenFade,
    ScreenShake,
    ScreenBloom,
    ScreenChromaticAberration,
    ScreenVignette,
    ScreenSaturation,
    ScreenHueShift,

    // ── UI Bars ─────────────────────────────────────────────────────────
    BarFillPercent(BarId),
    BarGhostPercent(BarId),

    // ── Custom ──────────────────────────────────────────────────────────
    /// A custom target driven by a closure. The closure receives the current
    /// tween value each frame.
    Custom(Box<dyn FnMut(f32) + Send>),

    /// Named float property — stored in a shared value map.
    Named(String),
}

// ── ActiveTween ─────────────────────────────────────────────────────────────

/// A single tween that is currently running or waiting (delayed).
pub struct ActiveTween {
    pub id: TweenId,
    pub target: TweenTarget,
    pub state: TweenState<f32>,
    /// Optional delay before the tween starts.
    pub delay_remaining: f32,
    /// Optional tag for group operations (e.g., "combat", "menu", "screen").
    pub tag: Option<String>,
    /// Callback invoked when the tween completes.
    pub on_complete: Option<Box<dyn FnOnce(&mut TweenManager) + Send>>,
    /// If true, the tween has been cancelled and should be removed.
    pub cancelled: bool,
}

// ── TweenManager ────────────────────────────────────────────────────────────

/// Central tween manager. Owns all active tweens and provides the game-facing API.
pub struct TweenManager {
    /// Active tweens, keyed by ID for fast lookup.
    tweens: Vec<ActiveTween>,
    /// Monotonically increasing ID counter.
    next_id: u64,
    /// Named float values driven by `TweenTarget::Named`.
    pub named_values: HashMap<String, f32>,
    /// Screen-level values that the renderer can read each frame.
    pub screen_fade: f32,
    pub screen_shake: f32,
    pub screen_bloom_override: Option<f32>,
    pub screen_chromatic_override: Option<f32>,
    pub screen_vignette_override: Option<f32>,
    pub screen_saturation_override: Option<f32>,
    pub screen_hue_shift: f32,
    /// Bar fill values, keyed by BarId.
    pub bar_values: HashMap<BarId, f32>,
    pub bar_ghost_values: HashMap<BarId, f32>,
    /// Camera overrides (None = no override, renderer uses defaults).
    pub camera_fov_override: Option<f32>,
    pub camera_position_override: [Option<f32>; 3],
    pub camera_target_override: [Option<f32>; 3],
    pub camera_trauma_add: f32,
}

impl TweenManager {
    pub fn new() -> Self {
        Self {
            tweens: Vec::with_capacity(256),
            next_id: 1,
            named_values: HashMap::new(),
            screen_fade: 0.0,
            screen_shake: 0.0,
            screen_bloom_override: None,
            screen_chromatic_override: None,
            screen_vignette_override: None,
            screen_saturation_override: None,
            screen_hue_shift: 0.0,
            bar_values: HashMap::new(),
            bar_ghost_values: HashMap::new(),
            camera_fov_override: None,
            camera_position_override: [None; 3],
            camera_target_override: [None; 3],
            camera_trauma_add: 0.0,
        }
    }

    // ── Starting tweens ─────────────────────────────────────────────────

    /// Start a tween immediately.
    pub fn start(
        &mut self,
        target: TweenTarget,
        from: f32,
        to: f32,
        duration: f32,
        easing: Easing,
    ) -> TweenId {
        self.start_inner(target, from, to, duration, easing, 0.0, None, None)
    }

    /// Start a tween with a delay.
    pub fn start_delayed(
        &mut self,
        target: TweenTarget,
        from: f32,
        to: f32,
        duration: f32,
        delay: f32,
        easing: Easing,
    ) -> TweenId {
        self.start_inner(target, from, to, duration, easing, delay, None, None)
    }

    /// Start a tween with a completion callback.
    pub fn start_with_callback(
        &mut self,
        target: TweenTarget,
        from: f32,
        to: f32,
        duration: f32,
        easing: Easing,
        on_complete: impl FnOnce(&mut TweenManager) + Send + 'static,
    ) -> TweenId {
        self.start_inner(target, from, to, duration, easing, 0.0, None, Some(Box::new(on_complete)))
    }

    /// Start a tagged tween (for group cancellation).
    pub fn start_tagged(
        &mut self,
        target: TweenTarget,
        from: f32,
        to: f32,
        duration: f32,
        easing: Easing,
        tag: &str,
    ) -> TweenId {
        self.start_inner(target, from, to, duration, easing, 0.0, Some(tag.to_string()), None)
    }

    /// Full-featured tween start.
    fn start_inner(
        &mut self,
        target: TweenTarget,
        from: f32,
        to: f32,
        duration: f32,
        easing: Easing,
        delay: f32,
        tag: Option<String>,
        on_complete: Option<Box<dyn FnOnce(&mut TweenManager) + Send>>,
    ) -> TweenId {
        let id = TweenId(self.next_id);
        self.next_id += 1;

        let tween = Tween::new(from, to, duration, easing);
        let state = TweenState::new(tween);

        self.tweens.push(ActiveTween {
            id,
            target,
            state,
            delay_remaining: delay,
            tag,
            on_complete,
            cancelled: false,
        });

        id
    }

    // ── Cancellation ────────────────────────────────────────────────────

    /// Cancel a specific tween by ID.
    pub fn cancel(&mut self, id: TweenId) {
        for t in &mut self.tweens {
            if t.id == id {
                t.cancelled = true;
                break;
            }
        }
    }

    /// Cancel all tweens with a given tag.
    pub fn cancel_tag(&mut self, tag: &str) {
        for t in &mut self.tweens {
            if t.tag.as_deref() == Some(tag) {
                t.cancelled = true;
            }
        }
    }

    /// Cancel all tweens targeting a specific glyph.
    pub fn cancel_glyph(&mut self, glyph_id: GlyphId) {
        for t in &mut self.tweens {
            let targets_glyph = matches!(
                &t.target,
                TweenTarget::GlyphPositionX(id)
                | TweenTarget::GlyphPositionY(id)
                | TweenTarget::GlyphPositionZ(id)
                | TweenTarget::GlyphScale(id)
                | TweenTarget::GlyphScaleX(id)
                | TweenTarget::GlyphScaleY(id)
                | TweenTarget::GlyphAlpha(id)
                | TweenTarget::GlyphEmission(id)
                | TweenTarget::GlyphRotation(id)
                | TweenTarget::GlyphColorR(id)
                | TweenTarget::GlyphColorG(id)
                | TweenTarget::GlyphColorB(id)
                | TweenTarget::GlyphGlowRadius(id)
                | TweenTarget::GlyphTemperature(id)
                | TweenTarget::GlyphEntropy(id)
                    if *id == glyph_id
            );
            if targets_glyph {
                t.cancelled = true;
            }
        }
    }

    /// Cancel all active tweens.
    pub fn cancel_all(&mut self) {
        for t in &mut self.tweens {
            t.cancelled = true;
        }
    }

    // ── Queries ─────────────────────────────────────────────────────────

    /// Allocate a new ID and push a raw ActiveTween. Used by game_tweens presets.
    pub fn push_raw(&mut self, target: TweenTarget, state: super::TweenState<f32>, delay: f32, tag: Option<String>, on_complete: Option<Box<dyn FnOnce(&mut TweenManager) + Send>>) -> TweenId {
        let id = TweenId(self.next_id);
        self.next_id += 1;
        self.tweens.push(ActiveTween {
            id, target, state,
            delay_remaining: delay,
            tag,
            on_complete,
            cancelled: false,
        });
        id
    }

    /// Check if a tween is still active.
    pub fn is_active(&self, id: TweenId) -> bool {
        self.tweens.iter().any(|t| t.id == id && !t.cancelled && !t.state.done)
    }

    /// Number of active tweens.
    pub fn active_count(&self) -> usize {
        self.tweens.iter().filter(|t| !t.cancelled && !t.state.done).count()
    }

    /// Get a named value (returns 0.0 if not set).
    pub fn get_named(&self, name: &str) -> f32 {
        self.named_values.get(name).copied().unwrap_or(0.0)
    }

    /// Get a bar fill value.
    pub fn get_bar(&self, bar: BarId) -> f32 {
        self.bar_values.get(&bar).copied().unwrap_or(0.0)
    }

    /// Get a bar ghost value.
    pub fn get_bar_ghost(&self, bar: BarId) -> f32 {
        self.bar_ghost_values.get(&bar).copied().unwrap_or(0.0)
    }

    // ── Tick ────────────────────────────────────────────────────────────

    /// Advance all tweens by `dt` seconds.
    ///
    /// This method:
    /// 1. Decrements delays on waiting tweens
    /// 2. Ticks active tweens and reads their current value
    /// 3. Applies each value to its target (stored in the manager's output fields)
    /// 4. Collects completed tweens and runs their on_complete callbacks
    /// 5. Removes completed/cancelled tweens
    ///
    /// **Important**: This does NOT directly modify the scene. The caller must
    /// read the manager's output fields (`screen_fade`, `bar_values`, etc.)
    /// and apply them to the engine/scene each frame.
    pub fn tick(&mut self, dt: f32) {
        // Reset per-frame accumulators.
        self.camera_trauma_add = 0.0;

        // Collect completed callback indices.
        let mut completed_callbacks: Vec<Box<dyn FnOnce(&mut TweenManager) + Send>> = Vec::new();

        for tween in &mut self.tweens {
            if tween.cancelled {
                continue;
            }

            // Handle delay.
            if tween.delay_remaining > 0.0 {
                tween.delay_remaining -= dt;
                if tween.delay_remaining > 0.0 {
                    continue;
                }
                // Overflow the delay into the tween.
                let overflow = -tween.delay_remaining;
                tween.delay_remaining = 0.0;
                tween.state.tick(overflow);
            } else {
                tween.state.tick(dt);
            }

            let value = tween.state.value();

            // Apply value to target.
            match &mut tween.target {
                TweenTarget::GlyphPositionX(_)
                | TweenTarget::GlyphPositionY(_)
                | TweenTarget::GlyphPositionZ(_)
                | TweenTarget::GlyphScale(_)
                | TweenTarget::GlyphScaleX(_)
                | TweenTarget::GlyphScaleY(_)
                | TweenTarget::GlyphAlpha(_)
                | TweenTarget::GlyphEmission(_)
                | TweenTarget::GlyphRotation(_)
                | TweenTarget::GlyphColorR(_)
                | TweenTarget::GlyphColorG(_)
                | TweenTarget::GlyphColorB(_)
                | TweenTarget::GlyphGlowRadius(_)
                | TweenTarget::GlyphTemperature(_)
                | TweenTarget::GlyphEntropy(_) => {
                    // Glyph targets are applied externally by the caller via apply_to_scene().
                }

                TweenTarget::CameraFov => {
                    self.camera_fov_override = Some(value);
                }
                TweenTarget::CameraPositionX => {
                    self.camera_position_override[0] = Some(value);
                }
                TweenTarget::CameraPositionY => {
                    self.camera_position_override[1] = Some(value);
                }
                TweenTarget::CameraPositionZ => {
                    self.camera_position_override[2] = Some(value);
                }
                TweenTarget::CameraTargetX => {
                    self.camera_target_override[0] = Some(value);
                }
                TweenTarget::CameraTargetY => {
                    self.camera_target_override[1] = Some(value);
                }
                TweenTarget::CameraTargetZ => {
                    self.camera_target_override[2] = Some(value);
                }
                TweenTarget::CameraTrauma => {
                    self.camera_trauma_add = value;
                }

                TweenTarget::ScreenFade => {
                    self.screen_fade = value;
                }
                TweenTarget::ScreenShake => {
                    self.screen_shake = value;
                }
                TweenTarget::ScreenBloom => {
                    self.screen_bloom_override = Some(value);
                }
                TweenTarget::ScreenChromaticAberration => {
                    self.screen_chromatic_override = Some(value);
                }
                TweenTarget::ScreenVignette => {
                    self.screen_vignette_override = Some(value);
                }
                TweenTarget::ScreenSaturation => {
                    self.screen_saturation_override = Some(value);
                }
                TweenTarget::ScreenHueShift => {
                    self.screen_hue_shift = value;
                }

                TweenTarget::BarFillPercent(bar_id) => {
                    self.bar_values.insert(*bar_id, value);
                }
                TweenTarget::BarGhostPercent(bar_id) => {
                    self.bar_ghost_values.insert(*bar_id, value);
                }

                TweenTarget::Custom(ref mut f) => {
                    f(value);
                }

                TweenTarget::Named(ref name) => {
                    self.named_values.insert(name.clone(), value);
                }
            }

            // Check completion.
            if tween.state.done {
                if let Some(cb) = tween.on_complete.take() {
                    completed_callbacks.push(cb);
                }
            }
        }

        // Remove completed/cancelled tweens.
        self.tweens.retain(|t| !t.cancelled && !t.state.done);

        // Run completion callbacks (these may start new tweens).
        for cb in completed_callbacks {
            cb(self);
        }
    }

    /// Apply glyph-targeting tweens to the scene's glyph pool.
    ///
    /// Call this after `tick()` and before rendering. The caller provides
    /// a mutable closure that can look up and modify glyphs by ID.
    pub fn apply_to_glyphs<F>(&self, mut apply: F)
    where
        F: FnMut(GlyphId, &str, f32),
    {
        for tween in &self.tweens {
            if tween.cancelled || tween.delay_remaining > 0.0 {
                continue;
            }
            let value = tween.state.value();
            match &tween.target {
                TweenTarget::GlyphPositionX(id) => apply(*id, "position_x", value),
                TweenTarget::GlyphPositionY(id) => apply(*id, "position_y", value),
                TweenTarget::GlyphPositionZ(id) => apply(*id, "position_z", value),
                TweenTarget::GlyphScale(id) => apply(*id, "scale", value),
                TweenTarget::GlyphScaleX(id) => apply(*id, "scale_x", value),
                TweenTarget::GlyphScaleY(id) => apply(*id, "scale_y", value),
                TweenTarget::GlyphAlpha(id) => apply(*id, "alpha", value),
                TweenTarget::GlyphEmission(id) => apply(*id, "emission", value),
                TweenTarget::GlyphRotation(id) => apply(*id, "rotation", value),
                TweenTarget::GlyphColorR(id) => apply(*id, "color_r", value),
                TweenTarget::GlyphColorG(id) => apply(*id, "color_g", value),
                TweenTarget::GlyphColorB(id) => apply(*id, "color_b", value),
                TweenTarget::GlyphGlowRadius(id) => apply(*id, "glow_radius", value),
                TweenTarget::GlyphTemperature(id) => apply(*id, "temperature", value),
                TweenTarget::GlyphEntropy(id) => apply(*id, "entropy", value),
                _ => {}
            }
        }
    }

    /// Reset all screen/camera overrides. Call at the start of each frame
    /// before `tick()` if you want tweens to be the sole source of overrides.
    pub fn reset_overrides(&mut self) {
        self.screen_bloom_override = None;
        self.screen_chromatic_override = None;
        self.screen_vignette_override = None;
        self.screen_saturation_override = None;
        self.screen_hue_shift = 0.0;
        self.camera_fov_override = None;
        self.camera_position_override = [None; 3];
        self.camera_target_override = [None; 3];
    }
}

impl Default for TweenManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_tick() {
        let mut mgr = TweenManager::new();
        let id = mgr.start(TweenTarget::ScreenFade, 0.0, 1.0, 1.0, Easing::Linear);
        assert!(mgr.is_active(id));
        mgr.tick(0.5);
        assert!((mgr.screen_fade - 0.5).abs() < 0.05);
        mgr.tick(0.6);
        assert!(!mgr.is_active(id));
        assert!((mgr.screen_fade - 1.0).abs() < 0.05);
    }

    #[test]
    fn delayed_start() {
        let mut mgr = TweenManager::new();
        mgr.start_delayed(TweenTarget::ScreenFade, 0.0, 1.0, 1.0, 0.5, Easing::Linear);
        mgr.tick(0.3);
        assert!((mgr.screen_fade - 0.0).abs() < 0.01, "Should still be in delay");
        mgr.tick(0.3); // 0.6 total, 0.1 past delay
        assert!(mgr.screen_fade > 0.0, "Should have started");
    }

    #[test]
    fn cancel_by_id() {
        let mut mgr = TweenManager::new();
        let id = mgr.start(TweenTarget::ScreenFade, 0.0, 1.0, 1.0, Easing::Linear);
        mgr.cancel(id);
        mgr.tick(0.5);
        assert!(!mgr.is_active(id));
    }

    #[test]
    fn cancel_by_tag() {
        let mut mgr = TweenManager::new();
        mgr.start_tagged(TweenTarget::ScreenFade, 0.0, 1.0, 1.0, Easing::Linear, "combat");
        mgr.start_tagged(TweenTarget::ScreenShake, 0.0, 1.0, 1.0, Easing::Linear, "combat");
        mgr.start_tagged(TweenTarget::ScreenBloom, 0.0, 1.0, 1.0, Easing::Linear, "menu");
        assert_eq!(mgr.active_count(), 3);
        mgr.cancel_tag("combat");
        mgr.tick(0.0);
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn bar_values() {
        let mut mgr = TweenManager::new();
        let bar = BarId(0);
        mgr.start(TweenTarget::BarFillPercent(bar), 1.0, 0.5, 0.5, Easing::Linear);
        mgr.tick(0.25);
        let val = mgr.get_bar(bar);
        assert!((val - 0.75).abs() < 0.05);
    }

    #[test]
    fn named_values() {
        let mut mgr = TweenManager::new();
        mgr.start(TweenTarget::Named("test".to_string()), 0.0, 10.0, 1.0, Easing::Linear);
        mgr.tick(0.5);
        assert!((mgr.get_named("test") - 5.0).abs() < 0.5);
    }

    #[test]
    fn callback_fires_on_complete() {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        let mut mgr = TweenManager::new();
        mgr.start_with_callback(
            TweenTarget::ScreenFade, 0.0, 1.0, 0.1, Easing::Linear,
            move |_mgr| { fired_clone.store(true, Ordering::SeqCst); },
        );
        mgr.tick(0.2);
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn callback_can_chain_tweens() {
        let mut mgr = TweenManager::new();
        mgr.start_with_callback(
            TweenTarget::ScreenFade, 0.0, 1.0, 0.1, Easing::Linear,
            |mgr| {
                mgr.start(TweenTarget::ScreenFade, 1.0, 0.0, 0.1, Easing::Linear);
            },
        );
        mgr.tick(0.15); // Complete first, start second
        assert!(mgr.active_count() >= 1);
    }
}
