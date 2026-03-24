//! Scene nodes — typed wrappers for the scene graph with parent-child transforms.
//!
//! Each node has a local transform (position, rotation_z, scale). The world
//! transform is computed by walking the parent chain. Dirty flags prevent
//! redundant world-transform recomputation.

use glam::{Mat4, Vec3};

// ── Transform ─────────────────────────────────────────────────────────────────

/// A 3-DOF local transform stored compactly.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position:   Vec3,
    pub rotation_z: f32,   // radians, applied around Z axis (2D-first engine)
    pub scale:      Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position:   Vec3::ZERO,
        rotation_z: 0.0,
        scale:      Vec3::ONE,
    };

    pub fn from_position(position: Vec3) -> Self {
        Self { position, ..Self::IDENTITY }
    }

    pub fn from_pos_scale(position: Vec3, scale: f32) -> Self {
        Self { position, scale: Vec3::splat(scale), ..Self::IDENTITY }
    }

    /// Build a 4×4 local-to-parent matrix.
    pub fn to_matrix(&self) -> Mat4 {
        let t = Mat4::from_translation(self.position);
        let r = Mat4::from_rotation_z(self.rotation_z);
        let s = Mat4::from_scale(self.scale);
        t * r * s
    }

    /// Interpolate toward another transform (for smooth animation).
    pub fn lerp_toward(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            position:   self.position.lerp(other.position, t),
            rotation_z: self.rotation_z + (other.rotation_z - self.rotation_z) * t,
            scale:      self.scale.lerp(other.scale, t),
        }
    }
}

impl Default for Transform {
    fn default() -> Self { Self::IDENTITY }
}

// ── Node ──────────────────────────────────────────────────────────────────────

/// Unique ID for a scene node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

/// A scene graph node with parent-child transform hierarchy.
#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id:             NodeId,
    pub name:           Option<String>,
    pub local:          Transform,
    pub visible:        bool,
    /// Whether world_transform needs recomputation.
    dirty:              bool,
    /// Cached world-space transform matrix.
    world_transform:    Mat4,
    pub parent:         Option<NodeId>,
    pub children:       Vec<NodeId>,
    /// Arbitrary tag for queries (e.g. "player", "enemy", "ui").
    pub tag:            Option<String>,
    /// Draw order within the same layer (lower = drawn first).
    pub sort_key:       i32,
    /// User-defined metadata slot (e.g. entity index).
    pub user_data:      u64,
}

