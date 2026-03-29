//! In-Engine Editor — master state, camera, undo/redo, grid, shortcuts.
//!
//! This module is the entry point for the editor subsystem.  It owns the
//! top-level `EditorState` and wires together all sub-panels.

pub mod inspector;
pub mod hierarchy;
pub mod console;
pub mod gizmos;
pub mod sdf_node_editor;
pub mod material_painter;
pub mod bone_rigger;
pub mod timeline;
pub mod kit_panel;
pub mod asset_browser;
pub mod camera_controller;
pub mod perf_overlay;
pub mod scene_io;
pub mod shader_graph;
pub mod viewport;
pub mod curve_editor;
pub mod physics_debug;
pub mod localization;
pub mod scripting;
pub mod profiler;
pub mod prefab;
pub mod terrain;
pub mod audio_editor;
pub mod post_fx;
pub mod light_probe;
pub mod particle_editor;
pub mod color_grading;
pub mod render_pipeline;
pub mod anim_retarget;
pub mod lod_manager;
pub mod material_editor;
pub mod node_editor;
pub mod animation_state_machine;
pub mod scene_editor;
pub mod cinematic_editor;
pub mod nav_mesh;
pub mod font_editor;
pub mod ui_canvas;
pub mod vfx_graph;
pub mod gpu_profiler;
pub mod network_editor;
pub mod input_editor;
pub mod build_system;
pub mod event_editor;
pub mod world_editor;
pub mod ai_behavior_editor;
pub mod physics_editor;
pub mod render_graph_editor;
pub mod dialogue_editor;
pub mod quest_editor;
pub mod spline_editor;
pub mod cinematic_sequencer;
pub mod inventory_editor;
pub mod ability_editor;
pub mod level_streaming_editor;
pub mod audio_mixer_editor;
pub mod modeling_editor;
pub mod animation_compression;
pub mod voxel_editor;
pub mod terrain_road_tool;
pub mod cutscene_importer;
pub mod map_editor;
pub mod particle_system_editor;
pub mod shader_compiler;
pub mod loot_editor;

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ─── re-export sub-types so callers can `use proof_engine::editor::*` ────────
pub use inspector::Inspector;
pub use hierarchy::HierarchyPanel;
pub use console::DevConsole;
pub use gizmos::GizmoRenderer;

// ─────────────────────────────────────────────────────────────────────────────
// Primitive identifiers (mirrored here so editor doesn't depend on internals)
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque handle to a scene entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EntityId(pub u32);

/// Opaque handle to a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GlyphId(pub u32);

// ─────────────────────────────────────────────────────────────────────────────
// EditorMode
// ─────────────────────────────────────────────────────────────────────────────

/// The current operational mode of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    /// Scene is running at full speed.
    Play,
    /// Scene is frozen; user edits geometry / properties.
    Edit,
    /// Scene is running at reduced speed or frame-by-frame.
    Pause,
}

impl Default for EditorMode {
    fn default() -> Self {
        EditorMode::Edit
    }
}

impl std::fmt::Display for EditorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorMode::Play => write!(f, "PLAY"),
            EditorMode::Edit => write!(f, "EDIT"),
            EditorMode::Pause => write!(f, "PAUSE"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Persistent editor preferences.
#[derive(Debug, Clone)]
pub struct EditorConfig {
    pub font_size: f32,
    pub theme: EditorTheme,
    pub show_gizmos: bool,
    pub show_grid: bool,
    pub snap_enabled: bool,
    pub snap_size: f32,
    pub auto_save_interval_secs: f32,
    pub max_undo_history: usize,
    pub panel_opacity: f32,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            theme: EditorTheme::Dark,
            show_gizmos: true,
            show_grid: true,
            snap_enabled: false,
            snap_size: 0.5,
            auto_save_interval_secs: 60.0,
            max_undo_history: 200,
            panel_opacity: 0.92,
        }
    }
}

/// Visual colour scheme for the editor UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTheme {
    Dark,
    Light,
    HighContrast,
    Solarized,
}

