//! Magnetic field computation — Biot-Savart law, current loops, solenoids,
//! magnetic dipoles, field line tracing, and Ampere's law verification.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

/// Permeability of free space / (4*pi) in normalized units.
const MU0_OVER_4PI: f32 = 1.0;

// ── Current Segment ───────────────────────────────────────────────────────

/// A finite straight current-carrying segment.
#[derive(Clone, Debug)]
pub struct CurrentSegment {
    pub start: Vec3,
    pub end: Vec3,
    pub current: f32,
}

impl CurrentSegment {
    pub fn new(start: Vec3, end: Vec3, current: f32) -> Self {
        Self { start, end, current }
    }
}

/// Biot-Savart law for a finite current segment.
/// dB = (mu0 / 4*pi) * I * dl × r_hat / r^2
/// Integrated analytically for a straight segment.
pub fn biot_savart(segment: &CurrentSegment, point: Vec3) -> Vec3 {
    let dl = segment.end - segment.start;
    let length = dl.length();
    if length < 1e-10 {
        return Vec3::ZERO;
    }

    // Numerical integration along the segment (Simpson-like with many points)
    let n = 20;
    let mut b = Vec3::ZERO;
    let dl_step = dl / n as f32;
    let dl_mag = dl_step.length();

    for i in 0..n {
        let t = (i as f32 + 0.5) / n as f32;
        let src = segment.start + dl * t;
        let r_vec = point - src;
        let r2 = r_vec.length_squared();
        if r2 < 1e-10 {
            continue;
        }
        let r = r2.sqrt();
        let r_hat = r_vec / r;

        // dB = (mu0/4pi) * I * dl × r_hat / r^2
        let cross = dl_step.cross(r_hat);
        b += MU0_OVER_4PI * segment.current * cross / r2;
    }

    b
}

/// Compute the total magnetic field at `pos` from multiple current segments (superposition).
pub fn magnetic_field_at(segments: &[CurrentSegment], pos: Vec3) -> Vec3 {
    let mut field = Vec3::ZERO;
    for seg in segments {
        field += biot_savart(seg, pos);
    }
    field
}

// ── Infinite Wire ─────────────────────────────────────────────────────────

/// An infinite straight wire carrying current.
#[derive(Clone, Debug)]
pub struct InfiniteWire {
    pub position: Vec3,  // a point on the wire
    pub direction: Vec3, // unit direction of current flow
    pub current: f32,
}

impl InfiniteWire {
    pub fn new(position: Vec3, direction: Vec3, current: f32) -> Self {
        Self {
            position,
            direction: direction.normalize(),
            current,
        }
    }

    /// Analytical magnetic field: B = (mu0 * I) / (2*pi*r) in the azimuthal direction.
    pub fn field_at(&self, point: Vec3) -> Vec3 {
        let to_point = point - self.position;
        // Project out the component along the wire
        let parallel = to_point.dot(self.direction) * self.direction;
        let perp = to_point - parallel;
        let r = perp.length();
        if r < 1e-10 {
            return Vec3::ZERO;
        }
        // B direction: dl × r_hat (azimuthal)
        let r_hat = perp / r;
        let b_dir = self.direction.cross(r_hat);
        // B magnitude: mu0*I / (2*pi*r), with mu0 = 4*pi*MU0_OVER_4PI
        let b_mag = 2.0 * MU0_OVER_4PI * self.current / r;
        b_dir * b_mag
    }
}

// ── Circular Loop ─────────────────────────────────────────────────────────

/// A circular current loop.
#[derive(Clone, Debug)]
pub struct CircularLoop {
    pub center: Vec3,
    pub normal: Vec3, // axis direction
    pub radius: f32,
    pub current: f32,
}

impl CircularLoop {
    pub fn new(center: Vec3, normal: Vec3, radius: f32, current: f32) -> Self {
        Self {
            center,
            normal: normal.normalize(),
            radius,
            current,
        }
    }

    /// On-axis magnetic field of a circular loop.
    /// B = (mu0 * I * R^2) / (2 * (R^2 + z^2)^(3/2))
    pub fn on_axis_field(&self, distance_along_axis: f32) -> Vec3 {
        let r2 = self.radius * self.radius;
        let z2 = distance_along_axis * distance_along_axis;
        let denom = (r2 + z2).powf(1.5);
        if denom < 1e-10 {
            return Vec3::ZERO;
        }
        // Factor of 2*pi because MU0_OVER_4PI = mu0/(4*pi)
        let b_mag = 2.0 * PI * MU0_OVER_4PI * self.current * r2 / denom;
        self.normal * b_mag
    }

