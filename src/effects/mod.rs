//! Screen effects coordinator — combines all post-processing passes into
//! a single, driven, event-triggered system.
//!

pub mod field_viz;

// # Architecture
//
// `EffectsController` owns all postfx parameter structs and drives them
// from high-level game events:
//   - `EffectEvent::CameraShake(trauma)` → screen shake + chromatic
//   - `EffectEvent::Explosion(pos, power)` → grain flash + bloom spike + distortion
//   - `EffectEvent::BossEnter` → full cinematic effect sequence
//   - `EffectEvent::PlayerDeath` → desaturation + darkening + vignette crush
//   - `EffectEvent::LevelUp` → hue rainbow + bloom burst
//   - `EffectEvent::ChaosRift(entropy)` → continuous chaos distortion
//
// The controller smoothly interpolates between effect states each frame.
// All parameters are exposed as public fields for direct access if needed.

use crate::render::postfx::{
    bloom::BloomParams,
    grain::GrainParams,
    scanlines::ScanlineParams,
    chromatic::ChromaticParams,
    distortion::DistortionParams,
    motion_blur::MotionBlurParams,
    color_grade::ColorGradeParams,
};
use crate::math::springs::SpringDamper;

// ── ColorGradeParams compat ───────────────────────────────────────────────────

// ── EffectEvent ───────────────────────────────────────────────────────────────

/// High-level game events that trigger post-processing effects.
#[derive(Debug, Clone)]
pub enum EffectEvent {
    /// Camera trauma — scales chromatic aberration, grain, and motion blur.
    CameraShake { trauma: f32 },
    /// Explosion at world position — grain flash, bloom spike, distortion burst.
    Explosion { power: f32, is_boss: bool },
    /// Boss entity enters the scene — full cinematic effect.
    BossEnter,
    /// Player death — color drain, vignette crush, slow fade to black.
    PlayerDeath,
    /// Level-up or victory — hue rainbow + bloom burst.
    LevelUp,
    /// Continuous chaos rift at given entropy level (0–1). Call each frame.
    ChaosRift { entropy: f32 },
    /// Heal — green tint flash + bloom pulse.
    Heal { amount_fraction: f32 },
    /// Flash a specific color (hit flash, pickup, etc.).
    ColorFlash { r: f32, g: f32, b: f32, intensity: f32, duration: f32 },
    /// Portal activation — chromatic + distortion ripple.
    Portal,
    /// Time slow — everything desaturates and motion blur increases.
    TimeSlow { factor: f32 },
    /// Resume normal speed after time slow.
    TimeResume,
    /// Lightning strike — instant white flash + chromatic.
    LightningStrike,
    /// Trigger a scanline glitch burst.
    DisplayGlitch { intensity: f32, duration: f32 },
    /// Reset all effects to default.
    Reset,
}

// ── EffectLayer ───────────────────────────────────────────────────────────────

/// A single time-limited effect overlay, active for `duration` seconds.
#[derive(Debug, Clone)]
struct EffectLayer {
    kind:     LayerKind,
    age:      f32,
    duration: f32,
    strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LayerKind {
    GrainFlash,
    BloomSpike,
    ChromaticBurst,
    DistortionBurst,
    ColorFlash { r: f32, g: f32, b: f32 },
    VignetteCrush,
    HueRainbow,
    GlitchBurst,
    WhiteFlash,
}

impl EffectLayer {
    fn is_expired(&self) -> bool { self.age >= self.duration }
    fn progress(&self) -> f32 { (self.age / self.duration).clamp(0.0, 1.0) }
    fn intensity(&self) -> f32 {
        let t = self.progress();
        // Default: decay curve (quick flash, exponential falloff)
        self.strength * (1.0 - t) * (1.0 - t)
    }
}

// ── EffectsController ─────────────────────────────────────────────────────────

/// The master effects coordinator.
///
/// Owns all postfx parameter structs, drives them from `EffectEvent`s,
/// and provides smooth spring-based transitions between states.
pub struct EffectsController {
    // ── Postfx parameter blocks ───────────────────────────────────────────────
    pub bloom:       BloomParams,
    pub grain:       GrainParams,
    pub scanlines:   ScanlineParams,
    pub chromatic:   ChromaticParams,
    pub distortion:  DistortionParams,
    pub motion_blur: MotionBlurParams,
    pub color_grade: ColorGradeParams,

