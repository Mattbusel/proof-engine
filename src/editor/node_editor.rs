
//! Generic node editor foundation — ports, connections, layout, zoom/pan, minimap.

use glam::{Vec2, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Port / pin definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortDirection { Input, Output }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortKind {
    Float, Float2, Float3, Float4,
    Int, Bool, Color,
    Texture2D, TextureCube, Sampler,
    Matrix2, Matrix3, Matrix4,
    String, Any, Flow, Object,
}

impl PortKind {
    pub fn color(self) -> Vec4 {
        match self {
            PortKind::Float => Vec4::new(0.6, 0.6, 0.6, 1.0),
            PortKind::Float2 => Vec4::new(0.4, 0.8, 0.4, 1.0),
            PortKind::Float3 => Vec4::new(0.4, 0.4, 0.9, 1.0),
            PortKind::Float4 => Vec4::new(0.8, 0.4, 0.8, 1.0),
            PortKind::Int => Vec4::new(0.4, 0.7, 0.9, 1.0),
            PortKind::Bool => Vec4::new(0.9, 0.7, 0.3, 1.0),
            PortKind::Color => Vec4::new(1.0, 0.8, 0.2, 1.0),
            PortKind::Texture2D | PortKind::TextureCube => Vec4::new(0.8, 0.5, 0.2, 1.0),
            PortKind::Sampler => Vec4::new(0.7, 0.4, 0.1, 1.0),
            PortKind::Matrix2 | PortKind::Matrix3 | PortKind::Matrix4 => Vec4::new(0.9, 0.2, 0.4, 1.0),
            PortKind::Flow => Vec4::new(1.0, 1.0, 1.0, 1.0),
            PortKind::Object => Vec4::new(0.5, 0.9, 0.9, 1.0),
            _ => Vec4::new(0.5, 0.5, 0.5, 1.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PortKind::Float => "float",
            PortKind::Float2 => "vec2",
            PortKind::Float3 => "vec3",
            PortKind::Float4 => "vec4",
            PortKind::Int => "int",
            PortKind::Bool => "bool",
            PortKind::Color => "color",
            PortKind::Texture2D => "Texture2D",
            PortKind::TextureCube => "TextureCube",
            PortKind::Sampler => "Sampler",
            PortKind::Matrix2 => "mat2",
            PortKind::Matrix3 => "mat3",
            PortKind::Matrix4 => "mat4",
            PortKind::String => "string",
            PortKind::Any => "any",
            PortKind::Flow => "flow",
            PortKind::Object => "object",
        }
    }

    pub fn can_connect_to(self, other: PortKind) -> bool {
        if self == other { return true; }
        if self == PortKind::Any || other == PortKind::Any { return true; }
        // Numeric promotions
        matches!((self, other),
            (PortKind::Float, PortKind::Float2) |
            (PortKind::Float, PortKind::Float3) |
            (PortKind::Float, PortKind::Float4) |
            (PortKind::Float, PortKind::Color) |
            (PortKind::Float3, PortKind::Color) |
            (PortKind::Float4, PortKind::Color) |
            (PortKind::Color, PortKind::Float3) |
            (PortKind::Color, PortKind::Float4) |
            (PortKind::Int, PortKind::Float)
        )
    }
}

#[derive(Debug, Clone)]
pub struct Port {
    pub id: u32,
    pub name: String,
    pub kind: PortKind,
    pub direction: PortDirection,
    pub optional: bool,
    pub default_value: Option<Vec4>,
    pub tooltip: String,
}

impl Port {
    pub fn input(id: u32, name: impl Into<String>, kind: PortKind) -> Self {
        Self {
            id, name: name.into(), kind, direction: PortDirection::Input,
            optional: false, default_value: None, tooltip: String::new(),
        }
    }
    pub fn output(id: u32, name: impl Into<String>, kind: PortKind) -> Self {
        Self {
            id, name: name.into(), kind, direction: PortDirection::Output,
            optional: false, default_value: None, tooltip: String::new(),
        }
    }
    pub fn with_default(mut self, v: Vec4) -> Self { self.default_value = Some(v); self }
    pub fn optional(mut self) -> Self { self.optional = true; self }
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeState {
    Normal,
    Selected,
    Hovered,
    Error,
    Warning,
    Disabled,
    Processing,
    Dirty,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    pub title: String,
    pub category: String,
    pub position: Vec2,
    pub size: Vec2,
    pub ports: Vec<Port>,
    pub state: NodeState,
    pub collapsed: bool,
    pub comment: Option<String>,
    pub color: Vec4,
    pub pinned: bool,
    pub metadata: HashMap<String, String>,
    pub execution_order: u32,
    pub last_processed_frame: u64,
    pub processing_time_us: u64,
}

impl Node {
    pub fn new(id: u32, title: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            category: category.into(),
            position: Vec2::ZERO,
            size: Vec2::new(150.0, 60.0),
            ports: Vec::new(),
            state: NodeState::Normal,
            collapsed: false,
            comment: None,
            color: Vec4::new(0.2, 0.2, 0.2, 1.0),
            pinned: false,
            metadata: HashMap::new(),
            execution_order: 0,
            last_processed_frame: 0,
            processing_time_us: 0,
        }
    }

    pub fn with_position(mut self, pos: Vec2) -> Self { self.position = pos; self }
    pub fn with_color(mut self, color: Vec4) -> Self { self.color = color; self }

    pub fn add_port(&mut self, port: Port) {
        self.ports.push(port);
        self.recalculate_size();
    }

    pub fn recalculate_size(&mut self) {
        let input_count = self.ports.iter().filter(|p| p.direction == PortDirection::Input).count();
        let output_count = self.ports.iter().filter(|p| p.direction == PortDirection::Output).count();
        let rows = input_count.max(output_count);
        self.size = Vec2::new(160.0, 40.0 + rows as f32 * 22.0);
    }

    pub fn port_position(&self, port_id: u32) -> Option<Vec2> {
        let inputs: Vec<u32> = self.ports.iter().filter(|p| p.direction == PortDirection::Input).map(|p| p.id).collect();
        let outputs: Vec<u32> = self.ports.iter().filter(|p| p.direction == PortDirection::Output).map(|p| p.id).collect();
        if let Some(i) = inputs.iter().position(|&id| id == port_id) {
            return Some(self.position + Vec2::new(0.0, 40.0 + i as f32 * 22.0));
        }
        if let Some(i) = outputs.iter().position(|&id| id == port_id) {
            return Some(self.position + Vec2::new(self.size.x, 40.0 + i as f32 * 22.0));
        }
        None
    }

    pub fn input_ports(&self) -> impl Iterator<Item = &Port> {
        self.ports.iter().filter(|p| p.direction == PortDirection::Input)
    }
    pub fn output_ports(&self) -> impl Iterator<Item = &Port> {
        self.ports.iter().filter(|p| p.direction == PortDirection::Output)
    }

    pub fn bounds(&self) -> [Vec2; 2] {
        [self.position, self.position + self.size]
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.position.x && p.x <= self.position.x + self.size.x &&
        p.y >= self.position.y && p.y <= self.position.y + self.size.y
    }
}

// ---------------------------------------------------------------------------
// Connection / edge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Connection {
    pub id: u32,
    pub from_node: u32,
    pub from_port: u32,
    pub to_node: u32,
    pub to_port: u32,
}

impl Connection {
    pub fn new(id: u32, from_node: u32, from_port: u32, to_node: u32, to_port: u32) -> Self {
        Self { id, from_node, from_port, to_node, to_port }
    }
}

// ---------------------------------------------------------------------------
// Node graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NodeGraph {
    pub name: String,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub next_node_id: u32,
    pub next_conn_id: u32,
    pub next_port_id: u32,
    pub dirty: bool,
}

