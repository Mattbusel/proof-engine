//! Shadow mapping subsystem for Proof Engine.
//!
//! Provides depth-buffer shadow maps, cascaded shadow maps for directional lights,
//! omnidirectional shadow maps for point lights (cubemap layout), shadow atlas packing,
//! PCF filtering, variance shadow maps, configurable bias, distance fade, and shadow
//! caster culling.

use super::lights::{Vec3, Mat4, Color, LightId, Light, CascadeShadowParams};
use std::collections::HashMap;

// ── Shadow Map ──────────────────────────────────────────────────────────────

/// A single 2D depth buffer for shadow mapping.
#[derive(Debug, Clone)]
pub struct ShadowMap {
    /// Width of the depth buffer in texels.
    pub width: u32,
    /// Height of the depth buffer in texels.
    pub height: u32,
    /// Depth values stored as a flat row-major array. 1.0 = far, 0.0 = near.
    pub depth_buffer: Vec<f32>,
    /// View-projection matrix used when rendering to this shadow map.
    pub view_projection: Mat4,
    /// Near plane distance.
    pub near: f32,
    /// Far plane distance.
    pub far: f32,
}

impl ShadowMap {
    /// Create a new shadow map with the given resolution.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            depth_buffer: vec![1.0; size],
            view_projection: Mat4::IDENTITY,
            near: 0.1,
            far: 100.0,
        }
    }

    /// Clear the depth buffer to the far value.
    pub fn clear(&mut self) {
        for d in self.depth_buffer.iter_mut() {
            *d = 1.0;
        }
    }

    /// Write a depth value at the given texel coordinates.
    pub fn write_depth(&mut self, x: u32, y: u32, depth: f32) {
        if x < self.width && y < self.height {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            if depth < self.depth_buffer[idx] {
                self.depth_buffer[idx] = depth;
            }
        }
    }

    /// Read the depth at the given texel coordinates.
    pub fn read_depth(&self, x: u32, y: u32) -> f32 {
        if x < self.width && y < self.height {
            self.depth_buffer[(y as usize) * (self.width as usize) + (x as usize)]
        } else {
            1.0
        }
    }

    /// Sample depth with bilinear filtering at normalized UV coordinates.
    pub fn sample_bilinear(&self, u: f32, v: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let fx = u * (self.width as f32 - 1.0);
        let fy = v * (self.height as f32 - 1.0);

        let x0 = fx.floor() as u32;
        let y0 = fy.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let frac_x = fx - fx.floor();
        let frac_y = fy - fy.floor();

        let d00 = self.read_depth(x0, y0);
        let d10 = self.read_depth(x1, y0);
        let d01 = self.read_depth(x0, y1);
        let d11 = self.read_depth(x1, y1);

        let top = d00 + (d10 - d00) * frac_x;
        let bottom = d01 + (d11 - d01) * frac_x;
        top + (bottom - top) * frac_y
    }

    /// Project a world-space point into shadow map UV + depth.
    pub fn project_point(&self, world_pos: Vec3) -> (f32, f32, f32) {
        let clip = self.view_projection.transform_point(world_pos);
        let u = clip.x * 0.5 + 0.5;
        let v = clip.y * 0.5 + 0.5;
        let depth = clip.z * 0.5 + 0.5;
        (u, v, depth)
    }

    /// Test if a world-space point is in shadow (simple depth comparison).
    pub fn is_in_shadow(&self, world_pos: Vec3, bias: f32) -> bool {
        let (u, v, depth) = self.project_point(world_pos);
        if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
            return false; // Outside shadow map
        }
        let stored_depth = self.sample_bilinear(u, v);
        depth - bias > stored_depth
    }

    /// Rasterize a triangle into the shadow map depth buffer.
    pub fn rasterize_triangle(&mut self, v0: Vec3, v1: Vec3, v2: Vec3) {
        let p0 = self.view_projection.transform_point(v0);
        let p1 = self.view_projection.transform_point(v1);
        let p2 = self.view_projection.transform_point(v2);

        // Convert to screen space
        let sx0 = (p0.x * 0.5 + 0.5) * self.width as f32;
        let sy0 = (p0.y * 0.5 + 0.5) * self.height as f32;
        let sz0 = p0.z * 0.5 + 0.5;

        let sx1 = (p1.x * 0.5 + 0.5) * self.width as f32;
        let sy1 = (p1.y * 0.5 + 0.5) * self.height as f32;
        let sz1 = p1.z * 0.5 + 0.5;

        let sx2 = (p2.x * 0.5 + 0.5) * self.width as f32;
        let sy2 = (p2.y * 0.5 + 0.5) * self.height as f32;
        let sz2 = p2.z * 0.5 + 0.5;

        // Compute bounding box
        let min_x = sx0.min(sx1).min(sx2).max(0.0) as u32;
        let max_x = sx0.max(sx1).max(sx2).min(self.width as f32 - 1.0) as u32;
        let min_y = sy0.min(sy1).min(sy2).max(0.0) as u32;
        let max_y = sy0.max(sy1).max(sy2).min(self.height as f32 - 1.0) as u32;

        // Rasterize with barycentric coordinates
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;

                let area = edge_function(sx0, sy0, sx1, sy1, sx2, sy2);
                if area.abs() < 1e-10 {
                    continue;
                }

                let w0 = edge_function(sx1, sy1, sx2, sy2, px, py);
                let w1 = edge_function(sx2, sy2, sx0, sy0, px, py);
                let w2 = edge_function(sx0, sy0, sx1, sy1, px, py);

                if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                    let inv_area = 1.0 / area;
                    let b0 = w0 * inv_area;
                    let b1 = w1 * inv_area;
                    let b2 = w2 * inv_area;

                    let depth = sz0 * b0 + sz1 * b1 + sz2 * b2;
                    self.write_depth(x, y, depth.clamp(0.0, 1.0));
                }
            }
        }
    }

    /// Get the total number of texels.
    pub fn texel_count(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    /// Get memory usage in bytes (approximate).
    pub fn memory_bytes(&self) -> usize {
        self.depth_buffer.len() * 4
    }
}

/// Edge function for triangle rasterization.
fn edge_function(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    (cx - ax) * (by - ay) - (cy - ay) * (bx - ax)
}

// ── Cascaded Shadow Map ─────────────────────────────────────────────────────

