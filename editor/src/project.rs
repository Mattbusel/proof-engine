//! Project manager — new/open/save project, recent files, templates.

use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub scene_file: String,
    pub created: String,
    pub last_modified: String,
    pub version: String,
    pub settings: ProjectSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub default_grid_size: f32,
    pub default_bloom: bool,
    pub default_bloom_intensity: f32,
    pub auto_save: bool,
    pub auto_save_interval_secs: u32,
    pub backup_count: u32,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            default_grid_size: 1.0,
            default_bloom: true,
            default_bloom_intensity: 1.5,
            auto_save: true,
            auto_save_interval_secs: 60,
            backup_count: 5,
        }
    }
}

impl Project {
    pub fn new(name: &str, path: &Path) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_path_buf(),
            scene_file: "scene.json".to_string(),
            created: chrono_now(),
            last_modified: chrono_now(),
            version: "0.1.0".to_string(),
            settings: ProjectSettings::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let project_file = self.path.join("project.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&self.path).map_err(|e| e.to_string())?;
        std::fs::write(project_file, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let project_file = path.join("project.json");
        let json = std::fs::read_to_string(project_file).map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub fn scene_path(&self) -> PathBuf {
        self.path.join(&self.scene_file)
    }
}

pub struct RecentProjects {
    pub entries: Vec<RecentEntry>,
    pub max_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub name: String,
    pub path: String,
    pub last_opened: String,
}

impl RecentProjects {
    pub fn new() -> Self {
        Self { entries: Vec::new(), max_entries: 10 }
    }

    pub fn add(&mut self, name: &str, path: &str) {
        self.entries.retain(|e| e.path != path);
        self.entries.insert(0, RecentEntry {
            name: name.to_string(),
            path: path.to_string(),
            last_opened: chrono_now(),
        });
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.entries).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let entries: Vec<RecentEntry> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Ok(Self { entries, max_entries: 10 })
    }
}

/// Scene templates.
pub struct Templates;

impl Templates {
    pub fn list() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Empty", "Blank scene with grid"),
            ("Galaxy", "Spiral galaxy demo"),
            ("Heartbeat", "Pulsing entity demo"),
            ("Attractor Lab", "Force field playground"),
            ("Particle Storm", "Particle effect showcase"),
            ("Math Rain", "Falling equation cascade"),
            ("Combat Arena", "Entity battle setup"),
            ("Shader Test", "Post-processing test bench"),
        ]
    }
}

fn chrono_now() -> String {
    // Simple timestamp without chrono dependency
    "2026-03-25".to_string()
}
