//! Adaptive quality management: auto-detect hardware capability, dynamically
//! adjust rendering quality to maintain a target frame rate, and benchmarking.

use super::backend::BackendCapabilities;
use super::renderer::MultiBackendRenderer;

// ---------------------------------------------------------------------------
// Quality level
// ---------------------------------------------------------------------------

/// Discrete quality tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QualityLevel {
    Potato = 0,
    Low    = 1,
    Medium = 2,
    High   = 3,
    Ultra  = 4,
}

impl QualityLevel {
    /// All levels from lowest to highest.
    pub const ALL: [QualityLevel; 5] = [
        Self::Potato, Self::Low, Self::Medium, Self::High, Self::Ultra,
    ];

    /// Try to go one level up.
    pub fn upgrade(self) -> Option<QualityLevel> {
        match self {
            Self::Potato => Some(Self::Low),
            Self::Low    => Some(Self::Medium),
            Self::Medium => Some(Self::High),
            Self::High   => Some(Self::Ultra),
            Self::Ultra  => None,
        }
    }

    /// Try to go one level down.
    pub fn downgrade(self) -> Option<QualityLevel> {
        match self {
            Self::Potato => None,
            Self::Low    => Some(Self::Potato),
            Self::Medium => Some(Self::Low),
            Self::High   => Some(Self::Medium),
            Self::Ultra  => Some(Self::High),
        }
    }

    /// Index 0..4 for array lookups.
    pub fn index(self) -> usize { self as usize }
}

impl std::fmt::Display for QualityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Potato => write!(f, "Potato"),
            Self::Low    => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High   => write!(f, "High"),
            Self::Ultra  => write!(f, "Ultra"),
        }
    }
}

// ---------------------------------------------------------------------------
// Quality profile
// ---------------------------------------------------------------------------

/// Concrete rendering settings for a quality level.
#[derive(Debug, Clone)]
pub struct QualityProfile {
    pub particle_count: u32,
    pub bloom_passes: u32,
    pub shadow_resolution: u32,
    pub postfx_enabled: bool,
    pub compute_enabled: bool,
    pub msaa: u32,
}

impl QualityProfile {
    /// Build a profile for the given quality level.
    pub fn for_level(level: QualityLevel) -> Self {
        match level {
            QualityLevel::Potato => Self {
                particle_count: 1_000,
                bloom_passes: 0,
                shadow_resolution: 256,
                postfx_enabled: false,
                compute_enabled: false,
                msaa: 1,
            },
            QualityLevel::Low => Self {
                particle_count: 5_000,
                bloom_passes: 1,
                shadow_resolution: 512,
                postfx_enabled: false,
                compute_enabled: false,
                msaa: 1,
            },
            QualityLevel::Medium => Self {
                particle_count: 20_000,
                bloom_passes: 2,
                shadow_resolution: 1024,
                postfx_enabled: true,
                compute_enabled: true,
                msaa: 2,
            },
            QualityLevel::High => Self {
                particle_count: 100_000,
                bloom_passes: 3,
                shadow_resolution: 2048,
                postfx_enabled: true,
                compute_enabled: true,
                msaa: 4,
            },
            QualityLevel::Ultra => Self {
                particle_count: 500_000,
                bloom_passes: 4,
                shadow_resolution: 4096,
                postfx_enabled: true,
                compute_enabled: true,
                msaa: 8,
            },
        }
    }

    /// Estimated VRAM usage in bytes for this profile (rough heuristic).
    pub fn estimated_vram(&self) -> u64 {
        let shadow = (self.shadow_resolution as u64) * (self.shadow_resolution as u64) * 4;
        let particles = (self.particle_count as u64) * 64; // ~64 bytes per particle
        let msaa_factor = self.msaa as u64;
        shadow * msaa_factor + particles
    }
}

// ---------------------------------------------------------------------------
// Auto-detect
// ---------------------------------------------------------------------------

