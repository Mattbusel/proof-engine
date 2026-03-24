//! Shadow atlas, cascade shadow maps, point light shadows, filtering, and debug visualization.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ShadowTile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShadowTile {
    pub light_id: u32,
    pub slot: u32,
    pub last_used: u32,
    pub dirty: bool,
}

impl ShadowTile {
    pub fn new(light_id: u32, slot: u32) -> Self {
        Self {
            light_id,
            slot,
            last_used: 0,
            dirty: true,
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowAtlasAlloc — free-list allocator for shadow tiles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShadowAtlasAlloc {
    pub total_slots: u32,
    free_list: Vec<u32>,
    allocated: HashMap<u32, u32>, // light_id -> slot
}

impl ShadowAtlasAlloc {
    pub fn new(total_slots: u32) -> Self {
        let free_list: Vec<u32> = (0..total_slots).collect();
        Self {
            total_slots,
            free_list,
            allocated: HashMap::new(),
        }
    }

    /// Allocate a slot for a light. Returns the slot index or None if full.
    pub fn allocate(&mut self, light_id: u32) -> Option<u32> {
        if let Some(&slot) = self.allocated.get(&light_id) {
            return Some(slot);
        }
        if let Some(slot) = self.free_list.pop() {
            self.allocated.insert(light_id, slot);
            Some(slot)
        } else {
            None
        }
    }

    /// Free the slot associated with a light.
    pub fn free(&mut self, light_id: u32) {
        if let Some(slot) = self.allocated.remove(&light_id) {
            self.free_list.push(slot);
        }
    }

    /// Evict the least recently used slot to make room for a new allocation.
    pub fn evict_lru(&mut self, tiles: &[ShadowTile]) -> Option<u32> {
        if tiles.is_empty() {
            return None;
        }
        let oldest = tiles.iter().min_by_key(|t| t.last_used)?;
        let light_id = oldest.light_id;
        self.free(light_id);
        Some(oldest.slot)
    }

    pub fn is_allocated(&self, light_id: u32) -> bool {
        self.allocated.contains_key(&light_id)
    }

    pub fn slot_for(&self, light_id: u32) -> Option<u32> {
        self.allocated.get(&light_id).copied()
    }

    pub fn num_free(&self) -> u32 {
        self.free_list.len() as u32
    }

    pub fn num_allocated(&self) -> u32 {
        self.allocated.len() as u32
    }
}

// ---------------------------------------------------------------------------
// ShadowAtlas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShadowAtlas {
    pub resolution: u32,
    pub num_slots: u32,
    pub tiles_per_row: u32,
    pub tile_resolution: u32,
    pub tiles: Vec<ShadowTile>,
    pub allocator: ShadowAtlasAlloc,
    /// Flat depth buffer for the entire atlas.
    pub depth_buffer: Vec<f32>,
}

impl ShadowAtlas {
    pub fn new(resolution: u32, num_slots: u32) -> Self {
        let tiles_per_row = (num_slots as f32).sqrt().ceil() as u32;
        let tile_resolution = resolution / tiles_per_row;
        let depth_size = (resolution * resolution) as usize;
        Self {
            resolution,
            num_slots,
            tiles_per_row,
            tile_resolution,
            tiles: Vec::new(),
            allocator: ShadowAtlasAlloc::new(num_slots),
            depth_buffer: vec![1.0f32; depth_size],
        }
    }

    /// Get the UV offset and scale for a given slot.
    pub fn slot_uv_rect(&self, slot: u32) -> (Vec2, Vec2) {
        let col = slot % self.tiles_per_row;
        let row = slot / self.tiles_per_row;
        let scale = 1.0 / self.tiles_per_row as f32;
        let offset = Vec2::new(col as f32 * scale, row as f32 * scale);
        (offset, Vec2::splat(scale))
    }

    /// Allocate a tile for a light. Returns slot index on success.
    pub fn allocate_tile(&mut self, light_id: u32, current_frame: u32) -> Option<u32> {
        if let Some(slot) = self.allocator.allocate(light_id) {
            // Update or create tile entry
            if let Some(tile) = self.tiles.iter_mut().find(|t| t.light_id == light_id) {
                tile.last_used = current_frame;
                tile.dirty = false;
            } else {
                let mut tile = ShadowTile::new(light_id, slot);
                tile.last_used = current_frame;
                self.tiles.push(tile);
            }
            Some(slot)
        } else {
            // Evict LRU and retry
            let tiles = self.tiles.clone();
            if let Some(_evicted_slot) = self.allocator.evict_lru(&tiles) {
                self.tiles.retain(|t| self.allocator.is_allocated(t.light_id));
                self.allocator.allocate(light_id).map(|slot| {
                    let mut tile = ShadowTile::new(light_id, slot);
                    tile.last_used = current_frame;
                    self.tiles.push(tile);
                    slot
                })
            } else {
                None
            }
        }
    }

    /// Mark a tile as dirty (needs re-render).
    pub fn mark_dirty(&mut self, light_id: u32) {
        if let Some(tile) = self.tiles.iter_mut().find(|t| t.light_id == light_id) {
            tile.dirty = true;
        }
    }

    /// Get all dirty tiles.
    pub fn dirty_tiles(&self) -> Vec<&ShadowTile> {
        self.tiles.iter().filter(|t| t.dirty).collect()
    }

    /// Mark all tiles as rendered (clean).
    pub fn clear_dirty(&mut self) {
        for tile in &mut self.tiles {
            tile.dirty = false;
        }
    }

    /// Write depth values into the atlas at a given slot.
    pub fn write_tile_depth(&mut self, slot: u32, data: &[f32]) {
        let tr = self.tile_resolution as usize;
        let col = (slot % self.tiles_per_row) as usize;
        let row = (slot / self.tiles_per_row) as usize;
        let atlas_w = self.resolution as usize;
        let start_x = col * tr;
        let start_y = row * tr;
        let copy_len = data.len().min(tr * tr);
        for i in 0..copy_len {
            let tx = i % tr;
            let ty = i / tr;
            let ax = start_x + tx;
            let ay = start_y + ty;
            let atlas_idx = ay * atlas_w + ax;
            if atlas_idx < self.depth_buffer.len() {
                self.depth_buffer[atlas_idx] = data[i];
            }
        }
    }

    /// Sample depth from the atlas at a given slot and UV.
    pub fn sample_depth(&self, slot: u32, uv: Vec2) -> f32 {
        let (offset, scale) = self.slot_uv_rect(slot);
        let atlas_uv = offset + uv * scale;
        let ax = (atlas_uv.x * self.resolution as f32).clamp(0.0, self.resolution as f32 - 1.0) as usize;
        let ay = (atlas_uv.y * self.resolution as f32).clamp(0.0, self.resolution as f32 - 1.0) as usize;
        let idx = ay * self.resolution as usize + ax;
        self.depth_buffer.get(idx).copied().unwrap_or(1.0)
    }

    pub fn clear(&mut self) {
        for v in self.depth_buffer.iter_mut() {
            *v = 1.0;
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowBias
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ShadowBias {
    pub constant: f32,
    pub slope_scale: f32,
    pub normal_offset: f32,
}

impl ShadowBias {
    pub fn new(constant: f32, slope_scale: f32, normal_offset: f32) -> Self {
        Self { constant, slope_scale, normal_offset }
    }

    pub fn default_directional() -> Self {
        Self { constant: 0.005, slope_scale: 0.01, normal_offset: 0.02 }
    }

    pub fn default_point() -> Self {
        Self { constant: 0.01, slope_scale: 0.02, normal_offset: 0.01 }
    }

    /// Compute total bias for a given slope (NoL = dot(normal, light_dir)).
    pub fn compute_bias(&self, nol: f32) -> f32 {
        let slope_factor = (1.0 - nol.clamp(0.0, 1.0)).max(0.0001).sqrt();
        self.constant + self.slope_scale * slope_factor
    }

    /// Compute normal-offset position.
    pub fn offset_position(&self, pos: Vec3, normal: Vec3) -> Vec3 {
        pos + normal * self.normal_offset
    }
}

// ---------------------------------------------------------------------------
// ShadowFilter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ShadowFilter {
    Hard,
    PCF { radius: u32, samples: u32 },
    PCSS { blocker_radius: f32, filter_radius: f32 },
    VSM,
    EVSM,
}

impl ShadowFilter {
    pub fn sample_count(&self) -> u32 {
        match self {
            ShadowFilter::Hard => 1,
            ShadowFilter::PCF { samples, .. } => *samples,
            ShadowFilter::PCSS { .. } => 32,
            ShadowFilter::VSM => 1,
            ShadowFilter::EVSM => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// CascadeShadowMap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CascadeShadowMap {
    pub num_cascades: u32,
    pub split_depths: [f32; 5],
    pub cascade_matrices: [Mat4; 4],
    pub cascade_biases: [ShadowBias; 4],
    pub cascade_resolutions: [u32; 4],
    pub depth_buffers: Vec<Vec<f32>>,
    pub light_direction: Vec3,
    pub lambda: f32,
    pub stabilize: bool,
    pub blend_band: f32,
}

impl CascadeShadowMap {
    pub fn new(num_cascades: u32, resolution: u32, light_direction: Vec3) -> Self {
        let depth_buffers = (0..num_cascades as usize)
            .map(|_| vec![1.0f32; (resolution * resolution) as usize])
            .collect();
        Self {
            num_cascades,
            split_depths: [0.1, 10.0, 50.0, 200.0, 1000.0],
            cascade_matrices: [Mat4::IDENTITY; 4],
            cascade_biases: [
                ShadowBias::new(0.002, 0.005, 0.01),
                ShadowBias::new(0.003, 0.007, 0.015),
                ShadowBias::new(0.005, 0.01, 0.02),
                ShadowBias::new(0.008, 0.015, 0.03),
            ],
            cascade_resolutions: [resolution; 4],
            depth_buffers,
            light_direction: light_direction.normalize(),
            lambda: 0.75,
            stabilize: true,
            blend_band: 0.1,
        }
    }

    /// Practical split scheme combining logarithmic and uniform splits.
    pub fn compute_splits(&mut self, near: f32, far: f32, lambda: f32) {
        self.lambda = lambda;
        self.split_depths[0] = near;
        self.split_depths[self.num_cascades as usize] = far;
        for i in 1..self.num_cascades as usize {
            let t = i as f32 / self.num_cascades as f32;
            let log_split = near * (far / near).powf(t);
            let uni_split = near + (far - near) * t;
            self.split_depths[i] = lambda * log_split + (1.0 - lambda) * uni_split;
        }
    }

    /// Compute the 8 frustum corner points for a cascade in world space.
    pub fn compute_cascade_frustum(&self, cascade: u32, view_proj_inv: Mat4) -> [Vec3; 8] {
        let idx = cascade as usize;
        let n = self.split_depths[idx];
        let f = self.split_depths[idx + 1];
        // Full view frustum near/far in NDC
        // We'll reconstruct via inverse VP at the split planes
        let ndc_near = (n - 0.1) / (1000.0 - 0.1) * 2.0 - 1.0; // approximate
        let ndc_far = (f - 0.1) / (1000.0 - 0.1) * 2.0 - 1.0;
        let corners_ndc = [
            Vec3::new(-1.0, -1.0, ndc_near),
            Vec3::new( 1.0, -1.0, ndc_near),
            Vec3::new(-1.0,  1.0, ndc_near),
            Vec3::new( 1.0,  1.0, ndc_near),
            Vec3::new(-1.0, -1.0, ndc_far),
            Vec3::new( 1.0, -1.0, ndc_far),
            Vec3::new(-1.0,  1.0, ndc_far),
            Vec3::new( 1.0,  1.0, ndc_far),
        ];
        let mut corners = [Vec3::ZERO; 8];
        for (i, &ndc) in corners_ndc.iter().enumerate() {
            corners[i] = view_proj_inv.project_point3(ndc);
        }
        corners
    }

    /// Compute a tight light-space orthographic projection fitting the frustum corners.
    pub fn fit_to_frustum(&self, corners: &[Vec3; 8], light_dir: Vec3) -> Mat4 {
        // Build a stable light-space coordinate system
        let light_forward = -light_dir.normalize();
        let light_right = if light_forward.abs().dot(Vec3::Y) < 0.99 {
            Vec3::Y.cross(light_forward).normalize()
        } else {
            Vec3::Z.cross(light_forward).normalize()
        };
        let light_up = light_forward.cross(light_right).normalize();
        let light_view = Mat4::from_cols(
            Vec4::new(light_right.x, light_up.x, light_forward.x, 0.0),
            Vec4::new(light_right.y, light_up.y, light_forward.y, 0.0),
            Vec4::new(light_right.z, light_up.z, light_forward.z, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        // Transform corners to light space
        let mut min_x = f32::MAX;
        let mut max_x = -f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_y = -f32::MAX;
        let mut min_z = f32::MAX;
        let mut max_z = -f32::MAX;
        for &corner in corners.iter() {
            let ls = light_view.transform_point3(corner);
            min_x = min_x.min(ls.x);
            max_x = max_x.max(ls.x);
            min_y = min_y.min(ls.y);
            max_y = max_y.max(ls.y);
            min_z = min_z.min(ls.z);
            max_z = max_z.max(ls.z);
        }
        // Optional stabilization: round to texel size
        if self.stabilize {
            let texel_size = (max_x - min_x) / 1024.0;
            min_x = (min_x / texel_size).floor() * texel_size;
            max_x = (max_x / texel_size).ceil() * texel_size;
            min_y = (min_y / texel_size).floor() * texel_size;
            max_y = (max_y / texel_size).ceil() * texel_size;
        }
        // Pull back the near plane to capture shadow casters behind the frustum
        let z_pull_back = 100.0;
        let ortho = Mat4::orthographic_rh(min_x, max_x, min_y, max_y, min_z - z_pull_back, max_z);
        ortho * light_view
    }

    /// Update all cascade matrices for the current frame.
    pub fn update_cascades(&mut self, near: f32, far: f32, view_proj_inv: Mat4) {
        self.compute_splits(near, far, self.lambda);
        for i in 0..self.num_cascades.min(4) {
            let corners = self.compute_cascade_frustum(i, view_proj_inv);
            self.cascade_matrices[i as usize] = self.fit_to_frustum(&corners, self.light_direction);
        }
    }

    /// Determine which cascade a world-space point belongs to, and its blend weight.
    pub fn select_cascade(&self, depth_view_space: f32) -> (u32, f32) {
        for i in 0..self.num_cascades as usize {
            let near = self.split_depths[i];
            let far = self.split_depths[i + 1];
            if depth_view_space >= near && depth_view_space < far {
                let blend_start = far * (1.0 - self.blend_band);
                let blend = if depth_view_space > blend_start {
                    (depth_view_space - blend_start) / (far - blend_start)
                } else {
                    0.0
                };
                return (i as u32, blend);
            }
        }
        (self.num_cascades - 1, 0.0)
    }

    /// Sample shadow factor for a world position using the appropriate cascade.
    pub fn sample_shadow(&self, world_pos: Vec3, depth_view: f32, filter: &ShadowFilter) -> f32 {
        let (cascade_idx, blend) = self.select_cascade(depth_view);
        let matrix = self.cascade_matrices[cascade_idx as usize];
        let clip = matrix.project_point3(world_pos);
        let ndc = Vec3::new(clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5, clip.z);
        if ndc.x < 0.0 || ndc.x > 1.0 || ndc.y < 0.0 || ndc.y > 1.0 {
            return 1.0;
        }
        let bias = self.cascade_biases[cascade_idx as usize];
        let shadow0 = self.sample_cascade(cascade_idx, ndc, bias.constant, filter);
        // Blend with next cascade at transition
        if blend > 0.0 && cascade_idx + 1 < self.num_cascades {
            let next_mat = self.cascade_matrices[(cascade_idx + 1) as usize];
            let nclip = next_mat.project_point3(world_pos);
            let nndc = Vec3::new(nclip.x * 0.5 + 0.5, nclip.y * 0.5 + 0.5, nclip.z);
            let nbias = self.cascade_biases[(cascade_idx + 1) as usize];
            let shadow1 = self.sample_cascade(cascade_idx + 1, nndc, nbias.constant, filter);
            shadow0 + blend * (shadow1 - shadow0)
        } else {
            shadow0
        }
    }

    fn sample_cascade(&self, cascade: u32, ndc: Vec3, bias: f32, filter: &ShadowFilter) -> f32 {
        let idx = cascade as usize;
        if idx >= self.depth_buffers.len() {
            return 1.0;
        }
        let res = self.cascade_resolutions[idx];
        let buf = &self.depth_buffers[idx];
        match filter {
            ShadowFilter::Hard => {
                self.sample_depth_point(buf, res, ndc.x, ndc.y, ndc.z, bias)
            }
            ShadowFilter::PCF { radius, samples } => {
                self.sample_depth_pcf(buf, res, ndc, bias, *radius, *samples)
            }
            ShadowFilter::PCSS { blocker_radius, filter_radius } => {
                self.sample_depth_pcss(buf, res, ndc, bias, *blocker_radius, *filter_radius)
            }
            ShadowFilter::VSM => {
                self.sample_depth_vsm(buf, res, ndc, bias)
            }
            ShadowFilter::EVSM => {
                self.sample_depth_evsm(buf, res, ndc, bias)
            }
        }
    }

    fn sample_depth_point(&self, buf: &[f32], res: u32, u: f32, v: f32, depth: f32, bias: f32) -> f32 {
        let px = ((u * res as f32) as u32).min(res - 1);
        let py = ((v * res as f32) as u32).min(res - 1);
        let stored = buf[(py * res + px) as usize];
        if depth - bias > stored { 0.0 } else { 1.0 }
    }

    fn sample_depth_pcf(&self, buf: &[f32], res: u32, ndc: Vec3, bias: f32, radius: u32, samples: u32) -> f32 {
        let r = radius as i32;
        let texel = 1.0 / res as f32;
        let mut sum = 0.0f32;
        let mut count = 0u32;
        let step = (2 * r + 1) as u32;
        let max_s = step * step;
        let skip = if max_s > samples { (max_s / samples).max(1) } else { 1 };
        let mut si = 0u32;
        'outer: for dy in -r..=r {
            for dx in -r..=r {
                if si % skip != 0 {
                    si += 1;
                    continue;
                }
                si += 1;
                let su = (ndc.x + dx as f32 * texel).clamp(0.0, 1.0);
                let sv = (ndc.y + dy as f32 * texel).clamp(0.0, 1.0);
                sum += self.sample_depth_point(buf, res, su, sv, ndc.z, bias);
                count += 1;
                if count >= samples {
                    break 'outer;
                }
            }
        }
        if count > 0 { sum / count as f32 } else { 1.0 }
    }

    fn estimate_blocker_distance(&self, buf: &[f32], res: u32, ndc: Vec3, bias: f32, search_radius: f32) -> Option<f32> {
        let texel = 1.0 / res as f32;
        let steps = 8i32;
        let mut blocker_sum = 0.0f32;
        let mut blocker_count = 0u32;
        for dy in -steps..=steps {
            for dx in -steps..=steps {
                let su = (ndc.x + dx as f32 * texel * search_radius).clamp(0.0, 1.0);
                let sv = (ndc.y + dy as f32 * texel * search_radius).clamp(0.0, 1.0);
                let px = ((su * res as f32) as u32).min(res - 1);
                let py = ((sv * res as f32) as u32).min(res - 1);
                let stored = buf[(py * res + px) as usize];
                if ndc.z - bias > stored {
                    blocker_sum += stored;
                    blocker_count += 1;
                }
            }
        }
        if blocker_count > 0 {
            Some(blocker_sum / blocker_count as f32)
        } else {
            None
        }
    }

    fn sample_depth_pcss(&self, buf: &[f32], res: u32, ndc: Vec3, bias: f32, blocker_radius: f32, filter_radius: f32) -> f32 {
        let blocker_dist = self.estimate_blocker_distance(buf, res, ndc, bias, blocker_radius);
        match blocker_dist {
            None => 1.0, // No blockers, fully lit
            Some(d_blocker) => {
                // Penumbra size proportional to distance between blocker and receiver
                let w_penumbra = (ndc.z - d_blocker) / d_blocker * filter_radius;
                self.sample_depth_pcf(buf, res, ndc, bias, w_penumbra.ceil() as u32, 32)
            }
        }
    }

    fn sample_depth_vsm(&self, buf: &[f32], res: u32, ndc: Vec3, bias: f32) -> f32 {
        // VSM requires a moments texture (mean and variance).
        // Approximate using neighboring samples.
        let px = ((ndc.x * res as f32) as u32).min(res - 1);
        let py = ((ndc.y * res as f32) as u32).min(res - 1);
        let mut sum = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut count = 0u32;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let sx = (px as i32 + dx).clamp(0, res as i32 - 1) as u32;
                let sy = (py as i32 + dy).clamp(0, res as i32 - 1) as u32;
                let d = buf[(sy * res + sx) as usize];
                sum += d;
                sum2 += d * d;
                count += 1;
            }
        }
        let mean = sum / count as f32;
        let variance = (sum2 / count as f32 - mean * mean).max(bias);
        let t = ndc.z;
        if t <= mean {
            return 1.0;
        }
        // Chebyshev's inequality upper bound
        let d = t - mean;
        (variance / (variance + d * d)).clamp(0.0, 1.0)
    }

    fn sample_depth_evsm(&self, buf: &[f32], res: u32, ndc: Vec3, bias: f32) -> f32 {
        // EVSM: exponentially warped shadow maps (approximated)
        let exp_c = 40.0f32;
        let px = ((ndc.x * res as f32) as u32).min(res - 1);
        let py = ((ndc.y * res as f32) as u32).min(res - 1);
        let stored = buf[(py * res + px) as usize];
        let warped_depth = (exp_c * ndc.z).exp();
        let warped_stored = (exp_c * stored).exp();
        // Clamp the result
        let p = (warped_stored / (warped_depth + bias)).clamp(0.0, 1.0);
        p.powi(2)
    }
}

// ---------------------------------------------------------------------------
// PointLightShadow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PointLightShadow {
    pub light_position: Vec3,
    pub near_plane: f32,
    pub far_plane: f32,
    pub resolution: u32,
    pub depth_buffers: Vec<Vec<f32>>,
    pub bias: ShadowBias,
}

impl PointLightShadow {
    pub fn new(position: Vec3, near: f32, far: f32, resolution: u32) -> Self {
        let face_size = (resolution * resolution) as usize;
        Self {
            light_position: position,
            near_plane: near,
            far_plane: far,
            resolution,
            depth_buffers: vec![vec![1.0f32; face_size]; 6],
            bias: ShadowBias::default_point(),
        }
    }

    /// Compute the 6 face view-projection matrices for a cube shadow map.
    pub fn face_matrices(&self, pos: Vec3) -> [Mat4; 6] {
        let projection = Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_2,
            1.0,
            self.near_plane,
            self.far_plane,
        );
        let directions = [
            (Vec3::X, Vec3::NEG_Y),
            (Vec3::NEG_X, Vec3::NEG_Y),
            (Vec3::Y, Vec3::Z),
            (Vec3::NEG_Y, Vec3::NEG_Z),
            (Vec3::Z, Vec3::NEG_Y),
            (Vec3::NEG_Z, Vec3::NEG_Y),
        ];
        let mut matrices = [Mat4::IDENTITY; 6];
        for (i, (dir, up)) in directions.iter().enumerate() {
            let view = Mat4::look_at_rh(pos, pos + *dir, *up);
            matrices[i] = projection * view;
        }
        matrices
    }

    /// Determine which cube face a direction falls into.
    pub fn face_index(dir: Vec3) -> usize {
        let ax = dir.x.abs();
        let ay = dir.y.abs();
        let az = dir.z.abs();
        if ax >= ay && ax >= az {
            if dir.x > 0.0 { 0 } else { 1 }
        } else if ay >= ax && ay >= az {
            if dir.y > 0.0 { 2 } else { 3 }
        } else {
            if dir.z > 0.0 { 4 } else { 5 }
        }
    }

    /// Sample shadow at a world position (for a given receiver position).
    pub fn sample_shadow(&self, world_pos: Vec3) -> f32 {
        let to_light = self.light_position - world_pos;
        let dist = to_light.length();
        if dist >= self.far_plane {
            return 1.0;
        }
        let dir = to_light / dist.max(1e-6);
        let face = Self::face_index(-dir);
        let mats = self.face_matrices(self.light_position);
        let clip = mats[face].project_point3(world_pos);
        let ndc = Vec3::new(clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5, clip.z);
        let res = self.resolution;
        let px = ((ndc.x * res as f32) as u32).min(res - 1);
        let py = ((ndc.y * res as f32) as u32).min(res - 1);
        let stored = self.depth_buffers[face][(py * res + px) as usize];
        if ndc.z - self.bias.constant > stored { 0.0 } else { 1.0 }
    }

    /// PCF sampling for point light shadows.
    pub fn sample_shadow_pcf(&self, world_pos: Vec3, kernel_size: u32) -> f32 {
        let to_light = self.light_position - world_pos;
        let dist = to_light.length();
        if dist >= self.far_plane {
            return 1.0;
        }
        let dir = to_light / dist.max(1e-6);
        let face = Self::face_index(-dir);
        let mats = self.face_matrices(self.light_position);
        let clip = mats[face].project_point3(world_pos);
        let ndc = Vec3::new(clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5, clip.z);
        let res = self.resolution;
        let texel = 1.0 / res as f32;
        let mut sum = 0.0f32;
        let mut count = 0u32;
        let r = kernel_size as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let su = (ndc.x + dx as f32 * texel).clamp(0.0, 1.0);
                let sv = (ndc.y + dy as f32 * texel).clamp(0.0, 1.0);
                let px = ((su * res as f32) as u32).min(res - 1);
                let py = ((sv * res as f32) as u32).min(res - 1);
                let stored = self.depth_buffers[face][(py * res + px) as usize];
                sum += if ndc.z - self.bias.constant <= stored { 1.0 } else { 0.0 };
                count += 1;
            }
        }
        if count > 0 { sum / count as f32 } else { 1.0 }
    }
}

// ---------------------------------------------------------------------------
// ShadowCache — caches static geometry shadow maps
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShadowCacheEntry {
    pub light_id: u32,
    pub cascade: u32,
    pub frame_captured: u32,
    pub depth_data: Vec<f32>,
    pub is_valid: bool,
}

#[derive(Debug, Clone)]
pub struct ShadowCache {
    pub entries: HashMap<u64, ShadowCacheEntry>,
    pub max_entries: usize,
    pub invalidation_radius: f32,
    current_frame: u32,
}

impl ShadowCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            invalidation_radius: 0.5,
            current_frame: 0,
        }
    }

    fn make_key(light_id: u32, cascade: u32) -> u64 {
        ((light_id as u64) << 32) | cascade as u64
    }

    pub fn get(&self, light_id: u32, cascade: u32) -> Option<&ShadowCacheEntry> {
        let key = Self::make_key(light_id, cascade);
        self.entries.get(&key).filter(|e| e.is_valid)
    }

    pub fn store(&mut self, light_id: u32, cascade: u32, depth_data: Vec<f32>) {
        if self.entries.len() >= self.max_entries {
            // Remove oldest entry
            let oldest_key = self.entries.iter()
                .min_by_key(|(_, e)| e.frame_captured)
                .map(|(k, _)| *k);
            if let Some(k) = oldest_key {
                self.entries.remove(&k);
            }
        }
        let key = Self::make_key(light_id, cascade);
        self.entries.insert(key, ShadowCacheEntry {
            light_id,
            cascade,
            frame_captured: self.current_frame,
            depth_data,
            is_valid: true,
        });
    }

    /// Invalidate cache entries for lights affected by a moving object.
    pub fn invalidate_near_position(&mut self, pos: Vec3, lights: &[(u32, Vec3)]) {
        for (light_id, light_pos) in lights {
            if (pos - *light_pos).length() < self.invalidation_radius * 100.0 {
                // Invalidate all cascades for this light
                for cascade in 0..4u32 {
                    let key = Self::make_key(*light_id, cascade);
                    if let Some(entry) = self.entries.get_mut(&key) {
                        entry.is_valid = false;
                    }
                }
            }
        }
    }

    pub fn invalidate_light(&mut self, light_id: u32) {
        for cascade in 0..4u32 {
            let key = Self::make_key(light_id, cascade);
            if let Some(entry) = self.entries.get_mut(&key) {
                entry.is_valid = false;
            }
        }
    }

    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
        // Purge entries that are too old
        let threshold = self.current_frame.saturating_sub(120); // 2 seconds at 60fps
        self.entries.retain(|_, e| e.frame_captured >= threshold && e.is_valid);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// ShadowRenderer — top-level shadow system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShadowRenderer {
    pub atlas: ShadowAtlas,
    pub csm: CascadeShadowMap,
    pub filter: ShadowFilter,
    pub cache: ShadowCache,
    pub point_shadows: HashMap<u32, PointLightShadow>,
    pub enabled: bool,
    pub current_frame: u32,
}

