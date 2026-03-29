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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        let bounds_center = bounds.center();
        let mut level = StreamingLevel::new(id, name, asset, bounds);
        level.load_distance = self.config.default_load_distance;
        level.unload_distance = self.config.default_unload_distance;
        self.dependency_graph.add_level(id);

        // Register in world grid
        let cell = self.world_grid.world_to_cell(bounds_center);
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
    pub instance_id: u64,
    pub level_id: u64,
    pub transform: Mat4,
    pub override_lod: Option<LodLevel>,
    pub visible: bool,
    pub cast_shadows: bool,
    pub custom_culling_distance: Option<f32>,
    pub tags: HashSet<String>,
    pub metadata: HashMap<String, String>,
    pub creation_time: f64,
    pub last_modified_time: f64,
    pub override_load_distance: Option<f32>,
    pub override_priority: Option<LoadPriority>,
}

impl LevelInstance {
    pub fn new(instance_id: u64, level_id: u64, transform: Mat4) -> Self {
        Self {
            instance_id,
            level_id,
            transform,
            override_lod: None,
            visible: true,
            cast_shadows: true,
            custom_culling_distance: None,
            tags: HashSet::new(),
            metadata: HashMap::new(),
            creation_time: 0.0,
            last_modified_time: 0.0,
            override_load_distance: None,
            override_priority: None,
        }
    }

    pub fn world_position(&self) -> Vec3 {
        self.transform.transform_point3(Vec3::ZERO)
    }

    pub fn position(&self) -> Vec3 {
        Vec3::new(self.transform.w_axis.x, self.transform.w_axis.y, self.transform.w_axis.z)
    }

    pub fn rotation_quat(&self) -> Quat {
        let m = &self.transform;
        let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
        let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
        let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
        let rm = Mat4::from_cols(
            m.x_axis / sx,
            m.y_axis / sy,
            m.z_axis / sz,
            Vec4::W,
        );
        Quat::from_mat4(&rm)
    }

    pub fn scale(&self) -> Vec3 {
        let m = &self.transform;
        Vec3::new(
            Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length(),
            Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length(),
            Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length(),
        )
    }

    pub fn add_tag(&mut self, tag: &str) {
        self.tags.insert(tag.to_string());
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    pub fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
        self.last_modified_time += 0.001;
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
            .map(|inst| inst.instance_id)
            .collect()
    }

    pub fn active_instance_count(&self) -> usize {
        self.instances.values().filter(|i| i.visible).count()
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

// ============================================================
// STREAMING DISTANCE CACHE
// ============================================================

#[derive(Debug)]
pub struct StreamingDistanceCache {
    pub distances: HashMap<u64, f32>,
    pub last_camera_pos: Vec3,
    pub dirty_threshold_sq: f32,
    pub frame_updated: u64,
}

impl StreamingDistanceCache {
    pub fn new() -> Self {
        Self {
            distances: HashMap::new(),
            last_camera_pos: Vec3::splat(f32::MAX),
            dirty_threshold_sq: 1.0,
            frame_updated: 0,
        }
    }

    pub fn update(&mut self, camera_pos: Vec3, levels: &[StreamingLevel], frame: u64) {
        let moved_sq = (camera_pos - self.last_camera_pos).length_squared();
        if moved_sq < self.dirty_threshold_sq && frame == self.frame_updated {
            return;
        }
        self.last_camera_pos = camera_pos;
        self.frame_updated = frame;
        for level in levels {
            let dist = level.bounds.distance_to_point(camera_pos);
            self.distances.insert(level.id, dist);
        }
    }

    pub fn get(&self, level_id: u64) -> Option<f32> {
        self.distances.get(&level_id).copied()
    }

    pub fn invalidate(&mut self) {
        self.last_camera_pos = Vec3::splat(f32::MAX);
    }

    pub fn nearest_level_id(&self) -> Option<u64> {
        self.distances.iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, _)| id)
    }
}

// ============================================================
// DYNAMIC OBJECT STREAMING TRACKER
// ============================================================

#[derive(Debug, Clone)]
pub struct DynamicStreamingObject {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub velocity: Vec3,
    pub bounds_radius: f32,
    pub current_cell: CellCoord,
    pub visible: bool,
    pub importance: f32,
    pub last_move_time_ms: f64,
}

impl DynamicStreamingObject {
    pub fn new(id: u64, name: String, position: Vec3, bounds_radius: f32) -> Self {
        Self {
            id,
            name,
            position,
            velocity: Vec3::ZERO,
            bounds_radius,
            current_cell: CellCoord::new(0, 0, 0),
            visible: true,
            importance: 1.0,
            last_move_time_ms: 0.0,
        }
    }

    pub fn update_position(&mut self, new_pos: Vec3, dt_s: f32, time_ms: f64) {
        let delta = new_pos - self.position;
        if dt_s > 1e-6 {
            self.velocity = delta / dt_s;
        }
        self.position = new_pos;
        if delta.length_squared() > 0.001 {
            self.last_move_time_ms = time_ms;
        }
    }

    pub fn predicted_position(&self, lookahead_s: f32) -> Vec3 {
        self.position + self.velocity * lookahead_s
    }

    pub fn bounds_sphere(&self) -> Sphere {
        Sphere::new(self.position, self.bounds_radius)
    }

    pub fn speed(&self) -> f32 { self.velocity.length() }
}

#[derive(Debug)]
pub struct DynamicObjectTracker {
    pub objects: HashMap<u64, DynamicStreamingObject>,
    pub next_id: u64,
    pub grid: WorldPartitionGrid,
    pub total_moves: u64,
}

impl DynamicObjectTracker {
    pub fn new(cell_size: f32) -> Self {
        Self {
            objects: HashMap::new(),
            next_id: 1,
            grid: WorldPartitionGrid::new(cell_size, Vec3::ZERO),
            total_moves: 0,
        }
    }

    pub fn spawn(&mut self, name: String, pos: Vec3, radius: f32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut obj = DynamicStreamingObject::new(id, name, pos, radius);
        obj.current_cell = self.grid.world_to_cell(pos);
        self.grid.register_object(id, pos);
        self.objects.insert(id, obj);
        id
    }

    pub fn despawn(&mut self, id: u64) {
        if let Some(_obj) = self.objects.remove(&id) {
            self.grid.unregister_object(id);
        }
    }

    pub fn update_position(&mut self, id: u64, new_pos: Vec3, dt_s: f32, time_ms: f64) {
        if let Some(obj) = self.objects.get_mut(&id) {
            let old_cell = obj.current_cell;
            obj.update_position(new_pos, dt_s, time_ms);
            let new_cell = self.grid.world_to_cell(new_pos);
            obj.current_cell = new_cell;
            if old_cell != new_cell {
                self.grid.register_object(id, new_pos);
                self.total_moves += 1;
            }
        }
    }

    pub fn query_near(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        self.grid.query_objects_in_radius(pos, radius)
    }

    pub fn objects_in_cell(&self, coord: CellCoord) -> Vec<u64> {
        self.grid.cells.get(&coord)
            .map(|c| c.dynamic_object_ids.clone())
            .unwrap_or_default()
    }

    pub fn total_objects(&self) -> usize { self.objects.len() }

    pub fn objects_by_importance(&self) -> Vec<&DynamicStreamingObject> {
        let mut sorted: Vec<&DynamicStreamingObject> = self.objects.values().collect();
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    pub fn fast_moving_objects(&self, speed_threshold: f32) -> Vec<u64> {
        self.objects.iter()
            .filter(|(_, o)| o.speed() > speed_threshold)
            .map(|(&id, _)| id)
            .collect()
    }
}

// ============================================================
// LEVEL STREAMING ANALYTICS
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingAnalyticsSession {
    pub session_id: u64,
    pub start_time_ms: f64,
    pub end_time_ms: f64,
    pub total_loads: u32,
    pub total_unloads: u32,
    pub total_evictions: u32,
    pub peak_memory_mb: f32,
    pub total_bandwidth_mb: f32,
    pub stall_events: u32,
    pub average_load_latency_ms: f32,
    pub prefetch_hit_rate: f32,
    pub unique_levels_loaded: HashSet<u64>,
}

impl StreamingAnalyticsSession {
    pub fn new(session_id: u64, start_ms: f64) -> Self {
        Self {
            session_id,
            start_time_ms: start_ms,
            end_time_ms: start_ms,
            total_loads: 0,
            total_unloads: 0,
            total_evictions: 0,
            peak_memory_mb: 0.0,
            total_bandwidth_mb: 0.0,
            stall_events: 0,
            average_load_latency_ms: 0.0,
            prefetch_hit_rate: 0.0,
            unique_levels_loaded: HashSet::new(),
        }
    }

    pub fn record_load(&mut self, level_id: u64, latency_ms: f32, mb: f32) {
        self.total_loads += 1;
        self.total_bandwidth_mb += mb;
        self.unique_levels_loaded.insert(level_id);
        let n = self.total_loads as f32;
        self.average_load_latency_ms = self.average_load_latency_ms * (n - 1.0) / n + latency_ms / n;
    }

    pub fn record_unload(&mut self) { self.total_unloads += 1; }
    pub fn record_eviction(&mut self) { self.total_evictions += 1; }
    pub fn record_stall(&mut self) { self.stall_events += 1; }

    pub fn update_peak_memory(&mut self, used_mb: f32) {
        self.peak_memory_mb = self.peak_memory_mb.max(used_mb);
    }

    pub fn duration_s(&self) -> f32 {
        ((self.end_time_ms - self.start_time_ms) / 1000.0) as f32
    }

    pub fn average_bandwidth_mb_s(&self) -> f32 {
        let d = self.duration_s();
        if d > 0.0 { self.total_bandwidth_mb / d } else { 0.0 }
    }

    pub fn finalize(&mut self, end_ms: f64) {
        self.end_time_ms = end_ms;
    }

    pub fn efficiency_score(&self) -> f32 {
        // Ratio of unique levels loaded vs total loads (high = low redundancy)
        if self.total_loads == 0 { return 1.0; }
        self.unique_levels_loaded.len() as f32 / self.total_loads as f32
    }
}

// ============================================================
// REGION OF INTEREST SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct RegionOfInterest {
    pub id: u64,
    pub name: String,
    pub bounds: Aabb,
    pub boost_priority: LoadPriority,
    pub boost_load_distance: f32,
    pub is_active: bool,
    pub activation_condition: String,
    pub activation_time_ms: f64,
}

impl RegionOfInterest {
    pub fn new(id: u64, name: String, bounds: Aabb, priority: LoadPriority) -> Self {
        Self {
            id,
            name,
            bounds,
            boost_priority: priority,
            boost_load_distance: 200.0,
            is_active: false,
            activation_condition: String::new(),
            activation_time_ms: 0.0,
        }
    }

    pub fn activate(&mut self, time_ms: f64) {
        self.is_active = true;
        self.activation_time_ms = time_ms;
    }

    pub fn deactivate(&mut self) { self.is_active = false; }

    pub fn camera_in_range(&self, camera_pos: Vec3, margin: f32) -> bool {
        self.is_active && self.bounds.expand_by(margin).contains_point(camera_pos)
    }

    pub fn overlap_area_with(&self, other: &Aabb) -> f32 {
        let ix = (self.bounds.max.x.min(other.max.x) - self.bounds.min.x.max(other.min.x)).max(0.0);
        let iy = (self.bounds.max.y.min(other.max.y) - self.bounds.min.y.max(other.min.y)).max(0.0);
        let iz = (self.bounds.max.z.min(other.max.z) - self.bounds.min.z.max(other.min.z)).max(0.0);
        ix * iy * iz
    }
}

#[derive(Debug)]
pub struct RegionOfInterestManager {
    pub regions: HashMap<u64, RegionOfInterest>,
    pub active_regions: HashSet<u64>,
    pub next_id: u64,
}

impl RegionOfInterestManager {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
            active_regions: HashSet::new(),
            next_id: 1,
        }
    }

    pub fn add_region(&mut self, name: String, bounds: Aabb, priority: LoadPriority) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.regions.insert(id, RegionOfInterest::new(id, name, bounds, priority));
        id
    }

    pub fn update(&mut self, camera_pos: Vec3, time_ms: f64) {
        self.active_regions.clear();
        for (id, region) in &mut self.regions {
            let in_range = region.bounds.expand_by(region.boost_load_distance).contains_point(camera_pos);
            if in_range && !region.is_active {
                region.activate(time_ms);
            } else if !in_range && region.is_active {
                region.deactivate();
            }
            if region.is_active {
                self.active_regions.insert(*id);
            }
        }
    }

    pub fn priority_for_level(&self, level: &StreamingLevel) -> LoadPriority {
        for &id in &self.active_regions {
            if let Some(region) = self.regions.get(&id) {
                if region.bounds.intersects(&level.bounds) {
                    return region.boost_priority;
                }
            }
        }
        level.priority
    }

    pub fn any_active_near(&self, pos: Vec3, radius: f32) -> bool {
        self.active_regions.iter().any(|&id| {
            self.regions.get(&id).map_or(false, |r| r.bounds.distance_to_point(pos) <= radius)
        })
    }
}

// ============================================================
// STREAMING CHECKPOINT SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct StreamingCheckpoint {
    pub id: u64,
    pub name: String,
    pub camera_position: Vec3,
    pub camera_direction: Vec3,
    pub loaded_level_ids: Vec<u64>,
    pub memory_used_mb: f32,
    pub timestamp_ms: f64,
    pub save_slot: u32,
}

impl StreamingCheckpoint {
    pub fn capture(
        id: u64,
        name: String,
        camera_pos: Vec3,
        camera_dir: Vec3,
        levels: &[StreamingLevel],
        memory_mb: f32,
        time_ms: f64,
    ) -> Self {
        let loaded: Vec<u64> = levels.iter()
            .filter(|l| l.state == StreamingState::Loaded)
            .map(|l| l.id)
            .collect();
        Self {
            id,
            name,
            camera_position: camera_pos,
            camera_direction: camera_dir,
            loaded_level_ids: loaded,
            memory_used_mb: memory_mb,
            timestamp_ms: time_ms,
            save_slot: 0,
        }
    }

    pub fn warm_up_requests(&self) -> Vec<u64> {
        self.loaded_level_ids.clone()
    }
}

