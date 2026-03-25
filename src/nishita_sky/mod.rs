//! Nishita atmospheric sky model.
//!
//! Clean-room implementation based on:
//! - Nishita et al., "Display of the Earth Taking into Account Atmospheric Scattering" (SIGGRAPH 1993)
//! - Bruneton & Neyret, "Precomputed Atmospheric Scattering" (EGSR 2008)
//!
//! Computes sky color and aerial perspective from physical Rayleigh and Mie scattering.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

// Physical constants
const EARTH_RADIUS: f32 = 6_371_000.0;      // meters
const ATMO_RADIUS: f32 = 6_471_000.0;       // top of atmosphere
const RAYLEIGH_SCALE_HEIGHT: f32 = 8_500.0;  // meters
const MIE_SCALE_HEIGHT: f32 = 1_200.0;       // meters
const RAYLEIGH_COEFF: Vec3 = Vec3::new(5.5e-6, 13.0e-6, 22.4e-6); // per meter (RGB)
const MIE_COEFF: f32 = 21.0e-6;             // per meter
const MIE_G: f32 = 0.76;                    // Mie scattering anisotropy

/// Configuration for the sky renderer.
#[derive(Debug, Clone)]
pub struct SkyConfig {
    pub sun_direction: Vec3,
    pub sun_intensity: f32,
    pub sun_color: Vec3,
    pub num_samples: u32,
    pub num_light_samples: u32,
    /// Ground albedo for multi-scattering approximation.
    pub ground_albedo: Vec3,
    /// Observer height above ground in meters.
    pub observer_height: f32,
}

impl Default for SkyConfig {
    fn default() -> Self {
        Self {
            sun_direction: Vec3::new(0.0, 0.3, -1.0).normalize(),
            sun_intensity: 22.0,
            sun_color: Vec3::ONE,
            num_samples: 16,
            num_light_samples: 8,
            ground_albedo: Vec3::splat(0.3),
            observer_height: 1.0,
        }
    }
}

/// Rayleigh phase function.
fn rayleigh_phase(cos_theta: f32) -> f32 {
    3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta)
}

/// Henyey-Greenstein phase function for Mie scattering.
fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let num = 3.0 * (1.0 - g2) * (1.0 + cos_theta * cos_theta);
    let denom = (8.0 * PI) * (2.0 + g2) * (1.0 + g2 - 2.0 * g * cos_theta).powf(1.5);
    num / denom
}

/// Ray-sphere intersection. Returns (near, far) or None.
fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<(f32, f32)> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 { return None; }
    let sqrt_disc = disc.sqrt();
    Some((-b - sqrt_disc, -b + sqrt_disc))
}

/// Compute sky color for a given view direction.
pub fn compute_sky_color(view_dir: Vec3, config: &SkyConfig) -> Vec3 {
    let origin = Vec3::new(0.0, EARTH_RADIUS + config.observer_height, 0.0);

    // Intersect view ray with atmosphere
    let (_, t_atmo) = match ray_sphere(origin, view_dir, Vec3::ZERO, ATMO_RADIUS) {
        Some(t) => t,
        None => return Vec3::ZERO,
    };

    // Check if ray hits earth
    let t_max = if let Some((t_near, _)) = ray_sphere(origin, view_dir, Vec3::ZERO, EARTH_RADIUS) {
        if t_near > 0.0 { t_near } else { t_atmo }
    } else {
        t_atmo
    };

    let segment_length = t_max / config.num_samples as f32;
    let sun_dir = config.sun_direction.normalize();
    let cos_theta = view_dir.dot(sun_dir);

    let phase_r = rayleigh_phase(cos_theta);
    let phase_m = mie_phase(cos_theta, MIE_G);

    let mut total_rayleigh = Vec3::ZERO;
    let mut total_mie = Vec3::ZERO;
    let mut optical_depth_r = 0.0f32;
    let mut optical_depth_m = 0.0f32;

    for i in 0..config.num_samples {
        let t = (i as f32 + 0.5) * segment_length;
        let sample_pos = origin + view_dir * t;
        let height = sample_pos.length() - EARTH_RADIUS;
        if height < 0.0 { break; }

        // Density at this height
        let hr = (-height / RAYLEIGH_SCALE_HEIGHT).exp();
        let hm = (-height / MIE_SCALE_HEIGHT).exp();

        optical_depth_r += hr * segment_length;
        optical_depth_m += hm * segment_length;

        // Light ray toward sun from this sample
        let (_, t_sun) = match ray_sphere(sample_pos, sun_dir, Vec3::ZERO, ATMO_RADIUS) {
            Some(t) => t,
            None => continue,
        };

        let light_segment = t_sun / config.num_light_samples as f32;
        let mut od_light_r = 0.0f32;
        let mut od_light_m = 0.0f32;
        let mut shadow = false;

        for j in 0..config.num_light_samples {
            let tl = (j as f32 + 0.5) * light_segment;
            let light_pos = sample_pos + sun_dir * tl;
            let light_height = light_pos.length() - EARTH_RADIUS;
            if light_height < 0.0 { shadow = true; break; }
            od_light_r += (-light_height / RAYLEIGH_SCALE_HEIGHT).exp() * light_segment;
            od_light_m += (-light_height / MIE_SCALE_HEIGHT).exp() * light_segment;
        }

        if shadow { continue; }

        // Combined optical depth (view + light)
        let tau_r = RAYLEIGH_COEFF * (optical_depth_r + od_light_r);
        let tau_m = MIE_COEFF * (optical_depth_m + od_light_m);
        let attenuation = Vec3::new(
            (-tau_r.x - tau_m).exp(),
            (-tau_r.y - tau_m).exp(),
            (-tau_r.z - tau_m).exp(),
        );

        total_rayleigh += attenuation * hr * segment_length;
        total_mie += attenuation * hm * segment_length;
    }

    let sun = config.sun_color * config.sun_intensity;
    let sky = sun * (total_rayleigh * RAYLEIGH_COEFF * phase_r + total_mie * MIE_COEFF * phase_m);

    sky
}

