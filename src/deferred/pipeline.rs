//! Full deferred rendering pipeline.
//!
//! Implements multi-pass deferred rendering with:
//! - Depth pre-pass (early Z rejection, front-to-back sorting)
//! - Geometry pass (fill G-Buffer from scene geometry)
//! - Lighting pass (fullscreen quad evaluating all lights against G-Buffer)
//! - Forward pass (transparent objects sorted back-to-front)
//! - Post-process pass (bloom, tone mapping, anti-aliasing)
//! - HDR framebuffer management
//! - Auto-exposure with histogram or average luminance
//! - Render queue with opaque/transparent/overlay buckets

use std::collections::HashMap;

use super::{
    Viewport, Mat4,
    vec3_sub, vec3_dot, vec3_length, clampf, lerpf, saturate,
};
use super::gbuffer::GBuffer;
use super::materials::MaterialSortKey;

// ---------------------------------------------------------------------------
// Light types
// ---------------------------------------------------------------------------

/// Types of lights supported by the deferred lighting pass.
#[derive(Debug, Clone)]
pub enum LightType {
    /// Directional light (sun). Direction + color + intensity.
    Directional {
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        cast_shadows: bool,
    },
    /// Point light (omnidirectional). Position + color + intensity + range.
    Point {
        position: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
        cast_shadows: bool,
    },
    /// Spot light. Position + direction + color + angles.
    Spot {
        position: [f32; 3],
        direction: [f32; 3],
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
        cast_shadows: bool,
    },
    /// Area light (rectangle). Position + normal + up + size.
    Area {
        position: [f32; 3],
        normal: [f32; 3],
        up: [f32; 3],
        width: f32,
        height: f32,
        color: [f32; 3],
        intensity: f32,
    },
    /// Ambient light (global illumination approximation).
    Ambient {
        color: [f32; 3],
        intensity: f32,
    },
}

impl LightType {
    /// Get the world-space position of this light (if applicable).
    pub fn position(&self) -> Option<[f32; 3]> {
        match self {
            Self::Directional { .. } | Self::Ambient { .. } => None,
            Self::Point { position, .. }
            | Self::Spot { position, .. }
            | Self::Area { position, .. } => Some(*position),
        }
    }

    /// Get the effective range/radius of influence.
    pub fn range(&self) -> f32 {
        match self {
            Self::Directional { .. } | Self::Ambient { .. } => f32::MAX,
            Self::Point { range, .. } | Self::Spot { range, .. } => *range,
            Self::Area { width, height, .. } => (*width + *height) * 2.0,
        }
    }

    /// Get the color of this light.
    pub fn color(&self) -> [f32; 3] {
        match self {
            Self::Directional { color, .. }
            | Self::Point { color, .. }
            | Self::Spot { color, .. }
            | Self::Area { color, .. }
            | Self::Ambient { color, .. } => *color,
        }
    }

    /// Get the intensity.
    pub fn intensity(&self) -> f32 {
        match self {
            Self::Directional { intensity, .. }
            | Self::Point { intensity, .. }
            | Self::Spot { intensity, .. }
            | Self::Area { intensity, .. }
            | Self::Ambient { intensity, .. } => *intensity,
        }
    }

    /// Whether this light casts shadows.
    pub fn casts_shadows(&self) -> bool {
        match self {
            Self::Directional { cast_shadows, .. }
            | Self::Point { cast_shadows, .. }
            | Self::Spot { cast_shadows, .. } => *cast_shadows,
            _ => false,
        }
    }

    /// Compute the light contribution at a given surface point.
    /// Returns (light_dir_to_surface, attenuation, color).
    pub fn evaluate(&self, surface_pos: [f32; 3]) -> ([f32; 3], f32, [f32; 3]) {
        match self {
            Self::Directional { direction, color, intensity, .. } => {
                let dir = [-direction[0], -direction[1], -direction[2]];
                (dir, *intensity, *color)
            }
            Self::Point { position, color, intensity, range, .. } => {
                let to_light = vec3_sub(*position, surface_pos);
                let dist = vec3_length(to_light);
                if dist > *range || dist < 1e-6 {
                    return ([0.0, 0.0, 0.0], 0.0, *color);
                }
                let dir = [to_light[0] / dist, to_light[1] / dist, to_light[2] / dist];
                let att = point_attenuation(dist, *range) * *intensity;
                (dir, att, *color)
            }
            Self::Spot {
                position, direction, color, intensity, range,
                inner_angle, outer_angle, ..
            } => {
                let to_light = vec3_sub(*position, surface_pos);
                let dist = vec3_length(to_light);
                if dist > *range || dist < 1e-6 {
                    return ([0.0, 0.0, 0.0], 0.0, *color);
                }
                let dir = [to_light[0] / dist, to_light[1] / dist, to_light[2] / dist];
                let cos_angle = -vec3_dot(dir, *direction);
                let cos_inner = inner_angle.cos();
                let cos_outer = outer_angle.cos();
                let spot_att = saturate((cos_angle - cos_outer) / (cos_inner - cos_outer).max(1e-6));
                let att = point_attenuation(dist, *range) * spot_att * *intensity;
                (dir, att, *color)
            }
            Self::Area { position, color, intensity, .. } => {
                let to_light = vec3_sub(*position, surface_pos);
                let dist = vec3_length(to_light);
                if dist < 1e-6 {
                    return ([0.0, 0.0, 1.0], *intensity, *color);
                }
                let dir = [to_light[0] / dist, to_light[1] / dist, to_light[2] / dist];
                let att = *intensity / (dist * dist + 1.0);
                (dir, att, *color)
            }
            Self::Ambient { color, intensity } => {
                ([0.0, 1.0, 0.0], *intensity, *color)
            }
        }
    }
}

/// Smooth distance-based attenuation for point/spot lights.
fn point_attenuation(distance: f32, range: f32) -> f32 {
    let ratio = clampf(distance / range, 0.0, 1.0);
    let att_factor = 1.0 - ratio * ratio;
    (att_factor * att_factor).max(0.0) / (distance * distance + 1.0)
}

// ---------------------------------------------------------------------------
// Render items and sorting
// ---------------------------------------------------------------------------

/// How to sort render items within a bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Sort front-to-back (for opaque geometry, early Z rejection).
    FrontToBack,
    /// Sort back-to-front (for transparent geometry).
    BackToFront,
    /// Sort by material to minimize state changes.
    ByMaterial,
    /// No sorting.
    None,
}

/// Which render bucket an item belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderBucket {
    /// Opaque geometry (rendered in geometry pass).
    Opaque,
    /// Transparent geometry (rendered in forward pass).
    Transparent,
    /// UI / overlay (rendered last, no depth test).
    Overlay,
    /// Debug geometry (wireframes, gizmos).
    Debug,
    /// Sky / background (rendered before geometry, writes to far plane).
    Sky,
}

impl RenderBucket {
    pub fn default_sort_mode(&self) -> SortMode {
        match self {
            Self::Opaque => SortMode::FrontToBack,
            Self::Transparent => SortMode::BackToFront,
            Self::Overlay => SortMode::None,
            Self::Debug => SortMode::None,
            Self::Sky => SortMode::None,
        }
    }
}

/// A single item to be rendered.
#[derive(Debug, Clone)]
pub struct RenderItem {
    /// Unique identifier for this item.
    pub id: u64,
    /// World transform.
    pub transform: Mat4,
    /// Mesh/geometry handle (opaque).
    pub mesh_handle: u64,
    /// Material index into the material library.
    pub material_index: u32,
    /// Material sort key for batching.
    pub sort_key: MaterialSortKey,
    /// Distance from camera (computed during sorting).
    pub camera_distance: f32,
    /// Which bucket this item belongs to.
    pub bucket: RenderBucket,
    /// Whether this item is visible (frustum culling result).
    pub visible: bool,
    /// Bounding sphere center (world space).
    pub bounds_center: [f32; 3],
    /// Bounding sphere radius.
    pub bounds_radius: f32,
    /// Instance count (for instanced rendering, 1 = no instancing).
    pub instance_count: u32,
    /// Instance data buffer handle (if instanced).
    pub instance_buffer: u64,
    /// Vertex count (for stats).
    pub vertex_count: u32,
    /// Index count (for stats).
    pub index_count: u32,
    /// Whether this item uses alpha testing (cutout).
    pub alpha_test: bool,
    /// Whether this item is two-sided (no backface culling).
    pub two_sided: bool,
}

impl RenderItem {
    pub fn new(id: u64, mesh_handle: u64, material_index: u32) -> Self {
        Self {
            id,
            transform: Mat4::IDENTITY,
            mesh_handle,
            material_index,
            sort_key: MaterialSortKey::default(),
            camera_distance: 0.0,
            bucket: RenderBucket::Opaque,
            visible: true,
            bounds_center: [0.0; 3],
            bounds_radius: 1.0,
            instance_count: 1,
            instance_buffer: 0,
            vertex_count: 0,
            index_count: 0,
            alpha_test: false,
            two_sided: false,
        }
    }

