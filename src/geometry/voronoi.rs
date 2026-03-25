//! Voronoi diagrams and Delaunay triangulation in 2D and 3D.

use glam::{Vec2, Vec3};
use std::collections::HashMap;

/// A cell in a Voronoi diagram.
#[derive(Debug, Clone)]
pub struct VoronoiCell {
    pub site: Vec2,
    pub vertices: Vec<Vec2>,
    pub neighbor_indices: Vec<usize>,
}

/// 2D Voronoi diagram computed from a set of sites.
#[derive(Debug, Clone)]
pub struct VoronoiDiagram {
    pub sites: Vec<Vec2>,
    pub cells: Vec<VoronoiCell>,
    pub bounds: (Vec2, Vec2),
}

impl VoronoiDiagram {
    /// Compute Voronoi diagram using Fortune's sweep line (simplified brute-force for now).
    pub fn compute(sites: &[Vec2], bounds_min: Vec2, bounds_max: Vec2) -> Self {
        let mut cells = Vec::with_capacity(sites.len());

        // For each site, find its Voronoi cell by clipping the half-planes
        for (i, &site) in sites.iter().enumerate() {
            let mut polygon = vec![
                bounds_min,
                Vec2::new(bounds_max.x, bounds_min.y),
                bounds_max,
                Vec2::new(bounds_min.x, bounds_max.y),
            ];

            let mut neighbors = Vec::new();
            for (j, &other) in sites.iter().enumerate() {
                if i == j { continue; }
                let clipped = clip_polygon_by_bisector(&polygon, site, other);
                if clipped.len() != polygon.len() || clipped.iter().zip(polygon.iter()).any(|(a, b)| (*a - *b).length() > 1e-6) {
                    neighbors.push(j);
                }
                polygon = clipped;
                if polygon.is_empty() { break; }
            }

            cells.push(VoronoiCell {
                site,
                vertices: polygon,
                neighbor_indices: neighbors,
            });
        }

        Self {
            sites: sites.to_vec(),
            cells,
            bounds: (bounds_min, bounds_max),
        }
    }

    /// Find the nearest site to a point.
    pub fn nearest_site(&self, point: Vec2) -> usize {
        self.sites.iter().enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (**a - point).length_squared();
                let db = (**b - point).length_squared();
                da.partial_cmp(&db).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Cell area for a given site.
    pub fn cell_area(&self, index: usize) -> f32 {
        polygon_area(&self.cells[index].vertices)
    }
}

/// Clip a convex polygon by the perpendicular bisector of two points.
/// Keeps the half on the side of `keep`.
fn clip_polygon_by_bisector(polygon: &[Vec2], keep: Vec2, other: Vec2) -> Vec<Vec2> {
    if polygon.is_empty() { return Vec::new(); }
    let mid = (keep + other) * 0.5;
    let normal = (other - keep).normalize();

    let mut output = Vec::new();
    let n = polygon.len();

    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let da = (a - mid).dot(normal);
        let db = (b - mid).dot(normal);

        if da <= 0.0 { output.push(a); }
        if (da > 0.0) != (db > 0.0) {
            let t = da / (da - db);
            output.push(a + (b - a) * t);
        }
    }
    output
}

fn polygon_area(verts: &[Vec2]) -> f32 {
    let n = verts.len();
    if n < 3 { return 0.0; }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += verts[i].x * verts[j].y;
        area -= verts[j].x * verts[i].y;
    }
    area.abs() * 0.5
}

// ── Delaunay Triangulation ──────────────────────────────────────────────────

/// A triangle in the Delaunay triangulation (indices into the point array).
#[derive(Debug, Clone, Copy)]
pub struct DelaunayTriangle {
    pub a: usize,
    pub b: usize,
    pub c: usize,
}

/// 2D Delaunay triangulation using the Bowyer-Watson algorithm.
#[derive(Debug, Clone)]
pub struct DelaunayTriangulation {
    pub points: Vec<Vec2>,
    pub triangles: Vec<DelaunayTriangle>,
}