/// Shadow mapping for directional lights using cascaded shadow maps.
/// Splits the view frustum into 4 cascades for better shadow resolution distribution.
#[derive(Debug, Clone)]
pub struct CascadedShadowMap {
    /// One shadow map per cascade (up to 4).
    pub cascades: [ShadowMap; 4],
    /// Number of active cascades (1..=4).
    pub cascade_count: u32,
    /// The view-projection matrix for each cascade.
    pub cascade_vp: [Mat4; 4],
    /// Split distances in view space.
    pub split_distances: [f32; 5],
    /// Resolution per cascade.
    pub resolution: u32,
    /// Whether to blend between cascades.
    pub blend_cascades: bool,
    /// Blend band width in normalized split space.
    pub blend_band: f32,
}

impl CascadedShadowMap {
    pub fn new(resolution: u32, cascade_count: u32) -> Self {
        let count = cascade_count.clamp(1, 4);
        Self {
            cascades: [
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
            ],
            cascade_count: count,
            cascade_vp: [Mat4::IDENTITY; 4],
            split_distances: [0.1, 10.0, 30.0, 80.0, 200.0],
            resolution,
            blend_cascades: true,
            blend_band: 0.1,
        }
    }

    /// Update cascade splits using the given parameters.
    pub fn update_splits(&mut self, params: &CascadeShadowParams) {
        self.cascade_count = params.cascade_count.min(4);
        self.split_distances = params.split_distances;
        self.blend_band = params.blend_band;
    }

    /// Set the view-projection matrix for a specific cascade.
    pub fn set_cascade_vp(&mut self, cascade: usize, vp: Mat4) {
        if cascade < 4 {
            self.cascade_vp[cascade] = vp;
            self.cascades[cascade].view_projection = vp;
        }
    }

    /// Compute cascade view-projection matrices from light direction and camera frustum.
    pub fn compute_cascade_matrices(
        &mut self,
        light_dir: Vec3,
        camera_frustum_slices: &[[Vec3; 8]; 4],
        params: &CascadeShadowParams,
    ) {
        self.update_splits(params);
        let count = self.cascade_count as usize;
        for i in 0..count {
            let vp = params.cascade_view_projection(light_dir, &camera_frustum_slices[i]);
            self.set_cascade_vp(i, vp);
        }
    }

    /// Clear all cascade depth buffers.
    pub fn clear_all(&mut self) {
        for i in 0..self.cascade_count as usize {
            self.cascades[i].clear();
        }
    }

    /// Determine which cascade a view-space depth falls into.
    pub fn select_cascade(&self, view_depth: f32) -> usize {
        for i in 0..self.cascade_count as usize {
            if view_depth < self.split_distances[i + 1] {
                return i;
            }
        }
        (self.cascade_count as usize).saturating_sub(1)
    }

    /// Test if a point is in shadow, using the appropriate cascade.
    pub fn is_in_shadow(&self, world_pos: Vec3, view_depth: f32, bias: &ShadowBias) -> bool {
        let cascade = self.select_cascade(view_depth);
        let effective_bias = bias.compute(0.0, 0.0); // Simplified — needs surface info
        self.cascades[cascade].is_in_shadow(world_pos, effective_bias)
    }

    /// Compute shadow factor with cascade blending (0.0 = fully shadowed, 1.0 = fully lit).
    pub fn shadow_factor(
        &self,
        world_pos: Vec3,
        view_depth: f32,
        bias: &ShadowBias,
    ) -> f32 {
        let cascade = self.select_cascade(view_depth);
        let effective_bias = bias.compute(0.0, 0.0);
        let in_shadow = self.cascades[cascade].is_in_shadow(world_pos, effective_bias);

        let base_factor = if in_shadow { 0.0 } else { 1.0 };

        if !self.blend_cascades || cascade + 1 >= self.cascade_count as usize {
            return base_factor;
        }

        // Check if we're in the blend band
        let split_near = self.split_distances[cascade + 1];
        let blend_start = split_near * (1.0 - self.blend_band);
        if view_depth > blend_start {
            let blend_t = (view_depth - blend_start) / (split_near - blend_start);
            let next_in_shadow = self.cascades[cascade + 1].is_in_shadow(world_pos, effective_bias);
            let next_factor = if next_in_shadow { 0.0 } else { 1.0 };
            base_factor * (1.0 - blend_t) + next_factor * blend_t
        } else {
            base_factor
        }
    }

    /// Compute frustum slice corners for a cascade given camera near/far/projection.
    pub fn compute_frustum_slice(
        near: f32,
        far: f32,
        fov_y: f32,
        aspect: f32,
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_up: Vec3,
    ) -> [Vec3; 8] {
        let camera_right = camera_forward.cross(camera_up).normalize();
        let corrected_up = camera_right.cross(camera_forward).normalize();

        let near_h = (fov_y * 0.5).tan() * near;
        let near_w = near_h * aspect;
        let far_h = (fov_y * 0.5).tan() * far;
        let far_w = far_h * aspect;

        let near_center = camera_pos + camera_forward * near;
        let far_center = camera_pos + camera_forward * far;

        [
            near_center + corrected_up * near_h - camera_right * near_w,
            near_center + corrected_up * near_h + camera_right * near_w,
            near_center - corrected_up * near_h + camera_right * near_w,
            near_center - corrected_up * near_h - camera_right * near_w,
            far_center + corrected_up * far_h - camera_right * far_w,
            far_center + corrected_up * far_h + camera_right * far_w,
            far_center - corrected_up * far_h + camera_right * far_w,
            far_center - corrected_up * far_h - camera_right * far_w,
        ]
    }

    /// Get total memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        let count = self.cascade_count as usize;
        (0..count).map(|i| self.cascades[i].memory_bytes()).sum()
    }
}

// ── Omnidirectional Shadow Map ──────────────────────────────────────────────

/// Shadow map for point lights using a 6-face cubemap layout.
#[derive(Debug, Clone)]
pub struct OmniShadowMap {
    /// Six shadow map faces: +X, -X, +Y, -Y, +Z, -Z.
    pub faces: [ShadowMap; 6],
    /// The light's position.
    pub light_position: Vec3,
    /// The light's radius (far plane).
    pub radius: f32,
    /// Resolution per face.
    pub resolution: u32,
}

