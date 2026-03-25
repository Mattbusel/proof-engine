//! Computational Geometry Engine — real-time mathematical surface construction,
//! manipulation, and rendering.
//!
//! Provides parametric surfaces, implicit surface extraction (marching cubes),
//! CSG operations, subdivision surfaces, Voronoi/Delaunay, convex hull,
//! Bezier/NURBS, geodesics, curvature visualization, and topology operations.
//!
//! A game can define a dungeon room as "the interior of a Klein bottle" and the
//! engine renders it with correct topology. Boss arenas can be non-Euclidean spaces.

pub mod parametric;
pub mod implicit;
pub mod csg;
pub mod subdivision;
pub mod voronoi;
pub mod hull;
pub mod bezier;
pub mod geodesic;
pub mod curvature;
pub mod topology;
pub mod deformation;

pub use parametric::{ParametricSurface, SurfaceSample, SurfaceGrid};
pub use implicit::{ScalarField, MarchingCubes, IsoVertex};
pub use csg::{CsgOp, CsgNode, CsgTree};
pub use subdivision::{SubdivisionScheme, SubdivMesh};
pub use voronoi::{VoronoiDiagram, VoronoiCell, DelaunayTriangulation};
pub use hull::ConvexHull;
pub use bezier::{BezierSurface, NurbsSurface, ControlPoint};
pub use geodesic::GeodesicSolver;
pub use curvature::{CurvatureField, CurvatureType};
pub use topology::{TopologyOp, SurfaceTopology, TopologicalSurface};
pub use deformation::{Deformer, DeformField};

use glam::{Vec2, Vec3, Vec4};

/// A triangle in 3D space with vertex indices.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

/// A mesh of vertices and triangles produced by geometry operations.
#[derive(Debug, Clone)]
pub struct GeoMesh {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<Vec4>,
    pub triangles: Vec<Triangle>,
}

impl GeoMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            triangles: Vec::new(),
        }
    }

    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn triangle_count(&self) -> usize { self.triangles.len() }

    pub fn add_vertex(&mut self, pos: Vec3, normal: Vec3, uv: Vec2) -> u32 {
        let idx = self.vertices.len() as u32;
        self.vertices.push(pos);
        self.normals.push(normal);
        self.uvs.push(uv);
        self.colors.push(Vec4::ONE);
        idx
    }

    pub fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.triangles.push(Triangle { a, b, c });
    }

    /// Compute flat normals from triangle geometry.
    pub fn recompute_normals(&mut self) {
        for n in &mut self.normals { *n = Vec3::ZERO; }
        for tri in &self.triangles {
            let a = self.vertices[tri.a as usize];
            let b = self.vertices[tri.b as usize];
            let c = self.vertices[tri.c as usize];
            let n = (b - a).cross(c - a);
            self.normals[tri.a as usize] += n;
            self.normals[tri.b as usize] += n;
            self.normals[tri.c as usize] += n;
        }
        for n in &mut self.normals {
            let len = n.length();
            if len > 1e-8 { *n /= len; }
        }
    }

    /// Bounding box (min, max).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &self.vertices {
            min = min.min(*v);
            max = max.max(*v);
        }
        (min, max)
    }

    /// Merge another mesh into this one.
    pub fn merge(&mut self, other: &GeoMesh) {
        let offset = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.colors.extend_from_slice(&other.colors);
        for tri in &other.triangles {
            self.triangles.push(Triangle {
                a: tri.a + offset,
                b: tri.b + offset,
                c: tri.c + offset,
            });
        }
    }
}

impl Default for GeoMesh {
    fn default() -> Self { Self::new() }
}
