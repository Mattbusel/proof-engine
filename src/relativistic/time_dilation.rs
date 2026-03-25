//! Time dilation effects for special and general relativity.

use glam::{Vec2, Vec3, Vec4};
use super::lorentz::lorentz_factor;

/// Time dilation factor: returns gamma. Proper time interval = coordinate time / gamma.
pub fn time_dilation_factor(v: f64, c: f64) -> f64 {
    lorentz_factor(v, c)
}

/// A clock experiencing time dilation due to velocity.
#[derive(Debug, Clone)]
pub struct DilatedClock {
    /// Accumulated proper time on this clock.
    pub proper_time: f64,
    /// Velocity of the clock relative to the observer's rest frame.
    pub velocity: f64,
    /// Speed of light.
    pub c: f64,
    /// Accumulated coordinate (observer) time.
    pub accumulated: f64,
}

impl DilatedClock {
    pub fn new(velocity: f64, c: f64) -> Self {
        Self {
            proper_time: 0.0,
            velocity,
            c,
            accumulated: 0.0,
        }
    }

    /// Advance the clock by `dt_observer` seconds of observer time.
    /// The proper time advances more slowly by factor 1/gamma.
    pub fn tick(&mut self, dt_observer: f64) {
        self.accumulated += dt_observer;
        let gamma = lorentz_factor(self.velocity, self.c);
        self.proper_time += dt_observer / gamma;
    }

    /// Get the ratio of proper time to coordinate time.
    pub fn rate(&self) -> f64 {
        1.0 / lorentz_factor(self.velocity, self.c)
    }

    /// Reset the clock.
    pub fn reset(&mut self) {
        self.proper_time = 0.0;
        self.accumulated = 0.0;
    }

    /// Set a new velocity (e.g., for acceleration phases).
    pub fn set_velocity(&mut self, v: f64) {
        self.velocity = v;
    }

    /// Get the current time difference between coordinate and proper time.
    pub fn lag(&self) -> f64 {
        self.accumulated - self.proper_time
    }
}

/// Twin paradox calculation.
/// Given a distance (one way) and travel speed v,
/// returns (traveler_time, stay_at_home_time).
/// Assumes instantaneous turnaround.
pub fn twin_paradox(distance: f64, v: f64, c: f64) -> (f64, f64) {
    let gamma = lorentz_factor(v, c);
    let stay_time = 2.0 * distance / v;
    let traveler_time = stay_time / gamma;
    (traveler_time, stay_time)
}

/// Render clocks with tick rate proportional to proper time rate.
#[derive(Debug, Clone)]
pub struct TimeDilationRenderer {
    pub c: f64,
    pub show_clock_hands: bool,
    pub clock_size: f32,
}

impl TimeDilationRenderer {
    pub fn new(c: f64) -> Self {
        Self {
            c,
            show_clock_hands: true,
            clock_size: 1.0,
        }
    }

    /// Compute the apparent tick rate of a clock at velocity v.
    /// Returns a fraction of normal tick rate (0 to 1).
    pub fn tick_rate(&self, v: f64) -> f32 {
        let gamma = lorentz_factor(v, self.c);
        (1.0 / gamma) as f32
    }

    /// Compute the clock hand angle for a given proper time.
    /// One full rotation = 60 "seconds" of proper time.
    pub fn clock_hand_angle(&self, proper_time: f64) -> f32 {
        let seconds = proper_time % 60.0;
        (seconds / 60.0 * std::f64::consts::TAU) as f32
    }

    /// Generate glyph data for a clock face at a position.
    /// Returns positions for 12 hour markers and the hand endpoint.
    pub fn clock_glyph_data(
        &self,
        center: Vec2,
        proper_time: f64,
    ) -> (Vec<Vec2>, Vec2) {
        let mut markers = Vec::with_capacity(12);
        let radius = self.clock_size;
        for i in 0..12 {
            let angle = (i as f32 / 12.0) * std::f32::consts::TAU;
            markers.push(center + Vec2::new(angle.cos() * radius, angle.sin() * radius));
        }
        let hand_angle = self.clock_hand_angle(proper_time);
        let hand_tip = center + Vec2::new(
            hand_angle.cos() * radius * 0.8,
            hand_angle.sin() * radius * 0.8,
        );
        (markers, hand_tip)
    }

