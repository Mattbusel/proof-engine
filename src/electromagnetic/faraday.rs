//! Faraday cage and electromagnetic shielding simulation.
//! Computes shielded fields, induced surface charges, and skin depth.

use glam::{Vec2, Vec3, Vec4};
use std::f32::consts::PI;

// ── Faraday Cage ──────────────────────────────────────────────────────────

/// A Faraday cage defined by boundary points forming a closed polygon.
#[derive(Clone, Debug)]
pub struct FaradayCage {
    pub boundary_points: Vec<Vec2>,
    pub conductivity: f32,
}

impl FaradayCage {
    pub fn new(boundary_points: Vec<Vec2>, conductivity: f32) -> Self {
        Self { boundary_points, conductivity }
    }

    /// Create a rectangular cage.
    pub fn rectangular(center: Vec2, width: f32, height: f32, conductivity: f32, segments: usize) -> Self {
        let half_w = width * 0.5;
        let half_h = height * 0.5;
        let mut points = Vec::with_capacity(segments * 4);
        let n = segments.max(2);

        // Bottom edge
        for i in 0..n {
            let t = i as f32 / n as f32;
            points.push(center + Vec2::new(-half_w + t * width, -half_h));
        }
        // Right edge
        for i in 0..n {
            let t = i as f32 / n as f32;
            points.push(center + Vec2::new(half_w, -half_h + t * height));
        }
        // Top edge
        for i in 0..n {
            let t = i as f32 / n as f32;
            points.push(center + Vec2::new(half_w - t * width, half_h));
        }
        // Left edge
        for i in 0..n {
            let t = i as f32 / n as f32;
            points.push(center + Vec2::new(-half_w, half_h - t * height));
        }

        Self { boundary_points: points, conductivity }
    }

    /// Create a circular cage.
    pub fn circular(center: Vec2, radius: f32, conductivity: f32, segments: usize) -> Self {
        let n = segments.max(8);
        let points: Vec<Vec2> = (0..n)
            .map(|i| {
                let theta = 2.0 * PI * i as f32 / n as f32;
                center + Vec2::new(radius * theta.cos(), radius * theta.sin())
            })
            .collect();
        Self { boundary_points: points, conductivity }
    }