/// Choose a quality level based on hardware capabilities.
pub fn auto_detect_quality(capabilities: &BackendCapabilities) -> QualityLevel {
    let mut score = 0u32;

    // Texture size
    if capabilities.max_texture_size >= 16384 { score += 3; }
    else if capabilities.max_texture_size >= 8192 { score += 2; }
    else if capabilities.max_texture_size >= 4096 { score += 1; }

    // Compute
    if capabilities.compute_shaders { score += 2; }

    // SSBO size
    if capabilities.max_ssbo_size >= 1024 * 1024 * 1024 { score += 2; }
    else if capabilities.max_ssbo_size >= 256 * 1024 * 1024 { score += 1; }

    // Indirect draw
    if capabilities.indirect_draw { score += 1; }
    if capabilities.multi_draw_indirect { score += 1; }

    // Workgroup size
    if capabilities.max_workgroup_size[0] >= 1024 { score += 1; }

    match score {
        0..=2  => QualityLevel::Potato,
        3..=4  => QualityLevel::Low,
        5..=7  => QualityLevel::Medium,
        8..=9  => QualityLevel::High,
        _      => QualityLevel::Ultra,
    }
}

// ---------------------------------------------------------------------------
// QualityManager
// ---------------------------------------------------------------------------

/// Dynamically adjusts quality level to maintain a target frame rate.
pub struct QualityManager {
    pub current: QualityLevel,
    pub target_fps: f32,
    fps_history: Vec<f32>,
    max_history: usize,
    upgrade_cooldown: f32,
    downgrade_cooldown: f32,
    time_since_change: f32,
    /// Minimum time (seconds) between quality changes.
    pub cooldown_seconds: f32,
    /// FPS must be above target * this factor before considering upgrade.
    pub upgrade_headroom: f32,
    /// FPS must be below target * this factor before considering downgrade.
    pub downgrade_threshold: f32,
}

impl QualityManager {
    pub fn new(initial: QualityLevel, target_fps: f32) -> Self {
        Self {
            current: initial,
            target_fps,
            fps_history: Vec::with_capacity(60),
            max_history: 60,
            upgrade_cooldown: 0.0,
            downgrade_cooldown: 0.0,
            time_since_change: 0.0,
            cooldown_seconds: 3.0,
            upgrade_headroom: 1.15,   // need 15% headroom above target
            downgrade_threshold: 0.85, // drop below 85% of target
        }
    }

    /// Call each frame with current FPS and delta time.
    pub fn tick(&mut self, current_fps: f32, dt: f32) {
        if self.fps_history.len() >= self.max_history {
            self.fps_history.remove(0);
        }
        self.fps_history.push(current_fps);
        self.time_since_change += dt;

        if self.time_since_change < self.cooldown_seconds {
            return;
        }

        let avg = self.average_fps();

        if self.should_downgrade_at(avg) {
            if let Some(lower) = self.current.downgrade() {
                self.current = lower;
                self.time_since_change = 0.0;
                self.fps_history.clear();
            }
        } else if self.should_upgrade_at(avg) {
            if let Some(higher) = self.current.upgrade() {
                self.current = higher;
                self.time_since_change = 0.0;
                self.fps_history.clear();
            }
        }
    }

    /// Average FPS over the history window.
    pub fn average_fps(&self) -> f32 {
        if self.fps_history.is_empty() { return 0.0; }
        let sum: f32 = self.fps_history.iter().sum();
        sum / self.fps_history.len() as f32
    }

    /// Whether the manager would upgrade at this moment.
    pub fn should_upgrade(&self) -> bool {
        self.should_upgrade_at(self.average_fps())
    }

    /// Whether the manager would downgrade at this moment.
    pub fn should_downgrade(&self) -> bool {
        self.should_downgrade_at(self.average_fps())
    }

    fn should_upgrade_at(&self, avg_fps: f32) -> bool {
        avg_fps > self.target_fps * self.upgrade_headroom
            && self.current.upgrade().is_some()
    }

    fn should_downgrade_at(&self, avg_fps: f32) -> bool {
        avg_fps < self.target_fps * self.downgrade_threshold
            && self.current.downgrade().is_some()
    }

    /// Current quality profile.
    pub fn profile(&self) -> QualityProfile {
        QualityProfile::for_level(self.current)
    }

    /// Force a specific quality level.
    pub fn set_quality(&mut self, level: QualityLevel) {
        self.current = level;
        self.time_since_change = 0.0;
        self.fps_history.clear();
    }
}

// ---------------------------------------------------------------------------
// BenchmarkResult
// ---------------------------------------------------------------------------

/// Result of a GPU benchmark run.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub fps: f32,
    pub gpu_ms: f32,
    pub cpu_ms: f32,
    pub vram_used: u64,
}

impl BenchmarkResult {
    pub fn score(&self) -> f32 {
        // Simple composite: FPS weighted by inverse GPU time.
        if self.gpu_ms > 0.0 {
            self.fps * (16.67 / self.gpu_ms)
        } else {
            self.fps
        }
    }
}