impl EditorTheme {
    /// Returns (background, foreground, accent) as Vec4 RGBA.
    pub fn colors(self) -> (Vec4, Vec4, Vec4) {
        match self {
            EditorTheme::Dark => (
                Vec4::new(0.10, 0.10, 0.12, 1.0),
                Vec4::new(0.90, 0.90, 0.90, 1.0),
                Vec4::new(0.25, 0.55, 1.00, 1.0),
            ),
            EditorTheme::Light => (
                Vec4::new(0.95, 0.95, 0.95, 1.0),
                Vec4::new(0.05, 0.05, 0.05, 1.0),
                Vec4::new(0.10, 0.40, 0.85, 1.0),
            ),
            EditorTheme::HighContrast => (
                Vec4::new(0.00, 0.00, 0.00, 1.0),
                Vec4::new(1.00, 1.00, 1.00, 1.0),
                Vec4::new(1.00, 1.00, 0.00, 1.0),
            ),
            EditorTheme::Solarized => (
                Vec4::new(0.00, 0.17, 0.21, 1.0),
                Vec4::new(0.63, 0.63, 0.60, 1.0),
                Vec4::new(0.52, 0.60, 0.00, 1.0),
            ),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorCamera
// ─────────────────────────────────────────────────────────────────────────────

/// Free-fly camera that is completely independent of the game camera.
/// Controlled by WASD + mouse look in the editor viewport.
#[derive(Debug, Clone)]
pub struct EditorCamera {
    pub position: Vec3,
    pub yaw: f32,   // radians, horizontal rotation
    pub pitch: f32, // radians, vertical rotation
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
    /// Whether the camera is currently being piloted (right-mouse held).
    pub active: bool,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 5.0, 10.0),
            yaw: 0.0,
            pitch: -0.3,
            move_speed: 5.0,
            look_sensitivity: 0.003,
            fov_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
            active: false,
        }
    }
}

impl EditorCamera {
    pub fn new() -> Self {
        Self::default()
    }

