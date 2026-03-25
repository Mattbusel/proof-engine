//! Black hole physics and rendering.

use glam::{Vec2, Vec3, Vec4};

/// Schwarzschild radius: rs = 2GM/c^2.
#[allow(non_snake_case)]
pub fn schwarzschild_radius(mass: f64, G: f64, c: f64) -> f64 {
    2.0 * G * mass / (c * c)
}

/// Photon sphere radius: r = 1.5 * rs.
pub fn photon_sphere_radius(rs: f64) -> f64 {
    1.5 * rs
}

/// Innermost stable circular orbit radius: r = 3 * rs (for Schwarzschild).
pub fn isco_radius(rs: f64) -> f64 {
    3.0 * rs
}

/// Black hole with mass, position, and Kerr spin parameter.
#[derive(Debug, Clone)]
pub struct BlackHole {
    pub mass: f64,
    pub position: Vec2,
    pub spin: f64, // Kerr parameter a = J/(Mc), dimensionless a* = a/M in [0,1)
}

impl BlackHole {
    pub fn new(mass: f64, position: Vec2, spin: f64) -> Self {
        Self {
            mass,
            position,
            spin: spin.clamp(0.0, 0.999),
        }
    }

    #[allow(non_snake_case)]
    pub fn rs(&self, G: f64, c: f64) -> f64 {
        schwarzschild_radius(self.mass, G, c)
    }

    pub fn photon_sphere(&self, rs: f64) -> f64 {
        photon_sphere_radius(rs)
    }

    pub fn isco(&self, rs: f64) -> f64 {
        // For Kerr, ISCO depends on spin. Prograde:
        // r_isco = M * (3 + Z2 - sqrt((3-Z1)(3+Z1+2*Z2)))
        // Simplified: for a=0, r_isco = 6M = 3rs
        if self.spin.abs() < 1e-10 {
            return isco_radius(rs);
        }
        let a = self.spin;
        let z1 = 1.0 + (1.0 - a * a).powf(1.0 / 3.0) * ((1.0 + a).powf(1.0 / 3.0) + (1.0 - a).powf(1.0 / 3.0));
        let z2 = (3.0 * a * a + z1 * z1).sqrt();
        let r = 3.0 + z2 - ((3.0 - z1) * (3.0 + z1 + 2.0 * z2)).sqrt();
        r * rs / 2.0 // convert from M units to coordinate radius
    }
}

/// Trace a light ray in the Schwarzschild metric using the geodesic equation.
/// Returns the path as a series of 2D positions.
pub fn ray_trace_schwarzschild(
    observer: Vec2,
    direction: Vec2,
    bh: &BlackHole,
    steps: usize,
) -> Vec<Vec2> {
    let rs = 2.0 * bh.mass; // using geometric units G=c=1 for tracing

    let mut path = Vec::with_capacity(steps);
    let mut pos = observer;
    let dir_norm = direction.normalize_or_zero();
    let mut vel = dir_norm;

    let dt = 0.1;

    for _ in 0..steps {
        path.push(pos);

        let r_vec = pos - bh.position;
        let r = r_vec.length() as f64;

        // Stop if captured
        if r < rs * 1.01 {
            break;
        }

        // Gravitational deflection: acceleration toward BH
        // In Schwarzschild, effective potential gives:
        // d^2u/dphi^2 + u = 3*rs*u^2/2 (where u = 1/r)
        // For ray tracing in 2D, use Newtonian-like with GR correction:
        let r_hat = r_vec.normalize_or_zero();
        let accel_mag = -1.5 * rs as f32 / (r as f32 * r as f32);
        let accel = r_hat * accel_mag;

        vel = vel + accel * dt;
        vel = vel.normalize() * 1.0; // keep speed = c = 1
        pos = pos + vel * dt;
    }

    path
}

/// Total deflection angle for a light ray with impact parameter b.
/// For weak field: alpha = 2*rs/b. For strong field (b near photon sphere), much larger.
pub fn ray_deflection(impact_param: f64, rs: f64) -> f64 {
    if impact_param <= 1.5 * rs {
        // Captured or orbiting
        return std::f64::consts::PI * 2.0;
    }
    // Approximate formula valid for b >> rs:
    // alpha = 2*rs/b + 15*pi*rs^2/(16*b^2) + ...
    let ratio = rs / impact_param;
    2.0 * ratio + 15.0 * std::f64::consts::PI * ratio * ratio / 16.0
}

