//! Engine configuration — hierarchical, hot-reloadable, command-line overridable.
//!
//! Loads from `engine.toml` with sensible defaults for all settings. Supports:
//! - TOML serialization/deserialization
//! - Profile system (default, debug, release, steam_deck, low_end)
//! - Command-line argument overrides (`--width`, `--height`, `--no-audio`, etc.)
//! - Hot reload: watch `engine.toml` for changes and re-apply at runtime
//! - Validation with clamped/sanitized values
//! - Diff-based change detection for per-subsystem notifications

use serde::{Deserialize, Serialize};

// ── Top-level ─────────────────────────────────────────────────────────────────

/// Top-level engine configuration, loadable from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub window_title:  String,
    pub window_width:  u32,
    pub window_height: u32,
    pub target_fps:    u32,
    pub vsync:         bool,
    pub audio:         AudioConfig,
    pub render:        RenderConfig,
    pub physics:       PhysicsConfig,
    pub input:         InputConfig,
    pub debug:         DebugConfig,
    pub gameplay:      GameplayConfig,
    pub accessibility: AccessibilityConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window_title:  "Proof Engine".to_string(),
            window_width:  1280,
            window_height: 800,
            target_fps:    60,
            vsync:         true,
            audio:         AudioConfig::default(),
            render:        RenderConfig::default(),
            physics:       PhysicsConfig::default(),
            input:         InputConfig::default(),
            debug:         DebugConfig::default(),
            gameplay:      GameplayConfig::default(),
            accessibility: AccessibilityConfig::default(),
        }
    }
}

impl EngineConfig {
    // ── Load/Save ──────────────────────────────────────────────────────────────

    /// Load from a TOML file, falling back to defaults on error.
    pub fn load(path: &str) -> Self {
        let mut cfg: Self = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        cfg.validate();
        cfg
    }

    /// Save to a TOML file.
    pub fn save(&self, path: &str) -> bool {
        toml::to_string_pretty(self)
            .ok()
            .and_then(|s| std::fs::write(path, s).ok())
            .is_some()
    }

    // ── Profiles ──────────────────────────────────────────────────────────────

    /// Low-end PC profile: reduced resolution, no post-fx, minimal effects.
    pub fn profile_low_end() -> Self {
        Self {
            window_width:  960,
            window_height: 540,
            target_fps:    30,
            vsync:         true,
            render:        RenderConfig {
                bloom_enabled:        false,
                motion_blur_enabled:  false,
                chromatic_aberration: 0.0,
                film_grain:           0.0,
                scanlines_enabled:    false,
                particle_multiplier:  0.5,
                shadow_quality:       ShadowQuality::Off,
                ..RenderConfig::default()
            },
            physics: PhysicsConfig {
                fluid_grid_size:  16,
                soft_body_iters:  2,
                ..PhysicsConfig::default()
            },
            ..Self::default()
        }
    }

    /// Steam Deck profile: 1280×800 locked 60, moderate effects.
    pub fn profile_steam_deck() -> Self {
        Self {
            window_width:  1280,
            window_height: 800,
            target_fps:    60,
            vsync:         true,
            render:        RenderConfig {
                bloom_intensity:      0.6,
                chromatic_aberration: 0.001,
                particle_multiplier:  0.75,
                shadow_quality:       ShadowQuality::Low,
                ..RenderConfig::default()
            },
            ..Self::default()
        }
    }

    /// Ultra profile: max everything.
    pub fn profile_ultra() -> Self {
        Self {
            window_width:  2560,
            window_height: 1440,
            target_fps:    144,
            vsync:         false,
            render:        RenderConfig {
                bloom_intensity:      1.5,
                chromatic_aberration: 0.003,
                film_grain:           0.03,
                particle_multiplier:  2.0,
                shadow_quality:       ShadowQuality::Ultra,
                ..RenderConfig::default()
            },
            physics: PhysicsConfig {
                fluid_grid_size:  128,
                soft_body_iters:  8,
                ..PhysicsConfig::default()
            },
            ..Self::default()
        }
    }

