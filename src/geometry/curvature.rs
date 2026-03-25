//! Curvature visualization — Gaussian, mean curvature rendered as glyph color.

use glam::{Vec3, Vec4};
use super::GeoMesh;
use std::collections::HashMap;

/// Type of curvature to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurvatureType {
    Gaussian,
    Mean,
    Principal1,
    Principal2,
}

/// Per-vertex curvature field for a mesh.
#[derive(Debug, Clone)]
pub struct CurvatureField {
    pub curvature_type: CurvatureType,
    pub values: Vec<f32>,
    pub min_value: f32,
    pub max_value: f32,
}

impl CurvatureField {
    /// Compute curvature for each vertex of the mesh.
    pub fn compute(mesh: &GeoMesh, curvature_type: CurvatureType) -> Self {
        let n = mesh.vertices.len();
        let mut values = vec![0.0f32; n];

        // Build vertex adjacency + one-ring neighborhoods
        let adj = build_one_ring(mesh);

        for vi in 0..n {
            let p = mesh.vertices[vi];
            let neighbors = match adj.get(&(vi as u32)) {
                Some(n) => n,
                None => continue,
            };
            if neighbors.is_empty() { continue; }

            match curvature_type {
                CurvatureType::Gaussian => {
                    // Angle deficit method: K = (2π - Σθ) / A
                    let mut angle_sum = 0.0f32;
                    let mut area = 0.0f32;

                    for i in 0..neighbors.len() {
                        let j = (i + 1) % neighbors.len();
                        let a = mesh.vertices[neighbors[i] as usize] - p;
                        let b = mesh.vertices[neighbors[j] as usize] - p;
                        let cos_angle = a.dot(b) / (a.length() * b.length()).max(1e-10);
                        angle_sum += cos_angle.clamp(-1.0, 1.0).acos();
                        area += a.cross(b).length() * 0.5;
                    }

                    let mixed_area = (area / 3.0).max(1e-10);
                    values[vi] = (std::f32::consts::TAU - angle_sum) / mixed_area;
                }
                CurvatureType::Mean => {
                    // Discrete Laplacian: H = |Δp| / (2A)
                    let mut laplacian = Vec3::ZERO;
                    let mut area = 0.0f32;

                    for &ni in neighbors {
                        let q = mesh.vertices[ni as usize];
                        laplacian += q - p;
                        area += (q - p).length();
                    }
                    laplacian /= neighbors.len() as f32;
                    let mixed_area = (area / neighbors.len() as f32).max(1e-10);
                    values[vi] = laplacian.length() / (2.0 * mixed_area);
                }
                CurvatureType::Principal1 | CurvatureType::Principal2 => {
                    // Approximate: H ± sqrt(H² - K)
                    let h = {
                        let mut lap = Vec3::ZERO;
                        for &ni in neighbors { lap += mesh.vertices[ni as usize] - p; }
                        lap /= neighbors.len() as f32;
                        lap.length() * 0.5
                    };
                    let k = {
                        let mut angle_sum = 0.0f32;
                        let mut area = 0.0f32;
                        for i in 0..neighbors.len() {
                            let j = (i + 1) % neighbors.len();
                            let a = mesh.vertices[neighbors[i] as usize] - p;
                            let b = mesh.vertices[neighbors[j] as usize] - p;
                            angle_sum += (a.dot(b) / (a.length() * b.length()).max(1e-10)).clamp(-1.0, 1.0).acos();
                            area += a.cross(b).length() * 0.5;
                        }
                        (std::f32::consts::TAU - angle_sum) / (area / 3.0).max(1e-10)
                    };
                    let disc = (h * h - k).max(0.0).sqrt();
                    values[vi] = if curvature_type == CurvatureType::Principal1 { h + disc } else { h - disc };
                }
            }
        }

        let min_value = values.iter().copied().fold(f32::MAX, f32::min);
        let max_value = values.iter().copied().fold(f32::MIN, f32::max);

        Self { curvature_type, values, min_value, max_value }
    }

    /// Map curvature values to colors and apply to mesh.
    pub fn colorize_mesh(&self, mesh: &mut GeoMesh) {
        let range = (self.max_value - self.min_value).max(1e-8);
        mesh.colors.resize(mesh.vertices.len(), Vec4::ONE);
        for (i, &val) in self.values.iter().enumerate() {
            let t = ((val - self.min_value) / range).clamp(0.0, 1.0);
            // Blue (low) → White (mid) → Red (high)
            let color = if t < 0.5 {
                let s = t * 2.0;
                Vec4::new(s, s, 1.0, 1.0)
            } else {
                let s = (t - 0.5) * 2.0;
                Vec4::new(1.0, 1.0 - s, 1.0 - s, 1.0)
            };
            mesh.colors[i] = color;
        }
    }
}

fn build_one_ring(mesh: &GeoMesh) -> HashMap<u32, Vec<u32>> {
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for tri in &mesh.triangles {
        for &(a, b) in &[(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)] {
            let entry = adj.entry(a).or_default();
            if !entry.contains(&b) { entry.push(b); }
            let entry = adj.entry(b).or_default();
            if !entry.contains(&a) { entry.push(a); }
        }
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;
    use crate::geometry::parametric::{Sphere, SurfaceGrid, ParametricSurface};

    #[test]
    fn sphere_gaussian_curvature_positive() {
        let sphere = Sphere { radius: 1.0 };
        let grid = SurfaceGrid::sample(&sphere, 10, 10);
        let mesh = grid.to_mesh();
        let curv = CurvatureField::compute(&mesh, CurvatureType::Gaussian);
        // Sphere has positive Gaussian curvature everywhere
        let positive_count = curv.values.iter().filter(|&&v| v > 0.0).count();
        assert!(positive_count > curv.values.len() / 2);
    }

    #[test]
    fn curvature_colorizes() {
        let sphere = Sphere { radius: 1.0 };
        let grid = SurfaceGrid::sample(&sphere, 6, 6);
        let mut mesh = grid.to_mesh();
        let curv = CurvatureField::compute(&mesh, CurvatureType::Mean);
        curv.colorize_mesh(&mut mesh);
        assert_eq!(mesh.colors.len(), mesh.vertices.len());
    }
}