/// Accretion disk model.
#[derive(Debug, Clone)]
pub struct AccretionDisk {
    pub inner_radius: f64,
    pub outer_radius: f64,
}

impl AccretionDisk {
    pub fn new(inner_radius: f64, outer_radius: f64) -> Self {
        Self { inner_radius, outer_radius }
    }

    /// Default temperature profile: T ~ r^(-3/4) (standard thin disk).
    pub fn temperature(&self, r: f64) -> f64 {
        if r < self.inner_radius || r > self.outer_radius {
            return 0.0;
        }
        (self.inner_radius / r).powf(0.75)
    }
}

/// Disk emission at radius r. Returns intensity based on temperature profile.
pub fn disk_emission(r: f64, disk: &AccretionDisk) -> f64 {
    let temp = disk.temperature(r);
    // Stefan-Boltzmann: intensity ~ T^4
    temp.powi(4)
}

/// Render a black hole scene: shadow, accretion disk, lensed background.
/// Returns (position, color) for each pixel in the grid.
pub fn render_black_hole(
    bh: &BlackHole,
    disk: &AccretionDisk,
    observer: Vec2,
    grid_size: usize,
) -> Vec<(Vec2, Vec4)> {
    let rs = 2.0 * bh.mass;
    let fov = (disk.outer_radius * 3.0) as f32;
    let mut pixels = Vec::with_capacity(grid_size * grid_size);

    for iy in 0..grid_size {
        for ix in 0..grid_size {
            let x = (ix as f32 / grid_size as f32 - 0.5) * fov + observer.x;
            let y = (iy as f32 / grid_size as f32 - 0.5) * fov + observer.y;
            let screen_pos = Vec2::new(x, y);

            let direction = (screen_pos - observer).normalize_or_zero();
            let impact = {
                // Impact parameter: perpendicular distance from BH to ray
                let to_bh = bh.position - observer;
                let along = to_bh.dot(direction);
                let perp = to_bh - direction * along;
                perp.length() as f64
            };

            let color = if impact < rs * 1.01 {
                // Shadow (captured)
                Vec4::new(0.0, 0.0, 0.0, 1.0)
            } else if impact < 1.5 * rs {
                // Photon ring
                Vec4::new(1.0, 0.8, 0.3, 1.0)
            } else if impact >= disk.inner_radius && impact <= disk.outer_radius {
                // Accretion disk
                let emission = disk_emission(impact, disk) as f32;
                let temp_color = Vec4::new(
                    emission.min(1.0),
                    (emission * 0.6).min(1.0),
                    (emission * 0.2).min(1.0),
                    1.0,
                );
                temp_color
            } else {
                // Background (slightly lensed)
                let defl = ray_deflection(impact, rs);
                let brightness = (0.1 + defl as f32 * 0.05).min(0.3);
                Vec4::new(brightness, brightness, brightness * 1.5, 1.0)
            };

            pixels.push((screen_pos, color));
        }
    }

    pixels
}

/// Compute the shadow boundary of a black hole as seen from a given distance.
/// Returns points on the shadow edge.
pub fn shadow_boundary(
    bh: &BlackHole,
    observer_distance: f64,
    angles: &[f64],
) -> Vec<Vec2> {
    let rs = 2.0 * bh.mass;
    // Shadow angular radius ~ 3*sqrt(3)/2 * rs / D for Schwarzschild
    let shadow_radius = 3.0 * 3.0_f64.sqrt() / 2.0 * rs;

    // For Kerr, the shadow is distorted. Approximate:
    let a = bh.spin;
    angles.iter().map(|&phi| {
        // Kerr shadow displacement
        let r_shadow = shadow_radius * (1.0 - a * 0.1 * phi.cos());
        let x = bh.position.x + (r_shadow * phi.cos()) as f32;
        let y = bh.position.y + (r_shadow * phi.sin()) as f32;
        Vec2::new(x, y)
    }).collect()
}

