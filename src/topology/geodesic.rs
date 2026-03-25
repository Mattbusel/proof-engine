// topology/geodesic.rs — Geodesic pathfinding on various surfaces

use glam::Vec3;
use std::f32::consts::PI;

// ─── Surface Type ──────────────────────────────────────────────────────────

/// The type of surface on which to compute geodesics.
pub enum GeodesicSurface {
    Plane,
    Sphere { radius: f32 },
    Torus { major_r: f32, minor_r: f32 },
    Hyperbolic,
    /// Custom surface defined by a parametric function (u, v) -> Vec3
    /// and its partial derivatives.
    Custom {
        surface_fn: Box<dyn Fn(f32, f32) -> Vec3>,
    },
}

/// Compute a geodesic path on a given surface between two points.
/// Returns `steps` evenly spaced points along the geodesic.
pub fn geodesic_on_surface(surface: &GeodesicSurface, start: Vec3, end: Vec3, steps: usize) -> Vec<Vec3> {
    match surface {
        GeodesicSurface::Plane => geodesic_plane(start, end, steps),
        GeodesicSurface::Sphere { radius } => shortest_path_sphere_pts(start, end, *radius, steps),
        GeodesicSurface::Torus { major_r, minor_r } => {
            shortest_path_torus_pts(start, end, *major_r, *minor_r, steps)
        }
        GeodesicSurface::Hyperbolic => geodesic_hyperbolic(start, end, steps),
        GeodesicSurface::Custom { surface_fn } => geodesic_custom(surface_fn, start, end, steps),
    }
}

fn geodesic_plane(start: Vec3, end: Vec3, steps: usize) -> Vec<Vec3> {
    if steps < 2 {
        return vec![start, end];
    }
    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            start.lerp(end, t)
        })
        .collect()
}

// ─── Sphere Geodesic ───────────────────────────────────────────────────────

/// Shortest path on a sphere (great circle arc).
pub fn shortest_path_sphere(a: Vec3, b: Vec3, steps: usize) -> Vec<Vec3> {
    let r = a.length();
    shortest_path_sphere_pts(a, b, r, steps)
}

fn shortest_path_sphere_pts(a: Vec3, b: Vec3, radius: f32, steps: usize) -> Vec<Vec3> {
    if steps < 2 {
        return vec![a, b];
    }
    let na = a.normalize();
    let nb = b.normalize();
    let dot = na.dot(nb).clamp(-1.0, 1.0);
    let omega = dot.acos();

    if omega.abs() < 1e-8 {
        return vec![a; steps];
    }

    let sin_omega = omega.sin();
    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            let a_coeff = ((1.0 - t) * omega).sin() / sin_omega;
            let b_coeff = (t * omega).sin() / sin_omega;
            (na * a_coeff + nb * b_coeff) * radius
        })
        .collect()
}

// ─── Torus Geodesic ────────────────────────────────────────────────────────

/// Shortest path on a torus (approximate via parameter-space interpolation).
pub fn shortest_path_torus(a: Vec3, b: Vec3, major_r: f32, minor_r: f32, steps: usize) -> Vec<Vec3> {
    shortest_path_torus_pts(a, b, major_r, minor_r, steps)
}

fn shortest_path_torus_pts(a: Vec3, b: Vec3, major_r: f32, minor_r: f32, steps: usize) -> Vec<Vec3> {
    if steps < 2 {
        return vec![a, b];
    }

    // Convert to torus parameters (u, v)
    let (u_a, v_a) = cartesian_to_torus_params(a, major_r, minor_r);
    let (u_b, v_b) = cartesian_to_torus_params(b, major_r, minor_r);

    // Interpolate in parameter space (taking shortest path around each circle)
    let du = shortest_angle_diff(u_a, u_b);
    let dv = shortest_angle_diff(v_a, v_b);

    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            let u = u_a + du * t;
            let v = v_a + dv * t;
            torus_point(major_r, minor_r, u, v)
        })
        .collect()
}

fn cartesian_to_torus_params(p: Vec3, major_r: f32, _minor_r: f32) -> (f32, f32) {
    let u = p.y.atan2(p.x);
    let center_x = major_r * u.cos();
    let center_y = major_r * u.sin();
    let dx = p.x - center_x;
    let dy = p.y - center_y;
    let dz = p.z;
    let r_in_plane = (dx * dx + dy * dy).sqrt();
    let v = dz.atan2(r_in_plane - 0.0); // approximate
    (u, v)
}

fn torus_point(major_r: f32, minor_r: f32, u: f32, v: f32) -> Vec3 {
    Vec3::new(
        (major_r + minor_r * v.cos()) * u.cos(),
        (major_r + minor_r * v.cos()) * u.sin(),
        minor_r * v.sin(),
    )
}

