//! Terrell rotation: apparent rotation of fast-moving objects due to finite light speed.

use glam::{Vec2, Vec3, Vec4};
use super::lorentz::lorentz_factor;

/// Terrell rotation angle: the apparent rotation angle of an object moving at speed v.
/// For an object at angle `object_angle` from the direction of motion,
/// the apparent rotation is approximately arcsin(v/c).
pub fn terrell_rotation_angle(v: f64, c: f64, object_angle: f64) -> f64 {
    let beta = v / c;
    if beta.abs() >= 1.0 {
        return std::f64::consts::FRAC_PI_2;
    }
    // The Terrell rotation angle depends on the viewing geometry.
    // For a sphere, the apparent rotation is arcsin(beta).
    // For general objects at angle theta, the rotation is:
    // alpha = arcsin(beta * sin(object_angle))
    let arg = beta * object_angle.sin();
    arg.clamp(-1.0, 1.0).asin()
}

/// Compute the apparent position of an object accounting for light travel time.
/// The observer sees the object at its retarded position.
pub fn apparent_position(
    true_pos: Vec3,
    velocity: Vec3,
    observer: Vec3,
    c: f64,
    time: f64,
) -> Vec3 {
    // Current true position at time t
    let current_pos = true_pos + velocity * time as f32;

    // Find retarded time: the time t_r such that |pos(t_r) - observer| = c * (time - t_r)
    // For constant velocity: pos(t_r) = true_pos + velocity * t_r
    // |true_pos + velocity * t_r - observer| = c * (time - t_r)
    //
    // Solve iteratively
    let mut t_r = time;
    for _ in 0..20 {
        let pos_at_tr = true_pos + velocity * t_r as f32;
        let dist = (pos_at_tr - observer).length() as f64;
        let t_r_new = time - dist / c;
        if (t_r_new - t_r).abs() < 1e-12 {
            break;
        }
        t_r = t_r_new;
    }

    // Return the position at retarded time
    true_pos + velocity * t_r as f32
}

/// Compute the retarded time: the time at which light must have left the object
/// to arrive at the observer at the current time.
/// t_retarded = t - |object_pos - observer_pos| / c
pub fn retarded_time(object_pos: Vec3, observer_pos: Vec3, c: f64) -> f64 {
    let dist = (object_pos - observer_pos).length() as f64;
    -dist / c // time offset (negative, meaning light was emitted this much earlier)
}

/// Terrell renderer: renders fast-moving objects with apparent rotation.
#[derive(Debug, Clone)]
pub struct TerellRenderer {
    pub c: f64,
    pub observer_pos: Vec3,
}

impl TerellRenderer {
    pub fn new(c: f64, observer_pos: Vec3) -> Self {
        Self { c, observer_pos }
    }

    /// Compute apparent vertices of an object given its velocity.
    /// Each vertex is displaced to its retarded position.
    pub fn apparent_vertices(
        &self,
        vertices: &[Vec3],
        center: Vec3,
        velocity: Vec3,
        time: f64,
    ) -> Vec<Vec3> {
        vertices.iter().map(|v| {
            apparent_position(*v, velocity, self.observer_pos, self.c, time)
        }).collect()
    }

    /// Compute the combined Terrell rotation + Lorentz contraction appearance.
    /// The Terrell effect means a sphere always looks circular (not contracted),
    /// but other shapes appear rotated.
    pub fn render_object(
        &self,
        vertices: &[Vec3],
        center: Vec3,
        velocity: Vec3,
        time: f64,
    ) -> Vec<Vec3> {
        let v = velocity.length() as f64;
        if v < 1e-10 {
            return vertices.to_vec();
        }

        let vel_dir = velocity.normalize();
        let to_observer = (self.observer_pos - center).normalize_or_zero();

        // Cross product gives the rotation axis
        let rotation_axis = vel_dir.cross(to_observer).normalize_or_zero();
        let sin_angle = (v / self.c) as f32;
        let cos_angle = (1.0 - sin_angle * sin_angle).max(0.0).sqrt();

        // Apply apparent rotation around the rotation axis
        vertices.iter().map(|vert| {
            let rel = *vert - center;
            // Rodrigues' rotation formula
            let rotated = rel * cos_angle
                + rotation_axis.cross(rel) * sin_angle
                + rotation_axis * rotation_axis.dot(rel) * (1.0 - cos_angle);
            center + rotated
        }).collect()
    }

    /// Full rendering pipeline: retarded positions + Terrell rotation.
    pub fn full_render(
        &self,
        vertices: &[Vec3],
        center: Vec3,
        velocity: Vec3,
        time: f64,
    ) -> Vec<Vec3> {
        // First compute retarded positions
        let retarded = self.apparent_vertices(vertices, center, velocity, time);
        // The retarded positions already encode the Terrell effect
        retarded
    }
}

/// Render a moving cube: compute apparent vertex positions considering light travel time.
/// `velocity` is the cube's velocity, `observer` is the observer position.
/// `cube_vertices` are the 8 corners of the cube in the cube's rest frame.
pub fn render_moving_cube(
    velocity: Vec3,
    observer: Vec3,
    cube_vertices: &[Vec3],
    c: f64,
    time: f64,
) -> Vec<Vec3> {
    cube_vertices.iter().map(|v| {
        apparent_position(*v, velocity, observer, c, time)
    }).collect()
}

