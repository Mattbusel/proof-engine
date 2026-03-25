//! Gravitational time dilation.

use glam::{Vec2, Vec4};

/// Schwarzschild time dilation factor: sqrt(1 - rs/r).
/// Returns 0 at the event horizon (r = rs), imaginary inside.
pub fn schwarzschild_time_dilation(r: f64, rs: f64) -> f64 {
    if r <= rs {
        return 0.0;
    }
    (1.0 - rs / r).sqrt()
}

/// Gravitational redshift between two radii in Schwarzschild geometry.
/// f_obs / f_emit = sqrt((1 - rs/r_emit) / (1 - rs/r_obs))
/// Returns the wavelength ratio: lambda_obs / lambda_emit.
pub fn gravitational_redshift(r_emit: f64, r_obs: f64, rs: f64) -> f64 {
    let factor_emit = schwarzschild_time_dilation(r_emit, rs);
    let factor_obs = schwarzschild_time_dilation(r_obs, rs);
    if factor_obs.abs() < 1e-15 {
        return f64::INFINITY;
    }
    factor_emit / factor_obs
}

/// Proper time rate at radius r around a mass M.
/// d(tau)/dt = sqrt(1 - 2GM/(rc^2))
#[allow(non_snake_case)]
pub fn proper_time_rate(r: f64, mass: f64, G: f64, c: f64) -> f64 {
    let rs = 2.0 * G * mass / (c * c);
    schwarzschild_time_dilation(r, rs)
}

/// GPS correction: combined special + general relativistic time correction.
/// Returns the correction in seconds per day.
///
/// GR effect: clocks higher in gravity run faster.
/// SR effect: moving clocks run slower.
pub fn gps_correction(orbit_radius: f64, earth_mass: f64, earth_radius: f64) -> f64 {
    let c = 299_792_458.0;
    let G = 6.674e-11;
    let rs = 2.0 * G * earth_mass / (c * c);

    // GR: rate at orbit vs surface
    let gr_surface = schwarzschild_time_dilation(earth_radius, rs);
    let gr_orbit = schwarzschild_time_dilation(orbit_radius, rs);
    // Fractional GR difference (orbit clock runs faster)
    let gr_frac = gr_orbit / gr_surface - 1.0;

    // SR: orbital velocity
    let v_orbit = (G * earth_mass / orbit_radius).sqrt();
    let beta = v_orbit / c;
    // SR time dilation (moving clock runs slow)
    let sr_frac = -0.5 * beta * beta; // to first order

    let total_frac = gr_frac + sr_frac;
    total_frac * 86400.0 // seconds per day
}

/// 2D grid of time dilation factors around a mass.
#[derive(Debug, Clone)]
pub struct GravTimeDilationField {
    pub width: usize,
    pub height: usize,
    pub center: Vec2,
    pub rs: f64,
    pub factors: Vec<f64>,
    pub cell_size: f32,
}

impl GravTimeDilationField {
    pub fn new(width: usize, height: usize, center: Vec2, mass: f64, c: f64, g_const: f64, cell_size: f32) -> Self {
        let rs = 2.0 * g_const * mass / (c * c);
        let mut factors = Vec::with_capacity(width * height);
        for iy in 0..height {
            for ix in 0..width {
                let x = (ix as f32 - width as f32 / 2.0) * cell_size + center.x;
                let y = (iy as f32 - height as f32 / 2.0) * cell_size + center.y;
                let r = ((x - center.x).powi(2) + (y - center.y).powi(2)).sqrt() as f64;
                factors.push(schwarzschild_time_dilation(r, rs));
            }
        }
        Self { width, height, center, rs, factors, cell_size }
    }

    /// Get the time dilation factor at a grid position.
    pub fn get(&self, ix: usize, iy: usize) -> f64 {
        if ix < self.width && iy < self.height {
            self.factors[iy * self.width + ix]
        } else {
            1.0
        }
    }

    /// Sample the time dilation factor at an arbitrary position.
    pub fn sample(&self, pos: Vec2) -> f64 {
        let r = (pos - self.center).length() as f64;
        schwarzschild_time_dilation(r, self.rs)
    }

    /// Get the Schwarzschild radius.
    pub fn schwarzschild_radius(&self) -> f64 {
        self.rs
    }

    /// Find the radius where time dilation equals a given factor.
    pub fn radius_for_factor(&self, factor: f64) -> f64 {
        // factor = sqrt(1 - rs/r) => factor^2 = 1 - rs/r => r = rs / (1 - factor^2)
        if factor >= 1.0 {
            return f64::INFINITY;
        }
        if factor <= 0.0 {
            return self.rs;
        }
        self.rs / (1.0 - factor * factor)
    }
}

/// Render clocks with tick rates adjusted by local gravitational time dilation.
#[derive(Debug, Clone)]
pub struct GravTimeRenderer {
    pub field: GravTimeDilationField,
    pub clock_size: f32,
}

impl GravTimeRenderer {
    pub fn new(field: GravTimeDilationField) -> Self {
        Self {
            field,
            clock_size: 1.0,
        }
    }

    /// Get the tick rate at a given position (0 to 1, where 0 = frozen at horizon).
    pub fn tick_rate_at(&self, pos: Vec2) -> f32 {
        self.field.sample(pos) as f32
    }