impl ShadowRenderer {
    pub fn new(atlas_resolution: u32, csm_resolution: u32, light_dir: Vec3) -> Self {
        Self {
            atlas: ShadowAtlas::new(atlas_resolution, 64),
            csm: CascadeShadowMap::new(4, csm_resolution, light_dir),
            filter: ShadowFilter::PCF { radius: 2, samples: 16 },
            cache: ShadowCache::new(256),
            point_shadows: HashMap::new(),
            enabled: true,
            current_frame: 0,
        }
    }

    pub fn update_cascades(&mut self, near: f32, far: f32, view_proj_inv: Mat4) {
        self.csm.update_cascades(near, far, view_proj_inv);
    }

    pub fn render_shadow_pass(&mut self) {
        let dirty = self.atlas.dirty_tiles();
        for tile in dirty {
            let _ = tile.light_id;
            // In a real implementation, render scene depth from light's perspective here.
        }
        self.atlas.clear_dirty();
        self.cache.advance_frame();
        self.current_frame += 1;
    }

    pub fn compute_shadow_factor(&self, world_pos: Vec3, depth_view: f32, normal: Vec3) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let biased_pos = self.csm.cascade_biases[0].offset_position(world_pos, normal);
        self.csm.sample_shadow(biased_pos, depth_view, &self.filter)
    }

    pub fn add_point_light_shadow(&mut self, light_id: u32, shadow: PointLightShadow) {
        self.point_shadows.insert(light_id, shadow);
    }

    pub fn compute_point_shadow(&self, light_id: u32, world_pos: Vec3) -> f32 {
        if let Some(shadow) = self.point_shadows.get(&light_id) {
            shadow.sample_shadow_pcf(world_pos, 1)
        } else {
            1.0
        }
    }

    pub fn set_filter(&mut self, filter: ShadowFilter) {
        self.filter = filter;
    }
}

