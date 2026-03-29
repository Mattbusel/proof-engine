#![allow(dead_code, unused_variables, unused_mut, unused_imports)]
use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, HashSet, BTreeMap, VecDeque};

// ============================================================
// CONSTANTS
// ============================================================

const MAX_OCTREE_DEPTH: u32 = 8;
const MAX_OBJECTS_PER_OCTREE_NODE: usize = 16;
const BVH_MAX_LEAF_OBJECTS: usize = 4;
const DEFAULT_CHUNK_SIZE: f32 = 256.0;
const DEFAULT_STREAMING_RADIUS: f32 = 2048.0;
const DEFAULT_MAX_LOADED_CHUNKS: usize = 256;
const DEFAULT_MAX_MEMORY_MB: u64 = 4096;
const LOD_DISTANCES: [f32; 5] = [64.0, 128.0, 256.0, 512.0, 1024.0];
const FRUSTUM_NEAR_PLANE: usize = 0;
const FRUSTUM_FAR_PLANE: usize = 1;
const FRUSTUM_LEFT_PLANE: usize = 2;
const FRUSTUM_RIGHT_PLANE: usize = 3;
const FRUSTUM_TOP_PLANE: usize = 4;
const FRUSTUM_BOTTOM_PLANE: usize = 5;
const VIRTUAL_TEXTURE_TILE_SIZE: u32 = 128;
const VIRTUAL_TEXTURE_ATLAS_SIZE: u32 = 4096;
const IMPOSTOR_ATLAS_COLS: u32 = 8;
const IMPOSTOR_ATLAS_ROWS: u32 = 8;
const TERRAIN_PATCH_SIZE: usize = 65;
const SCREEN_SPACE_ERROR_THRESHOLD: f32 = 2.0;
const SAH_TRAVERSAL_COST: f32 = 1.0;
const SAH_INTERSECTION_COST: f32 = 2.0;
const LRU_MAX_AGE_FRAMES: u64 = 300;
const STREAMING_PRIORITY_LEVELS: usize = 8;
const MAX_ASYNC_LOAD_QUEUE: usize = 1024;
const HLOD_CLUSTER_RADIUS: f32 = 512.0;
const DATA_LAYER_MAX: usize = 32;
const WORLD_PARTITION_CELL_SIZE: f32 = 512.0;
const PROFILER_HISTORY_FRAMES: usize = 128;
const NORMAL_SMOOTH_EPSILON: f32 = 1e-6;
const BILINEAR_CLAMP_EPSILON: f32 = 1e-5;
const CHUNK_DEPENDENCY_MAX_DEPTH: usize = 16;
const FEEDBACK_BUFFER_MIPS: usize = 8;

// ============================================================
// ENUMS
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LodLevel {
    Unloaded,
    Impostor,
    Low,
    Medium,
    High,
    Ultra,
}

impl LodLevel {
    pub fn index(&self) -> usize {
        match self {
            LodLevel::Unloaded => 0,
            LodLevel::Impostor => 1,
            LodLevel::Low => 2,
            LodLevel::Medium => 3,
            LodLevel::High => 4,
            LodLevel::Ultra => 5,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => LodLevel::Unloaded,
            1 => LodLevel::Impostor,
            2 => LodLevel::Low,
            3 => LodLevel::Medium,
            4 => LodLevel::High,
            5 => LodLevel::Ultra,
            _ => LodLevel::Unloaded,
        }
    }

    pub fn memory_multiplier(&self) -> f32 {
        match self {
            LodLevel::Unloaded => 0.0,
            LodLevel::Impostor => 0.02,
            LodLevel::Low => 0.1,
            LodLevel::Medium => 0.3,
            LodLevel::High => 0.7,
            LodLevel::Ultra => 1.0,
        }
    }

    pub fn vertex_reduction_ratio(&self) -> f32 {
        match self {
            LodLevel::Unloaded => 0.0,
            LodLevel::Impostor => 0.001,
            LodLevel::Low => 0.05,
            LodLevel::Medium => 0.2,
            LodLevel::High => 0.6,
            LodLevel::Ultra => 1.0,
        }
    }

    pub fn next_higher(&self) -> LodLevel {
        match self {
            LodLevel::Unloaded => LodLevel::Impostor,
            LodLevel::Impostor => LodLevel::Low,
            LodLevel::Low => LodLevel::Medium,
            LodLevel::Medium => LodLevel::High,
            LodLevel::High => LodLevel::Ultra,
            LodLevel::Ultra => LodLevel::Ultra,
        }
    }

    pub fn next_lower(&self) -> LodLevel {
        match self {
            LodLevel::Unloaded => LodLevel::Unloaded,
            LodLevel::Impostor => LodLevel::Unloaded,
            LodLevel::Low => LodLevel::Impostor,
            LodLevel::Medium => LodLevel::Low,
            LodLevel::High => LodLevel::Medium,
            LodLevel::Ultra => LodLevel::High,
        }
    }

    pub fn is_loaded(&self) -> bool {
        !matches!(self, LodLevel::Unloaded)
    }

    pub fn screen_space_error_threshold(&self) -> f32 {
        match self {
            LodLevel::Unloaded => f32::MAX,
            LodLevel::Impostor => 64.0,
            LodLevel::Low => 16.0,
            LodLevel::Medium => 4.0,
            LodLevel::High => 1.0,
            LodLevel::Ultra => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkLoadState {
    Unloaded,
    Queued,
    Loading,
    Loaded,
    Evicting,
}

impl ChunkLoadState {
    pub fn can_evict(&self) -> bool {
        matches!(self, ChunkLoadState::Loaded)
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, ChunkLoadState::Queued | ChunkLoadState::Loading)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ChunkLoadState::Loaded | ChunkLoadState::Loading)
    }

    pub fn transition_to_loaded(&self) -> Option<ChunkLoadState> {
        match self {
            ChunkLoadState::Loading => Some(ChunkLoadState::Loaded),
            _ => None,
        }
    }

    pub fn transition_to_unloaded(&self) -> Option<ChunkLoadState> {
        match self {
            ChunkLoadState::Evicting => Some(ChunkLoadState::Unloaded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BvhNodeKind {
    Internal,
    Leaf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    DistanceBased,
    PriorityBased,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataLayerMode {
    Included,
    Excluded,
    Inherited,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StreamingLoadType {
    Synchronous,
    Asynchronous,
    Prefetch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProfilerEventType {
    ChunkLoad,
    ChunkUnload,
    LodSwitch,
    FrustumCull,
    OctreeQuery,
    BvhQuery,
    TerrainStitch,
    ImpostorUpdate,
    VirtualTextureUpdate,
    HlodBuild,
}

// ============================================================
// CHUNK COORDINATE
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Copy)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn from_world_pos(pos: Vec3, chunk_size: f32) -> Self {
        let x = (pos.x / chunk_size).floor() as i32;
        let y = (pos.y / chunk_size).floor() as i32;
        let z = (pos.z / chunk_size).floor() as i32;
        Self { x, y, z }
    }

    pub fn to_world_min(&self, chunk_size: f32) -> Vec3 {
        Vec3::new(
            self.x as f32 * chunk_size,
            self.y as f32 * chunk_size,
            self.z as f32 * chunk_size,
        )
    }

    pub fn to_world_center(&self, chunk_size: f32) -> Vec3 {
        let min = self.to_world_min(chunk_size);
        min + Vec3::splat(chunk_size * 0.5)
    }

    pub fn manhattan_distance(&self, other: &ChunkCoord) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs() + (self.z - other.z).abs()
    }

    pub fn chebyshev_distance(&self, other: &ChunkCoord) -> i32 {
        let dx = (self.x - other.x).abs();
        let dy = (self.y - other.y).abs();
        let dz = (self.z - other.z).abs();
        dx.max(dy).max(dz)
    }

    pub fn euclidean_distance_sq(&self, other: &ChunkCoord) -> i64 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        let dz = (self.z - other.z) as i64;
        dx * dx + dy * dy + dz * dz
    }

    pub fn neighbors_6(&self) -> [ChunkCoord; 6] {
        [
            ChunkCoord::new(self.x + 1, self.y, self.z),
            ChunkCoord::new(self.x - 1, self.y, self.z),
            ChunkCoord::new(self.x, self.y + 1, self.z),
            ChunkCoord::new(self.x, self.y - 1, self.z),
            ChunkCoord::new(self.x, self.y, self.z + 1),
            ChunkCoord::new(self.x, self.y, self.z - 1),
        ]
    }

    pub fn neighbors_26(&self) -> Vec<ChunkCoord> {
        let mut result = Vec::with_capacity(26);
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                for dz in -1i32..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    result.push(ChunkCoord::new(self.x + dx, self.y + dy, self.z + dz));
                }
            }
        }
        result
    }

    pub fn chunks_in_radius(center: &ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
        let mut result = Vec::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    let coord = ChunkCoord::new(center.x + dx, center.y + dy, center.z + dz);
                    if coord.chebyshev_distance(center) <= radius {
                        result.push(coord);
                    }
                }
            }
        }
        result
    }

    pub fn is_adjacent(&self, other: &ChunkCoord) -> bool {
        self.chebyshev_distance(other) == 1
    }

    pub fn offset(&self, dx: i32, dy: i32, dz: i32) -> ChunkCoord {
        ChunkCoord::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub fn pack_u64(&self) -> u64 {
        let xi = (self.x as i64 + 0x0000_8000i64) as u64;
        let yi = (self.y as i64 + 0x0000_8000i64) as u64;
        let zi = (self.z as i64 + 0x0000_8000i64) as u64;
        (xi & 0xFFFF) | ((yi & 0xFFFF) << 16) | ((zi & 0xFFFF) << 32)
    }

    pub fn unpack_u64(packed: u64) -> ChunkCoord {
        let xi = ((packed & 0xFFFF) as i64 - 0x0000_8000i64) as i32;
        let yi = (((packed >> 16) & 0xFFFF) as i64 - 0x0000_8000i64) as i32;
        let zi = (((packed >> 32) & 0xFFFF) as i64 - 0x0000_8000i64) as i32;
        ChunkCoord::new(xi, yi, zi)
    }
}

impl std::fmt::Display for ChunkCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.x, self.y, self.z)
    }
}

// ============================================================
// CHUNK BOUNDS (AABB)
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl ChunkBounds {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Vec3, half_size: Vec3) -> Self {
        Self {
            min: center - half_size,
            max: center + half_size,
        }
    }

    pub fn from_chunk_coord(coord: &ChunkCoord, chunk_size: f32) -> Self {
        let min = coord.to_world_min(chunk_size);
        let max = min + Vec3::splat(chunk_size);
        Self { min, max }
    }

    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x
            && point.y >= self.min.y && point.y <= self.max.y
            && point.z >= self.min.z && point.z <= self.max.z
    }

    pub fn intersects(&self, other: &ChunkBounds) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn expand(&self, amount: f32) -> ChunkBounds {
        let delta = Vec3::splat(amount);
        ChunkBounds {
            min: self.min - delta,
            max: self.max + delta,
        }
    }

    pub fn volume(&self) -> f32 {
        let size = self.size();
        size.x * size.y * size.z
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn surface_area(&self) -> f32 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    pub fn half_size(&self) -> Vec3 {
        self.size() * 0.5
    }

    pub fn merge(&self, other: &ChunkBounds) -> ChunkBounds {
        ChunkBounds {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn intersection(&self, other: &ChunkBounds) -> Option<ChunkBounds> {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max);
        if min.x <= max.x && min.y <= max.y && min.z <= max.z {
            Some(ChunkBounds { min, max })
        } else {
            None
        }
    }

    pub fn distance_sq_to_point(&self, point: Vec3) -> f32 {
        let clamped = point.clamp(self.min, self.max);
        (point - clamped).length_squared()
    }

    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.distance_sq_to_point(point).sqrt()
    }

    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        point.clamp(self.min, self.max)
    }

    pub fn farthest_point(&self, point: Vec3) -> Vec3 {
        let center = self.center();
        let half = self.half_size();
        let dir = point - center;
        let sign = Vec3::new(
            if dir.x >= 0.0 { 1.0 } else { -1.0 },
            if dir.y >= 0.0 { 1.0 } else { -1.0 },
            if dir.z >= 0.0 { 1.0 } else { -1.0 },
        );
        center + half * sign
    }

    pub fn octant_bounds(&self, octant: usize) -> ChunkBounds {
        let center = self.center();
        let (min_x, max_x) = if octant & 1 == 0 { (self.min.x, center.x) } else { (center.x, self.max.x) };
        let (min_y, max_y) = if octant & 2 == 0 { (self.min.y, center.y) } else { (center.y, self.max.y) };
        let (min_z, max_z) = if octant & 4 == 0 { (self.min.z, center.z) } else { (center.z, self.max.z) };
        ChunkBounds {
            min: Vec3::new(min_x, min_y, min_z),
            max: Vec3::new(max_x, max_y, max_z),
        }
    }

    pub fn octant_for_point(&self, point: Vec3) -> usize {
        let center = self.center();
        let mut octant = 0usize;
        if point.x > center.x { octant |= 1; }
        if point.y > center.y { octant |= 2; }
        if point.z > center.z { octant |= 4; }
        octant
    }

    pub fn transformed_by(&self, transform: Mat4) -> ChunkBounds {
        let corners = self.corners();
        let mut new_min = Vec3::splat(f32::MAX);
        let mut new_max = Vec3::splat(f32::MIN);
        for corner in &corners {
            let transformed = transform.transform_point3(*corner);
            new_min = new_min.min(transformed);
            new_max = new_max.max(transformed);
        }
        ChunkBounds { min: new_min, max: new_max }
    }

    pub fn corners(&self) -> [Vec3; 8] {
        [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }

    pub fn is_degenerate(&self) -> bool {
        let size = self.size();
        size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0
    }

    pub fn scale(&self, factor: f32) -> ChunkBounds {
        let center = self.center();
        let half = self.half_size() * factor;
        ChunkBounds {
            min: center - half,
            max: center + half,
        }
    }
}

// ============================================================
// STREAMING CONFIG
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingConfig {
    pub max_loaded_chunks: usize,
    pub lod_distances: [f32; 5],
    pub max_memory_mb: u64,
    pub chunk_size: f32,
    pub streaming_radius: f32,
    pub vertical_streaming_radius: f32,
    pub enable_frustum_culling: bool,
    pub enable_occlusion_culling: bool,
    pub enable_hlod: bool,
    pub enable_virtual_textures: bool,
    pub enable_impostor_billboards: bool,
    pub max_concurrent_loads: usize,
    pub max_concurrent_unloads: usize,
    pub lod_bias: f32,
    pub screen_height_pixels: u32,
    pub fov_vertical_rad: f32,
    pub eviction_policy: EvictionPolicy,
    pub prefetch_distance: f32,
    pub min_lod_retain_frames: u64,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            max_loaded_chunks: DEFAULT_MAX_LOADED_CHUNKS,
            lod_distances: LOD_DISTANCES,
            max_memory_mb: DEFAULT_MAX_MEMORY_MB,
            chunk_size: DEFAULT_CHUNK_SIZE,
            streaming_radius: DEFAULT_STREAMING_RADIUS,
            vertical_streaming_radius: DEFAULT_STREAMING_RADIUS * 0.5,
            enable_frustum_culling: true,
            enable_occlusion_culling: false,
            enable_hlod: true,
            enable_virtual_textures: true,
            enable_impostor_billboards: true,
            max_concurrent_loads: 4,
            max_concurrent_unloads: 2,
            lod_bias: 0.0,
            screen_height_pixels: 1080,
            fov_vertical_rad: std::f32::consts::FRAC_PI_4,
            eviction_policy: EvictionPolicy::Lru,
            prefetch_distance: 1.5,
            min_lod_retain_frames: 10,
        }
    }
}

impl StreamingConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_quality_preset(preset: QualityPreset) -> Self {
        let mut cfg = Self::default();
        match preset {
            QualityPreset::Low => {
                cfg.max_loaded_chunks = 64;
                cfg.lod_distances = [32.0, 64.0, 128.0, 256.0, 512.0];
                cfg.max_memory_mb = 1024;
                cfg.max_concurrent_loads = 2;
                cfg.enable_hlod = false;
                cfg.enable_virtual_textures = false;
            }
            QualityPreset::Medium => {
                cfg.max_loaded_chunks = 128;
                cfg.lod_distances = [48.0, 96.0, 192.0, 384.0, 768.0];
                cfg.max_memory_mb = 2048;
                cfg.max_concurrent_loads = 3;
            }
            QualityPreset::High => {
                cfg.max_loaded_chunks = 256;
                cfg.max_concurrent_loads = 6;
                cfg.enable_occlusion_culling = true;
            }
            QualityPreset::Ultra => {
                cfg.max_loaded_chunks = 512;
                cfg.lod_distances = [96.0, 192.0, 384.0, 768.0, 1536.0];
                cfg.max_memory_mb = 8192;
                cfg.max_concurrent_loads = 8;
                cfg.enable_occlusion_culling = true;
            }
        }
        cfg
    }

    pub fn lod_for_distance(&self, dist: f32) -> LodLevel {
        let biased = dist * (1.0 + self.lod_bias);
        if biased < self.lod_distances[0] { LodLevel::Ultra }
        else if biased < self.lod_distances[1] { LodLevel::High }
        else if biased < self.lod_distances[2] { LodLevel::Medium }
        else if biased < self.lod_distances[3] { LodLevel::Low }
        else if biased < self.lod_distances[4] { LodLevel::Impostor }
        else { LodLevel::Unloaded }
    }

    pub fn streaming_radius_chunks(&self) -> i32 {
        (self.streaming_radius / self.chunk_size).ceil() as i32
    }

    pub fn memory_budget_per_lod(&self, lod: &LodLevel) -> u64 {
        let ratio = lod.memory_multiplier();
        (self.max_memory_mb as f32 * ratio * 0.3) as u64
    }
}

#[derive(Debug, Clone)]
pub enum QualityPreset {
    Low,
    Medium,
    High,
    Ultra,
}

// ============================================================
// STREAMING CAMERA
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingCamera {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    pub fov_deg: f32,
    pub near: f32,
    pub far: f32,
    pub aspect_ratio: f32,
    pub frustum_planes: [Vec4; 6],
    pub view_matrix: Mat4,
    pub proj_matrix: Mat4,
    pub view_proj_matrix: Mat4,
}

impl StreamingCamera {
    pub fn new(position: Vec3, target: Vec3, up: Vec3, fov_deg: f32, aspect: f32, near: f32, far: f32) -> Self {
        let forward = (target - position).normalize();
        let right = forward.cross(up).normalize();
        let up_corrected = right.cross(forward).normalize();
        let view = Mat4::look_at_rh(position, target, up_corrected);
        let proj = Mat4::perspective_rh(fov_deg.to_radians(), aspect, near, far);
        let view_proj = proj * view;
        let mut cam = Self {
            position,
            forward,
            up: up_corrected,
            right,
            fov_deg,
            near,
            far,
            aspect_ratio: aspect,
            frustum_planes: [Vec4::ZERO; 6],
            view_matrix: view,
            proj_matrix: proj,
            view_proj_matrix: view_proj,
        };
        cam.extract_frustum_planes();
        cam
    }

    pub fn extract_frustum_planes(&mut self) {
        let m = self.view_proj_matrix;
        let rows = [
            Vec4::new(m.col(0).x, m.col(1).x, m.col(2).x, m.col(3).x),
            Vec4::new(m.col(0).y, m.col(1).y, m.col(2).y, m.col(3).y),
            Vec4::new(m.col(0).z, m.col(1).z, m.col(2).z, m.col(3).z),
            Vec4::new(m.col(0).w, m.col(1).w, m.col(2).w, m.col(3).w),
        ];
        // Near: row3 + row2
        self.frustum_planes[FRUSTUM_NEAR_PLANE] = rows[3] + rows[2];
        // Far: row3 - row2
        self.frustum_planes[FRUSTUM_FAR_PLANE]  = rows[3] - rows[2];
        // Left: row3 + row0
        self.frustum_planes[FRUSTUM_LEFT_PLANE]  = rows[3] + rows[0];
        // Right: row3 - row0
        self.frustum_planes[FRUSTUM_RIGHT_PLANE] = rows[3] - rows[0];
        // Top: row3 - row1
        self.frustum_planes[FRUSTUM_TOP_PLANE]   = rows[3] - rows[1];
        // Bottom: row3 + row1
        self.frustum_planes[FRUSTUM_BOTTOM_PLANE]= rows[3] + rows[1];
        // Normalize each plane
        for plane in &mut self.frustum_planes {
            let len = Vec3::new(plane.x, plane.y, plane.z).length();
            if len > 1e-8 {
                *plane /= len;
            }
        }
    }

    pub fn update_position(&mut self, new_pos: Vec3, new_target: Vec3) {
        self.position = new_pos;
        self.forward = (new_target - new_pos).normalize();
        let world_up = Vec3::Y;
        self.right = self.forward.cross(world_up).normalize();
        self.up = self.right.cross(self.forward).normalize();
        self.view_matrix = Mat4::look_at_rh(new_pos, new_target, self.up);
        self.view_proj_matrix = self.proj_matrix * self.view_matrix;
        self.extract_frustum_planes();
    }

    pub fn project_sphere_to_screen(&self, center: Vec3, radius: f32, screen_height: f32) -> f32 {
        let dist = (center - self.position).length();
        if dist <= radius { return screen_height; }
        let fov_rad = self.fov_deg.to_radians();
        let proj_radius = (radius / dist) / (fov_rad * 0.5).tan();
        proj_radius * screen_height
    }

    pub fn compute_lod_screen_size(&self, bounds: &ChunkBounds, screen_height: f32) -> f32 {
        let center = bounds.center();
        let radius = bounds.half_size().length();
        self.project_sphere_to_screen(center, radius, screen_height)
    }

    pub fn distance_to_bounds(&self, bounds: &ChunkBounds) -> f32 {
        bounds.distance_to_point(self.position)
    }
}

// ============================================================
// FRUSTUM CULLING
// ============================================================

#[derive(Debug, Clone)]
pub struct FrustumCulling {
    pub planes: [Vec4; 6],
}

impl FrustumCulling {
    pub fn new(planes: [Vec4; 6]) -> Self {
        Self { planes }
    }

    pub fn from_view_proj(view_proj: Mat4) -> Self {
        let mut planes = [Vec4::ZERO; 6];
        let m = view_proj;
        let rows = [
            Vec4::new(m.col(0).x, m.col(1).x, m.col(2).x, m.col(3).x),
            Vec4::new(m.col(0).y, m.col(1).y, m.col(2).y, m.col(3).y),
            Vec4::new(m.col(0).z, m.col(1).z, m.col(2).z, m.col(3).z),
            Vec4::new(m.col(0).w, m.col(1).w, m.col(2).w, m.col(3).w),
        ];
        planes[FRUSTUM_NEAR_PLANE]   = rows[3] + rows[2];
        planes[FRUSTUM_FAR_PLANE]    = rows[3] - rows[2];
        planes[FRUSTUM_LEFT_PLANE]   = rows[3] + rows[0];
        planes[FRUSTUM_RIGHT_PLANE]  = rows[3] - rows[0];
        planes[FRUSTUM_TOP_PLANE]    = rows[3] - rows[1];
        planes[FRUSTUM_BOTTOM_PLANE] = rows[3] + rows[1];
        for plane in &mut planes {
            let len = Vec3::new(plane.x, plane.y, plane.z).length();
            if len > 1e-8 { *plane /= len; }
        }
        Self { planes }
    }

    pub fn test_aabb(&self, bounds: &ChunkBounds) -> FrustumResult {
        let mut result = FrustumResult::Inside;
        for plane in &self.planes {
            let normal = Vec3::new(plane.x, plane.y, plane.z);
            let d = plane.w;
            // positive vertex: the corner that is most in direction of normal
            let px = if normal.x >= 0.0 { bounds.max.x } else { bounds.min.x };
            let py = if normal.y >= 0.0 { bounds.max.y } else { bounds.min.y };
            let pz = if normal.z >= 0.0 { bounds.max.z } else { bounds.min.z };
            let p_vert = Vec3::new(px, py, pz);
            // negative vertex
            let nx_v = if normal.x >= 0.0 { bounds.min.x } else { bounds.max.x };
            let ny_v = if normal.y >= 0.0 { bounds.min.y } else { bounds.max.y };
            let nz_v = if normal.z >= 0.0 { bounds.min.z } else { bounds.max.z };
            let n_vert = Vec3::new(nx_v, ny_v, nz_v);

            if normal.dot(p_vert) + d < 0.0 {
                return FrustumResult::Outside;
            }
            if normal.dot(n_vert) + d < 0.0 {
                result = FrustumResult::Intersects;
            }
        }
        result
    }

    pub fn test_sphere(&self, center: Vec3, radius: f32) -> FrustumResult {
        let mut result = FrustumResult::Inside;
        for plane in &self.planes {
            let normal = Vec3::new(plane.x, plane.y, plane.z);
            let dist = normal.dot(center) + plane.w;
            if dist < -radius { return FrustumResult::Outside; }
            if dist < radius  { result = FrustumResult::Intersects; }
        }
        result
    }

    pub fn test_point(&self, point: Vec3) -> bool {
        for plane in &self.planes {
            let normal = Vec3::new(plane.x, plane.y, plane.z);
            if normal.dot(point) + plane.w < 0.0 { return false; }
        }
        true
    }

    pub fn test_aabb_fast(&self, bounds: &ChunkBounds) -> bool {
        for plane in &self.planes {
            let normal = Vec3::new(plane.x, plane.y, plane.z);
            let d = plane.w;
            let px = if normal.x >= 0.0 { bounds.max.x } else { bounds.min.x };
            let py = if normal.y >= 0.0 { bounds.max.y } else { bounds.min.y };
            let pz = if normal.z >= 0.0 { bounds.max.z } else { bounds.min.z };
            if normal.dot(Vec3::new(px, py, pz)) + d < 0.0 {
                return false;
            }
        }
        true
    }

    pub fn compute_visibility_mask(&self, bounds_list: &[ChunkBounds]) -> Vec<bool> {
        bounds_list.iter().map(|b| self.test_aabb_fast(b)).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrustumResult {
    Inside,
    Outside,
    Intersects,
}

// ============================================================
// OCTREE
// ============================================================

#[derive(Debug, Clone)]
pub struct OctreeNode {
    pub bounds: ChunkBounds,
    pub children: Option<Box<[OctreeNode; 8]>>,
    pub objects: Vec<u32>,
    pub depth: u32,
}

impl OctreeNode {
    pub fn new(bounds: ChunkBounds, depth: u32) -> Self {
        Self {
            bounds,
            children: None,
            objects: Vec::new(),
            depth,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn total_object_count(&self) -> usize {
        let mut count = self.objects.len();
        if let Some(children) = &self.children {
            for child in children.iter() {
                count += child.total_object_count();
            }
        }
        count
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn max_depth(&self) -> u32 {
        if let Some(children) = &self.children {
            children.iter().map(|c| c.max_depth()).max().unwrap_or(self.depth)
        } else {
            self.depth
        }
    }

    pub fn query_sphere(&self, center: Vec3, radius: f32, result: &mut Vec<u32>) {
        let dist_sq = self.bounds.distance_sq_to_point(center);
        if dist_sq > radius * radius { return; }
        result.extend_from_slice(&self.objects);
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.query_sphere(center, radius, result);
            }
        }
    }

    pub fn query_aabb(&self, query: &ChunkBounds, result: &mut Vec<u32>) {
        if !self.bounds.intersects(query) { return; }
        result.extend_from_slice(&self.objects);
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.query_aabb(query, result);
            }
        }
    }

    pub fn query_frustum(&self, frustum: &FrustumCulling, result: &mut Vec<u32>) {
        let fr = frustum.test_aabb(&self.bounds);
        match fr {
            FrustumResult::Outside => return,
            FrustumResult::Inside => {
                self.collect_all(result);
                return;
            }
            FrustumResult::Intersects => {
                result.extend_from_slice(&self.objects);
                if let Some(children) = &self.children {
                    for child in children.iter() {
                        child.query_frustum(frustum, result);
                    }
                }
            }
        }
    }

    pub fn collect_all(&self, result: &mut Vec<u32>) {
        result.extend_from_slice(&self.objects);
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.collect_all(result);
            }
        }
    }

    pub fn node_count(&self) -> usize {
        let mut count = 1;
        if let Some(children) = &self.children {
            for child in children.iter() {
                count += child.node_count();
            }
        }
        count
    }
}

// ============================================================
// OCTREE BUILDER
// ============================================================

#[derive(Debug, Clone)]
pub struct OctreeBuilder {
    pub max_depth: u32,
    pub max_per_node: usize,
}

impl OctreeBuilder {
    pub fn new(max_depth: u32, max_per_node: usize) -> Self {
        Self { max_depth, max_per_node }
    }

    pub fn build(&self, points: &[(u32, Vec3)]) -> OctreeNode {
        if points.is_empty() {
            return OctreeNode::new(
                ChunkBounds::new(Vec3::ZERO, Vec3::ZERO),
                0,
            );
        }
        let bounds = self.compute_bounds(points);
        let mut root = OctreeNode::new(bounds, 0);
        for &(id, pos) in points {
            self.insert(&mut root, id, pos);
        }
        root
    }

    fn compute_bounds(&self, points: &[(u32, Vec3)]) -> ChunkBounds {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for &(_, pos) in points {
            min = min.min(pos);
            max = max.max(pos);
        }
        let padding = Vec3::splat(0.001);
        ChunkBounds { min: min - padding, max: max + padding }
    }

    pub fn insert(&self, node: &mut OctreeNode, id: u32, pos: Vec3) {
        if !node.bounds.contains(pos) { return; }

        if node.is_leaf() {
            if node.objects.len() < self.max_per_node || node.depth >= self.max_depth {
                node.objects.push(id);
            } else {
                self.subdivide(node);
                self.insert_into_children(node, id, pos);
            }
        } else {
            self.insert_into_children(node, id, pos);
        }
    }

    fn insert_into_children(&self, node: &mut OctreeNode, id: u32, pos: Vec3) {
        let octant = node.bounds.octant_for_point(pos);
        if let Some(children) = &mut node.children {
            self.insert(&mut children[octant], id, pos);
        }
    }

    fn subdivide(&self, node: &mut OctreeNode) {
        let child_depth = node.depth + 1;
        let children: [OctreeNode; 8] = std::array::from_fn(|i| {
            let bounds = node.bounds.octant_bounds(i);
            OctreeNode::new(bounds, child_depth)
        });
        node.children = Some(Box::new(children));

        let existing = std::mem::take(&mut node.objects);
        for id in existing {
            // We need the position to re-insert; for now push to first matching child
            // In a real impl, we'd store positions too — so we check bounds inclusion
            // by placing them in root's objects as fallback for objects at exact center
            node.objects.push(id);
        }
    }

    pub fn build_with_positions(&self, points: &[(u32, Vec3)]) -> (OctreeNode, HashMap<u32, Vec3>) {
        let mut pos_map = HashMap::new();
        for &(id, pos) in points {
            pos_map.insert(id, pos);
        }
        let bounds = if points.is_empty() {
            ChunkBounds::new(Vec3::ZERO, Vec3::ONE)
        } else {
            self.compute_bounds(points)
        };
        let mut root = OctreeNode::new(bounds, 0);
        for &(id, pos) in points {
            self.insert_with_pos(&mut root, id, pos, &pos_map);
        }
        (root, pos_map)
    }

    fn insert_with_pos(&self, node: &mut OctreeNode, id: u32, pos: Vec3, pos_map: &HashMap<u32, Vec3>) {
        if !node.bounds.contains(pos) { return; }
        if node.is_leaf() {
            if node.objects.len() < self.max_per_node || node.depth >= self.max_depth {
                node.objects.push(id);
            } else {
                self.subdivide_with_pos(node, pos_map);
                let octant = node.bounds.octant_for_point(pos);
                if let Some(children) = &mut node.children {
                    self.insert_with_pos(&mut children[octant], id, pos, pos_map);
                }
            }
        } else {
            let octant = node.bounds.octant_for_point(pos);
            if let Some(children) = &mut node.children {
                self.insert_with_pos(&mut children[octant], id, pos, pos_map);
            }
        }
    }

    fn subdivide_with_pos(&self, node: &mut OctreeNode, pos_map: &HashMap<u32, Vec3>) {
        let child_depth = node.depth + 1;
        let children: [OctreeNode; 8] = std::array::from_fn(|i| {
            OctreeNode::new(node.bounds.octant_bounds(i), child_depth)
        });
        node.children = Some(Box::new(children));
        let existing = std::mem::take(&mut node.objects);
        for id in existing {
            if let Some(&pos) = pos_map.get(&id) {
                let octant = node.bounds.octant_for_point(pos);
                if let Some(children) = &mut node.children {
                    children[octant].objects.push(id);
                }
            } else {
                node.objects.push(id);
            }
        }
    }
}

// ============================================================
// BVH NODE
// ============================================================