/// The six cube faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeFace {
    PositiveX = 0,
    NegativeX = 1,
    PositiveY = 2,
    NegativeY = 3,
    PositiveZ = 4,
    NegativeZ = 5,
}

impl CubeFace {
    pub const ALL: [CubeFace; 6] = [
        CubeFace::PositiveX,
        CubeFace::NegativeX,
        CubeFace::PositiveY,
        CubeFace::NegativeY,
        CubeFace::PositiveZ,
        CubeFace::NegativeZ,
    ];

    /// Get the forward and up directions for this cube face.
    pub fn directions(self) -> (Vec3, Vec3) {
        match self {
            CubeFace::PositiveX => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0)),
            CubeFace::NegativeX => (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0)),
            CubeFace::PositiveY => (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
            CubeFace::NegativeY => (Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)),
            CubeFace::PositiveZ => (Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, -1.0, 0.0)),
            CubeFace::NegativeZ => (Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, -1.0, 0.0)),
        }
    }
}

impl OmniShadowMap {
    pub fn new(resolution: u32, light_position: Vec3, radius: f32) -> Self {
        let mut osm = Self {
            faces: [
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
                ShadowMap::new(resolution, resolution),
            ],
            light_position,
            radius,
            resolution,
        };
        osm.update_matrices();
        osm
    }

    /// Recompute the view-projection matrices for all six faces.
    pub fn update_matrices(&mut self) {
        let proj = Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.1, self.radius);
        for face in CubeFace::ALL {
            let (forward, up) = face.directions();
            let target = self.light_position + forward;
            let view = Mat4::look_at(self.light_position, target, up);
            let vp = proj.mul_mat4(view);
            self.faces[face as usize].view_projection = vp;
            self.faces[face as usize].near = 0.1;
            self.faces[face as usize].far = self.radius;
        }
    }

    /// Update the light position and recompute matrices.
    pub fn set_position(&mut self, pos: Vec3) {
        self.light_position = pos;
        self.update_matrices();
    }

    /// Clear all faces.
    pub fn clear_all(&mut self) {
        for face in &mut self.faces {
            face.clear();
        }
    }

    /// Determine which cube face a direction vector maps to.
    pub fn select_face(direction: Vec3) -> CubeFace {
        let abs = direction.abs();
        if abs.x >= abs.y && abs.x >= abs.z {
            if direction.x >= 0.0 { CubeFace::PositiveX } else { CubeFace::NegativeX }
        } else if abs.y >= abs.x && abs.y >= abs.z {
            if direction.y >= 0.0 { CubeFace::PositiveY } else { CubeFace::NegativeY }
        } else if direction.z >= 0.0 {
            CubeFace::PositiveZ
        } else {
            CubeFace::NegativeZ
        }
    }

    /// Test if a world-space point is in shadow.
    pub fn is_in_shadow(&self, world_pos: Vec3, bias: f32) -> bool {
        let dir = world_pos - self.light_position;
        let dist = dir.length();
        if dist > self.radius {
            return false;
        }
        let face = Self::select_face(dir);
        self.faces[face as usize].is_in_shadow(world_pos, bias)
    }

    /// Compute shadow factor (0.0 = shadowed, 1.0 = lit) with PCF.
    pub fn shadow_factor_pcf(&self, world_pos: Vec3, bias: f32, kernel: &PcfKernel) -> f32 {
        let dir = world_pos - self.light_position;
        let dist = dir.length();
        if dist > self.radius {
            return 1.0;
        }
        let face = Self::select_face(dir);
        let shadow_map = &self.faces[face as usize];
        kernel.sample(shadow_map, world_pos, bias)
    }

    /// Get total memory usage.
    pub fn memory_bytes(&self) -> usize {
        self.faces.iter().map(|f| f.memory_bytes()).sum()
    }
}

// ── Shadow Atlas ────────────────────────────────────────────────────────────

/// A region within the shadow atlas.
#[derive(Debug, Clone, Copy)]
pub struct ShadowAtlasRegion {
    /// Top-left X in the atlas (in texels).
    pub x: u32,
    /// Top-left Y in the atlas (in texels).
    pub y: u32,
    /// Width of this region.
    pub width: u32,
    /// Height of this region.
    pub height: u32,
    /// Which light owns this region.
    pub light_id: Option<LightId>,
}

impl ShadowAtlasRegion {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            light_id: None,
        }
    }

    /// Convert atlas-space UV to region-space UV.
    pub fn atlas_to_region_uv(&self, atlas_width: u32, atlas_height: u32, u: f32, v: f32) -> (f32, f32) {
        let region_u = (u * atlas_width as f32 - self.x as f32) / self.width as f32;
        let region_v = (v * atlas_height as f32 - self.y as f32) / self.height as f32;
        (region_u, region_v)
    }

    /// Convert region-space UV to atlas-space UV.
    pub fn region_to_atlas_uv(&self, atlas_width: u32, atlas_height: u32, u: f32, v: f32) -> (f32, f32) {
        let atlas_u = (self.x as f32 + u * self.width as f32) / atlas_width as f32;
        let atlas_v = (self.y as f32 + v * self.height as f32) / atlas_height as f32;
        (atlas_u, atlas_v)
    }

    /// Check if a point (in texels) falls within this region.
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Area in texels.
    pub fn area(&self) -> u32 {
        self.width * self.height
    }
}

/// Packs multiple shadow maps into a single atlas texture.
#[derive(Debug, Clone)]
pub struct ShadowAtlas {
    /// Total atlas width in texels.
    pub width: u32,
    /// Total atlas height in texels.
    pub height: u32,
    /// The atlas depth buffer.
    pub depth_buffer: Vec<f32>,
    /// Allocated regions.
    pub regions: Vec<ShadowAtlasRegion>,
    /// Free regions available for allocation (simple shelf packing).
    free_shelves: Vec<AtlasShelf>,
    /// Current shelf Y position.
    current_shelf_y: u32,
    /// Current shelf height.
    current_shelf_height: u32,
    /// Current X position on the active shelf.
    current_shelf_x: u32,
}

#[derive(Debug, Clone)]
struct AtlasShelf {
    y: u32,
    height: u32,
    remaining_width: u32,
    x_offset: u32,
}

