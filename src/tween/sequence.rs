//! Tween sequences — chains and parallel groups of tweens.
//!
//! A `TweenSequence` runs tweens one after another with optional overlap.
//! A `TweenTimeline` runs multiple named tracks in parallel.

use super::{Tween, TweenState, Lerp};
use super::easing::Easing;
use glam::{Vec2, Vec3, Vec4};

// ── SequenceStep ──────────────────────────────────────────────────────────────

/// A single step in a sequence, which may overlap with the previous step.
pub struct SequenceStep<T: Lerp + std::fmt::Debug> {
    pub tween:          TweenState<T>,
    /// Seconds of overlap with the previous step (negative = gap).
    pub overlap:        f32,
    /// Start time within the sequence (computed by SequenceBuilder).
    pub(crate) start_t: f32,
}

// ── TweenSequence ─────────────────────────────────────────────────────────────

/// A sequence of tweens played one after another with optional overlap.
///
/// The sequence tracks its own clock and drives one step at a time.
/// The current value is the output of the currently active step.
pub struct TweenSequence<T: Lerp + Clone + std::fmt::Debug> {
    pub steps:    Vec<SequenceStep<T>>,
    pub elapsed:  f32,
    pub looping:  bool,
    pub duration: f32,
    pub done:     bool,
    default_val:  T,
}

impl<T: Lerp + Clone + std::fmt::Debug> TweenSequence<T> {
    /// Build a sequence from a list of (tween, overlap) pairs.
    pub fn new(steps: Vec<(Tween<T>, f32)>, default_val: T, looping: bool) -> Self {
        let mut seq_steps: Vec<SequenceStep<T>> = Vec::with_capacity(steps.len());
        let mut cursor = 0.0_f32;
        for (tween, overlap) in steps {
            let start_t = cursor - overlap;
            cursor = start_t + tween.duration;
            seq_steps.push(SequenceStep {
                tween: TweenState::new(tween),
                overlap,
                start_t,
            });
        }
        let duration = cursor;
        Self { steps: seq_steps, elapsed: 0.0, looping, duration, done: false, default_val }
    }

    /// Advance the sequence by `dt` and return the current interpolated value.
    pub fn tick(&mut self, dt: f32) -> T {
        self.elapsed += dt;
        if self.looping && self.elapsed >= self.duration {
            self.elapsed -= self.duration;
        }
        self.done = !self.looping && self.elapsed >= self.duration;
        self.current_value()
    }

    /// Current value based on elapsed time, without advancing.
    pub fn current_value(&self) -> T {
        let t = if self.looping {
            self.elapsed % self.duration.max(f32::EPSILON)
        } else {
            self.elapsed.min(self.duration)
        };

        // Find the active step — the last step that has started
        let mut active: Option<usize> = None;
        for (i, step) in self.steps.iter().enumerate() {
            if t >= step.start_t {
                active = Some(i);
            }
        }

        if let Some(idx) = active {
            let step = &self.steps[idx];
            let local_t = (t - step.start_t).max(0.0);
            step.tween.tween.sample(local_t)
        } else {
            self.default_val.clone()
        }
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.done = false;
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration.max(f32::EPSILON)).clamp(0.0, 1.0)
    }
}

// ── SequenceBuilder ───────────────────────────────────────────────────────────

/// Fluent builder for TweenSequence.
///
/// ```rust,no_run
/// use proof_engine::tween::sequence::SequenceBuilder;
/// use proof_engine::tween::Easing;
///
/// let seq = SequenceBuilder::new(0.0f32)
///     .then(0.0, 1.0, 0.5, Easing::EaseOutCubic)
///     .then(1.0, 0.5, 0.3, Easing::EaseInQuad)
///     .overlap(0.1)
///     .looping(false)
///     .build();
/// ```
pub struct SequenceBuilder<T: Lerp + Clone + std::fmt::Debug> {
    steps:       Vec<(Tween<T>, f32)>,
    default_val: T,
    looping:     bool,
    next_overlap: f32,
}

impl<T: Lerp + Clone + std::fmt::Debug> SequenceBuilder<T> {
    pub fn new(default_val: T) -> Self {
        Self { steps: Vec::new(), default_val, looping: false, next_overlap: 0.0 }
    }