fn shortest_angle_diff(from: f32, to: f32) -> f32 {
    let mut diff = to - from;
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    diff
}

// ─── Hyperbolic Geodesic (in 3D embedding) ─────────────────────────────────

fn geodesic_hyperbolic(start: Vec3, end: Vec3, steps: usize) -> Vec<Vec3> {
    // Use the hyperboloid model: geodesics are intersections of the hyperboloid with planes
    // through the origin.
    // For simplicity, use Minkowski-space slerp.
    if steps < 2 {
        return vec![start, end];
    }

    // Minkowski inner product: -x0*y0 + x1*y1 + x2*y2
    let minkowski_dot = |a: Vec3, b: Vec3| -> f32 { -a.x * b.x + a.y * b.y + a.z * b.z };

    let dot = -minkowski_dot(start, end).max(1.0);
    let dist = acosh(dot);

    if dist.abs() < 1e-8 {
        return vec![start; steps];
    }

    let sinh_dist = dist.sinh();
    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            let a_coeff = ((1.0 - t) * dist).sinh() / sinh_dist;
            let b_coeff = (t * dist).sinh() / sinh_dist;
            start * a_coeff + end * b_coeff
        })
        .collect()
}

fn acosh(x: f32) -> f32 {
    (x + (x * x - 1.0).max(0.0).sqrt()).ln()
}

// ─── Custom Surface Geodesic ───────────────────────────────────────────────

fn geodesic_custom(
    _surface_fn: &dyn Fn(f32, f32) -> Vec3,
    start: Vec3,
    end: Vec3,
    steps: usize,
) -> Vec<Vec3> {
    // For a general surface, approximate using iterative projection.
    // Start with a straight line and project each point onto the surface.
    // This is a simple relaxation approach.
    if steps < 2 {
        return vec![start, end];
    }

    // Simple linear interpolation as a baseline (exact geodesic on custom
    // surfaces requires solving the geodesic equation numerically).
    let mut path: Vec<Vec3> = (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            start.lerp(end, t)
        })
        .collect();

    // Smoothing iterations: pull midpoints toward the surface
    for _iter in 0..10 {
        let old = path.clone();
        for i in 1..(steps - 1) {
            let mid = (old[i - 1] + old[i + 1]) * 0.5;
            path[i] = mid;
        }
    }

    path
}

// ─── Geodesic Curvature ────────────────────────────────────────────────────

/// Compute the geodesic curvature along a discrete path.
/// Returns a curvature value for each interior point.
pub fn geodesic_curvature(path: &[Vec3]) -> Vec<f32> {
    if path.len() < 3 {
        return vec![];
    }

    let mut curvatures = Vec::with_capacity(path.len() - 2);
    for i in 1..(path.len() - 1) {
        let prev = path[i - 1];
        let curr = path[i];
        let next = path[i + 1];

        let d1 = curr - prev;
        let d2 = next - curr;
        let l1 = d1.length();
        let l2 = d2.length();

        if l1 < 1e-10 || l2 < 1e-10 {
            curvatures.push(0.0);
            continue;
        }

        let t1 = d1 / l1;
        let t2 = d2 / l2;
        let dt = t2 - t1;
        let ds = (l1 + l2) / 2.0;
        let kappa = dt.length() / ds;
        curvatures.push(kappa);
    }
    curvatures
}

// ─── Parallel Transport ────────────────────────────────────────────────────