    /// Compute clock hand angle at a position given coordinate time.
    pub fn clock_angle_at(&self, pos: Vec2, coordinate_time: f64) -> f32 {
        let rate = self.field.sample(pos);
        let proper_time = coordinate_time * rate;
        let seconds = proper_time % 60.0;
        (seconds / 60.0 * std::f64::consts::TAU) as f32
    }

    /// Generate clock visualization data for multiple positions.
    pub fn render_clocks(
        &self,
        positions: &[Vec2],
        coordinate_time: f64,
    ) -> Vec<(Vec2, f32, f32)> {
        // Returns (position, hand_angle, tick_rate)
        positions.iter().map(|pos| {
            let rate = self.field.sample(*pos);
            let angle = self.clock_angle_at(*pos, coordinate_time);
            (*pos, angle, rate as f32)
        }).collect()
    }

    /// Compute the color for a clock based on its time dilation.
    /// Blue = fast (far from mass), red = slow (near mass).
    pub fn clock_color(&self, pos: Vec2) -> Vec4 {
        let rate = self.field.sample(pos) as f32;
        Vec4::new(1.0 - rate, 0.2, rate, 1.0)
    }
}

/// Compute the Shapiro time delay for a signal passing near a mass.
/// Extra delay = (4GM/c^3) * ln(4 * r1 * r2 / b^2)
/// where r1, r2 are distances of emitter/receiver from the mass, b is closest approach.
#[allow(non_snake_case)]
pub fn shapiro_delay(mass: f64, r1: f64, r2: f64, b: f64, G: f64, c: f64) -> f64 {
    if b <= 0.0 {
        return f64::INFINITY;
    }
    let factor = 4.0 * G * mass / (c * c * c);
    factor * (4.0 * r1 * r2 / (b * b)).ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0;
    const G: f64 = 6.674e-11;

    #[test]
    fn test_schwarzschild_at_infinity() {
        let factor = schwarzschild_time_dilation(1e20, 1.0);
        assert!((factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_schwarzschild_at_horizon() {
        let factor = schwarzschild_time_dilation(1.0, 1.0);
        assert!(factor.abs() < 1e-10, "Should be zero at horizon: {}", factor);
    }

    #[test]
    fn test_schwarzschild_inside_horizon() {
        let factor = schwarzschild_time_dilation(0.5, 1.0);
        assert_eq!(factor, 0.0, "Inside horizon returns 0");
    }

    #[test]
    fn test_gravitational_redshift() {
        // Light emitted near horizon, observed far away, should be heavily redshifted
        let rs = 1.0;
        let ratio = gravitational_redshift(1.1 * rs, 1000.0 * rs, rs);
        // ratio = sqrt(1-1/1.1) / sqrt(1-1/1000) ~ sqrt(0.0909) / 1 ~ 0.3015
        assert!(ratio < 1.0, "Should be redshifted: {}", ratio);
    }

    #[test]
    fn test_gps_correction_approximately_38us() {
        let orbit_r = 26_571_000.0; // GPS orbit
        let earth_mass = 5.972e24;
        let earth_r = 6_371_000.0;
        let correction = gps_correction(orbit_r, earth_mass, earth_r);
        let correction_us = correction * 1e6;
        assert!(
            (correction_us - 38.0).abs() < 10.0,
            "GPS correction: {} us/day, expected ~38",
            correction_us
        );
    }

    #[test]
    fn test_proper_time_rate() {
        let earth_mass = 5.972e24;
        let rate = proper_time_rate(6_371_000.0, earth_mass, G, C);
        // Should be very close to 1.0
        assert!((rate - 1.0).abs() < 1e-8);
        assert!(rate < 1.0, "Surface clock runs slow vs infinity");
    }

    #[test]
    fn test_grav_time_field() {
        let field = GravTimeDilationField::new(
            20, 20, Vec2::ZERO, 1e30, C, G, 100.0,
        );
        // Center should have some dilation
        let center_factor = field.get(10, 10);
        // Far corner should be closer to 1
        let corner_factor = field.get(0, 0);
        // Both should be positive
        assert!(center_factor >= 0.0);
        assert!(corner_factor >= 0.0);
    }

    #[test]
    fn test_radius_for_factor() {
        let field = GravTimeDilationField::new(10, 10, Vec2::ZERO, 1e30, C, G, 1.0);
        let r = field.radius_for_factor(0.5);
        // Verify: sqrt(1 - rs/r) = 0.5 => 1 - rs/r = 0.25 => r = rs/0.75
        let expected = field.rs / 0.75;
        assert!((r - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn test_shapiro_delay_positive() {
        let m_sun = 1.989e30;
        let delay = shapiro_delay(m_sun, 1.5e11, 1.5e11, 6.96e8, G, C);
        // Should be positive and on the order of ~200 microseconds for the sun
        assert!(delay > 0.0);
        let delay_us = delay * 1e6;
        assert!(delay_us > 100.0 && delay_us < 500.0,
            "Shapiro delay: {} us", delay_us);
    }

    #[test]
    fn test_grav_time_renderer() {
        let field = GravTimeDilationField::new(10, 10, Vec2::ZERO, 1e30, C, G, 1000.0);
        let renderer = GravTimeRenderer::new(field);
        let rate = renderer.tick_rate_at(Vec2::new(1000.0, 0.0));
        assert!(rate > 0.0 && rate <= 1.0);
    }
}
