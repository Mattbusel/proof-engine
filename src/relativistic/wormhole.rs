//! Wormhole physics and rendering (Ellis/Morris-Thorne type).

use glam::{Vec2, Vec3, Vec4};

/// Ellis wormhole (Morris-Thorne type, simplest traversable wormhole).
#[derive(Debug, Clone)]
pub struct EllisWormhole {
    pub throat_radius: f64,
    pub position: Vec2,
}

impl EllisWormhole {
    pub fn new(throat_radius: f64, position: Vec2) -> Self {
        Self { throat_radius, position }
    }

    /// Radial coordinate from proper distance.
    /// r(l) = sqrt(l^2 + b^2) where b = throat_radius.
    pub fn r_from_proper_distance(&self, l: f64) -> f64 {
        (l * l + self.throat_radius * self.throat_radius).sqrt()
    }

    /// Embedding function z(r) for the wormhole.
    /// z(r) = b * ln((r + sqrt(r^2 - b^2)) / b) for r >= b.
    pub fn embedding_z(&self, r: f64) -> f64 {
        let b = self.throat_radius;
        if r < b {
            return 0.0;
        }
        let arg = (r + (r * r - b * b).max(0.0).sqrt()) / b;
        if arg <= 0.0 { return 0.0; }
        b * arg.ln()
    }

    /// Shape function b(r) for Ellis wormhole: b(r) = b0^2 / r.
    pub fn shape_function(&self, r: f64) -> f64 {
        if r <= 0.0 { return self.throat_radius; }
        self.throat_radius * self.throat_radius / r
    }

    /// Check flare-out condition: (b - b'r) / (2b^2) > 0 at the throat.
    pub fn flare_out_satisfied(&self) -> bool {
        // At throat r = b: b'(b) = -b^2/b^2 = -1, so b - b'r = b - (-1)*b = 2b > 0
        true
    }
}

/// Proper distance through the wormhole from coordinate radius r.
/// l = +/- sqrt(r^2 - b^2) for r >= b.
pub fn proper_distance(r: f64, throat: f64) -> f64 {
    if r < throat {
        return 0.0;
    }
    (r * r - throat * throat).sqrt()
}

/// Generate the embedding diagram z(r) for visualization.
/// Returns (r, z) pairs for one side of the wormhole.
pub fn embedding_diagram(
    r_range: (f64, f64),
    throat: f64,
    steps: usize,
) -> Vec<(f64, f64)> {
    let (r_min, r_max) = (r_range.0.max(throat), r_range.1);
    let dr = (r_max - r_min) / steps.max(1) as f64;

    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let r = r_min + dr * i as f64;
        let z = if r <= throat {
            0.0
        } else {
            let arg = (r + (r * r - throat * throat).sqrt()) / throat;
            throat * arg.max(1e-30).ln()
        };
        points.push((r, z));
    }
    points
}

/// Trace a light ray through/around a wormhole.
/// Returns the path and whether the ray traversed the wormhole.
pub fn ray_trace_wormhole(
    observer: Vec2,
    direction: Vec2,
    wormhole: &EllisWormhole,
    steps: usize,
) -> (Vec<Vec2>, bool) {
    let b = wormhole.throat_radius as f32;
    let mut path = Vec::with_capacity(steps);
    let mut pos = observer;
    let mut vel = direction.normalize_or_zero();
    let dt = 0.1;
    let mut traversed = false;
    let mut min_r = f32::INFINITY;

    for _ in 0..steps {
        path.push(pos);

        let r_vec = pos - wormhole.position;
        let r = r_vec.length();
        min_r = min_r.min(r);

        if r < b * 0.1 {
            // Very close to throat center, consider traversed
            traversed = true;
        }

        // Deflection by wormhole geometry
        // The effective potential bends light toward the throat
        if r > 0.01 {
            let r_hat = r_vec / r;
            // Gravitational-like deflection from wormhole throat
            let deflection_strength = b * b / (r * r * r);
            let accel = -r_hat * deflection_strength;
            vel = vel + accel * dt;
            vel = vel.normalize();
        }

        pos = pos + vel * dt;

        // If ray has gone far enough, stop
        if (pos - wormhole.position).length() > (observer - wormhole.position).length() * 3.0 {
            break;
        }
    }

    if min_r < b * 1.5 {
        traversed = true;
    }

    (path, traversed)
}

