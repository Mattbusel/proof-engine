//! Relativistic searchlight/beaming effect.

use glam::{Vec3, Vec4};
use super::lorentz::lorentz_factor;

/// Relativistic beaming: intensity transformation for a source moving at speed v.
/// I_obs = I_rest / (gamma^3 * (1 - beta*cos(angle))^3)
/// where angle is measured in the observer frame between velocity and line of sight.
pub fn relativistic_beaming(intensity_rest: f64, v: f64, c: f64, angle: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    let d = gamma * (1.0 - beta * angle.cos());
    if d.abs() < 1e-15 {
        return intensity_rest * 1e15;
    }
    intensity_rest / (d * d * d)
}

/// Relativistic aberration: convert angle in source rest frame to observer frame.
/// cos(theta_obs) = (cos(theta_rest) + beta) / (1 + beta * cos(theta_rest))
pub fn aberration_angle(theta_rest: f64, v: f64, c: f64) -> f64 {
    let beta = v / c;
    let cos_rest = theta_rest.cos();
    let cos_obs = (cos_rest + beta) / (1.0 + beta * cos_rest);
    cos_obs.clamp(-1.0, 1.0).acos()
}

/// Headlight factor: forward intensity boost for a source moving at speed v.
/// This is the beaming factor at angle = 0 (directly forward).
/// D = 1 / (gamma * (1 - beta)), so I_obs = I_rest * D^3.
pub fn headlight_factor(v: f64, c: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    let d = 1.0 / (gamma * (1.0 - beta));
    d * d * d
}

/// Renderer that modifies entity brightness based on velocity direction relative to observer.
#[derive(Debug, Clone)]
pub struct SearchlightRenderer {
    pub c: f64,
    pub observer_pos: Vec3,
    pub max_boost: f32,
}

impl SearchlightRenderer {
    pub fn new(c: f64, observer_pos: Vec3) -> Self {
        Self {
            c,
            observer_pos,
            max_boost: 100.0,
        }
    }

    /// Compute the apparent brightness of an entity given its position, velocity, and base luminosity.
    pub fn entity_brightness(
        &self,
        entity_pos: Vec3,
        entity_velocity: Vec3,
        base_luminosity: f32,
    ) -> f32 {
        let v = entity_velocity.length() as f64;
        if v < 1e-10 {
            return base_luminosity;
        }

        let to_observer = (self.observer_pos - entity_pos).normalize_or_zero();
        let vel_dir = entity_velocity.normalize();
        let cos_angle = vel_dir.dot(to_observer) as f64;
        let angle = cos_angle.clamp(-1.0, 1.0).acos();

        let beamed = relativistic_beaming(base_luminosity as f64, v, self.c, angle);
        (beamed as f32).min(self.max_boost * base_luminosity)
    }

    /// Compute brightness for multiple entities.
    pub fn batch_brightness(
        &self,
        entities: &[(Vec3, Vec3, f32)], // (pos, velocity, base_luminosity)
    ) -> Vec<f32> {
        entities.iter().map(|(pos, vel, lum)| {
            self.entity_brightness(*pos, *vel, *lum)
        }).collect()
    }

    /// Apply beaming to a color by scaling its RGB components.
    pub fn beamed_color(
        &self,
        entity_pos: Vec3,
        entity_velocity: Vec3,
        base_color: Vec4,
    ) -> Vec4 {
        let factor = self.entity_brightness(entity_pos, entity_velocity, 1.0);
        Vec4::new(
            (base_color.x * factor).min(1.0),
            (base_color.y * factor).min(1.0),
            (base_color.z * factor).min(1.0),
            base_color.w,
        )
    }

    /// Compute the half-angle of the beaming cone (angle where intensity drops to half of forward max).
    pub fn beaming_half_angle(&self, v: f64) -> f64 {
        let beta = v / self.c;
        let gamma = lorentz_factor(v, self.c);
        // Approximate: theta_half ~ 1/gamma
        (1.0 / gamma).asin()
    }
}

/// Apparent brightness including distance and beaming.
/// L_obs = luminosity * D^3 / (4*pi*r^2) where D is the Doppler factor.
pub fn apparent_brightness(luminosity: f64, v: f64, c: f64, angle: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    let d = 1.0 / (gamma * (1.0 - beta * angle.cos()));
    luminosity * d * d * d
}