#[derive(Debug)]
pub struct CheckpointManager {
    pub checkpoints: HashMap<u64, StreamingCheckpoint>,
    pub next_id: u64,
    pub auto_checkpoint_interval_ms: f64,
    pub last_auto_checkpoint_ms: f64,
    pub max_checkpoints: usize,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            checkpoints: HashMap::new(),
            next_id: 1,
            auto_checkpoint_interval_ms: 60_000.0,
            last_auto_checkpoint_ms: 0.0,
            max_checkpoints: 16,
        }
    }

    pub fn save(
        &mut self,
        name: String,
        camera_pos: Vec3,
        camera_dir: Vec3,
        levels: &[StreamingLevel],
        memory_mb: f32,
        time_ms: f64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let cp = StreamingCheckpoint::capture(id, name, camera_pos, camera_dir, levels, memory_mb, time_ms);
        if self.checkpoints.len() >= self.max_checkpoints {
            if let Some(&oldest_id) = self.checkpoints.keys().next() {
                self.checkpoints.remove(&oldest_id);
            }
        }
        self.checkpoints.insert(id, cp);
        id
    }

    pub fn maybe_auto_checkpoint(
        &mut self,
        camera_pos: Vec3,
        camera_dir: Vec3,
        levels: &[StreamingLevel],
        memory_mb: f32,
        time_ms: f64,
    ) -> Option<u64> {
        if time_ms - self.last_auto_checkpoint_ms >= self.auto_checkpoint_interval_ms {
            self.last_auto_checkpoint_ms = time_ms;
            Some(self.save("Auto".into(), camera_pos, camera_dir, levels, memory_mb, time_ms))
        } else { None }
    }

    pub fn get_latest(&self) -> Option<&StreamingCheckpoint> {
        self.checkpoints.values()
            .max_by(|a, b| a.timestamp_ms.partial_cmp(&b.timestamp_ms).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn delete_checkpoint(&mut self, id: u64) -> bool {
        self.checkpoints.remove(&id).is_some()
    }

    pub fn total_saved_level_ids(&self) -> HashSet<u64> {
        let mut set = HashSet::new();
        for cp in self.checkpoints.values() {
            for &id in &cp.loaded_level_ids { set.insert(id); }
        }
        set
    }
}

// ============================================================
// LOD TRANSITION SMOOTHER
// ============================================================

#[derive(Debug, Clone)]
pub struct LodTransition {
    pub level_id: u64,
    pub from_lod: LodLevel,
    pub to_lod: LodLevel,
    pub progress: f32,
    pub duration_s: f32,
    pub blend_distance: f32,
}

impl LodTransition {
    pub fn new(level_id: u64, from: LodLevel, to: LodLevel, duration_s: f32) -> Self {
        Self {
            level_id,
            from_lod: from,
            to_lod: to,
            progress: 0.0,
            duration_s,
            blend_distance: 50.0,
        }
    }

    pub fn update_progress(&mut self, dt_s: f32) -> bool {
        self.progress += dt_s / self.duration_s.max(0.001);
        self.progress >= 1.0
    }

    pub fn blend_alpha(&self) -> f32 {
        let t = self.progress.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t) // smooth step
    }

    pub fn is_complete(&self) -> bool { self.progress >= 1.0 }

    pub fn reversed(&self) -> Self {
        LodTransition::new(self.level_id, self.to_lod, self.from_lod, self.duration_s)
    }
}

#[derive(Debug)]
pub struct LodTransitionManager {
    pub transitions: HashMap<u64, LodTransition>,
    pub completed: VecDeque<(u64, LodLevel)>,
}

impl LodTransitionManager {
    pub fn new() -> Self {
        Self {
            transitions: HashMap::new(),
            completed: VecDeque::with_capacity(64),
        }
    }

    pub fn begin_transition(&mut self, level_id: u64, from: LodLevel, to: LodLevel, duration_s: f32) {
        self.transitions.insert(level_id, LodTransition::new(level_id, from, to, duration_s));
    }

    pub fn update(&mut self, dt_s: f32) {
        let mut to_complete: Vec<u64> = Vec::new();
        for (id, t) in &mut self.transitions {
            if t.update_progress(dt_s) {
                to_complete.push(*id);
            }
        }
        for id in to_complete {
            if let Some(t) = self.transitions.remove(&id) {
                if self.completed.len() >= 64 { self.completed.pop_front(); }
                self.completed.push_back((id, t.to_lod));
            }
        }
    }

    pub fn get_blend_alpha(&self, level_id: u64) -> f32 {
        self.transitions.get(&level_id).map(|t| t.blend_alpha()).unwrap_or(1.0)
    }

    pub fn is_transitioning(&self, level_id: u64) -> bool {
        self.transitions.contains_key(&level_id)
    }

    pub fn active_count(&self) -> usize { self.transitions.len() }

    pub fn cancel_transition(&mut self, level_id: u64) {
        self.transitions.remove(&level_id);
    }
}

// ============================================================
// STREAMING FLOW OPTIMIZER
// ============================================================

pub struct StreamingFlowOptimizer;

impl StreamingFlowOptimizer {
    pub fn reorder_by_geography(requests: &mut Vec<LoadRequest>, levels: &HashMap<u64, StreamingLevel>) {
        requests.sort_by(|a, b| {
            let pos_a = levels.get(&a.level_id).map(|l| l.bounds.center()).unwrap_or(Vec3::ZERO);
            let pos_b = levels.get(&b.level_id).map(|l| l.bounds.center()).unwrap_or(Vec3::ZERO);
            pos_a.x.partial_cmp(&pos_b.x).unwrap_or(std::cmp::Ordering::Equal)
                .then(pos_a.z.partial_cmp(&pos_b.z).unwrap_or(std::cmp::Ordering::Equal))
        });
    }

    pub fn optimal_batch_size(bandwidth_mb_s: f32, average_level_mb: f32, target_latency_ms: f32) -> usize {
        if average_level_mb <= 0.0 || bandwidth_mb_s <= 0.0 { return 1; }
        let load_time_ms = average_level_mb / bandwidth_mb_s * 1000.0;
        let batch = (target_latency_ms / load_time_ms).ceil() as usize;
        batch.clamp(1, MAX_CONCURRENT_LOADS)
    }

    pub fn estimate_memory_after_loads(
        current_mb: f32,
        budget_mb: f32,
        loads: &[u64],
        levels: &HashMap<u64, StreamingLevel>,
    ) -> bool {
        let additional: f32 = loads.iter()
            .filter_map(|id| levels.get(id))
            .map(|l| l.memory_estimate_mb())
            .sum();
        current_mb + additional <= budget_mb
    }

    pub fn urgency_score(
        level: &StreamingLevel,
        camera_pos: Vec3,
        camera_vel: Vec3,
        time_to_load_ms: f32,
    ) -> f32 {
        let dist = level.bounds.distance_to_point(camera_pos);
        let speed = camera_vel.length();
        if speed < 0.1 { return 1.0 / dist.max(0.1); }
        let dir = camera_vel / speed;
        let to_level = (level.bounds.center() - camera_pos).normalize_or_zero();
        let dot = dir.dot(to_level).clamp(0.0, 1.0);
        let time_to_reach = dist / speed;
        let load_time_s = time_to_load_ms / 1000.0;
        if time_to_reach < load_time_s { 100.0 }
        else { dot * 10.0 / time_to_reach }
    }

    pub fn speed_adjusted_radius(base_radius: f32, speed_m_s: f32, load_latency_s: f32) -> f32 {
        base_radius + speed_m_s * load_latency_s * 1.5
    }

    pub fn compute_load_stagger_offset(
        index: usize,
        total: usize,
        max_bandwidth_mb_s: f32,
        level_size_mb: f32,
    ) -> f32 {
        if max_bandwidth_mb_s <= 0.0 || total == 0 { return 0.0; }
        let load_time = level_size_mb / max_bandwidth_mb_s;
        index as f32 * load_time / total as f32
    }

    pub fn streaming_load_factor(
        loading_levels: usize,
        max_concurrent: usize,
        queue_depth: usize,
    ) -> f32 {
        let in_flight_factor = loading_levels as f32 / max_concurrent.max(1) as f32;
        let queue_factor = (queue_depth as f32 / 16.0).min(1.0);
        (in_flight_factor * 0.7 + queue_factor * 0.3).clamp(0.0, 1.0)
    }
}

// ============================================================
// SECTOR WAYPOINT PATHFINDER
// ============================================================

#[derive(Debug)]
pub struct SectorWaypointPathfinder {
    pub adjacency: HashMap<u64, Vec<u64>>,
}

impl SectorWaypointPathfinder {
    pub fn new(sector_graph: &SectorGraph) -> Self {
        let mut adjacency = HashMap::new();
        for (id, sector) in &sector_graph.sectors {
            adjacency.insert(*id, sector.adjacent_sectors.clone());
        }
        Self { adjacency }
    }

    pub fn find_sector_path(&self, start: u64, end: u64) -> Option<Vec<u64>> {
        if start == end { return Some(vec![start]); }
        let mut visited: HashSet<u64> = HashSet::new();
        let mut queue: VecDeque<(u64, Vec<u64>)> = VecDeque::new();
        queue.push_back((start, vec![start]));
        visited.insert(start);
        while let Some((current, path)) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(&current) {
                for &next in neighbors {
                    if next == end {
                        let mut full_path = path.clone();
                        full_path.push(next);
                        return Some(full_path);
                    }
                    if !visited.contains(&next) {
                        visited.insert(next);
                        let mut new_path = path.clone();
                        new_path.push(next);
                        queue.push_back((next, new_path));
                    }
                }
            }
        }
        None
    }

    pub fn preload_levels_for_path(&self, path: &[u64], sector_graph: &SectorGraph) -> Vec<u64> {
        let mut levels = Vec::new();
        for &sector_id in path {
            if let Some(sector) = sector_graph.sectors.get(&sector_id) {
                for &lid in &sector.level_ids {
                    if !levels.contains(&lid) { levels.push(lid); }
                }
            }
        }
        levels
    }

    pub fn estimate_travel_time_s(
        &self,
        path: &[u64],
        sector_graph: &SectorGraph,
        speed_m_s: f32,
    ) -> f32 {
        if path.len() < 2 { return 0.0; }
        let mut total_dist = 0.0f32;
        for i in 0..(path.len() - 1) {
            let a = sector_graph.sectors.get(&path[i]).map(|s| s.bounds.center());
            let b = sector_graph.sectors.get(&path[i + 1]).map(|s| s.bounds.center());
            if let (Some(a), Some(b)) = (a, b) {
                total_dist += (b - a).length();
            }
        }
        if speed_m_s > 0.0 { total_dist / speed_m_s } else { f32::MAX }
    }

    pub fn reachable_sectors_within_distance(&self, start: u64, max_hops: usize) -> HashSet<u64> {
        let mut visited: HashSet<u64> = HashSet::new();
        let mut queue: VecDeque<(u64, usize)> = VecDeque::new();
        queue.push_back((start, 0));
        visited.insert(start);
        while let Some((id, depth)) = queue.pop_front() {
            if depth >= max_hops { continue; }
            if let Some(neighbors) = self.adjacency.get(&id) {
                for &next in neighbors {
                    if visited.insert(next) {
                        queue.push_back((next, depth + 1));
                    }
                }
            }
        }
        visited
    }
}

// ============================================================
// LEVEL ASSET CATALOGUE
// ============================================================

#[derive(Debug, Clone)]
pub struct LevelAssetEntry {
    pub asset: StreamingLevelAsset,
    pub tags: Vec<String>,
    pub last_used_frame: u64,
    pub load_count: u32,
    pub is_pinned: bool,
}

impl LevelAssetEntry {
    pub fn new(asset: StreamingLevelAsset) -> Self {
        Self {
            asset,
            tags: Vec::new(),
            last_used_frame: 0,
            load_count: 0,
            is_pinned: false,
        }
    }

    pub fn mark_used(&mut self, frame: u64) {
        self.last_used_frame = frame;
        self.load_count += 1;
    }

    pub fn size_mb(&self) -> f32 {
        self.asset.size_bytes as f32 / (1024.0 * 1024.0)
    }
}

#[derive(Debug)]
pub struct LevelAssetCatalogue {
    pub entries: HashMap<u64, LevelAssetEntry>,
    pub total_size_bytes: u64,
    pub tags_index: HashMap<String, Vec<u64>>,
}

impl LevelAssetCatalogue {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            total_size_bytes: 0,
            tags_index: HashMap::new(),
        }
    }

    pub fn register(&mut self, asset: StreamingLevelAsset) {
        let id = asset.id;
        let size = asset.size_bytes;
        self.total_size_bytes += size;
        self.entries.insert(id, LevelAssetEntry::new(asset));
    }

    pub fn tag_asset(&mut self, asset_id: u64, tag: &str) {
        if let Some(entry) = self.entries.get_mut(&asset_id) {
            if !entry.tags.contains(&tag.to_string()) {
                entry.tags.push(tag.to_string());
            }
        }
        self.tags_index.entry(tag.to_string()).or_default().push(asset_id);
    }

    pub fn assets_by_tag(&self, tag: &str) -> Vec<&LevelAssetEntry> {
        self.tags_index.get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.entries.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn largest_assets(&self, n: usize) -> Vec<&LevelAssetEntry> {
        let mut sorted: Vec<&LevelAssetEntry> = self.entries.values().collect();
        sorted.sort_by(|a, b| b.asset.size_bytes.cmp(&a.asset.size_bytes));
        sorted.into_iter().take(n).collect()
    }

    pub fn pin_asset(&mut self, id: u64) {
        if let Some(e) = self.entries.get_mut(&id) { e.is_pinned = true; }
    }

    pub fn unpin_asset(&mut self, id: u64) {
        if let Some(e) = self.entries.get_mut(&id) { e.is_pinned = false; }
    }

    pub fn total_size_mb(&self) -> f32 {
        self.total_size_bytes as f32 / (1024.0 * 1024.0)
    }

    pub fn unpinned_assets_sorted_by_lru(&self, current_frame: u64) -> Vec<u64> {
        let mut sorted: Vec<&LevelAssetEntry> = self.entries.values()
            .filter(|e| !e.is_pinned)
            .collect();
        sorted.sort_by_key(|e| e.last_used_frame);
        sorted.iter().map(|e| e.asset.id).collect()
    }
}

// ============================================================
// STREAMING WORLD COMPOSER
// ============================================================

#[derive(Debug)]
pub struct StreamingWorldComposer {
    pub world_name: String,
    pub base_level_ids: Vec<u64>,
    pub layer_groups: HashMap<String, Vec<u64>>,
    pub streaming_sets: HashMap<String, Vec<u64>>,
    pub world_bounds: Aabb,
    pub camera_start: Vec3,
    pub camera_start_dir: Vec3,
    pub description: String,
}

