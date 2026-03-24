//! Asset loaders — traits and built-in implementations for common file types.
//!
//! ## Extension Model
//!
//! Each loader declares which file extensions it handles via `extensions()`.
//! The `AssetRegistry` maps extensions to loaders at startup. When the server
//! needs to load a file it picks the matching loader and calls `load_bytes`.
//!
//! ## Sub-assets and Dependencies
//!
//! Loaders receive a `LoadContext` they can use to:
//! * Register labeled sub-assets (`ctx.add_labeled_asset`)
//! * Declare load-time dependencies (`ctx.add_dependency`)
//!
//! All sub-assets become independently addressable via `AssetPath` labels.

use std::any::Any;
use std::fmt;

use crate::asset::handle::AssetPath;

// ─────────────────────────────────────────────
//  AssetLoadError
// ─────────────────────────────────────────────

/// Errors that can occur while loading an asset.
#[derive(Debug, Clone)]
pub enum AssetLoadError {
    /// The underlying file could not be read.
    Io { path: String, message: String },
    /// The bytes could not be parsed as the expected format.
    Parse { path: String, message: String },
    /// The file format is not supported or was not recognized.
    UnsupportedFormat { extension: String },
    /// The file was valid but contained bad/out-of-range data.
    InvalidData { message: String },
    /// A dependency of this asset failed to load.
    DependencyFailed { dependency: String, message: String },
    /// No loader is registered for this asset type or extension.
    NoLoader,
    /// A catch-all for other errors.
    Other(String),
}

impl fmt::Display for AssetLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetLoadError::Io { path, message } => {
                write!(f, "IO error loading '{path}': {message}")
            }
            AssetLoadError::Parse { path, message } => {
                write!(f, "Parse error in '{path}': {message}")
            }
            AssetLoadError::UnsupportedFormat { extension } => {
                write!(f, "Unsupported file extension: '.{extension}'")
            }
            AssetLoadError::InvalidData { message } => {
                write!(f, "Invalid asset data: {message}")
            }
            AssetLoadError::DependencyFailed { dependency, message } => {
                write!(f, "Dependency '{dependency}' failed: {message}")
            }
            AssetLoadError::NoLoader => write!(f, "No loader registered for this asset type"),
            AssetLoadError::Other(s) => write!(f, "Asset load error: {s}"),
        }
    }
}

impl std::error::Error for AssetLoadError {}

// ─────────────────────────────────────────────
//  LoadedAsset
// ─────────────────────────────────────────────

/// The result of a successful load: the asset value plus any declared dependencies.
pub struct LoadedAsset<T> {
    /// The deserialized asset.
    pub asset: T,
    /// Paths of other assets that this asset depends on.
    pub dependencies: Vec<AssetPath>,
}

impl<T> LoadedAsset<T> {
    /// Create a loaded asset with no dependencies.
    pub fn new(asset: T) -> Self {
        Self { asset, dependencies: Vec::new() }
    }

    /// Create a loaded asset with explicit dependencies.
    pub fn with_dependencies(asset: T, dependencies: Vec<AssetPath>) -> Self {
        Self { asset, dependencies }
    }

    /// Add a dependency path.
    pub fn add_dependency(&mut self, path: impl Into<AssetPath>) {
        self.dependencies.push(path.into());
    }
}

// ─────────────────────────────────────────────
//  LoadContext
// ─────────────────────────────────────────────

/// Context passed to loaders allowing them to register sub-assets and dependencies.
///
/// A loader may produce multiple assets from one file (e.g. an atlas with named
/// frames). Each sub-asset is registered with a label and stored separately under
/// the path `"source_file#label"`.
pub struct LoadContext<'a> {
    /// The original file path being loaded.
    pub path: &'a str,
    /// Sub-assets registered by the loader.
    labeled: Vec<(String, Box<dyn Any + Send + Sync>)>,
    /// Dependency paths declared by the loader.
    dependencies: Vec<AssetPath>,
}

impl<'a> LoadContext<'a> {
    /// Create a new context for the given path.
    pub fn new(path: &'a str) -> Self {
        Self { path, labeled: Vec::new(), dependencies: Vec::new() }
    }