    /// Debug profile: all overlays on, no vsync, fast timestep.
    pub fn profile_debug() -> Self {
        Self {
            vsync:      false,
            target_fps: 0, // uncapped
            debug: DebugConfig {
                show_fps:          true,
                show_frame_graph:  true,
                show_physics:      true,
                show_spawn_zones:  true,
                show_entity_ids:   true,
                log_level:         LogLevel::Debug,
                ..DebugConfig::default()
            },
            ..Self::default()
        }
    }

    // ── Command-line override ──────────────────────────────────────────────────

    /// Parse and apply command-line arguments as overrides.
    /// Supported flags: `--width N`, `--height N`, `--fps N`, `--no-audio`,
    /// `--no-vsync`, `--no-bloom`, `--fullscreen`, `--windowed`.
    pub fn apply_args(&mut self, args: &[String]) {
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--width" => {
                    if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                        self.window_width = v;
                        i += 1;
                    }
                }
                "--height" => {
                    if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                        self.window_height = v;
                        i += 1;
                    }
                }
                "--fps" => {
                    if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                        self.target_fps = v;
                        i += 1;
                    }
                }
                "--no-audio"    => self.audio.enabled = false,
                "--no-vsync"    => self.vsync = false,
                "--no-bloom"    => self.render.bloom_enabled = false,
                "--no-postfx"   => {
                    self.render.bloom_enabled = false;
                    self.render.distortion_enabled = false;
                    self.render.motion_blur_enabled = false;
                    self.render.chromatic_aberration = 0.0;
                    self.render.film_grain = 0.0;
                }
                "--fullscreen"  => self.render.fullscreen = true,
                "--windowed"    => self.render.fullscreen = false,
                "--low-end"     => *self = Self::profile_low_end(),
                "--ultra"       => *self = Self::profile_ultra(),
                "--debug"       => *self = Self::profile_debug(),
                _ => {}
            }
            i += 1;
        }
        self.validate();
    }

    // ── Validation ─────────────────────────────────────────────────────────────

    /// Clamp and sanitize all values to valid ranges.
    pub fn validate(&mut self) {
        self.window_width  = self.window_width.clamp(320, 7680);
        self.window_height = self.window_height.clamp(240, 4320);
        self.target_fps    = if self.target_fps == 0 { 0 } else { self.target_fps.clamp(15, 360) };
        self.audio.validate();
        self.render.validate();
        self.physics.validate();
    }

    // ── Aspect ratio ──────────────────────────────────────────────────────────

    pub fn aspect_ratio(&self) -> f32 {
        self.window_width as f32 / self.window_height as f32
    }

    /// Check if resolution is standard 16:9.
    pub fn is_widescreen(&self) -> bool {
        let ar = self.aspect_ratio();
        (ar - 16.0 / 9.0).abs() < 0.05
    }

    /// Check if the config differs from another (for hot-reload diffs).
    pub fn diff(&self, other: &Self) -> ConfigDiff {
        ConfigDiff {
            window_changed:  self.window_width  != other.window_width
                          || self.window_height != other.window_height,
            audio_changed:   self.audio.enabled != other.audio.enabled
                          || (self.audio.master_volume - other.audio.master_volume).abs() > 0.01,
            render_changed:  self.render.bloom_enabled    != other.render.bloom_enabled
                          || self.render.bloom_intensity  != other.render.bloom_intensity,
            physics_changed: self.physics.fluid_grid_size != other.physics.fluid_grid_size,
        }
    }
}

/// Bitmask of which subsystems changed in a hot-reload diff.
#[derive(Debug, Default)]
pub struct ConfigDiff {
    pub window_changed:  bool,
    pub audio_changed:   bool,
    pub render_changed:  bool,
    pub physics_changed: bool,
}

impl ConfigDiff {
    pub fn any_changed(&self) -> bool {
        self.window_changed || self.audio_changed || self.render_changed || self.physics_changed
    }
}

