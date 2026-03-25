// Sparse Voxel Octree Global Illumination (SVOGI)
//
// Clean-room implementation based on published SIGGRAPH papers:
// - Crassin et al., "Interactive Indirect Illumination Using Voxel Cone Tracing" (2011)
// - Kaplanyan & Dachsbacher, "Cascaded Light Propagation Volumes" (2010)
// - Kaplanyan, "Light Propagation Volumes in CryEngine 3" (2009)

pub mod octree;
pub mod voxelize;
pub mod atlas;
pub mod inject;
pub mod propagate;
pub mod cone_trace;
pub mod integration;
pub mod sh;

pub use octree::{SparseVoxelOctree, OctreeNode, VoxelData, VoxelGrid, Aabb};
pub use voxelize::{VoxelizeConfig, Triangle, voxelize_triangles};
pub use atlas::{BrickAtlas, Brick, BlockPacker};
pub use inject::{LightSource, DirectionalLight, PointLight, SpotLight, ShadowMap};
pub use propagate::{PropagationConfig, propagate_light};
pub use cone_trace::{ConeTraceConfig, ConeTraceResult, trace_cone};
pub use integration::{SvogiSystem, SvogiConfig};
pub use sh::{SH2, SH3};

pub use octree::morton_encode_3d;
pub use octree::morton_decode_3d;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn full_pipeline_smoke_test() {
        let bounds = Aabb { min: Vec3::ZERO, max: Vec3::splat(8.0) };
        let config = SvogiConfig {
            resolution: 8,
            max_depth: 3,
            gi_intensity: 1.0,
            ao_intensity: 1.0,
            bounce_count: 2,
            update_rate: integration::UpdateRate::EveryFrame,
        };
        let mut system = SvogiSystem::init(bounds, config);

        let tri = Triangle {
            v0: Vec3::new(1.0, 1.0, 1.0),
            v1: Vec3::new(3.0, 1.0, 1.0),
            v2: Vec3::new(2.0, 3.0, 1.0),
            normal: Vec3::Z,
            color: glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
            emission: glam::Vec4::ZERO,
        };
        system.voxelize_scene(&[tri]);
        assert!(system.octree.node_count() > 0);
    }
}
