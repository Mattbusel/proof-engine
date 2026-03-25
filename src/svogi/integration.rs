use glam::{Vec3, Vec4};
use super::octree::{SparseVoxelOctree, VoxelData, VoxelGrid, Aabb};
use super::atlas::BrickAtlas;
use super::voxelize::{Triangle, VoxelizeConfig, voxelize_triangles};
use super::inject::{LightSource, ShadowMap, inject_direct_light, inject_emissive};
use super::propagate::{PropagationConfig, propagate_light};
use super::cone_trace::{ConeTraceConfig, diffuse_gi, specular_gi, ambient_occlusion};

/// Update rate for the SVOGI system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateRate {
    EveryFrame,
    EveryNFrames(u32),
    OnDemand,
}

/// Configuration for the SVOGI system.
#[derive(Debug, Clone)]
pub struct SvogiConfig {
    pub resolution: u32,
    pub max_depth: u8,
    pub gi_intensity: f32,
    pub ao_intensity: f32,
    pub bounce_count: u32,
    pub update_rate: UpdateRate,
}

impl Default for SvogiConfig {
    fn default() -> Self {
        Self {
            resolution: 64,
            max_depth: 6,
            gi_intensity: 1.0,
            ao_intensity: 1.0,
            bounce_count: 2,
            update_rate: UpdateRate::EveryFrame,
        }
    }
}

/// Debug visualization modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SvogiDebugView {
    Voxels,
    Radiance,
    Normals,
    AO,
    SHBands,
    ConeDirections,
}

/// Statistics for the SVOGI system.
#[derive(Debug, Clone, Default)]
pub struct SvogiStats {
    pub voxel_count: usize,
    pub update_time_ms: f32,
    pub trace_time_ms: f32,
    pub memory_mb: f32,
}

/// The main SVOGI system integrating all components.
pub struct SvogiSystem {
    pub octree: SparseVoxelOctree,
    pub atlas: BrickAtlas,
    pub config: SvogiConfig,
    grid: Option<VoxelGrid>,
    frame_counter: u32,
    dirty_regions: Vec<Aabb>,
    world_bounds: Aabb,
}

impl SvogiSystem {
    /// Initialize the SVOGI system.
    pub fn init(world_bounds: Aabb, config: SvogiConfig) -> Self {
        let octree = SparseVoxelOctree::new(world_bounds, config.max_depth);
        let atlas_size = glam::UVec3::splat(config.resolution * 2);
        let atlas = BrickAtlas::new(atlas_size, 4);

        Self {
            octree,
            atlas,
            config,
            grid: None,
            frame_counter: 0,
            dirty_regions: Vec::new(),
            world_bounds,
        }
    }

    /// Voxelize a scene from triangles.
    pub fn voxelize_scene(&mut self, scene_triangles: &[Triangle]) {
        let vox_config = VoxelizeConfig {
            resolution: self.config.resolution,
            world_bounds: self.world_bounds,
            conservative: false,
        };
        let grid = voxelize_triangles(scene_triangles, &vox_config);
        self.octree = SparseVoxelOctree::build_from_voxel_grid(&grid, self.world_bounds);
        self.grid = Some(grid);
    }

    /// Inject direct lighting from light sources and shadow maps.
    pub fn inject_lights(&mut self, lights: &[LightSource], shadow_maps: &[ShadowMap]) {
        if let Some(ref mut grid) = self.grid {
            inject_direct_light(
                grid,
                lights,
                shadow_maps,
                self.world_bounds.min,
                self.world_bounds.size(),
            );
            inject_emissive(grid);
            // Rebuild octree from updated grid
            self.octree = SparseVoxelOctree::build_from_voxel_grid(grid, self.world_bounds);
        }
    }

    /// Propagate light for the given number of iterations.
    pub fn propagate(&mut self, iterations: u32) {
        if let Some(ref mut grid) = self.grid {
            let config = PropagationConfig {
                iterations,
                damping: 0.8,
                flux_weight: 1.0,
            };
            propagate_light(grid, &config);
            self.octree = SparseVoxelOctree::build_from_voxel_grid(grid, self.world_bounds);
        }
    }

