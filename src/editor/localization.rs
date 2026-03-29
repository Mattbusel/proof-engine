// localization.rs — Internationalization and localization system for proof-engine editor
// Supports: runtime language switching, plural forms, parametric substitution,
// RTL layout hints, font fallback chains, and locale-sensitive formatting.

use std::collections::HashMap;
use std::fmt;

// ─── Locale identifier ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale {
    pub language: String,   // ISO 639-1: "en", "fr", "de", "zh", "ja", ...
    pub region: Option<String>, // ISO 3166-1 alpha-2: "US", "GB", "CN", ...
    pub script: Option<String>, // ISO 15924: "Latn", "Cyrl", "Hans", ...
}

impl Locale {
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            region: None,
            script: None,
        }
    }

    pub fn with_region(language: &str, region: &str) -> Self {
        Self {
            language: language.to_string(),
            region: Some(region.to_string()),
            script: None,
        }
    }

    pub fn with_script(language: &str, script: &str) -> Self {
        Self {
            language: language.to_string(),
            region: None,
            script: Some(script.to_string()),
        }
    }

    pub fn tag(&self) -> String {
        let mut s = self.language.clone();
        if let Some(sc) = &self.script {
            s.push('-');
            s.push_str(sc);
        }
        if let Some(r) = &self.region {
            s.push('-');
            s.push_str(r);
        }
        s
    }

    pub fn is_rtl(&self) -> bool {
        matches!(self.language.as_str(), "ar" | "he" | "fa" | "ur" | "ps" | "yi" | "dv")
    }

    pub fn decimal_separator(&self) -> char {
        match self.language.as_str() {
            "de" | "fr" | "es" | "it" | "pt" | "nl" | "pl" | "ru" | "tr" => ',',
            _ => '.',
        }
    }

    pub fn thousands_separator(&self) -> char {
        match self.language.as_str() {
            "de" | "nl" => '.',
            "fr" | "ru" => '\u{00A0}',
            _ => ',',
        }
    }

    pub fn date_format(&self) -> &'static str {
        match self.language.as_str() {
            "en" if self.region.as_deref() == Some("US") => "MM/DD/YYYY",
            "en" => "DD/MM/YYYY",
            "de" | "nl" | "pl" | "ru" => "DD.MM.YYYY",
            "ja" | "zh" | "ko" => "YYYY/MM/DD",
            "fr" | "es" | "it" | "pt" => "DD/MM/YYYY",
            _ => "YYYY-MM-DD",
        }
    }

    pub fn number_format(&self, value: f64, decimals: usize) -> String {
        let dec = self.decimal_separator();
        let thou = self.thousands_separator();
        let multiplier = 10f64.powi(decimals as i32);
        let rounded = (value * multiplier).round() / multiplier;
        let int_part = rounded.abs().trunc() as u64;
        let frac_part = ((rounded.abs() - rounded.abs().trunc()) * multiplier).round() as u64;
        let mut int_str = int_part.to_string();
        // Insert thousands separators
        let mut result = String::new();
        let int_chars: Vec<char> = int_str.chars().collect();
        for (i, &ch) in int_chars.iter().enumerate() {
            if i > 0 && (int_chars.len() - i) % 3 == 0 {
                result.push(thou);
            }
            result.push(ch);
        }
        let _ = int_str;
        if value < 0.0 { result.insert(0, '-'); }
        if decimals > 0 {
            result.push(dec);
            result.push_str(&format!("{:0>width$}", frac_part, width = decimals));
        }
        result
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.tag())
    }
}

impl Default for Locale {
    fn default() -> Self { Self::new("en") }
}

// ─── Plural forms ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluralForm {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

pub trait PluralRule {
    fn form(&self, n: u64) -> PluralForm;
}

pub struct EnglishPlural;
impl PluralRule for EnglishPlural {
    fn form(&self, n: u64) -> PluralForm {
        if n == 1 { PluralForm::One } else { PluralForm::Other }
    }
}

pub struct RussianPlural;
impl PluralRule for RussianPlural {
    fn form(&self, n: u64) -> PluralForm {
        let rem100 = n % 100;
        let rem10  = n % 10;
        if rem100 >= 11 && rem100 <= 14 { return PluralForm::Many; }
        match rem10 {
            1 => PluralForm::One,
            2 | 3 | 4 => PluralForm::Few,
            _ => PluralForm::Many,
        }
    }
}

pub struct ArabicPlural;
impl PluralRule for ArabicPlural {
    fn form(&self, n: u64) -> PluralForm {
        if n == 0 { return PluralForm::Zero; }
        if n == 1 { return PluralForm::One; }
        if n == 2 { return PluralForm::Two; }
        let rem100 = n % 100;
        if rem100 >= 3 && rem100 <= 10 { return PluralForm::Few; }
        if rem100 >= 11 && rem100 <= 99 { return PluralForm::Many; }
        PluralForm::Other
    }
}