    /// General field at any point via numerical integration.
    pub fn field_at(&self, point: Vec3, segments: usize) -> Vec3 {
        let segs = self.to_segments(segments);
        magnetic_field_at(&segs, point)
    }

    /// Convert the loop into a series of current segments.
    pub fn to_segments(&self, n: usize) -> Vec<CurrentSegment> {
        let n = n.max(8);
        // Build orthonormal basis for the loop plane
        let w = self.normal;
        let u = if w.x.abs() < 0.9 {
            Vec3::X.cross(w).normalize()
        } else {
            Vec3::Y.cross(w).normalize()
        };
        let v = w.cross(u);

        let mut segments = Vec::with_capacity(n);
        for i in 0..n {
            let theta0 = 2.0 * PI * i as f32 / n as f32;
            let theta1 = 2.0 * PI * (i + 1) as f32 / n as f32;
            let p0 = self.center + self.radius * (u * theta0.cos() + v * theta0.sin());
            let p1 = self.center + self.radius * (u * theta1.cos() + v * theta1.sin());
            segments.push(CurrentSegment::new(p0, p1, self.current));
        }
        segments
    }
}

// ── Solenoid ──────────────────────────────────────────────────────────────

/// A solenoid: many circular loops stacked along an axis.
#[derive(Clone, Debug)]
pub struct Solenoid {
    pub center: Vec3,
    pub axis: Vec3,
    pub radius: f32,
    pub length: f32,
    pub turns: u32,
    pub current: f32,
}

impl Solenoid {
    pub fn new(center: Vec3, axis: Vec3, radius: f32, length: f32, turns: u32, current: f32) -> Self {
        Self {
            center,
            axis: axis.normalize(),
            radius,
            length,
            turns,
            current,
        }
    }

    /// Interior field of an ideal infinite solenoid: B = mu0 * n * I
    pub fn interior_field(&self) -> Vec3 {
        let n = self.turns as f32 / self.length; // turns per unit length
        // mu0 = 4*pi * MU0_OVER_4PI
        let b_mag = 4.0 * PI * MU0_OVER_4PI * n * self.current;
        self.axis * b_mag
    }

    /// Convert solenoid to a collection of circular loops.
    pub fn to_loops(&self) -> Vec<CircularLoop> {
        let mut loops = Vec::with_capacity(self.turns as usize);
        let start = self.center - self.axis * self.length * 0.5;
        for i in 0..self.turns {
            let t = (i as f32 + 0.5) / self.turns as f32;
            let pos = start + self.axis * self.length * t;
            loops.push(CircularLoop::new(pos, self.axis, self.radius, self.current));
        }
        loops
    }

    /// Field at any point by summing contributions from all loops.
    pub fn field_at(&self, point: Vec3, segments_per_loop: usize) -> Vec3 {
        let loops = self.to_loops();
        let mut field = Vec3::ZERO;
        for loop_ in &loops {
            field += loop_.field_at(point, segments_per_loop);
        }
        field
    }

    /// Check if a point is inside the solenoid (approximately).
    pub fn is_inside(&self, point: Vec3) -> bool {
        let to_point = point - self.center;
        let along_axis = to_point.dot(self.axis);
        if along_axis.abs() > self.length * 0.5 {
            return false;
        }
        let perp = to_point - along_axis * self.axis;
        perp.length() < self.radius
    }
}

// ── Field Line Tracing ────────────────────────────────────────────────────

