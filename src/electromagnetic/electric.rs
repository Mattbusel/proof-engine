//! Electric field computation — Coulomb's law, field lines, dipoles,
//! capacitors, line charges, and Gauss's law verification.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

/// Coulomb constant k = 1/(4*pi*eps0). Using normalized units: k = 1.
const K_COULOMB: f32 = 1.0;

// ── Point Charge ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PointCharge {
    pub position: Vec3,
    pub charge: f32,
}

impl PointCharge {
    pub fn new(position: Vec3, charge: f32) -> Self {
        Self { position, charge }
    }
}

/// Compute the total electric field at `pos` from multiple point charges via Coulomb superposition.
pub fn electric_field_at(charges: &[PointCharge], pos: Vec3) -> Vec3 {
    let mut field = Vec3::ZERO;
    for c in charges {
        let r_vec = pos - c.position;
        let r2 = r_vec.length_squared();
        if r2 < 1e-10 {
            continue; // Skip self-interaction
        }
        let r = r2.sqrt();
        // E = k * q / r^2 * r_hat
        field += K_COULOMB * c.charge / r2 * (r_vec / r);
    }
    field
}

/// Compute the electric potential at `pos` from multiple point charges.
pub fn electric_potential_at(charges: &[PointCharge], pos: Vec3) -> f32 {
    let mut potential = 0.0f32;
    for c in charges {
        let r = (pos - c.position).length();
        if r < 1e-10 {
            continue;
        }
        // V = k * q / r
        potential += K_COULOMB * c.charge / r;
    }
    potential
}

// ── Electric Field Line ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ElectricFieldLine {
    pub points: Vec<Vec3>,
}

/// Trace a field line from `start` using RK4 integration.
pub fn trace_field_line(
    charges: &[PointCharge],
    start: Vec3,
    steps: usize,
    step_size: f32,
) -> ElectricFieldLine {
    let mut points = Vec::with_capacity(steps + 1);
    let mut pos = start;
    points.push(pos);

    for _ in 0..steps {
        // RK4 integration along the field direction
        let e1 = electric_field_at(charges, pos);
        if e1.length_squared() < 1e-12 {
            break;
        }
        let d1 = e1.normalize() * step_size;

        let e2 = electric_field_at(charges, pos + d1 * 0.5);
        if e2.length_squared() < 1e-12 {
            break;
        }
        let d2 = e2.normalize() * step_size;

        let e3 = electric_field_at(charges, pos + d2 * 0.5);
        if e3.length_squared() < 1e-12 {
            break;
        }
        let d3 = e3.normalize() * step_size;

        let e4 = electric_field_at(charges, pos + d3);
        if e4.length_squared() < 1e-12 {
            break;
        }
        let d4 = e4.normalize() * step_size;

        pos += (d1 + 2.0 * d2 + 2.0 * d3 + d4) / 6.0;
        points.push(pos);

        // Stop if too close to any charge
        let mut too_close = false;
        for c in charges {
            if (pos - c.position).length() < step_size * 0.5 {
                too_close = true;
                break;
            }
        }
        if too_close {
            break;
        }
    }

    ElectricFieldLine { points }
}

// ── Dipole ────────────────────────────────────────────────────────────────

/// Electric dipole: two equal and opposite charges separated by a small distance.
#[derive(Clone, Debug)]
pub struct Dipole {
    pub pos: Vec3,
    pub moment: Vec3, // p = q*d, direction from - to +
}

impl Dipole {
    pub fn new(pos: Vec3, moment: Vec3) -> Self {
        Self { pos, moment }
    }

    /// Electric field of a dipole at a given position (far-field approximation).
    /// E = (1/4*pi*eps0) * [3(p·r_hat)r_hat - p] / r^3
    pub fn field_at(&self, point: Vec3) -> Vec3 {
        let r_vec = point - self.pos;
        let r = r_vec.length();
        if r < 1e-10 {
            return Vec3::ZERO;
        }
        let r_hat = r_vec / r;
        let r3 = r * r * r;
        let p_dot_r = self.moment.dot(r_hat);
        K_COULOMB * (3.0 * p_dot_r * r_hat - self.moment) / r3
    }

    /// Electric potential of a dipole at a given position.
    /// V = (1/4*pi*eps0) * p·r_hat / r^2
    pub fn potential_at(&self, point: Vec3) -> f32 {
        let r_vec = point - self.pos;
        let r = r_vec.length();
        if r < 1e-10 {
            return 0.0;
        }
        let r_hat = r_vec / r;
        K_COULOMB * self.moment.dot(r_hat) / (r * r)
    }