impl ShadowAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            depth_buffer: vec![1.0; size],
            regions: Vec::new(),
            free_shelves: Vec::new(),
            current_shelf_y: 0,
            current_shelf_height: 0,
            current_shelf_x: 0,
        }
    }

    /// Clear the entire atlas depth buffer.
    pub fn clear(&mut self) {
        for d in self.depth_buffer.iter_mut() {
            *d = 1.0;
        }
    }

    /// Reset all allocations (but keep the depth buffer).
    pub fn reset_allocations(&mut self) {
        self.regions.clear();
        self.free_shelves.clear();
        self.current_shelf_y = 0;
        self.current_shelf_height = 0;
        self.current_shelf_x = 0;
    }

    /// Allocate a region of the given size. Returns the region index or None if full.
    pub fn allocate(&mut self, width: u32, height: u32, light_id: LightId) -> Option<usize> {
        // Try to fit on an existing shelf
        for shelf in &mut self.free_shelves {
            if height <= shelf.height && width <= shelf.remaining_width {
                let region = ShadowAtlasRegion {
                    x: shelf.x_offset,
                    y: shelf.y,
                    width,
                    height,
                    light_id: Some(light_id),
                };
                shelf.x_offset += width;
                shelf.remaining_width -= width;
                let idx = self.regions.len();
                self.regions.push(region);
                return Some(idx);
            }
        }

        // Try to fit on the current shelf
        if self.current_shelf_x + width <= self.width && height <= self.current_shelf_height {
            let region = ShadowAtlasRegion {
                x: self.current_shelf_x,
                y: self.current_shelf_y,
                width,
                height,
                light_id: Some(light_id),
            };
            self.current_shelf_x += width;
            let idx = self.regions.len();
            self.regions.push(region);
            return Some(idx);
        }

        // Start a new shelf
        if self.current_shelf_height > 0 {
            // Save the current shelf as a free shelf if there's remaining width
            let remaining = self.width - self.current_shelf_x;
            if remaining > 0 {
                self.free_shelves.push(AtlasShelf {
                    y: self.current_shelf_y,
                    height: self.current_shelf_height,
                    remaining_width: remaining,
                    x_offset: self.current_shelf_x,
                });
            }
        }

        let new_y = self.current_shelf_y + self.current_shelf_height;
        if new_y + height > self.height || width > self.width {
            return None; // Atlas is full
        }

        self.current_shelf_y = new_y;
        self.current_shelf_height = height;
        self.current_shelf_x = width;

        let region = ShadowAtlasRegion {
            x: 0,
            y: new_y,
            width,
            height,
            light_id: Some(light_id),
        };
        let idx = self.regions.len();
        self.regions.push(region);
        Some(idx)
    }

    /// Write depth at a position within a region.
    pub fn write_depth_in_region(&mut self, region_idx: usize, local_x: u32, local_y: u32, depth: f32) {
        if let Some(region) = self.regions.get(region_idx) {
            let ax = region.x + local_x;
            let ay = region.y + local_y;
            if ax < self.width && ay < self.height {
                let idx = (ay as usize) * (self.width as usize) + (ax as usize);
                if depth < self.depth_buffer[idx] {
                    self.depth_buffer[idx] = depth;
                }
            }
        }
    }

    /// Read depth at a position within a region.
    pub fn read_depth_in_region(&self, region_idx: usize, local_x: u32, local_y: u32) -> f32 {
        if let Some(region) = self.regions.get(region_idx) {
            let ax = region.x + local_x;
            let ay = region.y + local_y;
            if ax < self.width && ay < self.height {
                return self.depth_buffer[(ay as usize) * (self.width as usize) + (ax as usize)];
            }
        }
        1.0
    }

    /// Sample with bilinear filtering within a region at normalized UV.
    pub fn sample_region_bilinear(&self, region_idx: usize, u: f32, v: f32) -> f32 {
        let region = match self.regions.get(region_idx) {
            Some(r) => r,
            None => return 1.0,
        };

        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let fx = u * (region.width as f32 - 1.0);
        let fy = v * (region.height as f32 - 1.0);

        let x0 = fx.floor() as u32;
        let y0 = fy.floor() as u32;
        let x1 = (x0 + 1).min(region.width - 1);
        let y1 = (y0 + 1).min(region.height - 1);

        let frac_x = fx - fx.floor();
        let frac_y = fy - fy.floor();

        let d00 = self.read_depth_in_region(region_idx, x0, y0);
        let d10 = self.read_depth_in_region(region_idx, x1, y0);
        let d01 = self.read_depth_in_region(region_idx, x0, y1);
        let d11 = self.read_depth_in_region(region_idx, x1, y1);

        let top = d00 + (d10 - d00) * frac_x;
        let bottom = d01 + (d11 - d01) * frac_x;
        top + (bottom - top) * frac_y
    }

    /// Get the number of allocated regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Get memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.depth_buffer.len() * 4
    }

    /// Get utilization as a fraction (0..1).
    pub fn utilization(&self) -> f32 {
        let total = (self.width as u64) * (self.height as u64);
        if total == 0 {
            return 0.0;
        }
        let used: u64 = self.regions.iter().map(|r| r.area() as u64).sum();
        used as f32 / total as f32
    }
}

// ── PCF Filtering ───────────────────────────────────────────────────────────

/// Percentage-closer filtering kernel for soft shadows.
#[derive(Debug, Clone)]
pub struct PcfKernel {
    /// Sample offsets (in texels).
    pub offsets: Vec<(f32, f32)>,
    /// Corresponding weights (should sum to 1.0).
    pub weights: Vec<f32>,
    /// Texel size for the shadow map.
    pub texel_size: f32,
}

impl PcfKernel {
    /// Create a 3x3 PCF kernel.
    pub fn kernel_3x3(texel_size: f32) -> Self {
        let mut offsets = Vec::with_capacity(9);
        let mut weights = Vec::with_capacity(9);
        for dy in -1..=1 {
            for dx in -1..=1 {
                offsets.push((dx as f32, dy as f32));
                // Gaussian-like weights
                let dist_sq = (dx * dx + dy * dy) as f32;
                let w = (-dist_sq * 0.5).exp();
                weights.push(w);
            }
        }
        let sum: f32 = weights.iter().sum();
        for w in weights.iter_mut() {
            *w /= sum;
        }
        Self { offsets, weights, texel_size }
    }