    // ── Derived scalar states (spring-smoothed) ───────────────────────────────
    trauma:          f32,   // Current camera trauma [0, 1]
    entropy:         f32,   // Current chaos entropy [0, 1]
    time_slow:       f32,   // Time slow factor [0, 1] (1 = normal)

    // ── Springs for smooth parameter transitions ──────────────────────────────
    bloom_spring:    SpringDamper,
    grain_spring:    SpringDamper,
    chromatic_spring: SpringDamper,
    vignette_spring: SpringDamper,
    saturation_spring: SpringDamper,
    brightness_spring: SpringDamper,

    // ── Active timed layers ───────────────────────────────────────────────────
    layers: Vec<EffectLayer>,

    // ── State flags ───────────────────────────────────────────────────────────
    pub is_dead:          bool,
    pub boss_mode:        bool,
    pub chaos_rift_active: bool,
    pub time_slow_active:  bool,
}

impl EffectsController {
    pub fn new() -> Self {
        Self {
            bloom:       BloomParams::default(),
            grain:       GrainParams::default(),
            scanlines:   ScanlineParams::default(),
            chromatic:   ChromaticParams::none(),
            distortion:  DistortionParams::none(),
            motion_blur: MotionBlurParams::default(),
            color_grade: ColorGradeParams::default(),

            trauma:    0.0,
            entropy:   0.0,
            time_slow: 1.0,

            bloom_spring:      SpringDamper::critical(1.0, 6.0),
            grain_spring:      SpringDamper::critical(0.0, 8.0),
            chromatic_spring:  SpringDamper::critical(0.0, 8.0),
            vignette_spring:   SpringDamper::critical(0.15, 5.0),
            saturation_spring: SpringDamper::critical(1.0, 4.0),
            brightness_spring: SpringDamper::critical(0.0, 6.0),

            layers: Vec::new(),
            is_dead:           false,
            boss_mode:         false,
            chaos_rift_active: false,
            time_slow_active:  false,
        }
    }

    // ── Event handling ────────────────────────────────────────────────────────