    /// Direction the camera is currently facing.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    /// Right vector perpendicular to forward and world-up.
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    /// Up vector.
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize()
    }

    /// Apply mouse delta to yaw/pitch.
    pub fn rotate(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * self.look_sensitivity;
        self.pitch -= delta_y * self.look_sensitivity;
        self.pitch = self.pitch.clamp(-1.5, 1.5);
    }

    /// Move the camera in local space.  `input` is (right, up, forward) signed.
    pub fn translate(&mut self, input: Vec3, dt: f32) {
        let fwd = self.forward();
        let right = self.right();
        let up = self.up();
        let speed = self.move_speed * dt;
        self.position += right * input.x * speed;
        self.position += up * input.y * speed;
        self.position += fwd * input.z * speed;
    }

    /// Build a view matrix (row-major, compatible with glam).
    pub fn view_matrix(&self) -> glam::Mat4 {
        let target = self.position + self.forward();
        glam::Mat4::look_at_rh(self.position, target, Vec3::Y)
    }

    /// Build a projection matrix.
    pub fn projection_matrix(&self, aspect: f32) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(
            self.fov_degrees.to_radians(),
            aspect,
            self.near,
            self.far,
        )
    }

    /// Focus the camera on a world-space point.
    pub fn focus_on(&mut self, target: Vec3) {
        let offset = Vec3::new(3.0, 3.0, 5.0);
        self.position = target + offset;
        let dir = (target - self.position).normalize();
        self.pitch = dir.y.asin();
        self.yaw = dir.z.atan2(dir.x);
    }

    /// Orbit around a pivot point by delta angles.
    pub fn orbit(&mut self, pivot: Vec3, d_yaw: f32, d_pitch: f32) {
        let dist = (self.position - pivot).length();
        self.yaw += d_yaw;
        self.pitch = (self.pitch + d_pitch).clamp(-1.5, 1.5);
        self.position = pivot - self.forward() * dist;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SelectionSet
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks which entities / glyphs are currently selected in the editor.
#[derive(Debug, Clone, Default)]
pub struct SelectionSet {
    pub entities: Vec<EntityId>,
    pub glyphs: Vec<GlyphId>,
}

impl SelectionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.glyphs.is_empty()
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        self.glyphs.clear();
    }

    /// Select a single entity (replaces current selection).
    pub fn select_entity(&mut self, id: EntityId) {
        self.clear();
        self.entities.push(id);
    }

    /// Toggle an entity in/out of the selection (Ctrl+click).
    pub fn toggle_entity(&mut self, id: EntityId) {
        if let Some(pos) = self.entities.iter().position(|&e| e == id) {
            self.entities.remove(pos);
        } else {
            self.entities.push(id);
        }
    }

    /// Add a range of entity ids (Shift+click).
    pub fn add_entity_range(&mut self, ids: &[EntityId]) {
        for &id in ids {
            if !self.entities.contains(&id) {
                self.entities.push(id);
            }
        }
    }

    /// Select a single glyph.
    pub fn select_glyph(&mut self, id: GlyphId) {
        self.clear();
        self.glyphs.push(id);
    }

    /// Toggle a glyph.
    pub fn toggle_glyph(&mut self, id: GlyphId) {
        if let Some(pos) = self.glyphs.iter().position(|&g| g == id) {
            self.glyphs.remove(pos);
        } else {
            self.glyphs.push(id);
        }
    }

    /// Returns the primary selected entity (first in list), if any.
    pub fn primary_entity(&self) -> Option<EntityId> {
        self.entities.first().copied()
    }

    /// Returns the primary selected glyph (first in list), if any.
    pub fn primary_glyph(&self) -> Option<GlyphId> {
        self.glyphs.first().copied()
    }

    /// Total number of selected items.
    pub fn count(&self) -> usize {
        self.entities.len() + self.glyphs.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UndoHistory — command pattern
// ─────────────────────────────────────────────────────────────────────────────

/// Trait that every undoable editor command must implement.
pub trait EditorCommand: std::fmt::Debug + Send + Sync {
    /// Human-readable name for display in the undo stack.
    fn name(&self) -> &str;
    /// Apply / re-apply the command.
    fn execute(&mut self, state: &mut EditorState);
    /// Reverse the command.
    fn undo(&mut self, state: &mut EditorState);
}

// ── Concrete commands ─────────────────────────────────────────────────────────

/// Move one or more entities by a delta vector.
#[derive(Debug)]
pub struct MoveEntityCommand {
    pub entity_ids: Vec<EntityId>,
    pub delta: Vec3,
}

impl EditorCommand for MoveEntityCommand {
    fn name(&self) -> &str {
        "Move Entity"
    }
    fn execute(&mut self, state: &mut EditorState) {
        for &id in &self.entity_ids {
            if let Some(pos) = state.entity_positions.get_mut(&id) {
                *pos += self.delta;
            }
        }
    }
    fn undo(&mut self, state: &mut EditorState) {
        for &id in &self.entity_ids {
            if let Some(pos) = state.entity_positions.get_mut(&id) {
                *pos -= self.delta;
            }
        }
    }
}

/// Spawn an entity at a given position.
#[derive(Debug)]
pub struct SpawnEntityCommand {
    pub id: EntityId,
    pub position: Vec3,
    pub name: String,
    pub executed: bool,
}

impl EditorCommand for SpawnEntityCommand {
    fn name(&self) -> &str {
        "Spawn Entity"
    }
    fn execute(&mut self, state: &mut EditorState) {
        state.entity_positions.insert(self.id, self.position);
        state.entity_names.insert(self.id, self.name.clone());
        self.executed = true;
    }
    fn undo(&mut self, state: &mut EditorState) {
        state.entity_positions.remove(&self.id);
        state.entity_names.remove(&self.id);
        self.executed = false;
    }
}

/// Delete an entity from the scene.
#[derive(Debug)]
pub struct DeleteEntityCommand {
    pub id: EntityId,
    pub saved_position: Option<Vec3>,
    pub saved_name: Option<String>,
}

impl EditorCommand for DeleteEntityCommand {
    fn name(&self) -> &str {
        "Delete Entity"
    }
    fn execute(&mut self, state: &mut EditorState) {
        self.saved_position = state.entity_positions.remove(&self.id);
        self.saved_name = state.entity_names.remove(&self.id);
        state.selection.entities.retain(|&e| e != self.id);
    }
    fn undo(&mut self, state: &mut EditorState) {
        if let Some(pos) = self.saved_position {
            state.entity_positions.insert(self.id, pos);
        }
        if let Some(ref name) = self.saved_name {
            state.entity_names.insert(self.id, name.clone());
        }
    }
}

/// Set a named string property on an entity.
#[derive(Debug)]
pub struct SetPropertyCommand {
    pub entity_id: EntityId,
    pub property: String,
    pub old_value: String,
    pub new_value: String,
}

impl EditorCommand for SetPropertyCommand {
    fn name(&self) -> &str {
        "Set Property"
    }
    fn execute(&mut self, state: &mut EditorState) {
        state
            .entity_properties
            .entry(self.entity_id)
            .or_default()
            .insert(self.property.clone(), self.new_value.clone());
    }
    fn undo(&mut self, state: &mut EditorState) {
        state
            .entity_properties
            .entry(self.entity_id)
            .or_default()
            .insert(self.property.clone(), self.old_value.clone());
    }
}

/// Group multiple entities together under a shared label.
#[derive(Debug)]
pub struct GroupSelectionCommand {
    pub group_id: EntityId,
    pub members: Vec<EntityId>,
    pub group_name: String,
    pub executed: bool,
}

impl EditorCommand for GroupSelectionCommand {
    fn name(&self) -> &str {
        "Group Selection"
    }
    fn execute(&mut self, state: &mut EditorState) {
        state
            .entity_names
            .insert(self.group_id, self.group_name.clone());
        state
            .entity_groups
            .insert(self.group_id, self.members.clone());
        self.executed = true;
    }
    fn undo(&mut self, state: &mut EditorState) {
        state.entity_names.remove(&self.group_id);
        state.entity_groups.remove(&self.group_id);
        self.executed = false;
    }
}

// ── UndoHistory ───────────────────────────────────────────────────────────────

/// Ring-buffer undo/redo stack (max 200 entries).
pub struct UndoHistory {
    commands: Vec<Box<dyn EditorCommand>>,
    cursor: usize, // points one past the last executed command
    max_size: usize,
}

impl std::fmt::Debug for UndoHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UndoHistory")
            .field("cursor", &self.cursor)
            .field("len", &self.commands.len())
            .finish()
    }
}

