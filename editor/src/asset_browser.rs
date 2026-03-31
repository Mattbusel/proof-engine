//! Asset browser — browse and place glyphs, fields, entities, particles,
//! formations, color palettes, scene presets, and file-system assets.

use glam::{Vec3, Vec4};
use proof_engine::prelude::*;
use std::collections::HashMap;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};
use crate::scene::FieldType;
use crate::tools::{CHAR_PALETTES, COLOR_PALETTES};

// ── Asset Category ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategory {
    Glyphs,
    Colors,
    Fields,
    Particles,
    Formations,
    Presets,
    Files,
}

impl AssetCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Glyphs     => "Glyphs",
            Self::Colors     => "Colors",
            Self::Fields     => "Fields",
            Self::Particles  => "Particles",
            Self::Formations => "Formations",
            Self::Presets    => "Presets",
            Self::Files      => "Files",
        }
    }
    pub fn all() -> &'static [AssetCategory] {
        &[Self::Glyphs, Self::Colors, Self::Fields, Self::Particles, Self::Formations, Self::Presets, Self::Files]
    }
}

// ── File Asset Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Glyph,
    Scene,
    Audio,
    Script,
    Shader,
    Texture,
    Config,
    Unknown,
}

impl FileType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Glyph   => "Glyph",
            Self::Scene   => "Scene",
            Self::Audio   => "Audio",
            Self::Script  => "Script",
            Self::Shader  => "Shader",
            Self::Texture => "Texture",
            Self::Config  => "Config",
            Self::Unknown => "File",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Glyph   => "@",
            Self::Scene   => "#",
            Self::Audio   => "~",
            Self::Script  => ">",
            Self::Shader  => "*",
            Self::Texture => "T",
            Self::Config  => "C",
            Self::Unknown => "?",
        }
    }

    pub fn color(self) -> Vec4 {
        match self {
            Self::Glyph   => Vec4::new(0.5, 1.0, 0.6, 1.0),
            Self::Scene   => Vec4::new(0.4, 0.7, 1.0, 1.0),
            Self::Audio   => Vec4::new(1.0, 0.7, 0.3, 1.0),
            Self::Script  => Vec4::new(0.8, 0.6, 1.0, 1.0),
            Self::Shader  => Vec4::new(1.0, 0.4, 0.8, 1.0),
            Self::Texture => Vec4::new(1.0, 0.9, 0.3, 1.0),
            Self::Config  => Vec4::new(0.6, 0.8, 0.6, 1.0),
            Self::Unknown => Vec4::new(0.5, 0.5, 0.5, 1.0),
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "glyph" | "gly" => Self::Glyph,
            "scene" | "json" => Self::Scene,
            "wav" | "ogg" | "mp3" | "flac" => Self::Audio,
            "rs" | "lua" | "py" | "wren" => Self::Script,
            "glsl" | "wgsl" | "hlsl" => Self::Shader,
            "png" | "jpg" | "jpeg" | "bmp" | "tga" => Self::Texture,
            "toml" | "yaml" | "ini" | "cfg" => Self::Config,
            _ => Self::Unknown,
        }
    }
}

// ── File Asset ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileAsset {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size_bytes: u64,
    pub modified: String,
    pub tags: Vec<String>,
    pub starred: bool,
    pub preview_lines: Vec<String>, // first 5 lines for scripts
}

impl FileAsset {
    pub fn new(name: &str, path: &str, ext: &str, size_bytes: u64, modified: &str) -> Self {
        let file_type = FileType::from_extension(ext);
        Self {
            name: name.to_string(),
            path: path.to_string(),
            file_type,
            size_bytes,
            modified: modified.to_string(),
            tags: Vec::new(),
            starred: false,
            preview_lines: Vec::new(),
        }
    }