    /// Process a game event, triggering appropriate effects.
    pub fn send(&mut self, event: EffectEvent) {
        match event {
            EffectEvent::CameraShake { trauma } => {
                self.trauma = (self.trauma + trauma).clamp(0.0, 1.0);
            }

            EffectEvent::Explosion { power, is_boss } => {
                let p = power.clamp(0.0, 1.0);
                self.push_layer(EffectLayer {
                    kind: LayerKind::GrainFlash, age: 0.0,
                    duration: 0.3 + p * 0.4, strength: p * 0.8
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::BloomSpike, age: 0.0,
                    duration: 0.5 + p * 0.5, strength: p * 2.0
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::DistortionBurst, age: 0.0,
                    duration: 0.4 + p * 0.3, strength: p
                });
                if is_boss {
                    self.push_layer(EffectLayer {
                        kind: LayerKind::WhiteFlash, age: 0.0,
                        duration: 0.15, strength: 1.0
                    });
                    self.push_layer(EffectLayer {
                        kind: LayerKind::ChromaticBurst, age: 0.0,
                        duration: 0.6, strength: 1.0
                    });
                }
                self.trauma = (self.trauma + p * 0.5).min(1.0);
            }

            EffectEvent::BossEnter => {
                self.boss_mode = true;
                self.push_layer(EffectLayer {
                    kind: LayerKind::VignetteCrush, age: 0.0,
                    duration: 3.0, strength: 1.0
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::ChromaticBurst, age: 0.0,
                    duration: 1.5, strength: 0.8
                });
                self.saturation_spring.set_target(0.0);
            }

            EffectEvent::PlayerDeath => {
                self.is_dead = true;
                self.saturation_spring.set_target(0.0);
                self.brightness_spring.set_target(-0.8);
                self.vignette_spring.set_target(1.0);
            }

            EffectEvent::LevelUp => {
                self.push_layer(EffectLayer {
                    kind: LayerKind::HueRainbow, age: 0.0,
                    duration: 2.0, strength: 1.0
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::BloomSpike, age: 0.0,
                    duration: 0.8, strength: 3.0
                });
                self.bloom_spring.set_target(3.0);
            }

            EffectEvent::ChaosRift { entropy } => {
                self.entropy = entropy.clamp(0.0, 1.0);
                self.chaos_rift_active = true;
            }

            EffectEvent::Heal { amount_fraction } => {
                let g = amount_fraction.clamp(0.0, 1.0);
                self.push_layer(EffectLayer {
                    kind: LayerKind::ColorFlash { r: 0.2, g: 1.0, b: 0.3 },
                    age: 0.0, duration: 0.5, strength: g * 0.6
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::BloomSpike, age: 0.0,
                    duration: 0.4, strength: g * 1.5
                });
            }

            EffectEvent::ColorFlash { r, g, b, intensity, duration } => {
                self.push_layer(EffectLayer {
                    kind: LayerKind::ColorFlash { r, g, b },
                    age: 0.0, duration, strength: intensity
                });
            }

            EffectEvent::Portal => {
                self.push_layer(EffectLayer {
                    kind: LayerKind::DistortionBurst, age: 0.0,
                    duration: 0.8, strength: 0.6
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::ChromaticBurst, age: 0.0,
                    duration: 0.5, strength: 0.5
                });
            }

            EffectEvent::TimeSlow { factor } => {
                self.time_slow_active = true;
                self.time_slow = factor.clamp(0.05, 1.0);
                self.saturation_spring.set_target(0.4);
                self.motion_blur.scale = 0.6;
                self.motion_blur.temporal = 0.4;
            }

            EffectEvent::TimeResume => {
                self.time_slow_active = false;
                self.time_slow = 1.0;
                self.saturation_spring.set_target(1.0);
                self.motion_blur = MotionBlurParams::default();
            }

            EffectEvent::LightningStrike => {
                self.push_layer(EffectLayer {
                    kind: LayerKind::WhiteFlash, age: 0.0,
                    duration: 0.1, strength: 1.0
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::ChromaticBurst, age: 0.0,
                    duration: 0.3, strength: 1.0
                });
                self.trauma = (self.trauma + 0.4).min(1.0);
            }

            EffectEvent::DisplayGlitch { intensity, duration } => {
                let i = intensity.clamp(0.0, 1.0);
                self.push_layer(EffectLayer {
                    kind: LayerKind::GlitchBurst, age: 0.0,
                    duration, strength: i
                });
                self.push_layer(EffectLayer {
                    kind: LayerKind::ChromaticBurst, age: 0.0,
                    duration: duration * 0.7, strength: i * 0.8
                });
            }

            EffectEvent::Reset => {
                self.reset();
            }
        }
    }

    fn push_layer(&mut self, layer: EffectLayer) {
        self.layers.push(layer);
    }

    // ── Tick ──────────────────────────────────────────────────────────────────

