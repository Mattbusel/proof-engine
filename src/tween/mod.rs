//! Tween / animation system.
//!
//! Provides typed interpolation between values, 30+ easing functions,
//! multi-track keyframe timelines, and composable animation sequences.
//! Every interpolation can be driven by a `MathFunction` instead of a simple t ∈ [0,1].
//!
//! # Quick start
//!
//! ```rust,no_run
//! use proof_engine::tween::{Tween, Easing};
//! use glam::Vec3;
//!
//! let tween = Tween::new(Vec3::ZERO, Vec3::ONE, 2.0, Easing::EaseInOutCubic);
//! let pos = tween.sample(1.0); // halfway through → roughly Vec3(0.5, 0.5, 0.5)
//! ```

pub mod easing;
pub mod sequence;
pub mod keyframe;
pub mod tween_manager;
pub mod game_tweens;

pub use easing::Easing;
pub use sequence::{TweenSequence, SequenceBuilder};
pub use keyframe::{KeyframeTrack, Keyframe};

use glam::{Vec2, Vec3, Vec4};

// ── Lerp trait ─────────────────────────────────────────────────────────────────

/// Values that can be linearly interpolated.
pub trait Lerp: Clone {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
    fn zero() -> Self;
}

impl Lerp for f32 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self { a + (b - a) * t }
    fn zero() -> Self { 0.0 }
}

impl Lerp for Vec2 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self { *a + (*b - *a) * t }
    fn zero() -> Self { Vec2::ZERO }
}

impl Lerp for Vec3 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self { *a + (*b - *a) * t }
    fn zero() -> Self { Vec3::ZERO }
}

impl Lerp for Vec4 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self { *a + (*b - *a) * t }
    fn zero() -> Self { Vec4::ZERO }
}

// ── Tween<T> ──────────────────────────────────────────────────────────────────

/// A single interpolation from `from` to `to` over `duration` seconds.
#[derive(Clone, Debug)]
pub struct Tween<T: Lerp + std::fmt::Debug> {
    pub from:     T,
    pub to:       T,
    pub duration: f32,
    pub easing:   Easing,
    pub delay:    f32,
    /// Automatically reverse and repeat (`yoyo` mode). Negative for infinite.
    pub repeat:   i32,
    /// Whether to yoyo (ping-pong) on repeat.
    pub yoyo:     bool,
}

impl<T: Lerp + std::fmt::Debug> Tween<T> {
    pub fn new(from: T, to: T, duration: f32, easing: Easing) -> Self {
        Self { from, to, duration, easing, delay: 0.0, repeat: 0, yoyo: false }
    }

    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }

    pub fn with_repeat(mut self, repeat: i32, yoyo: bool) -> Self {
        self.repeat = repeat;
        self.yoyo = yoyo;
        self
    }

    /// Sample the tween at `time` seconds from the start.
    ///
    /// Returns the interpolated value. After duration + delay, clamps to `to`
    /// unless repeat is set.
    pub fn sample(&self, time: f32) -> T {
        let t = ((time - self.delay) / self.duration.max(f32::EPSILON)).clamp(0.0, 1.0);
        let raw_t = self.easing.apply(t);
        T::lerp(&self.from, &self.to, raw_t)
    }

    /// Sample with repeat/yoyo handling.
    pub fn sample_looped(&self, time: f32) -> T {
        let local = (time - self.delay).max(0.0);
        let period = self.duration.max(f32::EPSILON);
        let cycle = (local / period) as i32;

        // Check if we've exceeded repeat count
        if self.repeat >= 0 && cycle > self.repeat {
            return if self.yoyo && self.repeat % 2 == 1 {
                T::lerp(&self.to, &self.from, self.easing.apply(1.0))
            } else {
                T::lerp(&self.from, &self.to, self.easing.apply(1.0))
            };
        }

        let frac = (local / period).fract();
        let (a, b) = if self.yoyo && cycle % 2 == 1 {
            (&self.to, &self.from)
        } else {
            (&self.from, &self.to)
        };
        T::lerp(a, b, self.easing.apply(frac))
    }

    /// Returns true if the tween has finished (after duration + delay, accounting for repeat).
    pub fn is_complete(&self, time: f32) -> bool {
        let local = (time - self.delay).max(0.0);
        if self.repeat < 0 { return false; }
        local >= self.duration * (self.repeat as f32 + 1.0)
    }

    /// Total duration including all repeats and delay.
    pub fn total_duration(&self) -> f32 {
        if self.repeat < 0 { f32::INFINITY }
        else { self.delay + self.duration * (self.repeat as f32 + 1.0) }
    }
}

// ── Specialized constructors ───────────────────────────────────────────────────

/// Convenience methods for common tween patterns.
pub struct Tweens;

impl Tweens {
    pub fn fade_in(duration: f32) -> Tween<f32> {
        Tween::new(0.0, 1.0, duration, Easing::EaseInQuad)
    }

    pub fn fade_out(duration: f32) -> Tween<f32> {
        Tween::new(1.0, 0.0, duration, Easing::EaseOutQuad)
    }

    pub fn bounce_in(from: Vec3, to: Vec3, duration: f32) -> Tween<Vec3> {
        Tween::new(from, to, duration, Easing::EaseOutBounce)
    }