#[derive(Debug, Clone)]
pub struct BvhNode {
    pub bounds: ChunkBounds,
    pub kind: BvhNodeKind,
    pub left: Option<Box<BvhNode>>,
    pub right: Option<Box<BvhNode>>,
    pub objects: Vec<u32>,
    pub parent_index: Option<usize>,
}

impl BvhNode {
    pub fn new_leaf(bounds: ChunkBounds, objects: Vec<u32>) -> Self {
        Self {
            bounds,
            kind: BvhNodeKind::Leaf,
            left: None,
            right: None,
            objects,
            parent_index: None,
        }
    }

    pub fn new_internal(bounds: ChunkBounds, left: BvhNode, right: BvhNode) -> Self {
        Self {
            bounds,
            kind: BvhNodeKind::Internal,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
            objects: Vec::new(),
            parent_index: None,
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self.kind, BvhNodeKind::Leaf)
    }

    pub fn depth(&self) -> usize {
        match (&self.left, &self.right) {
            (Some(l), Some(r)) => 1 + l.depth().max(r.depth()),
            (Some(l), None) => 1 + l.depth(),
            (None, Some(r)) => 1 + r.depth(),
            (None, None) => 0,
        }
    }

    pub fn node_count(&self) -> usize {
        let mut count = 1;
        if let Some(l) = &self.left  { count += l.node_count(); }
        if let Some(r) = &self.right { count += r.node_count(); }
        count
    }

    pub fn query_ray(&self, origin: Vec3, dir: Vec3, t_min: f32, t_max: f32, result: &mut Vec<u32>) {
        if !ray_aabb_intersect(origin, dir, &self.bounds, t_min, t_max) { return; }
        if self.is_leaf() {
            result.extend_from_slice(&self.objects);
            return;
        }
        if let Some(l) = &self.left  { l.query_ray(origin, dir, t_min, t_max, result); }
        if let Some(r) = &self.right { r.query_ray(origin, dir, t_min, t_max, result); }
    }

    pub fn query_aabb(&self, query: &ChunkBounds, result: &mut Vec<u32>) {
        if !self.bounds.intersects(query) { return; }
        if self.is_leaf() {
            result.extend_from_slice(&self.objects);
            return;
        }
        if let Some(l) = &self.left  { l.query_aabb(query, result); }
        if let Some(r) = &self.right { r.query_aabb(query, result); }
    }

    pub fn query_frustum(&self, frustum: &FrustumCulling, result: &mut Vec<u32>) {
        match frustum.test_aabb(&self.bounds) {
            FrustumResult::Outside => {}
            FrustumResult::Inside => { self.collect_all(result); }
            FrustumResult::Intersects => {
                if self.is_leaf() {
                    result.extend_from_slice(&self.objects);
                } else {
                    if let Some(l) = &self.left  { l.query_frustum(frustum, result); }
                    if let Some(r) = &self.right { r.query_frustum(frustum, result); }
                }
            }
        }
    }

    fn collect_all(&self, result: &mut Vec<u32>) {
        result.extend_from_slice(&self.objects);
        if let Some(l) = &self.left  { l.collect_all(result); }
        if let Some(r) = &self.right { r.collect_all(result); }
    }
}

// ============================================================
// BVH BUILDER (SAH)
// ============================================================

#[derive(Debug, Clone)]
pub struct BvhBuilder {
    pub max_leaf_objects: usize,
    pub num_bins: usize,
}

impl BvhBuilder {
    pub fn new(max_leaf_objects: usize, num_bins: usize) -> Self {
        Self { max_leaf_objects, num_bins }
    }

    pub fn build(&self, objects: &[(u32, ChunkBounds)]) -> Option<BvhNode> {
        if objects.is_empty() { return None; }
        let indices: Vec<usize> = (0..objects.len()).collect();
        Some(self.build_recursive(objects, &indices))
    }

    fn build_recursive(&self, objects: &[(u32, ChunkBounds)], indices: &[usize]) -> BvhNode {
        if indices.len() <= self.max_leaf_objects {
            return self.make_leaf(objects, indices);
        }

        let node_bounds = self.compute_union_bounds(objects, indices);
        let (split_axis, split_pos, split_cost) = self.find_best_split(objects, indices, &node_bounds);

        // Check if splitting is worth it vs making a leaf
        let leaf_cost = SAH_INTERSECTION_COST * indices.len() as f32;
        if split_cost >= leaf_cost {
            return self.make_leaf(objects, indices);
        }

        let (left_indices, right_indices) = self.partition(objects, indices, split_axis, split_pos);

        if left_indices.is_empty() || right_indices.is_empty() {
            return self.make_leaf(objects, indices);
        }

        let left  = self.build_recursive(objects, &left_indices);
        let right = self.build_recursive(objects, &right_indices);
        BvhNode::new_internal(node_bounds, left, right)
    }

    fn find_best_split(
        &self,
        objects: &[(u32, ChunkBounds)],
        indices: &[usize],
        node_bounds: &ChunkBounds,
    ) -> (usize, f32, f32) {
        let node_sa = node_bounds.surface_area();
        let mut best_cost = f32::MAX;
        let mut best_axis = 0;
        let mut best_split = 0.0f32;

        let size = node_bounds.size();

        for axis in 0..3 {
            let axis_len = match axis { 0 => size.x, 1 => size.y, _ => size.z };
            if axis_len < 1e-8 { continue; }

            let axis_min = match axis { 0 => node_bounds.min.x, 1 => node_bounds.min.y, _ => node_bounds.min.z };

            for bin in 1..self.num_bins {
                let t = bin as f32 / self.num_bins as f32;
                let split_pos = axis_min + t * axis_len;

                let mut left_bounds: Option<ChunkBounds> = None;
                let mut right_bounds: Option<ChunkBounds> = None;
                let mut left_count = 0usize;
                let mut right_count = 0usize;

                for &idx in indices {
                    let center = objects[idx].1.center();
                    let coord = match axis { 0 => center.x, 1 => center.y, _ => center.z };
                    if coord < split_pos {
                        left_count += 1;
                        left_bounds = Some(match left_bounds {
                            Some(b) => b.merge(&objects[idx].1),
                            None => objects[idx].1.clone(),
                        });
                    } else {
                        right_count += 1;
                        right_bounds = Some(match right_bounds {
                            Some(b) => b.merge(&objects[idx].1),
                            None => objects[idx].1.clone(),
                        });
                    }
                }

                if left_count == 0 || right_count == 0 { continue; }

                let left_sa  = left_bounds.map(|b| b.surface_area()).unwrap_or(0.0);
                let right_sa = right_bounds.map(|b| b.surface_area()).unwrap_or(0.0);

                let cost = SAH_TRAVERSAL_COST
                    + SAH_INTERSECTION_COST * (
                        left_sa  / node_sa * left_count  as f32
                      + right_sa / node_sa * right_count as f32
                    );

                if cost < best_cost {
                    best_cost = cost;
                    best_axis = axis;
                    best_split = split_pos;
                }
            }
        }
        (best_axis, best_split, best_cost)
    }

    fn partition(
        &self,
        objects: &[(u32, ChunkBounds)],
        indices: &[usize],
        axis: usize,
        split: f32,
    ) -> (Vec<usize>, Vec<usize>) {
        let mut left  = Vec::new();
        let mut right = Vec::new();
        for &idx in indices {
            let center = objects[idx].1.center();
            let coord = match axis { 0 => center.x, 1 => center.y, _ => center.z };
            if coord < split { left.push(idx); } else { right.push(idx); }
        }
        (left, right)
    }

    fn make_leaf(&self, objects: &[(u32, ChunkBounds)], indices: &[usize]) -> BvhNode {
        let bounds = self.compute_union_bounds(objects, indices);
        let ids: Vec<u32> = indices.iter().map(|&i| objects[i].0).collect();
        BvhNode::new_leaf(bounds, ids)
    }

    fn compute_union_bounds(&self, objects: &[(u32, ChunkBounds)], indices: &[usize]) -> ChunkBounds {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for &idx in indices {
            min = min.min(objects[idx].1.min);
            max = max.max(objects[idx].1.max);
        }
        ChunkBounds { min, max }
    }
}

// ============================================================
// RAY-AABB INTERSECTION HELPER
// ============================================================

pub fn ray_aabb_intersect(origin: Vec3, dir: Vec3, bounds: &ChunkBounds, t_min: f32, t_max: f32) -> bool {
    let inv_dir = Vec3::new(
        if dir.x.abs() > 1e-12 { 1.0 / dir.x } else { f32::MAX },
        if dir.y.abs() > 1e-12 { 1.0 / dir.y } else { f32::MAX },
        if dir.z.abs() > 1e-12 { 1.0 / dir.z } else { f32::MAX },
    );
    let t1 = (bounds.min - origin) * inv_dir;
    let t2 = (bounds.max - origin) * inv_dir;
    let tmin = t1.min(t2);
    let tmax = t1.max(t2);
    let enter = tmin.x.max(tmin.y).max(tmin.z).max(t_min);
    let exit  = tmax.x.min(tmax.y).min(tmax.z).min(t_max);
    enter <= exit
}

// ============================================================
// STREAMING CHUNK
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingChunk {
    pub coord: ChunkCoord,
    pub bounds: ChunkBounds,
    pub lod_level: LodLevel,
    pub load_state: ChunkLoadState,
    pub resident_objects: Vec<u32>,
    pub memory_bytes: u64,
    pub last_visible_frame: u64,
    pub last_loaded_frame: u64,
    pub load_priority: f32,
    pub distance_to_viewer: f32,
    pub screen_space_size: f32,
    pub is_visible: bool,
    pub dependencies: Vec<ChunkCoord>,
    pub hlod_cluster_id: Option<u32>,
    pub data_layer_mask: u32,
    pub version: u32,
    pub flags: ChunkFlags,
}

#[derive(Debug, Clone, Default)]
pub struct ChunkFlags {
    pub dirty: bool,
    pub needs_lod_update: bool,
    pub needs_terrain_stitch: bool,
    pub impostor_valid: bool,
    pub heightmap_loaded: bool,
    pub collision_loaded: bool,
    pub nav_mesh_loaded: bool,
}

impl StreamingChunk {
    pub fn new(coord: ChunkCoord, chunk_size: f32) -> Self {
        let bounds = ChunkBounds::from_chunk_coord(&coord, chunk_size);
        Self {
            coord,
            bounds,
            lod_level: LodLevel::Unloaded,
            load_state: ChunkLoadState::Unloaded,
            resident_objects: Vec::new(),
            memory_bytes: 0,
            last_visible_frame: 0,
            last_loaded_frame: 0,
            load_priority: 0.0,
            distance_to_viewer: f32::MAX,
            screen_space_size: 0.0,
            is_visible: false,
            dependencies: Vec::new(),
            hlod_cluster_id: None,
            data_layer_mask: 0xFFFF_FFFF,
            version: 0,
            flags: ChunkFlags::default(),
        }
    }

    pub fn update_distance(&mut self, viewer_pos: Vec3) {
        self.distance_to_viewer = self.bounds.distance_to_point(viewer_pos);
    }

    pub fn compute_load_priority(&mut self, viewer_pos: Vec3, viewer_forward: Vec3) -> f32 {
        let center = self.bounds.center();
        let to_chunk = (center - viewer_pos).normalize_or_zero();
        let dot = viewer_forward.dot(to_chunk).max(0.0);
        let dist_factor = 1.0 / (1.0 + self.distance_to_viewer * 0.01);
        let facing_factor = 0.5 + 0.5 * dot;
        let priority = dist_factor * facing_factor * (if self.is_visible { 2.0 } else { 1.0 });
        self.load_priority = priority;
        priority
    }

    pub fn estimate_memory_for_lod(&self, lod: &LodLevel) -> u64 {
        let base_mb = 8u64;
        let object_mb = self.resident_objects.len() as u64 * 2;
        let total_mb = (base_mb + object_mb) as f32 * lod.memory_multiplier();
        (total_mb * 1024.0 * 1024.0) as u64
    }

    pub fn can_load(&self) -> bool {
        matches!(self.load_state, ChunkLoadState::Unloaded | ChunkLoadState::Queued)
    }

    pub fn can_evict(&self) -> bool {
        self.load_state.can_evict() && self.lod_level.is_loaded()
    }

    pub fn age_frames(&self, current_frame: u64) -> u64 {
        current_frame.saturating_sub(self.last_visible_frame)
    }

    pub fn needs_lod_upgrade(&self, desired_lod: &LodLevel) -> bool {
        self.lod_level < *desired_lod
    }

    pub fn needs_lod_downgrade(&self, desired_lod: &LodLevel) -> bool {
        self.lod_level > *desired_lod && self.lod_level != LodLevel::Unloaded
    }

    pub fn mark_visible(&mut self, frame: u64) {
        self.is_visible = true;
        self.last_visible_frame = frame;
    }

    pub fn mark_not_visible(&mut self) {
        self.is_visible = false;
    }

    pub fn add_object(&mut self, object_id: u32) {
        if !self.resident_objects.contains(&object_id) {
            self.resident_objects.push(object_id);
        }
    }

    pub fn remove_object(&mut self, object_id: u32) {
        self.resident_objects.retain(|&id| id != object_id);
    }

    pub fn is_dependency_satisfied(&self, loaded_chunks: &HashSet<ChunkCoord>) -> bool {
        self.dependencies.iter().all(|dep| loaded_chunks.contains(dep))
    }
}

// ============================================================
// STREAMING PRIORITY QUEUE
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingPriority {
    pub queue: BTreeMap<OrderedFloat, ChunkCoord>,
    pub coord_to_priority: HashMap<ChunkCoord, f32>,
    pub max_size: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct OrderedFloat(f32);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl StreamingPriority {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: BTreeMap::new(),
            coord_to_priority: HashMap::new(),
            max_size,
        }
    }

    pub fn push(&mut self, coord: ChunkCoord, priority: f32) {
        if let Some(&old_priority) = self.coord_to_priority.get(&coord) {
            self.queue.remove(&OrderedFloat(old_priority));
        }
        // Use negative priority so highest-priority items come first (BTreeMap is ascending)
        self.queue.insert(OrderedFloat(-priority), coord.clone());
        self.coord_to_priority.insert(coord, priority);
    }

    pub fn pop_highest(&mut self) -> Option<(ChunkCoord, f32)> {
        if let Some((key, coord)) = self.queue.pop_first() {
            let priority = -key.0;
            self.coord_to_priority.remove(&coord);
            Some((coord, priority))
        } else {
            None
        }
    }

    pub fn peek_highest(&self) -> Option<(&ChunkCoord, f32)> {
        self.queue.iter().next().map(|(k, v)| (v, -k.0))
    }

    pub fn contains(&self, coord: &ChunkCoord) -> bool {
        self.coord_to_priority.contains_key(coord)
    }

    pub fn remove(&mut self, coord: &ChunkCoord) {
        if let Some(priority) = self.coord_to_priority.remove(coord) {
            self.queue.remove(&OrderedFloat(-priority));
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn update_priorities(&mut self, viewer_pos: Vec3, chunks: &HashMap<ChunkCoord, StreamingChunk>) {
        let coords: Vec<ChunkCoord> = self.coord_to_priority.keys().cloned().collect();
        for coord in coords {
            if let Some(chunk) = chunks.get(&coord) {
                let priority = 1.0 / (1.0 + chunk.distance_to_viewer);
                self.push(coord, priority);
            }
        }
    }

    pub fn drain_up_to(&mut self, n: usize) -> Vec<(ChunkCoord, f32)> {
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(item) = self.pop_highest() {
                result.push(item);
            } else {
                break;
            }
        }
        result
    }

    pub fn recompute_all(&mut self, viewer_pos: Vec3, chunk_size: f32) {
        let coords: Vec<ChunkCoord> = self.coord_to_priority.keys().cloned().collect();
        let old_queue = std::mem::take(&mut self.queue);
        self.coord_to_priority.clear();
        for coord in coords {
            let center = coord.to_world_center(chunk_size);
            let dist = (center - viewer_pos).length();
            let priority = 1.0 / (1.0 + dist * 0.01);
            self.push(coord, priority);
        }
    }
}

// ============================================================
// MEMORY BUDGET
// ============================================================

#[derive(Debug, Clone)]
pub struct MemoryBudget {
    pub max_bytes: u64,
    pub used_bytes: u64,
    pub lod_usage: HashMap<LodLevel, u64>,
    pub chunk_memory: HashMap<ChunkCoord, u64>,
    pub lru_order: VecDeque<ChunkCoord>,
    pub access_count: HashMap<ChunkCoord, u64>,
    pub last_access_frame: HashMap<ChunkCoord, u64>,
    pub policy: EvictionPolicy,
}

impl MemoryBudget {
    pub fn new(max_mb: u64, policy: EvictionPolicy) -> Self {
        Self {
            max_bytes: max_mb * 1024 * 1024,
            used_bytes: 0,
            lod_usage: HashMap::new(),
            chunk_memory: HashMap::new(),
            lru_order: VecDeque::new(),
            access_count: HashMap::new(),
            last_access_frame: HashMap::new(),
            policy,
        }
    }

    pub fn available_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.used_bytes)
    }

    pub fn usage_ratio(&self) -> f32 {
        self.used_bytes as f32 / self.max_bytes as f32
    }

    pub fn can_allocate(&self, bytes: u64) -> bool {
        self.used_bytes + bytes <= self.max_bytes
    }

    pub fn allocate(&mut self, coord: ChunkCoord, lod: LodLevel, bytes: u64, frame: u64) -> bool {
        if !self.can_allocate(bytes) { return false; }
        self.used_bytes += bytes;
        *self.lod_usage.entry(lod).or_insert(0) += bytes;
        self.chunk_memory.insert(coord.clone(), bytes);
        self.touch(coord, frame);
        true
    }

    pub fn free(&mut self, coord: &ChunkCoord, lod: &LodLevel) {
        if let Some(bytes) = self.chunk_memory.remove(coord) {
            self.used_bytes = self.used_bytes.saturating_sub(bytes);
            if let Some(usage) = self.lod_usage.get_mut(lod) {
                *usage = usage.saturating_sub(bytes);
            }
        }
        self.lru_order.retain(|c| c != coord);
        self.access_count.remove(coord);
        self.last_access_frame.remove(coord);
    }

    pub fn touch(&mut self, coord: ChunkCoord, frame: u64) {
        self.lru_order.retain(|c| c != &coord);
        self.lru_order.push_back(coord.clone());
        *self.access_count.entry(coord.clone()).or_insert(0) += 1;
        self.last_access_frame.insert(coord, frame);
    }

    pub fn eviction_candidates(&self, num: usize, current_frame: u64) -> Vec<ChunkCoord> {
        match self.policy {
            EvictionPolicy::Lru => {
                self.lru_order.iter().take(num).cloned().collect()
            }
            EvictionPolicy::Lfu => {
                let mut by_count: Vec<_> = self.access_count.iter().collect();
                by_count.sort_by_key(|(_, &c)| c);
                by_count.iter().take(num).map(|(c, _)| (*c).clone()).collect()
            }
            EvictionPolicy::DistanceBased => {
                // Return oldest accessed candidates
                let mut by_frame: Vec<_> = self.last_access_frame.iter().collect();
                by_frame.sort_by_key(|(_, &f)| f);
                by_frame.iter().take(num).map(|(c, _)| (*c).clone()).collect()
            }
            EvictionPolicy::PriorityBased => {
                let mut aged: Vec<_> = self.last_access_frame
                    .iter()
                    .filter(|(_, &f)| current_frame.saturating_sub(f) > LRU_MAX_AGE_FRAMES)
                    .collect();
                aged.sort_by_key(|(_, &f)| f);
                aged.iter().take(num).map(|(c, _)| (*c).clone()).collect()
            }
        }
    }

    pub fn needs_eviction(&self) -> bool {
        self.usage_ratio() > 0.95
    }

    pub fn memory_for_coord(&self, coord: &ChunkCoord) -> u64 {
        *self.chunk_memory.get(coord).unwrap_or(&0)
    }

    pub fn lod_usage_mb(&self, lod: &LodLevel) -> f32 {
        *self.lod_usage.get(lod).unwrap_or(&0) as f32 / (1024.0 * 1024.0)
    }

    pub fn total_used_mb(&self) -> f32 {
        self.used_bytes as f32 / (1024.0 * 1024.0)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunk_memory.len()
    }
}

// ============================================================
// CHUNK MESH LOD
// ============================================================

#[derive(Debug, Clone)]
pub struct ChunkMeshLod {
    pub base_vertex_count: u32,
    pub lod_vertex_counts: [u32; 6],
    pub simplification_ratios: [f32; 6],
    pub screen_space_error_thresholds: [f32; 6],
    pub index_buffer_sizes: [u32; 6],
    pub memory_sizes_bytes: [u64; 6],
    pub transition_distances: [f32; 5],
}

impl ChunkMeshLod {
    pub fn new(base_vertex_count: u32, chunk_size: f32) -> Self {
        let ratios: [f32; 6] = [0.0, 0.001, 0.05, 0.2, 0.6, 1.0];
        let sse_thresholds: [f32; 6] = [f32::MAX, 64.0, 16.0, 4.0, 1.0, 0.0];
        let mut lod_vertex_counts = [0u32; 6];
        let mut index_buffer_sizes = [0u32; 6];
        let mut memory_sizes = [0u64; 6];
        for i in 0..6 {
            lod_vertex_counts[i] = (base_vertex_count as f32 * ratios[i]) as u32;
            index_buffer_sizes[i] = lod_vertex_counts[i] * 3; // approximate triangle list
            // 12 bytes per vertex (position), 4 bytes per index
            memory_sizes[i] = lod_vertex_counts[i] as u64 * 12 + index_buffer_sizes[i] as u64 * 4;
        }
        Self {
            base_vertex_count,
            lod_vertex_counts,
            simplification_ratios: ratios,
            screen_space_error_thresholds: sse_thresholds,
            index_buffer_sizes,
            memory_sizes_bytes: memory_sizes,
            transition_distances: LOD_DISTANCES,
        }
    }

    pub fn vertex_count_for_lod(&self, lod: &LodLevel) -> u32 {
        self.lod_vertex_counts[lod.index()]
    }

    pub fn memory_for_lod(&self, lod: &LodLevel) -> u64 {
        self.memory_sizes_bytes[lod.index()]
    }

    pub fn select_lod_for_screen_size(&self, screen_size_pixels: f32) -> LodLevel {
        for i in (0..6).rev() {
            if screen_size_pixels >= self.screen_space_error_thresholds[i] {
                return LodLevel::from_index(i);
            }
        }
        LodLevel::Unloaded
    }

    pub fn select_lod_for_distance(&self, dist: f32) -> LodLevel {
        if dist < self.transition_distances[0]      { LodLevel::Ultra }
        else if dist < self.transition_distances[1] { LodLevel::High }
        else if dist < self.transition_distances[2] { LodLevel::Medium }
        else if dist < self.transition_distances[3] { LodLevel::Low }
        else if dist < self.transition_distances[4] { LodLevel::Impostor }
        else { LodLevel::Unloaded }
    }

    pub fn blend_factor(&self, lod: &LodLevel, dist: f32) -> f32 {
        let idx = lod.index();
        if idx == 0 || idx >= 5 { return 1.0; }
        let near = self.transition_distances[idx - 1];
        let far  = self.transition_distances[idx];
        if far <= near { return 1.0; }
        ((dist - near) / (far - near)).clamp(0.0, 1.0)
    }

    pub fn total_triangle_count(&self, lod: &LodLevel) -> u32 {
        self.index_buffer_sizes[lod.index()] / 3
    }

    pub fn reduction_percentage(&self, lod: &LodLevel) -> f32 {
        (1.0 - self.simplification_ratios[lod.index()]) * 100.0
    }
}

// ============================================================
// IMPOSTOR BILLBOARD
// ============================================================

#[derive(Debug, Clone)]
pub struct ImpostorBillboard {
    pub atlas_uv_min: Vec2,
    pub atlas_uv_max: Vec2,
    pub world_position: Vec3,
    pub scale: Vec2,
    pub pivot_offset: Vec3,
    pub facing_angle_rad: f32,
    pub num_views: u32,
    pub current_view_index: u32,
    pub last_update_frame: u64,
    pub is_dirty: bool,
    pub depth_prepass_enabled: bool,
}

impl ImpostorBillboard {
    pub fn new(world_position: Vec3, scale: Vec2, num_views: u32) -> Self {
        let tile_w = 1.0 / IMPOSTOR_ATLAS_COLS as f32;
        let tile_h = 1.0 / IMPOSTOR_ATLAS_ROWS as f32;
        Self {
            atlas_uv_min: Vec2::ZERO,
            atlas_uv_max: Vec2::new(tile_w, tile_h),
            world_position,
            scale,
            pivot_offset: Vec3::ZERO,
            facing_angle_rad: 0.0,
            num_views,
            current_view_index: 0,
            last_update_frame: 0,
            is_dirty: true,
            depth_prepass_enabled: false,
        }
    }

    pub fn update_view_index(&mut self, camera_pos: Vec3) {
        let to_cam = (camera_pos - self.world_position).normalize_or_zero();
        let angle = to_cam.x.atan2(to_cam.z);
        let normalized = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
        self.current_view_index = (normalized * self.num_views as f32) as u32 % self.num_views;
        self.facing_angle_rad = angle;
        self.update_atlas_uvs();
    }

    fn update_atlas_uvs(&mut self) {
        let cols = IMPOSTOR_ATLAS_COLS;
        let rows = IMPOSTOR_ATLAS_ROWS;
        let tile_w = 1.0 / cols as f32;
        let tile_h = 1.0 / rows as f32;
        let col = (self.current_view_index % cols) as f32;
        let row = (self.current_view_index / cols) as f32;
        self.atlas_uv_min = Vec2::new(col * tile_w, row * tile_h);
        self.atlas_uv_max = Vec2::new((col + 1.0) * tile_w, (row + 1.0) * tile_h);
    }

    pub fn compute_billboard_matrix(&self, camera_pos: Vec3, camera_up: Vec3) -> Mat4 {
        let to_cam = (camera_pos - self.world_position).normalize_or_zero();
        let right = to_cam.cross(camera_up).normalize_or_zero();
        let up    = right.cross(to_cam).normalize_or_zero();
        let scaled_right = right * self.scale.x;
        let scaled_up    = up    * self.scale.y;
        let pos = self.world_position + self.pivot_offset;
        Mat4::from_cols(
            scaled_right.extend(0.0),
            scaled_up.extend(0.0),
            to_cam.extend(0.0),
            pos.extend(1.0),
        )
    }