impl UndoHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            commands: Vec::with_capacity(max_size),
            cursor: 0,
            max_size,
        }
    }

    /// Execute a command and push it onto the history.
    pub fn execute(&mut self, mut cmd: Box<dyn EditorCommand>, state: &mut EditorState) {
        // Drop any redo-able future if we branch from the middle.
        if self.cursor < self.commands.len() {
            self.commands.truncate(self.cursor);
        }
        cmd.execute(state);
        self.commands.push(cmd);
        // Evict oldest if over capacity.
        if self.commands.len() > self.max_size {
            self.commands.remove(0);
        } else {
            self.cursor += 1;
        }
    }

    /// Undo the last command.
    pub fn undo(&mut self, state: &mut EditorState) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        self.commands[self.cursor].undo(state);
        true
    }

    /// Redo the next undone command.
    pub fn redo(&mut self, state: &mut EditorState) -> bool {
        if self.cursor >= self.commands.len() {
            return false;
        }
        self.commands[self.cursor].execute(state);
        self.cursor += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.commands.len()
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.cursor = 0;
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the names of the last N commands (most recent first).
    pub fn recent_names(&self, n: usize) -> Vec<&str> {
        let start = if self.cursor > n { self.cursor - n } else { 0 };
        self.commands[start..self.cursor]
            .iter()
            .rev()
            .map(|c| c.name())
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorLayout — dockable panels
// ─────────────────────────────────────────────────────────────────────────────

/// Which editor panels are currently visible and their pixel bounds.
#[derive(Debug, Clone)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub visible: bool,
    pub collapsed: bool,
}

impl PanelRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height, visible: true, collapsed: false }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

#[derive(Debug, Clone)]
pub struct EditorLayout {
    pub hierarchy: PanelRect,
    pub inspector: PanelRect,
    pub console: PanelRect,
    pub viewport: PanelRect,
    pub toolbar: PanelRect,
    pub asset_browser: PanelRect,
}

impl Default for EditorLayout {
    fn default() -> Self {
        Self {
            hierarchy:     PanelRect::new(0.0,   20.0, 200.0, 600.0),
            inspector:     PanelRect::new(1080.0, 20.0, 220.0, 700.0),
            console:       PanelRect::new(0.0,   620.0, 1300.0, 200.0),
            viewport:      PanelRect::new(200.0,  20.0, 880.0, 600.0),
            toolbar:       PanelRect::new(0.0,   0.0,  1300.0,  20.0),
            asset_browser: PanelRect::new(200.0, 620.0, 880.0, 200.0),
        }
    }
}

impl EditorLayout {
    pub fn toggle_hierarchy(&mut self) {
        self.hierarchy.visible = !self.hierarchy.visible;
    }
    pub fn toggle_inspector(&mut self) {
        self.inspector.visible = !self.inspector.visible;
    }
    pub fn toggle_console(&mut self) {
        self.console.visible = !self.console.visible;
    }
    pub fn reset_to_default(&mut self) {
        *self = Self::default();
    }
    /// Move a panel by delta pixels (drag).
    pub fn drag_panel(&mut self, panel: PanelId, dx: f32, dy: f32) {
        let rect = match panel {
            PanelId::Hierarchy => &mut self.hierarchy,
            PanelId::Inspector => &mut self.inspector,
            PanelId::Console   => &mut self.console,
            PanelId::Viewport  => &mut self.viewport,
            PanelId::Toolbar   => &mut self.toolbar,
            PanelId::AssetBrowser => &mut self.asset_browser,
        };
        rect.x += dx;
        rect.y += dy;
    }
    /// Resize a panel.
    pub fn resize_panel(&mut self, panel: PanelId, dw: f32, dh: f32) {
        let rect = match panel {
            PanelId::Hierarchy => &mut self.hierarchy,
            PanelId::Inspector => &mut self.inspector,
            PanelId::Console   => &mut self.console,
            PanelId::Viewport  => &mut self.viewport,
            PanelId::Toolbar   => &mut self.toolbar,
            PanelId::AssetBrowser => &mut self.asset_browser,
        };
        rect.width  = (rect.width  + dw).max(50.0);
        rect.height = (rect.height + dh).max(30.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelId {
    Hierarchy,
    Inspector,
    Console,
    Viewport,
    Toolbar,
    AssetBrowser,
}

// ─────────────────────────────────────────────────────────────────────────────
// Shortcut registry
// ─────────────────────────────────────────────────────────────────────────────

/// A keyboard shortcut (key + modifiers).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: char,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Shortcut {
    pub fn key(key: char) -> Self {
        Self { key, ctrl: false, shift: false, alt: false }
    }
    pub fn ctrl(key: char) -> Self {
        Self { key, ctrl: true, shift: false, alt: false }
    }
    pub fn ctrl_shift(key: char) -> Self {
        Self { key, ctrl: true, shift: true, alt: false }
    }
}

/// Action the shortcut should trigger.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditorAction {
    Undo,
    Redo,
    Save,
    SaveAs,
    Open,
    New,
    Delete,
    Duplicate,
    SelectAll,
    DeselectAll,
    TogglePlay,
    TogglePause,
    FocusSelected,
    ToggleHierarchy,
    ToggleInspector,
    ToggleConsole,
    GizmoTranslate,
    GizmoRotate,
    GizmoScale,
    GizmoUniversal,
    SnapToggle,
    AxisX,
    AxisY,
    AxisZ,
    Screenshot,
    Custom(String),
}

/// Maps shortcuts to editor actions.
#[derive(Debug, Clone, Default)]
pub struct ShortcutRegistry {
    bindings: HashMap<Shortcut, EditorAction>,
}

impl ShortcutRegistry {
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.register_defaults();
        reg
    }