    /// Create a 5x5 PCF kernel.
    pub fn kernel_5x5(texel_size: f32) -> Self {
        let mut offsets = Vec::with_capacity(25);
        let mut weights = Vec::with_capacity(25);
        for dy in -2..=2 {
            for dx in -2..=2 {
                offsets.push((dx as f32, dy as f32));
                let dist_sq = (dx * dx + dy * dy) as f32;
                let w = (-dist_sq * 0.25).exp();
                weights.push(w);
            }
        }
        let sum: f32 = weights.iter().sum();
        for w in weights.iter_mut() {
            *w /= sum;
        }
        Self { offsets, weights, texel_size }
    }

    /// Create a Poisson disk PCF kernel with the given number of samples.
    pub fn poisson_disk(sample_count: usize, texel_size: f32) -> Self {
        // Generate a deterministic Poisson-like disk
        let mut offsets = Vec::with_capacity(sample_count);
        let mut weights = Vec::with_capacity(sample_count);

        let golden_angle = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
        for i in 0..sample_count {
            let r = ((i as f32 + 0.5) / sample_count as f32).sqrt() * 2.0;
            let theta = i as f32 * golden_angle;
            offsets.push((r * theta.cos(), r * theta.sin()));
            weights.push(1.0 / sample_count as f32);
        }

        Self { offsets, weights, texel_size }
    }

    /// Sample the shadow map with PCF filtering. Returns shadow factor (0.0..1.0).
    pub fn sample(&self, shadow_map: &ShadowMap, world_pos: Vec3, bias: f32) -> f32 {
        let (u, v, depth) = shadow_map.project_point(world_pos);
        if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
            return 1.0; // Outside shadow map = lit
        }

        let mut shadow_sum = 0.0f32;
        for (i, &(dx, dy)) in self.offsets.iter().enumerate() {
            let su = u + dx * self.texel_size;
            let sv = v + dy * self.texel_size;
            let stored_depth = shadow_map.sample_bilinear(su, sv);
            let lit = if depth - bias <= stored_depth { 1.0 } else { 0.0 };
            shadow_sum += lit * self.weights[i];
        }
        shadow_sum
    }

    /// Sample from a shadow atlas region.
    pub fn sample_atlas(
        &self,
        atlas: &ShadowAtlas,
        region_idx: usize,
        u: f32,
        v: f32,
        depth: f32,
        bias: f32,
    ) -> f32 {
        let region = match atlas.regions.get(region_idx) {
            Some(r) => r,
            None => return 1.0,
        };

        let texel_u = self.texel_size / region.width as f32;
        let texel_v = self.texel_size / region.height as f32;

        let mut shadow_sum = 0.0f32;
        for (i, &(dx, dy)) in self.offsets.iter().enumerate() {
            let su = (u + dx * texel_u).clamp(0.0, 1.0);
            let sv = (v + dy * texel_v).clamp(0.0, 1.0);
            let stored_depth = atlas.sample_region_bilinear(region_idx, su, sv);
            let lit = if depth - bias <= stored_depth { 1.0 } else { 0.0 };
            shadow_sum += lit * self.weights[i];
        }
        shadow_sum
    }
}

// ── Variance Shadow Map ─────────────────────────────────────────────────────

/// Variance shadow map for soft shadows using statistical analysis.
/// Stores depth and depth-squared moments for Chebyshev's inequality test.
#[derive(Debug, Clone)]
pub struct VarianceShadowMap {
    pub width: u32,
    pub height: u32,
    /// First moment (mean depth).
    pub moment1: Vec<f32>,
    /// Second moment (mean depth squared).
    pub moment2: Vec<f32>,
    pub view_projection: Mat4,
    /// Minimum variance to prevent light bleeding.
    pub min_variance: f32,
    /// Light bleed reduction factor (0..1).
    pub light_bleed_reduction: f32,
}

impl VarianceShadowMap {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            moment1: vec![1.0; size],
            moment2: vec![1.0; size],
            view_projection: Mat4::IDENTITY,
            min_variance: 0.00002,
            light_bleed_reduction: 0.2,
        }
    }

    /// Clear both moment buffers.
    pub fn clear(&mut self) {
        for v in self.moment1.iter_mut() {
            *v = 1.0;
        }
        for v in self.moment2.iter_mut() {
            *v = 1.0;
        }
    }

    /// Write a depth sample to the variance map (accumulates moments).
    pub fn write_depth(&mut self, x: u32, y: u32, depth: f32) {
        if x < self.width && y < self.height {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            // In a real implementation, this would be done per-pixel during rendering.
            // Here we just store the minimum depth and its square.
            if depth < self.moment1[idx] {
                self.moment1[idx] = depth;
                self.moment2[idx] = depth * depth;
            }
        }
    }

    /// Sample the moments at normalized UV with bilinear filtering.
    pub fn sample_moments(&self, u: f32, v: f32) -> (f32, f32) {
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let fx = u * (self.width as f32 - 1.0);
        let fy = v * (self.height as f32 - 1.0);

        let x0 = fx.floor() as u32;
        let y0 = fy.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let frac_x = fx - fx.floor();
        let frac_y = fy - fy.floor();

        let read = |buf: &[f32], x: u32, y: u32| -> f32 {
            buf[(y as usize) * (self.width as usize) + (x as usize)]
        };

        let m1_00 = read(&self.moment1, x0, y0);
        let m1_10 = read(&self.moment1, x1, y0);
        let m1_01 = read(&self.moment1, x0, y1);
        let m1_11 = read(&self.moment1, x1, y1);

        let m2_00 = read(&self.moment2, x0, y0);
        let m2_10 = read(&self.moment2, x1, y0);
        let m2_01 = read(&self.moment2, x0, y1);
        let m2_11 = read(&self.moment2, x1, y1);

        let m1_top = m1_00 + (m1_10 - m1_00) * frac_x;
        let m1_bot = m1_01 + (m1_11 - m1_01) * frac_x;
        let m1 = m1_top + (m1_bot - m1_top) * frac_y;

        let m2_top = m2_00 + (m2_10 - m2_00) * frac_x;
        let m2_bot = m2_01 + (m2_11 - m2_01) * frac_x;
        let m2 = m2_top + (m2_bot - m2_top) * frac_y;

        (m1, m2)
    }

    /// Compute the shadow factor using Chebyshev's inequality.
    pub fn shadow_factor(&self, world_pos: Vec3) -> f32 {
        let clip = self.view_projection.transform_point(world_pos);
        let u = clip.x * 0.5 + 0.5;
        let v = clip.y * 0.5 + 0.5;
        let depth = clip.z * 0.5 + 0.5;

        if u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0 {
            return 1.0;
        }

        let (mean, mean_sq) = self.sample_moments(u, v);

        // If fragment is closer than the mean, it's fully lit
        if depth <= mean {
            return 1.0;
        }

        // Chebyshev's inequality
        let variance = (mean_sq - mean * mean).max(self.min_variance);
        let d = depth - mean;
        let p_max = variance / (variance + d * d);

        // Light bleed reduction
        let reduced = ((p_max - self.light_bleed_reduction) / (1.0 - self.light_bleed_reduction)).max(0.0);
        reduced
    }

    /// Apply a box blur to the moment buffers (for smoother shadows).
    pub fn blur(&mut self, radius: u32) {
        let w = self.width as usize;
        let h = self.height as usize;

        // Horizontal pass
        let mut temp1 = vec![0.0f32; w * h];
        let mut temp2 = vec![0.0f32; w * h];

        for y in 0..h {
            for x in 0..w {
                let mut sum1 = 0.0f32;
                let mut sum2 = 0.0f32;
                let mut count = 0.0f32;

                let x_start = x.saturating_sub(radius as usize);
                let x_end = (x + radius as usize + 1).min(w);

                for sx in x_start..x_end {
                    sum1 += self.moment1[y * w + sx];
                    sum2 += self.moment2[y * w + sx];
                    count += 1.0;
                }
                temp1[y * w + x] = sum1 / count;
                temp2[y * w + x] = sum2 / count;
            }
        }

        // Vertical pass
        for y in 0..h {
            for x in 0..w {
                let mut sum1 = 0.0f32;
                let mut sum2 = 0.0f32;
                let mut count = 0.0f32;

                let y_start = y.saturating_sub(radius as usize);
                let y_end = (y + radius as usize + 1).min(h);

                for sy in y_start..y_end {
                    sum1 += temp1[sy * w + x];
                    sum2 += temp2[sy * w + x];
                    count += 1.0;
                }
                self.moment1[y * w + x] = sum1 / count;
                self.moment2[y * w + x] = sum2 / count;
            }
        }
    }

    /// Get memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        (self.moment1.len() + self.moment2.len()) * 4
    }
}