    pub fn with_transform(mut self, t: Mat4) -> Self {
        self.transform = t;
        // Extract position from last column for bounds center
        self.bounds_center = [t.cols[3][0], t.cols[3][1], t.cols[3][2]];
        self
    }

    pub fn with_bucket(mut self, b: RenderBucket) -> Self {
        self.bucket = b;
        self
    }

    pub fn with_bounds(mut self, center: [f32; 3], radius: f32) -> Self {
        self.bounds_center = center;
        self.bounds_radius = radius;
        self
    }

    /// Compute the distance from a camera position.
    pub fn compute_camera_distance(&mut self, camera_pos: [f32; 3]) {
        let dx = self.bounds_center[0] - camera_pos[0];
        let dy = self.bounds_center[1] - camera_pos[1];
        let dz = self.bounds_center[2] - camera_pos[2];
        self.camera_distance = dx * dx + dy * dy + dz * dz;
    }

    /// Check if this item's bounding sphere intersects a frustum (simplified).
    pub fn frustum_cull(&mut self, frustum_planes: &[[f32; 4]; 6]) -> bool {
        for plane in frustum_planes {
            let dist = plane[0] * self.bounds_center[0]
                + plane[1] * self.bounds_center[1]
                + plane[2] * self.bounds_center[2]
                + plane[3];
            if dist < -self.bounds_radius {
                self.visible = false;
                return false;
            }
        }
        self.visible = true;
        true
    }
}

// ---------------------------------------------------------------------------
// Render queue
// ---------------------------------------------------------------------------

/// Collects and sorts render items into buckets for the pipeline passes.
#[derive(Debug)]
pub struct RenderQueue {
    /// All render items, partitioned by bucket.
    pub buckets: HashMap<RenderBucket, Vec<RenderItem>>,
    /// Sort modes per bucket (overridable).
    pub sort_modes: HashMap<RenderBucket, SortMode>,
    /// Camera position used for distance-based sorting.
    pub camera_position: [f32; 3],
    /// Frustum planes for culling [left, right, bottom, top, near, far].
    pub frustum_planes: [[f32; 4]; 6],
    /// Total items submitted this frame.
    pub total_submitted: u32,
    /// Total items visible after culling.
    pub total_visible: u32,
    /// Total items culled.
    pub total_culled: u32,
    /// Next item ID.
    next_id: u64,
}

impl RenderQueue {
    pub fn new() -> Self {
        let mut sort_modes = HashMap::new();
        sort_modes.insert(RenderBucket::Opaque, SortMode::FrontToBack);
        sort_modes.insert(RenderBucket::Transparent, SortMode::BackToFront);
        sort_modes.insert(RenderBucket::Overlay, SortMode::None);
        sort_modes.insert(RenderBucket::Debug, SortMode::None);
        sort_modes.insert(RenderBucket::Sky, SortMode::None);

        Self {
            buckets: HashMap::new(),
            sort_modes,
            camera_position: [0.0; 3],
            frustum_planes: [[0.0; 4]; 6],
            total_submitted: 0,
            total_visible: 0,
            total_culled: 0,
            next_id: 1,
        }
    }

    /// Clear all buckets for a new frame.
    pub fn clear(&mut self) {
        for bucket in self.buckets.values_mut() {
            bucket.clear();
        }
        self.total_submitted = 0;
        self.total_visible = 0;
        self.total_culled = 0;
    }

    /// Set the camera position and frustum for this frame.
    pub fn set_camera(&mut self, position: [f32; 3], frustum_planes: [[f32; 4]; 6]) {
        self.camera_position = position;
        self.frustum_planes = frustum_planes;
    }

    /// Submit a render item to the queue.
    pub fn submit(&mut self, mut item: RenderItem) {
        item.id = self.next_id;
        self.next_id += 1;
        self.total_submitted += 1;

        // Compute distance from camera
        item.compute_camera_distance(self.camera_position);

        // Frustum cull
        let visible = item.frustum_cull(&self.frustum_planes);
        if visible {
            self.total_visible += 1;
        } else {
            self.total_culled += 1;
        }

        let bucket = item.bucket;
        self.buckets.entry(bucket).or_default().push(item);
    }

    /// Submit a batch of render items.
    pub fn submit_batch(&mut self, items: Vec<RenderItem>) {
        for item in items {
            self.submit(item);
        }
    }

    /// Sort all buckets according to their sort modes.
    pub fn sort(&mut self) {
        for (bucket, items) in &mut self.buckets {
            // Filter to only visible items
            items.retain(|item| item.visible);

            let mode = self.sort_modes.get(bucket)
                .copied()
                .unwrap_or(bucket.default_sort_mode());

            match mode {
                SortMode::FrontToBack => {
                    items.sort_by(|a, b| {
                        a.camera_distance.partial_cmp(&b.camera_distance)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                SortMode::BackToFront => {
                    items.sort_by(|a, b| {
                        b.camera_distance.partial_cmp(&a.camera_distance)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                SortMode::ByMaterial => {
                    items.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
                }
                SortMode::None => {}
            }
        }
    }

    /// Get items in a specific bucket (sorted).
    pub fn get_bucket(&self, bucket: RenderBucket) -> &[RenderItem] {
        self.buckets.get(&bucket).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get opaque items (convenience).
    pub fn opaque_items(&self) -> &[RenderItem] {
        self.get_bucket(RenderBucket::Opaque)
    }

    /// Get transparent items (convenience).
    pub fn transparent_items(&self) -> &[RenderItem] {
        self.get_bucket(RenderBucket::Transparent)
    }

    /// Get overlay items (convenience).
    pub fn overlay_items(&self) -> &[RenderItem] {
        self.get_bucket(RenderBucket::Overlay)
    }

    /// Return the total number of triangles across all visible items.
    pub fn total_triangles(&self) -> u64 {
        self.buckets.values()
            .flat_map(|items| items.iter())
            .filter(|i| i.visible)
            .map(|i| {
                let tris = if i.index_count > 0 {
                    i.index_count / 3
                } else {
                    i.vertex_count / 3
                };
                tris as u64 * i.instance_count as u64
            })
            .sum()
    }

    /// Return the total number of draw calls across all visible items.
    pub fn total_draw_calls(&self) -> u32 {
        self.buckets.values()
            .flat_map(|items| items.iter())
            .filter(|i| i.visible)
            .count() as u32
    }
}

impl Default for RenderQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HDR Framebuffer
// ---------------------------------------------------------------------------

/// Manages an HDR (RGBA16F) framebuffer for the lighting pass output
/// before tone mapping.
#[derive(Debug)]
pub struct HdrFramebuffer {
    /// Framebuffer object handle.
    pub fbo_handle: u64,
    /// HDR color texture handle.
    pub color_handle: u64,
    /// Depth renderbuffer handle (shared from G-Buffer or separate).
    pub depth_handle: u64,
    /// Current dimensions.
    pub width: u32,
    pub height: u32,
    /// Whether the framebuffer has been allocated.
    pub allocated: bool,
    /// Generation counter.
    pub generation: u32,
    /// Handle counter.
    next_handle: u64,
    /// Optional secondary color attachment for bright pixels (bloom source).
    pub bloom_handle: u64,
    /// Bloom threshold (pixels above this luminance go to bloom buffer).
    pub bloom_threshold: f32,
    /// Number of bloom mip levels.
    pub bloom_mip_levels: u32,
    /// Bloom mip chain handles.
    pub bloom_mips: Vec<u64>,
}

impl HdrFramebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            fbo_handle: 0,
            color_handle: 0,
            depth_handle: 0,
            width,
            height,
            allocated: false,
            generation: 0,
            next_handle: 1000,
            bloom_handle: 0,
            bloom_threshold: 1.0,
            bloom_mip_levels: 5,
            bloom_mips: Vec::new(),
        }
    }

    /// Allocate the HDR framebuffer.
    pub fn create(&mut self) -> Result<(), String> {
        self.fbo_handle = self.alloc_handle();
        self.color_handle = self.alloc_handle();
        self.depth_handle = self.alloc_handle();
        self.bloom_handle = self.alloc_handle();

        // Create bloom mip chain
        self.bloom_mips.clear();
        for _ in 0..self.bloom_mip_levels {
            let handle = self.alloc_handle();
            self.bloom_mips.push(handle);
        }

        self.allocated = true;
        self.generation += 1;
        Ok(())
    }

    /// Destroy the HDR framebuffer.
    pub fn destroy(&mut self) {
        self.fbo_handle = 0;
        self.color_handle = 0;
        self.depth_handle = 0;
        self.bloom_handle = 0;
        self.bloom_mips.clear();
        self.allocated = false;
    }

    /// Resize to new dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        if self.allocated {
            self.generation += 1;
            // In a real engine, reallocate textures
        }
    }

    /// Bind as the render target for the lighting pass.
    pub fn bind(&self) {
        // GL: bind FBO
    }

    /// Unbind.
    pub fn unbind(&self) {
        // GL: bind default FBO
    }

    /// Estimated memory usage in bytes.
    pub fn memory_bytes(&self) -> u64 {
        let base = self.width as u64 * self.height as u64;
        let color = base * 8; // RGBA16F = 8 bytes/pixel
        let depth = base * 4; // D32F = 4 bytes/pixel
        let bloom = base * 8; // RGBA16F bloom
        let bloom_mips = (bloom as f64 * 0.334) as u64; // mip chain overhead
        color + depth + bloom + bloom_mips
    }

    fn alloc_handle(&mut self) -> u64 {
        let h = self.next_handle;
        self.next_handle += 1;
        h
    }
}

// ---------------------------------------------------------------------------
// Exposure / tonemapping
// ---------------------------------------------------------------------------

/// Mode for automatic exposure calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposureMode {
    /// Fixed manual exposure.
    Manual,
    /// Average luminance of the scene.
    AverageLuminance,
    /// Histogram-based (ignore extreme bright/dark percentiles).
    Histogram,
    /// Spot metering (weight center of screen more).
    SpotMetering,
}

/// Controls automatic exposure adaptation.
#[derive(Debug, Clone)]
pub struct ExposureController {
    /// Current exposure mode.
    pub mode: ExposureMode,
    /// Current computed exposure value.
    pub exposure: f32,
    /// Target exposure (what we are adapting toward).
    pub target_exposure: f32,
    /// Manual exposure override (used when mode == Manual).
    pub manual_exposure: f32,
    /// Adaptation speed (how fast exposure changes, in EV/sec).
    pub adaptation_speed_up: f32,
    pub adaptation_speed_down: f32,
    /// Minimum exposure (prevents screen from going too dark).
    pub min_exposure: f32,
    /// Maximum exposure (prevents screen from going too bright).
    pub max_exposure: f32,
    /// EV compensation (artist-controlled bias).
    pub ev_compensation: f32,
    /// Key value for average luminance mode (typically 0.18 for 18% gray).
    pub key_value: f32,
    /// Histogram low percentile to ignore (e.g., 0.1 = bottom 10%).
    pub histogram_low_percentile: f32,
    /// Histogram high percentile to ignore (e.g., 0.9 = top 10%).
    pub histogram_high_percentile: f32,
    /// Number of histogram bins.
    pub histogram_bins: u32,
    /// The histogram data (populated each frame).
    pub histogram: Vec<u32>,
    /// Average luminance computed last frame.
    pub average_luminance: f32,
    /// Spot metering radius (fraction of screen width).
    pub spot_radius: f32,
}

impl ExposureController {
    pub fn new() -> Self {
        Self {
            mode: ExposureMode::AverageLuminance,
            exposure: 1.0,
            target_exposure: 1.0,
            manual_exposure: 1.0,
            adaptation_speed_up: 2.0,
            adaptation_speed_down: 1.0,
            min_exposure: 0.001,
            max_exposure: 100.0,
            ev_compensation: 0.0,
            key_value: 0.18,
            histogram_low_percentile: 0.1,
            histogram_high_percentile: 0.9,
            histogram_bins: 256,
            histogram: vec![0; 256],
            average_luminance: 0.18,
            spot_radius: 0.1,
        }
    }

