
//! Build system editor — platform targets, build configurations, asset pipeline,
//! packaging, code signing, deployment, CI integration, and build log viewer.

#![allow(non_camel_case_types)]

use glam::Vec4;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Platform targets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildPlatform {
    Windows,
    WindowsArm64,
    MacOS,
    MacOSArm64,
    Linux,
    LinuxArm64,
    Android,
    AndroidX86,
    iOS,
    WebGL,
    WebGPU,
    XboxSeriesX,
    XboxOne,
    PlayStation5,
    PlayStation4,
    SwitchNX,
    TVos,
    VisionOS,
    WasmModule,
    EmbeddedLinux,
}

impl BuildPlatform {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Windows => "Windows (x64)",
            Self::WindowsArm64 => "Windows (ARM64)",
            Self::MacOS => "macOS (x64)",
            Self::MacOSArm64 => "macOS (Apple Silicon)",
            Self::Linux => "Linux (x64)",
            Self::LinuxArm64 => "Linux (ARM64)",
            Self::Android => "Android (ARM64)",
            Self::AndroidX86 => "Android (x86_64)",
            Self::iOS => "iOS",
            Self::WebGL => "WebGL",
            Self::WebGPU => "WebGPU",
            Self::XboxSeriesX => "Xbox Series X/S",
            Self::XboxOne => "Xbox One",
            Self::PlayStation5 => "PlayStation 5",
            Self::PlayStation4 => "PlayStation 4",
            Self::SwitchNX => "Nintendo Switch",
            Self::TVos => "tvOS",
            Self::VisionOS => "visionOS",
            Self::WasmModule => "WASM Module",
            Self::EmbeddedLinux => "Embedded Linux",
        }
    }

    pub fn is_mobile(&self) -> bool {
        matches!(self, Self::Android | Self::AndroidX86 | Self::iOS | Self::TVos)
    }
    pub fn is_console(&self) -> bool {
        matches!(self, Self::XboxSeriesX | Self::XboxOne | Self::PlayStation5 | Self::PlayStation4 | Self::SwitchNX)
    }
    pub fn is_web(&self) -> bool {
        matches!(self, Self::WebGL | Self::WebGPU | Self::WasmModule)
    }
    pub fn is_desktop(&self) -> bool {
        matches!(self, Self::Windows | Self::WindowsArm64 | Self::MacOS | Self::MacOSArm64 | Self::Linux | Self::LinuxArm64)
    }
    pub fn supports_il2cpp(&self) -> bool {
        !self.is_web()
    }
    pub fn requires_signing(&self) -> bool {
        matches!(self, Self::iOS | Self::MacOS | Self::MacOSArm64 | Self::TVos | Self::VisionOS | Self::Android)
    }
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Windows | Self::WindowsArm64 => "exe",
            Self::MacOS | Self::MacOSArm64 => "app",
            Self::Linux | Self::LinuxArm64 | Self::EmbeddedLinux => "",
            Self::Android | Self::AndroidX86 => "apk",
            Self::iOS | Self::TVos | Self::VisionOS => "ipa",
            Self::WebGL | Self::WebGPU | Self::WasmModule => "html",
            _ => "pkg",
        }
    }
    pub fn min_sdk_version(&self) -> Option<u32> {
        match self {
            Self::Android | Self::AndroidX86 => Some(21),
            Self::iOS => Some(14),
            _ => None,
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Windows | Self::WindowsArm64 => "🪟",
            Self::MacOS | Self::MacOSArm64 | Self::TVos | Self::VisionOS => "🍎",
            Self::Linux | Self::LinuxArm64 => "🐧",
            Self::Android | Self::AndroidX86 => "🤖",
            Self::iOS => "📱",
            Self::WebGL | Self::WebGPU | Self::WasmModule => "🌐",
            Self::XboxSeriesX | Self::XboxOne => "🎮",
            Self::PlayStation5 | Self::PlayStation4 => "🎮",
            Self::SwitchNX => "🎮",
            _ => "📦",
        }
    }
}

// ---------------------------------------------------------------------------
// Build configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildConfiguration {
    Debug,
    DebugWithOptimizations,
    Development,
    Release,
    ReleaseFinal,
    Profiling,
    Shipping,
    QA,
}

