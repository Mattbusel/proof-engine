//! Settings, saved as TOML in `%APPDATA%\ProofSaver\settings.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the other monitors show when the saver spans more than one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OtherMonitors {
    /// Each monitor runs its own attractor, offset in the cycle.
    Different,
    /// Every monitor shows the same attractor.
    Same,
    /// Only the primary monitor draws; the rest stay black.
    Black,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `"cycle"`, or one attractor name from `scene::SCENES` (lower case).
    pub scene: String,
    /// Seconds each attractor stays on screen when cycling.
    pub scene_seconds: u32,
    /// Multiplier on how fast simulated time runs. 1.0 is normal.
    pub speed: f32,
    /// Frame rate cap. The saver sleeps between frames to hold it.
    pub fps_cap: u32,
    /// Show the attractor's name and equations for a few seconds.
    pub show_equations: bool,
    pub other_monitors: OtherMonitors,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scene: "cycle".into(),
            scene_seconds: 45,
            speed: 1.0,
            fps_cap: 30,
            show_equations: true,
            other_monitors: OtherMonitors::Different,
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir());
        base.join("ProofSaver").join("settings.toml")
    }

    /// Load the saved settings, or the defaults if there are none or they
    /// do not parse. Values are clamped to sane ranges either way.
    pub fn load() -> Self {
        let mut s: Self = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        s.sanitize();
        s
    }

    pub fn save(&self) -> std::io::Result<PathBuf> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, text)?;
        Ok(path)
    }

    pub fn sanitize(&mut self) {
        self.scene = self.scene.trim().to_ascii_lowercase();
        if self.scene != "cycle" && crate::scene::find(&self.scene).is_none() {
            self.scene = "cycle".into();
        }
        self.scene_seconds = self.scene_seconds.clamp(10, 3600);
        if !self.speed.is_finite() {
            self.speed = 1.0;
        }
        self.speed = self.speed.clamp(0.25, 3.0);
        self.fps_cap = self.fps_cap.clamp(10, 144);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_toml() {
        let s = Settings { scene: "aizawa".into(), speed: 0.6, ..Default::default() };
        let back: Settings = toml::from_str(&toml::to_string_pretty(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn garbage_is_clamped() {
        let mut s = Settings { scene: "nope".into(), speed: 99.0, fps_cap: 0, scene_seconds: 1, ..Default::default() };
        s.sanitize();
        assert_eq!(s.scene, "cycle");
        assert_eq!(s.speed, 3.0);
        assert_eq!(s.fps_cap, 10);
        assert_eq!(s.scene_seconds, 10);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = toml::from_str("scene = \"thomas\"").unwrap();
        assert_eq!(s.scene, "thomas");
        assert_eq!(s.fps_cap, Settings::default().fps_cap);
    }
}