/// Run a simple benchmark on the given renderer for `duration_secs`.
/// Because we have no real GPU, this measures CPU-side overhead of the
/// software backend.
pub fn run_benchmark(renderer: &mut MultiBackendRenderer, duration_secs: f32) -> BenchmarkResult {
    use std::time::Instant;
    use super::backend::{BufferUsage, PipelineLayout, ShaderStage};
    use super::renderer::{DrawCall, RenderPass};

    let start = Instant::now();
    let mut frames = 0u32;
    let target_duration = std::time::Duration::from_secs_f32(duration_secs);

    // Create minimal resources for the benchmark.
    let vbuf = renderer.create_vertex_buffer(&[0u8; 48]);
    let pipe = renderer.backend.create_pipeline(
        renderer.backend.create_shader("v", ShaderStage::Vertex),
        renderer.backend.create_shader("f", ShaderStage::Fragment),
        &PipelineLayout::default(),
    );
    let pass = RenderPass::new();
    let call = DrawCall::new(pipe, vbuf, 3);

    while start.elapsed() < target_duration {
        renderer.begin_frame();
        renderer.draw(&pass, &[call.clone()]);
        renderer.end_frame();
        frames += 1;
    }

    let elapsed = start.elapsed().as_secs_f32();
    let fps = frames as f32 / elapsed;
    let frame_ms = elapsed * 1000.0 / frames.max(1) as f32;

    renderer.destroy_buffer(vbuf);

    BenchmarkResult {
        fps,
        gpu_ms: frame_ms, // In software, GPU = CPU
        cpu_ms: frame_ms,
        vram_used: 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu_backend::backend::{BackendCapabilities, GpuBackend};

    #[test]
    fn quality_level_ordering() {
        assert!(QualityLevel::Potato < QualityLevel::Low);
        assert!(QualityLevel::Low < QualityLevel::Medium);
        assert!(QualityLevel::Medium < QualityLevel::High);
        assert!(QualityLevel::High < QualityLevel::Ultra);
    }

    #[test]
    fn quality_level_upgrade_downgrade() {
        assert_eq!(QualityLevel::Potato.upgrade(), Some(QualityLevel::Low));
        assert_eq!(QualityLevel::Ultra.upgrade(), None);
        assert_eq!(QualityLevel::Ultra.downgrade(), Some(QualityLevel::High));
        assert_eq!(QualityLevel::Potato.downgrade(), None);
    }

    #[test]
    fn quality_level_display() {
        assert_eq!(format!("{}", QualityLevel::Medium), "Medium");
        assert_eq!(format!("{}", QualityLevel::Potato), "Potato");
    }

    #[test]
    fn quality_level_index() {
        assert_eq!(QualityLevel::Potato.index(), 0);
        assert_eq!(QualityLevel::Ultra.index(), 4);
    }

    #[test]
    fn quality_profile_for_each_level() {
        for level in QualityLevel::ALL {
            let profile = QualityProfile::for_level(level);
            assert!(profile.particle_count > 0);
            assert!(profile.msaa >= 1);
            assert!(profile.shadow_resolution >= 256);
        }
    }

    #[test]
    fn quality_profiles_scale_up() {
        let potato = QualityProfile::for_level(QualityLevel::Potato);
        let ultra = QualityProfile::for_level(QualityLevel::Ultra);
        assert!(ultra.particle_count > potato.particle_count);
        assert!(ultra.shadow_resolution > potato.shadow_resolution);
        assert!(ultra.msaa > potato.msaa);
    }

    #[test]
    fn estimated_vram_increases_with_quality() {
        let low = QualityProfile::for_level(QualityLevel::Low);
        let high = QualityProfile::for_level(QualityLevel::High);
        assert!(high.estimated_vram() > low.estimated_vram());
    }

    #[test]
    fn auto_detect_vulkan() {
        let caps = BackendCapabilities::for_backend(GpuBackend::Vulkan);
        let level = auto_detect_quality(&caps);
        assert!(level >= QualityLevel::High);
    }

    #[test]
    fn auto_detect_software() {
        let caps = BackendCapabilities::for_backend(GpuBackend::Software);
        let level = auto_detect_quality(&caps);
        // Software has limited caps
        assert!(level <= QualityLevel::Medium);
    }

    #[test]
    fn auto_detect_minimal_caps() {
        let caps = BackendCapabilities {
            compute_shaders: false,
            max_texture_size: 1024,
            max_ssbo_size: 0,
            max_workgroup_size: [64, 64, 1],
            indirect_draw: false,
            multi_draw_indirect: false,
        };
        assert_eq!(auto_detect_quality(&caps), QualityLevel::Potato);
    }

    #[test]
    fn quality_manager_downgrade_on_low_fps() {
        let mut mgr = QualityManager::new(QualityLevel::High, 60.0);
        mgr.cooldown_seconds = 0.0; // disable cooldown for test

        // Simulate many frames at 30 FPS (below 85% of 60 = 51)
        for _ in 0..10 {
            mgr.tick(30.0, 0.016);
        }
        // Should have downgraded
        assert!(mgr.current < QualityLevel::High);
    }

    #[test]
    fn quality_manager_upgrade_on_high_fps() {
        let mut mgr = QualityManager::new(QualityLevel::Low, 60.0);
        mgr.cooldown_seconds = 0.0;

        // Simulate high FPS (above 115% of 60 = 69)
        for _ in 0..10 {
            mgr.tick(120.0, 0.008);
        }
        assert!(mgr.current > QualityLevel::Low);
    }

    #[test]
    fn quality_manager_stays_stable() {
        let mut mgr = QualityManager::new(QualityLevel::Medium, 60.0);
        mgr.cooldown_seconds = 0.0;

        // FPS right at target: should not change
        for _ in 0..20 {
            mgr.tick(60.0, 0.016);
        }
        assert_eq!(mgr.current, QualityLevel::Medium);
    }

    #[test]
    fn quality_manager_cooldown() {
        let mut mgr = QualityManager::new(QualityLevel::High, 60.0);
        mgr.cooldown_seconds = 5.0;

        // Even at low FPS, won't change until cooldown expires
        mgr.tick(10.0, 1.0);
        assert_eq!(mgr.current, QualityLevel::High);
    }

    #[test]
    fn quality_manager_should_upgrade_downgrade() {
        let mut mgr = QualityManager::new(QualityLevel::Medium, 60.0);
        mgr.cooldown_seconds = 0.0;
        for _ in 0..5 { mgr.tick(120.0, 0.008); }
        assert!(mgr.should_upgrade());
        assert!(!mgr.should_downgrade());
    }

    #[test]
    fn quality_manager_average_fps() {
        let mut mgr = QualityManager::new(QualityLevel::Medium, 60.0);
        mgr.tick(50.0, 0.016);
        mgr.tick(70.0, 0.016);
        let avg = mgr.average_fps();
        assert!((avg - 60.0).abs() < 0.01);
    }

    #[test]
    fn quality_manager_set_quality() {
        let mut mgr = QualityManager::new(QualityLevel::Low, 60.0);
        mgr.set_quality(QualityLevel::Ultra);
        assert_eq!(mgr.current, QualityLevel::Ultra);
    }

    #[test]
    fn quality_manager_profile() {
        let mgr = QualityManager::new(QualityLevel::High, 60.0);
        let profile = mgr.profile();
        assert_eq!(profile.msaa, 4);
    }

    #[test]
    fn benchmark_result_score() {
        let result = BenchmarkResult {
            fps: 60.0,
            gpu_ms: 16.67,
            cpu_ms: 16.67,
            vram_used: 0,
        };
        let score = result.score();
        assert!((score - 60.0).abs() < 0.1);
    }

    #[test]
    fn run_benchmark_returns_result() {
        let mut renderer = MultiBackendRenderer::software();
        let result = run_benchmark(&mut renderer, 0.05);
        assert!(result.fps > 0.0);
        assert!(result.gpu_ms > 0.0);
    }

    #[test]
    fn quality_manager_cannot_go_below_potato() {
        let mut mgr = QualityManager::new(QualityLevel::Potato, 60.0);
        mgr.cooldown_seconds = 0.0;
        for _ in 0..20 {
            mgr.tick(5.0, 0.2);
        }
        assert_eq!(mgr.current, QualityLevel::Potato);
    }

    #[test]
    fn quality_manager_cannot_go_above_ultra() {
        let mut mgr = QualityManager::new(QualityLevel::Ultra, 60.0);
        mgr.cooldown_seconds = 0.0;
        for _ in 0..20 {
            mgr.tick(300.0, 0.003);
        }
        assert_eq!(mgr.current, QualityLevel::Ultra);
    }
}