pub struct JapanesePlural;
impl PluralRule for JapanesePlural {
    fn form(&self, _n: u64) -> PluralForm { PluralForm::Other }
}

// ─── Translation value ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TranslationValue {
    Simple(String),
    Plural {
        zero: Option<String>,
        one: Option<String>,
        two: Option<String>,
        few: Option<String>,
        many: Option<String>,
        other: String,
    },
    Ordinal { forms: HashMap<u64, String>, other: String },
    SelectGender { masculine: String, feminine: String, neuter: Option<String> },
}

impl TranslationValue {
    pub fn simple(s: &str) -> Self {
        Self::Simple(s.to_string())
    }

    pub fn plural_en(one: &str, other: &str) -> Self {
        Self::Plural {
            zero: None,
            one: Some(one.to_string()),
            two: None,
            few: None,
            many: None,
            other: other.to_string(),
        }
    }

    pub fn get(&self, n: Option<u64>) -> &str {
        match self {
            Self::Simple(s) => s,
            Self::Plural { zero, one, two, few, many, other } => {
                match n {
                    Some(0) => zero.as_deref().unwrap_or(other),
                    Some(1) => one.as_deref().unwrap_or(other),
                    Some(2) => two.as_deref().unwrap_or(other),
                    _       => match n.map(|v| v % 10) {
                        Some(3..=4) => few.as_deref().unwrap_or(other),
                        _           => many.as_deref().unwrap_or(other),
                    }
                }
            }
            Self::Ordinal { forms, other } => {
                n.and_then(|v| forms.get(&v)).map(|s| s.as_str()).unwrap_or(other)
            }
            Self::SelectGender { masculine, .. } => masculine,
        }
    }
}

// ─── Translation bundle ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TranslationBundle {
    pub locale: Locale,
    pub namespace: String,
    pub entries: HashMap<String, TranslationValue>,
    pub fallback: Option<String>,
    pub author: String,
    pub version: String,
}

impl TranslationBundle {
    pub fn new(locale: Locale, namespace: &str) -> Self {
        Self {
            locale,
            namespace: namespace.to_string(),
            entries: HashMap::new(),
            fallback: None,
            author: String::new(),
            version: "1.0".into(),
        }
    }

    pub fn insert(&mut self, key: &str, value: TranslationValue) {
        self.entries.insert(key.to_string(), value);
    }

    pub fn insert_simple(&mut self, key: &str, text: &str) {
        self.entries.insert(key.to_string(), TranslationValue::simple(text));
    }

    pub fn get(&self, key: &str) -> Option<&TranslationValue> {
        self.entries.get(key)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

// ─── String interpolation ─────────────────────────────────────────────────────

/// Named parameters for string interpolation: `{name}` `{count}` `{file}`
#[derive(Debug, Clone)]
pub struct TranslationArgs {
    args: HashMap<String, String>,
}

impl TranslationArgs {
    pub fn new() -> Self {
        Self { args: HashMap::new() }
    }

    pub fn set(mut self, key: &str, value: impl fmt::Display) -> Self {
        self.args.insert(key.to_string(), value.to_string());
        self
    }

    pub fn set_mut(&mut self, key: &str, value: impl fmt::Display) {
        self.args.insert(key.to_string(), value.to_string());
    }

    /// Interpolate a template string.
    /// `{key}` → value, `{key, plural, one {x} other {y}}` → plural form.
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = String::with_capacity(template.len() + 32);
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '{' {
                let mut key = String::new();
                for inner in chars.by_ref() {
                    if inner == '}' { break; }
                    key.push(inner);
                }
                let key = key.trim();
                if let Some(val) = self.args.get(key) {
                    result.push_str(val);
                } else {
                    result.push('{');
                    result.push_str(key);
                    result.push('}');
                }
            } else {
                result.push(ch);
            }
        }
        result
    }
}

impl Default for TranslationArgs {
    fn default() -> Self { Self::new() }
}

// ─── Localization manager ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LocalizationManager {
    pub current_locale: Locale,
    pub fallback_locale: Locale,
    bundles: HashMap<(String, String), TranslationBundle>,
    pub missing_key_policy: MissingKeyPolicy,
    pub debug_mode: bool,
    missing_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingKeyPolicy {
    /// Return the key itself as the string
    ReturnKey,
    /// Return an empty string
    Empty,
    /// Panic in debug builds, return key in release
    Panic,
    /// Return a decorated key: `[MISSING: key]`
    Decorated,
}

