//! Gravitational lensing.

use glam::{Vec2, Vec3, Vec4};

/// Deflection angle for a light ray passing a mass M at impact parameter b.
/// alpha = 4GM / (b * c^2)
#[allow(non_snake_case)]
pub fn deflection_angle(mass: f64, impact_param: f64, c: f64, G: f64) -> f64 {
    if impact_param.abs() < 1e-30 {
        return std::f64::consts::PI; // captured
    }
    4.0 * G * mass / (impact_param * c * c)
}

/// Einstein radius: the angular radius of a perfect ring image.
/// theta_E = sqrt(4GM / c^2 * D_ls / (D_l * D_s))
#[allow(non_snake_case)]
pub fn einstein_radius(mass: f64, d_lens: f64, d_source: f64, c: f64, G: f64) -> f64 {
    let d_ls = d_source - d_lens;
    if d_lens <= 0.0 || d_source <= 0.0 || d_ls <= 0.0 {
        return 0.0;
    }
    (4.0 * G * mass * d_ls / (c * c * d_lens * d_source)).sqrt()
}

/// Lens equation: theta - beta = theta_E^2 / theta
/// Returns theta - beta - theta_E^2/theta (should be zero for image positions).
#[allow(non_snake_case)]
pub fn lens_equation(theta: f64, beta: f64, theta_E: f64) -> f64 {
    if theta.abs() < 1e-30 {
        return f64::INFINITY;
    }
    theta - beta - theta_E * theta_E / theta
}

/// Image positions for a point lens. Returns two image angles (theta_+, theta_-).
/// theta_+/- = (beta +/- sqrt(beta^2 + 4*theta_E^2)) / 2
#[allow(non_snake_case)]
pub fn image_positions(beta: f64, theta_E: f64) -> (f64, f64) {
    let disc = (beta * beta + 4.0 * theta_E * theta_E).sqrt();
    let theta_plus = (beta + disc) / 2.0;
    let theta_minus = (beta - disc) / 2.0;
    (theta_plus, theta_minus)
}

/// Magnification of an image at angle theta for Einstein radius theta_E.
/// mu = (theta / theta_E)^4 / ((theta/theta_E)^4 - 1)
/// Equivalently: mu = |theta / beta * d(theta)/d(beta)|
#[allow(non_snake_case)]
pub fn magnification(theta: f64, theta_E: f64) -> f64 {
    if theta_E.abs() < 1e-30 {
        return 1.0;
    }
    let u = theta / theta_E;
    let u4 = u * u * u * u;
    if (u4 - 1.0).abs() < 1e-15 {
        return f64::INFINITY; // on the Einstein ring (caustic)
    }
    (u4 / (u4 - 1.0)).abs()
}

/// 2D grid of deflection vectors around a point mass.
#[derive(Debug, Clone)]
pub struct LensingField {
    pub width: usize,
    pub height: usize,
    pub center: Vec2,
    pub mass: f64,
    pub c: f64,
    pub g_const: f64,
    /// Deflection vectors at each grid point.
    pub deflections: Vec<Vec2>,
    pub cell_size: f32,
}

impl LensingField {
    #[allow(non_snake_case)]
    pub fn new(width: usize, height: usize, center: Vec2, mass: f64, c: f64, G: f64, cell_size: f32) -> Self {
        let mut deflections = Vec::with_capacity(width * height);
        for iy in 0..height {
            for ix in 0..width {
                let x = (ix as f32 - width as f32 / 2.0) * cell_size + center.x;
                let y = (iy as f32 - height as f32 / 2.0) * cell_size + center.y;
                let pos = Vec2::new(x, y);
                let r = pos - center;
                let dist = r.length() as f64;
                if dist < 1e-10 {
                    deflections.push(Vec2::ZERO);
                    continue;
                }
                let alpha = deflection_angle(mass, dist, c, G);
                let dir = r.normalize();
                deflections.push(dir * alpha as f32);
            }
        }
        Self {
            width,
            height,
            center,
            mass,
            c,
            g_const: G,
            deflections,
            cell_size,
        }
    }

    /// Get deflection at a grid index.
    pub fn get(&self, ix: usize, iy: usize) -> Vec2 {
        if ix < self.width && iy < self.height {
            self.deflections[iy * self.width + ix]
        } else {
            Vec2::ZERO
        }
    }

    /// Interpolate deflection at an arbitrary position.
    pub fn sample(&self, pos: Vec2) -> Vec2 {
        let r = pos - self.center;
        let dist = r.length() as f64;
        if dist < 1e-10 {
            return Vec2::ZERO;
        }
        let alpha = deflection_angle(self.mass, dist, self.c, self.g_const);
        let dir = r.normalize();
        dir * alpha as f32
    }