// ── Shadow Bias ─────────────────────────────────────────────────────────────

/// Configurable shadow bias combining constant, slope-scaled, and normal offset.
#[derive(Debug, Clone, Copy)]
pub struct ShadowBias {
    /// Constant depth bias (added directly to the depth comparison).
    pub constant: f32,
    /// Slope-scaled bias (multiplied by the depth slope).
    pub slope_scale: f32,
    /// Normal offset (offsets the shadow lookup along the surface normal).
    pub normal_offset: f32,
}

impl Default for ShadowBias {
    fn default() -> Self {
        Self {
            constant: 0.005,
            slope_scale: 1.5,
            normal_offset: 0.02,
        }
    }
}

impl ShadowBias {
    pub fn new(constant: f32, slope_scale: f32, normal_offset: f32) -> Self {
        Self { constant, slope_scale, normal_offset }
    }

    /// Compute the effective bias given the depth slope and surface angle.
    pub fn compute(&self, depth_slope: f32, _cos_angle: f32) -> f32 {
        self.constant + self.slope_scale * depth_slope
    }

    /// Compute the world-space offset along the surface normal.
    pub fn normal_offset_vec(&self, normal: Vec3) -> Vec3 {
        normal * self.normal_offset
    }

    /// Apply normal offset to a world position before shadow lookup.
    pub fn apply_normal_offset(&self, position: Vec3, normal: Vec3) -> Vec3 {
        position + self.normal_offset_vec(normal)
    }
}

// ── Shadow Config ───────────────────────────────────────────────────────────

/// Global shadow configuration.
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// Maximum shadow distance from the camera.
    pub max_distance: f32,
    /// Distance at which shadows start fading out.
    pub fade_start: f32,
    /// Atlas resolution (width and height).
    pub atlas_resolution: u32,
    /// Default shadow map resolution per light.
    pub default_resolution: u32,
    /// Default bias settings.
    pub bias: ShadowBias,
    /// PCF kernel to use (3x3 or 5x5).
    pub pcf_mode: PcfMode,
    /// Whether to use variance shadow maps.
    pub use_vsm: bool,
    /// Maximum number of shadow-casting lights.
    pub max_shadow_casters: u32,
    /// Whether to enable shadow caster culling.
    pub cull_shadow_casters: bool,
}

/// PCF filter mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcfMode {
    None,
    Pcf3x3,
    Pcf5x5,
    PoissonDisk16,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            max_distance: 200.0,
            fade_start: 150.0,
            atlas_resolution: 4096,
            default_resolution: 1024,
            bias: ShadowBias::default(),
            pcf_mode: PcfMode::Pcf3x3,
            use_vsm: false,
            max_shadow_casters: 16,
            cull_shadow_casters: true,
        }
    }
}

impl ShadowConfig {
    /// Compute shadow distance fade factor (1.0 = full shadow, 0.0 = faded).
    pub fn distance_fade(&self, distance: f32) -> f32 {
        if distance >= self.max_distance {
            return 0.0;
        }
        if distance <= self.fade_start {
            return 1.0;
        }
        let range = self.max_distance - self.fade_start;
        if range <= 0.0 {
            return 0.0;
        }
        1.0 - (distance - self.fade_start) / range
    }

    /// Create a PCF kernel based on the current mode.
    pub fn create_pcf_kernel(&self) -> PcfKernel {
        let texel_size = 1.0 / self.default_resolution as f32;
        match self.pcf_mode {
            PcfMode::None => PcfKernel {
                offsets: vec![(0.0, 0.0)],
                weights: vec![1.0],
                texel_size,
            },
            PcfMode::Pcf3x3 => PcfKernel::kernel_3x3(texel_size),
            PcfMode::Pcf5x5 => PcfKernel::kernel_5x5(texel_size),
            PcfMode::PoissonDisk16 => PcfKernel::poisson_disk(16, texel_size),
        }
    }
}

// ── Shadow Caster Culling ───────────────────────────────────────────────────