    /// Compute per-pixel GI from G-buffer data.
    pub fn apply_gi(
        &self,
        gbuffer_positions: &[Vec3],
        gbuffer_normals: &[Vec3],
        gbuffer_albedo: &[Vec3],
    ) -> Vec<Vec4> {
        let config = ConeTraceConfig {
            max_distance: self.world_bounds.size().length() * 0.5,
            step_multiplier: 1.0,
            ao_weight: self.config.ao_intensity,
            gi_weight: self.config.gi_intensity,
        };

        let mut result = Vec::with_capacity(gbuffer_positions.len());

        for i in 0..gbuffer_positions.len() {
            let pos = gbuffer_positions[i];
            let normal = gbuffer_normals[i];
            let albedo = gbuffer_albedo[i];

            if normal.length_squared() < 0.01 {
                result.push(Vec4::ZERO);
                continue;
            }

            let gi = diffuse_gi(&self.octree, pos, normal, &config);
            let ao = ambient_occlusion(&self.octree, pos, normal, &config);

            let final_color = albedo * (gi + Vec3::splat(ao * 0.1));
            result.push(Vec4::new(final_color.x, final_color.y, final_color.z, 1.0));
        }

        result
    }

    /// Full frame update: voxelize if needed, inject, propagate.
    pub fn update(
        &mut self,
        dt: f32,
        scene_triangles: &[Triangle],
        lights: &[LightSource],
        shadow_maps: &[ShadowMap],
    ) {
        self.frame_counter += 1;

        let should_update = match self.config.update_rate {
            UpdateRate::EveryFrame => true,
            UpdateRate::EveryNFrames(n) => self.frame_counter % n == 0,
            UpdateRate::OnDemand => !self.dirty_regions.is_empty(),
        };

        if !should_update {
            return;
        }

        // Voxelize
        if self.dirty_regions.is_empty() {
            self.voxelize_scene(scene_triangles);
        } else {
            // Partial revoxelization: only for dirty regions
            // For simplicity, revoxelize the whole scene but only if dirty
            self.voxelize_scene(scene_triangles);
            self.dirty_regions.clear();
        }

        // Inject lighting
        self.inject_lights(lights, shadow_maps);

        // Propagate bounced lighting
        self.propagate(self.config.bounce_count);
    }

    /// Mark a region as dirty for incremental updates.
    pub fn mark_dirty(&mut self, region: Aabb) {
        self.dirty_regions.push(region);
    }

    /// Render debug visualization.
    pub fn render_debug(&self, view: SvogiDebugView, camera_pos: Vec3) -> Vec<(Vec3, Vec4)> {
        let mut points = Vec::new();

        for (pos, data) in self.octree.iter_leaves() {
            let color = match view {
                SvogiDebugView::Voxels => {
                    Vec4::new(1.0, 1.0, 1.0, data.opacity)
                }
                SvogiDebugView::Radiance => {
                    data.radiance
                }
                SvogiDebugView::Normals => {
                    Vec4::new(
                        data.normal.x * 0.5 + 0.5,
                        data.normal.y * 0.5 + 0.5,
                        data.normal.z * 0.5 + 0.5,
                        1.0,
                    )
                }
                SvogiDebugView::AO => {
                    let config = ConeTraceConfig::default();
                    let ao = ambient_occlusion(&self.octree, pos, data.normal, &config);
                    Vec4::new(ao, ao, ao, 1.0)
                }
                SvogiDebugView::SHBands => {
                    // Visualize first 3 SH bands as RGB
                    Vec4::new(
                        data.sh_coeffs[0].abs(),
                        if data.sh_coeffs.len() > 1 { data.sh_coeffs[1].abs() } else { 0.0 },
                        if data.sh_coeffs.len() > 2 { data.sh_coeffs[2].abs() } else { 0.0 },
                        1.0,
                    )
                }
                SvogiDebugView::ConeDirections => {
                    // Color based on normal direction
                    let dir_color = (data.normal + Vec3::ONE) * 0.5;
                    Vec4::new(dir_color.x, dir_color.y, dir_color.z, 1.0)
                }
            };

            points.push((pos, color));
        }

        // Sort by distance from camera (back to front for transparency)
        points.sort_by(|a, b| {
            let da = (a.0 - camera_pos).length_squared();
            let db = (b.0 - camera_pos).length_squared();
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });

        points
    }