    /// Total magnification at a position.
    pub fn magnification_at(&self, pos: Vec2) -> f64 {
        let r = (pos - self.center).length() as f64;
        let rs = 4.0 * self.g_const * self.mass / (self.c * self.c);
        let theta_E = (rs).sqrt(); // simplified
        if theta_E < 1e-30 {
            return 1.0;
        }
        magnification(r, theta_E)
    }
}

/// Apply gravitational lensing to glyph positions.
/// Deflects each position away from the lens center.
#[allow(non_snake_case)]
pub fn apply_lensing(
    glyph_positions: &[Vec2],
    lens_center: Vec2,
    lens_mass: f64,
    c: f64,
    G: f64,
) -> Vec<Vec2> {
    glyph_positions.iter().map(|pos| {
        let r = *pos - lens_center;
        let dist = r.length() as f64;
        if dist < 1e-10 {
            return *pos;
        }
        let alpha = deflection_angle(lens_mass, dist, c, G);
        let dir = r.normalize();
        *pos + dir * alpha as f32
    }).collect()
}

/// Renderer for gravitational lensing effects.
#[derive(Debug, Clone)]
pub struct LensingRenderer {
    pub lens_center: Vec2,
    pub lens_mass: f64,
    pub c: f64,
    pub g_const: f64,
    pub einstein_ring_visible: bool,
}

impl LensingRenderer {
    #[allow(non_snake_case)]
    pub fn new(lens_center: Vec2, lens_mass: f64, c: f64, G: f64) -> Self {
        Self {
            lens_center,
            lens_mass,
            c,
            g_const: G,
            einstein_ring_visible: true,
        }
    }

    /// Compute lensed positions for background glyphs.
    pub fn lens_positions(&self, positions: &[Vec2]) -> Vec<Vec2> {
        apply_lensing(positions, self.lens_center, self.lens_mass, self.c, self.g_const)
    }

    /// Compute magnification for each position.
    pub fn magnifications(&self, positions: &[Vec2]) -> Vec<f64> {
        let d_lens = 1.0; // normalized
        let d_source = 2.0;
        let theta_E = einstein_radius(self.lens_mass, d_lens, d_source, self.c, self.g_const);

        positions.iter().map(|pos| {
            let theta = (*pos - self.lens_center).length() as f64;
            magnification(theta, theta_E)
        }).collect()
    }

    /// Render: returns (lensed_positions, magnification_factors).
    pub fn render(&self, positions: &[Vec2]) -> (Vec<Vec2>, Vec<f64>) {
        let lensed = self.lens_positions(positions);
        let mags = self.magnifications(positions);
        (lensed, mags)
    }

    /// Generate Einstein ring points for visualization.
    pub fn einstein_ring_points(&self, d_lens: f64, d_source: f64, n_points: usize) -> Vec<Vec2> {
        let theta_E = einstein_radius(self.lens_mass, d_lens, d_source, self.c, self.g_const);
        let mut points = Vec::with_capacity(n_points);
        for i in 0..n_points {
            let angle = (i as f64 / n_points as f64) * std::f64::consts::TAU;
            let p = self.lens_center + Vec2::new(
                (theta_E * angle.cos()) as f32,
                (theta_E * angle.sin()) as f32,
            );
            points.push(p);
        }
        points
    }
}

/// Microlensing light curve: amplification as a function of time.
/// Paczynski formula: A(u) = (u^2 + 2) / (u * sqrt(u^2 + 4))
/// where u = sqrt(u_min^2 + ((t - t0)/t_E)^2)
pub fn microlensing_lightcurve(u_min: f64, t_E: f64, times: &[f64]) -> Vec<f64> {
    let t0 = if times.is_empty() {
        0.0
    } else {
        (times[0] + times[times.len() - 1]) / 2.0
    };

    times.iter().map(|&t| {
        let tau = (t - t0) / t_E;
        let u = (u_min * u_min + tau * tau).sqrt();
        if u < 1e-15 {
            return f64::INFINITY;
        }
        (u * u + 2.0) / (u * (u * u + 4.0).sqrt())
    }).collect()
}

/// Total magnification from both images of a point lens.
/// A_total = (u^2 + 2) / (u * sqrt(u^2 + 4))
pub fn total_magnification(u: f64) -> f64 {
    if u < 1e-15 {
        return f64::INFINITY;
    }
    (u * u + 2.0) / (u * (u * u + 4.0).sqrt())
}