/// For multiple objects, compute where they appear based on retarded time.
/// Each object is (position, velocity). Returns apparent positions.
pub fn finite_light_speed_positions(
    objects: &[(Vec3, Vec3)],
    observer: Vec3,
    c: f64,
) -> Vec<Vec3> {
    objects.iter().map(|(pos, vel)| {
        // Find retarded time iteratively
        let dist = (*pos - observer).length() as f64;
        let t_delay = dist / c;
        // Apparent position is where the object was t_delay ago
        *pos - *vel * t_delay as f32
    }).collect()
}

/// Generate unit cube vertices centered at origin.
pub fn unit_cube_vertices(center: Vec3, half_size: f32) -> Vec<Vec3> {
    let h = half_size;
    vec![
        center + Vec3::new(-h, -h, -h),
        center + Vec3::new( h, -h, -h),
        center + Vec3::new( h,  h, -h),
        center + Vec3::new(-h,  h, -h),
        center + Vec3::new(-h, -h,  h),
        center + Vec3::new( h, -h,  h),
        center + Vec3::new( h,  h,  h),
        center + Vec3::new(-h,  h,  h),
    ]
}

/// Compute the time delay for light from each vertex to reach the observer.
pub fn light_travel_delays(vertices: &[Vec3], observer: Vec3, c: f64) -> Vec<f64> {
    vertices.iter().map(|v| {
        (*v - observer).length() as f64 / c
    }).collect()
}

/// Check if Terrell rotation makes a sphere still appear circular.
/// Returns the max deviation from circular appearance.
pub fn sphere_appearance_deviation(
    v: f64,
    c: f64,
    n_points: usize,
    observer_distance: f64,
) -> f64 {
    let observer = Vec3::new(observer_distance as f32, 0.0, 0.0);
    let center = Vec3::ZERO;
    let velocity = Vec3::new(0.0, v as f32, 0.0);
    let radius = 1.0_f32;

    // Generate sphere surface points
    let mut apparent_radii = Vec::new();
    for i in 0..n_points {
        let theta = (i as f64 / n_points as f64) * std::f64::consts::TAU;
        let point = center + Vec3::new(0.0, theta.cos() as f32, theta.sin() as f32) * radius;

        let app = apparent_position(point, velocity, observer, c, 0.0);
        let app_center = apparent_position(center, velocity, observer, c, 0.0);
        let r = (app - app_center).length();
        apparent_radii.push(r);
    }

    if apparent_radii.is_empty() {
        return 0.0;
    }
    let mean = apparent_radii.iter().sum::<f32>() / apparent_radii.len() as f32;
    let max_dev = apparent_radii.iter().map(|r| (r - mean).abs()).fold(0.0_f32, f32::max);
    (max_dev / mean) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0;

    #[test]
    fn test_terrell_rotation_angle_at_rest() {
        let angle = terrell_rotation_angle(0.0, C, std::f64::consts::FRAC_PI_2);
        assert!(angle.abs() < 1e-10, "No rotation at rest: {}", angle);
    }

    #[test]
    fn test_terrell_rotation_angle_increases_with_v() {
        let a1 = terrell_rotation_angle(0.5 * C, C, std::f64::consts::FRAC_PI_2);
        let a2 = terrell_rotation_angle(0.9 * C, C, std::f64::consts::FRAC_PI_2);
        assert!(a2 > a1, "Rotation should increase with v: {} vs {}", a1, a2);
    }

    #[test]
    fn test_terrell_rotation_angle_at_zero_viewing_angle() {
        // At theta=0 (head-on), the apparent rotation should be zero
        let angle = terrell_rotation_angle(0.9 * C, C, 0.0);
        assert!(angle.abs() < 1e-10, "No rotation at head-on: {}", angle);
    }

    #[test]
    fn test_retarded_time() {
        let t = retarded_time(
            Vec3::new(C as f32, 0.0, 0.0),
            Vec3::ZERO,
            C,
        );
        // Distance = c, so delay = 1 second
        assert!((t - (-1.0)).abs() < 1e-6, "Retarded time offset: {}", t);
    }

    #[test]
    fn test_apparent_position_stationary() {
        let app = apparent_position(
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            C,
            0.0,
        );
        // Stationary object: apparent position = true position
        assert!((app.x - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_finite_light_speed_positions() {
        let objects = vec![
            (Vec3::new(10.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            (Vec3::new(20.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
        ];
        let apparent = finite_light_speed_positions(&objects, Vec3::ZERO, C);
        // Both should be shifted backward along velocity by light delay
        assert!(apparent[0].x < 10.0);
        assert!(apparent[1].x < 20.0);
        // Farther object has more delay
        let shift_0 = 10.0 - apparent[0].x;
        let shift_1 = 20.0 - apparent[1].x;
        assert!(shift_1 > shift_0);
    }

    #[test]
    fn test_render_moving_cube() {
        let verts = unit_cube_vertices(Vec3::ZERO, 1.0);
        assert_eq!(verts.len(), 8);
        let velocity = Vec3::new(0.5 * C as f32, 0.0, 0.0);
        let observer = Vec3::new(100.0, 0.0, 0.0);
        let apparent = render_moving_cube(velocity, observer, &verts, C, 0.0);
        assert_eq!(apparent.len(), 8);
    }

    #[test]
    fn test_light_travel_delays() {
        let verts = vec![
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
        ];
        let delays = light_travel_delays(&verts, Vec3::ZERO, C);
        assert!(delays[1] > delays[0]);
        assert!((delays[0] - 10.0 / C).abs() < 1e-10);
    }
}
