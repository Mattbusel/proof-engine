//! Engine configuration.

use serde::{Deserialize, Serialize};

/// Top-level engine configuration, loadable from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub target_fps: u32,
    pub vsync: bool,
    pub audio: AudioConfig,
    pub render: RenderConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window_title: "Proof Engine".to_string(),
            window_width: 1280,
            window_height: 800,
            target_fps: 60,
            vsync: true,
            audio: AudioConfig::default(),
            render: RenderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub enabled: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self { enabled: true, master_volume: 1.0, music_volume: 0.6, sfx_volume: 0.8 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
    pub distortion_enabled: bool,
    pub motion_blur_enabled: bool,
    pub chromatic_aberration: f32,
    pub film_grain: f32,
    pub scanlines_enabled: bool,
    pub font_size: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            bloom_enabled: true,
            bloom_intensity: 1.0,
            distortion_enabled: true,
            motion_blur_enabled: true,
            chromatic_aberration: 0.002,
            film_grain: 0.02,
            scanlines_enabled: false,
            font_size: 16,
        }
    }
}

impl EngineConfig {
    pub fn load(path: &str) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }
}
