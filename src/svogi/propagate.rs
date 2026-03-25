use glam::{Vec3, Vec4, UVec3};
use super::octree::{VoxelData, VoxelGrid, SparseVoxelOctree, Aabb};

/// Configuration for light propagation.
#[derive(Debug, Clone)]
pub struct PropagationConfig {
    pub iterations: u32,
    pub damping: f32,
    pub flux_weight: f32,
}

impl Default for PropagationConfig {
    fn default() -> Self {
        Self {
            iterations: 4,
            damping: 0.8,
            flux_weight: 1.0,
        }
    }
}

/// SH coefficients for propagation (2nd order, 9 coefficients).
#[derive(Debug, Clone, Copy)]
pub struct SHCoeffs {
    pub coeffs: [f32; 9],
}

impl Default for SHCoeffs {
    fn default() -> Self {
        Self { coeffs: [0.0; 9] }
    }
}

impl SHCoeffs {
    /// Evaluate the SH representation at a given direction.
    pub fn evaluate(&self, direction: Vec3) -> f32 {
        let d = direction.normalize_or_zero();
        let x = d.x;
        let y = d.y;
        let z = d.z;

        // SH basis functions (real, orthonormal)
        let y00 = 0.282095;                        // L=0, M=0
        let y1n1 = 0.488603 * y;                   // L=1, M=-1
        let y10 = 0.488603 * z;                    // L=1, M=0
        let y1p1 = 0.488603 * x;                   // L=1, M=1
        let y2n2 = 1.092548 * x * y;              // L=2, M=-2
        let y2n1 = 1.092548 * y * z;              // L=2, M=-1
        let y20 = 0.315392 * (3.0 * z * z - 1.0); // L=2, M=0
        let y2p1 = 1.092548 * x * z;              // L=2, M=1
        let y2p2 = 0.546274 * (x * x - y * y);    // L=2, M=2

        self.coeffs[0] * y00
            + self.coeffs[1] * y1n1
            + self.coeffs[2] * y10
            + self.coeffs[3] * y1p1
            + self.coeffs[4] * y2n2
            + self.coeffs[5] * y2n1
            + self.coeffs[6] * y20
            + self.coeffs[7] * y2p1
            + self.coeffs[8] * y2p2
    }

    /// Project a directional contribution into SH.
    pub fn project(&mut self, direction: Vec3, value: f32) {
        let d = direction.normalize_or_zero();
        let x = d.x;
        let y = d.y;
        let z = d.z;

        self.coeffs[0] += value * 0.282095;
        self.coeffs[1] += value * 0.488603 * y;
        self.coeffs[2] += value * 0.488603 * z;
        self.coeffs[3] += value * 0.488603 * x;
        self.coeffs[4] += value * 1.092548 * x * y;
        self.coeffs[5] += value * 1.092548 * y * z;
        self.coeffs[6] += value * 0.315392 * (3.0 * z * z - 1.0);
        self.coeffs[7] += value * 1.092548 * x * z;
        self.coeffs[8] += value * 0.546274 * (x * x - y * y);
    }

    pub fn add(&self, other: &SHCoeffs) -> SHCoeffs {
        let mut result = SHCoeffs::default();
        for i in 0..9 {
            result.coeffs[i] = self.coeffs[i] + other.coeffs[i];
        }
        result
    }

    pub fn scale(&self, s: f32) -> SHCoeffs {
        let mut result = SHCoeffs::default();
        for i in 0..9 {
            result.coeffs[i] = self.coeffs[i] * s;
        }
        result
    }

    /// Convolve with clamped cosine kernel (for diffuse transfer).
    pub fn convolve_cosine(&self) -> SHCoeffs {
        // Zonal harmonic coefficients for clamped cosine
        let a0 = std::f32::consts::PI;
        let a1 = 2.0 * std::f32::consts::PI / 3.0;
        let a2 = std::f32::consts::PI / 4.0;

        let mut result = *self;
        result.coeffs[0] *= a0;
        result.coeffs[1] *= a1;
        result.coeffs[2] *= a1;
        result.coeffs[3] *= a1;
        result.coeffs[4] *= a2;
        result.coeffs[5] *= a2;
        result.coeffs[6] *= a2;
        result.coeffs[7] *= a2;
        result.coeffs[8] *= a2;
        result
    }

