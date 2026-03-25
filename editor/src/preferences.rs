//! Editor preferences — persistent settings.

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorPrefs {
    pub grid_visible: bool,
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub snap_size: f32,
    pub auto_save: bool,
    pub auto_save_interval: f32,
    pub theme: String,
    pub font_size: f32,
    pub bloom_default: bool,
    pub bloom_intensity: f32,
    pub camera_speed: f32,
    pub undo_limit: usize,
    pub recent_files: Vec<String>,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            grid_visible: true, grid_size: 1.0, snap_to_grid: false, snap_size: 0.5,
            auto_save: true, auto_save_interval: 60.0,
            theme: "dark".to_string(), font_size: 14.0,
            bloom_default: true, bloom_intensity: 1.5,
            camera_speed: 12.0, undo_limit: 200,
            recent_files: Vec::new(),
        }
    }
}

impl EditorPrefs {
    pub fn save(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load(path: &str) -> Result<Self, String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub fn add_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
    }
}