    /// Add a tween step from `from` to `to` over `duration` seconds.
    pub fn then(mut self, from: T, to: T, duration: f32, easing: Easing) -> Self {
        let overlap = self.next_overlap;
        self.next_overlap = 0.0;
        self.steps.push((Tween::new(from, to, duration, easing), overlap));
        self
    }

    /// Set overlap (in seconds) for the next step. Positive = overlap with previous.
    pub fn overlap(mut self, seconds: f32) -> Self {
        self.next_overlap = seconds;
        self
    }

    /// Add a pause (gap) before the next step.
    pub fn wait(mut self, seconds: f32) -> Self {
        self.next_overlap = -seconds;
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn build(self) -> TweenSequence<T> {
        TweenSequence::new(self.steps, self.default_val, self.looping)
    }
}

// ── TweenTimeline ─────────────────────────────────────────────────────────────

/// Multiple named f32 animation tracks running in parallel.
///
/// Each track is an independent `TweenSequence<f32>`.
/// Access current values by name each frame.
pub struct TweenTimeline {
    pub tracks:  std::collections::HashMap<String, TweenSequence<f32>>,
    pub elapsed: f32,
    pub looping: bool,
    duration:    f32,
    pub done:    bool,
}

impl TweenTimeline {
    pub fn new(looping: bool) -> Self {
        Self {
            tracks:  std::collections::HashMap::new(),
            elapsed: 0.0,
            looping,
            duration: 0.0,
            done:    false,
        }
    }

    /// Add a named track.
    pub fn add_track(&mut self, name: impl Into<String>, seq: TweenSequence<f32>) {
        self.duration = self.duration.max(seq.duration);
        self.tracks.insert(name.into(), seq);
    }

    /// Advance all tracks and return the current state.
    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
        if self.looping && self.elapsed >= self.duration {
            self.elapsed -= self.duration;
            for track in self.tracks.values_mut() { track.reset(); }
        }
        self.done = !self.looping && self.elapsed >= self.duration;
        for track in self.tracks.values_mut() {
            track.tick(dt);
        }
    }

    /// Get the current value of a named track. Returns 0.0 if not found.
    pub fn get(&self, name: &str) -> f32 {
        self.tracks.get(name).map(|t| t.current_value()).unwrap_or(0.0)
    }

    /// Reset all tracks to the beginning.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.done = false;
        for track in self.tracks.values_mut() { track.reset(); }
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration.max(f32::EPSILON)).clamp(0.0, 1.0)
    }
}

// ── Predefined game-useful timelines ──────────────────────────────────────────

impl TweenTimeline {
    /// Build a damage flash timeline: screen red flash + slight scale pop.
    pub fn damage_flash(intensity: f32) -> Self {
        let mut tl = Self::new(false);

        let flash_seq = SequenceBuilder::new(0.0f32)
            .then(intensity, 0.0, 0.3, Easing::EaseOutExpo)
            .build();
        tl.add_track("flash", flash_seq);

        let scale_seq = SequenceBuilder::new(1.0f32)
            .then(1.0 + intensity * 0.1, 1.0, 0.25, Easing::EaseOutBack)
            .build();
        tl.add_track("scale", scale_seq);

        tl
    }

    /// Build a level-up timeline: brightness flash + long glow fade.
    pub fn level_up() -> Self {
        let mut tl = Self::new(false);

        let flash = SequenceBuilder::new(0.0f32)
            .then(1.5, 1.0, 0.15, Easing::EaseOutExpo)
            .then(1.0, 1.0, 1.5, Easing::Linear)
            .then(1.0, 0.0, 0.5, Easing::EaseInQuad)
            .build();
        tl.add_track("brightness", flash);

        let hue_shift = SequenceBuilder::new(0.0f32)
            .then(0.0, 360.0, 2.0, Easing::Linear)
            .build();
        tl.add_track("hue_shift", hue_shift);

        let bloom = SequenceBuilder::new(1.0f32)
            .then(3.0, 1.0, 0.8, Easing::EaseOutCubic)
            .build();
        tl.add_track("bloom", bloom);

        tl
    }

