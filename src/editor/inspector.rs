//! Inspector panel — property editor for any scene object.
//!
//! The inspector reflects the fields of whichever entity/glyph is selected and
//! lets the user edit them in real time.  All edits produce `SetPropertyCommand`
//! entries so they can be undone.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Primitive field types
// ─────────────────────────────────────────────────────────────────────────────

/// A boolean checkbox field.
#[derive(Debug, Clone)]
pub struct BoolField {
    pub name: String,
    pub value: bool,
    pub tooltip: Option<String>,
}

impl BoolField {
    pub fn new(name: impl Into<String>, value: bool) -> Self {
        Self { name: name.into(), value, tooltip: None }
    }
    pub fn with_tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tooltip = Some(tip.into());
        self
    }
    pub fn toggle(&mut self) {
        self.value = !self.value;
    }
    pub fn render_ascii(&self) -> String {
        let check = if self.value { "[x]" } else { "[ ]" };
        format!("{} {}", check, self.name)
    }
}

/// An integer spinner field.
#[derive(Debug, Clone)]
pub struct IntField {
    pub name: String,
    pub value: i64,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub step: i64,
}

impl IntField {
    pub fn new(name: impl Into<String>, value: i64) -> Self {
        Self { name: name.into(), value, min: None, max: None, step: 1 }
    }
    pub fn with_range(mut self, min: i64, max: i64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
    pub fn increment(&mut self) {
        self.value += self.step;
        if let Some(m) = self.max { self.value = self.value.min(m); }
    }
    pub fn decrement(&mut self) {
        self.value -= self.step;
        if let Some(m) = self.min { self.value = self.value.max(m); }
    }
    pub fn set(&mut self, v: i64) {
        let v = if let Some(m) = self.min { v.max(m) } else { v };
        let v = if let Some(m) = self.max { v.min(m) } else { v };
        self.value = v;
    }
    pub fn render_ascii(&self) -> String {
        format!("{}: {}", self.name, self.value)
    }
}

/// A floating-point number field.
#[derive(Debug, Clone)]
pub struct FloatField {
    pub name: String,
    pub value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub precision: usize,
}

impl FloatField {
    pub fn new(name: impl Into<String>, value: f64) -> Self {
        Self { name: name.into(), value, min: None, max: None, step: 0.1, precision: 3 }
    }
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min); self.max = Some(max); self
    }
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step; self
    }
    pub fn set(&mut self, v: f64) {
        let v = if let Some(m) = self.min { v.max(m) } else { v };
        let v = if let Some(m) = self.max { v.min(m) } else { v };
        self.value = v;
    }
    pub fn render_ascii(&self) -> String {
        format!("{}: {:.prec$}", self.name, self.value, prec = self.precision)
    }
}

/// A text input field.
#[derive(Debug, Clone)]
pub struct StringField {
    pub name: String,
    pub value: String,
    pub max_len: usize,
    pub multiline: bool,
    pub placeholder: String,
}

impl StringField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            max_len: 256,
            multiline: false,
            placeholder: String::new(),
        }
    }
    pub fn set(&mut self, v: impl Into<String>) {
        let v: String = v.into();
        self.value = if v.len() > self.max_len { v[..self.max_len].to_string() } else { v };
    }
    pub fn push_char(&mut self, c: char) {
        if self.value.len() < self.max_len {
            self.value.push(c);
        }
    }
    pub fn pop_char(&mut self) {
        self.value.pop();
    }
    pub fn render_ascii(&self) -> String {
        format!("{}: \"{}\"", self.name, self.value)
    }
}