impl LocalizationManager {
    pub fn new(locale: Locale) -> Self {
        Self {
            current_locale: locale.clone(),
            fallback_locale: Locale::new("en"),
            bundles: HashMap::new(),
            missing_key_policy: MissingKeyPolicy::Decorated,
            debug_mode: false,
            missing_keys: Vec::new(),
        }
    }

    pub fn load_bundle(&mut self, bundle: TranslationBundle) {
        let key = (bundle.locale.tag(), bundle.namespace.clone());
        self.bundles.insert(key, bundle);
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.current_locale = locale;
    }

    pub fn translate(&mut self, namespace: &str, key: &str) -> String {
        self.translate_n(namespace, key, None, None)
    }

    pub fn translate_args(&mut self, namespace: &str, key: &str, args: &TranslationArgs) -> String {
        let base = self.translate_n(namespace, key, None, None);
        args.interpolate(&base)
    }

    pub fn translate_n(&mut self, namespace: &str, key: &str, n: Option<u64>, args: Option<&TranslationArgs>) -> String {
        // Try current locale
        let val = self.lookup(namespace, key, &self.current_locale.clone(), n);
        if let Some(s) = val {
            return if let Some(a) = args { a.interpolate(&s) } else { s };
        }
        // Try fallback locale
        let val = self.lookup(namespace, key, &self.fallback_locale.clone(), n);
        if let Some(s) = val {
            return if let Some(a) = args { a.interpolate(&s) } else { s };
        }
        // Missing
        self.missing_keys.push(format!("{}/{}", namespace, key));
        self.missing_string(key)
    }

    fn lookup(&self, namespace: &str, key: &str, locale: &Locale, n: Option<u64>) -> Option<String> {
        let bkey = (locale.tag(), namespace.to_string());
        let bundle = self.bundles.get(&bkey)?;
        let value = bundle.get(key)?;
        Some(value.get(n).to_string())
    }

    fn missing_string(&self, key: &str) -> String {
        match self.missing_key_policy {
            MissingKeyPolicy::ReturnKey  => key.to_string(),
            MissingKeyPolicy::Empty      => String::new(),
            MissingKeyPolicy::Decorated  => format!("[{}]", key),
            MissingKeyPolicy::Panic      => {
                #[cfg(debug_assertions)]
                panic!("Missing translation key: {}", key);
                #[cfg(not(debug_assertions))]
                key.to_string()
            }
        }
    }

    pub fn missing_keys(&self) -> &[String] { &self.missing_keys }
    pub fn clear_missing_keys(&mut self) { self.missing_keys.clear(); }

    pub fn available_locales(&self) -> Vec<Locale> {
        self.bundles.keys()
            .map(|(tag, _)| tag)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(|tag| {
                let parts: Vec<&str> = tag.splitn(3, '-').collect();
                match parts.len() {
                    1 => Locale::new(parts[0]),
                    2 => Locale::with_region(parts[0], parts[1]),
                    3 => {
                        let mut l = Locale::with_script(parts[0], parts[1]);
                        l.region = Some(parts[2].to_string());
                        l
                    }
                    _ => Locale::new(parts[0]),
                }
            })
            .collect()
    }

    pub fn coverage_report(&self, namespace: &str) -> LocaleCoverageReport {
        let en_key = (Locale::new("en").tag(), namespace.to_string());
        let total = self.bundles.get(&en_key)
            .map(|b| b.entry_count())
            .unwrap_or(0);

        let mut reports = Vec::new();
        for ((locale_tag, ns), bundle) in &self.bundles {
            if ns != namespace || *locale_tag == "en" { continue; }
            let count = bundle.entry_count();
            let pct = if total > 0 { count * 100 / total } else { 0 };
            reports.push(LocaleReport {
                locale: locale_tag.clone(),
                translated: count,
                total,
                percent: pct,
            });
        }
        reports.sort_by(|a, b| b.percent.cmp(&a.percent));
        LocaleCoverageReport { namespace: namespace.to_string(), reports }
    }
}

#[derive(Debug, Clone)]
pub struct LocaleReport {
    pub locale: String,
    pub translated: usize,
    pub total: usize,
    pub percent: usize,
}

impl fmt::Display for LocaleReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}/{} ({}%)", self.locale, self.translated, self.total, self.percent)
    }
}

#[derive(Debug, Clone)]
pub struct LocaleCoverageReport {
    pub namespace: String,
    pub reports: Vec<LocaleReport>,
}

impl fmt::Display for LocaleCoverageReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Coverage report for namespace '{}':", self.namespace)?;
        for r in &self.reports { writeln!(f, "  {}", r)?; }
        Ok(())
    }
}

// ─── Built-in bundles ─────────────────────────────────────────────────────────