    /// Build a boss entrance timeline: dark vignette crunch → reveal.
    pub fn boss_entrance() -> Self {
        let mut tl = Self::new(false);

        let vignette = SequenceBuilder::new(0.15f32)
            .then(0.15, 0.9, 0.8, Easing::EaseInCubic)
            .then(0.9, 0.9, 1.2, Easing::Linear)
            .then(0.9, 0.2, 0.6, Easing::EaseOutExpo)
            .build();
        tl.add_track("vignette", vignette);

        let chromatic = SequenceBuilder::new(0.002f32)
            .then(0.002, 0.02, 0.8, Easing::EaseInExpo)
            .then(0.02, 0.001, 0.4, Easing::EaseOutCubic)
            .build();
        tl.add_track("chromatic", chromatic);

        let saturation = SequenceBuilder::new(1.0f32)
            .then(1.0, 0.0, 0.8, Easing::EaseInCubic)
            .then(0.0, 1.2, 0.6, Easing::EaseOutBack)
            .build();
        tl.add_track("saturation", saturation);

        tl
    }

    /// Build a death sequence: drain color, crush vignette, fade to black.
    pub fn death_sequence() -> Self {
        let mut tl = Self::new(false);

        let saturation = SequenceBuilder::new(1.0f32)
            .then(1.0, 0.0, 2.5, Easing::EaseInCubic)
            .build();
        tl.add_track("saturation", saturation);

        let brightness = SequenceBuilder::new(0.0f32)
            .then(0.0, -0.8, 3.0, Easing::EaseInQuart)
            .build();
        tl.add_track("brightness", brightness);

        let vignette = SequenceBuilder::new(0.15f32)
            .then(0.15, 1.0, 3.0, Easing::EaseInCubic)
            .build();
        tl.add_track("vignette", vignette);

        let chromatic = SequenceBuilder::new(0.002f32)
            .then(0.002, 0.015, 1.5, Easing::EaseInQuad)
            .build();
        tl.add_track("chromatic", chromatic);

        tl
    }

    /// Build a healing pulse: green tint flash + brightness glow.
    pub fn heal_pulse(amount_fraction: f32) -> Self {
        let mut tl = Self::new(false);

        let green = SequenceBuilder::new(1.0f32)
            .then(1.0, 1.0 + amount_fraction * 0.4, 0.15, Easing::EaseOutExpo)
            .then(1.0 + amount_fraction * 0.4, 1.0, 0.4, Easing::EaseInQuad)
            .build();
        tl.add_track("green_tint", green);

        let bloom = SequenceBuilder::new(1.0f32)
            .then(1.0, 1.8, 0.15, Easing::EaseOutExpo)
            .then(1.8, 1.0, 0.5, Easing::EaseOutCubic)
            .build();
        tl.add_track("bloom", bloom);

        tl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_basic() {
        let mut seq = SequenceBuilder::new(0.0f32)
            .then(0.0, 1.0, 1.0, Easing::Linear)
            .then(1.0, 2.0, 1.0, Easing::Linear)
            .build();
        let v0 = seq.tick(0.0);
        assert!((v0 - 0.0).abs() < 1e-4);
        seq.elapsed = 0.5;
        let v_mid = seq.current_value();
        assert!((v_mid - 0.5).abs() < 1e-4, "expected 0.5 got {v_mid}");
        seq.elapsed = 1.5;
        let v2 = seq.current_value();
        assert!((v2 - 1.5).abs() < 1e-4, "expected 1.5 got {v2}");
    }

    #[test]
    fn timeline_damage_flash() {
        let mut tl = TweenTimeline::damage_flash(1.0);
        let flash_start = tl.get("flash");
        assert!((flash_start - 1.0).abs() < 0.01, "flash starts at intensity");
        tl.tick(0.3);
        let flash_end = tl.get("flash");
        assert!(flash_end < 0.2, "flash should decay quickly");
    }

    #[test]
    fn builder_wait() {
        let seq = SequenceBuilder::new(0.0f32)
            .then(0.0, 1.0, 0.5, Easing::Linear)
            .wait(0.5)
            .then(1.0, 2.0, 0.5, Easing::Linear)
            .build();
        // Second step starts at 0.5 (first step) + 0.5 (gap) = 1.0
        assert!((seq.steps[1].start_t - 1.0).abs() < 1e-4, "second step at t=1.0");
    }
}