/// A 2-component vector field.
#[derive(Debug, Clone)]
pub struct Vec2Field {
    pub name: String,
    pub value: Vec2,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl Vec2Field {
    pub fn new(name: impl Into<String>, value: Vec2) -> Self {
        Self { name: name.into(), value, min: None, max: None }
    }
    pub fn set(&mut self, v: Vec2) {
        self.value = v;
        if let Some(lo) = self.min { self.value = self.value.max(Vec2::splat(lo)); }
        if let Some(hi) = self.max { self.value = self.value.min(Vec2::splat(hi)); }
    }
    pub fn render_ascii(&self) -> String {
        format!("{}: ({:.3}, {:.3})", self.name, self.value.x, self.value.y)
    }
}

/// A 3-component vector field.
#[derive(Debug, Clone)]
pub struct Vec3Field {
    pub name: String,
    pub value: Vec3,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl Vec3Field {
    pub fn new(name: impl Into<String>, value: Vec3) -> Self {
        Self { name: name.into(), value, min: None, max: None }
    }
    pub fn set(&mut self, v: Vec3) {
        self.value = v;
        if let Some(lo) = self.min { self.value = self.value.max(Vec3::splat(lo)); }
        if let Some(hi) = self.max { self.value = self.value.min(Vec3::splat(hi)); }
    }
    pub fn render_ascii(&self) -> String {
        format!(
            "{}: ({:.3}, {:.3}, {:.3})",
            self.name, self.value.x, self.value.y, self.value.z
        )
    }
}

/// A 4-component vector field.
#[derive(Debug, Clone)]
pub struct Vec4Field {
    pub name: String,
    pub value: Vec4,
}

impl Vec4Field {
    pub fn new(name: impl Into<String>, value: Vec4) -> Self {
        Self { name: name.into(), value }
    }
    pub fn set(&mut self, v: Vec4) {
        self.value = v;
    }
    pub fn render_ascii(&self) -> String {
        format!(
            "{}: ({:.3}, {:.3}, {:.3}, {:.3})",
            self.name,
            self.value.x,
            self.value.y,
            self.value.z,
            self.value.w,
        )
    }
}

/// An RGBA colour picker.
#[derive(Debug, Clone)]
pub struct ColorField {
    pub name: String,
    pub color: Vec4,     // RGBA in [0, 1]
    pub popup_open: bool,
    // HSV cache for the colour wheel
    hue: f32,
    saturation: f32,
    brightness: f32,
}

impl ColorField {
    pub fn new(name: impl Into<String>, color: Vec4) -> Self {
        let (h, s, b) = rgb_to_hsv(color.x, color.y, color.z);
        Self { name: name.into(), color, popup_open: false, hue: h, saturation: s, brightness: b }
    }
    pub fn set_rgb(&mut self, r: f32, g: f32, b_: f32) {
        let (h, s, b) = rgb_to_hsv(r, g, b_);
        self.color.x = r; self.color.y = g; self.color.z = b_;
        self.hue = h; self.saturation = s; self.brightness = b;
    }
    pub fn set_hsv(&mut self, h: f32, s: f32, v: f32) {
        let (r, g, b) = hsv_to_rgb(h, s, v);
        self.hue = h; self.saturation = s; self.brightness = v;
        self.color.x = r; self.color.y = g; self.color.z = b;
    }
    pub fn set_alpha(&mut self, a: f32) {
        self.color.w = a.clamp(0.0, 1.0);
    }
    pub fn render_ascii(&self) -> String {
        format!(
            "{}: rgba({:.2},{:.2},{:.2},{:.2})",
            self.name,
            self.color.x,
            self.color.y,
            self.color.z,
            self.color.w,
        )
    }
    pub fn open_popup(&mut self) { self.popup_open = true; }
    pub fn close_popup(&mut self) { self.popup_open = false; }
    pub fn hue(&self) -> f32 { self.hue }
    pub fn saturation(&self) -> f32 { self.saturation }
    pub fn brightness(&self) -> f32 { self.brightness }
}

/// An enumeration drop-down.
#[derive(Debug, Clone)]
pub struct EnumField {
    pub name: String,
    pub variants: Vec<String>,
    pub selected: usize,
}

impl EnumField {
    pub fn new(name: impl Into<String>, variants: Vec<String>, selected: usize) -> Self {
        let selected = selected.min(variants.len().saturating_sub(1));
        Self { name: name.into(), variants, selected }
    }
    pub fn selected_name(&self) -> &str {
        self.variants.get(self.selected).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn next(&mut self) {
        if !self.variants.is_empty() {
            self.selected = (self.selected + 1) % self.variants.len();
        }
    }
    pub fn prev(&mut self) {
        if !self.variants.is_empty() {
            self.selected = if self.selected == 0 { self.variants.len() - 1 } else { self.selected - 1 };
        }
    }
    pub fn render_ascii(&self) -> String {
        format!("{}: [{}]", self.name, self.selected_name())
    }
}

/// A value slider with explicit min/max bounds.
#[derive(Debug, Clone)]
pub struct SliderField {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub display_precision: usize,
}

impl SliderField {
    pub fn new(name: impl Into<String>, value: f32, min: f32, max: f32) -> Self {
        let value = value.clamp(min, max);
        Self { name: name.into(), value, min, max, display_precision: 2 }
    }
    pub fn set(&mut self, v: f32) {
        self.value = v.clamp(self.min, self.max);
    }
    /// Normalised position in [0, 1].
    pub fn normalized(&self) -> f32 {
        if (self.max - self.min).abs() < f32::EPSILON { 0.0 }
        else { (self.value - self.min) / (self.max - self.min) }
    }
    pub fn set_normalized(&mut self, t: f32) {
        self.value = self.min + t.clamp(0.0, 1.0) * (self.max - self.min);
    }
    pub fn render_ascii(&self, bar_width: usize) -> String {
        let filled = ((self.normalized() * bar_width as f32) as usize).min(bar_width);
        let bar: String = std::iter::repeat('#').take(filled)
            .chain(std::iter::repeat('-').take(bar_width - filled))
            .collect();
        format!("{}: [{}] {:.prec$}", self.name, bar, self.value, prec = self.display_precision)
    }
}

/// A reference to an asset by path / ID.
#[derive(Debug, Clone)]
pub struct AssetRefField {
    pub name: String,
    pub asset_type: String,
    pub asset_path: Option<String>,
    pub asset_id: Option<u64>,
}

impl AssetRefField {
    pub fn new(name: impl Into<String>, asset_type: impl Into<String>) -> Self {
        Self { name: name.into(), asset_type: asset_type.into(), asset_path: None, asset_id: None }
    }
    pub fn set_path(&mut self, path: impl Into<String>) {
        self.asset_path = Some(path.into());
    }
    pub fn clear(&mut self) {
        self.asset_path = None;
        self.asset_id = None;
    }
    pub fn render_ascii(&self) -> String {
        let val = self.asset_path.as_deref().unwrap_or("<none>");
        format!("{} [{}]: {}", self.name, self.asset_type, val)
    }
}

/// A script file reference with an embedded source snippet.
#[derive(Debug, Clone)]
pub struct ScriptField {
    pub name: String,
    pub script_path: Option<String>,
    pub inline_source: String,
    pub bound_globals: HashMap<String, String>,
}

impl ScriptField {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            script_path: None,
            inline_source: String::new(),
            bound_globals: HashMap::new(),
        }
    }
    pub fn set_global(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.bound_globals.insert(key.into(), value.into());
    }
    pub fn render_ascii(&self) -> String {
        let src = self.script_path.as_deref().unwrap_or("<inline>");
        format!("{}: {}", self.name, src)
    }
}

/// A list of homogeneous values.
#[derive(Debug, Clone)]
pub struct ListField {
    pub name: String,
    pub items: Vec<String>, // serialized as strings for generic display
    pub selected_index: Option<usize>,
    pub collapsed: bool,
}

impl ListField {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), items: Vec::new(), selected_index: None, collapsed: false }
    }
    pub fn push(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
    }
    pub fn remove_selected(&mut self) {
        if let Some(i) = self.selected_index {
            if i < self.items.len() {
                self.items.remove(i);
                self.selected_index = if self.items.is_empty() {
                    None
                } else {
                    Some(i.min(self.items.len() - 1))
                };
            }
        }
    }
    pub fn render_ascii(&self) -> String {
        let mut out = format!("{}:\n", self.name);
        for (i, item) in self.items.iter().enumerate() {
            let cursor = if self.selected_index == Some(i) { ">" } else { " " };
            out.push_str(&format!("  {}{}: {}\n", cursor, i, item));
        }
        out
    }
}