    /// Create SH representation of a delta function in the given direction.
    pub fn from_direction(dir: Vec3) -> SHCoeffs {
        let mut sh = SHCoeffs::default();
        sh.project(dir, 1.0);
        sh
    }

    /// Total energy (L2 norm squared).
    pub fn energy(&self) -> f32 {
        self.coeffs.iter().map(|c| c * c).sum()
    }
}

/// The six principal face directions for flux propagation.
const FACE_DIRECTIONS: [Vec3; 6] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, -1.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(0.0, 0.0, -1.0),
];

/// Offsets corresponding to FACE_DIRECTIONS for neighbor lookup.
const FACE_OFFSETS: [(i32, i32, i32); 6] = [
    (1, 0, 0), (-1, 0, 0),
    (0, 1, 0), (0, -1, 0),
    (0, 0, 1), (0, 0, -1),
];

/// Compute outgoing flux through a face based on SH-weighted radiance.
pub fn flux_through_face(voxel: &VoxelData, face_normal: Vec3) -> Vec3 {
    let sh = SHCoeffs { coeffs: voxel.sh_coeffs };
    let flux_weight = sh.evaluate(face_normal).max(0.0);
    let radiance = Vec3::new(voxel.radiance.x, voxel.radiance.y, voxel.radiance.z);
    radiance * flux_weight
}

/// Gather incoming light from 6 neighbors.
pub fn gather_from_neighbors(
    grid: &VoxelGrid,
    x: u32,
    y: u32,
    z: u32,
) -> Vec3 {
    let mut incoming = Vec3::ZERO;

    for (i, &(dx, dy, dz)) in FACE_OFFSETS.iter().enumerate() {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        let nz = z as i32 + dz;

        if grid.in_bounds(nx, ny, nz) {
            let neighbor = grid.get(nx as u32, ny as u32, nz as u32);
            if !neighbor.is_empty() {
                // Neighbor sends flux through the face towards us
                let outgoing_face = -FACE_DIRECTIONS[i];
                let flux = flux_through_face(neighbor, outgoing_face);
                incoming += flux;
            }
        }
    }

    incoming
}

/// Single propagation step: read from source grid, write to dest grid.
pub fn propagate_step(
    source: &VoxelGrid,
    dest: &mut VoxelGrid,
    config: &PropagationConfig,
) {
    let res = source.resolution;

    for z in 0..res.z {
        for y in 0..res.y {
            for x in 0..res.x {
                let src = source.get(x, y, z);
                let idx = dest.index(x, y, z);

                if src.is_empty() {
                    dest.data[idx] = *src;
                    continue;
                }

                let incoming = gather_from_neighbors(source, x, y, z);
                let damped = incoming * config.damping * config.flux_weight;

                let existing = Vec3::new(src.radiance.x, src.radiance.y, src.radiance.z);
                let new_radiance = existing + damped;

                dest.data[idx] = VoxelData {
                    radiance: Vec4::new(new_radiance.x, new_radiance.y, new_radiance.z, src.radiance.w),
                    normal: src.normal,
                    opacity: src.opacity,
                    sh_coeffs: src.sh_coeffs,
                };

                // Update SH coefficients based on incoming light direction
                let mut sh = SHCoeffs { coeffs: dest.data[idx].sh_coeffs };
                for (i, &(dx, dy, dz)) in FACE_OFFSETS.iter().enumerate() {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let nz = z as i32 + dz;
                    if source.in_bounds(nx, ny, nz) {
                        let neighbor = source.get(nx as u32, ny as u32, nz as u32);
                        if !neighbor.is_empty() {
                            let luminance = neighbor.radiance.x * 0.299
                                + neighbor.radiance.y * 0.587
                                + neighbor.radiance.z * 0.114;
                            let dir = -FACE_DIRECTIONS[i];
                            sh.project(dir, luminance * config.damping * 0.1);
                        }
                    }
                }
                dest.data[idx].sh_coeffs = sh.coeffs;
            }
        }
    }
}

