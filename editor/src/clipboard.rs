//! Clipboard — copy/paste scene nodes.

use crate::scene::SceneNode;

pub struct Clipboard {
    pub nodes: Vec<SceneNode>,
}

impl Clipboard {
    pub fn new() -> Self { Self { nodes: Vec::new() } }

    pub fn copy(&mut self, nodes: &[SceneNode]) {
        self.nodes = nodes.to_vec();
    }

    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }
    pub fn clear(&mut self) { self.nodes.clear(); }
}
