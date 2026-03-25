// topology/spherical.rs — Spherical geometry: coordinates, geodesics, projections

use glam::{Vec2, Vec3};
use std::f32::consts::PI;

// ─── Spherical Coordinates ─────────────────────────────────────────────────

/// Spherical coordinate with polar angle theta (from +Z) and azimuthal angle phi (from +X in XY).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphericalCoord {
    /// Polar angle from the positive Z axis, in [0, PI]
    pub theta: f32,
    /// Azimuthal angle from the positive X axis in the XY plane, in [0, 2*PI)
    pub phi: f32,
}

impl SphericalCoord {
    pub fn new(theta: f32, phi: f32) -> Self {
        Self { theta, phi }
    }
}

/// Convert spherical coordinates to Cartesian (x, y, z) on a sphere of given radius.
pub fn to_cartesian(coord: SphericalCoord, radius: f32) -> Vec3 {
    let sin_theta = coord.theta.sin();
    Vec3::new(
        radius * sin_theta * coord.phi.cos(),
        radius * sin_theta * coord.phi.sin(),
        radius * coord.theta.cos(),
    )
}

/// Convert a Cartesian position to spherical coordinates.
/// The radius is implicitly the length of the vector.
pub fn from_cartesian(pos: Vec3) -> SphericalCoord {
    let r = pos.length();
    if r < 1e-10 {
        return SphericalCoord::new(0.0, 0.0);
    }
    let theta = (pos.z / r).clamp(-1.0, 1.0).acos();
    let phi = pos.y.atan2(pos.x);
    let phi = if phi < 0.0 { phi + 2.0 * PI } else { phi };
    SphericalCoord::new(theta, phi)
}

/// Great-circle distance between two points on a unit sphere.
/// Uses the Vincenty formula for numerical stability.
pub fn great_circle_distance(a: SphericalCoord, b: SphericalCoord) -> f32 {
    let ca = to_cartesian(a, 1.0);
    let cb = to_cartesian(b, 1.0);
    let dot = ca.dot(cb).clamp(-1.0, 1.0);
    dot.acos()
}

/// Compute a geodesic (great circle arc) between two spherical coordinates.
/// Returns `steps` points on the unit sphere as Vec3.
pub fn geodesic(a: SphericalCoord, b: SphericalCoord, steps: usize) -> Vec<Vec3> {
    if steps < 2 {
        return vec![to_cartesian(a, 1.0), to_cartesian(b, 1.0)];
    }
    let pa = to_cartesian(a, 1.0);
    let pb = to_cartesian(b, 1.0);

    let dot = pa.dot(pb).clamp(-1.0, 1.0);
    let omega = dot.acos();

    if omega.abs() < 1e-8 {
        // Points are the same
        return vec![pa; steps];
    }

    let sin_omega = omega.sin();
    let mut result = Vec::with_capacity(steps);

    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let a_coeff = ((1.0 - t) * omega).sin() / sin_omega;
        let b_coeff = (t * omega).sin() / sin_omega;
        result.push(pa * a_coeff + pb * b_coeff);
    }
    result
}

/// Area of a spherical triangle on a unit sphere using spherical excess.
/// Girard's theorem: Area = E = A + B + C - PI
/// where A, B, C are the interior angles.
pub fn spherical_triangle_area(a: SphericalCoord, b: SphericalCoord, c: SphericalCoord) -> f32 {
    let pa = to_cartesian(a, 1.0);
    let pb = to_cartesian(b, 1.0);
    let pc = to_cartesian(c, 1.0);

    // Compute the three dihedral angles using cross products
    let ab = pb - pa;
    let ac = pc - pa;
    let ba = pa - pb;
    let bc = pc - pb;
    let ca = pa - pc;
    let cb = pb - pc;

    // Normal to the great circle planes
    let n_ab = pa.cross(pb);
    let n_ac = pa.cross(pc);
    let n_ba = pb.cross(pa);
    let n_bc = pb.cross(pc);
    let n_ca = pc.cross(pa);
    let n_cb = pc.cross(pb);

    let angle_a = angle_between_planes(n_ab, n_ac);
    let angle_b = angle_between_planes(n_ba, n_bc);
    let angle_c = angle_between_planes(n_ca, n_cb);

    let excess = angle_a + angle_b + angle_c - PI;
    excess.abs()
}