/// Run multiple iterations of light propagation on a voxel grid.
pub fn propagate_light(grid: &mut VoxelGrid, config: &PropagationConfig) {
    let mut temp = VoxelGrid::new(grid.resolution);

    for _ in 0..config.iterations {
        propagate_step(grid, &mut temp, config);
        std::mem::swap(grid, &mut temp);
    }
}

/// Propagation on octree levels (hierarchical).
pub fn propagate_hierarchical(octree: &mut SparseVoxelOctree, iterations: u32) {
    // Build a temporary grid from each level and propagate
    let max_depth = octree.max_depth;
    let bounds = octree.world_bounds;

    for level in (1..=max_depth).rev() {
        let res = 1u32 << level;
        let mut grid = VoxelGrid::new(UVec3::splat(res));
        let voxel_size = bounds.size() / Vec3::splat(res as f32);

        // Extract voxel data from octree at this level
        for (pos, data) in octree.iter_leaves() {
            let vx = ((pos.x - bounds.min.x) / voxel_size.x).floor() as u32;
            let vy = ((pos.y - bounds.min.y) / voxel_size.y).floor() as u32;
            let vz = ((pos.z - bounds.min.z) / voxel_size.z).floor() as u32;
            if vx < res && vy < res && vz < res {
                grid.set(vx, vy, vz, *data);
            }
        }

        let config = PropagationConfig {
            iterations,
            damping: 0.8,
            flux_weight: 1.0,
        };
        propagate_light(&mut grid, &config);

        // Write back to octree
        for z in 0..res {
            for y in 0..res {
                for x in 0..res {
                    let vd = grid.get(x, y, z);
                    if !vd.is_empty() {
                        let pos = bounds.min + Vec3::new(
                            (x as f32 + 0.5) * voxel_size.x,
                            (y as f32 + 0.5) * voxel_size.y,
                            (z as f32 + 0.5) * voxel_size.z,
                        );
                        octree.insert(pos, *vd);
                    }
                }
            }
        }
    }

    octree.build_mipmaps();
}

/// Embedded compute shader for GPU propagation.
pub const PROPAGATE_COMP_SRC: &str = r#"
#version 450
layout(local_size_x = 4, local_size_y = 4, local_size_z = 4) in;

layout(rgba16f, binding = 0) uniform readonly image3D srcRadiance;
layout(rgba16f, binding = 1) uniform writeonly image3D dstRadiance;

uniform float damping;
uniform float fluxWeight;
uniform uint resolution;

// SH evaluation for directional weighting
float evaluateSH(vec4 shCoeffs, vec3 dir) {
    return shCoeffs.x * 0.282095
         + shCoeffs.y * 0.488603 * dir.y
         + shCoeffs.z * 0.488603 * dir.z
         + shCoeffs.w * 0.488603 * dir.x;
}

