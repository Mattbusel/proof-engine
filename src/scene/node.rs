//! Scene nodes — typed wrappers for scene graph entries.

use glam::Vec3;

/// A node in the scene graph.
#[derive(Debug, Clone)]
pub struct SceneNode {
    pub position: Vec3,
    pub visible: bool,
    pub name: Option<String>,
}

impl SceneNode {
    pub fn new(position: Vec3) -> Self {
        Self { position, visible: true, name: None }
    }
}
