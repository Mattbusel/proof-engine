#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const MAX_STREAMING_LEVELS: usize = 512;
const MAX_CELLS_PER_AXIS: usize = 256;
const DEFAULT_CELL_SIZE: f32 = 512.0;
const MAX_MEMORY_BUDGET_MB: f32 = 2048.0;
const STREAMING_HYSTERESIS: f32 = 50.0;
const MAX_CONCURRENT_LOADS: usize = 4;
const PREFETCH_LOOKAHEAD_SECONDS: f32 = 2.0;
const HZB_MAX_MIPS: usize = 8;
const MAX_SECTOR_PORTALS: usize = 32;
const LOD_BIAS_DISTANCE_SCALE: f32 = 0.001;
const MAX_DEPENDENCY_DEPTH: usize = 64;
const BANDWIDTH_ESTIMATE_WINDOW: usize = 60;
const LEVEL_TIMELINE_CAPACITY: usize = 1024;

// ============================================================
// ENUMS
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamingState {
    Unloaded,
    Queued,
    Loading,
    Loaded,
    Unloading,
    Failed,
    Evicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Prefetch = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodLevel {
    Lod0 = 0,
    Lod1 = 1,
    Lod2 = 2,
    Lod3 = 3,
    Lod4 = 4,
    Culled = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeShape {
    Box,
    Sphere,
    ConvexHull,
    Cylinder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorTransitionType {
    Immediate,
    Fade,
    Portal,
    Teleport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelPersistence {
    AlwaysLoaded,
    Dynamic,
    Transient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Distance,
    Priority,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DependencyEdgeType {
    HardDependency,
    SoftDependency,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlay {
    None,
    StreamingState,
    MemoryUsage,
    LoadDistance,
    CellGrid,
    OcclusionHzb,
    FrustumCulled,
    PriorityHeatmap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingEventKind {
    LevelQueued,
    LevelLoadStarted,
    LevelLoadCompleted,
    LevelUnloadStarted,
    LevelUnloadCompleted,
    LevelLoadFailed,
    MemoryPressure,
    BudgetExceeded,
    PrefetchHit,
    PrefetchMiss,
}

// ============================================================
// CORE DATA STRUCTURES
// ============================================================

#[derive(Debug, Clone)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn surface_area(&self) -> f32 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    pub fn volume(&self) -> f32 {
        let s = self.size();
        s.x * s.y * s.z
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn expand_by(&self, amount: f32) -> Aabb {
        Aabb {
            min: self.min - Vec3::splat(amount),
            max: self.max + Vec3::splat(amount),
        }
    }

    pub fn distance_sq_to_point(&self, p: Vec3) -> f32 {
        let dx = (self.min.x - p.x).max(0.0).max(p.x - self.max.x);
        let dy = (self.min.y - p.y).max(0.0).max(p.y - self.max.y);
        let dz = (self.min.z - p.z).max(0.0).max(p.z - self.max.z);
        dx * dx + dy * dy + dz * dz
    }

    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        self.distance_sq_to_point(p).sqrt()
    }

    pub fn merge(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn from_center_extents(center: Vec3, extents: Vec3) -> Self {
        Self {
            min: center - extents,
            max: center + extents,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        (p - self.center).length_squared() <= self.radius * self.radius
    }

    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        let dist_sq = aabb.distance_sq_to_point(self.center);
        dist_sq <= self.radius * self.radius
    }

    pub fn intersects_sphere(&self, other: &Sphere) -> bool {
        let r = self.radius + other.radius;
        (self.center - other.center).length_squared() <= r * r
    }
}

#[derive(Debug, Clone)]
pub struct FrustumPlane {
    pub normal: Vec3,
    pub distance: f32,
}

impl FrustumPlane {
    pub fn new(normal: Vec3, distance: f32) -> Self {
        let len = normal.length();
        Self {
            normal: if len > 1e-6 { normal / len } else { normal },
            distance: if len > 1e-6 { distance / len } else { distance },
        }
    }

    pub fn signed_distance_to(&self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.distance
    }
}

#[derive(Debug, Clone)]
pub struct Frustum {
    pub planes: [FrustumPlane; 6],
}

impl Frustum {
    /// Build frustum from view-projection matrix
    pub fn from_view_proj(vp: Mat4) -> Self {
        let cols = vp.to_cols_array_2d();
        // rows of vp for plane extraction
        let r0 = Vec4::new(cols[0][0], cols[1][0], cols[2][0], cols[3][0]);
        let r1 = Vec4::new(cols[0][1], cols[1][1], cols[2][1], cols[3][1]);
        let r2 = Vec4::new(cols[0][2], cols[1][2], cols[2][2], cols[3][2]);
        let r3 = Vec4::new(cols[0][3], cols[1][3], cols[2][3], cols[3][3]);

        let left   = r3 + r0;
        let right  = r3 - r0;
        let bottom = r3 + r1;
        let top    = r3 - r1;
        let near   = r3 + r2;
        let far    = r3 - r2;

        let make = |v: Vec4| FrustumPlane::new(Vec3::new(v.x, v.y, v.z), v.w);

        Self {
            planes: [
                make(left),
                make(right),
                make(bottom),
                make(top),
                make(near),
                make(far),
            ],
        }
    }

    pub fn test_aabb(&self, aabb: &Aabb) -> bool {
        let c = aabb.center();
        let e = aabb.extents();
        for plane in &self.planes {
            let r = e.x * plane.normal.x.abs()
                + e.y * plane.normal.y.abs()
                + e.z * plane.normal.z.abs();
            let d = plane.signed_distance_to(c);
            if d + r < 0.0 {
                return false;
            }
        }
        true
    }

    pub fn test_sphere(&self, sphere: &Sphere) -> bool {
        for plane in &self.planes {
            if plane.signed_distance_to(sphere.center) < -sphere.radius {
                return false;
            }
        }
        true
    }

    pub fn test_point(&self, p: Vec3) -> bool {
        for plane in &self.planes {
            if plane.signed_distance_to(p) < 0.0 {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct ConvexHull {
    pub planes: Vec<FrustumPlane>,
    pub vertices: Vec<Vec3>,
}

impl ConvexHull {
    pub fn new(vertices: Vec<Vec3>) -> Self {
        // Build convex hull planes from vertices (simplified — assume convex input)
        let mut planes = Vec::new();
        // Use centroid for orientation
        let centroid = if !vertices.is_empty() {
            vertices.iter().fold(Vec3::ZERO, |acc, &v| acc + v) / vertices.len() as f32
        } else {
            Vec3::ZERO
        };

        // Build face normals for a simple convex polyhedron (triangulated)
        let n = vertices.len();
        for i in 0..n {
            for j in (i + 1)..n {
                for k in (j + 1)..n {
                    let a = vertices[i];
                    let b = vertices[j];
                    let c = vertices[k];
                    let normal = (b - a).cross(c - a);
                    if normal.length_squared() < 1e-10 {
                        continue;
                    }
                    let n = normal.normalize();
                    let d = -n.dot(a);
                    // Ensure normal points outward from centroid
                    if n.dot(centroid) + d > 0.0 {
                        planes.push(FrustumPlane::new(-n, -d));
                    } else {
                        planes.push(FrustumPlane::new(n, d));
                    }
                    break;
                }
                break;
            }
            break;
        }

        Self { planes, vertices }
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        for plane in &self.planes {
            if plane.signed_distance_to(p) < 0.0 {
                return false;
            }
        }
        true
    }

    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        let c = aabb.center();
        let e = aabb.extents();
        for plane in &self.planes {
            let r = e.x * plane.normal.x.abs()
                + e.y * plane.normal.y.abs()
                + e.z * plane.normal.z.abs();
            if plane.signed_distance_to(c) + r < 0.0 {
                return false;
            }
        }
        true
    }
}

// ============================================================
// HZB (HIERARCHICAL Z BUFFER) OCCLUSION SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct HzbMipLevel {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>, // min-z per tile
}

impl HzbMipLevel {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![1.0f32; width * height],
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let px = ((u * self.width as f32) as usize).min(self.width.saturating_sub(1));
        let py = ((v * self.height as f32) as usize).min(self.height.saturating_sub(1));
        self.data[py * self.width + px]
    }

    pub fn sample_bilinear(&self, u: f32, v: f32) -> f32 {
        let x = u * (self.width as f32 - 1.0);
        let y = v * (self.height as f32 - 1.0);
        let x0 = (x as usize).min(self.width.saturating_sub(1));
        let y0 = (y as usize).min(self.height.saturating_sub(1));
        let x1 = (x0 + 1).min(self.width.saturating_sub(1));
        let y1 = (y0 + 1).min(self.height.saturating_sub(1));
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let v00 = self.data[y0 * self.width + x0];
        let v10 = self.data[y0 * self.width + x1];
        let v01 = self.data[y1 * self.width + x0];
        let v11 = self.data[y1 * self.width + x1];
        v00 * (1.0 - fx) * (1.0 - fy)
            + v10 * fx * (1.0 - fy)
            + v01 * (1.0 - fx) * fy
            + v11 * fx * fy
    }
}

#[derive(Debug, Clone)]
pub struct HierarchicalZBuffer {
    pub mips: Vec<HzbMipLevel>,
    pub base_width: usize,
    pub base_height: usize,
}

impl HierarchicalZBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let mut mips = Vec::new();
        let mut w = width;
        let mut h = height;
        for _ in 0..HZB_MAX_MIPS {
            mips.push(HzbMipLevel::new(w, h));
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            if w == 1 && h == 1 {
                mips.push(HzbMipLevel::new(1, 1));
                break;
            }
        }
        Self { mips, base_width: width, base_height: height }
    }

    /// Build mip hierarchy from base depth buffer via min-reduction
    pub fn build_from_depth(&mut self, depth: &[f32]) {
        if self.mips.is_empty() { return; }
        let w = self.base_width;
        let h = self.base_height;
        // Fill mip 0 from depth
        let mip0 = &mut self.mips[0];
        let len = (w * h).min(depth.len()).min(mip0.data.len());
        mip0.data[..len].copy_from_slice(&depth[..len]);

        // Build subsequent mips
        for i in 1..self.mips.len() {
            let pw = self.mips[i - 1].width;
            let ph = self.mips[i - 1].height;
            let nw = (pw / 2).max(1);
            let nh = (ph / 2).max(1);
            let prev_data = self.mips[i - 1].data.clone();
            let cur = &mut self.mips[i];
            cur.width = nw;
            cur.height = nh;
            cur.data.resize(nw * nh, 1.0);
            for y in 0..nh {
                for x in 0..nw {
                    let sx = (x * 2).min(pw.saturating_sub(1));
                    let sy = (y * 2).min(ph.saturating_sub(1));
                    let sx1 = (sx + 1).min(pw.saturating_sub(1));
                    let sy1 = (sy + 1).min(ph.saturating_sub(1));
                    let v00 = prev_data[sy * pw + sx];
                    let v10 = prev_data[sy * pw + sx1];
                    let v01 = prev_data[sy1 * pw + sx];
                    let v11 = prev_data[sy1 * pw + sx1];
                    // min-z reduction (closest depth wins)
                    cur.data[y * nw + x] = v00.min(v10).min(v01).min(v11);
                }
            }
        }
    }

    /// Test an AABB against HZB. Returns true if potentially visible.
    pub fn test_aabb_visibility(&self, aabb: &Aabb, view_proj: &Mat4) -> bool {
        if self.mips.is_empty() { return true; }

        // Project AABB corners into NDC, find screen-space bounding rect
        let corners = [
            Vec3::new(aabb.min.x, aabb.min.y, aabb.min.z),
            Vec3::new(aabb.max.x, aabb.min.y, aabb.min.z),
            Vec3::new(aabb.min.x, aabb.max.y, aabb.min.z),
            Vec3::new(aabb.max.x, aabb.max.y, aabb.min.z),
            Vec3::new(aabb.min.x, aabb.min.y, aabb.max.z),
            Vec3::new(aabb.max.x, aabb.min.y, aabb.max.z),
            Vec3::new(aabb.min.x, aabb.max.y, aabb.max.z),
            Vec3::new(aabb.max.x, aabb.max.y, aabb.max.z),
        ];

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut min_z = f32::MAX;
        let mut all_behind = true;

        for &c in &corners {
            let clip = *view_proj * Vec4::new(c.x, c.y, c.z, 1.0);
            if clip.w <= 0.0 { continue; }
            all_behind = false;
            let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
            let u = (ndc.x * 0.5 + 0.5).clamp(0.0, 1.0);
            let v = (1.0 - (ndc.y * 0.5 + 0.5)).clamp(0.0, 1.0);
            min_x = min_x.min(u);
            min_y = min_y.min(v);
            max_x = max_x.max(u);
            max_y = max_y.max(v);
            min_z = min_z.min(ndc.z.clamp(0.0, 1.0));
        }

        if all_behind { return false; }

        // Pick appropriate mip level based on screen-space coverage
        let w_uv = max_x - min_x;
        let h_uv = max_y - min_y;
        let max_dim = w_uv.max(h_uv);
        let mip = if max_dim <= 0.0 {
            self.mips.len() - 1
        } else {
            let level = (-max_dim.log2()).max(0.0) as usize;
            level.min(self.mips.len() - 1)
        };

        let mip_data = &self.mips[mip];
        // Sample max-depth in screen-rect to test against our min projected depth
        let x0 = ((min_x * mip_data.width as f32) as usize).min(mip_data.width.saturating_sub(1));
        let x1 = ((max_x * mip_data.width as f32) as usize).min(mip_data.width.saturating_sub(1));
        let y0 = ((min_y * mip_data.height as f32) as usize).min(mip_data.height.saturating_sub(1));
        let y1 = ((max_y * mip_data.height as f32) as usize).min(mip_data.height.saturating_sub(1));

        let mut occluder_depth = f32::MIN;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = mip_data.data[y * mip_data.width + x];
                occluder_depth = occluder_depth.max(d);
            }
        }

        // Visible if our closest point is in front of occluder
        min_z <= occluder_depth + 1e-4
    }
}

// ============================================================
// STREAMING LEVEL
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingLevelAsset {
    pub id: u64,
    pub name: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub uncompressed_size_bytes: u64,
    pub dependencies: Vec<u64>,
    pub load_time_estimate_ms: f32,
}

#[derive(Debug, Clone)]
pub struct StreamingLevel {
    pub id: u64,
    pub name: String,
    pub asset: StreamingLevelAsset,
    pub bounds: Aabb,
    pub sphere_bounds: Sphere,
    pub load_distance: f32,
    pub unload_distance: f32,
    pub priority: LoadPriority,
    pub persistence: LevelPersistence,
    pub state: StreamingState,
    pub lod_bias: f32,
    pub memory_footprint_mb: f32,
    pub current_lod: LodLevel,
    pub sector_id: Option<u64>,
    pub load_timestamp_ms: f64,
    pub unload_timestamp_ms: f64,
    pub load_count: u32,
    pub transform: Mat4,
    pub is_visible: bool,
    pub is_frustum_culled: bool,
    pub is_occlusion_culled: bool,
    pub distance_to_camera: f32,
    pub screen_size: f32,
    pub importance_weight: f32,
}

impl StreamingLevel {
    pub fn new(id: u64, name: String, asset: StreamingLevelAsset, bounds: Aabb) -> Self {
        let center = bounds.center();
        let radius = bounds.extents().length();
        Self {
            id,
            name,
            asset,
            bounds: bounds.clone(),
            sphere_bounds: Sphere::new(center, radius),
            load_distance: 1000.0,
            unload_distance: 1200.0,
            priority: LoadPriority::Medium,
            persistence: LevelPersistence::Dynamic,
            state: StreamingState::Unloaded,
            lod_bias: 0.0,
            memory_footprint_mb: 0.0,
            current_lod: LodLevel::Culled,
            sector_id: None,
            load_timestamp_ms: 0.0,
            unload_timestamp_ms: 0.0,
            load_count: 0,
            transform: Mat4::IDENTITY,
            is_visible: false,
            is_frustum_culled: false,
            is_occlusion_culled: false,
            distance_to_camera: f32::MAX,
            screen_size: 0.0,
            importance_weight: 1.0,
        }
    }

    pub fn compute_lod(&self, distance: f32, lod_bias: f32) -> LodLevel {
        let adjusted = distance * (1.0 + lod_bias * LOD_BIAS_DISTANCE_SCALE);
        if adjusted < 100.0 { LodLevel::Lod0 }
        else if adjusted < 300.0 { LodLevel::Lod1 }
        else if adjusted < 600.0 { LodLevel::Lod2 }
        else if adjusted < 1000.0 { LodLevel::Lod3 }
        else if adjusted < self.load_distance { LodLevel::Lod4 }
        else { LodLevel::Culled }
    }

    pub fn compute_screen_size(&self, camera_pos: Vec3, fov_y_rad: f32, viewport_height: f32) -> f32 {
        let dist = (self.sphere_bounds.center - camera_pos).length().max(0.01);
        let angular_size = 2.0 * (self.sphere_bounds.radius / dist).atan();
        let pixels = (angular_size / fov_y_rad) * viewport_height;
        pixels / viewport_height
    }

    pub fn should_load(&self, camera_pos: Vec3) -> bool {
        match self.persistence {
            LevelPersistence::AlwaysLoaded => true,
            _ => self.bounds.distance_to_point(camera_pos) < self.load_distance,
        }
    }

    pub fn should_unload(&self, camera_pos: Vec3) -> bool {
        match self.persistence {
            LevelPersistence::AlwaysLoaded => false,
            _ => self.bounds.distance_to_point(camera_pos) > self.unload_distance,
        }
    }

    pub fn memory_estimate_mb(&self) -> f32 {
        let base = self.asset.size_bytes as f32 / (1024.0 * 1024.0);
        match self.current_lod {
            LodLevel::Lod0 => base,
            LodLevel::Lod1 => base * 0.7,
            LodLevel::Lod2 => base * 0.4,
            LodLevel::Lod3 => base * 0.2,
            LodLevel::Lod4 => base * 0.1,
            LodLevel::Culled => 0.0,
        }
    }
}

// ============================================================
// WORLD PARTITION GRID
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl CellCoord {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn neighbors_2d(&self) -> [CellCoord; 8] {
        [
            CellCoord::new(self.x - 1, self.y - 1, self.z),
            CellCoord::new(self.x,     self.y - 1, self.z),
            CellCoord::new(self.x + 1, self.y - 1, self.z),
            CellCoord::new(self.x - 1, self.y,     self.z),
            CellCoord::new(self.x + 1, self.y,     self.z),
            CellCoord::new(self.x - 1, self.y + 1, self.z),
            CellCoord::new(self.x,     self.y + 1, self.z),
            CellCoord::new(self.x + 1, self.y + 1, self.z),
        ]
    }

    pub fn neighbors_3d(&self) -> Vec<CellCoord> {
        let mut result = Vec::with_capacity(26);
        for dz in -1i32..=1 {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 && dz == 0 { continue; }
                    result.push(CellCoord::new(self.x + dx, self.y + dy, self.z + dz));
                }
            }
        }
        result
    }

    pub fn manhattan_distance(&self, other: &CellCoord) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs() + (self.z - other.z).abs()
    }

    pub fn chebyshev_distance(&self, other: &CellCoord) -> i32 {
        let dx = (self.x - other.x).abs();
        let dy = (self.y - other.y).abs();
        let dz = (self.z - other.z).abs();
        dx.max(dy).max(dz)
    }
}