    /// Convert dipole to a pair of point charges for exact computation.
    pub fn to_charges(&self, separation: f32) -> [PointCharge; 2] {
        let d = self.moment.normalize() * separation * 0.5;
        let q = self.moment.length() / separation;
        [
            PointCharge::new(self.pos + d, q),
            PointCharge::new(self.pos - d, -q),
        ]
    }
}

// ── Capacitor ─────────────────────────────────────────────────────────────

/// Parallel plate capacitor with uniform field between plates.
#[derive(Clone, Debug)]
pub struct Capacitor {
    pub plate1: Vec3, // center of plate 1
    pub plate2: Vec3, // center of plate 2
    pub charge_density: f32, // surface charge density (sigma)
}

impl Capacitor {
    pub fn new(plate1: Vec3, plate2: Vec3, charge_density: f32) -> Self {
        Self { plate1, plate2, charge_density }
    }

    /// Uniform electric field between the plates: E = sigma / eps0.
    /// Direction: from plate1 (+) to plate2 (-).
    pub fn field(&self) -> Vec3 {
        let direction = (self.plate2 - self.plate1).normalize();
        // In normalized units eps0 = 1/(4*pi*k) = 1/(4*pi) with k=1
        // But for simplicity with k=1: E = sigma / eps0 = 4*pi*k*sigma
        // Using SI-like: E = sigma (with eps0=1 normalization)
        direction * self.charge_density
    }

    /// Check if a point is between the plates (projected along plate normal).
    pub fn is_between_plates(&self, point: Vec3) -> bool {
        let axis = self.plate2 - self.plate1;
        let len = axis.length();
        if len < 1e-10 {
            return false;
        }
        let n = axis / len;
        let t = (point - self.plate1).dot(n);
        t >= 0.0 && t <= len
    }

    /// Electric field at a point. Returns uniform field between plates, zero outside.
    pub fn field_at(&self, point: Vec3) -> Vec3 {
        if self.is_between_plates(point) {
            self.field()
        } else {
            Vec3::ZERO
        }
    }

    /// Potential difference between the plates.
    pub fn voltage(&self) -> f32 {
        let d = (self.plate2 - self.plate1).length();
        self.charge_density * d
    }

    /// Capacitance per unit area: C/A = eps0 / d.
    pub fn capacitance_per_area(&self) -> f32 {
        let d = (self.plate2 - self.plate1).length();
        if d < 1e-10 {
            return 0.0;
        }
        1.0 / d // eps0 = 1 in normalized units
    }
}

// ── Line Charge ───────────────────────────────────────────────────────────

/// Finite line charge with uniform charge per unit length.
#[derive(Clone, Debug)]
pub struct LineCharge {
    pub start: Vec3,
    pub end: Vec3,
    pub charge_per_length: f32,
}

impl LineCharge {
    pub fn new(start: Vec3, end: Vec3, charge_per_length: f32) -> Self {
        Self { start, end, charge_per_length }
    }

    /// Compute electric field at a point by numerical integration along the line.
    /// Divides the line into `segments` pieces and sums contributions.
    pub fn field_at(&self, point: Vec3, segments: usize) -> Vec3 {
        let segments = segments.max(1);
        let dl = (self.end - self.start) / segments as f32;
        let dq = self.charge_per_length * dl.length();
        let mut field = Vec3::ZERO;

        for i in 0..segments {
            let t = (i as f32 + 0.5) / segments as f32;
            let src = self.start + (self.end - self.start) * t;
            let r_vec = point - src;
            let r2 = r_vec.length_squared();
            if r2 < 1e-10 {
                continue;
            }
            let r = r2.sqrt();
            field += K_COULOMB * dq / r2 * (r_vec / r);
        }
        field
    }

    /// Compute electric potential at a point by numerical integration.
    pub fn potential_at(&self, point: Vec3, segments: usize) -> f32 {
        let segments = segments.max(1);
        let dl = (self.end - self.start) / segments as f32;
        let dq = self.charge_per_length * dl.length();
        let mut potential = 0.0f32;

        for i in 0..segments {
            let t = (i as f32 + 0.5) / segments as f32;
            let src = self.start + (self.end - self.start) * t;
            let r = (point - src).length();
            if r < 1e-10 {
                continue;
            }
            potential += K_COULOMB * dq / r;
        }
        potential
    }

    /// Total charge on the line.
    pub fn total_charge(&self) -> f32 {
        self.charge_per_length * (self.end - self.start).length()
    }
}

// ── Gauss's Law ───────────────────────────────────────────────────────────