/// AABB for culling shadow casters.
#[derive(Debug, Clone, Copy)]
pub struct CasterBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl CasterBounds {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Test if this AABB intersects a frustum (simplified 6-plane test).
    pub fn intersects_frustum(&self, frustum_planes: &[(Vec3, f32); 6]) -> bool {
        for &(normal, dist) in frustum_planes {
            let p = Vec3::new(
                if normal.x >= 0.0 { self.max.x } else { self.min.x },
                if normal.y >= 0.0 { self.max.y } else { self.min.y },
                if normal.z >= 0.0 { self.max.z } else { self.min.z },
            );
            if normal.dot(p) + dist < 0.0 {
                return false;
            }
        }
        true
    }

    /// Test if this AABB is within a sphere (for point light culling).
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        let mut dist_sq = 0.0f32;

        if center.x < self.min.x {
            let d = self.min.x - center.x;
            dist_sq += d * d;
        } else if center.x > self.max.x {
            let d = center.x - self.max.x;
            dist_sq += d * d;
        }

        if center.y < self.min.y {
            let d = self.min.y - center.y;
            dist_sq += d * d;
        } else if center.y > self.max.y {
            let d = center.y - self.max.y;
            dist_sq += d * d;
        }

        if center.z < self.min.z {
            let d = self.min.z - center.z;
            dist_sq += d * d;
        } else if center.z > self.max.z {
            let d = center.z - self.max.z;
            dist_sq += d * d;
        }

        dist_sq <= radius * radius
    }
}

/// Cull shadow casters for a specific light.
pub fn cull_shadow_casters(
    casters: &[CasterBounds],
    light: &Light,
) -> Vec<usize> {
    let mut visible = Vec::new();

    match light {
        Light::Point(pl) => {
            for (i, caster) in casters.iter().enumerate() {
                if caster.intersects_sphere(pl.position, pl.radius) {
                    visible.push(i);
                }
            }
        }
        Light::Spot(sl) => {
            // Simplified: use sphere intersection with the spot's bounding sphere
            for (i, caster) in casters.iter().enumerate() {
                if caster.intersects_sphere(sl.position, sl.radius) {
                    visible.push(i);
                }
            }
        }
        Light::Directional(_) => {
            // Directional lights can't cull by position easily;
            // cull by the cascade frustum in a real implementation.
            // Here we just include all casters.
            for i in 0..casters.len() {
                visible.push(i);
            }
        }
        _ => {
            // Non-shadow-casting lights don't need culling
        }
    }

    visible
}

// ── Shadow System ───────────────────────────────────────────────────────────

/// Top-level shadow system that orchestrates shadow map allocation and rendering.
#[derive(Debug)]
pub struct ShadowSystem {
    pub config: ShadowConfig,
    pub atlas: ShadowAtlas,
    pub cascaded_maps: HashMap<LightId, CascadedShadowMap>,
    pub omni_maps: HashMap<LightId, OmniShadowMap>,
    pub spot_maps: HashMap<LightId, usize>, // region index in atlas
    pub pcf_kernel: PcfKernel,
    pub vsm_maps: HashMap<LightId, VarianceShadowMap>,
    /// Stats from the last frame.
    pub stats: ShadowStats,
}

/// Statistics for shadow rendering.
#[derive(Debug, Clone, Default)]
pub struct ShadowStats {
    pub shadow_casters: u32,
    pub cascaded_maps: u32,
    pub omni_maps: u32,
    pub spot_maps: u32,
    pub atlas_utilization: f32,
    pub total_memory_bytes: usize,
}

impl ShadowSystem {
    pub fn new(config: ShadowConfig) -> Self {
        let pcf_kernel = config.create_pcf_kernel();
        let atlas_res = config.atlas_resolution;
        Self {
            config,
            atlas: ShadowAtlas::new(atlas_res, atlas_res),
            cascaded_maps: HashMap::new(),
            omni_maps: HashMap::new(),
            spot_maps: HashMap::new(),
            pcf_kernel,
            vsm_maps: HashMap::new(),
            stats: ShadowStats::default(),
        }
    }

    /// Allocate shadow maps for the given shadow-casting lights.
    pub fn allocate_for_lights(&mut self, lights: &[(LightId, &Light)]) {
        self.atlas.reset_allocations();
        self.atlas.clear();
        self.cascaded_maps.clear();
        self.omni_maps.clear();
        self.spot_maps.clear();
        self.vsm_maps.clear();

        let mut caster_count = 0u32;

        for &(id, light) in lights {
            if caster_count >= self.config.max_shadow_casters {
                break;
            }
            if !light.is_enabled() || !light.casts_shadows() {
                continue;
            }

            match light {
                Light::Directional(dl) => {
                    let csm = CascadedShadowMap::new(
                        dl.cascade_params.resolution,
                        dl.cascade_params.cascade_count,
                    );
                    self.cascaded_maps.insert(id, csm);
                    caster_count += 1;
                }
                Light::Point(pl) => {
                    let res = self.config.default_resolution.min(512);
                    let osm = OmniShadowMap::new(res, pl.position, pl.radius);
                    self.omni_maps.insert(id, osm);
                    caster_count += 1;
                }
                Light::Spot(_) => {
                    let res = self.config.default_resolution;
                    if let Some(region_idx) = self.atlas.allocate(res, res, id) {
                        self.spot_maps.insert(id, region_idx);
                    }
                    caster_count += 1;
                }
                _ => {}
            }

            // Optionally create VSM
            if self.config.use_vsm {
                let res = self.config.default_resolution;
                let vsm = VarianceShadowMap::new(res, res);
                self.vsm_maps.insert(id, vsm);
            }
        }

        self.update_stats();
    }