#[derive(Debug, Clone)]
pub struct WorldCell {
    pub coord: CellCoord,
    pub bounds: Aabb,
    pub level_ids: Vec<u64>,
    pub dynamic_object_ids: Vec<u64>,
    pub memory_used_mb: f32,
    pub is_active: bool,
    pub load_priority_score: f32,
    pub last_accessed_frame: u64,
}

impl WorldCell {
    pub fn new(coord: CellCoord, cell_size: f32) -> Self {
        let min = Vec3::new(
            coord.x as f32 * cell_size,
            coord.z as f32 * cell_size,
            coord.y as f32 * cell_size,
        );
        let max = min + Vec3::splat(cell_size);
        Self {
            coord,
            bounds: Aabb::new(min, max),
            level_ids: Vec::new(),
            dynamic_object_ids: Vec::new(),
            memory_used_mb: 0.0,
            is_active: false,
            load_priority_score: 0.0,
            last_accessed_frame: 0,
        }
    }

    pub fn center(&self) -> Vec3 {
        self.bounds.center()
    }

    pub fn compute_priority_score(&mut self, camera_pos: Vec3, camera_dir: Vec3) -> f32 {
        let dist = self.bounds.distance_to_point(camera_pos).max(0.01);
        let to_cell = (self.center() - camera_pos).normalize_or_zero();
        let dot = camera_dir.dot(to_cell).clamp(0.0, 1.0);
        // Score: high if close and in front of camera
        let score = (1.0 / dist) * (0.5 + 0.5 * dot);
        self.load_priority_score = score;
        score
    }
}

#[derive(Debug)]
pub struct WorldPartitionGrid {
    pub cell_size: f32,
    pub cells: HashMap<CellCoord, WorldCell>,
    pub spatial_hash: HashMap<u64, CellCoord>, // object_id -> cell
    pub origin: Vec3,
    pub active_radius_cells: i32,
}

impl WorldPartitionGrid {
    pub fn new(cell_size: f32, origin: Vec3) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            spatial_hash: HashMap::new(),
            origin,
            active_radius_cells: 4,
        }
    }

    pub fn world_to_cell(&self, pos: Vec3) -> CellCoord {
        let rel = pos - self.origin;
        CellCoord::new(
            (rel.x / self.cell_size).floor() as i32,
            (rel.z / self.cell_size).floor() as i32,
            (rel.y / self.cell_size).floor() as i32,
        )
    }

    pub fn cell_to_world_center(&self, coord: CellCoord) -> Vec3 {
        Vec3::new(
            self.origin.x + (coord.x as f32 + 0.5) * self.cell_size,
            self.origin.y + (coord.z as f32 + 0.5) * self.cell_size,
            self.origin.z + (coord.y as f32 + 0.5) * self.cell_size,
        )
    }

    pub fn get_or_create_cell(&mut self, coord: CellCoord) -> &mut WorldCell {
        let cell_size = self.cell_size;
        self.cells.entry(coord).or_insert_with(|| WorldCell::new(coord, cell_size))
    }

    pub fn get_cells_in_radius(&self, center: Vec3, radius: f32) -> Vec<CellCoord> {
        let coord = self.world_to_cell(center);
        let cell_radius = (radius / self.cell_size).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dz in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                for dx in -cell_radius..=cell_radius {
                    let c = CellCoord::new(coord.x + dx, coord.y + dy, coord.z + dz);
                    if let Some(cell) = self.cells.get(&c) {
                        if cell.bounds.distance_to_point(center) <= radius {
                            result.push(c);
                        }
                    } else {
                        // Cell doesn't exist yet but check if it could be in range
                        let cell_center = self.cell_to_world_center(c);
                        if (cell_center - center).length() <= radius + self.cell_size {
                            result.push(c);
                        }
                    }
                }
            }
        }
        result
    }

    pub fn register_object(&mut self, object_id: u64, pos: Vec3) {
        let new_coord = self.world_to_cell(pos);
        // Remove from old cell if it exists
        if let Some(&old_coord) = self.spatial_hash.get(&object_id) {
            if old_coord != new_coord {
                if let Some(cell) = self.cells.get_mut(&old_coord) {
                    cell.dynamic_object_ids.retain(|&id| id != object_id);
                }
            }
        }
        self.spatial_hash.insert(object_id, new_coord);
        let cell = self.get_or_create_cell(new_coord);
        if !cell.dynamic_object_ids.contains(&object_id) {
            cell.dynamic_object_ids.push(object_id);
        }
    }

    pub fn unregister_object(&mut self, object_id: u64) {
        if let Some(coord) = self.spatial_hash.remove(&object_id) {
            if let Some(cell) = self.cells.get_mut(&coord) {
                cell.dynamic_object_ids.retain(|&id| id != object_id);
            }
        }
    }

    pub fn query_objects_in_radius(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        let cells = self.get_cells_in_radius(pos, radius);
        let mut result = Vec::new();
        for coord in cells {
            if let Some(cell) = self.cells.get(&coord) {
                for &oid in &cell.dynamic_object_ids {
                    result.push(oid);
                }
            }
        }
        result
    }

    pub fn optimize_cell_size(&self, level_count: usize, world_size: f32) -> f32 {
        // Heuristic: aim for ~4-8 levels per cell
        let target_cells = (level_count / 6).max(1);
        let cells_per_axis = (target_cells as f32).cbrt().ceil() as usize;
        (world_size / cells_per_axis as f32).max(64.0)
    }

    pub fn compute_total_memory_mb(&self) -> f32 {
        self.cells.values().map(|c| c.memory_used_mb).sum()
    }

    pub fn get_neighbor_cells(&self, coord: CellCoord) -> Vec<&WorldCell> {
        coord.neighbors_3d()
            .into_iter()
            .filter_map(|c| self.cells.get(&c))
            .collect()
    }
}

// ============================================================
// STREAMING VOLUMES
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingVolumeBox {
    pub transform: Mat4,
    pub half_extents: Vec3,
}

impl StreamingVolumeBox {
    pub fn contains_point(&self, world_pos: Vec3) -> bool {
        // Transform point to local space
        let inv = self.transform.inverse();
        let local = inv.transform_point3(world_pos);
        local.x.abs() <= self.half_extents.x
            && local.y.abs() <= self.half_extents.y
            && local.z.abs() <= self.half_extents.z
    }

    pub fn distance_to_point(&self, world_pos: Vec3) -> f32 {
        let inv = self.transform.inverse();
        let local = inv.transform_point3(world_pos);
        let dx = (local.x.abs() - self.half_extents.x).max(0.0);
        let dy = (local.y.abs() - self.half_extents.y).max(0.0);
        let dz = (local.z.abs() - self.half_extents.z).max(0.0);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn world_aabb(&self) -> Aabb {
        // Transform all 8 corners
        let e = self.half_extents;
        let corners = [
            Vec3::new(-e.x, -e.y, -e.z),
            Vec3::new( e.x, -e.y, -e.z),
            Vec3::new(-e.x,  e.y, -e.z),
            Vec3::new( e.x,  e.y, -e.z),
            Vec3::new(-e.x, -e.y,  e.z),
            Vec3::new( e.x, -e.y,  e.z),
            Vec3::new(-e.x,  e.y,  e.z),
            Vec3::new( e.x,  e.y,  e.z),
        ];
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for c in corners {
            let w = self.transform.transform_point3(c);
            min = min.min(w);
            max = max.max(w);
        }
        Aabb::new(min, max)
    }
}

#[derive(Debug, Clone)]
pub struct StreamingVolume {
    pub id: u64,
    pub name: String,
    pub shape: VolumeShape,
    pub box_volume: Option<StreamingVolumeBox>,
    pub sphere_volume: Option<Sphere>,
    pub convex_hull: Option<ConvexHull>,
    pub load_radius: f32,
    pub unload_radius: f32,
    pub importance_weight: f32,
    pub target_level_ids: Vec<u64>,
    pub is_enabled: bool,
}

impl StreamingVolume {
    pub fn new_box(id: u64, name: String, transform: Mat4, half_extents: Vec3) -> Self {
        Self {
            id,
            name,
            shape: VolumeShape::Box,
            box_volume: Some(StreamingVolumeBox { transform, half_extents }),
            sphere_volume: None,
            convex_hull: None,
            load_radius: 0.0,
            unload_radius: 0.0,
            importance_weight: 1.0,
            target_level_ids: Vec::new(),
            is_enabled: true,
        }
    }

    pub fn new_sphere(id: u64, name: String, center: Vec3, load_radius: f32, unload_radius: f32) -> Self {
        Self {
            id,
            name,
            shape: VolumeShape::Sphere,
            box_volume: None,
            sphere_volume: Some(Sphere::new(center, load_radius)),
            convex_hull: None,
            load_radius,
            unload_radius,
            importance_weight: 1.0,
            target_level_ids: Vec::new(),
            is_enabled: true,
        }
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        if !self.is_enabled { return false; }
        match self.shape {
            VolumeShape::Box => {
                self.box_volume.as_ref().map_or(false, |b| b.contains_point(p))
            }
            VolumeShape::Sphere => {
                self.sphere_volume.as_ref().map_or(false, |s| s.contains_point(p))
            }
            VolumeShape::ConvexHull => {
                self.convex_hull.as_ref().map_or(false, |c| c.contains_point(p))
            }
            VolumeShape::Cylinder => {
                // Use sphere as fallback
                self.sphere_volume.as_ref().map_or(false, |s| {
                    let flat = Vec3::new(p.x - s.center.x, 0.0, p.z - s.center.z);
                    flat.length_squared() <= s.radius * s.radius
                        && (p.y - s.center.y).abs() <= s.radius
                })
            }
        }
    }

    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        match self.shape {
            VolumeShape::Box => {
                self.box_volume.as_ref().map_or(f32::MAX, |b| b.distance_to_point(p))
            }
            VolumeShape::Sphere | VolumeShape::Cylinder => {
                self.sphere_volume.as_ref().map_or(f32::MAX, |s| {
                    ((s.center - p).length() - s.radius).max(0.0)
                })
            }
            VolumeShape::ConvexHull => {
                // Approximate with AABB
                if let Some(ch) = &self.convex_hull {
                    if ch.contains_point(p) { 0.0 } else { 1.0 } // simplified
                } else { f32::MAX }
            }
        }
    }

    pub fn should_trigger_load(&self, p: Vec3) -> bool {
        match self.shape {
            VolumeShape::Sphere => {
                self.sphere_volume.as_ref().map_or(false, |s| {
                    (s.center - p).length() < self.load_radius
                })
            }
            _ => self.contains_point(p),
        }
    }

    pub fn should_trigger_unload(&self, p: Vec3) -> bool {
        match self.shape {
            VolumeShape::Sphere => {
                self.sphere_volume.as_ref().map_or(false, |s| {
                    (s.center - p).length() > self.unload_radius
                })
            }
            _ => !self.contains_point(p),
        }
    }
}

// ============================================================
// ASYNC LOADING QUEUE
// ============================================================

#[derive(Debug, Clone)]
pub struct LoadRequest {
    pub level_id: u64,
    pub priority: LoadPriority,
    pub distance_weight: f32,
    pub enqueue_time_ms: f64,
    pub predicted_load_time_ms: f32,
    pub is_prefetch: bool,
}

impl LoadRequest {
    pub fn score(&self) -> f32 {
        let priority_bonus = match self.priority {
            LoadPriority::Critical => 1000.0,
            LoadPriority::High => 100.0,
            LoadPriority::Medium => 10.0,
            LoadPriority::Low => 1.0,
            LoadPriority::Prefetch => 0.1,
        };
        priority_bonus + self.distance_weight * 10.0
    }
}

#[derive(Debug)]
pub struct StreamingLoadQueue {
    pub pending: Vec<LoadRequest>,
    pub in_flight: Vec<LoadRequest>,
    pub max_concurrent: usize,
    pub total_bytes_loaded: u64,
    pub total_loads: u64,
    pub bandwidth_samples: VecDeque<f32>, // MB/s
}

impl StreamingLoadQueue {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            pending: Vec::new(),
            in_flight: Vec::new(),
            max_concurrent,
            total_bytes_loaded: 0,
            total_loads: 0,
            bandwidth_samples: VecDeque::with_capacity(BANDWIDTH_ESTIMATE_WINDOW),
        }
    }

    pub fn enqueue(&mut self, request: LoadRequest) {
        // Remove duplicates
        self.pending.retain(|r| r.level_id != request.level_id);
        self.pending.push(request);
        // Sort by score descending
        self.pending.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn dequeue_next(&mut self) -> Option<LoadRequest> {
        if self.in_flight.len() >= self.max_concurrent { return None; }
        if self.pending.is_empty() { return None; }
        let req = self.pending.remove(0);
        self.in_flight.push(req.clone());
        Some(req)
    }

    pub fn complete_load(&mut self, level_id: u64, bytes_loaded: u64, time_ms: f32) {
        self.in_flight.retain(|r| r.level_id != level_id);
        self.total_bytes_loaded += bytes_loaded;
        self.total_loads += 1;
        if time_ms > 0.0 {
            let mb_per_s = (bytes_loaded as f32 / (1024.0 * 1024.0)) / (time_ms / 1000.0);
            if self.bandwidth_samples.len() >= BANDWIDTH_ESTIMATE_WINDOW {
                self.bandwidth_samples.pop_front();
            }
            self.bandwidth_samples.push_back(mb_per_s);
        }
    }

    pub fn cancel_load(&mut self, level_id: u64) {
        self.pending.retain(|r| r.level_id != level_id);
        self.in_flight.retain(|r| r.level_id != level_id);
    }

    pub fn estimated_bandwidth_mb_s(&self) -> f32 {
        if self.bandwidth_samples.is_empty() { return 100.0; }
        let sum: f32 = self.bandwidth_samples.iter().sum();
        sum / self.bandwidth_samples.len() as f32
    }

    pub fn estimated_remaining_time_ms(&self) -> f32 {
        let bandwidth = self.estimated_bandwidth_mb_s();
        let total_pending_mb: f32 = self.pending.iter()
            .map(|r| r.predicted_load_time_ms)
            .sum::<f32>() / 1000.0; // rough
        if bandwidth > 0.0 { total_pending_mb / bandwidth * 1000.0 } else { f32::MAX }
    }

    pub fn is_loading(&self, level_id: u64) -> bool {
        self.in_flight.iter().any(|r| r.level_id == level_id)
    }

    pub fn is_pending(&self, level_id: u64) -> bool {
        self.pending.iter().any(|r| r.level_id == level_id)
    }

    pub fn queue_depth(&self) -> usize {
        self.pending.len() + self.in_flight.len()
    }
}

// ============================================================
// PREFETCH PREDICTOR
// ============================================================

#[derive(Debug, Clone)]
pub struct CameraVelocityTracker {
    pub positions: VecDeque<Vec3>,
    pub timestamps: VecDeque<f64>,
    pub max_samples: usize,
}

impl CameraVelocityTracker {
    pub fn new(max_samples: usize) -> Self {
        Self {
            positions: VecDeque::with_capacity(max_samples),
            timestamps: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add_sample(&mut self, pos: Vec3, time_ms: f64) {
        if self.positions.len() >= self.max_samples {
            self.positions.pop_front();
            self.timestamps.pop_front();
        }
        self.positions.push_back(pos);
        self.timestamps.push_back(time_ms);
    }

    pub fn velocity(&self) -> Vec3 {
        let n = self.positions.len();
        if n < 2 { return Vec3::ZERO; }
        let dt = (self.timestamps[n - 1] - self.timestamps[0]) / 1000.0; // seconds
        if dt < 1e-6 { return Vec3::ZERO; }
        let dp = self.positions[n - 1] - self.positions[0];
        dp / dt as f32
    }

    pub fn acceleration(&self) -> Vec3 {
        let n = self.positions.len();
        if n < 3 { return Vec3::ZERO; }
        let mid = n / 2;
        let dt1 = ((self.timestamps[mid] - self.timestamps[0]) / 1000.0) as f32;
        let dt2 = ((self.timestamps[n - 1] - self.timestamps[mid]) / 1000.0) as f32;
        if dt1 < 1e-6 || dt2 < 1e-6 { return Vec3::ZERO; }
        let v1 = (self.positions[mid] - self.positions[0]) / dt1;
        let v2 = (self.positions[n - 1] - self.positions[mid]) / dt2;
        let dt = (dt1 + dt2) * 0.5;
        (v2 - v1) / dt
    }

    pub fn predict_position(&self, lookahead_s: f32) -> Vec3 {
        if self.positions.is_empty() { return Vec3::ZERO; }
        let current = *self.positions.back().unwrap();
        let vel = self.velocity();
        let acc = self.acceleration();
        // Kinematic prediction: p + v*t + 0.5*a*t^2
        current + vel * lookahead_s + acc * 0.5 * lookahead_s * lookahead_s
    }
}

#[derive(Debug)]
pub struct PrefetchPredictor {
    pub camera_tracker: CameraVelocityTracker,
    pub lookahead_seconds: f32,
    pub prefetch_budget_ratio: f32, // fraction of memory budget for prefetch
}

impl PrefetchPredictor {
    pub fn new(lookahead_seconds: f32) -> Self {
        Self {
            camera_tracker: CameraVelocityTracker::new(30),
            lookahead_seconds,
            prefetch_budget_ratio: 0.2,
        }
    }

    pub fn update(&mut self, camera_pos: Vec3, time_ms: f64) {
        self.camera_tracker.add_sample(camera_pos, time_ms);
    }

    pub fn predicted_camera_pos(&self) -> Vec3 {
        self.camera_tracker.predict_position(self.lookahead_seconds)
    }

    pub fn get_prefetch_candidates<'a>(
        &self,
        levels: &'a [StreamingLevel],
        current_pos: Vec3,
    ) -> Vec<u64> {
        let predicted = self.predicted_camera_pos();
        let mut candidates = Vec::new();
        for level in levels {
            if level.state != StreamingState::Unloaded { continue; }
            // Check if predicted position would require this level
            if level.bounds.distance_to_point(predicted) < level.load_distance {
                // Not needed from current pos
                if level.bounds.distance_to_point(current_pos) >= level.load_distance {
                    candidates.push(level.id);
                }
            }
        }
        candidates
    }
}

// ============================================================
// MEMORY PRESSURE MANAGER
// ============================================================

#[derive(Debug)]
pub struct MemoryPressureManager {
    pub budget_mb: f32,
    pub used_mb: f32,
    pub eviction_policy: EvictionPolicy,
    pub lru_order: VecDeque<u64>, // level_ids in LRU order
    pub access_counts: HashMap<u64, u64>,
    pub pressure_threshold: f32,
    pub critical_threshold: f32,
}

impl MemoryPressureManager {
    pub fn new(budget_mb: f32) -> Self {
        Self {
            budget_mb,
            used_mb: 0.0,
            eviction_policy: EvictionPolicy::Lru,
            lru_order: VecDeque::new(),
            access_counts: HashMap::new(),
            pressure_threshold: 0.8,
            critical_threshold: 0.95,
        }
    }

