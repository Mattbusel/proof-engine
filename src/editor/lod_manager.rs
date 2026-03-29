
//! Level-of-detail manager — mesh LOD generation, terrain LOD, streaming, occlusion culling.

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// LOD level descriptor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LodTransitionMode {
    Discrete,
    CrossFade,
    SpeedTree,
    Dither,
}

#[derive(Debug, Clone)]
pub struct LodLevel {
    pub index: usize,
    pub mesh_id: u64,
    pub screen_relative_transition_height: f32,
    pub fade_transition_width: f32,
    pub renderers_enabled: Vec<bool>,
    pub triangle_count: u32,
    pub vertex_count: u32,
    pub reduction_ratio: f32,
    pub shadow_casting: bool,
    pub shadow_receiving: bool,
    pub motion_vectors: bool,
    pub skinned_motion_vectors: bool,
}

impl LodLevel {
    pub fn new(index: usize, mesh_id: u64, transition: f32, tris: u32, verts: u32) -> Self {
        Self {
            index,
            mesh_id,
            screen_relative_transition_height: transition,
            fade_transition_width: 0.1,
            renderers_enabled: vec![true],
            triangle_count: tris,
            vertex_count: verts,
            reduction_ratio: 1.0,
            shadow_casting: true,
            shadow_receiving: true,
            motion_vectors: false,
            skinned_motion_vectors: false,
        }
    }
}

// ---------------------------------------------------------------------------
// LOD group
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodGroup {
    pub id: u64,
    pub name: String,
    pub center: Vec3,
    pub size: f32,
    pub levels: Vec<LodLevel>,
    pub transition_mode: LodTransitionMode,
    pub animate_cross_fading: bool,
    pub fade_mode: FadeMode,
    pub current_lod: usize,
    pub fade_t: f32,
    pub enabled: bool,
    pub position: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadeMode {
    None,
    CrossFade,
    SpeedTree,
}

impl LodGroup {
    pub fn new(id: u64, name: impl Into<String>, position: Vec3, size: f32) -> Self {
        Self {
            id,
            name: name.into(),
            center: Vec3::ZERO,
            size,
            levels: Vec::new(),
            transition_mode: LodTransitionMode::CrossFade,
            animate_cross_fading: true,
            fade_mode: FadeMode::CrossFade,
            current_lod: 0,
            fade_t: 0.0,
            enabled: true,
            position,
        }
    }

    pub fn add_level(&mut self, level: LodLevel) {
        let i = self.levels.partition_point(|l| l.screen_relative_transition_height > level.screen_relative_transition_height);
        self.levels.insert(i, level);
        // Renumber
        for (j, l) in self.levels.iter_mut().enumerate() {
            l.index = j;
        }
    }

    /// Compute the screen coverage for a given camera distance.
    pub fn screen_coverage(&self, distance: f32, fov_tan: f32, screen_height: f32) -> f32 {
        if distance < 0.001 { return 1.0; }
        let world_size = self.size;
        let projected_size = world_size / (distance * fov_tan);
        (projected_size / screen_height).clamp(0.0, 1.0)
    }

    /// Determine which LOD should be active.
    pub fn compute_lod(&self, screen_coverage: f32) -> usize {
        for (i, level) in self.levels.iter().enumerate() {
            if screen_coverage >= level.screen_relative_transition_height {
                return i;
            }
        }
        self.levels.len().saturating_sub(1) // Culled
    }

    pub fn update(&mut self, camera_pos: Vec3, fov_tan: f32, screen_height: f32, dt: f32) {
        let dist = self.position.distance(camera_pos);
        let cov = self.screen_coverage(dist, fov_tan, screen_height);
        let new_lod = self.compute_lod(cov);
        if new_lod != self.current_lod {
            if self.transition_mode == LodTransitionMode::CrossFade {
                self.fade_t = 1.0;
            }
            self.current_lod = new_lod;
        }
        if self.fade_t > 0.0 {
            self.fade_t = (self.fade_t - dt * 4.0).max(0.0);
        }
    }

    pub fn triangle_reduction_from_lod0(&self, lod_idx: usize) -> f32 {
        if self.levels.is_empty() { return 1.0; }
        let base = self.levels[0].triangle_count as f32;
        let current = self.levels.get(lod_idx).map(|l| l.triangle_count as f32).unwrap_or(0.0);
        if base > 0.0 { current / base } else { 1.0 }
    }
}

// ---------------------------------------------------------------------------
// Mesh simplification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimplificationAlgorithm {
    QEM,           // Quadric Error Metrics
    MeshOptimizer,
    Sloppy,
    UniformGrid,
}