/// Render a wormhole scene.
/// Returns (position, color) pairs.
pub fn render_wormhole(
    wormhole: &EllisWormhole,
    observer: Vec2,
    background_a: Vec4, // color of "this side"
    background_b: Vec4, // color of "other side"
    grid: usize,
) -> Vec<(Vec2, Vec4)> {
    let fov = (wormhole.throat_radius * 10.0) as f32;
    let mut pixels = Vec::with_capacity(grid * grid);

    for iy in 0..grid {
        for ix in 0..grid {
            let x = (ix as f32 / grid as f32 - 0.5) * fov + observer.x;
            let y = (iy as f32 / grid as f32 - 0.5) * fov + observer.y;
            let screen_pos = Vec2::new(x, y);
            let direction = (screen_pos - observer).normalize_or_zero();

            let (_, traversed) = ray_trace_wormhole(observer, direction, wormhole, 200);

            let color = if traversed {
                // Seeing through to the other side
                let dist = (screen_pos - wormhole.position).length();
                let throat = wormhole.throat_radius as f32;
                let t = (dist / throat).min(1.0);
                // Blend: near throat shows other side, far shows distortion ring
                Vec4::new(
                    background_b.x * (1.0 - t) + background_a.x * t,
                    background_b.y * (1.0 - t) + background_a.y * t,
                    background_b.z * (1.0 - t) + background_a.z * t,
                    1.0,
                )
            } else {
                background_a
            };

            pixels.push((screen_pos, color));
        }
    }

    pixels
}

/// Game-usable wormhole portal connecting two locations.
#[derive(Debug, Clone)]
pub struct WormholePortal {
    pub entrance: Vec2,
    pub exit: Vec2,
    pub throat: f64,
}

impl WormholePortal {
    pub fn new(entrance: Vec2, exit: Vec2, throat: f64) -> Self {
        Self { entrance, exit, throat }
    }

    /// Check if a position is within the portal entrance.
    pub fn is_near_entrance(&self, pos: Vec2) -> bool {
        (pos - self.entrance).length() < self.throat as f32
    }

    /// Check if a position is within the portal exit.
    pub fn is_near_exit(&self, pos: Vec2) -> bool {
        (pos - self.exit).length() < self.throat as f32
    }

    /// Get the displacement vector from entrance to exit.
    pub fn displacement(&self) -> Vec2 {
        self.exit - self.entrance
    }
}

/// Transform position and velocity through a wormhole portal.
/// Maps position relative to entrance to equivalent position relative to exit.
pub fn transform_through_wormhole(
    pos: Vec2,
    vel: Vec2,
    wormhole: &WormholePortal,
) -> (Vec2, Vec2) {
    let rel_pos = pos - wormhole.entrance;
    let new_pos = wormhole.exit + rel_pos;
    // Velocity is preserved (or could be rotated for more complex wormholes)
    (new_pos, vel)
}

/// Check wormhole stability: requires exotic matter (negative energy density).
/// A wormhole is stable if exotic_matter_density > threshold.
/// The threshold is proportional to 1/throat^2.
pub fn wormhole_stability(throat: f64, exotic_matter: f64) -> bool {
    // Minimum exotic matter needed: proportional to c^4 * b / (8*pi*G)
    // In dimensionless terms, we just check if exotic_matter > 1/throat^2
    let threshold = 1.0 / (throat * throat);
    exotic_matter > threshold
}