// ---------------------------------------------------------------------------
// ShadowDebugVisualize
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShadowDebugVisualize {
    pub show_cascades: bool,
    pub show_atlas: bool,
    pub show_bias: bool,
    pub cascade_colors: [Vec4; 4],
    pub overlay_alpha: f32,
}

impl ShadowDebugVisualize {
    pub fn new() -> Self {
        Self {
            show_cascades: false,
            show_atlas: false,
            show_bias: false,
            cascade_colors: [
                Vec4::new(1.0, 0.2, 0.2, 0.3), // Cascade 0: red
                Vec4::new(0.2, 1.0, 0.2, 0.3), // Cascade 1: green
                Vec4::new(0.2, 0.2, 1.0, 0.3), // Cascade 2: blue
                Vec4::new(1.0, 1.0, 0.2, 0.3), // Cascade 3: yellow
            ],
            overlay_alpha: 0.3,
        }
    }

    /// Get the debug color tint for a fragment given its depth and the CSM.
    pub fn get_cascade_color(&self, csm: &CascadeShadowMap, depth_view: f32) -> Vec4 {
        if !self.show_cascades {
            return Vec4::ONE;
        }
        let (cascade, blend) = csm.select_cascade(depth_view);
        let color0 = self.cascade_colors[cascade as usize];
        if blend > 0.0 && (cascade + 1) < csm.num_cascades {
            let color1 = self.cascade_colors[(cascade + 1) as usize];
            Vec4::new(
                color0.x + blend * (color1.x - color0.x),
                color0.y + blend * (color1.y - color0.y),
                color0.z + blend * (color1.z - color0.z),
                color0.w + blend * (color1.w - color0.w),
            )
        } else {
            color0
        }
    }

