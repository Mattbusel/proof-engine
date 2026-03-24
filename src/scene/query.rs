//! Scene query types and helpers.

use crate::glyph::GlyphId;
use glam::Vec3;

/// Result of a raycast against scene geometry.
#[derive(Debug, Clone)]
pub struct RaycastHit {
    pub glyph_id: GlyphId,
    pub distance: f32,
    pub point:    Vec3,
    pub normal:   Vec3,
}

/// Parameters for a sphere overlap query.
#[derive(Debug, Clone)]
pub struct SphereQuery {
    pub center: Vec3,
    pub radius: f32,
    pub mask:   u32,
}

/// Parameters for a frustum culling query.
#[derive(Debug, Clone)]
pub struct FrustumQuery {
    /// 6 frustum planes (normal, distance).
    pub planes: [(Vec3, f32); 6],
}

impl FrustumQuery {
    /// Test if an AABB is inside (or overlapping) the frustum.
    pub fn contains_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for (normal, d) in &self.planes {
            // Find the positive vertex (furthest along normal)
            let p = Vec3::new(
                if normal.x >= 0.0 { max.x } else { min.x },
                if normal.y >= 0.0 { max.y } else { min.y },
                if normal.z >= 0.0 { max.z } else { min.z },
            );
            if normal.dot(p) + d < 0.0 { return false; }
        }
        true
    }

    /// Test if a sphere is inside or overlapping the frustum.
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        for (normal, d) in &self.planes {
            if normal.dot(center) + d < -radius { return false; }
        }
        true
    }
}

/// High-level query interface that wraps scene state.
pub struct SceneQuery;

impl SceneQuery {
    /// Build a perspective frustum query from a view-projection matrix.
    pub fn frustum_from_vp(vp: &glam::Mat4) -> FrustumQuery {
        let cols = vp.to_cols_array_2d();
        // Gribb-Hartmann frustum plane extraction
        let planes = [
            // Left
            (Vec3::new(cols[0][3] + cols[0][0], cols[1][3] + cols[1][0], cols[2][3] + cols[2][0]),
             cols[3][3] + cols[3][0]),
            // Right
            (Vec3::new(cols[0][3] - cols[0][0], cols[1][3] - cols[1][0], cols[2][3] - cols[2][0]),
             cols[3][3] - cols[3][0]),
            // Bottom
            (Vec3::new(cols[0][3] + cols[0][1], cols[1][3] + cols[1][1], cols[2][3] + cols[2][1]),
             cols[3][3] + cols[3][1]),
            // Top
            (Vec3::new(cols[0][3] - cols[0][1], cols[1][3] - cols[1][1], cols[2][3] - cols[2][1]),
             cols[3][3] - cols[3][1]),
            // Near
            (Vec3::new(cols[0][3] + cols[0][2], cols[1][3] + cols[1][2], cols[2][3] + cols[2][2]),
             cols[3][3] + cols[3][2]),
            // Far
            (Vec3::new(cols[0][3] - cols[0][2], cols[1][3] - cols[1][2], cols[2][3] - cols[2][2]),
             cols[3][3] - cols[3][2]),
        ];
        FrustumQuery { planes }
    }
}