    /// Advance all effects by `dt` seconds.
    ///
    /// Call this every frame before reading the parameter blocks.
    pub fn tick(&mut self, dt: f32) {
        // Decay trauma
        self.trauma = (self.trauma - dt * 1.5).max(0.0);
        let t2 = self.trauma * self.trauma;

        // Decay chaos rift entropy if not being driven
        if !self.chaos_rift_active {
            self.entropy = (self.entropy - dt * 0.5).max(0.0);
        }
        self.chaos_rift_active = false; // reset; will be re-set next frame if still active

        // ── Advance layers ────────────────────────────────────────────────────
        let mut grain_add    = 0.0_f32;
        let mut bloom_add    = 0.0_f32;
        let mut chromatic_add = 0.0_f32;
        let mut distortion_add = 0.0_f32;
        let mut vignette_add = 0.0_f32;
        let mut brightness_add = 0.0_f32;
        let mut hue_shift = 0.0_f32;

        for layer in &mut self.layers {
            layer.age += dt;
            let intensity = layer.intensity();
            match layer.kind {
                LayerKind::GrainFlash         => grain_add      += intensity * 0.8,
                LayerKind::BloomSpike         => bloom_add      += intensity * 2.0,
                LayerKind::ChromaticBurst     => chromatic_add  += intensity * 0.025,
                LayerKind::DistortionBurst    => distortion_add += intensity * 0.05,
                LayerKind::VignetteCrush      => vignette_add   += intensity * 0.7,
                LayerKind::HueRainbow         => hue_shift       = layer.progress() * 360.0,
                LayerKind::GlitchBurst        => {
                    grain_add    += intensity * 0.4;
                    chromatic_add += intensity * 0.02;
                }
                LayerKind::WhiteFlash         => brightness_add += intensity * 1.5,
                LayerKind::ColorFlash { .. }  => {
                    brightness_add += intensity * 0.3;
                }
            }
        }
        self.layers.retain(|l| !l.is_expired());

        // ── Trauma contribution ───────────────────────────────────────────────
        chromatic_add  += t2 * 0.015;
        grain_add      += t2 * 0.4;
        bloom_add      += t2 * 0.5;

        // ── Chaos rift contribution ────────────────────────────────────────────
        let e = self.entropy;
        distortion_add += e * 0.08;
        chromatic_add  += e * 0.02;
        grain_add      += e * 0.3;

        // ── Spring targets ────────────────────────────────────────────────────
        self.bloom_spring.set_target(1.0 + bloom_add);
        self.grain_spring.set_target(grain_add);
        self.chromatic_spring.set_target(chromatic_add);
        if !self.is_dead && !self.boss_mode {
            self.saturation_spring.set_target(1.0 - e * 0.3);
            self.vignette_spring.set_target(0.15 + vignette_add);
            self.brightness_spring.set_target(brightness_add);
        }

        // ── Advance springs ───────────────────────────────────────────────────
        self.bloom_spring.tick(dt);
        self.grain_spring.tick(dt);
        self.chromatic_spring.tick(dt);
        self.vignette_spring.tick(dt);
        self.saturation_spring.tick(dt);
        self.brightness_spring.tick(dt);

        // ── Write to parameter blocks ─────────────────────────────────────────
        let bloom_target = self.bloom_spring.position.max(0.0);
        self.bloom.threshold = (0.8 - (bloom_target - 1.0) * 0.2).clamp(0.0, 1.0);
        self.bloom.intensity = bloom_target;

        self.grain.intensity = self.grain_spring.position.max(0.0);
        self.grain.enabled = self.grain.intensity > 0.001;

        let chrom = self.chromatic_spring.position.max(0.0);
        if chrom > 0.001 {
            self.chromatic = ChromaticParams {
                enabled: true,
                red_offset: 0.002 + chrom,
                blue_offset: 0.003 + chrom * 1.2,
                green_offset: chrom * 0.1,
                radial_scale: true,
                tangential: t2 * 0.3 + e * 0.2,
                spectrum_spread: (chrom * 10.0).min(0.8),
                barrel_distortion: e * 0.04,
            };
        } else {
            self.chromatic = ChromaticParams::none();
        }

        let dist = distortion_add;
        if dist > 0.001 || e > 0.01 {
            self.distortion.enabled = true;
            self.distortion.scale = (dist + e * 0.5).min(3.0);
            self.distortion.max_offset = (dist * 0.5 + e * 0.06).min(0.15);
            self.distortion.chromatic_split = (dist * 2.0 + e * 0.4).min(1.0);
        } else {
            self.distortion.enabled = false;
        }

        self.color_grade.saturation = self.saturation_spring.position.clamp(0.0, 2.0);
        self.color_grade.brightness = self.brightness_spring.position.clamp(-1.0, 2.0);
        self.color_grade.vignette = self.vignette_spring.position.clamp(0.0, 1.0);
        self.color_grade.hue_shift = hue_shift;

        // Boss mode: boost contrast
        if self.boss_mode {
            self.color_grade.contrast = 1.3;
        } else {
            self.color_grade.contrast = 1.0;
        }

        // Time slow: slightly warm the color grade
        if self.time_slow_active {
            self.color_grade.saturation = self.color_grade.saturation * 0.5;
        }
    }