    /// Update exposure for the current frame.
    pub fn update(&mut self, dt: f32) {
        match self.mode {
            ExposureMode::Manual => {
                self.exposure = self.manual_exposure;
                return;
            }
            ExposureMode::AverageLuminance => {
                self.target_exposure = self.compute_exposure_from_luminance(self.average_luminance);
            }
            ExposureMode::Histogram => {
                let avg = self.compute_histogram_average();
                self.target_exposure = self.compute_exposure_from_luminance(avg);
            }
            ExposureMode::SpotMetering => {
                self.target_exposure = self.compute_exposure_from_luminance(self.average_luminance);
            }
        }

        // Apply EV compensation
        self.target_exposure *= (2.0f32).powf(self.ev_compensation);

        // Clamp
        self.target_exposure = clampf(self.target_exposure, self.min_exposure, self.max_exposure);

        // Adapt
        let speed = if self.target_exposure > self.exposure {
            self.adaptation_speed_up
        } else {
            self.adaptation_speed_down
        };

        let factor = 1.0 - (-speed * dt).exp();
        self.exposure = lerpf(self.exposure, self.target_exposure, factor);
        self.exposure = clampf(self.exposure, self.min_exposure, self.max_exposure);
    }

    /// Compute exposure from average scene luminance.
    fn compute_exposure_from_luminance(&self, luminance: f32) -> f32 {
        if luminance < 1e-6 {
            return self.max_exposure;
        }
        self.key_value / luminance
    }

    /// Compute average luminance from histogram (ignoring extreme percentiles).
    fn compute_histogram_average(&self) -> f32 {
        let total: u32 = self.histogram.iter().sum();
        if total == 0 {
            return 0.18;
        }

        let low_count = (total as f32 * self.histogram_low_percentile) as u32;
        let high_count = (total as f32 * self.histogram_high_percentile) as u32;

        let mut running = 0u32;
        let mut weighted_sum = 0.0f64;
        let mut valid_count = 0u32;

        for (i, &count) in self.histogram.iter().enumerate() {
            let prev_running = running;
            running += count;

            // Skip pixels in the bottom percentile
            if running <= low_count {
                continue;
            }
            // Stop at the top percentile
            if prev_running >= high_count {
                break;
            }

            let contributing = if prev_running < low_count {
                count - (low_count - prev_running)
            } else if running > high_count {
                high_count - prev_running
            } else {
                count
            };

            // Map bin index to log luminance, then to linear
            let t = i as f32 / self.histogram_bins as f32;
            let log_lum = t * 20.0 - 10.0; // map [0,1] to [-10, 10] log range
            let lum = log_lum.exp();

            weighted_sum += lum as f64 * contributing as f64;
            valid_count += contributing;
        }

        if valid_count == 0 {
            return 0.18;
        }

        (weighted_sum / valid_count as f64) as f32
    }

    /// Feed the controller a luminance value for the current frame
    /// (computed from downsampled HDR buffer).
    pub fn feed_luminance(&mut self, luminance: f32) {
        self.average_luminance = luminance.max(1e-6);
    }

    /// Feed a histogram for the current frame.
    pub fn feed_histogram(&mut self, histogram: Vec<u32>) {
        self.histogram = histogram;
    }

    /// Get the current exposure multiplier for the tone mapping shader.
    pub fn exposure_multiplier(&self) -> f32 {
        self.exposure
    }

    /// Reset to defaults.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for ExposureController {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tone mapping
// ---------------------------------------------------------------------------

/// Available tone mapping operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMappingOperator {
    /// Simple Reinhard: L / (1 + L)
    Reinhard,
    /// Extended Reinhard with white point.
    ReinhardExtended,
    /// ACES filmic curve (approximation).
    AcesFilmic,
    /// Uncharted 2 / Hable filmic.
    Hable,
    /// Exposure only (linear, just multiply by exposure).
    Linear,
    /// AgX (new standard tone mapper).
    AgX,
}

impl ToneMappingOperator {
    /// Apply tone mapping to a linear HDR color.
    pub fn apply(&self, color: [f32; 3], exposure: f32) -> [f32; 3] {
        let c = [
            color[0] * exposure,
            color[1] * exposure,
            color[2] * exposure,
        ];
        match self {
            Self::Reinhard => Self::reinhard(c),
            Self::ReinhardExtended => Self::reinhard_extended(c, 4.0),
            Self::AcesFilmic => Self::aces_filmic(c),
            Self::Hable => Self::hable(c),
            Self::Linear => [
                saturate(c[0]),
                saturate(c[1]),
                saturate(c[2]),
            ],
            Self::AgX => Self::agx(c),
        }
    }

    fn reinhard(c: [f32; 3]) -> [f32; 3] {
        [
            c[0] / (1.0 + c[0]),
            c[1] / (1.0 + c[1]),
            c[2] / (1.0 + c[2]),
        ]
    }

    fn reinhard_extended(c: [f32; 3], white_point: f32) -> [f32; 3] {
        let wp2 = white_point * white_point;
        [
            c[0] * (1.0 + c[0] / wp2) / (1.0 + c[0]),
            c[1] * (1.0 + c[1] / wp2) / (1.0 + c[1]),
            c[2] * (1.0 + c[2] / wp2) / (1.0 + c[2]),
        ]
    }