impl NodeGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            connections: Vec::new(),
            next_node_id: 1,
            next_conn_id: 1,
            next_port_id: 1,
            dirty: false,
        }
    }

    pub fn add_node(&mut self, mut node: Node) -> u32 {
        let id = self.next_node_id;
        node.id = id;
        self.next_node_id += 1;
        self.nodes.push(node);
        self.dirty = true;
        id
    }

    pub fn remove_node(&mut self, id: u32) {
        self.nodes.retain(|n| n.id != id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        self.dirty = true;
    }

    pub fn connect(&mut self, from_node: u32, from_port: u32, to_node: u32, to_port: u32) -> Result<u32, &'static str> {
        // Validate nodes exist
        if self.node(from_node).is_none() { return Err("source node not found"); }
        if self.node(to_node).is_none() { return Err("target node not found"); }
        // Check for existing input connection (inputs can only have one source)
        self.connections.retain(|c| !(c.to_node == to_node && c.to_port == to_port));
        // Cycle check
        if self.would_create_cycle(from_node, to_node) {
            return Err("connection would create cycle");
        }
        let id = self.next_conn_id;
        self.next_conn_id += 1;
        self.connections.push(Connection::new(id, from_node, from_port, to_node, to_port));
        self.dirty = true;
        Ok(id)
    }

    pub fn disconnect(&mut self, conn_id: u32) {
        self.connections.retain(|c| c.id != conn_id);
        self.dirty = true;
    }

    pub fn would_create_cycle(&self, from: u32, to: u32) -> bool {
        // BFS from `to` — does it reach `from`?
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![to];
        while let Some(n) = queue.pop() {
            if n == from { return true; }
            if visited.contains(&n) { continue; }
            visited.insert(n);
            for c in &self.connections {
                if c.from_node == n { queue.push(c.to_node); }
            }
        }
        false
    }

    pub fn topological_order(&self) -> Vec<u32> {
        let mut in_degree: HashMap<u32, usize> = self.nodes.iter().map(|n| (n.id, 0)).collect();
        for conn in &self.connections {
            *in_degree.entry(conn.to_node).or_insert(0) += 1;
        }
        let mut queue: Vec<u32> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop() {
            order.push(n);
            for conn in &self.connections {
                if conn.from_node == n {
                    let d = in_degree.entry(conn.to_node).or_insert(1);
                    *d -= 1;
                    if *d == 0 { queue.push(conn.to_node); }
                }
            }
        }
        order
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn node_mut(&mut self, id: u32) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn connections_from(&self, node_id: u32) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(move |c| c.from_node == node_id)
    }

    pub fn connections_to(&self, node_id: u32) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(move |c| c.to_node == node_id)
    }

    pub fn upstream_nodes(&self, node_id: u32) -> Vec<u32> {
        self.connections.iter()
            .filter(|c| c.to_node == node_id)
            .map(|c| c.from_node)
            .collect()
    }

    pub fn downstream_nodes(&self, node_id: u32) -> Vec<u32> {
        self.connections.iter()
            .filter(|c| c.from_node == node_id)
            .map(|c| c.to_node)
            .collect()
    }

    pub fn selected_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| n.state == NodeState::Selected)
    }

    pub fn auto_layout(&mut self) {
        // Simple layered layout (Sugiyama-style approximation)
        let order = self.topological_order();
        let mut layers: HashMap<u32, u32> = HashMap::new();
        for &n in &order {
            let layer = self.connections_to(n)
                .filter_map(|c| layers.get(&c.from_node))
                .max()
                .map(|&l| l + 1)
                .unwrap_or(0);
            layers.insert(n, layer);
        }
        let mut layer_counts: HashMap<u32, u32> = HashMap::new();
        for n in self.nodes.iter_mut() {
            let layer = layers.get(&n.id).copied().unwrap_or(0);
            let row = *layer_counts.entry(layer).or_insert(0);
            *layer_counts.get_mut(&layer).unwrap() += 1;
            n.position = Vec2::new(layer as f32 * 220.0 + 20.0, row as f32 * 140.0 + 20.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Graph view / viewport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeEditorAction {
    None,
    DragNode(u32),
    DragConnection(u32, u32),   // from_node, from_port
    DragSelect,
    Pan,
    Zoom,
}

#[derive(Debug, Clone)]
pub struct NodeEditorView {
    pub offset: Vec2,
    pub zoom: f32,
    pub canvas_size: Vec2,
    pub selection_rect: Option<[Vec2; 2]>,
    pub active_action: NodeEditorAction,
    pub hovered_node: Option<u32>,
    pub hovered_port: Option<(u32, u32)>,
    pub pending_connection: Option<(u32, u32)>,
    pub show_minimap: bool,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub grid_size: f32,
    pub minimap_rect: [Vec2; 2],
}

impl NodeEditorView {
    pub fn new(canvas_size: Vec2) -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
            canvas_size,
            selection_rect: None,
            active_action: NodeEditorAction::None,
            hovered_node: None,
            hovered_port: None,
            pending_connection: None,
            show_minimap: true,
            show_grid: true,
            snap_to_grid: false,
            grid_size: 20.0,
            minimap_rect: [
                Vec2::new(canvas_size.x - 200.0, canvas_size.y - 150.0),
                Vec2::new(canvas_size.x - 10.0, canvas_size.y - 10.0),
            ],
        }
    }

    pub fn screen_to_canvas(&self, screen: Vec2) -> Vec2 {
        (screen - self.canvas_size * 0.5 - self.offset) / self.zoom
    }

    pub fn canvas_to_screen(&self, canvas: Vec2) -> Vec2 {
        canvas * self.zoom + self.canvas_size * 0.5 + self.offset
    }

    pub fn zoom_at(&mut self, screen_pivot: Vec2, factor: f32) {
        let canvas_pivot = self.screen_to_canvas(screen_pivot);
        self.zoom = (self.zoom * factor).clamp(0.1, 8.0);
        let new_screen = self.canvas_to_screen(canvas_pivot);
        self.offset += screen_pivot - new_screen;
    }

    pub fn pan(&mut self, delta: Vec2) {
        self.offset += delta;
    }

    pub fn fit_to_graph(&mut self, graph: &NodeGraph) {
        if graph.nodes.is_empty() { return; }
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for node in &graph.nodes {
            min = min.min(node.position);
            max = max.max(node.position + node.size);
        }
        let content_size = max - min;
        let scale_x = (self.canvas_size.x - 80.0) / content_size.x.max(1.0);
        let scale_y = (self.canvas_size.y - 80.0) / content_size.y.max(1.0);
        self.zoom = scale_x.min(scale_y).min(1.5);
        let center = (min + max) * 0.5;
        self.offset = -center * self.zoom;
    }

    pub fn snap_position(&self, pos: Vec2) -> Vec2 {
        if !self.snap_to_grid { return pos; }
        let g = self.grid_size;
        Vec2::new((pos.x / g).round() * g, (pos.y / g).round() * g)
    }

    pub fn nodes_in_selection(&self, graph: &NodeGraph) -> Vec<u32> {
        if let Some([a, b]) = self.selection_rect {
            let min = a.min(b);
            let max = a.max(b);
            let canvas_min = self.screen_to_canvas(min);
            let canvas_max = self.screen_to_canvas(max);
            graph.nodes.iter()
                .filter(|n| n.position.x < canvas_max.x && n.position.x + n.size.x > canvas_min.x &&
                            n.position.y < canvas_max.y && n.position.y + n.size.y > canvas_min.y)
                .map(|n| n.id)
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn cubic_bezier_connection(from: Vec2, to: Vec2) -> [Vec2; 4] {
        let dx = (to.x - from.x).abs().max(50.0) * 0.5;
        let cp1 = Vec2::new(from.x + dx, from.y);
        let cp2 = Vec2::new(to.x - dx, to.y);
        [from, cp1, cp2, to]
    }
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub separator_after: bool,
    pub action: String,
    pub children: Vec<ContextMenuItem>,
}

impl ContextMenuItem {
    pub fn action(label: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            enabled: true,
            separator_after: false,
            action: action.into(),
            children: Vec::new(),
        }
    }
    pub fn separator() -> Self {
        Self { label: "---".into(), shortcut: None, enabled: false, separator_after: false, action: String::new(), children: Vec::new() }
    }
    pub fn submenu(label: impl Into<String>, children: Vec<ContextMenuItem>) -> Self {
        Self { label: label.into(), shortcut: None, enabled: true, separator_after: false, action: String::new(), children }
    }
}

// ---------------------------------------------------------------------------
// Node editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NodeEditor {
    pub graphs: Vec<NodeGraph>,
    pub active_graph: usize,
    pub view: NodeEditorView,
    pub clipboard: Vec<Node>,
    pub search_query: String,
    pub show_context_menu: bool,
    pub context_menu_pos: Vec2,
    pub context_menu_items: Vec<ContextMenuItem>,
    pub undo_stack: Vec<NodeGraph>,
    pub undo_pos: usize,
    pub status_message: String,
    pub node_categories: Vec<String>,
}

impl NodeEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            graphs: vec![NodeGraph::new("Main")],
            active_graph: 0,
            view: NodeEditorView::new(Vec2::new(1200.0, 800.0)),
            clipboard: Vec::new(),
            search_query: String::new(),
            show_context_menu: false,
            context_menu_pos: Vec2::ZERO,
            context_menu_items: Vec::new(),
            undo_stack: Vec::new(),
            undo_pos: 0,
            status_message: String::new(),
            node_categories: vec!["Math".into(), "Logic".into(), "Color".into(), "Texture".into(), "Utility".into()],
        };
        ed.populate_demo_graph();
        ed
    }

    fn populate_demo_graph(&mut self) {
        let g = &mut self.graphs[0];
        let mut n1 = Node::new(0, "Add", "Math");
        n1.add_port(Port::input(1, "A", PortKind::Float).with_default(Vec4::ZERO));
        n1.add_port(Port::input(2, "B", PortKind::Float).with_default(Vec4::ZERO));
        n1.add_port(Port::output(3, "Result", PortKind::Float));
        n1.position = Vec2::new(200.0, 100.0);
        let id1 = g.add_node(n1);

        let mut n2 = Node::new(0, "Multiply", "Math");
        n2.add_port(Port::input(1, "A", PortKind::Float));
        n2.add_port(Port::input(2, "B", PortKind::Float));
        n2.add_port(Port::output(3, "Result", PortKind::Float));
        n2.position = Vec2::new(400.0, 100.0);
        let id2 = g.add_node(n2);

        let mut n3 = Node::new(0, "Output", "Output");
        n3.add_port(Port::input(1, "Value", PortKind::Float));
        n3.position = Vec2::new(600.0, 100.0);
        n3.color = Vec4::new(0.1, 0.35, 0.1, 1.0);
        let id3 = g.add_node(n3);

        let _ = g.connect(id1, 3, id2, 1);
        let _ = g.connect(id2, 3, id3, 1);
    }

    pub fn active_graph(&self) -> &NodeGraph {
        &self.graphs[self.active_graph]
    }

    pub fn active_graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graphs[self.active_graph]
    }

    pub fn snapshot(&mut self) {
        let graph = self.active_graph().clone();
        self.undo_stack.truncate(self.undo_pos);
        self.undo_stack.push(graph);
        self.undo_pos = self.undo_stack.len();
    }

    pub fn undo(&mut self) {
        if self.undo_pos > 1 {
            self.undo_pos -= 1;
            self.graphs[self.active_graph] = self.undo_stack[self.undo_pos - 1].clone();
        }
    }

    pub fn redo(&mut self) {
        if self.undo_pos < self.undo_stack.len() {
            self.graphs[self.active_graph] = self.undo_stack[self.undo_pos].clone();
            self.undo_pos += 1;
        }
    }

    pub fn copy_selected(&mut self) {
        self.clipboard = self.active_graph().selected_nodes().cloned().collect();
    }

    pub fn paste(&mut self) {
        if self.clipboard.is_empty() { return; }
        self.snapshot();
        let nodes: Vec<Node> = self.clipboard.clone();
        let g = self.active_graph_mut();
        for node in nodes {
            let mut n = node;
            n.position += Vec2::new(20.0, 20.0);
            n.state = NodeState::Selected;
            g.add_node(n);
        }
    }

    pub fn delete_selected(&mut self) {
        self.snapshot();
        let to_remove: Vec<u32> = self.active_graph().selected_nodes().map(|n| n.id).collect();
        let g = self.active_graph_mut();
        for id in to_remove {
            g.remove_node(id);
        }
    }

    pub fn select_all(&mut self) {
        for n in self.active_graph_mut().nodes.iter_mut() {
            n.state = NodeState::Selected;
        }
    }

    pub fn deselect_all(&mut self) {
        for n in self.active_graph_mut().nodes.iter_mut() {
            if n.state == NodeState::Selected {
                n.state = NodeState::Normal;
            }
        }
    }

    pub fn auto_layout(&mut self) {
        self.snapshot();
        self.active_graph_mut().auto_layout();
        let ai = self.active_graph;
        self.view.fit_to_graph(&self.graphs[ai]);
    }

    pub fn build_context_menu(&mut self, pos: Vec2) {
        self.context_menu_pos = pos;
        self.show_context_menu = true;
        let canvas_pos = self.view.screen_to_canvas(pos);
        let on_node = self.active_graph().nodes.iter().any(|n| n.contains_point(canvas_pos));
        if on_node {
            self.context_menu_items = vec![
                ContextMenuItem::action("Delete", "delete_selected"),
                ContextMenuItem::action("Duplicate", "duplicate"),
                ContextMenuItem::separator(),
                ContextMenuItem::action("Collapse", "collapse"),
                ContextMenuItem::action("Add Comment", "add_comment"),
                ContextMenuItem::separator(),
                ContextMenuItem::action("Properties", "properties"),
            ];
        } else {
            self.context_menu_items = vec![
                ContextMenuItem::submenu("Add Node", vec![
                    ContextMenuItem::action("Add", "add_add_node"),
                    ContextMenuItem::action("Multiply", "add_mul_node"),
                    ContextMenuItem::action("Constant", "add_const_node"),
                ]),
                ContextMenuItem::separator(),
                ContextMenuItem::action("Select All", "select_all"),
                ContextMenuItem::action("Auto Layout", "auto_layout"),
                ContextMenuItem::separator(),
                ContextMenuItem::action("Paste", "paste"),
            ];
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_compatibility() {
        assert!(PortKind::Float.can_connect_to(PortKind::Float3));
        assert!(!PortKind::Texture2D.can_connect_to(PortKind::Float));
        assert!(PortKind::Any.can_connect_to(PortKind::Matrix4));
    }

    #[test]
    fn test_graph_cycle_detection() {
        let mut g = NodeGraph::new("test");
        let n1 = g.add_node(Node::new(0, "A", "test"));
        let n2 = g.add_node(Node::new(0, "B", "test"));
        let _ = g.connect(n1, 0, n2, 0);
        assert!(g.would_create_cycle(n2, n1));
        assert!(!g.would_create_cycle(n1, n2));
    }

    #[test]
    fn test_topo_sort() {
        let mut g = NodeGraph::new("test");
        let n1 = g.add_node(Node::new(0, "A", "test"));
        let n2 = g.add_node(Node::new(0, "B", "test"));
        let n3 = g.add_node(Node::new(0, "C", "test"));
        let _ = g.connect(n1, 0, n2, 0);
        let _ = g.connect(n2, 0, n3, 0);
        let order = g.topological_order();
        let pos1 = order.iter().position(|&x| x == n1).unwrap();
        let pos2 = order.iter().position(|&x| x == n2).unwrap();
        assert!(pos1 < pos2);
    }

    #[test]
    fn test_view_transform() {
        let mut view = NodeEditorView::new(Vec2::new(800.0, 600.0));
        view.zoom = 2.0;
        let canvas = Vec2::new(10.0, 20.0);
        let screen = view.canvas_to_screen(canvas);
        let back = view.screen_to_canvas(screen);
        assert!((back.x - canvas.x).abs() < 0.001);
    }

    #[test]
    fn test_node_editor() {
        let mut ed = NodeEditor::new();
        assert!(!ed.active_graph().nodes.is_empty());
        ed.select_all();
        ed.copy_selected();
        ed.paste();
        let count = ed.active_graph().nodes.len();
        assert!(count > 3);
    }
}
