// topology/hyperbolic.rs — Hyperbolic space rendering (Poincare disk, Klein disk, tilings)

use glam::Vec2;
use std::f32::consts::PI;

// ─── Poincare Disk Model ───────────────────────────────────────────────────

/// Maps world positions into the Poincare disk model of the hyperbolic plane.
/// Points inside the unit disk represent the entire hyperbolic plane.
pub struct PoincareDisk {
    /// Scale factor mapping world units to disk radius
    pub scale: f32,
}

impl PoincareDisk {
    pub fn new(scale: f32) -> Self {
        Self { scale }
    }

    /// Convert a world position to Poincare disk coordinates.
    /// Uses the exponential map: disk_pos = tanh(|p|/2) * (p/|p|)
    pub fn to_disk(&self, world_pos: Vec2) -> Vec2 {
        let p = world_pos / self.scale;
        let r = p.length();
        if r < 1e-8 {
            return Vec2::ZERO;
        }
        let disk_r = (r / 2.0).tanh();
        p.normalize() * disk_r
    }

    /// Convert a Poincare disk coordinate back to world position.
    /// Inverse of to_disk: world = 2 * atanh(|d|) * (d/|d|) * scale
    pub fn from_disk(&self, disk_pos: Vec2) -> Vec2 {
        let r = disk_pos.length();
        if r < 1e-8 {
            return Vec2::ZERO;
        }
        if r >= 1.0 {
            // Clamp to boundary
            let clamped_r = 0.9999;
            let world_r = 2.0 * atanh(clamped_r);
            return disk_pos.normalize() * world_r * self.scale;
        }
        let world_r = 2.0 * atanh(r);
        disk_pos.normalize() * world_r * self.scale
    }

    /// Compute the hyperbolic distance between two points in the Poincare disk.
    /// d(a,b) = acosh(1 + 2|a-b|^2 / ((1-|a|^2)(1-|b|^2)))
    pub fn hyperbolic_distance(&self, a: Vec2, b: Vec2) -> f32 {
        let a_sq = a.length_squared();
        let b_sq = b.length_squared();
        let diff_sq = (a - b).length_squared();

        let denom = (1.0 - a_sq) * (1.0 - b_sq);
        if denom <= 0.0 {
            return f32::INFINITY;
        }
        let arg = 1.0 + 2.0 * diff_sq / denom;
        acosh(arg)
    }

    /// Compute a geodesic (hyperbolic line) between two points on the disk.
    /// In the Poincare model geodesics are circular arcs orthogonal to the boundary,
    /// or diameters through the origin.
    pub fn geodesic(&self, a: Vec2, b: Vec2, steps: usize) -> Vec<Vec2> {
        if steps < 2 {
            return vec![a, b];
        }

        // Use Mobius transforms: translate a to origin, interpolate along diameter, translate back.
        let mut result = Vec::with_capacity(steps);
        // Map a to origin via Mobius transform with parameter -a
        let neg_a = -a;
        let b_mapped = mobius_transform_impl(b, neg_a);

        // b_mapped now lies on a diameter through origin — geodesic is a straight line
        for i in 0..steps {
            let t = i as f32 / (steps - 1) as f32;
            let point_on_line = b_mapped * t;
            // Map back
            result.push(mobius_transform_impl(point_on_line, a));
        }
        result
    }

    /// Apply a Mobius transform (isometry of the Poincare disk).
    /// T_a(z) = (z + a) / (1 + conj(a)*z)
    /// treating Vec2 as complex numbers.
    pub fn mobius_transform(&self, z: Vec2, a: Vec2) -> Vec2 {
        mobius_transform_impl(z, a)
    }
}

/// Complex multiply: (a+bi)(c+di) = (ac-bd) + (ad+bc)i
fn complex_mul(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x)
}

/// Complex conjugate
fn complex_conj(a: Vec2) -> Vec2 {
    Vec2::new(a.x, -a.y)
}

/// Complex division: a / b
fn complex_div(a: Vec2, b: Vec2) -> Vec2 {
    let denom = b.x * b.x + b.y * b.y;
    if denom < 1e-12 {
        return Vec2::ZERO;
    }
    Vec2::new(
        (a.x * b.x + a.y * b.y) / denom,
        (a.y * b.x - a.x * b.y) / denom,
    )
}

/// Mobius transform T_a(z) = (z + a) / (1 + conj(a)*z)
fn mobius_transform_impl(z: Vec2, a: Vec2) -> Vec2 {
    let numerator = z + a;
    let denominator = Vec2::new(1.0, 0.0) + complex_mul(complex_conj(a), z);
    complex_div(numerator, denominator)
}

fn atanh(x: f32) -> f32 {
    0.5 * ((1.0 + x) / (1.0 - x)).ln()
}