    fn aces_filmic(c: [f32; 3]) -> [f32; 3] {
        // Stephen Hill's ACES approximation
        fn aces_channel(x: f32) -> f32 {
            let a = 2.51;
            let b = 0.03;
            let c = 2.43;
            let d = 0.59;
            let e = 0.14;
            saturate((x * (a * x + b)) / (x * (c * x + d) + e))
        }
        [aces_channel(c[0]), aces_channel(c[1]), aces_channel(c[2])]
    }

    fn hable(c: [f32; 3]) -> [f32; 3] {
        fn hable_partial(x: f32) -> f32 {
            let a = 0.15;
            let b = 0.50;
            let cc = 0.10;
            let d = 0.20;
            let e = 0.02;
            let f = 0.30;
            ((x * (a * x + cc * b) + d * e) / (x * (a * x + b) + d * f)) - e / f
        }
        let exposure_bias = 2.0;
        let white_scale = 1.0 / hable_partial(11.2);
        [
            hable_partial(c[0] * exposure_bias) * white_scale,
            hable_partial(c[1] * exposure_bias) * white_scale,
            hable_partial(c[2] * exposure_bias) * white_scale,
        ]
    }

    fn agx(c: [f32; 3]) -> [f32; 3] {
        // Simplified AgX-like curve
        fn agx_channel(x: f32) -> f32 {
            let x = x.max(0.0);
            let a = x.ln().max(-10.0).min(10.0);
            let mapped = 0.5 + 0.5 * (a * 0.3).tanh();
            mapped
        }
        [agx_channel(c[0]), agx_channel(c[1]), agx_channel(c[2])]
    }

    /// Generate GLSL code for this tone mapping operator.
    pub fn glsl_function(&self) -> &'static str {
        match self {
            Self::Reinhard => {
                r#"vec3 tonemap(vec3 c) { return c / (1.0 + c); }"#
            }
            Self::ReinhardExtended => {
                r#"vec3 tonemap(vec3 c) {
    float wp2 = 16.0;
    return c * (1.0 + c / wp2) / (1.0 + c);
}"#
            }
            Self::AcesFilmic => {
                r#"vec3 tonemap(vec3 x) {
    float a = 2.51; float b = 0.03;
    float c = 2.43; float d = 0.59; float e = 0.14;
    return clamp((x*(a*x+b))/(x*(c*x+d)+e), 0.0, 1.0);
}"#
            }
            Self::Hable => {
                r#"float hable(float x) {
    float A=0.15,B=0.50,C=0.10,D=0.20,E=0.02,F=0.30;
    return ((x*(A*x+C*B)+D*E)/(x*(A*x+B)+D*F))-E/F;
}
vec3 tonemap(vec3 c) {
    float w = 1.0/hable(11.2);
    return vec3(hable(c.x*2.0)*w, hable(c.y*2.0)*w, hable(c.z*2.0)*w);
}"#
            }
            Self::Linear => {
                r#"vec3 tonemap(vec3 c) { return clamp(c, 0.0, 1.0); }"#
            }
            Self::AgX => {
                r#"vec3 tonemap(vec3 c) {
    vec3 a = log(max(c, vec3(0.0001)));
    return 0.5 + 0.5 * tanh(a * 0.3);
}"#
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Depth pre-pass
// ---------------------------------------------------------------------------

/// Depth pre-pass that writes only depth, enabling early-Z rejection
/// in the subsequent geometry pass.
#[derive(Debug)]
pub struct DepthPrePass {
    /// Whether the depth pre-pass is enabled.
    pub enabled: bool,
    /// Shader program handle for the depth-only pass.
    pub shader_handle: u64,
    /// Items to render in this pass (opaque, front-to-back sorted).
    pub items: Vec<u64>,
    /// Whether to use the depth from the G-Buffer or a separate depth buffer.
    pub use_gbuffer_depth: bool,
    /// Statistics: number of items rendered in the pre-pass.
    pub rendered_count: u32,
    /// Time taken for the depth pre-pass (microseconds).
    pub time_us: u64,
    /// Depth function (Less, LessEqual, etc.).
    pub depth_func: DepthFunction,
    /// Whether depth writing is enabled.
    pub depth_write: bool,
    /// Whether to do alpha test in the depth pre-pass (for cutout materials).
    pub alpha_test_in_prepass: bool,
    /// Alpha test threshold.
    pub alpha_threshold: f32,
}

/// Depth comparison function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthFunction {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    Always,
    Never,
}

impl DepthPrePass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            shader_handle: 0,
            items: Vec::new(),
            use_gbuffer_depth: true,
            rendered_count: 0,
            time_us: 0,
            depth_func: DepthFunction::Less,
            depth_write: true,
            alpha_test_in_prepass: false,
            alpha_threshold: 0.5,
        }
    }

    /// Execute the depth pre-pass using items from the render queue.
    pub fn execute(&mut self, queue: &RenderQueue, _gbuffer: &mut GBuffer) {
        let start = std::time::Instant::now();

        self.items.clear();
        self.rendered_count = 0;

        if !self.enabled {
            return;
        }

        let opaque = queue.opaque_items();
        for item in opaque {
            if !item.visible {
                continue;
            }
            // Skip alpha-tested items unless we handle them
            if item.alpha_test && !self.alpha_test_in_prepass {
                continue;
            }
            self.items.push(item.id);
            self.rendered_count += 1;
            // In a real engine: bind depth shader, set uniforms, draw
        }

        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Get GLSL source for the depth-only vertex shader.
    pub fn vertex_shader() -> &'static str {
        r#"#version 330 core
layout(location = 0) in vec3 a_position;
uniform mat4 u_model;
uniform mat4 u_view_projection;
void main() {
    gl_Position = u_view_projection * u_model * vec4(a_position, 1.0);
}
"#
    }

    /// Get GLSL source for the depth-only fragment shader (alpha test variant).
    pub fn fragment_shader_alpha_test() -> &'static str {
        r#"#version 330 core
uniform sampler2D u_albedo_tex;
uniform float u_alpha_threshold;
in vec2 v_texcoord;
void main() {
    float alpha = texture(u_albedo_tex, v_texcoord).a;
    if (alpha < u_alpha_threshold) discard;
}
"#
    }
}

impl Default for DepthPrePass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Geometry pass
// ---------------------------------------------------------------------------

/// The geometry pass fills the G-Buffer with per-pixel data from scene geometry.
#[derive(Debug)]
pub struct GeometryPass {
    /// Whether this pass is enabled.
    pub enabled: bool,
    /// Shader program handle.
    pub shader_handle: u64,
    /// Number of draw calls executed.
    pub draw_call_count: u32,
    /// Number of triangles rendered.
    pub triangle_count: u64,
    /// Time taken (microseconds).
    pub time_us: u64,
    /// Whether depth testing is enabled (should be, using pre-pass depth).
    pub depth_test: bool,
    /// Depth function for geometry pass (Equal if depth pre-pass ran, Less otherwise).
    pub depth_func: DepthFunction,
    /// Whether to write depth (false if depth pre-pass already wrote it).
    pub depth_write: bool,
    /// Whether backface culling is enabled.
    pub backface_cull: bool,
    /// Polygon offset for depth fighting prevention.
    pub polygon_offset: Option<(f32, f32)>,
    /// Whether instanced rendering is used.
    pub use_instancing: bool,
    /// Maximum instances per draw call.
    pub max_instances_per_draw: u32,
}

impl GeometryPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            shader_handle: 0,
            draw_call_count: 0,
            triangle_count: 0,
            time_us: 0,
            depth_test: true,
            depth_func: DepthFunction::Equal,
            depth_write: false,
            backface_cull: true,
            polygon_offset: None,
            use_instancing: true,
            max_instances_per_draw: 1024,
        }
    }

    /// Execute the geometry pass.
    pub fn execute(&mut self, queue: &RenderQueue, gbuffer: &mut GBuffer) {
        let start = std::time::Instant::now();

        self.draw_call_count = 0;
        self.triangle_count = 0;

        if !self.enabled {
            return;
        }

        // Bind G-Buffer as render target
        let _ = gbuffer.bind();

        // Clear G-Buffer
        gbuffer.clear_all();

        let opaque = queue.opaque_items();
        for item in opaque {
            if !item.visible {
                continue;
            }

            self.draw_call_count += 1;
            let tris = if item.index_count > 0 {
                item.index_count / 3
            } else {
                item.vertex_count / 3
            };
            self.triangle_count += tris as u64 * item.instance_count as u64;

            // In a real engine:
            // 1. Bind geometry pass shader
            // 2. Set model/view/projection uniforms from item.transform
            // 3. Bind material textures
            // 4. Draw mesh (or draw instanced if instance_count > 1)
        }

        gbuffer.unbind();
        gbuffer.stats.geometry_draw_calls = self.draw_call_count;

        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Generate the geometry pass vertex shader.
    pub fn vertex_shader() -> &'static str {
        r#"#version 330 core
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_texcoord;
layout(location = 3) in vec3 a_tangent;

uniform mat4 u_model;
uniform mat4 u_view;
uniform mat4 u_projection;
uniform mat3 u_normal_matrix;

out vec3 v_world_pos;
out vec3 v_normal;
out vec2 v_texcoord;
out vec3 v_tangent;

void main() {
    vec4 world_pos = u_model * vec4(a_position, 1.0);
    v_world_pos = world_pos.xyz;
    v_normal = u_normal_matrix * a_normal;
    v_texcoord = a_texcoord;
    v_tangent = u_normal_matrix * a_tangent;
    gl_Position = u_projection * u_view * world_pos;
}
"#
    }

    /// Generate the geometry pass fragment shader.
    pub fn fragment_shader() -> &'static str {
        r#"#version 330 core