/// Verify Gauss's law: the electric flux through a closed surface equals Q_enc / eps0.
/// surface_points and normals define the surface; normals point outward.
/// Returns the total flux integral E · dA.
pub fn gauss_flux(
    charges: &[PointCharge],
    surface_points: &[Vec3],
    normals: &[Vec3],
    area_per_element: f32,
) -> f32 {
    assert_eq!(surface_points.len(), normals.len());
    let mut flux = 0.0f32;
    for (i, point) in surface_points.iter().enumerate() {
        let e = electric_field_at(charges, *point);
        flux += e.dot(normals[i]) * area_per_element;
    }
    flux
}

/// Generate surface points and normals for a sphere centered at `center` with given `radius`.
pub fn sphere_surface(center: Vec3, radius: f32, theta_steps: usize, phi_steps: usize) -> (Vec<Vec3>, Vec<Vec3>) {
    let mut points = Vec::new();
    let mut normals = Vec::new();
    for i in 0..theta_steps {
        let theta = PI * (i as f32 + 0.5) / theta_steps as f32;
        for j in 0..phi_steps {
            let phi = 2.0 * PI * j as f32 / phi_steps as f32;
            let normal = Vec3::new(
                theta.sin() * phi.cos(),
                theta.sin() * phi.sin(),
                theta.cos(),
            );
            points.push(center + normal * radius);
            normals.push(normal);
        }
    }
    (points, normals)
}

// ── Electric Field Renderer ───────────────────────────────────────────────

/// Renderer for electric field visualization.
pub struct ElectricFieldRenderer {
    pub field_line_color_positive: Vec4,
    pub field_line_color_negative: Vec4,
    pub equipotential_color: Vec4,
    pub arrow_spacing: f32,
}

impl ElectricFieldRenderer {
    pub fn new() -> Self {
        Self {
            field_line_color_positive: Vec4::new(1.0, 0.2, 0.1, 1.0),
            field_line_color_negative: Vec4::new(0.1, 0.3, 1.0, 1.0),
            equipotential_color: Vec4::new(0.5, 0.5, 0.5, 0.4),
            arrow_spacing: 2.0,
        }
    }

    /// Generate field line start points around each charge.
    pub fn generate_field_lines(
        &self,
        charges: &[PointCharge],
        lines_per_charge: usize,
        steps: usize,
        step_size: f32,
    ) -> Vec<ElectricFieldLine> {
        let mut lines = Vec::new();
        for charge in charges {
            if charge.charge.abs() < 1e-10 {
                continue;
            }
            for i in 0..lines_per_charge {
                let angle = 2.0 * PI * i as f32 / lines_per_charge as f32;
                let offset = Vec3::new(angle.cos(), angle.sin(), 0.0) * step_size * 2.0;
                let start = charge.position + offset;
                // Trace in direction of field for positive charges, against for negative
                if charge.charge > 0.0 {
                    lines.push(trace_field_line(charges, start, steps, step_size));
                } else {
                    lines.push(trace_field_line(charges, start, steps, -step_size));
                }
            }
        }
        lines
    }

    /// Color a field line point based on the local field magnitude.
    pub fn color_for_field(&self, field_magnitude: f32, is_positive_source: bool) -> Vec4 {
        let brightness = (field_magnitude * 0.1).min(1.0);
        if is_positive_source {
            self.field_line_color_positive * brightness
        } else {
            self.field_line_color_negative * brightness
        }
    }

    /// Generate equipotential contour points in the xy-plane.
    pub fn equipotential_contour(
        &self,
        charges: &[PointCharge],
        potential_value: f32,
        x_range: (f32, f32),
        y_range: (f32, f32),
        resolution: usize,
    ) -> Vec<Vec3> {
        let mut contour_points = Vec::new();
        let dx = (x_range.1 - x_range.0) / resolution as f32;
        let dy = (y_range.1 - y_range.0) / resolution as f32;
        let tolerance = (dx + dy) * 0.5;

        for i in 0..resolution {
            for j in 0..resolution {
                let x = x_range.0 + (i as f32 + 0.5) * dx;
                let y = y_range.0 + (j as f32 + 0.5) * dy;
                let pos = Vec3::new(x, y, 0.0);
                let v = electric_potential_at(charges, pos);
                if (v - potential_value).abs() < tolerance {
                    contour_points.push(pos);
                }
            }
        }
        contour_points
    }

    /// Get the rendering glyph for a field line point.
    pub fn trail_glyph(direction: Vec3) -> char {
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
}

impl Default for ElectricFieldRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inverse_square_law() {
        let charges = [PointCharge::new(Vec3::ZERO, 1.0)];
        let e1 = electric_field_at(&charges, Vec3::new(1.0, 0.0, 0.0));
        let e2 = electric_field_at(&charges, Vec3::new(2.0, 0.0, 0.0));
        // E ∝ 1/r^2, so E(2r) = E(r)/4
        let ratio = e1.length() / e2.length();
        assert!((ratio - 4.0).abs() < 0.01, "ratio={}", ratio);
    }