    /// Render multiple clocks at different velocities.
    pub fn render_clocks(
        &self,
        clocks: &[DilatedClock],
        positions: &[Vec2],
    ) -> Vec<(Vec<Vec2>, Vec2)> {
        clocks.iter().zip(positions.iter()).map(|(clock, pos)| {
            self.clock_glyph_data(*pos, clock.proper_time)
        }).collect()
    }
}

/// Dilated muon lifetime. Rest lifetime ~ 2.2 microseconds.
/// Returns the dilated lifetime at speed v.
pub fn muon_lifetime(v: f64) -> f64 {
    let c = 299_792_458.0;
    let rest_lifetime = 2.2e-6; // seconds
    let gamma = lorentz_factor(v, c);
    rest_lifetime * gamma
}

/// Gravitational time dilation (weak field approximation).
/// dt_high / dt_low = 1 + g*h/c^2 (to first order).
/// Returns the fractional time difference for height_diff.
pub fn gravitational_time_dilation(height_diff: f64, g: f64, c: f64) -> f64 {
    g * height_diff / (c * c)
}

/// Visualize two clocks at different velocities/heights side by side.
#[derive(Debug, Clone)]
pub struct ClockComparison {
    pub clock_a: DilatedClock,
    pub clock_b: DilatedClock,
    pub position_a: Vec2,
    pub position_b: Vec2,
    pub label_a: String,
    pub label_b: String,
}

impl ClockComparison {
    pub fn new(
        v_a: f64,
        v_b: f64,
        c: f64,
        pos_a: Vec2,
        pos_b: Vec2,
    ) -> Self {
        Self {
            clock_a: DilatedClock::new(v_a, c),
            clock_b: DilatedClock::new(v_b, c),
            position_a: pos_a,
            position_b: pos_b,
            label_a: format!("v = {:.2}c", v_a / c),
            label_b: format!("v = {:.2}c", v_b / c),
        }
    }

    /// Advance both clocks by the same observer time.
    pub fn tick(&mut self, dt_observer: f64) {
        self.clock_a.tick(dt_observer);
        self.clock_b.tick(dt_observer);
    }

    /// Get the time difference between the two clocks.
    pub fn time_difference(&self) -> f64 {
        self.clock_a.proper_time - self.clock_b.proper_time
    }

    /// Get the ratio of proper times.
    pub fn time_ratio(&self) -> f64 {
        if self.clock_b.proper_time.abs() < 1e-15 {
            return 1.0;
        }
        self.clock_a.proper_time / self.clock_b.proper_time
    }

    /// Reset both clocks.
    pub fn reset(&mut self) {
        self.clock_a.reset();
        self.clock_b.reset();
    }

    /// Generate rendering data for both clocks.
    pub fn render_data(&self, renderer: &TimeDilationRenderer) -> ((Vec<Vec2>, Vec2), (Vec<Vec2>, Vec2)) {
        let a = renderer.clock_glyph_data(self.position_a, self.clock_a.proper_time);
        let b = renderer.clock_glyph_data(self.position_b, self.clock_b.proper_time);
        (a, b)
    }
}

/// Compute the GPS time correction needed per day.
/// Combines special relativistic (velocity) and general relativistic (gravity) effects.
/// Returns the correction in seconds per day.
pub fn gps_time_correction(orbit_radius: f64, earth_mass: f64, earth_radius: f64) -> f64 {
    let c = 299_792_458.0;
    let G = 6.674e-11;

    // Orbital velocity for circular orbit: v = sqrt(GM/r)
    let v_sat = (G * earth_mass / orbit_radius).sqrt();

    // SR effect: satellite clock runs slow by -v^2/(2c^2) per unit time (negative = slow)
    let sr_correction = -v_sat * v_sat / (2.0 * c * c);

    // GR effect: satellite clock runs fast by GM/(rc^2) - GM/(R_earth c^2)
    let gr_correction = G * earth_mass / (earth_radius * c * c) - G * earth_mass / (orbit_radius * c * c);

    // Total fractional correction per second, then scale to seconds per day
    let total_fractional = sr_correction + gr_correction;
    total_fractional * 86400.0
}

/// Distance a muon can travel at speed v given dilated lifetime.
pub fn muon_travel_distance(v: f64) -> f64 {
    let lifetime = muon_lifetime(v);
    v * lifetime
}