/// Generate a sky lookup table (256x128) for fast runtime sampling.
pub fn generate_sky_lut(config: &SkyConfig) -> Vec<[f32; 3]> {
    let w = 256;
    let h = 128;
    let mut lut = Vec::with_capacity(w * h);

    for y in 0..h {
        let phi = PI * y as f32 / (h - 1) as f32; // 0 = zenith, PI = nadir
        for x in 0..w {
            let theta = 2.0 * PI * x as f32 / (w - 1) as f32;
            let dir = Vec3::new(
                phi.sin() * theta.cos(),
                phi.cos(),
                phi.sin() * theta.sin(),
            );
            let color = compute_sky_color(dir, config);
            lut.push([color.x, color.y, color.z]);
        }
    }

    lut
}

/// Time-of-day presets.
pub struct SkyPresets;

impl SkyPresets {
    pub fn noon() -> SkyConfig {
        SkyConfig {
            sun_direction: Vec3::new(0.0, 1.0, 0.0).normalize(),
            sun_intensity: 22.0,
            ..Default::default()
        }
    }

    pub fn sunset() -> SkyConfig {
        SkyConfig {
            sun_direction: Vec3::new(0.5, 0.05, -0.5).normalize(),
            sun_intensity: 20.0,
            sun_color: Vec3::new(1.0, 0.6, 0.3),
            ..Default::default()
        }
    }

    pub fn night() -> SkyConfig {
        SkyConfig {
            sun_direction: Vec3::new(0.0, -0.5, -1.0).normalize(),
            sun_intensity: 0.1,
            sun_color: Vec3::new(0.3, 0.3, 0.5),
            ..Default::default()
        }
    }

    pub fn dawn() -> SkyConfig {
        SkyConfig {
            sun_direction: Vec3::new(-0.8, 0.1, -0.3).normalize(),
            sun_intensity: 15.0,
            sun_color: Vec3::new(1.0, 0.5, 0.3),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sky_color_noon() {
        let config = SkyPresets::noon();
        let zenith = compute_sky_color(Vec3::Y, &config);
        assert!(zenith.x > 0.0 && zenith.y > 0.0 && zenith.z > 0.0);
        // Sky should be bluish at zenith
        assert!(zenith.z > zenith.x, "zenith should be blue-ish");
    }

    #[test]
    fn test_sky_color_horizon() {
        let config = SkyPresets::sunset();
        let horizon = compute_sky_color(Vec3::new(0.0, 0.01, -1.0).normalize(), &config);
        // Sunset horizon should be reddish
        assert!(horizon.x > horizon.z * 0.5, "horizon at sunset should be warm");
    }

    #[test]
    fn test_rayleigh_phase_symmetry() {
        // Rayleigh is symmetric: f(cos) = f(-cos) for squared term
        let a = rayleigh_phase(0.5);
        let b = rayleigh_phase(-0.5);
        assert!((a - b).abs() < 0.001);
    }

    #[test]
    fn test_sky_lut_size() {
        let config = SkyConfig { num_samples: 4, num_light_samples: 2, ..Default::default() };
        let lut = generate_sky_lut(&config);
        assert_eq!(lut.len(), 256 * 128);
    }
}