/// Tidal force magnitude at radius r from a mass M, for an object of size L.
/// F_tidal = 2 * G * M * L / r^3
pub fn tidal_force(r: f64, mass: f64, size: f64) -> f64 {
    let G = 6.674e-11;
    2.0 * G * mass * size / (r * r * r)
}

/// Hawking temperature: T = hbar * c^3 / (8 * pi * G * M * k_B).
pub fn hawking_temperature(mass: f64) -> f64 {
    let hbar = 1.0546e-34;
    let c = 299_792_458.0;
    let G = 6.674e-11;
    let k_B = 1.381e-23;
    hbar * c * c * c / (8.0 * std::f64::consts::PI * G * mass * k_B)
}

/// Full black hole renderer.
#[derive(Debug, Clone)]
pub struct BlackHoleRenderer {
    pub bh: BlackHole,
    pub disk: Option<AccretionDisk>,
    pub observer_distance: f64,
    pub resolution: usize,
    pub show_shadow: bool,
    pub show_photon_ring: bool,
    pub show_accretion: bool,
}

impl BlackHoleRenderer {
    pub fn new(bh: BlackHole, observer_distance: f64) -> Self {
        Self {
            bh,
            disk: None,
            observer_distance,
            resolution: 64,
            show_shadow: true,
            show_photon_ring: true,
            show_accretion: true,
        }
    }

    pub fn with_disk(mut self, disk: AccretionDisk) -> Self {
        self.disk = Some(disk);
        self
    }

    /// Render the full black hole scene.
    pub fn render(&self) -> Vec<(Vec2, Vec4)> {
        let disk = self.disk.clone().unwrap_or(AccretionDisk::new(
            isco_radius(2.0 * self.bh.mass),
            10.0 * 2.0 * self.bh.mass,
        ));
        let observer = self.bh.position + Vec2::new(self.observer_distance as f32, 0.0);
        render_black_hole(&self.bh, &disk, observer, self.resolution)
    }

    /// Get shadow boundary points.
    pub fn shadow(&self) -> Vec<Vec2> {
        let n = 64;
        let angles: Vec<f64> = (0..n).map(|i| i as f64 / n as f64 * std::f64::consts::TAU).collect();
        shadow_boundary(&self.bh, self.observer_distance, &angles)
    }

    /// Get photon ring radius.
    pub fn photon_ring_radius(&self) -> f64 {
        photon_sphere_radius(2.0 * self.bh.mass)
    }

    /// Get ISCO radius.
    pub fn isco_r(&self) -> f64 {
        self.bh.isco(2.0 * self.bh.mass)
    }

    /// Compute tidal force at a given distance.
    pub fn tidal_at(&self, r: f64, object_size: f64) -> f64 {
        tidal_force(r, self.bh.mass, object_size)
    }
}

/// Capture cross section for a Schwarzschild black hole.
/// sigma = 27 * pi * rs^2 / 4
pub fn capture_cross_section(rs: f64) -> f64 {
    27.0 * std::f64::consts::PI * rs * rs / 4.0
}

/// Compute orbital period at radius r around a Schwarzschild BH.
/// Using Kepler's third law with GR correction.
pub fn orbital_period(r: f64, mass: f64) -> f64 {
    let G = 6.674e-11;
    2.0 * std::f64::consts::PI * (r * r * r / (G * mass)).sqrt()
}

