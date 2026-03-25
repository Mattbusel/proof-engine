//! Convex hull computation in 2D and 3D.

use glam::{Vec2, Vec3};

/// 2D convex hull result.
#[derive(Debug, Clone)]
pub struct ConvexHull2D {
    pub points: Vec<Vec2>,
    pub hull_indices: Vec<usize>,
}

/// 3D convex hull result.
#[derive(Debug, Clone)]
pub struct ConvexHull {
    pub points: Vec<Vec3>,
    pub hull_faces: Vec<[usize; 3]>,
    pub hull_vertices: Vec<usize>,
}

impl ConvexHull2D {
    /// Compute 2D convex hull using Andrew's monotone chain algorithm.
    pub fn compute(points: &[Vec2]) -> Self {
        let n = points.len();
        if n < 3 {
            return Self { points: points.to_vec(), hull_indices: (0..n).collect() };
        }

        let mut sorted: Vec<(usize, Vec2)> = points.iter().copied().enumerate().collect();
        sorted.sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap().then(a.1.y.partial_cmp(&b.1.y).unwrap()));

        let mut hull = Vec::with_capacity(2 * n);

        // Lower hull
        for &(i, p) in &sorted {
            while hull.len() >= 2 {
                let a = points[hull[hull.len() - 2]];
                let b = points[hull[hull.len() - 1]];
                if cross2d(a, b, p) <= 0.0 { hull.pop(); } else { break; }
            }
            hull.push(i);
        }

        // Upper hull
        let lower_len = hull.len() + 1;
        for &(i, p) in sorted.iter().rev() {
            while hull.len() >= lower_len {
                let a = points[hull[hull.len() - 2]];
                let b = points[hull[hull.len() - 1]];
                if cross2d(a, b, p) <= 0.0 { hull.pop(); } else { break; }
            }
            hull.push(i);
        }
        hull.pop(); // remove duplicate of first point

        Self { points: points.to_vec(), hull_indices: hull }
    }

    pub fn hull_points(&self) -> Vec<Vec2> {
        self.hull_indices.iter().map(|&i| self.points[i]).collect()
    }

    pub fn perimeter(&self) -> f32 {
        let pts = self.hull_points();
        let n = pts.len();
        (0..n).map(|i| (pts[(i + 1) % n] - pts[i]).length()).sum()
    }

    pub fn area(&self) -> f32 {
        let pts = self.hull_points();
        let n = pts.len();
        if n < 3 { return 0.0; }
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
        }
        area.abs() * 0.5
    }
}

impl ConvexHull {
    /// Compute 3D convex hull using incremental algorithm.
    pub fn compute(points: &[Vec3]) -> Self {
        let n = points.len();
        if n < 4 {
            return Self {
                points: points.to_vec(),
                hull_faces: Vec::new(),
                hull_vertices: (0..n).collect(),
            };
        }

        // Start with a tetrahedron from first 4 non-coplanar points
        let mut faces: Vec<[usize; 3]> = vec![
            [0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2],
        ];

        // Ensure outward-facing normals
        let center = (points[0] + points[1] + points[2] + points[3]) * 0.25;
        for face in &mut faces {
            let a = points[face[0]];
            let b = points[face[1]];
            let c = points[face[2]];
            let n = (b - a).cross(c - a);
            let mid = (a + b + c) / 3.0;
            if n.dot(mid - center) < 0.0 {
                face.swap(1, 2);
            }
        }

        // Add remaining points
        for pi in 4..n {
            let p = points[pi];
            let mut visible = Vec::new();

            for (fi, face) in faces.iter().enumerate() {
                let a = points[face[0]];
                let b = points[face[1]];
                let c = points[face[2]];
                let normal = (b - a).cross(c - a);
                if normal.dot(p - a) > 1e-8 {
                    visible.push(fi);
                }
            }

            if visible.is_empty() { continue; }

            // Find horizon edges
            let mut horizon: Vec<(usize, usize)> = Vec::new();
            for &fi in &visible {
                let face = faces[fi];
                let edges = [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])];
                for &(a, b) in &edges {
                    let is_shared = visible.iter().any(|&ofi| {
                        if ofi == fi { return false; }
                        let of = faces[ofi];
                        let oedges = [(of[0], of[1]), (of[1], of[2]), (of[2], of[0])];
                        oedges.iter().any(|&(oa, ob)| (oa == b && ob == a))
                    });
                    if !is_shared {
                        horizon.push((a, b));
                    }
                }
            }

            // Remove visible faces
            visible.sort_unstable_by(|a, b| b.cmp(a));
            for fi in visible {
                faces.swap_remove(fi);
            }

            // Add new faces from horizon to new point
            for (a, b) in horizon {
                faces.push([a, b, pi]);
            }
        }

        let mut hull_verts: Vec<usize> = faces.iter()
            .flat_map(|f| f.iter().copied())
            .collect();
        hull_verts.sort_unstable();
        hull_verts.dedup();

        Self {
            points: points.to_vec(),
            hull_faces: faces,
            hull_vertices: hull_verts,
        }
    }

    pub fn face_count(&self) -> usize { self.hull_faces.len() }
    pub fn vertex_count(&self) -> usize { self.hull_vertices.len() }
}

fn cross2d(o: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_2d_square() {
        let points = vec![
            Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0),
            Vec2::new(0.5, 0.5), // interior point
        ];
        let hull = ConvexHull2D::compute(&points);
        assert_eq!(hull.hull_indices.len(), 4); // only corners
    }

    #[test]
    fn hull_2d_area() {
        let points = vec![
            Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0),
        ];
        let hull = ConvexHull2D::compute(&points);
        assert!((hull.area() - 4.0).abs() < 0.01);
    }

    #[test]
    fn hull_3d_tetrahedron() {
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0),
        ];
        let hull = ConvexHull::compute(&points);
        assert_eq!(hull.face_count(), 4);
        assert_eq!(hull.vertex_count(), 4);
    }

    #[test]
    fn hull_3d_with_interior() {
        let mut points = vec![
            Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0), Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0), Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 1.0),
            Vec3::ZERO, // interior
        ];
        let hull = ConvexHull::compute(&points);
        assert_eq!(hull.vertex_count(), 8); // interior excluded
    }
}