in vec3 v_world_pos;
in vec3 v_normal;
in vec2 v_texcoord;
in vec3 v_tangent;

layout(location = 0) out vec4 out_position;
layout(location = 1) out vec2 out_normal;
layout(location = 2) out vec4 out_albedo;
layout(location = 3) out vec4 out_emission;
layout(location = 4) out float out_matid;
layout(location = 5) out float out_roughness;
layout(location = 6) out float out_metallic;

uniform sampler2D u_albedo_map;
uniform sampler2D u_normal_map;
uniform sampler2D u_roughness_map;
uniform sampler2D u_metallic_map;
uniform sampler2D u_emission_map;
uniform vec4 u_albedo_color;
uniform float u_roughness;
uniform float u_metallic;
uniform vec3 u_emission;
uniform float u_material_id;
uniform bool u_has_normal_map;

// Octahedral encoding
vec2 oct_encode(vec3 n) {
    float sum = abs(n.x) + abs(n.y) + abs(n.z);
    vec2 o = n.xy / sum;
    if (n.z < 0.0) {
        o = (1.0 - abs(o.yx)) * vec2(o.x >= 0.0 ? 1.0 : -1.0, o.y >= 0.0 ? 1.0 : -1.0);
    }
    return o;
}

void main() {
    out_position = vec4(v_world_pos, 1.0);

    vec3 N = normalize(v_normal);
    if (u_has_normal_map) {
        vec3 T = normalize(v_tangent);
        vec3 B = cross(N, T);
        mat3 TBN = mat3(T, B, N);
        vec3 tangent_normal = texture(u_normal_map, v_texcoord).xyz * 2.0 - 1.0;
        N = normalize(TBN * tangent_normal);
    }
    out_normal = oct_encode(N);

    out_albedo = texture(u_albedo_map, v_texcoord) * u_albedo_color;
    out_emission = vec4(u_emission + texture(u_emission_map, v_texcoord).rgb, 1.0);
    out_matid = u_material_id / 255.0;
    out_roughness = texture(u_roughness_map, v_texcoord).r * u_roughness;
    out_metallic = texture(u_metallic_map, v_texcoord).r * u_metallic;
}
"#
    }
}

impl Default for GeometryPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Lighting pass
// ---------------------------------------------------------------------------

/// The lighting pass reads from the G-Buffer and evaluates all lights
/// to produce an HDR color result.
#[derive(Debug)]
pub struct LightingPass {
    /// Whether this pass is enabled.
    pub enabled: bool,
    /// Shader program handle.
    pub shader_handle: u64,
    /// All lights in the scene.
    pub lights: Vec<LightType>,
    /// Maximum number of lights to evaluate per pixel.
    pub max_lights_per_pixel: u32,
    /// Whether to use light volumes (render spheres/cones for point/spot lights).
    pub use_light_volumes: bool,
    /// Time taken (microseconds).
    pub time_us: u64,
    /// Number of lights evaluated this frame.
    pub lights_evaluated: u32,
    /// Ambient color (added to all pixels).
    pub ambient_color: [f32; 3],
    /// Ambient intensity.
    pub ambient_intensity: f32,
    /// Whether to apply SSAO (screen-space ambient occlusion).
    pub ssao_enabled: bool,
    /// SSAO radius.
    pub ssao_radius: f32,
    /// SSAO bias.
    pub ssao_bias: f32,
    /// SSAO kernel size.
    pub ssao_kernel_size: u32,
    /// Environment map handle (for IBL).
    pub environment_map: u64,
    /// Whether image-based lighting is enabled.
    pub ibl_enabled: bool,
    /// IBL intensity multiplier.
    pub ibl_intensity: f32,
}