    pub fn screen_space_bounds(&self, camera: &StreamingCamera) -> (Vec2, Vec2) {
        let proj_pos = camera.proj_matrix * camera.view_matrix * self.world_position.extend(1.0);
        if proj_pos.w.abs() < 1e-8 {
            return (Vec2::ZERO, Vec2::ZERO);
        }
        let ndc = proj_pos.truncate() / proj_pos.w;
        let half_scale = self.scale * 0.5 / proj_pos.w;
        let center_2d = Vec2::new(ndc.x, ndc.y);
        (center_2d - half_scale, center_2d + half_scale)
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn clear_dirty(&mut self, frame: u64) {
        self.is_dirty = false;
        self.last_update_frame = frame;
    }

    pub fn should_update(&self, camera_pos: Vec3, angle_threshold_deg: f32) -> bool {
        if self.is_dirty { return true; }
        let to_cam = (camera_pos - self.world_position).normalize_or_zero();
        let current_angle = to_cam.x.atan2(to_cam.z);
        let diff = (current_angle - self.facing_angle_rad).abs();
        let wrap = if diff > std::f32::consts::PI { 2.0 * std::f32::consts::PI - diff } else { diff };
        let step = 2.0 * std::f32::consts::PI / self.num_views as f32;
        wrap > step * 0.5 + angle_threshold_deg.to_radians()
    }
}

// ============================================================
// TERRAIN HEIGHTMAP
// ============================================================

#[derive(Debug, Clone)]
pub struct TerrainHeightmap {
    pub width: usize,
    pub height: usize,
    pub heights: Vec<f32>,
    pub cell_size: f32,
    pub origin: Vec2,
    pub min_height: f32,
    pub max_height: f32,
    pub scale_y: f32,
}

impl TerrainHeightmap {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2, scale_y: f32) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            heights: vec![0.0; size],
            cell_size,
            origin,
            min_height: 0.0,
            max_height: 0.0,
            scale_y,
        }
    }

    pub fn set_height(&mut self, x: usize, z: usize, h: f32) {
        if x < self.width && z < self.height {
            self.heights[z * self.width + x] = h;
            self.min_height = self.min_height.min(h);
            self.max_height = self.max_height.max(h);
        }
    }

    pub fn get_height(&self, x: usize, z: usize) -> f32 {
        if x < self.width && z < self.height {
            self.heights[z * self.width + x]
        } else {
            0.0
        }
    }

    pub fn sample_bilinear(&self, world_x: f32, world_z: f32) -> f32 {
        let local_x = (world_x - self.origin.x) / self.cell_size;
        let local_z = (world_z - self.origin.y) / self.cell_size;

        let ix = local_x.floor() as isize;
        let iz = local_z.floor() as isize;
        let fx = local_x - ix as f32;
        let fz = local_z - iz as f32;

        let h00 = self.get_clamped(ix,     iz    );
        let h10 = self.get_clamped(ix + 1, iz    );
        let h01 = self.get_clamped(ix,     iz + 1);
        let h11 = self.get_clamped(ix + 1, iz + 1);

        let h0 = h00 * (1.0 - fx) + h10 * fx;
        let h1 = h01 * (1.0 - fx) + h11 * fx;
        (h0 * (1.0 - fz) + h1 * fz) * self.scale_y
    }

    fn get_clamped(&self, x: isize, z: isize) -> f32 {
        let cx = x.clamp(0, self.width as isize - 1) as usize;
        let cz = z.clamp(0, self.height as isize - 1) as usize;
        self.heights[cz * self.width + cx]
    }

    pub fn compute_normal(&self, x: usize, z: usize) -> Vec3 {
        let left  = self.get_height(x.saturating_sub(1), z);
        let right = if x + 1 < self.width { self.get_height(x + 1, z) } else { self.get_height(x, z) };
        let down  = self.get_height(x, z.saturating_sub(1));
        let up    = if z + 1 < self.height { self.get_height(x, z + 1) } else { self.get_height(x, z) };

        let dx = (right - left) * self.scale_y / (2.0 * self.cell_size);
        let dz = (up   - down ) * self.scale_y / (2.0 * self.cell_size);
        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    pub fn compute_normal_bilinear(&self, world_x: f32, world_z: f32) -> Vec3 {
        let epsilon = self.cell_size * 0.5;
        let h_px = self.sample_bilinear(world_x + epsilon, world_z);
        let h_nx = self.sample_bilinear(world_x - epsilon, world_z);
        let h_pz = self.sample_bilinear(world_x, world_z + epsilon);
        let h_nz = self.sample_bilinear(world_x, world_z - epsilon);

        let dx = (h_px - h_nx) / (2.0 * epsilon);
        let dz = (h_pz - h_nz) / (2.0 * epsilon);
        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    pub fn slope_at(&self, world_x: f32, world_z: f32) -> f32 {
        let normal = self.compute_normal_bilinear(world_x, world_z);
        normal.dot(Vec3::Y).acos().to_degrees()
    }

    pub fn curvature_at(&self, x: usize, z: usize) -> f32 {
        if x == 0 || x >= self.width - 1 || z == 0 || z >= self.height - 1 {
            return 0.0;
        }
        let h   = self.get_height(x, z);
        let h_l = self.get_height(x - 1, z);
        let h_r = self.get_height(x + 1, z);
        let h_u = self.get_height(x, z + 1);
        let h_d = self.get_height(x, z - 1);

        let d2x = (h_l - 2.0 * h + h_r) / (self.cell_size * self.cell_size);
        let d2z = (h_d - 2.0 * h + h_u) / (self.cell_size * self.cell_size);
        d2x + d2z
    }

    pub fn bounds(&self) -> ChunkBounds {
        let world_width  = (self.width  - 1) as f32 * self.cell_size;
        let world_height = (self.height - 1) as f32 * self.cell_size;
        ChunkBounds {
            min: Vec3::new(self.origin.x, self.min_height * self.scale_y, self.origin.y),
            max: Vec3::new(self.origin.x + world_width, self.max_height * self.scale_y, self.origin.y + world_height),
        }
    }

    pub fn generate_flat(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        TerrainHeightmap::new(width, height, cell_size, origin, 1.0)
    }

    pub fn generate_sinusoidal(width: usize, height: usize, cell_size: f32, origin: Vec2, amplitude: f32, frequency: f32) -> Self {
        let mut hm = TerrainHeightmap::new(width, height, cell_size, origin, 1.0);
        for z in 0..height {
            for x in 0..width {
                let wx = origin.x + x as f32 * cell_size;
                let wz = origin.y + z as f32 * cell_size;
                let h = amplitude * (wx * frequency).sin() * (wz * frequency).cos();
                hm.set_height(x, z, h);
            }
        }
        hm
    }
}

// ============================================================
// TERRAIN PATCH
// ============================================================

#[derive(Debug, Clone)]
pub struct TerrainPatch {
    pub coord: ChunkCoord,
    pub heightmap: TerrainHeightmap,
    pub lod_level: LodLevel,
    pub neighbor_lods: [Option<LodLevel>; 4], // +x, -x, +z, -z
    pub seam_data: [Vec<f32>; 4],
    pub needs_stitch: bool,
    pub error_metric: f32,
}

impl TerrainPatch {
    pub fn new(coord: ChunkCoord, size: usize, cell_size: f32) -> Self {
        let origin = Vec2::new(
            coord.x as f32 * (size as f32 - 1.0) * cell_size,
            coord.z as f32 * (size as f32 - 1.0) * cell_size,
        );
        let heightmap = TerrainHeightmap::new(size, size, cell_size, origin, 1.0);
        let seam_data = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        Self {
            coord,
            heightmap,
            lod_level: LodLevel::Unloaded,
            neighbor_lods: [None, None, None, None],
            seam_data,
            needs_stitch: false,
            error_metric: 0.0,
        }
    }

    pub fn compute_seam_data(&mut self, side: usize) {
        let size = self.heightmap.width;
        let mut seam = Vec::with_capacity(size);
        match side {
            0 => { // +x edge
                for z in 0..self.heightmap.height {
                    seam.push(self.heightmap.get_height(size - 1, z));
                }
            }
            1 => { // -x edge
                for z in 0..self.heightmap.height {
                    seam.push(self.heightmap.get_height(0, z));
                }
            }
            2 => { // +z edge
                for x in 0..self.heightmap.width {
                    seam.push(self.heightmap.get_height(x, size - 1));
                }
            }
            3 => { // -z edge
                for x in 0..self.heightmap.width {
                    seam.push(self.heightmap.get_height(x, 0));
                }
            }
            _ => {}
        }
        self.seam_data[side] = seam;
    }

    pub fn stitch_edge(&mut self, side: usize, neighbor_seam: &[f32], neighbor_lod: &LodLevel) {
        let my_lod_idx = self.lod_level.index();
        let neighbor_lod_idx = neighbor_lod.index();

        if my_lod_idx <= neighbor_lod_idx {
            return; // no stitching needed if we're same or finer
        }

        let ratio = (1 << (my_lod_idx - neighbor_lod_idx)) as usize;
        let my_size = self.heightmap.width;

        match side {
            0 => { // +x edge
                for z in 0..my_size {
                    if z % ratio != 0 {
                        let z0 = (z / ratio) * ratio;
                        let z1 = (z0 + ratio).min(my_size - 1);
                        let t  = (z - z0) as f32 / ratio as f32;
                        let h0 = if z0 < neighbor_seam.len() { neighbor_seam[z0 / ratio] } else { 0.0 };
                        let h1 = if z1 / ratio < neighbor_seam.len() { neighbor_seam[z1 / ratio] } else { h0 };
                        let blended = h0 * (1.0 - t) + h1 * t;
                        self.heightmap.set_height(my_size - 1, z, blended);
                    }
                }
            }
            1 => { // -x edge
                for z in 0..my_size {
                    if z % ratio != 0 {
                        let z0 = (z / ratio) * ratio;
                        let z1 = (z0 + ratio).min(my_size - 1);
                        let t  = (z - z0) as f32 / ratio as f32;
                        let h0 = if z0 < neighbor_seam.len() { neighbor_seam[z0 / ratio] } else { 0.0 };
                        let h1 = if z1 / ratio < neighbor_seam.len() { neighbor_seam[z1 / ratio] } else { h0 };
                        let blended = h0 * (1.0 - t) + h1 * t;
                        self.heightmap.set_height(0, z, blended);
                    }
                }
            }
            2 => { // +z edge
                for x in 0..my_size {
                    if x % ratio != 0 {
                        let x0 = (x / ratio) * ratio;
                        let x1 = (x0 + ratio).min(my_size - 1);
                        let t  = (x - x0) as f32 / ratio as f32;
                        let h0 = if x0 < neighbor_seam.len() { neighbor_seam[x0 / ratio] } else { 0.0 };
                        let h1 = if x1 / ratio < neighbor_seam.len() { neighbor_seam[x1 / ratio] } else { h0 };
                        let blended = h0 * (1.0 - t) + h1 * t;
                        self.heightmap.set_height(x, my_size - 1, blended);
                    }
                }
            }
            3 => { // -z edge
                for x in 0..my_size {
                    if x % ratio != 0 {
                        let x0 = (x / ratio) * ratio;
                        let x1 = (x0 + ratio).min(my_size - 1);
                        let t  = (x - x0) as f32 / ratio as f32;
                        let h0 = if x0 < neighbor_seam.len() { neighbor_seam[x0 / ratio] } else { 0.0 };
                        let h1 = if x1 / ratio < neighbor_seam.len() { neighbor_seam[x1 / ratio] } else { h0 };
                        let blended = h0 * (1.0 - t) + h1 * t;
                        self.heightmap.set_height(x, 0, blended);
                    }
                }
            }
            _ => {}
        }
        self.needs_stitch = false;
    }

    pub fn compute_error_metric(&mut self) -> f32 {
        let size = self.heightmap.width;
        if size < 3 { self.error_metric = 0.0; return 0.0; }
        let mut max_error = 0.0f32;
        for z in 1..size - 1 {
            for x in 1..size - 1 {
                let h = self.heightmap.get_height(x, z);
                let avg = (
                    self.heightmap.get_height(x - 1, z) +
                    self.heightmap.get_height(x + 1, z) +
                    self.heightmap.get_height(x, z - 1) +
                    self.heightmap.get_height(x, z + 1)
                ) * 0.25;
                max_error = max_error.max((h - avg).abs());
            }
        }
        self.error_metric = max_error;
        max_error
    }

    pub fn lod_for_screen_size(&self, screen_pixels: f32) -> LodLevel {
        if screen_pixels > 512.0       { LodLevel::Ultra   }
        else if screen_pixels > 256.0  { LodLevel::High    }
        else if screen_pixels > 128.0  { LodLevel::Medium  }
        else if screen_pixels > 64.0   { LodLevel::Low     }
        else if screen_pixels > 16.0   { LodLevel::Impostor}
        else                           { LodLevel::Unloaded}
    }

    pub fn update_neighbor_lods(&mut self, neighbors: [Option<LodLevel>; 4]) {
        let changed = self.neighbor_lods != neighbors;
        self.neighbor_lods = neighbors;
        if changed { self.needs_stitch = true; }
    }
}

// ============================================================
// VIRTUAL TEXTURE
// ============================================================

#[derive(Debug, Clone)]
pub struct VirtualTextureTile {
    pub mip: u32,
    pub tile_x: u32,
    pub tile_y: u32,
    pub atlas_slot: u32,
    pub last_requested_frame: u64,
    pub is_resident: bool,
    pub priority: f32,
}

impl VirtualTextureTile {
    pub fn new(mip: u32, tile_x: u32, tile_y: u32) -> Self {
        Self {
            mip,
            tile_x,
            tile_y,
            atlas_slot: u32::MAX,
            last_requested_frame: 0,
            is_resident: false,
            priority: 0.0,
        }
    }

    pub fn tile_id(&self) -> u64 {
        (self.mip as u64) | ((self.tile_x as u64) << 8) | ((self.tile_y as u64) << 24)
    }
}

#[derive(Debug, Clone)]
pub struct VirtualTexture {
    pub page_table: Vec<Vec<u32>>,       // [mip][tile_index] -> atlas slot
    pub tile_cache: HashMap<u64, VirtualTextureTile>,
    pub atlas_size: u32,
    pub tile_size: u32,
    pub num_mips: u32,
    pub max_resident_tiles: usize,
    pub resident_count: usize,
    pub free_slots: VecDeque<u32>,
    pub feedback_buffer: Vec<u32>,
    pub current_frame: u64,
}

impl VirtualTexture {
    pub fn new(atlas_size: u32, tile_size: u32, num_mips: u32) -> Self {
        let tiles_per_row = atlas_size / tile_size;
        let max_tiles = (tiles_per_row * tiles_per_row) as usize;
        let mut free_slots = VecDeque::with_capacity(max_tiles);
        for i in 0..max_tiles as u32 {
            free_slots.push_back(i);
        }
        let page_table: Vec<Vec<u32>> = (0..num_mips)
            .map(|mip| {
                let tiles_at_mip = (tiles_per_row >> mip).max(1);
                vec![u32::MAX; (tiles_at_mip * tiles_at_mip) as usize]
            })
            .collect();

        Self {
            page_table,
            tile_cache: HashMap::new(),
            atlas_size,
            tile_size,
            num_mips,
            max_resident_tiles: max_tiles,
            resident_count: 0,
            free_slots,
            feedback_buffer: vec![0u32; FEEDBACK_BUFFER_MIPS],
            current_frame: 0,
        }
    }

    pub fn request_tile(&mut self, mip: u32, tile_x: u32, tile_y: u32, frame: u64) {
        let tile_id = (mip as u64) | ((tile_x as u64) << 8) | ((tile_y as u64) << 24);
        if let Some(tile) = self.tile_cache.get_mut(&tile_id) {
            tile.last_requested_frame = frame;
            return;
        }
        let mut tile = VirtualTextureTile::new(mip, tile_x, tile_y);
        tile.last_requested_frame = frame;
        self.tile_cache.insert(tile_id, tile);
    }

    pub fn load_tile(&mut self, mip: u32, tile_x: u32, tile_y: u32) -> Option<u32> {
        let tile_id = (mip as u64) | ((tile_x as u64) << 8) | ((tile_y as u64) << 24);
        let slot = self.free_slots.pop_front()?;

        if let Some(tile) = self.tile_cache.get_mut(&tile_id) {
            tile.atlas_slot = slot;
            tile.is_resident = true;
        }

        let tiles_at_mip = ((self.atlas_size / self.tile_size) >> mip).max(1) as usize;
        let index = tile_y as usize * tiles_at_mip + tile_x as usize;
        if (mip as usize) < self.page_table.len() && index < self.page_table[mip as usize].len() {
            self.page_table[mip as usize][index] = slot;
        }

        self.resident_count += 1;
        Some(slot)
    }

    pub fn evict_tile(&mut self, mip: u32, tile_x: u32, tile_y: u32) {
        let tile_id = (mip as u64) | ((tile_x as u64) << 8) | ((tile_y as u64) << 24);
        if let Some(tile) = self.tile_cache.get_mut(&tile_id) {
            if tile.is_resident {
                let slot = tile.atlas_slot;
                tile.is_resident = false;
                tile.atlas_slot = u32::MAX;
                self.free_slots.push_back(slot);
                self.resident_count -= 1;

                let tiles_at_mip = ((self.atlas_size / self.tile_size) >> mip).max(1) as usize;
                let index = tile_y as usize * tiles_at_mip + tile_x as usize;
                if (mip as usize) < self.page_table.len() && index < self.page_table[mip as usize].len() {
                    self.page_table[mip as usize][index] = u32::MAX;
                }
            }
        }
    }

    pub fn analyze_feedback_buffer(&self) -> Vec<(u32, u32, u32, f32)> {
        let mut requests = Vec::new();
        for (mip_idx, &count) in self.feedback_buffer.iter().enumerate() {
            if count > 0 {
                let mip = mip_idx as u32;
                let tiles = ((self.atlas_size / self.tile_size) >> mip).max(1);
                let priority = count as f32 / (tiles * tiles) as f32;
                // Generate tile requests for all tiles in this mip
                for y in 0..tiles {
                    for x in 0..tiles {
                        requests.push((mip, x, y, priority));
                    }
                }
            }
        }
        requests.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        requests
    }

    pub fn record_feedback(&mut self, mip: u32) {
        let idx = (mip as usize).min(self.feedback_buffer.len() - 1);
        self.feedback_buffer[idx] += 1;
    }

    pub fn clear_feedback(&mut self) {
        for v in &mut self.feedback_buffer { *v = 0; }
    }

    pub fn tile_atlas_uv(&self, slot: u32) -> (Vec2, Vec2) {
        let tiles_per_row = self.atlas_size / self.tile_size;
        let col = slot % tiles_per_row;
        let row = slot / tiles_per_row;
        let uv_tile_size = self.tile_size as f32 / self.atlas_size as f32;
        let uv_min = Vec2::new(col as f32 * uv_tile_size, row as f32 * uv_tile_size);
        let uv_max = uv_min + Vec2::splat(uv_tile_size);
        (uv_min, uv_max)
    }

    pub fn evict_lru_tiles(&mut self, target_free: usize) {
        if self.free_slots.len() >= target_free { return; }
        let mut by_age: Vec<_> = self.tile_cache.values()
            .filter(|t| t.is_resident)
            .map(|t| (t.tile_id(), t.last_requested_frame, t.mip, t.tile_x, t.tile_y))
            .collect();
        by_age.sort_by_key(|&(_, frame, _, _, _)| frame);

        let to_evict = (target_free - self.free_slots.len()).min(by_age.len());
        for i in 0..to_evict {
            let (_, _, mip, tx, ty) = by_age[i];
            self.evict_tile(mip, tx, ty);
        }
    }
}

// ============================================================
// STREAMING STATS
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct StreamingStats {
    pub frame_number: u64,
    pub chunks_loaded_this_frame: u32,
    pub chunks_unloaded_this_frame: u32,
    pub chunks_lod_switched_this_frame: u32,
    pub total_chunks_loaded: u32,
    pub total_chunks_unloaded: u32,
    pub total_draw_calls_saved: u64,
    pub memory_used_bytes: u64,
    pub memory_budget_bytes: u64,
    pub chunks_in_frustum: u32,
    pub chunks_frustum_culled: u32,
    pub chunks_distance_culled: u32,
    pub active_chunks: u32,
    pub queued_loads: u32,
    pub active_loads: u32,
    pub impostor_draw_calls: u32,
    pub lod_low_draw_calls: u32,
    pub lod_medium_draw_calls: u32,
    pub lod_high_draw_calls: u32,
    pub lod_ultra_draw_calls: u32,
    pub terrain_stitch_ops: u32,
    pub virtual_texture_uploads: u32,
    pub hlod_merges: u32,
    pub frame_load_time_us: u64,
    pub frame_cull_time_us: u64,
    pub frame_lod_time_us: u64,
    pub peak_memory_bytes: u64,
    pub avg_chunk_load_time_us: f64,
    pub total_visible_objects: u32,
    pub total_culled_objects: u32,
}

impl StreamingStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset_frame_counters(&mut self) {
        self.chunks_loaded_this_frame = 0;
        self.chunks_unloaded_this_frame = 0;
        self.chunks_lod_switched_this_frame = 0;
        self.impostor_draw_calls = 0;
        self.lod_low_draw_calls = 0;
        self.lod_medium_draw_calls = 0;
        self.lod_high_draw_calls = 0;
        self.lod_ultra_draw_calls = 0;
        self.terrain_stitch_ops = 0;
        self.virtual_texture_uploads = 0;
        self.frame_load_time_us = 0;
        self.frame_cull_time_us = 0;
        self.frame_lod_time_us = 0;
        self.chunks_in_frustum = 0;
        self.chunks_frustum_culled = 0;
        self.chunks_distance_culled = 0;
    }

    pub fn memory_usage_ratio(&self) -> f32 {
        if self.memory_budget_bytes == 0 { return 0.0; }
        self.memory_used_bytes as f32 / self.memory_budget_bytes as f32
    }

    pub fn draw_calls_saved_ratio(&self) -> f32 {
        let total = self.impostor_draw_calls + self.lod_low_draw_calls
            + self.lod_medium_draw_calls + self.lod_high_draw_calls + self.lod_ultra_draw_calls;
        if total == 0 { return 0.0; }
        self.total_draw_calls_saved as f32 / total as f32
    }

    pub fn advance_frame(&mut self) {
        self.frame_number += 1;
        self.total_chunks_loaded += self.chunks_loaded_this_frame;
        self.total_chunks_unloaded += self.chunks_unloaded_this_frame;
        if self.memory_used_bytes > self.peak_memory_bytes {
            self.peak_memory_bytes = self.memory_used_bytes;
        }
        if self.chunks_loaded_this_frame > 0 {
            let load_time = self.frame_load_time_us as f64;
            let count = self.chunks_loaded_this_frame as f64;
            let alpha = 0.1;
            self.avg_chunk_load_time_us = self.avg_chunk_load_time_us * (1.0 - alpha)
                + (load_time / count) * alpha;
        }
        self.reset_frame_counters();
    }

    pub fn record_lod_draw_call(&mut self, lod: &LodLevel) {
        match lod {
            LodLevel::Impostor => self.impostor_draw_calls += 1,
            LodLevel::Low      => self.lod_low_draw_calls += 1,
            LodLevel::Medium   => self.lod_medium_draw_calls += 1,
            LodLevel::High     => self.lod_high_draw_calls += 1,
            LodLevel::Ultra    => self.lod_ultra_draw_calls += 1,
            LodLevel::Unloaded => {}
        }
    }
}

// ============================================================
// WORLD PARTITION
// ============================================================

#[derive(Debug, Clone)]
pub struct WorldPartitionCell {
    pub coord: ChunkCoord,
    pub bounds: ChunkBounds,
    pub actors: Vec<u32>,
    pub is_loaded: bool,
    pub streaming_source_count: u32,
}

impl WorldPartitionCell {
    pub fn new(coord: ChunkCoord, cell_size: f32) -> Self {
        let bounds = ChunkBounds::from_chunk_coord(&coord, cell_size);
        Self {
            coord,
            bounds,
            actors: Vec::new(),
            is_loaded: false,
            streaming_source_count: 0,
        }
    }

    pub fn add_actor(&mut self, actor_id: u32) {
        if !self.actors.contains(&actor_id) {
            self.actors.push(actor_id);
        }
    }

    pub fn remove_actor(&mut self, actor_id: u32) {
        self.actors.retain(|&id| id != actor_id);
    }
}

#[derive(Debug, Clone)]
pub struct WorldPartition {
    pub cells: HashMap<ChunkCoord, WorldPartitionCell>,
    pub cell_size: f32,
    pub actor_to_cell: HashMap<u32, ChunkCoord>,
    pub loaded_cells: HashSet<ChunkCoord>,
    pub streaming_sources: Vec<Vec3>,
    pub bounds: ChunkBounds,
}

impl WorldPartition {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            actor_to_cell: HashMap::new(),
            loaded_cells: HashSet::new(),
            streaming_sources: Vec::new(),
            bounds: ChunkBounds::new(Vec3::ZERO, Vec3::ZERO),
        }
    }

    pub fn register_actor(&mut self, actor_id: u32, world_pos: Vec3) {
        let coord = ChunkCoord::from_world_pos(world_pos, self.cell_size);
        let cell = self.cells.entry(coord.clone()).or_insert_with(|| {
            WorldPartitionCell::new(coord.clone(), self.cell_size)
        });
        cell.add_actor(actor_id);
        self.actor_to_cell.insert(actor_id, coord);
        self.recompute_bounds();
    }

    pub fn unregister_actor(&mut self, actor_id: u32) {
        if let Some(coord) = self.actor_to_cell.remove(&actor_id) {
            if let Some(cell) = self.cells.get_mut(&coord) {
                cell.remove_actor(actor_id);
            }
        }
    }

    pub fn move_actor(&mut self, actor_id: u32, new_pos: Vec3) {
        let new_coord = ChunkCoord::from_world_pos(new_pos, self.cell_size);
        if let Some(old_coord) = self.actor_to_cell.get(&actor_id).cloned() {
            if old_coord == new_coord { return; }
            if let Some(old_cell) = self.cells.get_mut(&old_coord) {
                old_cell.remove_actor(actor_id);
            }
        }
        let cell = self.cells.entry(new_coord.clone()).or_insert_with(|| {
            WorldPartitionCell::new(new_coord.clone(), self.cell_size)
        });
        cell.add_actor(actor_id);
        self.actor_to_cell.insert(actor_id, new_coord);
    }

    pub fn add_streaming_source(&mut self, pos: Vec3) {
        self.streaming_sources.push(pos);
    }

    pub fn clear_streaming_sources(&mut self) {
        self.streaming_sources.clear();
    }

    pub fn compute_cells_to_load(&self, load_radius: f32) -> HashSet<ChunkCoord> {
        let mut to_load = HashSet::new();
        let radius_cells = (load_radius / self.cell_size).ceil() as i32;
        for &source in &self.streaming_sources {
            let center = ChunkCoord::from_world_pos(source, self.cell_size);
            let candidates = ChunkCoord::chunks_in_radius(&center, radius_cells);
            for coord in candidates {
                if let Some(cell) = self.cells.get(&coord) {
                    let dist = cell.bounds.distance_to_point(source);
                    if dist <= load_radius {
                        to_load.insert(coord);
                    }
                }
            }
        }
        to_load
    }

    pub fn get_actors_in_bounds(&self, bounds: &ChunkBounds) -> Vec<u32> {
        let mut result = Vec::new();
        for (_, cell) in &self.cells {
            if cell.bounds.intersects(bounds) {
                result.extend_from_slice(&cell.actors);
            }
        }
        result
    }

    pub fn get_actors_in_radius(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let query_bounds = ChunkBounds::from_center_size(center, Vec3::splat(radius));
        let mut result = Vec::new();
        for (_, cell) in &self.cells {
            if cell.bounds.intersects(&query_bounds) {
                for &actor in &cell.actors {
                    result.push(actor);
                }
            }
        }
        result
    }

    fn recompute_bounds(&mut self) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for (_, cell) in &self.cells {
            if !cell.actors.is_empty() {
                min = min.min(cell.bounds.min);
                max = max.max(cell.bounds.max);
            }
        }
        if min.x <= max.x {
            self.bounds = ChunkBounds { min, max };
        }
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn actor_count(&self) -> usize {
        self.actor_to_cell.len()
    }
}

// ============================================================
// ACTOR STREAMING PROXY
// ============================================================

#[derive(Debug, Clone)]
pub struct ActorStreamingProxy {
    pub actor_id: u32,
    pub world_position: Vec3,
    pub world_rotation: Quat,
    pub world_scale: Vec3,
    pub bounds: ChunkBounds,
    pub streaming_distance: f32,
    pub lod_level: LodLevel,
    pub is_loaded: bool,
    pub data_layer_mask: u32,
    pub hlod_cluster_id: Option<u32>,
    pub last_frame_visible: u64,
    pub importance: f32,
}

impl ActorStreamingProxy {
    pub fn new(actor_id: u32, position: Vec3, bounds: ChunkBounds) -> Self {
        Self {
            actor_id,
            world_position: position,
            world_rotation: Quat::IDENTITY,
            world_scale: Vec3::ONE,
            bounds,
            streaming_distance: 1000.0,
            lod_level: LodLevel::Unloaded,
            is_loaded: false,
            data_layer_mask: 0xFFFF_FFFF,
            hlod_cluster_id: None,
            last_frame_visible: 0,
            importance: 1.0,
        }
    }

    pub fn world_transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.world_scale,
            self.world_rotation,
            self.world_position,
        )
    }

    pub fn distance_to_viewer(&self, viewer_pos: Vec3) -> f32 {
        (self.world_position - viewer_pos).length()
    }

    pub fn should_load(&self, viewer_pos: Vec3) -> bool {
        let dist = self.distance_to_viewer(viewer_pos);
        dist <= self.streaming_distance
    }

    pub fn desired_lod(&self, viewer_pos: Vec3, config: &StreamingConfig) -> LodLevel {
        let dist = self.distance_to_viewer(viewer_pos);
        config.lod_for_distance(dist * self.importance.recip())
    }

    pub fn screen_size_at_distance(&self, viewer_pos: Vec3, camera: &StreamingCamera, screen_height: f32) -> f32 {
        let radius = self.bounds.half_size().length();
        camera.project_sphere_to_screen(self.world_position, radius, screen_height)
    }

    pub fn is_in_data_layer(&self, layer_mask: u32) -> bool {
        (self.data_layer_mask & layer_mask) != 0
    }

    pub fn update_transform(&mut self, pos: Vec3, rot: Quat, scale: Vec3) {
        self.world_position = pos;
        self.world_rotation = rot;
        self.world_scale = scale;
        let extent = self.bounds.size() * scale * 0.5;
        self.bounds = ChunkBounds::from_center_size(pos, extent);
    }
}

// ============================================================
// DATA LAYER SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct DataLayer {
    pub id: u32,
    pub name: String,
    pub mode: DataLayerMode,
    pub parent_id: Option<u32>,
    pub child_ids: Vec<u32>,
    pub is_visible: bool,
    pub is_loaded: bool,
    pub actor_ids: Vec<u32>,
    pub spatial_bounds: Option<ChunkBounds>,
    pub load_state: ChunkLoadState,
    pub debug_color: Vec3,
}

impl DataLayer {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            mode: DataLayerMode::Inherited,
            parent_id: None,
            child_ids: Vec::new(),
            is_visible: true,
            is_loaded: false,
            actor_ids: Vec::new(),
            spatial_bounds: None,
            load_state: ChunkLoadState::Unloaded,
            debug_color: Vec3::ONE,
        }
    }

    pub fn add_actor(&mut self, actor_id: u32) {
        if !self.actor_ids.contains(&actor_id) {
            self.actor_ids.push(actor_id);
        }
    }

    pub fn remove_actor(&mut self, actor_id: u32) {
        self.actor_ids.retain(|&id| id != actor_id);
    }

    pub fn is_active(&self) -> bool {
        self.is_visible && self.is_loaded
    }

    pub fn effective_mode(&self) -> DataLayerMode {
        match &self.mode {
            DataLayerMode::Inherited => DataLayerMode::Included,
            other => other.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DataLayerSystem {
    pub layers: HashMap<u32, DataLayer>,
    pub next_id: u32,
    pub active_layers: HashSet<u32>,
    pub actor_layer_map: HashMap<u32, Vec<u32>>,
}

impl DataLayerSystem {
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
            next_id: 1,
            active_layers: HashSet::new(),
            actor_layer_map: HashMap::new(),
        }
    }

    pub fn create_layer(&mut self, name: String) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.layers.insert(id, DataLayer::new(id, name));
        id
    }

    pub fn delete_layer(&mut self, id: u32) {
        if let Some(layer) = self.layers.remove(&id) {
            for actor_id in &layer.actor_ids {
                if let Some(layers) = self.actor_layer_map.get_mut(actor_id) {
                    layers.retain(|&lid| lid != id);
                }
            }
        }
        self.active_layers.remove(&id);
    }

    pub fn add_actor_to_layer(&mut self, layer_id: u32, actor_id: u32) {
        if let Some(layer) = self.layers.get_mut(&layer_id) {
            layer.add_actor(actor_id);
        }
        self.actor_layer_map.entry(actor_id).or_insert_with(Vec::new).push(layer_id);
    }

    pub fn activate_layer(&mut self, id: u32) {
        self.active_layers.insert(id);
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.is_loaded = true;
            layer.is_visible = true;
        }
    }

    pub fn deactivate_layer(&mut self, id: u32) {
        self.active_layers.remove(&id);
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.is_loaded = false;
            layer.is_visible = false;
        }
    }

    pub fn is_actor_visible(&self, actor_id: u32) -> bool {
        match self.actor_layer_map.get(&actor_id) {
            None => true,
            Some(layer_ids) => {
                layer_ids.iter().all(|lid| {
                    self.layers.get(lid).map_or(true, |l| {
                        match l.effective_mode() {
                            DataLayerMode::Included  => self.active_layers.contains(lid),
                            DataLayerMode::Excluded  => !self.active_layers.contains(lid),
                            DataLayerMode::Inherited => true,
                        }
                    })
                })
            }
        }
    }

    pub fn actors_in_active_layers(&self) -> Vec<u32> {
        let mut actors = Vec::new();
        for lid in &self.active_layers {
            if let Some(layer) = self.layers.get(lid) {
                actors.extend_from_slice(&layer.actor_ids);
            }
        }
        actors.sort_unstable();
        actors.dedup();
        actors
    }

    pub fn compute_layer_bounds(&mut self, layer_id: u32, actor_positions: &HashMap<u32, Vec3>) {
        if let Some(layer) = self.layers.get_mut(&layer_id) {
            let mut min = Vec3::splat(f32::MAX);
            let mut max = Vec3::splat(f32::MIN);
            for &actor_id in &layer.actor_ids {
                if let Some(&pos) = actor_positions.get(&actor_id) {
                    min = min.min(pos);
                    max = max.max(pos);
                }
            }
            if min.x <= max.x {
                layer.spatial_bounds = Some(ChunkBounds {
                    min: min - Vec3::splat(1.0),
                    max: max + Vec3::splat(1.0),
                });
            }
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}

// ============================================================
// HLOD CLUSTER
// ============================================================

#[derive(Debug, Clone)]
pub struct HlodCluster {
    pub cluster_id: u32,
    pub actor_ids: Vec<u32>,
    pub merged_bounds: ChunkBounds,
    pub center: Vec3,
    pub radius: f32,
    pub lod_level: LodLevel,
    pub simplified_vertex_count: u32,
    pub simplified_triangle_count: u32,
    pub memory_bytes: u64,
    pub is_built: bool,
    pub sub_clusters: Vec<u32>,
    pub parent_cluster_id: Option<u32>,
    pub depth: u32,
    pub importance: f32,
}

impl HlodCluster {
    pub fn new(cluster_id: u32, depth: u32) -> Self {
        Self {
            cluster_id,
            actor_ids: Vec::new(),
            merged_bounds: ChunkBounds::new(Vec3::ZERO, Vec3::ZERO),
            center: Vec3::ZERO,
            radius: 0.0,
            lod_level: LodLevel::Low,
            simplified_vertex_count: 0,
            simplified_triangle_count: 0,
            memory_bytes: 0,
            is_built: false,
            sub_clusters: Vec::new(),
            parent_cluster_id: None,
            depth,
            importance: 1.0,
        }
    }

    pub fn add_actor(&mut self, actor_id: u32, bounds: &ChunkBounds) {
        self.actor_ids.push(actor_id);
        if self.actor_ids.len() == 1 {
            self.merged_bounds = bounds.clone();
        } else {
            self.merged_bounds = self.merged_bounds.merge(bounds);
        }
        self.center = self.merged_bounds.center();
        let half = self.merged_bounds.half_size();
        self.radius = half.length();
    }

    pub fn compute_simplified_geometry(&mut self) {
        // Estimate: each actor contributes N triangles, simplified to 10%
        let tris_per_actor = 1000u32;
        let total_tris = tris_per_actor * self.actor_ids.len() as u32;
        let ratio = match self.depth {
            0 => 0.1,
            1 => 0.2,
            2 => 0.4,
            _ => 0.5,
        };
        self.simplified_triangle_count = (total_tris as f32 * ratio) as u32;
        self.simplified_vertex_count   = (self.simplified_triangle_count as f32 * 0.6) as u32;
        self.memory_bytes = self.simplified_vertex_count as u64 * 32 // 32 bytes per vertex
                          + self.simplified_triangle_count as u64 * 12; // 12 bytes per triangle
    }

    pub fn compute_screen_size(&self, camera_pos: Vec3, screen_height: f32, fov_rad: f32) -> f32 {
        let dist = (self.center - camera_pos).length();
        if dist < 1e-6 { return screen_height; }
        let angular = self.radius / dist;
        let half_fov_tan = (fov_rad * 0.5).tan();
        (angular / half_fov_tan) * screen_height
    }

    pub fn should_use_hlod(&self, camera_pos: Vec3, screen_height: f32, fov_rad: f32, threshold: f32) -> bool {
        self.compute_screen_size(camera_pos, screen_height, fov_rad) < threshold
    }

    pub fn overlap_with(&self, other: &HlodCluster) -> bool {
        self.merged_bounds.intersects(&other.merged_bounds)
    }

    pub fn merge_cluster(&mut self, other: &HlodCluster) {
        self.actor_ids.extend_from_slice(&other.actor_ids);
        self.merged_bounds = self.merged_bounds.merge(&other.merged_bounds);
        self.center = self.merged_bounds.center();
        let half = self.merged_bounds.half_size();
        self.radius = half.length();
        self.simplified_vertex_count   += other.simplified_vertex_count;
        self.simplified_triangle_count += other.simplified_triangle_count;
        self.memory_bytes += other.memory_bytes;
    }
}

// ============================================================
// HLOD BUILDER
// ============================================================

#[derive(Debug, Clone)]
pub struct HlodBuilder {
    pub cluster_radius: f32,
    pub max_actors_per_cluster: usize,
    pub max_depth: u32,
    pub simplification_ratio: f32,
    pub next_cluster_id: u32,
}

impl HlodBuilder {
    pub fn new(cluster_radius: f32, max_actors: usize, max_depth: u32) -> Self {
        Self {
            cluster_radius,
            max_actors_per_cluster: max_actors,
            max_depth,
            simplification_ratio: 0.1,
            next_cluster_id: 1,
        }
    }

    pub fn build_clusters(&mut self, actors: &[(u32, Vec3, ChunkBounds)]) -> Vec<HlodCluster> {
        if actors.is_empty() { return Vec::new(); }
        self.build_level(actors, 0)
    }

    fn build_level(&mut self, actors: &[(u32, Vec3, ChunkBounds)], depth: u32) -> Vec<HlodCluster> {
        let mut clusters: Vec<HlodCluster> = Vec::new();
        let mut assigned: Vec<bool> = vec![false; actors.len()];

        for i in 0..actors.len() {
            if assigned[i] { continue; }
            let mut cluster = HlodCluster::new(self.next_cluster_id, depth);
            self.next_cluster_id += 1;
            let seed_pos = actors[i].1;
            cluster.add_actor(actors[i].0, &actors[i].2);
            assigned[i] = true;

            for j in (i + 1)..actors.len() {
                if assigned[j] { continue; }
                if cluster.actor_ids.len() >= self.max_actors_per_cluster { break; }
                let dist = (actors[j].1 - seed_pos).length();
                if dist <= self.cluster_radius {
                    cluster.add_actor(actors[j].0, &actors[j].2);
                    assigned[j] = true;
                }
            }

            cluster.compute_simplified_geometry();
            clusters.push(cluster);
        }

        clusters
    }

    pub fn build_hierarchical(&mut self, actors: &[(u32, Vec3, ChunkBounds)]) -> Vec<Vec<HlodCluster>> {
        let mut hierarchy: Vec<Vec<HlodCluster>> = Vec::new();
        let leaf_clusters = self.build_clusters(actors);
        hierarchy.push(leaf_clusters);

        let mut depth = 1u32;
        while depth <= self.max_depth {
            let prev_clusters = hierarchy.last().unwrap();
            if prev_clusters.len() <= 1 { break; }

            let cluster_actors: Vec<(u32, Vec3, ChunkBounds)> = prev_clusters.iter()
                .map(|c| (c.cluster_id, c.center, c.merged_bounds.clone()))
                .collect();

            let new_radius = self.cluster_radius * (1 << depth) as f32;
            let mut builder = HlodBuilder::new(new_radius, self.max_actors_per_cluster * 4, self.max_depth);
            builder.next_cluster_id = self.next_cluster_id;
            let next_level = builder.build_clusters(&cluster_actors);
            self.next_cluster_id = builder.next_cluster_id;
            hierarchy.push(next_level);
            depth += 1;
        }
        hierarchy
    }

    pub fn spatially_sort_actors(&self, actors: &mut [(u32, Vec3, ChunkBounds)]) {
        // Z-order curve sort for spatial locality
        actors.sort_by(|a, b| {
            let za = morton_encode_2d(a.1.x as u32, a.1.z as u32);
            let zb = morton_encode_2d(b.1.x as u32, b.1.z as u32);
            za.cmp(&zb)
        });
    }

    pub fn compute_cluster_bounds(cluster: &HlodCluster) -> ChunkBounds {
        cluster.merged_bounds.clone()
    }

    pub fn merge_small_clusters(&self, clusters: &mut Vec<HlodCluster>, min_actors: usize) {
        let small_ids: Vec<usize> = clusters.iter().enumerate()
            .filter(|(_, c)| c.actor_ids.len() < min_actors)
            .map(|(i, _)| i)
            .collect();

        for i in small_ids.iter().rev() {
            if *i >= clusters.len() { continue; }
            let small = clusters.remove(*i);
            // Find nearest cluster to merge into
            let mut best = 0;
            let mut best_dist = f32::MAX;
            for (j, c) in clusters.iter().enumerate() {
                let dist = (c.center - small.center).length();
                if dist < best_dist { best_dist = dist; best = j; }
            }
            if !clusters.is_empty() {
                clusters[best].merge_cluster(&small);
            } else {
                clusters.push(small);
            }
        }
    }
}

// Morton encoding for Z-order curve
fn morton_encode_2d(x: u32, y: u32) -> u64 {
    let mut result = 0u64;
    let x = x as u64;
    let y = y as u64;
    for i in 0..32u64 {
        result |= ((x >> i) & 1) << (2 * i);
        result |= ((y >> i) & 1) << (2 * i + 1);
    }
    result
}

// ============================================================
// STREAMING DISTANCE CALCULATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingDistanceCalculator {
    pub screen_height: f32,
    pub fov_vertical_rad: f32,
    pub min_screen_size_fraction: f32,
    pub max_streaming_distance: f32,
    pub lod_bias: f32,
}