#[derive(Debug, Clone)]
pub struct SimplificationSettings {
    pub algorithm: SimplificationAlgorithm,
    pub target_ratio: f32,           // 0..1, proportion of triangles to keep
    pub max_error: f32,
    pub preserve_borders: bool,
    pub preserve_uvs: bool,
    pub preserve_normals: bool,
    pub preserve_attributes: bool,
    pub lock_border: bool,
    pub merge_threshold: f32,
    pub attribute_weight: f32,
}

impl Default for SimplificationSettings {
    fn default() -> Self {
        Self {
            algorithm: SimplificationAlgorithm::QEM,
            target_ratio: 0.5,
            max_error: 0.001,
            preserve_borders: true,
            preserve_uvs: true,
            preserve_normals: true,
            preserve_attributes: true,
            lock_border: false,
            merge_threshold: 1e-4,
            attribute_weight: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimplificationResult {
    pub original_tris: u32,
    pub result_tris: u32,
    pub original_verts: u32,
    pub result_verts: u32,
    pub max_deviation: f32,
    pub rms_deviation: f32,
    pub processing_ms: f32,
    pub success: bool,
    pub error_message: Option<String>,
}

impl SimplificationResult {
    pub fn ratio(&self) -> f32 {
        if self.original_tris == 0 { return 1.0; }
        self.result_tris as f32 / self.original_tris as f32
    }
}

/// Simulates LOD generation result (no actual mesh processing).
pub fn generate_lod_levels(mesh_id: u64, original_tris: u32, original_verts: u32, ratios: &[f32]) -> Vec<(LodLevel, SimplificationResult)> {
    ratios.iter().enumerate().map(|(i, &ratio)| {
        let result_tris = (original_tris as f32 * ratio) as u32;
        let result_verts = (original_verts as f32 * ratio) as u32;
        let transition = match i {
            0 => 1.0,
            1 => 0.5,
            2 => 0.25,
            3 => 0.1,
            _ => 0.05 / i as f32,
        };
        let level = LodLevel::new(i, mesh_id * 100 + i as u64, transition, result_tris, result_verts);
        let result = SimplificationResult {
            original_tris,
            result_tris,
            original_verts,
            result_verts,
            max_deviation: 0.001 * (1.0 - ratio),
            rms_deviation: 0.0003 * (1.0 - ratio),
            processing_ms: original_tris as f32 * 0.001 * (1.0 - ratio),
            success: true,
            error_message: None,
        };
        (level, result)
    }).collect()
}

// ---------------------------------------------------------------------------
// Occlusion culling
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OcclusionMode {
    None,
    HiZ,          // Hierarchical Z-buffer
    SoftwareRasterization,
    PvsPortal,
    Umbra,
}

#[derive(Debug, Clone)]
pub struct OcclusionSettings {
    pub mode: OcclusionMode,
    pub occluder_size_threshold: f32,
    pub occludee_size_threshold: f32,
    pub backface_culling: bool,
    pub hi_z_mip_levels: u32,
    pub conservative_depth: bool,
    pub async_readback: bool,
    pub readback_frame_delay: u32,
    pub debug_draw_occluders: bool,
    pub debug_draw_occludees: bool,
}

impl Default for OcclusionSettings {
    fn default() -> Self {
        Self {
            mode: OcclusionMode::HiZ,
            occluder_size_threshold: 0.01,
            occludee_size_threshold: 0.001,
            backface_culling: true,
            hi_z_mip_levels: 8,
            conservative_depth: false,
            async_readback: true,
            readback_frame_delay: 2,
            debug_draw_occluders: false,
            debug_draw_occludees: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OcclusionQuery {
    pub object_id: u64,
    pub bounding_sphere: (Vec3, f32),
    pub visible_last_frame: bool,
    pub frames_invisible: u32,
    pub frames_visible: u32,
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamingState {
    Unloaded,
    Queued,
    Loading,
    Loaded,
    Unloading,
    Error,
}

#[derive(Debug, Clone)]
pub struct StreamableAsset {
    pub id: u64,
    pub name: String,
    pub size_bytes: u64,
    pub lod_group: Option<u64>,
    pub streaming_state: StreamingState,
    pub load_priority: f32,
    pub last_visible_frame: u64,
    pub retain_frames: u32,
    pub memory_budget_category: MemoryCategory,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryCategory {
    Critical,
    High,
    Medium,
    Low,
    Background,
}

impl MemoryCategory {
    pub fn eviction_priority(self) -> u8 {
        match self {
            MemoryCategory::Critical => 255,
            MemoryCategory::High => 200,
            MemoryCategory::Medium => 128,
            MemoryCategory::Low => 64,
            MemoryCategory::Background => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamingManager {
    pub assets: Vec<StreamableAsset>,
    pub max_memory_bytes: u64,
    pub current_memory_bytes: u64,
    pub load_queue: Vec<u64>,
    pub unload_queue: Vec<u64>,
    pub current_frame: u64,
    pub bandwidth_limit_bytes_per_frame: u64,
    pub bytes_loaded_this_frame: u64,
    pub priority_bias_distance: f32,
    pub camera_pos: Vec3,
}

impl StreamingManager {
    pub fn new(max_memory_mb: u32) -> Self {
        Self {
            assets: Vec::new(),
            max_memory_bytes: max_memory_mb as u64 * 1_048_576,
            current_memory_bytes: 0,
            load_queue: Vec::new(),
            unload_queue: Vec::new(),
            current_frame: 0,
            bandwidth_limit_bytes_per_frame: 64 * 1_048_576, // 64 MB/frame
            bytes_loaded_this_frame: 0,
            priority_bias_distance: 50.0,
            camera_pos: Vec3::ZERO,
        }
    }

    pub fn register_asset(&mut self, asset: StreamableAsset) {
        self.assets.push(asset);
    }

    pub fn update(&mut self, camera_pos: Vec3) {
        self.camera_pos = camera_pos;
        self.current_frame += 1;
        self.bytes_loaded_this_frame = 0;
        // Simulate loading
        let mut loaded = Vec::new();
        for &id in &self.load_queue {
            if let Some(asset) = self.assets.iter_mut().find(|a| a.id == id) {
                if self.current_memory_bytes + asset.size_bytes <= self.max_memory_bytes
                    && self.bytes_loaded_this_frame + asset.size_bytes <= self.bandwidth_limit_bytes_per_frame {
                    asset.streaming_state = StreamingState::Loaded;
                    self.current_memory_bytes += asset.size_bytes;
                    self.bytes_loaded_this_frame += asset.size_bytes;
                    loaded.push(id);
                }
            }
        }
        self.load_queue.retain(|id| !loaded.contains(id));
        // Simulate unloading
        let mut unloaded = Vec::new();
        for &id in &self.unload_queue {
            if let Some(asset) = self.assets.iter_mut().find(|a| a.id == id) {
                self.current_memory_bytes = self.current_memory_bytes.saturating_sub(asset.size_bytes);
                asset.streaming_state = StreamingState::Unloaded;
                unloaded.push(id);
            }
        }
        self.unload_queue.retain(|id| !unloaded.contains(id));
        // Evict old assets if over budget
        if self.current_memory_bytes > self.max_memory_bytes {
            self.evict_lru();
        }
    }

    pub fn request_load(&mut self, id: u64, priority: f32) {
        if let Some(asset) = self.assets.iter_mut().find(|a| a.id == id) {
            if asset.streaming_state == StreamingState::Unloaded {
                asset.streaming_state = StreamingState::Queued;
                asset.load_priority = priority;
                self.load_queue.push(id);
                // Sort by priority descending
                let assets = &self.assets;
                self.load_queue.sort_by(|a, b| {
                    let pa = assets.iter().find(|x| x.id == *a).map(|x| x.load_priority).unwrap_or(0.0);
                    let pb = assets.iter().find(|x| x.id == *b).map(|x| x.load_priority).unwrap_or(0.0);
                    pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
    }

    pub fn request_unload(&mut self, id: u64) {
        if let Some(asset) = self.assets.iter_mut().find(|a| a.id == id) {
            if asset.streaming_state == StreamingState::Loaded {
                asset.streaming_state = StreamingState::Unloading;
                self.unload_queue.push(id);
            }
        }
    }

    fn evict_lru(&mut self) {
        // Find least-recently-used non-critical loaded asset
        let current_frame = self.current_frame;
        let evict_id = self.assets.iter()
            .filter(|a| a.streaming_state == StreamingState::Loaded && a.memory_budget_category != MemoryCategory::Critical)
            .min_by_key(|a| a.last_visible_frame)
            .map(|a| a.id);
        if let Some(id) = evict_id {
            self.request_unload(id);
        }
    }

    pub fn memory_pressure(&self) -> f32 {
        self.current_memory_bytes as f32 / self.max_memory_bytes as f32
    }

    pub fn loaded_count(&self) -> usize {
        self.assets.iter().filter(|a| a.streaming_state == StreamingState::Loaded).count()
    }
}

// ---------------------------------------------------------------------------
// LOD Manager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodManagerStats {
    pub total_lod_groups: usize,
    pub active_lod_groups: usize,
    pub lod0_count: usize,
    pub lod1_count: usize,
    pub lod2_count: usize,
    pub culled_count: usize,
    pub total_triangles_without_lod: u64,
    pub total_triangles_with_lod: u64,
    pub savings_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct LodManager {
    pub groups: Vec<LodGroup>,
    pub occlusion_settings: OcclusionSettings,
    pub occlusion_queries: Vec<OcclusionQuery>,
    pub streaming: StreamingManager,
    pub camera_pos: Vec3,
    pub camera_fov: f32,
    pub screen_height: f32,
    pub enable_lod: bool,
    pub enable_occlusion: bool,
    pub lod_bias: f32,
    pub max_active_groups: usize,
    pub stats: LodManagerStats,
}

impl LodManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            groups: Vec::new(),
            occlusion_settings: OcclusionSettings::default(),
            occlusion_queries: Vec::new(),
            streaming: StreamingManager::new(1024),
            camera_pos: Vec3::ZERO,
            camera_fov: 60.0,
            screen_height: 1080.0,
            enable_lod: true,
            enable_occlusion: true,
            lod_bias: 1.0,
            max_active_groups: 10000,
            stats: LodManagerStats {
                total_lod_groups: 0, active_lod_groups: 0, lod0_count: 0,
                lod1_count: 0, lod2_count: 0, culled_count: 0,
                total_triangles_without_lod: 0, total_triangles_with_lod: 0, savings_ratio: 0.0,
            },
        };
        mgr.populate_demo();
        mgr
    }

    fn populate_demo(&mut self) {
        // Add a variety of LOD groups
        let positions = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(-10.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 15.0),
            Vec3::new(20.0, 0.0, -5.0),
        ];
        let sizes = [2.0_f32, 5.0, 1.5, 8.0, 3.0];
        let base_tris = [5000u32, 12000, 3000, 20000, 8000];
        for (i, ((pos, size), tris)) in positions.iter().zip(sizes.iter()).zip(base_tris.iter()).enumerate() {
            let mut group = LodGroup::new(i as u64 + 1, format!("Object_{}", i), *pos, *size);
            let levels = generate_lod_levels(i as u64 * 100, *tris, tris / 2, &[1.0, 0.5, 0.25, 0.1]);
            for (level, _) in levels {
                group.add_level(level);
            }
            self.groups.push(group);
        }
    }

    pub fn add_group(&mut self, group: LodGroup) {
        self.groups.push(group);
    }

    pub fn update(&mut self, camera_pos: Vec3, dt: f32) {
        self.camera_pos = camera_pos;
        self.streaming.update(camera_pos);
        if !self.enable_lod { return; }
        let fov_tan = (self.camera_fov * 0.5 * std::f32::consts::PI / 180.0).tan();
        let screen_h = self.screen_height;
        // Update LOD for each group
        for group in &mut self.groups {
            if !group.enabled { continue; }
            group.update(camera_pos, fov_tan, screen_h, dt);
        }
        self.recompute_stats();
    }

    fn recompute_stats(&mut self) {
        let mut stats = LodManagerStats {
            total_lod_groups: self.groups.len(),
            active_lod_groups: self.groups.iter().filter(|g| g.enabled).count(),
            lod0_count: 0, lod1_count: 0, lod2_count: 0, culled_count: 0,
            total_triangles_without_lod: 0,
            total_triangles_with_lod: 0,
            savings_ratio: 0.0,
        };
        for group in &self.groups {
            if !group.enabled { continue; }
            if group.levels.is_empty() { continue; }
            let base_tris = group.levels[0].triangle_count as u64;
            stats.total_triangles_without_lod += base_tris;
            let cur_tris = group.levels.get(group.current_lod).map(|l| l.triangle_count as u64).unwrap_or(0);
            stats.total_triangles_with_lod += cur_tris;
            match group.current_lod {
                0 => stats.lod0_count += 1,
                1 => stats.lod1_count += 1,
                2 => stats.lod2_count += 1,
                _ => stats.culled_count += 1,
            }
        }
        if stats.total_triangles_without_lod > 0 {
            stats.savings_ratio = 1.0 - (stats.total_triangles_with_lod as f32 / stats.total_triangles_without_lod as f32);
        }
        self.stats = stats;
    }

    pub fn find_group(&self, id: u64) -> Option<&LodGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    pub fn find_group_mut(&mut self, id: u64) -> Option<&mut LodGroup> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    pub fn triangle_savings_str(&self) -> String {
        format!("{:.1}% triangle reduction ({} → {})",
            self.stats.savings_ratio * 100.0,
            self.stats.total_triangles_without_lod,
            self.stats.total_triangles_with_lod)
    }

    pub fn generate_lod_for_group(&mut self, group_id: u64, original_tris: u32, original_verts: u32, ratios: &[f32]) {
        if let Some(group) = self.find_group_mut(group_id) {
            group.levels.clear();
            let levels = generate_lod_levels(group_id, original_tris, original_verts, ratios);
            for (level, _) in levels {
                group.add_level(level);
            }
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
    fn test_lod_group() {
        let mut group = LodGroup::new(1, "test", Vec3::ZERO, 5.0);
        let levels = generate_lod_levels(1, 10000, 5000, &[1.0, 0.5, 0.25]);
        for (l, _) in levels { group.add_level(l); }
        assert_eq!(group.levels.len(), 3);
        let cov = group.screen_coverage(10.0, 0.5773, 1080.0);
        assert!(cov > 0.0 && cov <= 1.0);
    }

    #[test]
    fn test_lod_compute() {
        let mut group = LodGroup::new(1, "test", Vec3::ZERO, 5.0);
        let levels = generate_lod_levels(1, 10000, 5000, &[1.0, 0.5, 0.25, 0.1]);
        for (l, _) in levels { group.add_level(l); }
        assert_eq!(group.compute_lod(1.0), 0);
        assert_eq!(group.compute_lod(0.4), 1);
        assert_eq!(group.compute_lod(0.05), 3);
    }

    #[test]
    fn test_streaming_manager() {
        let mut mgr = StreamingManager::new(256);
        mgr.register_asset(StreamableAsset {
            id: 1, name: "TestMesh".into(), size_bytes: 1_048_576,
            lod_group: None, streaming_state: StreamingState::Unloaded,
            load_priority: 1.0, last_visible_frame: 0, retain_frames: 10,
            memory_budget_category: MemoryCategory::Medium,
        });
        mgr.request_load(1, 1.0);
        mgr.update(Vec3::ZERO);
        assert_eq!(mgr.loaded_count(), 1);
    }

    #[test]
    fn test_lod_manager() {
        let mut mgr = LodManager::new();
        assert!(!mgr.groups.is_empty());
        mgr.update(Vec3::ZERO, 0.016);
        assert!(mgr.stats.total_lod_groups > 0);
    }
}