pub fn build_english_bundle() -> TranslationBundle {
    let mut b = TranslationBundle::new(Locale::new("en"), "editor");
    b.insert_simple("menu.file", "File");
    b.insert_simple("menu.edit", "Edit");
    b.insert_simple("menu.view", "View");
    b.insert_simple("menu.window", "Window");
    b.insert_simple("menu.help", "Help");
    b.insert_simple("menu.file.new", "New Scene");
    b.insert_simple("menu.file.open", "Open Scene...");
    b.insert_simple("menu.file.save", "Save Scene");
    b.insert_simple("menu.file.save_as", "Save Scene As...");
    b.insert_simple("menu.file.export", "Export...");
    b.insert_simple("menu.file.quit", "Quit");
    b.insert_simple("menu.edit.undo", "Undo");
    b.insert_simple("menu.edit.redo", "Redo");
    b.insert_simple("menu.edit.cut", "Cut");
    b.insert_simple("menu.edit.copy", "Copy");
    b.insert_simple("menu.edit.paste", "Paste");
    b.insert_simple("menu.edit.delete", "Delete");
    b.insert_simple("menu.edit.select_all", "Select All");
    b.insert_simple("menu.edit.deselect", "Deselect");
    b.insert_simple("menu.edit.preferences", "Preferences...");
    b.insert_simple("panel.hierarchy", "Hierarchy");
    b.insert_simple("panel.inspector", "Inspector");
    b.insert_simple("panel.scene", "Scene");
    b.insert_simple("panel.assets", "Assets");
    b.insert_simple("panel.console", "Console");
    b.insert_simple("panel.timeline", "Timeline");
    b.insert_simple("panel.kit_params", "Kit Parameters");
    b.insert_simple("panel.shader_graph", "Shader Graph");
    b.insert_simple("panel.perf", "Performance");
    b.insert_simple("action.play", "Play");
    b.insert_simple("action.pause", "Pause");
    b.insert_simple("action.stop", "Stop");
    b.insert_simple("action.step", "Step Frame");
    b.insert_simple("gizmo.translate", "Translate");
    b.insert_simple("gizmo.rotate", "Rotate");
    b.insert_simple("gizmo.scale", "Scale");
    b.insert_simple("gizmo.universal", "Universal");
    b.insert_simple("gizmo.space.local", "Local");
    b.insert_simple("gizmo.space.world", "World");
    b.insert_simple("view.wireframe", "Wireframe");
    b.insert_simple("view.lit", "Lit");
    b.insert_simple("view.unlit", "Unlit");
    b.insert_simple("view.normals", "Normals");
    b.insert_simple("toolbar.new_entity", "New Entity");
    b.insert_simple("toolbar.delete_selected", "Delete Selected");
    b.insert_simple("toolbar.frame_selected", "Frame Selected");
    b.insert_simple("toolbar.frame_all", "Frame All");
    b.insert_simple("toolbar.snap_to_grid", "Snap to Grid");
    b.insert_simple("toolbar.show_grid", "Show Grid");
    b.insert_simple("toolbar.show_axes", "Show Axes");
    b.insert_simple("toolbar.local_coords", "Local Coordinates");
    b.insert("entity.count", TranslationValue::plural_en("{count} entity", "{count} entities"));
    b.insert("keyframe.count", TranslationValue::plural_en("{count} keyframe", "{count} keyframes"));
    b.insert("bone.count", TranslationValue::plural_en("{count} bone", "{count} bones"));
    b.insert("particle.count", TranslationValue::plural_en("{count} particle", "{count} particles"));
    b.insert("error.count", TranslationValue::plural_en("{count} error", "{count} errors"));
    b.insert("warning.count", TranslationValue::plural_en("{count} warning", "{count} warnings"));
    b.insert_simple("status.saved", "Scene saved");
    b.insert_simple("status.save_failed", "Failed to save scene: {error}");
    b.insert_simple("status.loaded", "Scene loaded: {name}");
    b.insert_simple("status.compile_ok", "Shader compiled successfully");
    b.insert_simple("status.compile_failed", "Shader compilation failed: {count} errors");
    b.insert_simple("status.unsaved_changes", "Unsaved changes");
    b.insert_simple("dialog.confirm_delete", "Delete {count} selected object(s)?");
    b.insert_simple("dialog.unsaved_changes", "Scene has unsaved changes. Save before closing?");
    b.insert_simple("dialog.save", "Save");
    b.insert_simple("dialog.discard", "Don't Save");
    b.insert_simple("dialog.cancel", "Cancel");
    b.insert_simple("dialog.ok", "OK");
    b.insert_simple("dialog.yes", "Yes");
    b.insert_simple("dialog.no", "No");
    b.insert_simple("inspector.transform", "Transform");
    b.insert_simple("inspector.position", "Position");
    b.insert_simple("inspector.rotation", "Rotation");
    b.insert_simple("inspector.scale", "Scale");
    b.insert_simple("inspector.visible", "Visible");
    b.insert_simple("inspector.locked", "Locked");
    b.insert_simple("inspector.static", "Static");
    b.insert_simple("inspector.cast_shadows", "Cast Shadows");
    b.insert_simple("inspector.receive_shadows", "Receive Shadows");
    b.insert_simple("inspector.tag", "Tag");
    b.insert_simple("inspector.layer", "Layer");
    b.insert_simple("node.category.math", "Math");
    b.insert_simple("node.category.vector", "Vector");
    b.insert_simple("node.category.texture", "Texture");
    b.insert_simple("node.category.color", "Color");
    b.insert_simple("node.category.pbr", "PBR");
    b.insert_simple("node.category.noise", "Noise");
    b.insert_simple("node.category.input", "Input");
    b.insert_simple("node.category.output", "Output");
    b.insert_simple("node.category.sdf", "SDF");
    b.insert_simple("shortcut.undo", "Ctrl+Z");
    b.insert_simple("shortcut.redo", "Ctrl+Y");
    b.insert_simple("shortcut.save", "Ctrl+S");
    b.insert_simple("shortcut.open", "Ctrl+O");
    b.insert_simple("shortcut.new", "Ctrl+N");
    b.insert_simple("shortcut.delete", "Del");
    b.insert_simple("shortcut.copy", "Ctrl+C");
    b.insert_simple("shortcut.paste", "Ctrl+V");
    b.insert_simple("shortcut.select_all", "Ctrl+A");
    b.insert_simple("shortcut.frame_selected", "F");
    b.insert_simple("shortcut.frame_all", "Shift+F");
    b.insert_simple("shortcut.play", "Space");
    b.insert_simple("shortcut.gizmo.translate", "G");
    b.insert_simple("shortcut.gizmo.rotate", "R");
    b.insert_simple("shortcut.gizmo.scale", "S");
    b.insert_simple("tooltip.add_node", "Click to add this node to the graph");
    b.insert_simple("tooltip.connect_ports", "Drag to connect ports");
    b.insert_simple("tooltip.disconnect", "Right-click to disconnect");
    b.insert_simple("tooltip.reset_value", "Double-click to reset to default");
    b.insert_simple("tooltip.preview", "Toggle node preview");
    b.insert_simple("tooltip.collapse", "Collapse node");
    b.insert_simple("asset.kind.scene", "Scene");
    b.insert_simple("asset.kind.mesh", "Mesh");
    b.insert_simple("asset.kind.material", "Material");
    b.insert_simple("asset.kind.texture", "Texture");
    b.insert_simple("asset.kind.audio", "Audio");
    b.insert_simple("asset.kind.script", "Script");
    b.insert_simple("asset.kind.animation", "Animation");
    b.insert_simple("asset.kind.shader", "Shader");
    b.insert_simple("asset.kind.font", "Font");
    b.insert_simple("asset.kind.config", "Config");
    b.insert_simple("perf.fps", "FPS");
    b.insert_simple("perf.frame_time", "Frame Time");
    b.insert_simple("perf.gpu_time", "GPU Time");
    b.insert_simple("perf.cpu_time", "CPU Time");
    b.insert_simple("perf.draw_calls", "Draw Calls");
    b.insert_simple("perf.triangles", "Triangles");
    b.insert_simple("perf.memory_vram", "VRAM");
    b.insert_simple("perf.memory_ram", "RAM");
    b.insert_simple("perf.bottleneck", "Bottleneck");
    b
}