impl LightingPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            shader_handle: 0,
            lights: Vec::new(),
            max_lights_per_pixel: 128,
            use_light_volumes: false,
            time_us: 0,
            lights_evaluated: 0,
            ambient_color: [0.03, 0.03, 0.05],
            ambient_intensity: 1.0,
            ssao_enabled: false,
            ssao_radius: 0.5,
            ssao_bias: 0.025,
            ssao_kernel_size: 64,
            environment_map: 0,
            ibl_enabled: false,
            ibl_intensity: 1.0,
        }
    }

    /// Add a light to the scene.
    pub fn add_light(&mut self, light: LightType) {
        self.lights.push(light);
    }

    /// Remove all lights.
    pub fn clear_lights(&mut self) {
        self.lights.clear();
    }

    /// Execute the lighting pass.
    pub fn execute(
        &mut self,
        gbuffer: &GBuffer,
        hdr_fb: &HdrFramebuffer,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_pos: [f32; 3],
    ) {
        let start = std::time::Instant::now();

        if !self.enabled {
            return;
        }

        // Bind HDR framebuffer as render target
        hdr_fb.bind();

        // Bind G-Buffer textures for reading
        let _bindings = gbuffer.bind_for_reading();

        // In a real engine:
        // 1. Bind lighting shader
        // 2. Set G-Buffer sampler uniforms
        // 3. Set camera uniforms (inverse VP matrix, camera position)
        // 4. Upload light data (UBO or SSBO)
        // 5. Draw fullscreen quad

        self.lights_evaluated = self.lights.len().min(self.max_lights_per_pixel as usize) as u32;

        let _ = view_matrix;
        let _ = projection_matrix;
        let _ = camera_pos;

        hdr_fb.unbind();

        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Evaluate PBR lighting at a single point (for CPU-side validation).
    pub fn evaluate_pbr(
        &self,
        position: [f32; 3],
        normal: [f32; 3],
        albedo: [f32; 3],
        roughness: f32,
        metallic: f32,
        camera_pos: [f32; 3],
    ) -> [f32; 3] {
        let v = super::vec3_normalize(vec3_sub(camera_pos, position));
        let mut total = [
            self.ambient_color[0] * self.ambient_intensity * albedo[0],
            self.ambient_color[1] * self.ambient_intensity * albedo[1],
            self.ambient_color[2] * self.ambient_intensity * albedo[2],
        ];

        for light in &self.lights {
            let (l, attenuation, light_color) = light.evaluate(position);
            if attenuation < 1e-6 {
                continue;
            }

            let n_dot_l = vec3_dot(normal, l).max(0.0);
            if n_dot_l < 1e-6 {
                continue;
            }

            // Simplified Cook-Torrance BRDF
            let h = super::vec3_normalize(super::vec3_add(v, l));
            let n_dot_h = vec3_dot(normal, h).max(0.0);
            let n_dot_v = vec3_dot(normal, v).max(0.001);

            // GGX distribution
            let a = roughness * roughness;
            let a2 = a * a;
            let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
            let d = a2 / (std::f32::consts::PI * denom * denom).max(1e-6);

            // Schlick-GGX geometry
            let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
            let g1_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
            let g1_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
            let g = g1_v * g1_l;

            // Fresnel (Schlick)
            let f0_base = lerpf(0.04, 1.0, metallic);
            let f0 = [
                lerpf(0.04, albedo[0], metallic),
                lerpf(0.04, albedo[1], metallic),
                lerpf(0.04, albedo[2], metallic),
            ];
            let _ = f0_base;
            let v_dot_h = vec3_dot(v, h).max(0.0);
            let fresnel_factor = (1.0 - v_dot_h).powf(5.0);
            let f = [
                f0[0] + (1.0 - f0[0]) * fresnel_factor,
                f0[1] + (1.0 - f0[1]) * fresnel_factor,
                f0[2] + (1.0 - f0[2]) * fresnel_factor,
            ];

            // Specular BRDF: DFG / (4 * NdotV * NdotL)
            let spec_denom = (4.0 * n_dot_v * n_dot_l).max(1e-6);
            let spec = [
                d * g * f[0] / spec_denom,
                d * g * f[1] / spec_denom,
                d * g * f[2] / spec_denom,
            ];

            // Diffuse (Lambertian)
            let kd = [
                (1.0 - f[0]) * (1.0 - metallic),
                (1.0 - f[1]) * (1.0 - metallic),
                (1.0 - f[2]) * (1.0 - metallic),
            ];
            let diffuse = [
                kd[0] * albedo[0] / std::f32::consts::PI,
                kd[1] * albedo[1] / std::f32::consts::PI,
                kd[2] * albedo[2] / std::f32::consts::PI,
            ];

            // Total contribution
            for i in 0..3 {
                total[i] += (diffuse[i] + spec[i]) * light_color[i] * attenuation * n_dot_l;
            }
        }

        total
    }

    /// Generate the lighting pass fragment shader.
    pub fn fragment_shader() -> &'static str {
        r#"#version 330 core
in vec2 v_texcoord;
out vec4 frag_color;

uniform sampler2D g_position;
uniform sampler2D g_normal;
uniform sampler2D g_albedo;
uniform sampler2D g_emission;
uniform sampler2D g_roughness;
uniform sampler2D g_metallic;
uniform sampler2D g_depth;

uniform vec3 u_camera_pos;
uniform mat4 u_inv_view_proj;

struct Light {
    int type;       // 0=dir, 1=point, 2=spot
    vec3 position;
    vec3 direction;
    vec3 color;
    float intensity;
    float range;
    float inner_angle;
    float outer_angle;
};

#define MAX_LIGHTS 128
uniform Light u_lights[MAX_LIGHTS];
uniform int u_light_count;
uniform vec3 u_ambient;

const float PI = 3.14159265359;

vec3 oct_decode(vec2 o) {
    float z = 1.0 - abs(o.x) - abs(o.y);
    vec2 xy = z >= 0.0 ? o : (1.0 - abs(o.yx)) * vec2(o.x >= 0.0 ? 1.0 : -1.0, o.y >= 0.0 ? 1.0 : -1.0);
    return normalize(vec3(xy, z));
}

float ggx_distribution(float NdotH, float roughness) {
    float a2 = roughness * roughness * roughness * roughness;
    float d = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

float geometry_schlick(float NdotV, float NdotL, float roughness) {
    float k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    float g1 = NdotV / (NdotV * (1.0 - k) + k);
    float g2 = NdotL / (NdotL * (1.0 - k) + k);
    return g1 * g2;
}

vec3 fresnel_schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

void main() {
    vec3 pos = texture(g_position, v_texcoord).xyz;
    vec3 N = oct_decode(texture(g_normal, v_texcoord).xy);
    vec4 albedo_alpha = texture(g_albedo, v_texcoord);
    vec3 albedo = albedo_alpha.rgb;
    vec3 emission = texture(g_emission, v_texcoord).rgb;
    float roughness = texture(g_roughness, v_texcoord).r;
    float metallic = texture(g_metallic, v_texcoord).r;

    vec3 V = normalize(u_camera_pos - pos);
    vec3 F0 = mix(vec3(0.04), albedo, metallic);

    vec3 Lo = vec3(0.0);
    for (int i = 0; i < u_light_count && i < MAX_LIGHTS; i++) {
        vec3 L;
        float attenuation;

        if (u_lights[i].type == 0) {
            L = -u_lights[i].direction;
            attenuation = u_lights[i].intensity;
        } else {
            vec3 toLight = u_lights[i].position - pos;
            float dist = length(toLight);
            L = toLight / dist;
            float r = dist / u_lights[i].range;
            attenuation = u_lights[i].intensity * max((1.0 - r*r), 0.0) / (dist*dist + 1.0);

            if (u_lights[i].type == 2) {
                float cosAngle = dot(-L, u_lights[i].direction);
                float spot = clamp((cosAngle - cos(u_lights[i].outer_angle)) /
                    (cos(u_lights[i].inner_angle) - cos(u_lights[i].outer_angle)), 0.0, 1.0);
                attenuation *= spot;
            }
        }

        vec3 H = normalize(V + L);
        float NdotL = max(dot(N, L), 0.0);
        float NdotH = max(dot(N, H), 0.0);
        float NdotV = max(dot(N, V), 0.001);

        float D = ggx_distribution(NdotH, roughness);
        float G = geometry_schlick(NdotV, NdotL, roughness);
        vec3 F = fresnel_schlick(max(dot(H, V), 0.0), F0);

        vec3 spec = D * G * F / max(4.0 * NdotV * NdotL, 0.001);
        vec3 kD = (1.0 - F) * (1.0 - metallic);
        vec3 diffuse = kD * albedo / PI;

        Lo += (diffuse + spec) * u_lights[i].color * attenuation * NdotL;
    }

    vec3 ambient = u_ambient * albedo;
    vec3 color = ambient + Lo + emission;
    frag_color = vec4(color, 1.0);
}
"#
    }
}

impl Default for LightingPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Forward pass (transparent objects)
// ---------------------------------------------------------------------------

/// Forward rendering pass for transparent objects, particles, and
/// alpha-blended glyphs. These are sorted back-to-front and rendered
/// after the deferred lighting pass.
#[derive(Debug)]
pub struct ForwardPass {
    /// Whether this pass is enabled.
    pub enabled: bool,
    /// Shader program handle.
    pub shader_handle: u64,
    /// Draw call count this frame.
    pub draw_call_count: u32,
    /// Triangle count this frame.
    pub triangle_count: u64,
    /// Time taken (microseconds).
    pub time_us: u64,
    /// Whether depth testing is enabled (yes, read-only).
    pub depth_test: bool,
    /// Whether depth writing is enabled (usually no for transparent objects).
    pub depth_write: bool,
    /// Blend mode.
    pub blend_mode: ForwardBlendMode,
    /// Whether to use premultiplied alpha.
    pub premultiplied_alpha: bool,
    /// Whether to render particles in this pass.
    pub render_particles: bool,
    /// Whether to render alpha-blended text/glyphs in this pass.
    pub render_glyphs: bool,
    /// Maximum number of transparent layers for OIT (Order-Independent Transparency).
    /// 0 = disabled (use simple sorted blending).
    pub oit_layers: u32,
}

/// Blend modes for the forward pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardBlendMode {
    /// Standard alpha blending: src*alpha + dst*(1-alpha).
    AlphaBlend,
    /// Additive blending: src + dst.
    Additive,
    /// Premultiplied alpha: src + dst*(1-alpha).
    PremultipliedAlpha,
    /// Multiply: src * dst.
    Multiply,
}

impl ForwardPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            shader_handle: 0,
            draw_call_count: 0,
            triangle_count: 0,
            time_us: 0,
            depth_test: true,
            depth_write: false,
            blend_mode: ForwardBlendMode::AlphaBlend,
            premultiplied_alpha: false,
            render_particles: true,
            render_glyphs: true,
            oit_layers: 0,
        }
    }

    /// Execute the forward pass.
    pub fn execute(
        &mut self,
        queue: &RenderQueue,
        hdr_fb: &HdrFramebuffer,
        _gbuffer: &GBuffer,
        _lights: &[LightType],
        _view: &Mat4,
        _proj: &Mat4,
        _camera_pos: [f32; 3],
    ) {
        let start = std::time::Instant::now();

        self.draw_call_count = 0;
        self.triangle_count = 0;

        if !self.enabled {
            return;
        }

        hdr_fb.bind();

        // Render transparent items back-to-front
        let transparent = queue.transparent_items();
        for item in transparent {
            if !item.visible {
                continue;
            }
            self.draw_call_count += 1;
            let tris = if item.index_count > 0 {
                item.index_count / 3
            } else {
                item.vertex_count / 3
            };
            self.triangle_count += tris as u64 * item.instance_count as u64;
        }

        // Render overlay items
        let overlay = queue.overlay_items();
        for item in overlay {
            if !item.visible {
                continue;
            }
            self.draw_call_count += 1;
        }

        hdr_fb.unbind();

        self.time_us = start.elapsed().as_micros() as u64;
    }
}

impl Default for ForwardPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Post-process pass
// ---------------------------------------------------------------------------