impl StreamingDistanceCalculator {
    pub fn new(screen_height: f32, fov_rad: f32) -> Self {
        Self {
            screen_height,
            fov_vertical_rad: fov_rad,
            min_screen_size_fraction: 0.01,
            max_streaming_distance: 10000.0,
            lod_bias: 0.0,
        }
    }

    pub fn compute_streaming_distance(&self, bounding_radius: f32, min_screen_fraction: f32) -> f32 {
        let target_screen = self.screen_height * min_screen_fraction;
        let half_fov_tan = (self.fov_vertical_rad * 0.5).tan();
        if target_screen < 1e-8 || half_fov_tan < 1e-8 {
            return self.max_streaming_distance;
        }
        let dist = bounding_radius * self.screen_height / (target_screen * half_fov_tan);
        (dist * (1.0 + self.lod_bias)).min(self.max_streaming_distance)
    }

    pub fn compute_lod_transition_distances(&self, bounding_radius: f32) -> [f32; 5] {
        let fractions = [0.8, 0.4, 0.2, 0.1, 0.05];
        let mut dists = [0.0f32; 5];
        for (i, &frac) in fractions.iter().enumerate() {
            dists[i] = self.compute_streaming_distance(bounding_radius, frac);
        }
        dists
    }

    pub fn screen_size_at_distance(&self, bounding_radius: f32, distance: f32) -> f32 {
        if distance < 1e-6 { return self.screen_height; }
        let half_fov_tan = (self.fov_vertical_rad * 0.5).tan();
        let angular_size = bounding_radius / distance;
        (angular_size / half_fov_tan) * self.screen_height
    }

    pub fn desired_lod_at_distance(&self, distance: f32, bounding_radius: f32) -> LodLevel {
        let screen_size = self.screen_size_at_distance(bounding_radius, distance);
        let fraction = screen_size / self.screen_height;
        if fraction > 0.5        { LodLevel::Ultra   }
        else if fraction > 0.2   { LodLevel::High    }
        else if fraction > 0.08  { LodLevel::Medium  }
        else if fraction > 0.03  { LodLevel::Low     }
        else if fraction > 0.01  { LodLevel::Impostor}
        else                     { LodLevel::Unloaded}
    }

    pub fn compute_cull_distance(&self, bounding_radius: f32) -> f32 {
        self.compute_streaming_distance(bounding_radius, self.min_screen_size_fraction)
    }

    pub fn importance_adjusted_distance(&self, distance: f32, importance: f32) -> f32 {
        distance / importance.max(0.01)
    }

    pub fn compute_lod_blend_alpha(&self, distance: f32, near_dist: f32, far_dist: f32) -> f32 {
        if distance <= near_dist { return 0.0; }
        if distance >= far_dist  { return 1.0; }
        let range = far_dist - near_dist;
        if range < 1e-6 { return 1.0; }
        (distance - near_dist) / range
    }
}

// ============================================================
// CHUNK DEPENDENCY
// ============================================================

#[derive(Debug, Clone)]
pub struct ChunkDependencyEdge {
    pub from: ChunkCoord,
    pub to: ChunkCoord,
    pub is_hard: bool,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct ChunkDependency {
    pub dependency_graph: HashMap<ChunkCoord, Vec<ChunkCoord>>,
    pub reverse_graph: HashMap<ChunkCoord, Vec<ChunkCoord>>,
    pub edges: Vec<ChunkDependencyEdge>,
    pub topological_order: Vec<ChunkCoord>,
    pub is_dirty: bool,
}

impl ChunkDependency {
    pub fn new() -> Self {
        Self {
            dependency_graph: HashMap::new(),
            reverse_graph: HashMap::new(),
            edges: Vec::new(),
            topological_order: Vec::new(),
            is_dirty: true,
        }
    }

    pub fn add_dependency(&mut self, from: ChunkCoord, to: ChunkCoord, is_hard: bool) {
        self.dependency_graph.entry(from.clone()).or_insert_with(Vec::new).push(to.clone());
        self.reverse_graph.entry(to.clone()).or_insert_with(Vec::new).push(from.clone());
        self.edges.push(ChunkDependencyEdge {
            from,
            to,
            is_hard,
            weight: if is_hard { 1.0 } else { 0.5 },
        });
        self.is_dirty = true;
    }

    pub fn remove_dependency(&mut self, from: &ChunkCoord, to: &ChunkCoord) {
        if let Some(deps) = self.dependency_graph.get_mut(from) {
            deps.retain(|c| c != to);
        }
        if let Some(revs) = self.reverse_graph.get_mut(to) {
            revs.retain(|c| c != from);
        }
        self.edges.retain(|e| !(&e.from == from && &e.to == to));
        self.is_dirty = true;
    }

    pub fn dependencies_of(&self, coord: &ChunkCoord) -> Vec<&ChunkCoord> {
        self.dependency_graph.get(coord).map(|v| v.iter().collect()).unwrap_or_default()
    }

    pub fn dependents_of(&self, coord: &ChunkCoord) -> Vec<&ChunkCoord> {
        self.reverse_graph.get(coord).map(|v| v.iter().collect()).unwrap_or_default()
    }

    pub fn topological_sort(&mut self) -> bool {
        if !self.is_dirty { return true; }
        // Kahn's algorithm
        let mut in_degree: HashMap<ChunkCoord, usize> = HashMap::new();
        let all_nodes: HashSet<ChunkCoord> = self.dependency_graph.keys()
            .chain(self.reverse_graph.keys())
            .cloned()
            .collect();

        for node in &all_nodes {
            in_degree.entry(node.clone()).or_insert(0);
        }
        for edge in &self.edges {
            *in_degree.entry(edge.from.clone()).or_insert(0) += 1;
        }

        let mut queue: VecDeque<ChunkCoord> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(c, _)| c.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(dependents) = self.reverse_graph.get(&node) {
                for dep in dependents.clone() {
                    let deg = in_degree.entry(dep.clone()).or_insert(0);
                    if *deg > 0 { *deg -= 1; }
                    if *deg == 0 { queue.push_back(dep); }
                }
            }
        }

        if order.len() == all_nodes.len() {
            self.topological_order = order;
            self.is_dirty = false;
            true
        } else {
            false // cycle detected
        }
    }

    pub fn transitive_dependencies(&self, coord: &ChunkCoord, max_depth: usize) -> HashSet<ChunkCoord> {
        let mut visited = HashSet::new();
        let mut stack = vec![(coord.clone(), 0)];
        while let Some((current, depth)) = stack.pop() {
            if depth >= max_depth || !visited.insert(current.clone()) { continue; }
            if let Some(deps) = self.dependency_graph.get(&current) {
                for dep in deps {
                    stack.push((dep.clone(), depth + 1));
                }
            }
        }
        visited.remove(coord);
        visited
    }

    pub fn has_cycle(&self) -> bool {
        let all_nodes: HashSet<ChunkCoord> = self.dependency_graph.keys()
            .chain(self.reverse_graph.keys())
            .cloned()
            .collect();
        let mut color: HashMap<ChunkCoord, u8> = HashMap::new(); // 0=white, 1=gray, 2=black
        for node in &all_nodes {
            if *color.get(node).unwrap_or(&0) == 0 {
                if self.dfs_has_cycle(node, &mut color) { return true; }
            }
        }
        false
    }

    fn dfs_has_cycle(&self, node: &ChunkCoord, color: &mut HashMap<ChunkCoord, u8>) -> bool {
        color.insert(node.clone(), 1);
        if let Some(neighbors) = self.dependency_graph.get(node) {
            for neighbor in neighbors {
                let c = *color.get(neighbor).unwrap_or(&0);
                if c == 1 { return true; }
                if c == 0 && self.dfs_has_cycle(neighbor, color) { return true; }
            }
        }
        color.insert(node.clone(), 2);
        false
    }

    pub fn can_load(&self, coord: &ChunkCoord, loaded: &HashSet<ChunkCoord>) -> bool {
        if let Some(deps) = self.dependency_graph.get(coord) {
            deps.iter().all(|dep| loaded.contains(dep))
        } else {
            true
        }
    }
}

// ============================================================
// ASYNC LOAD QUEUE
// ============================================================

#[derive(Debug, Clone)]
pub struct AsyncLoadRequest {
    pub request_id: u64,
    pub coord: ChunkCoord,
    pub target_lod: LodLevel,
    pub priority: f32,
    pub frame_queued: u64,
    pub load_type: StreamingLoadType,
    pub is_cancelled: bool,
    pub retry_count: u32,
}