impl StreamingWorldComposer {
    pub fn new(world_name: String) -> Self {
        Self {
            world_name,
            base_level_ids: Vec::new(),
            layer_groups: HashMap::new(),
            streaming_sets: HashMap::new(),
            world_bounds: Aabb::new(-Vec3::splat(10000.0), Vec3::splat(10000.0)),
            camera_start: Vec3::ZERO,
            camera_start_dir: Vec3::NEG_Z,
            description: String::new(),
        }
    }

    pub fn add_to_layer(&mut self, layer: &str, level_id: u64) {
        self.layer_groups.entry(layer.to_string()).or_default().push(level_id);
    }

    pub fn create_streaming_set(&mut self, set_name: &str, level_ids: Vec<u64>) {
        self.streaming_sets.insert(set_name.to_string(), level_ids);
    }

    pub fn get_streaming_set(&self, set_name: &str) -> Vec<u64> {
        self.streaming_sets.get(set_name).cloned().unwrap_or_default()
    }

    pub fn levels_in_layer(&self, layer: &str) -> Vec<u64> {
        self.layer_groups.get(layer).cloned().unwrap_or_default()
    }

    pub fn all_managed_level_ids(&self) -> Vec<u64> {
        let mut result = self.base_level_ids.clone();
        for ids in self.layer_groups.values() {
            for &id in ids {
                if !result.contains(&id) { result.push(id); }
            }
        }
        result
    }

    pub fn layer_names(&self) -> Vec<&str> {
        self.layer_groups.keys().map(|s| s.as_str()).collect()
    }

    pub fn set_world_bounds_from_levels(&mut self, levels: &[StreamingLevel]) {
        let managed = self.all_managed_level_ids();
        let mut merged = Aabb::new(Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for l in levels {
            if managed.contains(&l.id) {
                merged = merged.merge(&l.bounds);
            }
        }
        if merged.min.x <= merged.max.x {
            self.world_bounds = merged;
        }
    }

    pub fn count_layers(&self) -> usize { self.layer_groups.len() }
}

// ============================================================
// UTILITY FUNCTIONS
// ============================================================

pub fn compute_cell_priority_scores(
    cells: &mut HashMap<CellCoord, WorldCell>,
    camera_pos: Vec3,
    camera_dir: Vec3,
    budget_pressure: f32,
) {
    for cell in cells.values_mut() {
        let _ = cell.compute_priority_score(camera_pos, camera_dir);
        if budget_pressure > 0.8 {
            cell.load_priority_score *= 1.0 - (budget_pressure - 0.8) * 2.0;
        }
    }
}

pub fn cells_by_priority(cells: &HashMap<CellCoord, WorldCell>, top_n: usize) -> Vec<CellCoord> {
    let mut sorted: Vec<(&CellCoord, f32)> = cells.iter()
        .map(|(k, v)| (k, v.load_priority_score))
        .collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.into_iter().take(top_n).map(|(k, _)| *k).collect()
}

pub fn estimate_level_load_time_ms(
    size_bytes: u64,
    bandwidth_mb_s: f32,
    decompression_factor: f32,
) -> f32 {
    if bandwidth_mb_s <= 0.0 { return f32::MAX; }
    let mb = size_bytes as f32 / (1024.0 * 1024.0);
    let io_time_ms = mb / bandwidth_mb_s * 1000.0;
    let decomp_time_ms = mb * decompression_factor;
    io_time_ms + decomp_time_ms
}

pub fn lru_eviction_order(
    levels: &[StreamingLevel],
    lru_order: &VecDeque<u64>,
) -> Vec<u64> {
    let mut result = Vec::new();
    for &id in lru_order.iter() {
        if let Some(l) = levels.iter().find(|l| l.id == id) {
            if l.state == StreamingState::Loaded && l.persistence != LevelPersistence::AlwaysLoaded {
                result.push(id);
            }
        }
    }
    result
}

pub fn should_stream_via_portal(
    portal: &Portal,
    camera_pos: Vec3,
    max_portal_stream_dist: f32,
) -> bool {
    let dist = (portal.center - camera_pos).length();
    portal.is_open && dist < max_portal_stream_dist
}

pub fn occlusion_cull_sectors(
    sectors: &[u64],
    visible_pvs: &HashSet<u64>,
) -> (Vec<u64>, Vec<u64>) {
    let mut visible = Vec::new();
    let mut culled = Vec::new();
    for &id in sectors {
        if visible_pvs.contains(&id) { visible.push(id); } else { culled.push(id); }
    }
    (visible, culled)
}

pub fn compute_portal_screen_coverage(portal: &Portal, camera_pos: Vec3, fov_y: f32) -> f32 {
    let dist = (portal.center - camera_pos).length().max(0.01);
    let angular_h = 2.0 * (portal.half_extents.x / dist).atan();
    let angular_v = 2.0 * (portal.half_extents.y / dist).atan();
    (angular_h / fov_y).min(1.0) * (angular_v / fov_y).min(1.0)
}

pub fn compute_streaming_jitter(load_times: &[f32]) -> f32 {
    if load_times.len() < 2 { return 0.0; }
    let mean = load_times.iter().sum::<f32>() / load_times.len() as f32;
    let var = load_times.iter().map(|&t| (t - mean).powi(2)).sum::<f32>() / load_times.len() as f32;
    var.sqrt()
}

pub fn priority_weighted_sort(requests: &mut Vec<LoadRequest>) {
    requests.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap_or(std::cmp::Ordering::Equal));
}

pub fn build_adjacency_matrix(sectors: &HashMap<u64, Sector>) -> HashMap<(u64, u64), f32> {
    let mut matrix = HashMap::new();
    for (id, sector) in sectors {
        for &adj_id in &sector.adjacent_sectors {
            let a = sector.bounds.center();
            let b = sectors.get(&adj_id).map(|s| s.bounds.center()).unwrap_or(Vec3::ZERO);
            matrix.insert((*id, adj_id), (b - a).length());
        }
    }
    matrix
}

pub fn level_memory_breakdown(levels: &[StreamingLevel]) -> HashMap<LodLevel, f32> {
    let mut breakdown: HashMap<LodLevel, f32> = HashMap::new();
    for level in levels {
        if level.state == StreamingState::Loaded {
            *breakdown.entry(level.current_lod).or_insert(0.0) += level.memory_footprint_mb;
        }
    }
    breakdown
}

pub fn sector_coverage_area(sector: &Sector) -> f32 {
    let s = sector.bounds.size();
    s.x * s.z
}

pub fn streaming_priority_from_coverage(coverage_ratio: f32, base_priority: LoadPriority) -> LoadPriority {
    if coverage_ratio > 0.25 { LoadPriority::Critical }
    else if coverage_ratio > 0.1 { LoadPriority::High }
    else if coverage_ratio > 0.01 { LoadPriority::Medium }
    else { base_priority }
}

pub fn sector_transition_fade_curve(progress: f32, transition: SectorTransitionType) -> f32 {
    match transition {
        SectorTransitionType::Immediate => 1.0,
        SectorTransitionType::Fade => {
            if progress < 0.5 { progress * 2.0 } else { (1.0 - progress) * 2.0 }
        }
        SectorTransitionType::Portal => { let t = progress; t * t * (3.0 - 2.0 * t) }
        SectorTransitionType::Teleport => { if progress < 0.1 || progress > 0.9 { 0.0 } else { 1.0 } }
    }
}

// CellCoord ordering for dedup
impl PartialOrd for CellCoord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl Ord for CellCoord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.x.cmp(&other.x).then(self.y.cmp(&other.y)).then(self.z.cmp(&other.z))
    }
}

pub fn cells_to_activate(
    camera_pos: Vec3,
    camera_vel: Vec3,
    cell_size: f32,
    base_radius: f32,
    lookahead_s: f32,
    grid_origin: Vec3,
) -> Vec<CellCoord> {
    let predicted = camera_pos + camera_vel * lookahead_s;
    let world_to_coord = |p: Vec3| {
        let rel = p - grid_origin;
        CellCoord::new((rel.x / cell_size).floor() as i32, (rel.z / cell_size).floor() as i32, (rel.y / cell_size).floor() as i32)
    };
    let center = world_to_coord(camera_pos);
    let pred = world_to_coord(predicted);
    let cell_r = (base_radius / cell_size).ceil() as i32 + 1;
    let mut coords = Vec::new();
    for &base in &[center, pred] {
        for dz in -cell_r..=cell_r {
            for dx in -cell_r..=cell_r {
                coords.push(CellCoord::new(base.x + dx, base.y, base.z + dz));
            }
        }
    }
    coords.sort();
    coords.dedup();
    coords
}

// ============================================================
// FULL STREAMING WORLD MANAGER
// ============================================================

#[derive(Debug)]
pub struct StreamingWorldManager {
    pub editor: FullLevelStreamingEditor,
    pub dynamic_tracker: DynamicObjectTracker,
    pub composer: StreamingWorldComposer,
    pub roi_manager: RegionOfInterestManager,
    pub checkpoint_mgr: CheckpointManager,
    pub lod_transitions: LodTransitionManager,
    pub analytics: StreamingAnalyticsSession,
    pub asset_catalogue: LevelAssetCatalogue,
    pub command_history: CommandHistory,
    pub distance_cache: StreamingDistanceCache,
    pub settings_panel: LevelStreamingSettingsPanel,
    pub pathfinder: Option<SectorWaypointPathfinder>,
    pub flow_state: WorldFlowState,
}

#[derive(Debug, Default, Clone)]
pub struct WorldFlowState {
    pub is_loading_world: bool,
    pub load_progress: f32,
    pub current_phase: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub is_simulation_mode: bool,
    pub last_checkpoint_id: Option<u64>,
}

impl StreamingWorldManager {
    pub fn new(world_name: String) -> Self {
        let config = LevelStreamingEditorConfig::default();
        let settings = LevelStreamingSettingsPanel::new(&config);
        let editor = FullLevelStreamingEditor::new(config);
        Self {
            editor,
            dynamic_tracker: DynamicObjectTracker::new(DEFAULT_CELL_SIZE),
            composer: StreamingWorldComposer::new(world_name),
            roi_manager: RegionOfInterestManager::new(),
            checkpoint_mgr: CheckpointManager::new(),
            lod_transitions: LodTransitionManager::new(),
            analytics: StreamingAnalyticsSession::new(1, 0.0),
            asset_catalogue: LevelAssetCatalogue::new(),
            command_history: CommandHistory::new(128),
            distance_cache: StreamingDistanceCache::new(),
            settings_panel: settings,
            pathfinder: None,
            flow_state: WorldFlowState::default(),
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        let cam_pos = self.editor.core.camera_position;
        let cam_dir = self.editor.core.camera_direction;
        let time_ms = self.editor.core.current_time_ms;

        self.editor.tick(dt_s);
        self.roi_manager.update(cam_pos, time_ms);
        self.lod_transitions.update(dt_s);

        let levels_vec: Vec<StreamingLevel> = self.editor.core.levels.values().cloned().collect();
        let memory_mb = self.editor.core.memory_manager.used_mb;
        self.analytics.update_peak_memory(memory_mb);

        let _ = self.checkpoint_mgr.maybe_auto_checkpoint(cam_pos, cam_dir, &levels_vec, memory_mb, time_ms);
        self.distance_cache.update(cam_pos, &levels_vec, self.editor.core.current_frame);

        if self.settings_panel.has_unsaved_changes() {
            self.settings_panel.apply_to_config(&mut self.editor.core.config);
            self.editor.core.set_memory_budget(self.editor.core.config.memory_budget_mb);
        }

        let completed: Vec<(u64, LodLevel)> = self.lod_transitions.completed.drain(..).collect();
        for (level_id, new_lod) in completed {
            if let Some(level) = self.editor.core.levels.get_mut(&level_id) {
                level.current_lod = new_lod;
            }
        }

        if self.pathfinder.is_none() && !self.editor.core.sector_graph.sectors.is_empty() {
            self.pathfinder = Some(SectorWaypointPathfinder::new(&self.editor.core.sector_graph));
        }
    }

    pub fn do_command(&mut self, cmd: StreamingEditorCommand) {
        apply_streaming_command(&mut self.editor, &cmd);
        self.command_history.push(cmd);
    }

    pub fn undo(&mut self) {
        if let Some(cmd) = self.command_history.undo() {
            undo_streaming_command(&mut self.editor, &cmd);
        }
    }

    pub fn redo(&mut self) {
        if let Some(cmd) = self.command_history.redo() {
            apply_streaming_command(&mut self.editor, &cmd);
        }
    }

    pub fn save_checkpoint(&mut self) -> Option<u64> {
        let cam_pos = self.editor.core.camera_position;
        let cam_dir = self.editor.core.camera_direction;
        let levels_vec: Vec<StreamingLevel> = self.editor.core.levels.values().cloned().collect();
        let memory_mb = self.editor.core.memory_manager.used_mb;
        let time_ms = self.editor.core.current_time_ms;
        let id = self.checkpoint_mgr.save("Manual".into(), cam_pos, cam_dir, &levels_vec, memory_mb, time_ms);
        self.flow_state.last_checkpoint_id = Some(id);
        Some(id)
    }

    pub fn find_path_to_sector(&self, from_sector: u64, to_sector: u64) -> Option<Vec<u64>> {
        self.pathfinder.as_ref()?.find_sector_path(from_sector, to_sector)
    }

    pub fn world_report(&self) -> WorldStreamingReport {
        let core_report = self.editor.core.get_streaming_report();
        WorldStreamingReport {
            core: core_report,
            dynamic_objects: self.dynamic_tracker.total_objects(),
            active_roi_count: self.roi_manager.active_regions.len(),
            lod_transitions_active: self.lod_transitions.active_count(),
            analytics_total_loads: self.analytics.total_loads,
            analytics_bandwidth_mb_s: self.analytics.average_bandwidth_mb_s(),
            asset_catalogue_mb: self.asset_catalogue.total_size_mb(),
            has_unsaved_settings: self.settings_panel.has_unsaved_changes(),
            can_undo: self.command_history.can_undo(),
            can_redo: self.command_history.can_redo(),
        }
    }

    pub fn spawn_dynamic_object(&mut self, name: String, pos: Vec3, radius: f32) -> u64 {
        self.dynamic_tracker.spawn(name, pos, radius)
    }

    pub fn update_dynamic_object(&mut self, id: u64, new_pos: Vec3, dt_s: f32) {
        let time_ms = self.editor.core.current_time_ms;
        self.dynamic_tracker.update_position(id, new_pos, dt_s, time_ms);
    }

    pub fn get_levels_to_stream_for_object(&self, object_id: u64, extra_radius: f32) -> Vec<u64> {
        if let Some(obj) = self.dynamic_tracker.objects.get(&object_id) {
            return self.editor.core.query_levels_near(obj.position, obj.bounds_radius + extra_radius);
        }
        Vec::new()
    }

    pub fn set_simulation_speed(&mut self, speed: f32) {
        self.editor.core.simulator.playback_speed = speed.max(0.0);
        self.flow_state.is_simulation_mode = speed > 0.0 && self.editor.core.simulator.is_running;
    }

    pub fn full_reset(&mut self) {
        self.editor.core.reset_simulation();
        self.analytics = StreamingAnalyticsSession::new(self.analytics.session_id + 1, self.editor.core.current_time_ms);
        self.flow_state = WorldFlowState::default();
    }
}

#[derive(Debug, Clone)]
pub struct WorldStreamingReport {
    pub core: StreamingReport,
    pub dynamic_objects: usize,
    pub active_roi_count: usize,
    pub lod_transitions_active: usize,
    pub analytics_total_loads: u32,
    pub analytics_bandwidth_mb_s: f32,
    pub asset_catalogue_mb: f32,
    pub has_unsaved_settings: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

// ============================================================
// EXTENDED TESTS
// ============================================================

#[cfg(test)]
mod extended_tests {
    use super::*;