    pub fn elastic_pop(from: f32, to: f32, duration: f32) -> Tween<f32> {
        Tween::new(from, to, duration, Easing::EaseOutElastic)
    }

    pub fn camera_slide(from: Vec3, to: Vec3, duration: f32) -> Tween<Vec3> {
        Tween::new(from, to, duration, Easing::EaseInOutCubic)
    }

    pub fn damage_number_rise(origin: Vec3, height: f32, duration: f32) -> Tween<Vec3> {
        Tween::new(origin, origin + Vec3::Y * height, duration, Easing::EaseOutCubic)
    }

    pub fn color_flash(base: Vec4, flash: Vec4, duration: f32) -> Tween<Vec4> {
        Tween::new(flash, base, duration, Easing::EaseOutExpo)
    }

    pub fn shake_decay(intensity: f32, duration: f32) -> Tween<f32> {
        Tween::new(intensity, 0.0, duration, Easing::EaseOutExpo)
    }

    pub fn health_bar(from: f32, to: f32, duration: f32) -> Tween<f32> {
        Tween::new(from, to, duration, Easing::EaseOutBack)
    }

    pub fn pulse(amplitude: f32, rate: f32) -> Tween<f32> {
        Tween::new(1.0 - amplitude, 1.0 + amplitude, 1.0 / rate, Easing::EaseInOutSine)
            .with_repeat(-1, true)
    }
}

// ── TweenState ─────────────────────────────────────────────────────────────────

/// A running tween with its own clock.
pub struct TweenState<T: Lerp + std::fmt::Debug> {
    pub tween: Tween<T>,
    elapsed:   f32,
    pub done:  bool,
}

impl<T: Lerp + std::fmt::Debug> TweenState<T> {
    pub fn new(tween: Tween<T>) -> Self {
        Self { done: false, tween, elapsed: 0.0 }
    }

    /// Advance time and return the current value.
    pub fn tick(&mut self, dt: f32) -> T {
        self.elapsed += dt;
        self.done = self.tween.is_complete(self.elapsed);
        if self.tween.repeat < 0 || !self.done {
            self.tween.sample_looped(self.elapsed)
        } else {
            self.tween.sample(self.tween.total_duration())
        }
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.done = false;
    }

    pub fn value(&self) -> T {
        self.tween.sample_looped(self.elapsed)
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.tween.duration.max(f32::EPSILON)).clamp(0.0, 1.0)
    }
}

// ── AnimationGroup ─────────────────────────────────────────────────────────────

/// Runs multiple f32 tweens in parallel, identified by string key.
pub struct AnimationGroup {
    tweens: std::collections::HashMap<String, TweenState<f32>>,
}

impl AnimationGroup {
    pub fn new() -> Self { Self { tweens: std::collections::HashMap::new() } }

    pub fn add(&mut self, key: impl Into<String>, tween: Tween<f32>) {
        self.tweens.insert(key.into(), TweenState::new(tween));
    }

    pub fn remove(&mut self, key: &str) {
        self.tweens.remove(key);
    }

    pub fn tick(&mut self, dt: f32) {
        self.tweens.values_mut().for_each(|t| { t.tick(dt); });
        self.tweens.retain(|_, t| !t.done);
    }

    pub fn get(&self, key: &str) -> f32 {
        self.tweens.get(key).map(|t| t.value()).unwrap_or(0.0)
    }

    pub fn is_running(&self, key: &str) -> bool {
        self.tweens.contains_key(key)
    }

    pub fn all_done(&self) -> bool {
        self.tweens.is_empty()
    }
}

impl Default for AnimationGroup {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_endpoints() {
        let tw = Tween::new(0.0f32, 10.0f32, 2.0, Easing::Linear);
        assert!((tw.sample(0.0) - 0.0).abs() < 1e-4);
        assert!((tw.sample(2.0) - 10.0).abs() < 1e-4);
        assert!((tw.sample(1.0) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tween_complete() {
        let tw = Tween::new(0.0f32, 1.0f32, 1.0, Easing::Linear);
        assert!(!tw.is_complete(0.5));
        assert!(tw.is_complete(1.0));
        assert!(tw.is_complete(2.0));
    }

    #[test]
    fn tween_delay() {
        let tw = Tween::new(0.0f32, 1.0f32, 1.0, Easing::Linear).with_delay(0.5);
        assert!((tw.sample(0.0) - 0.0).abs() < 1e-4);
        assert!((tw.sample(0.5) - 0.0).abs() < 1e-4);
        assert!((tw.sample(1.5) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn tween_state_advances() {
        let tw = Tween::new(0.0f32, 1.0f32, 0.5, Easing::Linear);
        let mut state = TweenState::new(tw);
        let v = state.tick(0.25);
        assert!((v - 0.5).abs() < 1e-4, "expected 0.5 got {v}");
        assert!(!state.done);
        state.tick(0.25);
        assert!(state.done);
    }

    #[test]
    fn vec3_tween() {
        let tw = Tween::new(Vec3::ZERO, Vec3::ONE, 1.0, Easing::Linear);
        let mid = tw.sample(0.5);
        assert!((mid.x - 0.5).abs() < 1e-4);
        assert!((mid.y - 0.5).abs() < 1e-4);
        assert!((mid.z - 0.5).abs() < 1e-4);
    }
}