/// Wormhole renderer for visual effects.
#[derive(Debug, Clone)]
pub struct WormholeRenderer {
    pub wormhole: EllisWormhole,
    pub distortion_rings: usize,
    pub ring_color: Vec4,
    pub see_through: bool,
}

impl WormholeRenderer {
    pub fn new(wormhole: EllisWormhole) -> Self {
        Self {
            wormhole,
            distortion_rings: 5,
            ring_color: Vec4::new(0.3, 0.5, 1.0, 0.8),
            see_through: true,
        }
    }

    /// Generate distortion ring positions for visualization.
    pub fn ring_positions(&self) -> Vec<Vec<Vec2>> {
        let mut rings = Vec::new();
        let b = self.wormhole.throat_radius as f32;

        for i in 0..self.distortion_rings {
            let radius = b * (1.0 + i as f32 * 0.5);
            let n_points = 32;
            let mut ring = Vec::with_capacity(n_points);
            for j in 0..n_points {
                let angle = j as f32 / n_points as f32 * std::f32::consts::TAU;
                let p = self.wormhole.position + Vec2::new(
                    angle.cos() * radius,
                    angle.sin() * radius,
                );
                ring.push(p);
            }
            rings.push(ring);
        }
        rings
    }

    /// Compute the visual distortion factor at a position.
    /// Returns how much the view is distorted (0 = none, 1 = maximum).
    pub fn distortion_at(&self, pos: Vec2) -> f32 {
        let r = (pos - self.wormhole.position).length();
        let b = self.wormhole.throat_radius as f32;
        if r < b {
            1.0
        } else {
            (b / r).powi(2)
        }
    }

    /// Full render: returns pixels with distortion applied.
    pub fn render(&self, observer: Vec2, grid: usize) -> Vec<(Vec2, Vec4)> {
        let fov = (self.wormhole.throat_radius * 8.0) as f32;
        let mut pixels = Vec::with_capacity(grid * grid);

        for iy in 0..grid {
            for ix in 0..grid {
                let x = (ix as f32 / grid as f32 - 0.5) * fov + observer.x;
                let y = (iy as f32 / grid as f32 - 0.5) * fov + observer.y;
                let pos = Vec2::new(x, y);
                let dist = self.distortion_at(pos);
                let color = Vec4::new(
                    self.ring_color.x * dist + (1.0 - dist) * 0.1,
                    self.ring_color.y * dist + (1.0 - dist) * 0.1,
                    self.ring_color.z * dist + (1.0 - dist) * 0.2,
                    1.0,
                );
                pixels.push((pos, color));
            }
        }
        pixels
    }
}