    /// Register a named sub-asset. The label is used in `AssetPath` with `#label`.
    pub fn add_labeled_asset<T: Any + Send + Sync>(&mut self, label: impl Into<String>, asset: T) {
        self.labeled.push((label.into(), Box::new(asset)));
    }

    /// Declare that this asset depends on another path.
    pub fn add_dependency(&mut self, path: impl Into<AssetPath>) {
        self.dependencies.push(path.into());
    }

    /// Drain all labeled sub-assets.
    pub fn take_labeled(&mut self) -> Vec<(String, Box<dyn Any + Send + Sync>)> {
        std::mem::take(&mut self.labeled)
    }

    /// Drain all declared dependencies.
    pub fn take_dependencies(&mut self) -> Vec<AssetPath> {
        std::mem::take(&mut self.dependencies)
    }

    /// The file extension of the path being loaded.
    pub fn extension(&self) -> Option<&str> {
        self.path.rsplit('.').next().filter(|e| !e.contains('/'))
    }
}

// ─────────────────────────────────────────────
//  AssetLoader trait
// ─────────────────────────────────────────────

/// Trait implemented by each file-format loader.
///
/// Loaders are stateless — they don't hold the loaded data themselves. The
/// server stores loaders in a registry keyed by file extension.
pub trait AssetLoader: Send + Sync + 'static {
    /// The file extensions this loader handles, e.g. `&["txt", "text"]`.
    fn extensions(&self) -> &[&str];

    /// Deserialize raw bytes into a boxed asset value.
    ///
    /// The `path` argument is provided for error messages only; the loader
    /// must not perform additional file I/O through it.
    fn load_bytes(
        &self,
        bytes: &[u8],
        path: &str,
        ctx: &mut LoadContext<'_>,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError>;

    /// Human-readable name of this loader (used in diagnostics).
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

// ─────────────────────────────────────────────
//  Asset trait
// ─────────────────────────────────────────────

/// Marker trait for asset types. Implementing this makes a type eligible to be
/// stored and retrieved through the asset system.
///
/// The associated `Loader` type provides the default loader for this asset; the
/// server can override it at runtime.
pub trait Asset: Any + Send + Sync + 'static {
    type Loader: AssetLoader + Default;
}

// ─────────────────────────────────────────────
//  AssetProcessor trait
// ─────────────────────────────────────────────

/// Optional post-load processing step. Processors run after the loader succeeds
/// and can mutate or validate the asset before it is placed into storage.
pub trait AssetProcessor: Send + Sync + 'static {
    /// The asset type this processor handles.
    type Asset: Asset;

    /// Run the processor. Return `Err` to cause the asset to be marked as failed.
    fn process(&self, asset: &mut Self::Asset) -> Result<(), AssetLoadError>;

    /// Human-readable name of this processor.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

// ─────────────────────────────────────────────
//  TextAsset
// ─────────────────────────────────────────────

/// A plain UTF-8 text file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAsset {
    pub text: String,
    /// Original path the asset was loaded from (informational).
    pub source_path: String,
}

impl TextAsset {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), source_path: String::new() }
    }

    /// Returns the number of lines in the text.
    pub fn line_count(&self) -> usize {
        self.text.lines().count()
    }

    /// Returns a specific line (0-indexed), if in range.
    pub fn line(&self, index: usize) -> Option<&str> {
        self.text.lines().nth(index)
    }
}

/// Loader for `TextAsset`. Handles `.txt` and `.text` files.
#[derive(Default)]
pub struct TextAssetLoader;

impl AssetLoader for TextAssetLoader {
    fn extensions(&self) -> &[&str] {
        &["txt", "text", "md", "log"]
    }

    fn load_bytes(
        &self,
        bytes: &[u8],
        path: &str,
        _ctx: &mut LoadContext<'_>,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError> {
        let text = std::str::from_utf8(bytes).map_err(|e| AssetLoadError::Parse {
            path: path.to_string(),
            message: format!("invalid UTF-8: {e}"),
        })?;
        Ok(Box::new(TextAsset {
            text: text.to_string(),
            source_path: path.to_string(),
        }))
    }
}