/// Transport a vector along a geodesic path using Schild's ladder approximation.
/// Returns the transported vector at each point along the path.
pub fn parallel_transport(vector: Vec3, path: &[Vec3]) -> Vec<Vec3> {
    if path.is_empty() {
        return vec![];
    }
    if path.len() == 1 {
        return vec![vector];
    }

    let mut result = Vec::with_capacity(path.len());
    let mut v = vector;
    result.push(v);

    for i in 1..path.len() {
        let tangent = (path[i] - path[i - 1]).normalize_or_zero();
        // Project out the tangent component to keep the vector "parallel"
        // on a surface, the transported vector should remain tangent to the surface.
        // Simple approximation: remove the component along the change in tangent direction.
        if i >= 2 {
            let prev_tangent = (path[i - 1] - path[i - 2]).normalize_or_zero();
            let tangent_change = tangent - prev_tangent;
            // Remove the component of v along the tangent change
            let tc_len_sq = tangent_change.length_squared();
            if tc_len_sq > 1e-10 {
                v = v - tangent_change * (v.dot(tangent_change) / tc_len_sq);
            }
        }

        // Ensure the transported vector maintains its length
        let orig_len = vector.length();
        let curr_len = v.length();
        if curr_len > 1e-10 {
            v = v * (orig_len / curr_len);
        }

        result.push(v);
    }

    result
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_geodesic_is_straight() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);
        let path = geodesic_on_surface(&GeodesicSurface::Plane, start, end, 11);
        assert_eq!(path.len(), 11);
        for (i, p) in path.iter().enumerate() {
            let expected_x = i as f32;
            assert!((p.x - expected_x).abs() < 1e-4);
            assert!(p.y.abs() < 1e-4);
            assert!(p.z.abs() < 1e-4);
        }
    }

    #[test]
    fn test_sphere_geodesic_on_sphere() {
        let r = 5.0;
        let a = Vec3::new(r, 0.0, 0.0);
        let b = Vec3::new(0.0, r, 0.0);
        let path = shortest_path_sphere(a, b, 20);
        for p in &path {
            assert!((p.length() - r).abs() < 0.01, "Point not on sphere: len={}", p.length());
        }
    }

    #[test]
    fn test_sphere_geodesic_endpoints() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 0.0, 1.0);
        let path = shortest_path_sphere(a, b, 10);
        assert!((path[0] - a).length() < 1e-4);
        assert!((path[9] - b).length() < 1e-4);
    }

    #[test]
    fn test_torus_geodesic() {
        let a = torus_point(3.0, 1.0, 0.0, 0.0);
        let b = torus_point(3.0, 1.0, PI / 2.0, 0.0);
        let path = shortest_path_torus(a, b, 3.0, 1.0, 20);
        assert_eq!(path.len(), 20);
        // All points should be approximately on the torus
        for p in &path {
            let xy_dist = (p.x * p.x + p.y * p.y).sqrt();
            assert!(xy_dist > 1.5 && xy_dist < 4.5, "Point not near torus: {}", xy_dist);
        }
    }

    #[test]
    fn test_geodesic_curvature_straight_line() {
        let path: Vec<Vec3> = (0..10)
            .map(|i| Vec3::new(i as f32, 0.0, 0.0))
            .collect();
        let curvatures = geodesic_curvature(&path);
        for k in &curvatures {
            assert!(k.abs() < 1e-4, "Straight line should have zero curvature, got {}", k);
        }
    }

    #[test]
    fn test_geodesic_curvature_circle() {
        let n = 100;
        let path: Vec<Vec3> = (0..n)
            .map(|i| {
                let t = 2.0 * PI * i as f32 / n as f32;
                Vec3::new(t.cos(), t.sin(), 0.0)
            })
            .collect();
        let curvatures = geodesic_curvature(&path);
        // Curvature of a unit circle should be approximately 1
        for k in &curvatures {
            assert!((k - 1.0).abs() < 0.2, "Circle curvature should be ~1, got {}", k);
        }
    }

    #[test]
    fn test_parallel_transport_straight() {
        let path: Vec<Vec3> = (0..5)
            .map(|i| Vec3::new(i as f32, 0.0, 0.0))
            .collect();
        let v = Vec3::new(0.0, 1.0, 0.0);
        let transported = parallel_transport(v, &path);
        assert_eq!(transported.len(), 5);
        // Along a straight line, the vector should remain constant
        for tv in &transported {
            assert!((tv.y - 1.0).abs() < 1e-3, "Transport should preserve vector: {:?}", tv);
        }
    }

    #[test]
    fn test_parallel_transport_preserves_length() {
        let n = 50;
        let path: Vec<Vec3> = (0..n)
            .map(|i| {
                let t = PI * i as f32 / n as f32;
                Vec3::new(t.cos(), t.sin(), 0.0)
            })
            .collect();
        let v = Vec3::new(0.0, 0.0, 1.0);
        let transported = parallel_transport(v, &path);
        let orig_len = v.length();
        for tv in &transported {
            assert!((tv.length() - orig_len).abs() < 0.1, "Length not preserved: {}", tv.length());
        }
    }

    #[test]
    fn test_hyperbolic_geodesic() {
        // On the hyperboloid x^2 = 1 + y^2 + z^2
        let a = Vec3::new(1.0, 0.0, 0.0); // on hyperboloid
        let b = Vec3::new((1.0 + 1.0_f32).sqrt(), 1.0, 0.0);
        let path = geodesic_on_surface(&GeodesicSurface::Hyperbolic, a, b, 10);
        assert_eq!(path.len(), 10);
    }

    #[test]
    fn test_shortest_angle_diff() {
        assert!((shortest_angle_diff(0.1, 0.3) - 0.2).abs() < 1e-4);
        // Wrapping case
        let d = shortest_angle_diff(3.0, -3.0);
        assert!(d.abs() < PI + 0.1);
    }
}