// ── AudioConfig ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub enabled:       bool,
    pub master_volume: f32,
    pub music_volume:  f32,
    pub sfx_volume:    f32,
    /// Sample rate override (0 = use device default).
    pub sample_rate:   u32,
    /// Buffer size in frames (0 = auto).
    pub buffer_size:   u32,
    /// Enable spatialized 3D audio.
    pub spatial_audio: bool,
    /// Reverb room size (0.0 = dry, 1.0 = large hall).
    pub reverb_room:   f32,
    pub audio_backend: AudioBackend,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled:       true,
            master_volume: 1.0,
            music_volume:  0.6,
            sfx_volume:    0.8,
            sample_rate:   0,
            buffer_size:   0,
            spatial_audio: true,
            reverb_room:   0.2,
            audio_backend: AudioBackend::Default,
        }
    }
}

impl AudioConfig {
    pub fn validate(&mut self) {
        self.master_volume = self.master_volume.clamp(0.0, 1.0);
        self.music_volume  = self.music_volume.clamp(0.0, 1.0);
        self.sfx_volume    = self.sfx_volume.clamp(0.0, 1.0);
        self.reverb_room   = self.reverb_room.clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AudioBackend { Default, Wasapi, Asio, PulseAudio, Alsa, CoreAudio }

// ── RenderConfig ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub bloom_enabled:        bool,
    pub bloom_intensity:      f32,
    /// Bloom radius in pixels (higher = wider glow).
    pub bloom_radius:         f32,
    pub distortion_enabled:   bool,
    pub motion_blur_enabled:  bool,
    /// Motion blur sample count (2-16).
    pub motion_blur_samples:  u32,
    pub chromatic_aberration: f32,
    pub film_grain:           f32,
    pub scanlines_enabled:    bool,
    pub scanline_intensity:   f32,
    pub font_size:            u32,
    pub fullscreen:           bool,
    /// Render scale (1.0 = native, 0.5 = half res).
    pub render_scale:         f32,
    /// Particle system multiplier (1.0 = full, 0.5 = half particles).
    pub particle_multiplier:  f32,
    pub shadow_quality:       ShadowQuality,
    /// Enable anti-aliasing (FXAA approximation for terminal renderer).
    pub antialiasing:         bool,
    /// Color depth per channel for dithering: 8, 16, or 32.
    pub color_depth:          u8,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            bloom_enabled:        true,
            bloom_intensity:      0.0,
            bloom_radius:         0.0,
            distortion_enabled:   false,
            motion_blur_enabled:  false,
            motion_blur_samples:  2,
            chromatic_aberration: 0.0,
            film_grain:           0.0,
            scanlines_enabled:    false,
            scanline_intensity:   0.15,
            font_size:            32,
            fullscreen:           false,
            render_scale:         1.0,
            particle_multiplier:  1.0,
            shadow_quality:       ShadowQuality::Medium,
            antialiasing:         true,
            color_depth:          8,
        }
    }
}