    #[test]
    fn test_superposition() {
        let q1 = PointCharge::new(Vec3::new(-1.0, 0.0, 0.0), 1.0);
        let q2 = PointCharge::new(Vec3::new(1.0, 0.0, 0.0), 1.0);

        // At the midpoint between two equal charges, E_x should cancel
        let e = electric_field_at(&[q1, q2], Vec3::new(0.0, 0.0, 0.0));
        assert!(e.x.abs() < 1e-6, "x-component should cancel: {}", e.x);
    }

    #[test]
    fn test_potential_symmetry() {
        let charges = [PointCharge::new(Vec3::ZERO, 1.0)];
        let v1 = electric_potential_at(&charges, Vec3::new(1.0, 0.0, 0.0));
        let v2 = electric_potential_at(&charges, Vec3::new(0.0, 1.0, 0.0));
        assert!((v1 - v2).abs() < 1e-6, "Potential should be spherically symmetric");
    }

    #[test]
    fn test_dipole_far_field() {
        let dipole = Dipole::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
        // Along the axis (z-axis), field should be ~ 2p / r^3
        let e_axis = dipole.field_at(Vec3::new(0.0, 0.0, 10.0));
        let expected = 2.0 * 1.0 / (10.0_f32.powi(3));
        assert!((e_axis.z - expected).abs() < 0.001, "Axial dipole field: {}", e_axis.z);

        // Perpendicular to axis, field should be ~ -p / r^3
        let e_perp = dipole.field_at(Vec3::new(10.0, 0.0, 0.0));
        let expected_perp = -1.0 / (10.0_f32.powi(3));
        assert!((e_perp.z - expected_perp).abs() < 0.001, "Perpendicular dipole field: {}", e_perp.z);
    }

    #[test]
    fn test_gauss_law() {
        let charges = [PointCharge::new(Vec3::ZERO, 3.0)];
        let (points, normals) = sphere_surface(Vec3::ZERO, 5.0, 40, 80);
        let area_element = 4.0 * PI * 25.0 / (40 * 80) as f32;
        let flux = gauss_flux(&charges, &points, &normals, area_element);
        // Gauss's law: flux = Q / eps0 = 4*pi*k*Q = 4*pi*Q (with k=1)
        let expected = 4.0 * PI * 3.0;
        let relative_error = (flux - expected).abs() / expected;
        assert!(relative_error < 0.05, "Gauss's law: flux={}, expected={}", flux, expected);
    }

    #[test]
    fn test_capacitor_uniform_field() {
        let cap = Capacitor::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            2.0,
        );
        let e = cap.field();
        assert!((e.z - 2.0).abs() < 1e-6);
        assert!(e.x.abs() < 1e-6);

        assert!(cap.is_between_plates(Vec3::new(0.0, 0.0, 0.5)));
        assert!(!cap.is_between_plates(Vec3::new(0.0, 0.0, 1.5)));
    }

    #[test]
    fn test_line_charge_symmetry() {
        // A line charge along z-axis: field should be radially symmetric in xy-plane
        let lc = LineCharge::new(
            Vec3::new(0.0, 0.0, -5.0),
            Vec3::new(0.0, 0.0, 5.0),
            1.0,
        );
        let e1 = lc.field_at(Vec3::new(1.0, 0.0, 0.0), 100);
        let e2 = lc.field_at(Vec3::new(0.0, 1.0, 0.0), 100);
        // Magnitudes should be approximately equal
        let ratio = e1.length() / e2.length();
        assert!((ratio - 1.0).abs() < 0.05, "Line charge field should be symmetric: ratio={}", ratio);
    }

    #[test]
    fn test_field_line_tracing() {
        let charges = [PointCharge::new(Vec3::ZERO, 1.0)];
        let line = trace_field_line(&charges, Vec3::new(0.5, 0.0, 0.0), 50, 0.1);
        assert!(line.points.len() > 1);
        // Field lines from positive charge should move outward
        let last = line.points.last().unwrap();
        assert!(last.length() > 0.5, "Field line should move away from positive charge");
    }

    #[test]
    fn test_renderer_trail_glyph() {
        assert_eq!(ElectricFieldRenderer::trail_glyph(Vec3::new(1.0, 0.0, 0.0)), '→');
        assert_eq!(ElectricFieldRenderer::trail_glyph(Vec3::new(0.0, 1.0, 0.0)), '↑');
    }

    #[test]
    fn test_dipole_to_charges() {
        let dipole = Dipole::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0));
        let charges = dipole.to_charges(0.1);
        assert!((charges[0].charge - 20.0).abs() < 1e-4);
        assert!((charges[1].charge + 20.0).abs() < 1e-4);
    }
}
