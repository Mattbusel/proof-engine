//! Marching cubes stub (table to be regenerated).

use glam::{Vec3, Vec4};

pub const EDGE_TABLE: [u16; 256] = [0u16; 256];
pub const TRI_TABLE: [[i8; 16]; 256] = [[-1i8; 16]; 256];

#[derive(Debug, Clone, Default)]
pub struct MCVertex { pub position: Vec3, pub normal: Vec3, pub color: Vec4, pub emission: f32 }

#[derive(Debug, Clone, Default)]
pub struct ExtractedMesh { pub vertices: Vec<MCVertex>, pub indices: Vec<u32> }

pub struct MarchingCubesExtractor;
impl MarchingCubesExtractor {
    pub fn extract(_field: &dyn Fn(Vec3) -> f32, _bounds_min: Vec3, _bounds_max: Vec3, _resolution: u32, _threshold: f32) -> ExtractedMesh {
        ExtractedMesh::default()
    }
}