impl AsyncLoadRequest {
    pub fn new(id: u64, coord: ChunkCoord, lod: LodLevel, priority: f32, frame: u64) -> Self {
        Self {
            request_id: id,
            coord,
            target_lod: lod,
            priority,
            frame_queued: frame,
            load_type: StreamingLoadType::Asynchronous,
            is_cancelled: false,
            retry_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AsyncLoadQueue {
    pub pending: BTreeMap<u64, AsyncLoadRequest>,  // sorted by -priority*1e6 as key
    pub in_flight: HashMap<u64, AsyncLoadRequest>,
    pub completed: VecDeque<(u64, bool)>, // (request_id, success)
    pub cancelled: HashSet<u64>,
    pub next_id: u64,
    pub max_in_flight: usize,
    pub max_pending: usize,
}

impl AsyncLoadQueue {
    pub fn new(max_in_flight: usize, max_pending: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            in_flight: HashMap::new(),
            completed: VecDeque::new(),
            cancelled: HashSet::new(),
            next_id: 1,
            max_in_flight,
            max_pending,
        }
    }

    pub fn enqueue(&mut self, coord: ChunkCoord, lod: LodLevel, priority: f32, frame: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let req = AsyncLoadRequest::new(id, coord, lod, priority, frame);
        // Use negated priority * large factor as sort key for descending order
        let key = ((-priority * 1_000_000.0) as i64 as u64).wrapping_add(id);
        self.pending.insert(key, req);
        if self.pending.len() > self.max_pending {
            self.pending.pop_last();
        }
        id
    }

    pub fn cancel(&mut self, request_id: u64) {
        self.cancelled.insert(request_id);
        if let Some(req) = self.in_flight.get_mut(&request_id) {
            req.is_cancelled = true;
        }
        self.pending.retain(|_, req| req.request_id != request_id);
    }

    pub fn cancel_coord(&mut self, coord: &ChunkCoord) {
        let ids_to_cancel: Vec<u64> = self.in_flight.values()
            .filter(|r| &r.coord == coord)
            .map(|r| r.request_id)
            .chain(
                self.pending.values()
                    .filter(|r| &r.coord == coord)
                    .map(|r| r.request_id)
            )
            .collect();
        for id in ids_to_cancel { self.cancel(id); }
    }

    pub fn dispatch_available(&mut self, frame: u64) -> Vec<AsyncLoadRequest> {
        let to_dispatch = self.max_in_flight - self.in_flight.len().min(self.max_in_flight);
        let mut dispatched = Vec::new();
        let mut keys_to_remove = Vec::new();

        for (&key, req) in &self.pending {
            if dispatched.len() >= to_dispatch { break; }
            if self.cancelled.contains(&req.request_id) {
                keys_to_remove.push(key);
                continue;
            }
            keys_to_remove.push(key);
            dispatched.push(req.clone());
        }

        for key in keys_to_remove {
            if let Some(req) = self.pending.remove(&key) {
                if !self.cancelled.contains(&req.request_id) {
                    self.in_flight.insert(req.request_id, req);
                }
            }
        }

        dispatched
    }

    pub fn complete_request(&mut self, request_id: u64, success: bool) {
        self.in_flight.remove(&request_id);
        self.cancelled.remove(&request_id);
        self.completed.push_back((request_id, success));
        while self.completed.len() > 256 {
            self.completed.pop_front();
        }
    }

    pub fn drain_completed(&mut self) -> Vec<(u64, bool)> {
        self.completed.drain(..).collect()
    }

    pub fn is_pending(&self, coord: &ChunkCoord) -> bool {
        self.pending.values().any(|r| &r.coord == coord && !self.cancelled.contains(&r.request_id))
    }

    pub fn is_in_flight(&self, coord: &ChunkCoord) -> bool {
        self.in_flight.values().any(|r| &r.coord == coord && !r.is_cancelled)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn update_priority(&mut self, coord: &ChunkCoord, new_priority: f32) {
        let to_update: Vec<(u64, u64)> = self.pending.iter()
            .filter(|(_, r)| &r.coord == coord)
            .map(|(&k, r)| (k, r.request_id))
            .collect();
        for (old_key, req_id) in to_update {
            if let Some(mut req) = self.pending.remove(&old_key) {
                req.priority = new_priority;
                let new_key = ((-new_priority * 1_000_000.0) as i64 as u64).wrapping_add(req_id);
                self.pending.insert(new_key, req);
            }
        }
    }

    pub fn stale_requests(&self, current_frame: u64, max_age_frames: u64) -> Vec<u64> {
        self.pending.values()
            .filter(|r| current_frame.saturating_sub(r.frame_queued) > max_age_frames)
            .map(|r| r.request_id)
            .collect()
    }
}

// ============================================================
// STREAMING PROFILER
// ============================================================

#[derive(Debug, Clone)]
pub struct ProfilerEvent {
    pub event_type: ProfilerEventType,
    pub start_time_us: u64,
    pub duration_us: u64,
    pub frame: u64,
    pub metadata: u32,
}

#[derive(Debug, Clone)]
pub struct FrameProfileData {
    pub frame: u64,
    pub events: Vec<ProfilerEvent>,
    pub total_load_us: u64,
    pub total_unload_us: u64,
    pub total_cull_us: u64,
    pub total_lod_us: u64,
    pub chunks_loaded: u32,
    pub chunks_unloaded: u32,
}

impl FrameProfileData {
    pub fn new(frame: u64) -> Self {
        Self {
            frame,
            events: Vec::new(),
            total_load_us: 0,
            total_unload_us: 0,
            total_cull_us: 0,
            total_lod_us: 0,
            chunks_loaded: 0,
            chunks_unloaded: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamingProfiler {
    pub history: VecDeque<FrameProfileData>,
    pub current_frame_data: FrameProfileData,
    pub current_frame: u64,
    pub active_timers: HashMap<String, u64>,
    pub max_history: usize,
    pub total_events_recorded: u64,
    pub peak_load_time_us: u64,
    pub peak_frame_time_us: u64,
}

impl StreamingProfiler {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            current_frame_data: FrameProfileData::new(0),
            current_frame: 0,
            active_timers: HashMap::new(),
            max_history,
            total_events_recorded: 0,
            peak_load_time_us: 0,
            peak_frame_time_us: 0,
        }
    }

    pub fn begin_frame(&mut self, frame: u64) {
        self.current_frame = frame;
        self.current_frame_data = FrameProfileData::new(frame);
    }

    pub fn end_frame(&mut self) {
        let data = self.current_frame_data.clone();
        let frame_total = data.total_load_us + data.total_unload_us
            + data.total_cull_us + data.total_lod_us;
        if frame_total > self.peak_frame_time_us {
            self.peak_frame_time_us = frame_total;
        }
        if data.total_load_us > self.peak_load_time_us {
            self.peak_load_time_us = data.total_load_us;
        }
        self.history.push_back(data);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    pub fn record_event(&mut self, event_type: ProfilerEventType, start_us: u64, duration_us: u64, meta: u32) {
        let event = ProfilerEvent {
            event_type: event_type.clone(),
            start_time_us: start_us,
            duration_us,
            frame: self.current_frame,
            metadata: meta,
        };
        match event_type {
            ProfilerEventType::ChunkLoad => {
                self.current_frame_data.total_load_us += duration_us;
                self.current_frame_data.chunks_loaded += 1;
            }
            ProfilerEventType::ChunkUnload => {
                self.current_frame_data.total_unload_us += duration_us;
                self.current_frame_data.chunks_unloaded += 1;
            }
            ProfilerEventType::FrustumCull => {
                self.current_frame_data.total_cull_us += duration_us;
            }
            ProfilerEventType::LodSwitch => {
                self.current_frame_data.total_lod_us += duration_us;
            }
            _ => {}
        }
        self.current_frame_data.events.push(event);
        self.total_events_recorded += 1;
    }

    pub fn average_load_time_us(&self) -> f64 {
        if self.history.is_empty() { return 0.0; }
        let total: u64 = self.history.iter().map(|f| f.total_load_us).sum();
        total as f64 / self.history.len() as f64
    }

    pub fn average_chunks_per_frame(&self) -> f64 {
        if self.history.is_empty() { return 0.0; }
        let total: u32 = self.history.iter().map(|f| f.chunks_loaded).sum();
        total as f64 / self.history.len() as f64
    }

    pub fn compute_percentile_load_time(&self, pct: f64) -> u64 {
        let mut times: Vec<u64> = self.history.iter().map(|f| f.total_load_us).collect();
        times.sort_unstable();
        if times.is_empty() { return 0; }
        let idx = ((pct / 100.0) * (times.len() - 1) as f64) as usize;
        times[idx.min(times.len() - 1)]
    }

    pub fn frame_time_ms(&self, frame_idx: usize) -> f64 {
        if let Some(data) = self.history.get(frame_idx) {
            (data.total_load_us + data.total_unload_us + data.total_cull_us + data.total_lod_us) as f64 / 1000.0
        } else {
            0.0
        }
    }

    pub fn event_count_by_type(&self, event_type: &ProfilerEventType) -> usize {
        self.history.iter()
            .flat_map(|f| f.events.iter())
            .filter(|e| std::mem::discriminant(&e.event_type) == std::mem::discriminant(event_type))
            .count()
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn report_summary(&self) -> ProfilerSummary {
        ProfilerSummary {
            frames_recorded: self.history.len(),
            avg_load_time_us: self.average_load_time_us(),
            avg_chunks_per_frame: self.average_chunks_per_frame(),
            peak_load_time_us: self.peak_load_time_us,
            peak_frame_time_us: self.peak_frame_time_us,
            p99_load_time_us: self.compute_percentile_load_time(99.0),
            total_events: self.total_events_recorded,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfilerSummary {
    pub frames_recorded: usize,
    pub avg_load_time_us: f64,
    pub avg_chunks_per_frame: f64,
    pub peak_load_time_us: u64,
    pub peak_frame_time_us: u64,
    pub p99_load_time_us: u64,
    pub total_events: u64,
}

// ============================================================
// WORLD STREAMING EDITOR
// ============================================================

#[derive(Debug, Clone)]
pub struct WorldStreamingEditor {
    pub config: StreamingConfig,
    pub chunks: HashMap<ChunkCoord, StreamingChunk>,
    pub viewer_position: Vec3,
    pub viewer_forward: Vec3,
    pub camera: Option<StreamingCamera>,
    pub frustum: Option<FrustumCulling>,
    pub load_queue: AsyncLoadQueue,
    pub unload_queue: AsyncLoadQueue,
    pub memory_budget: MemoryBudget,
    pub streaming_stats: StreamingStats,
    pub profiler: StreamingProfiler,
    pub octree: Option<OctreeNode>,
    pub bvh: Option<BvhNode>,
    pub world_partition: WorldPartition,
    pub data_layers: DataLayerSystem,
    pub hlod_hierarchy: Vec<Vec<HlodCluster>>,
    pub virtual_texture: Option<VirtualTexture>,
    pub terrain_patches: HashMap<ChunkCoord, TerrainPatch>,
    pub actor_proxies: HashMap<u32, ActorStreamingProxy>,
    pub impostors: HashMap<u32, ImpostorBillboard>,
    pub chunk_dependencies: ChunkDependency,
    pub loaded_chunk_set: HashSet<ChunkCoord>,
    pub frame_number: u64,
    pub current_time_us: u64,
    pub pending_lod_transitions: Vec<(ChunkCoord, LodLevel, LodLevel)>,
    pub mesh_lods: HashMap<u32, ChunkMeshLod>,
    pub streaming_distance_calculator: StreamingDistanceCalculator,
    pub hlod_builder: HlodBuilder,
}

impl WorldStreamingEditor {
    pub fn new(config: StreamingConfig) -> Self {
        let memory_budget = MemoryBudget::new(config.max_memory_mb, config.eviction_policy.clone());
        let max_in_flight = config.max_concurrent_loads;
        let max_pending = MAX_ASYNC_LOAD_QUEUE;
        let screen_h = config.screen_height_pixels as f32;
        let fov_rad = config.fov_vertical_rad;
        let cell_size = WORLD_PARTITION_CELL_SIZE;
        let vt = if config.enable_virtual_textures {
            Some(VirtualTexture::new(VIRTUAL_TEXTURE_ATLAS_SIZE, VIRTUAL_TEXTURE_TILE_SIZE, FEEDBACK_BUFFER_MIPS as u32))
        } else {
            None
        };
        Self {
            config: config.clone(),
            chunks: HashMap::new(),
            viewer_position: Vec3::ZERO,
            viewer_forward: Vec3::NEG_Z,
            camera: None,
            frustum: None,
            load_queue: AsyncLoadQueue::new(max_in_flight, max_pending),
            unload_queue: AsyncLoadQueue::new(config.max_concurrent_unloads, max_pending / 4),
            memory_budget,
            streaming_stats: StreamingStats::new(),
            profiler: StreamingProfiler::new(PROFILER_HISTORY_FRAMES),
            octree: None,
            bvh: None,
            world_partition: WorldPartition::new(cell_size),
            data_layers: DataLayerSystem::new(),
            hlod_hierarchy: Vec::new(),
            virtual_texture: vt,
            terrain_patches: HashMap::new(),
            actor_proxies: HashMap::new(),
            impostors: HashMap::new(),
            chunk_dependencies: ChunkDependency::new(),
            loaded_chunk_set: HashSet::new(),
            frame_number: 0,
            current_time_us: 0,
            pending_lod_transitions: Vec::new(),
            mesh_lods: HashMap::new(),
            streaming_distance_calculator: StreamingDistanceCalculator::new(screen_h, fov_rad),
            hlod_builder: HlodBuilder::new(HLOD_CLUSTER_RADIUS, 32, 3),
        }
    }

    pub fn set_viewer(&mut self, position: Vec3, forward: Vec3) {
        self.viewer_position = position;
        self.viewer_forward = forward.normalize_or_zero();
        self.world_partition.clear_streaming_sources();
        self.world_partition.add_streaming_source(position);
    }

    pub fn set_camera(&mut self, camera: StreamingCamera) {
        let planes = camera.frustum_planes;
        self.camera = Some(camera);
        self.frustum = Some(FrustumCulling::new(planes));
    }

    pub fn tick(&mut self, delta_time_s: f32, time_us: u64) {
        self.current_time_us = time_us;
        self.profiler.begin_frame(self.frame_number);
        self.streaming_stats.advance_frame();

        self.update_chunk_visibility();
        self.update_chunk_lods();
        self.process_load_queue();
        self.process_unload_queue();
        self.evict_memory_if_needed();
        self.update_terrain_stitching();
        self.update_impostors();
        self.update_virtual_textures();

        self.profiler.end_frame();
        self.frame_number += 1;
    }

    fn update_chunk_visibility(&mut self) {
        let viewer = self.viewer_position;
        let frame = self.frame_number;
        let streaming_radius = self.config.streaming_radius;
        let vertical_radius = self.config.vertical_streaming_radius;

        let viewer_coord = ChunkCoord::from_world_pos(viewer, self.config.chunk_size);
        let radius_chunks = self.config.streaming_radius_chunks();

        let candidates = ChunkCoord::chunks_in_radius(&viewer_coord, radius_chunks);

        // Ensure chunks exist for all candidates in radius
        for coord in &candidates {
            self.chunks.entry(coord.clone()).or_insert_with(|| {
                StreamingChunk::new(coord.clone(), self.config.chunk_size)
            });
        }

        let frustum = self.frustum.clone();
        let cull_enabled = self.config.enable_frustum_culling;

        for (coord, chunk) in self.chunks.iter_mut() {
            chunk.update_distance(viewer);

            let vert_dist = (chunk.bounds.center().y - viewer.y).abs();
            if vert_dist > vertical_radius {
                chunk.mark_not_visible();
                continue;
            }
            if chunk.distance_to_viewer > streaming_radius {
                chunk.mark_not_visible();
                continue;
            }
            if cull_enabled {
                if let Some(ref frust) = frustum {
                    if !frust.test_aabb_fast(&chunk.bounds) {
                        chunk.mark_not_visible();
                        continue;
                    }
                }
            }
            chunk.mark_visible(frame);
            chunk.compute_load_priority(viewer, self.viewer_forward);
        }
    }

    fn update_chunk_lods(&mut self) {
        let viewer = self.viewer_position;
        let screen_h = self.config.screen_height_pixels as f32;

        let camera_opt = self.camera.clone();
        let config = self.config.clone();
        let frame = self.frame_number;

        let mut transitions = Vec::new();

        for (coord, chunk) in self.chunks.iter_mut() {
            if !chunk.is_visible || chunk.load_state == ChunkLoadState::Unloaded {
                continue;
            }
            let desired_lod = if let Some(ref cam) = camera_opt {
                let screen_size = cam.compute_lod_screen_size(&chunk.bounds, screen_h);
                ChunkMeshLod::new(10000, config.chunk_size).select_lod_for_screen_size(screen_size)
            } else {
                config.lod_for_distance(chunk.distance_to_viewer)
            };

            if desired_lod != chunk.lod_level && chunk.last_loaded_frame + config.min_lod_retain_frames <= frame {
                transitions.push((coord.clone(), chunk.lod_level.clone(), desired_lod));
            }
        }
        self.pending_lod_transitions = transitions;
        for (coord, old_lod, new_lod) in &self.pending_lod_transitions {
            if let Some(chunk) = self.chunks.get_mut(coord) {
                chunk.lod_level = new_lod.clone();
                chunk.flags.needs_lod_update = true;
            }
            self.streaming_stats.chunks_lod_switched_this_frame += 1;
        }
    }

    fn process_load_queue(&mut self) {
        let frame = self.frame_number;
        let viewer = self.viewer_position;

        // Re-prioritize pending requests
        let chunk_size = self.config.chunk_size;
        self.load_queue.update_priority(&ChunkCoord::new(0,0,0), 0.0); // trigger no-op

        // Enqueue visible unloaded chunks
        let mut to_enqueue: Vec<(ChunkCoord, LodLevel, f32)> = Vec::new();
        for (coord, chunk) in &self.chunks {
            if !chunk.is_visible { continue; }
            if chunk.load_state != ChunkLoadState::Unloaded { continue; }
            if self.load_queue.is_pending(coord) || self.load_queue.is_in_flight(coord) { continue; }
            let desired_lod = self.config.lod_for_distance(chunk.distance_to_viewer);
            if desired_lod == LodLevel::Unloaded { continue; }
            if !self.chunk_dependencies.can_load(coord, &self.loaded_chunk_set) { continue; }
            to_enqueue.push((coord.clone(), desired_lod, chunk.load_priority));
        }

        for (coord, lod, priority) in to_enqueue {
            let id = self.load_queue.enqueue(coord.clone(), lod, priority, frame);
            if let Some(chunk) = self.chunks.get_mut(&coord) {
                chunk.load_state = ChunkLoadState::Queued;
            }
        }

        // Dispatch available slots
        let dispatched = self.load_queue.dispatch_available(frame);
        for req in dispatched {
            if let Some(chunk) = self.chunks.get_mut(&req.coord) {
                chunk.load_state = ChunkLoadState::Loading;
            }
            // Simulate instant load completion for editor purposes
            let est_bytes = self.estimate_chunk_memory(&req.coord, &req.target_lod);
            let can_alloc = self.memory_budget.can_allocate(est_bytes);
            let success = can_alloc;
            if success {
                self.memory_budget.allocate(req.coord.clone(), req.target_lod.clone(), est_bytes, frame);
                if let Some(chunk) = self.chunks.get_mut(&req.coord) {
                    chunk.load_state = ChunkLoadState::Loaded;
                    chunk.lod_level = req.target_lod.clone();
                    chunk.memory_bytes = est_bytes;
                    chunk.last_loaded_frame = frame;
                }
                self.loaded_chunk_set.insert(req.coord.clone());
                self.streaming_stats.chunks_loaded_this_frame += 1;
            }
            self.load_queue.complete_request(req.request_id, success);
        }
    }

    fn process_unload_queue(&mut self) {
        let viewer = self.viewer_position;
        let streaming_radius = self.config.streaming_radius;
        let frame = self.frame_number;

        // Mark distant loaded chunks for eviction
        let mut to_evict: Vec<ChunkCoord> = Vec::new();
        for (coord, chunk) in &self.chunks {
            if !chunk.can_evict() { continue; }
            if chunk.distance_to_viewer > streaming_radius * 1.1 {
                to_evict.push(coord.clone());
            }
        }

        for coord in &to_evict {
            if !self.unload_queue.is_pending(coord) && !self.unload_queue.is_in_flight(coord) {
                let priority = if let Some(chunk) = self.chunks.get(coord) { chunk.distance_to_viewer } else { 0.0 };
                self.unload_queue.enqueue(coord.clone(), LodLevel::Unloaded, priority, frame);
                if let Some(chunk) = self.chunks.get_mut(coord) {
                    chunk.load_state = ChunkLoadState::Evicting;
                }
            }
        }

        let dispatched = self.unload_queue.dispatch_available(frame);
        for req in dispatched {
            let old_lod = self.chunks.get(&req.coord).map(|c| c.lod_level.clone()).unwrap_or(LodLevel::Unloaded);
            self.memory_budget.free(&req.coord, &old_lod);
            if let Some(chunk) = self.chunks.get_mut(&req.coord) {
                chunk.load_state = ChunkLoadState::Unloaded;
                chunk.lod_level = LodLevel::Unloaded;
                chunk.memory_bytes = 0;
            }
            self.loaded_chunk_set.remove(&req.coord);
            self.streaming_stats.chunks_unloaded_this_frame += 1;
            self.unload_queue.complete_request(req.request_id, true);
        }
    }

    fn evict_memory_if_needed(&mut self) {
        if !self.memory_budget.needs_eviction() { return; }
        let frame = self.frame_number;
        let candidates = self.memory_budget.eviction_candidates(8, frame);
        for coord in candidates {
            if let Some(chunk) = self.chunks.get(&coord) {
                if chunk.can_evict() && !chunk.is_visible {
                    let lod = chunk.lod_level.clone();
                    self.memory_budget.free(&coord, &lod);
                    if let Some(c) = self.chunks.get_mut(&coord) {
                        c.load_state = ChunkLoadState::Unloaded;
                        c.lod_level = LodLevel::Unloaded;
                        c.memory_bytes = 0;
                    }
                    self.loaded_chunk_set.remove(&coord);
                }
            }
        }
    }

    fn update_terrain_stitching(&mut self) {
        let needs_stitch: Vec<ChunkCoord> = self.terrain_patches
            .iter()
            .filter(|(_, p)| p.needs_stitch)
            .map(|(c, _)| c.clone())
            .collect();

        for coord in needs_stitch {
            let neighbors_6 = coord.neighbors_6();
            let neighbor_seams_and_lods: Vec<_> = [0usize, 1, 2, 3].iter().map(|&side| {
                let ncoord = neighbors_6[side * 2];
                let seam = self.terrain_patches.get(&ncoord).map(|p| p.seam_data[1 - side % 2].clone()).unwrap_or_default();
                let lod = self.terrain_patches.get(&ncoord).map(|p| p.lod_level.clone());
                (seam, lod)
            }).collect();

            if let Some(patch) = self.terrain_patches.get_mut(&coord) {
                for side in 0..4 {
                    if let (seam, Some(neighbor_lod)) = &neighbor_seams_and_lods[side] {
                        if !seam.is_empty() {
                            patch.stitch_edge(side, seam, neighbor_lod);
                        }
                    }
                }
                self.streaming_stats.terrain_stitch_ops += 1;
            }
        }
    }

    fn update_impostors(&mut self) {
        if !self.config.enable_impostor_billboards { return; }
        let viewer = self.viewer_position;
        let frame = self.frame_number;
        for (_, impostor) in self.impostors.iter_mut() {
            if impostor.should_update(viewer, 5.0) {
                impostor.update_view_index(viewer);
                impostor.clear_dirty(frame);
            }
        }
    }

    fn update_virtual_textures(&mut self) {
        if !self.config.enable_virtual_textures { return; }
        if let Some(ref mut vt) = self.virtual_texture {
            let requests = vt.analyze_feedback_buffer();
            let to_load: Vec<_> = requests.iter().take(8).filter(|r| r.3 > 0.01).cloned().collect();
            for (mip, tx, ty, _) in to_load {
                vt.load_tile(mip, tx, ty);
                self.streaming_stats.virtual_texture_uploads += 1;
            }
            vt.evict_lru_tiles(vt.max_resident_tiles / 4);
            vt.clear_feedback();
        }
    }

    pub fn estimate_chunk_memory(&self, coord: &ChunkCoord, lod: &LodLevel) -> u64 {
        let base = 4 * 1024 * 1024u64; // 4 MB base
        (base as f32 * lod.memory_multiplier()) as u64
    }

    pub fn register_actor(&mut self, actor_id: u32, position: Vec3, bounds: ChunkBounds, streaming_dist: f32) {
        let mut proxy = ActorStreamingProxy::new(actor_id, position, bounds);
        proxy.streaming_distance = streaming_dist;
        self.actor_proxies.insert(actor_id, proxy);
        self.world_partition.register_actor(actor_id, position);
    }

    pub fn unregister_actor(&mut self, actor_id: u32) {
        self.actor_proxies.remove(&actor_id);
        self.world_partition.unregister_actor(actor_id);
    }

    pub fn add_terrain_patch(&mut self, coord: ChunkCoord, size: usize, cell_size: f32) {
        let patch = TerrainPatch::new(coord.clone(), size, cell_size);
        self.terrain_patches.insert(coord, patch);
    }

    pub fn build_octree(&mut self) {
        let points: Vec<(u32, Vec3)> = self.actor_proxies.iter()
            .map(|(&id, proxy)| (id, proxy.world_position))
            .collect();
        let builder = OctreeBuilder::new(MAX_OCTREE_DEPTH, MAX_OBJECTS_PER_OCTREE_NODE);
        self.octree = Some(builder.build(&points));
    }

    pub fn build_bvh(&mut self) {
        let objects: Vec<(u32, ChunkBounds)> = self.actor_proxies.iter()
            .map(|(&id, proxy)| (id, proxy.bounds.clone()))
            .collect();
        let builder = BvhBuilder::new(BVH_MAX_LEAF_OBJECTS, 16);
        self.bvh = builder.build(&objects);
    }

    pub fn build_hlod(&mut self) {
        let actors: Vec<(u32, Vec3, ChunkBounds)> = self.actor_proxies.iter()
            .map(|(&id, proxy)| (id, proxy.world_position, proxy.bounds.clone()))
            .collect();
        self.hlod_hierarchy = self.hlod_builder.build_hierarchical(&actors);
        self.streaming_stats.hlod_merges += 1;
    }

    pub fn query_visible_objects(&self) -> Vec<u32> {
        let mut result = Vec::new();
        if let Some(ref frustum) = self.frustum {
            if let Some(ref bvh) = self.bvh {
                bvh.query_frustum(frustum, &mut result);
            }
        }
        result
    }

    pub fn query_objects_in_radius(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let mut result = Vec::new();
        if let Some(ref octree) = self.octree {
            octree.query_sphere(center, radius, &mut result);
        }
        result
    }

    pub fn query_objects_in_bounds(&self, bounds: &ChunkBounds) -> Vec<u32> {
        let mut result = Vec::new();
        if let Some(ref bvh) = self.bvh {
            bvh.query_aabb(bounds, &mut result);
        }
        result
    }

    pub fn get_chunk_lod(&self, coord: &ChunkCoord) -> LodLevel {
        self.chunks.get(coord).map(|c| c.lod_level.clone()).unwrap_or(LodLevel::Unloaded)
    }

    pub fn get_chunk_state(&self, coord: &ChunkCoord) -> ChunkLoadState {
        self.chunks.get(coord).map(|c| c.load_state.clone()).unwrap_or(ChunkLoadState::Unloaded)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.loaded_chunk_set.len()
    }

    pub fn memory_usage_mb(&self) -> f32 {
        self.memory_budget.total_used_mb()
    }

    pub fn stats(&self) -> &StreamingStats {
        &self.streaming_stats
    }

    pub fn profiler_summary(&self) -> ProfilerSummary {
        self.profiler.report_summary()
    }

    pub fn set_lod_bias(&mut self, bias: f32) {
        self.config.lod_bias = bias;
        self.streaming_distance_calculator.lod_bias = bias;
    }

    pub fn force_lod(&mut self, coord: &ChunkCoord, lod: LodLevel) {
        if let Some(chunk) = self.chunks.get_mut(coord) {
            let old = chunk.lod_level.clone();
            chunk.lod_level = lod.clone();
            chunk.flags.needs_lod_update = true;
            self.pending_lod_transitions.push((coord.clone(), old, lod));
        }
    }

    pub fn get_terrain_height(&self, world_x: f32, world_z: f32) -> f32 {
        let coord = ChunkCoord::from_world_pos(
            Vec3::new(world_x, 0.0, world_z),
            self.config.chunk_size,
        );
        if let Some(patch) = self.terrain_patches.get(&coord) {
            patch.heightmap.sample_bilinear(world_x, world_z)
        } else {
            0.0
        }
    }

    pub fn get_terrain_normal(&self, world_x: f32, world_z: f32) -> Vec3 {
        let coord = ChunkCoord::from_world_pos(
            Vec3::new(world_x, 0.0, world_z),
            self.config.chunk_size,
        );
        if let Some(patch) = self.terrain_patches.get(&coord) {
            patch.heightmap.compute_normal_bilinear(world_x, world_z)
        } else {
            Vec3::Y
        }
    }

    pub fn compute_streaming_bounds(&self) -> ChunkBounds {
        let half_r = Vec3::new(
            self.config.streaming_radius,
            self.config.vertical_streaming_radius,
            self.config.streaming_radius,
        );
        ChunkBounds::from_center_size(self.viewer_position, half_r)
    }

    pub fn debug_draw_chunks(&self) -> Vec<(ChunkBounds, Vec3, LodLevel)> {
        self.chunks.values()
            .filter(|c| c.load_state != ChunkLoadState::Unloaded)
            .map(|c| {
                let color = match c.lod_level {
                    LodLevel::Ultra    => Vec3::new(0.0, 1.0, 0.0),
                    LodLevel::High     => Vec3::new(0.5, 1.0, 0.0),
                    LodLevel::Medium   => Vec3::new(1.0, 1.0, 0.0),
                    LodLevel::Low      => Vec3::new(1.0, 0.5, 0.0),
                    LodLevel::Impostor => Vec3::new(1.0, 0.0, 0.0),
                    LodLevel::Unloaded => Vec3::new(0.3, 0.3, 0.3),
                };
                (c.bounds.clone(), color, c.lod_level.clone())
            })
            .collect()
    }
}

// ============================================================
// ADDITIONAL MATH UTILITIES
// ============================================================

pub fn compute_view_matrix(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_rh(eye, target, up)
}

pub fn compute_projection_matrix(fov_y_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    Mat4::perspective_rh(fov_y_deg.to_radians(), aspect, near, far)
}

pub fn compute_ortho_matrix(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    Mat4::orthographic_rh(left, right, bottom, top, near, far)
}

pub fn screen_to_world_ray(
    screen_x: f32,
    screen_y: f32,
    screen_w: f32,
    screen_h: f32,
    inv_view_proj: Mat4,
) -> (Vec3, Vec3) {
    let ndc_x = (screen_x / screen_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_y / screen_h) * 2.0;
    let near_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, -1.0));
    let far_point  = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y,  1.0));
    let dir = (far_point - near_point).normalize();
    (near_point, dir)
}

pub fn compute_sphere_screen_radius(center: Vec3, radius: f32, proj_matrix: Mat4, screen_height: f32) -> f32 {
    let proj_center = proj_matrix.project_point3(center);
    let proj_edge   = proj_matrix.project_point3(center + Vec3::new(radius, 0.0, 0.0));
    let screen_radius = (proj_center - proj_edge).length() * screen_height * 0.5;
    screen_radius.abs()
}

pub fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

pub fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn smoother_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn project_point_to_plane(point: Vec3, plane_normal: Vec3, plane_d: f32) -> Vec3 {
    let dist = plane_normal.dot(point) + plane_d;
    point - plane_normal * dist
}

pub fn distance_point_to_line(point: Vec3, line_origin: Vec3, line_dir: Vec3) -> f32 {
    let v = point - line_origin;
    let d = v - line_dir * v.dot(line_dir);
    d.length()
}

pub fn closest_point_on_segment(point: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-12 { return a; }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

pub fn triangle_area(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (b - a).cross(c - a).length() * 0.5
}

pub fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (b - a).cross(c - a).normalize()
}

pub fn barycentric_coords(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = point - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-12 { return Vec3::new(1.0, 0.0, 0.0); }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    Vec3::new(1.0 - v - w, v, w)
}

pub fn point_in_triangle(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
    let bary = barycentric_coords(point, a, b, c);
    bary.x >= 0.0 && bary.y >= 0.0 && bary.z >= 0.0
}

pub fn compute_tangent_space(pos0: Vec3, pos1: Vec3, pos2: Vec3, uv0: Vec2, uv1: Vec2, uv2: Vec2) -> (Vec3, Vec3) {
    let edge1 = pos1 - pos0;
    let edge2 = pos2 - pos0;
    let duv1  = uv1 - uv0;
    let duv2  = uv2 - uv0;
    let denom = duv1.x * duv2.y - duv2.x * duv1.y;
    if denom.abs() < 1e-12 {
        return (Vec3::X, Vec3::Y);
    }
    let inv = 1.0 / denom;
    let tangent   = (edge1 * duv2.y - edge2 * duv1.y) * inv;
    let bitangent = (edge2 * duv1.x - edge1 * duv2.x) * inv;
    (tangent.normalize(), bitangent.normalize())
}

pub fn compute_lod_bias_from_mip(mip: u32, base_mip: u32) -> f32 {
    if mip <= base_mip { 0.0 } else { (mip - base_mip) as f32 }
}

// ============================================================
// NOISE UTILITIES (for terrain generation)
// ============================================================

pub fn hash_2d(x: i32, y: i32) -> u32 {
    let mut h = x.wrapping_mul(1234567891i32).wrapping_add(y.wrapping_mul(987654321i32)) as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h
}

pub fn hash_3d(x: i32, y: i32, z: i32) -> u32 {
    let mut h = x.wrapping_mul(1234567891i32)
        .wrapping_add(y.wrapping_mul(987654321i32))
        .wrapping_add(z.wrapping_mul(741852963i32)) as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h
}

pub fn value_noise_2d(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = smooth_step(0.0, 1.0, fx);
    let uy = smooth_step(0.0, 1.0, fy);
    let v00 = (hash_2d(ix,     iy    ) as f32) / u32::MAX as f32;
    let v10 = (hash_2d(ix + 1, iy    ) as f32) / u32::MAX as f32;
    let v01 = (hash_2d(ix,     iy + 1) as f32) / u32::MAX as f32;
    let v11 = (hash_2d(ix + 1, iy + 1) as f32) / u32::MAX as f32;
    let h0 = v00 * (1.0 - ux) + v10 * ux;
    let h1 = v01 * (1.0 - ux) + v11 * ux;
    h0 * (1.0 - uy) + h1 * uy
}

pub fn fbm_noise_2d(x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut sum = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max_amp = 0.0f32;
    for _ in 0..octaves {
        sum += value_noise_2d(x * frequency, y * frequency) * amplitude;
        max_amp += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    if max_amp > 0.0 { sum / max_amp } else { 0.0 }
}

pub fn generate_heightmap_fbm(
    width: usize,
    height: usize,
    cell_size: f32,
    origin: Vec2,
    octaves: u32,
    scale: f32,
    amplitude: f32,
) -> TerrainHeightmap {
    let mut hm = TerrainHeightmap::new(width, height, cell_size, origin, 1.0);
    for z in 0..height {
        for x in 0..width {
            let wx = (origin.x + x as f32 * cell_size) * scale;
            let wz = (origin.y + z as f32 * cell_size) * scale;
            let h = fbm_noise_2d(wx, wz, octaves, 2.0, 0.5) * amplitude;
            hm.set_height(x, z, h);
        }
    }
    hm
}

// ============================================================
// LOD TRANSITION MANAGER
// ============================================================

#[derive(Debug, Clone)]
pub struct LodTransitionState {
    pub coord: ChunkCoord,
    pub from_lod: LodLevel,
    pub to_lod: LodLevel,
    pub blend_alpha: f32,
    pub transition_duration_frames: u32,
    pub frames_elapsed: u32,
}

impl LodTransitionState {
    pub fn new(coord: ChunkCoord, from: LodLevel, to: LodLevel, duration_frames: u32) -> Self {
        Self {
            coord,
            from_lod: from,
            to_lod: to,
            blend_alpha: 0.0,
            transition_duration_frames: duration_frames,
            frames_elapsed: 0,
        }
    }

    pub fn advance(&mut self) -> bool {
        self.frames_elapsed += 1;
        self.blend_alpha = if self.transition_duration_frames > 0 {
            (self.frames_elapsed as f32 / self.transition_duration_frames as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.blend_alpha >= 1.0
    }

    pub fn is_complete(&self) -> bool {
        self.blend_alpha >= 1.0
    }

    pub fn smooth_alpha(&self) -> f32 {
        smooth_step(0.0, 1.0, self.blend_alpha)
    }
}

#[derive(Debug, Clone)]
pub struct LodTransitionManager {
    pub active_transitions: HashMap<ChunkCoord, LodTransitionState>,
    pub transition_duration_frames: u32,
    pub enable_smooth_transitions: bool,
}

impl LodTransitionManager {
    pub fn new(duration_frames: u32, smooth: bool) -> Self {
        Self {
            active_transitions: HashMap::new(),
            transition_duration_frames: duration_frames,
            enable_smooth_transitions: smooth,
        }
    }

    pub fn begin_transition(&mut self, coord: ChunkCoord, from: LodLevel, to: LodLevel) {
        let duration = if self.enable_smooth_transitions { self.transition_duration_frames } else { 0 };
        let state = LodTransitionState::new(coord.clone(), from, to, duration);
        self.active_transitions.insert(coord, state);
    }

    pub fn tick(&mut self) -> Vec<ChunkCoord> {
        let mut completed = Vec::new();
        for (coord, state) in self.active_transitions.iter_mut() {
            if state.advance() {
                completed.push(coord.clone());
            }
        }
        for c in &completed {
            self.active_transitions.remove(c);
        }
        completed
    }

    pub fn get_blend_alpha(&self, coord: &ChunkCoord) -> f32 {
        self.active_transitions.get(coord).map(|s| s.smooth_alpha()).unwrap_or(1.0)
    }

    pub fn is_transitioning(&self, coord: &ChunkCoord) -> bool {
        self.active_transitions.contains_key(coord)
    }

    pub fn active_count(&self) -> usize {
        self.active_transitions.len()
    }

    pub fn cancel_transition(&mut self, coord: &ChunkCoord) {
        self.active_transitions.remove(coord);
    }
}

// ============================================================
// VISIBILITY GRID (broad-phase spatial hash)
// ============================================================

#[derive(Debug, Clone)]
pub struct VisibilityGrid {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32, i32), Vec<u32>>,
    pub object_cells: HashMap<u32, (i32, i32, i32)>,
}

impl VisibilityGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            object_cells: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: u32, pos: Vec3) {
        let cell = self.pos_to_cell(pos);
        self.cells.entry(cell).or_insert_with(Vec::new).push(id);
        self.object_cells.insert(id, cell);
    }

    pub fn remove(&mut self, id: u32) {
        if let Some(cell) = self.object_cells.remove(&id) {
            if let Some(ids) = self.cells.get_mut(&cell) {
                ids.retain(|&i| i != id);
            }
        }
    }

    pub fn update(&mut self, id: u32, new_pos: Vec3) {
        let new_cell = self.pos_to_cell(new_pos);
        if let Some(old_cell) = self.object_cells.get(&id).cloned() {
            if old_cell == new_cell { return; }
            if let Some(ids) = self.cells.get_mut(&old_cell) {
                ids.retain(|&i| i != id);
            }
        }
        self.cells.entry(new_cell).or_insert_with(Vec::new).push(id);
        self.object_cells.insert(id, new_cell);
    }

    pub fn query_radius(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let cells = self.cells_in_radius(center, radius);
        let mut result = Vec::new();
        for cell in cells {
            if let Some(ids) = self.cells.get(&cell) {
                result.extend_from_slice(ids);
            }
        }
        result
    }

    pub fn query_aabb(&self, bounds: &ChunkBounds) -> Vec<u32> {
        let cells = self.cells_in_aabb(bounds);
        let mut result = Vec::new();
        for cell in cells {
            if let Some(ids) = self.cells.get(&cell) {
                result.extend_from_slice(ids);
            }
        }
        result
    }

    fn pos_to_cell(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    fn cells_in_radius(&self, center: Vec3, radius: f32) -> Vec<(i32, i32, i32)> {
        let cr = (radius / self.cell_size).ceil() as i32;
        let cc = self.pos_to_cell(center);
        let mut cells = Vec::new();
        for dx in -cr..=cr {
            for dy in -cr..=cr {
                for dz in -cr..=cr {
                    cells.push((cc.0 + dx, cc.1 + dy, cc.2 + dz));
                }
            }
        }
        cells
    }

    fn cells_in_aabb(&self, bounds: &ChunkBounds) -> Vec<(i32, i32, i32)> {
        let min_c = self.pos_to_cell(bounds.min);
        let max_c = self.pos_to_cell(bounds.max);
        let mut cells = Vec::new();
        for cx in min_c.0..=max_c.0 {
            for cy in min_c.1..=max_c.1 {
                for cz in min_c.2..=max_c.2 {
                    cells.push((cx, cy, cz));
                }
            }
        }
        cells
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn object_count(&self) -> usize {
        self.object_cells.len()
    }
}

// ============================================================
// SCREEN SPACE ERROR METRIC
// ============================================================

#[derive(Debug, Clone)]
pub struct ScreenSpaceErrorMetric {
    pub screen_height_pixels: f32,
    pub fov_vertical_rad: f32,
    pub error_threshold: f32,
}

impl ScreenSpaceErrorMetric {
    pub fn new(screen_height: f32, fov_rad: f32, threshold: f32) -> Self {
        Self {
            screen_height_pixels: screen_height,
            fov_vertical_rad: fov_rad,
            error_threshold: threshold,
        }
    }

    pub fn project_error(&self, world_error: f32, distance: f32) -> f32 {
        if distance < 1e-6 { return f32::MAX; }
        let half_fov_tan = (self.fov_vertical_rad * 0.5).tan();
        (world_error / distance) / half_fov_tan * self.screen_height_pixels
    }

    pub fn needs_refinement(&self, world_error: f32, distance: f32) -> bool {
        self.project_error(world_error, distance) > self.error_threshold
    }

    pub fn select_lod_for_bounds(&self, bounds: &ChunkBounds, camera_pos: Vec3) -> LodLevel {
        let dist = bounds.distance_to_point(camera_pos);
        let radius = bounds.half_size().length();
        let screen_size = self.project_error(radius, dist.max(0.001));
        if screen_size > 512.0       { LodLevel::Ultra   }
        else if screen_size > 256.0  { LodLevel::High    }
        else if screen_size > 64.0   { LodLevel::Medium  }
        else if screen_size > 16.0   { LodLevel::Low     }
        else if screen_size > 2.0    { LodLevel::Impostor}
        else                         { LodLevel::Unloaded}
    }

    pub fn blend_factor(&self, world_error: f32, distance: f32) -> f32 {
        let sse = self.project_error(world_error, distance);
        let near = self.error_threshold * 0.5;
        let far  = self.error_threshold * 2.0;
        smooth_step(near, far, sse)
    }

    pub fn max_error_for_distance(&self, distance: f32) -> f32 {
        let half_fov_tan = (self.fov_vertical_rad * 0.5).tan();
        self.error_threshold * distance * half_fov_tan / self.screen_height_pixels
    }
}

// ============================================================
// CHUNK POOL
// ============================================================

#[derive(Debug, Clone)]
pub struct ChunkPool {
    pub free_chunks: VecDeque<StreamingChunk>,
    pub allocated_count: usize,
    pub chunk_size: f32,
    pub pool_capacity: usize,
}

impl ChunkPool {
    pub fn new(capacity: usize, chunk_size: f32) -> Self {
        Self {
            free_chunks: VecDeque::with_capacity(capacity),
            allocated_count: 0,
            chunk_size,
            pool_capacity: capacity,
        }
    }

    pub fn pre_allocate(&mut self, count: usize) {
        for i in 0..count.min(self.pool_capacity) {
            let coord = ChunkCoord::new(i as i32, 0, 0);
            self.free_chunks.push_back(StreamingChunk::new(coord, self.chunk_size));
        }
    }

    pub fn acquire(&mut self, coord: ChunkCoord) -> StreamingChunk {
        if let Some(mut chunk) = self.free_chunks.pop_front() {
            chunk.coord = coord.clone();
            chunk.bounds = ChunkBounds::from_chunk_coord(&coord, self.chunk_size);
            chunk.lod_level = LodLevel::Unloaded;
            chunk.load_state = ChunkLoadState::Unloaded;
            chunk.resident_objects.clear();
            chunk.memory_bytes = 0;
            chunk.is_visible = false;
            chunk.flags = ChunkFlags::default();
            self.allocated_count += 1;
            chunk
        } else {
            self.allocated_count += 1;
            StreamingChunk::new(coord, self.chunk_size)
        }
    }

    pub fn release(&mut self, chunk: StreamingChunk) {
        if self.free_chunks.len() < self.pool_capacity {
            self.free_chunks.push_back(chunk);
        }
        if self.allocated_count > 0 { self.allocated_count -= 1; }
    }

    pub fn free_count(&self) -> usize {
        self.free_chunks.len()
    }

    pub fn allocated_count(&self) -> usize {
        self.allocated_count
    }
}

// ============================================================
// STREAMING MESH SIMPLIFIER
// ============================================================

#[derive(Debug, Clone)]
pub struct MeshVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

#[derive(Debug, Clone)]
pub struct SimplifiedMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub lod_level: LodLevel,
    pub error_metric: f32,
}

impl SimplifiedMesh {
    pub fn new(lod: LodLevel) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            lod_level: lod,
            error_metric: 0.0,
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn memory_bytes(&self) -> usize {
        // Each vertex: 3+3+2 = 8 floats = 32 bytes; each index: 4 bytes
        self.vertices.len() * 32 + self.indices.len() * 4
    }

    pub fn compute_bounds(&self) -> ChunkBounds {
        if self.vertices.is_empty() {
            return ChunkBounds::new(Vec3::ZERO, Vec3::ZERO);
        }
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &self.vertices {
            min = min.min(v.position);
            max = max.max(v.position);
        }
        ChunkBounds { min, max }
    }

    pub fn recompute_normals(&mut self) {
        let n = self.vertices.len();
        let mut normals = vec![Vec3::ZERO; n];
        for tri in self.indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            if i0 >= n || i1 >= n || i2 >= n { continue; }
            let p0 = self.vertices[i0].position;
            let p1 = self.vertices[i1].position;
            let p2 = self.vertices[i2].position;
            let normal = triangle_normal(p0, p1, p2);
            normals[i0] += normal;
            normals[i1] += normal;
            normals[i2] += normal;
        }
        for (i, v) in self.vertices.iter_mut().enumerate() {
            let len = normals[i].length();
            if len > NORMAL_SMOOTH_EPSILON {
                v.normal = normals[i] / len;
            }
        }
    }
}

// ============================================================
// STREAMING EVENT SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub enum StreamingEvent {
    ChunkLoaded { coord: ChunkCoord, lod: LodLevel },
    ChunkUnloaded { coord: ChunkCoord },
    LodChanged { coord: ChunkCoord, from: LodLevel, to: LodLevel },
    MemoryWarning { used_mb: f32, budget_mb: f32 },
    LoadQueueFull { size: usize },
    ActorLoaded { actor_id: u32, lod: LodLevel },
    ActorUnloaded { actor_id: u32 },
    HlodActivated { cluster_id: u32 },
    HlodDeactivated { cluster_id: u32 },
    VirtualTextureEviction { tile_count: u32 },
}

#[derive(Debug, Clone)]
pub struct StreamingEventQueue {
    pub events: VecDeque<StreamingEvent>,
    pub max_events: usize,
    pub total_events_enqueued: u64,
    pub total_events_dequeued: u64,
}

impl StreamingEventQueue {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(max_events),
            max_events,
            total_events_enqueued: 0,
            total_events_dequeued: 0,
        }
    }

    pub fn push(&mut self, event: StreamingEvent) {
        if self.events.len() >= self.max_events {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.total_events_enqueued += 1;
    }

    pub fn drain(&mut self) -> Vec<StreamingEvent> {
        let count = self.events.len();
        self.total_events_dequeued += count as u64;
        self.events.drain(..).collect()
    }

    pub fn peek(&self) -> Option<&StreamingEvent> {
        self.events.front()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn count_by_type(&self, discriminant: &str) -> usize {
        self.events.iter().filter(|e| {
            match (e, discriminant) {
                (StreamingEvent::ChunkLoaded { .. }, "ChunkLoaded") => true,
                (StreamingEvent::ChunkUnloaded { .. }, "ChunkUnloaded") => true,
                (StreamingEvent::LodChanged { .. }, "LodChanged") => true,
                _ => false,
            }
        }).count()
    }
}

// ============================================================
// SCENE GRAPH NODE
// ============================================================

#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id: u32,
    pub local_transform: Mat4,
    pub world_transform: Mat4,
    pub bounds: ChunkBounds,
    pub children: Vec<u32>,
    pub parent: Option<u32>,
    pub lod_level: LodLevel,
    pub is_visible: bool,
    pub is_static: bool,
    pub chunk_coord: Option<ChunkCoord>,
}

impl SceneNode {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            local_transform: Mat4::IDENTITY,
            world_transform: Mat4::IDENTITY,
            bounds: ChunkBounds::new(Vec3::ZERO, Vec3::ONE),
            children: Vec::new(),
            parent: None,
            lod_level: LodLevel::Unloaded,
            is_visible: true,
            is_static: true,
            chunk_coord: None,
        }
    }

    pub fn set_translation(&mut self, pos: Vec3) {
        let (scale, rot, _) = self.local_transform.to_scale_rotation_translation();
        self.local_transform = Mat4::from_scale_rotation_translation(scale, rot, pos);
    }

    pub fn set_rotation(&mut self, rot: Quat) {
        let (scale, _, pos) = self.local_transform.to_scale_rotation_translation();
        self.local_transform = Mat4::from_scale_rotation_translation(scale, rot, pos);
    }

    pub fn set_scale(&mut self, scale: Vec3) {
        let (_, rot, pos) = self.local_transform.to_scale_rotation_translation();
        self.local_transform = Mat4::from_scale_rotation_translation(scale, rot, pos);
    }

    pub fn update_world_transform(&mut self, parent_world: Mat4) {
        self.world_transform = parent_world * self.local_transform;
        let world_bounds = self.bounds.transformed_by(self.world_transform);
        self.bounds = world_bounds;
    }

    pub fn world_position(&self) -> Vec3 {
        let (_, _, pos) = self.world_transform.to_scale_rotation_translation();
        pos
    }

    pub fn world_forward(&self) -> Vec3 {
        let rot = Quat::from_mat4(&self.world_transform);
        rot * Vec3::NEG_Z
    }
}

#[derive(Debug, Clone)]
pub struct SceneGraph {
    pub nodes: HashMap<u32, SceneNode>,
    pub root_nodes: Vec<u32>,
    pub next_id: u32,
    pub dirty_nodes: HashSet<u32>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
            next_id: 1,
            dirty_nodes: HashSet::new(),
        }
    }

    pub fn create_node(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.insert(id, SceneNode::new(id));
        self.root_nodes.push(id);
        id
    }

    pub fn attach_child(&mut self, parent: u32, child: u32) {
        if let Some(p) = self.nodes.get_mut(&parent) {
            if !p.children.contains(&child) {
                p.children.push(child);
            }
        }
        if let Some(c) = self.nodes.get_mut(&child) {
            c.parent = Some(parent);
        }
        self.root_nodes.retain(|&id| id != child);
        self.dirty_nodes.insert(child);
    }

    pub fn detach(&mut self, node_id: u32) {
        if let Some(parent_id) = self.nodes.get(&node_id).and_then(|n| n.parent) {
            if let Some(p) = self.nodes.get_mut(&parent_id) {
                p.children.retain(|&id| id != node_id);
            }
        }
        if let Some(n) = self.nodes.get_mut(&node_id) {
            n.parent = None;
        }
        self.root_nodes.push(node_id);
        self.dirty_nodes.insert(node_id);
    }

    pub fn update_transforms(&mut self) {
        let roots: Vec<u32> = self.root_nodes.clone();
        for root in roots {
            self.update_subtree(root, Mat4::IDENTITY);
        }
        self.dirty_nodes.clear();
    }

    fn update_subtree(&mut self, node_id: u32, parent_world: Mat4) {
        let world = {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.world_transform = parent_world * node.local_transform;
                node.world_transform
            } else {
                return;
            }
        };
        let children: Vec<u32> = self.nodes.get(&node_id).map(|n| n.children.clone()).unwrap_or_default();
        for child in children {
            self.update_subtree(child, world);
        }
    }

    pub fn mark_dirty(&mut self, node_id: u32) {
        self.dirty_nodes.insert(node_id);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

// ============================================================
// STREAMING MESH LOD MANAGER
// ============================================================

#[derive(Debug, Clone)]
pub struct MeshLodManager {
    pub mesh_lods: HashMap<u32, Vec<SimplifiedMesh>>,
    pub screen_space_metric: ScreenSpaceErrorMetric,
    pub transition_manager: LodTransitionManager,
    pub current_lods: HashMap<u32, usize>,
}

impl MeshLodManager {
    pub fn new(screen_height: f32, fov_rad: f32) -> Self {
        Self {
            mesh_lods: HashMap::new(),
            screen_space_metric: ScreenSpaceErrorMetric::new(screen_height, fov_rad, SCREEN_SPACE_ERROR_THRESHOLD),
            transition_manager: LodTransitionManager::new(4, true),
            current_lods: HashMap::new(),
        }
    }

    pub fn register_mesh(&mut self, mesh_id: u32, lods: Vec<SimplifiedMesh>) {
        self.current_lods.insert(mesh_id, lods.len().saturating_sub(1));
        self.mesh_lods.insert(mesh_id, lods);
    }

    pub fn update_lod(&mut self, mesh_id: u32, camera_pos: Vec3) {
        let lods = match self.mesh_lods.get(&mesh_id) {
            Some(l) => l.clone(),
            None => return,
        };
        if lods.is_empty() { return; }
        let bounds = lods[0].compute_bounds();
        let desired_lod = self.screen_space_metric.select_lod_for_bounds(&bounds, camera_pos);
        let desired_idx = desired_lod.index().min(lods.len() - 1);
        let current_idx = *self.current_lods.get(&mesh_id).unwrap_or(&0);
        if desired_idx != current_idx {
            self.transition_manager.begin_transition(
                ChunkCoord::new(mesh_id as i32, 0, 0),
                LodLevel::from_index(current_idx),
                LodLevel::from_index(desired_idx),
            );
            self.current_lods.insert(mesh_id, desired_idx);
        }
    }

    pub fn get_current_lod_mesh(&self, mesh_id: u32) -> Option<&SimplifiedMesh> {
        let lods = self.mesh_lods.get(&mesh_id)?;
        let idx = *self.current_lods.get(&mesh_id)?;
        lods.get(idx)
    }

    pub fn tick(&mut self) {
        self.transition_manager.tick();
    }

    pub fn total_triangle_count(&self) -> usize {
        self.mesh_lods.values()
            .filter_map(|lods| {
                let idx = 0; // default
                lods.get(idx).map(|m| m.triangle_count())
            })
            .sum()
    }

    pub fn memory_usage_bytes(&self) -> usize {
        self.mesh_lods.values()
            .flat_map(|lods| lods.iter())
            .map(|m| m.memory_bytes())
            .sum()
    }
}

// ============================================================
// CLUSTER GRID
// ============================================================

#[derive(Debug, Clone)]
pub struct ClusterGrid {
    pub cell_size: f32,
    pub clusters: HashMap<(i32, i32), Vec<u32>>,
    pub actor_cluster_map: HashMap<u32, u32>,
    pub cluster_bounds: HashMap<u32, ChunkBounds>,
    pub next_cluster_id: u32,
}

impl ClusterGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            clusters: HashMap::new(),
            actor_cluster_map: HashMap::new(),
            cluster_bounds: HashMap::new(),
            next_cluster_id: 1,
        }
    }

    pub fn cell_for_pos(&self, pos: Vec3) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    pub fn insert_actor(&mut self, actor_id: u32, pos: Vec3, bounds: ChunkBounds) {
        let cell = self.cell_for_pos(pos);
        let cluster_id = if let Some(&existing) = self.clusters.get(&cell).and_then(|v| v.first()) {
            existing
        } else {
            let id = self.next_cluster_id;
            self.next_cluster_id += 1;
            self.clusters.entry(cell).or_insert_with(Vec::new).push(id);
            id
        };
        self.actor_cluster_map.insert(actor_id, cluster_id);
        let cb = self.cluster_bounds.entry(cluster_id).or_insert_with(|| bounds.clone());
        *cb = cb.merge(&bounds);
    }

    pub fn get_cluster_for_actor(&self, actor_id: u32) -> Option<u32> {
        self.actor_cluster_map.get(&actor_id).cloned()
    }

    pub fn actors_in_cell(&self, cell: (i32, i32)) -> Vec<u32> {
        self.clusters.get(&cell)
            .map(|cluster_ids| {
                cluster_ids.iter()
                    .flat_map(|cid| {
                        self.actor_cluster_map.iter()
                            .filter(|(_, &c)| c == *cid)
                            .map(|(&a, _)| a)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn cells_in_radius(&self, center: Vec3, radius: f32) -> Vec<(i32, i32)> {
        let cr = (radius / self.cell_size).ceil() as i32;
        let cc = self.cell_for_pos(center);
        let mut cells = Vec::new();
        for dx in -cr..=cr {
            for dz in -cr..=cr {
                let cx = cc.0 + dx;
                let cz = cc.1 + dz;
                let world_x = cx as f32 * self.cell_size + self.cell_size * 0.5;
                let world_z = cz as f32 * self.cell_size + self.cell_size * 0.5;
                let cell_center = Vec3::new(world_x, center.y, world_z);
                if (cell_center - center).length() <= radius + self.cell_size * std::f32::consts::SQRT_2 * 0.5 {
                    cells.push((cx, cz));
                }
            }
        }
        cells
    }

    pub fn cluster_count(&self) -> usize {
        self.cluster_bounds.len()
    }
}

// ============================================================
// SPATIAL QUERY SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct SpatialQueryResult {
    pub object_id: u32,
    pub distance: f32,
    pub intersection_point: Vec3,
    pub normal: Vec3,
}

#[derive(Debug, Clone)]
pub struct SpatialQuerySystem {
    pub bvh: Option<BvhNode>,
    pub octree: Option<OctreeNode>,
    pub visibility_grid: VisibilityGrid,
    pub object_bounds: HashMap<u32, ChunkBounds>,
}

impl SpatialQuerySystem {
    pub fn new(grid_cell_size: f32) -> Self {
        Self {
            bvh: None,
            octree: None,
            visibility_grid: VisibilityGrid::new(grid_cell_size),
            object_bounds: HashMap::new(),
        }
    }

    pub fn register_object(&mut self, id: u32, pos: Vec3, bounds: ChunkBounds) {
        self.visibility_grid.insert(id, pos);
        self.object_bounds.insert(id, bounds);
    }

    pub fn unregister_object(&mut self, id: u32) {
        self.visibility_grid.remove(id);
        self.object_bounds.remove(&id);
    }

    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> Vec<SpatialQueryResult> {
        let mut candidates = Vec::new();
        if let Some(ref bvh) = self.bvh {
            bvh.query_ray(origin, direction, 0.0, max_dist, &mut candidates);
        }
        let mut results = Vec::new();
        for id in candidates {
            if let Some(bounds) = self.object_bounds.get(&id) {
                if ray_aabb_intersect(origin, direction, bounds, 0.0, max_dist) {
                    let closest = bounds.closest_point(origin);
                    let dist = (closest - origin).length();
                    let normal = (origin - closest).normalize_or_zero();
                    results.push(SpatialQueryResult {
                        object_id: id,
                        distance: dist,
                        intersection_point: closest,
                        normal,
                    });
                }
            }
        }
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    pub fn overlap_sphere(&self, center: Vec3, radius: f32) -> Vec<u32> {
        let candidates = self.visibility_grid.query_radius(center, radius);
        candidates.into_iter()
            .filter(|id| {
                if let Some(bounds) = self.object_bounds.get(id) {
                    bounds.distance_to_point(center) <= radius
                } else {
                    false
                }
            })
            .collect()
    }

    pub fn overlap_aabb(&self, bounds: &ChunkBounds) -> Vec<u32> {
        let candidates = self.visibility_grid.query_aabb(bounds);
        candidates.into_iter()
            .filter(|id| {
                if let Some(ob) = self.object_bounds.get(id) {
                    ob.intersects(bounds)
                } else {
                    false
                }
            })
            .collect()
    }

    pub fn frustum_query(&self, frustum: &FrustumCulling) -> Vec<u32> {
        let mut result = Vec::new();
        if let Some(ref bvh) = self.bvh {
            bvh.query_frustum(frustum, &mut result);
        } else {
            for (&id, bounds) in &self.object_bounds {
                if frustum.test_aabb_fast(bounds) {
                    result.push(id);
                }
            }
        }
        result
    }

    pub fn rebuild_bvh(&mut self) {
        let objects: Vec<(u32, ChunkBounds)> = self.object_bounds.iter()
            .map(|(&id, b)| (id, b.clone()))
            .collect();
        let builder = BvhBuilder::new(BVH_MAX_LEAF_OBJECTS, 16);
        self.bvh = builder.build(&objects);
    }

    pub fn rebuild_octree(&mut self) {
        let points: Vec<(u32, Vec3)> = self.object_bounds.iter()
            .map(|(&id, b)| (id, b.center()))
            .collect();
        let builder = OctreeBuilder::new(MAX_OCTREE_DEPTH, MAX_OBJECTS_PER_OCTREE_NODE);
        self.octree = Some(builder.build(&points));
    }
}

// ============================================================
// ADDITIONAL SPATIAL MATH
// ============================================================

pub fn sphere_vs_sphere(c0: Vec3, r0: f32, c1: Vec3, r1: f32) -> bool {
    (c0 - c1).length_squared() <= (r0 + r1) * (r0 + r1)
}

pub fn capsule_vs_sphere(cap_a: Vec3, cap_b: Vec3, cap_r: f32, sphere_c: Vec3, sphere_r: f32) -> bool {
    let closest = closest_point_on_segment(sphere_c, cap_a, cap_b);
    (sphere_c - closest).length_squared() <= (cap_r + sphere_r) * (cap_r + sphere_r)
}

pub fn aabb_vs_sphere(bounds: &ChunkBounds, center: Vec3, radius: f32) -> bool {
    bounds.distance_sq_to_point(center) <= radius * radius
}

pub fn obb_vs_point(center: Vec3, half_extents: Vec3, orientation: Quat, point: Vec3) -> bool {
    let local = Quat::conjugate(orientation).mul_vec3(point - center);
    local.x.abs() <= half_extents.x
        && local.y.abs() <= half_extents.y
        && local.z.abs() <= half_extents.z
}

pub fn compute_aabb_from_obb(center: Vec3, half_extents: Vec3, orientation: Quat) -> ChunkBounds {
    let mat = Mat4::from_quat(orientation);
    let wx = mat.col(0).truncate() * half_extents.x;
    let wy = mat.col(1).truncate() * half_extents.y;
    let wz = mat.col(2).truncate() * half_extents.z;
    let abs_wx = Vec3::new(wx.x.abs(), wx.y.abs(), wx.z.abs());
    let abs_wy = Vec3::new(wy.x.abs(), wy.y.abs(), wy.z.abs());
    let abs_wz = Vec3::new(wz.x.abs(), wz.y.abs(), wz.z.abs());
    let new_half = abs_wx + abs_wy + abs_wz;
    ChunkBounds {
        min: center - new_half,
        max: center + new_half,
    }
}

pub fn slerp_quat(a: Quat, b: Quat, t: f32) -> Quat {
    a.slerp(b, t)
}

pub fn compute_look_at_quat(forward: Vec3, up: Vec3) -> Quat {
    let f = forward.normalize();
    let r = up.cross(f).normalize();
    let u = f.cross(r).normalize();
    Quat::from_mat3(&glam::Mat3::from_cols(r, u, f))
}

// ============================================================
// TERRAIN MATERIAL BLENDING
// ============================================================

#[derive(Debug, Clone)]
pub struct TerrainMaterialLayer {
    pub material_id: u32,
    pub blend_weight: f32,
    pub uv_scale: Vec2,
    pub normal_intensity: f32,
    pub roughness: f32,
    pub metalness: f32,
}

impl TerrainMaterialLayer {
    pub fn new(material_id: u32) -> Self {
        Self {
            material_id,
            blend_weight: 1.0,
            uv_scale: Vec2::ONE,
            normal_intensity: 1.0,
            roughness: 0.8,
            metalness: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerrainMaterialBlender {
    pub layers: Vec<TerrainMaterialLayer>,
    pub splat_map: Vec<Vec4>,  // RGBA splat weights for up to 4 materials
    pub width: usize,
    pub height: usize,
}

impl TerrainMaterialBlender {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            layers: Vec::new(),
            splat_map: vec![Vec4::new(1.0, 0.0, 0.0, 0.0); width * height],
            width,
            height,
        }
    }

    pub fn add_layer(&mut self, layer: TerrainMaterialLayer) {
        self.layers.push(layer);
    }

    pub fn set_splat(&mut self, x: usize, z: usize, weights: Vec4) {
        if x < self.width && z < self.height {
            let normalized = {
                let sum = weights.x + weights.y + weights.z + weights.w;
                if sum > 1e-8 { weights / sum } else { Vec4::new(1.0, 0.0, 0.0, 0.0) }
            };
            self.splat_map[z * self.width + x] = normalized;
        }
    }

    pub fn get_splat(&self, x: usize, z: usize) -> Vec4 {
        if x < self.width && z < self.height {
            self.splat_map[z * self.width + x]
        } else {
            Vec4::new(1.0, 0.0, 0.0, 0.0)
        }
    }

    pub fn sample_splat_bilinear(&self, fx: f32, fz: f32) -> Vec4 {
        let ix = fx.floor() as isize;
        let iz = fz.floor() as isize;
        let tx = fx - ix as f32;
        let tz = fz - iz as f32;
        let get = |x: isize, z: isize| -> Vec4 {
            let cx = x.clamp(0, self.width as isize - 1) as usize;
            let cz = z.clamp(0, self.height as isize - 1) as usize;
            self.splat_map[cz * self.width + cx]
        };
        let v00 = get(ix,     iz    );
        let v10 = get(ix + 1, iz    );
        let v01 = get(ix,     iz + 1);
        let v11 = get(ix + 1, iz + 1);
        let h0 = v00 * (1.0 - tx) + v10 * tx;
        let h1 = v01 * (1.0 - tx) + v11 * tx;
        h0 * (1.0 - tz) + h1 * tz
    }

    pub fn blend_roughness(&self, splat: Vec4) -> f32 {
        let mut result = 0.0f32;
        for (i, layer) in self.layers.iter().enumerate().take(4) {
            let w = match i { 0 => splat.x, 1 => splat.y, 2 => splat.z, _ => splat.w };
            result += layer.roughness * w;
        }
        result
    }

    pub fn blend_normal_intensity(&self, splat: Vec4) -> f32 {
        let mut result = 0.0f32;
        for (i, layer) in self.layers.iter().enumerate().take(4) {
            let w = match i { 0 => splat.x, 1 => splat.y, 2 => splat.z, _ => splat.w };
            result += layer.normal_intensity * w;
        }
        result
    }

    pub fn dominant_material_at(&self, x: usize, z: usize) -> u32 {
        let splat = self.get_splat(x, z);
        let weights = [splat.x, splat.y, splat.z, splat.w];
        let max_idx = weights.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i).unwrap_or(0);
        self.layers.get(max_idx).map(|l| l.material_id).unwrap_or(0)
    }
}

// ============================================================
// CHUNK NEIGHBOR MANAGER
// ============================================================

#[derive(Debug, Clone)]
pub struct ChunkNeighborManager {
    pub neighbor_cache: HashMap<ChunkCoord, [Option<ChunkCoord>; 6]>,
    pub loaded_set: HashSet<ChunkCoord>,
}

impl ChunkNeighborManager {
    pub fn new() -> Self {
        Self {
            neighbor_cache: HashMap::new(),
            loaded_set: HashSet::new(),
        }
    }

    pub fn register_loaded(&mut self, coord: ChunkCoord) {
        self.loaded_set.insert(coord);
    }

    pub fn unregister(&mut self, coord: &ChunkCoord) {
        self.loaded_set.remove(coord);
        self.neighbor_cache.remove(coord);
    }

    pub fn get_neighbors(&mut self, coord: &ChunkCoord) -> [Option<ChunkCoord>; 6] {
        if let Some(cached) = self.neighbor_cache.get(coord) {
            return *cached;
        }
        let n6 = coord.neighbors_6();
        let result: [Option<ChunkCoord>; 6] = std::array::from_fn(|i| {
            if self.loaded_set.contains(&n6[i]) { Some(n6[i].clone()) } else { None }
        });
        self.neighbor_cache.insert(coord.clone(), result);
        result
    }

    pub fn all_neighbors_loaded(&self, coord: &ChunkCoord) -> bool {
        coord.neighbors_6().iter().all(|n| self.loaded_set.contains(n))
    }

    pub fn loaded_neighbor_count(&self, coord: &ChunkCoord) -> usize {
        coord.neighbors_6().iter().filter(|n| self.loaded_set.contains(n)).count()
    }

    pub fn needs_seam_update(&self, coord: &ChunkCoord, other: &ChunkCoord) -> bool {
        coord.is_adjacent(other) && self.loaded_set.contains(coord) && self.loaded_set.contains(other)
    }

    pub fn invalidate_cache_for(&mut self, coord: &ChunkCoord) {
        self.neighbor_cache.remove(coord);
        for neighbor in coord.neighbors_26() {
            self.neighbor_cache.remove(&neighbor);
        }
    }
}

// ============================================================
// PREFETCH SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct PrefetchSystem {
    pub velocity_buffer: VecDeque<Vec3>,
    pub velocity_history: usize,
    pub prefetch_distance: f32,
    pub prefetch_angle_deg: f32,
    pub predicted_position: Vec3,
    pub predicted_forward: Vec3,
    pub confidence: f32,
}

impl PrefetchSystem {
    pub fn new(history: usize, prefetch_dist: f32) -> Self {
        Self {
            velocity_buffer: VecDeque::with_capacity(history),
            velocity_history: history,
            prefetch_distance: prefetch_dist,
            prefetch_angle_deg: 90.0,
            predicted_position: Vec3::ZERO,
            predicted_forward: Vec3::NEG_Z,
            confidence: 0.0,
        }
    }

    pub fn update(&mut self, current_pos: Vec3, current_forward: Vec3, dt: f32) {
        if dt < 1e-6 { return; }
        if let Some(&prev_pos) = self.velocity_buffer.back() {
            let vel = (current_pos - prev_pos) / dt;
            self.velocity_buffer.push_back(vel);
        } else {
            self.velocity_buffer.push_back(Vec3::ZERO);
        }
        // Actually push current position as sample
        self.velocity_buffer.push_back(current_pos);
        while self.velocity_buffer.len() > self.velocity_history * 2 {
            self.velocity_buffer.pop_front();
        }
        self.compute_prediction(current_pos, current_forward, dt);
    }

    fn compute_prediction(&mut self, pos: Vec3, forward: Vec3, dt: f32) {
        // Exponential moving average of velocity
        let n = self.velocity_buffer.len();
        if n < 2 {
            self.predicted_position = pos + forward * self.prefetch_distance;
            self.predicted_forward = forward;
            self.confidence = 0.1;
            return;
        }
        let mut avg_vel = Vec3::ZERO;
        let mut weight_sum = 0.0f32;
        let samples: Vec<_> = self.velocity_buffer.iter().cloned().collect();
        for i in 0..samples.len().saturating_sub(1) {
            let w = (i + 1) as f32;
            avg_vel += samples[i] * w;
            weight_sum += w;
        }
        if weight_sum > 0.0 { avg_vel /= weight_sum; }
        let speed = avg_vel.length();
        self.confidence = (speed * 0.1).clamp(0.0, 1.0);
        let lookahead_time = self.prefetch_distance / speed.max(1.0);
        self.predicted_position = pos + avg_vel * lookahead_time;
        self.predicted_forward = if speed > 0.01 { avg_vel.normalize() } else { forward };
    }

    pub fn chunks_to_prefetch(&self, chunk_size: f32, extra_radius_chunks: i32) -> Vec<ChunkCoord> {
        let center = ChunkCoord::from_world_pos(self.predicted_position, chunk_size);
        ChunkCoord::chunks_in_radius(&center, extra_radius_chunks)
    }

    pub fn should_prefetch(&self, coord: &ChunkCoord, chunk_size: f32) -> bool {
        if self.confidence < 0.3 { return false; }
        let center = coord.to_world_center(chunk_size);
        let to_chunk = (center - self.predicted_position).normalize_or_zero();
        let angle_cos = self.predicted_forward.dot(to_chunk);
        angle_cos >= (self.prefetch_angle_deg.to_radians() * 0.5).cos()
    }
}

// ============================================================
// LEVEL STREAMING VOLUME
// ============================================================

#[derive(Debug, Clone)]
pub struct LevelStreamingVolume {
    pub id: u32,
    pub bounds: ChunkBounds,
    pub trigger_on_enter: bool,
    pub trigger_on_exit: bool,
    pub associated_chunks: Vec<ChunkCoord>,
    pub load_distance_override: Option<f32>,
    pub priority: f32,
    pub is_active: bool,
    pub last_state: bool,
}

impl LevelStreamingVolume {
    pub fn new(id: u32, bounds: ChunkBounds) -> Self {
        Self {
            id,
            bounds,
            trigger_on_enter: true,
            trigger_on_exit: false,
            associated_chunks: Vec::new(),
            load_distance_override: None,
            priority: 1.0,
            is_active: false,
            last_state: false,
        }
    }

    pub fn check_viewer(&mut self, viewer_pos: Vec3) -> Option<bool> {
        let inside = self.bounds.contains(viewer_pos);
        if inside != self.last_state {
            self.last_state = inside;
            Some(inside)
        } else {
            None
        }
    }

    pub fn effective_load_distance(&self, base_distance: f32) -> f32 {
        self.load_distance_override.unwrap_or(base_distance) * self.priority
    }

    pub fn contains_viewer(&self, viewer_pos: Vec3) -> bool {
        self.bounds.contains(viewer_pos)
    }

    pub fn distance_to_viewer(&self, viewer_pos: Vec3) -> f32 {
        self.bounds.distance_to_point(viewer_pos)
    }
}

#[derive(Debug, Clone)]
pub struct LevelStreamingVolumeManager {
    pub volumes: HashMap<u32, LevelStreamingVolume>,
    pub next_id: u32,
    pub active_volume_ids: HashSet<u32>,
}

impl LevelStreamingVolumeManager {
    pub fn new() -> Self {
        Self {
            volumes: HashMap::new(),
            next_id: 1,
            active_volume_ids: HashSet::new(),
        }
    }

    pub fn add_volume(&mut self, bounds: ChunkBounds) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.volumes.insert(id, LevelStreamingVolume::new(id, bounds));
        id
    }

    pub fn remove_volume(&mut self, id: u32) {
        self.volumes.remove(&id);
        self.active_volume_ids.remove(&id);
    }

    pub fn update_viewer(&mut self, viewer_pos: Vec3) -> Vec<(u32, bool)> {
        let mut events = Vec::new();
        for (id, volume) in self.volumes.iter_mut() {
            if let Some(entered) = volume.check_viewer(viewer_pos) {
                if entered {
                    self.active_volume_ids.insert(*id);
                } else {
                    self.active_volume_ids.remove(id);
                }
                events.push((*id, entered));
            }
        }
        events
    }

    pub fn chunks_to_force_load(&self) -> Vec<(ChunkCoord, f32)> {
        let mut result = Vec::new();
        for id in &self.active_volume_ids {
            if let Some(vol) = self.volumes.get(id) {
                for coord in &vol.associated_chunks {
                    result.push((coord.clone(), vol.priority));
                }
            }
        }
        result
    }

    pub fn active_volume_count(&self) -> usize {
        self.active_volume_ids.len()
    }
}

// ============================================================
// RUNTIME STATISTICS
// ============================================================

#[derive(Debug, Clone)]
pub struct RuntimeHistogram {
    pub buckets: Vec<u64>,
    pub min_val: f32,
    pub max_val: f32,
    pub total_samples: u64,
    pub sum: f64,
}

impl RuntimeHistogram {
    pub fn new(num_buckets: usize, min_val: f32, max_val: f32) -> Self {
        Self {
            buckets: vec![0u64; num_buckets],
            min_val,
            max_val,
            total_samples: 0,
            sum: 0.0,
        }
    }

    pub fn record(&mut self, value: f32) {
        let n = self.buckets.len();
        let range = self.max_val - self.min_val;
        if range <= 0.0 { return; }
        let idx = (((value - self.min_val) / range) * n as f32) as usize;
        let clamped = idx.min(n - 1);
        self.buckets[clamped] += 1;
        self.total_samples += 1;
        self.sum += value as f64;
    }

    pub fn mean(&self) -> f64 {
        if self.total_samples == 0 { return 0.0; }
        self.sum / self.total_samples as f64
    }

    pub fn percentile(&self, pct: f32) -> f32 {
        if self.total_samples == 0 { return self.min_val; }
        let target = (pct / 100.0 * self.total_samples as f32) as u64;
        let mut cumulative = 0u64;
        let n = self.buckets.len();
        let range = self.max_val - self.min_val;
        for (i, &count) in self.buckets.iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                return self.min_val + (i as f32 / n as f32) * range;
            }
        }
        self.max_val
    }

    pub fn reset(&mut self) {
        for b in &mut self.buckets { *b = 0; }
        self.total_samples = 0;
        self.sum = 0.0;
    }

    pub fn mode_bucket(&self) -> usize {
        self.buckets.iter().enumerate().max_by_key(|(_, &c)| c).map(|(i, _)| i).unwrap_or(0)
    }

    pub fn mode_value(&self) -> f32 {
        let n = self.buckets.len();
        let range = self.max_val - self.min_val;
        self.min_val + (self.mode_bucket() as f32 / n as f32) * range
    }
}

