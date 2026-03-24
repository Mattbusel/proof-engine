//! Volumetric lighting effects for Proof Engine.
//!
//! Provides volumetric light shafts (god rays), volumetric fog with ray marching,
//! tiled light culling for deferred rendering, and 3D frustum-based clustered light
//! assignment for clustered forward rendering.

use super::lights::{Vec3, Color, Mat4, LightId, Light};
use std::collections::HashMap;
use std::f32::consts::PI;

// ── Volumetric Light Shafts ─────────────────────────────────────────────────

/// God rays via radial blur from a light's screen position.
#[derive(Debug, Clone)]
pub struct VolumetricLightShafts {
    /// Number of samples along each ray.
    pub sample_count: u32,
    /// Density of the light shaft effect (0..1).
    pub density: f32,
    /// Weight for combining scattered light.
    pub weight: f32,
    /// Decay factor per sample along the ray.
    pub decay: f32,
    /// Exposure multiplier.
    pub exposure: f32,
    /// Light screen-space position (0..1 in each axis). Updated per frame.
    pub light_screen_pos: (f32, f32),
    /// Light color for the shafts.
    pub light_color: Color,
    /// Whether the effect is enabled.
    pub enabled: bool,
}

impl Default for VolumetricLightShafts {
    fn default() -> Self {
        Self {
            sample_count: 64,
            density: 1.0,
            weight: 0.01,
            decay: 0.97,
            exposure: 1.0,
            light_screen_pos: (0.5, 0.5),
            light_color: Color::WHITE,
            enabled: true,
        }
    }
}

impl VolumetricLightShafts {
    pub fn new(sample_count: u32) -> Self {
        Self {
            sample_count,
            ..Default::default()
        }
    }

    /// Update the light's screen position from a world position and view-projection matrix.
    pub fn update_light_position(&mut self, light_world_pos: Vec3, view_projection: &Mat4) {
        let clip = view_projection.transform_point(light_world_pos);
        self.light_screen_pos = (clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5);
    }

    /// Check if the light is on screen.
    pub fn is_light_visible(&self) -> bool {
        let (sx, sy) = self.light_screen_pos;
        sx >= -0.2 && sx <= 1.2 && sy >= -0.2 && sy <= 1.2
    }

    /// Apply the radial blur effect to a frame buffer.
    /// The input is a buffer of colors (width x height) and an occlusion buffer (same size).
    /// Returns the light shaft contribution buffer.
    pub fn compute(
        &self,
        width: u32,
        height: u32,
        occlusion_buffer: &[f32],
    ) -> Vec<Color> {
        let w = width as usize;
        let h = height as usize;
        let mut result = vec![Color::BLACK; w * h];

        if !self.enabled || !self.is_light_visible() {
            return result;
        }

        let (lx, ly) = self.light_screen_pos;

        for y in 0..h {
            for x in 0..w {
                let pixel_x = x as f32 / w as f32;
                let pixel_y = y as f32 / h as f32;

                // Direction from pixel to light in screen space
                let dx = lx - pixel_x;
                let dy = ly - pixel_y;

                let delta_x = dx * self.density / self.sample_count as f32;
                let delta_y = dy * self.density / self.sample_count as f32;

                let mut sample_x = pixel_x;
                let mut sample_y = pixel_y;
                let mut illumination_decay = 1.0f32;
                let mut accumulated = Color::BLACK;

                for _ in 0..self.sample_count {
                    sample_x += delta_x;
                    sample_y += delta_y;

                    let sx = (sample_x * w as f32) as usize;
                    let sy = (sample_y * h as f32) as usize;

                    if sx < w && sy < h {
                        let occlusion = occlusion_buffer[sy * w + sx];
                        // Only accumulate light where not occluded
                        let sample_value = (1.0 - occlusion) * illumination_decay * self.weight;
                        accumulated = Color::new(
                            accumulated.r + self.light_color.r * sample_value,
                            accumulated.g + self.light_color.g * sample_value,
                            accumulated.b + self.light_color.b * sample_value,
                        );
                    }

                    illumination_decay *= self.decay;
                }

                result[y * w + x] = Color::new(
                    accumulated.r * self.exposure,
                    accumulated.g * self.exposure,
                    accumulated.b * self.exposure,
                );
            }
        }

        result
    }

    /// Apply a cheap half-resolution version for performance.
    pub fn compute_half_res(
        &self,
        width: u32,
        height: u32,
        occlusion_buffer: &[f32],
    ) -> Vec<Color> {
        let half_w = width / 2;
        let half_h = height / 2;

        // Downsample occlusion buffer
        let half_size = (half_w as usize) * (half_h as usize);
        let mut half_occlusion = vec![0.0f32; half_size];
        for y in 0..half_h as usize {
            for x in 0..half_w as usize {
                let sx = x * 2;
                let sy = y * 2;
                let w_full = width as usize;
                if sx + 1 < width as usize && sy + 1 < height as usize {
                    let avg = (occlusion_buffer[sy * w_full + sx]
                        + occlusion_buffer[sy * w_full + sx + 1]
                        + occlusion_buffer[(sy + 1) * w_full + sx]
                        + occlusion_buffer[(sy + 1) * w_full + sx + 1])
                        * 0.25;
                    half_occlusion[y * half_w as usize + x] = avg;
                }
            }
        }

        self.compute(half_w, half_h, &half_occlusion)
    }
}