    pub fn size_display(&self) -> String {
        if self.size_bytes < 1024 {
            format!("{} B", self.size_bytes)
        } else if self.size_bytes < 1024 * 1024 {
            format!("{:.1} KB", self.size_bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.size_bytes as f64 / (1024.0 * 1024.0))
        }
    }

    pub fn matches_search(&self, query: &str) -> bool {
        if query.is_empty() { return true; }
        let q = query.to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.tags.iter().any(|t| t.to_lowercase().contains(&q))
            || self.file_type.label().to_lowercase().contains(&q)
    }
}

// ── Asset Directory ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AssetDirectory {
    pub path: String,
    pub name: String,
    pub files: Vec<FileAsset>,
    pub subdirs: Vec<String>,
    pub expanded: bool,
}

impl AssetDirectory {
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            path: path.to_string(),
            name: name.to_string(),
            files: Vec::new(),
            subdirs: Vec::new(),
            expanded: true,
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

// ── Asset Dependencies ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AssetDependencies {
    /// asset path → set of scene names that use it
    pub used_by: HashMap<String, Vec<String>>,
    /// asset paths that are referenced but not found on disk
    pub missing: Vec<String>,
}

impl AssetDependencies {
    pub fn is_missing(&self, path: &str) -> bool {
        self.missing.iter().any(|m| m == path)
    }

    pub fn uses(&self, path: &str) -> &[String] {
        self.used_by.get(path).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

// ── Asset Preview State ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AssetPreview {
    pub preview_path: Option<String>,
    pub preview_type: Option<FileType>,
    pub waveform_bars: Vec<f32>,   // simulated audio waveform
    pub glyph_anim_t: f32,        // time for glyph color cycling
}

impl AssetPreview {
    pub fn select(&mut self, asset: &FileAsset) {
        self.preview_path = Some(asset.path.clone());
        self.preview_type = Some(asset.file_type);
        if asset.file_type == FileType::Audio && self.waveform_bars.is_empty() {
            // Generate a fake waveform
            use std::f32::consts::PI;
            self.waveform_bars = (0..80).map(|i| {
                let t = i as f32 / 80.0 * PI * 4.0;
                (t.sin() * 0.5 + 0.3 + (t * 1.7).cos() * 0.2).abs()
            }).collect();
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.glyph_anim_t += dt;
    }

    pub fn glyph_color(&self) -> Vec4 {
        let t = self.glyph_anim_t;
        Vec4::new(
            0.5 + 0.5 * (t * 0.7).sin(),
            0.5 + 0.5 * (t * 0.9 + 1.0).sin(),
            0.5 + 0.5 * (t * 0.5 + 2.0).sin(),
            1.0,
        )
    }
}

// ── Drag-Drop State ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct DragDropState {
    pub dragging: bool,
    pub drag_asset: Option<String>, // path of dragged asset
    pub drag_type: Option<FileType>,
    pub drag_glyph: Option<char>,
    pub drag_color: Option<Vec4>,
    pub drop_pos: Option<Vec3>,  // world position of drop target
}

impl DragDropState {
    pub fn start_drag_asset(&mut self, path: &str, ft: FileType) {
        self.dragging = true;
        self.drag_asset = Some(path.to_string());
        self.drag_type = Some(ft);
        self.drag_glyph = None;
        self.drag_color = None;
    }

    pub fn start_drag_glyph(&mut self, ch: char) {
        self.dragging = true;
        self.drag_glyph = Some(ch);
        self.drag_asset = None;
        self.drag_type = None;
    }

    pub fn start_drag_color(&mut self, col: Vec4) {
        self.dragging = true;
        self.drag_color = Some(col);
        self.drag_asset = None;
        self.drag_type = None;
    }

    pub fn cancel(&mut self) {
        *self = DragDropState::default();
    }
}

// ── Recent / Favourites ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct AssetHistory {
    pub recent: Vec<String>,    // paths, max 20
    pub starred: Vec<String>,   // paths
}