/// Post-processing pass: bloom, tone mapping, and anti-aliasing.
#[derive(Debug)]
pub struct PostProcessPass {
    /// Whether post-processing is enabled.
    pub enabled: bool,
    /// Shader program handle.
    pub shader_handle: u64,
    /// Time taken (microseconds).
    pub time_us: u64,
    /// Bloom settings.
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub bloom_radius: f32,
    pub bloom_mip_count: u32,
    /// Tone mapping operator.
    pub tone_mapping: ToneMappingOperator,
    /// Exposure controller.
    pub exposure: ExposureController,
    /// Gamma correction value.
    pub gamma: f32,
    /// Vignette settings.
    pub vignette_enabled: bool,
    pub vignette_intensity: f32,
    pub vignette_smoothness: f32,
    /// Chromatic aberration.
    pub chromatic_aberration_enabled: bool,
    pub chromatic_aberration_intensity: f32,
    /// Film grain.
    pub film_grain_enabled: bool,
    pub film_grain_intensity: f32,
    /// Dithering (reduces banding in gradients).
    pub dithering_enabled: bool,
    /// Color grading LUT texture handle.
    pub color_lut_handle: u64,
    pub color_lut_enabled: bool,
    /// Saturation adjustment (1.0 = no change).
    pub saturation: f32,
    /// Contrast adjustment (1.0 = no change).
    pub contrast: f32,
    /// Brightness adjustment (0.0 = no change).
    pub brightness: f32,
}

impl PostProcessPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            shader_handle: 0,
            time_us: 0,
            bloom_enabled: true,
            bloom_intensity: 0.5,
            bloom_threshold: 1.0,
            bloom_radius: 5.0,
            bloom_mip_count: 5,
            tone_mapping: ToneMappingOperator::AcesFilmic,
            exposure: ExposureController::new(),
            gamma: 2.2,
            vignette_enabled: false,
            vignette_intensity: 0.3,
            vignette_smoothness: 2.0,
            chromatic_aberration_enabled: false,
            chromatic_aberration_intensity: 0.005,
            film_grain_enabled: false,
            film_grain_intensity: 0.05,
            dithering_enabled: true,
            color_lut_handle: 0,
            color_lut_enabled: false,
            saturation: 1.0,
            contrast: 1.0,
            brightness: 0.0,
        }
    }

    /// Execute the post-processing pass.
    pub fn execute(&mut self, hdr_fb: &HdrFramebuffer, _viewport: &Viewport, dt: f32) {
        let start = std::time::Instant::now();

        if !self.enabled {
            return;
        }

        // Update exposure
        self.exposure.update(dt);

        // In a real engine:
        // 1. Extract bright pixels for bloom (threshold)
        // 2. Downsample bloom chain
        // 3. Upsample + blur bloom chain
        // 4. Composite bloom + HDR color
        // 5. Tone map
        // 6. Gamma correct
        // 7. Apply vignette, chromatic aberration, film grain
        // 8. Apply color grading LUT
        // 9. Dither output

        let _ = hdr_fb;

        self.time_us = start.elapsed().as_micros() as u64;
    }

    /// Apply bloom extraction (returns which pixels are bright enough).
    pub fn bloom_extract(&self, color: [f32; 3]) -> [f32; 3] {
        let luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
        if luminance > self.bloom_threshold {
            let excess = luminance - self.bloom_threshold;
            let factor = excess / luminance.max(1e-6);
            [
                color[0] * factor * self.bloom_intensity,
                color[1] * factor * self.bloom_intensity,
                color[2] * factor * self.bloom_intensity,
            ]
        } else {
            [0.0, 0.0, 0.0]
        }
    }

    /// Apply vignette effect.
    pub fn apply_vignette(&self, color: [f32; 3], uv: [f32; 2]) -> [f32; 3] {
        if !self.vignette_enabled {
            return color;
        }
        let center = [uv[0] - 0.5, uv[1] - 0.5];
        let dist = (center[0] * center[0] + center[1] * center[1]).sqrt() * 1.414;
        let vignette = 1.0 - self.vignette_intensity * dist.powf(self.vignette_smoothness);
        let v = vignette.max(0.0);
        [color[0] * v, color[1] * v, color[2] * v]
    }

    /// Apply saturation/contrast/brightness adjustments.
    pub fn color_adjust(&self, color: [f32; 3]) -> [f32; 3] {
        // Brightness
        let c = [
            color[0] + self.brightness,
            color[1] + self.brightness,
            color[2] + self.brightness,
        ];

        // Contrast
        let c = [
            (c[0] - 0.5) * self.contrast + 0.5,
            (c[1] - 0.5) * self.contrast + 0.5,
            (c[2] - 0.5) * self.contrast + 0.5,
        ];

        // Saturation
        let lum = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        [
            lerpf(lum, c[0], self.saturation),
            lerpf(lum, c[1], self.saturation),
            lerpf(lum, c[2], self.saturation),
        ]
    }

    /// Generate the post-process fragment shader.
    pub fn fragment_shader(&self) -> String {
        let mut s = String::from(r#"#version 330 core
in vec2 v_texcoord;
out vec4 frag_color;

uniform sampler2D u_hdr_color;
uniform sampler2D u_bloom;
uniform float u_exposure;
uniform float u_gamma;
uniform float u_bloom_intensity;
uniform float u_time;
"#);

        s.push_str(self.tone_mapping.glsl_function());
        s.push('\n');

        s.push_str(r#"
void main() {
    vec3 hdr = texture(u_hdr_color, v_texcoord).rgb;
    vec3 bloom = texture(u_bloom, v_texcoord).rgb;
    hdr += bloom * u_bloom_intensity;
    hdr *= u_exposure;
    vec3 mapped = tonemap(hdr);
    mapped = pow(mapped, vec3(1.0 / u_gamma));
    frag_color = vec4(mapped, 1.0);
}
"#);

        s
    }
}

impl Default for PostProcessPass {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Full deferred pipeline
// ---------------------------------------------------------------------------

/// Frame statistics for the entire deferred pipeline.
#[derive(Debug, Clone, Default)]
pub struct DeferredFrameStats {
    /// Total frame time (microseconds).
    pub total_time_us: u64,
    /// Time per pass.
    pub depth_prepass_us: u64,
    pub geometry_pass_us: u64,
    pub lighting_pass_us: u64,
    pub forward_pass_us: u64,
    pub postprocess_pass_us: u64,
    pub aa_pass_us: u64,
    /// Draw call counts.
    pub total_draw_calls: u32,
    pub opaque_draw_calls: u32,
    pub transparent_draw_calls: u32,
    /// Triangle count.
    pub total_triangles: u64,
    /// Items submitted / visible / culled.
    pub items_submitted: u32,
    pub items_visible: u32,
    pub items_culled: u32,
    /// G-Buffer memory usage.
    pub gbuffer_memory_mb: f32,
    /// Current exposure.
    pub exposure: f32,
    /// Current frame number.
    pub frame_number: u64,
}

/// The complete deferred rendering pipeline, orchestrating all passes.
#[derive(Debug)]
pub struct DeferredPipeline {
    /// The G-Buffer.
    pub gbuffer: GBuffer,
    /// HDR framebuffer.
    pub hdr_framebuffer: HdrFramebuffer,
    /// Depth pre-pass.
    pub depth_prepass: DepthPrePass,
    /// Geometry pass.
    pub geometry_pass: GeometryPass,
    /// Lighting pass.
    pub lighting_pass: LightingPass,
    /// Forward pass.
    pub forward_pass: ForwardPass,
    /// Post-processing pass.
    pub postprocess_pass: PostProcessPass,
    /// Render queue.
    pub render_queue: RenderQueue,
    /// Current viewport.
    pub viewport: Viewport,
    /// View matrix.
    pub view_matrix: Mat4,
    /// Projection matrix.
    pub projection_matrix: Mat4,
    /// Camera position.
    pub camera_position: [f32; 3],
    /// Frame statistics.
    pub frame_stats: DeferredFrameStats,
    /// Whether the pipeline has been initialized.
    pub initialized: bool,
    /// Frame counter.
    pub frame_number: u64,
    /// Delta time for the current frame.
    pub dt: f32,
}

impl DeferredPipeline {
    /// Create a new deferred pipeline with the given viewport dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let viewport = Viewport::new(width, height);
        Self {
            gbuffer: GBuffer::new(viewport),
            hdr_framebuffer: HdrFramebuffer::new(width, height),
            depth_prepass: DepthPrePass::new(),
            geometry_pass: GeometryPass::new(),
            lighting_pass: LightingPass::new(),
            forward_pass: ForwardPass::new(),
            postprocess_pass: PostProcessPass::new(),
            render_queue: RenderQueue::new(),
            viewport,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            camera_position: [0.0; 3],
            frame_stats: DeferredFrameStats::default(),
            initialized: false,
            frame_number: 0,
            dt: 0.016,
        }
    }

    /// Initialize all GPU resources.
    pub fn initialize(&mut self) -> Result<(), String> {
        self.gbuffer.create().map_err(|e| e.to_string())?;
        self.hdr_framebuffer.create()?;
        self.initialized = true;
        Ok(())
    }

    /// Shut down and release all resources.
    pub fn shutdown(&mut self) {
        self.gbuffer.destroy();
        self.hdr_framebuffer.destroy();
        self.initialized = false;
    }

    /// Resize the pipeline to new viewport dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport = Viewport::new(width, height);
        let _ = self.gbuffer.resize(width, height);
        self.hdr_framebuffer.resize(width, height);
    }

    /// Set the camera for this frame.
    pub fn set_camera(
        &mut self,
        position: [f32; 3],
        view: Mat4,
        projection: Mat4,
        frustum_planes: [[f32; 4]; 6],
    ) {
        self.camera_position = position;
        self.view_matrix = view;
        self.projection_matrix = projection;
        self.render_queue.set_camera(position, frustum_planes);
    }

    /// Submit a render item.
    pub fn submit(&mut self, item: RenderItem) {
        self.render_queue.submit(item);
    }

    /// Execute the full deferred rendering pipeline for one frame.
    pub fn execute_frame(&mut self, dt: f32) {
        let frame_start = std::time::Instant::now();
        self.dt = dt;
        self.frame_number += 1;

        // Sort the render queue
        self.render_queue.sort();

        // 1. Depth pre-pass
        self.depth_prepass.execute(&self.render_queue, &mut self.gbuffer);

        // 2. Geometry pass
        self.geometry_pass.execute(&self.render_queue, &mut self.gbuffer);

        // 3. Lighting pass
        self.lighting_pass.execute(
            &self.gbuffer,
            &self.hdr_framebuffer,
            &self.view_matrix,
            &self.projection_matrix,
            self.camera_position,
        );

        // 4. Forward pass
        let lights_clone: Vec<LightType> = self.lighting_pass.lights.clone();
        self.forward_pass.execute(
            &self.render_queue,
            &self.hdr_framebuffer,
            &self.gbuffer,
            &lights_clone,
            &self.view_matrix,
            &self.projection_matrix,
            self.camera_position,
        );

        // 5. Post-processing
        self.postprocess_pass.execute(&self.hdr_framebuffer, &self.viewport, dt);

        // Collect stats
        self.frame_stats = DeferredFrameStats {
            total_time_us: frame_start.elapsed().as_micros() as u64,
            depth_prepass_us: self.depth_prepass.time_us,
            geometry_pass_us: self.geometry_pass.time_us,
            lighting_pass_us: self.lighting_pass.time_us,
            forward_pass_us: self.forward_pass.time_us,
            postprocess_pass_us: self.postprocess_pass.time_us,
            aa_pass_us: 0,
            total_draw_calls: self.geometry_pass.draw_call_count + self.forward_pass.draw_call_count,
            opaque_draw_calls: self.geometry_pass.draw_call_count,
            transparent_draw_calls: self.forward_pass.draw_call_count,
            total_triangles: self.geometry_pass.triangle_count + self.forward_pass.triangle_count,
            items_submitted: self.render_queue.total_submitted,
            items_visible: self.render_queue.total_visible,
            items_culled: self.render_queue.total_culled,
            gbuffer_memory_mb: self.gbuffer.stats().total_memory_bytes as f32 / (1024.0 * 1024.0),
            exposure: self.postprocess_pass.exposure.exposure,
            frame_number: self.frame_number,
        };

        // Clear the queue for next frame
        self.render_queue.clear();
    }

    /// Get a summary string of the current frame stats.
    pub fn stats_summary(&self) -> String {
        let s = &self.frame_stats;
        format!(
            "Frame {} | {:.1}ms total | Draws: {} | Tris: {} | Visible: {}/{} | Exposure: {:.2} | GBuf: {:.1}MB",
            s.frame_number,
            s.total_time_us as f64 / 1000.0,
            s.total_draw_calls,
            s.total_triangles,
            s.items_visible,
            s.items_submitted,
            s.exposure,
            s.gbuffer_memory_mb,
        )
    }
}