impl Asset for TextAsset {
    type Loader = TextAssetLoader;
}

// ─────────────────────────────────────────────
//  BytesAsset
// ─────────────────────────────────────────────

/// A raw byte buffer — the bytes are stored exactly as read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesAsset {
    pub bytes: Vec<u8>,
    pub source_path: String,
}

impl BytesAsset {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, source_path: String::new() }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Interpret the bytes as a UTF-8 string slice, if valid.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
}

/// Loader for `BytesAsset`. Handles any extension not claimed by a more specific loader.
#[derive(Default)]
pub struct BytesAssetLoader;

impl AssetLoader for BytesAssetLoader {
    fn extensions(&self) -> &[&str] {
        &["bin", "dat", "raw", "bytes"]
    }

    fn load_bytes(
        &self,
        bytes: &[u8],
        path: &str,
        _ctx: &mut LoadContext<'_>,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError> {
        Ok(Box::new(BytesAsset {
            bytes: bytes.to_vec(),
            source_path: path.to_string(),
        }))
    }
}

impl Asset for BytesAsset {
    type Loader = BytesAssetLoader;
}

// ─────────────────────────────────────────────
//  TomlAsset
// ─────────────────────────────────────────────

/// A parsed TOML document. Useful for configuration files.
#[derive(Debug, Clone)]
pub struct TomlAsset {
    pub value: toml::Value,
    pub source_path: String,
}

impl TomlAsset {
    pub fn new(value: toml::Value) -> Self {
        Self { value, source_path: String::new() }
    }

    /// Get a nested value by dot-separated key, e.g. `"display.width"`.
    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        let mut current = &self.value;
        for part in key.split('.') {
            current = current.get(part)?;
        }
        Some(current)
    }

    /// Get a string value by dot-separated key.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// Get an integer value by dot-separated key.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_integer()
    }

    /// Get a float value by dot-separated key.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_float()
    }

    /// Get a boolean value by dot-separated key.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }
}

/// Loader for `TomlAsset`.
#[derive(Default)]
pub struct TomlAssetLoader;

impl AssetLoader for TomlAssetLoader {
    fn extensions(&self) -> &[&str] {
        &["toml"]
    }

    fn load_bytes(
        &self,
        bytes: &[u8],
        path: &str,
        _ctx: &mut LoadContext<'_>,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError> {
        let text = std::str::from_utf8(bytes).map_err(|e| AssetLoadError::Parse {
            path: path.to_string(),
            message: format!("invalid UTF-8: {e}"),
        })?;
        let value = text.parse::<toml::Value>().map_err(|e| AssetLoadError::Parse {
            path: path.to_string(),
            message: format!("TOML parse error: {e}"),
        })?;
        Ok(Box::new(TomlAsset { value, source_path: path.to_string() }))
    }
}

impl Asset for TomlAsset {
    type Loader = TomlAssetLoader;
}

// ─────────────────────────────────────────────
//  ScriptAsset
// ─────────────────────────────────────────────

/// A script source file (Lua, custom scripting language, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptAsset {
    pub source: String,
    pub language: ScriptLanguage,
    pub source_path: String,
}

/// The scripting language of a `ScriptAsset`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptLanguage {
    Lua,
    Custom(String),
    Unknown,
}

impl ScriptAsset {
    pub fn new(source: impl Into<String>, language: ScriptLanguage) -> Self {
        Self { source: source.into(), language, source_path: String::new() }
    }

    pub fn line_count(&self) -> usize {
        self.source.lines().count()
    }

    /// Returns the first line of the source (often a shebang or comment header).
    pub fn header_line(&self) -> Option<&str> {
        self.source.lines().next()
    }
}

/// Loader for `ScriptAsset`.
#[derive(Default)]
pub struct ScriptAssetLoader;

impl AssetLoader for ScriptAssetLoader {
    fn extensions(&self) -> &[&str] {
        &["lua", "script", "scr", "gs"]
    }