impl AssetHistory {
    pub fn push_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        if self.recent.len() > 20 { self.recent.truncate(20); }
    }

    pub fn toggle_star(&mut self, path: &str) {
        if self.starred.iter().any(|p| p == path) {
            self.starred.retain(|p| p != path);
        } else {
            self.starred.push(path.to_string());
        }
    }

    pub fn is_starred(&self, path: &str) -> bool {
        self.starred.iter().any(|p| p == path)
    }
}

// ── AssetBrowser ─────────────────────────────────────────────────────────────

pub struct AssetBrowser {
    pub active_category: AssetCategory,
    pub search: String,
    pub scroll: f32,
    pub selected_glyph: Option<char>,
    pub selected_color: Option<Vec4>,
    pub selected_field: Option<FieldType>,
    pub selected_preset: Option<String>,

    // File system
    pub root_dirs: Vec<AssetDirectory>,
    pub selected_file: Option<FileAsset>,
    pub file_search: String,
    pub show_file_type_filter: bool,
    pub file_type_filter: Option<FileType>,

    // Preview
    pub preview: AssetPreview,

    // Drag/drop
    pub drag_drop: DragDropState,

    // History
    pub history: AssetHistory,

    // Dependencies
    pub dependencies: AssetDependencies,

    // Tree collapse state per directory path
    pub dir_expanded: HashMap<String, bool>,

    // Tag editing
    pub tag_edit_path: Option<String>,
    pub tag_edit_input: String,

    // Find-uses panel
    pub show_find_uses: bool,
    pub find_uses_path: Option<String>,
}

impl AssetBrowser {
    pub fn new() -> Self {
        let mut browser = Self {
            active_category: AssetCategory::Glyphs,
            search: String::new(),
            scroll: 0.0,
            selected_glyph: None,
            selected_color: None,
            selected_field: None,
            selected_preset: None,
            root_dirs: Vec::new(),
            selected_file: None,
            file_search: String::new(),
            show_file_type_filter: false,
            file_type_filter: None,
            preview: AssetPreview::default(),
            drag_drop: DragDropState::default(),
            history: AssetHistory::default(),
            dependencies: AssetDependencies::default(),
            dir_expanded: HashMap::new(),
            tag_edit_path: None,
            tag_edit_input: String::new(),
            show_find_uses: false,
            find_uses_path: None,
        };
        browser.populate_demo_files();
        browser
    }

    fn populate_demo_files(&mut self) {
        let mut assets_dir = AssetDirectory::new("assets", "assets/");
        assets_dir.files.push(FileAsset::new("hero.glyph",    "assets/hero.glyph",    "glyph",  1240,  "2026-03-10"));
        assets_dir.files.push(FileAsset::new("main_scene.json","assets/main_scene.json","json", 42000, "2026-03-28"));
        assets_dir.files.push(FileAsset::new("explosion.wav",  "assets/explosion.wav", "wav",  210000, "2026-02-15"));
        assets_dir.subdirs.push("scripts".into());
        assets_dir.subdirs.push("shaders".into());

        let mut scripts_dir = AssetDirectory::new("scripts", "assets/scripts/");
        let mut spawn_script = FileAsset::new("spawn_enemy.lua","assets/scripts/spawn_enemy.lua","lua", 880, "2026-03-20");
        spawn_script.preview_lines = vec![
            "-- spawn_enemy.lua".into(),
            "function on_spawn(entity)".into(),
            "  entity.hp = 100".into(),
            "  entity.state = 'idle'".into(),
            "end".into(),
        ];
        scripts_dir.files.push(spawn_script);
        scripts_dir.files.push(FileAsset::new("ai_wander.lua","assets/scripts/ai_wander.lua","lua", 640, "2026-03-18"));
        scripts_dir.files.push(FileAsset::new("quest_trigger.rs","assets/scripts/quest_trigger.rs","rs", 1500, "2026-03-25"));

        let mut shaders_dir = AssetDirectory::new("shaders", "assets/shaders/");
        shaders_dir.files.push(FileAsset::new("bloom.wgsl",    "assets/shaders/bloom.wgsl",    "wgsl", 3200, "2026-01-10"));
        shaders_dir.files.push(FileAsset::new("glyph.glsl",    "assets/shaders/glyph.glsl",    "glsl", 2800, "2026-02-05"));
        shaders_dir.files.push(FileAsset::new("particle.wgsl", "assets/shaders/particle.wgsl", "wgsl", 4100, "2026-03-01"));

        self.root_dirs.push(assets_dir);
        self.root_dirs.push(scripts_dir);
        self.root_dirs.push(shaders_dir);

        // Demo deps
        self.dependencies.used_by.insert("assets/hero.glyph".into(), vec!["main_scene.json".into(), "arena.json".into()]);
        self.dependencies.missing.push("assets/missing_texture.png".into());
    }