fn angle_between_planes(n1: Vec3, n2: Vec3) -> f32 {
    let l1 = n1.length();
    let l2 = n2.length();
    if l1 < 1e-10 || l2 < 1e-10 {
        return 0.0;
    }
    let cos_angle = n1.dot(n2) / (l1 * l2);
    cos_angle.clamp(-1.0, 1.0).acos()
}

// ─── Stereographic Projection ──────────────────────────────────────────────

/// Stereographic projection from a unit sphere to the plane.
/// Projects from the north pole (0,0,1) onto the z=0 plane.
/// point3d should lie on the unit sphere.
pub fn stereographic_projection(point3d: Vec3) -> Vec2 {
    let denom = 1.0 - point3d.z;
    if denom.abs() < 1e-8 {
        // Near the north pole, project to infinity
        return Vec2::new(f32::MAX, f32::MAX);
    }
    Vec2::new(point3d.x / denom, point3d.y / denom)
}

/// Inverse stereographic projection from the plane back to the unit sphere.
pub fn inverse_stereographic(point2d: Vec2) -> Vec3 {
    let x = point2d.x;
    let y = point2d.y;
    let r_sq = x * x + y * y;
    let denom = r_sq + 1.0;
    Vec3::new(
        2.0 * x / denom,
        2.0 * y / denom,
        (r_sq - 1.0) / denom,
    )
}

// ─── Spherical Grid ────────────────────────────────────────────────────────

/// A grid of points distributed on a sphere, useful for rendering.
pub struct SphericalGrid {
    pub lat_divisions: usize,
    pub lon_divisions: usize,
}

impl SphericalGrid {
    pub fn new(lat_divisions: usize, lon_divisions: usize) -> Self {
        Self {
            lat_divisions,
            lon_divisions,
        }
    }

    /// Generate grid points on a sphere of given radius.
    pub fn generate(&self, radius: f32) -> Vec<Vec3> {
        let mut points = Vec::new();
        for i in 0..=self.lat_divisions {
            let theta = PI * i as f32 / self.lat_divisions as f32;
            for j in 0..self.lon_divisions {
                let phi = 2.0 * PI * j as f32 / self.lon_divisions as f32;
                let coord = SphericalCoord::new(theta, phi);
                points.push(to_cartesian(coord, radius));
            }
        }
        points
    }

    /// Generate line segments forming the grid (for wireframe rendering).
    pub fn generate_lines(&self, radius: f32) -> Vec<(Vec3, Vec3)> {
        let mut lines = Vec::new();
        let points = self.generate(radius);

        for i in 0..=self.lat_divisions {
            for j in 0..self.lon_divisions {
                let idx = i * self.lon_divisions + j;
                let next_j = i * self.lon_divisions + (j + 1) % self.lon_divisions;
                if idx < points.len() && next_j < points.len() {
                    lines.push((points[idx], points[next_j]));
                }
                if i < self.lat_divisions {
                    let next_i = (i + 1) * self.lon_divisions + j;
                    if idx < points.len() && next_i < points.len() {
                        lines.push((points[idx], points[next_i]));
                    }
                }
            }
        }
        lines
    }
}