    /// Check if a point is inside the cage (ray casting algorithm).
    pub fn contains(&self, point: Vec2) -> bool {
        let n = self.boundary_points.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let pi = self.boundary_points[i];
            let pj = self.boundary_points[j];
            if ((pi.y > point.y) != (pj.y > point.y))
                && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Center of the cage (centroid of boundary points).
    pub fn center(&self) -> Vec2 {
        if self.boundary_points.is_empty() {
            return Vec2::ZERO;
        }
        let sum: Vec2 = self.boundary_points.iter().copied().sum();
        sum / self.boundary_points.len() as f32
    }
}

/// Compute the shielded electric field inside and around a Faraday cage.
/// The field inside a perfect conductor is zero; outside it is distorted.
/// Returns field vectors on a grid.
pub fn compute_shielded_field(
    cage: &FaradayCage,
    external_field: Vec3,
    grid_nx: usize,
    grid_ny: usize,
    grid_dx: f32,
) -> Vec<Vec3> {
    let size = grid_nx * grid_ny;
    let mut field = vec![external_field; size];

    // For each grid point, check if it's inside the cage
    // If inside, field = 0 (perfect shielding)
    // If on the boundary, field = 0 (conductor)
    // If outside, apply a simple distortion model
    for y in 0..grid_ny {
        for x in 0..grid_nx {
            let pos = Vec2::new(x as f32 * grid_dx, y as f32 * grid_dx);
            let i = x + y * grid_nx;

            if cage.contains(pos) {
                // Inside cage: field is zero (perfect Faraday shielding)
                field[i] = Vec3::ZERO;
            } else {
                // Outside: field is distorted near the cage
                // Simple model: field lines bend around the cage
                let to_center = Vec2::new(cage.center().x - pos.x, cage.center().y - pos.y);
                let dist = to_center.length();
                if dist < 1e-10 {
                    field[i] = Vec3::ZERO;
                    continue;
                }

                // Find closest boundary point
                let mut min_dist = f32::MAX;
                for bp in &cage.boundary_points {
                    let d = (*bp - pos).length();
                    if d < min_dist {
                        min_dist = d;
                    }
                }

                // Near the boundary, field is enhanced (charge accumulation)
                if min_dist < grid_dx * 3.0 {
                    let enhancement = 1.0 + 1.0 / (min_dist / grid_dx + 0.5);
                    field[i] = external_field * enhancement;
                }
            }
        }
    }

    field
}

/// Shielding effectiveness in dB.
/// SE = 20 * log10(E_incident / E_transmitted)
/// For a good conductor: SE depends on frequency and thickness.
pub fn shielding_effectiveness(cage: &FaradayCage, frequency: f32) -> f32 {
    // Simplified model: SE = 20*log10(1 + sigma/(2*omega*eps0))
    // In normalized units:
    let omega = 2.0 * PI * frequency;
    if omega < 1e-10 {
        return f32::INFINITY; // DC: perfect shielding
    }
    let ratio = 1.0 + cage.conductivity / (2.0 * omega);
    20.0 * ratio.log10()
}

/// Compute induced surface charge density on the cage boundary due to an external E field.
/// The surface charge is proportional to the normal component of the external field.
pub fn induced_surface_charge(cage: &FaradayCage, e_external: Vec3) -> Vec<f32> {
    let n = cage.boundary_points.len();
    if n < 2 {
        return vec![0.0; n];
    }
    let mut charges = Vec::with_capacity(n);

    for i in 0..n {
        let next = (i + 1) % n;
        let prev = if i == 0 { n - 1 } else { i - 1 };

        // Outward normal at this boundary point
        let tangent = cage.boundary_points[next] - cage.boundary_points[prev];
        let normal = Vec2::new(-tangent.y, tangent.x).normalize();

        // Surface charge: sigma = eps0 * E_n (normal component)
        // In normalized units eps0 = 1
        let e_normal = e_external.x * normal.x + e_external.y * normal.y;
        charges.push(e_normal);
    }

    charges
}

// ── Skin Depth ────────────────────────────────────────────────────────────

/// Skin depth: delta = sqrt(2 / (omega * mu * sigma))
pub fn skin_depth(conductivity: f32, permeability: f32, frequency: f32) -> f32 {
    let omega = 2.0 * PI * frequency;
    if omega < 1e-10 || conductivity < 1e-10 || permeability < 1e-10 {
        return f32::INFINITY;
    }
    (2.0 / (omega * permeability * conductivity)).sqrt()
}

// ── Conducting Sphere ─────────────────────────────────────────────────────

/// Analytical solution for EM shielding by a conducting sphere.
#[derive(Clone, Debug)]
pub struct ConductingSphere {
    pub center: Vec3,
    pub radius: f32,
    pub conductivity: f32,
}

impl ConductingSphere {
    pub fn new(center: Vec3, radius: f32, conductivity: f32) -> Self {
        Self { center, radius, conductivity }
    }

    /// Check if a point is inside the sphere.
    pub fn contains(&self, point: Vec3) -> bool {
        (point - self.center).length() < self.radius
    }

    /// Electric field around a conducting sphere in a uniform external field E0.
    /// Outside: E = E0 + dipole correction.
    /// Inside: E = 0 (perfect conductor).
    pub fn field_at(&self, point: Vec3, e_external: Vec3) -> Vec3 {
        let r_vec = point - self.center;
        let r = r_vec.length();

        if r < self.radius {
            return Vec3::ZERO; // Inside: perfectly shielded
        }

        if r < 1e-10 {
            return Vec3::ZERO;
        }

        let r_hat = r_vec / r;
        let a = self.radius;
        let ratio = (a / r).powi(3);

        // Induced dipole: p = 4*pi*eps0*a^3 * E0
        // Correction field: like a dipole field
        let e0_dot_r = e_external.dot(r_hat);
        let dipole_correction = ratio * (3.0 * e0_dot_r * r_hat - e_external);

        e_external + dipole_correction
    }

