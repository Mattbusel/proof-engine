use glam::{Vec3, Vec4, IVec3, UVec3};
use super::octree::{Aabb, VoxelData, VoxelGrid};

/// Configuration for voxelization.
#[derive(Debug, Clone)]
pub struct VoxelizeConfig {
    pub resolution: u32,
    pub world_bounds: Aabb,
    pub conservative: bool,
}

/// A triangle with vertex positions, normal, color, and emission.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
    pub normal: Vec3,
    pub color: Vec4,
    pub emission: Vec4,
}

impl Triangle {
    pub fn aabb(&self) -> Aabb {
        Aabb {
            min: self.v0.min(self.v1).min(self.v2),
            max: self.v0.max(self.v1).max(self.v2),
        }
    }

    pub fn compute_normal(&self) -> Vec3 {
        let e1 = self.v1 - self.v0;
        let e2 = self.v2 - self.v0;
        e1.cross(e2).normalize_or_zero()
    }
}

/// Statistics from voxelization.
#[derive(Debug, Clone, Default)]
pub struct VoxelizeStats {
    pub triangles_processed: u32,
    pub voxels_written: u32,
    pub fill_ratio: f32,
}

/// GPU voxelization uniform parameters (for compute shader path).
#[derive(Debug, Clone, Copy)]
pub struct GpuVoxelizeParams {
    pub resolution: u32,
    pub world_min: Vec3,
    pub world_max: Vec3,
    pub proj_x: glam::Mat4,
    pub proj_y: glam::Mat4,
    pub proj_z: glam::Mat4,
}

impl GpuVoxelizeParams {
    pub fn from_config(config: &VoxelizeConfig) -> Self {
        let size = config.world_bounds.size();
        let center = config.world_bounds.center();
        let half = size * 0.5;

        // Orthographic projections along each axis
        let proj_x = glam::Mat4::orthographic_rh(
            -half.y, half.y, -half.z, half.z, 0.0, size.x,
        );
        let proj_y = glam::Mat4::orthographic_rh(
            -half.x, half.x, -half.z, half.z, 0.0, size.y,
        );
        let proj_z = glam::Mat4::orthographic_rh(
            -half.x, half.x, -half.y, half.y, 0.0, size.z,
        );

        Self {
            resolution: config.resolution,
            world_min: config.world_bounds.min,
            world_max: config.world_bounds.max,
            proj_x,
            proj_y,
            proj_z,
        }
    }
}

/// Voxelize a set of triangles into a dense grid.
pub fn voxelize_triangles(triangles: &[Triangle], config: &VoxelizeConfig) -> VoxelGrid {
    let res = config.resolution;
    let mut grid = VoxelGrid::new(UVec3::new(res, res, res));
    let world_size = config.world_bounds.size();
    let voxel_size = world_size / Vec3::splat(res as f32);
    let half_voxel = voxel_size * 0.5;

    for tri in triangles {
        if config.conservative {
            let voxels = conservative_voxelize(tri, config);
            for iv in voxels {
                if grid.in_bounds(iv.x, iv.y, iv.z) {
                    let vd = grid.get_mut(iv.x as u32, iv.y as u32, iv.z as u32);
                    vd.radiance = tri.color + tri.emission;
                    vd.normal = tri.normal;
                    vd.opacity = tri.color.w;
                }
            }
        } else {
            let tri_aabb = tri.aabb();

            // Convert triangle AABB to voxel coordinates
            let vmin = ((tri_aabb.min - config.world_bounds.min) / voxel_size).floor();
            let vmax = ((tri_aabb.max - config.world_bounds.min) / voxel_size).ceil();

            let ix_min = (vmin.x as i32).max(0);
            let iy_min = (vmin.y as i32).max(0);
            let iz_min = (vmin.z as i32).max(0);
            let ix_max = (vmax.x as i32).min(res as i32 - 1);
            let iy_max = (vmax.y as i32).min(res as i32 - 1);
            let iz_max = (vmax.z as i32).min(res as i32 - 1);

            for z in iz_min..=iz_max {
                for y in iy_min..=iy_max {
                    for x in ix_min..=ix_max {
                        let box_center = config.world_bounds.min + Vec3::new(
                            (x as f32 + 0.5) * voxel_size.x,
                            (y as f32 + 0.5) * voxel_size.y,
                            (z as f32 + 0.5) * voxel_size.z,
                        );
                        if triangle_box_intersection(tri, box_center, half_voxel) {
                            let vd = grid.get_mut(x as u32, y as u32, z as u32);
                            vd.radiance = tri.color + tri.emission;
                            vd.normal = tri.normal;
                            vd.opacity = tri.color.w;
                        }
                    }
                }
            }
        }
    }

    grid
}