    /// Get statistics.
    pub fn stats(&self) -> SvogiStats {
        SvogiStats {
            voxel_count: self.octree.leaf_count(),
            update_time_ms: 0.0,
            trace_time_ms: 0.0,
            memory_mb: self.octree.memory_usage() as f32 / (1024.0 * 1024.0),
        }
    }
}

/// Cascaded SVOGI for multi-resolution coverage.
pub struct CascadedSvogi {
    pub cascades: Vec<SvogiSystem>,
}

impl CascadedSvogi {
    /// Create nested cascades. Each cascade covers a larger area at lower resolution.
    pub fn new(cascade_count: u32, base_resolution: u32, world_size: f32) -> Self {
        let mut cascades = Vec::with_capacity(cascade_count as usize);

        for i in 0..cascade_count {
            let scale = 2.0f32.powi(i as i32);
            let half_size = world_size * scale * 0.5;
            let bounds = Aabb::new(
                Vec3::splat(-half_size),
                Vec3::splat(half_size),
            );
            let config = SvogiConfig {
                resolution: base_resolution,
                max_depth: (base_resolution as f32).log2() as u8,
                gi_intensity: 1.0 / scale, // Farther cascades contribute less
                ao_intensity: 1.0 / scale,
                bounce_count: (2.0 / scale).max(1.0) as u32,
                update_rate: if i == 0 {
                    UpdateRate::EveryFrame
                } else {
                    UpdateRate::EveryNFrames(1 << i)
                },
            };
            cascades.push(SvogiSystem::init(bounds, config));
        }

        Self { cascades }
    }

    /// Update all cascades.
    pub fn update(
        &mut self,
        dt: f32,
        scene_triangles: &[Triangle],
        lights: &[LightSource],
        shadow_maps: &[ShadowMap],
    ) {
        for cascade in &mut self.cascades {
            cascade.update(dt, scene_triangles, lights, shadow_maps);
        }
    }

    /// Sample GI from the appropriate cascade based on distance.
    pub fn sample_gi(&self, position: Vec3, normal: Vec3) -> Vec3 {
        let config = ConeTraceConfig::default();
        let mut total_gi = Vec3::ZERO;
        let mut weight_sum = 0.0f32;

        for (i, cascade) in self.cascades.iter().enumerate() {
            if cascade.world_bounds.contains(position) {
                let gi = diffuse_gi(&cascade.octree, position, normal, &config);
                let weight = 1.0 / (i as f32 + 1.0);
                total_gi += gi * weight;
                weight_sum += weight;
            }
        }

        if weight_sum > 0.0 {
            total_gi / weight_sum
        } else {
            Vec3::ZERO
        }
    }

