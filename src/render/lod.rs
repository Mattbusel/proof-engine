//! Level-of-detail, mesh streaming, frustum/occlusion culling, and render batching.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// LodLevel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodLevel {
    pub distance: f32,
    pub triangle_count: u32,
    pub screen_size_threshold: f32,
    pub mesh_variant: String,
}

impl LodLevel {
    pub fn new(distance: f32, triangles: u32, screen_size: f32, variant: impl Into<String>) -> Self {
        Self {
            distance,
            triangle_count: triangles,
            screen_size_threshold: screen_size,
            mesh_variant: variant.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// LodGroup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodGroup {
    pub id: u32,
    pub levels: Vec<LodLevel>,
    pub current_level: u32,
    pub position: Vec3,
    pub bounds_radius: f32,
}

impl LodGroup {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            levels: Vec::new(),
            current_level: 0,
            position: Vec3::ZERO,
            bounds_radius: 1.0,
        }
    }

    pub fn add_level(&mut self, level: LodLevel) {
        // Keep sorted by distance ascending
        self.levels.push(level);
        self.levels.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn current_level_data(&self) -> Option<&LodLevel> {
        self.levels.get(self.current_level as usize)
    }
}

// ---------------------------------------------------------------------------
// LodTransition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodTransition {
    pub from_level: u32,
    pub to_level: u32,
    pub blend: f32,
    pub blend_speed: f32,
    pub active: bool,
}

impl LodTransition {
    pub fn new(from: u32, to: u32, blend_speed: f32) -> Self {
        Self {
            from_level: from,
            to_level: to,
            blend: 0.0,
            blend_speed,
            active: true,
        }
    }