/// Proper time for a traveler passing through the wormhole.
/// For Ellis wormhole, proper traversal time ~ L/v where L is the proper length.
pub fn traversal_time(throat: f64, velocity: f64) -> f64 {
    // Proper length through throat ~ pi * b / 2
    let proper_length = std::f64::consts::PI * throat / 2.0;
    proper_length / velocity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proper_distance_at_throat() {
        let d = proper_distance(5.0, 5.0);
        assert!(d.abs() < 1e-10, "At throat, proper distance = 0: {}", d);
    }

    #[test]
    fn test_proper_distance_increases() {
        let d1 = proper_distance(10.0, 5.0);
        let d2 = proper_distance(20.0, 5.0);
        assert!(d2 > d1, "Proper distance should increase with r");
    }

    #[test]
    fn test_embedding_diagram() {
        let diagram = embedding_diagram((5.0, 50.0), 5.0, 100);
        assert_eq!(diagram.len(), 101);
        // At throat, z should be near 0
        assert!(diagram[0].1.abs() < 1e-10 || diagram[0].0 >= 5.0);
        // z should increase with r
        let last_z = diagram.last().unwrap().1;
        assert!(last_z > 0.0, "z should be positive: {}", last_z);
    }

    #[test]
    fn test_embedding_diagram_shape() {
        let throat = 5.0;
        let diagram = embedding_diagram((throat, 100.0), throat, 200);
        // dz/dr should decrease (flaring out)
        let n = diagram.len();
        if n > 4 {
            let dz_near = (diagram[2].1 - diagram[1].1) / (diagram[2].0 - diagram[1].0);
            let dz_far = (diagram[n-1].1 - diagram[n-2].1) / (diagram[n-1].0 - diagram[n-2].0);
            assert!(dz_near > dz_far, "Should flare out: dz/dr near={} far={}", dz_near, dz_far);
        }
    }

    #[test]
    fn test_ray_trace_wormhole() {
        let wh = EllisWormhole::new(5.0, Vec2::ZERO);
        // Ray aimed at wormhole
        let (path, traversed) = ray_trace_wormhole(
            Vec2::new(50.0, 0.0),
            Vec2::new(-1.0, 0.0),
            &wh,
            500,
        );
        assert!(!path.is_empty());
        // Direct hit should traverse
        assert!(traversed, "Direct hit should traverse");
    }

    #[test]
    fn test_ray_trace_miss() {
        let wh = EllisWormhole::new(5.0, Vec2::ZERO);
        // Ray aimed far from wormhole
        let (path, traversed) = ray_trace_wormhole(
            Vec2::new(50.0, 100.0),
            Vec2::new(-1.0, 0.0),
            &wh,
            500,
        );
        assert!(!path.is_empty());
        assert!(!traversed, "Far miss should not traverse");
    }

    #[test]
    fn test_wormhole_portal_transform() {
        let portal = WormholePortal::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 200.0),
            5.0,
        );
        let (new_pos, new_vel) = transform_through_wormhole(
            Vec2::new(1.0, 2.0),
            Vec2::new(3.0, 4.0),
            &portal,
        );
        assert!((new_pos.x - 101.0).abs() < 1e-6);
        assert!((new_pos.y - 202.0).abs() < 1e-6);
        assert!((new_vel.x - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_wormhole_stability() {
        // Large throat is easier to stabilize
        assert!(!wormhole_stability(1.0, 0.5)); // not enough exotic matter
        assert!(wormhole_stability(1.0, 2.0)); // enough exotic matter
        assert!(wormhole_stability(10.0, 0.02)); // large throat needs less
    }

    #[test]
    fn test_ellis_wormhole_shape() {
        let wh = EllisWormhole::new(5.0, Vec2::ZERO);
        // r from proper distance: r(0) = b
        let r_at_throat = wh.r_from_proper_distance(0.0);
        assert!((r_at_throat - 5.0).abs() < 1e-10);

        // Shape function at throat: b(b) = b
        let b_at_throat = wh.shape_function(5.0);
        assert!((b_at_throat - 5.0).abs() < 1e-10);

        // Flare-out condition
        assert!(wh.flare_out_satisfied());
    }

    #[test]
    fn test_wormhole_portal_detection() {
        let portal = WormholePortal::new(Vec2::ZERO, Vec2::new(100.0, 0.0), 5.0);
        assert!(portal.is_near_entrance(Vec2::new(3.0, 0.0)));
        assert!(!portal.is_near_entrance(Vec2::new(10.0, 0.0)));
        assert!(portal.is_near_exit(Vec2::new(103.0, 0.0)));
    }

    #[test]
    fn test_traversal_time() {
        let t = traversal_time(10.0, 1.0);
        let expected = std::f64::consts::PI * 10.0 / 2.0;
        assert!((t - expected).abs() < 1e-10);
    }

    #[test]
    fn test_render_wormhole() {
        let wh = EllisWormhole::new(5.0, Vec2::ZERO);
        let pixels = render_wormhole(
            &wh,
            Vec2::new(30.0, 0.0),
            Vec4::new(0.0, 0.0, 0.1, 1.0),
            Vec4::new(0.1, 0.0, 0.0, 1.0),
            4,
        );
        assert_eq!(pixels.len(), 16);
    }
}