    pub fn is_under_pressure(&self) -> bool {
        self.used_mb / self.budget_mb > self.pressure_threshold
    }

    pub fn is_critical(&self) -> bool {
        self.used_mb / self.budget_mb > self.critical_threshold
    }

    pub fn available_mb(&self) -> f32 {
        (self.budget_mb - self.used_mb).max(0.0)
    }

    pub fn can_load(&self, size_mb: f32) -> bool {
        self.used_mb + size_mb <= self.budget_mb
    }

    pub fn record_access(&mut self, level_id: u64) {
        self.lru_order.retain(|&id| id != level_id);
        self.lru_order.push_back(level_id);
        *self.access_counts.entry(level_id).or_insert(0) += 1;
    }

    pub fn record_load(&mut self, level_id: u64, size_mb: f32) {
        self.used_mb += size_mb;
        self.record_access(level_id);
    }

    pub fn record_unload(&mut self, level_id: u64, size_mb: f32) {
        self.used_mb = (self.used_mb - size_mb).max(0.0);
        self.lru_order.retain(|&id| id != level_id);
        self.access_counts.remove(&level_id);
    }

    pub fn select_eviction_candidates(&self, needed_mb: f32, levels: &[StreamingLevel]) -> Vec<u64> {
        let mut candidates: Vec<u64> = Vec::new();
        let mut freed = 0.0f32;

        let sortable_levels: Vec<&StreamingLevel> = levels.iter()
            .filter(|l| l.state == StreamingState::Loaded && l.persistence != LevelPersistence::AlwaysLoaded)
            .collect();

        let mut order: Vec<usize> = (0..sortable_levels.len()).collect();

        match self.eviction_policy {
            EvictionPolicy::Lru => {
                // Sort by LRU position (front = least recently used)
                order.sort_by_key(|&i| {
                    self.lru_order.iter().position(|&id| id == sortable_levels[i].id)
                        .unwrap_or(0)
                });
            }
            EvictionPolicy::Lfu => {
                order.sort_by_key(|&i| {
                    self.access_counts.get(&sortable_levels[i].id).copied().unwrap_or(0)
                });
            }
            EvictionPolicy::Distance => {
                order.sort_by(|&a, &b| {
                    sortable_levels[b].distance_to_camera
                        .partial_cmp(&sortable_levels[a].distance_to_camera)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            EvictionPolicy::Priority => {
                order.sort_by_key(|&i| sortable_levels[i].priority as u8);
            }
        }

        for i in order {
            if freed >= needed_mb { break; }
            let lvl = sortable_levels[i];
            candidates.push(lvl.id);
            freed += lvl.memory_footprint_mb;
        }

        candidates
    }

    pub fn pressure_ratio(&self) -> f32 {
        if self.budget_mb <= 0.0 { return 1.0; }
        (self.used_mb / self.budget_mb).clamp(0.0, 1.0)
    }
}

// ============================================================
// DEPENDENCY GRAPH
// ============================================================

#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub level_id: u64,
    pub dependencies: Vec<u64>,
    pub dependents: Vec<u64>,
    pub edge_types: HashMap<u64, DependencyEdgeType>,
}

impl DependencyNode {
    pub fn new(level_id: u64) -> Self {
        Self {
            level_id,
            dependencies: Vec::new(),
            dependents: Vec::new(),
            edge_types: HashMap::new(),
        }
    }

    pub fn add_dependency(&mut self, dep_id: u64, edge_type: DependencyEdgeType) {
        if !self.dependencies.contains(&dep_id) {
            self.dependencies.push(dep_id);
            self.edge_types.insert(dep_id, edge_type);
        }
    }
}

#[derive(Debug)]
pub struct DependencyGraph {
    pub nodes: HashMap<u64, DependencyNode>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    pub fn add_level(&mut self, level_id: u64) {
        self.nodes.entry(level_id).or_insert_with(|| DependencyNode::new(level_id));
    }

    pub fn add_dependency(&mut self, from: u64, to: u64, edge_type: DependencyEdgeType) {
        self.add_level(from);
        self.add_level(to);
        if let Some(node) = self.nodes.get_mut(&from) {
            node.add_dependency(to, edge_type);
        }
        if let Some(node) = self.nodes.get_mut(&to) {
            if !node.dependents.contains(&from) {
                node.dependents.push(from);
            }
        }
    }

    /// DFS-based cycle detection. Returns Some(cycle) if a cycle exists.
    pub fn detect_cycles(&self) -> Option<Vec<u64>> {
        let mut visited: HashSet<u64> = HashSet::new();
        let mut rec_stack: HashSet<u64> = HashSet::new();
        let mut path: Vec<u64> = Vec::new();

        for &start_id in self.nodes.keys() {
            if !visited.contains(&start_id) {
                if let Some(cycle) = self.dfs_cycle_detect(start_id, &mut visited, &mut rec_stack, &mut path) {
                    return Some(cycle);
                }
            }
        }
        None
    }

    fn dfs_cycle_detect(
        &self,
        node_id: u64,
        visited: &mut HashSet<u64>,
        rec_stack: &mut HashSet<u64>,
        path: &mut Vec<u64>,
    ) -> Option<Vec<u64>> {
        visited.insert(node_id);
        rec_stack.insert(node_id);
        path.push(node_id);

        if let Some(node) = self.nodes.get(&node_id) {
            for &dep in &node.dependencies {
                let edge = node.edge_types.get(&dep).copied().unwrap_or(DependencyEdgeType::HardDependency);
                if edge == DependencyEdgeType::Optional { continue; }

                if !visited.contains(&dep) {
                    if let Some(cycle) = self.dfs_cycle_detect(dep, visited, rec_stack, path) {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(&dep) {
                    // Found cycle - extract it
                    if let Some(start) = path.iter().position(|&id| id == dep) {
                        return Some(path[start..].to_vec());
                    }
                    return Some(vec![dep]);
                }
            }
        }

        path.pop();
        rec_stack.remove(&node_id);
        None
    }

    /// Topological sort (Kahn's algorithm). Returns load order.
    pub fn topological_sort(&self) -> Result<Vec<u64>, Vec<u64>> {
        let mut in_degree: HashMap<u64, usize> = HashMap::new();
        for &id in self.nodes.keys() {
            in_degree.insert(id, 0);
        }
        for node in self.nodes.values() {
            for &dep in &node.dependencies {
                let edge = node.edge_types.get(&dep).copied().unwrap_or(DependencyEdgeType::HardDependency);
                if edge != DependencyEdgeType::Optional {
                    *in_degree.entry(dep).or_insert(0) += 0; // ensure dep exists
                    *in_degree.entry(node.level_id).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<u64> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut order = Vec::new();

        while let Some(id) = queue.pop_front() {
            order.push(id);
            if let Some(node) = self.nodes.get(&id) {
                for &dependent in &node.dependents {
                    if let Some(d) = in_degree.get_mut(&dependent) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            queue.push_back(dependent);
                        }
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            // Cycle detected — return remaining nodes
            let remaining: Vec<u64> = in_degree.iter()
                .filter(|(_, &d)| d > 0)
                .map(|(&id, _)| id)
                .collect();
            Err(remaining)
        } else {
            Ok(order)
        }
    }

    /// Plan parallel load batches based on dependency order
    pub fn parallel_load_plan(&self) -> Vec<Vec<u64>> {
        let mut batches: Vec<Vec<u64>> = Vec::new();
        match self.topological_sort() {
            Ok(order) => {
                let mut loaded: HashSet<u64> = HashSet::new();
                let mut remaining: Vec<u64> = order;
                while !remaining.is_empty() {
                    let mut batch: Vec<u64> = Vec::new();
                    let mut next_remaining: Vec<u64> = Vec::new();
                    for id in remaining {
                        let can_load = if let Some(node) = self.nodes.get(&id) {
                            node.dependencies.iter().all(|dep| {
                                let edge = node.edge_types.get(dep).copied()
                                    .unwrap_or(DependencyEdgeType::HardDependency);
                                edge == DependencyEdgeType::Optional || loaded.contains(dep)
                            })
                        } else { true };
                        if can_load {
                            batch.push(id);
                        } else {
                            next_remaining.push(id);
                        }
                    }
                    if batch.is_empty() { break; } // Safety against infinite loop
                    for &id in &batch { loaded.insert(id); }
                    batches.push(batch);
                    remaining = next_remaining;
                }
            }
            Err(_) => {
                // Fallback: load everything in one batch
                batches.push(self.nodes.keys().copied().collect());
            }
        }
        batches
    }

    pub fn get_all_dependencies(&self, level_id: u64, include_optional: bool) -> HashSet<u64> {
        let mut result = HashSet::new();
        let mut stack = vec![level_id];
        while let Some(id) = stack.pop() {
            if result.contains(&id) { continue; }
            result.insert(id);
            if let Some(node) = self.nodes.get(&id) {
                for &dep in &node.dependencies {
                    let edge = node.edge_types.get(&dep).copied()
                        .unwrap_or(DependencyEdgeType::HardDependency);
                    if include_optional || edge != DependencyEdgeType::Optional {
                        stack.push(dep);
                    }
                }
            }
        }
        result.remove(&level_id);
        result
    }
}

// ============================================================
// PORTAL SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct Portal {
    pub id: u64,
    pub sector_a: u64,
    pub sector_b: u64,
    pub center: Vec3,
    pub normal: Vec3,
    pub half_extents: Vec2,
    pub is_open: bool,
    pub transmission: f32, // 0-1, visibility through portal
}

impl Portal {
    pub fn new(id: u64, sector_a: u64, sector_b: u64, center: Vec3, normal: Vec3, half_extents: Vec2) -> Self {
        Self {
            id,
            sector_a,
            sector_b,
            center,
            normal: normal.normalize_or_zero(),
            half_extents,
            is_open: true,
            transmission: 1.0,
        }
    }

    pub fn is_visible_from(&self, camera_pos: Vec3) -> bool {
        if !self.is_open { return false; }
        let to_portal = (self.center - camera_pos).normalize_or_zero();
        // Visible if camera is on positive side of portal
        self.normal.dot(to_portal) < 0.0
    }

    pub fn project_to_clip(&self, view_proj: &Mat4) -> Option<[Vec2; 4]> {
        let right = Vec3::new(self.normal.z, 0.0, -self.normal.x).normalize_or_zero();
        let up = right.cross(self.normal).normalize_or_zero();
        let corners = [
            self.center + right * self.half_extents.x + up * self.half_extents.y,
            self.center - right * self.half_extents.x + up * self.half_extents.y,
            self.center - right * self.half_extents.x - up * self.half_extents.y,
            self.center + right * self.half_extents.x - up * self.half_extents.y,
        ];
        let mut result = [[0.0f32; 2]; 4];
        for (i, &c) in corners.iter().enumerate() {
            let clip = *view_proj * Vec4::new(c.x, c.y, c.z, 1.0);
            if clip.w <= 0.0 { return None; }
            result[i] = [clip.x / clip.w, clip.y / clip.w];
        }
        Some(result.map(|[x, y]| Vec2::new(x, y)))
    }
}

// ============================================================
// SECTOR SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct Waypoint {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub tags: Vec<String>,
    pub is_spawn_point: bool,
    pub spawn_radius: f32,
}

impl Waypoint {
    pub fn new(id: u64, name: String, position: Vec3) -> Self {
        Self {
            id,
            name,
            position,
            rotation: Quat::IDENTITY,
            tags: Vec::new(),
            is_spawn_point: false,
            spawn_radius: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sector {
    pub id: u64,
    pub name: String,
    pub bounds: Aabb,
    pub portals: Vec<u64>,          // Portal IDs
    pub adjacent_sectors: Vec<u64>, // Sector IDs
    pub waypoints: Vec<Waypoint>,
    pub level_ids: Vec<u64>,
    pub ai_spawn_budget: u32,
    pub is_interior: bool,
    pub ambient_sound_id: Option<u64>,
    pub reverb_preset: u32,
    pub transition_type: SectorTransitionType,
}

impl Sector {
    pub fn new(id: u64, name: String, bounds: Aabb) -> Self {
        Self {
            id,
            name,
            bounds,
            portals: Vec::new(),
            adjacent_sectors: Vec::new(),
            waypoints: Vec::new(),
            level_ids: Vec::new(),
            ai_spawn_budget: 10,
            is_interior: false,
            ambient_sound_id: None,
            reverb_preset: 0,
            transition_type: SectorTransitionType::Fade,
        }
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        self.bounds.contains_point(p)
    }

    pub fn nearest_waypoint(&self, pos: Vec3) -> Option<&Waypoint> {
        self.waypoints.iter().min_by(|a, b| {
            let da = (a.position - pos).length_squared();
            let db = (b.position - pos).length_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn spawn_waypoints(&self) -> Vec<&Waypoint> {
        self.waypoints.iter().filter(|w| w.is_spawn_point).collect()
    }
}

#[derive(Debug)]
pub struct SectorGraph {
    pub sectors: HashMap<u64, Sector>,
    pub portals: HashMap<u64, Portal>,
    pub current_sector: Option<u64>,
}

impl SectorGraph {
    pub fn new() -> Self {
        Self {
            sectors: HashMap::new(),
            portals: HashMap::new(),
            current_sector: None,
        }
    }

    pub fn add_sector(&mut self, sector: Sector) {
        self.sectors.insert(sector.id, sector);
    }

    pub fn add_portal(&mut self, portal: Portal) {
        let id = portal.id;
        let sa = portal.sector_a;
        let sb = portal.sector_b;
        self.portals.insert(id, portal);
        if let Some(s) = self.sectors.get_mut(&sa) {
            if !s.portals.contains(&id) { s.portals.push(id); }
            if !s.adjacent_sectors.contains(&sb) { s.adjacent_sectors.push(sb); }
        }
        if let Some(s) = self.sectors.get_mut(&sb) {
            if !s.portals.contains(&id) { s.portals.push(id); }
            if !s.adjacent_sectors.contains(&sa) { s.adjacent_sectors.push(sa); }
        }
    }

    pub fn find_sector_at(&self, pos: Vec3) -> Option<u64> {
        for (id, sector) in &self.sectors {
            if sector.contains_point(pos) {
                return Some(*id);
            }
        }
        None
    }

    /// BFS to find potentially visible sectors through portals from a camera position
    pub fn compute_pvs(&self, camera_pos: Vec3, view_proj: &Mat4, max_depth: usize) -> HashSet<u64> {
        let start = match self.find_sector_at(camera_pos) {
            Some(id) => id,
            None => return HashSet::new(),
        };

        let mut visible = HashSet::new();
        visible.insert(start);
        let mut queue: VecDeque<(u64, usize)> = VecDeque::new();
        queue.push_back((start, 0));

        while let Some((sector_id, depth)) = queue.pop_front() {
            if depth >= max_depth { continue; }
            let portal_ids = if let Some(s) = self.sectors.get(&sector_id) {
                s.portals.clone()
            } else { continue };

            for portal_id in portal_ids {
                if let Some(portal) = self.portals.get(&portal_id) {
                    if !portal.is_open { continue; }
                    if !portal.is_visible_from(camera_pos) { continue; }
                    let next = if portal.sector_a == sector_id { portal.sector_b } else { portal.sector_a };
                    if visible.insert(next) {
                        queue.push_back((next, depth + 1));
                    }
                }
            }
        }

        visible
    }

    pub fn get_adjacent_level_ids(&self, sector_id: u64) -> Vec<u64> {
        let mut result = Vec::new();
        if let Some(sector) = self.sectors.get(&sector_id) {
            for &level_id in &sector.level_ids {
                result.push(level_id);
            }
            for &adj_id in &sector.adjacent_sectors {
                if let Some(adj) = self.sectors.get(&adj_id) {
                    for &level_id in &adj.level_ids {
                        if !result.contains(&level_id) {
                            result.push(level_id);
                        }
                    }
                }
            }
        }
        result
    }
}

// ============================================================
// STREAMING EVENT TIMELINE
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingEvent {
    pub timestamp_ms: f64,
    pub kind: StreamingEventKind,
    pub level_id: u64,
    pub level_name: String,
    pub data_mb: f32,
    pub duration_ms: f32,
    pub camera_pos: Vec3,
}

#[derive(Debug)]
pub struct StreamingTimeline {
    pub events: VecDeque<StreamingEvent>,
    pub capacity: usize,
    pub start_time_ms: f64,
    pub bandwidth_history: VecDeque<(f64, f32)>, // (time, MB/s)
    pub memory_history: VecDeque<(f64, f32)>,    // (time, MB used)
}

impl StreamingTimeline {
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            start_time_ms: 0.0,
            bandwidth_history: VecDeque::with_capacity(capacity),
            memory_history: VecDeque::with_capacity(capacity),
        }
    }

    pub fn record(&mut self, event: StreamingEvent) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn record_bandwidth(&mut self, time_ms: f64, mb_per_s: f32) {
        if self.bandwidth_history.len() >= self.capacity {
            self.bandwidth_history.pop_front();
        }
        self.bandwidth_history.push_back((time_ms, mb_per_s));
    }

    pub fn record_memory(&mut self, time_ms: f64, mb_used: f32) {
        if self.memory_history.len() >= self.capacity {
            self.memory_history.pop_front();
        }
        self.memory_history.push_back((time_ms, mb_used));
    }

    pub fn events_in_range(&self, start_ms: f64, end_ms: f64) -> Vec<&StreamingEvent> {
        self.events.iter()
            .filter(|e| e.timestamp_ms >= start_ms && e.timestamp_ms <= end_ms)
            .collect()
    }

    pub fn load_events(&self) -> Vec<&StreamingEvent> {
        self.events.iter()
            .filter(|e| e.kind == StreamingEventKind::LevelLoadCompleted)
            .collect()
    }

    pub fn unload_events(&self) -> Vec<&StreamingEvent> {
        self.events.iter()
            .filter(|e| e.kind == StreamingEventKind::LevelUnloadCompleted)
            .collect()
    }

    pub fn total_data_loaded_mb(&self) -> f32 {
        self.load_events().iter().map(|e| e.data_mb).sum()
    }

    pub fn average_load_time_ms(&self) -> f32 {
        let loads = self.load_events();
        if loads.is_empty() { return 0.0; }
        let total: f32 = loads.iter().map(|e| e.duration_ms).sum();
        total / loads.len() as f32
    }
}

// ============================================================
// LOD + STREAMING BUDGET INTEGRATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct LodBudgetEntry {
    pub level_id: u64,
    pub lod_memory_mb: f32,       // memory at current LOD
    pub streaming_memory_mb: f32, // streaming overhead
    pub lod_level: LodLevel,
    pub lod_bias_contribution: f32,
}

#[derive(Debug)]
pub struct CombinedBudgetManager {
    pub total_budget_mb: f32,
    pub lod_budget_mb: f32,
    pub streaming_budget_mb: f32,
    pub entries: HashMap<u64, LodBudgetEntry>,
}

impl CombinedBudgetManager {
    pub fn new(total_budget_mb: f32) -> Self {
        Self {
            total_budget_mb,
            lod_budget_mb: total_budget_mb * 0.6,
            streaming_budget_mb: total_budget_mb * 0.4,
            entries: HashMap::new(),
        }
    }

    pub fn update_entry(&mut self, level_id: u64, lod: LodLevel, lod_mem: f32, stream_mem: f32) {
        let entry = self.entries.entry(level_id).or_insert(LodBudgetEntry {
            level_id,
            lod_memory_mb: 0.0,
            streaming_memory_mb: 0.0,
            lod_level: LodLevel::Culled,
            lod_bias_contribution: 0.0,
        });
        entry.lod_level = lod;
        entry.lod_memory_mb = lod_mem;
        entry.streaming_memory_mb = stream_mem;
    }

    pub fn total_lod_usage_mb(&self) -> f32 {
        self.entries.values().map(|e| e.lod_memory_mb).sum()
    }

    pub fn total_streaming_usage_mb(&self) -> f32 {
        self.entries.values().map(|e| e.streaming_memory_mb).sum()
    }

    pub fn total_usage_mb(&self) -> f32 {
        self.total_lod_usage_mb() + self.total_streaming_usage_mb()
    }

    pub fn available_mb(&self) -> f32 {
        (self.total_budget_mb - self.total_usage_mb()).max(0.0)
    }

    pub fn compute_global_lod_bias(&self) -> f32 {
        let pressure = self.total_usage_mb() / self.total_budget_mb;
        if pressure < 0.7 { 0.0 }
        else if pressure < 0.9 { (pressure - 0.7) / 0.2 * 2.0 }
        else { 2.0 + (pressure - 0.9) / 0.1 * 4.0 }
    }

    pub fn optimal_lod_for_budget(&self, level_id: u64, distance: f32) -> LodLevel {
        let bias = self.compute_global_lod_bias();
        let adjusted_dist = distance * (1.0 + bias * 0.5);
        if adjusted_dist < 100.0 { LodLevel::Lod0 }
        else if adjusted_dist < 300.0 { LodLevel::Lod1 }
        else if adjusted_dist < 600.0 { LodLevel::Lod2 }
        else if adjusted_dist < 1000.0 { LodLevel::Lod3 }
        else { LodLevel::Lod4 }
    }
}

// ============================================================
// DEBUG VISUALIZATION
// ============================================================

#[derive(Debug, Clone)]
pub struct DebugDrawCommand {
    pub kind: DebugDrawKind,
    pub color: Vec4,
    pub duration_ms: f32,
}

#[derive(Debug, Clone)]
pub enum DebugDrawKind {
    Box { min: Vec3, max: Vec3 },
    Sphere { center: Vec3, radius: f32 },
    Line { start: Vec3, end: Vec3 },
    Text { pos: Vec3, text: String },
    Arrow { start: Vec3, end: Vec3 },
}

#[derive(Debug)]
pub struct StreamingDebugVisualizer {
    pub overlay: DebugOverlay,
    pub draw_commands: Vec<DebugDrawCommand>,
    pub heatmap_data: HashMap<CellCoord, f32>, // 0-1 heat value
    pub visible_sector_ids: HashSet<u64>,
    pub show_load_distances: bool,
    pub show_memory_usage: bool,
    pub show_cell_grid: bool,
    pub max_commands: usize,
}

impl StreamingDebugVisualizer {
    pub fn new() -> Self {
        Self {
            overlay: DebugOverlay::None,
            draw_commands: Vec::new(),
            heatmap_data: HashMap::new(),
            visible_sector_ids: HashSet::new(),
            show_load_distances: false,
            show_memory_usage: false,
            show_cell_grid: false,
            max_commands: 4096,
        }
    }

    pub fn clear(&mut self) {
        self.draw_commands.clear();
    }

    fn add_command(&mut self, cmd: DebugDrawCommand) {
        if self.draw_commands.len() < self.max_commands {
            self.draw_commands.push(cmd);
        }
    }

    pub fn draw_level_bounds(&mut self, level: &StreamingLevel) {
        let color = match level.state {
            StreamingState::Loaded => Vec4::new(0.0, 1.0, 0.0, 0.5),
            StreamingState::Loading => Vec4::new(1.0, 1.0, 0.0, 0.5),
            StreamingState::Unloading => Vec4::new(1.0, 0.5, 0.0, 0.5),
            StreamingState::Queued => Vec4::new(0.0, 0.5, 1.0, 0.5),
            StreamingState::Unloaded => Vec4::new(0.5, 0.5, 0.5, 0.2),
            StreamingState::Failed => Vec4::new(1.0, 0.0, 0.0, 0.7),
            StreamingState::Evicted => Vec4::new(0.3, 0.0, 0.3, 0.3),
        };
        self.add_command(DebugDrawCommand {
            kind: DebugDrawKind::Box {
                min: level.bounds.min,
                max: level.bounds.max,
            },
            color,
            duration_ms: 0.0,
        });
    }

    pub fn draw_load_distance_sphere(&mut self, level: &StreamingLevel) {
        if !self.show_load_distances { return; }
        self.add_command(DebugDrawCommand {
            kind: DebugDrawKind::Sphere {
                center: level.bounds.center(),
                radius: level.load_distance,
            },
            color: Vec4::new(0.0, 0.8, 0.0, 0.3),
            duration_ms: 0.0,
        });
        self.add_command(DebugDrawCommand {
            kind: DebugDrawKind::Sphere {
                center: level.bounds.center(),
                radius: level.unload_distance,
            },
            color: Vec4::new(1.0, 0.3, 0.0, 0.2),
            duration_ms: 0.0,
        });
    }

    pub fn draw_cell_grid(&mut self, grid: &WorldPartitionGrid, camera_pos: Vec3) {
        if !self.show_cell_grid { return; }
        let center_cell = grid.world_to_cell(camera_pos);
        let r = 5i32;
        for dz in -r..=r {
            for dx in -r..=r {
                let coord = CellCoord::new(center_cell.x + dx, center_cell.y, center_cell.z + dz);
                let min = Vec3::new(
                    grid.origin.x + coord.x as f32 * grid.cell_size,
                    camera_pos.y - 1.0,
                    grid.origin.z + coord.z as f32 * grid.cell_size,
                );
                let max = min + Vec3::new(grid.cell_size, 2.0, grid.cell_size);
                let heat = self.heatmap_data.get(&coord).copied().unwrap_or(0.0);
                let color = Vec4::new(heat, 1.0 - heat, 0.0, 0.3);
                self.add_command(DebugDrawCommand {
                    kind: DebugDrawKind::Box { min, max },
                    color,
                    duration_ms: 0.0,
                });
            }
        }
    }

    pub fn draw_memory_label(&mut self, level: &StreamingLevel) {
        if !self.show_memory_usage { return; }
        let text = format!("{:.1}MB / LOD{:?}", level.memory_footprint_mb, level.current_lod);
        self.add_command(DebugDrawCommand {
            kind: DebugDrawKind::Text {
                pos: level.bounds.center() + Vec3::Y * level.bounds.extents().y,
                text,
            },
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            duration_ms: 0.0,
        });
    }

    pub fn update_heatmap(&mut self, grid: &WorldPartitionGrid) {
        self.heatmap_data.clear();
        let max_mem = grid.cells.values()
            .map(|c| c.memory_used_mb)
            .fold(0.01f32, f32::max);
        for (coord, cell) in &grid.cells {
            let heat = cell.memory_used_mb / max_mem;
            self.heatmap_data.insert(*coord, heat.clamp(0.0, 1.0));
        }
    }

    pub fn draw_sector_portals(&mut self, sector_graph: &SectorGraph) {
        for portal in sector_graph.portals.values() {
            let color = if portal.is_open {
                Vec4::new(0.0, 1.0, 1.0, 0.5)
            } else {
                Vec4::new(1.0, 0.0, 0.0, 0.5)
            };
            self.add_command(DebugDrawCommand {
                kind: DebugDrawKind::Sphere {
                    center: portal.center,
                    radius: 0.5,
                },
                color,
                duration_ms: 0.0,
            });
        }
    }
}

// ============================================================
// PERSISTENT LEVEL STATE
// ============================================================

#[derive(Debug, Clone)]
pub struct LevelSaveState {
    pub level_id: u64,
    pub is_active: bool,
    pub transform: Mat4,
    pub custom_properties: HashMap<String, f32>,
    pub save_timestamp: f64,
    pub loaded_sub_objects: Vec<u64>,
}

impl LevelSaveState {
    pub fn new(level_id: u64) -> Self {
        Self {
            level_id,
            is_active: false,
            transform: Mat4::IDENTITY,
            custom_properties: HashMap::new(),
            save_timestamp: 0.0,
            loaded_sub_objects: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct PersistentLevelManager {
    pub base_level_id: u64,
    pub save_states: HashMap<u64, LevelSaveState>,
    pub transient_level_ids: HashSet<u64>,
    pub dynamic_level_ids: HashSet<u64>,
}

impl PersistentLevelManager {
    pub fn new(base_level_id: u64) -> Self {
        Self {
            base_level_id,
            save_states: HashMap::new(),
            transient_level_ids: HashSet::new(),
            dynamic_level_ids: HashSet::new(),
        }
    }

    pub fn save_level_state(&mut self, level: &StreamingLevel, timestamp: f64) {
        let state = self.save_states.entry(level.id).or_insert_with(|| LevelSaveState::new(level.id));
        state.is_active = level.state == StreamingState::Loaded;
        state.transform = level.transform;
        state.save_timestamp = timestamp;
    }

    pub fn restore_level_state(&self, level_id: u64) -> Option<&LevelSaveState> {
        self.save_states.get(&level_id)
    }

    pub fn mark_transient(&mut self, level_id: u64) {
        self.transient_level_ids.insert(level_id);
        self.dynamic_level_ids.remove(&level_id);
    }

    pub fn mark_dynamic(&mut self, level_id: u64) {
        self.dynamic_level_ids.insert(level_id);
        self.transient_level_ids.remove(&level_id);
    }

    pub fn get_levels_to_restore(&self) -> Vec<u64> {
        self.save_states.iter()
            .filter(|(id, state)| {
                state.is_active && !self.transient_level_ids.contains(id)
            })
            .map(|(&id, _)| id)
            .collect()
    }
}

// ============================================================
// SIMULATION MODE
// ============================================================

#[derive(Debug, Clone)]
pub struct SimulationCamera {
    pub position: Vec3,
    pub direction: Vec3,
    pub speed: f32,
    pub path: Vec<Vec3>,
    pub path_index: usize,
    pub loop_path: bool,
    pub time_accumulated_s: f32,
}

impl SimulationCamera {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            direction: Vec3::NEG_Z,
            speed: 10.0,
            path: Vec::new(),
            path_index: 0,
            loop_path: false,
            time_accumulated_s: 0.0,
        }
    }

    pub fn update(&mut self, dt_s: f32) {
        self.time_accumulated_s += dt_s;
        if self.path.is_empty() { return; }
        let target = self.path[self.path_index];
        let to_target = target - self.position;
        let dist = to_target.length();
        let step = self.speed * dt_s;
        if dist <= step {
            self.position = target;
            self.path_index += 1;
            if self.path_index >= self.path.len() {
                if self.loop_path {
                    self.path_index = 0;
                } else {
                    self.path_index = self.path.len() - 1;
                }
            }
        } else {
            self.direction = to_target / dist;
            self.position = self.position + self.direction * step;
        }
    }

    pub fn add_waypoint(&mut self, pos: Vec3) {
        self.path.push(pos);
    }

    pub fn reset(&mut self) {
        self.path_index = 0;
        self.time_accumulated_s = 0.0;
        if !self.path.is_empty() {
            self.position = self.path[0];
        }
    }
}

#[derive(Debug)]
pub struct StreamingSimulator {
    pub camera: SimulationCamera,
    pub simulated_time_ms: f64,
    pub simulated_frames: u64,
    pub playback_speed: f32,
    pub is_running: bool,
    pub load_events_simulated: u32,
    pub unload_events_simulated: u32,
}

impl StreamingSimulator {
    pub fn new() -> Self {
        Self {
            camera: SimulationCamera::new(Vec3::ZERO),
            simulated_time_ms: 0.0,
            simulated_frames: 0,
            playback_speed: 1.0,
            is_running: false,
            load_events_simulated: 0,
            unload_events_simulated: 0,
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        if !self.is_running { return; }
        let sim_dt = dt_s * self.playback_speed;
        self.camera.update(sim_dt);
        self.simulated_time_ms += sim_dt as f64 * 1000.0;
        self.simulated_frames += 1;
    }

    pub fn start(&mut self) { self.is_running = true; }
    pub fn stop(&mut self) { self.is_running = false; }
    pub fn reset(&mut self) {
        self.simulated_time_ms = 0.0;
        self.simulated_frames = 0;
        self.load_events_simulated = 0;
        self.unload_events_simulated = 0;
        self.camera.reset();
    }
}

// ============================================================
// STREAMING BUDGET TOOL
// ============================================================

#[derive(Debug, Clone)]
pub struct BudgetBreakdown {
    pub total_budget_mb: f32,
    pub used_mb: f32,
    pub available_mb: f32,
    pub loaded_level_count: usize,
    pub loading_level_count: usize,
    pub pending_level_count: usize,
    pub largest_level_name: String,
    pub largest_level_mb: f32,
    pub per_category: HashMap<String, f32>,
}

#[derive(Debug)]
pub struct StreamingBudgetTool {
    pub breakdown: BudgetBreakdown,
    pub history: VecDeque<(f64, f32)>, // (time, used_mb)
    pub peak_usage_mb: f32,
    pub history_capacity: usize,
}

impl StreamingBudgetTool {
    pub fn new(total_budget_mb: f32) -> Self {
        Self {
            breakdown: BudgetBreakdown {
                total_budget_mb,
                used_mb: 0.0,
                available_mb: total_budget_mb,
                loaded_level_count: 0,
                loading_level_count: 0,
                pending_level_count: 0,
                largest_level_name: String::new(),
                largest_level_mb: 0.0,
                per_category: HashMap::new(),
            },
            history: VecDeque::with_capacity(512),
            peak_usage_mb: 0.0,
            history_capacity: 512,
        }
    }

    pub fn update(&mut self, levels: &[StreamingLevel], time_ms: f64) {
        let mut used = 0.0f32;
        let mut loaded = 0;
        let mut loading = 0;
        let mut pending = 0;
        let mut largest_mb = 0.0f32;
        let mut largest_name = String::new();
        let mut per_category: HashMap<String, f32> = HashMap::new();

        for level in levels {
            match level.state {
                StreamingState::Loaded => {
                    loaded += 1;
                    used += level.memory_footprint_mb;
                    let cat = format!("{:?}", level.persistence);
                    *per_category.entry(cat).or_insert(0.0) += level.memory_footprint_mb;
                    if level.memory_footprint_mb > largest_mb {
                        largest_mb = level.memory_footprint_mb;
                        largest_name = level.name.clone();
                    }
                }
                StreamingState::Loading => loading += 1,
                StreamingState::Queued => pending += 1,
                _ => {}
            }
        }

        self.breakdown.used_mb = used;
        self.breakdown.available_mb = self.breakdown.total_budget_mb - used;
        self.breakdown.loaded_level_count = loaded;
        self.breakdown.loading_level_count = loading;
        self.breakdown.pending_level_count = pending;
        self.breakdown.largest_level_name = largest_name;
        self.breakdown.largest_level_mb = largest_mb;
        self.breakdown.per_category = per_category;

        self.peak_usage_mb = self.peak_usage_mb.max(used);

        if self.history.len() >= self.history_capacity {
            self.history.pop_front();
        }
        self.history.push_back((time_ms, used));
    }

    pub fn usage_percent(&self) -> f32 {
        if self.breakdown.total_budget_mb <= 0.0 { return 0.0; }
        (self.breakdown.used_mb / self.breakdown.total_budget_mb * 100.0).clamp(0.0, 100.0)
    }
}

// ============================================================
// MAP OVERVIEW
// ============================================================

#[derive(Debug, Clone)]
pub struct MapThumbnail {
    pub sector_id: u64,
    pub screen_rect: [f32; 4], // x, y, w, h in [0,1]
    pub color: Vec4,
    pub label: String,
    pub is_loaded: bool,
    pub memory_mb: f32,
}

#[derive(Debug)]
pub struct MapOverview {
    pub world_bounds: Aabb,
    pub thumbnails: Vec<MapThumbnail>,
    pub camera_pos_normalized: Vec2,
    pub zoom: f32,
    pub selected_level_id: Option<u64>,
    pub hovered_level_id: Option<u64>,
    pub filter_state: Option<StreamingState>,
    pub show_labels: bool,
    pub show_memory: bool,
}

impl MapOverview {
    pub fn new(world_bounds: Aabb) -> Self {
        Self {
            world_bounds,
            thumbnails: Vec::new(),
            camera_pos_normalized: Vec2::ZERO,
            zoom: 1.0,
            selected_level_id: None,
            hovered_level_id: None,
            filter_state: None,
            show_labels: true,
            show_memory: false,
        }
    }

    pub fn world_to_map_uv(&self, world_pos: Vec3) -> Vec2 {
        let size = self.world_bounds.size();
        let rel = world_pos - self.world_bounds.min;
        Vec2::new(
            rel.x / size.x.max(1.0),
            rel.z / size.z.max(1.0),
        )
    }

    pub fn map_uv_to_world(&self, uv: Vec2) -> Vec3 {
        let size = self.world_bounds.size();
        self.world_bounds.min + Vec3::new(
            uv.x * size.x,
            0.0,
            uv.y * size.z,
        )
    }

    pub fn update_thumbnails(&mut self, levels: &[StreamingLevel]) {
        self.thumbnails.clear();
        let world_size = self.world_bounds.size();
        for level in levels {
            let min_uv = self.world_to_map_uv(level.bounds.min);
            let max_uv = self.world_to_map_uv(level.bounds.max);
            let color = match level.state {
                StreamingState::Loaded => Vec4::new(0.2, 0.8, 0.2, 0.7),
                StreamingState::Loading => Vec4::new(1.0, 1.0, 0.0, 0.7),
                StreamingState::Unloaded => Vec4::new(0.3, 0.3, 0.3, 0.5),
                _ => Vec4::new(0.5, 0.5, 0.5, 0.5),
            };
            self.thumbnails.push(MapThumbnail {
                sector_id: level.id,
                screen_rect: [min_uv.x, min_uv.y, max_uv.x - min_uv.x, max_uv.y - min_uv.y],
                color,
                label: level.name.clone(),
                is_loaded: level.state == StreamingState::Loaded,
                memory_mb: level.memory_footprint_mb,
            });
        }
    }

    pub fn set_camera_position(&mut self, world_pos: Vec3) {
        self.camera_pos_normalized = self.world_to_map_uv(world_pos);
    }

    pub fn levels_at_map_uv(&self, uv: Vec2) -> Vec<u64> {
        self.thumbnails.iter()
            .filter(|t| {
                let r = t.screen_rect;
                uv.x >= r[0] && uv.x <= r[0] + r[2] && uv.y >= r[1] && uv.y <= r[1] + r[3]
            })
            .map(|t| t.sector_id)
            .collect()
    }
}

// ============================================================
// MAIN LEVEL STREAMING EDITOR
// ============================================================

#[derive(Debug)]
pub struct LevelStreamingEditorConfig {
    pub memory_budget_mb: f32,
    pub max_concurrent_loads: usize,
    pub default_load_distance: f32,
    pub default_unload_distance: f32,
    pub default_cell_size: f32,
    pub eviction_policy: EvictionPolicy,
    pub enable_prefetch: bool,
    pub prefetch_lookahead_s: f32,
    pub enable_frustum_culling: bool,
    pub enable_occlusion_culling: bool,
    pub debug_overlay: DebugOverlay,
    pub simulation_playback_speed: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub fov_y_rad: f32,
}

impl Default for LevelStreamingEditorConfig {
    fn default() -> Self {
        Self {
            memory_budget_mb: 1024.0,
            max_concurrent_loads: MAX_CONCURRENT_LOADS,
            default_load_distance: 1000.0,
            default_unload_distance: 1200.0,
            default_cell_size: DEFAULT_CELL_SIZE,
            eviction_policy: EvictionPolicy::Lru,
            enable_prefetch: true,
            prefetch_lookahead_s: PREFETCH_LOOKAHEAD_SECONDS,
            enable_frustum_culling: true,
            enable_occlusion_culling: true,
            debug_overlay: DebugOverlay::None,
            simulation_playback_speed: 1.0,
            viewport_width: 1920,
            viewport_height: 1080,
            fov_y_rad: std::f32::consts::FRAC_PI_4,
        }
    }
}

#[derive(Debug)]
pub struct LevelStreamingEditor {
    // Levels
    pub levels: HashMap<u64, StreamingLevel>,
    pub next_level_id: u64,

    // World partition
    pub world_grid: WorldPartitionGrid,

    // Load queue and memory
    pub load_queue: StreamingLoadQueue,
    pub memory_manager: MemoryPressureManager,

    // Streaming volumes
    pub volumes: HashMap<u64, StreamingVolume>,
    pub next_volume_id: u64,

    // Prefetch
    pub prefetcher: PrefetchPredictor,

    // Dependencies
    pub dependency_graph: DependencyGraph,

    // Sectors
    pub sector_graph: SectorGraph,

    // Persistent state
    pub persistent_manager: PersistentLevelManager,

    // Debug
    pub debug_visualizer: StreamingDebugVisualizer,

    // Timeline
    pub timeline: StreamingTimeline,

    // Budget tool
    pub budget_tool: StreamingBudgetTool,

    // Map overview
    pub map_overview: MapOverview,

    // HZB
    pub hzb: HierarchicalZBuffer,

    // Combined LOD budget
    pub lod_budget: CombinedBudgetManager,

    // Simulation
    pub simulator: StreamingSimulator,

    // Camera state
    pub camera_position: Vec3,
    pub camera_direction: Vec3,
    pub camera_view_proj: Mat4,

    // Config
    pub config: LevelStreamingEditorConfig,

    // Frame tracking
    pub current_frame: u64,
    pub current_time_ms: f64,

    // Statistics
    pub stats: StreamingStats,
}

#[derive(Debug, Default, Clone)]
pub struct StreamingStats {
    pub total_levels: usize,
    pub loaded_levels: usize,
    pub loading_levels: usize,
    pub queued_levels: usize,
    pub unloaded_levels: usize,
    pub frustum_culled_levels: usize,
    pub occlusion_culled_levels: usize,
    pub total_memory_mb: f32,
    pub bandwidth_mb_s: f32,
    pub frame_load_count: u32,
    pub frame_unload_count: u32,
    pub prefetch_hits: u32,
    pub prefetch_misses: u32,
}

impl LevelStreamingEditor {
    pub fn new(config: LevelStreamingEditorConfig) -> Self {
        let budget_mb = config.memory_budget_mb;
        let viewport_w = config.viewport_width as usize;
        let viewport_h = config.viewport_height as usize;

        Self {
            levels: HashMap::new(),
            next_level_id: 1,
            world_grid: WorldPartitionGrid::new(config.default_cell_size, Vec3::ZERO),
            load_queue: StreamingLoadQueue::new(config.max_concurrent_loads),
            memory_manager: MemoryPressureManager::new(budget_mb),
            volumes: HashMap::new(),
            next_volume_id: 1,
            prefetcher: PrefetchPredictor::new(config.prefetch_lookahead_s),
            dependency_graph: DependencyGraph::new(),
            sector_graph: SectorGraph::new(),
            persistent_manager: PersistentLevelManager::new(0),
            debug_visualizer: StreamingDebugVisualizer::new(),
            timeline: StreamingTimeline::new(LEVEL_TIMELINE_CAPACITY),
            budget_tool: StreamingBudgetTool::new(budget_mb),
            map_overview: MapOverview::new(Aabb::new(-Vec3::splat(5000.0), Vec3::splat(5000.0))),
            hzb: HierarchicalZBuffer::new(viewport_w / 4, viewport_h / 4),
            lod_budget: CombinedBudgetManager::new(budget_mb),
            simulator: StreamingSimulator::new(),
            camera_position: Vec3::ZERO,
            camera_direction: Vec3::NEG_Z,
            camera_view_proj: Mat4::IDENTITY,
            config,
            current_frame: 0,
            current_time_ms: 0.0,
            stats: StreamingStats::default(),
        }
    }

    pub fn add_level(&mut self, name: String, asset: StreamingLevelAsset, bounds: Aabb) -> u64 {
        let id = self.next_level_id;
        self.next_level_id += 1;
        let mut level = StreamingLevel::new(id, name, asset, bounds);
        level.load_distance = self.config.default_load_distance;
        level.unload_distance = self.config.default_unload_distance;
        self.dependency_graph.add_level(id);

        // Register in world grid
        let cell = self.world_grid.world_to_cell(bounds.center());
        {
            let c = self.world_grid.get_or_create_cell(cell);
            c.level_ids.push(id);
        }

        self.levels.insert(id, level);
        id
    }

    pub fn remove_level(&mut self, id: u64) {
        if let Some(level) = self.levels.remove(&id) {
            // Remove from grid
            let cell = self.world_grid.world_to_cell(level.bounds.center());
            if let Some(c) = self.world_grid.cells.get_mut(&cell) {
                c.level_ids.retain(|&lid| lid != id);
            }
            // Remove from dependency graph
            self.dependency_graph.nodes.remove(&id);
        }
    }

    pub fn add_streaming_volume_sphere(&mut self, name: String, center: Vec3, load_r: f32, unload_r: f32) -> u64 {
        let id = self.next_volume_id;
        self.next_volume_id += 1;
        let vol = StreamingVolume::new_sphere(id, name, center, load_r, unload_r);
        self.volumes.insert(id, vol);
        id
    }

    pub fn add_streaming_volume_box(&mut self, name: String, transform: Mat4, half_extents: Vec3) -> u64 {
        let id = self.next_volume_id;
        self.next_volume_id += 1;
        let vol = StreamingVolume::new_box(id, name, transform, half_extents);
        self.volumes.insert(id, vol);
        id
    }

    pub fn update_camera(&mut self, position: Vec3, direction: Vec3, view_proj: Mat4) {
        self.camera_position = position;
        self.camera_direction = direction;
        self.camera_view_proj = view_proj;
        self.prefetcher.update(position, self.current_time_ms);
        self.map_overview.set_camera_position(position);
    }

    /// Main per-frame update
    pub fn tick(&mut self, dt_s: f32) {
        self.current_frame += 1;
        self.current_time_ms += dt_s as f64 * 1000.0;

        // Update simulator if running
        self.simulator.tick(dt_s);
        let cam_pos = if self.simulator.is_running {
            self.simulator.camera.position
        } else {
            self.camera_position
        };

        self.stats.frame_load_count = 0;
        self.stats.frame_unload_count = 0;

        self.update_distance_and_culling(cam_pos);
        self.process_streaming_volumes(cam_pos);
        self.update_load_unload_decisions(cam_pos);
        self.process_prefetch(cam_pos);
        self.process_load_queue();
        self.manage_memory_pressure(cam_pos);
        self.update_lod_budget(cam_pos);
        self.update_debug_visualization();
        self.budget_tool.update(&self.levels.values().cloned().collect::<Vec<_>>(), self.current_time_ms);
        self.map_overview.update_thumbnails(&self.levels.values().cloned().collect::<Vec<_>>());

        self.collect_stats();

        self.timeline.record_memory(self.current_time_ms, self.memory_manager.used_mb);
        self.timeline.record_bandwidth(
            self.current_time_ms,
            self.load_queue.estimated_bandwidth_mb_s(),
        );
    }

    fn update_distance_and_culling(&mut self, cam_pos: Vec3) {
        let frustum = Frustum::from_view_proj(self.camera_view_proj);
        let vp_h = self.config.viewport_height as f32;
        let fov = self.config.fov_y_rad;
        let mut frustum_culled = 0;
        let mut occlusion_culled = 0;

        let level_ids: Vec<u64> = self.levels.keys().copied().collect();
        for id in level_ids {
            if let Some(level) = self.levels.get_mut(&id) {
                let dist = level.bounds.distance_to_point(cam_pos);
                level.distance_to_camera = dist;
                level.screen_size = level.compute_screen_size(cam_pos, fov, vp_h);
                level.current_lod = level.compute_lod(dist, level.lod_bias);

                // Frustum cull
                if self.config.enable_frustum_culling {
                    level.is_frustum_culled = !frustum.test_aabb(&level.bounds);
                    if level.is_frustum_culled { frustum_culled += 1; }
                } else {
                    level.is_frustum_culled = false;
                }

                // Occlusion cull (only if loaded and visible)
                if self.config.enable_occlusion_culling && !level.is_frustum_culled {
                    level.is_occlusion_culled = !self.hzb.test_aabb_visibility(&level.bounds, &self.camera_view_proj);
                    if level.is_occlusion_culled { occlusion_culled += 1; }
                } else {
                    level.is_occlusion_culled = false;
                }

                level.is_visible = !level.is_frustum_culled && !level.is_occlusion_culled;
            }
        }

        self.stats.frustum_culled_levels = frustum_culled;
        self.stats.occlusion_culled_levels = occlusion_culled;
    }

    fn process_streaming_volumes(&mut self, cam_pos: Vec3) {
        let volume_ids: Vec<u64> = self.volumes.keys().copied().collect();
        for vid in volume_ids {
            if let Some(vol) = self.volumes.get(&vid) {
                if !vol.is_enabled { continue; }
                let should_load = vol.should_trigger_load(cam_pos);
                let target_ids = vol.target_level_ids.clone();
                let importance = vol.importance_weight;
                for lid in target_ids {
                    if let Some(level) = self.levels.get_mut(&lid) {
                        if should_load && level.state == StreamingState::Unloaded {
                            level.importance_weight = level.importance_weight.max(importance);
                        }
                    }
                }
            }
        }
    }

    fn update_load_unload_decisions(&mut self, cam_pos: Vec3) {
        let level_ids: Vec<u64> = self.levels.keys().copied().collect();
        for id in level_ids {
            let (should_load, should_unload, dist, importance, name) = {
                if let Some(level) = self.levels.get(&id) {
                    let sl = level.should_load(cam_pos);
                    let su = level.should_unload(cam_pos);
                    (sl, su, level.distance_to_camera, level.importance_weight, level.name.clone())
                } else { continue }
            };

            if let Some(level) = self.levels.get_mut(&id) {
                match level.state {
                    StreamingState::Unloaded | StreamingState::Evicted => {
                        if should_load {
                            let mem_est = level.memory_estimate_mb();
                            if self.memory_manager.can_load(mem_est) || level.persistence == LevelPersistence::AlwaysLoaded {
                                level.state = StreamingState::Queued;
                                let dist_weight = 1.0 / (dist + 1.0) * importance;
                                self.load_queue.enqueue(LoadRequest {
                                    level_id: id,
                                    priority: level.priority,
                                    distance_weight: dist_weight,
                                    enqueue_time_ms: self.current_time_ms,
                                    predicted_load_time_ms: level.asset.load_time_estimate_ms,
                                    is_prefetch: false,
                                });
                            }
                        }
                    }
                    StreamingState::Loaded => {
                        if should_unload {
                            level.state = StreamingState::Unloading;
                            self.stats.frame_unload_count += 1;
                        }
                    }
                    StreamingState::Unloading => {
                        // Simulate unload completion (in real engine: async)
                        let size = level.memory_footprint_mb;
                        self.memory_manager.record_unload(id, size);
                        level.state = StreamingState::Unloaded;
                        level.unload_timestamp_ms = self.current_time_ms;
                        let event = StreamingEvent {
                            timestamp_ms: self.current_time_ms,
                            kind: StreamingEventKind::LevelUnloadCompleted,
                            level_id: id,
                            level_name: name.clone(),
                            data_mb: size,
                            duration_ms: 16.0, // simulated
                            camera_pos: cam_pos,
                        };
                        self.timeline.record(event);
                    }
                    _ => {}
                }
            }
        }
    }

    fn process_prefetch(&mut self, cam_pos: Vec3) {
        if !self.config.enable_prefetch { return; }
        let levels_vec: Vec<StreamingLevel> = self.levels.values().cloned().collect();
        let candidates = self.prefetcher.get_prefetch_candidates(&levels_vec, cam_pos);
        for lid in candidates {
            if let Some(level) = self.levels.get_mut(&lid) {
                if level.state == StreamingState::Unloaded {
                    level.state = StreamingState::Queued;
                    self.load_queue.enqueue(LoadRequest {
                        level_id: lid,
                        priority: LoadPriority::Prefetch,
                        distance_weight: 0.1,
                        enqueue_time_ms: self.current_time_ms,
                        predicted_load_time_ms: level.asset.load_time_estimate_ms,
                        is_prefetch: true,
                    });
                }
            }
        }
    }

    fn process_load_queue(&mut self) {
        while let Some(request) = self.load_queue.dequeue_next() {
            if let Some(level) = self.levels.get_mut(&request.level_id) {
                if level.state == StreamingState::Queued || level.state == StreamingState::Unloaded {
                    level.state = StreamingState::Loading;
                    self.stats.frame_load_count += 1;
                }
            }
        }

        // Simulate load completion for all in-flight levels
        let in_flight_ids: Vec<u64> = self.load_queue.in_flight.iter().map(|r| r.level_id).collect();
        for lid in in_flight_ids {
            // Simulated instant completion (real engine: check async result)
            let (size_bytes, name, time_ms) = {
                if let Some(level) = self.levels.get(&lid) {
                    (level.asset.size_bytes, level.name.clone(), level.asset.load_time_estimate_ms)
                } else { continue }
            };

            self.load_queue.complete_load(lid, size_bytes, time_ms);
            let size_mb = size_bytes as f32 / (1024.0 * 1024.0);
            self.memory_manager.record_load(lid, size_mb);

            if let Some(level) = self.levels.get_mut(&lid) {
                level.state = StreamingState::Loaded;
                level.memory_footprint_mb = size_mb;
                level.load_timestamp_ms = self.current_time_ms;
                level.load_count += 1;
            }

            let event = StreamingEvent {
                timestamp_ms: self.current_time_ms,
                kind: StreamingEventKind::LevelLoadCompleted,
                level_id: lid,
                level_name: name,
                data_mb: size_mb,
                duration_ms: time_ms,
                camera_pos: self.camera_position,
            };
            self.timeline.record(event);
        }
    }

    fn manage_memory_pressure(&mut self, cam_pos: Vec3) {
        if !self.memory_manager.is_under_pressure() { return; }

        let needed = self.memory_manager.used_mb - self.memory_manager.budget_mb * self.memory_manager.pressure_threshold;
        let levels_vec: Vec<StreamingLevel> = self.levels.values().cloned().collect();
        let candidates = self.memory_manager.select_eviction_candidates(needed, &levels_vec);

        for lid in candidates {
            if let Some(level) = self.levels.get_mut(&lid) {
                if level.state == StreamingState::Loaded {
                    let size = level.memory_footprint_mb;
                    self.memory_manager.record_unload(lid, size);
                    level.state = StreamingState::Evicted;
                }
            }
        }
    }

    fn update_lod_budget(&mut self, cam_pos: Vec3) {
        let level_ids: Vec<u64> = self.levels.keys().copied().collect();
        for id in level_ids {
            if let Some(level) = self.levels.get(&id) {
                let lod_mem = level.memory_estimate_mb();
                let stream_mem = if level.state == StreamingState::Loaded { 5.0 } else { 0.0 };
                self.lod_budget.update_entry(id, level.current_lod, lod_mem, stream_mem);
            }
        }
    }

    fn update_debug_visualization(&mut self) {
        self.debug_visualizer.clear();
        match self.config.debug_overlay {
            DebugOverlay::None => {}
            DebugOverlay::StreamingState => {
                for level in self.levels.values() {
                    self.debug_visualizer.draw_level_bounds(level);
                }
            }
            DebugOverlay::MemoryUsage => {
                for level in self.levels.values() {
                    self.debug_visualizer.draw_level_bounds(level);
                    self.debug_visualizer.draw_memory_label(level);
                }
            }
            DebugOverlay::LoadDistance => {
                for level in self.levels.values() {
                    self.debug_visualizer.draw_load_distance_sphere(level);
                }
            }
            DebugOverlay::CellGrid => {
                self.debug_visualizer.update_heatmap(&self.world_grid);
                self.debug_visualizer.draw_cell_grid(&self.world_grid, self.camera_position);
            }
            DebugOverlay::PriorityHeatmap => {
                self.debug_visualizer.update_heatmap(&self.world_grid);
                for level in self.levels.values() {
                    self.debug_visualizer.draw_level_bounds(level);
                }
            }
            _ => {}
        }
    }

    fn collect_stats(&mut self) {
        let mut total = 0;
        let mut loaded = 0;
        let mut loading = 0;
        let mut queued = 0;
        let mut unloaded = 0;
        let mut memory = 0.0f32;

        for level in self.levels.values() {
            total += 1;
            memory += level.memory_footprint_mb;
            match level.state {
                StreamingState::Loaded => loaded += 1,
                StreamingState::Loading => loading += 1,
                StreamingState::Queued => queued += 1,
                StreamingState::Unloaded | StreamingState::Evicted => unloaded += 1,
                _ => {}
            }
        }

        self.stats.total_levels = total;
        self.stats.loaded_levels = loaded;
        self.stats.loading_levels = loading;
        self.stats.queued_levels = queued;
        self.stats.unloaded_levels = unloaded;
        self.stats.total_memory_mb = memory;
        self.stats.bandwidth_mb_s = self.load_queue.estimated_bandwidth_mb_s();
    }

    pub fn set_debug_overlay(&mut self, overlay: DebugOverlay) {
        self.config.debug_overlay = overlay;
        self.debug_visualizer.overlay = overlay;
    }

    pub fn get_level(&self, id: u64) -> Option<&StreamingLevel> {
        self.levels.get(&id)
    }

    pub fn get_level_mut(&mut self, id: u64) -> Option<&mut StreamingLevel> {
        self.levels.get_mut(&id)
    }

    pub fn force_load_level(&mut self, id: u64) {
        if let Some(level) = self.levels.get_mut(&id) {
            level.state = StreamingState::Queued;
            level.priority = LoadPriority::Critical;
            let lid = level.id;
            let time_ms = level.asset.load_time_estimate_ms;
            self.load_queue.enqueue(LoadRequest {
                level_id: lid,
                priority: LoadPriority::Critical,
                distance_weight: 1000.0,
                enqueue_time_ms: self.current_time_ms,
                predicted_load_time_ms: time_ms,
                is_prefetch: false,
            });
        }
    }

    pub fn force_unload_level(&mut self, id: u64) {
        if let Some(level) = self.levels.get_mut(&id) {
            if level.persistence != LevelPersistence::AlwaysLoaded {
                level.state = StreamingState::Unloading;
            }
        }
    }

    pub fn set_memory_budget(&mut self, budget_mb: f32) {
        self.config.memory_budget_mb = budget_mb;
        self.memory_manager.budget_mb = budget_mb;
        self.budget_tool.breakdown.total_budget_mb = budget_mb;
        self.lod_budget.total_budget_mb = budget_mb;
    }

    pub fn add_level_dependency(&mut self, from: u64, to: u64, edge: DependencyEdgeType) {
        self.dependency_graph.add_dependency(from, to, edge);
    }

    pub fn check_for_circular_dependencies(&self) -> Option<Vec<u64>> {
        self.dependency_graph.detect_cycles()
    }

    pub fn get_load_plan(&self) -> Vec<Vec<u64>> {
        self.dependency_graph.parallel_load_plan()
    }

    pub fn start_simulation(&mut self) {
        self.simulator.start();
    }

    pub fn stop_simulation(&mut self) {
        self.simulator.stop();
    }

    pub fn reset_simulation(&mut self) {
        self.simulator.reset();
        // Reset all levels to unloaded
        for level in self.levels.values_mut() {
            if level.persistence != LevelPersistence::AlwaysLoaded {
                level.state = StreamingState::Unloaded;
                level.memory_footprint_mb = 0.0;
            }
        }
        self.memory_manager.used_mb = 0.0;
        self.memory_manager.lru_order.clear();
        self.load_queue.pending.clear();
        self.load_queue.in_flight.clear();
    }

    pub fn get_world_bounds(&self) -> Aabb {
        let mut result = Aabb::new(Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for level in self.levels.values() {
            result = result.merge(&level.bounds);
        }
        if result.min.x > result.max.x {
            Aabb::new(Vec3::ZERO, Vec3::ZERO)
        } else {
            result
        }
    }

    pub fn cells_in_camera_radius(&self, radius: f32) -> Vec<CellCoord> {
        self.world_grid.get_cells_in_radius(self.camera_position, radius)
    }

    pub fn query_levels_near(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        self.levels.values()
            .filter(|l| l.bounds.distance_to_point(pos) <= radius)
            .map(|l| l.id)
            .collect()
    }

    pub fn get_streaming_report(&self) -> StreamingReport {
        StreamingReport {
            total_levels: self.stats.total_levels,
            loaded_count: self.stats.loaded_levels,
            loading_count: self.stats.loading_levels,
            queued_count: self.stats.queued_levels,
            total_memory_mb: self.stats.total_memory_mb,
            memory_budget_mb: self.config.memory_budget_mb,
            bandwidth_mb_s: self.stats.bandwidth_mb_s,
            estimated_queue_time_ms: self.load_queue.estimated_remaining_time_ms(),
            has_circular_deps: self.dependency_graph.detect_cycles().is_some(),
            memory_pressure_ratio: self.memory_manager.pressure_ratio(),
            current_sector: self.sector_graph.current_sector,
            global_lod_bias: self.lod_budget.compute_global_lod_bias(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamingReport {
    pub total_levels: usize,
    pub loaded_count: usize,
    pub loading_count: usize,
    pub queued_count: usize,
    pub total_memory_mb: f32,
    pub memory_budget_mb: f32,
    pub bandwidth_mb_s: f32,
    pub estimated_queue_time_ms: f32,
    pub has_circular_deps: bool,
    pub memory_pressure_ratio: f32,
    pub current_sector: Option<u64>,
    pub global_lod_bias: f32,
}

// ============================================================
// ADVANCED STREAMING FEATURES
// ============================================================

/// Bandwidth estimator using exponential moving average
#[derive(Debug)]
pub struct BandwidthEstimator {
    pub ema: f32,
    pub alpha: f32, // EMA smoothing factor
    pub peak_mb_s: f32,
    pub min_mb_s: f32,
    pub sample_count: u64,
}

impl BandwidthEstimator {
    pub fn new(initial_estimate_mb_s: f32) -> Self {
        Self {
            ema: initial_estimate_mb_s,
            alpha: 0.1,
            peak_mb_s: initial_estimate_mb_s,
            min_mb_s: initial_estimate_mb_s,
            sample_count: 0,
        }
    }

    pub fn add_sample(&mut self, mb_per_s: f32) {
        self.ema = self.alpha * mb_per_s + (1.0 - self.alpha) * self.ema;
        self.peak_mb_s = self.peak_mb_s.max(mb_per_s);
        self.min_mb_s = if self.sample_count == 0 { mb_per_s } else { self.min_mb_s.min(mb_per_s) };
        self.sample_count += 1;
    }

    pub fn estimate(&self) -> f32 { self.ema }

    pub fn estimated_load_time_ms(&self, size_mb: f32) -> f32 {
        if self.ema <= 0.0 { return f32::MAX; }
        size_mb / self.ema * 1000.0
    }

    pub fn variance_adjusted_estimate(&self, size_mb: f32, confidence: f32) -> f32 {
        // Use pessimistic estimate for high confidence requirements
        let rate = if confidence > 0.9 {
            self.min_mb_s.max(self.ema * 0.5)
        } else {
            self.ema
        };
        if rate <= 0.0 { return f32::MAX; }
        size_mb / rate * 1000.0
    }
}

/// Occlusion query result cache
#[derive(Debug)]
pub struct OcclusionCache {
    pub results: HashMap<u64, (bool, u64)>, // level_id -> (is_visible, frame)
    pub cache_lifetime_frames: u64,
}

impl OcclusionCache {
    pub fn new(lifetime_frames: u64) -> Self {
        Self {
            results: HashMap::new(),
            cache_lifetime_frames: lifetime_frames,
        }
    }

    pub fn get(&self, level_id: u64, current_frame: u64) -> Option<bool> {
        if let Some(&(visible, frame)) = self.results.get(&level_id) {
            if current_frame - frame <= self.cache_lifetime_frames {
                return Some(visible);
            }
        }
        None
    }

    pub fn set(&mut self, level_id: u64, visible: bool, frame: u64) {
        self.results.insert(level_id, (visible, frame));
    }

    pub fn evict_stale(&mut self, current_frame: u64) {
        self.results.retain(|_, (_, frame)| {
            current_frame - *frame <= self.cache_lifetime_frames
        });
    }
}

/// World streaming bandwidth tracker
#[derive(Debug)]
pub struct WorldStreamingBandwidthTracker {
    pub estimator: BandwidthEstimator,
    pub frame_data: VecDeque<(u64, f32)>, // (frame, MB this frame)
    pub total_mb_streamed: f32,
    pub peak_frame_mb: f32,
}

impl WorldStreamingBandwidthTracker {
    pub fn new() -> Self {
        Self {
            estimator: BandwidthEstimator::new(50.0),
            frame_data: VecDeque::with_capacity(256),
            total_mb_streamed: 0.0,
            peak_frame_mb: 0.0,
        }
    }

    pub fn record_frame(&mut self, frame: u64, mb_this_frame: f32, dt_s: f32) {
        if self.frame_data.len() >= 256 { self.frame_data.pop_front(); }
        self.frame_data.push_back((frame, mb_this_frame));
        self.total_mb_streamed += mb_this_frame;
        self.peak_frame_mb = self.peak_frame_mb.max(mb_this_frame);
        if dt_s > 1e-6 {
            self.estimator.add_sample(mb_this_frame / dt_s);
        }
    }

    pub fn average_mb_per_frame(&self) -> f32 {
        if self.frame_data.is_empty() { return 0.0; }
        let total: f32 = self.frame_data.iter().map(|(_, mb)| mb).sum();
        total / self.frame_data.len() as f32
    }
}

/// Level streaming profiler
#[derive(Debug)]
pub struct LevelStreamingProfiler {
    pub frame_timings: VecDeque<f32>,        // ms per frame for streaming logic
    pub load_timings: HashMap<u64, f32>,     // level_id -> last load time ms
    pub queue_depth_history: VecDeque<usize>,
    pub memory_history_full: VecDeque<f32>,
    pub bandwidth_tracker: WorldStreamingBandwidthTracker,
    pub total_frames_profiled: u64,
}

impl LevelStreamingProfiler {
    pub fn new() -> Self {
        Self {
            frame_timings: VecDeque::with_capacity(256),
            load_timings: HashMap::new(),
            queue_depth_history: VecDeque::with_capacity(256),
            memory_history_full: VecDeque::with_capacity(256),
            bandwidth_tracker: WorldStreamingBandwidthTracker::new(),
            total_frames_profiled: 0,
        }
    }

    pub fn record_frame(&mut self, timing_ms: f32, queue_depth: usize, memory_mb: f32, streamed_mb: f32, dt_s: f32) {
        if self.frame_timings.len() >= 256 { self.frame_timings.pop_front(); }
        if self.queue_depth_history.len() >= 256 { self.queue_depth_history.pop_front(); }
        if self.memory_history_full.len() >= 256 { self.memory_history_full.pop_front(); }

        self.frame_timings.push_back(timing_ms);
        self.queue_depth_history.push_back(queue_depth);
        self.memory_history_full.push_back(memory_mb);
        self.bandwidth_tracker.record_frame(self.total_frames_profiled, streamed_mb, dt_s);
        self.total_frames_profiled += 1;
    }

    pub fn average_frame_time_ms(&self) -> f32 {
        if self.frame_timings.is_empty() { return 0.0; }
        self.frame_timings.iter().sum::<f32>() / self.frame_timings.len() as f32
    }

    pub fn p99_frame_time_ms(&self) -> f32 {
        let mut sorted: Vec<f32> = self.frame_timings.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (sorted.len() as f32 * 0.99) as usize;
        sorted.get(idx).copied().unwrap_or(0.0)
    }

    pub fn max_queue_depth(&self) -> usize {
        self.queue_depth_history.iter().copied().max().unwrap_or(0)
    }

    pub fn peak_memory_mb(&self) -> f32 {
        self.memory_history_full.iter().copied().fold(0.0f32, f32::max)
    }
}

// ============================================================
// LEVEL INSTANCE MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct LevelInstance {
    pub id: u64,
    pub level_id: u64,
    pub transform: Mat4,
    pub is_active: bool,
    pub override_load_distance: Option<f32>,
    pub override_priority: Option<LoadPriority>,
}

impl LevelInstance {
    pub fn new(id: u64, level_id: u64, transform: Mat4) -> Self {
        Self {
            id,
            level_id,
            transform,
            is_active: true,
            override_load_distance: None,
            override_priority: None,
        }
    }

    pub fn world_position(&self) -> Vec3 {
        self.transform.transform_point3(Vec3::ZERO)
    }
}

/// Manages multiple instances of the same level template
#[derive(Debug)]
pub struct LevelInstanceManager {
    pub instances: HashMap<u64, LevelInstance>,
    pub next_instance_id: u64,
    pub instance_to_base: HashMap<u64, u64>, // instance_id -> level_id
    pub base_to_instances: HashMap<u64, Vec<u64>>, // level_id -> [instance_ids]
}

impl LevelInstanceManager {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            next_instance_id: 1,
            instance_to_base: HashMap::new(),
            base_to_instances: HashMap::new(),
        }
    }

    pub fn instantiate(&mut self, level_id: u64, transform: Mat4) -> u64 {
        let id = self.next_instance_id;
        self.next_instance_id += 1;
        self.instances.insert(id, LevelInstance::new(id, level_id, transform));
        self.instance_to_base.insert(id, level_id);
        self.base_to_instances.entry(level_id).or_default().push(id);
        id
    }

    pub fn remove_instance(&mut self, instance_id: u64) {
        if let Some(inst) = self.instances.remove(&instance_id) {
            self.instance_to_base.remove(&instance_id);
            if let Some(list) = self.base_to_instances.get_mut(&inst.level_id) {
                list.retain(|&id| id != instance_id);
            }
        }
    }

    pub fn get_instances_for_level(&self, level_id: u64) -> Vec<&LevelInstance> {
        self.base_to_instances.get(&level_id)
            .map(|ids| ids.iter().filter_map(|id| self.instances.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn instances_near(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        self.instances.values()
            .filter(|inst| {
                let wp = inst.world_position();
                (wp - pos).length() <= radius
            })
            .map(|inst| inst.id)
            .collect()
    }

    pub fn active_instance_count(&self) -> usize {
        self.instances.values().filter(|i| i.is_active).count()
    }
}

// ============================================================
// STREAMING EDITOR UI STATE
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorPanel {
    MapOverview,
    LevelList,
    BudgetTool,
    Timeline,
    Settings,
    DependencyGraph,
    SectorEditor,
    SimulationControl,
    Profiler,
}

#[derive(Debug)]
pub struct LevelStreamingEditorUiState {
    pub active_panel: EditorPanel,
    pub selected_level_ids: HashSet<u64>,
    pub search_filter: String,
    pub state_filter: Option<StreamingState>,
    pub sort_by_distance: bool,
    pub sort_by_memory: bool,
    pub show_only_visible: bool,
    pub timeline_scroll_x: f32,
    pub timeline_zoom: f32,
    pub map_scroll: Vec2,
    pub map_zoom: f32,
    pub is_editing_volume: bool,
    pub editing_volume_id: Option<u64>,
    pub simulation_panel_open: bool,
    pub budget_panel_open: bool,
    pub dep_graph_panel_open: bool,
}

impl LevelStreamingEditorUiState {
    pub fn new() -> Self {
        Self {
            active_panel: EditorPanel::MapOverview,
            selected_level_ids: HashSet::new(),
            search_filter: String::new(),
            state_filter: None,
            sort_by_distance: false,
            sort_by_memory: false,
            show_only_visible: false,
            timeline_scroll_x: 0.0,
            timeline_zoom: 1.0,
            map_scroll: Vec2::ZERO,
            map_zoom: 1.0,
            is_editing_volume: false,
            editing_volume_id: None,
            simulation_panel_open: false,
            budget_panel_open: false,
            dep_graph_panel_open: false,
        }
    }

    pub fn select_level(&mut self, id: u64, multi_select: bool) {
        if !multi_select {
            self.selected_level_ids.clear();
        }
        self.selected_level_ids.insert(id);
    }

    pub fn deselect_level(&mut self, id: u64) {
        self.selected_level_ids.remove(&id);
    }

    pub fn is_level_selected(&self, id: u64) -> bool {
        self.selected_level_ids.contains(&id)
    }

    pub fn filtered_levels<'a>(&'a self, levels: &'a [StreamingLevel]) -> Vec<&'a StreamingLevel> {
        let mut result: Vec<&StreamingLevel> = levels.iter()
            .filter(|l| {
                let name_match = self.search_filter.is_empty()
                    || l.name.to_lowercase().contains(&self.search_filter.to_lowercase());
                let state_match = self.state_filter.map_or(true, |s| l.state == s);
                let visibility_match = !self.show_only_visible || l.is_visible;
                name_match && state_match && visibility_match
            })
            .collect();

        if self.sort_by_distance {
            result.sort_by(|a, b| a.distance_to_camera.partial_cmp(&b.distance_to_camera)
                .unwrap_or(std::cmp::Ordering::Equal));
        } else if self.sort_by_memory {
            result.sort_by(|a, b| b.memory_footprint_mb.partial_cmp(&a.memory_footprint_mb)
                .unwrap_or(std::cmp::Ordering::Equal));
        }

        result
    }
}

// ============================================================
// GRID SPATIAL HASH (ADVANCED)
// ============================================================

#[derive(Debug)]
pub struct SpatialHashGrid {
    pub bucket_size: f32,
    pub buckets: HashMap<(i32, i32, i32), Vec<u64>>,
}

impl SpatialHashGrid {
    pub fn new(bucket_size: f32) -> Self {
        Self { bucket_size, buckets: HashMap::new() }
    }

    fn hash_pos(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.bucket_size).floor() as i32,
            (pos.y / self.bucket_size).floor() as i32,
            (pos.z / self.bucket_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, id: u64, pos: Vec3) {
        let h = self.hash_pos(pos);
        self.buckets.entry(h).or_default().push(id);
    }

    pub fn remove(&mut self, id: u64, pos: Vec3) {
        let h = self.hash_pos(pos);
        if let Some(v) = self.buckets.get_mut(&h) {
            v.retain(|&i| i != id);
        }
    }

    pub fn query_radius(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        let r = (radius / self.bucket_size).ceil() as i32 + 1;
        let h = self.hash_pos(pos);
        let mut result = Vec::new();
        for dz in -r..=r {
            for dy in -r..=r {
                for dx in -r..=r {
                    let key = (h.0 + dx, h.1 + dy, h.2 + dz);
                    if let Some(v) = self.buckets.get(&key) {
                        result.extend_from_slice(v);
                    }
                }
            }
        }
        result
    }

    pub fn clear(&mut self) {
        self.buckets.clear();
    }

    pub fn total_entries(&self) -> usize {
        self.buckets.values().map(|v| v.len()).sum()
    }
}

// ============================================================
// LEVEL TRANSITION CONTROLLER
// ============================================================

#[derive(Debug, Clone)]
pub struct LevelTransition {
    pub from_level_id: u64,
    pub to_level_id: u64,
    pub transition_type: SectorTransitionType,
    pub progress: f32,   // 0..1
    pub duration_s: f32,
    pub is_complete: bool,
}

impl LevelTransition {
    pub fn new(from: u64, to: u64, transition_type: SectorTransitionType, duration_s: f32) -> Self {
        Self {
            from_level_id: from,
            to_level_id: to,
            transition_type,
            progress: 0.0,
            duration_s,
            is_complete: false,
        }
    }

    pub fn update(&mut self, dt_s: f32) {
        if self.is_complete { return; }
        self.progress += dt_s / self.duration_s.max(0.001);
        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.is_complete = true;
        }
    }

    pub fn fade_alpha(&self) -> f32 {
        match self.transition_type {
            SectorTransitionType::Fade => {
                if self.progress < 0.5 {
                    self.progress * 2.0
                } else {
                    (1.0 - self.progress) * 2.0
                }
            }
            SectorTransitionType::Immediate => 0.0,
            _ => 1.0 - self.progress,
        }
    }

    pub fn smoothed_progress(&self) -> f32 {
        // Smoothstep
        let t = self.progress;
        t * t * (3.0 - 2.0 * t)
    }
}

#[derive(Debug)]
pub struct TransitionController {
    pub active_transitions: Vec<LevelTransition>,
    pub completed_transitions: VecDeque<LevelTransition>,
}

impl TransitionController {
    pub fn new() -> Self {
        Self {
            active_transitions: Vec::new(),
            completed_transitions: VecDeque::new(),
        }
    }

    pub fn begin_transition(&mut self, from: u64, to: u64, t: SectorTransitionType, duration_s: f32) {
        self.active_transitions.push(LevelTransition::new(from, to, t, duration_s));
    }

    pub fn update(&mut self, dt_s: f32) {
        let mut to_complete = Vec::new();
        for (i, t) in self.active_transitions.iter_mut().enumerate() {
            t.update(dt_s);
            if t.is_complete { to_complete.push(i); }
        }
        for i in to_complete.into_iter().rev() {
            let t = self.active_transitions.remove(i);
            if self.completed_transitions.len() >= 64 { self.completed_transitions.pop_front(); }
            self.completed_transitions.push_back(t);
        }
    }

    pub fn is_transitioning(&self) -> bool {
        !self.active_transitions.is_empty()
    }

    pub fn get_fade_alpha(&self, level_id: u64) -> f32 {
        let mut alpha = 1.0f32;
        for t in &self.active_transitions {
            if t.from_level_id == level_id || t.to_level_id == level_id {
                alpha = alpha.min(1.0 - t.fade_alpha());
            }
        }
        alpha
    }
}

// ============================================================
// CULLING MANAGER
// ============================================================

#[derive(Debug)]
pub struct CullingManager {
    pub frustum: Frustum,
    pub hzb: HierarchicalZBuffer,
    pub occlusion_cache: OcclusionCache,
    pub total_tested: u64,
    pub total_culled_frustum: u64,
    pub total_culled_occlusion: u64,
    pub total_passed: u64,
}

impl CullingManager {
    pub fn new(vp_width: usize, vp_height: usize) -> Self {
        Self {
            frustum: Frustum::from_view_proj(Mat4::IDENTITY),
            hzb: HierarchicalZBuffer::new(vp_width / 4, vp_height / 4),
            occlusion_cache: OcclusionCache::new(4),
            total_tested: 0,
            total_culled_frustum: 0,
            total_culled_occlusion: 0,
            total_passed: 0,
        }
    }

    pub fn update_frustum(&mut self, view_proj: Mat4) {
        self.frustum = Frustum::from_view_proj(view_proj);
    }

    pub fn update_hzb(&mut self, depth_buffer: &[f32]) {
        self.hzb.build_from_depth(depth_buffer);
    }

    pub fn test_level(&mut self, level: &StreamingLevel, view_proj: &Mat4, frame: u64) -> bool {
        self.total_tested += 1;

        // Frustum test
        if !self.frustum.test_aabb(&level.bounds) {
            self.total_culled_frustum += 1;
            return false;
        }

        // Occlusion cache check
        if let Some(cached_visible) = self.occlusion_cache.get(level.id, frame) {
            if !cached_visible {
                self.total_culled_occlusion += 1;
                return false;
            }
            self.total_passed += 1;
            return true;
        }

        // HZB test
        let hzb_visible = self.hzb.test_aabb_visibility(&level.bounds, view_proj);
        self.occlusion_cache.set(level.id, hzb_visible, frame);

        if !hzb_visible {
            self.total_culled_occlusion += 1;
            return false;
        }

        self.total_passed += 1;
        true
    }

    pub fn cull_efficiency(&self) -> f32 {
        if self.total_tested == 0 { return 0.0; }
        (self.total_culled_frustum + self.total_culled_occlusion) as f32 / self.total_tested as f32
    }

    pub fn reset_stats(&mut self) {
        self.total_tested = 0;
        self.total_culled_frustum = 0;
        self.total_culled_occlusion = 0;
        self.total_passed = 0;
    }
}

// ============================================================
// STREAMING LEVEL EDITOR (EXTENDED)
// ============================================================

/// Extended version of the editor with all subsystems
#[derive(Debug)]
pub struct FullLevelStreamingEditor {
    pub core: LevelStreamingEditor,
    pub instance_manager: LevelInstanceManager,
    pub ui_state: LevelStreamingEditorUiState,
    pub profiler: LevelStreamingProfiler,
    pub transition_controller: TransitionController,
    pub culling_manager: CullingManager,
    pub spatial_hash: SpatialHashGrid,
    pub bandwidth_estimator: BandwidthEstimator,
    pub occlusion_cache_ext: OcclusionCache,
}

impl FullLevelStreamingEditor {
    pub fn new(config: LevelStreamingEditorConfig) -> Self {
        let vp_w = config.viewport_width as usize;
        let vp_h = config.viewport_height as usize;
        Self {
            core: LevelStreamingEditor::new(config),
            instance_manager: LevelInstanceManager::new(),
            ui_state: LevelStreamingEditorUiState::new(),
            profiler: LevelStreamingProfiler::new(),
            transition_controller: TransitionController::new(),
            culling_manager: CullingManager::new(vp_w, vp_h),
            spatial_hash: SpatialHashGrid::new(DEFAULT_CELL_SIZE),
            bandwidth_estimator: BandwidthEstimator::new(50.0),
            occlusion_cache_ext: OcclusionCache::new(8),
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        let start_frame = self.core.current_frame;
        self.core.tick(dt_s);
        self.transition_controller.update(dt_s);
        self.culling_manager.update_frustum(self.core.camera_view_proj);
        self.occlusion_cache_ext.evict_stale(self.core.current_frame);

        let queue_depth = self.core.load_queue.queue_depth();
        let memory_mb = self.core.memory_manager.used_mb;
        self.profiler.record_frame(dt_s * 1000.0, queue_depth, memory_mb, 0.0, dt_s);
    }

    pub fn add_level_with_instance(
        &mut self,
        name: String,
        asset: StreamingLevelAsset,
        bounds: Aabb,
        transform: Mat4,
    ) -> (u64, u64) {
        let level_id = self.core.add_level(name, asset, bounds);
        let inst_id = self.instance_manager.instantiate(level_id, transform);
        (level_id, inst_id)
    }

    pub fn select_level_in_ui(&mut self, id: u64, multi: bool) {
        self.ui_state.select_level(id, multi);
    }

    pub fn set_active_panel(&mut self, panel: EditorPanel) {
        self.ui_state.active_panel = panel;
    }

    pub fn get_profiler_summary(&self) -> String {
        format!(
            "Avg frame: {:.2}ms | P99: {:.2}ms | Peak mem: {:.1}MB | Max queue: {}",
            self.profiler.average_frame_time_ms(),
            self.profiler.p99_frame_time_ms(),
            self.profiler.peak_memory_mb(),
            self.profiler.max_queue_depth(),
        )
    }

    pub fn transition_to_sector(&mut self, from_level: u64, to_level: u64, transition: SectorTransitionType) {
        let duration = match transition {
            SectorTransitionType::Immediate => 0.0,
            SectorTransitionType::Fade => 1.0,
            SectorTransitionType::Portal => 0.5,
            SectorTransitionType::Teleport => 0.2,
        };
        self.transition_controller.begin_transition(from_level, to_level, transition, duration);
        self.core.force_load_level(to_level);
    }

    pub fn rebuild_spatial_hash(&mut self) {
        self.spatial_hash.clear();
        for level in self.core.levels.values() {
            self.spatial_hash.insert(level.id, level.bounds.center());
        }
    }

    pub fn query_levels_frustum_culled(&self) -> Vec<u64> {
        self.core.levels.values()
            .filter(|l| l.is_frustum_culled)
            .map(|l| l.id)
            .collect()
    }

    pub fn query_levels_occlusion_culled(&self) -> Vec<u64> {
        self.core.levels.values()
            .filter(|l| l.is_occlusion_culled)
            .map(|l| l.id)
            .collect()
    }

    pub fn estimate_load_order_time_ms(&self) -> f32 {
        let plan = self.core.dependency_graph.parallel_load_plan();
        let bandwidth = self.bandwidth_estimator.estimate();
        let mut total_ms = 0.0f32;
        for batch in plan {
            // Batch loads in parallel — time is the max of the batch
            let batch_time_ms = batch.iter()
                .filter_map(|&lid| self.core.levels.get(&lid))
                .map(|l| self.bandwidth_estimator.estimated_load_time_ms(
                    l.asset.size_bytes as f32 / (1024.0 * 1024.0)
                ))
                .fold(0.0f32, f32::max);
            total_ms += batch_time_ms;
        }
        total_ms
    }
}

// ============================================================
// EDITOR COMMAND SYSTEM (UNDO/REDO)
// ============================================================

#[derive(Debug, Clone)]
pub enum StreamingEditorCommand {
    SetLoadDistance { level_id: u64, old: f32, new: f32 },
    SetUnloadDistance { level_id: u64, old: f32, new: f32 },
    SetPriority { level_id: u64, old: LoadPriority, new: LoadPriority },
    SetPersistence { level_id: u64, old: LevelPersistence, new: LevelPersistence },
    AddDependency { from: u64, to: u64, edge: DependencyEdgeType },
    RemoveDependency { from: u64, to: u64 },
    MoveLevelBounds { level_id: u64, old_bounds: Aabb, new_bounds: Aabb },
    AddVolume { volume_id: u64 },
    RemoveVolume { volume_id: u64 },
    SetMemoryBudget { old: f32, new: f32 },
}

#[derive(Debug)]
pub struct CommandHistory {
    pub undo_stack: Vec<StreamingEditorCommand>,
    pub redo_stack: Vec<StreamingEditorCommand>,
    pub max_history: usize,
}

impl CommandHistory {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    pub fn push(&mut self, cmd: StreamingEditorCommand) {
        self.redo_stack.clear();
        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(cmd);
    }

    pub fn undo(&mut self) -> Option<StreamingEditorCommand> {
        if let Some(cmd) = self.undo_stack.pop() {
            self.redo_stack.push(cmd.clone());
            Some(cmd)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<StreamingEditorCommand> {
        if let Some(cmd) = self.redo_stack.pop() {
            self.undo_stack.push(cmd.clone());
            Some(cmd)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn clear(&mut self) { self.undo_stack.clear(); self.redo_stack.clear(); }
}

pub fn apply_streaming_command(editor: &mut FullLevelStreamingEditor, cmd: &StreamingEditorCommand) {
    match cmd {
        StreamingEditorCommand::SetLoadDistance { level_id, new, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.load_distance = *new;
            }
        }
        StreamingEditorCommand::SetUnloadDistance { level_id, new, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.unload_distance = *new;
            }
        }
        StreamingEditorCommand::SetPriority { level_id, new, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.priority = *new;
            }
        }
        StreamingEditorCommand::SetPersistence { level_id, new, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.persistence = *new;
            }
        }
        StreamingEditorCommand::AddDependency { from, to, edge } => {
            editor.core.dependency_graph.add_dependency(*from, *to, *edge);
        }
        StreamingEditorCommand::RemoveDependency { from, to } => {
            if let Some(node) = editor.core.dependency_graph.nodes.get_mut(from) {
                node.dependencies.retain(|&d| d != *to);
                node.edge_types.remove(to);
            }
        }
        StreamingEditorCommand::MoveLevelBounds { level_id, new_bounds, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.bounds = new_bounds.clone();
                l.sphere_bounds = Sphere::new(new_bounds.center(), new_bounds.extents().length());
            }
        }
        StreamingEditorCommand::AddVolume { .. } => {}
        StreamingEditorCommand::RemoveVolume { volume_id } => {
            editor.core.volumes.remove(volume_id);
        }
        StreamingEditorCommand::SetMemoryBudget { new, .. } => {
            editor.core.set_memory_budget(*new);
        }
    }
}

pub fn undo_streaming_command(editor: &mut FullLevelStreamingEditor, cmd: &StreamingEditorCommand) {
    match cmd {
        StreamingEditorCommand::SetLoadDistance { level_id, old, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.load_distance = *old;
            }
        }
        StreamingEditorCommand::SetUnloadDistance { level_id, old, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.unload_distance = *old;
            }
        }
        StreamingEditorCommand::SetPriority { level_id, old, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.priority = *old;
            }
        }
        StreamingEditorCommand::SetPersistence { level_id, old, .. } => {
            if let Some(l) = editor.core.levels.get_mut(level_id) {
                l.persistence = *old;
            }
        }
        StreamingEditorCommand::AddDependency { from, to, .. } => {
            if let Some(node) = editor.core.dependency_graph.nodes.get_mut(from) {
                node.dependencies.retain(|&d| d != *to);
            }
        }
        StreamingEditorCommand::SetMemoryBudget { old, .. } => {
            editor.core.set_memory_budget(*old);
        }
        _ => {}
    }
}

// ============================================================
// LEVEL STREAMING SETTINGS PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct LevelStreamingSettingsPanel {
    pub show_advanced: bool,
    pub pending_budget_mb: f32,
    pub pending_cell_size: f32,
    pub pending_load_dist: f32,
    pub pending_unload_dist: f32,
    pub pending_max_loads: usize,
    pub pending_prefetch: bool,
    pub pending_eviction: EvictionPolicy,
    pub pending_frustum_cull: bool,
    pub pending_occlusion_cull: bool,
    pub is_dirty: bool,
}

impl LevelStreamingSettingsPanel {
    pub fn new(config: &LevelStreamingEditorConfig) -> Self {
        Self {
            show_advanced: false,
            pending_budget_mb: config.memory_budget_mb,
            pending_cell_size: config.default_cell_size,
            pending_load_dist: config.default_load_distance,
            pending_unload_dist: config.default_unload_distance,
            pending_max_loads: config.max_concurrent_loads,
            pending_prefetch: config.enable_prefetch,
            pending_eviction: config.eviction_policy,
            pending_frustum_cull: config.enable_frustum_culling,
            pending_occlusion_cull: config.enable_occlusion_culling,
            is_dirty: false,
        }
    }

    pub fn set_budget_mb(&mut self, v: f32) {
        self.pending_budget_mb = v.max(64.0);
        self.is_dirty = true;
    }

    pub fn set_load_distance(&mut self, v: f32) {
        self.pending_load_dist = v.max(10.0);
        if self.pending_unload_dist < self.pending_load_dist {
            self.pending_unload_dist = self.pending_load_dist + STREAMING_HYSTERESIS;
        }
        self.is_dirty = true;
    }

    pub fn apply_to_config(&mut self, config: &mut LevelStreamingEditorConfig) {
        config.memory_budget_mb = self.pending_budget_mb;
        config.default_cell_size = self.pending_cell_size;
        config.default_load_distance = self.pending_load_dist;
        config.default_unload_distance = self.pending_unload_dist;
        config.max_concurrent_loads = self.pending_max_loads;
        config.enable_prefetch = self.pending_prefetch;
        config.eviction_policy = self.pending_eviction;
        config.enable_frustum_culling = self.pending_frustum_cull;
        config.enable_occlusion_culling = self.pending_occlusion_cull;
        self.is_dirty = false;
    }

    pub fn has_unsaved_changes(&self) -> bool { self.is_dirty }
}

// ============================================================
// STREAMING HEAT MAP
// ============================================================

#[derive(Debug)]
pub struct StreamingHeatMap {
    pub grid_w: usize,
    pub grid_h: usize,
    pub cell_size: f32,
    pub origin: Vec2,
    pub data: Vec<f32>,     // normalized heat 0..1
    pub raw_counts: Vec<u32>, // load/unload events per cell
}

impl StreamingHeatMap {
    pub fn new(grid_w: usize, grid_h: usize, cell_size: f32, origin: Vec2) -> Self {
        let n = grid_w * grid_h;
        Self {
            grid_w,
            grid_h,
            cell_size,
            origin,
            data: vec![0.0; n],
            raw_counts: vec![0; n],
        }
    }

    pub fn world_to_cell(&self, world_x: f32, world_z: f32) -> Option<(usize, usize)> {
        let cx = ((world_x - self.origin.x) / self.cell_size) as isize;
        let cz = ((world_z - self.origin.y) / self.cell_size) as isize;
        if cx >= 0 && cx < self.grid_w as isize && cz >= 0 && cz < self.grid_h as isize {
            Some((cx as usize, cz as usize))
        } else {
            None
        }
    }

    pub fn record_event(&mut self, world_x: f32, world_z: f32) {
        if let Some((cx, cz)) = self.world_to_cell(world_x, world_z) {
            self.raw_counts[cz * self.grid_w + cx] += 1;
        }
    }

    pub fn normalize(&mut self) {
        let max = self.raw_counts.iter().copied().max().unwrap_or(1).max(1) as f32;
        for (i, &count) in self.raw_counts.iter().enumerate() {
            self.data[i] = count as f32 / max;
        }
    }

    pub fn decay(&mut self, factor: f32) {
        for c in &mut self.raw_counts {
            *c = (*c as f32 * factor) as u32;
        }
        self.normalize();
    }

    pub fn sample(&self, world_x: f32, world_z: f32) -> f32 {
        self.world_to_cell(world_x, world_z)
            .map(|(cx, cz)| self.data[cz * self.grid_w + cx])
            .unwrap_or(0.0)
    }

    pub fn peak_cell(&self) -> Option<(usize, usize)> {
        let (idx, _) = self.data.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;
        Some((idx % self.grid_w, idx / self.grid_w))
    }
}

// ============================================================
// AI SPAWN AWARENESS
// ============================================================

#[derive(Debug, Clone)]
pub struct AiSpawnRequest {
    pub spawn_id: u64,
    pub type_id: u32,
    pub preferred_sector: Option<u64>,
    pub spawn_position: Option<Vec3>,
    pub count: u32,
    pub priority: u32,
}

#[derive(Debug)]
pub struct SectorAwareAiSpawner {
    pub pending_requests: VecDeque<AiSpawnRequest>,
    pub active_spawns: HashMap<u64, Vec<u64>>, // sector_id -> [spawn_ids]
    pub max_per_sector: u32,
}

impl SectorAwareAiSpawner {
    pub fn new(max_per_sector: u32) -> Self {
        Self {
            pending_requests: VecDeque::new(),
            active_spawns: HashMap::new(),
            max_per_sector,
        }
    }

    pub fn request_spawn(&mut self, req: AiSpawnRequest) {
        self.pending_requests.push_back(req);
    }

    pub fn process_requests(&mut self, sector_graph: &SectorGraph, camera_pos: Vec3) {
        let mut processed = Vec::new();
        for (i, req) in self.pending_requests.iter().enumerate() {
            let target_sector = req.preferred_sector
                .or_else(|| sector_graph.find_sector_at(camera_pos));
            let Some(sector_id) = target_sector else { continue };
            let current_count = self.active_spawns.get(&sector_id).map(|v| v.len()).unwrap_or(0) as u32;
            if current_count + req.count > self.max_per_sector { continue; }
            if let Some(sector) = sector_graph.sectors.get(&sector_id) {
                if sector.level_ids.is_empty() { continue; }
                // Spawn at nearest waypoint if no position given
                let spawn_pos = req.spawn_position.or_else(|| {
                    sector.nearest_waypoint(camera_pos).map(|w| w.position)
                });
                if spawn_pos.is_some() {
                    let entry = self.active_spawns.entry(sector_id).or_default();
                    for _ in 0..req.count {
                        entry.push(req.spawn_id);
                    }
                    processed.push(i);
                }
            }
        }
        for i in processed.into_iter().rev() {
            self.pending_requests.remove(i);
        }
    }

    pub fn total_active_spawns(&self) -> usize {
        self.active_spawns.values().map(|v| v.len()).sum()
    }

    pub fn clear_sector(&mut self, sector_id: u64) {
        self.active_spawns.remove(&sector_id);
    }
}

// ============================================================
// STREAMING LOD ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct LodAnalysisResult {
    pub level_id: u64,
    pub recommended_lod: LodLevel,
    pub current_lod: LodLevel,
    pub lod_mismatch: bool,
    pub potential_memory_save_mb: f32,
    pub screen_size_px: f32,
    pub distance_m: f32,
}

pub fn analyze_lod_distribution(
    levels: &[StreamingLevel],
    budget_manager: &CombinedBudgetManager,
) -> Vec<LodAnalysisResult> {
    let global_bias = budget_manager.compute_global_lod_bias();
    levels.iter().map(|level| {
        let recommended = budget_manager.optimal_lod_for_budget(level.id, level.distance_to_camera);
        let current_mem = level.memory_estimate_mb();
        let rec_mem = {
            let mut tmp = level.clone();
            tmp.current_lod = recommended;
            tmp.memory_estimate_mb()
        };
        LodAnalysisResult {
            level_id: level.id,
            recommended_lod: recommended,
            current_lod: level.current_lod,
            lod_mismatch: recommended != level.current_lod,
            potential_memory_save_mb: (current_mem - rec_mem).max(0.0),
            screen_size_px: level.screen_size,
            distance_m: level.distance_to_camera,
        }
    }).collect()
}

pub fn compute_streaming_importance(
    level: &StreamingLevel,
    camera_pos: Vec3,
    camera_dir: Vec3,
    time_since_last_load_s: f32,
) -> f32 {
    let dist = level.bounds.distance_to_point(camera_pos).max(0.01);

    // Distance factor: closer is more important
    let dist_factor = 1.0 / (1.0 + dist * 0.001);

    // Directional factor: more important if in camera view direction
    let to_level = (level.bounds.center() - camera_pos).normalize_or_zero();
    let dir_factor = (camera_dir.dot(to_level) * 0.5 + 0.5).powf(2.0);

    // Recency factor: levels not recently loaded get a small bump
    let recency = (1.0 - (-time_since_last_load_s * 0.1).exp()) * 0.2;

    // Screen size factor
    let screen_factor = level.screen_size.clamp(0.0, 1.0);

    // Importance weight from user
    let user_weight = level.importance_weight;

    (dist_factor * dir_factor + recency + screen_factor * 0.3) * user_weight
}

pub fn compute_cell_load_radius(
    camera_velocity: Vec3,
    base_radius: f32,
    lookahead_s: f32,
) -> f32 {
    let speed = camera_velocity.length();
    // Expand radius in velocity direction proportional to speed
    base_radius + speed * lookahead_s
}

pub fn distance_based_lod_bias(distance: f32, budget_pressure: f32) -> f32 {
    // Base LOD bias from budget pressure
    let budget_bias = budget_pressure * 3.0;
    // Additional bias from distance
    let dist_bias = (distance * LOD_BIAS_DISTANCE_SCALE).powf(1.5);
    budget_bias + dist_bias
}

pub fn hysteresis_check_load(dist: f32, load_dist: f32, hysteresis: f32) -> bool {
    dist < load_dist - hysteresis
}

pub fn hysteresis_check_unload(dist: f32, unload_dist: f32, hysteresis: f32) -> bool {
    dist > unload_dist + hysteresis
}

pub fn memory_mb_to_bytes(mb: f32) -> u64 {
    (mb * 1024.0 * 1024.0) as u64
}

pub fn bytes_to_memory_mb(bytes: u64) -> f32 {
    bytes as f32 / (1024.0 * 1024.0)
}

pub fn compute_sector_portal_visibility(
    portal: &Portal,
    camera_pos: Vec3,
    camera_dir: Vec3,
) -> f32 {
    if !portal.is_open { return 0.0; }
    let to_portal = portal.center - camera_pos;
    let dist = to_portal.length();
    if dist < 1e-4 { return 1.0; }
    let norm = to_portal / dist;
    let dot_dir = camera_dir.dot(norm).max(0.0);
    let dot_normal = (-portal.normal).dot(norm).max(0.0);
    dot_dir * dot_normal * portal.transmission * (1.0 / (1.0 + dist * 0.001))
}

pub fn compute_level_bandwidth_estimate_mb_s(
    file_size_bytes: u64,
    load_time_ms: f32,
) -> f32 {
    if load_time_ms <= 0.0 { return 0.0; }
    let mb = file_size_bytes as f32 / (1024.0 * 1024.0);
    mb / (load_time_ms / 1000.0)
}

pub fn priority_score_for_distance(
    dist: f32,
    base_priority: LoadPriority,
    importance: f32,
) -> f32 {
    let p = match base_priority {
        LoadPriority::Critical => 10000.0,
        LoadPriority::High => 1000.0,
        LoadPriority::Medium => 100.0,
        LoadPriority::Low => 10.0,
        LoadPriority::Prefetch => 1.0,
    };
    let d = (1000.0 / dist.max(1.0)).min(100.0);
    p * importance + d
}

// ============================================================
// TESTS / EXAMPLES
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_asset(id: u64, size_bytes: u64) -> StreamingLevelAsset {
        StreamingLevelAsset {
            id,
            name: format!("Asset_{}", id),
            file_path: format!("content/levels/level_{}.pak", id),
            size_bytes,
            uncompressed_size_bytes: size_bytes * 2,
            dependencies: Vec::new(),
            load_time_estimate_ms: 200.0,
        }
    }

    #[test]
    fn test_aabb_distance() {
        let aabb = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
        assert!((aabb.distance_to_point(Vec3::splat(5.0)) - 0.0).abs() < 1e-5);
        let dist = aabb.distance_to_point(Vec3::new(20.0, 5.0, 5.0));
        assert!((dist - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_frustum_culling() {
        // Simple perspective-like matrix
        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
        let vp = proj * view;
        let frustum = Frustum::from_view_proj(vp);
        let aabb = Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        // Center of world should be visible
        let _ = frustum.test_aabb(&aabb);
    }

    #[test]
    fn test_dependency_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency(1, 2, DependencyEdgeType::HardDependency);
        graph.add_dependency(2, 3, DependencyEdgeType::HardDependency);
        graph.add_dependency(3, 1, DependencyEdgeType::HardDependency);
        assert!(graph.detect_cycles().is_some());
    }

    #[test]
    fn test_dependency_no_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency(1, 2, DependencyEdgeType::HardDependency);
        graph.add_dependency(2, 3, DependencyEdgeType::HardDependency);
        assert!(graph.detect_cycles().is_none());
    }

    #[test]
    fn test_memory_manager_eviction() {
        let mut mgr = MemoryPressureManager::new(100.0);
        mgr.record_load(1, 40.0);
        mgr.record_load(2, 30.0);
        mgr.record_load(3, 20.0);
        assert!(!mgr.is_critical());
        mgr.record_load(4, 8.0);
        // 98/100 = 98%, over critical threshold
        assert!(mgr.is_critical());
    }

    #[test]
    fn test_hzb_build() {
        let mut hzb = HierarchicalZBuffer::new(8, 8);
        let depth = vec![0.5f32; 64];
        hzb.build_from_depth(&depth);
        assert!(hzb.mips[0].data.iter().all(|&d| (d - 0.5).abs() < 1e-5));
    }

    #[test]
    fn test_prefetch_prediction() {
        let mut predictor = PrefetchPredictor::new(2.0);
        let t = 0.0;
        predictor.update(Vec3::ZERO, t);
        predictor.update(Vec3::new(10.0, 0.0, 0.0), 1000.0);
        let predicted = predictor.predicted_camera_pos();
        // Moving at 10 units/sec, 2s lookahead => ~20 units ahead
        assert!(predicted.x > 15.0);
    }

    #[test]
    fn test_world_partition() {
        let mut grid = WorldPartitionGrid::new(512.0, Vec3::ZERO);
        let coord = grid.world_to_cell(Vec3::new(256.0, 0.0, 256.0));
        assert_eq!(coord, CellCoord::new(0, 0, 0));
        let coord2 = grid.world_to_cell(Vec3::new(600.0, 0.0, 600.0));
        assert_eq!(coord2, CellCoord::new(1, 1, 0));
    }

    #[test]
    fn test_load_queue_priority() {
        let mut queue = StreamingLoadQueue::new(4);
        queue.enqueue(LoadRequest {
            level_id: 1, priority: LoadPriority::Low,
            distance_weight: 0.1, enqueue_time_ms: 0.0,
            predicted_load_time_ms: 100.0, is_prefetch: false,
        });
        queue.enqueue(LoadRequest {
            level_id: 2, priority: LoadPriority::Critical,
            distance_weight: 0.1, enqueue_time_ms: 0.0,
            predicted_load_time_ms: 100.0, is_prefetch: false,
        });
        let next = queue.dequeue_next().unwrap();
        assert_eq!(next.level_id, 2); // Critical should come first
    }

    #[test]
    fn test_streaming_level_lod() {
        let asset = make_asset(1, 50 * 1024 * 1024);
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(100.0));
        let level = StreamingLevel::new(1, "Test".into(), asset, bounds);
        assert_eq!(level.compute_lod(50.0, 0.0), LodLevel::Lod0);
        assert_eq!(level.compute_lod(200.0, 0.0), LodLevel::Lod1);
        assert_eq!(level.compute_lod(500.0, 0.0), LodLevel::Lod2);
        assert_eq!(level.compute_lod(800.0, 0.0), LodLevel::Lod3);
        assert_eq!(level.compute_lod(1500.0, 0.0), LodLevel::Culled);
    }

    #[test]
    fn test_editor_add_level() {
        let config = LevelStreamingEditorConfig::default();
        let mut editor = LevelStreamingEditor::new(config);
        let asset = make_asset(1, 10 * 1024 * 1024);
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(512.0));
        let id = editor.add_level("TestLevel".into(), asset, bounds);
        assert!(editor.levels.contains_key(&id));
    }

    #[test]
    fn test_sector_pvs() {
        let mut sg = SectorGraph::new();
        let s1 = Sector::new(1, "Room1".into(), Aabb::new(Vec3::ZERO, Vec3::splat(10.0)));
        let s2 = Sector::new(2, "Room2".into(), Aabb::new(Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 10.0)));
        sg.add_sector(s1);
        sg.add_sector(s2);
        let portal = Portal::new(1, 1, 2, Vec3::new(10.0, 5.0, 5.0), Vec3::new(-1.0, 0.0, 0.0), Vec2::new(2.0, 2.0));
        sg.add_portal(portal);
        // Camera in sector 1
        let pvs = sg.compute_pvs(Vec3::new(5.0, 5.0, 5.0), &Mat4::IDENTITY, 4);
        assert!(pvs.contains(&1));
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency(3, 2, DependencyEdgeType::HardDependency);
        graph.add_dependency(3, 1, DependencyEdgeType::HardDependency);
        graph.add_dependency(2, 1, DependencyEdgeType::HardDependency);
        let result = graph.topological_sort().expect("no cycle");
        // 1 should come before 2 and 3
        let pos_1 = result.iter().position(|&x| x == 1).unwrap();
        let pos_2 = result.iter().position(|&x| x == 2).unwrap();
        let pos_3 = result.iter().position(|&x| x == 3).unwrap();
        assert!(pos_1 < pos_2);
        assert!(pos_1 < pos_3);
    }

    #[test]
    fn test_command_history() {
        let mut hist = CommandHistory::new(10);
        hist.push(StreamingEditorCommand::SetMemoryBudget { old: 512.0, new: 1024.0 });
        assert!(hist.can_undo());
        let cmd = hist.undo().unwrap();
        assert!(hist.can_redo());
    }

    #[test]
    fn test_bandwidth_estimator() {
        let mut est = BandwidthEstimator::new(50.0);
        est.add_sample(100.0);
        est.add_sample(100.0);
        // EMA should be moving toward 100
        assert!(est.estimate() > 50.0);
    }

    #[test]
    fn test_heatmap() {
        let mut hm = StreamingHeatMap::new(10, 10, 100.0, Vec2::ZERO);
        hm.record_event(50.0, 50.0);
        hm.record_event(50.0, 50.0);
        hm.normalize();
        assert!((hm.sample(50.0, 50.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_spatial_hash() {
        let mut hash = SpatialHashGrid::new(100.0);
        hash.insert(1, Vec3::new(50.0, 0.0, 50.0));
        hash.insert(2, Vec3::new(1000.0, 0.0, 1000.0));
        let results = hash.query_radius(Vec3::new(50.0, 0.0, 50.0), 10.0);
        assert!(results.contains(&1));
        assert!(!results.contains(&2));
    }

    #[test]
    fn test_sphere_containment() {
        let s = Sphere::new(Vec3::ZERO, 10.0);
        assert!(s.contains_point(Vec3::new(5.0, 0.0, 0.0)));
        assert!(!s.contains_point(Vec3::new(15.0, 0.0, 0.0)));
    }

    #[test]
    fn test_combined_budget() {
        let mut mgr = CombinedBudgetManager::new(1000.0);
        mgr.update_entry(1, LodLevel::Lod0, 100.0, 10.0);
        mgr.update_entry(2, LodLevel::Lod1, 50.0, 5.0);
        assert!((mgr.total_usage_mb() - 165.0).abs() < 1e-4);
    }
}