fn acosh(x: f32) -> f32 {
    (x + (x * x - 1.0).sqrt()).ln()
}

// ─── Klein Disk Model ──────────────────────────────────────────────────────

/// The Klein (Beltrami-Klein) disk model of hyperbolic geometry.
/// Geodesics are straight lines (chords) but angles are distorted.
pub struct KleinDisk;

impl KleinDisk {
    /// Convert a world position to Klein disk coordinates.
    /// Points map into the unit disk.
    pub fn to_klein(&self, pos: Vec2) -> Vec2 {
        let r = pos.length();
        if r < 1e-8 {
            return Vec2::ZERO;
        }
        let klein_r = (r / 2.0).tanh() * 2.0 / (1.0 + (r / 2.0).tanh().powi(2));
        // Simplification: klein_r = tanh(r) for the Klein model
        let klein_r = r.tanh();
        pos.normalize() * klein_r
    }

    /// Convert Klein disk coordinates back to world position.
    pub fn from_klein(&self, pos: Vec2) -> Vec2 {
        let r = pos.length();
        if r < 1e-8 {
            return Vec2::ZERO;
        }
        if r >= 1.0 {
            return pos.normalize() * 10.0; // clamp
        }
        let world_r = atanh(r);
        pos.normalize() * world_r
    }

    /// Convert a Poincare disk point to Klein disk point.
    /// K = 2P / (1 + |P|^2)
    pub fn poincare_to_klein(pos: Vec2) -> Vec2 {
        let r_sq = pos.length_squared();
        pos * (2.0 / (1.0 + r_sq))
    }

    /// Convert a Klein disk point to Poincare disk point.
    /// P = K / (1 + sqrt(1 - |K|^2))
    pub fn klein_to_poincare(pos: Vec2) -> Vec2 {
        let r_sq = pos.length_squared();
        if r_sq >= 1.0 {
            return pos.normalize() * 0.9999;
        }
        let factor = 1.0 + (1.0 - r_sq).sqrt();
        pos / factor
    }
}

// ─── Hyperbolic Polygon & Tiling ───────────────────────────────────────────

/// A polygon in hyperbolic space, represented in the Poincare disk.
#[derive(Clone, Debug)]
pub struct HyperbolicPolygon {
    pub vertices: Vec<Vec2>,
    pub center: Vec2,
}

/// Generate regular tilings {p,q} in hyperbolic space.
/// A {p,q} tiling has regular p-gons with q meeting at each vertex.
/// The tiling is hyperbolic when (p-2)(q-2) > 4.
pub struct HyperbolicTiling;

impl HyperbolicTiling {
    /// Generate a {p,q} tiling up to the given depth (number of layers from center).
    /// Returns polygons in Poincare disk coordinates.
    pub fn generate(p: usize, q: usize, depth: usize) -> Vec<HyperbolicPolygon> {
        if p < 3 || q < 3 {
            return vec![];
        }
        // Check that tiling is hyperbolic
        if (p - 2) * (q - 2) <= 4 {
            return vec![]; // Euclidean or spherical, not hyperbolic
        }

        let mut polygons = Vec::new();
        let mut visited_centers: Vec<Vec2> = Vec::new();

        // Compute the radius of the central polygon's circumscribed circle in the Poincare disk
        let central_radius = central_polygon_radius(p, q);

        // Create central polygon
        let central = create_regular_polygon(Vec2::ZERO, central_radius, p, 0.0);
        polygons.push(central.clone());
        visited_centers.push(Vec2::ZERO);

        if depth == 0 {
            return polygons;
        }

        // BFS expansion
        let mut frontier: Vec<HyperbolicPolygon> = vec![central];

        for _layer in 0..depth {
            let mut next_frontier = Vec::new();
            for poly in &frontier {
                // For each edge, try to place an adjacent polygon
                let n = poly.vertices.len();
                for i in 0..n {
                    let v1 = poly.vertices[i];
                    let v2 = poly.vertices[(i + 1) % n];
                    let edge_mid = (v1 + v2) * 0.5;

                    // Reflect center through this edge to get neighbor center
                    let neighbor_center = reflect_through_geodesic(poly.center, v1, v2);

                    if neighbor_center.length() >= 0.98 {
                        continue; // too close to boundary
                    }

                    // Check if we already have a polygon near this center
                    let dominated = visited_centers.iter().any(|c| (*c - neighbor_center).length() < 0.01);
                    if dominated {
                        continue;
                    }

                    // Create new polygon via Mobius transform
                    // Move origin to neighbor center, create polygon, move back
                    let new_poly = create_polygon_at(neighbor_center, central_radius, p, v1, v2);
                    visited_centers.push(neighbor_center);
                    polygons.push(new_poly.clone());
                    next_frontier.push(new_poly);
                }
            }
            frontier = next_frontier;
        }

        polygons
    }
}