impl BuildConfiguration {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::DebugWithOptimizations => "Debug (Optimized)",
            Self::Development => "Development",
            Self::Release => "Release",
            Self::ReleaseFinal => "Release Final",
            Self::Profiling => "Profiling",
            Self::Shipping => "Shipping",
            Self::QA => "QA",
        }
    }
    pub fn optimization_level(&self) -> u8 {
        match self {
            Self::Debug => 0,
            Self::DebugWithOptimizations => 1,
            Self::Development => 2,
            Self::Release | Self::Profiling | Self::QA => 3,
            Self::ReleaseFinal | Self::Shipping => 3,
        }
    }
    pub fn includes_debug_info(&self) -> bool {
        matches!(self, Self::Debug | Self::DebugWithOptimizations | Self::Development | Self::Profiling | Self::QA)
    }
    pub fn includes_assertions(&self) -> bool {
        matches!(self, Self::Debug | Self::DebugWithOptimizations | Self::Development | Self::QA)
    }
    pub fn strip_logs(&self) -> bool {
        matches!(self, Self::Shipping | Self::ReleaseFinal)
    }
    pub fn enables_cheats(&self) -> bool {
        matches!(self, Self::Debug | Self::DebugWithOptimizations | Self::Development | Self::QA)
    }
    pub fn enable_profiler(&self) -> bool {
        matches!(self, Self::Profiling | Self::Development | Self::Debug)
    }
    pub fn defines(&self) -> Vec<&'static str> {
        let mut defs = vec![];
        if self.includes_debug_info() { defs.push("DEBUG"); }
        if self.includes_assertions() { defs.push("ASSERTIONS_ENABLED"); }
        if self.strip_logs() { defs.push("STRIP_LOGS"); }
        if self.enables_cheats() { defs.push("CHEATS_ENABLED"); }
        if self.enable_profiler() { defs.push("PROFILER_ENABLED"); }
        match self {
            Self::Shipping | Self::ReleaseFinal => defs.push("FINAL_BUILD"),
            Self::QA => defs.push("QA_BUILD"),
            _ => {}
        }
        defs
    }
}

// ---------------------------------------------------------------------------
// Compiler / scripting backend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptingBackend { Mono, IL2CPP, NativeAOT, WASM }

impl ScriptingBackend {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mono => "Mono",
            Self::IL2CPP => "IL2CPP",
            Self::NativeAOT => "Native AOT",
            Self::WASM => "WebAssembly",
        }
    }
    pub fn supports_hot_reload(&self) -> bool {
        matches!(self, Self::Mono)
    }
    pub fn compile_time_estimate_min(&self) -> u32 {
        match self {
            Self::Mono => 1,
            Self::IL2CPP => 8,
            Self::NativeAOT => 6,
            Self::WASM => 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Asset processing pipeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetProcessorKind {
    TextureCompressor,
    AudioTranscoder,
    MeshOptimizer,
    ShaderCompiler,
    FontPacker,
    VideoEncoder,
    LocalizationPacker,
    AnimationCompressor,
    PhysicsColliderBuilder,
    NavMeshBaker,
    LightmapBaker,
    SdfGenerator,
    AtlasPacker,
    PrefabSerializer,
    SceneSerializer,
    MaterialBaker,
    LutBaker,
    EnvironmentProbeConvolver,
}

impl AssetProcessorKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TextureCompressor => "Texture Compressor",
            Self::AudioTranscoder => "Audio Transcoder",
            Self::MeshOptimizer => "Mesh Optimizer",
            Self::ShaderCompiler => "Shader Compiler",
            Self::FontPacker => "Font Packer",
            Self::VideoEncoder => "Video Encoder",
            Self::LocalizationPacker => "Localization Packer",
            Self::AnimationCompressor => "Animation Compressor",
            Self::PhysicsColliderBuilder => "Physics Collider Builder",
            Self::NavMeshBaker => "NavMesh Baker",
            Self::LightmapBaker => "Lightmap Baker",
            Self::SdfGenerator => "SDF Generator",
            Self::AtlasPacker => "Atlas Packer",
            Self::PrefabSerializer => "Prefab Serializer",
            Self::SceneSerializer => "Scene Serializer",
            Self::MaterialBaker => "Material Baker",
            Self::LutBaker => "LUT Baker",
            Self::EnvironmentProbeConvolver => "Env Probe Convolver",
        }
    }
    pub fn is_slow(&self) -> bool {
        matches!(self, Self::LightmapBaker | Self::VideoEncoder | Self::ShaderCompiler | Self::EnvironmentProbeConvolver)
    }
    pub fn can_parallelize(&self) -> bool {
        !matches!(self, Self::AtlasPacker | Self::NavMeshBaker)
    }
    pub fn estimated_time_secs(&self, asset_count: usize) -> f32 {
        let per_asset = match self {
            Self::TextureCompressor => 0.5,
            Self::AudioTranscoder => 0.3,
            Self::MeshOptimizer => 0.2,
            Self::ShaderCompiler => 2.0,
            Self::LightmapBaker => 60.0,
            Self::VideoEncoder => 30.0,
            Self::NavMeshBaker => 10.0,
            _ => 0.1,
        };
        per_asset * asset_count as f32
    }
}

#[derive(Debug, Clone)]
pub struct AssetProcessorSettings {
    pub kind: AssetProcessorKind,
    pub enabled: bool,
    pub worker_threads: u32,
    pub cache_enabled: bool,
    pub incremental: bool,
    pub platform_overrides: HashMap<BuildPlatform, HashMap<String, String>>,
    pub params: HashMap<String, String>,
}

impl AssetProcessorSettings {
    pub fn new(kind: AssetProcessorKind) -> Self {
        Self {
            kind,
            enabled: true,
            worker_threads: 4,
            cache_enabled: true,
            incremental: true,
            platform_overrides: HashMap::new(),
            params: HashMap::new(),
        }
    }
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }
}

// ---------------------------------------------------------------------------
// Texture compression formats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureCompressionFormat {
    Uncompressed,
    DXT1,
    DXT5,
    BC7,
    BC6H,
    ETC2,
    ASTC4x4,
    ASTC6x6,
    ASTC8x8,
    PVRTC,
    Crunch,
    Basis,
}