    fn register_defaults(&mut self) {
        self.bind(Shortcut::ctrl('z'), EditorAction::Undo);
        self.bind(Shortcut::ctrl('y'), EditorAction::Redo);
        self.bind(Shortcut::ctrl('s'), EditorAction::Save);
        self.bind(Shortcut::ctrl_shift('s'), EditorAction::SaveAs);
        self.bind(Shortcut::ctrl('o'), EditorAction::Open);
        self.bind(Shortcut::ctrl('n'), EditorAction::New);
        self.bind(Shortcut::key('\x7f'), EditorAction::Delete); // Delete key
        self.bind(Shortcut::ctrl('d'), EditorAction::Duplicate);
        self.bind(Shortcut::ctrl('a'), EditorAction::SelectAll);
        self.bind(Shortcut::key('g'), EditorAction::GizmoTranslate);
        self.bind(Shortcut::key('r'), EditorAction::GizmoRotate);
        self.bind(Shortcut::key('s'), EditorAction::GizmoScale);
        self.bind(Shortcut::key('x'), EditorAction::AxisX);
        self.bind(Shortcut::key('y'), EditorAction::AxisY);
        self.bind(Shortcut::key('z'), EditorAction::AxisZ);
        self.bind(Shortcut::key('f'), EditorAction::FocusSelected);
        self.bind(Shortcut::key(' '), EditorAction::TogglePlay);
    }

    pub fn bind(&mut self, shortcut: Shortcut, action: EditorAction) {
        self.bindings.insert(shortcut, action);
    }

    pub fn unbind(&mut self, shortcut: &Shortcut) {
        self.bindings.remove(shortcut);
    }

    pub fn resolve(&self, shortcut: &Shortcut) -> Option<&EditorAction> {
        self.bindings.get(shortcut)
    }

    pub fn all_bindings(&self) -> &HashMap<Shortcut, EditorAction> {
        &self.bindings
    }