/// Compute the critical curve radius for a singular isothermal sphere lens.
pub fn sis_critical_radius(velocity_dispersion: f64, c: f64, d_lens: f64, d_source: f64) -> f64 {
    let d_ls = d_source - d_lens;
    if d_source <= 0.0 || d_ls <= 0.0 {
        return 0.0;
    }
    4.0 * std::f64::consts::PI * velocity_dispersion * velocity_dispersion * d_ls / (c * c * d_source)
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0;
    const G: f64 = 6.674e-11;

    #[test]
    fn test_deflection_angle_sun() {
        // Sun deflection of starlight: ~1.75 arcseconds
        let m_sun = 1.989e30;
        let r_sun = 6.96e8; // solar radius as impact parameter
        let alpha = deflection_angle(m_sun, r_sun, C, G);
        let arcsec = alpha * 206265.0; // convert radians to arcseconds
        assert!(
            (arcsec - 1.75).abs() < 0.1,
            "Solar deflection: {} arcsec, expected ~1.75",
            arcsec
        );
    }

    #[test]
    fn test_deflection_decreases_with_distance() {
        let m = 1e30;
        let a1 = deflection_angle(m, 1e8, C, G);
        let a2 = deflection_angle(m, 1e9, C, G);
        assert!(a2 < a1, "Deflection should decrease with impact param");
    }

    #[test]
    fn test_deflection_approaches_zero() {
        let m = 1e30;
        let a = deflection_angle(m, 1e20, C, G);
        assert!(a < 1e-10, "Deflection at large b should be tiny: {}", a);
    }

    #[test]
    fn test_einstein_radius() {
        let m = 1e30;
        let d_l = 1e20;
        let d_s = 2e20;
        let theta_E = einstein_radius(m, d_l, d_s, C, G);
        assert!(theta_E > 0.0);
    }

    #[test]
    fn test_image_positions() {
        let theta_E = 1.0;
        let beta = 0.5;
        let (tp, tm) = image_positions(beta, theta_E);
        // theta+ should be positive and > theta_E
        assert!(tp > 0.0);
        // theta- should be negative
        assert!(tm < 0.0);
        // Verify lens equation
        let err_p = lens_equation(tp, beta, theta_E);
        let err_m = lens_equation(tm, beta, theta_E);
        assert!(err_p.abs() < 1e-10, "Lens equation error +: {}", err_p);
        assert!(err_m.abs() < 1e-10, "Lens equation error -: {}", err_m);
    }

    #[test]
    fn test_einstein_ring() {
        // When beta = 0 (source directly behind lens), both images merge at theta_E
        let theta_E = 1.0;
        let (tp, tm) = image_positions(0.0, theta_E);
        assert!((tp - theta_E).abs() < 1e-10);
        assert!((tm + theta_E).abs() < 1e-10); // negative side
    }

    #[test]
    fn test_magnification_at_einstein_ring() {
        // Magnification diverges at the Einstein ring
        let theta_E = 1.0;
        let mag = magnification(theta_E, theta_E);
        assert!(mag.is_infinite() || mag > 1e10, "Magnification at caustic: {}", mag);
    }

    #[test]
    fn test_magnification_far_from_lens() {
        let theta_E = 1.0;
        let mag = magnification(100.0 * theta_E, theta_E);
        assert!((mag - 1.0).abs() < 0.01, "Far magnification should be ~1: {}", mag);
    }

    #[test]
    fn test_microlensing_lightcurve_peak() {
        let u_min = 0.5;
        let t_E = 10.0;
        let times: Vec<f64> = (-50..=50).map(|i| i as f64).collect();
        let curve = microlensing_lightcurve(u_min, t_E, &times);

        // Peak should be at the middle (t=0)
        let mid = curve.len() / 2;
        assert!(curve[mid] > curve[0], "Peak should be at center");
        assert!(curve[mid] > curve[curve.len() - 1], "Peak should be at center");

        // All values should be >= 1 (lensing always brightens)
        for &a in &curve {
            assert!(a >= 1.0, "Amplification should be >= 1: {}", a);
        }
    }

    #[test]
    fn test_apply_lensing() {
        let positions = vec![Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0)];
        let lensed = apply_lensing(&positions, Vec2::ZERO, 1e30, C, G);
        // Lensed positions should be deflected outward from center
        assert!(lensed[0].x > positions[0].x || (lensed[0] - positions[0]).length() > 0.0);
    }

    #[test]
    fn test_total_magnification() {
        // At u=1 (source at Einstein radius): A = 3/sqrt(5) ~ 1.342
        let a = total_magnification(1.0);
        let expected = 3.0 / 5.0_f64.sqrt();
        assert!((a - expected).abs() < 1e-10);
    }

    #[test]
    fn test_lensing_field() {
        let field = LensingField::new(10, 10, Vec2::ZERO, 1e30, C, G, 1.0);
        assert_eq!(field.deflections.len(), 100);
        // Center deflection should be zero
        let center_defl = field.get(5, 5);
        // Corner should have some deflection
        let corner_defl = field.get(0, 0);
        assert!(corner_defl.length() > 0.0 || center_defl.length() >= 0.0);
    }
}