/// A key-value map field.
#[derive(Debug, Clone)]
pub struct MapField {
    pub name: String,
    pub entries: Vec<(String, String)>,
    pub collapsed: bool,
}

impl MapField {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), entries: Vec::new(), collapsed: false }
    }
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let k: String = key.into();
        if let Some(e) = self.entries.iter_mut().find(|(ek, _)| ek == &k) {
            e.1 = value.into();
        } else {
            self.entries.push((k, value.into()));
        }
    }
    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|(k, _)| k != key);
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    pub fn render_ascii(&self) -> String {
        let mut out = format!("{}:\n", self.name);
        for (k, v) in &self.entries {
            out.push_str(&format!("  {} = {}\n", k, v));
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// InspectorEntry — tagged union of all field types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum InspectorEntry {
    Bool(BoolField),
    Int(IntField),
    Float(FloatField),
    String(StringField),
    Vec2(Vec2Field),
    Vec3(Vec3Field),
    Vec4(Vec4Field),
    Color(ColorField),
    Enum(EnumField),
    Slider(SliderField),
    AssetRef(AssetRefField),
    Script(ScriptField),
    List(ListField),
    Map(MapField),
    Separator,
    Label(std::string::String),
}

impl InspectorEntry {
    pub fn name(&self) -> &str {
        match self {
            InspectorEntry::Bool(f) => &f.name,
            InspectorEntry::Int(f) => &f.name,
            InspectorEntry::Float(f) => &f.name,
            InspectorEntry::String(f) => &f.name,
            InspectorEntry::Vec2(f) => &f.name,
            InspectorEntry::Vec3(f) => &f.name,
            InspectorEntry::Vec4(f) => &f.name,
            InspectorEntry::Color(f) => &f.name,
            InspectorEntry::Enum(f) => &f.name,
            InspectorEntry::Slider(f) => &f.name,
            InspectorEntry::AssetRef(f) => &f.name,
            InspectorEntry::Script(f) => &f.name,
            InspectorEntry::List(f) => &f.name,
            InspectorEntry::Map(f) => &f.name,
            InspectorEntry::Separator => "",
            InspectorEntry::Label(s) => s,
        }
    }

    /// Render as an ASCII line for the console/debug overlay.
    pub fn render_ascii(&self) -> String {
        match self {
            InspectorEntry::Bool(f) => f.render_ascii(),
            InspectorEntry::Int(f) => f.render_ascii(),
            InspectorEntry::Float(f) => f.render_ascii(),
            InspectorEntry::String(f) => f.render_ascii(),
            InspectorEntry::Vec2(f) => f.render_ascii(),
            InspectorEntry::Vec3(f) => f.render_ascii(),
            InspectorEntry::Vec4(f) => f.render_ascii(),
            InspectorEntry::Color(f) => f.render_ascii(),
            InspectorEntry::Enum(f) => f.render_ascii(),
            InspectorEntry::Slider(f) => f.render_ascii(20),
            InspectorEntry::AssetRef(f) => f.render_ascii(),
            InspectorEntry::Script(f) => f.render_ascii(),
            InspectorEntry::List(f) => f.render_ascii(),
            InspectorEntry::Map(f) => f.render_ascii(),
            InspectorEntry::Separator => "────────────────────────────────────".into(),
            InspectorEntry::Label(s) => s.clone(),
        }
    }

    pub fn matches_search(&self, query: &str) -> bool {
        if query.is_empty() { return true; }
        self.name().to_lowercase().contains(&query.to_lowercase())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inspectable trait
// ─────────────────────────────────────────────────────────────────────────────

/// Implement this trait to make a type editable in the inspector.
pub trait Inspectable {
    /// Returns the property groups for this type.
    fn inspect(&self) -> Vec<PropertyGroup>;
    /// Apply an edited entry value back to self (by entry name + serialized value).
    fn apply_edit(&mut self, entry_name: &str, serialized_value: &str) -> Result<(), String>;
    /// Human-readable type name.
    fn type_name(&self) -> &'static str;
}

// ─────────────────────────────────────────────────────────────────────────────
// PropertyGroup
// ─────────────────────────────────────────────────────────────────────────────

/// A named, foldable group of inspector entries.
#[derive(Debug, Clone)]
pub struct PropertyGroup {
    pub name: String,
    pub entries: Vec<InspectorEntry>,
    pub collapsed: bool,
    pub icon: char,
}

impl PropertyGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), entries: Vec::new(), collapsed: false, icon: '▼' }
    }
    pub fn with_icon(mut self, icon: char) -> Self {
        self.icon = icon; self
    }
    pub fn add(mut self, entry: InspectorEntry) -> Self {
        self.entries.push(entry); self
    }
    pub fn push(&mut self, entry: InspectorEntry) {
        self.entries.push(entry);
    }
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
        self.icon = if self.collapsed { '▶' } else { '▼' };
    }
    pub fn visible_entries(&self) -> &[InspectorEntry] {
        if self.collapsed { &[] } else { &self.entries }
    }
    pub fn render_ascii(&self) -> String {
        let mut out = format!("{} {}\n", self.icon, self.name);
        if !self.collapsed {
            for e in &self.entries {
                out.push_str("  ");
                out.push_str(&e.render_ascii());
                out.push('\n');
            }
        }
        out
    }
    pub fn filter(&self, query: &str) -> Vec<&InspectorEntry> {
        self.entries.iter().filter(|e| e.matches_search(query)).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// InspectorContext
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks dirty state, validation errors and the clipboard for copy/paste.
#[derive(Debug, Clone, Default)]
pub struct InspectorContext {
    pub dirty_properties: Vec<String>,
    pub validation_errors: HashMap<String, String>,
    pub clipboard: Option<ClipboardEntry>,
    pub undo_stack: Vec<(String, String, String)>, // (entity_id, prop, old_value)
}

/// A value that has been copied to the inspector clipboard.
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub source_type: String,
    pub property_name: String,
    pub serialized_value: String,
}