// ============================================================
// CHUNK SERIALIZATION HELPERS
// ============================================================

#[derive(Debug, Clone)]
pub struct ChunkHeader {
    pub magic: u32,
    pub version: u32,
    pub coord: ChunkCoord,
    pub data_size_bytes: u64,
    pub lod_count: u32,
    pub has_terrain: bool,
    pub has_collision: bool,
    pub has_nav: bool,
    pub object_count: u32,
    pub checksum: u32,
}

impl ChunkHeader {
    pub const MAGIC: u32 = 0x43484E4B; // 'CHNK'
    pub const VERSION: u32 = 1;

    pub fn new(coord: ChunkCoord) -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            coord,
            data_size_bytes: 0,
            lod_count: 0,
            has_terrain: false,
            has_collision: false,
            has_nav: false,
            object_count: 0,
            checksum: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC && self.version <= Self::VERSION
    }

    pub fn compute_checksum(&self) -> u32 {
        let packed = self.coord.pack_u64();
        let mut h = packed as u32;
        h ^= self.data_size_bytes as u32;
        h = h.wrapping_mul(0x9e3779b9);
        h ^= self.object_count;
        h = h.rotate_left(13);
        h
    }

    pub fn validate_checksum(&self) -> bool {
        self.checksum == self.compute_checksum()
    }

    pub fn finalize(&mut self) {
        self.checksum = self.compute_checksum();
    }
}

// ============================================================
// RENDERING STATE
// ============================================================

#[derive(Debug, Clone)]
pub struct RenderVisibilityState {
    pub visible_chunks: Vec<ChunkCoord>,
    pub impostor_chunks: Vec<ChunkCoord>,
    pub hlod_clusters: Vec<u32>,
    pub total_triangles: u64,
    pub total_draw_calls: u32,
    pub culled_by_frustum: u32,
    pub culled_by_distance: u32,
    pub culled_by_occlusion: u32,
}

impl RenderVisibilityState {
    pub fn new() -> Self {
        Self {
            visible_chunks: Vec::new(),
            impostor_chunks: Vec::new(),
            hlod_clusters: Vec::new(),
            total_triangles: 0,
            total_draw_calls: 0,
            culled_by_frustum: 0,
            culled_by_distance: 0,
            culled_by_occlusion: 0,
        }
    }

    pub fn reset(&mut self) {
        self.visible_chunks.clear();
        self.impostor_chunks.clear();
        self.hlod_clusters.clear();
        self.total_triangles = 0;
        self.total_draw_calls = 0;
        self.culled_by_frustum = 0;
        self.culled_by_distance = 0;
        self.culled_by_occlusion = 0;
    }

    pub fn total_culled(&self) -> u32 {
        self.culled_by_frustum + self.culled_by_distance + self.culled_by_occlusion
    }

    pub fn visibility_ratio(&self, total_chunks: u32) -> f32 {
        if total_chunks == 0 { return 0.0; }
        self.visible_chunks.len() as f32 / total_chunks as f32
    }
}

// ============================================================
// FINAL INTEGRATION / TESTS
// ============================================================

pub fn create_default_editor() -> WorldStreamingEditor {
    WorldStreamingEditor::new(StreamingConfig::default())
}

pub fn create_editor_with_preset(preset: QualityPreset) -> WorldStreamingEditor {
    WorldStreamingEditor::new(StreamingConfig::with_quality_preset(preset))
}

pub fn build_test_world(editor: &mut WorldStreamingEditor, actor_count: u32, world_size: f32) {
    for i in 0..actor_count {
        let angle = (i as f32 / actor_count as f32) * std::f32::consts::TAU;
        let radius = (i as f32 / actor_count as f32) * world_size * 0.5;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let y = value_noise_2d(x * 0.01, z * 0.01) * 50.0;
        let pos = Vec3::new(x, y, z);
        let half = Vec3::splat(5.0 + (i % 10) as f32);
        let bounds = ChunkBounds::from_center_size(pos, half);
        let dist = 200.0 + (i % 5) as f32 * 100.0;
        editor.register_actor(i, pos, bounds, dist);
    }
    editor.build_octree();
    editor.build_bvh();
}

pub fn run_streaming_simulation(editor: &mut WorldStreamingEditor, frames: u32, path: &[(Vec3, Vec3)]) {
    let step = if path.is_empty() { 0 } else { (frames as usize / path.len()).max(1) };
    for frame in 0..frames {
        let path_idx = (frame as usize / step).min(path.len().saturating_sub(1));
        let (pos, fwd) = if path.is_empty() { (Vec3::ZERO, Vec3::NEG_Z) } else { path[path_idx] };
        editor.set_viewer(pos, fwd);
        editor.tick(1.0 / 60.0, frame as u64 * 16667);
    }
}

pub fn compute_streaming_coverage(editor: &WorldStreamingEditor) -> f32 {
    let loaded = editor.loaded_chunk_count();
    let total = editor.chunk_count();
    if total == 0 { return 0.0; }
    loaded as f32 / total as f32
}

pub fn debug_print_lod_distribution(editor: &WorldStreamingEditor) -> HashMap<LodLevel, usize> {
    let mut dist: HashMap<LodLevel, usize> = HashMap::new();
    for chunk in editor.chunks.values() {
        *dist.entry(chunk.lod_level.clone()).or_insert(0) += 1;
    }
    dist
}

pub fn compute_world_bounds_from_chunks(editor: &WorldStreamingEditor) -> Option<ChunkBounds> {
    let mut all_bounds: Option<ChunkBounds> = None;
    for chunk in editor.chunks.values() {
        all_bounds = Some(match all_bounds {
            None => chunk.bounds.clone(),
            Some(b) => b.merge(&chunk.bounds),
        });
    }
    all_bounds
}

pub fn find_nearest_loaded_chunk(editor: &WorldStreamingEditor, pos: Vec3) -> Option<ChunkCoord> {
    editor.chunks.iter()
        .filter(|(_, c)| c.load_state == ChunkLoadState::Loaded)
        .min_by(|(_, a), (_, b)| {
            let da = a.bounds.distance_sq_to_point(pos);
            let db = b.bounds.distance_sq_to_point(pos);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(coord, _)| coord.clone())
}

pub fn recompute_all_chunk_priorities(editor: &mut WorldStreamingEditor) {
    let viewer = editor.viewer_position;
    let forward = editor.viewer_forward;
    let coords: Vec<ChunkCoord> = editor.chunks.keys().cloned().collect();
    for coord in coords {
        if let Some(chunk) = editor.chunks.get_mut(&coord) {
            chunk.compute_load_priority(viewer, forward);
        }
    }
}

pub fn estimate_total_world_memory(editor: &WorldStreamingEditor) -> u64 {
    editor.chunks.values()
        .filter(|c| c.load_state == ChunkLoadState::Loaded)
        .map(|c| c.memory_bytes)
        .sum()
}

// ============================================================
// UNIT-STYLE VALIDATION FUNCTIONS
// ============================================================

pub fn validate_chunk_bounds(bounds: &ChunkBounds) -> bool {
    !bounds.is_degenerate()
        && bounds.volume() > 0.0
        && bounds.surface_area() > 0.0
        && bounds.center() == (bounds.min + bounds.max) * 0.5
}

pub fn validate_frustum_planes(planes: &[Vec4; 6]) -> bool {
    for plane in planes {
        let n = Vec3::new(plane.x, plane.y, plane.z);
        let len = n.length();
        if (len - 1.0).abs() > 0.01 { return false; }
    }
    true
}

pub fn validate_bvh(node: &BvhNode) -> bool {
    if node.is_leaf() { return true; }
    let ok_left  = node.left.as_ref().map(|l| l.bounds.intersects(&node.bounds) && validate_bvh(l)).unwrap_or(true);
    let ok_right = node.right.as_ref().map(|r| r.bounds.intersects(&node.bounds) && validate_bvh(r)).unwrap_or(true);
    ok_left && ok_right
}

pub fn validate_lod_transitions(from: &LodLevel, to: &LodLevel) -> bool {
    // Transitions of more than 1 step are allowed but should be flagged
    let diff = (from.index() as isize - to.index() as isize).abs();
    diff <= 2
}

pub fn stress_test_octree(count: u32, world_size: f32) -> (OctreeNode, usize) {
    let mut points = Vec::with_capacity(count as usize);
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let r = (i as f32 / count as f32) * world_size * 0.5;
        let h = value_noise_2d(angle, r * 0.01) * 50.0;
        let pos = Vec3::new(angle.cos() * r, h, angle.sin() * r);
        points.push((i, pos));
    }
    let builder = OctreeBuilder::new(MAX_OCTREE_DEPTH, MAX_OBJECTS_PER_OCTREE_NODE);
    let tree = builder.build(&points);
    let node_count = tree.node_count();
    (tree, node_count)
}