    #[test]
    fn test_dynamic_object_tracker() {
        let mut tracker = DynamicObjectTracker::new(512.0);
        let id = tracker.spawn("Npc1".into(), Vec3::ZERO, 1.0);
        assert!(tracker.objects.contains_key(&id));
        tracker.update_position(id, Vec3::new(600.0, 0.0, 0.0), 1.0, 1000.0);
        let obj = &tracker.objects[&id];
        assert_ne!(obj.current_cell, CellCoord::new(0, 0, 0));
    }

    #[test]
    fn test_streaming_world_manager_tick() {
        let mut mgr = StreamingWorldManager::new("TestWorld".into());
        mgr.editor.core.update_camera(Vec3::new(100.0, 0.0, 100.0), Vec3::NEG_Z, Mat4::IDENTITY);
        mgr.tick(0.016);
        let report = mgr.world_report();
        assert_eq!(report.core.total_levels, 0);
    }

    #[test]
    fn test_distance_cache() {
        let mut cache = StreamingDistanceCache::new();
        let asset = StreamingLevelAsset { id:1, name:"a".into(), file_path:"".into(),
            size_bytes:0, uncompressed_size_bytes:0, dependencies:vec![], load_time_estimate_ms:0.0 };
        let level = StreamingLevel::new(1,"a".into(),asset,Aabb::new(Vec3::new(100.,0.,0.),Vec3::new(200.,10.,10.)));
        cache.update(Vec3::ZERO, &[level], 1);
        assert!(cache.get(1).is_some());
        let dist = cache.get(1).unwrap();
        assert!((dist - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_lod_transition_smooth_step() {
        let mut t = LodTransition::new(1, LodLevel::Lod0, LodLevel::Lod1, 1.0);
        t.progress = 0.5;
        let alpha = t.blend_alpha();
        // smooth step at 0.5 = 0.5
        assert!((alpha - 0.5).abs() < 0.01);
        t.progress = 0.0;
        assert!((t.blend_alpha() - 0.0).abs() < 0.01);
        t.progress = 1.0;
        assert!((t.blend_alpha() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_analytics_session() {
        let mut session = StreamingAnalyticsSession::new(1, 0.0);
        session.record_load(1, 250.0, 50.0);
        session.record_load(2, 150.0, 30.0);
        assert_eq!(session.total_loads, 2);
        assert!((session.average_load_latency_ms - 200.0).abs() < 1.0);
        assert_eq!(session.unique_levels_loaded.len(), 2);
    }

    #[test]
    fn test_region_of_interest() {
        let mut mgr = RegionOfInterestManager::new();
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(100.0));
        let id = mgr.add_region("Combat".into(), bounds, LoadPriority::High);
        mgr.update(Vec3::new(50.0, 0.0, 50.0), 0.0);
        assert!(mgr.active_regions.contains(&id));
        mgr.update(Vec3::new(500.0, 0.0, 500.0), 100.0);
        assert!(!mgr.active_regions.contains(&id));
    }

    #[test]
    fn test_checkpoint_save_restore() {
        let mut mgr = CheckpointManager::new();
        let id = mgr.save("Test".into(), Vec3::ZERO, Vec3::NEG_Z, &[], 128.0, 1000.0);
        let cp = mgr.checkpoints.get(&id).unwrap();
        assert_eq!(cp.memory_used_mb, 128.0);
        assert_eq!(cp.loaded_level_ids.len(), 0);
    }

    #[test]
    fn test_sector_pathfinding() {
        let mut sg = SectorGraph::new();
        sg.add_sector(Sector::new(1,"A".into(),Aabb::new(Vec3::ZERO,Vec3::splat(10.))));
        sg.add_sector(Sector::new(2,"B".into(),Aabb::new(Vec3::splat(10.),Vec3::splat(20.))));
        sg.add_sector(Sector::new(3,"C".into(),Aabb::new(Vec3::splat(20.),Vec3::splat(30.))));
        sg.add_portal(Portal::new(1,1,2,Vec3::new(10.,5.,5.),Vec3::X,Vec2::splat(2.)));
        sg.add_portal(Portal::new(2,2,3,Vec3::new(20.,5.,5.),Vec3::X,Vec2::splat(2.)));
        let pf = SectorWaypointPathfinder::new(&sg);
        let path = pf.find_sector_path(1,3).unwrap();
        assert_eq!(path, vec![1,2,3]);
    }

    #[test]
    fn test_asset_catalogue() {
        let mut cat = LevelAssetCatalogue::new();
        let asset = StreamingLevelAsset { id:1, name:"Forest".into(), file_path:"".into(),
            size_bytes:1024*1024, uncompressed_size_bytes:0, dependencies:vec![], load_time_estimate_ms:200.0 };
        cat.register(asset);
        cat.tag_asset(1,"outdoor");
        assert_eq!(cat.assets_by_tag("outdoor").len(), 1);
        assert!((cat.total_size_mb() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_cells_to_activate_velocity() {
        let coords = cells_to_activate(
            Vec3::ZERO, Vec3::new(20.0,0.0,0.0), 512.0, 256.0, 2.0, Vec3::ZERO
        );
        assert!(!coords.is_empty());
        // All coords should be unique (dedup)
        let mut sorted = coords.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), coords.len());
    }

    #[test]
    fn test_streaming_flow_optimizer_batch_size() {
        let batch = StreamingFlowOptimizer::optimal_batch_size(100.0, 25.0, 500.0);
        assert!(batch >= 1 && batch <= MAX_CONCURRENT_LOADS);
    }

    #[test]
    fn test_world_composer_layers() {
        let mut composer = StreamingWorldComposer::new("World".into());
        composer.add_to_layer("terrain", 1);
        composer.add_to_layer("terrain", 2);
        composer.add_to_layer("buildings", 3);
        assert_eq!(composer.levels_in_layer("terrain").len(), 2);
        assert_eq!(composer.all_managed_level_ids().len(), 3);
        assert_eq!(composer.count_layers(), 2);
    }

    #[test]
    fn test_lod_analysis() {
        let asset = StreamingLevelAsset { id:1, name:"T".into(), file_path:"".into(),
            size_bytes:10*1024*1024, uncompressed_size_bytes:0, dependencies:vec![], load_time_estimate_ms:0.0 };
        let mut level = StreamingLevel::new(1,"T".into(),asset,Aabb::new(Vec3::ZERO,Vec3::splat(100.)));
        level.distance_to_camera = 50.0;
        level.current_lod = LodLevel::Lod0;
        let mgr = CombinedBudgetManager::new(1000.0);
        let analysis = analyze_lod_distribution(&[level.clone()], &mgr);
        assert_eq!(analysis.len(), 1);
        assert_eq!(analysis[0].current_lod, LodLevel::Lod0);
    }

    #[test]
    fn test_streaming_importance() {
        let asset = StreamingLevelAsset { id:1, name:"T".into(), file_path:"".into(),
            size_bytes:0, uncompressed_size_bytes:0, dependencies:vec![], load_time_estimate_ms:0.0 };
        let level = StreamingLevel::new(1,"T".into(),asset,Aabb::new(Vec3::new(50.,0.,0.),Vec3::new(150.,50.,50.)));
        let importance = compute_streaming_importance(
            &level, Vec3::ZERO, Vec3::X, 5.0
        );
        assert!(importance.is_finite() && importance > 0.0);
    }

    #[test]
    fn test_hysteresis() {
        assert!(hysteresis_check_load(80.0, 100.0, 10.0));
        assert!(!hysteresis_check_load(95.0, 100.0, 10.0));
        assert!(hysteresis_check_unload(125.0, 110.0, 10.0));
        assert!(!hysteresis_check_unload(115.0, 110.0, 10.0));
    }
}

// ============================================================
// SECTION: Streaming Tile Map (2D overhead layout)
// ============================================================

#[derive(Clone, Debug)]
pub struct StreamingTile {
    pub tile_x: i32,
    pub tile_y: i32,
    pub tile_size_world: f32,
    pub level_ids: Vec<u64>,
    pub terrain_height_min: f32,
    pub terrain_height_max: f32,
    pub is_water: bool,
    pub biome_id: u32,
    pub detail_density: f32,
    pub last_visited_time: f64,
}

impl StreamingTile {
    pub fn new(tile_x: i32, tile_y: i32, tile_size: f32) -> Self {
        Self {
            tile_x,
            tile_y,
            tile_size_world: tile_size,
            level_ids: Vec::new(),
            terrain_height_min: 0.0,
            terrain_height_max: 100.0,
            is_water: false,
            biome_id: 0,
            detail_density: 1.0,
            last_visited_time: 0.0,
        }
    }

    pub fn world_center(&self) -> Vec3 {
        Vec3::new(
            (self.tile_x as f32 + 0.5) * self.tile_size_world,
            (self.terrain_height_min + self.terrain_height_max) * 0.5,
            (self.tile_y as f32 + 0.5) * self.tile_size_world,
        )
    }

    pub fn world_bounds(&self) -> Aabb {
        let min = Vec3::new(
            self.tile_x as f32 * self.tile_size_world,
            self.terrain_height_min,
            self.tile_y as f32 * self.tile_size_world,
        );
        let max = Vec3::new(
            (self.tile_x + 1) as f32 * self.tile_size_world,
            self.terrain_height_max,
            (self.tile_y + 1) as f32 * self.tile_size_world,
        );
        Aabb::new(min, max)
    }

    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        let center = self.world_center();
        let half = self.tile_size_world * 0.5;
        let dx = (center.x - point.x).abs() - half;
        let dz = (center.z - point.z).abs() - half;
        (dx.max(0.0) * dx.max(0.0) + dz.max(0.0) * dz.max(0.0)).sqrt()
    }

    pub fn contains_point_2d(&self, x: f32, z: f32) -> bool {
        let min_x = self.tile_x as f32 * self.tile_size_world;
        let min_z = self.tile_y as f32 * self.tile_size_world;
        let max_x = min_x + self.tile_size_world;
        let max_z = min_z + self.tile_size_world;
        x >= min_x && x < max_x && z >= min_z && z < max_z
    }
}

#[derive(Clone, Debug)]
pub struct StreamingTileMap {
    pub tile_size_world: f32,
    pub tiles: HashMap<(i32, i32), StreamingTile>,
    pub world_origin: Vec3,
    pub max_tiles: usize,
}

impl StreamingTileMap {
    pub fn new(tile_size_world: f32) -> Self {
        Self {
            tile_size_world,
            tiles: HashMap::new(),
            world_origin: Vec3::ZERO,
            max_tiles: 1024,
        }
    }

    pub fn world_to_tile(&self, world_pos: Vec3) -> (i32, i32) {
        let tx = ((world_pos.x - self.world_origin.x) / self.tile_size_world).floor() as i32;
        let tz = ((world_pos.z - self.world_origin.z) / self.tile_size_world).floor() as i32;
        (tx, tz)
    }

    pub fn get_or_create(&mut self, tile_x: i32, tile_y: i32) -> &mut StreamingTile {
        self.tiles.entry((tile_x, tile_y)).or_insert_with(|| {
            StreamingTile::new(tile_x, tile_y, self.tile_size_world)
        })
    }

    pub fn get(&self, tile_x: i32, tile_y: i32) -> Option<&StreamingTile> {
        self.tiles.get(&(tile_x, tile_y))
    }

    pub fn tiles_in_radius(&self, center: Vec3, radius: f32) -> Vec<(i32, i32)> {
        let tile_radius = (radius / self.tile_size_world).ceil() as i32 + 1;
        let (cx, cz) = self.world_to_tile(center);
        let mut result = Vec::new();
        for tx in (cx - tile_radius)..=(cx + tile_radius) {
            for tz in (cz - tile_radius)..=(cz + tile_radius) {
                if let Some(tile) = self.tiles.get(&(tx, tz)) {
                    if tile.distance_to_point(center) <= radius {
                        result.push((tx, tz));
                    }
                } else {
                    // Check distance from center to potential tile
                    let half = self.tile_size_world * 0.5;
                    let tc_x = (tx as f32 + 0.5) * self.tile_size_world;
                    let tc_z = (tz as f32 + 0.5) * self.tile_size_world;
                    let dx = (center.x - tc_x).abs() - half;
                    let dz = (center.z - tc_z).abs() - half;
                    let dist = (dx.max(0.0).powi(2) + dz.max(0.0).powi(2)).sqrt();
                    if dist <= radius {
                        result.push((tx, tz));
                    }
                }
            }
        }
        result
    }

    pub fn tiles_in_frustum(&self, frustum: &Frustum) -> Vec<(i32, i32)> {
        self.tiles.iter()
            .filter(|(_, tile)| frustum.test_aabb(&tile.world_bounds()))
            .map(|(&k, _)| k)
            .collect()
    }

    pub fn add_level_to_tile(&mut self, tile_x: i32, tile_y: i32, level_id: u64) {
        let tile = self.get_or_create(tile_x, tile_y);
        if !tile.level_ids.contains(&level_id) {
            tile.level_ids.push(level_id);
        }
    }

    pub fn remove_level_from_all(&mut self, level_id: u64) {
        for tile in self.tiles.values_mut() {
            tile.level_ids.retain(|&id| id != level_id);
        }
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn stale_tiles(&self, current_time: f64, max_age_seconds: f64) -> Vec<(i32, i32)> {
        self.tiles.iter()
            .filter(|(_, t)| current_time - t.last_visited_time > max_age_seconds)
            .map(|(&k, _)| k)
            .collect()
    }

    pub fn evict_stale(&mut self, current_time: f64, max_age_seconds: f64) -> usize {
        let stale = self.stale_tiles(current_time, max_age_seconds);
        let count = stale.len();
        for key in stale {
            self.tiles.remove(&key);
        }
        count
    }
}

// ============================================================
// SECTION: Level Instancer (manages multiple placed instances)
// ============================================================

#[derive(Clone, Debug)]
pub struct LevelInstancer {
    pub instances: HashMap<u64, LevelInstance>,
    pub next_instance_id: u64,
    pub spatial_index: HashMap<(i32, i32), Vec<u64>>, // tile -> instance ids
    pub tile_size: f32,
}

impl LevelInstancer {
    pub fn new(tile_size: f32) -> Self {
        Self {
            instances: HashMap::new(),
            next_instance_id: 1,
            spatial_index: HashMap::new(),
            tile_size,
        }
    }

    pub fn place_instance(&mut self, level_id: u64, transform: Mat4) -> u64 {
        let id = self.next_instance_id;
        self.next_instance_id += 1;
        let instance = LevelInstance::new(id, level_id, transform);
        let tile_key = self.world_to_tile(instance.position());
        self.spatial_index.entry(tile_key).or_insert_with(Vec::new).push(id);
        self.instances.insert(id, instance);
        id
    }

    fn world_to_tile(&self, pos: Vec3) -> (i32, i32) {
        ((pos.x / self.tile_size).floor() as i32,
         (pos.z / self.tile_size).floor() as i32)
    }

    pub fn remove_instance(&mut self, id: u64) -> Option<LevelInstance> {
        if let Some(inst) = self.instances.remove(&id) {
            let tile_key = self.world_to_tile(inst.position());
            if let Some(list) = self.spatial_index.get_mut(&tile_key) {
                list.retain(|&i| i != id);
            }
            Some(inst)
        } else {
            None
        }
    }

    pub fn instances_in_radius(&self, center: Vec3, radius: f32) -> Vec<u64> {
        let tile_r = (radius / self.tile_size).ceil() as i32 + 1;
        let (cx, cz) = self.world_to_tile(center);
        let mut result = Vec::new();
        for tx in (cx - tile_r)..=(cx + tile_r) {
            for tz in (cz - tile_r)..=(cz + tile_r) {
                if let Some(ids) = self.spatial_index.get(&(tx, tz)) {
                    for &id in ids {
                        if let Some(inst) = self.instances.get(&id) {
                            let dist = (inst.position() - center).length();
                            if dist <= radius {
                                result.push(id);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    pub fn instances_with_tag(&self, tag: &str) -> Vec<u64> {
        self.instances.values()
            .filter(|i| i.has_tag(tag))
            .map(|i| i.instance_id)
            .collect()
    }

    pub fn instances_for_level(&self, level_id: u64) -> Vec<u64> {
        self.instances.values()
            .filter(|i| i.level_id == level_id)
            .map(|i| i.instance_id)
            .collect()
    }

    pub fn update_transform(&mut self, id: u64, new_transform: Mat4) {
        let old_pos = self.instances.get(&id).map(|i| i.position());
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.transform = new_transform;
            inst.last_modified_time += 0.001;
        }
        if let Some(old_pos) = old_pos {
            let old_tile = self.world_to_tile(old_pos);
            let new_pos = self.instances.get(&id).map(|i| i.position()).unwrap_or(old_pos);
            let new_tile = self.world_to_tile(new_pos);
            if old_tile != new_tile {
                if let Some(list) = self.spatial_index.get_mut(&old_tile) {
                    list.retain(|&i| i != id);
                }
                self.spatial_index.entry(new_tile).or_insert_with(Vec::new).push(id);
            }
        }
    }

    pub fn count_by_level(&self) -> HashMap<u64, usize> {
        let mut counts: HashMap<u64, usize> = HashMap::new();
        for inst in self.instances.values() {
            *counts.entry(inst.level_id).or_insert(0) += 1;
        }
        counts
    }

    pub fn visible_instances_in_frustum(&self, frustum: &Frustum, level_bounds: &HashMap<u64, Aabb>) -> Vec<u64> {
        self.instances.values()
            .filter(|inst| {
                if !inst.visible { return false; }
                if let Some(bounds) = level_bounds.get(&inst.level_id) {
                    // Transform bounds by instance transform (approximate AABB)
                    let pos = inst.position();
                    let translated = Aabb::new(bounds.min + pos, bounds.max + pos);
                    frustum.test_aabb(&translated)
                } else {
                    true
                }
            })
            .map(|i| i.instance_id)
            .collect()
    }
}

// ============================================================
// SECTION: Terrain Patch LOD System
// ============================================================

#[derive(Clone, Debug)]
pub struct TerrainPatch {
    pub patch_id: u32,
    pub grid_x: i32,
    pub grid_z: i32,
    pub patch_size: f32,
    pub current_lod: u32,
    pub max_lod: u32,
    pub height_data: Vec<f32>, // flattened height grid
    pub height_grid_res: u32,  // resolution per side
    pub vertex_count: u32,
    pub is_stitched: bool,
    pub neighbor_lods: [u32; 4], // N, S, E, W
    pub morph_fraction: f32,     // for smooth LOD transitions
}

impl TerrainPatch {
    pub fn new(patch_id: u32, grid_x: i32, grid_z: i32, patch_size: f32, max_lod: u32) -> Self {
        let base_res = 64u32;
        let res = base_res;
        let height_data = vec![0.0f32; (res * res) as usize];
        Self {
            patch_id,
            grid_x,
            grid_z,
            patch_size,
            current_lod: 0,
            max_lod,
            height_data,
            height_grid_res: res,
            vertex_count: res * res,
            is_stitched: false,
            neighbor_lods: [0; 4],
            morph_fraction: 0.0,
        }
    }

    pub fn world_position(&self) -> Vec3 {
        Vec3::new(
            self.grid_x as f32 * self.patch_size,
            0.0,
            self.grid_z as f32 * self.patch_size,
        )
    }

    pub fn world_bounds(&self) -> Aabb {
        let min_x = self.grid_x as f32 * self.patch_size;
        let min_z = self.grid_z as f32 * self.patch_size;
        let h_min = self.height_data.iter().copied().fold(f32::INFINITY, f32::min);
        let h_max = self.height_data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Aabb::new(
            Vec3::new(min_x, h_min, min_z),
            Vec3::new(min_x + self.patch_size, h_max, min_z + self.patch_size),
        )
    }

    pub fn sample_height_bilinear(&self, local_x: f32, local_z: f32) -> f32 {
        // Sample height using bilinear interpolation
        let res = self.height_grid_res as f32;
        let u = (local_x / self.patch_size) * (res - 1.0);
        let v = (local_z / self.patch_size) * (res - 1.0);
        let x0 = u.floor() as usize;
        let z0 = v.floor() as usize;
        let x1 = (x0 + 1).min(self.height_grid_res as usize - 1);
        let z1 = (z0 + 1).min(self.height_grid_res as usize - 1);
        let fx = u - u.floor();
        let fz = v - v.floor();
        let res_u = self.height_grid_res as usize;
        let h00 = self.height_data[z0 * res_u + x0];
        let h10 = self.height_data[z0 * res_u + x1];
        let h01 = self.height_data[z1 * res_u + x0];
        let h11 = self.height_data[z1 * res_u + x1];
        h00 * (1.0 - fx) * (1.0 - fz)
            + h10 * fx * (1.0 - fz)
            + h01 * (1.0 - fx) * fz
            + h11 * fx * fz
    }

    pub fn compute_normal_at(&self, local_x: f32, local_z: f32) -> Vec3 {
        let step = self.patch_size / self.height_grid_res as f32;
        let hx_plus  = self.sample_height_bilinear((local_x + step).min(self.patch_size), local_z);
        let hx_minus = self.sample_height_bilinear((local_x - step).max(0.0), local_z);
        let hz_plus  = self.sample_height_bilinear(local_x, (local_z + step).min(self.patch_size));
        let hz_minus = self.sample_height_bilinear(local_x, (local_z - step).max(0.0));
        let grad_x = (hx_plus - hx_minus) / (2.0 * step);
        let grad_z = (hz_plus - hz_minus) / (2.0 * step);
        Vec3::new(-grad_x, 1.0, -grad_z).normalize()
    }

    pub fn desired_lod_for_distance(&self, distance: f32) -> u32 {
        let thresholds = [50.0, 150.0, 400.0, 900.0, 2000.0];
        for (lod, &threshold) in thresholds.iter().enumerate() {
            if distance < threshold {
                return lod as u32;
            }
        }
        self.max_lod
    }

    pub fn update_lod(&mut self, camera_pos: Vec3) {
        let center = self.world_position() + Vec3::splat(self.patch_size * 0.5);
        let dist = (camera_pos - center).length();
        let desired = self.desired_lod_for_distance(dist);
        if desired != self.current_lod {
            self.morph_fraction = 0.0;
        } else {
            self.morph_fraction = (self.morph_fraction + 0.05).min(1.0);
        }
        self.current_lod = desired.min(self.max_lod);
        // Recompute vertex count for this LOD
        let step = 1u32 << self.current_lod;
        let reduced_res = (self.height_grid_res / step).max(2);
        self.vertex_count = reduced_res * reduced_res;
    }

    pub fn needs_stitching(&self) -> bool {
        self.neighbor_lods.iter().any(|&n| n != self.current_lod)
    }

    pub fn stitch_skirt_vertices(&self) -> Vec<Vec3> {
        // Generate skirt vertices around the patch edge to hide T-junctions
        let mut skirt = Vec::new();
        let step = 1u32 << self.current_lod;
        let res = self.height_grid_res / step;
        let cell_size = self.patch_size / res as f32;
        let base = self.world_position();
        // Bottom edge (z=0)
        for i in 0..=res {
            let x = base.x + i as f32 * cell_size;
            let h = self.sample_height_bilinear(i as f32 * cell_size, 0.0);
            skirt.push(Vec3::new(x, h, base.z));
            skirt.push(Vec3::new(x, h - 1.0, base.z)); // skirt hanging down
        }
        skirt
    }
}

// ============================================================
// SECTION: Terrain Manager
// ============================================================

#[derive(Clone, Debug)]
pub struct TerrainManager {
    pub patches: HashMap<(i32, i32), TerrainPatch>,
    pub patch_size: f32,
    pub max_lod: u32,
    pub streaming_radius: f32,
    pub total_vertices_rendered: u32,
    pub total_patches_visible: u32,
}

impl TerrainManager {
    pub fn new(patch_size: f32, max_lod: u32, streaming_radius: f32) -> Self {
        Self {
            patches: HashMap::new(),
            patch_size,
            max_lod,
            streaming_radius,
            total_vertices_rendered: 0,
            total_patches_visible: 0,
        }
    }

    pub fn get_or_create_patch(&mut self, grid_x: i32, grid_z: i32) -> &mut TerrainPatch {
        let sz = self.patch_size;
        let ml = self.max_lod;
        let next_id = self.patches.len() as u32 + 1;
        self.patches.entry((grid_x, grid_z)).or_insert_with(|| {
            TerrainPatch::new(next_id, grid_x, grid_z, sz, ml)
        })
    }

    pub fn update(&mut self, camera_pos: Vec3) {
        let tile_r = (self.streaming_radius / self.patch_size).ceil() as i32 + 1;
        let cx = (camera_pos.x / self.patch_size).floor() as i32;
        let cz = (camera_pos.z / self.patch_size).floor() as i32;
        let mut total_verts = 0u32;
        let mut visible = 0u32;
        for tx in (cx - tile_r)..=(cx + tile_r) {
            for tz in (cz - tile_r)..=(cz + tile_r) {
                let center = Vec3::new(
                    (tx as f32 + 0.5) * self.patch_size,
                    camera_pos.y,
                    (tz as f32 + 0.5) * self.patch_size,
                );
                let dist = (camera_pos - center).length();
                if dist <= self.streaming_radius {
                    let patch = self.get_or_create_patch(tx, tz);
                    patch.update_lod(camera_pos);
                    total_verts += patch.vertex_count;
                    visible += 1;
                }
            }
        }
        // Update neighbor LODs for stitching
        let keys: Vec<(i32, i32)> = self.patches.keys().copied().collect();
        for &(tx, tz) in &keys {
            let neighbors = [
                ((tx, tz - 1), 0usize),
                ((tx, tz + 1), 1),
                ((tx + 1, tz), 2),
                ((tx - 1, tz), 3),
            ];
            let my_lod = self.patches[&(tx, tz)].current_lod;
            let _ = my_lod;
            let mut nlods = [0u32; 4];
            for (nkey, dir) in &neighbors {
                nlods[*dir] = self.patches.get(nkey).map(|p| p.current_lod).unwrap_or(0);
            }
            if let Some(patch) = self.patches.get_mut(&(tx, tz)) {
                patch.neighbor_lods = nlods;
            }
        }
        self.total_vertices_rendered = total_verts;
        self.total_patches_visible = visible;
    }

    pub fn sample_height_world(&self, world_x: f32, world_z: f32) -> f32 {
        let gx = (world_x / self.patch_size).floor() as i32;
        let gz = (world_z / self.patch_size).floor() as i32;
        if let Some(patch) = self.patches.get(&(gx, gz)) {
            let local_x = world_x - gx as f32 * self.patch_size;
            let local_z = world_z - gz as f32 * self.patch_size;
            patch.sample_height_bilinear(local_x.max(0.0), local_z.max(0.0))
        } else {
            0.0
        }
    }

    pub fn visible_patch_count(&self) -> usize {
        self.patches.len()
    }

    pub fn patches_needing_stitch(&self) -> Vec<(i32, i32)> {
        self.patches.iter()
            .filter(|(_, p)| p.needs_stitching())
            .map(|(&k, _)| k)
            .collect()
    }
}

// ============================================================
// SECTION: Streaming Priority Queue with Deadline Scheduling
// ============================================================

#[derive(Clone, Debug)]
pub struct DeadlineRequest {
    pub id: u64,
    pub priority: f32,
    pub deadline_seconds: f64,
    pub estimated_load_ms: f32,
    pub size_bytes: u64,
    pub level_id: u64,
    pub request_time: f64,
    pub cancelled: bool,
}

impl DeadlineRequest {
    pub fn urgency_at(&self, current_time: f64) -> f32 {
        let remaining = (self.deadline_seconds - current_time).max(0.001) as f32;
        let normalized_load = self.estimated_load_ms / 1000.0;
        self.priority * (normalized_load / remaining).min(100.0)
    }

    pub fn is_overdue(&self, current_time: f64) -> bool {
        current_time > self.deadline_seconds
    }

    pub fn slack_ms(&self, current_time: f64) -> f32 {
        ((self.deadline_seconds - current_time) * 1000.0 - self.estimated_load_ms as f64).max(0.0) as f32
    }
}

#[derive(Clone, Debug)]
pub struct DeadlineScheduler {
    pub requests: Vec<DeadlineRequest>,
    pub next_request_id: u64,
    pub total_scheduled: u64,
    pub total_completed: u64,
    pub total_missed: u64,
    pub bandwidth_bytes_per_sec: f64,
    pub inflight_bytes: u64,
    pub max_inflight_bytes: u64,
}

impl DeadlineScheduler {
    pub fn new(bandwidth_bytes_per_sec: f64) -> Self {
        Self {
            requests: Vec::new(),
            next_request_id: 1,
            total_scheduled: 0,
            total_completed: 0,
            total_missed: 0,
            bandwidth_bytes_per_sec,
            inflight_bytes: 0,
            max_inflight_bytes: 64 * 1024 * 1024, // 64 MB in flight
        }
    }

    pub fn submit(&mut self, level_id: u64, priority: f32, deadline: f64, size_bytes: u64,
                  estimated_load_ms: f32, current_time: f64) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.requests.push(DeadlineRequest {
            id,
            priority,
            deadline_seconds: deadline,
            estimated_load_ms,
            size_bytes,
            level_id,
            request_time: current_time,
            cancelled: false,
        });
        self.total_scheduled += 1;
        id
    }

    pub fn cancel(&mut self, request_id: u64) {
        if let Some(r) = self.requests.iter_mut().find(|r| r.id == request_id) {
            r.cancelled = true;
        }
    }

    pub fn update(&mut self, current_time: f64, delta_seconds: f64) {
        // Remove cancelled requests
        self.requests.retain(|r| !r.cancelled);
        // Check for missed deadlines
        let missed: Vec<u64> = self.requests.iter()
            .filter(|r| r.is_overdue(current_time))
            .map(|r| r.id)
            .collect();
        self.total_missed += missed.len() as u64;
        self.requests.retain(|r| !r.is_overdue(current_time));
        // Compute available bandwidth this tick
        let available_bytes = (self.bandwidth_bytes_per_sec * delta_seconds) as u64;
        self.inflight_bytes = self.inflight_bytes.saturating_sub(available_bytes);
    }

    pub fn next_batch(&mut self, current_time: f64, max_count: usize) -> Vec<u64> {
        // Sort by Earliest Deadline First (EDF) with priority tie-breaking
        let mut sortable: Vec<(usize, f32)> = self.requests.iter().enumerate()
            .map(|(i, r)| (i, r.urgency_at(current_time)))
            .collect();
        sortable.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let mut result = Vec::new();
        let mut inflight = self.inflight_bytes;
        for (idx, _urgency) in sortable.iter().take(max_count) {
            let r = &self.requests[*idx];
            if inflight + r.size_bytes <= self.max_inflight_bytes {
                inflight += r.size_bytes;
                result.push(r.id);
            }
        }
        self.inflight_bytes = inflight;
        result
    }

    pub fn complete_request(&mut self, request_id: u64) {
        if let Some(pos) = self.requests.iter().position(|r| r.id == request_id) {
            let r = self.requests.remove(pos);
            self.inflight_bytes = self.inflight_bytes.saturating_sub(r.size_bytes);
            self.total_completed += 1;
        }
    }

    pub fn utilization(&self) -> f32 {
        self.inflight_bytes as f32 / self.max_inflight_bytes as f32
    }

    pub fn deadline_miss_rate(&self) -> f32 {
        if self.total_scheduled == 0 { return 0.0; }
        self.total_missed as f32 / self.total_scheduled as f32
    }
}

// ============================================================
// SECTION: Spatial Hash Acceleration
// ============================================================

#[derive(Clone, Debug)]
pub struct SpatialHash3D {
    pub cell_size: f32,
    cells: HashMap<(i32, i32, i32), Vec<u64>>,
    pub object_cells: HashMap<u64, (i32, i32, i32)>,
}

impl SpatialHash3D {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            object_cells: HashMap::new(),
        }
    }

    fn hash_pos(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, object_id: u64, pos: Vec3) {
        let key = self.hash_pos(pos);
        self.cells.entry(key).or_insert_with(Vec::new).push(object_id);
        self.object_cells.insert(object_id, key);
    }

    pub fn remove(&mut self, object_id: u64) {
        if let Some(&key) = self.object_cells.get(&object_id) {
            if let Some(list) = self.cells.get_mut(&key) {
                list.retain(|&id| id != object_id);
            }
            self.object_cells.remove(&object_id);
        }
    }

    pub fn update(&mut self, object_id: u64, new_pos: Vec3) {
        self.remove(object_id);
        self.insert(object_id, new_pos);
    }

    pub fn query_radius(&self, center: Vec3, radius: f32) -> Vec<u64> {
        let r_cells = (radius / self.cell_size).ceil() as i32 + 1;
        let cc = self.hash_pos(center);
        let mut result = Vec::new();
        for x in (cc.0 - r_cells)..=(cc.0 + r_cells) {
            for y in (cc.1 - r_cells)..=(cc.1 + r_cells) {
                for z in (cc.2 - r_cells)..=(cc.2 + r_cells) {
                    if let Some(list) = self.cells.get(&(x, y, z)) {
                        result.extend_from_slice(list);
                    }
                }
            }
        }
        result
    }

    pub fn query_aabb(&self, aabb: &Aabb) -> Vec<u64> {
        let min_key = self.hash_pos(aabb.min);
        let max_key = self.hash_pos(aabb.max);
        let mut result = Vec::new();
        for x in min_key.0..=max_key.0 {
            for y in min_key.1..=max_key.1 {
                for z in min_key.2..=max_key.2 {
                    if let Some(list) = self.cells.get(&(x, y, z)) {
                        result.extend_from_slice(list);
                    }
                }
            }
        }
        result
    }

    pub fn object_count(&self) -> usize {
        self.object_cells.len()
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn average_occupancy(&self) -> f32 {
        if self.cells.is_empty() { return 0.0; }
        let total: usize = self.cells.values().map(|v| v.len()).sum();
        total as f32 / self.cells.len() as f32
    }
}

// ============================================================
// SECTION: Volumetric Fog Streaming Zone
// ============================================================

#[derive(Clone, Debug)]
pub struct FogZone {
    pub zone_id: u32,
    pub bounds: Aabb,
    pub fog_density: f32,
    pub fog_color: Vec3,
    pub scatter_coefficient: f32,
    pub absorption_coefficient: f32,
    pub height_falloff: f32,    // exponential height fog falloff
    pub height_offset: f32,
    pub animation_speed: f32,
    pub turbulence: f32,
    pub enabled: bool,
    pub blend_distance: f32,    // transition zone width
}

impl FogZone {
    pub fn new(zone_id: u32, bounds: Aabb) -> Self {
        Self {
            zone_id,
            bounds,
            fog_density: 0.01,
            fog_color: Vec3::new(0.7, 0.75, 0.8),
            scatter_coefficient: 0.005,
            absorption_coefficient: 0.003,
            height_falloff: 0.01,
            height_offset: 0.0,
            animation_speed: 0.1,
            turbulence: 0.5,
            enabled: true,
            blend_distance: 50.0,
        }
    }

    pub fn density_at(&self, world_pos: Vec3, time: f64) -> f32 {
        if !self.enabled { return 0.0; }
        if !self.bounds.contains_point(world_pos) { return 0.0; }
        // Height-based exponential falloff
        let h = (world_pos.y - self.height_offset).max(0.0);
        let height_factor = (-self.height_falloff * h).exp();
        // Turbulence using sine approximation
        let t = time as f32;
        let noise = (world_pos.x * 0.1 + t * self.animation_speed).sin()
            * (world_pos.z * 0.1 + t * self.animation_speed * 0.7).cos()
            * self.turbulence * 0.5 + 0.5;
        let blend = self.blend_factor(world_pos);
        self.fog_density * height_factor * (1.0 + noise) * blend
    }

    fn blend_factor(&self, pos: Vec3) -> f32 {
        // Smooth blend at zone boundaries
        let min_dist = [
            pos.x - self.bounds.min.x,
            self.bounds.max.x - pos.x,
            pos.y - self.bounds.min.y,
            self.bounds.max.y - pos.y,
            pos.z - self.bounds.min.z,
            self.bounds.max.z - pos.z,
        ].iter().copied().fold(f32::INFINITY, f32::min);
        let t = (min_dist / self.blend_distance).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    pub fn transmittance_along_ray(&self, start: Vec3, end: Vec3, samples: u32, time: f64) -> f32 {
        // Beer-Lambert law: T = exp(-integral(sigma_t * ds))
        let mut optical_depth = 0.0f32;
        let dir = end - start;
        let total_len = dir.length();
        if total_len < 1e-6 { return 1.0; }
        let step = total_len / samples as f32;
        let d = dir / total_len;
        for i in 0..samples {
            let t = (i as f32 + 0.5) * step;
            let pos = start + d * t;
            let density = self.density_at(pos, time);
            optical_depth += density * step * (self.scatter_coefficient + self.absorption_coefficient);
        }
        (-optical_depth).exp()
    }

    pub fn phase_function_henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
        // Henyey-Greenstein phase function for anisotropic scattering
        let g2 = g * g;
        let denom = (1.0 + g2 - 2.0 * g * cos_theta).powf(1.5);
        (1.0 - g2) / (4.0 * std::f32::consts::PI * denom.max(1e-10))
    }
}

// ============================================================
// SECTION: Level Streaming Scene Graph Node
// ============================================================

#[derive(Clone, Debug)]
pub struct SceneNode {
    pub node_id: u64,
    pub name: String,
    pub local_transform: Mat4,
    pub world_transform: Mat4,
    pub parent_id: Option<u64>,
    pub children: Vec<u64>,
    pub level_id: Option<u64>,
    pub is_static: bool,
    pub dirty: bool,
    pub visibility_distance: f32,
    pub lod_bias: f32,
}

impl SceneNode {
    pub fn new(node_id: u64, name: &str) -> Self {
        Self {
            node_id,
            name: name.to_string(),
            local_transform: Mat4::IDENTITY,
            world_transform: Mat4::IDENTITY,
            parent_id: None,
            children: Vec::new(),
            level_id: None,
            is_static: false,
            dirty: true,
            visibility_distance: 1000.0,
            lod_bias: 0.0,
        }
    }

    pub fn set_local_transform(&mut self, transform: Mat4) {
        self.local_transform = transform;
        self.dirty = true;
    }

    pub fn world_position(&self) -> Vec3 {
        Vec3::new(
            self.world_transform.w_axis.x,
            self.world_transform.w_axis.y,
            self.world_transform.w_axis.z,
        )
    }

    pub fn is_visible_from(&self, camera_pos: Vec3) -> bool {
        let dist = (self.world_position() - camera_pos).length();
        dist <= self.visibility_distance
    }
}

#[derive(Clone, Debug)]
pub struct SceneGraph {
    pub nodes: HashMap<u64, SceneNode>,
    pub root_nodes: Vec<u64>,
    pub next_node_id: u64,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
            next_node_id: 1,
        }
    }

    pub fn create_node(&mut self, name: &str) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        self.nodes.insert(id, SceneNode::new(id, name));
        self.root_nodes.push(id);
        id
    }

    pub fn attach_child(&mut self, parent_id: u64, child_id: u64) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if !parent.children.contains(&child_id) {
                parent.children.push(child_id);
            }
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent_id = Some(parent_id);
            child.dirty = true;
        }
        self.root_nodes.retain(|&id| id != child_id);
    }

    pub fn detach_child(&mut self, child_id: u64) {
        let parent_id = self.nodes.get(&child_id).and_then(|n| n.parent_id);
        if let Some(pid) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&pid) {
                parent.children.retain(|&id| id != child_id);
            }
        }
        if let Some(node) = self.nodes.get_mut(&child_id) {
            node.parent_id = None;
            node.dirty = true;
        }
        if !self.root_nodes.contains(&child_id) {
            self.root_nodes.push(child_id);
        }
    }

    pub fn update_world_transforms(&mut self) {
        // Iterative BFS to propagate dirty transforms
        let roots = self.root_nodes.clone();
        let mut queue = std::collections::VecDeque::new();
        for root in roots {
            if let Some(node) = self.nodes.get_mut(&root) {
                if node.dirty {
                    node.world_transform = node.local_transform;
                    node.dirty = false;
                }
            }
            queue.push_back(root);
        }
        while let Some(nid) = queue.pop_front() {
            let (parent_world, children) = if let Some(node) = self.nodes.get(&nid) {
                (node.world_transform, node.children.clone())
            } else {
                continue;
            };
            for child_id in children {
                if let Some(child) = self.nodes.get_mut(&child_id) {
                    if child.dirty {
                        child.world_transform = parent_world * child.local_transform;
                        child.dirty = false;
                    }
                }
                queue.push_back(child_id);
            }
        }
    }

    pub fn find_by_name(&self, name: &str) -> Option<u64> {
        self.nodes.values()
            .find(|n| n.name == name)
            .map(|n| n.node_id)
    }

    pub fn collect_subtree(&self, root_id: u64) -> Vec<u64> {
        let mut result = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(root_id);
        while let Some(nid) = queue.pop_front() {
            result.push(nid);
            if let Some(node) = self.nodes.get(&nid) {
                for &child in &node.children {
                    queue.push_back(child);
                }
            }
        }
        result
    }

    pub fn depth_of(&self, node_id: u64) -> u32 {
        let mut depth = 0u32;
        let mut current = node_id;
        loop {
            if let Some(node) = self.nodes.get(&current) {
                if let Some(pid) = node.parent_id {
                    depth += 1;
                    current = pid;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        depth
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn root_count(&self) -> usize {
        self.root_nodes.len()
    }

    pub fn remove_subtree(&mut self, root_id: u64) -> usize {
        let subtree = self.collect_subtree(root_id);
        let count = subtree.len();
        self.detach_child(root_id);
        for id in &subtree {
            self.nodes.remove(id);
        }
        self.root_nodes.retain(|id| !subtree.contains(id));
        count
    }
}

// ============================================================
// SECTION: Streaming World Metrics Dashboard
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct StreamingMetricsDashboard {
    pub frame_number: u64,
    pub current_time: f64,
    // Load/unload counts
    pub loads_this_frame: u32,
    pub unloads_this_frame: u32,
    pub total_loads: u64,
    pub total_unloads: u64,
    // Memory
    pub peak_memory_mb: f32,
    pub current_memory_mb: f32,
    pub memory_budget_mb: f32,
    // Performance
    pub avg_load_time_ms: f32,
    pub max_load_time_ms: f32,
    pub streaming_stalls: u32,
    pub frames_with_load: u32,
    // Counts
    pub active_levels: u32,
    pub loading_levels: u32,
    pub visible_levels: u32,
    pub culled_levels: u32,
    // Bandwidth
    pub bytes_loaded_this_sec: u64,
    pub bytes_unloaded_this_sec: u64,
    pub bandwidth_utilization: f32,
    // History ring buffer
    pub memory_history: VecDeque<f32>,
    pub fps_history: VecDeque<f32>,
    history_capacity: usize,
}

impl StreamingMetricsDashboard {
    pub fn new(history_capacity: usize) -> Self {
        Self {
            memory_history: VecDeque::with_capacity(history_capacity),
            fps_history: VecDeque::with_capacity(history_capacity),
            history_capacity,
            memory_budget_mb: 512.0,
            ..Default::default()
        }
    }

    pub fn begin_frame(&mut self, time: f64, fps: f32) {
        self.frame_number += 1;
        self.current_time = time;
        self.loads_this_frame = 0;
        self.unloads_this_frame = 0;
        // Update history
        if self.memory_history.len() >= self.history_capacity {
            self.memory_history.pop_front();
        }
        self.memory_history.push_back(self.current_memory_mb);
        if self.fps_history.len() >= self.history_capacity {
            self.fps_history.pop_front();
        }
        self.fps_history.push_back(fps);
    }

    pub fn record_load(&mut self, load_time_ms: f32, bytes: u64) {
        self.loads_this_frame += 1;
        self.total_loads += 1;
        self.bytes_loaded_this_sec += bytes;
        if load_time_ms > self.max_load_time_ms {
            self.max_load_time_ms = load_time_ms;
        }
        // Rolling average
        let n = self.total_loads as f32;
        self.avg_load_time_ms = (self.avg_load_time_ms * (n - 1.0) + load_time_ms) / n;
    }

    pub fn record_unload(&mut self, bytes: u64) {
        self.unloads_this_frame += 1;
        self.total_unloads += 1;
        self.bytes_unloaded_this_sec += bytes;
    }

    pub fn record_stall(&mut self) {
        self.streaming_stalls += 1;
    }

    pub fn update_memory(&mut self, used_mb: f32) {
        self.current_memory_mb = used_mb;
        if used_mb > self.peak_memory_mb {
            self.peak_memory_mb = used_mb;
        }
    }

    pub fn memory_utilization(&self) -> f32 {
        if self.memory_budget_mb < 1.0 { return 0.0; }
        self.current_memory_mb / self.memory_budget_mb
    }

    pub fn average_fps(&self) -> f32 {
        if self.fps_history.is_empty() { return 0.0; }
        self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32
    }

    pub fn min_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(f32::INFINITY, f32::min)
    }

    pub fn memory_trend(&self) -> f32 {
        // Slope of memory over last N frames (linear regression simplified)
        let n = self.memory_history.len();
        if n < 2 { return 0.0; }
        let x_mean = (n as f32 - 1.0) * 0.5;
        let y_mean: f32 = self.memory_history.iter().sum::<f32>() / n as f32;
        let mut numer = 0.0f32;
        let mut denom = 0.0f32;
        for (i, &y) in self.memory_history.iter().enumerate() {
            let x = i as f32 - x_mean;
            numer += x * (y - y_mean);
            denom += x * x;
        }
        if denom.abs() < 1e-10 { return 0.0; }
        numer / denom
    }

    pub fn is_memory_critical(&self) -> bool {
        self.memory_utilization() > 0.9
    }

    pub fn is_bandwidth_saturated(&self) -> bool {
        self.bandwidth_utilization > 0.95
    }

    pub fn report_summary(&self) -> HashMap<String, f32> {
        let mut map = HashMap::new();
        map.insert("memory_mb".to_string(), self.current_memory_mb);
        map.insert("memory_utilization".to_string(), self.memory_utilization());
        map.insert("avg_load_ms".to_string(), self.avg_load_time_ms);
        map.insert("max_load_ms".to_string(), self.max_load_time_ms);
        map.insert("stalls".to_string(), self.streaming_stalls as f32);
        map.insert("total_loads".to_string(), self.total_loads as f32);
        map.insert("avg_fps".to_string(), self.average_fps());
        map.insert("memory_trend_mb_per_frame".to_string(), self.memory_trend());
        map
    }
}

// ============================================================
// SECTION: Integrated Level Streaming World Manager (Complete)
// ============================================================

#[derive(Clone, Debug)]
pub struct IntegratedStreamingWorld {
    pub scene_graph: SceneGraph,
    pub terrain: TerrainManager,
    pub tile_map: StreamingTileMap,
    pub instancer: LevelInstancer,
    pub fog_zones: Vec<FogZone>,
    pub deadline_scheduler: DeadlineScheduler,
    pub spatial_hash: SpatialHash3D,
    pub metrics: StreamingMetricsDashboard,
    pub camera_pos: Vec3,
    pub camera_dir: Vec3,
    pub camera_velocity: Vec3,
    pub current_time: f64,
    pub delta_time: f32,
    pub stream_radius_main: f32,
    pub stream_radius_secondary: f32,
    pub frame_count: u64,
}

impl IntegratedStreamingWorld {
    pub fn new() -> Self {
        Self {
            scene_graph: SceneGraph::new(),
            terrain: TerrainManager::new(256.0, 5, 2000.0),
            tile_map: StreamingTileMap::new(512.0),
            instancer: LevelInstancer::new(256.0),
            fog_zones: Vec::new(),
            deadline_scheduler: DeadlineScheduler::new(100.0 * 1024.0 * 1024.0), // 100 MB/s
            spatial_hash: SpatialHash3D::new(128.0),
            metrics: StreamingMetricsDashboard::new(120),
            camera_pos: Vec3::ZERO,
            camera_dir: Vec3::NEG_Z,
            camera_velocity: Vec3::ZERO,
            current_time: 0.0,
            delta_time: 0.016,
            stream_radius_main: 800.0,
            stream_radius_secondary: 1500.0,
            frame_count: 0,
        }
    }

    pub fn update(&mut self, camera_pos: Vec3, camera_dir: Vec3, delta_time: f32) {
        let prev_pos = self.camera_pos;
        self.camera_pos = camera_pos;
        self.camera_dir = camera_dir.normalize_or_zero();
        self.camera_velocity = (camera_pos - prev_pos) / delta_time.max(1e-6);
        self.delta_time = delta_time;
        self.current_time += delta_time as f64;
        self.frame_count += 1;
        // Update subsystems
        let fps = if delta_time > 1e-6 { 1.0 / delta_time } else { 60.0 };
        self.metrics.begin_frame(self.current_time, fps);
        self.terrain.update(camera_pos);
        self.deadline_scheduler.update(self.current_time, delta_time as f64);
        self.scene_graph.update_world_transforms();
        let mem_mb = self.estimate_memory_mb();
        self.metrics.update_memory(mem_mb);
    }

    fn estimate_memory_mb(&self) -> f32 {
        let terrain_verts = self.terrain.total_vertices_rendered as f32 * 32.0; // 32 bytes/vert
        let scene_nodes = self.scene_graph.node_count() as f32 * 256.0;
        let instances = self.instancer.instances.len() as f32 * 512.0;
        let tiles = self.tile_map.tile_count() as f32 * 128.0;
        (terrain_verts + scene_nodes + instances + tiles) / (1024.0 * 1024.0)
    }

    pub fn add_fog_zone(&mut self, bounds: Aabb) -> u32 {
        let id = self.fog_zones.len() as u32 + 1;
        self.fog_zones.push(FogZone::new(id, bounds));
        id
    }

    pub fn fog_density_at(&self, pos: Vec3) -> f32 {
        self.fog_zones.iter()
            .filter(|z| z.enabled)
            .map(|z| z.density_at(pos, self.current_time))
            .sum()
    }

    pub fn schedule_level_load(&mut self, level_id: u64, priority: f32, size_bytes: u64) -> u64 {
        let deadline = self.current_time + 2.0; // 2 second deadline
        let est_ms = size_bytes as f32 / (100.0 * 1024.0); // estimate at 100 MB/s
        self.deadline_scheduler.submit(level_id, priority, deadline, size_bytes, est_ms, self.current_time)
    }

    pub fn place_level_instance(&mut self, level_id: u64, position: Vec3) -> u64 {
        let transform = Mat4::from_translation(position);
        let instance_id = self.instancer.place_instance(level_id, transform);
        self.spatial_hash.insert(instance_id, position);
        // Create scene node
        let node_id = self.scene_graph.create_node(&format!("Level_{}", instance_id));
        if let Some(node) = self.scene_graph.nodes.get_mut(&node_id) {
            node.local_transform = transform;
            node.world_transform = transform;
            node.level_id = Some(level_id);
            node.dirty = false;
        }
        instance_id
    }

    pub fn instances_near(&self, center: Vec3, radius: f32) -> Vec<u64> {
        self.spatial_hash.query_radius(center, radius)
    }

    pub fn terrain_height_at(&self, x: f32, z: f32) -> f32 {
        self.terrain.sample_height_world(x, z)
    }

    pub fn predicted_camera_position(&self, look_ahead_seconds: f32) -> Vec3 {
        self.camera_pos + self.camera_velocity * look_ahead_seconds
            + Vec3::Y * 0.5 * (-9.8) * look_ahead_seconds * look_ahead_seconds
    }

    pub fn tiles_to_preload(&self, look_ahead_seconds: f32) -> Vec<(i32, i32)> {
        let future_pos = self.predicted_camera_position(look_ahead_seconds);
        let mut tiles = self.tile_map.tiles_in_radius(future_pos, self.stream_radius_main);
        let current_tiles = self.tile_map.tiles_in_radius(self.camera_pos, self.stream_radius_main);
        tiles.retain(|t| !current_tiles.contains(t));
        tiles
    }

    pub fn memory_mb(&self) -> f32 { self.metrics.current_memory_mb }
    pub fn is_critical(&self) -> bool { self.metrics.is_memory_critical() }
    pub fn frame_count(&self) -> u64 { self.frame_count }
    pub fn avg_fps(&self) -> f32 { self.metrics.average_fps() }
}

// ============================================================
// SECTION: Additional Unit Tests
// ============================================================

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_streaming_tile_bounds() {
        let tile = StreamingTile::new(2, 3, 100.0);
        let bounds = tile.world_bounds();
        assert!((bounds.min.x - 200.0).abs() < 0.01);
        assert!((bounds.max.x - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_streaming_tile_contains() {
        let tile = StreamingTile::new(0, 0, 100.0);
        assert!(tile.contains_point_2d(50.0, 50.0));
        assert!(!tile.contains_point_2d(150.0, 50.0));
    }

    #[test]
    fn test_tile_map_world_to_tile() {
        let map = StreamingTileMap::new(128.0);
        let (tx, tz) = map.world_to_tile(Vec3::new(300.0, 0.0, 300.0));
        assert_eq!(tx, 2);
        assert_eq!(tz, 2);
    }

    #[test]
    fn test_tile_map_tiles_in_radius() {
        let mut map = StreamingTileMap::new(100.0);
        map.get_or_create(0, 0).last_visited_time = 0.0;
        map.get_or_create(1, 0).last_visited_time = 0.0;
        map.get_or_create(0, 1).last_visited_time = 0.0;
        let tiles = map.tiles_in_radius(Vec3::new(50.0, 0.0, 50.0), 200.0);
        assert!(!tiles.is_empty());
    }

    #[test]
    fn test_tile_map_evict_stale() {
        let mut map = StreamingTileMap::new(100.0);
        map.get_or_create(0, 0).last_visited_time = 0.0;
        map.get_or_create(1, 1).last_visited_time = 0.0;
        let evicted = map.evict_stale(100.0, 50.0);
        assert_eq!(evicted, 2);
        assert_eq!(map.tile_count(), 0);
    }

    #[test]
    fn test_level_instance_position() {
        let t = Mat4::from_translation(Vec3::new(10.0, 5.0, -3.0));
        let inst = LevelInstance::new(1, 42, t);
        let pos = inst.position();
        assert!((pos.x - 10.0).abs() < 0.01);
        assert!((pos.y - 5.0).abs() < 0.01);
        assert!((pos.z + 3.0).abs() < 0.01);
    }

    #[test]
    fn test_level_instancer_place_and_query() {
        let mut instancer = LevelInstancer::new(100.0);
        let t = Mat4::from_translation(Vec3::new(50.0, 0.0, 50.0));
        let id = instancer.place_instance(1, t);
        let found = instancer.instances_in_radius(Vec3::new(50.0, 0.0, 50.0), 10.0);
        assert!(found.contains(&id));
    }

    #[test]
    fn test_level_instancer_remove() {
        let mut instancer = LevelInstancer::new(100.0);
        let t = Mat4::IDENTITY;
        let id = instancer.place_instance(5, t);
        assert!(instancer.instances.contains_key(&id));
        instancer.remove_instance(id);
        assert!(!instancer.instances.contains_key(&id));
    }

    #[test]
    fn test_terrain_patch_height_sample() {
        let mut patch = TerrainPatch::new(1, 0, 0, 100.0, 5);
        // Fill with a known height
        for h in &mut patch.height_data { *h = 42.0; }
        let h = patch.sample_height_bilinear(50.0, 50.0);
        assert!((h - 42.0).abs() < 0.01);
    }

    #[test]
    fn test_terrain_patch_lod_selection() {
        let mut patch = TerrainPatch::new(1, 0, 0, 256.0, 5);
        patch.update_lod(Vec3::new(128.0, 0.0, 128.0)); // near center
        assert_eq!(patch.current_lod, 0); // should be highest quality
        patch.update_lod(Vec3::new(10000.0, 0.0, 10000.0)); // very far
        assert!(patch.current_lod > 0);
    }

    #[test]
    fn test_terrain_manager_update() {
        let mut mgr = TerrainManager::new(256.0, 4, 1000.0);
        mgr.update(Vec3::new(0.0, 0.0, 0.0));
        assert!(mgr.visible_patch_count() > 0);
    }

    #[test]
    fn test_deadline_scheduler_submit_complete() {
        let mut sched = DeadlineScheduler::new(100.0 * 1024.0 * 1024.0);
        let id = sched.submit(1, 1.0, 2.0, 1024, 1.0, 0.0);
        sched.complete_request(id);
        assert_eq!(sched.total_completed, 1);
        assert!(sched.requests.is_empty());
    }

    #[test]
    fn test_deadline_scheduler_miss_rate() {
        let mut sched = DeadlineScheduler::new(1.0);
        sched.submit(1, 1.0, 0.001, 1024, 1.0, 0.0);
        sched.update(1.0, 1.0); // past the deadline
        assert_eq!(sched.total_missed, 1);
    }

    #[test]
    fn test_spatial_hash_insert_query() {
        let mut sh = SpatialHash3D::new(10.0);
        sh.insert(1, Vec3::new(5.0, 0.0, 5.0));
        sh.insert(2, Vec3::new(100.0, 0.0, 100.0));
        let result = sh.query_radius(Vec3::new(5.0, 0.0, 5.0), 5.0);
        assert!(result.contains(&1));
        assert!(!result.contains(&2));
    }

    #[test]
    fn test_spatial_hash_update() {
        let mut sh = SpatialHash3D::new(10.0);
        sh.insert(1, Vec3::new(5.0, 0.0, 5.0));
        sh.update(1, Vec3::new(200.0, 0.0, 200.0));
        let near = sh.query_radius(Vec3::new(5.0, 0.0, 5.0), 5.0);
        assert!(!near.contains(&1));
        let far = sh.query_radius(Vec3::new(200.0, 0.0, 200.0), 5.0);
        assert!(far.contains(&1));
    }

    #[test]
    fn test_fog_zone_density() {
        let bounds = Aabb::new(Vec3::new(-100.0, -50.0, -100.0), Vec3::new(100.0, 50.0, 100.0));
        let zone = FogZone::new(1, bounds);
        let d = zone.density_at(Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert!(d >= 0.0 && d.is_finite());
        let d_outside = zone.density_at(Vec3::new(1000.0, 0.0, 0.0), 0.0);
        assert_eq!(d_outside, 0.0);
    }

    #[test]
    fn test_fog_transmittance() {
        let bounds = Aabb::new(Vec3::new(-200.0, -100.0, -200.0), Vec3::new(200.0, 100.0, 200.0));
        let zone = FogZone::new(1, bounds);
        let t = zone.transmittance_along_ray(
            Vec3::new(-100.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            16,
            0.0,
        );
        assert!(t > 0.0 && t <= 1.0);
    }

    #[test]
    fn test_henyey_greenstein_forward() {
        // Forward scattering (cos_theta=1): should be maximum
        let g = 0.8;
        let forward = FogZone::phase_function_henyey_greenstein(1.0, g);
        let backward = FogZone::phase_function_henyey_greenstein(-1.0, g);
        assert!(forward > backward);
    }

    #[test]
    fn test_scene_graph_attach() {
        let mut sg = SceneGraph::new();
        let parent = sg.create_node("parent");
        let child = sg.create_node("child");
        sg.attach_child(parent, child);
        assert!(sg.nodes[&parent].children.contains(&child));
        assert_eq!(sg.nodes[&child].parent_id, Some(parent));
        assert!(!sg.root_nodes.contains(&child));
    }

    #[test]
    fn test_scene_graph_world_transform() {
        let mut sg = SceneGraph::new();
        let parent = sg.create_node("parent");
        let child = sg.create_node("child");
        sg.attach_child(parent, child);
        let t_parent = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let t_child = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        sg.nodes.get_mut(&parent).unwrap().local_transform = t_parent;
        sg.nodes.get_mut(&parent).unwrap().world_transform = t_parent;
        sg.nodes.get_mut(&child).unwrap().local_transform = t_child;
        sg.nodes.get_mut(&child).unwrap().dirty = true;
        sg.update_world_transforms();
        let child_pos = sg.nodes[&child].world_transform.w_axis.x;
        assert!((child_pos - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_scene_graph_depth() {
        let mut sg = SceneGraph::new();
        let a = sg.create_node("a");
        let b = sg.create_node("b");
        let c = sg.create_node("c");
        sg.attach_child(a, b);
        sg.attach_child(b, c);
        assert_eq!(sg.depth_of(c), 2);
        assert_eq!(sg.depth_of(a), 0);
    }

    #[test]
    fn test_scene_graph_subtree_removal() {
        let mut sg = SceneGraph::new();
        let a = sg.create_node("a");
        let b = sg.create_node("b");
        let c = sg.create_node("c");
        sg.attach_child(a, b);
        sg.attach_child(b, c);
        let removed = sg.remove_subtree(b);
        assert_eq!(removed, 2); // b and c
        assert!(sg.nodes.contains_key(&a));
        assert!(!sg.nodes.contains_key(&b));
        assert!(!sg.nodes.contains_key(&c));
    }

    #[test]
    fn test_metrics_dashboard_memory_trend() {
        let mut dash = StreamingMetricsDashboard::new(30);
        for i in 0..20 {
            dash.begin_frame(i as f64 * 0.016, 60.0);
            dash.update_memory(100.0 + i as f32 * 2.0); // linearly increasing memory
        }
        let trend = dash.memory_trend();
        assert!(trend > 0.0, "Memory trend should be positive (increasing): {}", trend);
    }

    #[test]
    fn test_metrics_dashboard_critical() {
        let mut dash = StreamingMetricsDashboard::new(10);
        dash.memory_budget_mb = 100.0;
        dash.update_memory(95.0);
        assert!(dash.is_memory_critical());
        dash.update_memory(80.0);
        assert!(!dash.is_memory_critical());
    }

    #[test]
    fn test_integrated_world_update() {
        let mut world = IntegratedStreamingWorld::new();
        world.update(Vec3::new(100.0, 10.0, 100.0), Vec3::NEG_Z, 0.016);
        assert!(world.frame_count == 1);
        assert!(world.current_time > 0.0);
    }

    #[test]
    fn test_integrated_world_fog() {
        let mut world = IntegratedStreamingWorld::new();
        let bounds = Aabb::new(Vec3::new(-500.0, -200.0, -500.0), Vec3::new(500.0, 200.0, 500.0));
        world.add_fog_zone(bounds);
        let density = world.fog_density_at(Vec3::ZERO);
        assert!(density >= 0.0 && density.is_finite());
    }

    #[test]
    fn test_integrated_world_instance_placement() {
        let mut world = IntegratedStreamingWorld::new();
        let id = world.place_level_instance(42, Vec3::new(100.0, 0.0, 100.0));
        let near = world.instances_near(Vec3::new(100.0, 0.0, 100.0), 50.0);
        assert!(near.contains(&id));
    }

    #[test]
    fn test_integrated_world_prediction() {
        let mut world = IntegratedStreamingWorld::new();
        world.camera_velocity = Vec3::new(10.0, 0.0, 0.0);
        let future = world.predicted_camera_position(1.0);
        // At t=1s: x should advance ~10 units
        assert!((future.x - world.camera_pos.x - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_terrain_patch_normal() {
        let mut patch = TerrainPatch::new(1, 0, 0, 100.0, 5);
        // Flat terrain: normal should point straight up
        for h in &mut patch.height_data { *h = 0.0; }
        let normal = patch.compute_normal_at(50.0, 50.0);
        assert!((normal.y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_terrain_patch_sloped_normal() {
        let mut patch = TerrainPatch::new(1, 0, 0, 100.0, 5);
        let res = patch.height_grid_res as usize;
        // Create an X-slope
        for z in 0..res {
            for x in 0..res {
                patch.height_data[z * res + x] = x as f32 * 1.0;
            }
        }
        let normal = patch.compute_normal_at(50.0, 50.0);
        // Should have negative X component (slope going up in +X)
        assert!(normal.x < 0.0);
        assert!(normal.is_finite());
    }

    #[test]
    fn test_scene_graph_find_by_name() {
        let mut sg = SceneGraph::new();
        let _a = sg.create_node("alpha");
        let _b = sg.create_node("beta");
        let found = sg.find_by_name("beta");
        assert!(found.is_some());
        let not_found = sg.find_by_name("gamma");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_deadline_scheduler_batch() {
        let mut sched = DeadlineScheduler::new(100.0 * 1024.0 * 1024.0);
        for i in 0..10 {
            sched.submit(i, 1.0, 2.0, 1024 * 1024, 10.0, 0.0);
        }
        let batch = sched.next_batch(0.0, 5);
        assert!(!batch.is_empty());
        assert!(batch.len() <= 5);
    }

    #[test]
    fn test_spatial_hash_aabb_query() {
        let mut sh = SpatialHash3D::new(10.0);
        sh.insert(1, Vec3::new(5.0, 0.0, 5.0));
        sh.insert(2, Vec3::new(50.0, 0.0, 50.0));
        let aabb = Aabb::new(Vec3::new(0.0, -5.0, 0.0), Vec3::new(10.0, 5.0, 10.0));
        let result = sh.query_aabb(&aabb);
        assert!(result.contains(&1));
    }

    #[test]
    fn test_level_instancer_count_by_level() {
        let mut instancer = LevelInstancer::new(100.0);
        instancer.place_instance(1, Mat4::IDENTITY);
        instancer.place_instance(1, Mat4::from_translation(Vec3::X));
        instancer.place_instance(2, Mat4::from_translation(Vec3::Y));
        let counts = instancer.count_by_level();
        assert_eq!(counts[&1], 2);
        assert_eq!(counts[&2], 1);
    }

    #[test]
    fn test_fog_zone_disabled() {
        let bounds = Aabb::new(Vec3::splat(-100.0), Vec3::splat(100.0));
        let mut zone = FogZone::new(1, bounds);
        zone.enabled = false;
        let d = zone.density_at(Vec3::ZERO, 0.0);
        assert_eq!(d, 0.0);
    }

    #[test]
    fn test_terrain_patch_stitch_skirt_nonempty() {
        let patch = TerrainPatch::new(1, 0, 0, 100.0, 3);
        let skirt = patch.stitch_skirt_vertices();
        assert!(!skirt.is_empty());
    }

    #[test]
    fn test_tile_map_add_remove_level() {
        let mut map = StreamingTileMap::new(100.0);
        map.add_level_to_tile(0, 0, 99);
        assert!(map.tiles.get(&(0, 0)).map(|t| t.level_ids.contains(&99)).unwrap_or(false));
        map.remove_level_from_all(99);
        assert!(map.tiles.get(&(0, 0)).map(|t| t.level_ids.is_empty()).unwrap_or(true));
    }

    #[test]
    fn test_deadline_scheduler_utilization() {
        let sched = DeadlineScheduler::new(1024.0);
        assert!((sched.utilization() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_spatial_hash_average_occupancy() {
        let mut sh = SpatialHash3D::new(10.0);
        sh.insert(1, Vec3::ZERO);
        sh.insert(2, Vec3::new(1.0, 0.0, 0.0));
        let occ = sh.average_occupancy();
        assert!(occ >= 1.0 && occ.is_finite());
    }

    #[test]
    fn test_integrated_world_terrain_height() {
        let mut world = IntegratedStreamingWorld::new();
        world.update(Vec3::ZERO, Vec3::NEG_Z, 0.016);
        let h = world.terrain_height_at(50.0, 50.0);
        assert!(h.is_finite());
    }

    #[test]
    fn test_metrics_dashboard_report() {
        let mut dash = StreamingMetricsDashboard::new(10);
        dash.record_load(10.0, 1024 * 1024);
        dash.record_unload(512 * 1024);
        let report = dash.report_summary();
        assert!(report.contains_key("avg_load_ms"));
        assert!((report["avg_load_ms"] - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_scene_graph_collect_subtree() {
        let mut sg = SceneGraph::new();
        let a = sg.create_node("a");
        let b = sg.create_node("b");
        let c = sg.create_node("c");
        sg.attach_child(a, b);
        sg.attach_child(a, c);
        let sub = sg.collect_subtree(a);
        assert_eq!(sub.len(), 3);
        assert!(sub.contains(&a) && sub.contains(&b) && sub.contains(&c));
    }

    #[test]
    fn test_level_instance_tags() {
        let mut inst = LevelInstance::new(1, 1, Mat4::IDENTITY);
        inst.add_tag("outdoor");
        inst.add_tag("night");
        assert!(inst.has_tag("outdoor"));
        assert!(!inst.has_tag("indoor"));
    }

    #[test]
    fn test_level_instance_scale() {
        let t = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let inst = LevelInstance::new(1, 1, t);
        let scale = inst.scale();
        assert!((scale.x - 2.0).abs() < 0.01);
        assert!((scale.y - 3.0).abs() < 0.01);
        assert!((scale.z - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_terrain_patch_vertex_count_at_lod() {
        let mut patch = TerrainPatch::new(1, 0, 0, 256.0, 4);
        // LOD 0: full res
        patch.update_lod(Vec3::new(128.0, 0.0, 128.0));
        let verts_lod0 = patch.vertex_count;
        // Move far away to trigger high LOD
        patch.update_lod(Vec3::new(10000.0, 0.0, 10000.0));
        let verts_lod_high = patch.vertex_count;
        assert!(verts_lod0 >= verts_lod_high, "Lower LOD should have fewer vertices");
    }

    #[test]
    fn test_integrated_world_schedule_load() {
        let mut world = IntegratedStreamingWorld::new();
        let req_id = world.schedule_level_load(7, 1.0, 4 * 1024 * 1024);
        assert!(req_id > 0);
        assert!(!world.deadline_scheduler.requests.is_empty());
    }

    #[test]
    fn test_fog_zone_height_falloff() {
        let bounds = Aabb::new(Vec3::new(-200.0, -100.0, -200.0), Vec3::new(200.0, 1000.0, 200.0));
        let mut zone = FogZone::new(1, bounds);
        zone.height_falloff = 0.1;
        zone.turbulence = 0.0;
        let d_low  = zone.density_at(Vec3::new(0.0, 0.0, 0.0), 0.0);
        let d_high = zone.density_at(Vec3::new(0.0, 100.0, 0.0), 0.0);
        assert!(d_low >= d_high, "Density should decrease with height");
    }

    #[test]
    fn test_spatial_hash_remove() {
        let mut sh = SpatialHash3D::new(10.0);
        sh.insert(10, Vec3::ZERO);
        sh.remove(10);
        assert_eq!(sh.object_count(), 0);
        assert!(sh.query_radius(Vec3::ZERO, 5.0).is_empty());
    }
}