impl InspectorContext {
    pub fn new() -> Self { Self::default() }

    pub fn mark_dirty(&mut self, name: impl Into<String>) {
        let n = name.into();
        if !self.dirty_properties.contains(&n) {
            self.dirty_properties.push(n);
        }
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_properties.clear();
    }

    pub fn is_dirty(&self, name: &str) -> bool {
        self.dirty_properties.contains(&name.to_string())
    }

    pub fn add_validation_error(&mut self, prop: impl Into<String>, msg: impl Into<String>) {
        self.validation_errors.insert(prop.into(), msg.into());
    }

    pub fn clear_validation_error(&mut self, prop: &str) {
        self.validation_errors.remove(prop);
    }

    pub fn has_errors(&self) -> bool {
        !self.validation_errors.is_empty()
    }

    pub fn copy_value(&mut self, entry: &InspectorEntry, source_type: impl Into<String>) {
        self.clipboard = Some(ClipboardEntry {
            source_type: source_type.into(),
            property_name: entry.name().to_string(),
            serialized_value: entry.render_ascii(),
        });
    }

    pub fn paste_value(&self) -> Option<&ClipboardEntry> {
        self.clipboard.as_ref()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SearchBar
// ─────────────────────────────────────────────────────────────────────────────

/// A text filter for property names.
#[derive(Debug, Clone, Default)]
pub struct SearchBar {
    pub query: String,
    pub focused: bool,
    pub case_sensitive: bool,
}

impl SearchBar {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }
    pub fn pop(&mut self) {
        self.query.pop();
    }
    pub fn clear(&mut self) {
        self.query.clear();
    }
    pub fn matches(&self, text: &str) -> bool {
        if self.query.is_empty() { return true; }
        if self.case_sensitive {
            text.contains(&self.query)
        } else {
            text.to_lowercase().contains(&self.query.to_lowercase())
        }
    }
    pub fn render_ascii(&self) -> String {
        format!("Search: [{}{}]", self.query, if self.focused { "|" } else { "" })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TransformInspector
// ─────────────────────────────────────────────────────────────────────────────

/// Inspector section for position/rotation/scale.
#[derive(Debug, Clone)]
pub struct TransformInspector {
    pub position: Vec3Field,
    pub rotation: Vec3Field, // Euler angles in degrees
    pub scale: Vec3Field,
    pub snap_position: f32,
    pub snap_rotation: f32,
    pub snap_scale: f32,
    pub snap_enabled: bool,
    pub local_space: bool,
}

impl TransformInspector {
    pub fn new(pos: Vec3, rot: Vec3, scale: Vec3) -> Self {
        Self {
            position: Vec3Field::new("Position", pos),
            rotation: Vec3Field::new("Rotation", rot),
            scale:    Vec3Field::new("Scale",    scale),
            snap_position: 0.25,
            snap_rotation: 15.0,
            snap_scale:    0.1,
            snap_enabled: false,
            local_space: true,
        }
    }

    pub fn apply_snap_position(&mut self) {
        if !self.snap_enabled { return; }
        let s = self.snap_position;
        let p = self.position.value;
        self.position.set(Vec3::new(
            (p.x / s).round() * s,
            (p.y / s).round() * s,
            (p.z / s).round() * s,
        ));
    }

    pub fn apply_snap_rotation(&mut self) {
        if !self.snap_enabled { return; }
        let s = self.snap_rotation;
        let r = self.rotation.value;
        self.rotation.set(Vec3::new(
            (r.x / s).round() * s,
            (r.y / s).round() * s,
            (r.z / s).round() * s,
        ));
    }

    pub fn to_property_group(&self) -> PropertyGroup {
        let mut g = PropertyGroup::new("Transform").with_icon('↔');
        g.push(InspectorEntry::Vec3(self.position.clone()));
        g.push(InspectorEntry::Vec3(self.rotation.clone()));
        g.push(InspectorEntry::Vec3(self.scale.clone()));
        g.push(InspectorEntry::Bool(BoolField::new("Snap", self.snap_enabled)));
        g
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GlyphInspector
// ─────────────────────────────────────────────────────────────────────────────

/// Inspector section for a single glyph's visual properties.
#[derive(Debug, Clone)]
pub struct GlyphInspector {
    pub character:  StringField,
    pub color:      ColorField,
    pub emission:   SliderField,
    pub glow_color: ColorField,
    pub glow_radius: FloatField,
    pub layer:      EnumField,
    pub blend_mode: EnumField,
    pub mass:       FloatField,
    pub charge:     FloatField,
    pub temperature: FloatField,
    pub entropy:    SliderField,
}

impl GlyphInspector {
    pub fn new() -> Self {
        Self {
            character:  StringField::new("Character", "A"),
            color:      ColorField::new("Color",      Vec4::ONE),
            emission:   SliderField::new("Emission",  0.0, 0.0, 2.0),
            glow_color: ColorField::new("Glow Color", Vec4::new(1.0, 0.8, 0.2, 1.0)),
            glow_radius: FloatField::new("Glow Radius", 1.0).with_range(0.0, 20.0),
            layer: EnumField::new(
                "Layer",
                vec!["Background".into(), "World".into(), "Entity".into(),
                     "Particle".into(), "UI".into(), "Overlay".into()],
                2,
            ),
            blend_mode: EnumField::new(
                "Blend Mode",
                vec!["Normal".into(), "Additive".into(), "Multiply".into(), "Screen".into()],
                0,
            ),
            mass:        FloatField::new("Mass",        1.0).with_range(0.001, 1000.0),
            charge:      FloatField::new("Charge",      0.0).with_range(-10.0, 10.0),
            temperature: FloatField::new("Temperature", 0.0).with_range(0.0, 10000.0),
            entropy:     SliderField::new("Entropy",    0.0, 0.0, 1.0),
        }
    }

    pub fn to_property_group(&self) -> PropertyGroup {
        let mut g = PropertyGroup::new("Glyph").with_icon('✦');
        g.push(InspectorEntry::String(self.character.clone()));
        g.push(InspectorEntry::Color(self.color.clone()));
        g.push(InspectorEntry::Slider(self.emission.clone()));
        g.push(InspectorEntry::Color(self.glow_color.clone()));
        g.push(InspectorEntry::Float(self.glow_radius.clone()));
        g.push(InspectorEntry::Enum(self.layer.clone()));
        g.push(InspectorEntry::Enum(self.blend_mode.clone()));
        g.push(InspectorEntry::Separator);
        g.push(InspectorEntry::Float(self.mass.clone()));
        g.push(InspectorEntry::Float(self.charge.clone()));
        g.push(InspectorEntry::Float(self.temperature.clone()));
        g.push(InspectorEntry::Slider(self.entropy.clone()));
        g
    }
}

impl Default for GlyphInspector {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// ForceFieldInspector
// ─────────────────────────────────────────────────────────────────────────────

/// Inspector section for force-field parameters.
#[derive(Debug, Clone)]
pub struct ForceFieldInspector {
    pub field_type: EnumField,
    pub strength:   FloatField,
    pub radius:     FloatField,
    pub falloff:    EnumField,
    pub position:   Vec3Field,
    pub direction:  Vec3Field,
    pub enabled:    BoolField,
    pub frequency:  FloatField,
    pub amplitude:  FloatField,
    pub phase:      FloatField,
}

impl ForceFieldInspector {
    pub fn new() -> Self {
        Self {
            field_type: EnumField::new(
                "Type",
                vec!["Gravity".into(), "Repulsion".into(), "Vortex".into(),
                     "Attractor".into(), "Wind".into(), "Turbulence".into(),
                     "Drag".into(), "Custom".into()],
                0,
            ),
            strength:  FloatField::new("Strength",  1.0).with_range(-1000.0, 1000.0),
            radius:    FloatField::new("Radius",     5.0).with_range(0.0, 500.0),
            falloff:   EnumField::new(
                "Falloff",
                vec!["None".into(), "Linear".into(), "InvSq".into(), "Exp".into()],
                1,
            ),
            position:  Vec3Field::new("Position",  Vec3::ZERO),
            direction: Vec3Field::new("Direction", Vec3::new(0.0, -1.0, 0.0)),
            enabled:   BoolField::new("Enabled",   true),
            frequency: FloatField::new("Frequency", 1.0).with_range(0.0, 100.0),
            amplitude: FloatField::new("Amplitude", 1.0).with_range(0.0, 100.0),
            phase:     FloatField::new("Phase",     0.0).with_range(0.0, std::f64::consts::TAU),
        }
    }

    pub fn to_property_group(&self) -> PropertyGroup {
        let mut g = PropertyGroup::new("Force Field").with_icon('⊛');
        g.push(InspectorEntry::Bool(self.enabled.clone()));
        g.push(InspectorEntry::Enum(self.field_type.clone()));
        g.push(InspectorEntry::Float(self.strength.clone()));
        g.push(InspectorEntry::Float(self.radius.clone()));
        g.push(InspectorEntry::Enum(self.falloff.clone()));
        g.push(InspectorEntry::Vec3(self.position.clone()));
        g.push(InspectorEntry::Vec3(self.direction.clone()));
        g.push(InspectorEntry::Separator);
        g.push(InspectorEntry::Float(self.frequency.clone()));
        g.push(InspectorEntry::Float(self.amplitude.clone()));
        g.push(InspectorEntry::Float(self.phase.clone()));
        g
    }
}

impl Default for ForceFieldInspector {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// ParticleInspector
// ─────────────────────────────────────────────────────────────────────────────

/// Inspector section for a particle emitter.
#[derive(Debug, Clone)]
pub struct ParticleInspector {
    pub preset:       EnumField,
    pub origin:       Vec3Field,
    pub active_count: IntField,
    pub emit_rate:    FloatField,
    pub lifetime:     FloatField,
    pub speed:        FloatField,
    pub spread_angle: SliderField,
    pub gravity_scale: FloatField,
    pub color_start:  ColorField,
    pub color_end:    ColorField,
    pub size_start:   FloatField,
    pub size_end:     FloatField,
    pub emitting:     BoolField,
}

impl ParticleInspector {
    pub fn new() -> Self {
        Self {
            preset: EnumField::new(
                "Preset",
                vec!["Explosion".into(), "Fire".into(), "Smoke".into(),
                     "Sparkle".into(), "Rain".into(), "Custom".into()],
                0,
            ),
            origin:       Vec3Field::new("Origin",       Vec3::ZERO),
            active_count: IntField::new("Active Particles", 0).with_range(0, 100_000),
            emit_rate:    FloatField::new("Emit Rate",   50.0).with_range(0.0, 10000.0),
            lifetime:     FloatField::new("Lifetime",     2.0).with_range(0.0, 60.0),
            speed:        FloatField::new("Speed",        5.0).with_range(0.0, 500.0),
            spread_angle: SliderField::new("Spread Angle", 45.0, 0.0, 360.0),
            gravity_scale: FloatField::new("Gravity Scale", 1.0).with_range(-10.0, 10.0),
            color_start:  ColorField::new("Color Start", Vec4::new(1.0, 0.8, 0.2, 1.0)),
            color_end:    ColorField::new("Color End",   Vec4::new(1.0, 0.0, 0.0, 0.0)),
            size_start:   FloatField::new("Size Start",  1.0).with_range(0.01, 10.0),
            size_end:     FloatField::new("Size End",    0.0).with_range(0.0, 10.0),
            emitting:     BoolField::new("Emitting",     true),
        }
    }

    pub fn to_property_group(&self) -> PropertyGroup {
        let mut g = PropertyGroup::new("Particle Emitter").with_icon('✦');
        g.push(InspectorEntry::Bool(self.emitting.clone()));
        g.push(InspectorEntry::Enum(self.preset.clone()));
        g.push(InspectorEntry::Vec3(self.origin.clone()));
        g.push(InspectorEntry::Int(self.active_count.clone()));
        g.push(InspectorEntry::Float(self.emit_rate.clone()));
        g.push(InspectorEntry::Float(self.lifetime.clone()));
        g.push(InspectorEntry::Float(self.speed.clone()));
        g.push(InspectorEntry::Slider(self.spread_angle.clone()));
        g.push(InspectorEntry::Float(self.gravity_scale.clone()));
        g.push(InspectorEntry::Separator);
        g.push(InspectorEntry::Color(self.color_start.clone()));
        g.push(InspectorEntry::Color(self.color_end.clone()));
        g.push(InspectorEntry::Float(self.size_start.clone()));
        g.push(InspectorEntry::Float(self.size_end.clone()));
        g
    }
}

impl Default for ParticleInspector {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// ScriptInspector
// ─────────────────────────────────────────────────────────────────────────────

/// Inspector for a script bound to an entity.
#[derive(Debug, Clone)]
pub struct ScriptInspector {
    pub script:    ScriptField,
    pub globals:   MapField,
    pub enabled:   BoolField,
    pub log_output: ListField,
}

impl ScriptInspector {
    pub fn new() -> Self {
        Self {
            script:     ScriptField::new("Script"),
            globals:    MapField::new("Globals"),
            enabled:    BoolField::new("Enabled", true),
            log_output: ListField::new("Output"),
        }
    }

    pub fn to_property_group(&self) -> PropertyGroup {
        let mut g = PropertyGroup::new("Script").with_icon('⌨');
        g.push(InspectorEntry::Bool(self.enabled.clone()));
        g.push(InspectorEntry::Script(self.script.clone()));
        g.push(InspectorEntry::Map(self.globals.clone()));
        g.push(InspectorEntry::List(self.log_output.clone()));
        g
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.log_output.push(msg);
    }

    pub fn set_global(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.globals.insert(key, val);
        self.script.set_global(
            self.globals.entries.last().map(|(k, _)| k.as_str()).unwrap_or(""),
            self.globals.entries.last().map(|(_, v)| v.as_str()).unwrap_or(""),
        );
    }
}

impl Default for ScriptInspector {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// ComponentInspector
// ─────────────────────────────────────────────────────────────────────────────

/// Shows all components of the selected entity as foldable property groups.
#[derive(Debug, Clone)]
pub struct ComponentInspector {
    pub entity_name: String,
    pub entity_id:   u32,
    pub transform:   TransformInspector,
    pub glyph:       Option<GlyphInspector>,
    pub force_field: Option<ForceFieldInspector>,
    pub particle:    Option<ParticleInspector>,
    pub script:      Option<ScriptInspector>,
    pub custom_groups: Vec<PropertyGroup>,
}

impl ComponentInspector {
    pub fn new(name: impl Into<String>, id: u32) -> Self {
        Self {
            entity_name: name.into(),
            entity_id: id,
            transform: TransformInspector::new(Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
            glyph: None,
            force_field: None,
            particle: None,
            script: None,
            custom_groups: Vec::new(),
        }
    }

    pub fn all_groups(&self) -> Vec<PropertyGroup> {
        let mut groups = vec![self.transform.to_property_group()];
        if let Some(ref g) = self.glyph       { groups.push(g.to_property_group()); }
        if let Some(ref f) = self.force_field  { groups.push(f.to_property_group()); }
        if let Some(ref p) = self.particle     { groups.push(p.to_property_group()); }
        if let Some(ref s) = self.script       { groups.push(s.to_property_group()); }
        groups.extend(self.custom_groups.iter().cloned());
        groups
    }

    pub fn add_custom_group(&mut self, group: PropertyGroup) {
        self.custom_groups.push(group);
    }

    pub fn render_ascii(&self, search: &SearchBar) -> String {
        let mut out = format!("┌── Entity: {} (id={})\n", self.entity_name, self.entity_id);
        for group in self.all_groups() {
            let filtered: Vec<_> = group.filter(&search.query);
            if !search.query.is_empty() && filtered.is_empty() { continue; }
            out.push_str(&group.render_ascii());
        }
        out.push_str("└──────────────────────────────\n");
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inspector (main panel)
// ─────────────────────────────────────────────────────────────────────────────

/// The top-level inspector panel.
pub struct Inspector {
    pub context: InspectorContext,
    pub search:  SearchBar,
    pub component_inspector: Option<ComponentInspector>,
    pub scroll_offset: f32,
    pub width:  f32,
    pub height: f32,
}

impl Inspector {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            context: InspectorContext::new(),
            search: SearchBar::new(),
            component_inspector: None,
            scroll_offset: 0.0,
            width,
            height,
        }
    }

    /// Load data for a given entity.
    pub fn load_entity(&mut self, id: u32, name: impl Into<String>) {
        self.component_inspector = Some(ComponentInspector::new(name, id));
        self.scroll_offset = 0.0;
        self.context.clear_dirty();
    }

    pub fn clear(&mut self) {
        self.component_inspector = None;
        self.context.clear_dirty();
    }

    pub fn scroll(&mut self, delta: f32) {
        self.scroll_offset = (self.scroll_offset + delta).max(0.0);
    }

    /// Render the panel to an ASCII string.
    pub fn render_ascii(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.search.render_ascii());
        out.push('\n');
        match &self.component_inspector {
            Some(ci) => out.push_str(&ci.render_ascii(&self.search)),
            None => out.push_str("(no selection)\n"),
        }
        out
    }

    /// Copy a named property to the clipboard.
    pub fn copy_property(&mut self, entry: &InspectorEntry) {
        let type_name = self
            .component_inspector
            .as_ref()
            .map(|ci| ci.entity_name.as_str())
            .unwrap_or("unknown");
        self.context.copy_value(entry, type_name);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Colour utility functions
// ─────────────────────────────────────────────────────────────────────────────

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta < f32::EPSILON {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max < f32::EPSILON { 0.0 } else { delta / max };
    let v = max;
    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s < f32::EPSILON {
        return (v, v, v);
    }
    let hi = ((h / 60.0) as i32) % 6;
    let f = h / 60.0 - (h / 60.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_field_toggle() {
        let mut f = BoolField::new("visible", true);
        assert!(f.value);
        f.toggle();
        assert!(!f.value);
        f.toggle();
        assert!(f.value);
    }

    #[test]
    fn test_int_field_clamp() {
        let mut f = IntField::new("count", 5).with_range(0, 10);
        f.set(15);
        assert_eq!(f.value, 10);
        f.set(-3);
        assert_eq!(f.value, 0);
    }

    #[test]
    fn test_float_field_set() {
        let mut f = FloatField::new("speed", 1.0).with_range(0.0, 10.0);
        f.set(5.5);
        assert!((f.value - 5.5).abs() < 1e-10);
        f.set(20.0);
        assert!((f.value - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_slider_normalized() {
        let mut s = SliderField::new("vol", 5.0, 0.0, 10.0);
        assert!((s.normalized() - 0.5).abs() < 1e-6);
        s.set_normalized(1.0);
        assert!((s.value - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_enum_cycle() {
        let mut e = EnumField::new("layer", vec!["A".into(), "B".into(), "C".into()], 0);
        e.next();
        assert_eq!(e.selected_name(), "B");
        e.prev();
        assert_eq!(e.selected_name(), "A");
        e.prev(); // wraps
        assert_eq!(e.selected_name(), "C");
    }

    #[test]
    fn test_color_field_hsv_roundtrip() {
        let mut c = ColorField::new("col", Vec4::new(1.0, 0.0, 0.0, 1.0));
        let (h, s, v) = (c.hue(), c.saturation(), c.brightness());
        c.set_hsv(h, s, v);
        assert!((c.color.x - 1.0).abs() < 0.01);
        assert!((c.color.y).abs() < 0.01);
    }

    #[test]
    fn test_string_field_max_len() {
        let mut f = StringField::new("name", "");
        f.max_len = 5;
        f.set("hello world");
        assert_eq!(f.value, "hello");
    }

    #[test]
    fn test_list_field_remove() {
        let mut l = ListField::new("items");
        l.push("a");
        l.push("b");
        l.push("c");
        l.selected_index = Some(1);
        l.remove_selected();
        assert_eq!(l.items, vec!["a", "c"]);
    }

    #[test]
    fn test_map_field_insert_update() {
        let mut m = MapField::new("props");
        m.insert("x", "1");
        m.insert("y", "2");
        m.insert("x", "99");
        assert_eq!(m.get("x"), Some("99"));
        assert_eq!(m.entries.len(), 2);
    }

    #[test]
    fn test_search_bar_filter() {
        let mut s = SearchBar::new();
        s.push('p');
        s.push('o');
        s.push('s');
        assert!(s.matches("position"));
        assert!(!s.matches("rotation"));
    }

    #[test]
    fn test_property_group_collapse() {
        let mut g = PropertyGroup::new("Transform")
            .add(InspectorEntry::Bool(BoolField::new("visible", true)));
        assert_eq!(g.visible_entries().len(), 1);
        g.toggle_collapsed();
        assert_eq!(g.visible_entries().len(), 0);
    }

    #[test]
    fn test_component_inspector_groups() {
        let mut ci = ComponentInspector::new("Hero", 1);
        ci.glyph = Some(GlyphInspector::new());
        let groups = ci.all_groups();
        assert!(groups.len() >= 2);
    }

    #[test]
    fn test_inspector_context_dirty() {
        let mut ctx = InspectorContext::new();
        ctx.mark_dirty("position");
        assert!(ctx.is_dirty("position"));
        ctx.clear_dirty();
        assert!(!ctx.is_dirty("position"));
    }

    #[test]
    fn test_vec3_field_clamping() {
        let mut f = Vec3Field::new("pos", Vec3::ZERO);
        f.min = Some(-5.0);
        f.max = Some(5.0);
        f.set(Vec3::new(10.0, -10.0, 3.0));
        assert!((f.value.x - 5.0).abs() < 1e-6);
        assert!((f.value.y + 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_transform_inspector_snap() {
        let mut ti = TransformInspector::new(
            Vec3::new(0.3, 0.7, -0.1),
            Vec3::ZERO,
            Vec3::ONE,
        );
        ti.snap_enabled = true;
        ti.snap_position = 0.25;
        ti.apply_snap_position();
        let p = ti.position.value;
        assert!((p.x - 0.25).abs() < 1e-5);
        assert!((p.y - 0.75).abs() < 1e-5);
        assert!((p.z).abs() < 1e-5);
    }
}