impl SceneNode {
    pub fn new(id: NodeId, position: Vec3) -> Self {
        Self {
            id,
            name: None,
            local: Transform::from_position(position),
            visible: true,
            dirty: true,
            world_transform: Mat4::IDENTITY,
            parent: None,
            children: Vec::new(),
            tag: None,
            sort_key: 0,
            user_data: 0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn with_sort_key(mut self, key: i32) -> Self {
        self.sort_key = key;
        self
    }

    /// Set local position and mark dirty.
    pub fn set_position(&mut self, pos: Vec3) {
        self.local.position = pos;
        self.dirty = true;
    }

    /// Set local rotation (Z axis) and mark dirty.
    pub fn set_rotation_z(&mut self, angle: f32) {
        self.local.rotation_z = angle;
        self.dirty = true;
    }

    /// Set uniform local scale and mark dirty.
    pub fn set_scale(&mut self, scale: f32) {
        self.local.scale = Vec3::splat(scale);
        self.dirty = true;
    }

    /// Set the full local transform and mark dirty.
    pub fn set_transform(&mut self, t: Transform) {
        self.local = t;
        self.dirty = true;
    }

    /// Compute and cache the world transform given the parent's world matrix.
    /// Returns whether recomputation occurred.
    pub fn update_world_transform(&mut self, parent_world: &Mat4) -> bool {
        if !self.dirty { return false; }
        self.world_transform = *parent_world * self.local.to_matrix();
        self.dirty = false;
        true
    }

    /// Get the cached world transform matrix.
    pub fn world_matrix(&self) -> &Mat4 { &self.world_transform }

    /// World-space position (column 3 of world matrix).
    pub fn world_position(&self) -> Vec3 {
        self.world_transform.w_axis.truncate()
    }

    /// World-space scale (magnitude of columns 0, 1, 2).
    pub fn world_scale(&self) -> Vec3 {
        Vec3::new(
            self.world_transform.x_axis.truncate().length(),
            self.world_transform.y_axis.truncate().length(),
            self.world_transform.z_axis.truncate().length(),
        )
    }

    /// Mark this node (and implicitly its subtree) as needing world-transform update.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Returns true if the node's world transform needs recomputation.
    pub fn is_dirty(&self) -> bool { self.dirty }

    /// Translate local position by delta.
    pub fn translate(&mut self, delta: Vec3) {
        self.local.position += delta;
        self.dirty = true;
    }

    /// Rotate local transform by angle (radians, around Z).
    pub fn rotate_z(&mut self, angle: f32) {
        self.local.rotation_z += angle;
        self.dirty = true;
    }

    pub fn is_visible(&self) -> bool { self.visible }
    pub fn set_visible(&mut self, v: bool) { self.visible = v; }
}

// ── Scene graph ───────────────────────────────────────────────────────────────

/// A flat scene graph with parent-child node relationships.
pub struct SceneGraph {
    nodes:    Vec<SceneNode>,
    next_id:  u64,
    roots:    Vec<NodeId>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), next_id: 1, roots: Vec::new() }
    }

    /// Allocate a new root node.
    pub fn create_root(&mut self, position: Vec3) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        self.roots.push(id);
        self.nodes.push(SceneNode::new(id, position));
        id
    }

    /// Allocate a new node with a parent.
    pub fn create_child(&mut self, parent: NodeId, position: Vec3) -> Option<NodeId> {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        let mut node = SceneNode::new(id, position);
        node.parent = Some(parent);

        // Record child on parent
        if let Some(p) = self.get_mut(parent) {
            p.children.push(id);
        } else {
            return None;
        }

        self.nodes.push(node);
        Some(id)
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&SceneNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Remove a node and all its descendants. Returns the count of removed nodes.
    pub fn remove_subtree(&mut self, id: NodeId) -> usize {
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let nid = to_remove[i];
            if let Some(node) = self.get(nid) {
                to_remove.extend(node.children.iter().copied());
            }
            i += 1;
        }
        let removed = to_remove.len();
        self.nodes.retain(|n| !to_remove.contains(&n.id));
        self.roots.retain(|r| !to_remove.contains(r));
        removed
    }

    /// Update world transforms for all dirty nodes (depth-first from roots).
    pub fn flush_transforms(&mut self) {
        let roots: Vec<NodeId> = self.roots.clone();
        for root in roots {
            self.flush_subtree(root, &Mat4::IDENTITY);
        }
    }

    fn flush_subtree(&mut self, id: NodeId, parent_world: &Mat4) {
        let children: Vec<NodeId>;
        let new_world;
        {
            let node = match self.nodes.iter_mut().find(|n| n.id == id) {
                Some(n) => n,
                None    => return,
            };
            node.update_world_transform(parent_world);
            new_world = *node.world_matrix();
            children = node.children.clone();
        }
        for child in children {
            self.flush_subtree(child, &new_world);
        }
    }

    /// Find all nodes with a specific tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<NodeId> {
        self.nodes.iter()
            .filter(|n| n.tag.as_deref() == Some(tag))
            .map(|n| n.id)
            .collect()
    }

    /// Find all visible nodes sorted by sort_key.
    pub fn visible_sorted(&self) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.nodes.iter()
            .filter(|n| n.visible)
            .map(|n| n.id)
            .collect();
        ids.sort_by_key(|&id| {
            self.get(id).map(|n| n.sort_key).unwrap_or(0)
        });
        ids
    }

    /// Number of nodes in the graph.
    pub fn len(&self) -> usize { self.nodes.len() }
    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }

    /// Iterate over all nodes.
    pub fn iter(&self) -> impl Iterator<Item = &SceneNode> { self.nodes.iter() }
}

impl Default for SceneGraph {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_root_node() {
        let mut graph = SceneGraph::new();
        let id = graph.create_root(Vec3::new(1.0, 2.0, 0.0));
        graph.flush_transforms();
        let node = graph.get(id).unwrap();
        let wp = node.world_position();
        assert!((wp.x - 1.0).abs() < 0.001);
        assert!((wp.y - 2.0).abs() < 0.001);
    }

    #[test]
    fn child_inherits_parent_transform() {
        let mut graph = SceneGraph::new();
        let root = graph.create_root(Vec3::new(5.0, 0.0, 0.0));
        let child = graph.create_child(root, Vec3::new(1.0, 0.0, 0.0)).unwrap();
        graph.flush_transforms();
        let wp = graph.get(child).unwrap().world_position();
        // World position = parent(5,0,0) + local(1,0,0) = (6,0,0)
        assert!((wp.x - 6.0).abs() < 0.001);
    }

    #[test]
    fn dirty_flag_cleared_after_flush() {
        let mut graph = SceneGraph::new();
        let id = graph.create_root(Vec3::ZERO);
        graph.flush_transforms();
        assert!(!graph.get(id).unwrap().is_dirty());
        graph.get_mut(id).unwrap().translate(Vec3::X);
        assert!(graph.get(id).unwrap().is_dirty());
    }

    #[test]
    fn remove_subtree() {
        let mut graph = SceneGraph::new();
        let root  = graph.create_root(Vec3::ZERO);
        let child = graph.create_child(root, Vec3::X).unwrap();
        let _gc   = graph.create_child(child, Vec3::Y).unwrap();
        assert_eq!(graph.len(), 3);
        graph.remove_subtree(child);
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn find_by_tag() {
        let mut graph = SceneGraph::new();
        let id = graph.create_root(Vec3::ZERO);
        graph.get_mut(id).unwrap().tag = Some("player".to_string());
        let results = graph.find_by_tag("player");
        assert_eq!(results, vec![id]);
    }
}