    // ── Reset ──────────────────────────────────────────────────────────────────

    /// Reset all effects to default (no trauma, no layers, default postfx).
    pub fn reset(&mut self) {
        self.trauma      = 0.0;
        self.entropy     = 0.0;
        self.time_slow   = 1.0;
        self.is_dead     = false;
        self.boss_mode   = false;
        self.chaos_rift_active = false;
        self.time_slow_active  = false;
        self.layers.clear();

        self.bloom       = BloomParams::default();
        self.grain       = GrainParams::default();
        self.scanlines   = ScanlineParams::default();
        self.chromatic   = ChromaticParams::none();
        self.distortion  = DistortionParams::none();
        self.motion_blur = MotionBlurParams::default();
        self.color_grade = ColorGradeParams::default();

        self.bloom_spring.teleport(1.0);
        self.grain_spring.teleport(0.0);
        self.chromatic_spring.teleport(0.0);
        self.vignette_spring.teleport(0.15);
        self.saturation_spring.teleport(1.0);
        self.brightness_spring.teleport(0.0);
    }

    // ── Accessors ──────────────────────────────────────────────────────────────

    pub fn trauma(&self) -> f32 { self.trauma }
    pub fn entropy(&self) -> f32 { self.entropy }
    pub fn time_slow_factor(&self) -> f32 { self.time_slow }
    pub fn active_layer_count(&self) -> usize { self.layers.len() }

    /// True if any timed effects are currently running.
    pub fn has_active_effects(&self) -> bool {
        !self.layers.is_empty() || self.trauma > 0.01 || self.entropy > 0.01
    }

    /// Summary string for debug overlay.
    pub fn debug_summary(&self) -> String {
        format!(
            "trauma={:.2} entropy={:.2} layers={} bloom={:.2} grain={:.2} chrom={:.3} dist={} sat={:.2}",
            self.trauma, self.entropy, self.layers.len(),
            self.bloom.intensity, self.grain.intensity,
            self.chromatic.red_offset,
            self.distortion.enabled,
            self.color_grade.saturation,
        )
    }
}

impl Default for EffectsController {
    fn default() -> Self { Self::new() }
}

// ── EffectPresets ─────────────────────────────────────────────────────────────

/// Pre-baked effect sequences for common scenarios.
pub struct EffectPresets;

impl EffectPresets {
    /// Generate a burst of events simulating a boss fight opening.
    pub fn boss_opening() -> Vec<EffectEvent> {
        vec![
            EffectEvent::BossEnter,
            EffectEvent::CameraShake { trauma: 0.7 },
            EffectEvent::DisplayGlitch { intensity: 0.5, duration: 0.4 },
        ]
    }

    /// Player takes a heavy hit.
    pub fn heavy_hit(damage_fraction: f32) -> Vec<EffectEvent> {
        vec![
            EffectEvent::CameraShake { trauma: damage_fraction * 0.8 },
            EffectEvent::Explosion { power: damage_fraction * 0.5, is_boss: false },
            EffectEvent::ColorFlash {
                r: 1.0, g: 0.1, b: 0.1,
                intensity: damage_fraction * 0.6,
                duration: 0.3,
            },
        ]
    }