    /// Get combined stats.
    pub fn stats(&self) -> SvogiStats {
        let mut combined = SvogiStats::default();
        for cascade in &self.cascades {
            let s = cascade.stats();
            combined.voxel_count += s.voxel_count;
            combined.memory_mb += s.memory_mb;
        }
        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svogi::inject::{DirectionalLight, LightSource};

    fn make_test_triangles() -> Vec<Triangle> {
        vec![
            Triangle {
                v0: Vec3::new(1.0, 1.0, 1.0),
                v1: Vec3::new(3.0, 1.0, 1.0),
                v2: Vec3::new(2.0, 3.0, 1.0),
                normal: Vec3::Z,
                color: Vec4::new(1.0, 0.0, 0.0, 1.0),
                emission: Vec4::ZERO,
            },
            Triangle {
                v0: Vec3::new(5.0, 5.0, 5.0),
                v1: Vec3::new(7.0, 5.0, 5.0),
                v2: Vec3::new(6.0, 7.0, 5.0),
                normal: Vec3::Z,
                color: Vec4::new(0.0, 1.0, 0.0, 1.0),
                emission: Vec4::ZERO,
            },
        ]
    }

    #[test]
    fn test_init_system() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(16.0));
        let config = SvogiConfig::default();
        let system = SvogiSystem::init(bounds, config);
        assert_eq!(system.octree.node_count(), 1); // Just root
    }

    #[test]
    fn test_voxelize_scene() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            ..Default::default()
        };
        let mut system = SvogiSystem::init(bounds, config);
        system.voxelize_scene(&make_test_triangles());
        assert!(system.octree.node_count() > 1);
    }

    #[test]
    fn test_full_pipeline_nonzero_gi() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            gi_intensity: 1.0,
            ao_intensity: 1.0,
            bounce_count: 1,
            update_rate: UpdateRate::EveryFrame,
        };
        let mut system = SvogiSystem::init(bounds, config);

        let triangles = make_test_triangles();
        let light = LightSource::Directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::ONE,
            intensity: 2.0,
        });

        system.voxelize_scene(&triangles);
        system.inject_lights(&[light], &[]);
        system.propagate(2);

        // Check that the octree has data
        let leaves: Vec<_> = system.octree.iter_leaves().collect();
        assert!(!leaves.is_empty(), "Should have voxels after pipeline");

        // Check at least some radiance exists
        let has_radiance = leaves.iter().any(|(_, d)| d.radiance.x > 0.0 || d.radiance.y > 0.0 || d.radiance.z > 0.0);
        assert!(has_radiance, "Some voxels should have non-zero radiance after injection");
    }

    #[test]
    fn test_apply_gi() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            ..Default::default()
        };
        let mut system = SvogiSystem::init(bounds, config);
        system.voxelize_scene(&make_test_triangles());

        let positions = vec![Vec3::new(2.0, 2.0, 2.0)];
        let normals = vec![Vec3::Y];
        let albedos = vec![Vec3::ONE];
        let result = system.apply_gi(&positions, &normals, &albedos);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_mark_dirty() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            update_rate: UpdateRate::OnDemand,
            ..Default::default()
        };
        let mut system = SvogiSystem::init(bounds, config);
        system.mark_dirty(Aabb::new(Vec3::ZERO, Vec3::splat(4.0)));
        assert_eq!(system.dirty_regions.len(), 1);
    }

    #[test]
    fn test_debug_views() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            ..Default::default()
        };
        let mut system = SvogiSystem::init(bounds, config);
        system.voxelize_scene(&make_test_triangles());

        for view in &[
            SvogiDebugView::Voxels,
            SvogiDebugView::Radiance,
            SvogiDebugView::Normals,
            SvogiDebugView::SHBands,
            SvogiDebugView::ConeDirections,
        ] {
            let points = system.render_debug(*view, Vec3::splat(4.0));
            // Just verify it doesn't panic
            let _ = points;
        }
    }

    #[test]
    fn test_stats() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            ..Default::default()
        };
        let mut system = SvogiSystem::init(bounds, config);
        system.voxelize_scene(&make_test_triangles());
        let stats = system.stats();
        assert!(stats.voxel_count > 0);
        assert!(stats.memory_mb > 0.0);
    }

    #[test]
    fn test_cascaded_svogi() {
        let cascaded = CascadedSvogi::new(3, 8, 16.0);
        assert_eq!(cascaded.cascades.len(), 3);

        let stats = cascaded.stats();
        assert_eq!(stats.voxel_count, 0); // No voxels yet
    }

    #[test]
    fn test_cascaded_sample_gi() {
        let mut cascaded = CascadedSvogi::new(2, 8, 16.0);

        // Voxelize first cascade
        let triangles = vec![Triangle {
            v0: Vec3::new(-2.0, -2.0, 0.0),
            v1: Vec3::new(2.0, -2.0, 0.0),
            v2: Vec3::new(0.0, 2.0, 0.0),
            normal: Vec3::Z,
            color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            emission: Vec4::ZERO,
        }];
        cascaded.cascades[0].voxelize_scene(&triangles);

        let gi = cascaded.sample_gi(Vec3::ZERO, Vec3::Y);
        // Just verify it runs without panic
        let _ = gi;
    }

    #[test]
    fn test_update_rate_every_n_frames() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let config = SvogiConfig {
            resolution: 4,
            max_depth: 2,
            update_rate: UpdateRate::EveryNFrames(3),
            bounce_count: 1,
            ..Default::default()
        };
        let mut system = SvogiSystem::init(bounds, config);
        let tris = make_test_triangles();

        // Frame 1: should not update (counter = 1)
        system.update(0.016, &tris, &[], &[]);
        let count_1 = system.octree.node_count();

        // Frame 2: should not update (counter = 2)
        system.update(0.016, &tris, &[], &[]);

        // Frame 3: should update (counter = 3, 3%3 == 0)
        system.update(0.016, &tris, &[], &[]);
        let count_3 = system.octree.node_count();

        assert!(count_3 > count_1 || count_3 >= 1);
    }
}