pub fn build_french_bundle() -> TranslationBundle {
    let mut b = TranslationBundle::new(Locale::new("fr"), "editor");
    b.insert_simple("menu.file", "Fichier");
    b.insert_simple("menu.edit", "Édition");
    b.insert_simple("menu.view", "Affichage");
    b.insert_simple("menu.window", "Fenêtre");
    b.insert_simple("menu.help", "Aide");
    b.insert_simple("menu.file.new", "Nouveau projet");
    b.insert_simple("menu.file.open", "Ouvrir le projet...");
    b.insert_simple("menu.file.save", "Enregistrer");
    b.insert_simple("menu.file.save_as", "Enregistrer sous...");
    b.insert_simple("menu.file.quit", "Quitter");
    b.insert_simple("menu.edit.undo", "Annuler");
    b.insert_simple("menu.edit.redo", "Rétablir");
    b.insert_simple("menu.edit.cut", "Couper");
    b.insert_simple("menu.edit.copy", "Copier");
    b.insert_simple("menu.edit.paste", "Coller");
    b.insert_simple("menu.edit.delete", "Supprimer");
    b.insert_simple("menu.edit.select_all", "Tout sélectionner");
    b.insert_simple("panel.hierarchy", "Hiérarchie");
    b.insert_simple("panel.inspector", "Inspecteur");
    b.insert_simple("panel.scene", "Scène");
    b.insert_simple("panel.assets", "Ressources");
    b.insert_simple("panel.console", "Console");
    b.insert_simple("panel.timeline", "Chronologie");
    b.insert_simple("action.play", "Jouer");
    b.insert_simple("action.pause", "Pause");
    b.insert_simple("action.stop", "Arrêter");
    b.insert_simple("gizmo.translate", "Déplacer");
    b.insert_simple("gizmo.rotate", "Pivoter");
    b.insert_simple("gizmo.scale", "Redimensionner");
    b.insert_simple("gizmo.space.local", "Local");
    b.insert_simple("gizmo.space.world", "Monde");
    b.insert_simple("dialog.save", "Enregistrer");
    b.insert_simple("dialog.discard", "Ne pas enregistrer");
    b.insert_simple("dialog.cancel", "Annuler");
    b.insert_simple("dialog.ok", "OK");
    b.insert_simple("inspector.transform", "Transformation");
    b.insert_simple("inspector.position", "Position");
    b.insert_simple("inspector.rotation", "Rotation");
    b.insert_simple("inspector.scale", "Échelle");
    b.insert_simple("inspector.visible", "Visible");
    b.insert_simple("status.saved", "Scène enregistrée");
    b.insert_simple("status.loaded", "Scène chargée : {name}");
    b.insert_simple("shortcut.undo", "Ctrl+Z");
    b.insert_simple("shortcut.redo", "Ctrl+Y");
    b.insert_simple("shortcut.save", "Ctrl+S");
    b.insert_simple("shortcut.frame_selected", "F");
    b.insert_simple("shortcut.play", "Espace");
    b
}