impl RenderConfig {
    pub fn validate(&mut self) {
        self.bloom_intensity      = self.bloom_intensity.clamp(0.0, 5.0);
        self.bloom_radius         = self.bloom_radius.clamp(1.0, 32.0);
        self.chromatic_aberration = self.chromatic_aberration.clamp(0.0, 0.05);
        self.film_grain           = self.film_grain.clamp(0.0, 0.5);
        self.scanline_intensity   = self.scanline_intensity.clamp(0.0, 1.0);
        self.font_size            = self.font_size.clamp(8, 64);
        self.render_scale         = self.render_scale.clamp(0.25, 2.0);
        self.particle_multiplier  = self.particle_multiplier.clamp(0.0, 4.0);
        self.motion_blur_samples  = self.motion_blur_samples.clamp(1, 16);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ShadowQuality { Off, Low, Medium, High, Ultra }

// ── PhysicsConfig ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    /// Fixed timestep for physics simulation (seconds).
    pub fixed_dt:         f32,
    /// Maximum physics sub-steps per frame.
    pub max_sub_steps:    u32,
    /// Fluid simulation grid size (cells per axis).
    pub fluid_grid_size:  usize,
    /// Soft body position correction iterations.
    pub soft_body_iters:  usize,
    /// Gravity vector (Y-down = negative).
    pub gravity_y:        f32,
    /// Collision detection broadphase strategy.
    pub broadphase:       BroadphaseStrategy,
    /// Enable sleeping for idle bodies.
    pub sleep_enabled:    bool,
    /// Velocity threshold below which bodies can sleep.
    pub sleep_threshold:  f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            fixed_dt:        1.0 / 60.0,
            max_sub_steps:   4,
            fluid_grid_size: 64,
            soft_body_iters: 4,
            gravity_y:       -9.8,
            broadphase:      BroadphaseStrategy::Grid,
            sleep_enabled:   true,
            sleep_threshold: 0.01,
        }
    }
}

impl PhysicsConfig {
    pub fn validate(&mut self) {
        self.fixed_dt        = self.fixed_dt.clamp(1.0 / 240.0, 1.0 / 15.0);
        self.max_sub_steps   = self.max_sub_steps.clamp(1, 16);
        self.fluid_grid_size = self.fluid_grid_size.clamp(8, 256);
        self.soft_body_iters = self.soft_body_iters.clamp(1, 32);
        self.sleep_threshold = self.sleep_threshold.clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum BroadphaseStrategy { BruteForce, Grid, BvhTree }

// ── InputConfig ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Mouse sensitivity multiplier.
    pub mouse_sensitivity: f32,
    /// Mouse Y-axis invert.
    pub mouse_invert_y: bool,
    /// Deadzone for analog gamepad axes.
    pub gamepad_deadzone: f32,
    /// Rumble intensity (0.0-1.0).
    pub gamepad_rumble: f32,
    /// Enable key repeat for held keys.
    pub key_repeat: bool,
    /// Key repeat initial delay (seconds).
    pub key_repeat_delay: f32,
    /// Key repeat rate (repeats/second).
    pub key_repeat_rate: f32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 1.0,
            mouse_invert_y:    false,
            gamepad_deadzone:  0.15,
            gamepad_rumble:    0.7,
            key_repeat:        true,
            key_repeat_delay:  0.4,
            key_repeat_rate:   30.0,
        }
    }
}

// ── DebugConfig ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    pub show_fps:         bool,
    pub show_frame_graph: bool,
    pub show_physics:     bool,
    pub show_spawn_zones: bool,
    pub show_entity_ids:  bool,
    pub show_force_fields: bool,
    pub show_particle_count: bool,
    pub log_level:        LogLevel,
    /// Cap the log output to this many bytes per second to avoid spam.
    pub log_rate_limit:   usize,
    /// Enable Tracy/puffin profiler integration.
    pub profiler_enabled: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            show_fps:            false,
            show_frame_graph:    false,
            show_physics:        false,
            show_spawn_zones:    false,
            show_entity_ids:     false,
            show_force_fields:   false,
            show_particle_count: false,
            log_level:           LogLevel::Info,
            log_rate_limit:      1_000_000,
            profiler_enabled:    false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum LogLevel { Off, Error, Warn, Info, Debug, Trace }

// ── GameplayConfig ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameplayConfig {
    /// Seed for the world generator (0 = random).
    pub world_seed: u64,
    /// Starting dungeon depth.
    pub start_depth: u32,
    /// Global difficulty multiplier (0.5 = easy, 1.0 = normal, 2.0 = brutal).
    pub difficulty: f32,
    /// Enable permadeath.
    pub permadeath: bool,
    /// Auto-save every N seconds (0 = disabled).
    pub autosave_interval: u32,
    /// Enable developer cheats.
    pub cheats_enabled: bool,
    /// Entity tick budget per frame (max entities updated per frame).
    pub entity_tick_budget: usize,
    /// Maximum number of active enemies.
    pub max_enemies: usize,
}

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            world_seed:         0,
            start_depth:        1,
            difficulty:         1.0,
            permadeath:         false,
            autosave_interval:  60,
            cheats_enabled:     false,
            entity_tick_budget: 256,
            max_enemies:        64,
        }
    }
}