    /// Find the shortcut bound to a given action (first match).
    pub fn shortcut_for(&self, action: &EditorAction) -> Option<&Shortcut> {
        self.bindings.iter().find_map(|(k, v)| if v == action { Some(k) } else { None })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorStats
// ─────────────────────────────────────────────────────────────────────────────

/// Live performance / scene statistics displayed in the editor overlay.
#[derive(Debug, Clone, Default)]
pub struct EditorStats {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub draw_calls: u32,
    pub entities: u32,
    pub glyphs: u32,
    pub particles: u32,
    pub force_fields: u32,
    pub memory_mb: f32,
    pub gpu_memory_mb: f32,
    pub triangles: u64,
    /// Frame counter since engine start.
    pub frame_index: u64,
    /// Accumulated time since engine start.
    pub elapsed_secs: f32,
    /// Rolling average fps over last N frames.
    fps_samples: Vec<f32>,
    fps_idx: usize,
}

impl EditorStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with a new frame's delta time.
    pub fn update(&mut self, dt: f32) {
        let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        let idx = self.fps_idx % 64;
        if self.fps_samples.len() <= idx { self.fps_samples.resize(idx + 1, 0.0); }
        self.fps_samples[idx] = fps;
        self.fps_idx = self.fps_idx.wrapping_add(1);
        let n = self.fps_samples.len() as f32;
        self.fps = self.fps_samples.iter().sum::<f32>() / n.max(1.0);
        self.frame_time_ms = dt * 1000.0;
        self.frame_index += 1;
        self.elapsed_secs += dt;
    }

    /// Render a compact stats string for the toolbar.
    pub fn summary(&self) -> String {
        format!(
            "FPS:{:.0} | Frame:{:.2}ms | Entities:{} | Particles:{} | DC:{} | Mem:{:.1}MB",
            self.fps,
            self.frame_time_ms,
            self.entities,
            self.particles,
            self.draw_calls,
            self.memory_mb,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GridRenderer
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration and rendering logic for the infinite editor grid.
#[derive(Debug, Clone)]
pub struct GridRenderer {
    pub enabled: bool,
    pub cell_size: f32,
    pub major_every: u32,   // draw a thicker line every N cells
    pub color_minor: Vec4,
    pub color_major: Vec4,
    pub color_origin: Vec4,
    pub fade_start: f32,    // distance from camera at which grid begins to fade
    pub fade_end: f32,      // distance at which grid is fully transparent
    pub y_plane: f32,       // world-space Y position of the ground plane
}

impl Default for GridRenderer {
    fn default() -> Self {
        Self {
            enabled: true,
            cell_size: 1.0,
            major_every: 5,
            color_minor:  Vec4::new(0.4, 0.4, 0.4, 0.35),
            color_major:  Vec4::new(0.6, 0.6, 0.6, 0.60),
            color_origin: Vec4::new(0.8, 0.8, 0.8, 0.90),
            fade_start: 20.0,
            fade_end: 60.0,
            y_plane: 0.0,
        }
    }
}

impl GridRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute alpha fade factor at a given distance from the camera.
    pub fn fade_alpha(&self, distance: f32) -> f32 {
        if distance <= self.fade_start {
            1.0
        } else if distance >= self.fade_end {
            0.0
        } else {
            1.0 - (distance - self.fade_start) / (self.fade_end - self.fade_start)
        }
    }

    /// Returns a list of grid lines within the visible region.
    /// Each line is (start: Vec3, end: Vec3, color: Vec4).
    pub fn build_lines(
        &self,
        camera_pos: Vec3,
        half_extent: f32,
    ) -> Vec<(Vec3, Vec3, Vec4)> {
        if !self.enabled {
            return Vec::new();
        }
        let cs = self.cell_size;
        let cx = (camera_pos.x / cs).floor() as i32;
        let cz = (camera_pos.z / cs).floor() as i32;
        let extent = (half_extent / cs).ceil() as i32 + 1;

        let mut lines = Vec::new();
        let y = self.y_plane;

        for i in -extent..=extent {
            let lx = (cx + i) as f32 * cs;
            let lz = (cz + i) as f32 * cs;

            let is_major_x = (cx + i).unsigned_abs() % self.major_every == 0;
            let is_major_z = (cz + i).unsigned_abs() % self.major_every == 0;
            let is_origin_x = cx + i == 0;
            let is_origin_z = cz + i == 0;

            let col_x = if is_origin_x {
                self.color_origin
            } else if is_major_x {
                self.color_major
            } else {
                self.color_minor
            };

            let col_z = if is_origin_z {
                self.color_origin
            } else if is_major_z {
                self.color_major
            } else {
                self.color_minor
            };

            let z0 = (cz - extent) as f32 * cs;
            let z1 = (cz + extent) as f32 * cs;
            let x0 = (cx - extent) as f32 * cs;
            let x1 = (cx + extent) as f32 * cs;

            let dist_x = (Vec2::new(lx, y) - Vec2::new(camera_pos.x, camera_pos.y)).length();
            let dist_z = (Vec2::new(lz, y) - Vec2::new(camera_pos.z, camera_pos.y)).length();

            let alpha_x = self.fade_alpha(dist_x);
            let alpha_z = self.fade_alpha(dist_z);

            let mut c_x = col_x;
            c_x.w *= alpha_x;
            let mut c_z = col_z;
            c_z.w *= alpha_z;

            if c_x.w > 0.01 {
                lines.push((Vec3::new(lx, y, z0), Vec3::new(lx, y, z1), c_x));
            }
            if c_z.w > 0.01 {
                lines.push((Vec3::new(x0, y, lz), Vec3::new(x1, y, lz), c_z));
            }
        }
        lines
    }

    /// Render grid as ASCII art for debug/console output.
    pub fn render_ascii(&self, width: usize, height: usize) -> String {
        let mut out = String::with_capacity(width * height);
        for row in 0..height {
            for col in 0..width {
                let on_major_h = row % (self.major_every as usize) == 0;
                let on_major_v = col % (self.major_every as usize) == 0;
                let ch = if on_major_h && on_major_v {
                    '+'
                } else if on_major_h {
                    '-'
                } else if on_major_v {
                    '|'
                } else {
                    ' '
                };
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorState — master struct
// ─────────────────────────────────────────────────────────────────────────────

/// The top-level editor state.  Owns everything the editor needs to function.
pub struct EditorState {
    // ── Mode & config ─────────────────────────────────────────────────────────
    pub mode: EditorMode,
    pub config: EditorConfig,

    // ── Camera ────────────────────────────────────────────────────────────────
    pub camera: EditorCamera,

    // ── Selection ─────────────────────────────────────────────────────────────
    pub selection: SelectionSet,

    // ── Undo/redo ─────────────────────────────────────────────────────────────
    pub undo_history: UndoHistory,

    // ── Layout & panels ───────────────────────────────────────────────────────
    pub layout: EditorLayout,
    pub shortcuts: ShortcutRegistry,

    // ── Stats ─────────────────────────────────────────────────────────────────
    pub stats: EditorStats,

    // ── Grid ─────────────────────────────────────────────────────────────────
    pub grid: GridRenderer,

    // ── Scene mirror (lightweight copies for editor logic) ────────────────────
    pub entity_positions:  HashMap<EntityId, Vec3>,
    pub entity_names:      HashMap<EntityId, String>,
    pub entity_properties: HashMap<EntityId, HashMap<String, String>>,
    pub entity_groups:     HashMap<EntityId, Vec<EntityId>>,

    // ── Dirty flag ───────────────────────────────────────────────────────────
    pub dirty: bool,
    pub scene_path: Option<String>,
    pub next_entity_id: u32,
}

impl std::fmt::Debug for EditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorState")
            .field("mode", &self.mode)
            .field("dirty", &self.dirty)
            .finish()
    }
}

impl EditorState {
    pub fn new(config: EditorConfig) -> Self {
        let max_undo = config.max_undo_history;
        Self {
            mode: EditorMode::Edit,
            config,
            camera: EditorCamera::new(),
            selection: SelectionSet::new(),
            undo_history: UndoHistory::new(max_undo),
            layout: EditorLayout::default(),
            shortcuts: ShortcutRegistry::new(),
            stats: EditorStats::new(),
            grid: GridRenderer::new(),
            entity_positions:  HashMap::new(),
            entity_names:      HashMap::new(),
            entity_properties: HashMap::new(),
            entity_groups:     HashMap::new(),
            dirty: false,
            scene_path: None,
            next_entity_id: 1,
        }
    }

    /// Allocate a fresh EntityId.
    pub fn alloc_entity_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    /// Switch the editor mode.
    pub fn set_mode(&mut self, mode: EditorMode) {
        self.mode = mode;
    }

    /// Tick the editor (update stats, process any deferred logic).
    pub fn tick(&mut self, dt: f32) {
        self.stats.update(dt);
    }

    /// Execute an undoable command.
    pub fn do_command(&mut self, cmd: Box<dyn EditorCommand>) {
        // We need to temporarily separate self from undo_history to satisfy borrow rules.
        // Use an unsafe swap approach via pointer — safe here because EditorState is not
        // re-entrant and we own both fields.
        let mut history = std::mem::replace(
            &mut self.undo_history,
            UndoHistory::new(0),
        );
        history.execute(cmd, self);
        self.undo_history = history;
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        let mut history = std::mem::replace(&mut self.undo_history, UndoHistory::new(0));
        let result = history.undo(self);
        self.undo_history = history;
        result
    }

    pub fn redo(&mut self) -> bool {
        let mut history = std::mem::replace(&mut self.undo_history, UndoHistory::new(0));
        let result = history.redo(self);
        self.undo_history = history;
        result
    }

    /// Process a keyboard shortcut.
    pub fn handle_shortcut(&mut self, shortcut: &Shortcut) -> Option<EditorAction> {
        let action = self.shortcuts.resolve(shortcut).cloned()?;
        match &action {
            EditorAction::Undo => { self.undo(); }
            EditorAction::Redo => { self.redo(); }
            EditorAction::TogglePlay => {
                self.mode = match self.mode {
                    EditorMode::Play  => EditorMode::Edit,
                    EditorMode::Edit  => EditorMode::Play,
                    EditorMode::Pause => EditorMode::Play,
                };
            }
            EditorAction::TogglePause => {
                self.mode = match self.mode {
                    EditorMode::Play  => EditorMode::Pause,
                    EditorMode::Pause => EditorMode::Play,
                    EditorMode::Edit  => EditorMode::Edit,
                };
            }
            EditorAction::ToggleHierarchy => self.layout.toggle_hierarchy(),
            EditorAction::ToggleInspector => self.layout.toggle_inspector(),
            EditorAction::ToggleConsole   => self.layout.toggle_console(),
            EditorAction::FocusSelected => {
                if let Some(id) = self.selection.primary_entity() {
                    if let Some(&pos) = self.entity_positions.get(&id) {
                        self.camera.focus_on(pos);
                    }
                }
            }
            EditorAction::SnapToggle => {
                self.config.snap_enabled = !self.config.snap_enabled;
            }
            _ => {}
        }
        Some(action)
    }

    /// Snap a world-space position to the snap grid.
    pub fn snap_position(&self, pos: Vec3) -> Vec3 {
        if !self.config.snap_enabled {
            return pos;
        }
        let s = self.config.snap_size;
        Vec3::new(
            (pos.x / s).round() * s,
            (pos.y / s).round() * s,
            (pos.z / s).round() * s,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> EditorState {
        EditorState::new(EditorConfig::default())
    }

    #[test]
    fn test_editor_mode_default() {
        let state = make_state();
        assert_eq!(state.mode, EditorMode::Edit);
    }

    #[test]
    fn test_alloc_entity_id() {
        let mut state = make_state();
        let a = state.alloc_entity_id();
        let b = state.alloc_entity_id();
        assert_ne!(a, b);
    }

    #[test]
    fn test_undo_redo() {
        let mut state = make_state();
        let id = state.alloc_entity_id();
        let cmd = Box::new(SpawnEntityCommand {
            id,
            position: Vec3::ZERO,
            name: "test".into(),
            executed: false,
        });
        state.do_command(cmd);
        assert!(state.entity_positions.contains_key(&id));
        state.undo();
        assert!(!state.entity_positions.contains_key(&id));
        state.redo();
        assert!(state.entity_positions.contains_key(&id));
    }

    #[test]
    fn test_move_entity_undo() {
        let mut state = make_state();
        let id = state.alloc_entity_id();
        state.entity_positions.insert(id, Vec3::ZERO);
        let cmd = Box::new(MoveEntityCommand {
            entity_ids: vec![id],
            delta: Vec3::new(1.0, 0.0, 0.0),
        });
        state.do_command(cmd);
        assert_eq!(state.entity_positions[&id], Vec3::new(1.0, 0.0, 0.0));
        state.undo();
        assert_eq!(state.entity_positions[&id], Vec3::ZERO);
    }

    #[test]
    fn test_selection_set() {
        let mut sel = SelectionSet::new();
        let a = EntityId(1);
        let b = EntityId(2);
        sel.select_entity(a);
        assert_eq!(sel.count(), 1);
        sel.toggle_entity(b);
        assert_eq!(sel.count(), 2);
        sel.toggle_entity(a);
        assert_eq!(sel.count(), 1);
        assert_eq!(sel.primary_entity(), Some(b));
    }

    #[test]
    fn test_camera_forward() {
        let cam = EditorCamera::default();
        let fwd = cam.forward();
        assert!((fwd.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_camera_focus_on() {
        let mut cam = EditorCamera::default();
        let target = Vec3::new(5.0, 0.0, 5.0);
        cam.focus_on(target);
        let d = (cam.position - target).length();
        assert!(d > 0.5, "camera should not be at target");
    }

    #[test]
    fn test_grid_fade_alpha() {
        let grid = GridRenderer::default();
        assert!((grid.fade_alpha(0.0) - 1.0).abs() < 1e-6);
        assert!((grid.fade_alpha(100.0)).abs() < 1e-6);
        let mid = grid.fade_alpha((grid.fade_start + grid.fade_end) / 2.0);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn test_grid_build_lines() {
        let grid = GridRenderer::default();
        let lines = grid.build_lines(Vec3::ZERO, 10.0);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_shortcut_registry() {
        let reg = ShortcutRegistry::new();
        let sc = Shortcut::ctrl('z');
        assert_eq!(reg.resolve(&sc), Some(&EditorAction::Undo));
    }

    #[test]
    fn test_snap_position() {
        let mut state = make_state();
        state.config.snap_enabled = true;
        state.config.snap_size = 1.0;
        let snapped = state.snap_position(Vec3::new(0.7, 1.3, -0.4));
        assert_eq!(snapped, Vec3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn test_stats_update() {
        let mut stats = EditorStats::new();
        stats.update(0.016);
        assert!(stats.fps > 0.0);
        assert!(stats.frame_index == 1);
    }

    #[test]
    fn test_undo_history_ring_buffer() {
        let mut state = make_state();
        // Overflow the ring buffer
        for i in 0..210u32 {
            let id = EntityId(i);
            state.entity_positions.insert(id, Vec3::ZERO);
            let cmd = Box::new(MoveEntityCommand {
                entity_ids: vec![id],
                delta: Vec3::new(1.0, 0.0, 0.0),
            });
            state.do_command(cmd);
        }
        assert!(state.undo_history.len() <= state.config.max_undo_history);
    }

    #[test]
    fn test_delete_entity_undo() {
        let mut state = make_state();
        let id = state.alloc_entity_id();
        state.entity_positions.insert(id, Vec3::new(1.0, 2.0, 3.0));
        state.entity_names.insert(id, "hero".into());
        let cmd = Box::new(DeleteEntityCommand {
            id,
            saved_position: None,
            saved_name: None,
        });
        state.do_command(cmd);
        assert!(!state.entity_positions.contains_key(&id));
        state.undo();
        assert!(state.entity_positions.contains_key(&id));
    }

    #[test]
    fn test_editor_layout_toggle() {
        let mut layout = EditorLayout::default();
        assert!(layout.hierarchy.visible);
        layout.toggle_hierarchy();
        assert!(!layout.hierarchy.visible);
        layout.toggle_hierarchy();
        assert!(layout.hierarchy.visible);
    }
}