    /// Compute the shadow factor for a world point from a specific light.
    pub fn shadow_factor(
        &self,
        light_id: LightId,
        world_pos: Vec3,
        normal: Vec3,
        view_depth: f32,
    ) -> f32 {
        // Distance fade
        let fade = self.config.distance_fade(view_depth);
        if fade <= 0.0 {
            return 1.0;
        }

        let biased_pos = self.config.bias.apply_normal_offset(world_pos, normal);
        let effective_bias = self.config.bias.compute(0.0, 0.0);

        // Check VSM first
        if self.config.use_vsm {
            if let Some(vsm) = self.vsm_maps.get(&light_id) {
                let factor = vsm.shadow_factor(biased_pos);
                return 1.0 - (1.0 - factor) * fade;
            }
        }

        // Check cascaded shadow map
        if let Some(csm) = self.cascaded_maps.get(&light_id) {
            let factor = csm.shadow_factor(biased_pos, view_depth, &self.config.bias);
            return 1.0 - (1.0 - factor) * fade;
        }

        // Check omni shadow map
        if let Some(osm) = self.omni_maps.get(&light_id) {
            let factor = osm.shadow_factor_pcf(biased_pos, effective_bias, &self.pcf_kernel);
            return 1.0 - (1.0 - factor) * fade;
        }

        // Check spot shadow map in atlas
        if let Some(&region_idx) = self.spot_maps.get(&light_id) {
            if let Some(region) = self.atlas.regions.get(region_idx) {
                let _ = region; // Would project using the spot light's VP matrix
                // Simplified: just return lit
                return 1.0;
            }
        }

        1.0 // No shadow map = fully lit
    }

    /// Compute combined shadow factor from all shadow-casting lights at a point.
    pub fn combined_shadow_factor(
        &self,
        world_pos: Vec3,
        normal: Vec3,
        view_depth: f32,
    ) -> f32 {
        let mut min_factor = 1.0f32;

        for &id in self.cascaded_maps.keys() {
            let f = self.shadow_factor(id, world_pos, normal, view_depth);
            min_factor = min_factor.min(f);
        }
        for &id in self.omni_maps.keys() {
            let f = self.shadow_factor(id, world_pos, normal, view_depth);
            min_factor = min_factor.min(f);
        }
        for &id in self.spot_maps.keys() {
            let f = self.shadow_factor(id, world_pos, normal, view_depth);
            min_factor = min_factor.min(f);
        }

        min_factor
    }

    fn update_stats(&mut self) {
        let mut total_mem = self.atlas.memory_bytes();
        for csm in self.cascaded_maps.values() {
            total_mem += csm.memory_bytes();
        }
        for osm in self.omni_maps.values() {
            total_mem += osm.memory_bytes();
        }
        for vsm in self.vsm_maps.values() {
            total_mem += vsm.memory_bytes();
        }

        self.stats = ShadowStats {
            shadow_casters: (self.cascaded_maps.len() + self.omni_maps.len() + self.spot_maps.len()) as u32,
            cascaded_maps: self.cascaded_maps.len() as u32,
            omni_maps: self.omni_maps.len() as u32,
            spot_maps: self.spot_maps.len() as u32,
            atlas_utilization: self.atlas.utilization(),
            total_memory_bytes: total_mem,
        };
    }

    /// Get a reference to the current stats.
    pub fn stats(&self) -> &ShadowStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_map_depth() {
        let mut sm = ShadowMap::new(64, 64);
        sm.write_depth(10, 10, 0.5);
        assert!((sm.read_depth(10, 10) - 0.5).abs() < 1e-5);
        // Closer depth should overwrite
        sm.write_depth(10, 10, 0.3);
        assert!((sm.read_depth(10, 10) - 0.3).abs() < 1e-5);
        // Farther depth should not overwrite
        sm.write_depth(10, 10, 0.8);
        assert!((sm.read_depth(10, 10) - 0.3).abs() < 1e-5);
    }

    #[test]
    fn test_shadow_map_clear() {
        let mut sm = ShadowMap::new(16, 16);
        sm.write_depth(5, 5, 0.2);
        sm.clear();
        assert!((sm.read_depth(5, 5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cascaded_cascade_selection() {
        let csm = CascadedShadowMap::new(512, 4);
        assert_eq!(csm.select_cascade(5.0), 0);
        assert_eq!(csm.select_cascade(20.0), 1);
        assert_eq!(csm.select_cascade(50.0), 2);
        assert_eq!(csm.select_cascade(100.0), 3);
    }

    #[test]
    fn test_omni_face_selection() {
        assert_eq!(
            OmniShadowMap::select_face(Vec3::new(1.0, 0.0, 0.0)),
            CubeFace::PositiveX
        );
        assert_eq!(
            OmniShadowMap::select_face(Vec3::new(0.0, -1.0, 0.0)),
            CubeFace::NegativeY
        );
        assert_eq!(
            OmniShadowMap::select_face(Vec3::new(0.0, 0.0, -1.0)),
            CubeFace::NegativeZ
        );
    }

    #[test]
    fn test_shadow_atlas_allocation() {
        let mut atlas = ShadowAtlas::new(2048, 2048);
        let id1 = LightId(1);
        let id2 = LightId(2);

        let r1 = atlas.allocate(512, 512, id1);
        assert!(r1.is_some());

        let r2 = atlas.allocate(512, 512, id2);
        assert!(r2.is_some());

        assert_eq!(atlas.region_count(), 2);
        assert!(atlas.utilization() > 0.0);
    }

    #[test]
    fn test_pcf_kernel_weights() {
        let kernel = PcfKernel::kernel_3x3(1.0 / 512.0);
        let sum: f32 = kernel.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
        assert_eq!(kernel.offsets.len(), 9);
    }

    #[test]
    fn test_vsm_shadow_factor() {
        let mut vsm = VarianceShadowMap::new(64, 64);
        vsm.view_projection = Mat4::IDENTITY;
        // All cleared to 1.0, so everything should be lit
        let factor = vsm.shadow_factor(Vec3::new(0.0, 0.0, 0.5));
        assert!(factor > 0.0);
    }

    #[test]
    fn test_shadow_bias() {
        let bias = ShadowBias::new(0.005, 2.0, 0.03);
        let computed = bias.compute(0.01, 0.8);
        assert!(computed > 0.005); // Should be constant + slope contribution
    }

    #[test]
    fn test_caster_bounds_sphere() {
        let bounds = CasterBounds::new(
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, 1.0),
        );
        assert!(bounds.intersects_sphere(Vec3::ZERO, 2.0));
        assert!(!bounds.intersects_sphere(Vec3::new(10.0, 10.0, 10.0), 1.0));
    }

    #[test]
    fn test_shadow_distance_fade() {
        let config = ShadowConfig {
            max_distance: 200.0,
            fade_start: 150.0,
            ..Default::default()
        };
        assert!((config.distance_fade(100.0) - 1.0).abs() < 1e-5);
        assert!((config.distance_fade(200.0)).abs() < 1e-5);
        assert!(config.distance_fade(175.0) > 0.0 && config.distance_fade(175.0) < 1.0);
    }
}