    /// Area of Effect explosion.
    pub fn aoe_explosion(power: f32) -> Vec<EffectEvent> {
        vec![
            EffectEvent::Explosion { power, is_boss: power > 0.8 },
            EffectEvent::CameraShake { trauma: power * 0.6 },
        ]
    }

    /// Dimensional rift opening — sustained chaos effects.
    pub fn rift_opening(entropy: f32) -> Vec<EffectEvent> {
        vec![
            EffectEvent::ChaosRift { entropy },
            EffectEvent::DisplayGlitch { intensity: entropy * 0.6, duration: 0.5 },
            EffectEvent::Portal,
        ]
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_smoke() {
        let mut ctrl = EffectsController::new();
        ctrl.send(EffectEvent::CameraShake { trauma: 0.5 });
        ctrl.tick(0.016);
        assert!(ctrl.trauma > 0.0, "trauma should be set");
    }

    #[test]
    fn explosion_triggers_layers() {
        let mut ctrl = EffectsController::new();
        ctrl.send(EffectEvent::Explosion { power: 0.8, is_boss: false });
        // Should have timed layers
        assert!(ctrl.active_layer_count() > 0, "explosion should create layers");
    }

    #[test]
    fn layers_expire() {
        let mut ctrl = EffectsController::new();
        ctrl.send(EffectEvent::Explosion { power: 0.5, is_boss: false });
        // Advance past the longest layer duration (1.0 sec for 0.5 power)
        for _ in 0..120 {
            ctrl.tick(0.016);
        }
        assert_eq!(ctrl.active_layer_count(), 0, "all layers should expire");
    }

    #[test]
    fn reset_clears_everything() {
        let mut ctrl = EffectsController::new();
        ctrl.send(EffectEvent::Explosion { power: 1.0, is_boss: true });
        ctrl.send(EffectEvent::BossEnter);
        ctrl.send(EffectEvent::PlayerDeath);
        ctrl.tick(0.016);
        ctrl.send(EffectEvent::Reset);
        ctrl.tick(0.0);
        assert!(!ctrl.is_dead);
        assert!(!ctrl.boss_mode);
        assert_eq!(ctrl.active_layer_count(), 0);
    }

    #[test]
    fn chaos_rift_activates_distortion() {
        let mut ctrl = EffectsController::new();
        ctrl.chaos_rift_active = true;
        ctrl.entropy = 0.8;
        ctrl.tick(0.016);
        assert!(ctrl.distortion.enabled, "high entropy should enable distortion");
    }

    #[test]
    fn trauma_decays() {
        let mut ctrl = EffectsController::new();
        ctrl.send(EffectEvent::CameraShake { trauma: 1.0 });
        for _ in 0..60 {
            ctrl.tick(0.016);
        }
        assert!(ctrl.trauma < 0.5, "trauma should decay over time: {}", ctrl.trauma);
    }

    #[test]
    fn preset_boss_opening_generates_events() {
        let events = EffectPresets::boss_opening();
        assert!(!events.is_empty(), "boss opening should produce events");
        let mut ctrl = EffectsController::new();
        for e in events { ctrl.send(e); }
        ctrl.tick(0.016);
        assert!(ctrl.boss_mode, "boss mode should be set");
    }

    #[test]
    fn time_slow_desaturates() {
        let mut ctrl = EffectsController::new();
        ctrl.send(EffectEvent::TimeSlow { factor: 0.2 });
        for _ in 0..30 {
            ctrl.tick(0.016);
        }
        assert!(
            ctrl.color_grade.saturation < 0.8,
            "time slow should reduce saturation: {}", ctrl.color_grade.saturation
        );
    }
}