// ── AccessibilityConfig ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    /// Enable colorblind compensation mode.
    pub colorblind_mode:   ColorblindMode,
    /// Global text size multiplier.
    pub text_size:         f32,
    /// Reduce motion (disables screen shake, reduces particles).
    pub reduce_motion:     bool,
    /// High contrast mode (brighter UI, bolder outlines).
    pub high_contrast:     bool,
    /// Screen flash warning suppression (warn when flash > this intensity).
    pub flash_warning:     f32,
    /// Enable subtitles / audio cues for deaf accessibility.
    pub audio_cues_visual: bool,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            colorblind_mode:   ColorblindMode::None,
            text_size:         1.0,
            reduce_motion:     false,
            high_contrast:     false,
            flash_warning:     0.5,
            audio_cues_visual: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ColorblindMode { None, Deuteranopia, Protanopia, Tritanopia, Achromatopsia }

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let c = EngineConfig::default();
        assert_eq!(c.window_width,  1280);
        assert_eq!(c.window_height, 800);
        assert_eq!(c.target_fps,    60);
        assert!(c.audio.enabled);
        assert!(c.render.bloom_enabled);
    }

    #[test]
    fn test_validate_clamps() {
        let mut c = EngineConfig::default();
        c.window_width  = 99999;
        c.window_height = 0;
        c.render.bloom_intensity = 100.0;
        c.validate();
        assert_eq!(c.window_width, 7680);
        assert_eq!(c.window_height, 240);
        assert!(c.render.bloom_intensity <= 5.0);
    }

    #[test]
    fn test_apply_args_width_height() {
        let mut c = EngineConfig::default();
        c.apply_args(&["--width".to_string(), "1920".to_string(),
                       "--height".to_string(), "1080".to_string()]);
        assert_eq!(c.window_width,  1920);
        assert_eq!(c.window_height, 1080);
    }

    #[test]
    fn test_apply_args_no_audio() {
        let mut c = EngineConfig::default();
        c.apply_args(&["--no-audio".to_string()]);
        assert!(!c.audio.enabled);
    }

    #[test]
    fn test_apply_args_no_postfx() {
        let mut c = EngineConfig::default();
        c.apply_args(&["--no-postfx".to_string()]);
        assert!(!c.render.bloom_enabled);
        assert_eq!(c.render.chromatic_aberration, 0.0);
    }

    #[test]
    fn test_profile_low_end() {
        let c = EngineConfig::profile_low_end();
        assert!(!c.render.bloom_enabled);
        assert_eq!(c.target_fps, 30);
    }

    #[test]
    fn test_profile_steam_deck() {
        let c = EngineConfig::profile_steam_deck();
        assert_eq!(c.window_width, 1280);
        assert_eq!(c.window_height, 800);
    }

    #[test]
    fn test_aspect_ratio() {
        let c = EngineConfig::default(); // 1280x800
        let ar = c.aspect_ratio();
        assert!((ar - 1.6).abs() < 0.01);
    }

    #[test]
    fn test_diff_detects_changes() {
        let a = EngineConfig::default();
        let mut b = a.clone();
        b.window_width = 1920;
        let diff = a.diff(&b);
        assert!(diff.window_changed);
        assert!(!diff.audio_changed);
    }

    #[test]
    fn test_round_trip_toml() {
        let c = EngineConfig::default();
        let toml_str = toml::to_string_pretty(&c).expect("serialize");
        let c2: EngineConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(c.window_width, c2.window_width);
        assert_eq!(c.target_fps,   c2.target_fps);
    }
}