// ── Volumetric Fog ──────────────────────────────────────────────────────────

/// Describes a fog density field.
#[derive(Debug, Clone)]
pub enum FogDensityField {
    /// Uniform density everywhere.
    Uniform(f32),
    /// Height-based exponential fog.
    HeightExponential {
        base_density: f32,
        falloff: f32,
        base_height: f32,
    },
    /// Spherical fog volume.
    Sphere {
        center: Vec3,
        radius: f32,
        density: f32,
    },
    /// Box-shaped fog volume.
    Box {
        min: Vec3,
        max: Vec3,
        density: f32,
    },
    /// Layered: multiple fog sources combined.
    Layered(Vec<FogDensityField>),
}

impl Default for FogDensityField {
    fn default() -> Self {
        Self::HeightExponential {
            base_density: 0.02,
            falloff: 0.5,
            base_height: 0.0,
        }
    }
}

impl FogDensityField {
    /// Sample density at a world position.
    pub fn sample(&self, pos: Vec3) -> f32 {
        match self {
            Self::Uniform(d) => *d,
            Self::HeightExponential { base_density, falloff, base_height } => {
                let height_diff = pos.y - base_height;
                base_density * (-falloff * height_diff.max(0.0)).exp()
            }
            Self::Sphere { center, radius, density } => {
                let dist = center.distance(pos);
                if dist < *radius {
                    let t = dist / radius;
                    density * (1.0 - t * t).max(0.0)
                } else {
                    0.0
                }
            }
            Self::Box { min, max, density } => {
                if pos.x >= min.x && pos.x <= max.x
                    && pos.y >= min.y && pos.y <= max.y
                    && pos.z >= min.z && pos.z <= max.z
                {
                    *density
                } else {
                    0.0
                }
            }
            Self::Layered(layers) => {
                layers.iter().map(|l| l.sample(pos)).sum()
            }
        }
    }
}

/// Volumetric fog with ray marching, scattering, absorption, and the
/// Henyey-Greenstein phase function.
#[derive(Debug, Clone)]
pub struct VolumetricFog {
    /// Fog density field.
    pub density_field: FogDensityField,
    /// Scattering coefficient (how much light is scattered per unit distance).
    pub scattering: f32,
    /// Absorption coefficient (how much light is absorbed per unit distance).
    pub absorption: f32,
    /// Fog color.
    pub fog_color: Color,
    /// Henyey-Greenstein asymmetry parameter (-1 = back scatter, 0 = isotropic, 1 = forward).
    pub hg_asymmetry: f32,
    /// Number of ray marching steps.
    pub step_count: u32,
    /// Maximum ray marching distance.
    pub max_distance: f32,
    /// Whether fog is enabled.
    pub enabled: bool,
    /// Ambient fog contribution (light scattered from the environment).
    pub ambient_contribution: f32,
    /// Temporal reprojection jitter offset (for temporal anti-aliasing of fog).
    pub jitter_offset: f32,
}

impl Default for VolumetricFog {
    fn default() -> Self {
        Self {
            density_field: FogDensityField::default(),
            scattering: 0.05,
            absorption: 0.01,
            fog_color: Color::new(0.7, 0.75, 0.85),
            hg_asymmetry: 0.3,
            step_count: 64,
            max_distance: 100.0,
            enabled: true,
            ambient_contribution: 0.15,
            jitter_offset: 0.0,
        }
    }
}

impl VolumetricFog {
    pub fn new(density_field: FogDensityField) -> Self {
        Self {
            density_field,
            ..Default::default()
        }
    }

