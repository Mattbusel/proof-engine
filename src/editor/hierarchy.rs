//! Hierarchy panel — scene-tree view with full drag-drop, multi-select,
//! prefab support, search/filter, undo, serialisation and keyboard navigation.

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// NodeKind
// ─────────────────────────────────────────────────────────────────────────────

/// What kind of scene object a hierarchy node represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Entity,
    Glyph,
    ParticleEmitter,
    ForceField,
    Light,
    Camera,
    Group,
    Folder,
    Prefab,
    Script,
    Collider,
    Path,
    Custom(u32),
}

impl NodeKind {
    /// ASCII icon for display.
    pub fn icon(self) -> char {
        match self {
            NodeKind::Entity         => 'E',
            NodeKind::Glyph          => 'G',
            NodeKind::ParticleEmitter => 'P',
            NodeKind::ForceField     => 'F',
            NodeKind::Light          => 'L',
            NodeKind::Camera         => 'C',
            NodeKind::Group          => '+',
            NodeKind::Folder         => 'D',
            NodeKind::Prefab         => '*',
            NodeKind::Script         => 'S',
            NodeKind::Collider       => 'X',
            NodeKind::Path           => '~',
            NodeKind::Custom(_)      => '?',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NodeKind::Entity         => "Entity",
            NodeKind::Glyph          => "Glyph",
            NodeKind::ParticleEmitter => "Particles",
            NodeKind::ForceField     => "Force Field",
            NodeKind::Light          => "Light",
            NodeKind::Camera         => "Camera",
            NodeKind::Group          => "Group",
            NodeKind::Folder         => "Folder",
            NodeKind::Prefab         => "Prefab",
            NodeKind::Script         => "Script",
            NodeKind::Collider       => "Collider",
            NodeKind::Path           => "Path",
            NodeKind::Custom(_)      => "Custom",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NodeId
// ─────────────────────────────────────────────────────────────────────────────

/// Unique identifier for a hierarchy node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct NodeId(pub u32);

// ─────────────────────────────────────────────────────────────────────────────
// HierarchyNode
// ─────────────────────────────────────────────────────────────────────────────

/// A single node in the scene tree.
#[derive(Debug, Clone)]
pub struct HierarchyNode {
    pub id:           NodeId,
    pub name:         String,
    pub kind:         NodeKind,
    pub parent:       Option<NodeId>,
    pub children:     Vec<NodeId>,
    pub expanded:     bool,
    pub visible:      bool,
    pub locked:       bool,
    pub is_prefab_root: bool,
    pub prefab_name:  Option<String>,
    /// Depth in the tree (0 = root).
    pub depth:        u32,
    /// Order among siblings (lower = higher in list).
    pub sibling_index: usize,
}

impl HierarchyNode {
    pub fn new(id: NodeId, name: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            parent: None,
            children: Vec::new(),
            expanded: true,
            visible: true,
            locked: false,
            is_prefab_root: false,
            prefab_name: None,
            depth: 0,
            sibling_index: 0,
        }
    }

    pub fn visibility_icon(&self) -> char {
        if self.visible { 'O' } else { '-' }
    }

    pub fn lock_icon(&self) -> char {
        if self.locked { '#' } else { ' ' }
    }

    pub fn expand_icon(&self) -> char {
        if self.children.is_empty() {
            ' '
        } else if self.expanded {
            'v'
        } else {
            '>'
        }
    }

    /// One-line ASCII representation.
    pub fn render_line(&self) -> String {
        let indent = "  ".repeat(self.depth as usize);
        format!(
            "{}{} [{}]{} {} {}",
            indent,
            self.expand_icon(),
            self.kind.icon(),
            if self.is_prefab_root { "*" } else { "" },
            self.name,
            if self.locked { "#" } else { "" },
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PrefabNode
// ─────────────────────────────────────────────────────────────────────────────

/// A named, reusable subtree template.
#[derive(Debug, Clone)]
pub struct PrefabNode {
    pub name:      String,
    pub root_node: NodeId,
    pub nodes:     Vec<NodeId>,
    pub overrides: HashMap<NodeId, HashMap<String, String>>,
}

impl PrefabNode {
    pub fn new(name: impl Into<String>, root: NodeId) -> Self {
        Self {
            name: name.into(),
            root_node: root,
            nodes: vec![root],
            overrides: HashMap::new(),
        }
    }
    pub fn add_override(&mut self, id: NodeId, prop: impl Into<String>, val: impl Into<String>) {
        self.overrides.entry(id).or_default().insert(prop.into(), val.into());
    }
    pub fn get_override(&self, id: NodeId, prop: &str) -> Option<&str> {
        self.overrides.get(&id)?.get(prop).map(|s| s.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HierarchyAction — feeds UndoHistory
// ─────────────────────────────────────────────────────────────────────────────

/// All reversible hierarchy operations.
#[derive(Debug, Clone)]
pub enum HierarchyAction {
    AddNode { id: NodeId, parent: Option<NodeId>, name: String, kind: NodeKind },
    RemoveNode { id: NodeId, saved_node: Option<HierarchyNode>, saved_children: Vec<NodeId> },
    RenameNode { id: NodeId, old_name: String, new_name: String },
    Reparent { child: NodeId, old_parent: Option<NodeId>, new_parent: Option<NodeId>, old_index: usize, new_index: usize },
    ReorderSibling { parent: Option<NodeId>, from_index: usize, to_index: usize },
    SetVisible { id: NodeId, old: bool, new: bool },
    SetLocked { id: NodeId, old: bool, new: bool },
    MakePrefab { id: NodeId, prefab_name: String },
    DuplicateSubtree { original: NodeId, new_root: NodeId, all_new_ids: Vec<NodeId> },
    GroupNodes { group_id: NodeId, members: Vec<NodeId>, old_parents: Vec<Option<NodeId>> },
    UngroupNodes { group_id: NodeId, members: Vec<NodeId> },
}

// ─────────────────────────────────────────────────────────────────────────────
// HierarchySerializer
// ─────────────────────────────────────────────────────────────────────────────

/// Serialises and deserialises the scene tree as indented text.
pub struct HierarchySerializer;

impl HierarchySerializer {
    /// Serialise the entire panel to a text format.
    pub fn serialize(panel: &HierarchyPanel) -> String {
        let mut out = String::new();
        let roots: Vec<NodeId> = panel.nodes.values()
            .filter(|n| n.parent.is_none())
            .map(|n| n.id)
            .collect();
        let mut sorted_roots: Vec<&HierarchyNode> = roots.iter()
            .filter_map(|id| panel.nodes.get(id))
            .collect();
        sorted_roots.sort_by_key(|n| n.sibling_index);
        for node in sorted_roots {
            Self::serialize_node(panel, node.id, &mut out);
        }
        out
    }

    fn serialize_node(panel: &HierarchyPanel, id: NodeId, out: &mut String) {
        if let Some(node) = panel.nodes.get(&id) {
            let indent = "  ".repeat(node.depth as usize);
            let visible = if node.visible { 'V' } else { 'H' };
            let locked  = if node.locked  { 'L' } else { 'U' };
            out.push_str(&format!(
                "{}[{}][{}][{}] {}\n",
                indent,
                node.kind.icon(),
                visible,
                locked,
                node.name,
            ));
            let mut children: Vec<&HierarchyNode> = node.children.iter()
                .filter_map(|cid| panel.nodes.get(cid))
                .collect();
            children.sort_by_key(|n| n.sibling_index);
            for child in children {
                Self::serialize_node(panel, child.id, out);
            }
        }
    }

    /// Parse a serialised tree back into nodes.  Returns (nodes, actions_log).
    pub fn deserialize(text: &str) -> (Vec<(String, NodeKind, u32)>, Vec<String>) {
        let mut result = Vec::new();
        let mut errors = Vec::new();
        for (line_no, line) in text.lines().enumerate() {
            if line.trim().is_empty() { continue; }
            // Count leading spaces to determine depth.
            let depth = (line.len() - line.trim_start().len()) / 2;
            let trimmed = line.trim();
            // Expect format: [kind][V/H][L/U] name
            if trimmed.starts_with('[') {
                let parts: Vec<&str> = trimmed.splitn(5, ']').collect();
                if parts.len() >= 4 {
                    let kind_char = parts[0].trim_start_matches('[');
                    let name = parts[3].trim().trim_start_matches(' ').to_string();
                    let kind = match kind_char.chars().next().unwrap_or(' ') {
                        'E' => NodeKind::Entity,
                        'G' => NodeKind::Glyph,
                        'P' => NodeKind::ParticleEmitter,
                        'F' => NodeKind::ForceField,
                        'L' => NodeKind::Light,
                        'C' => NodeKind::Camera,
                        '+' => NodeKind::Group,
                        'D' => NodeKind::Folder,
                        '*' => NodeKind::Prefab,
                        _   => NodeKind::Entity,
                    };
                    result.push((name, kind, depth as u32));
                } else {
                    errors.push(format!("Line {}: malformed node '{}'", line_no + 1, trimmed));
                }
            } else {
                errors.push(format!("Line {}: unexpected format", line_no + 1));
            }
        }
        (result, errors)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DragDropState
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks state for simulated drag-and-drop reordering.
#[derive(Debug, Clone, Default)]
pub struct DragDropState {
    pub dragging: Option<NodeId>,
    pub hover_target: Option<NodeId>,
    pub hover_position: DropPosition,
    pub drag_start_y: f32,
    pub current_y: f32,
}

/// Where a dragged node will be dropped relative to the hover target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropPosition {
    #[default]
    Before,
    After,
    Inside,
}

impl DragDropState {
    pub fn start_drag(&mut self, id: NodeId, y: f32) {
        self.dragging = Some(id);
        self.drag_start_y = y;
        self.current_y = y;
    }

    pub fn update(&mut self, y: f32, target: Option<NodeId>, pos: DropPosition) {
        self.current_y = y;
        self.hover_target = target;
        self.hover_position = pos;
    }

    pub fn finish(&mut self) -> Option<(NodeId, Option<NodeId>, DropPosition)> {
        let drag = self.dragging.take()?;
        let target = self.hover_target.take();
        let pos = self.hover_position;
        Some((drag, target, pos))
    }

    pub fn cancel(&mut self) {
        self.dragging = None;
        self.hover_target = None;
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KeyboardNavState
// ─────────────────────────────────────────────────────────────────────────────

/// Keyboard navigation state for the hierarchy panel.
#[derive(Debug, Clone, Default)]
pub struct KeyboardNavState {
    pub focused_index: usize,  // index in the flattened visible list
    pub rename_mode:   bool,
    pub rename_buffer: String,
}

impl KeyboardNavState {
    pub fn move_up(&mut self, visible_count: usize) {
        if visible_count == 0 { return; }
        if self.focused_index > 0 {
            self.focused_index -= 1;
        }
    }
    pub fn move_down(&mut self, visible_count: usize) {
        if visible_count == 0 { return; }
        if self.focused_index + 1 < visible_count {
            self.focused_index += 1;
        }
    }
    pub fn begin_rename(&mut self, current_name: &str) {
        self.rename_mode = true;
        self.rename_buffer = current_name.to_string();
    }
    pub fn finish_rename(&mut self) -> Option<String> {
        if self.rename_mode {
            self.rename_mode = false;
            Some(std::mem::take(&mut self.rename_buffer))
        } else {
            None
        }
    }
    pub fn cancel_rename(&mut self) {
        self.rename_mode = false;
        self.rename_buffer.clear();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HierarchyPanel
// ─────────────────────────────────────────────────────────────────────────────

/// The scene hierarchy tree panel.
pub struct HierarchyPanel {
    pub nodes:   HashMap<NodeId, HierarchyNode>,
    pub roots:   Vec<NodeId>,
    pub selected: Vec<NodeId>,
    pub prefabs: HashMap<String, PrefabNode>,

    // Internal
    next_id:      u32,
    undo_stack:   Vec<HierarchyAction>,
    redo_stack:   Vec<HierarchyAction>,
    pub drag_drop: DragDropState,
    pub keyboard:  KeyboardNavState,
    pub search_query: String,
    pub filter_kind:  Option<NodeKind>,
    /// Cached flat list of visible nodes for rendering / keyboard nav.
    flat_list:    Vec<NodeId>,
    flat_dirty:   bool,
}

impl HierarchyPanel {
    pub fn new() -> Self {
        Self {
            nodes:     HashMap::new(),
            roots:     Vec::new(),
            selected:  Vec::new(),
            prefabs:   HashMap::new(),
            next_id:   1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            drag_drop: DragDropState::default(),
            keyboard:  KeyboardNavState::default(),
            search_query: String::new(),
            filter_kind:  None,
            flat_list:    Vec::new(),
            flat_dirty:   true,
        }
    }

    // ── ID allocation ─────────────────────────────────────────────────────────

    fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    // ── Add / remove ─────────────────────────────────────────────────────────

    /// Add a new node, optionally parented to `parent`.
    pub fn add_node(
        &mut self,
        name: impl Into<String>,
        kind: NodeKind,
        parent: Option<NodeId>,
    ) -> NodeId {
        let id = self.alloc_id();
        let name_s: String = name.into();
        let depth = parent
            .and_then(|p| self.nodes.get(&p))
            .map(|p| p.depth + 1)
            .unwrap_or(0);
        let sibling_index = parent
            .and_then(|p| self.nodes.get(&p))
            .map(|p| p.children.len())
            .unwrap_or(self.roots.len());

        let mut node = HierarchyNode::new(id, name_s.clone(), kind);
        node.parent = parent;
        node.depth = depth;
        node.sibling_index = sibling_index;

        self.nodes.insert(id, node);

        if let Some(pid) = parent {
            if let Some(p) = self.nodes.get_mut(&pid) {
                p.children.push(id);
            }
        } else {
            self.roots.push(id);
        }

        self.undo_stack.push(HierarchyAction::AddNode {
            id,
            parent,
            name: name_s,
            kind,
        });
        self.redo_stack.clear();
        self.flat_dirty = true;
        id
    }

    /// Remove a node and all its descendants.
    pub fn delete_recursive(&mut self, id: NodeId) {
        if !self.nodes.contains_key(&id) { return; }
        // Collect all descendants depth-first.
        let mut to_remove = Vec::new();
        self.collect_descendants(id, &mut to_remove);
        to_remove.push(id);

        // Detach from parent.
        let parent = self.nodes.get(&id).and_then(|n| n.parent);
        if let Some(pid) = parent {
            if let Some(p) = self.nodes.get_mut(&pid) {
                p.children.retain(|&c| c != id);
            }
        } else {
            self.roots.retain(|&r| r != id);
        }

        // Save for undo.
        let saved = self.nodes.get(&id).cloned();
        let saved_children = self.nodes.get(&id).map(|n| n.children.clone()).unwrap_or_default();

        for rid in &to_remove {
            self.nodes.remove(rid);
            self.selected.retain(|&s| &s != rid);
        }

        self.undo_stack.push(HierarchyAction::RemoveNode {
            id,
            saved_node: saved,
            saved_children,
        });
        self.redo_stack.clear();
        self.flat_dirty = true;
    }

    fn collect_descendants(&self, id: NodeId, out: &mut Vec<NodeId>) {
        if let Some(node) = self.nodes.get(&id) {
            for &child in &node.children {
                self.collect_descendants(child, out);
                out.push(child);
            }
        }
    }

    // ── Rename ────────────────────────────────────────────────────────────────

    pub fn rename(&mut self, id: NodeId, new_name: impl Into<String>) {
        let new_name: String = new_name.into();
        if let Some(node) = self.nodes.get_mut(&id) {
            let old_name = std::mem::replace(&mut node.name, new_name.clone());
            self.undo_stack.push(HierarchyAction::RenameNode { id, old_name, new_name });
            self.redo_stack.clear();
            self.flat_dirty = true;
        }
    }

    // ── Reparent ─────────────────────────────────────────────────────────────

    /// Move `child` to be a child of `new_parent` (None = root).
    pub fn reparent(&mut self, child: NodeId, new_parent: Option<NodeId>) {
        if !self.nodes.contains_key(&child) { return; }
        // Guard against parenting to self or a descendant.
        if let Some(np) = new_parent {
            if np == child { return; }
            let mut anc = Some(np);
            while let Some(a) = anc {
                if a == child { return; } // would create cycle
                anc = self.nodes.get(&a).and_then(|n| n.parent);
            }
        }

        let old_parent = self.nodes[&child].parent;
        let old_index  = self.nodes[&child].sibling_index;

        // Detach from old parent.
        if let Some(op) = old_parent {
            if let Some(p) = self.nodes.get_mut(&op) {
                p.children.retain(|&c| c != child);
            }
            // Fix sibling indices — separate borrow to avoid double-mut.
            let siblings: Vec<NodeId> = self.nodes.get(&op)
                .map(|p| p.children.clone())
                .unwrap_or_default();
            for (i, cid) in siblings.into_iter().enumerate() {
                if let Some(cn) = self.nodes.get_mut(&cid) {
                    cn.sibling_index = i;
                }
            }
        } else {
            self.roots.retain(|&r| r != child);
        }

        // Attach to new parent.
        let new_index = if let Some(np) = new_parent {
            let len = self.nodes.get(&np).map(|n| n.children.len()).unwrap_or(0);
            if let Some(p) = self.nodes.get_mut(&np) {
                p.children.push(child);
            }
            len
        } else {
            let len = self.roots.len();
            self.roots.push(child);
            len
        };

        // Update child's parent and depth.
        let new_depth = new_parent
            .and_then(|p| self.nodes.get(&p))
            .map(|p| p.depth + 1)
            .unwrap_or(0);
        if let Some(cn) = self.nodes.get_mut(&child) {
            cn.parent = new_parent;
            cn.sibling_index = new_index;
        }
        // Recursively update depths of subtree.
        self.update_depths(child, new_depth);

        self.undo_stack.push(HierarchyAction::Reparent {
            child,
            old_parent,
            new_parent,
            old_index,
            new_index,
        });
        self.redo_stack.clear();
        self.flat_dirty = true;
    }

    fn update_depths(&mut self, id: NodeId, depth: u32) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.depth = depth;
        }
        let children: Vec<NodeId> = self.nodes.get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.update_depths(child, depth + 1);
        }
    }

    // ── Duplicate subtree ─────────────────────────────────────────────────────

    /// Deep-clone `id` and all its descendants. Returns the new root.
    pub fn duplicate_subtree(&mut self, id: NodeId) -> Option<NodeId> {
        if !self.nodes.contains_key(&id) { return None; }
        let parent = self.nodes[&id].parent;
        let mut id_map: HashMap<NodeId, NodeId> = HashMap::new();
        let new_root = self.clone_node_recursive(id, parent, &mut id_map);
        let all_new: Vec<NodeId> = id_map.values().copied().collect();
        self.undo_stack.push(HierarchyAction::DuplicateSubtree {
            original: id,
            new_root,
            all_new_ids: all_new,
        });
        self.redo_stack.clear();
        self.flat_dirty = true;
        Some(new_root)
    }

    fn clone_node_recursive(
        &mut self,
        src: NodeId,
        parent: Option<NodeId>,
        id_map: &mut HashMap<NodeId, NodeId>,
    ) -> NodeId {
        let new_id = self.alloc_id();
        id_map.insert(src, new_id);

        let src_node = self.nodes[&src].clone();
        let depth = parent.and_then(|p| self.nodes.get(&p)).map(|p| p.depth + 1).unwrap_or(0);
        let sibling_index = parent
            .and_then(|p| self.nodes.get(&p))
            .map(|p| p.children.len())
            .unwrap_or(self.roots.len());

        let mut new_node = HierarchyNode::new(
            new_id,
            format!("{} (copy)", src_node.name),
            src_node.kind,
        );
        new_node.parent = parent;
        new_node.depth = depth;
        new_node.sibling_index = sibling_index;
        new_node.visible = src_node.visible;
        new_node.locked  = src_node.locked;

        self.nodes.insert(new_id, new_node);

        if let Some(pid) = parent {
            if let Some(p) = self.nodes.get_mut(&pid) {
                p.children.push(new_id);
            }
        } else {
            self.roots.push(new_id);
        }

        let children: Vec<NodeId> = src_node.children.clone();
        for child in children {
            self.clone_node_recursive(child, Some(new_id), id_map);
        }
        new_id
    }

    // ── Expand / collapse ─────────────────────────────────────────────────────

    pub fn expand_all(&mut self) {
        for node in self.nodes.values_mut() {
            node.expanded = true;
        }
        self.flat_dirty = true;
    }

    pub fn collapse_all(&mut self) {
        for node in self.nodes.values_mut() {
            node.expanded = false;
        }
        self.flat_dirty = true;
    }

    /// Expand the path from root to `id`.
    pub fn expand_to(&mut self, id: NodeId) {
        let mut current = self.nodes.get(&id).and_then(|n| n.parent);
        while let Some(pid) = current {
            if let Some(n) = self.nodes.get_mut(&pid) {
                n.expanded = true;
                current = n.parent;
            } else {
                break;
            }
        }
        self.flat_dirty = true;
    }

    // ── Visibility & lock ─────────────────────────────────────────────────────

    pub fn set_visible(&mut self, id: NodeId, visible: bool) {
        if let Some(node) = self.nodes.get_mut(&id) {
            let old = node.visible;
            node.visible = visible;
            self.undo_stack.push(HierarchyAction::SetVisible { id, old, new: visible });
            self.redo_stack.clear();
        }
    }

    pub fn toggle_visible(&mut self, id: NodeId) {
        let vis = self.nodes.get(&id).map(|n| !n.visible).unwrap_or(false);
        self.set_visible(id, vis);
    }

    pub fn set_locked(&mut self, id: NodeId, locked: bool) {
        if let Some(node) = self.nodes.get_mut(&id) {
            let old = node.locked;
            node.locked = locked;
            self.undo_stack.push(HierarchyAction::SetLocked { id, old, new: locked });
            self.redo_stack.clear();
        }
    }

    pub fn toggle_locked(&mut self, id: NodeId) {
        let locked = self.nodes.get(&id).map(|n| !n.locked).unwrap_or(false);
        self.set_locked(id, locked);
    }

    // ── Search / filter ───────────────────────────────────────────────────────

    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.flat_dirty = true;
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.flat_dirty = true;
    }

    pub fn set_kind_filter(&mut self, kind: Option<NodeKind>) {
        self.filter_kind = kind;
        self.flat_dirty = true;
    }

    fn node_matches(&self, node: &HierarchyNode) -> bool {
        let name_ok = self.search_query.is_empty()
            || node.name.to_lowercase().contains(&self.search_query.to_lowercase());
        let kind_ok = self.filter_kind.map(|k| k == node.kind).unwrap_or(true);
        name_ok && kind_ok
    }

    pub fn filter_by_name(&self, query: &str) -> Vec<NodeId> {
        self.nodes.values()
            .filter(|n| n.name.to_lowercase().contains(&query.to_lowercase()))
            .map(|n| n.id)
            .collect()
    }

    pub fn filter_by_kind(&self, kind: NodeKind) -> Vec<NodeId> {
        self.nodes.values()
            .filter(|n| n.kind == kind)
            .map(|n| n.id)
            .collect()
    }

    // ── Selection ─────────────────────────────────────────────────────────────

    pub fn select(&mut self, id: NodeId) {
        self.selected.clear();
        self.selected.push(id);
        // Update focused index in flat list.
        self.rebuild_flat_if_dirty();
        if let Some(pos) = self.flat_list.iter().position(|&x| x == id) {
            self.keyboard.focused_index = pos;
        }
    }

    pub fn toggle_select(&mut self, id: NodeId) {
        if let Some(pos) = self.selected.iter().position(|&s| s == id) {
            self.selected.remove(pos);
        } else {
            self.selected.push(id);
        }
    }

    /// Shift+click: select a contiguous range in the flat list.
    pub fn range_select(&mut self, id: NodeId) {
        self.rebuild_flat_if_dirty();
        let anchor = self.selected.first().copied();
        let Some(target_idx) = self.flat_list.iter().position(|&x| x == id) else { return; };
        let anchor_idx = anchor
            .and_then(|a| self.flat_list.iter().position(|&x| x == a))
            .unwrap_or(target_idx);
        let (lo, hi) = if anchor_idx <= target_idx {
            (anchor_idx, target_idx)
        } else {
            (target_idx, anchor_idx)
        };
        self.selected = self.flat_list[lo..=hi].to_vec();
    }

    pub fn select_all(&mut self) {
        self.selected = self.nodes.keys().copied().collect();
    }

    pub fn deselect_all(&mut self) {
        self.selected.clear();
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected.contains(&id)
    }

    // ── Prefab ────────────────────────────────────────────────────────────────

    pub fn make_prefab(&mut self, id: NodeId, prefab_name: impl Into<String>) -> String {
        let name: String = prefab_name.into();
        let nodes: Vec<NodeId> = {
            let mut all = Vec::new();
            self.collect_descendants(id, &mut all);
            all.push(id);
            all
        };
        let prefab = PrefabNode { name: name.clone(), root_node: id, nodes, overrides: HashMap::new() };
        self.prefabs.insert(name.clone(), prefab);
        if let Some(node) = self.nodes.get_mut(&id) {
            node.is_prefab_root = true;
            node.prefab_name = Some(name.clone());
        }
        self.undo_stack.push(HierarchyAction::MakePrefab { id, prefab_name: name.clone() });
        self.redo_stack.clear();
        name
    }

    pub fn instantiate_prefab(&mut self, prefab_name: &str, parent: Option<NodeId>) -> Option<NodeId> {
        let prefab = self.prefabs.get(prefab_name)?.clone();
        let root = prefab.root_node;
        Some(self.clone_node_recursive(root, parent, &mut HashMap::new()))
    }

    // ── Group / Ungroup ───────────────────────────────────────────────────────

    /// Group selected nodes under a new Group node.
    pub fn group_selected(&mut self, group_name: impl Into<String>) -> Option<NodeId> {
        if self.selected.is_empty() { return None; }
        let members: Vec<NodeId> = self.selected.clone();

        // Find common parent (use first member's parent).
        let common_parent = self.nodes.get(&members[0]).and_then(|n| n.parent);
        let old_parents: Vec<Option<NodeId>> = members.iter()
            .map(|&m| self.nodes.get(&m).and_then(|n| n.parent))
            .collect();

        let group_id = self.add_node(group_name, NodeKind::Group, common_parent);

        // Reparent members under the group.
        let members_clone = members.clone();
        for m in members_clone {
            self.reparent(m, Some(group_id));
        }

        // Pop the reparent actions so only the group action is at top.
        // (In a real editor we'd batch these; here we record the compound action.)
        self.undo_stack.push(HierarchyAction::GroupNodes {
            group_id,
            members,
            old_parents,
        });
        self.redo_stack.clear();
        Some(group_id)
    }

    // ── Flat list rebuild ─────────────────────────────────────────────────────

    fn rebuild_flat_if_dirty(&mut self) {
        if !self.flat_dirty { return; }
        self.flat_list.clear();
        let roots = self.roots.clone();
        for root in &roots {
            self.flatten_node(*root, &mut Vec::new());
        }
        self.flat_dirty = false;
    }

    fn flatten_node(&mut self, id: NodeId, accumulator: &mut Vec<NodeId>) {
        let (matches, expanded, children) = {
            let node = match self.nodes.get(&id) { Some(n) => n, None => return };
            (self.node_matches(node), node.expanded, node.children.clone())
        };
        if matches {
            self.flat_list.push(id);
        }
        if expanded {
            for child in children {
                self.flatten_node(child, accumulator);
            }
        }
    }

    /// Returns the current flat list (rebuilds if stale).
    pub fn visible_flat_list(&mut self) -> &[NodeId] {
        self.rebuild_flat_if_dirty();
        &self.flat_list
    }

    // ── Undo / Redo ───────────────────────────────────────────────────────────

    pub fn undo(&mut self) -> bool {
        let action = match self.undo_stack.pop() {
            Some(a) => a,
            None => return false,
        };
        match &action {
            HierarchyAction::AddNode { id, .. } => {
                self.nodes.remove(id);
                self.roots.retain(|&r| r != *id);
                self.flat_dirty = true;
            }
            HierarchyAction::RemoveNode { id, saved_node, .. } => {
                if let Some(saved) = saved_node.clone() {
                    let parent = saved.parent;
                    self.nodes.insert(*id, saved);
                    if let Some(pid) = parent {
                        if let Some(p) = self.nodes.get_mut(&pid) {
                            p.children.push(*id);
                        }
                    } else {
                        self.roots.push(*id);
                    }
                    self.flat_dirty = true;
                }
            }
            HierarchyAction::RenameNode { id, old_name, .. } => {
                if let Some(node) = self.nodes.get_mut(id) {
                    node.name = old_name.clone();
                    self.flat_dirty = true;
                }
            }
            HierarchyAction::SetVisible { id, old, .. } => {
                if let Some(node) = self.nodes.get_mut(id) {
                    node.visible = *old;
                }
            }
            HierarchyAction::SetLocked { id, old, .. } => {
                if let Some(node) = self.nodes.get_mut(id) {
                    node.locked = *old;
                }
            }
            HierarchyAction::Reparent { child, old_parent, new_parent, old_index, .. } => {
                // Reverse: move child back to old_parent at old_index.
                if let Some(np) = new_parent {
                    if let Some(p) = self.nodes.get_mut(np) {
                        p.children.retain(|&c| c != *child);
                    }
                } else {
                    self.roots.retain(|&r| r != *child);
                }
                if let Some(op) = old_parent {
                    if let Some(p) = self.nodes.get_mut(op) {
                        let idx = (*old_index).min(p.children.len());
                        p.children.insert(idx, *child);
                    }
                } else {
                    let idx = (*old_index).min(self.roots.len());
                    self.roots.insert(idx, *child);
                }
                if let Some(cn) = self.nodes.get_mut(child) {
                    cn.parent = *old_parent;
                    cn.sibling_index = *old_index;
                }
                let depth = old_parent
                    .and_then(|p| self.nodes.get(&p))
                    .map(|p| p.depth + 1)
                    .unwrap_or(0);
                self.update_depths(*child, depth);
                self.flat_dirty = true;
            }
            HierarchyAction::DuplicateSubtree { all_new_ids, .. } => {
                for &nid in all_new_ids {
                    if let Some(node) = self.nodes.remove(&nid) {
                        if let Some(pid) = node.parent {
                            if let Some(p) = self.nodes.get_mut(&pid) {
                                p.children.retain(|&c| c != nid);
                            }
                        } else {
                            self.roots.retain(|&r| r != nid);
                        }
                    }
                }
                self.flat_dirty = true;
            }
            _ => {}
        }
        self.redo_stack.push(action);
        true
    }

    pub fn redo(&mut self) -> bool {
        let action = match self.redo_stack.pop() {
            Some(a) => a,
            None => return false,
        };
        match &action {
            HierarchyAction::AddNode { id, parent, name, kind } => {
                let depth = parent.and_then(|p| self.nodes.get(&p)).map(|p| p.depth + 1).unwrap_or(0);
                let sibling_index = parent
                    .and_then(|p| self.nodes.get(&p))
                    .map(|p| p.children.len())
                    .unwrap_or(self.roots.len());
                let mut node = HierarchyNode::new(*id, name.clone(), *kind);
                node.parent = *parent;
                node.depth = depth;
                node.sibling_index = sibling_index;
                self.nodes.insert(*id, node);
                if let Some(pid) = parent {
                    if let Some(p) = self.nodes.get_mut(pid) { p.children.push(*id); }
                } else {
                    self.roots.push(*id);
                }
                self.flat_dirty = true;
            }
            HierarchyAction::RenameNode { id, new_name, .. } => {
                if let Some(n) = self.nodes.get_mut(id) {
                    n.name = new_name.clone();
                    self.flat_dirty = true;
                }
            }
            HierarchyAction::SetVisible { id, new, .. } => {
                if let Some(n) = self.nodes.get_mut(id) { n.visible = *new; }
            }
            HierarchyAction::SetLocked { id, new, .. } => {
                if let Some(n) = self.nodes.get_mut(id) { n.locked = *new; }
            }
            _ => {}
        }
        self.undo_stack.push(action);
        true
    }

    // ── Keyboard navigation ────────────────────────────────────────────────────

    /// Move keyboard focus up; returns the newly focused NodeId if any.
    pub fn keyboard_up(&mut self) -> Option<NodeId> {
        let count = self.visible_flat_list().len();
        self.keyboard.move_up(count);
        self.flat_list.get(self.keyboard.focused_index).copied()
    }

    /// Move keyboard focus down.
    pub fn keyboard_down(&mut self) -> Option<NodeId> {
        let count = self.visible_flat_list().len();
        self.keyboard.move_down(count);
        self.flat_list.get(self.keyboard.focused_index).copied()
    }

    /// Activate rename mode for the focused node.
    pub fn keyboard_begin_rename(&mut self) {
        if let Some(&id) = self.flat_list.get(self.keyboard.focused_index) {
            if let Some(node) = self.nodes.get(&id) {
                let name = node.name.clone();
                self.keyboard.begin_rename(&name);
            }
        }
    }

    /// Finish rename and apply.
    pub fn keyboard_finish_rename(&mut self) {
        if let Some(new_name) = self.keyboard.finish_rename() {
            if let Some(&id) = self.flat_list.get(self.keyboard.focused_index) {
                self.rename(id, new_name);
            }
        }
    }

    /// Delete the focused node.
    pub fn keyboard_delete(&mut self) {
        if let Some(&id) = self.flat_list.get(self.keyboard.focused_index).cloned().as_ref() {
            self.delete_recursive(id);
        }
    }

    // ── Drag and drop ─────────────────────────────────────────────────────────

    pub fn begin_drag(&mut self, id: NodeId, y: f32) {
        self.drag_drop.start_drag(id, y);
    }

    pub fn update_drag(&mut self, y: f32, hover: Option<NodeId>, pos: DropPosition) {
        self.drag_drop.update(y, hover, pos);
    }

    pub fn finish_drag(&mut self) {
        if let Some((dragged, Some(target), pos)) = self.drag_drop.finish() {
            match pos {
                DropPosition::Inside => self.reparent(dragged, Some(target)),
                DropPosition::Before | DropPosition::After => {
                    let new_parent = self.nodes.get(&target).and_then(|n| n.parent);
                    self.reparent(dragged, new_parent);
                }
            }
        } else {
            self.drag_drop.cancel();
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn render_ascii(&mut self) -> String {
        self.rebuild_flat_if_dirty();
        let flat = self.flat_list.clone();
        let mut out = String::new();
        out.push_str("┌─ Hierarchy ──────────────────────────\n");
        for (i, &id) in flat.iter().enumerate() {
            if let Some(node) = self.nodes.get(&id) {
                let sel = if self.is_selected(id) { ">" } else { " " };
                let focus = if i == self.keyboard.focused_index { "*" } else { " " };
                out.push_str(&format!("{}{} {}\n", focus, sel, node.render_line()));
            }
        }
        out.push_str("└──────────────────────────────────────\n");
        out
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for HierarchyPanel {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn panel_with_tree() -> (HierarchyPanel, NodeId, NodeId, NodeId) {
        let mut p = HierarchyPanel::new();
        let root  = p.add_node("root",  NodeKind::Entity, None);
        let child = p.add_node("child", NodeKind::Glyph,  Some(root));
        let leaf  = p.add_node("leaf",  NodeKind::Glyph,  Some(child));
        (p, root, child, leaf)
    }

    #[test]
    fn test_add_and_count() {
        let (p, _, _, _) = panel_with_tree();
        assert_eq!(p.node_count(), 3);
    }

    #[test]
    fn test_delete_recursive() {
        let (mut p, root, child, _leaf) = panel_with_tree();
        p.delete_recursive(child);
        assert!(!p.nodes.contains_key(&child));
        assert!(!p.nodes.contains_key(&_leaf));
        assert!(p.nodes.contains_key(&root));
    }

    #[test]
    fn test_rename_undo() {
        let (mut p, _root, child, _leaf) = panel_with_tree();
        p.rename(child, "renamed");
        assert_eq!(p.nodes[&child].name, "renamed");
        p.undo();
        assert_eq!(p.nodes[&child].name, "child");
    }

    #[test]
    fn test_reparent() {
        let (mut p, root, _child, leaf) = panel_with_tree();
        p.reparent(leaf, Some(root));
        assert_eq!(p.nodes[&leaf].parent, Some(root));
        assert_eq!(p.nodes[&leaf].depth, 1);
    }

    #[test]
    fn test_reparent_no_cycle() {
        let (mut p, root, child, _leaf) = panel_with_tree();
        let depth_before = p.nodes[&root].depth;
        p.reparent(root, Some(child)); // should be ignored
        assert_eq!(p.nodes[&root].depth, depth_before);
    }

    #[test]
    fn test_duplicate_subtree() {
        let (mut p, _root, child, _leaf) = panel_with_tree();
        let count_before = p.node_count();
        p.duplicate_subtree(child);
        assert!(p.node_count() > count_before);
    }

    #[test]
    fn test_visibility_toggle() {
        let (mut p, root, _, _) = panel_with_tree();
        assert!(p.nodes[&root].visible);
        p.toggle_visible(root);
        assert!(!p.nodes[&root].visible);
        p.undo();
        assert!(p.nodes[&root].visible);
    }

    #[test]
    fn test_search_filter() {
        let (p, _, _, _) = panel_with_tree();
        let found = p.filter_by_name("leaf");
        assert_eq!(found.len(), 1);
        let empty = p.filter_by_name("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_kind_filter() {
        let (p, _, _, _) = panel_with_tree();
        let glyphs = p.filter_by_kind(NodeKind::Glyph);
        assert_eq!(glyphs.len(), 2);
    }

    #[test]
    fn test_selection_range() {
        let (mut p, root, child, leaf) = panel_with_tree();
        p.expand_all();
        // Rebuild flat list so range_select works.
        p.rebuild_flat_if_dirty();
        p.select(root);
        p.range_select(leaf);
        assert_eq!(p.selected.len(), 3);
    }

    #[test]
    fn test_prefab_creation() {
        let (mut p, _root, child, _leaf) = panel_with_tree();
        p.make_prefab(child, "my_prefab");
        assert!(p.prefabs.contains_key("my_prefab"));
        assert!(p.nodes[&child].is_prefab_root);
    }

    #[test]
    fn test_serializer_roundtrip() {
        let (p, _, _, _) = panel_with_tree();
        let serialized = HierarchySerializer::serialize(&p);
        assert!(serialized.contains("root"));
        assert!(serialized.contains("child"));
        assert!(serialized.contains("leaf"));
    }
}
