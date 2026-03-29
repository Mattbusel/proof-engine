//! Asset Browser — hierarchical filesystem view of saved SDF models, materials,
//! animations, scenes, and shaders.
//!
//! # Overview
//!
//! The asset browser maintains a virtual directory tree rooted at the project's
//! `assets/` folder.  It scans on startup and on explicit refresh, building a
//! `DirNode` tree.  Each `AssetEntry` carries a `AssetKind` tag, thumbnail
//! metadata, and import/export helpers.
//!
//! # Interaction
//!
//! - Clicking a folder expands/collapses it.
//! - Double-clicking an asset opens it in the appropriate editor panel
//!   (SDF node editor for `.sdf`, material painter for `.mat`, etc.).
//! - Right-click opens a context menu: Rename, Duplicate, Delete, Show in
//!   Explorer.
//! - Drag from browser → viewport/hierarchy creates an instance.
//! - The search bar filters by name and tags; results are shown flat.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// AssetKind
// ─────────────────────────────────────────────────────────────────────────────

/// The type of an asset file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    SdfGraph,      // .sdf  — node graph serialised as binary
    Material,      // .mat  — material preset
    Animation,     // .anim — AnimClip
    Scene,         // .scene — full scene binary
    SceneToml,     // .toml — human-readable scene
    Shader,        // .glsl / .wgsl
    Texture,       // .png / .jpg / .exr
    Audio,         // .wav / .ogg / .flac
    Script,        // .lua / .rhai
    Prefab,        // .prefab — serialised entity hierarchy
    Palette,       // .pal — color palette
    Unknown,
}