    /// Henyey-Greenstein phase function.
    /// Evaluates the probability of light scattering at angle `cos_theta`.
    pub fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
        let g2 = g * g;
        let denom = 1.0 + g2 - 2.0 * g * cos_theta;
        if denom <= 0.0 {
            return 1.0 / (4.0 * PI);
        }
        (1.0 - g2) / (4.0 * PI * denom.powf(1.5))
    }

    /// Compute the extinction coefficient at a point.
    fn extinction_at(&self, pos: Vec3) -> f32 {
        let density = self.density_field.sample(pos);
        (self.scattering + self.absorption) * density
    }

    /// Ray march through the fog volume from a camera ray.
    /// Returns (accumulated fog color, transmittance).
    pub fn ray_march(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        max_dist: f32,
        light_dir: Vec3,
        light_color: Color,
        light_intensity: f32,
    ) -> (Color, f32) {
        if !self.enabled {
            return (Color::BLACK, 1.0);
        }

        let effective_max = max_dist.min(self.max_distance);
        let step_size = effective_max / self.step_count as f32;
        let dir = ray_dir.normalize();

        let mut accumulated_color = Color::BLACK;
        let mut transmittance = 1.0f32;

        let cos_theta = dir.dot((-light_dir).normalize());
        let phase = Self::henyey_greenstein(cos_theta, self.hg_asymmetry);

        for i in 0..self.step_count {
            let t = (i as f32 + 0.5 + self.jitter_offset) * step_size;
            let sample_pos = ray_origin + dir * t;

            let density = self.density_field.sample(sample_pos);
            if density <= 0.0 {
                continue;
            }

            let extinction = (self.scattering + self.absorption) * density;
            let sample_transmittance = (-extinction * step_size).exp();

            // In-scattered light from the main light source
            let in_scattered = Color::new(
                light_color.r * light_intensity * self.scattering * density * phase,
                light_color.g * light_intensity * self.scattering * density * phase,
                light_color.b * light_intensity * self.scattering * density * phase,
            );

            // Ambient scattering
            let ambient = Color::new(
                self.fog_color.r * self.ambient_contribution * self.scattering * density,
                self.fog_color.g * self.ambient_contribution * self.scattering * density,
                self.fog_color.b * self.ambient_contribution * self.scattering * density,
            );

            // Integrate: add in-scattered light weighted by current transmittance
            let luminance_step = Color::new(
                (in_scattered.r + ambient.r) * transmittance * step_size,
                (in_scattered.g + ambient.g) * transmittance * step_size,
                (in_scattered.b + ambient.b) * transmittance * step_size,
            );

            accumulated_color = Color::new(
                accumulated_color.r + luminance_step.r,
                accumulated_color.g + luminance_step.g,
                accumulated_color.b + luminance_step.b,
            );

            transmittance *= sample_transmittance;

            // Early out if nearly fully opaque
            if transmittance < 0.001 {
                break;
            }
        }

        (accumulated_color, transmittance)
    }

    /// Ray march with multiple light contributions.
    pub fn ray_march_multi_light(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        max_dist: f32,
        lights: &[(Vec3, Color, f32)], // (direction, color, intensity)
    ) -> (Color, f32) {
        if !self.enabled {
            return (Color::BLACK, 1.0);
        }

        let effective_max = max_dist.min(self.max_distance);
        let step_size = effective_max / self.step_count as f32;
        let dir = ray_dir.normalize();

        let mut accumulated_color = Color::BLACK;
        let mut transmittance = 1.0f32;

        // Precompute phase values for each light
        let phase_values: Vec<f32> = lights.iter().map(|(light_dir, _, _)| {
            let cos_theta = dir.dot((-*light_dir).normalize());
            Self::henyey_greenstein(cos_theta, self.hg_asymmetry)
        }).collect();

        for i in 0..self.step_count {
            let t = (i as f32 + 0.5 + self.jitter_offset) * step_size;
            let sample_pos = ray_origin + dir * t;

            let density = self.density_field.sample(sample_pos);
            if density <= 0.0 {
                continue;
            }

            let extinction = (self.scattering + self.absorption) * density;
            let sample_transmittance = (-extinction * step_size).exp();

            let mut in_scattered = Color::BLACK;
            for (j, (_, light_color, intensity)) in lights.iter().enumerate() {
                let phase = phase_values[j];
                in_scattered = Color::new(
                    in_scattered.r + light_color.r * intensity * self.scattering * density * phase,
                    in_scattered.g + light_color.g * intensity * self.scattering * density * phase,
                    in_scattered.b + light_color.b * intensity * self.scattering * density * phase,
                );
            }

            let ambient = Color::new(
                self.fog_color.r * self.ambient_contribution * self.scattering * density,
                self.fog_color.g * self.ambient_contribution * self.scattering * density,
                self.fog_color.b * self.ambient_contribution * self.scattering * density,
            );

            let luminance_step = Color::new(
                (in_scattered.r + ambient.r) * transmittance * step_size,
                (in_scattered.g + ambient.g) * transmittance * step_size,
                (in_scattered.b + ambient.b) * transmittance * step_size,
            );

            accumulated_color = Color::new(
                accumulated_color.r + luminance_step.r,
                accumulated_color.g + luminance_step.g,
                accumulated_color.b + luminance_step.b,
            );

            transmittance *= sample_transmittance;

            if transmittance < 0.001 {
                break;
            }
        }

        (accumulated_color, transmittance)
    }

    /// Apply fog to a final pixel color given scene depth.
    pub fn apply_to_pixel(
        &self,
        scene_color: Color,
        fog_color: Color,
        transmittance: f32,
    ) -> Color {
        Color::new(
            scene_color.r * transmittance + fog_color.r,
            scene_color.g * transmittance + fog_color.g,
            scene_color.b * transmittance + fog_color.b,
        )
    }

    /// Compute the optical depth along a ray (integral of extinction).
    pub fn optical_depth(&self, origin: Vec3, direction: Vec3, distance: f32) -> f32 {
        let steps = (self.step_count / 2).max(4);
        let step_size = distance / steps as f32;
        let dir = direction.normalize();
        let mut depth = 0.0f32;

        for i in 0..steps {
            let t = (i as f32 + 0.5) * step_size;
            let pos = origin + dir * t;
            depth += self.extinction_at(pos) * step_size;
        }

        depth
    }

    /// Compute transmittance along a ray.
    pub fn transmittance(&self, origin: Vec3, direction: Vec3, distance: f32) -> f32 {
        (-self.optical_depth(origin, direction, distance)).exp()
    }
}

// ── Tiled Light Culling ─────────────────────────────────────────────────────

/// Screen-space tile for tiled deferred rendering.
#[derive(Debug, Clone)]
pub struct ScreenTile {
    /// Tile position in tiles (not pixels).
    pub tile_x: u32,
    pub tile_y: u32,
    /// Lights that affect this tile.
    pub light_ids: Vec<LightId>,
    /// Min and max depth in this tile (for tighter culling).
    pub min_depth: f32,
    pub max_depth: f32,
}