    /// Advance the transition blend. Returns true when complete.
    pub fn update(&mut self, dt: f32) -> bool {
        if !self.active {
            return true;
        }
        self.blend += dt * self.blend_speed;
        if self.blend >= 1.0 {
            self.blend = 1.0;
            self.active = false;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// LodSelector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodSelector {
    pub hysteresis: f32,
    pub bias: f32,
    pub screen_height_pixels: f32,
    pub field_of_view: f32,
    pub use_screen_size: bool,
}

impl LodSelector {
    pub fn new(screen_height: f32, fov_radians: f32) -> Self {
        Self {
            hysteresis: 0.1,
            bias: 0.0,
            screen_height_pixels: screen_height,
            field_of_view: fov_radians,
            use_screen_size: true,
        }
    }

    /// Compute projected screen size ratio for an object.
    pub fn compute_screen_size(&self, distance: f32, bounds_radius: f32) -> f32 {
        if distance < 1e-5 {
            return 1.0;
        }
        let half_fov = self.field_of_view * 0.5;
        let projected_radius = bounds_radius / (distance * half_fov.tan());
        (projected_radius * 0.5 * self.screen_height_pixels / self.screen_height_pixels).clamp(0.0, 1.0)
    }

    /// Select the optimal LOD level for given distance and screen size.
    pub fn select_lod(&self, distance: f32, screen_size: f32, levels: &[LodLevel]) -> u32 {
        if levels.is_empty() {
            return 0;
        }
        let effective_distance = distance * (1.0 + self.bias);
        let effective_screen = screen_size * (1.0 + self.bias);
        if self.use_screen_size {
            for (i, level) in levels.iter().enumerate() {
                if effective_screen >= level.screen_size_threshold {
                    return i as u32;
                }
            }
        } else {
            for (i, level) in levels.iter().enumerate() {
                if effective_distance <= level.distance {
                    return i as u32;
                }
            }
        }
        (levels.len() - 1) as u32
    }

    /// Select LOD with hysteresis to prevent popping near thresholds.
    pub fn select_lod_with_hysteresis(
        &self,
        distance: f32,
        screen_size: f32,
        levels: &[LodLevel],
        current: u32,
    ) -> u32 {
        let ideal = self.select_lod(distance, screen_size, levels);
        if ideal == current {
            return current;
        }
        // Apply hysteresis band: only switch if outside the hysteresis zone
        if ideal > current {
            // Moving to lower quality (farther away) — add hysteresis to delay
            let threshold_screen = levels.get(current as usize).map(|l| l.screen_size_threshold).unwrap_or(0.0);
            if screen_size < threshold_screen * (1.0 - self.hysteresis) {
                ideal
            } else {
                current
            }
        } else {
            // Moving to higher quality (closer) — add hysteresis to delay
            let threshold_screen = levels.get(ideal as usize).map(|l| l.screen_size_threshold).unwrap_or(1.0);
            if screen_size > threshold_screen * (1.0 + self.hysteresis) {
                ideal
            } else {
                current
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Impostor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Impostor {
    pub albedo_atlas_idx: u32,
    pub normal_atlas_idx: u32,
    pub angles: u32,
    pub parallax_depth: f32,
    pub billboard_mode: BillboardMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillboardMode {
    /// Always faces camera (2D billboard)
    FullBillboard,
    /// Only rotates around Y axis
    YAxisBillboard,
    /// Discrete octahedral angle snapping
    OctahedralImpostor,
}

impl Impostor {
    pub fn new(albedo_idx: u32, normal_idx: u32, angles: u32) -> Self {
        Self {
            albedo_atlas_idx: albedo_idx,
            normal_atlas_idx: normal_idx,
            angles,
            parallax_depth: 0.1,
            billboard_mode: BillboardMode::OctahedralImpostor,
        }
    }

    /// Compute the UV coordinates in the atlas for a given view direction.
    pub fn compute_atlas_uv(&self, view_dir: Vec3, frame_size: f32) -> (Vec2, Vec2) {
        let view_norm = view_dir.normalize();
        match self.billboard_mode {
            BillboardMode::FullBillboard | BillboardMode::YAxisBillboard => {
                // Snap to nearest pre-rendered angle
                let angle = view_norm.z.atan2(view_norm.x);
                let t = (angle / (2.0 * std::f32::consts::PI) + 1.0) % 1.0;
                let frame = (t * self.angles as f32) as u32 % self.angles;
                let frames_per_row = (self.angles as f32).sqrt().ceil() as u32;
                let col = frame % frames_per_row;
                let row = frame / frames_per_row;
                let uv_scale = frame_size / frames_per_row as f32;
                let uv_offset = Vec2::new(col as f32 * uv_scale, row as f32 * uv_scale);
                (uv_offset, Vec2::splat(uv_scale))
            }
            BillboardMode::OctahedralImpostor => {
                // Project onto octahedron
                let abs_sum = view_norm.x.abs() + view_norm.y.abs() + view_norm.z.abs();
                let oct = Vec2::new(view_norm.x / abs_sum, view_norm.y / abs_sum);
                // Fold lower hemisphere
                let oct = if view_norm.z < 0.0 {
                    Vec2::new(
                        (1.0 - oct.y.abs()) * if oct.x >= 0.0 { 1.0 } else { -1.0 },
                        (1.0 - oct.x.abs()) * if oct.y >= 0.0 { 1.0 } else { -1.0 },
                    )
                } else {
                    oct
                };
                let uv = oct * 0.5 + Vec2::splat(0.5);
                (uv, Vec2::splat(frame_size))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ImpostorCapture
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImpostorCapture {
    pub view_dirs: Vec<Vec3>,
    pub capture_size: u32,
    pub atlas_rows: u32,
    pub atlas_cols: u32,
    pub captured: bool,
}

impl ImpostorCapture {
    pub fn new_octahedral(num_views: u32, size: u32) -> Self {
        let mut view_dirs = Vec::with_capacity(num_views as usize);
        let sqrt_n = (num_views as f32).sqrt().ceil() as u32;
        for i in 0..num_views {
            let col = i % sqrt_n;
            let row = i / sqrt_n;
            let u = col as f32 / (sqrt_n - 1).max(1) as f32 * 2.0 - 1.0;
            let v = row as f32 / (sqrt_n - 1).max(1) as f32 * 2.0 - 1.0;
            // Octahedral decode
            let z = 1.0 - u.abs() - v.abs();
            let dir = if z >= 0.0 {
                Vec3::new(u, v, z).normalize()
            } else {
                Vec3::new(
                    (1.0 - v.abs()) * if u >= 0.0 { 1.0 } else { -1.0 },
                    (1.0 - u.abs()) * if v >= 0.0 { 1.0 } else { -1.0 },
                    z,
                ).normalize()
            };
            view_dirs.push(dir);
        }
        let cols = sqrt_n;
        let rows = (num_views + cols - 1) / cols;
        Self {
            view_dirs,
            capture_size: size,
            atlas_rows: rows,
            atlas_cols: cols,
            captured: false,
        }
    }

    pub fn total_atlas_size(&self) -> (u32, u32) {
        (self.atlas_cols * self.capture_size, self.atlas_rows * self.capture_size)
    }
}

// ---------------------------------------------------------------------------
// MeshStreamingRequest & Queue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MeshStreamingRequest {
    pub mesh_id: String,
    pub priority: f32,
    pub lod_needed: u32,
    pub requester_pos: Vec3,
    pub distance: f32,
}

impl MeshStreamingRequest {
    pub fn new(mesh_id: impl Into<String>, priority: f32, lod: u32) -> Self {
        Self {
            mesh_id: mesh_id.into(),
            priority,
            lod_needed: lod,
            requester_pos: Vec3::ZERO,
            distance: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshStreamingQueue {
    pub requests: Vec<MeshStreamingRequest>,
    pub max_per_frame: u32,
    pub bandwidth_limit_bytes: u64,
}

impl MeshStreamingQueue {
    pub fn new(max_per_frame: u32) -> Self {
        Self {
            requests: Vec::new(),
            max_per_frame,
            bandwidth_limit_bytes: 32 * 1024 * 1024, // 32 MB/frame default
        }
    }

    /// Insert a request, maintaining priority order (highest first).
    pub fn push(&mut self, request: MeshStreamingRequest) {
        // Replace if same mesh_id with higher priority
        if let Some(existing) = self.requests.iter_mut().find(|r| r.mesh_id == request.mesh_id) {
            if request.priority > existing.priority {
                *existing = request;
            }
            return;
        }
        let pos = self.requests.partition_point(|r| r.priority >= request.priority);
        self.requests.insert(pos, request);
    }

    /// Pop the highest priority requests for this frame.
    pub fn pop_batch(&mut self) -> Vec<MeshStreamingRequest> {
        let n = self.max_per_frame.min(self.requests.len() as u32) as usize;
        self.requests.drain(..n).collect()
    }

    /// Remove all requests for a given mesh.
    pub fn cancel(&mut self, mesh_id: &str) {
        self.requests.retain(|r| r.mesh_id != mesh_id);
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Update priorities based on camera movement.
    pub fn update_priorities(&mut self, camera_pos: Vec3) {
        for req in &mut self.requests {
            let dist = (req.requester_pos - camera_pos).length();
            req.distance = dist;
            // Priority decays with distance
            req.priority = 1.0 / (1.0 + dist * 0.01);
        }
        self.requests.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
    }
}

// ---------------------------------------------------------------------------
// MeshStreamingCache — LRU cache with memory budget
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CachedMesh {
    pub mesh_id: String,
    pub lod: u32,
    pub size_bytes: u64,
    pub last_used_frame: u64,
    pub vertex_data: Vec<f32>,
    pub index_data: Vec<u32>,
}

impl CachedMesh {
    pub fn new(mesh_id: impl Into<String>, lod: u32, vertices: Vec<f32>, indices: Vec<u32>) -> Self {
        let size_bytes = (vertices.len() * 4 + indices.len() * 4) as u64;
        Self {
            mesh_id: mesh_id.into(),
            lod,
            size_bytes,
            last_used_frame: 0,
            vertex_data: vertices,
            index_data: indices,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshStreamingCache {
    pub meshes: HashMap<String, CachedMesh>,
    pub budget_bytes: u64,
    pub used_bytes: u64,
    pub current_frame: u64,
    pub eviction_count: u64,
}

impl MeshStreamingCache {
    pub fn new(budget_mb: u64) -> Self {
        Self {
            meshes: HashMap::new(),
            budget_bytes: budget_mb * 1024 * 1024,
            used_bytes: 0,
            current_frame: 0,
            eviction_count: 0,
        }
    }

    /// Insert a mesh, evicting LRU entries if over budget.
    pub fn insert(&mut self, mesh: CachedMesh) {
        let key = format!("{}_{}", mesh.mesh_id, mesh.lod);
        // Remove if already present
        if let Some(old) = self.meshes.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(old.size_bytes);
        }
        // Evict until we have room
        while self.used_bytes + mesh.size_bytes > self.budget_bytes && !self.meshes.is_empty() {
            self.evict_lru();
        }
        self.used_bytes += mesh.size_bytes;
        self.meshes.insert(key, mesh);
    }

    fn evict_lru(&mut self) {
        let oldest_key = self.meshes.iter()
            .min_by_key(|(_, m)| m.last_used_frame)
            .map(|(k, _)| k.clone());
        if let Some(key) = oldest_key {
            if let Some(evicted) = self.meshes.remove(&key) {
                self.used_bytes = self.used_bytes.saturating_sub(evicted.size_bytes);
                self.eviction_count += 1;
            }
        }
    }

    pub fn get(&mut self, mesh_id: &str, lod: u32) -> Option<&CachedMesh> {
        let key = format!("{}_{}", mesh_id, lod);
        if let Some(mesh) = self.meshes.get_mut(&key) {
            mesh.last_used_frame = self.current_frame;
            Some(mesh)
        } else {
            None
        }
    }

    pub fn contains(&self, mesh_id: &str, lod: u32) -> bool {
        let key = format!("{}_{}", mesh_id, lod);
        self.meshes.contains_key(&key)
    }

    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
    }

    pub fn usage_ratio(&self) -> f32 {
        if self.budget_bytes > 0 {
            self.used_bytes as f32 / self.budget_bytes as f32
        } else {
            0.0
        }
    }

    pub fn clear(&mut self) {
        self.meshes.clear();
        self.used_bytes = 0;
    }
}

// ---------------------------------------------------------------------------
// FrustumCuller
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    /// Planes in order: left, right, bottom, top, near, far.
    /// Vec4: (nx, ny, nz, d) where nx*x + ny*y + nz*z + d >= 0 means inside.
    pub planes: [Vec4; 6],
}

impl Frustum {
    pub fn new() -> Self {
        Self { planes: [Vec4::ZERO; 6] }
    }

    /// Extract frustum planes from a combined view-projection matrix.
    pub fn from_view_proj(vp: Mat4) -> Self {
        // Gribb/Hartmann method
        let c = vp.to_cols_array_2d();
        // c[col][row] in column-major layout
        // Row 0..3 of matrix
        let row0 = Vec4::new(c[0][0], c[1][0], c[2][0], c[3][0]);
        let row1 = Vec4::new(c[0][1], c[1][1], c[2][1], c[3][1]);
        let row2 = Vec4::new(c[0][2], c[1][2], c[2][2], c[3][2]);
        let row3 = Vec4::new(c[0][3], c[1][3], c[2][3], c[3][3]);

        let planes = [
            Self::normalize_plane(row3 + row0), // Left
            Self::normalize_plane(row3 - row0), // Right
            Self::normalize_plane(row3 + row1), // Bottom
            Self::normalize_plane(row3 - row1), // Top
            Self::normalize_plane(row3 + row2), // Near
            Self::normalize_plane(row3 - row2), // Far
        ];
        Self { planes }
    }

    fn normalize_plane(p: Vec4) -> Vec4 {
        let len = Vec3::new(p.x, p.y, p.z).length();
        if len > 1e-6 {
            p / len
        } else {
            p
        }
    }

    fn plane_signed_dist(plane: Vec4, point: Vec3) -> f32 {
        plane.x * point.x + plane.y * point.y + plane.z * point.z + plane.w
    }

    /// Test an AABB (axis-aligned bounding box) against the frustum.
    /// Returns true if the AABB is possibly visible (not fully outside any plane).
    pub fn test_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Find the AABB corner most in the direction of the plane normal (positive vertex)
            let px = if plane.x >= 0.0 { max.x } else { min.x };
            let py = if plane.y >= 0.0 { max.y } else { min.y };
            let pz = if plane.z >= 0.0 { max.z } else { min.z };
            if Self::plane_signed_dist(*plane, Vec3::new(px, py, pz)) < 0.0 {
                return false; // Fully outside this plane
            }
        }
        true
    }

    /// Test a sphere against the frustum.
    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if Self::plane_signed_dist(*plane, center) < -radius {
                return false;
            }
        }
        true
    }

    /// Containment test: returns true if AABB is fully inside all planes.
    pub fn contains_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Test all 8 corners
            let corners = [
                Vec3::new(min.x, min.y, min.z),
                Vec3::new(max.x, min.y, min.z),
                Vec3::new(min.x, max.y, min.z),
                Vec3::new(max.x, max.y, min.z),
                Vec3::new(min.x, min.y, max.z),
                Vec3::new(max.x, min.y, max.z),
                Vec3::new(min.x, max.y, max.z),
                Vec3::new(max.x, max.y, max.z),
            ];
            for &corner in &corners {
                if Self::plane_signed_dist(*plane, corner) < 0.0 {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for Frustum {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// OcclusionCuller — hierarchical Z-buffer (software)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HiZBuffer {
    pub levels: Vec<Vec<f32>>,
    pub widths: Vec<u32>,
    pub heights: Vec<u32>,
    pub num_levels: u32,
}

impl HiZBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let mut levels = Vec::new();
        let mut widths = Vec::new();
        let mut heights = Vec::new();
        let mut w = width;
        let mut h = height;
        while w >= 1 && h >= 1 {
            levels.push(vec![1.0f32; (w * h) as usize]);
            widths.push(w);
            heights.push(h);
            if w == 1 && h == 1 {
                break;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        let num_levels = levels.len() as u32;
        Self { levels, widths, heights, num_levels }
    }

    /// Build mip chain from base depth buffer.
    pub fn build_from_depth(&mut self, base_depth: &[f32]) {
        if self.levels.is_empty() {
            return;
        }
        // Copy base
        let len = base_depth.len().min(self.levels[0].len());
        self.levels[0][..len].copy_from_slice(&base_depth[..len]);
        // Build mips by taking max (conservative: farthest depth)
        for level in 1..self.num_levels as usize {
            let w = self.widths[level] as usize;
            let h = self.heights[level] as usize;
            let pw = self.widths[level - 1] as usize;
            let ph = self.heights[level - 1] as usize;
            let prev = self.levels[level - 1].clone();
            let cur = &mut self.levels[level];
            for y in 0..h {
                for x in 0..w {
                    let px = (x * 2).min(pw - 1);
                    let py = (y * 2).min(ph - 1);
                    let px1 = (px + 1).min(pw - 1);
                    let py1 = (py + 1).min(ph - 1);
                    let d00 = prev[py * pw + px];
                    let d10 = prev[py * pw + px1];
                    let d01 = prev[py1 * pw + px];
                    let d11 = prev[py1 * pw + px1];
                    cur[y * w + x] = d00.max(d10).max(d01).max(d11);
                }
            }
        }
    }

    /// Sample a mip level at a normalized UV.
    pub fn sample(&self, level: u32, uv: Vec2) -> f32 {
        let l = level.min(self.num_levels - 1) as usize;
        let w = self.widths[l];
        let h = self.heights[l];
        let x = (uv.x * w as f32).clamp(0.0, (w - 1) as f32) as u32;
        let y = (uv.y * h as f32).clamp(0.0, (h - 1) as f32) as u32;
        self.levels[l][(y * w + x) as usize]
    }
}

#[derive(Debug, Clone)]
pub struct OcclusionCuller {
    pub hiz: HiZBuffer,
    pub occlusion_threshold: f32,
    pub enabled: bool,
    pub query_count: u64,
    pub culled_count: u64,
}

impl OcclusionCuller {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            hiz: HiZBuffer::new(width, height),
            occlusion_threshold: 0.0,
            enabled: true,
            query_count: 0,
            culled_count: 0,
        }
    }

    pub fn update_depth(&mut self, depth_buffer: &[f32]) {
        self.hiz.build_from_depth(depth_buffer);
    }

    /// Test if an AABB is visible given a view-projection matrix.
    /// Returns true if visible, false if occluded.
    pub fn test_aabb_visibility(&mut self, min_ws: Vec3, max_ws: Vec3, view_proj: Mat4) -> bool {
        if !self.enabled {
            return true;
        }
        self.query_count += 1;
        // Project AABB corners to screen space
        let corners = [
            Vec3::new(min_ws.x, min_ws.y, min_ws.z),
            Vec3::new(max_ws.x, min_ws.y, min_ws.z),
            Vec3::new(min_ws.x, max_ws.y, min_ws.z),
            Vec3::new(max_ws.x, max_ws.y, min_ws.z),
            Vec3::new(min_ws.x, min_ws.y, max_ws.z),
            Vec3::new(max_ws.x, min_ws.y, max_ws.z),
            Vec3::new(min_ws.x, max_ws.y, max_ws.z),
            Vec3::new(max_ws.x, max_ws.y, max_ws.z),
        ];
        let mut min_x = 1.0f32;
        let mut max_x = 0.0f32;
        let mut min_y = 1.0f32;
        let mut max_y = 0.0f32;
        let mut min_depth = f32::MAX;
        for &corner in &corners {
            let clip = view_proj.project_point3(corner);
            if clip.z < 0.0 {
                return true; // Behind camera, don't cull
            }
            let sx = clip.x * 0.5 + 0.5;
            let sy = clip.y * 0.5 + 0.5;
            min_x = min_x.min(sx);
            max_x = max_x.max(sx);
            min_y = min_y.min(sy);
            max_y = max_y.max(sy);
            min_depth = min_depth.min(clip.z);
        }
        if min_x > 1.0 || max_x < 0.0 || min_y > 1.0 || max_y < 0.0 {
            self.culled_count += 1;
            return false; // Outside screen
        }
        // Choose appropriate mip level based on AABB screen footprint
        let screen_w = (max_x - min_x) * self.hiz.widths[0] as f32;
        let screen_h = (max_y - min_y) * self.hiz.heights[0] as f32;
        let max_dim = screen_w.max(screen_h).max(1.0);
        let level = (max_dim.log2().ceil() as u32).min(self.hiz.num_levels - 1);
        // Sample HiZ at the AABB's screen extents
        let center_uv = Vec2::new(
            (min_x + max_x) * 0.5,
            (min_y + max_y) * 0.5,
        );
        let hiz_depth = self.hiz.sample(level, center_uv);
        if min_depth > hiz_depth + self.occlusion_threshold {
            self.culled_count += 1;
            false
        } else {
            true
        }
    }

    pub fn reset_stats(&mut self) {
        self.query_count = 0;
        self.culled_count = 0;
    }

    pub fn cull_ratio(&self) -> f32 {
        if self.query_count > 0 {
            self.culled_count as f32 / self.query_count as f32
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// DrawCall
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DrawCall {
    pub mesh_id: String,
    pub material_id: String,
    pub transform: Mat4,
    pub lod: u32,
    pub flags: u32,
    pub depth: f32,
    pub batch_id: u32,
}

impl DrawCall {
    pub const FLAG_OPAQUE: u32 = 1 << 0;
    pub const FLAG_TRANSPARENT: u32 = 1 << 1;
    pub const FLAG_CAST_SHADOW: u32 = 1 << 2;
    pub const FLAG_RECEIVE_SHADOW: u32 = 1 << 3;
    pub const FLAG_SKINNED: u32 = 1 << 4;
    pub const FLAG_INSTANCED: u32 = 1 << 5;

    pub fn new(mesh_id: impl Into<String>, material_id: impl Into<String>, transform: Mat4) -> Self {
        Self {
            mesh_id: mesh_id.into(),
            material_id: material_id.into(),
            transform,
            lod: 0,
            flags: Self::FLAG_OPAQUE | Self::FLAG_CAST_SHADOW | Self::FLAG_RECEIVE_SHADOW,
            depth: 0.0,
            batch_id: 0,
        }
    }

    pub fn is_opaque(&self) -> bool { self.flags & Self::FLAG_OPAQUE != 0 }
    pub fn is_transparent(&self) -> bool { self.flags & Self::FLAG_TRANSPARENT != 0 }
    pub fn casts_shadow(&self) -> bool { self.flags & Self::FLAG_CAST_SHADOW != 0 }
    pub fn is_skinned(&self) -> bool { self.flags & Self::FLAG_SKINNED != 0 }
    pub fn is_instanced(&self) -> bool { self.flags & Self::FLAG_INSTANCED != 0 }
}

// ---------------------------------------------------------------------------
// RenderBatcher
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RenderBatch {
    pub batch_id: u32,
    pub material_id: String,
    pub mesh_id: String,
    pub draw_calls: Vec<DrawCall>,
    pub is_opaque: bool,
    pub depth_sort_key: f32,
    pub instance_count: u32,
}

impl RenderBatch {
    pub fn new(id: u32, material_id: impl Into<String>, mesh_id: impl Into<String>, opaque: bool) -> Self {
        Self {
            batch_id: id,
            material_id: material_id.into(),
            mesh_id: mesh_id.into(),
            draw_calls: Vec::new(),
            is_opaque: opaque,
            depth_sort_key: 0.0,
            instance_count: 0,
        }
    }

    pub fn add_draw_call(&mut self, mut dc: DrawCall) {
        dc.batch_id = self.batch_id;
        self.depth_sort_key = if self.is_opaque {
            self.depth_sort_key.min(dc.depth)
        } else {
            self.depth_sort_key.max(dc.depth)
        };
        self.instance_count += 1;
        self.draw_calls.push(dc);
    }
}

#[derive(Debug, Clone)]
pub struct RenderBatcher {
    pub opaque_batches: Vec<RenderBatch>,
    pub transparent_batches: Vec<RenderBatch>,
    pub shadow_draw_calls: Vec<DrawCall>,
    pub batch_id_counter: u32,
    pub max_instances_per_batch: u32,
    pub stats: BatcherStats,
}

#[derive(Debug, Clone, Default)]
pub struct BatcherStats {
    pub total_draw_calls: u32,
    pub batched_draw_calls: u32,
    pub opaque_batches: u32,
    pub transparent_batches: u32,
    pub shadow_casters: u32,
    pub frustum_culled: u32,
    pub occlusion_culled: u32,
}

impl RenderBatcher {
    pub fn new() -> Self {
        Self {
            opaque_batches: Vec::new(),
            transparent_batches: Vec::new(),
            shadow_draw_calls: Vec::new(),
            batch_id_counter: 0,
            max_instances_per_batch: 512,
            stats: BatcherStats::default(),
        }
    }

    pub fn clear(&mut self) {
        self.opaque_batches.clear();
        self.transparent_batches.clear();
        self.shadow_draw_calls.clear();
        self.stats = BatcherStats::default();
    }

    fn batch_key(dc: &DrawCall) -> String {
        format!("{}_{}_{}_{}", dc.material_id, dc.mesh_id, dc.lod, dc.flags & DrawCall::FLAG_SKINNED)
    }

    /// Submit a draw call for batching. Camera_pos used for depth sorting.
    pub fn submit(&mut self, mut dc: DrawCall, camera_pos: Vec3) {
        // Compute depth from camera to object translation
        let obj_pos = Vec3::new(dc.transform.w_axis.x, dc.transform.w_axis.y, dc.transform.w_axis.z);
        dc.depth = (obj_pos - camera_pos).length();
        self.stats.total_draw_calls += 1;
        if dc.casts_shadow() {
            self.shadow_draw_calls.push(dc.clone());
            self.stats.shadow_casters += 1;
        }
        let key = Self::batch_key(&dc);
        if dc.is_opaque() {
            let max_inst = self.max_instances_per_batch;
            // Find existing batch with same key and room for more instances
            let existing = self.opaque_batches.iter_mut().find(|b| {
                b.material_id == dc.material_id
                    && b.mesh_id == dc.mesh_id
                    && b.instance_count < max_inst
            });
            if let Some(batch) = existing {
                batch.add_draw_call(dc);
            } else {
                let id = self.batch_id_counter;
                self.batch_id_counter += 1;
                let mut batch = RenderBatch::new(id, &dc.material_id, &dc.mesh_id, true);
                batch.add_draw_call(dc);
                self.opaque_batches.push(batch);
            }
        } else {
            // Transparent: generally not batched (need depth sort)
            let id = self.batch_id_counter;
            self.batch_id_counter += 1;
            let mut batch = RenderBatch::new(id, &dc.material_id, &dc.mesh_id, false);
            batch.add_draw_call(dc);
            self.transparent_batches.push(batch);
        }
        let _ = key;
    }

    /// Sort opaque batches front-to-back, transparent batches back-to-front.
    pub fn sort(&mut self) {
        // Front-to-back for opaque (minimize overdraw)
        self.opaque_batches.sort_by(|a, b| {
            a.depth_sort_key.partial_cmp(&b.depth_sort_key).unwrap_or(std::cmp::Ordering::Equal)
        });
        // Back-to-front for transparent (correct alpha blending)
        self.transparent_batches.sort_by(|a, b| {
            b.depth_sort_key.partial_cmp(&a.depth_sort_key).unwrap_or(std::cmp::Ordering::Equal)
        });
        self.stats.opaque_batches = self.opaque_batches.len() as u32;
        self.stats.transparent_batches = self.transparent_batches.len() as u32;
        self.stats.batched_draw_calls = self.stats.total_draw_calls;
    }

    /// Cull draw calls against a frustum before batching.
    pub fn cull_and_submit(
        &mut self,
        draw_calls: Vec<DrawCall>,
        frustum: &Frustum,
        camera_pos: Vec3,
        bounds: &HashMap<String, (Vec3, Vec3)>, // mesh_id -> (aabb_min, aabb_max)
    ) {
        for dc in draw_calls {
            if let Some(&(aabb_min, aabb_max)) = bounds.get(&dc.mesh_id) {
                // Transform AABB to world space (simple: translate by object position)
                let obj_pos = Vec3::new(dc.transform.w_axis.x, dc.transform.w_axis.y, dc.transform.w_axis.z);
                let ws_min = aabb_min + obj_pos;
                let ws_max = aabb_max + obj_pos;
                if !frustum.test_aabb(ws_min, ws_max) {
                    self.stats.frustum_culled += 1;
                    continue;
                }
            }
            self.submit(dc, camera_pos);
        }
        self.sort();
    }

    pub fn total_instance_count(&self) -> u32 {
        let opaque: u32 = self.opaque_batches.iter().map(|b| b.instance_count).sum();
        let transparent: u32 = self.transparent_batches.iter().map(|b| b.instance_count).sum();
        opaque + transparent
    }

    pub fn all_batches(&self) -> impl Iterator<Item = &RenderBatch> {
        self.opaque_batches.iter().chain(self.transparent_batches.iter())
    }
}

impl Default for RenderBatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LodManager — top-level LOD system integrating all components
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LodManager {
    pub groups: HashMap<u32, LodGroup>,
    pub transitions: HashMap<u32, LodTransition>,
    pub selector: LodSelector,
    pub streaming_queue: MeshStreamingQueue,
    pub streaming_cache: MeshStreamingCache,
    pub impostor_threshold_distance: f32,
    pub impostors: HashMap<u32, Impostor>,
    pub next_group_id: u32,
}

impl LodManager {
    pub fn new(screen_height: f32, fov: f32) -> Self {
        Self {
            groups: HashMap::new(),
            transitions: HashMap::new(),
            selector: LodSelector::new(screen_height, fov),
            streaming_queue: MeshStreamingQueue::new(8),
            streaming_cache: MeshStreamingCache::new(512),
            impostor_threshold_distance: 500.0,
            impostors: HashMap::new(),
            next_group_id: 1,
        }
    }

    pub fn add_group(&mut self, group: LodGroup) {
        let id = group.id;
        self.groups.insert(id, group);
    }

    pub fn create_group(&mut self, position: Vec3, bounds_radius: f32) -> u32 {
        let id = self.next_group_id;
        self.next_group_id += 1;
        let mut group = LodGroup::new(id);
        group.position = position;
        group.bounds_radius = bounds_radius;
        self.groups.insert(id, group);
        id
    }

    pub fn add_impostor(&mut self, group_id: u32, impostor: Impostor) {
        self.impostors.insert(group_id, impostor);
    }

    /// Update LOD for all groups given camera position.
    pub fn update(&mut self, camera_pos: Vec3, dt: f32) {
        let groups: Vec<(u32, Vec3, f32, Vec<LodLevel>, u32)> = self.groups.values()
            .map(|g| (g.id, g.position, g.bounds_radius, g.levels.clone(), g.current_level))
            .collect();

        for (id, pos, radius, levels, current) in groups {
            let dist = (pos - camera_pos).length();
            let screen_size = self.selector.compute_screen_size(dist, radius);
            let new_level = self.selector.select_lod_with_hysteresis(dist, screen_size, &levels, current);

            if new_level != current {
                // Start a cross-fade transition
                let transition = LodTransition::new(current, new_level, 2.0);
                self.transitions.insert(id, transition);
                if let Some(group) = self.groups.get_mut(&id) {
                    group.current_level = new_level;
                }
                // Request streaming if needed
                if let Some(level_data) = levels.get(new_level as usize) {
                    if !self.streaming_cache.contains(&level_data.mesh_variant, new_level) {
                        let priority = 1.0 / (1.0 + dist * 0.01);
                        let mut req = MeshStreamingRequest::new(&level_data.mesh_variant, priority, new_level);
                        req.requester_pos = pos;
                        req.distance = dist;
                        self.streaming_queue.push(req);
                    }
                }
            }
        }

        // Update transitions
        let completed: Vec<u32> = self.transitions.iter_mut()
            .filter_map(|(&id, t)| if t.update(dt) { Some(id) } else { None })
            .collect();
        for id in completed {
            self.transitions.remove(&id);
        }

        self.streaming_cache.advance_frame();
        self.streaming_queue.update_priorities(camera_pos);
    }

    /// Get the current LOD for a group, accounting for active transitions.
    pub fn get_lod_blend(&self, group_id: u32) -> (u32, u32, f32) {
        if let Some(transition) = self.transitions.get(&group_id) {
            (transition.from_level, transition.to_level, transition.blend)
        } else if let Some(group) = self.groups.get(&group_id) {
            (group.current_level, group.current_level, 1.0)
        } else {
            (0, 0, 1.0)
        }
    }

    /// Process streaming requests and (mock) load mesh data.
    pub fn process_streaming(&mut self) {
        let batch = self.streaming_queue.pop_batch();
        for req in batch {
            // In a real implementation, this would async-load from disk.
            // Here we create placeholder geometry.
            let vertex_count = (1000 / (req.lod_needed + 1).max(1)) * 8;
            let index_count = vertex_count / 4 * 6;
            let vertices = vec![0.0f32; vertex_count as usize];
            let indices: Vec<u32> = (0..index_count).collect();
            let mesh = CachedMesh::new(&req.mesh_id, req.lod_needed, vertices, indices);
            self.streaming_cache.insert(mesh);
        }
    }

    pub fn group_count(&self) -> usize { self.groups.len() }
    pub fn active_transitions(&self) -> usize { self.transitions.len() }
    pub fn pending_streams(&self) -> usize { self.streaming_queue.len() }
}