/// SAT-based triangle-box intersection test.
pub fn triangle_box_intersection(tri: &Triangle, box_center: Vec3, box_half: Vec3) -> bool {
    // Translate triangle so box center is at origin
    let v0 = tri.v0 - box_center;
    let v1 = tri.v1 - box_center;
    let v2 = tri.v2 - box_center;

    let e0 = v1 - v0;
    let e1 = v2 - v1;
    let e2 = v0 - v2;

    let h = box_half;

    // Test 9 cross-product axes (edge x box-face-normal)
    let axes = [
        Vec3::new(0.0, -e0.z, e0.y),
        Vec3::new(0.0, -e1.z, e1.y),
        Vec3::new(0.0, -e2.z, e2.y),
        Vec3::new(e0.z, 0.0, -e0.x),
        Vec3::new(e1.z, 0.0, -e1.x),
        Vec3::new(e2.z, 0.0, -e2.x),
        Vec3::new(-e0.y, e0.x, 0.0),
        Vec3::new(-e1.y, e1.x, 0.0),
        Vec3::new(-e2.y, e2.x, 0.0),
    ];

    for axis in &axes {
        let p0 = v0.dot(*axis);
        let p1 = v1.dot(*axis);
        let p2 = v2.dot(*axis);
        let r = h.x * axis.x.abs() + h.y * axis.y.abs() + h.z * axis.z.abs();
        let min_p = p0.min(p1).min(p2);
        let max_p = p0.max(p1).max(p2);
        if min_p > r || max_p < -r {
            return false;
        }
    }

    // Test 3 box face normals (AABB axes)
    if v0.x.min(v1.x).min(v2.x) > h.x || v0.x.max(v1.x).max(v2.x) < -h.x {
        return false;
    }
    if v0.y.min(v1.y).min(v2.y) > h.y || v0.y.max(v1.y).max(v2.y) < -h.y {
        return false;
    }
    if v0.z.min(v1.z).min(v2.z) > h.z || v0.z.max(v1.z).max(v2.z) < -h.z {
        return false;
    }

    // Test triangle normal
    let tri_normal = e0.cross(e1);
    let d = tri_normal.dot(v0);
    let r = h.x * tri_normal.x.abs() + h.y * tri_normal.y.abs() + h.z * tri_normal.z.abs();
    if d.abs() > r {
        return false;
    }

    true
}

/// Conservative voxelization: dilated rasterization producing all voxels a triangle might touch.
pub fn conservative_voxelize(tri: &Triangle, config: &VoxelizeConfig) -> Vec<IVec3> {
    let res = config.resolution;
    let world_size = config.world_bounds.size();
    let voxel_size = world_size / Vec3::splat(res as f32);
    let half_voxel = voxel_size * 0.5;
    // Expand the triangle AABB by one voxel in each direction for conservative coverage
    let tri_aabb = tri.aabb();
    let expanded_min = tri_aabb.min - voxel_size;
    let expanded_max = tri_aabb.max + voxel_size;

    let vmin = ((expanded_min - config.world_bounds.min) / voxel_size).floor();
    let vmax = ((expanded_max - config.world_bounds.min) / voxel_size).ceil();

    let ix_min = (vmin.x as i32).max(0);
    let iy_min = (vmin.y as i32).max(0);
    let iz_min = (vmin.z as i32).max(0);
    let ix_max = (vmax.x as i32).min(res as i32 - 1);
    let iy_max = (vmax.y as i32).min(res as i32 - 1);
    let iz_max = (vmax.z as i32).min(res as i32 - 1);

    let mut result = Vec::new();
    for z in iz_min..=iz_max {
        for y in iy_min..=iy_max {
            for x in ix_min..=ix_max {
                let box_center = config.world_bounds.min + Vec3::new(
                    (x as f32 + 0.5) * voxel_size.x,
                    (y as f32 + 0.5) * voxel_size.y,
                    (z as f32 + 0.5) * voxel_size.z,
                );
                // Use slightly expanded half-voxel for conservative test
                let expanded_half = half_voxel * 1.5;
                if triangle_box_intersection(tri, box_center, expanded_half) {
                    result.push(IVec3::new(x, y, z));
                }
            }
        }
    }
    result
}