impl DelaunayTriangulation {
    /// Compute Delaunay triangulation of the given points.
    pub fn compute(points: &[Vec2]) -> Self {
        if points.len() < 3 {
            return Self { points: points.to_vec(), triangles: Vec::new() };
        }

        // Super-triangle that contains all points
        let (mut min, mut max) = (points[0], points[0]);
        for p in points {
            min = min.min(*p);
            max = max.max(*p);
        }
        let d = (max - min).max_element() * 10.0;
        let mid = (min + max) * 0.5;

        let mut all_points = vec![
            mid + Vec2::new(-d, -d),
            mid + Vec2::new(d, -d),
            mid + Vec2::new(0.0, d),
        ];
        let super_start = 0;
        all_points.extend_from_slice(points);

        let mut triangles = vec![DelaunayTriangle { a: 0, b: 1, c: 2 }];

        // Insert each point
        for pi in 3..all_points.len() {
            let p = all_points[pi];
            let mut bad_triangles = Vec::new();

            for (ti, tri) in triangles.iter().enumerate() {
                if circumcircle_contains(&all_points, tri, p) {
                    bad_triangles.push(ti);
                }
            }

            // Find boundary edges of the "hole"
            let mut boundary: Vec<(usize, usize)> = Vec::new();
            for &ti in &bad_triangles {
                let tri = &triangles[ti];
                let edges = [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)];
                for &(a, b) in &edges {
                    let shared = bad_triangles.iter().any(|&oti| {
                        if oti == ti { return false; }
                        let ot = &triangles[oti];
                        let oedges = [(ot.a, ot.b), (ot.b, ot.c), (ot.c, ot.a)];
                        oedges.iter().any(|&(oa, ob)| (oa == b && ob == a) || (oa == a && ob == b))
                    });
                    if !shared {
                        boundary.push((a, b));
                    }
                }
            }

            // Remove bad triangles (in reverse order to preserve indices)
            bad_triangles.sort_unstable_by(|a, b| b.cmp(a));
            for ti in bad_triangles {
                triangles.swap_remove(ti);
            }

            // Create new triangles from boundary edges to new point
            for (a, b) in boundary {
                triangles.push(DelaunayTriangle { a, b, c: pi });
            }
        }

        // Remove triangles that reference super-triangle vertices
        triangles.retain(|t| t.a >= 3 && t.b >= 3 && t.c >= 3);

        // Remap indices (subtract 3 to account for removed super-triangle)
        for t in &mut triangles {
            t.a -= 3;
            t.b -= 3;
            t.c -= 3;
        }

        Self {
            points: points.to_vec(),
            triangles,
        }
    }

    pub fn triangle_count(&self) -> usize { self.triangles.len() }
}

fn circumcircle_contains(points: &[Vec2], tri: &DelaunayTriangle, p: Vec2) -> bool {
    let a = points[tri.a];
    let b = points[tri.b];
    let c = points[tri.c];

    let ax = a.x - p.x;
    let ay = a.y - p.y;
    let bx = b.x - p.x;
    let by = b.y - p.y;
    let cx = c.x - p.x;
    let cy = c.y - p.y;

    let det = ax * (by * (cx * cx + cy * cy) - cy * (bx * bx + by * by))
            - bx * (ay * (cx * cx + cy * cy) - cy * (ax * ax + ay * ay))
            + cx * (ay * (bx * bx + by * by) - by * (ax * ax + ay * ay));

    det > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voronoi_creates_cells() {
        let sites = vec![Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(2.0, 3.0)];
        let vor = VoronoiDiagram::compute(&sites, Vec2::ZERO, Vec2::splat(4.0));
        assert_eq!(vor.cells.len(), 3);
        for cell in &vor.cells {
            assert!(!cell.vertices.is_empty());
        }
    }

    #[test]
    fn nearest_site_correct() {
        let sites = vec![Vec2::ZERO, Vec2::new(10.0, 0.0)];
        let vor = VoronoiDiagram::compute(&sites, Vec2::splat(-20.0), Vec2::splat(20.0));
        assert_eq!(vor.nearest_site(Vec2::new(1.0, 0.0)), 0);
        assert_eq!(vor.nearest_site(Vec2::new(9.0, 0.0)), 1);
    }

    #[test]
    fn delaunay_basic() {
        let points = vec![
            Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0),
        ];
        let dt = DelaunayTriangulation::compute(&points);
        assert!(dt.triangle_count() >= 2);
    }

    #[test]
    fn delaunay_too_few_points() {
        let dt = DelaunayTriangulation::compute(&[Vec2::ZERO, Vec2::X]);
        assert_eq!(dt.triangle_count(), 0);
    }
}