/// Multi-step time dilation for a clock undergoing varying velocity.
/// Takes a series of (duration, velocity) segments and returns total proper time.
pub fn integrated_proper_time(segments: &[(f64, f64)], c: f64) -> f64 {
    let mut total = 0.0;
    for &(dt, v) in segments {
        let gamma = lorentz_factor(v, c);
        total += dt / gamma;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0;

    #[test]
    fn test_time_dilation_factor_at_rest() {
        let factor = time_dilation_factor(0.0, C);
        assert!((factor - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_time_dilation_factor_half_c() {
        let factor = time_dilation_factor(0.5 * C, C);
        let expected = 1.0 / (1.0 - 0.25_f64).sqrt();
        assert!((factor - expected).abs() < 1e-6);
    }

    #[test]
    fn test_dilated_clock() {
        let mut clock = DilatedClock::new(0.866 * C, C);
        // gamma ~ 2
        clock.tick(10.0);
        // proper time ~ 5
        assert!((clock.proper_time - 5.0).abs() < 0.1);
        assert!((clock.accumulated - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_twin_paradox() {
        // Travel to a star 4 light-years away at 0.8c
        let distance = 4.0 * C * 365.25 * 86400.0; // 4 light-years in meters
        let v = 0.8 * C;
        let (traveler, stay) = twin_paradox(distance, v, C);

        // stay time = 2 * 4ly / 0.8c = 10 years
        let stay_years = stay / (365.25 * 86400.0);
        assert!((stay_years - 10.0).abs() < 0.01);

        // gamma at 0.8c = 5/3, so traveler_time = 10 / (5/3) = 6 years
        let traveler_years = traveler / (365.25 * 86400.0);
        assert!((traveler_years - 6.0).abs() < 0.01);

        // Traveler ages less
        assert!(traveler < stay);
    }

    #[test]
    fn test_muon_reaches_ground() {
        let v = 0.998 * C;
        let distance = muon_travel_distance(v);
        // Atmosphere is ~15 km. Without dilation, muon travels ~660m.
        // With dilation at 0.998c, gamma ~ 15.8, distance ~ 10.4 km.
        // At even higher speeds they exceed 15 km.
        let rest_distance = v * 2.2e-6;
        assert!(distance > rest_distance);
        // Check that dilated distance is much larger
        assert!(distance > 5.0 * rest_distance);
    }

    #[test]
    fn test_muon_lifetime_dilation() {
        let v = 0.99 * C;
        let dilated = muon_lifetime(v);
        let rest = 2.2e-6;
        let gamma = lorentz_factor(v, C);
        assert!((dilated - rest * gamma).abs() < 1e-15);
    }

    #[test]
    fn test_gravitational_time_dilation_weak_field() {
        // At 1 meter height diff with g=9.8
        let frac = gravitational_time_dilation(1.0, 9.8, C);
        // Should be about 1.09e-16
        assert!(frac > 0.0);
        assert!(frac < 1e-14);
    }

    #[test]
    fn test_gps_correction() {
        // GPS satellite orbit radius ~ 26,571 km = 26_571_000 m
        let orbit_r = 26_571_000.0;
        let earth_mass = 5.972e24;
        let earth_r = 6_371_000.0;

        let correction = gps_time_correction(orbit_r, earth_mass, earth_r);
        // GPS correction is approximately +38 microseconds/day
        let correction_us = correction * 1e6;
        assert!(
            (correction_us - 38.0).abs() < 10.0,
            "GPS correction: {} us/day, expected ~38",
            correction_us
        );
    }

    #[test]
    fn test_clock_comparison() {
        let mut comp = ClockComparison::new(
            0.0, 0.866 * C, C,
            Vec2::new(-5.0, 0.0),
            Vec2::new(5.0, 0.0),
        );
        comp.tick(10.0);
        assert!((comp.clock_a.proper_time - 10.0).abs() < 1e-10);
        assert!((comp.clock_b.proper_time - 5.0).abs() < 0.1);
        assert!(comp.time_difference() > 0.0);
    }

    #[test]
    fn test_integrated_proper_time() {
        let segments = vec![
            (5.0, 0.0),       // at rest for 5s
            (5.0, 0.866 * C), // at 0.866c for 5s (gamma~2)
        ];
        let tau = integrated_proper_time(&segments, C);
        // 5 + 5/2 = 7.5
        assert!((tau - 7.5).abs() < 0.1);
    }

    #[test]
    fn test_dilated_clock_lag() {
        let mut clock = DilatedClock::new(0.6 * C, C);
        clock.tick(100.0);
        assert!(clock.lag() > 0.0);
        // gamma at 0.6c = 1.25, proper = 80, lag = 20
        assert!((clock.lag() - 20.0).abs() < 0.1);
    }
}