/// Voxelize spheres into a grid.
pub fn voxelize_spheres(spheres: &[(Vec3, f32, Vec4)], config: &VoxelizeConfig) -> VoxelGrid {
    let res = config.resolution;
    let mut grid = VoxelGrid::new(UVec3::new(res, res, res));
    let world_size = config.world_bounds.size();
    let voxel_size = world_size / Vec3::splat(res as f32);

    for &(center, radius, color) in spheres {
        let vmin = ((center - Vec3::splat(radius) - config.world_bounds.min) / voxel_size).floor();
        let vmax = ((center + Vec3::splat(radius) - config.world_bounds.min) / voxel_size).ceil();

        let ix_min = (vmin.x as i32).max(0);
        let iy_min = (vmin.y as i32).max(0);
        let iz_min = (vmin.z as i32).max(0);
        let ix_max = (vmax.x as i32).min(res as i32 - 1);
        let iy_max = (vmax.y as i32).min(res as i32 - 1);
        let iz_max = (vmax.z as i32).min(res as i32 - 1);

        for z in iz_min..=iz_max {
            for y in iy_min..=iy_max {
                for x in ix_min..=ix_max {
                    let world_pos = config.world_bounds.min + Vec3::new(
                        (x as f32 + 0.5) * voxel_size.x,
                        (y as f32 + 0.5) * voxel_size.y,
                        (z as f32 + 0.5) * voxel_size.z,
                    );
                    let dist = (world_pos - center).length();
                    if dist <= radius {
                        let normal = if dist > 1e-6 {
                            (world_pos - center).normalize()
                        } else {
                            Vec3::Y
                        };
                        let vd = grid.get_mut(x as u32, y as u32, z as u32);
                        vd.radiance = color;
                        vd.normal = normal;
                        vd.opacity = color.w;
                    }
                }
            }
        }
    }
    grid
}

/// Voxelize a point cloud.
pub fn voxelize_point_cloud(points: &[(Vec3, Vec4)], config: &VoxelizeConfig) -> VoxelGrid {
    let res = config.resolution;
    let mut grid = VoxelGrid::new(UVec3::new(res, res, res));
    let world_size = config.world_bounds.size();
    let voxel_size = world_size / Vec3::splat(res as f32);

    for &(pos, color) in points {
        if !config.world_bounds.contains(pos) {
            continue;
        }
        let voxel_pos = ((pos - config.world_bounds.min) / voxel_size).floor();
        let x = (voxel_pos.x as u32).min(res - 1);
        let y = (voxel_pos.y as u32).min(res - 1);
        let z = (voxel_pos.z as u32).min(res - 1);

        let vd = grid.get_mut(x, y, z);
        vd.radiance = color;
        vd.normal = Vec3::Y; // default normal for points
        vd.opacity = color.w;
    }
    grid
}

/// Merge two voxel grids of the same resolution. Non-empty voxels from b overwrite a.
pub fn merge_grids(a: &VoxelGrid, b: &VoxelGrid) -> VoxelGrid {
    assert_eq!(a.resolution, b.resolution, "Grids must have same resolution");
    let mut result = a.clone();
    for i in 0..result.data.len() {
        if !b.data[i].is_empty() {
            if result.data[i].is_empty() {
                result.data[i] = b.data[i];
            } else {
                // Blend: average
                result.data[i].radiance = (result.data[i].radiance + b.data[i].radiance) * 0.5;
                result.data[i].normal = (result.data[i].normal + b.data[i].normal).normalize_or_zero();
                result.data[i].opacity = (result.data[i].opacity + b.data[i].opacity) * 0.5;
            }
        }
    }
    result
}