    fn load_bytes(
        &self,
        bytes: &[u8],
        path: &str,
        _ctx: &mut LoadContext<'_>,
    ) -> Result<Box<dyn Any + Send + Sync>, AssetLoadError> {
        let source = std::str::from_utf8(bytes).map_err(|e| AssetLoadError::Parse {
            path: path.to_string(),
            message: format!("invalid UTF-8 in script: {e}"),
        })?;
        let language = if path.ends_with(".lua") {
            ScriptLanguage::Lua
        } else {
            ScriptLanguage::Unknown
        };
        Ok(Box::new(ScriptAsset {
            source: source.to_string(),
            language,
            source_path: path.to_string(),
        }))
    }
}

impl Asset for ScriptAsset {
    type Loader = ScriptAssetLoader;
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(path: &str) -> LoadContext<'_> {
        LoadContext::new(path)
    }

    #[test]
    fn text_asset_loader_valid_utf8() {
        let loader = TextAssetLoader;
        let bytes = b"Hello, world!";
        let mut ctx = make_ctx("test.txt");
        let result = loader.load_bytes(bytes, "test.txt", &mut ctx);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let asset = boxed.downcast_ref::<TextAsset>().unwrap();
        assert_eq!(asset.text, "Hello, world!");
    }

    #[test]
    fn text_asset_loader_invalid_utf8() {
        let loader = TextAssetLoader;
        let bytes = &[0xFF, 0xFE, 0x00];
        let mut ctx = make_ctx("bad.txt");
        let result = loader.load_bytes(bytes, "bad.txt", &mut ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AssetLoadError::Parse { .. }));
    }

    #[test]
    fn bytes_asset_loader_raw() {
        let loader = BytesAssetLoader;
        let bytes = &[1u8, 2, 3, 4, 5];
        let mut ctx = make_ctx("data.bin");
        let result = loader.load_bytes(bytes, "data.bin", &mut ctx);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let asset = boxed.downcast_ref::<BytesAsset>().unwrap();
        assert_eq!(asset.bytes, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn toml_asset_loader_parse() {
        let loader = TomlAssetLoader;
        let toml_src = b"[window]\nwidth = 1920\nheight = 1080\n";
        let mut ctx = make_ctx("config.toml");
        let result = loader.load_bytes(toml_src, "config.toml", &mut ctx);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let asset = boxed.downcast_ref::<TomlAsset>().unwrap();
        assert_eq!(asset.get_int("window.width"), Some(1920));
        assert_eq!(asset.get_int("window.height"), Some(1080));
    }

    #[test]
    fn script_asset_loader_lua() {
        let loader = ScriptAssetLoader;
        let src = b"-- hello from lua\nprint('hi')";
        let mut ctx = make_ctx("init.lua");
        let result = loader.load_bytes(src, "init.lua", &mut ctx);
        assert!(result.is_ok());
        let boxed = result.unwrap();
        let asset = boxed.downcast_ref::<ScriptAsset>().unwrap();
        assert_eq!(asset.language, ScriptLanguage::Lua);
        assert_eq!(asset.line_count(), 2);
    }

    #[test]
    fn load_context_sub_assets() {
        let mut ctx = LoadContext::new("atlas.png");
        ctx.add_labeled_asset("frame_0", TextAsset::new("frame data"));
        ctx.add_dependency(AssetPath::new("palette.toml"));
        let labeled = ctx.take_labeled();
        let deps = ctx.take_dependencies();
        assert_eq!(labeled.len(), 1);
        assert_eq!(labeled[0].0, "frame_0");
        assert_eq!(deps.len(), 1);
    }

    #[test]
    fn text_asset_line_helpers() {
        let asset = TextAsset::new("line one\nline two\nline three");
        assert_eq!(asset.line_count(), 3);
        assert_eq!(asset.line(1), Some("line two"));
        assert_eq!(asset.line(99), None);
    }

    #[test]
    fn asset_loader_extensions() {
        assert!(TextAssetLoader.extensions().contains(&"txt"));
        assert!(TomlAssetLoader.extensions().contains(&"toml"));
        assert!(ScriptAssetLoader.extensions().contains(&"lua"));
        assert!(BytesAssetLoader.extensions().contains(&"bin"));
    }
}