/// Solid angle transformation under Lorentz boost.
/// d_omega_obs = d_omega_rest / (gamma^2 * (1 - beta*cos(theta))^2)
pub fn solid_angle_transform(d_omega_rest: f64, v: f64, c: f64, theta: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    let denom = gamma * (1.0 - beta * theta.cos());
    d_omega_rest / (denom * denom)
}

/// Backward dimming factor (angle = pi).
pub fn backward_dimming(v: f64, c: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    let d = 1.0 / (gamma * (1.0 + beta));
    d * d * d
}

/// Compute the Doppler factor D for a given angle.
/// D = 1 / (gamma * (1 - beta * cos(theta)))
pub fn doppler_boost_factor(v: f64, c: f64, theta: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    1.0 / (gamma * (1.0 - beta * theta.cos()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const C: f64 = 299_792_458.0;

    #[test]
    fn test_forward_brightness_boost() {
        let fwd = relativistic_beaming(1.0, 0.9 * C, C, 0.0);
        assert!(fwd > 1.0, "Forward should be boosted: {}", fwd);
        assert!(fwd > 100.0, "At 0.9c forward boost should be large: {}", fwd);
    }

    #[test]
    fn test_backward_dimming() {
        let bwd = relativistic_beaming(1.0, 0.9 * C, C, PI);
        assert!(bwd < 1.0, "Backward should be dimmed: {}", bwd);
    }

    #[test]
    fn test_headlight_factor_increases_with_v() {
        let h1 = headlight_factor(0.5 * C, C);
        let h2 = headlight_factor(0.9 * C, C);
        assert!(h2 > h1, "Higher v should give more forward boost: {} vs {}", h1, h2);
    }

    #[test]
    fn test_headlight_at_rest() {
        let h = headlight_factor(0.0, C);
        assert!((h - 1.0).abs() < 1e-6, "At rest, headlight factor = 1: {}", h);
    }

    #[test]
    fn test_aberration_forward() {
        // Isotropic emission at pi/2 in rest frame gets beamed forward
        let theta_obs = aberration_angle(std::f64::consts::FRAC_PI_2, 0.9 * C, C);
        assert!(theta_obs < std::f64::consts::FRAC_PI_2, "Should be aberrated forward: {}", theta_obs);
    }

    #[test]
    fn test_aberration_zero_velocity() {
        let theta_obs = aberration_angle(1.0, 0.0, C);
        assert!((theta_obs - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_total_luminosity_conservation() {
        // Numerically integrate beamed intensity over solid angle.
        // Total power should be conserved (scales as gamma^2 in directed case,
        // but integrating D^3 * sin(theta) d(theta) should give a consistent result).
        let v = 0.5 * C;
        let n = 10000;
        let mut sum_beamed = 0.0;
        let mut sum_rest = 0.0;
        let dtheta = PI / n as f64;
        for i in 0..n {
            let theta = (i as f64 + 0.5) * dtheta;
            let d_omega = 2.0 * PI * theta.sin() * dtheta;
            sum_beamed += relativistic_beaming(1.0, v, C, theta) * d_omega;
            sum_rest += 1.0 * d_omega;
        }
        // The total should be boosted by gamma^2 for a moving source
        // Actually total radiated power transforms as P_obs = P_rest (Lorentz invariant for total)
        // but beaming redistributes it. For isotropic rest emission:
        // integral of D^3 over solid angle = 4*pi * gamma^2
        let gamma = lorentz_factor(v, C);
        let expected_ratio = gamma * gamma;
        let actual_ratio = sum_beamed / sum_rest;
        assert!(
            (actual_ratio - expected_ratio).abs() / expected_ratio < 0.05,
            "Luminosity ratio: {} expected: {}",
            actual_ratio, expected_ratio
        );
    }

    #[test]
    fn test_solid_angle_transform() {
        let d_omega = solid_angle_transform(1.0, 0.0, C, 0.0);
        assert!((d_omega - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_searchlight_renderer() {
        let renderer = SearchlightRenderer::new(C, Vec3::new(0.0, 0.0, 0.0));
        // Object moving toward observer should be bright
        let bright = renderer.entity_brightness(
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(-0.9 * C as f32, 0.0, 0.0),
            1.0,
        );
        // Object moving away should be dim
        let dim = renderer.entity_brightness(
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.9 * C as f32, 0.0, 0.0),
            1.0,
        );
        assert!(bright > dim, "Forward should be brighter: {} vs {}", bright, dim);
    }
}