impl TextureCompressionFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Uncompressed => "Uncompressed",
            Self::DXT1 => "DXT1 (BC1)",
            Self::DXT5 => "DXT5 (BC3)",
            Self::BC7 => "BC7 (high quality)",
            Self::BC6H => "BC6H (HDR)",
            Self::ETC2 => "ETC2",
            Self::ASTC4x4 => "ASTC 4x4",
            Self::ASTC6x6 => "ASTC 6x6",
            Self::ASTC8x8 => "ASTC 8x8",
            Self::PVRTC => "PVRTC",
            Self::Crunch => "Crunch",
            Self::Basis => "Basis Universal",
        }
    }
    pub fn compression_ratio(&self) -> f32 {
        match self {
            Self::Uncompressed => 1.0,
            Self::DXT1 => 6.0,
            Self::DXT5 => 4.0,
            Self::BC7 => 4.0,
            Self::BC6H => 4.0,
            Self::ETC2 => 4.0,
            Self::ASTC4x4 => 8.0,
            Self::ASTC6x6 => 18.0,
            Self::ASTC8x8 => 32.0,
            Self::PVRTC => 4.0,
            Self::Crunch => 20.0,
            Self::Basis => 25.0,
        }
    }
    pub fn supported_platforms(&self) -> Vec<BuildPlatform> {
        match self {
            Self::DXT1 | Self::DXT5 | Self::BC7 | Self::BC6H => vec![BuildPlatform::Windows, BuildPlatform::WindowsArm64, BuildPlatform::Linux, BuildPlatform::MacOS],
            Self::ETC2 => vec![BuildPlatform::Android, BuildPlatform::AndroidX86],
            Self::ASTC4x4 | Self::ASTC6x6 | Self::ASTC8x8 => vec![BuildPlatform::iOS, BuildPlatform::Android],
            Self::PVRTC => vec![BuildPlatform::iOS],
            Self::Basis | Self::Crunch => vec![BuildPlatform::Windows, BuildPlatform::Linux, BuildPlatform::MacOS, BuildPlatform::Android, BuildPlatform::iOS, BuildPlatform::WebGL],
            Self::Uncompressed => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Build step
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildStepKind {
    CleanOutputDir,
    CompileScripts,
    CompileShaders,
    ProcessAssets,
    GenerateAssetManifest,
    LinkBinaries,
    CopyNativeLibraries,
    PackageData,
    SignBinaries,
    CompressOutput,
    UploadSymbols,
    RunTests,
    GenerateDocumentation,
    NotifyCISystem,
    ArchiveBuild,
    DeployToStore,
    RunSmokeTest,
    BumpVersion,
}

impl BuildStepKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CleanOutputDir => "Clean Output Directory",
            Self::CompileScripts => "Compile Scripts",
            Self::CompileShaders => "Compile Shaders",
            Self::ProcessAssets => "Process Assets",
            Self::GenerateAssetManifest => "Generate Asset Manifest",
            Self::LinkBinaries => "Link Binaries",
            Self::CopyNativeLibraries => "Copy Native Libraries",
            Self::PackageData => "Package Data",
            Self::SignBinaries => "Sign Binaries",
            Self::CompressOutput => "Compress Output",
            Self::UploadSymbols => "Upload Debug Symbols",
            Self::RunTests => "Run Tests",
            Self::GenerateDocumentation => "Generate Documentation",
            Self::NotifyCISystem => "Notify CI System",
            Self::ArchiveBuild => "Archive Build",
            Self::DeployToStore => "Deploy to Store",
            Self::RunSmokeTest => "Run Smoke Test",
            Self::BumpVersion => "Bump Version",
        }
    }
    pub fn can_skip(&self) -> bool {
        matches!(self, Self::RunTests | Self::GenerateDocumentation | Self::UploadSymbols | Self::RunSmokeTest)
    }
    pub fn requires_internet(&self) -> bool {
        matches!(self, Self::UploadSymbols | Self::NotifyCISystem | Self::DeployToStore)
    }
    pub fn estimated_duration_secs(&self) -> f32 {
        match self {
            Self::CleanOutputDir => 2.0,
            Self::CompileScripts => 30.0,
            Self::CompileShaders => 120.0,
            Self::ProcessAssets => 180.0,
            Self::GenerateAssetManifest => 5.0,
            Self::LinkBinaries => 15.0,
            Self::CopyNativeLibraries => 3.0,
            Self::PackageData => 60.0,
            Self::SignBinaries => 10.0,
            Self::CompressOutput => 20.0,
            Self::UploadSymbols => 30.0,
            Self::RunTests => 300.0,
            Self::GenerateDocumentation => 60.0,
            Self::NotifyCISystem => 2.0,
            Self::ArchiveBuild => 30.0,
            Self::DeployToStore => 600.0,
            Self::RunSmokeTest => 120.0,
            Self::BumpVersion => 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildStep {
    pub kind: BuildStepKind,
    pub enabled: bool,
    pub skip_on_error: bool,
    pub run_on_failure: bool,
    pub custom_command: Option<String>,
    pub environment_vars: HashMap<String, String>,
    pub timeout_secs: Option<u32>,
}

impl BuildStep {
    pub fn new(kind: BuildStepKind) -> Self {
        Self {
            kind,
            enabled: true,
            skip_on_error: false,
            run_on_failure: false,
            custom_command: None,
            environment_vars: HashMap::new(),
            timeout_secs: None,
        }
    }
    pub fn optional(mut self) -> Self { self.skip_on_error = true; self }
    pub fn with_timeout(mut self, secs: u32) -> Self { self.timeout_secs = Some(secs); self }
    pub fn with_command(mut self, cmd: &str) -> Self { self.custom_command = Some(cmd.to_string()); self }
}

// ---------------------------------------------------------------------------
// Build target
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BuildTarget {
    pub name: String,
    pub platform: BuildPlatform,
    pub configuration: BuildConfiguration,
    pub scripting_backend: ScriptingBackend,
    pub architecture: Architecture,
    pub output_path: String,
    pub steps: Vec<BuildStep>,
    pub asset_processors: Vec<AssetProcessorSettings>,
    pub texture_compression: TextureCompressionFormat,
    pub defines: Vec<String>,
    pub excluded_scenes: Vec<String>,
    pub included_scenes: Vec<String>,
    pub compression: PackageCompression,
    pub split_application_binary: bool,
    pub development_build: bool,
    pub connect_profiler: bool,
    pub allow_debugging: bool,
    pub il2cpp_optimization: IL2CPPOptimization,
    pub strip_engine_code: bool,
    pub strip_unused_code: bool,
    pub managed_stripping_level: StrippingLevel,
    pub android_settings: Option<AndroidBuildSettings>,
    pub ios_settings: Option<IosBuildSettings>,
    pub signing_settings: Option<SigningSettings>,
    pub custom_build_script: Option<String>,
    pub post_build_script: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Architecture { X64, ARM64, X86, Universal, ARMv7 }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageCompression { None, LZ4, LZ4HC, LZMA, Zstd }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IL2CPPOptimization { None, Size, Speed }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrippingLevel { Disabled, Minimal, Medium, High, Experimental }

impl Architecture {
    pub fn label(&self) -> &'static str {
        match self {
            Self::X64 => "x86_64", Self::ARM64 => "ARM64", Self::X86 => "x86",
            Self::Universal => "Universal", Self::ARMv7 => "ARMv7",
        }
    }
}

impl PackageCompression {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None", Self::LZ4 => "LZ4", Self::LZ4HC => "LZ4HC (Slow)",
            Self::LZMA => "LZMA", Self::Zstd => "Zstd",
        }
    }
    pub fn ratio_estimate(&self) -> f32 {
        match self {
            Self::None => 1.0, Self::LZ4 => 1.4, Self::LZ4HC => 1.6,
            Self::LZMA => 2.2, Self::Zstd => 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AndroidBuildSettings {
    pub package_name: String,
    pub version_code: u32,
    pub version_name: String,
    pub min_sdk: u32,
    pub target_sdk: u32,
    pub keystore_path: String,
    pub keystore_alias: String,
    pub build_apk: bool,
    pub build_aab: bool,
    pub split_apks: bool,
    pub target_architectures: Vec<Architecture>,
    pub install_location: AndroidInstallLocation,
    pub internet_access: bool,
    pub vibration: bool,
    pub accelerometer: bool,
    pub enable_arm_neon: bool,
    pub il2cpp_target_api_level: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AndroidInstallLocation { Auto, InternalOnly, PreferExternal }

impl Default for AndroidBuildSettings {
    fn default() -> Self {
        Self {
            package_name: "com.example.game".to_string(),
            version_code: 1,
            version_name: "1.0.0".to_string(),
            min_sdk: 21,
            target_sdk: 33,
            keystore_path: String::new(),
            keystore_alias: String::new(),
            build_apk: true,
            build_aab: false,
            split_apks: false,
            target_architectures: vec![Architecture::ARM64],
            install_location: AndroidInstallLocation::Auto,
            internet_access: true,
            vibration: true,
            accelerometer: false,
            enable_arm_neon: true,
            il2cpp_target_api_level: 29,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IosBuildSettings {
    pub bundle_id: String,
    pub short_version: String,
    pub bundle_version: String,
    pub target_device: IosTargetDevice,
    pub target_ios_version: String,
    pub uses_location: bool,
    pub uses_camera: bool,
    pub uses_microphone: bool,
    pub uses_motion_sensors: bool,
    pub hdr_display_mode: bool,
    pub requires_full_screen: bool,
    pub application_category: IosCategory,
    pub bitcode_enabled: bool,
    pub apple_developer_team: String,
    pub provisioning_profile: String,
    pub metal_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IosTargetDevice { iPhone, iPad, iPhoneAndiPad }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IosCategory { Games, Entertainment, Utilities, Education, Other }

impl Default for IosBuildSettings {
    fn default() -> Self {
        Self {
            bundle_id: "com.example.game".to_string(),
            short_version: "1.0".to_string(),
            bundle_version: "1".to_string(),
            target_device: IosTargetDevice::iPhoneAndiPad,
            target_ios_version: "14.0".to_string(),
            uses_location: false,
            uses_camera: false,
            uses_microphone: false,
            uses_motion_sensors: false,
            hdr_display_mode: false,
            requires_full_screen: true,
            application_category: IosCategory::Games,
            bitcode_enabled: false,
            apple_developer_team: String::new(),
            provisioning_profile: String::new(),
            metal_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SigningSettings {
    pub certificate_path: String,
    pub certificate_password: String,
    pub notarize: bool,
    pub staple: bool,
    pub entitlements_path: String,
    pub codesign_identity: String,
    pub timestamp_server: String,
}

impl BuildTarget {
    pub fn windows_release() -> Self {
        Self {
            name: "Windows Release".to_string(),
            platform: BuildPlatform::Windows,
            configuration: BuildConfiguration::Release,
            scripting_backend: ScriptingBackend::NativeAOT,
            architecture: Architecture::X64,
            output_path: "build/windows".to_string(),
            steps: vec![
                BuildStep::new(BuildStepKind::CleanOutputDir),
                BuildStep::new(BuildStepKind::CompileShaders),
                BuildStep::new(BuildStepKind::CompileScripts),
                BuildStep::new(BuildStepKind::ProcessAssets),
                BuildStep::new(BuildStepKind::GenerateAssetManifest),
                BuildStep::new(BuildStepKind::LinkBinaries),
                BuildStep::new(BuildStepKind::PackageData),
                BuildStep::new(BuildStepKind::CompressOutput),
                BuildStep::new(BuildStepKind::RunTests).optional(),
                BuildStep::new(BuildStepKind::ArchiveBuild),
            ],
            asset_processors: vec![
                AssetProcessorSettings::new(AssetProcessorKind::TextureCompressor).with_param("format", "BC7"),
                AssetProcessorSettings::new(AssetProcessorKind::ShaderCompiler).with_param("api", "D3D12"),
                AssetProcessorSettings::new(AssetProcessorKind::AudioTranscoder).with_param("format", "OGG"),
                AssetProcessorSettings::new(AssetProcessorKind::MeshOptimizer),
            ],
            texture_compression: TextureCompressionFormat::BC7,
            defines: vec!["PLATFORM_WINDOWS".to_string()],
            excluded_scenes: Vec::new(),
            included_scenes: Vec::new(),
            compression: PackageCompression::LZ4HC,
            split_application_binary: false,
            development_build: false,
            connect_profiler: false,
            allow_debugging: false,
            il2cpp_optimization: IL2CPPOptimization::Speed,
            strip_engine_code: true,
            strip_unused_code: true,
            managed_stripping_level: StrippingLevel::Medium,
            android_settings: None,
            ios_settings: None,
            signing_settings: None,
            custom_build_script: None,
            post_build_script: None,
            enabled: true,
        }
    }

    pub fn android_release() -> Self {
        let mut t = Self::windows_release();
        t.name = "Android Release".to_string();
        t.platform = BuildPlatform::Android;
        t.output_path = "build/android".to_string();
        t.texture_compression = TextureCompressionFormat::ASTC4x4;
        t.scripting_backend = ScriptingBackend::IL2CPP;
        t.android_settings = Some(AndroidBuildSettings::default());
        t.defines = vec!["PLATFORM_ANDROID".to_string()];
        t.asset_processors = vec![
            AssetProcessorSettings::new(AssetProcessorKind::TextureCompressor).with_param("format", "ASTC"),
            AssetProcessorSettings::new(AssetProcessorKind::ShaderCompiler).with_param("api", "Vulkan"),
            AssetProcessorSettings::new(AssetProcessorKind::AudioTranscoder).with_param("format", "OGG"),
        ];
        t
    }

    pub fn web_gl() -> Self {
        let mut t = Self::windows_release();
        t.name = "WebGL".to_string();
        t.platform = BuildPlatform::WebGL;
        t.output_path = "build/webgl".to_string();
        t.scripting_backend = ScriptingBackend::WASM;
        t.texture_compression = TextureCompressionFormat::Basis;
        t.compression = PackageCompression::Zstd;
        t.defines = vec!["PLATFORM_WEBGL".to_string()];
        t.asset_processors = vec![
            AssetProcessorSettings::new(AssetProcessorKind::TextureCompressor).with_param("format", "Basis"),
            AssetProcessorSettings::new(AssetProcessorKind::ShaderCompiler).with_param("api", "WebGL2"),
        ];
        t
    }

    pub fn total_estimated_build_time_secs(&self) -> f32 {
        self.steps.iter()
            .filter(|s| s.enabled)
            .map(|s| s.kind.estimated_duration_secs())
            .sum()
    }

    pub fn step_count(&self) -> usize {
        self.steps.iter().filter(|s| s.enabled).count()
    }
}

// ---------------------------------------------------------------------------
// Build log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel { Trace, Debug, Info, Warning, Error, Fatal }

impl LogLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trace => "TRACE", Self::Debug => "DEBUG", Self::Info => "INFO",
            Self::Warning => "WARN", Self::Error => "ERROR", Self::Fatal => "FATAL",
        }
    }
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Trace => Vec4::new(0.5, 0.5, 0.5, 1.0),
            Self::Debug => Vec4::new(0.6, 0.8, 1.0, 1.0),
            Self::Info => Vec4::new(0.9, 0.9, 0.9, 1.0),
            Self::Warning => Vec4::new(1.0, 0.8, 0.0, 1.0),
            Self::Error => Vec4::new(1.0, 0.3, 0.3, 1.0),
            Self::Fatal => Vec4::new(1.0, 0.0, 0.5, 1.0),
        }
    }
    pub fn is_error(&self) -> bool { matches!(self, Self::Error | Self::Fatal) }
}

#[derive(Debug, Clone)]
pub struct BuildLogEntry {
    pub timestamp_ms: f64,
    pub level: LogLevel,
    pub step: Option<BuildStepKind>,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub asset_path: Option<String>,
    pub duration_ms: Option<f32>,
}

impl BuildLogEntry {
    pub fn info(msg: &str) -> Self {
        Self { timestamp_ms: 0.0, level: LogLevel::Info, step: None, message: msg.to_string(), file: None, line: None, asset_path: None, duration_ms: None }
    }
    pub fn warning(msg: &str) -> Self {
        Self { level: LogLevel::Warning, ..Self::info(msg) }
    }
    pub fn error(msg: &str) -> Self {
        Self { level: LogLevel::Error, ..Self::info(msg) }
    }
}

#[derive(Debug, Clone)]
pub struct BuildLog {
    pub entries: Vec<BuildLogEntry>,
    pub warnings: u32,
    pub errors: u32,
    pub start_time_ms: f64,
    pub end_time_ms: Option<f64>,
    pub success: Option<bool>,
}

impl BuildLog {
    pub fn new() -> Self {
        Self { entries: Vec::new(), warnings: 0, errors: 0, start_time_ms: 0.0, end_time_ms: None, success: None }
    }
    pub fn push(&mut self, entry: BuildLogEntry) {
        if entry.level == LogLevel::Warning { self.warnings += 1; }
        if entry.level.is_error() { self.errors += 1; }
        self.entries.push(entry);
    }
    pub fn duration_ms(&self) -> Option<f64> {
        self.end_time_ms.map(|e| e - self.start_time_ms)
    }
    pub fn duration_secs(&self) -> Option<f32> {
        self.duration_ms().map(|d| d as f32 / 1000.0)
    }
    pub fn filter_by_level(&self, level: LogLevel) -> Vec<&BuildLogEntry> {
        self.entries.iter().filter(|e| e.level == level).collect()
    }
    pub fn errors_and_warnings(&self) -> Vec<&BuildLogEntry> {
        self.entries.iter().filter(|e| e.level.is_error() || e.level == LogLevel::Warning).collect()
    }
    pub fn generate_summary(&self) -> String {
        let dur = self.duration_secs().map(|d| format!("{:.1}s", d)).unwrap_or_else(|| "N/A".to_string());
        let status = match self.success {
            Some(true) => "SUCCESS",
            Some(false) => "FAILED",
            None => "IN PROGRESS",
        };
        format!("Build {}: {} warnings, {} errors, duration: {}", status, self.warnings, self.errors, dur)
    }
}

impl Default for BuildLog {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Build run state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildState {
    Idle,
    Preparing,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl BuildState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Preparing => "Preparing",
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
    pub fn is_active(&self) -> bool { matches!(self, Self::Running | Self::Preparing) }
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Idle => Vec4::new(0.6, 0.6, 0.6, 1.0),
            Self::Preparing | Self::Running => Vec4::new(0.4, 0.8, 1.0, 1.0),
            Self::Paused => Vec4::new(1.0, 0.8, 0.2, 1.0),
            Self::Completed => Vec4::new(0.3, 0.9, 0.3, 1.0),
            Self::Failed => Vec4::new(1.0, 0.3, 0.3, 1.0),
            Self::Cancelled => Vec4::new(0.8, 0.5, 0.2, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Version info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build_number: u32,
    pub pre_release: String,
    pub git_commit: String,
    pub git_branch: String,
    pub build_date: String,
    pub channel: ReleaseChannel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReleaseChannel { Dev, Alpha, Beta, ReleaseCandidate, Stable, Hotfix }

impl ReleaseChannel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dev => "dev", Self::Alpha => "alpha", Self::Beta => "beta",
            Self::ReleaseCandidate => "rc", Self::Stable => "stable", Self::Hotfix => "hotfix",
        }
    }
}

impl VersionInfo {
    pub fn format_full(&self) -> String {
        let pre = if self.pre_release.is_empty() { String::new() } else { format!("-{}", self.pre_release) };
        format!("{}.{}.{}{} (build {})", self.major, self.minor, self.patch, pre, self.build_number)
    }
    pub fn format_short(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
    pub fn bump_patch(&mut self) { self.patch += 1; self.build_number += 1; }
    pub fn bump_minor(&mut self) { self.minor += 1; self.patch = 0; self.build_number += 1; }
    pub fn bump_major(&mut self) { self.major += 1; self.minor = 0; self.patch = 0; self.build_number += 1; }
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            major: 0, minor: 1, patch: 0, build_number: 1,
            pre_release: "alpha".to_string(),
            git_commit: "abcdef1".to_string(),
            git_branch: "main".to_string(),
            build_date: "2026-03-29".to_string(),
            channel: ReleaseChannel::Alpha,
        }
    }
}

// ---------------------------------------------------------------------------
// CI integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CIProvider { GitHub, GitLab, Jenkins, CircleCI, TeamCity, AzureDevOps, Buildkite, Bamboo, Custom }

impl CIProvider {
    pub fn label(&self) -> &'static str {
        match self {
            Self::GitHub => "GitHub Actions",
            Self::GitLab => "GitLab CI",
            Self::Jenkins => "Jenkins",
            Self::CircleCI => "CircleCI",
            Self::TeamCity => "TeamCity",
            Self::AzureDevOps => "Azure DevOps",
            Self::Buildkite => "Buildkite",
            Self::Bamboo => "Bamboo",
            Self::Custom => "Custom",
        }
    }
    pub fn webhook_format(&self) -> &'static str {
        match self {
            Self::GitHub => "json",
            Self::Jenkins => "text/plain",
            _ => "json",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CISettings {
    pub provider: CIProvider,
    pub webhook_url: String,
    pub api_token: String,
    pub trigger_on_push: bool,
    pub trigger_on_tag: bool,
    pub trigger_branches: Vec<String>,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notify_emails: Vec<String>,
    pub artifact_retention_days: u32,
    pub parallel_jobs: u32,
}

impl Default for CISettings {
    fn default() -> Self {
        Self {
            provider: CIProvider::GitHub,
            webhook_url: String::new(),
            api_token: String::new(),
            trigger_on_push: true,
            trigger_on_tag: true,
            trigger_branches: vec!["main".to_string(), "release/*".to_string()],
            notify_on_success: false,
            notify_on_failure: true,
            notify_emails: Vec::new(),
            artifact_retention_days: 30,
            parallel_jobs: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Build system editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildEditorPanel {
    Targets,
    AssetPipeline,
    Versioning,
    CIIntegration,
    Log,
    Artifacts,
    Settings,
}

impl BuildEditorPanel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Targets => "Build Targets",
            Self::AssetPipeline => "Asset Pipeline",
            Self::Versioning => "Versioning",
            Self::CIIntegration => "CI/CD",
            Self::Log => "Build Log",
            Self::Artifacts => "Artifacts",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildArtifact {
    pub name: String,
    pub platform: BuildPlatform,
    pub configuration: BuildConfiguration,
    pub version: String,
    pub file_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub path: String,
    pub created_at: String,
    pub sha256: String,
    pub upload_url: Option<String>,
    pub build_log_path: String,
}

impl BuildArtifact {
    pub fn size_mb(&self) -> f32 { self.file_size_bytes as f32 / (1024.0 * 1024.0) }
    pub fn compressed_mb(&self) -> f32 { self.compressed_size_bytes as f32 / (1024.0 * 1024.0) }
    pub fn compression_ratio(&self) -> f32 {
        if self.file_size_bytes == 0 { return 1.0; }
        self.file_size_bytes as f32 / self.compressed_size_bytes.max(1) as f32
    }
}

#[derive(Debug)]
pub struct BuildSystemEditor {
    pub targets: Vec<BuildTarget>,
    pub selected_target: Option<usize>,
    pub active_panel: BuildEditorPanel,
    pub build_state: BuildState,
    pub current_step_index: usize,
    pub step_progress: f32,
    pub overall_progress: f32,
    pub current_log: BuildLog,
    pub historical_logs: Vec<BuildLog>,
    pub artifacts: Vec<BuildArtifact>,
    pub version: VersionInfo,
    pub ci_settings: CISettings,
    pub asset_processors: Vec<AssetProcessorSettings>,
    pub build_queue: Vec<usize>,
    pub last_build_duration_secs: f32,
    pub search_filter: String,
    pub log_level_filter: Option<LogLevel>,
    pub show_only_errors: bool,
    pub auto_run_tests: bool,
    pub parallel_asset_processing: bool,
    pub asset_cache_path: String,
    pub output_base_path: String,
}

impl Default for BuildSystemEditor {
    fn default() -> Self {
        let targets = vec![
            BuildTarget::windows_release(),
            BuildTarget::android_release(),
            BuildTarget::web_gl(),
        ];

        let mut log = BuildLog::new();
        log.push(BuildLogEntry::info("Build system initialized"));
        log.push(BuildLogEntry::info("Loaded 3 build targets"));
        log.push(BuildLogEntry::warning("Shader cache is stale, recommend recompiling"));

        let asset_processors = vec![
            AssetProcessorSettings::new(AssetProcessorKind::TextureCompressor).with_param("quality", "high"),
            AssetProcessorSettings::new(AssetProcessorKind::AudioTranscoder).with_param("bitrate", "192"),
            AssetProcessorSettings::new(AssetProcessorKind::MeshOptimizer).with_param("lod_levels", "3"),
            AssetProcessorSettings::new(AssetProcessorKind::ShaderCompiler).with_param("cache", "true"),
            AssetProcessorSettings::new(AssetProcessorKind::FontPacker),
            AssetProcessorSettings::new(AssetProcessorKind::AnimationCompressor),
            AssetProcessorSettings::new(AssetProcessorKind::NavMeshBaker),
            AssetProcessorSettings::new(AssetProcessorKind::LutBaker),
        ];

        Self {
            targets,
            selected_target: Some(0),
            active_panel: BuildEditorPanel::Targets,
            build_state: BuildState::Idle,
            current_step_index: 0,
            step_progress: 0.0,
            overall_progress: 0.0,
            current_log: log,
            historical_logs: Vec::new(),
            artifacts: Vec::new(),
            version: VersionInfo::default(),
            ci_settings: CISettings::default(),
            asset_processors,
            build_queue: Vec::new(),
            last_build_duration_secs: 0.0,
            search_filter: String::new(),
            log_level_filter: None,
            show_only_errors: false,
            auto_run_tests: false,
            parallel_asset_processing: true,
            asset_cache_path: ".build_cache".to_string(),
            output_base_path: "build".to_string(),
        }
    }
}

impl BuildSystemEditor {
    pub fn selected_target(&self) -> Option<&BuildTarget> {
        self.selected_target.and_then(|i| self.targets.get(i))
    }

    pub fn start_build(&mut self, target_index: usize) {
        self.build_state = BuildState::Running;
        self.current_step_index = 0;
        self.step_progress = 0.0;
        self.overall_progress = 0.0;
        self.current_log = BuildLog::new();
        self.current_log.push(BuildLogEntry::info(&format!("Starting build: {}", self.targets[target_index].name)));
    }

    pub fn simulate_build_tick(&mut self, dt: f32) {
        if self.build_state != BuildState::Running { return; }
        let target = match self.selected_target.and_then(|i| self.targets.get(i)) {
            Some(t) => t,
            None => return,
        };
        let total_steps = target.step_count();
        if total_steps == 0 { self.build_state = BuildState::Completed; return; }

        self.step_progress += dt * 0.5;
        if self.step_progress >= 1.0 {
            self.step_progress = 0.0;
            self.current_step_index += 1;
            if self.current_step_index >= total_steps {
                self.build_state = BuildState::Completed;
                self.current_log.success = Some(true);
                self.current_log.push(BuildLogEntry::info("Build completed successfully"));
                return;
            }
            let enabled_steps: Vec<&BuildStep> = target.steps.iter().filter(|s| s.enabled).collect();
            if let Some(step) = enabled_steps.get(self.current_step_index) {
                self.current_log.push(BuildLogEntry::info(&format!("Step: {}", step.kind.label())));
            }
        }
        self.overall_progress = (self.current_step_index as f32 + self.step_progress) / total_steps as f32;
    }

    pub fn cancel_build(&mut self) {
        if self.build_state.is_active() {
            self.build_state = BuildState::Cancelled;
            self.current_log.success = Some(false);
            self.current_log.push(BuildLogEntry::warning("Build cancelled by user"));
        }
    }

    pub fn filtered_log_entries(&self) -> Vec<&BuildLogEntry> {
        let q = self.search_filter.to_lowercase();
        self.current_log.entries.iter().filter(|e| {
            let level_ok = if self.show_only_errors { e.level.is_error() }
                else if let Some(lvl) = self.log_level_filter { e.level == lvl } else { true };
            let text_ok = q.is_empty() || e.message.to_lowercase().contains(&q);
            level_ok && text_ok
        }).collect()
    }

    pub fn add_to_queue(&mut self, target_index: usize) {
        if !self.build_queue.contains(&target_index) {
            self.build_queue.push(target_index);
        }
    }

    pub fn remove_from_queue(&mut self, target_index: usize) {
        self.build_queue.retain(|&i| i != target_index);
    }

    pub fn total_estimated_build_time(&self) -> f32 {
        self.targets.iter().filter(|t| t.enabled).map(|t| t.total_estimated_build_time_secs()).sum()
    }

    pub fn platforms_summary(&self) -> Vec<(BuildPlatform, usize)> {
        let mut counts: HashMap<BuildPlatform, usize> = HashMap::new();
        for t in &self.targets {
            *counts.entry(t.platform).or_insert(0) += 1;
        }
        counts.into_iter().collect()
    }

    pub fn generate_ci_yaml(&self) -> String {
        match self.ci_settings.provider {
            CIProvider::GitHub => {
                let mut lines = vec![
                    "name: Build".to_string(),
                    "on:".to_string(),
                    "  push:".to_string(),
                    "    branches: [main, 'release/*']".to_string(),
                    "  tags: ['v*']".to_string(),
                    "jobs:".to_string(),
                    "  build:".to_string(),
                    "    runs-on: ubuntu-latest".to_string(),
                    "    steps:".to_string(),
                    "    - uses: actions/checkout@v4".to_string(),
                    "    - name: Build All Targets".to_string(),
                    "      run: cargo build --release".to_string(),
                ];
                for target in &self.targets {
                    if target.enabled {
                        lines.push(format!("    # Target: {} ({})", target.name, target.platform.label()));
                    }
                }
                lines.join("\n")
            }
            _ => "# CI configuration not yet generated for this provider".to_string(),
        }
    }
}