/// Compute the Poincare disk radius of the central polygon in a {p,q} tiling.
fn central_polygon_radius(p: usize, q: usize) -> f32 {
    let pi_p = PI / p as f32;
    let pi_q = PI / q as f32;
    // The circumradius in the hyperbolic plane satisfies:
    // cosh(R) = cos(pi/q) / sin(pi/p)
    let cosh_r = pi_q.cos() / pi_p.sin();
    let hyp_r = acosh(cosh_r);
    // Convert hyperbolic distance to Poincare disk radius: r = tanh(R/2)
    (hyp_r / 2.0).tanh()
}

/// Create a regular p-gon centered at `center` with circumradius `radius` in disk coords.
fn create_regular_polygon(center: Vec2, radius: f32, p: usize, rotation: f32) -> HyperbolicPolygon {
    let mut vertices = Vec::with_capacity(p);
    for i in 0..p {
        let angle = rotation + 2.0 * PI * i as f32 / p as f32;
        let vertex_local = Vec2::new(angle.cos(), angle.sin()) * radius;
        // Translate from origin to center using Mobius transform
        let vertex = mobius_transform_impl(vertex_local, center);
        vertices.push(vertex);
    }
    HyperbolicPolygon { vertices, center }
}

/// Create a polygon at a given center, oriented to share the edge (v1, v2) with its neighbor.
fn create_polygon_at(center: Vec2, radius: f32, p: usize, v1: Vec2, v2: Vec2) -> HyperbolicPolygon {
    // Determine the rotation so two of the polygon's vertices coincide with v1 and v2
    let edge_mid = (v1 + v2) * 0.5;
    let to_edge = edge_mid - center;
    let base_angle = to_edge.y.atan2(to_edge.x);
    let rotation = base_angle - PI / p as f32;

    create_regular_polygon(center, radius, p, rotation)
}

/// Reflect point `p` through the hyperbolic geodesic defined by points a, b on the Poincare disk.
fn reflect_through_geodesic(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    // Move a to origin
    let neg_a = -a;
    let b_m = mobius_transform_impl(b, neg_a);
    let p_m = mobius_transform_impl(p, neg_a);

    // Now geodesic through origin and b_m is a straight line (diameter)
    // Reflect p_m through this line
    let angle = b_m.y.atan2(b_m.x);
    let cos2 = (2.0 * angle).cos();
    let sin2 = (2.0 * angle).sin();
    let reflected = Vec2::new(
        p_m.x * cos2 + p_m.y * sin2,
        p_m.x * sin2 - p_m.y * cos2,
    );

    // Move back
    mobius_transform_impl(reflected, a)
}

// ─── Hyperbolic Grid ───────────────────────────────────────────────────────

/// A deformed grid for rendering game elements in hyperbolic space.
/// Space expands exponentially as you move away from the center.
pub struct HyperbolicGrid {
    pub disk: PoincareDisk,
    pub grid_lines: usize,
}

impl HyperbolicGrid {
    pub fn new(scale: f32, grid_lines: usize) -> Self {
        Self {
            disk: PoincareDisk::new(scale),
            grid_lines,
        }
    }

    /// Generate a deformed grid of points in the Poincare disk model.
    /// Returns pairs of points representing line segments.
    pub fn generate_grid(&self) -> Vec<(Vec2, Vec2)> {
        let mut segments = Vec::new();
        let n = self.grid_lines;
        let step = 2.0 * self.disk.scale / n as f32;

        // Horizontal lines
        for j in 0..=n {
            let y = -self.disk.scale + j as f32 * step;
            let mut prev = None;
            for i in 0..=n * 4 {
                let x = -self.disk.scale + i as f32 * step / 4.0;
                let world = Vec2::new(x, y);
                let disk = self.disk.to_disk(world);
                if disk.length() < 0.99 {
                    if let Some(p) = prev {
                        segments.push((p, disk));
                    }
                    prev = Some(disk);
                } else {
                    prev = None;
                }
            }
        }

        // Vertical lines
        for i in 0..=n {
            let x = -self.disk.scale + i as f32 * step;
            let mut prev = None;
            for j in 0..=n * 4 {
                let y = -self.disk.scale + j as f32 * step / 4.0;
                let world = Vec2::new(x, y);
                let disk = self.disk.to_disk(world);
                if disk.length() < 0.99 {
                    if let Some(p) = prev {
                        segments.push((p, disk));
                    }
                    prev = Some(disk);
                } else {
                    prev = None;
                }
            }
        }

        segments
    }

    /// Transform an entity position from world space to hyperbolic disk space.
    pub fn transform_entity(&self, world_pos: Vec2) -> Vec2 {
        self.disk.to_disk(world_pos)
    }