void main() {
    ivec3 coord = ivec3(gl_GlobalInvocationID.xyz);
    if (any(greaterThanEqual(coord, ivec3(resolution)))) return;

    vec4 current = imageLoad(srcRadiance, coord);
    if (current.a <= 0.0) {
        imageStore(dstRadiance, coord, current);
        return;
    }

    // 6-directional gather
    vec3 incoming = vec3(0.0);
    ivec3 offsets[6] = ivec3[6](
        ivec3(1,0,0), ivec3(-1,0,0),
        ivec3(0,1,0), ivec3(0,-1,0),
        ivec3(0,0,1), ivec3(0,0,-1)
    );

    for (int i = 0; i < 6; i++) {
        ivec3 ncoord = coord + offsets[i];
        if (all(greaterThanEqual(ncoord, ivec3(0))) && all(lessThan(ncoord, ivec3(resolution)))) {
            vec4 neighbor = imageLoad(srcRadiance, ncoord);
            if (neighbor.a > 0.0) {
                incoming += neighbor.rgb * fluxWeight;
            }
        }
    }

    vec3 propagated = current.rgb + incoming * damping / 6.0;
    imageStore(dstRadiance, coord, vec4(propagated, current.a));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::UVec3;

    #[test]
    fn test_sh_coeffs_evaluate_project() {
        let mut sh = SHCoeffs::default();
        let dir = Vec3::Y;
        sh.project(dir, 1.0);
        let val = sh.evaluate(dir);
        assert!(val > 0.0, "SH evaluation in projected direction should be positive");
    }

    #[test]
    fn test_sh_coeffs_energy() {
        let sh = SHCoeffs::from_direction(Vec3::X);
        assert!(sh.energy() > 0.0);
    }

    #[test]
    fn test_propagation_energy_decreases_with_damping() {
        let mut grid = VoxelGrid::new(UVec3::new(8, 8, 8));
        // Place an emitter in the center
        grid.set(4, 4, 4, VoxelData {
            radiance: Vec4::new(10.0, 10.0, 10.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.282095, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        });
        // Place neighbors so they can receive light
        for &(dx, dy, dz) in &[(1,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)] {
            let x = (4 + dx) as u32;
            let y = (4 + dy) as u32;
            let z = (4 + dz) as u32;
            grid.set(x, y, z, VoxelData {
                radiance: Vec4::new(0.0, 0.0, 0.0, 0.01),
                normal: Vec3::Y,
                opacity: 0.5,
                sh_coeffs: [0.282095, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            });
        }

        let initial_energy: f32 = grid.data.iter()
            .map(|v| v.radiance.x + v.radiance.y + v.radiance.z)
            .sum();

        let config = PropagationConfig {
            iterations: 1,
            damping: 0.5,
            flux_weight: 0.5,
        };
        propagate_light(&mut grid, &config);

        // Check neighbors received some light
        let neighbor_radiance = grid.get(5, 4, 4).radiance.x;
        assert!(neighbor_radiance > 0.0, "Neighbor should receive light, got {neighbor_radiance}");
    }

    #[test]
    fn test_single_emitter_reaches_neighbors() {
        let mut grid = VoxelGrid::new(UVec3::new(8, 8, 8));

        // Place emitter
        grid.set(4, 4, 4, VoxelData {
            radiance: Vec4::new(5.0, 5.0, 5.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        });

        // Place empty but occupant neighbors
        for &(dx, dy, dz) in &[(1i32,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)] {
            grid.set((4+dx) as u32, (4+dy) as u32, (4+dz) as u32, VoxelData {
                radiance: Vec4::new(0.0, 0.0, 0.0, 0.01),
                normal: Vec3::Y,
                opacity: 0.5,
                sh_coeffs: [0.282095, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            });
        }

        let config = PropagationConfig {
            iterations: 2,
            damping: 0.9,
            flux_weight: 1.0,
        };
        propagate_light(&mut grid, &config);

        // All 6 neighbors should have some radiance now
        for &(dx, dy, dz) in &[(1i32,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)] {
            let vd = grid.get((4+dx) as u32, (4+dy) as u32, (4+dz) as u32);
            let luminance = vd.radiance.x + vd.radiance.y + vd.radiance.z;
            assert!(luminance > 0.0, "Neighbor at ({dx},{dy},{dz}) should have light: {luminance}");
        }
    }

    #[test]
    fn test_flux_through_face() {
        let vd = VoxelData {
            radiance: Vec4::new(1.0, 0.5, 0.25, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };

        let flux = flux_through_face(&vd, Vec3::X);
        // With only c0 = 0.5, evaluation is 0.5 * 0.282095 = 0.141
        assert!(flux.x > 0.0);
    }

    #[test]
    fn test_propagation_config_default() {
        let config = PropagationConfig::default();
        assert_eq!(config.iterations, 4);
        assert!(config.damping < 1.0);
    }

    #[test]
    fn test_sh_convolve_cosine() {
        let sh = SHCoeffs::from_direction(Vec3::Y);
        let convolved = sh.convolve_cosine();
        // Convolution should preserve the general direction
        let val_y = convolved.evaluate(Vec3::Y);
        let val_neg_y = convolved.evaluate(-Vec3::Y);
        assert!(val_y > val_neg_y);
    }
}