    /// Shielding effectiveness at the center.
    pub fn shielding_at_center(&self) -> f32 {
        // Perfect conductor: field is exactly zero inside
        f32::INFINITY
    }

    /// Induced surface charge density at a point on the sphere surface.
    /// sigma = 3 * eps0 * E0 * cos(theta), where theta is angle from E0 direction.
    pub fn surface_charge_at(&self, surface_point: Vec3, e_external: Vec3) -> f32 {
        let r_vec = (surface_point - self.center).normalize();
        let e_hat = if e_external.length() > 1e-10 {
            e_external.normalize()
        } else {
            return 0.0;
        };
        let cos_theta = r_vec.dot(e_hat);
        3.0 * e_external.length() * cos_theta
    }
}

// ── Cage Renderer ─────────────────────────────────────────────────────────

/// Renderer for Faraday cage visualization.
pub struct CageRenderer {
    pub cage_color: Vec4,
    pub interior_color: Vec4,
    pub field_color: Vec4,
    pub field_scale: f32,
}

impl CageRenderer {
    pub fn new() -> Self {
        Self {
            cage_color: Vec4::new(0.8, 0.8, 0.2, 1.0),
            interior_color: Vec4::new(0.0, 0.1, 0.0, 0.3),
            field_color: Vec4::new(0.5, 0.5, 1.0, 0.6),
            field_scale: 1.0,
        }
    }

    /// Render the cage boundary as a series of glyphs.
    pub fn render_boundary(&self, cage: &FaradayCage) -> Vec<(Vec2, char, Vec4)> {
        let mut result = Vec::new();
        let n = cage.boundary_points.len();
        for i in 0..n {
            let next = (i + 1) % n;
            let dir = cage.boundary_points[next] - cage.boundary_points[i];
            let ch = if dir.x.abs() > dir.y.abs() { '─' } else { '│' };
            result.push((cage.boundary_points[i], ch, self.cage_color));
        }
        result
    }

    /// Render the field on a grid, showing shielded interior.
    pub fn render_field_grid(
        &self,
        cage: &FaradayCage,
        field: &[Vec3],
        grid_nx: usize,
        grid_ny: usize,
        grid_dx: f32,
    ) -> Vec<(Vec2, Vec4)> {
        let mut result = Vec::with_capacity(grid_nx * grid_ny);
        for y in 0..grid_ny {
            for x in 0..grid_nx {
                let pos = Vec2::new(x as f32 * grid_dx, y as f32 * grid_dx);
                let i = x + y * grid_nx;
                let f = field[i];
                let mag = f.length();

                let color = if cage.contains(pos) {
                    self.interior_color
                } else {
                    let brightness = (mag * self.field_scale).min(1.0);
                    Vec4::new(
                        self.field_color.x * brightness,
                        self.field_color.y * brightness,
                        self.field_color.z * brightness,
                        brightness * 0.8,
                    )
                };
                result.push((pos, color));
            }
        }
        result
    }
}

impl Default for CageRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cage_contains() {
        let cage = FaradayCage::rectangular(Vec2::new(5.0, 5.0), 4.0, 4.0, 1e6, 10);
        assert!(cage.contains(Vec2::new(5.0, 5.0)), "Center should be inside");
        assert!(!cage.contains(Vec2::new(0.0, 0.0)), "Origin should be outside");
    }

    #[test]
    fn test_circular_cage() {
        let cage = FaradayCage::circular(Vec2::new(5.0, 5.0), 3.0, 1e6, 32);
        assert!(cage.contains(Vec2::new(5.0, 5.0)));
        assert!(!cage.contains(Vec2::new(5.0, 9.0)));
    }