/// Gravitational wave frequency from binary BH system at separation r.
pub fn gw_frequency(total_mass: f64, separation: f64) -> f64 {
    let G = 6.674e-11;
    let period = 2.0 * std::f64::consts::PI * (separation.powi(3) / (G * total_mass)).sqrt();
    2.0 / period // GW frequency is twice orbital frequency
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0;
    const G: f64 = 6.674e-11;
    const M_SUN: f64 = 1.989e30;

    #[test]
    fn test_schwarzschild_radius_sun() {
        let rs = schwarzschild_radius(M_SUN, G, C);
        // ~2953 meters
        assert!((rs - 2953.0).abs() < 10.0, "Solar rs: {} m", rs);
    }

    #[test]
    fn test_photon_sphere() {
        let rs = 10.0;
        assert!((photon_sphere_radius(rs) - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_isco() {
        let rs = 10.0;
        assert!((isco_radius(rs) - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_capture_cross_section() {
        let rs = 1.0;
        let sigma = capture_cross_section(rs);
        let expected = 27.0 * std::f64::consts::PI / 4.0;
        assert!((sigma - expected).abs() < 1e-10);
    }

    #[test]
    fn test_ray_deflection_weak_field() {
        let rs = 1.0;
        let b = 1000.0;
        let alpha = ray_deflection(b, rs);
        // Should be approximately 2*rs/b = 0.002
        assert!((alpha - 0.002).abs() < 0.001, "Weak field deflection: {}", alpha);
    }

    #[test]
    fn test_ray_deflection_strong_field() {
        let rs = 1.0;
        let b = 1.6 * rs; // just outside photon sphere
        let alpha = ray_deflection(b, rs);
        assert!(alpha > 1.0, "Strong field should have large deflection: {}", alpha);
    }

    #[test]
    fn test_ray_deflection_capture() {
        let rs = 1.0;
        let b = 1.4 * rs; // inside photon sphere
        let alpha = ray_deflection(b, rs);
        assert!((alpha - std::f64::consts::TAU).abs() < 1e-10, "Should be captured");
    }

    #[test]
    fn test_tidal_force_increases_with_proximity() {
        let f1 = tidal_force(1000.0, M_SUN, 1.0);
        let f2 = tidal_force(100.0, M_SUN, 1.0);
        assert!(f2 > f1, "Tidal force should increase closer: {} vs {}", f1, f2);
    }

    #[test]
    fn test_hawking_temperature() {
        let temp = hawking_temperature(M_SUN);
        // ~6e-8 K for solar mass
        assert!(temp > 1e-9 && temp < 1e-6, "Hawking T for sun: {} K", temp);

        // Smaller BH = hotter
        let temp2 = hawking_temperature(M_SUN * 0.1);
        assert!(temp2 > temp, "Smaller BH should be hotter");
    }

    #[test]
    fn test_accretion_disk() {
        let disk = AccretionDisk::new(6.0, 100.0);
        assert!(disk.temperature(6.0) > 0.0);
        assert!(disk.temperature(5.0) == 0.0); // inside ISCO
        assert!(disk.temperature(200.0) == 0.0); // outside disk

        let e1 = disk_emission(6.0, &disk);
        let e2 = disk_emission(50.0, &disk);
        assert!(e1 > e2, "Inner disk should be brighter");
    }

    #[test]
    fn test_shadow_boundary() {
        let bh = BlackHole::new(1.0, Vec2::ZERO, 0.0);
        let angles: Vec<f64> = (0..8).map(|i| i as f64 * std::f64::consts::PI / 4.0).collect();
        let boundary = shadow_boundary(&bh, 100.0, &angles);
        assert_eq!(boundary.len(), 8);
        // Should be roughly circular for non-spinning BH
        let radii: Vec<f32> = boundary.iter().map(|p| p.length()).collect();
        let mean = radii.iter().sum::<f32>() / radii.len() as f32;
        for r in &radii {
            assert!((r - mean).abs() / mean < 0.01, "Should be circular for a=0");
        }
    }

    #[test]
    fn test_kerr_isco() {
        let rs = 10.0;
        let bh_static = BlackHole::new(5.0, Vec2::ZERO, 0.0);
        let bh_spinning = BlackHole::new(5.0, Vec2::ZERO, 0.9);
        let isco_s = bh_static.isco(rs);
        let isco_k = bh_spinning.isco(rs);
        // Spinning BH has smaller prograde ISCO
        assert!(isco_k < isco_s, "Kerr ISCO {} should be smaller than Schwarzschild {}", isco_k, isco_s);
    }

    #[test]
    fn test_render_black_hole() {
        let bh = BlackHole::new(1.0, Vec2::ZERO, 0.0);
        let disk = AccretionDisk::new(3.0, 10.0);
        let pixels = render_black_hole(&bh, &disk, Vec2::new(50.0, 0.0), 8);
        assert_eq!(pixels.len(), 64);
    }

    #[test]
    fn test_ray_trace() {
        let bh = BlackHole::new(1.0, Vec2::ZERO, 0.0);
        let path = ray_trace_schwarzschild(Vec2::new(20.0, 5.0), Vec2::new(-1.0, 0.0), &bh, 100);
        assert!(!path.is_empty());
    }
}