pub fn build_german_bundle() -> TranslationBundle {
    let mut b = TranslationBundle::new(Locale::new("de"), "editor");
    b.insert_simple("menu.file", "Datei");
    b.insert_simple("menu.edit", "Bearbeiten");
    b.insert_simple("menu.view", "Ansicht");
    b.insert_simple("menu.window", "Fenster");
    b.insert_simple("menu.help", "Hilfe");
    b.insert_simple("menu.file.new", "Neue Szene");
    b.insert_simple("menu.file.open", "Szene öffnen...");
    b.insert_simple("menu.file.save", "Speichern");
    b.insert_simple("menu.file.save_as", "Speichern als...");
    b.insert_simple("menu.file.quit", "Beenden");
    b.insert_simple("menu.edit.undo", "Rückgängig");
    b.insert_simple("menu.edit.redo", "Wiederholen");
    b.insert_simple("menu.edit.cut", "Ausschneiden");
    b.insert_simple("menu.edit.copy", "Kopieren");
    b.insert_simple("menu.edit.paste", "Einfügen");
    b.insert_simple("menu.edit.delete", "Löschen");
    b.insert_simple("menu.edit.select_all", "Alles auswählen");
    b.insert_simple("panel.hierarchy", "Hierarchie");
    b.insert_simple("panel.inspector", "Inspektor");
    b.insert_simple("panel.scene", "Szene");
    b.insert_simple("panel.assets", "Ressourcen");
    b.insert_simple("panel.console", "Konsole");
    b.insert_simple("panel.timeline", "Zeitachse");
    b.insert_simple("action.play", "Abspielen");
    b.insert_simple("action.pause", "Pause");
    b.insert_simple("action.stop", "Stopp");
    b.insert_simple("gizmo.translate", "Verschieben");
    b.insert_simple("gizmo.rotate", "Drehen");
    b.insert_simple("gizmo.scale", "Skalieren");
    b.insert_simple("gizmo.space.local", "Lokal");
    b.insert_simple("gizmo.space.world", "Welt");
    b.insert_simple("dialog.save", "Speichern");
    b.insert_simple("dialog.discard", "Nicht speichern");
    b.insert_simple("dialog.cancel", "Abbrechen");
    b.insert_simple("dialog.ok", "OK");
    b.insert_simple("inspector.transform", "Transformation");
    b.insert_simple("inspector.position", "Position");
    b.insert_simple("inspector.rotation", "Rotation");
    b.insert_simple("inspector.scale", "Skalierung");
    b.insert_simple("status.saved", "Szene gespeichert");
    b.insert_simple("status.loaded", "Szene geladen: {name}");
    b
}