pub fn stress_test_bvh(count: u32, world_size: f32) -> (Option<BvhNode>, usize) {
    let mut objects = Vec::with_capacity(count as usize);
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let r = (i as f32 / count as f32) * world_size * 0.5;
        let center = Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
        let half = Vec3::splat(5.0);
        let bounds = ChunkBounds::from_center_size(center, half);
        objects.push((i, bounds));
    }
    let builder = BvhBuilder::new(BVH_MAX_LEAF_OBJECTS, 16);
    let bvh = builder.build(&objects);
    let node_count = bvh.as_ref().map(|b| b.node_count()).unwrap_or(0);
    (bvh, node_count)
}

pub fn benchmark_frustum_cull(
    frustum: &FrustumCulling,
    bounds_list: &[ChunkBounds],
) -> (usize, usize) {
    let mut visible = 0;
    let mut culled = 0;
    for bounds in bounds_list {
        if frustum.test_aabb_fast(bounds) {
            visible += 1;
        } else {
            culled += 1;
        }
    }
    (visible, culled)
}

pub fn generate_grid_bounds(cols: usize, rows: usize, cell_size: f32) -> Vec<(ChunkCoord, ChunkBounds)> {
    let mut result = Vec::with_capacity(cols * rows);
    for z in 0..rows as i32 {
        for x in 0..cols as i32 {
            let coord = ChunkCoord::new(x, 0, z);
            let bounds = ChunkBounds::from_chunk_coord(&coord, cell_size);
            result.push((coord, bounds));
        }
    }
    result
}

pub fn compute_lod_histogram(editor: &WorldStreamingEditor) -> [u32; 6] {
    let mut hist = [0u32; 6];
    for chunk in editor.chunks.values() {
        hist[chunk.lod_level.index()] += 1;
    }
    hist
}

pub fn build_minimal_test_scene() -> WorldStreamingEditor {
    let mut editor = create_default_editor();
    let camera = StreamingCamera::new(
        Vec3::new(0.0, 100.0, 0.0),
        Vec3::ZERO,
        Vec3::Y,
        60.0,
        16.0 / 9.0,
        0.1,
        10000.0,
    );
    editor.set_camera(camera);
    editor.set_viewer(Vec3::new(0.0, 100.0, 0.0), Vec3::NEG_Z);

    // Add some test terrain patches
    for z in -2i32..=2 {
        for x in -2i32..=2 {
            let coord = ChunkCoord::new(x, 0, z);
            editor.add_terrain_patch(coord, TERRAIN_PATCH_SIZE, 4.0);
        }
    }

    // Build spatial structures
    build_test_world(&mut editor, 200, 2048.0);
    editor
}

// ============================================================
// IMPORTS USED IN IMPLEMENTATION (re-exports for clarity)
// ============================================================

// (re-exports removed — imports already at top)

// ============================================================
// ACTOR IMPORTANCE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct ActorImportance {
    pub actor_id: u32,
    pub base_importance: f32,
    pub distance_falloff_exponent: f32,
    pub is_gameplay_relevant: bool,
    pub last_interaction_frame: u64,
    pub interaction_boost: f32,
    pub tag_boosts: HashMap<String, f32>,
}

impl ActorImportance {
    pub fn new(actor_id: u32, base_importance: f32) -> Self {
        Self {
            actor_id,
            base_importance,
            distance_falloff_exponent: 2.0,
            is_gameplay_relevant: false,
            last_interaction_frame: 0,
            interaction_boost: 0.0,
            tag_boosts: HashMap::new(),
        }
    }

    pub fn compute_importance(&self, distance: f32, current_frame: u64) -> f32 {
        let dist_factor = 1.0 / (1.0 + distance.powf(self.distance_falloff_exponent) * 0.0001);
        let gameplay_factor = if self.is_gameplay_relevant { 3.0 } else { 1.0 };
        let interaction_decay = {
            let age = current_frame.saturating_sub(self.last_interaction_frame) as f32;
            self.interaction_boost * (-age * 0.01).exp()
        };
        let tag_sum: f32 = self.tag_boosts.values().sum();
        (self.base_importance + interaction_boost_clamped(interaction_decay) + tag_sum)
            * dist_factor
            * gameplay_factor
    }

    pub fn boost_interaction(&mut self, frame: u64, boost: f32) {
        self.last_interaction_frame = frame;
        self.interaction_boost = (self.interaction_boost + boost).min(10.0);
    }

    pub fn add_tag_boost(&mut self, tag: String, value: f32) {
        *self.tag_boosts.entry(tag).or_insert(0.0) += value;
    }

    pub fn remove_tag_boost(&mut self, tag: &str) {
        self.tag_boosts.remove(tag);
    }

    pub fn effective_streaming_distance(&self, base_dist: f32, distance: f32, frame: u64) -> f32 {
        let importance = self.compute_importance(distance, frame);
        base_dist * importance.sqrt().clamp(0.5, 4.0)
    }
}

fn interaction_boost_clamped(v: f32) -> f32 { v.clamp(0.0, 10.0) }

// ============================================================
// DYNAMIC LOADING BUDGET CONTROLLER
// ============================================================

#[derive(Debug, Clone)]
pub struct LoadingBudgetController {
    pub max_loads_per_frame: usize,
    pub max_unloads_per_frame: usize,
    pub target_frame_time_ms: f32,
    pub last_frame_time_ms: f32,
    pub smoothed_frame_time_ms: f32,
    pub smoothing_alpha: f32,
    pub overbudget_scale: f32,
    pub underbudget_scale: f32,
    pub min_loads: usize,
    pub max_loads_cap: usize,
}

impl LoadingBudgetController {
    pub fn new(target_ms: f32, max_loads: usize) -> Self {
        Self {
            max_loads_per_frame: max_loads,
            max_unloads_per_frame: max_loads / 2,
            target_frame_time_ms: target_ms,
            last_frame_time_ms: target_ms,
            smoothed_frame_time_ms: target_ms,
            smoothing_alpha: 0.1,
            overbudget_scale: 0.7,
            underbudget_scale: 1.3,
            min_loads: 1,
            max_loads_cap: max_loads * 4,
        }
    }

    pub fn update(&mut self, measured_frame_time_ms: f32) {
        self.last_frame_time_ms = measured_frame_time_ms;
        self.smoothed_frame_time_ms = self.smoothed_frame_time_ms * (1.0 - self.smoothing_alpha)
            + measured_frame_time_ms * self.smoothing_alpha;
        self.adjust_budget();
    }

    fn adjust_budget(&mut self) {
        let ratio = self.smoothed_frame_time_ms / self.target_frame_time_ms;
        if ratio > 1.1 {
            // Over budget: reduce loads
            let new_max = (self.max_loads_per_frame as f32 * self.overbudget_scale) as usize;
            self.max_loads_per_frame = new_max.max(self.min_loads);
        } else if ratio < 0.9 {
            // Under budget: increase loads
            let new_max = (self.max_loads_per_frame as f32 * self.underbudget_scale) as usize;
            self.max_loads_per_frame = new_max.min(self.max_loads_cap);
        }
        self.max_unloads_per_frame = self.max_loads_per_frame / 2;
    }

    pub fn loads_allowed(&self) -> usize {
        self.max_loads_per_frame
    }

    pub fn unloads_allowed(&self) -> usize {
        self.max_unloads_per_frame
    }

    pub fn is_overbudget(&self) -> bool {
        self.smoothed_frame_time_ms > self.target_frame_time_ms * 1.1
    }

    pub fn headroom_ms(&self) -> f32 {
        (self.target_frame_time_ms - self.smoothed_frame_time_ms).max(0.0)
    }
}

// ============================================================
// DISTANCE FIELD APPROXIMATION
// ============================================================

#[derive(Debug, Clone)]
pub struct DistanceField2D {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
    pub cell_size: f32,
    pub origin: Vec2,
}

impl DistanceField2D {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        Self {
            width,
            height,
            data: vec![f32::MAX; width * height],
            cell_size,
            origin,
        }
    }

    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = value;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            f32::MAX
        }
    }

    pub fn sample(&self, world_x: f32, world_y: f32) -> f32 {
        let lx = (world_x - self.origin.x) / self.cell_size;
        let ly = (world_y - self.origin.y) / self.cell_size;
        let ix = lx.floor() as isize;
        let iy = ly.floor() as isize;
        let fx = lx - ix as f32;
        let fy = ly - iy as f32;
        let get_c = |x: isize, y: isize| -> f32 {
            let cx = x.clamp(0, self.width as isize - 1) as usize;
            let cy = y.clamp(0, self.height as isize - 1) as usize;
            self.data[cy * self.width + cx]
        };
        let v00 = get_c(ix,     iy    );
        let v10 = get_c(ix + 1, iy    );
        let v01 = get_c(ix,     iy + 1);
        let v11 = get_c(ix + 1, iy + 1);
        let h0 = v00 * (1.0 - fx) + v10 * fx;
        let h1 = v01 * (1.0 - fx) + v11 * fx;
        h0 * (1.0 - fy) + h1 * fy
    }

    pub fn compute_from_obstacles(
        &mut self,
        obstacles: &[(f32, f32)], // world positions
    ) {
        for z in 0..self.height {
            for x in 0..self.width {
                let wx = self.origin.x + x as f32 * self.cell_size;
                let wy = self.origin.y + z as f32 * self.cell_size;
                let min_dist = obstacles.iter()
                    .map(|(ox, oy)| ((wx - ox) * (wx - ox) + (wy - oy) * (wy - oy)).sqrt())
                    .fold(f32::MAX, f32::min);
                self.data[z * self.width + x] = min_dist;
            }
        }
    }

    pub fn gradient_at(&self, world_x: f32, world_y: f32) -> Vec2 {
        let eps = self.cell_size;
        let dx = (self.sample(world_x + eps, world_y) - self.sample(world_x - eps, world_y)) / (2.0 * eps);
        let dy = (self.sample(world_x, world_y + eps) - self.sample(world_x, world_y - eps)) / (2.0 * eps);
        Vec2::new(dx, dy)
    }

    pub fn is_inside_obstacle(&self, world_x: f32, world_y: f32, threshold: f32) -> bool {
        self.sample(world_x, world_y) < threshold
    }

    pub fn sweep_pass_horizontal(&mut self) {
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            // Forward pass
            for x in 1..w {
                let prev = self.data[y * w + (x - 1)];
                if prev + self.cell_size < self.data[y * w + x] {
                    self.data[y * w + x] = prev + self.cell_size;
                }
            }
            // Backward pass
            for x in (0..w - 1).rev() {
                let next = self.data[y * w + (x + 1)];
                if next + self.cell_size < self.data[y * w + x] {
                    self.data[y * w + x] = next + self.cell_size;
                }
            }
        }
    }

    pub fn sweep_pass_vertical(&mut self) {
        let w = self.width;
        let h = self.height;
        for x in 0..w {
            for y in 1..h {
                let prev = self.data[(y - 1) * w + x];
                if prev + self.cell_size < self.data[y * w + x] {
                    self.data[y * w + x] = prev + self.cell_size;
                }
            }
            for y in (0..h - 1).rev() {
                let next = self.data[(y + 1) * w + x];
                if next + self.cell_size < self.data[y * w + x] {
                    self.data[y * w + x] = next + self.cell_size;
                }
            }
        }
    }

    pub fn fast_sweep(&mut self) {
        self.sweep_pass_horizontal();
        self.sweep_pass_vertical();
        self.sweep_pass_horizontal();
    }
}

// ============================================================
// RENDER BATCH BUILDER
// ============================================================

#[derive(Debug, Clone)]
pub struct RenderBatch {
    pub mesh_id: u32,
    pub lod_level: LodLevel,
    pub instance_data: Vec<Mat4>,
    pub bounds_union: ChunkBounds,
    pub material_id: u32,
    pub is_impostor: bool,
}