    pub fn tick(&mut self, dt: f32) {
        self.preview.tick(dt);
    }

    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, height: f32, theme: &WidgetTheme) {
        // Background
        WidgetDraw::fill_rect(engine, Rect::new(x, y, width, height), theme.bg);
        WidgetDraw::text(engine, x + 0.3, y - 0.1, "ASSETS", theme.accent, 0.25, RenderLayer::UI);

        // Category tabs
        let mut tx = x + 0.3;
        let tab_y = y - 0.7;
        for cat in AssetCategory::all() {
            let active = *cat == self.active_category;
            let color = if active { theme.accent } else { theme.fg_dim };
            let sz = if active { 0.15 } else { 0.06 };
            WidgetDraw::text(engine, tx, tab_y, cat.label(), color, sz, RenderLayer::UI);
            tx += cat.label().len() as f32 * 0.38 + 0.4;
        }
        WidgetDraw::separator(engine, x + 0.2, tab_y - 0.4, width - 0.4, theme.separator);

        // Content
        let content_y = tab_y - 0.8;
        match self.active_category {
            AssetCategory::Glyphs     => self.render_glyphs(engine, x, content_y, width, theme),
            AssetCategory::Colors     => self.render_colors(engine, x, content_y, width, theme),
            AssetCategory::Fields     => self.render_fields(engine, x, content_y, width, theme),
            AssetCategory::Particles  => self.render_particles(engine, x, content_y, width, theme),
            AssetCategory::Formations => self.render_formations(engine, x, content_y, width, theme),
            AssetCategory::Presets    => self.render_presets(engine, x, content_y, width, theme),
            AssetCategory::Files      => self.render_files(engine, x, content_y, width, theme),
        }

        // Drag indicator overlay
        if self.drag_drop.dragging {
            let label = if let Some(ch) = self.drag_drop.drag_glyph {
                format!("Dragging: '{}'", ch)
            } else if let Some(ref p) = self.drag_drop.drag_asset {
                format!("Dragging: {}", p)
            } else {
                "Dragging...".into()
            };
            WidgetDraw::fill_rect(engine, Rect::new(x, y - height + 0.4, width, 0.45), Vec4::new(0.2, 0.3, 0.5, 0.7));
            WidgetDraw::text(engine, x + 0.3, y - height + 0.45, &label, Vec4::new(1.0, 1.0, 0.5, 1.0), 0.09, RenderLayer::UI);
        }
    }

    fn render_glyphs(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        // Search bar
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, cy, width - 0.6, 0.4), Vec4::new(0.1, 0.1, 0.15, 0.9));
        let hint = if self.search.is_empty() { "Search glyphs..." } else { &self.search };
        WidgetDraw::text(engine, x + 0.4, cy + 0.04, hint, theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Recent glyphs (starred hint)
        if let Some(ch) = self.selected_glyph {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("Selected: {}", ch), theme.accent, 0.12, RenderLayer::UI);
            cy -= 0.4;
        }

        for (name, chars) in CHAR_PALETTES {
            WidgetDraw::text(engine, x + 0.3, cy, name, theme.fg, 0.12, RenderLayer::UI);
            cy -= 0.5;
            let mut cx = x + 0.5;
            for &ch in *chars {
                let selected = self.selected_glyph == Some(ch);
                let color = if selected { theme.accent } else { theme.fg };
                let sz = if selected { 0.4 } else { 0.15 };
                WidgetDraw::text(engine, cx, cy, &ch.to_string(), color, sz, RenderLayer::UI);
                // drag hint
                if selected {
                    WidgetDraw::fill_rect(engine, Rect::new(cx - 0.05, cy - 0.08, 0.5, 0.42), Vec4::new(0.3, 0.6, 1.0, 0.2));
                }
                cx += 0.6;
                if cx > x + width - 0.4 { cx = x + 0.5; cy -= 0.55; }
            }
            cy -= 0.7;
        }
    }

    fn render_colors(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        if let Some(col) = self.selected_color {
            WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, cy + 0.04, 0.9, 0.38), col);
            WidgetDraw::text(engine, x + 1.3, cy + 0.1, &format!("({:.2},{:.2},{:.2})", col.x, col.y, col.z), theme.fg_dim, 0.07, RenderLayer::UI);
            cy -= 0.5;
        }

        for (name, colors) in COLOR_PALETTES {
            WidgetDraw::text(engine, x + 0.3, cy, name, theme.fg, 0.1, RenderLayer::UI);
            cy -= 0.5;
            let mut cx = x + 0.5;
            for &(r, g, b) in *colors {
                let col = Vec4::new(r, g, b, 1.0);
                let selected = self.selected_color == Some(col);
                WidgetDraw::color_swatch(engine, cx, cy, col);
                if selected {
                    WidgetDraw::fill_rect(engine, Rect::new(cx - 0.05, cy - 0.05, 1.2, 1.2), Vec4::new(1.0, 1.0, 1.0, 0.15));
                }
                cx += 1.5;
                if cx > x + width - 1.5 { cx = x + 0.5; cy -= 1.2; }
            }
            cy -= 1.3;
        }
    }

    fn render_fields(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        for ft in FieldType::all() {
            let selected = self.selected_field == Some(*ft);
            let color = if selected { theme.accent } else { theme.fg };
            WidgetDraw::text(engine, x + 0.3, cy, &format!("~ {}", ft.label()), color, if selected { 0.25 } else { 0.08 }, RenderLayer::UI);
            cy -= 0.55;
        }
    }

    fn render_particles(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let presets = [
            "Death Explosion", "Level Up", "Crit Burst", "Hit Sparks",
            "Fire Burst", "Smoke Puff", "Electric Discharge", "Ice Shatter",
            "Poison Cloud", "Heal Spiral", "Shield Hit", "Confetti",
        ];
        let mut cy = y;
        for name in &presets {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("* {}", name), theme.fg, 0.08, RenderLayer::UI);
            cy -= 0.55;
        }
    }

    fn render_formations(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let formations = [
            "Diamond", "Ring", "Cross", "Star", "Arrow", "Grid",
            "Spiral", "Helix", "Shield", "Crescent", "Skull", "Heart",
        ];
        let mut cy = y;
        for name in &formations {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("# {}", name), theme.fg, 0.08, RenderLayer::UI);
            cy -= 0.55;
        }
    }

    fn render_presets(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let presets = [
            ("Galaxy", "Spiral arm formation"), ("Heartbeat", "Pulsing entity"),
            ("Supernova", "Explosion sequence"), ("Math Rain", "Matrix-style cascade"),
            ("Attractor Garden", "Multiple attractors"), ("Force Field Lab", "Interactive fields"),
        ];
        let mut cy = y;
        for (name, desc) in &presets {
            let selected = self.selected_preset.as_deref() == Some(name);
            let bg = if selected { Vec4::new(0.2, 0.3, 0.5, 0.4) } else { Vec4::ZERO };
            if selected {
                WidgetDraw::fill_rect(engine, Rect::new(x + 0.2, cy - 0.7, width - 0.4, 0.85), bg);
            }
            WidgetDraw::text(engine, x + 0.3, cy, name, theme.accent, 0.15, RenderLayer::UI);
            WidgetDraw::text(engine, x + 0.3, cy - 0.4, desc, theme.fg_dim, 0.05, RenderLayer::UI);
            cy -= 0.9;
        }
    }

    // ── File System Browser ────────────────────────────────────────────────

    fn render_files(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        let sidebar_w = width * 0.42;
        let content_x = x + sidebar_w + 0.2;
        let content_w = width - sidebar_w - 0.3;

        // ── Left: Tree sidebar ───────────────────────────────────────────
        // File type filter bar
        WidgetDraw::text(engine, x + 0.3, cy, "Filter:", theme.fg_dim, 0.07, RenderLayer::UI);
        let types = [FileType::Scene, FileType::Script, FileType::Audio, FileType::Shader];
        let mut fx = x + 1.2;
        for ft in types {
            let active = self.file_type_filter == Some(ft);
            let bg = if active { ft.color() * Vec4::splat(0.5) } else { Vec4::new(0.12, 0.12, 0.18, 0.8) };
            WidgetDraw::fill_rect(engine, Rect::new(fx, cy, 1.05, 0.36), bg);
            WidgetDraw::text(engine, fx + 0.06, cy + 0.04, ft.icon(), ft.color(), 0.07, RenderLayer::UI);
            WidgetDraw::text(engine, fx + 0.22, cy + 0.04, ft.label(), theme.fg_dim, 0.065, RenderLayer::UI);
            fx += 1.15;
        }
        cy -= 0.5;

        // Search
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, cy, sidebar_w - 0.1, 0.4), Vec4::new(0.1, 0.1, 0.15, 0.9));
        let hint = if self.file_search.is_empty() { "Search files..." } else { &self.file_search };
        WidgetDraw::text(engine, x + 0.42, cy + 0.05, hint, theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Directory tree
        let tree_start = cy;
        for dir in &self.root_dirs {
            let dir_expanded = self.dir_expanded.get(&dir.path).copied().unwrap_or(dir.expanded);
            let expand_icon = if dir_expanded { "v" } else { ">" };
            WidgetDraw::text(engine, x + 0.3, cy, &format!("{} [{}] {} ({})", expand_icon, "D", dir.name, dir.file_count()), theme.accent, 0.09, RenderLayer::UI);
            cy -= 0.38;

            if dir_expanded {
                for file in &dir.files {
                    // Apply search and type filter
                    if !file.matches_search(&self.file_search) { continue; }
                    if let Some(ft) = self.file_type_filter {
                        if file.file_type != ft { continue; }
                    }

                    let selected = self.selected_file.as_ref().map(|f| &f.path == &file.path).unwrap_or(false);
                    let missing = self.dependencies.is_missing(&file.path);

                    let bg = if selected { Vec4::new(0.2, 0.3, 0.5, 0.4) } else { Vec4::ZERO };
                    if selected {
                        WidgetDraw::fill_rect(engine, Rect::new(x + 0.6, cy, sidebar_w - 0.9, 0.36), bg);
                    }

                    let icon_col = if missing { theme.error } else { file.file_type.color() };
                    let miss_prefix = if missing { "! " } else { "  " };
                    WidgetDraw::text(engine, x + 0.7, cy, &format!("{}{} {}", miss_prefix, file.file_type.icon(), file.name), icon_col, 0.07, RenderLayer::UI);
                    // Star indicator
                    if self.history.is_starred(&file.path) {
                        WidgetDraw::text(engine, x + sidebar_w - 0.4, cy, "*", Vec4::new(1.0, 0.9, 0.2, 1.0), 0.1, RenderLayer::UI);
                    }
                    cy -= 0.35;
                }
            }
        }
        let _ = tree_start;

        // ── Right: Preview panel ─────────────────────────────────────────
        if let Some(ref selected) = self.selected_file {
            let preview_y = y;
            self.render_file_preview(engine, content_x, preview_y, content_w, selected, theme);
        } else {
            WidgetDraw::text(engine, content_x + 0.3, y - 1.0, "Select a file", theme.fg_dim, 0.1, RenderLayer::UI);
            WidgetDraw::text(engine, content_x + 0.3, y - 1.5, "to preview", theme.fg_dim, 0.08, RenderLayer::UI);
        }

        // ── Recent / Favourites strip at bottom ──────────────────────────
        let bottom_y = y - 12.0;
        WidgetDraw::separator(engine, x + 0.2, bottom_y, width - 0.4, theme.separator);
        WidgetDraw::text(engine, x + 0.3, bottom_y - 0.1, "RECENT:", theme.fg_dim, 0.07, RenderLayer::UI);
        let mut rx = x + 1.5;
        for path in self.history.recent.iter().take(5) {
            let name = path.rsplit('/').next().unwrap_or(path);
            WidgetDraw::text(engine, rx, bottom_y - 0.1, name, theme.fg, 0.065, RenderLayer::UI);
            rx += name.len() as f32 * 0.075 + 0.3;
        }
    }

    fn render_file_preview(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, asset: &FileAsset, theme: &WidgetTheme) {
        let mut cy = y;
        let ft = asset.file_type;

        // Header
        WidgetDraw::text(engine, x, cy, &format!("{} {}", ft.icon(), asset.name), ft.color(), 0.2, RenderLayer::UI);
        cy -= 0.55;
        WidgetDraw::text(engine, x, cy, &format!("{} | {}", ft.label(), asset.size_display()), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.35;
        WidgetDraw::text(engine, x, cy, &format!("Modified: {}", asset.modified), theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.42;

        // Missing warning
        if self.dependencies.is_missing(&asset.path) {
            WidgetDraw::fill_rect(engine, Rect::new(x, cy, width - 0.2, 0.4), Vec4::new(0.5, 0.1, 0.1, 0.6));
            WidgetDraw::text(engine, x + 0.1, cy + 0.06, "! MISSING ON DISK", theme.error, 0.09, RenderLayer::UI);
            cy -= 0.5;
        }

        // Tags
        WidgetDraw::text(engine, x, cy, "Tags:", theme.fg_dim, 0.07, RenderLayer::UI);
        let mut tx = x + 0.9;
        if asset.tags.is_empty() {
            WidgetDraw::text(engine, tx, cy, "(none)", theme.fg_dim, 0.07, RenderLayer::UI);
        } else {
            for tag in &asset.tags {
                WidgetDraw::fill_rect(engine, Rect::new(tx, cy, tag.len() as f32 * 0.1 + 0.3, 0.35), Vec4::new(0.2, 0.35, 0.5, 0.8));
                WidgetDraw::text(engine, tx + 0.07, cy + 0.04, tag, theme.fg, 0.065, RenderLayer::UI);
                tx += tag.len() as f32 * 0.1 + 0.45;
            }
        }
        WidgetDraw::text(engine, tx, cy, "[+tag]", theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.46;

        // Type-specific preview
        match ft {
            FileType::Glyph => {
                // Large glyph with animated color
                let glyph_ch = asset.name.chars().next().unwrap_or('@');
                let col = self.preview.glyph_color();
                WidgetDraw::fill_rect(engine, Rect::new(x, cy - 2.5, width - 0.2, 2.8), Vec4::new(0.06, 0.06, 0.1, 0.9));
                WidgetDraw::text(engine, x + width * 0.35, cy - 1.8, &glyph_ch.to_string(), col, 0.8, RenderLayer::UI);
                cy -= 3.0;
            }
            FileType::Audio => {
                // Simulated waveform
                WidgetDraw::fill_rect(engine, Rect::new(x, cy - 1.5, width - 0.2, 1.7), Vec4::new(0.06, 0.06, 0.1, 0.9));
                let wf_w = width - 0.6;
                let wf_step = wf_w / self.preview.waveform_bars.len().max(1) as f32;
                for (i, &amp) in self.preview.waveform_bars.iter().enumerate() {
                    let bx = x + 0.2 + i as f32 * wf_step;
                    let bh = amp * 1.4;
                    let mid = cy - 0.85;
                    let col = Vec4::new(0.3, 0.7 + amp * 0.3, 1.0, 0.8);
                    WidgetDraw::text(engine, bx, mid - bh * 0.5, "|", col, 0.08, RenderLayer::UI);
                }
                cy -= 1.9;
                WidgetDraw::text(engine, x, cy, &format!("[Play]  [Stop]  {}", asset.name), theme.fg_dim, 0.07, RenderLayer::UI);
                cy -= 0.4;
            }
            FileType::Scene => {
                WidgetDraw::fill_rect(engine, Rect::new(x, cy - 1.2, width - 0.2, 1.4), Vec4::new(0.06, 0.06, 0.1, 0.9));
                WidgetDraw::text(engine, x + 0.2, cy - 0.35, "Scene file", theme.fg, 0.1, RenderLayer::UI);
                WidgetDraw::text(engine, x + 0.2, cy - 0.8, &format!("{:.1} KB", asset.size_bytes as f32 / 1024.0), theme.fg_dim, 0.08, RenderLayer::UI);
                cy -= 1.5;
            }
            FileType::Script => {
                WidgetDraw::fill_rect(engine, Rect::new(x, cy - 2.4, width - 0.2, 2.6), Vec4::new(0.05, 0.07, 0.1, 0.92));
                let mut ly = cy - 0.25;
                for (i, line) in asset.preview_lines.iter().enumerate().take(5) {
                    let line_col = if i == 0 { theme.fg_dim } else { theme.fg };
                    WidgetDraw::text(engine, x + 0.15, ly, line, line_col, 0.065, RenderLayer::UI);
                    ly -= 0.42;
                }
                if asset.preview_lines.len() > 5 {
                    WidgetDraw::text(engine, x + 0.15, ly, "...", theme.fg_dim, 0.065, RenderLayer::UI);
                }
                cy -= 2.7;
            }
            _ => {
                WidgetDraw::text(engine, x, cy - 0.4, "(no preview)", theme.fg_dim, 0.08, RenderLayer::UI);
                cy -= 0.6;
            }
        }

        // Uses
        let uses = self.dependencies.uses(&asset.path);
        WidgetDraw::text(engine, x, cy, &format!("Used by {} scene(s)", uses.len()), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.38;
        for scene in uses.iter().take(3) {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("- {}", scene), theme.fg, 0.065, RenderLayer::UI);
            cy -= 0.32;
        }
        if uses.len() > 3 {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("... and {} more", uses.len() - 3), theme.fg_dim, 0.065, RenderLayer::UI);
            cy -= 0.32;
        }

        // Action buttons
        cy -= 0.2;
        WidgetDraw::text(engine, x, cy, "[Place in Scene]", theme.accent, 0.09, RenderLayer::UI);
        WidgetDraw::text(engine, x + 2.3, cy, "[Find Uses]", theme.fg_dim, 0.08, RenderLayer::UI);
        WidgetDraw::text(engine, x + 3.8, cy, "[Star]", Vec4::new(1.0, 0.9, 0.2, 0.9), 0.08, RenderLayer::UI);
        let _ = cy;
    }
}