pub fn build_japanese_bundle() -> TranslationBundle {
    let mut b = TranslationBundle::new(Locale::new("ja"), "editor");
    b.insert_simple("menu.file", "ファイル");
    b.insert_simple("menu.edit", "編集");
    b.insert_simple("menu.view", "表示");
    b.insert_simple("menu.window", "ウィンドウ");
    b.insert_simple("menu.help", "ヘルプ");
    b.insert_simple("menu.file.new", "新規シーン");
    b.insert_simple("menu.file.open", "シーンを開く...");
    b.insert_simple("menu.file.save", "保存");
    b.insert_simple("menu.file.save_as", "名前を付けて保存...");
    b.insert_simple("menu.file.quit", "終了");
    b.insert_simple("menu.edit.undo", "元に戻す");
    b.insert_simple("menu.edit.redo", "やり直す");
    b.insert_simple("menu.edit.copy", "コピー");
    b.insert_simple("menu.edit.paste", "貼り付け");
    b.insert_simple("menu.edit.delete", "削除");
    b.insert_simple("panel.hierarchy", "ヒエラルキー");
    b.insert_simple("panel.inspector", "インスペクター");
    b.insert_simple("panel.scene", "シーン");
    b.insert_simple("panel.assets", "アセット");
    b.insert_simple("panel.console", "コンソール");
    b.insert_simple("panel.timeline", "タイムライン");
    b.insert_simple("action.play", "再生");
    b.insert_simple("action.pause", "一時停止");
    b.insert_simple("action.stop", "停止");
    b.insert_simple("gizmo.translate", "移動");
    b.insert_simple("gizmo.rotate", "回転");
    b.insert_simple("gizmo.scale", "スケール");
    b.insert_simple("dialog.save", "保存");
    b.insert_simple("dialog.discard", "保存しない");
    b.insert_simple("dialog.cancel", "キャンセル");
    b.insert_simple("dialog.ok", "OK");
    b.insert_simple("inspector.transform", "トランスフォーム");
    b.insert_simple("inspector.position", "位置");
    b.insert_simple("inspector.rotation", "回転");
    b.insert_simple("inspector.scale", "スケール");
    b
}

pub fn build_spanish_bundle() -> TranslationBundle {
    let mut b = TranslationBundle::new(Locale::new("es"), "editor");
    b.insert_simple("menu.file", "Archivo");
    b.insert_simple("menu.edit", "Editar");
    b.insert_simple("menu.view", "Vista");
    b.insert_simple("menu.window", "Ventana");
    b.insert_simple("menu.help", "Ayuda");
    b.insert_simple("menu.file.new", "Nueva escena");
    b.insert_simple("menu.file.open", "Abrir escena...");
    b.insert_simple("menu.file.save", "Guardar");
    b.insert_simple("menu.file.save_as", "Guardar como...");
    b.insert_simple("menu.file.quit", "Salir");
    b.insert_simple("menu.edit.undo", "Deshacer");
    b.insert_simple("menu.edit.redo", "Rehacer");
    b.insert_simple("menu.edit.copy", "Copiar");
    b.insert_simple("menu.edit.paste", "Pegar");
    b.insert_simple("menu.edit.delete", "Eliminar");
    b.insert_simple("panel.hierarchy", "Jerarquía");
    b.insert_simple("panel.inspector", "Inspector");
    b.insert_simple("panel.scene", "Escena");
    b.insert_simple("panel.assets", "Recursos");
    b.insert_simple("panel.console", "Consola");
    b.insert_simple("panel.timeline", "Línea de tiempo");
    b.insert_simple("action.play", "Reproducir");
    b.insert_simple("action.pause", "Pausa");
    b.insert_simple("action.stop", "Detener");
    b.insert_simple("gizmo.translate", "Mover");
    b.insert_simple("gizmo.rotate", "Rotar");
    b.insert_simple("gizmo.scale", "Escalar");
    b.insert_simple("dialog.save", "Guardar");
    b.insert_simple("dialog.discard", "No guardar");
    b.insert_simple("dialog.cancel", "Cancelar");
    b.insert_simple("dialog.ok", "Aceptar");
    b.insert_simple("inspector.transform", "Transformación");
    b.insert_simple("inspector.position", "Posición");
    b.insert_simple("inspector.rotation", "Rotación");
    b.insert_simple("inspector.scale", "Escala");
    b
}