impl RenderBatch {
    pub fn new(mesh_id: u32, lod: LodLevel, material_id: u32) -> Self {
        Self {
            mesh_id,
            lod_level: lod,
            instance_data: Vec::new(),
            bounds_union: ChunkBounds::new(Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            material_id,
            is_impostor: false,
        }
    }

    pub fn add_instance(&mut self, transform: Mat4, bounds: &ChunkBounds) {
        self.instance_data.push(transform);
        self.bounds_union = self.bounds_union.merge(bounds);
    }

    pub fn instance_count(&self) -> usize {
        self.instance_data.len()
    }

    pub fn is_valid(&self) -> bool {
        !self.instance_data.is_empty()
    }

    pub fn sort_back_to_front(&mut self, camera_pos: Vec3) {
        self.instance_data.sort_by(|a, b| {
            let pa = a.col(3).truncate();
            let pb = b.col(3).truncate();
            let da = (pa - camera_pos).length_squared();
            let db = (pb - camera_pos).length_squared();
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn sort_front_to_back(&mut self, camera_pos: Vec3) {
        self.instance_data.sort_by(|a, b| {
            let pa = a.col(3).truncate();
            let pb = b.col(3).truncate();
            let da = (pa - camera_pos).length_squared();
            let db = (pb - camera_pos).length_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

#[derive(Debug, Clone)]
pub struct RenderBatchBuilder {
    pub batches: HashMap<u64, RenderBatch>,
    pub max_instances_per_batch: usize,
}

impl RenderBatchBuilder {
    pub fn new(max_instances: usize) -> Self {
        Self {
            batches: HashMap::new(),
            max_instances_per_batch: max_instances,
        }
    }

    pub fn add(&mut self, mesh_id: u32, lod: LodLevel, material_id: u32, transform: Mat4, bounds: &ChunkBounds) {
        let key = (mesh_id as u64) | ((material_id as u64) << 32) | ((lod.index() as u64) << 48);
        let batch = self.batches.entry(key).or_insert_with(|| RenderBatch::new(mesh_id, lod.clone(), material_id));
        if batch.instance_count() < self.max_instances_per_batch {
            batch.add_instance(transform, bounds);
        } else {
            // Overflow: create new batch with modified key
            let overflow_key = key ^ ((batch.instance_count() as u64) << 56);
            let ob = self.batches.entry(overflow_key).or_insert_with(|| RenderBatch::new(mesh_id, lod.clone(), material_id));
            ob.add_instance(transform, bounds);
        }
    }

    pub fn build(&self) -> Vec<&RenderBatch> {
        let mut batches: Vec<_> = self.batches.values().filter(|b| b.is_valid()).collect();
        // Sort by material to minimize state changes
        batches.sort_by_key(|b| b.material_id);
        batches
    }

    pub fn clear(&mut self) {
        self.batches.clear();
    }

    pub fn total_instances(&self) -> usize {
        self.batches.values().map(|b| b.instance_count()).sum()
    }

    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }
}

// ============================================================
// CHUNK PATCH STITCHER
// ============================================================

#[derive(Debug, Clone)]
pub struct PatchStitcher {
    pub blend_region_cells: usize,
    pub use_geomorphing: bool,
    pub geomorph_distance_range: (f32, f32),
}

impl PatchStitcher {
    pub fn new(blend_cells: usize, geomorph: bool, near: f32, far: f32) -> Self {
        Self {
            blend_region_cells: blend_cells,
            use_geomorphing: geomorph,
            geomorph_distance_range: (near, far),
        }
    }

    pub fn compute_geomorph_alpha(&self, distance: f32) -> f32 {
        let (near, far) = self.geomorph_distance_range;
        smooth_step(near, far, distance)
    }

    pub fn blend_heights(
        &self,
        h_fine: f32,
        h_coarse: f32,
        blend_alpha: f32,
    ) -> f32 {
        h_fine * (1.0 - blend_alpha) + h_coarse * blend_alpha
    }

    pub fn compute_skirt_heights(
        &self,
        edge_heights: &[f32],
        skirt_depth: f32,
    ) -> Vec<f32> {
        edge_heights.iter().map(|&h| h - skirt_depth).collect()
    }

    pub fn stitch_border(
        &self,
        patch: &mut TerrainHeightmap,
        side: usize,
        neighbor: &TerrainHeightmap,
        blend_alpha: f32,
    ) {
        let size = patch.width;
        match side {
            0 => { // +x
                for z in 0..patch.height {
                    let my_h    = patch.get_height(size - 1, z);
                    let nb_h    = neighbor.get_height(0, z);
                    let blended = my_h * (1.0 - blend_alpha) + nb_h * blend_alpha;
                    patch.set_height(size - 1, z, blended);
                }
            }
            1 => { // -x
                for z in 0..patch.height {
                    let my_h    = patch.get_height(0, z);
                    let nb_h    = neighbor.get_height(size - 1, z);
                    let blended = my_h * (1.0 - blend_alpha) + nb_h * blend_alpha;
                    patch.set_height(0, z, blended);
                }
            }
            2 => { // +z
                for x in 0..patch.width {
                    let my_h    = patch.get_height(x, size - 1);
                    let nb_h    = neighbor.get_height(x, 0);
                    let blended = my_h * (1.0 - blend_alpha) + nb_h * blend_alpha;
                    patch.set_height(x, size - 1, blended);
                }
            }
            3 => { // -z
                for x in 0..patch.width {
                    let my_h    = patch.get_height(x, 0);
                    let nb_h    = neighbor.get_height(x, size - 1);
                    let blended = my_h * (1.0 - blend_alpha) + nb_h * blend_alpha;
                    patch.set_height(x, 0, blended);
                }
            }
            _ => {}
        }
    }

    pub fn compute_blend_weights_for_row(
        &self,
        row_len: usize,
        is_start: bool,
    ) -> Vec<f32> {
        let blend_count = self.blend_region_cells.min(row_len);
        let mut weights = vec![1.0f32; row_len];
        for i in 0..blend_count {
            let t = i as f32 / blend_count as f32;
            let w = if is_start { t } else { 1.0 - t };
            let idx = if is_start { i } else { row_len - 1 - i };
            weights[idx] = smooth_step(0.0, 1.0, w);
        }
        weights
    }
}

// ============================================================
// INSTANCE CULLING PIPELINE
// ============================================================

#[derive(Debug, Clone)]
pub struct InstanceCullingPipeline {
    pub frustum: FrustumCulling,
    pub lod_calculator: StreamingDistanceCalculator,
    pub screen_error_metric: ScreenSpaceErrorMetric,
    pub max_instances: usize,
    pub culled_count: u32,
    pub passed_count: u32,
}

impl InstanceCullingPipeline {
    pub fn new(
        frustum: FrustumCulling,
        screen_h: f32,
        fov_rad: f32,
        max_instances: usize,
    ) -> Self {
        Self {
            frustum,
            lod_calculator: StreamingDistanceCalculator::new(screen_h, fov_rad),
            screen_error_metric: ScreenSpaceErrorMetric::new(screen_h, fov_rad, SCREEN_SPACE_ERROR_THRESHOLD),
            max_instances,
            culled_count: 0,
            passed_count: 0,
        }
    }

    pub fn cull_instances(
        &mut self,
        instances: &[(u32, Mat4, ChunkBounds)],
        camera_pos: Vec3,
    ) -> Vec<(u32, Mat4, LodLevel)> {
        self.culled_count = 0;
        self.passed_count = 0;
        let mut result = Vec::with_capacity(instances.len());

        for (id, transform, bounds) in instances {
            if !self.frustum.test_aabb_fast(bounds) {
                self.culled_count += 1;
                continue;
            }
            let dist = bounds.distance_to_point(camera_pos);
            let lod = self.screen_error_metric.select_lod_for_bounds(bounds, camera_pos);
            if lod == LodLevel::Unloaded {
                self.culled_count += 1;
                continue;
            }
            result.push((*id, *transform, lod));
            self.passed_count += 1;
            if result.len() >= self.max_instances { break; }
        }
        result
    }

    pub fn cull_ratio(&self) -> f32 {
        let total = self.culled_count + self.passed_count;
        if total == 0 { return 0.0; }
        self.culled_count as f32 / total as f32
    }

    pub fn update_frustum(&mut self, view_proj: Mat4) {
        self.frustum = FrustumCulling::from_view_proj(view_proj);
    }

    pub fn sort_by_distance_asc(
        instances: &mut Vec<(u32, Mat4, LodLevel)>,
        camera_pos: Vec3,
    ) {
        instances.sort_by(|(_, ta, _), (_, tb, _)| {
            let da = (ta.col(3).truncate() - camera_pos).length_squared();
            let db = (tb.col(3).truncate() - camera_pos).length_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn group_by_lod(
        instances: Vec<(u32, Mat4, LodLevel)>,
    ) -> HashMap<usize, Vec<(u32, Mat4)>> {
        let mut map: HashMap<usize, Vec<(u32, Mat4)>> = HashMap::new();
        for (id, t, lod) in instances {
            map.entry(lod.index()).or_insert_with(Vec::new).push((id, t));
        }
        map
    }
}

// ============================================================
// HEIGHTFIELD COLLISION
// ============================================================

#[derive(Debug, Clone)]
pub struct HeightfieldCollision {
    pub heightmap: TerrainHeightmap,
    pub friction: f32,
    pub restitution: f32,
    pub layer_mask: u32,
}

impl HeightfieldCollision {
    pub fn new(heightmap: TerrainHeightmap) -> Self {
        Self {
            heightmap,
            friction: 0.7,
            restitution: 0.1,
            layer_mask: 0xFFFF_FFFF,
        }
    }

    pub fn height_at_world(&self, x: f32, z: f32) -> f32 {
        self.heightmap.sample_bilinear(x, z)
    }

    pub fn normal_at_world(&self, x: f32, z: f32) -> Vec3 {
        self.heightmap.compute_normal_bilinear(x, z)
    }

    pub fn penetration_depth(&self, point: Vec3) -> f32 {
        let surface_h = self.height_at_world(point.x, point.z);
        (surface_h - point.y).max(0.0)
    }

    pub fn resolve_sphere(
        &self,
        center: Vec3,
        radius: f32,
    ) -> Option<(Vec3, Vec3)> {
        let surf_h = self.height_at_world(center.x, center.z);
        let pen = surf_h + radius - center.y;
        if pen <= 0.0 { return None; }
        let normal = self.normal_at_world(center.x, center.z);
        let resolved = center + normal * pen;
        Some((resolved, normal))
    }

    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32, steps: usize) -> Option<(Vec3, Vec3, f32)> {
        let dir_n = dir.normalize();
        let step_size = max_dist / steps as f32;
        for i in 0..=steps {
            let t = i as f32 * step_size;
            let p = origin + dir_n * t;
            let h = self.height_at_world(p.x, p.z);
            if p.y <= h {
                let normal = self.normal_at_world(p.x, p.z);
                return Some((p, normal, t));
            }
        }
        None
    }

    pub fn slope_degrees_at(&self, x: f32, z: f32) -> f32 {
        self.heightmap.slope_at(x, z)
    }

    pub fn is_walkable(&self, x: f32, z: f32, max_slope_degrees: f32) -> bool {
        self.slope_degrees_at(x, z) <= max_slope_degrees
    }

    pub fn compute_contact_manifold(
        &self,
        sphere_center: Vec3,
        sphere_radius: f32,
        sample_radius: f32,
        samples: u32,
    ) -> Vec<(Vec3, Vec3, f32)> {
        let mut contacts = Vec::new();
        for i in 0..samples {
            let angle = (i as f32 / samples as f32) * std::f32::consts::TAU;
            let sx = sphere_center.x + angle.cos() * sample_radius;
            let sz = sphere_center.z + angle.sin() * sample_radius;
            let h = self.height_at_world(sx, sz);
            let contact_pt = Vec3::new(sx, h, sz);
            let pen = (sphere_center.y - sphere_radius) - h;
            if pen < 0.0 {
                let normal = self.normal_at_world(sx, sz);
                contacts.push((contact_pt, normal, pen.abs()));
            }
        }
        contacts
    }
}

// ============================================================
// ADAPTIVE LOD CONTROLLER
// ============================================================

#[derive(Debug, Clone)]
pub struct AdaptiveLodController {
    pub target_fps: f32,
    pub current_fps: f32,
    pub global_lod_bias: f32,
    pub min_bias: f32,
    pub max_bias: f32,
    pub adjustment_speed: f32,
    pub history: VecDeque<f32>,
    pub history_size: usize,
    pub hysteresis: f32,
}

impl AdaptiveLodController {
    pub fn new(target_fps: f32) -> Self {
        Self {
            target_fps,
            current_fps: target_fps,
            global_lod_bias: 0.0,
            min_bias: -1.0,
            max_bias: 2.0,
            adjustment_speed: 0.05,
            history: VecDeque::new(),
            history_size: 30,
            hysteresis: 5.0,
        }
    }

    pub fn update(&mut self, measured_fps: f32) {
        self.current_fps = measured_fps;
        self.history.push_back(measured_fps);
        if self.history.len() > self.history_size {
            self.history.pop_front();
        }
        let avg_fps = self.history.iter().sum::<f32>() / self.history.len() as f32;
        let deficit = self.target_fps - avg_fps;
        if deficit > self.hysteresis {
            // Need to reduce quality
            self.global_lod_bias = (self.global_lod_bias + self.adjustment_speed).min(self.max_bias);
        } else if deficit < -self.hysteresis {
            // Have headroom, increase quality
            self.global_lod_bias = (self.global_lod_bias - self.adjustment_speed).max(self.min_bias);
        }
    }

    pub fn adjusted_lod_distance(&self, base_dist: f32) -> f32 {
        base_dist * (1.0 - self.global_lod_bias * 0.2)
    }

    pub fn adjusted_streaming_radius(&self, base_radius: f32) -> f32 {
        base_radius * (1.0 - self.global_lod_bias * 0.15).clamp(0.5, 1.5)
    }

    pub fn is_struggling(&self) -> bool {
        self.current_fps < self.target_fps * 0.8
    }

    pub fn quality_factor(&self) -> f32 {
        (1.0 - self.global_lod_bias / self.max_bias).clamp(0.0, 1.0)
    }
}

// ============================================================
// REGION BITMASK
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct RegionBitmask {
    pub bits: Vec<u64>,
    pub width: usize,
    pub height: usize,
}

impl RegionBitmask {
    pub fn new(width: usize, height: usize) -> Self {
        let words = (width * height + 63) / 64;
        Self { bits: vec![0u64; words], width, height }
    }

    pub fn set(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            self.bits[idx / 64] |= 1u64 << (idx % 64);
        }
    }

    pub fn clear(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            self.bits[idx / 64] &= !(1u64 << (idx % 64));
        }
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            (self.bits[idx / 64] >> (idx % 64)) & 1 == 1
        } else {
            false
        }
    }

    pub fn count_set(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn or_with(&mut self, other: &RegionBitmask) {
        let len = self.bits.len().min(other.bits.len());
        for i in 0..len {
            self.bits[i] |= other.bits[i];
        }
    }

    pub fn and_with(&mut self, other: &RegionBitmask) {
        let len = self.bits.len().min(other.bits.len());
        for i in 0..len {
            self.bits[i] &= other.bits[i];
        }
    }

    pub fn invert(&mut self) {
        for word in &mut self.bits { *word = !*word; }
        // Clear extra bits in the last word
        let total = self.width * self.height;
        let last_bits = total % 64;
        if last_bits != 0 {
            if let Some(last) = self.bits.last_mut() {
                let mask = (1u64 << last_bits) - 1;
                *last &= mask;
            }
        }
    }

    pub fn flood_fill(&mut self, start_x: usize, start_y: usize) {
        let mut stack = vec![(start_x, start_y)];
        while let Some((x, y)) = stack.pop() {
            if self.get(x, y) { continue; }
            self.set(x, y);
            if x > 0            { stack.push((x - 1, y)); }
            if x + 1 < self.width { stack.push((x + 1, y)); }
            if y > 0            { stack.push((x, y - 1)); }
            if y + 1 < self.height { stack.push((x, y + 1)); }
        }
    }
}

// ============================================================
// CHUNK UPDATE SCHEDULER
// ============================================================

#[derive(Debug, Clone)]
pub struct ChunkUpdateScheduler {
    pub update_queue: VecDeque<(ChunkCoord, u64)>,  // (coord, scheduled_frame)
    pub in_progress: HashSet<ChunkCoord>,
    pub completed_this_frame: Vec<ChunkCoord>,
    pub max_updates_per_frame: usize,
    pub update_interval_frames: u64,
    pub next_scheduled_frame: HashMap<ChunkCoord, u64>,
}

impl ChunkUpdateScheduler {
    pub fn new(max_per_frame: usize, interval: u64) -> Self {
        Self {
            update_queue: VecDeque::new(),
            in_progress: HashSet::new(),
            completed_this_frame: Vec::new(),
            max_updates_per_frame: max_per_frame,
            update_interval_frames: interval,
            next_scheduled_frame: HashMap::new(),
        }
    }

    pub fn schedule(&mut self, coord: ChunkCoord, current_frame: u64) {
        let next = *self.next_scheduled_frame.get(&coord).unwrap_or(&0);
        if current_frame >= next && !self.in_progress.contains(&coord) {
            self.update_queue.push_back((coord.clone(), current_frame));
            self.next_scheduled_frame.insert(coord, current_frame + self.update_interval_frames);
        }
    }

    pub fn dispatch(&mut self, current_frame: u64) -> Vec<ChunkCoord> {
        let mut dispatched = Vec::new();
        let max = self.max_updates_per_frame;
        while dispatched.len() < max {
            if let Some((coord, frame)) = self.update_queue.pop_front() {
                if current_frame < frame + self.update_interval_frames * 2 {
                    self.in_progress.insert(coord.clone());
                    dispatched.push(coord);
                }
            } else {
                break;
            }
        }
        dispatched
    }

    pub fn complete(&mut self, coord: ChunkCoord) {
        self.in_progress.remove(&coord);
        self.completed_this_frame.push(coord);
    }

    pub fn end_frame(&mut self) {
        self.completed_this_frame.clear();
    }

    pub fn pending_count(&self) -> usize {
        self.update_queue.len()
    }

    pub fn in_progress_count(&self) -> usize {
        self.in_progress.len()
    }

    pub fn reschedule_all_loaded(&mut self, loaded: &HashSet<ChunkCoord>, frame: u64) {
        for coord in loaded {
            self.schedule(coord.clone(), frame);
        }
    }
}

// ============================================================
// IMPOSTOR CAPTURE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct ImpostorCaptureJob {
    pub actor_id: u32,
    pub world_bounds: ChunkBounds,
    pub num_views: u32,
    pub atlas_slot: u32,
    pub is_complete: bool,
    pub capture_frame: u64,
    pub view_directions: Vec<Vec3>,
}

impl ImpostorCaptureJob {
    pub fn new(actor_id: u32, bounds: ChunkBounds, num_views: u32, slot: u32) -> Self {
        let view_directions = Self::compute_view_directions(num_views);
        Self {
            actor_id,
            world_bounds: bounds,
            num_views,
            atlas_slot: slot,
            is_complete: false,
            capture_frame: 0,
            view_directions,
        }
    }

    fn compute_view_directions(num_views: u32) -> Vec<Vec3> {
        (0..num_views)
            .map(|i| {
                let angle = (i as f32 / num_views as f32) * std::f32::consts::TAU;
                Vec3::new(angle.cos(), 0.0, angle.sin())
            })
            .collect()
    }

    pub fn view_matrix_for_view(&self, view_idx: u32) -> Mat4 {
        if view_idx as usize >= self.view_directions.len() {
            return Mat4::IDENTITY;
        }
        let dir = self.view_directions[view_idx as usize];
        let center = self.world_bounds.center();
        let radius = self.world_bounds.half_size().length() * 2.0;
        let eye = center - dir * radius;
        Mat4::look_at_rh(eye, center, Vec3::Y)
    }

    pub fn atlas_uv_for_view(&self, view_idx: u32) -> (Vec2, Vec2) {
        let cols = IMPOSTOR_ATLAS_COLS;
        let rows = IMPOSTOR_ATLAS_ROWS;
        let slot_col = (self.atlas_slot % cols) as f32;
        let slot_row = (self.atlas_slot / cols) as f32;
        let view_col = (view_idx % cols) as f32;
        let view_row = (view_idx / cols) as f32;
        let cell_w = 1.0 / cols as f32;
        let cell_h = 1.0 / rows as f32;
        let _ = (slot_col, slot_row); // suppress unused warning
        let uv_min = Vec2::new(view_col * cell_w, view_row * cell_h);
        let uv_max = uv_min + Vec2::new(cell_w, cell_h);
        (uv_min, uv_max)
    }

    pub fn mark_complete(&mut self, frame: u64) {
        self.is_complete = true;
        self.capture_frame = frame;
    }
}

#[derive(Debug, Clone)]
pub struct ImpostorCaptureQueue {
    pub pending: VecDeque<ImpostorCaptureJob>,
    pub in_progress: Option<ImpostorCaptureJob>,
    pub next_atlas_slot: u32,
    pub max_atlas_slots: u32,
    pub completed_jobs: Vec<ImpostorCaptureJob>,
}

impl ImpostorCaptureQueue {
    pub fn new(max_slots: u32) -> Self {
        Self {
            pending: VecDeque::new(),
            in_progress: None,
            next_atlas_slot: 0,
            max_atlas_slots: max_slots,
            completed_jobs: Vec::new(),
        }
    }

    pub fn request_capture(&mut self, actor_id: u32, bounds: ChunkBounds, num_views: u32) -> Option<u32> {
        if self.next_atlas_slot >= self.max_atlas_slots { return None; }
        let slot = self.next_atlas_slot;
        self.next_atlas_slot += 1;
        let job = ImpostorCaptureJob::new(actor_id, bounds, num_views, slot);
        self.pending.push_back(job);
        Some(slot)
    }

    pub fn tick(&mut self, frame: u64) -> Option<&ImpostorCaptureJob> {
        if self.in_progress.is_none() {
            self.in_progress = self.pending.pop_front();
        }
        if let Some(ref mut job) = self.in_progress {
            if !job.is_complete {
                job.mark_complete(frame);
                let completed = self.in_progress.take().unwrap();
                self.completed_jobs.push(completed);
            }
        }
        self.completed_jobs.last()
    }

    pub fn drain_completed(&mut self) -> Vec<ImpostorCaptureJob> {
        std::mem::take(&mut self.completed_jobs)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

// ============================================================
// SECTOR STREAMING MAP
// ============================================================

#[derive(Debug, Clone)]
pub struct SectorStreamingMap {
    pub sectors: HashMap<ChunkCoord, SectorInfo>,
    pub sector_size_chunks: u32,
    pub chunk_size: f32,
}

#[derive(Debug, Clone)]
pub struct SectorInfo {
    pub coord: ChunkCoord,
    pub chunks: Vec<ChunkCoord>,
    pub is_loaded: bool,
    pub priority: f32,
    pub load_order: u32,
    pub memory_estimate_mb: f32,
    pub last_resident_frame: u64,
}

impl SectorInfo {
    pub fn new(coord: ChunkCoord, sector_size: u32) -> Self {
        let mut chunks = Vec::new();
        let size = sector_size as i32;
        for dz in 0..size {
            for dx in 0..size {
                chunks.push(ChunkCoord::new(
                    coord.x * size + dx,
                    coord.y,
                    coord.z * size + dz,
                ));
            }
        }
        Self {
            coord,
            chunks,
            is_loaded: false,
            priority: 0.0,
            load_order: 0,
            memory_estimate_mb: sector_size as f32 * sector_size as f32 * 8.0,
            last_resident_frame: 0,
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

impl SectorStreamingMap {
    pub fn new(sector_size_chunks: u32, chunk_size: f32) -> Self {
        Self {
            sectors: HashMap::new(),
            sector_size_chunks,
            chunk_size,
        }
    }

    pub fn world_pos_to_sector(&self, pos: Vec3) -> ChunkCoord {
        let chunk = ChunkCoord::from_world_pos(pos, self.chunk_size);
        let size = self.sector_size_chunks as i32;
        ChunkCoord::new(
            chunk.x.div_euclid(size),
            chunk.y.div_euclid(size),
            chunk.z.div_euclid(size),
        )
    }

    pub fn get_or_create_sector(&mut self, coord: ChunkCoord) -> &mut SectorInfo {
        let size = self.sector_size_chunks;
        self.sectors.entry(coord.clone()).or_insert_with(|| SectorInfo::new(coord, size))
    }

    pub fn sectors_in_radius(&self, center_pos: Vec3, radius_m: f32) -> Vec<ChunkCoord> {
        let sector_size_m = self.sector_size_chunks as f32 * self.chunk_size;
        let center_sector = self.world_pos_to_sector(center_pos);
        let radius_sectors = (radius_m / sector_size_m).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dz in -radius_sectors..=radius_sectors {
            for dx in -radius_sectors..=radius_sectors {
                let coord = center_sector.offset(dx, 0, dz);
                let world_center = Vec3::new(
                    (coord.x as f32 + 0.5) * sector_size_m,
                    center_pos.y,
                    (coord.z as f32 + 0.5) * sector_size_m,
                );
                if (world_center - center_pos).length() <= radius_m + sector_size_m {
                    result.push(coord);
                }
            }
        }
        result
    }

    pub fn compute_sector_priorities(&mut self, viewer_pos: Vec3) {
        let chunk_size = self.chunk_size;
        let sector_size = self.sector_size_chunks;
        for (coord, info) in self.sectors.iter_mut() {
            let sector_world = Vec3::new(
                (coord.x as f32 + 0.5) * sector_size as f32 * chunk_size,
                viewer_pos.y,
                (coord.z as f32 + 0.5) * sector_size as f32 * chunk_size,
            );
            let dist = (sector_world - viewer_pos).length();
            info.priority = 1.0 / (1.0 + dist * 0.001);
        }
    }

    pub fn loaded_sector_count(&self) -> usize {
        self.sectors.values().filter(|s| s.is_loaded).count()
    }

    pub fn total_sector_count(&self) -> usize {
        self.sectors.len()
    }

    pub fn total_memory_estimate_mb(&self) -> f32 {
        self.sectors.values().filter(|s| s.is_loaded).map(|s| s.memory_estimate_mb).sum()
    }
}

// ============================================================
// WORLD BOUNDS TRACKER
// ============================================================

#[derive(Debug, Clone)]
pub struct WorldBoundsTracker {
    pub world_bounds: ChunkBounds,
    pub occupied_cells: HashSet<ChunkCoord>,
    pub cell_size: f32,
    pub total_actors: u32,
    pub dirty: bool,
}

impl WorldBoundsTracker {
    pub fn new(cell_size: f32) -> Self {
        Self {
            world_bounds: ChunkBounds::new(Vec3::ZERO, Vec3::ZERO),
            occupied_cells: HashSet::new(),
            cell_size,
            total_actors: 0,
            dirty: false,
        }
    }

    pub fn register(&mut self, pos: Vec3) {
        let coord = ChunkCoord::from_world_pos(pos, self.cell_size);
        self.occupied_cells.insert(coord);
        self.total_actors += 1;
        self.dirty = true;
    }

    pub fn recompute_bounds(&mut self) {
        if !self.dirty { return; }
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for coord in &self.occupied_cells {
            let cell_min = coord.to_world_min(self.cell_size);
            let cell_max = cell_min + Vec3::splat(self.cell_size);
            min = min.min(cell_min);
            max = max.max(cell_max);
        }
        if min.x <= max.x {
            self.world_bounds = ChunkBounds { min, max };
        }
        self.dirty = false;
    }

    pub fn center(&mut self) -> Vec3 {
        self.recompute_bounds();
        self.world_bounds.center()
    }

    pub fn extents(&mut self) -> Vec3 {
        self.recompute_bounds();
        self.world_bounds.size()
    }

    pub fn is_point_in_world(&self, pos: Vec3) -> bool {
        self.world_bounds.contains(pos)
    }

    pub fn cell_count(&self) -> usize {
        self.occupied_cells.len()
    }
}

// ============================================================
// STREAMING LEVEL MANAGER
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingLevel {
    pub id: u32,
    pub name: String,
    pub bounds: ChunkBounds,
    pub chunks: Vec<ChunkCoord>,
    pub load_state: ChunkLoadState,
    pub is_persistent: bool,
    pub min_streaming_distance: f32,
    pub max_streaming_distance: f32,
    pub priority: f32,
}

impl StreamingLevel {
    pub fn new(id: u32, name: String, bounds: ChunkBounds) -> Self {
        Self {
            id,
            name,
            bounds,
            chunks: Vec::new(),
            load_state: ChunkLoadState::Unloaded,
            is_persistent: false,
            min_streaming_distance: 0.0,
            max_streaming_distance: 2048.0,
            priority: 1.0,
        }
    }

    pub fn should_load(&self, viewer_pos: Vec3) -> bool {
        let dist = self.bounds.distance_to_point(viewer_pos);
        dist >= self.min_streaming_distance && dist <= self.max_streaming_distance
    }

    pub fn should_unload(&self, viewer_pos: Vec3) -> bool {
        if self.is_persistent { return false; }
        let dist = self.bounds.distance_to_point(viewer_pos);
        dist > self.max_streaming_distance * 1.2
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

#[derive(Debug, Clone)]
pub struct StreamingLevelManager {
    pub levels: HashMap<u32, StreamingLevel>,
    pub next_id: u32,
    pub loaded_levels: HashSet<u32>,
    pub pending_load: HashSet<u32>,
    pub pending_unload: HashSet<u32>,
}

impl StreamingLevelManager {
    pub fn new() -> Self {
        Self {
            levels: HashMap::new(),
            next_id: 1,
            loaded_levels: HashSet::new(),
            pending_load: HashSet::new(),
            pending_unload: HashSet::new(),
        }
    }

    pub fn register_level(&mut self, name: String, bounds: ChunkBounds) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.levels.insert(id, StreamingLevel::new(id, name, bounds));
        id
    }

    pub fn update_streaming(&mut self, viewer_pos: Vec3) {
        self.pending_load.clear();
        self.pending_unload.clear();

        for (id, level) in &self.levels {
            if self.loaded_levels.contains(id) {
                if level.should_unload(viewer_pos) {
                    self.pending_unload.insert(*id);
                }
            } else {
                if level.should_load(viewer_pos) {
                    self.pending_load.insert(*id);
                }
            }
        }
    }

    pub fn commit_loads(&mut self) {
        for id in self.pending_load.drain().collect::<Vec<_>>() {
            self.loaded_levels.insert(id);
            if let Some(level) = self.levels.get_mut(&id) {
                level.load_state = ChunkLoadState::Loaded;
            }
        }
    }

    pub fn commit_unloads(&mut self) {
        for id in self.pending_unload.drain().collect::<Vec<_>>() {
            self.loaded_levels.remove(&id);
            if let Some(level) = self.levels.get_mut(&id) {
                level.load_state = ChunkLoadState::Unloaded;
            }
        }
    }

    pub fn chunks_for_loaded_levels(&self) -> Vec<ChunkCoord> {
        self.loaded_levels.iter()
            .filter_map(|id| self.levels.get(id))
            .flat_map(|l| l.chunks.iter().cloned())
            .collect()
    }

    pub fn loaded_level_count(&self) -> usize {
        self.loaded_levels.len()
    }

    pub fn total_level_count(&self) -> usize {
        self.levels.len()
    }
}

// ============================================================
// VISIBILITY PROPAGATION (PORTAL SYSTEM STUB)
// ============================================================

#[derive(Debug, Clone)]
pub struct Portal {
    pub id: u32,
    pub from_chunk: ChunkCoord,
    pub to_chunk: ChunkCoord,
    pub center: Vec3,
    pub normal: Vec3,
    pub half_extents: Vec2,
    pub is_open: bool,
}

impl Portal {
    pub fn new(id: u32, from: ChunkCoord, to: ChunkCoord, center: Vec3, normal: Vec3, half_extents: Vec2) -> Self {
        Self {
            id,
            from_chunk: from,
            to_chunk: to,
            center,
            normal,
            half_extents,
            is_open: true,
        }
    }

    pub fn is_visible_from(&self, viewer_pos: Vec3, viewer_forward: Vec3) -> bool {
        if !self.is_open { return false; }
        let to_portal = (self.center - viewer_pos).normalize_or_zero();
        let facing = viewer_forward.dot(to_portal);
        let normal_facing = self.normal.dot(to_portal);
        facing > -0.5 && normal_facing < 0.1
    }

    pub fn bounds_2d(&self) -> [Vec3; 4] {
        let right = Vec3::new(-self.normal.z, 0.0, self.normal.x).normalize_or_zero();
        let up    = Vec3::Y;
        let c     = self.center;
        let hw    = right * self.half_extents.x;
        let hh    = up * self.half_extents.y;
        [c - hw - hh, c + hw - hh, c + hw + hh, c - hw + hh]
    }

    pub fn project_to_screen_rect(
        &self,
        view_proj: Mat4,
    ) -> Option<(Vec2, Vec2)> {
        let corners = self.bounds_2d();
        let mut min_ndc = Vec2::splat(f32::MAX);
        let mut max_ndc = Vec2::splat(f32::MIN);
        for corner in &corners {
            let proj = view_proj.project_point3(*corner);
            let ndc = Vec2::new(proj.x, proj.y);
            min_ndc = min_ndc.min(ndc);
            max_ndc = max_ndc.max(ndc);
        }
        if min_ndc.x > 1.0 || max_ndc.x < -1.0 || min_ndc.y > 1.0 || max_ndc.y < -1.0 {
            None
        } else {
            Some((min_ndc, max_ndc))
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortalVisibilitySystem {
    pub portals: HashMap<u32, Portal>,
    pub chunk_portals: HashMap<ChunkCoord, Vec<u32>>,
    pub next_id: u32,
    pub visible_chunks: HashSet<ChunkCoord>,
}

impl PortalVisibilitySystem {
    pub fn new() -> Self {
        Self {
            portals: HashMap::new(),
            chunk_portals: HashMap::new(),
            next_id: 1,
            visible_chunks: HashSet::new(),
        }
    }

    pub fn add_portal(&mut self, from: ChunkCoord, to: ChunkCoord, center: Vec3, normal: Vec3, half: Vec2) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let portal = Portal::new(id, from.clone(), to.clone(), center, normal, half);
        self.portals.insert(id, portal);
        self.chunk_portals.entry(from).or_insert_with(Vec::new).push(id);
        id
    }

    pub fn compute_visibility(
        &mut self,
        viewer_chunk: &ChunkCoord,
        viewer_pos: Vec3,
        viewer_forward: Vec3,
        max_depth: usize,
    ) {
        self.visible_chunks.clear();
        self.visible_chunks.insert(viewer_chunk.clone());
        let mut to_visit = vec![(viewer_chunk.clone(), 0usize)];
        while let Some((current, depth)) = to_visit.pop() {
            if depth >= max_depth { continue; }
            if let Some(portal_ids) = self.chunk_portals.get(&current).cloned() {
                for pid in portal_ids {
                    if let Some(portal) = self.portals.get(&pid) {
                        if portal.is_visible_from(viewer_pos, viewer_forward) {
                            let dest = portal.to_chunk.clone();
                            if self.visible_chunks.insert(dest.clone()) {
                                to_visit.push((dest, depth + 1));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn is_chunk_visible(&self, coord: &ChunkCoord) -> bool {
        self.visible_chunks.contains(coord)
    }

    pub fn visible_chunk_count(&self) -> usize {
        self.visible_chunks.len()
    }
}

// ============================================================
// FINAL EXTENDED UTILITIES
// ============================================================

pub fn compute_chunk_lod_blend_weights(
    chunk: &StreamingChunk,
    camera: &StreamingCamera,
    config: &StreamingConfig,
    screen_height: f32,
) -> (LodLevel, LodLevel, f32) {
    let dist = chunk.distance_to_viewer;
    let desired = config.lod_for_distance(dist);
    let blend_near = if desired.index() > 0 {
        config.lod_distances[desired.index().saturating_sub(1)]
    } else {
        0.0
    };
    let blend_far = if desired.index() < 5 {
        config.lod_distances[desired.index().min(4)]
    } else {
        config.streaming_radius
    };
    let alpha = smooth_step(blend_near, blend_far, dist);
    (desired.next_lower(), desired.clone(), alpha)
}

pub fn compute_dynamic_streaming_radius(
    base_radius: f32,
    memory_pressure: f32,
    fps_scale: f32,
) -> f32 {
    let mem_scale = (1.0 - memory_pressure * 0.5).clamp(0.4, 1.0);
    let fps_factor = fps_scale.clamp(0.5, 1.5);
    base_radius * mem_scale * fps_factor
}

pub fn clamp_viewer_to_world_bounds(viewer: Vec3, world: &ChunkBounds) -> Vec3 {
    viewer.clamp(world.min, world.max)
}

pub fn world_to_chunk_grid(pos: Vec3, chunk_size: f32) -> (i32, i32, i32) {
    (
        (pos.x / chunk_size).floor() as i32,
        (pos.y / chunk_size).floor() as i32,
        (pos.z / chunk_size).floor() as i32,
    )
}

pub fn chunk_grid_to_world_center(cx: i32, cy: i32, cz: i32, chunk_size: f32) -> Vec3 {
    Vec3::new(
        (cx as f32 + 0.5) * chunk_size,
        (cy as f32 + 0.5) * chunk_size,
        (cz as f32 + 0.5) * chunk_size,
    )
}

pub fn compute_level_streaming_priority(
    level_bounds: &ChunkBounds,
    viewer_pos: Vec3,
    viewer_velocity: Vec3,
    lookahead_t: f32,
) -> f32 {
    let predicted = viewer_pos + viewer_velocity * lookahead_t;
    let dist_current   = level_bounds.distance_to_point(viewer_pos);
    let dist_predicted = level_bounds.distance_to_point(predicted);
    let approach_rate = (dist_current - dist_predicted) / lookahead_t.max(0.001);
    let base = 1.0 / (1.0 + dist_current * 0.001);
    let approach_bonus = approach_rate.max(0.0) * 0.01;
    (base + approach_bonus).clamp(0.0, 1.0)
}

pub fn estimate_lod_memory_total(
    chunks: &HashMap<ChunkCoord, StreamingChunk>,
    base_chunk_memory_mb: f32,
) -> f32 {
    chunks.values()
        .filter(|c| c.lod_level.is_loaded())
        .map(|c| base_chunk_memory_mb * c.lod_level.memory_multiplier())
        .sum()
}

pub fn compute_lod_switch_hysteresis(
    current_lod: &LodLevel,
    desired_lod: &LodLevel,
    base_dist: f32,
    hysteresis_fraction: f32,
) -> f32 {
    if current_lod == desired_lod { return base_dist; }
    if current_lod < desired_lod {
        base_dist * (1.0 + hysteresis_fraction)
    } else {
        base_dist * (1.0 - hysteresis_fraction)
    }
}

pub fn build_lod_distance_array(base_dist: f32, scale_factor: f32) -> [f32; 5] {
    [
        base_dist,
        base_dist * scale_factor,
        base_dist * scale_factor * scale_factor,
        base_dist * scale_factor * scale_factor * scale_factor,
        base_dist * scale_factor * scale_factor * scale_factor * scale_factor,
    ]
}

pub fn compute_per_frame_memory_delta(
    prev_loaded: &HashSet<ChunkCoord>,
    curr_loaded: &HashSet<ChunkCoord>,
    memory_per_chunk_mb: f32,
) -> f32 {
    let newly_loaded   = curr_loaded.difference(prev_loaded).count() as f32;
    let newly_unloaded = prev_loaded.difference(curr_loaded).count() as f32;
    (newly_loaded - newly_unloaded) * memory_per_chunk_mb
}

pub fn aabb_corner_distances(bounds: &ChunkBounds, point: Vec3) -> [f32; 8] {
    let corners = bounds.corners();
    std::array::from_fn(|i| (corners[i] - point).length())
}

pub fn bvh_sah_cost(left_sa: f32, right_sa: f32, parent_sa: f32, left_count: usize, right_count: usize) -> f32 {
    SAH_TRAVERSAL_COST + SAH_INTERSECTION_COST * (
        left_sa  / parent_sa * left_count  as f32
      + right_sa / parent_sa * right_count as f32
    )
}

pub fn compute_cluster_merge_cost(a: &HlodCluster, b: &HlodCluster) -> f32 {
    let merged = a.merged_bounds.merge(&b.merged_bounds);
    let sa_merged = merged.surface_area();
    let sa_a = a.merged_bounds.surface_area();
    let sa_b = b.merged_bounds.surface_area();
    sa_merged - sa_a - sa_b
}

pub fn evaluate_lod_quality_metrics(
    editor: &WorldStreamingEditor,
    reference_positions: &[Vec3],
) -> Vec<(Vec3, LodLevel, f32)> {
    reference_positions.iter().map(|&pos| {
        let dist = pos.length();
        let lod = editor.config.lod_for_distance(dist);
        let quality = 1.0 - lod.memory_multiplier();
        (pos, lod, quality)
    }).collect()
}

pub fn build_streaming_config_from_hardware_caps(
    available_memory_mb: u64,
    available_cores: u32,
    screen_width: u32,
    screen_height: u32,
    target_fps: f32,
) -> StreamingConfig {
    let mut cfg = StreamingConfig::default();
    cfg.max_memory_mb = available_memory_mb;
    cfg.max_concurrent_loads = (available_cores / 2).max(1) as usize;
    cfg.screen_height_pixels = screen_height;
    let aspect = screen_width as f32 / screen_height as f32;
    let budget_tier = available_memory_mb / 1024;
    if budget_tier >= 8 {
        cfg.max_loaded_chunks = 512;
        cfg.enable_occlusion_culling = true;
        cfg.enable_hlod = true;
        cfg.enable_virtual_textures = true;
    } else if budget_tier >= 4 {
        cfg.max_loaded_chunks = 256;
        cfg.enable_hlod = true;
    } else {
        cfg.max_loaded_chunks = 128;
        cfg.enable_impostor_billboards = false;
        cfg.enable_virtual_textures = false;
    }
    if target_fps >= 120.0 {
        cfg.lod_distances = build_lod_distance_array(32.0, 2.0);
    } else if target_fps >= 60.0 {
        cfg.lod_distances = LOD_DISTANCES;
    } else {
        cfg.lod_distances = build_lod_distance_array(24.0, 2.2);
    }
    cfg
}


// ============================================================
// DENSE MATH SUPPLEMENT
// ============================================================

pub fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let m0 = (p2 - p0) * 0.5;
    let m1 = (p3 - p1) * 0.5;
    let b0 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let b1 = t3 - 2.0 * t2 + t;
    let b2 = -2.0 * t3 + 3.0 * t2;
    let b3 = t3 - t2;
    p1 * b0 + m0 * b1 + p2 * b2 + m1 * b3
}

pub fn bezier_cubic(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

pub fn bezier_cubic_tangent(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let u = 1.0 - t;
    (p1 - p0) * (3.0 * u * u) + (p2 - p1) * (6.0 * u * t) + (p3 - p2) * (3.0 * t * t)
}

pub fn bezier_arc_length_table(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, samples: usize) -> Vec<f32> {
    let mut table = vec![0.0f32; samples + 1];
    let mut prev = p0;
    for i in 1..=samples {
        let t = i as f32 / samples as f32;
        let cur = bezier_cubic(p0, p1, p2, p3, t);
        table[i] = table[i - 1] + (cur - prev).length();
        prev = cur;
    }
    table
}

pub fn bezier_arc_length_t(table: &[f32], target_len: f32) -> f32 {
    let total = *table.last().unwrap_or(&0.0);
    if total < 1e-8 { return 0.0; }
    let target = (target_len / total).clamp(0.0, 1.0) * total;
    let n = table.len() - 1;
    for i in 1..table.len() {
        if table[i] >= target {
            let t0 = (i - 1) as f32 / n as f32;
            let t1 = i as f32 / n as f32;
            let frac = if table[i] - table[i-1] > 1e-8 {
                (target - table[i-1]) / (table[i] - table[i-1])
            } else { 0.0 };
            return t0 + (t1 - t0) * frac;
        }
    }
    1.0
}

pub fn thermal_erosion_pass(hm: &mut TerrainHeightmap, talus_angle: f32, carry_fraction: f32) {
    let w = hm.width;
    let h = hm.height;
    let talus = talus_angle.tan() * hm.cell_size;
    let mut delta = vec![0.0f32; w * h];
    for z in 1..h-1 {
        for x in 1..w-1 {
            let c = hm.get_height(x, z);
            let neighbors = [(x-1, z), (x+1, z), (x, z-1), (x, z+1)];
            let mut max_diff = 0.0f32;
            let mut max_n = (x, z);
            for &(nx, nz) in &neighbors {
                let diff = c - hm.get_height(nx, nz);
                if diff > max_diff { max_diff = diff; max_n = (nx, nz); }
            }
            if max_diff > talus {
                let transport = (max_diff - talus) * carry_fraction;
                delta[z * w + x] -= transport;
                delta[max_n.1 * w + max_n.0] += transport;
            }
        }
    }
    for z in 0..h {
        for x in 0..w {
            hm.set_height(x, z, hm.get_height(x, z) + delta[z * w + x]);
        }
    }
}

pub fn hydraulic_erosion_step(
    hm: &mut TerrainHeightmap, drop_x: f32, drop_z: f32,
    volume: f32, erosion_rate: f32, deposition_rate: f32, max_steps: usize,
) {
    let (mut x, mut z, mut vol, mut sediment) = (drop_x, drop_z, volume, 0.0f32);
    for _ in 0..max_steps {
        let grad = hm.compute_normal_bilinear(x, z);
        let speed = Vec2::new(-grad.x, -grad.z).length();
        if speed < 1e-6 { break; }
        let capacity = speed * vol;
        let h_here = hm.sample_bilinear(x, z);
        let ix = ((x - hm.origin.x) / hm.cell_size) as usize;
        let iz = ((z - hm.origin.y) / hm.cell_size) as usize;
        if sediment > capacity {
            let d = (sediment - capacity) * deposition_rate;
            sediment -= d;
            hm.set_height(ix, iz, h_here + d);
        } else {
            let e = (capacity - sediment).min(h_here * erosion_rate);
            sediment += e;
            hm.set_height(ix, iz, (h_here - e).max(0.0));
        }
        x -= grad.x * hm.cell_size;
        z -= grad.z * hm.cell_size;
        vol *= 0.99;
        if vol < 0.01 { break; }
    }
}

pub fn compute_heightmap_ao(hm: &TerrainHeightmap, num_rays: usize, max_dist: f32) -> Vec<f32> {
    let (w, h) = (hm.width, hm.height);
    let mut ao = vec![1.0f32; w * h];
    for z in 0..h {
        for x in 0..w {
            let height = hm.get_height(x, z);
            let wx = hm.origin.x + x as f32 * hm.cell_size;
            let wz = hm.origin.y + z as f32 * hm.cell_size;
            let mut occ = 0.0f32;
            let steps = (max_dist / hm.cell_size) as usize;
            for ray in 0..num_rays {
                let angle = (ray as f32 / num_rays as f32) * std::f32::consts::TAU;
                let (dx, dz) = (angle.cos(), angle.sin());
                let mut max_h = 0.0f32;
                for step in 1..=steps {
                    let t = step as f32 * hm.cell_size;
                    let sh = hm.sample_bilinear(wx + dx * t, wz + dz * t);
                    let horizon = (sh - height) / t;
                    if horizon > max_h { max_h = horizon; }
                }
                occ += (max_h.atan() / std::f32::consts::FRAC_PI_2).clamp(0.0, 1.0);
            }
            ao[z * w + x] = 1.0 - (occ / num_rays as f32).clamp(0.0, 1.0);
        }
    }
    ao
}

pub fn build_lod_transition_curve(min_dist: f32, max_dist: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps).map(|i| {
        let t = i as f32 / steps as f32;
        let d = min_dist + t * (max_dist - min_dist);
        (d, (1.0 - (d - min_dist) / (max_dist - min_dist + 1e-8)).clamp(0.0, 1.0))
    }).collect()
}

pub fn sh_l1_eval(coeffs: &[Vec3; 4], normal: Vec3) -> Vec3 {
    let n = normal.normalize_or_zero();
    coeffs[0] * 0.282095 + coeffs[1] * 0.488603 * n.y
        + coeffs[2] * 0.488603 * n.z + coeffs[3] * 0.488603 * n.x
}

pub fn sh_l1_project(dir: Vec3, color: Vec3) -> [Vec3; 4] {
    let n = dir.normalize_or_zero();
    let w = std::f32::consts::FRAC_1_PI * 0.25;
    [color*w*0.282095, color*w*0.488603*n.y, color*w*0.488603*n.z, color*w*0.488603*n.x]
}

pub fn pack_rgba8(r: f32, g: f32, b: f32, a: f32) -> u32 {
    (r.clamp(0.0,1.0)*255.0) as u32
    | (((g.clamp(0.0,1.0)*255.0) as u32) << 8)
    | (((b.clamp(0.0,1.0)*255.0) as u32) << 16)
    | (((a.clamp(0.0,1.0)*255.0) as u32) << 24)
}

pub fn unpack_rgba8(packed: u32) -> (f32, f32, f32, f32) {
    ((packed & 0xFF) as f32 / 255.0, ((packed>>8)&0xFF) as f32/255.0,
     ((packed>>16)&0xFF) as f32/255.0, ((packed>>24)&0xFF) as f32/255.0)
}

pub fn halton(index: u32, base: u32) -> f32 {
    let (mut result, mut f, mut i) = (0.0f32, 1.0f32, index);
    while i > 0 { f /= base as f32; result += f * (i % base) as f32; i /= base; }
    result
}

pub fn halton_2d(index: u32) -> Vec2 { Vec2::new(halton(index,2), halton(index,3)) }
pub fn halton_3d(index: u32) -> Vec3 { Vec3::new(halton(index,2), halton(index,3), halton(index,5)) }

pub fn exp_smooth(current: f32, target: f32, lambda: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

pub fn exp_smooth_vec3(current: Vec3, target: Vec3, lambda: f32, dt: f32) -> Vec3 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

pub fn aabb_solid_angle_approx(bounds: &ChunkBounds, viewpoint: Vec3) -> f32 {
    let dist = (bounds.center() - viewpoint).length();
    if dist < 1e-6 { return std::f32::consts::TAU * 2.0; }
    let angle = (bounds.half_size().length() / dist).min(1.0).asin();
    std::f32::consts::PI * angle * angle
}

pub fn chunk_visual_importance(bounds: &ChunkBounds, viewpoint: Vec3, object_count: usize) -> f32 {
    aabb_solid_angle_approx(bounds, viewpoint)
        * (object_count as f32 / bounds.volume().max(1.0)).sqrt()
}

pub fn isqrt(n: u64) -> u64 {
    if n == 0 { return 0; }
    let (mut x, mut y) = (n, (n + 1) / 2);
    while y < x { x = y; y = (x + n / x) / 2; }
    x
}

pub fn required_lod_levels(world_size: f32, min_feature: f32) -> u32 {
    if min_feature <= 0.0 { return 1; }
    ((world_size / min_feature).log2().ceil() as u32).max(1)
}

pub fn estimate_bvh_memory_bytes(n: usize) -> usize { 2 * n * 128 }

pub fn estimate_octree_memory_bytes(n: usize, depth: u32) -> usize {
    ((8usize.pow(depth + 1) - 1) / 7).min(n * 4) * 64
}

pub fn bvh_sah_cost_fn(lsa: f32, rsa: f32, psa: f32, lc: usize, rc: usize) -> f32 {
    SAH_TRAVERSAL_COST + SAH_INTERSECTION_COST * (lsa/psa*lc as f32 + rsa/psa*rc as f32)
}

pub fn cluster_merge_cost(a: &HlodCluster, b: &HlodCluster) -> f32 {
    a.merged_bounds.merge(&b.merged_bounds).surface_area()
        - a.merged_bounds.surface_area() - b.merged_bounds.surface_area()
}

pub fn evaluate_lod_quality(editor: &WorldStreamingEditor, positions: &[Vec3]) -> Vec<(Vec3, LodLevel, f32)> {
    positions.iter().map(|&p| {
        let lod = editor.config.lod_for_distance(p.length());
        (p, lod.clone(), 1.0 - lod.memory_multiplier())
    }).collect()
}

pub fn system_capacity_summary() -> Vec<(&'static str, usize)> {
    vec![
        ("MAX_OCTREE_DEPTH",          MAX_OCTREE_DEPTH as usize),
        ("BVH_MAX_LEAF_OBJECTS",      BVH_MAX_LEAF_OBJECTS),
        ("DEFAULT_MAX_LOADED_CHUNKS", DEFAULT_MAX_LOADED_CHUNKS),
        ("DEFAULT_MAX_MEMORY_MB",     DEFAULT_MAX_MEMORY_MB as usize),
        ("MAX_ASYNC_LOAD_QUEUE",      MAX_ASYNC_LOAD_QUEUE),
        ("TERRAIN_PATCH_SIZE",        TERRAIN_PATCH_SIZE),
        ("PROFILER_HISTORY_FRAMES",   PROFILER_HISTORY_FRAMES),
    ]
}