pub fn compute_voxelize_stats(grid: &VoxelGrid, triangles_processed: u32) -> VoxelizeStats {
    let total = (grid.resolution.x * grid.resolution.y * grid.resolution.z) as u32;
    let filled = grid.filled_count() as u32;
    VoxelizeStats {
        triangles_processed,
        voxels_written: filled,
        fill_ratio: if total > 0 { filled as f32 / total as f32 } else { 0.0 },
    }
}

/// Embedded compute shader source for GPU voxelization.
pub const VOXELIZE_COMP_SRC: &str = r#"
#version 450
layout(local_size_x = 64) in;

struct Triangle {
    vec4 v0;
    vec4 v1;
    vec4 v2;
    vec4 normal;
    vec4 color;
    vec4 emission;
};

layout(std430, binding = 0) readonly buffer TriangleBuffer {
    Triangle triangles[];
};

layout(r32ui, binding = 1) uniform uimage3D voxelGrid;

uniform mat4 projX;
uniform mat4 projY;
uniform mat4 projZ;
uniform uint resolution;
uniform vec3 worldMin;
uniform vec3 worldSize;

void main() {
    uint triIdx = gl_GlobalInvocationID.x;
    if (triIdx >= triangles.length()) return;

    Triangle tri = triangles[triIdx];
    vec3 v0 = tri.v0.xyz;
    vec3 v1 = tri.v1.xyz;
    vec3 v2 = tri.v2.xyz;

    // Choose dominant axis for projection (conservative rasterization)
    vec3 n = abs(tri.normal.xyz);
    mat4 proj;
    if (n.x >= n.y && n.x >= n.z) {
        proj = projX;
    } else if (n.y >= n.z) {
        proj = projY;
    } else {
        proj = projZ;
    }

    // Compute AABB in voxel space
    vec3 voxelSize = worldSize / float(resolution);
    vec3 minV = min(min(v0, v1), v2);
    vec3 maxV = max(max(v0, v1), v2);
    // Conservative expansion
    minV -= voxelSize;
    maxV += voxelSize;

    ivec3 iMin = ivec3(clamp((minV - worldMin) / voxelSize, vec3(0), vec3(resolution - 1)));
    ivec3 iMax = ivec3(clamp((maxV - worldMin) / voxelSize, vec3(0), vec3(resolution - 1)));

    for (int z = iMin.z; z <= iMax.z; z++) {
        for (int y = iMin.y; y <= iMax.y; y++) {
            for (int x = iMin.x; x <= iMax.x; x++) {
                // Pack color into uint for atomic write
                uint packed = (uint(tri.color.r * 255.0) << 24) |
                              (uint(tri.color.g * 255.0) << 16) |
                              (uint(tri.color.b * 255.0) << 8) |
                              (uint(tri.color.a * 255.0));
                imageAtomicMax(voxelGrid, ivec3(x, y, z), packed);
            }
        }
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(res: u32) -> VoxelizeConfig {
        VoxelizeConfig {
            resolution: res,
            world_bounds: Aabb::new(Vec3::ZERO, Vec3::splat(res as f32)),
            conservative: false,
        }
    }

    #[test]
    fn test_single_triangle_voxelizes() {
        let config = make_config(8);
        let tri = Triangle {
            v0: Vec3::new(2.0, 2.0, 4.0),
            v1: Vec3::new(6.0, 2.0, 4.0),
            v2: Vec3::new(4.0, 6.0, 4.0),
            normal: Vec3::Z,
            color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            emission: Vec4::ZERO,
        };

        let grid = voxelize_triangles(&[tri], &config);
        let filled = grid.filled_count();
        assert!(filled > 0, "Triangle should produce at least one voxel, got {filled}");
    }

    #[test]
    fn test_triangle_box_intersection_basic() {
        let tri = Triangle {
            v0: Vec3::new(-1.0, 0.0, 0.0),
            v1: Vec3::new(1.0, 0.0, 0.0),
            v2: Vec3::new(0.0, 1.0, 0.0),
            normal: Vec3::Z,
            color: Vec4::ONE,
            emission: Vec4::ZERO,
        };
        // Box centered at origin with half-size 0.5
        assert!(triangle_box_intersection(&tri, Vec3::ZERO, Vec3::splat(0.5)));
        // Box far away
        assert!(!triangle_box_intersection(&tri, Vec3::splat(10.0), Vec3::splat(0.5)));
    }

    #[test]
    fn test_sphere_voxelization() {
        let config = make_config(16);
        let spheres = vec![(Vec3::splat(8.0), 3.0, Vec4::new(0.0, 1.0, 0.0, 1.0))];
        let grid = voxelize_spheres(&spheres, &config);
        let filled = grid.filled_count();
        assert!(filled > 10, "Sphere should fill many voxels, got {filled}");

        // Check a voxel at the center is filled
        let center_voxel = grid.get(8, 8, 8);
        assert!(!center_voxel.is_empty());
    }

    #[test]
    fn test_point_cloud_voxelization() {
        let config = make_config(8);
        let points = vec![
            (Vec3::new(1.0, 1.0, 1.0), Vec4::new(1.0, 0.0, 0.0, 1.0)),
            (Vec3::new(5.0, 5.0, 5.0), Vec4::new(0.0, 1.0, 0.0, 1.0)),
        ];
        let grid = voxelize_point_cloud(&points, &config);
        assert_eq!(grid.filled_count(), 2);
    }

    #[test]
    fn test_empty_scene_empty_grid() {
        let config = make_config(4);
        let grid = voxelize_triangles(&[], &config);
        assert_eq!(grid.filled_count(), 0);
    }

    #[test]
    fn test_conservative_voxelize() {
        let config = VoxelizeConfig {
            resolution: 8,
            world_bounds: Aabb::new(Vec3::ZERO, Vec3::splat(8.0)),
            conservative: true,
        };
        let tri = Triangle {
            v0: Vec3::new(3.0, 3.0, 4.0),
            v1: Vec3::new(5.0, 3.0, 4.0),
            v2: Vec3::new(4.0, 5.0, 4.0),
            normal: Vec3::Z,
            color: Vec4::ONE,
            emission: Vec4::ZERO,
        };
        let grid = voxelize_triangles(&[tri], &config);
        let non_conservative_config = VoxelizeConfig {
            conservative: false,
            ..config.clone()
        };
        let grid_nc = voxelize_triangles(&[tri], &non_conservative_config);
        // Conservative should produce at least as many voxels
        assert!(grid.filled_count() >= grid_nc.filled_count());
    }

    #[test]
    fn test_merge_grids() {
        let mut a = VoxelGrid::new(UVec3::new(4, 4, 4));
        let mut b = VoxelGrid::new(UVec3::new(4, 4, 4));
        a.set(0, 0, 0, VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        b.set(1, 1, 1, VoxelData {
            radiance: Vec4::new(0.0, 0.0, 1.0, 1.0),
            normal: Vec3::X,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        let merged = merge_grids(&a, &b);
        assert_eq!(merged.filled_count(), 2);
    }

    #[test]
    fn test_voxelize_stats() {
        let config = make_config(4);
        let tri = Triangle {
            v0: Vec3::new(0.5, 0.5, 2.0),
            v1: Vec3::new(3.5, 0.5, 2.0),
            v2: Vec3::new(2.0, 3.5, 2.0),
            normal: Vec3::Z,
            color: Vec4::ONE,
            emission: Vec4::ZERO,
        };
        let grid = voxelize_triangles(&[tri], &config);
        let stats = compute_voxelize_stats(&grid, 1);
        assert_eq!(stats.triangles_processed, 1);
        assert!(stats.voxels_written > 0);
        assert!(stats.fill_ratio > 0.0);
        assert!(stats.fill_ratio <= 1.0);
    }

    #[test]
    fn test_gpu_params() {
        let config = make_config(64);
        let params = GpuVoxelizeParams::from_config(&config);
        assert_eq!(params.resolution, 64);
    }

    #[test]
    fn test_triangle_aabb() {
        let tri = Triangle {
            v0: Vec3::new(1.0, 2.0, 3.0),
            v1: Vec3::new(4.0, 1.0, 2.0),
            v2: Vec3::new(2.0, 5.0, 1.0),
            normal: Vec3::Z,
            color: Vec4::ONE,
            emission: Vec4::ZERO,
        };
        let aabb = tri.aabb();
        assert!((aabb.min.x - 1.0).abs() < 1e-5);
        assert!((aabb.max.x - 4.0).abs() < 1e-5);
        assert!((aabb.min.y - 1.0).abs() < 1e-5);
        assert!((aabb.max.y - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_triangle_compute_normal() {
        let tri = Triangle {
            v0: Vec3::new(0.0, 0.0, 0.0),
            v1: Vec3::new(1.0, 0.0, 0.0),
            v2: Vec3::new(0.0, 1.0, 0.0),
            normal: Vec3::Z,
            color: Vec4::ONE,
            emission: Vec4::ZERO,
        };
        let n = tri.compute_normal();
        assert!((n.z - 1.0).abs() < 1e-5 || (n.z + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_multiple_triangles() {
        let config = make_config(16);
        let tris = vec![
            Triangle {
                v0: Vec3::new(1.0, 1.0, 8.0),
                v1: Vec3::new(5.0, 1.0, 8.0),
                v2: Vec3::new(3.0, 5.0, 8.0),
                normal: Vec3::Z,
                color: Vec4::new(1.0, 0.0, 0.0, 1.0),
                emission: Vec4::ZERO,
            },
            Triangle {
                v0: Vec3::new(8.0, 8.0, 8.0),
                v1: Vec3::new(12.0, 8.0, 8.0),
                v2: Vec3::new(10.0, 12.0, 8.0),
                normal: Vec3::Z,
                color: Vec4::new(0.0, 0.0, 1.0, 1.0),
                emission: Vec4::ZERO,
            },
        ];
        let grid = voxelize_triangles(&tris, &config);
        assert!(grid.filled_count() > 2, "Two triangles should voxelize to more than 2 voxels");
    }

    #[test]
    fn test_sphere_at_boundary() {
        let config = make_config(8);
        let spheres = vec![(Vec3::ZERO, 1.0, Vec4::ONE)];
        let grid = voxelize_spheres(&spheres, &config);
        // Sphere is partially outside the grid, should still have some voxels
        let count = grid.filled_count();
        assert!(count > 0, "Sphere at boundary should produce some voxels");
    }

    #[test]
    fn test_point_cloud_outside_bounds() {
        let config = make_config(4);
        let points = vec![
            (Vec3::splat(-10.0), Vec4::ONE), // Outside bounds
            (Vec3::splat(2.0), Vec4::ONE),   // Inside bounds
        ];
        let grid = voxelize_point_cloud(&points, &config);
        assert_eq!(grid.filled_count(), 1, "Only the in-bounds point should be voxelized");
    }

    #[test]
    fn test_merge_overlapping_grids() {
        let mut a = VoxelGrid::new(UVec3::new(4, 4, 4));
        let mut b = VoxelGrid::new(UVec3::new(4, 4, 4));
        // Both grids have a voxel at the same position
        a.set(1, 1, 1, VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        b.set(1, 1, 1, VoxelData {
            radiance: Vec4::new(0.0, 0.0, 1.0, 1.0),
            normal: Vec3::X,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        let merged = merge_grids(&a, &b);
        let v = merged.get(1, 1, 1);
        // Should be blended
        assert!((v.radiance.x - 0.5).abs() < 1e-5);
        assert!((v.radiance.z - 0.5).abs() < 1e-5);
    }
}