impl AssetKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "sdf"    => AssetKind::SdfGraph,
            "mat"    => AssetKind::Material,
            "anim"   => AssetKind::Animation,
            "scene"  => AssetKind::Scene,
            "toml"   => AssetKind::SceneToml,
            "glsl" | "wgsl" | "vert" | "frag" | "comp" => AssetKind::Shader,
            "png" | "jpg" | "jpeg" | "exr" | "hdr"     => AssetKind::Texture,
            "wav" | "ogg" | "flac" | "mp3"             => AssetKind::Audio,
            "lua" | "rhai"                              => AssetKind::Script,
            "prefab"                                    => AssetKind::Prefab,
            "pal"                                       => AssetKind::Palette,
            _                                           => AssetKind::Unknown,
        }
    }

    pub fn icon(self) -> char {
        match self {
            AssetKind::SdfGraph  => 'S',
            AssetKind::Material  => 'M',
            AssetKind::Animation => 'A',
            AssetKind::Scene     => 'W',
            AssetKind::SceneToml => 'T',
            AssetKind::Shader    => 'H',
            AssetKind::Texture   => 'I',
            AssetKind::Audio     => '♪',
            AssetKind::Script    => 'L',
            AssetKind::Prefab    => 'P',
            AssetKind::Palette   => 'C',
            AssetKind::Unknown   => '?',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AssetKind::SdfGraph  => "SDF Graph",
            AssetKind::Material  => "Material",
            AssetKind::Animation => "Animation",
            AssetKind::Scene     => "Scene",
            AssetKind::SceneToml => "Scene (TOML)",
            AssetKind::Shader    => "Shader",
            AssetKind::Texture   => "Texture",
            AssetKind::Audio     => "Audio",
            AssetKind::Script    => "Script",
            AssetKind::Prefab    => "Prefab",
            AssetKind::Palette   => "Palette",
            AssetKind::Unknown   => "Unknown",
        }
    }

    pub fn color_rgb(self) -> (f32, f32, f32) {
        match self {
            AssetKind::SdfGraph  => (0.5, 0.8, 1.0),
            AssetKind::Material  => (1.0, 0.6, 0.2),
            AssetKind::Animation => (0.8, 0.4, 1.0),
            AssetKind::Scene     => (0.4, 1.0, 0.5),
            AssetKind::SceneToml => (0.3, 0.9, 0.4),
            AssetKind::Shader    => (1.0, 0.9, 0.3),
            AssetKind::Texture   => (0.9, 0.9, 0.9),
            AssetKind::Audio     => (0.6, 1.0, 0.8),
            AssetKind::Script    => (1.0, 0.8, 0.5),
            AssetKind::Prefab    => (1.0, 0.5, 0.5),
            AssetKind::Palette   => (0.9, 0.6, 0.9),
            AssetKind::Unknown   => (0.5, 0.5, 0.5),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AssetId
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetId(pub u64);

impl AssetId {
    /// Deterministic ID from a canonical path string.
    pub fn from_path(p: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        p.hash(&mut h);
        AssetId(h.finish())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AssetEntry
// ─────────────────────────────────────────────────────────────────────────────

/// Metadata for a single asset file.
#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub id:          AssetId,
    pub name:        String,
    pub path:        PathBuf,
    pub kind:        AssetKind,
    pub size_bytes:  u64,
    pub tags:        Vec<String>,
    pub thumbnail:   Option<ThumbnailData>,
    pub selected:    bool,
    pub favourite:   bool,
    /// True if the disk file is newer than the last cached scan.
    pub dirty:       bool,
}

impl AssetEntry {
    pub fn new(path: PathBuf) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = path.extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = AssetKind::from_extension(&ext);
        let id = AssetId::from_path(&path.to_string_lossy());
        Self {
            id, name, path, kind,
            size_bytes: 0,
            tags: Vec::new(),
            thumbnail: None,
            selected: false,
            favourite: false,
            dirty: false,
        }
    }

    pub fn display_size(&self) -> String {
        if self.size_bytes < 1024 { format!("{} B", self.size_bytes) }
        else if self.size_bytes < 1024 * 1024 { format!("{:.1} KB", self.size_bytes as f64 / 1024.0) }
        else { format!("{:.2} MB", self.size_bytes as f64 / (1024.0 * 1024.0)) }
    }
}

/// Simple placeholder for a thumbnail — in a real implementation this would
/// hold a GPU texture handle.
#[derive(Debug, Clone)]
pub struct ThumbnailData {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA8
}

impl ThumbnailData {
    pub fn placeholder(width: u32, height: u32, r: u8, g: u8, b: u8) -> Self {
        let pixels = vec![r, g, b, 255u8].into_iter()
            .cycle().take((width * height * 4) as usize).collect();
        Self { width, height, pixels }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DirNode
// ─────────────────────────────────────────────────────────────────────────────

/// A node in the virtual directory tree.
#[derive(Debug)]
pub struct DirNode {
    pub path:     PathBuf,
    pub name:     String,
    pub expanded: bool,
    pub children: Vec<DirNodeId>,
    pub assets:   Vec<AssetId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DirNodeId(pub u32);

impl DirNode {
    pub fn new(path: PathBuf) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        Self { path, name, expanded: false, children: Vec::new(), assets: Vec::new() }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SortMode / ViewMode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode { #[default] Name, Kind, Size, Modified, Favourite }

impl SortMode {
    pub fn label(self) -> &'static str {
        match self { Self::Name=>"Name", Self::Kind=>"Kind", Self::Size=>"Size",
                     Self::Modified=>"Modified", Self::Favourite=>"Favourite" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode { #[default] Grid, List, Detail }

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self { Self::Grid=>"Grid", Self::List=>"List", Self::Detail=>"Detail" }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Filter
// ─────────────────────────────────────────────────────────────────────────────

/// Active filter in the asset browser.
#[derive(Debug, Clone, Default)]
pub struct AssetFilter {
    pub search:     String,
    pub kinds:      Vec<AssetKind>,
    pub tags:       Vec<String>,
    pub favourites_only: bool,
}

impl AssetFilter {
    pub fn matches(&self, entry: &AssetEntry) -> bool {
        if self.favourites_only && !entry.favourite { return false; }
        if !self.kinds.is_empty() && !self.kinds.contains(&entry.kind) { return false; }
        if !self.tags.is_empty() {
            let has_all = self.tags.iter().all(|t| entry.tags.contains(t));
            if !has_all { return false; }
        }
        if !self.search.is_empty() {
            let q = self.search.to_lowercase();
            if !entry.name.to_lowercase().contains(&q) { return false; }
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AssetBrowser
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level asset browser state.
#[derive(Debug)]
pub struct AssetBrowser {
    pub root_path:      PathBuf,
    dir_nodes:          Vec<DirNode>,
    next_dir_id:        u32,
    assets:             HashMap<AssetId, AssetEntry>,
    pub root_dir:       DirNodeId,
    pub active_dir:     DirNodeId,
    pub selected:       Vec<AssetId>,
    pub hovered:        Option<AssetId>,
    pub filter:         AssetFilter,
    pub sort_mode:      SortMode,
    pub sort_ascending: bool,
    pub view_mode:      ViewMode,
    pub thumbnail_size: u32,
    pub show_hidden:    bool,
    /// In-progress drag: asset being dragged.
    pub drag_asset:     Option<AssetId>,
    /// Clipboard for copy/paste within the browser.
    clipboard:          Vec<AssetId>,
    /// Whether clipboard is a cut (move) or copy.
    clipboard_cut:      bool,
}

impl AssetBrowser {
    pub fn new(root: PathBuf) -> Self {
        let root_node = DirNode { path: root.clone(), name: "assets".into(),
            expanded: true, children: Vec::new(), assets: Vec::new() };
        let root_id = DirNodeId(0);
        Self {
            root_path: root,
            dir_nodes: vec![root_node],
            next_dir_id: 1,
            assets: HashMap::new(),
            root_dir: root_id,
            active_dir: root_id,
            selected: Vec::new(),
            hovered: None,
            filter: AssetFilter::default(),
            sort_mode: SortMode::default(),
            sort_ascending: true,
            view_mode: ViewMode::default(),
            thumbnail_size: 96,
            show_hidden: false,
            drag_asset: None,
            clipboard: Vec::new(),
            clipboard_cut: false,
        }
    }

    // ── Scan ──────────────────────────────────────────────────────────────

    /// Simulate scanning a directory and adding entries.
    pub fn mock_scan(&mut self) {
        // Add some sample assets for testing / editor mock
        let samples = vec![
            ("body_scaffold.sdf",        AssetKind::SdfGraph,  1_024),
            ("leon.sdf",                 AssetKind::SdfGraph,  8_192),
            ("skin_default.mat",         AssetKind::Material,    512),
            ("metal_chrome.mat",         AssetKind::Material,    512),
            ("idle.anim",                AssetKind::Animation, 4_096),
            ("walk_cycle.anim",          AssetKind::Animation, 8_192),
            ("main_scene.scene",         AssetKind::Scene,   65_536),
            ("main_scene.toml",          AssetKind::SceneToml, 8_192),
            ("particle_vert.glsl",       AssetKind::Shader,   2_048),
            ("sdf_body_frag.glsl",       AssetKind::Shader,  16_384),
            ("albedo_atlas.png",         AssetKind::Texture, 256_000),
            ("normal_map.png",           AssetKind::Texture, 256_000),
            ("soundtrack.ogg",           AssetKind::Audio,   1_024_000),
            ("game_logic.lua",           AssetKind::Script,   4_096),
        ];

        for (name, kind, size) in samples {
            let path = self.root_path.join(name);
            let mut entry = AssetEntry::new(path.clone());
            entry.kind = kind;
            entry.size_bytes = size;
            let id = entry.id;
            if let Some(dir) = self.dir_nodes.get_mut(0) {
                dir.assets.push(id);
            }
            let (r, g, b) = kind.color_rgb();
            entry.thumbnail = Some(ThumbnailData::placeholder(
                self.thumbnail_size, self.thumbnail_size,
                (r * 200.0) as u8, (g * 200.0) as u8, (b * 200.0) as u8,
            ));
            self.assets.insert(id, entry);
        }
    }

    // ── Selection ─────────────────────────────────────────────────────────

    pub fn select_only(&mut self, id: AssetId) {
        for entry in self.assets.values_mut() { entry.selected = false; }
        self.selected.clear();
        self.selected.push(id);
        if let Some(e) = self.assets.get_mut(&id) { e.selected = true; }
    }

    pub fn toggle_select(&mut self, id: AssetId) {
        if let Some(pos) = self.selected.iter().position(|&x| x == id) {
            self.selected.remove(pos);
            if let Some(e) = self.assets.get_mut(&id) { e.selected = false; }
        } else {
            self.selected.push(id);
            if let Some(e) = self.assets.get_mut(&id) { e.selected = true; }
        }
    }

    pub fn select_all_in_active_dir(&mut self) {
        let active = self.active_dir;
        if let Some(dir) = self.dir_nodes.get(active.0 as usize) {
            let ids: Vec<_> = dir.assets.clone();
            for id in ids {
                if !self.selected.contains(&id) { self.selected.push(id); }
                if let Some(e) = self.assets.get_mut(&id) { e.selected = true; }
            }
        }
    }

    pub fn clear_selection(&mut self) {
        for id in self.selected.drain(..) {
            if let Some(e) = self.assets.get_mut(&id) { e.selected = false; }
        }
    }

    // ── Filtered / sorted view ────────────────────────────────────────────

    pub fn visible_assets(&self) -> Vec<&AssetEntry> {
        let active = self.active_dir;
        let dir_assets = self.dir_nodes.get(active.0 as usize)
            .map(|d| d.assets.as_slice())
            .unwrap_or(&[]);

        let mut entries: Vec<&AssetEntry> = dir_assets.iter()
            .filter_map(|id| self.assets.get(id))
            .filter(|e| self.filter.matches(e))
            .collect();

        match self.sort_mode {
            SortMode::Name     => entries.sort_by(|a, b| a.name.cmp(&b.name)),
            SortMode::Kind     => entries.sort_by(|a, b| a.kind.label().cmp(b.kind.label())),
            SortMode::Size     => entries.sort_by(|a, b| a.size_bytes.cmp(&b.size_bytes)),
            SortMode::Favourite=> entries.sort_by(|a, b| b.favourite.cmp(&a.favourite)),
            SortMode::Modified => {},
        }
        if !self.sort_ascending { entries.reverse(); }
        entries
    }

    pub fn search_all(&self, query: &str) -> Vec<&AssetEntry> {
        if query.is_empty() { return Vec::new(); }
        let q = query.to_lowercase();
        self.assets.values()
            .filter(|e| e.name.to_lowercase().contains(&q))
            .collect()
    }

    // ── Clipboard operations ──────────────────────────────────────────────

    pub fn copy_selected(&mut self) {
        self.clipboard = self.selected.clone();
        self.clipboard_cut = false;
    }

    pub fn cut_selected(&mut self) {
        self.clipboard = self.selected.clone();
        self.clipboard_cut = true;
    }

    pub fn paste_count(&self) -> usize { self.clipboard.len() }

    // ── Favourites ────────────────────────────────────────────────────────

    pub fn toggle_favourite(&mut self, id: AssetId) {
        if let Some(e) = self.assets.get_mut(&id) {
            e.favourite = !e.favourite;
        }
    }

    // ── Directory navigation ──────────────────────────────────────────────

    pub fn navigate_to(&mut self, dir_id: DirNodeId) {
        self.active_dir = dir_id;
        self.clear_selection();
    }

    pub fn toggle_dir_expand(&mut self, dir_id: DirNodeId) {
        if let Some(dir) = self.dir_nodes.get_mut(dir_id.0 as usize) {
            dir.expanded = !dir.expanded;
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────

    pub fn get(&self, id: AssetId) -> Option<&AssetEntry> { self.assets.get(&id) }
    pub fn get_mut(&mut self, id: AssetId) -> Option<&mut AssetEntry> { self.assets.get_mut(&id) }
    pub fn total_assets(&self) -> usize { self.assets.len() }

    pub fn asset_count_by_kind(&self) -> HashMap<AssetKind, usize> {
        let mut counts: HashMap<AssetKind, usize> = HashMap::new();
        for e in self.assets.values() {
            *counts.entry(e.kind).or_insert(0) += 1;
        }
        counts
    }

    // ── Status ────────────────────────────────────────────────────────────

    pub fn status_line(&self) -> String {
        let visible = self.visible_assets().len();
        let total   = self.total_assets();
        let sel     = self.selected.len();
        format!(
            "Assets — {visible} visible / {total} total | {sel} selected | {:?} view | sort by {}{}",
            self.view_mode,
            self.sort_mode.label(),
            if self.sort_ascending { " ↑" } else { " ↓" }
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ImportOptions — controls for asset import pipeline
// ─────────────────────────────────────────────────────────────────────────────

/// Options for importing an asset.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub target_dir:     PathBuf,
    pub overwrite:      bool,
    pub generate_thumb: bool,
    pub normalize:      bool,
    pub tags:           Vec<String>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            target_dir:     PathBuf::from("assets"),
            overwrite:      false,
            generate_thumb: true,
            normalize:      false,
            tags:           Vec::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_browser() -> AssetBrowser {
        let mut b = AssetBrowser::new(PathBuf::from("assets"));
        b.mock_scan();
        b
    }

    #[test]
    fn scan_populates_assets() {
        let b = test_browser();
        assert!(b.total_assets() > 0);
    }

    #[test]
    fn filter_by_kind() {
        let mut b = test_browser();
        b.filter.kinds = vec![AssetKind::SdfGraph];
        let visible = b.visible_assets();
        assert!(visible.iter().all(|e| e.kind == AssetKind::SdfGraph));
    }

    #[test]
    fn filter_by_name() {
        let mut b = test_browser();
        b.filter.search = "leon".into();
        let results = b.search_all("leon");
        assert!(!results.is_empty());
    }

    #[test]
    fn select_toggle() {
        let mut b = test_browser();
        let id = *b.assets.keys().next().unwrap();
        b.select_only(id);
        assert_eq!(b.selected.len(), 1);
        b.toggle_select(id);
        assert_eq!(b.selected.len(), 0);
    }

    #[test]
    fn asset_kind_from_extension() {
        assert_eq!(AssetKind::from_extension("sdf"),  AssetKind::SdfGraph);
        assert_eq!(AssetKind::from_extension("mat"),  AssetKind::Material);
        assert_eq!(AssetKind::from_extension("anim"), AssetKind::Animation);
        assert_eq!(AssetKind::from_extension("png"),  AssetKind::Texture);
    }

    #[test]
    fn colour_hex_parse() {
        use super::super::material_painter::ColorPicker;
        let c = ColorPicker::new("t", glam::Vec4::new(1.0, 0.0, 0.0, 1.0));
        let hex = c.to_hex();
        let back = ColorPicker::from_hex(&hex).unwrap();
        assert!((back.x - 1.0).abs() < 0.01);
    }
}