impl ScreenTile {
    pub fn new(tile_x: u32, tile_y: u32) -> Self {
        Self {
            tile_x,
            tile_y,
            light_ids: Vec::new(),
            min_depth: 1.0,
            max_depth: 0.0,
        }
    }

    /// Update the depth range from a depth buffer region.
    pub fn update_depth_range(&mut self, depths: &[f32]) {
        self.min_depth = 1.0;
        self.max_depth = 0.0;
        for &d in depths {
            if d < 1.0 {
                self.min_depth = self.min_depth.min(d);
                self.max_depth = self.max_depth.max(d);
            }
        }
    }

    /// Get the light count for this tile.
    pub fn light_count(&self) -> usize {
        self.light_ids.len()
    }
}

/// Divides the screen into tiles and assigns lights for deferred rendering.
#[derive(Debug, Clone)]
pub struct TiledLightCulling {
    /// Tile size in pixels.
    pub tile_size: u32,
    /// Screen width.
    pub screen_width: u32,
    /// Screen height.
    pub screen_height: u32,
    /// Number of tiles in X.
    pub tiles_x: u32,
    /// Number of tiles in Y.
    pub tiles_y: u32,
    /// All tiles.
    pub tiles: Vec<ScreenTile>,
    /// View-projection matrix for the current frame.
    pub view_projection: Mat4,
    /// Inverse projection for reconstructing view-space positions.
    pub inv_projection: Mat4,
    /// Near plane.
    pub near: f32,
    /// Far plane.
    pub far: f32,
}

impl TiledLightCulling {
    pub fn new(screen_width: u32, screen_height: u32, tile_size: u32) -> Self {
        let tiles_x = (screen_width + tile_size - 1) / tile_size;
        let tiles_y = (screen_height + tile_size - 1) / tile_size;
        let mut tiles = Vec::with_capacity((tiles_x * tiles_y) as usize);
        for y in 0..tiles_y {
            for x in 0..tiles_x {
                tiles.push(ScreenTile::new(x, y));
            }
        }
        Self {
            tile_size,
            screen_width,
            screen_height,
            tiles_x,
            tiles_y,
            tiles,
            view_projection: Mat4::IDENTITY,
            inv_projection: Mat4::IDENTITY,
            near: 0.1,
            far: 1000.0,
        }
    }