    #[test]
    fn test_shielded_field_interior_zero() {
        let cage = FaradayCage::rectangular(Vec2::new(5.0, 5.0), 4.0, 4.0, 1e6, 10);
        let external = Vec3::new(1.0, 0.0, 0.0);
        let field = compute_shielded_field(&cage, external, 10, 10, 1.0);

        // Check that interior points have zero field
        let center_idx = 5 + 5 * 10;
        let interior_field = field[center_idx];
        assert!(
            interior_field.length() < 0.01,
            "Interior field should be ~0: {:?}",
            interior_field
        );
    }

    #[test]
    fn test_skin_depth_formula() {
        let sd = skin_depth(1e7, 1.0, 1e6);
        // delta = sqrt(2 / (2*pi*1e6 * 1 * 1e7)) = sqrt(2 / (2*pi*1e13))
        let expected = (2.0 / (2.0 * PI * 1e6 * 1.0 * 1e7)).sqrt();
        assert!((sd - expected).abs() < 1e-10, "sd={}, expected={}", sd, expected);
    }

    #[test]
    fn test_skin_depth_decreases_with_frequency() {
        let sd_low = skin_depth(1e6, 1.0, 1e3);
        let sd_high = skin_depth(1e6, 1.0, 1e6);
        assert!(sd_high < sd_low, "Skin depth should decrease with frequency");
    }

    #[test]
    fn test_conducting_sphere_interior() {
        let sphere = ConductingSphere::new(Vec3::ZERO, 1.0, 1e7);
        let e_ext = Vec3::new(1.0, 0.0, 0.0);
        let e_inside = sphere.field_at(Vec3::new(0.0, 0.0, 0.0), e_ext);
        assert!(e_inside.length() < 1e-6, "Field inside sphere should be zero");
    }

    #[test]
    fn test_conducting_sphere_far_field() {
        let sphere = ConductingSphere::new(Vec3::ZERO, 1.0, 1e7);
        let e_ext = Vec3::new(1.0, 0.0, 0.0);
        // Far from sphere, field should approach external field
        let e_far = sphere.field_at(Vec3::new(100.0, 0.0, 0.0), e_ext);
        assert!(
            (e_far - e_ext).length() < 0.01,
            "Far field should approach E_external: {:?}",
            e_far
        );
    }

    #[test]
    fn test_induced_surface_charge() {
        let cage = FaradayCage::circular(Vec2::new(0.0, 0.0), 1.0, 1e6, 16);
        let charges = induced_surface_charge(&cage, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(charges.len(), 16);
        // Total induced charge should be approximately zero (charge conservation)
        let total: f32 = charges.iter().sum();
        // This won't be exactly zero due to discrete sampling, but should be small
        assert!(total.abs() < 1.0, "Total induced charge should be small: {}", total);
    }

    #[test]
    fn test_shielding_effectiveness() {
        let cage = FaradayCage::new(vec![], 1e6);
        let se = shielding_effectiveness(&cage, 1e3);
        assert!(se > 0.0, "Shielding effectiveness should be positive");
        // Higher conductivity should give better shielding
        let cage2 = FaradayCage::new(vec![], 1e8);
        let se2 = shielding_effectiveness(&cage2, 1e3);
        assert!(se2 > se, "Higher conductivity = better shielding");
    }

    #[test]
    fn test_sphere_surface_charge() {
        let sphere = ConductingSphere::new(Vec3::ZERO, 1.0, 1e7);
        let e_ext = Vec3::new(1.0, 0.0, 0.0);
        // At the "pole" facing the field: cos(0) = 1
        let sigma_pole = sphere.surface_charge_at(Vec3::new(1.0, 0.0, 0.0), e_ext);
        assert!(sigma_pole > 0.0, "Charge at pole facing field should be positive");
        // At the equator: cos(90°) = 0
        let sigma_eq = sphere.surface_charge_at(Vec3::new(0.0, 1.0, 0.0), e_ext);
        assert!(sigma_eq.abs() < 1e-6, "Charge at equator should be zero");
    }
}