    /// Transform multiple entity positions.
    pub fn transform_entities(&self, positions: &[Vec2]) -> Vec<Vec2> {
        positions.iter().map(|p| self.disk.to_disk(*p)).collect()
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poincare_roundtrip() {
        let disk = PoincareDisk::new(10.0);
        let world = Vec2::new(3.0, 4.0);
        let d = disk.to_disk(world);
        let back = disk.from_disk(d);
        assert!((world - back).length() < 0.01, "Roundtrip failed: {:?} vs {:?}", world, back);
    }

    #[test]
    fn test_poincare_origin() {
        let disk = PoincareDisk::new(1.0);
        let d = disk.to_disk(Vec2::ZERO);
        assert!(d.length() < 1e-6);
    }

    #[test]
    fn test_disk_points_inside_unit_circle() {
        let disk = PoincareDisk::new(5.0);
        for x in -10..=10 {
            for y in -10..=10 {
                let d = disk.to_disk(Vec2::new(x as f32, y as f32));
                assert!(d.length() < 1.0 + 1e-6, "Point outside disk: {:?}", d);
            }
        }
    }

    #[test]
    fn test_hyperbolic_distance_zero() {
        let disk = PoincareDisk::new(1.0);
        let a = Vec2::new(0.3, 0.2);
        let d = disk.hyperbolic_distance(a, a);
        assert!(d.abs() < 1e-4, "Self-distance should be ~0, got {}", d);
    }

    #[test]
    fn test_hyperbolic_distance_symmetry() {
        let disk = PoincareDisk::new(1.0);
        let a = Vec2::new(0.3, 0.1);
        let b = Vec2::new(-0.2, 0.4);
        let d1 = disk.hyperbolic_distance(a, b);
        let d2 = disk.hyperbolic_distance(b, a);
        assert!((d1 - d2).abs() < 1e-4, "Distance not symmetric: {} vs {}", d1, d2);
    }

    #[test]
    fn test_mobius_preserves_distance() {
        let disk = PoincareDisk::new(1.0);
        let a = Vec2::new(0.2, 0.1);
        let b = Vec2::new(-0.1, 0.3);
        let c = Vec2::new(0.15, -0.05);

        let d_before = disk.hyperbolic_distance(a, b);
        let a_t = disk.mobius_transform(a, c);
        let b_t = disk.mobius_transform(b, c);
        let d_after = disk.hyperbolic_distance(a_t, b_t);

        assert!((d_before - d_after).abs() < 0.05,
            "Mobius should preserve distance: {} vs {}", d_before, d_after);
    }

    #[test]
    fn test_geodesic_endpoints() {
        let disk = PoincareDisk::new(1.0);
        let a = Vec2::new(0.2, 0.1);
        let b = Vec2::new(-0.3, 0.2);
        let path = disk.geodesic(a, b, 20);
        assert!((path[0] - a).length() < 1e-4);
        assert!((path[path.len() - 1] - b).length() < 0.05);
    }

    #[test]
    fn test_klein_poincare_roundtrip() {
        let p = Vec2::new(0.3, 0.2);
        let k = KleinDisk::poincare_to_klein(p);
        let back = KleinDisk::klein_to_poincare(k);
        assert!((p - back).length() < 1e-4, "Klein-Poincare roundtrip failed");
    }

    #[test]
    fn test_hyperbolic_tiling_basic() {
        // {5,4} tiling (pentagons, 4 at each vertex)
        let polygons = HyperbolicTiling::generate(5, 4, 1);
        assert!(!polygons.is_empty(), "Tiling should produce polygons");
        for poly in &polygons {
            assert_eq!(poly.vertices.len(), 5);
            for v in &poly.vertices {
                assert!(v.length() < 1.0 + 1e-3, "Vertex outside disk");
            }
        }
    }

    #[test]
    fn test_hyperbolic_tiling_not_hyperbolic() {
        // {4,4} is Euclidean, should return empty
        let polygons = HyperbolicTiling::generate(4, 4, 2);
        assert!(polygons.is_empty());
    }

    #[test]
    fn test_hyperbolic_grid_generation() {
        let grid = HyperbolicGrid::new(5.0, 4);
        let segments = grid.generate_grid();
        assert!(!segments.is_empty());
        for (a, b) in &segments {
            assert!(a.length() < 1.0);
            assert!(b.length() < 1.0);
        }
    }

    #[test]
    fn test_complex_operations() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        let prod = complex_mul(a, b);
        // (1+2i)(3+4i) = 3+4i+6i+8i^2 = -5+10i
        assert!((prod.x - (-5.0)).abs() < 1e-6);
        assert!((prod.y - 10.0).abs() < 1e-6);
    }
}