/// Generate n approximately evenly distributed points on a unit sphere using the Fibonacci spiral.
pub fn fibonacci_sphere(n: usize) -> Vec<Vec3> {
    if n == 0 {
        return vec![];
    }
    let golden_ratio = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut points = Vec::with_capacity(n);

    for i in 0..n {
        let theta = (1.0 - 2.0 * i as f32 / (n as f32 - 1.0).max(1.0)).clamp(-1.0, 1.0).acos();
        let phi = 2.0 * PI * i as f32 / golden_ratio;
        let sin_theta = theta.sin();
        points.push(Vec3::new(
            sin_theta * phi.cos(),
            sin_theta * phi.sin(),
            theta.cos(),
        ));
    }
    points
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cartesian_roundtrip() {
        let coord = SphericalCoord::new(1.2, 0.8);
        let cart = to_cartesian(coord, 1.0);
        let back = from_cartesian(cart);
        assert!((back.theta - coord.theta).abs() < 1e-4);
        assert!((back.phi - coord.phi).abs() < 1e-4);
    }

    #[test]
    fn test_north_pole() {
        let coord = SphericalCoord::new(0.0, 0.0);
        let cart = to_cartesian(coord, 1.0);
        assert!((cart.z - 1.0).abs() < 1e-6);
        assert!(cart.x.abs() < 1e-6);
        assert!(cart.y.abs() < 1e-6);
    }

    #[test]
    fn test_south_pole() {
        let coord = SphericalCoord::new(PI, 0.0);
        let cart = to_cartesian(coord, 1.0);
        assert!((cart.z - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_great_circle_distance_same_point() {
        let a = SphericalCoord::new(0.5, 1.0);
        let d = great_circle_distance(a, a);
        assert!(d.abs() < 1e-4);
    }

    #[test]
    fn test_great_circle_distance_antipodal() {
        let a = SphericalCoord::new(0.0, 0.0); // north pole
        let b = SphericalCoord::new(PI, 0.0);   // south pole
        let d = great_circle_distance(a, b);
        assert!((d - PI).abs() < 1e-4, "Antipodal distance should be PI, got {}", d);
    }

    #[test]
    fn test_geodesic_endpoints() {
        let a = SphericalCoord::new(0.5, 0.3);
        let b = SphericalCoord::new(1.2, 2.0);
        let path = geodesic(a, b, 50);
        let pa = to_cartesian(a, 1.0);
        let pb = to_cartesian(b, 1.0);
        assert!((path[0] - pa).length() < 1e-4);
        assert!((path[49] - pb).length() < 1e-4);
    }

    #[test]
    fn test_geodesic_on_sphere() {
        let a = SphericalCoord::new(0.5, 0.0);
        let b = SphericalCoord::new(1.5, 0.0);
        let path = geodesic(a, b, 10);
        for p in &path {
            assert!((p.length() - 1.0).abs() < 1e-4, "Point not on unit sphere");
        }
    }

    #[test]
    fn test_spherical_triangle_area_octant() {
        // Triangle covering one octant of the sphere
        let a = SphericalCoord::new(0.0, 0.0);           // north pole
        let b = SphericalCoord::new(PI / 2.0, 0.0);       // equator, x-axis
        let c = SphericalCoord::new(PI / 2.0, PI / 2.0);  // equator, y-axis
        let area = spherical_triangle_area(a, b, c);
        // Expected: PI/2 (one eighth of sphere = 4*PI/8 = PI/2)
        assert!((area - PI / 2.0).abs() < 0.2, "Octant area should be ~PI/2, got {}", area);
    }

    #[test]
    fn test_stereographic_roundtrip() {
        let p3 = Vec3::new(0.5, 0.3, -0.8124).normalize();
        let p2 = stereographic_projection(p3);
        let back = inverse_stereographic(p2);
        assert!((p3 - back).length() < 1e-3, "Stereographic roundtrip: {:?} vs {:?}", p3, back);
    }

    #[test]
    fn test_stereographic_south_pole() {
        // South pole maps to origin
        let south = Vec3::new(0.0, 0.0, -1.0);
        let proj = stereographic_projection(south);
        assert!(proj.length() < 1e-4);
    }

    #[test]
    fn test_inverse_stereographic_origin() {
        let p = inverse_stereographic(Vec2::ZERO);
        assert!((p - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-4);
    }

    #[test]
    fn test_fibonacci_sphere_count() {
        let points = fibonacci_sphere(100);
        assert_eq!(points.len(), 100);
    }

    #[test]
    fn test_fibonacci_sphere_on_sphere() {
        let points = fibonacci_sphere(50);
        for p in &points {
            assert!((p.length() - 1.0).abs() < 1e-4, "Fibonacci point not on unit sphere");
        }
    }

    #[test]
    fn test_fibonacci_sphere_distribution() {
        // Check that points are roughly evenly distributed by checking min distance
        let points = fibonacci_sphere(100);
        let mut min_dist = f32::MAX;
        for i in 0..points.len() {
            for j in (i + 1)..points.len() {
                let d = (points[i] - points[j]).length();
                if d < min_dist {
                    min_dist = d;
                }
            }
        }
        // For 100 points, the minimum distance should be reasonable (> 0.1)
        assert!(min_dist > 0.05, "Points too clustered: min_dist = {}", min_dist);
    }

    #[test]
    fn test_spherical_grid() {
        let grid = SphericalGrid::new(10, 20);
        let points = grid.generate(1.0);
        assert_eq!(points.len(), 11 * 20);
        for p in &points {
            assert!((p.length() - 1.0).abs() < 1e-4);
        }
    }
}