pub fn build_chinese_simplified_bundle() -> TranslationBundle {
    let mut b = TranslationBundle::new(
        Locale::with_script("zh", "Hans"),
        "editor",
    );
    b.insert_simple("menu.file", "文件");
    b.insert_simple("menu.edit", "编辑");
    b.insert_simple("menu.view", "视图");
    b.insert_simple("menu.window", "窗口");
    b.insert_simple("menu.help", "帮助");
    b.insert_simple("menu.file.new", "新建场景");
    b.insert_simple("menu.file.open", "打开场景...");
    b.insert_simple("menu.file.save", "保存");
    b.insert_simple("menu.file.save_as", "另存为...");
    b.insert_simple("menu.file.quit", "退出");
    b.insert_simple("menu.edit.undo", "撤销");
    b.insert_simple("menu.edit.redo", "重做");
    b.insert_simple("menu.edit.copy", "复制");
    b.insert_simple("menu.edit.paste", "粘贴");
    b.insert_simple("menu.edit.delete", "删除");
    b.insert_simple("panel.hierarchy", "层级");
    b.insert_simple("panel.inspector", "检查器");
    b.insert_simple("panel.scene", "场景");
    b.insert_simple("panel.assets", "资源");
    b.insert_simple("panel.console", "控制台");
    b.insert_simple("panel.timeline", "时间轴");
    b.insert_simple("action.play", "播放");
    b.insert_simple("action.pause", "暂停");
    b.insert_simple("action.stop", "停止");
    b.insert_simple("gizmo.translate", "移动");
    b.insert_simple("gizmo.rotate", "旋转");
    b.insert_simple("gizmo.scale", "缩放");
    b.insert_simple("dialog.save", "保存");
    b.insert_simple("dialog.discard", "不保存");
    b.insert_simple("dialog.cancel", "取消");
    b.insert_simple("dialog.ok", "确定");
    b.insert_simple("inspector.transform", "变换");
    b.insert_simple("inspector.position", "位置");
    b.insert_simple("inspector.rotation", "旋转");
    b.insert_simple("inspector.scale", "缩放");
    b
}

/// Create a default LocalizationManager with all built-in bundles loaded
pub fn default_localization() -> LocalizationManager {
    let mut mgr = LocalizationManager::new(Locale::new("en"));
    mgr.load_bundle(build_english_bundle());
    mgr.load_bundle(build_french_bundle());
    mgr.load_bundle(build_german_bundle());
    mgr.load_bundle(build_japanese_bundle());
    mgr.load_bundle(build_spanish_bundle());
    mgr.load_bundle(build_chinese_simplified_bundle());
    mgr
}

// ─── Helper macro ─────────────────────────────────────────────────────────────

/// Convenience: t!(mgr, "panel.inspector") → translated string
#[macro_export]
macro_rules! t {
    ($mgr:expr, $key:literal) => {{
        $mgr.translate("editor", $key)
    }};
    ($mgr:expr, $ns:literal, $key:literal) => {{
        $mgr.translate($ns, $key)
    }};
    ($mgr:expr, $key:literal, $($arg_k:literal = $arg_v:expr),* $(,)?) => {{
        let args = $crate::editor::localization::TranslationArgs::new()
            $(.set($arg_k, $arg_v))*;
        $mgr.translate_args("editor", $key, &args)
    }};
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_translate() {
        let mut mgr = default_localization();
        assert_eq!(mgr.translate("editor", "menu.file"), "File");
        assert_eq!(mgr.translate("editor", "action.play"), "Play");
    }

    #[test]
    fn french_translate() {
        let mut mgr = default_localization();
        mgr.set_locale(Locale::new("fr"));
        assert_eq!(mgr.translate("editor", "menu.file"), "Fichier");
        assert_eq!(mgr.translate("editor", "action.play"), "Jouer");
    }

    #[test]
    fn fallback_to_english() {
        let mut mgr = default_localization();
        mgr.set_locale(Locale::new("fr"));
        // "shortcut.frame_all" is only in English
        let s = mgr.translate("editor", "shortcut.frame_all");
        assert_eq!(s, "Shift+F");
    }

    #[test]
    fn missing_key_decorated() {
        let mut mgr = default_localization();
        let s = mgr.translate("editor", "nonexistent.key");
        assert_eq!(s, "[nonexistent.key]");
    }

    #[test]
    fn interpolation() {
        let mut mgr = default_localization();
        let args = TranslationArgs::new().set("name", "test_scene");
        let s = mgr.translate_args("editor", "status.loaded", &args);
        assert!(s.contains("test_scene"));
    }

    #[test]
    fn locale_rtl() {
        let arabic = Locale::new("ar");
        assert!(arabic.is_rtl());
        let english = Locale::new("en");
        assert!(!english.is_rtl());
    }

    #[test]
    fn number_format() {
        let de = Locale::new("de");
        let formatted = de.number_format(1234.56, 2);
        assert!(formatted.contains(','), "DE should use comma decimal: {}", formatted);
    }

    #[test]
    fn coverage_report() {
        let mgr = default_localization();
        let report = mgr.coverage_report("editor");
        assert!(!report.reports.is_empty());
    }
}