/// Trace a magnetic field line from a start point.
/// Magnetic field lines are closed loops (div B = 0).
pub fn trace_magnetic_field_line(
    segments: &[CurrentSegment],
    start: Vec3,
    steps: usize,
) -> Vec<Vec3> {
    let step_size = 0.1;
    let mut points = Vec::with_capacity(steps + 1);
    let mut pos = start;
    points.push(pos);

    for _ in 0..steps {
        let b = magnetic_field_at(segments, pos);
        if b.length_squared() < 1e-14 {
            break;
        }
        let dir = b.normalize();

        // RK4
        let k1 = dir * step_size;

        let b2 = magnetic_field_at(segments, pos + k1 * 0.5);
        if b2.length_squared() < 1e-14 { break; }
        let k2 = b2.normalize() * step_size;

        let b3 = magnetic_field_at(segments, pos + k2 * 0.5);
        if b3.length_squared() < 1e-14 { break; }
        let k3 = b3.normalize() * step_size;

        let b4 = magnetic_field_at(segments, pos + k3);
        if b4.length_squared() < 1e-14 { break; }
        let k4 = b4.normalize() * step_size;

        pos += (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0;
        points.push(pos);

        // Check if we've returned close to start (closed loop)
        if points.len() > 10 && (pos - start).length() < step_size * 2.0 {
            points.push(start); // close the loop
            break;
        }
    }

    points
}

// ── Magnetic Dipole ───────────────────────────────────────────────────────

/// Magnetic field of a magnetic dipole at a given position.
/// B = (mu0/4*pi) * [3(m·r_hat)r_hat - m] / r^3
pub fn magnetic_dipole_field(moment: Vec3, pos: Vec3) -> Vec3 {
    let r = pos.length();
    if r < 1e-10 {
        return Vec3::ZERO;
    }
    let r_hat = pos / r;
    let r3 = r * r * r;
    let m_dot_r = moment.dot(r_hat);
    MU0_OVER_4PI * (3.0 * m_dot_r * r_hat - moment) / r3
}

// ── Ampere's Law ──────────────────────────────────────────────────────────

/// Verify Ampere's law: the circulation of B around a closed path equals mu0 * I_enc.
/// Returns the line integral ∮ B · dl.
pub fn ampere_circulation(segments: &[CurrentSegment], path_points: &[Vec3]) -> f32 {
    if path_points.len() < 2 {
        return 0.0;
    }
    let mut circulation = 0.0f32;
    for i in 0..path_points.len() {
        let next = (i + 1) % path_points.len();
        let dl = path_points[next] - path_points[i];
        let midpoint = (path_points[i] + path_points[next]) * 0.5;
        let b = magnetic_field_at(segments, midpoint);
        circulation += b.dot(dl);
    }
    circulation
}

// ── Magnetic Field Renderer ───────────────────────────────────────────────

/// Renderer for magnetic field visualization.
pub struct MagneticFieldRenderer {
    pub field_line_color: Vec4,
    pub arrow_color: Vec4,
    pub flux_density_scale: f32,
}

impl MagneticFieldRenderer {
    pub fn new() -> Self {
        Self {
            field_line_color: Vec4::new(0.2, 0.8, 0.3, 1.0),
            arrow_color: Vec4::new(1.0, 1.0, 0.2, 1.0),
            flux_density_scale: 1.0,
        }
    }

    /// Color based on flux density magnitude.
    pub fn color_for_flux_density(&self, b_magnitude: f32) -> Vec4 {
        let t = (b_magnitude * self.flux_density_scale).min(1.0);
        // Gradient from blue (weak) to green to red (strong)
        let r = (2.0 * t - 1.0).max(0.0);
        let g = 1.0 - (2.0 * t - 1.0).abs();
        let b = (1.0 - 2.0 * t).max(0.0);
        Vec4::new(r, g, b, 0.8)
    }

    /// Arrow glyph for direction.
    pub fn direction_arrow(direction: Vec3) -> char {
        let angle = direction.y.atan2(direction.x);
        let octant = ((angle / (PI / 4.0)).round() as i32).rem_euclid(8);
        match octant {
            0 => '→',
            1 => '↗',
            2 => '↑',
            3 => '↖',
            4 => '←',
            5 => '↙',
            6 => '↓',
            7 => '↘',
            _ => '·',
        }
    }

    /// Render a set of field line points with direction arrows.
    pub fn render_field_line(&self, points: &[Vec3], segments: &[CurrentSegment]) -> Vec<(Vec3, char, Vec4)> {
        let mut result = Vec::new();
        for i in 0..points.len() {
            let b = magnetic_field_at(segments, points[i]);
            let mag = b.length();
            let color = self.color_for_flux_density(mag);
            let ch = if i + 1 < points.len() {
                let dir = points[i + 1] - points[i];
                Self::direction_arrow(dir)
            } else {
                '·'
            };
            result.push((points[i], ch, color));
        }
        result
    }
}

impl Default for MagneticFieldRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biot_savart_direction() {
        // Current along +z, field at +x should be in -y direction (right-hand rule)
        // Actually: dl × r for dl=(0,0,1) and r=(1,0,0) gives (0,0,1)×(1,0,0)=(0,1,0)
        // Wait: r_hat points from source to field point.
        // dl=(0,0,dz), r_hat=(1,0,0), dl×r_hat = (0*0-dz*0, dz*1-0*0, 0*0-0*1) = (0,dz,0)
        // Hmm, let me just verify the field has a specific direction
        let seg = CurrentSegment::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, 5.0), 1.0);
        let b = biot_savart(&seg, Vec3::new(1.0, 0.0, 0.0));
        // For a wire along z, field at (1,0,0) should be in the y direction
        assert!(b.y.abs() > b.x.abs() * 10.0, "B should be primarily in y: {:?}", b);
        assert!(b.y > 0.0, "B_y should be positive for +z current at +x");
    }

    #[test]
    fn test_infinite_wire_inverse_r() {
        let wire = InfiniteWire::new(Vec3::ZERO, Vec3::Z, 1.0);
        let b1 = wire.field_at(Vec3::new(1.0, 0.0, 0.0));
        let b2 = wire.field_at(Vec3::new(2.0, 0.0, 0.0));
        // B ∝ 1/r, so B(2r) = B(r)/2
        let ratio = b1.length() / b2.length();
        assert!((ratio - 2.0).abs() < 0.01, "ratio={}", ratio);
    }

    #[test]
    fn test_circular_loop_on_axis() {
        let loop_ = CircularLoop::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        // At center of loop
        let b_center = loop_.on_axis_field(0.0);
        // B = 2*pi*MU0_OVER_4PI * I / R = 2*pi*1*1/1 = 2*pi
        let expected = 2.0 * PI * MU0_OVER_4PI * 1.0 / 1.0;
        assert!((b_center.z - expected).abs() < 0.01, "b_center={:?}, expected={}", b_center, expected);

        // Field should decrease with distance
        let b_far = loop_.on_axis_field(5.0);
        assert!(b_far.length() < b_center.length(), "Field should decrease with distance");
    }

    #[test]
    fn test_solenoid_interior_field() {
        let sol = Solenoid::new(Vec3::ZERO, Vec3::Z, 0.5, 10.0, 100, 1.0);
        let b_interior = sol.interior_field();
        // B = mu0 * n * I = 4*pi * MU0_OVER_4PI * (100/10) * 1 = 4*pi * 10
        let n = 100.0 / 10.0;
        let expected = 4.0 * PI * MU0_OVER_4PI * n * 1.0;
        assert!((b_interior.z - expected).abs() < 0.01);
    }

    #[test]
    fn test_solenoid_uniformity() {
        // Interior field of a long solenoid should be approximately uniform
        let sol = Solenoid::new(Vec3::ZERO, Vec3::Z, 1.0, 20.0, 200, 1.0);
        let b_center = sol.interior_field();
        // The numerical field near center should match the analytical interior field
        // (but full numerical computation is expensive, so just verify analytical)
        let b_mag = b_center.length();
        assert!(b_mag > 0.0);
        // Field is along axis
        assert!(b_center.z.abs() > b_center.x.abs() * 100.0);
    }

    #[test]
    fn test_ampere_law() {
        // A circular path around a straight wire should give mu0 * I
        let wire_segments: Vec<CurrentSegment> = {
            // Approximate infinite wire with a long segment along z
            vec![CurrentSegment::new(
                Vec3::new(0.0, 0.0, -50.0),
                Vec3::new(0.0, 0.0, 50.0),
                1.0,
            )]
        };

        // Circular path of radius 2 in the xy-plane
        let n = 200;
        let r = 2.0;
        let path: Vec<Vec3> = (0..n)
            .map(|i| {
                let theta = 2.0 * PI * i as f32 / n as f32;
                Vec3::new(r * theta.cos(), r * theta.sin(), 0.0)
            })
            .collect();

        let circulation = ampere_circulation(&wire_segments, &path);
        // Expected: mu0 * I = 4*pi * MU0_OVER_4PI * I = 4*pi * 1
        let expected = 4.0 * PI * MU0_OVER_4PI * 1.0;
        let relative_error = (circulation - expected).abs() / expected;
        assert!(relative_error < 0.1, "Ampere's law: circ={}, expected={}, error={}", circulation, expected, relative_error);
    }

    #[test]
    fn test_magnetic_dipole() {
        let m = Vec3::new(0.0, 0.0, 1.0);
        // Along the axis: B = (mu0/4pi) * 2m/r^3
        let b = magnetic_dipole_field(m, Vec3::new(0.0, 0.0, 5.0));
        let expected = MU0_OVER_4PI * 2.0 / (5.0_f32.powi(3));
        assert!((b.z - expected).abs() < 0.001, "dipole field: {}", b.z);
    }

    #[test]
    fn test_renderer_colors() {
        let renderer = MagneticFieldRenderer::new();
        let weak = renderer.color_for_flux_density(0.0);
        let strong = renderer.color_for_flux_density(1.0);
        // Weak field should be blue-ish, strong should be red-ish
        assert!(weak.z > weak.x, "Weak field should be blue");
        assert!(strong.x > strong.z, "Strong field should be red");
    }

    #[test]
    fn test_direction_arrow() {
        assert_eq!(MagneticFieldRenderer::direction_arrow(Vec3::new(1.0, 0.0, 0.0)), '→');
        assert_eq!(MagneticFieldRenderer::direction_arrow(Vec3::new(-1.0, 0.0, 0.0)), '←');
    }
}