impl Default for DeferredPipeline {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_queue_sorting() {
        let mut queue = RenderQueue::new();
        // Set all frustum planes to accept everything
        queue.set_camera(
            [0.0, 0.0, 0.0],
            [[0.0, 0.0, 1.0, 1000.0]; 6],
        );

        let item1 = RenderItem::new(0, 1, 0).with_bounds([0.0, 0.0, 10.0], 1.0);
        let item2 = RenderItem::new(0, 2, 0).with_bounds([0.0, 0.0, 5.0], 1.0);
        let item3 = RenderItem::new(0, 3, 0).with_bounds([0.0, 0.0, 20.0], 1.0);

        queue.submit(item1);
        queue.submit(item2);
        queue.submit(item3);
        queue.sort();

        let opaque = queue.opaque_items();
        assert_eq!(opaque.len(), 3);
        // Front-to-back: closest first
        assert!(opaque[0].camera_distance <= opaque[1].camera_distance);
        assert!(opaque[1].camera_distance <= opaque[2].camera_distance);
    }

    #[test]
    fn test_render_queue_transparent_sorting() {
        let mut queue = RenderQueue::new();
        queue.set_camera(
            [0.0, 0.0, 0.0],
            [[0.0, 0.0, 1.0, 1000.0]; 6],
        );

        let item1 = RenderItem::new(0, 1, 0)
            .with_bucket(RenderBucket::Transparent)
            .with_bounds([0.0, 0.0, 10.0], 1.0);
        let item2 = RenderItem::new(0, 2, 0)
            .with_bucket(RenderBucket::Transparent)
            .with_bounds([0.0, 0.0, 5.0], 1.0);

        queue.submit(item1);
        queue.submit(item2);
        queue.sort();

        let transparent = queue.transparent_items();
        assert_eq!(transparent.len(), 2);
        // Back-to-front: farthest first
        assert!(transparent[0].camera_distance >= transparent[1].camera_distance);
    }

    #[test]
    fn test_light_evaluation() {
        let light = LightType::Directional {
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            cast_shadows: false,
        };
        let (dir, att, _color) = light.evaluate([0.0, 0.0, 0.0]);
        assert!((dir[1] - 1.0).abs() < 0.001); // reversed direction
        assert!((att - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_point_light_attenuation() {
        let light = LightType::Point {
            position: [0.0, 5.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 10.0,
            range: 20.0,
            cast_shadows: false,
        };
        let (_, att_near, _) = light.evaluate([0.0, 4.0, 0.0]);
        let (_, att_far, _) = light.evaluate([0.0, -10.0, 0.0]);
        assert!(att_near > att_far, "Near attenuation should be greater");
    }

    #[test]
    fn test_tone_mapping() {
        let color = [2.0, 1.0, 0.5];
        let reinhard = ToneMappingOperator::Reinhard.apply(color, 1.0);
        for c in &reinhard {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
        let aces = ToneMappingOperator::AcesFilmic.apply(color, 1.0);
        for c in &aces {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
    }

    #[test]
    fn test_exposure_controller() {
        let mut ec = ExposureController::new();
        ec.mode = ExposureMode::AverageLuminance;
        ec.feed_luminance(0.5);
        ec.update(0.016);
        assert!(ec.exposure > 0.0);
    }

    #[test]
    fn test_pipeline_creation() {
        let mut pipeline = DeferredPipeline::new(1920, 1080);
        assert!(!pipeline.initialized);
        pipeline.initialize().unwrap();
        assert!(pipeline.initialized);
    }

    #[test]
    fn test_pipeline_frame() {
        let mut pipeline = DeferredPipeline::new(800, 600);
        pipeline.initialize().unwrap();

        pipeline.lighting_pass.add_light(LightType::Directional {
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            cast_shadows: false,
        });

        pipeline.set_camera(
            [0.0, 5.0, 10.0],
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [[0.0, 0.0, 1.0, 1000.0]; 6],
        );

        let item = RenderItem::new(0, 1, 0)
            .with_bounds([0.0, 0.0, 0.0], 5.0);
        pipeline.submit(item);

        pipeline.execute_frame(0.016);
        assert_eq!(pipeline.frame_stats.frame_number, 1);
    }

    #[test]
    fn test_hdr_framebuffer() {
        let mut fb = HdrFramebuffer::new(1920, 1080);
        fb.create().unwrap();
        assert!(fb.allocated);
        assert!(fb.memory_bytes() > 0);
        fb.resize(2560, 1440);
        assert_eq!(fb.width, 2560);
    }

    #[test]
    fn test_bloom_extract() {
        let pp = PostProcessPass::new();
        let bright = pp.bloom_extract([2.0, 2.0, 2.0]);
        assert!(bright[0] > 0.0);
        let dark = pp.bloom_extract([0.1, 0.1, 0.1]);
        assert!((dark[0] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_pbr_lighting() {
        let mut lp = LightingPass::new();
        lp.add_light(LightType::Directional {
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 2.0,
            cast_shadows: false,
        });

        let result = lp.evaluate_pbr(
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.8, 0.2, 0.2],
            0.5,
            0.0,
            [0.0, 5.0, 5.0],
        );

        assert!(result[0] > 0.0);
        assert!(result[1] > 0.0);
        assert!(result[2] > 0.0);
    }
}