    /// Visualize the shadow atlas tiles (returns UV rects of active tiles).
    pub fn visualize_atlas_tiles(&self, atlas: &ShadowAtlas) -> Vec<(Vec2, Vec2, Vec4)> {
        if !self.show_atlas {
            return Vec::new();
        }
        let colors = [
            Vec4::new(1.0, 0.5, 0.0, 0.5),
            Vec4::new(0.0, 0.8, 1.0, 0.5),
            Vec4::new(0.8, 0.0, 1.0, 0.5),
            Vec4::new(0.0, 1.0, 0.5, 0.5),
        ];
        atlas.tiles.iter().enumerate().map(|(i, tile)| {
            let (offset, scale) = atlas.slot_uv_rect(tile.slot);
            let color = colors[i % colors.len()];
            (offset, scale, color)
        }).collect()
    }

    /// Compute bias heatmap color for a slope (NoL).
    pub fn bias_heatmap_color(&self, nol: f32, bias: &ShadowBias) -> Vec4 {
        if !self.show_bias {
            return Vec4::ONE;
        }
        let computed = bias.compute_bias(nol);
        // Map bias range [0, 0.05] to color: green=low, red=high
        let t = (computed / 0.05).clamp(0.0, 1.0);
        Vec4::new(t, 1.0 - t, 0.0, 1.0)
    }
}

impl Default for ShadowDebugVisualize {
    fn default() -> Self {
        Self::new()
    }
}