    /// Resize the tiling when the screen resolution changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.tiles_x = (width + self.tile_size - 1) / self.tile_size;
        self.tiles_y = (height + self.tile_size - 1) / self.tile_size;
        self.tiles.clear();
        for y in 0..self.tiles_y {
            for x in 0..self.tiles_x {
                self.tiles.push(ScreenTile::new(x, y));
            }
        }
    }

    /// Update depth ranges for all tiles from a full-screen depth buffer.
    pub fn update_depth_ranges(&mut self, depth_buffer: &[f32]) {
        let w = self.screen_width as usize;

        for tile in &mut self.tiles {
            let tx = tile.tile_x as usize;
            let ty = tile.tile_y as usize;
            let ts = self.tile_size as usize;

            let x_start = tx * ts;
            let y_start = ty * ts;
            let x_end = (x_start + ts).min(self.screen_width as usize);
            let y_end = (y_start + ts).min(self.screen_height as usize);

            tile.min_depth = 1.0;
            tile.max_depth = 0.0;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let d = depth_buffer[y * w + x];
                    if d < 1.0 {
                        tile.min_depth = tile.min_depth.min(d);
                        tile.max_depth = tile.max_depth.max(d);
                    }
                }
            }
        }
    }

    /// Cull lights against all tiles.
    pub fn cull_lights(&mut self, lights: &[(LightId, &Light)]) {
        // Clear previous assignments
        for tile in &mut self.tiles {
            tile.light_ids.clear();
        }

        for &(id, light) in lights {
            if !light.is_enabled() {
                continue;
            }

            match light.position() {
                None => {
                    // Directional lights affect all tiles
                    for tile in &mut self.tiles {
                        tile.light_ids.push(id);
                    }
                }
                Some(pos) => {
                    let radius = light.radius();

                    // Project light sphere to screen-space AABB
                    let screen_bounds = self.project_sphere_to_screen(pos, radius);
                    if let Some((sx_min, sy_min, sx_max, sy_max)) = screen_bounds {
                        let tile_x_min = (sx_min / self.tile_size as f32).floor().max(0.0) as u32;
                        let tile_y_min = (sy_min / self.tile_size as f32).floor().max(0.0) as u32;
                        let tile_x_max = ((sx_max / self.tile_size as f32).ceil() as u32).min(self.tiles_x);
                        let tile_y_max = ((sy_max / self.tile_size as f32).ceil() as u32).min(self.tiles_y);

                        for ty in tile_y_min..tile_y_max {
                            for tx in tile_x_min..tile_x_max {
                                let idx = (ty * self.tiles_x + tx) as usize;
                                if idx < self.tiles.len() {
                                    self.tiles[idx].light_ids.push(id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Project a sphere onto the screen, returning (min_x, min_y, max_x, max_y) in pixels.
    fn project_sphere_to_screen(&self, center: Vec3, radius: f32) -> Option<(f32, f32, f32, f32)> {
        let clip_center = self.view_projection.transform_point(center);

        // Check if the sphere is behind the camera
        if clip_center.z < -1.0 - radius {
            return None;
        }

        // Conservative screen-space bounds
        let ndc_x = clip_center.x;
        let ndc_y = clip_center.y;
        let dist = center.length().max(0.1);
        let angular_radius = (radius / dist).min(1.0);

        let sx = (ndc_x * 0.5 + 0.5) * self.screen_width as f32;
        let sy = (ndc_y * 0.5 + 0.5) * self.screen_height as f32;
        let screen_radius = angular_radius * self.screen_width as f32;

        Some((
            (sx - screen_radius).max(0.0),
            (sy - screen_radius).max(0.0),
            (sx + screen_radius).min(self.screen_width as f32),
            (sy + screen_radius).min(self.screen_height as f32),
        ))
    }

    /// Get the tile at a pixel coordinate.
    pub fn tile_at_pixel(&self, x: u32, y: u32) -> Option<&ScreenTile> {
        let tx = x / self.tile_size;
        let ty = y / self.tile_size;
        if tx < self.tiles_x && ty < self.tiles_y {
            Some(&self.tiles[(ty * self.tiles_x + tx) as usize])
        } else {
            None
        }
    }

    /// Get statistics.
    pub fn stats(&self) -> TiledCullingStats {
        let mut total_assignments = 0usize;
        let mut max_per_tile = 0usize;
        let mut tiles_with_lights = 0u32;

        for tile in &self.tiles {
            let count = tile.light_ids.len();
            total_assignments += count;
            max_per_tile = max_per_tile.max(count);
            if count > 0 {
                tiles_with_lights += 1;
            }
        }

        TiledCullingStats {
            total_tiles: self.tiles.len() as u32,
            tiles_with_lights,
            total_light_tile_pairs: total_assignments as u32,
            max_lights_per_tile: max_per_tile as u32,
            avg_lights_per_active_tile: if tiles_with_lights > 0 {
                total_assignments as f32 / tiles_with_lights as f32
            } else {
                0.0
            },
        }
    }
}

/// Statistics for tiled light culling.
#[derive(Debug, Clone)]
pub struct TiledCullingStats {
    pub total_tiles: u32,
    pub tiles_with_lights: u32,
    pub total_light_tile_pairs: u32,
    pub max_lights_per_tile: u32,
    pub avg_lights_per_active_tile: f32,
}

// ── Light Cluster ───────────────────────────────────────────────────────────

/// A single 3D cluster in the frustum.
#[derive(Debug, Clone)]
pub struct LightCluster {
    /// Lights assigned to this cluster.
    pub light_ids: Vec<LightId>,
    /// Cluster AABB in view space.
    pub min_bounds: Vec3,
    pub max_bounds: Vec3,
}

impl LightCluster {
    pub fn new(min_bounds: Vec3, max_bounds: Vec3) -> Self {
        Self {
            light_ids: Vec::new(),
            min_bounds,
            max_bounds,
        }
    }

    /// Check if a sphere (in view space) intersects this cluster's AABB.
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        let mut dist_sq = 0.0f32;

        let check = |c: f32, min: f32, max: f32| -> f32 {
            if c < min {
                let d = min - c;
                d * d
            } else if c > max {
                let d = c - max;
                d * d
            } else {
                0.0
            }
        };

        dist_sq += check(center.x, self.min_bounds.x, self.max_bounds.x);
        dist_sq += check(center.y, self.min_bounds.y, self.max_bounds.y);
        dist_sq += check(center.z, self.min_bounds.z, self.max_bounds.z);

        dist_sq <= radius * radius
    }
}

/// 3D frustum-based clustered light assignment for clustered forward rendering.
#[derive(Debug, Clone)]
pub struct ClusteredLightAssignment {
    /// Number of clusters in X (screen width).
    pub clusters_x: u32,
    /// Number of clusters in Y (screen height).
    pub clusters_y: u32,
    /// Number of clusters in Z (depth slices).
    pub clusters_z: u32,
    /// All clusters stored in a flat array.
    pub clusters: Vec<LightCluster>,
    /// Camera near plane.
    pub near: f32,
    /// Camera far plane.
    pub far: f32,
    /// Field of view (vertical, in radians).
    pub fov_y: f32,
    /// Aspect ratio.
    pub aspect: f32,
    /// View matrix for the current frame.
    pub view_matrix: Mat4,
    /// Logarithmic depth slice distribution.
    pub log_depth: bool,
}

impl ClusteredLightAssignment {
    pub fn new(
        clusters_x: u32,
        clusters_y: u32,
        clusters_z: u32,
        near: f32,
        far: f32,
        fov_y: f32,
        aspect: f32,
    ) -> Self {
        let total = (clusters_x as usize) * (clusters_y as usize) * (clusters_z as usize);
        let mut assignment = Self {
            clusters_x,
            clusters_y,
            clusters_z,
            clusters: Vec::with_capacity(total),
            near,
            far,
            fov_y,
            aspect,
            view_matrix: Mat4::IDENTITY,
            log_depth: true,
        };
        assignment.build_clusters();
        assignment
    }

    /// Compute the depth of a Z slice boundary.
    fn slice_depth(&self, slice: u32) -> f32 {
        let t = slice as f32 / self.clusters_z as f32;
        if self.log_depth {
            // Logarithmic distribution: more slices near the camera
            self.near * (self.far / self.near).powf(t)
        } else {
            self.near + (self.far - self.near) * t
        }
    }

    /// Determine which Z slice a view-space depth falls into.
    pub fn depth_to_slice(&self, depth: f32) -> u32 {
        if depth <= self.near {
            return 0;
        }
        if depth >= self.far {
            return self.clusters_z.saturating_sub(1);
        }

        let slice = if self.log_depth {
            let log_near = self.near.ln();
            let log_far = self.far.ln();
            let log_depth = depth.ln();
            ((log_depth - log_near) / (log_far - log_near) * self.clusters_z as f32) as u32
        } else {
            (((depth - self.near) / (self.far - self.near)) * self.clusters_z as f32) as u32
        };

        slice.min(self.clusters_z - 1)
    }

    /// Build cluster AABBs in view space.
    fn build_clusters(&mut self) {
        self.clusters.clear();

        let tan_half_fov = (self.fov_y * 0.5).tan();

        for z in 0..self.clusters_z {
            let z_near = self.slice_depth(z);
            let z_far = self.slice_depth(z + 1);

            for y in 0..self.clusters_y {
                for x in 0..self.clusters_x {
                    // Compute the tile's NDC extents
                    let tile_x_ndc = (x as f32 / self.clusters_x as f32) * 2.0 - 1.0;
                    let tile_x_ndc_end = ((x + 1) as f32 / self.clusters_x as f32) * 2.0 - 1.0;
                    let tile_y_ndc = (y as f32 / self.clusters_y as f32) * 2.0 - 1.0;
                    let tile_y_ndc_end = ((y + 1) as f32 / self.clusters_y as f32) * 2.0 - 1.0;

                    // Convert to view space at the near and far depths
                    let x_min_near = tile_x_ndc * tan_half_fov * self.aspect * z_near;
                    let x_max_near = tile_x_ndc_end * tan_half_fov * self.aspect * z_near;
                    let y_min_near = tile_y_ndc * tan_half_fov * z_near;
                    let y_max_near = tile_y_ndc_end * tan_half_fov * z_near;

                    let x_min_far = tile_x_ndc * tan_half_fov * self.aspect * z_far;
                    let x_max_far = tile_x_ndc_end * tan_half_fov * self.aspect * z_far;
                    let y_min_far = tile_y_ndc * tan_half_fov * z_far;
                    let y_max_far = tile_y_ndc_end * tan_half_fov * z_far;

                    let min_bounds = Vec3::new(
                        x_min_near.min(x_min_far),
                        y_min_near.min(y_min_far),
                        -z_far, // View space Z is negative
                    );
                    let max_bounds = Vec3::new(
                        x_max_near.max(x_max_far),
                        y_max_near.max(y_max_far),
                        -z_near,
                    );

                    self.clusters.push(LightCluster::new(min_bounds, max_bounds));
                }
            }
        }
    }

    /// Get cluster index from 3D coordinates.
    fn cluster_index(&self, x: u32, y: u32, z: u32) -> usize {
        (z as usize) * (self.clusters_x as usize * self.clusters_y as usize)
            + (y as usize) * (self.clusters_x as usize)
            + (x as usize)
    }

    /// Get cluster index from a screen pixel and depth.
    pub fn cluster_at(&self, pixel_x: u32, pixel_y: u32, depth: f32, screen_w: u32, screen_h: u32) -> usize {
        let cx = (pixel_x as f32 / screen_w as f32 * self.clusters_x as f32) as u32;
        let cy = (pixel_y as f32 / screen_h as f32 * self.clusters_y as f32) as u32;
        let cz = self.depth_to_slice(depth);

        let cx = cx.min(self.clusters_x - 1);
        let cy = cy.min(self.clusters_y - 1);

        self.cluster_index(cx, cy, cz)
    }

    /// Assign lights to clusters.
    pub fn assign_lights(
        &mut self,
        lights: &[(LightId, Vec3, f32)], // (id, view-space position, radius)
    ) {
        // Clear existing assignments
        for cluster in &mut self.clusters {
            cluster.light_ids.clear();
        }

        for &(id, view_pos, radius) in lights {
            // Find the depth range this light covers
            let light_z_near = (-view_pos.z - radius).max(self.near);
            let light_z_far = (-view_pos.z + radius).min(self.far);

            if light_z_far < self.near || light_z_near > self.far {
                continue; // Light is outside the frustum depth range
            }

            let z_start = self.depth_to_slice(light_z_near);
            let z_end = self.depth_to_slice(light_z_far);

            for z in z_start..=z_end.min(self.clusters_z - 1) {
                for y in 0..self.clusters_y {
                    for x in 0..self.clusters_x {
                        let idx = self.cluster_index(x, y, z);
                        if idx < self.clusters.len() && self.clusters[idx].intersects_sphere(view_pos, radius) {
                            self.clusters[idx].light_ids.push(id);
                        }
                    }
                }
            }
        }
    }

    /// Assign lights from world-space positions, transforming to view space first.
    pub fn assign_lights_world(
        &mut self,
        lights: &[(LightId, &Light)],
    ) {
        let mut view_lights = Vec::new();

        for &(id, light) in lights {
            if !light.is_enabled() {
                continue;
            }
            if let Some(pos) = light.position() {
                let view_pos = self.view_matrix.transform_point(pos);
                let radius = light.radius();
                view_lights.push((id, view_pos, radius));
            }
        }

        self.assign_lights(&view_lights);

        // Directional lights go into every cluster
        for &(id, light) in lights {
            if let Light::Directional(_) = light {
                if light.is_enabled() {
                    for cluster in &mut self.clusters {
                        cluster.light_ids.push(id);
                    }
                }
            }
        }
    }

    /// Get the lights for a specific cluster.
    pub fn lights_for_cluster(&self, index: usize) -> &[LightId] {
        if index < self.clusters.len() {
            &self.clusters[index].light_ids
        } else {
            &[]
        }
    }

    /// Get the lights at a screen pixel and depth.
    pub fn lights_at_pixel(&self, pixel_x: u32, pixel_y: u32, depth: f32, screen_w: u32, screen_h: u32) -> &[LightId] {
        let idx = self.cluster_at(pixel_x, pixel_y, depth, screen_w, screen_h);
        self.lights_for_cluster(idx)
    }

    /// Total number of clusters.
    pub fn total_clusters(&self) -> usize {
        self.clusters.len()
    }

    /// Get statistics.
    pub fn stats(&self) -> ClusteredStats {
        let mut total_assignments = 0usize;
        let mut max_per_cluster = 0usize;
        let mut active_clusters = 0u32;
        let mut empty_clusters = 0u32;

        for cluster in &self.clusters {
            let count = cluster.light_ids.len();
            total_assignments += count;
            max_per_cluster = max_per_cluster.max(count);
            if count > 0 {
                active_clusters += 1;
            } else {
                empty_clusters += 1;
            }
        }

        ClusteredStats {
            total_clusters: self.clusters.len() as u32,
            active_clusters,
            empty_clusters,
            total_light_cluster_pairs: total_assignments as u32,
            max_lights_per_cluster: max_per_cluster as u32,
            avg_lights_per_active_cluster: if active_clusters > 0 {
                total_assignments as f32 / active_clusters as f32
            } else {
                0.0
            },
        }
    }

    /// Rebuild clusters (call when camera params change).
    pub fn rebuild(&mut self) {
        self.build_clusters();
    }
}

/// Statistics for clustered light assignment.
#[derive(Debug, Clone)]
pub struct ClusteredStats {
    pub total_clusters: u32,
    pub active_clusters: u32,
    pub empty_clusters: u32,
    pub total_light_cluster_pairs: u32,
    pub max_lights_per_cluster: u32,
    pub avg_lights_per_active_cluster: f32,
}

// ── Volumetric System ───────────────────────────────────────────────────────

/// Orchestrates all volumetric effects.
#[derive(Debug)]
pub struct VolumetricSystem {
    pub light_shafts: VolumetricLightShafts,
    pub fog: VolumetricFog,
    pub tiled_culling: Option<TiledLightCulling>,
    pub clustered_assignment: Option<ClusteredLightAssignment>,
    /// Whether volumetric light shafts are enabled.
    pub shafts_enabled: bool,
    /// Whether volumetric fog is enabled.
    pub fog_enabled: bool,
    /// Whether tiled culling is active.
    pub tiled_culling_enabled: bool,
    /// Whether clustered assignment is active.
    pub clustered_enabled: bool,
}

impl VolumetricSystem {
    pub fn new() -> Self {
        Self {
            light_shafts: VolumetricLightShafts::default(),
            fog: VolumetricFog::default(),
            tiled_culling: None,
            clustered_assignment: None,
            shafts_enabled: true,
            fog_enabled: true,
            tiled_culling_enabled: false,
            clustered_enabled: false,
        }
    }

    /// Initialize tiled light culling for the given screen resolution.
    pub fn init_tiled_culling(&mut self, width: u32, height: u32, tile_size: u32) {
        self.tiled_culling = Some(TiledLightCulling::new(width, height, tile_size));
        self.tiled_culling_enabled = true;
    }

    /// Initialize clustered forward rendering.
    pub fn init_clustered(
        &mut self,
        clusters_x: u32,
        clusters_y: u32,
        clusters_z: u32,
        near: f32,
        far: f32,
        fov_y: f32,
        aspect: f32,
    ) {
        self.clustered_assignment = Some(ClusteredLightAssignment::new(
            clusters_x, clusters_y, clusters_z,
            near, far, fov_y, aspect,
        ));
        self.clustered_enabled = true;
    }

    /// Update tiled light culling with current lights and depth buffer.
    pub fn update_tiled(
        &mut self,
        lights: &[(LightId, &Light)],
        depth_buffer: &[f32],
    ) {
        if !self.tiled_culling_enabled {
            return;
        }
        if let Some(ref mut tiled) = self.tiled_culling {
            tiled.update_depth_ranges(depth_buffer);
            tiled.cull_lights(lights);
        }
    }

    /// Update clustered light assignment with current lights.
    pub fn update_clustered(&mut self, lights: &[(LightId, &Light)]) {
        if !self.clustered_enabled {
            return;
        }
        if let Some(ref mut clustered) = self.clustered_assignment {
            clustered.assign_lights_world(lights);
        }
    }

    /// Update the light shaft screen position.
    pub fn update_light_shaft_position(&mut self, light_world_pos: Vec3, view_projection: &Mat4) {
        if self.shafts_enabled {
            self.light_shafts.update_light_position(light_world_pos, view_projection);
        }
    }

    /// Get tiled culling stats.
    pub fn tiled_stats(&self) -> Option<TiledCullingStats> {
        self.tiled_culling.as_ref().map(|t| t.stats())
    }

    /// Get clustered stats.
    pub fn clustered_stats(&self) -> Option<ClusteredStats> {
        self.clustered_assignment.as_ref().map(|c| c.stats())
    }
}

impl Default for VolumetricSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_henyey_greenstein() {
        // Forward scattering should be stronger when g > 0
        let forward = VolumetricFog::henyey_greenstein(1.0, 0.5);
        let backward = VolumetricFog::henyey_greenstein(-1.0, 0.5);
        assert!(forward > backward);

        // Isotropic when g = 0
        let iso_fwd = VolumetricFog::henyey_greenstein(1.0, 0.0);
        let iso_bwd = VolumetricFog::henyey_greenstein(-1.0, 0.0);
        assert!((iso_fwd - iso_bwd).abs() < 0.01);
    }

    #[test]
    fn test_fog_density_height() {
        let field = FogDensityField::HeightExponential {
            base_density: 1.0,
            falloff: 1.0,
            base_height: 0.0,
        };
        let low = field.sample(Vec3::new(0.0, 0.0, 0.0));
        let high = field.sample(Vec3::new(0.0, 10.0, 0.0));
        assert!(low > high); // Fog should be denser at lower heights
    }

    #[test]
    fn test_fog_density_sphere() {
        let field = FogDensityField::Sphere {
            center: Vec3::ZERO,
            radius: 5.0,
            density: 1.0,
        };
        let center = field.sample(Vec3::ZERO);
        let edge = field.sample(Vec3::new(5.0, 0.0, 0.0));
        let outside = field.sample(Vec3::new(10.0, 0.0, 0.0));
        assert!(center > edge);
        assert!(outside < 1e-5);
    }

    #[test]
    fn test_volumetric_fog_ray_march() {
        let fog = VolumetricFog::new(FogDensityField::Uniform(0.1));
        let (color, transmittance) = fog.ray_march(
            Vec3::ZERO,
            Vec3::FORWARD,
            50.0,
            Vec3::new(0.0, -1.0, 0.0),
            Color::WHITE,
            1.0,
        );
        assert!(transmittance < 1.0); // Some light should be absorbed
        assert!(color.r > 0.0); // Some light should be scattered
    }

    #[test]
    fn test_tiled_culling_creation() {
        let tiled = TiledLightCulling::new(1920, 1080, 16);
        assert_eq!(tiled.tiles_x, 120);
        assert_eq!(tiled.tiles_y, (1080 + 15) / 16);
        assert_eq!(tiled.tiles.len(), (tiled.tiles_x * tiled.tiles_y) as usize);
    }

    #[test]
    fn test_clustered_depth_slicing() {
        let clustered = ClusteredLightAssignment::new(
            16, 8, 24, 0.1, 1000.0, 1.0, 1.777,
        );
        // Near depth should map to slice 0
        assert_eq!(clustered.depth_to_slice(0.1), 0);
        // Far depth should map to the last slice
        assert_eq!(clustered.depth_to_slice(1000.0), 23);
        // Mid depth should be somewhere in between
        let mid = clustered.depth_to_slice(10.0);
        assert!(mid > 0 && mid < 23);
    }

    #[test]
    fn test_clustered_light_assignment() {
        let mut clustered = ClusteredLightAssignment::new(
            4, 4, 4, 0.1, 100.0, 1.0, 1.0,
        );

        // A light at the center of the frustum should hit some clusters
        let lights = vec![
            (LightId(1), Vec3::new(0.0, 0.0, -10.0), 5.0),
        ];
        clustered.assign_lights(&lights);

        let stats = clustered.stats();
        assert!(stats.active_clusters > 0);
        assert!(stats.total_light_cluster_pairs > 0);
    }

    #[test]
    fn test_light_shafts_visibility() {
        let mut shafts = VolumetricLightShafts::default();
        shafts.light_screen_pos = (0.5, 0.5);
        assert!(shafts.is_light_visible());

        shafts.light_screen_pos = (2.0, 2.0);
        assert!(!shafts.is_light_visible());
    }

    #[test]
    fn test_fog_transmittance() {
        let fog = VolumetricFog::new(FogDensityField::Uniform(0.1));
        let t1 = fog.transmittance(Vec3::ZERO, Vec3::FORWARD, 10.0);
        let t2 = fog.transmittance(Vec3::ZERO, Vec3::FORWARD, 50.0);
        // Longer distance = less transmittance
        assert!(t1 > t2);
        // Both should be between 0 and 1
        assert!(t1 > 0.0 && t1 < 1.0);
        assert!(t2 > 0.0 && t2 < 1.0);
    }

    #[test]
    fn test_cluster_sphere_intersection() {
        let cluster = LightCluster::new(
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, 1.0),
        );
        assert!(cluster.intersects_sphere(Vec3::ZERO, 0.5));
        assert!(cluster.intersects_sphere(Vec3::new(2.0, 0.0, 0.0), 1.5));
        assert!(!cluster.intersects_sphere(Vec3::new(5.0, 5.0, 5.0), 1.0));
    }
}
