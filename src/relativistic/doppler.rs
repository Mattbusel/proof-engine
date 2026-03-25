//! Relativistic Doppler effect.

use glam::{Vec3, Vec4};
use super::lorentz::lorentz_factor;

/// Relativistic longitudinal Doppler effect.
/// Returns observed frequency given source frequency f_source, speed v, and whether approaching.
/// f_obs = f_source * sqrt((1+beta)/(1-beta)) for approaching
/// f_obs = f_source * sqrt((1-beta)/(1+beta)) for receding
pub fn relativistic_doppler(f_source: f64, v: f64, c: f64, approaching: bool) -> f64 {
    let beta = (v / c).abs().min(0.9999999);
    if approaching {
        f_source * ((1.0 + beta) / (1.0 - beta)).sqrt()
    } else {
        f_source * ((1.0 - beta) / (1.0 + beta)).sqrt()
    }
}

/// Transverse Doppler effect (purely relativistic, no classical analog).
/// f_obs = f_source / gamma (always a redshift).
pub fn transverse_doppler(f_source: f64, v: f64, c: f64) -> f64 {
    let gamma = lorentz_factor(v, c);
    f_source / gamma
}

/// Wavelength shift: returns observed wavelength.
/// lambda_obs = lambda_source / doppler_factor
/// Positive v = receding (redshift), negative v = approaching (blueshift).
pub fn wavelength_shift(lambda_source: f64, v: f64, c: f64) -> f64 {
    let beta = v / c;
    let beta_clamped = beta.clamp(-0.9999999, 0.9999999);
    lambda_source * ((1.0 + beta_clamped) / (1.0 - beta_clamped)).sqrt()
}

/// Cosmological redshift parameter z.
/// z = sqrt((1+beta)/(1-beta)) - 1
pub fn redshift_z(v: f64, c: f64) -> f64 {
    let beta = (v / c).abs().min(0.9999999);
    ((1.0 + beta) / (1.0 - beta)).sqrt() - 1.0
}

/// Convert a wavelength in nanometers to an approximate RGBA color.
/// Maps the visible spectrum (380-780nm) to RGB.
/// Outside visible range, returns dim values.
pub fn color_from_wavelength(wavelength_nm: f64) -> Vec4 {
    let w = wavelength_nm;
    let (mut r, mut g, mut b);

    if w < 380.0 {
        // Ultraviolet - faint violet
        r = 0.2;
        g = 0.0;
        b = 0.3;
    } else if w < 440.0 {
        r = -(w - 440.0) / (440.0 - 380.0);
        g = 0.0;
        b = 1.0;
    } else if w < 490.0 {
        r = 0.0;
        g = (w - 440.0) / (490.0 - 440.0);
        b = 1.0;
    } else if w < 510.0 {
        r = 0.0;
        g = 1.0;
        b = -(w - 510.0) / (510.0 - 490.0);
    } else if w < 580.0 {
        r = (w - 510.0) / (580.0 - 510.0);
        g = 1.0;
        b = 0.0;
    } else if w < 645.0 {
        r = 1.0;
        g = -(w - 645.0) / (645.0 - 580.0);
        b = 0.0;
    } else if w < 780.0 {
        r = 1.0;
        g = 0.0;
        b = 0.0;
    } else {
        // Infrared - faint red
        r = 0.3;
        g = 0.0;
        b = 0.0;
    }

    // Intensity fall-off at edges of visible spectrum
    let intensity = if w < 380.0 || w > 780.0 {
        0.3
    } else if w < 420.0 {
        0.3 + 0.7 * (w - 380.0) / (420.0 - 380.0)
    } else if w > 700.0 {
        0.3 + 0.7 * (780.0 - w) / (780.0 - 700.0)
    } else {
        1.0
    };

    r *= intensity;
    g *= intensity;
    b *= intensity;

    Vec4::new(r as f32, g as f32, b as f32, 1.0)
}

/// Shift a base color by the relativistic Doppler effect.
/// Uses the radial velocity component (projection of v onto observer direction).
pub fn doppler_color_shift(
    base_color: Vec4,
    v: f64,
    c: f64,
    direction: Vec3,
    observer_dir: Vec3,
) -> Vec4 {
    let dir_norm = direction.normalize_or_zero();
    let obs_norm = observer_dir.normalize_or_zero();
    // Radial velocity component: positive = receding
    let v_radial = -(v as f32) * dir_norm.dot(obs_norm);
    let v_r = v_radial as f64;

    // Estimate a "dominant wavelength" from the color and shift it
    // Simple approach: shift the color temperature
    let beta = (v_r / c).clamp(-0.9999999, 0.9999999);
    let doppler = ((1.0 + beta) / (1.0 - beta)).sqrt();

    // Shift RGB channels: blueshift moves energy up, redshift moves it down
    let shift = (1.0 / doppler) as f32;

    // Apply shift by interpolating channels
    if shift > 1.0 {
        // Blueshift: move red->green->blue
        let t = (shift - 1.0).min(1.0);
        Vec4::new(
            base_color.x * (1.0 - t) + base_color.y * t,
            base_color.y * (1.0 - t) + base_color.z * t,
            base_color.z + base_color.x * t,
            base_color.w,
        )
    } else {
        // Redshift: move blue->green->red
        let t = (1.0 - shift).min(1.0);
        Vec4::new(
            base_color.x + base_color.z * t,
            base_color.y * (1.0 - t) + base_color.x * t,
            base_color.z * (1.0 - t) + base_color.y * t,
            base_color.w,
        )
    }
}

/// Renderer that shifts entity colors based on radial velocity toward observer.
#[derive(Debug, Clone)]
pub struct DopplerRenderer {
    pub c: f64,
    pub observer_pos: Vec3,
    pub intensity_shift: bool,
}

impl DopplerRenderer {
    pub fn new(c: f64, observer_pos: Vec3) -> Self {
        Self {
            c,
            observer_pos,
            intensity_shift: true,
        }
    }

    /// Compute the Doppler-shifted color for an entity.
    pub fn shifted_color(
        &self,
        base_color: Vec4,
        entity_pos: Vec3,
        entity_velocity: Vec3,
    ) -> Vec4 {
        let to_observer = (self.observer_pos - entity_pos).normalize_or_zero();
        let v = entity_velocity.length() as f64;
        if v < 1e-10 {
            return base_color;
        }
        let direction = entity_velocity.normalize();
        doppler_color_shift(base_color, v, self.c, direction, to_observer)
    }

    /// Compute the Doppler frequency ratio (observed/emitted).
    pub fn frequency_ratio(&self, entity_pos: Vec3, entity_velocity: Vec3) -> f64 {
        let to_observer = (self.observer_pos - entity_pos).normalize_or_zero();
        let v_radial = -entity_velocity.dot(to_observer) as f64;
        let beta = (v_radial / self.c).clamp(-0.9999999, 0.9999999);
        ((1.0 + beta) / (1.0 - beta)).sqrt()
    }

    /// Batch process: shift colors for multiple entities.
    pub fn shift_colors(
        &self,
        entities: &[(Vec3, Vec3, Vec4)], // (position, velocity, base_color)
    ) -> Vec<Vec4> {
        entities.iter().map(|(pos, vel, col)| {
            self.shifted_color(*col, *pos, *vel)
        }).collect()
    }

    /// Compute intensity scaling from Doppler effect.
    /// Intensity scales as (f_obs/f_source)^3 for a moving isotropic source.
    pub fn intensity_factor(&self, entity_pos: Vec3, entity_velocity: Vec3) -> f32 {
        let ratio = self.frequency_ratio(entity_pos, entity_velocity);
        (ratio.powi(3)).min(10.0) as f32
    }
}

/// Cosmological redshift: shifts wavelength by factor (1 + z).
pub fn cosmic_redshift(z: f64, base_wavelength: f64) -> f64 {
    base_wavelength * (1.0 + z)
}

/// Compute the velocity from redshift z (special relativistic formula).
pub fn velocity_from_redshift(z: f64, c: f64) -> f64 {
    let z1 = z + 1.0;
    c * (z1 * z1 - 1.0) / (z1 * z1 + 1.0)
}

/// General Doppler formula for arbitrary angle theta between velocity and line of sight.
/// f_obs = f_source / (gamma * (1 - beta * cos(theta)))
/// theta = 0 means source moving directly toward observer.
pub fn general_doppler(f_source: f64, v: f64, c: f64, theta: f64) -> f64 {
    let beta = v / c;
    let gamma = lorentz_factor(v, c);
    f_source / (gamma * (1.0 - beta * theta.cos()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0;

    #[test]
    fn test_doppler_blueshift() {
        let f_obs = relativistic_doppler(1000.0, 0.5 * C, C, true);
        assert!(f_obs > 1000.0, "Approaching should blueshift: {}", f_obs);
    }

    #[test]
    fn test_doppler_redshift() {
        let f_obs = relativistic_doppler(1000.0, 0.5 * C, C, false);
        assert!(f_obs < 1000.0, "Receding should redshift: {}", f_obs);
    }

    #[test]
    fn test_doppler_symmetry() {
        let f_blue = relativistic_doppler(1000.0, 0.5 * C, C, true);
        let f_red = relativistic_doppler(1000.0, 0.5 * C, C, false);
        // f_blue * f_red = f_source^2
        assert!(
            (f_blue * f_red - 1000.0 * 1000.0).abs() < 1.0,
            "Product should equal f^2: {} * {} = {}",
            f_blue, f_red, f_blue * f_red
        );
    }

    #[test]
    fn test_transverse_doppler_redshift() {
        let f_obs = transverse_doppler(1000.0, 0.5 * C, C);
        assert!(f_obs < 1000.0, "Transverse Doppler should always redshift: {}", f_obs);
    }

    #[test]
    fn test_transverse_less_than_longitudinal() {
        let f_trans = transverse_doppler(1000.0, 0.5 * C, C);
        let f_long = relativistic_doppler(1000.0, 0.5 * C, C, false);
        // Transverse redshift is less extreme than longitudinal receding
        assert!(
            f_trans > f_long,
            "Transverse {} should be less shifted than longitudinal receding {}",
            f_trans, f_long
        );
    }

    #[test]
    fn test_wavelength_shift_receding() {
        let lambda_obs = wavelength_shift(500.0, 0.5 * C, C);
        assert!(lambda_obs > 500.0, "Receding should increase wavelength: {}", lambda_obs);
    }

    #[test]
    fn test_wavelength_shift_approaching() {
        let lambda_obs = wavelength_shift(500.0, -0.5 * C, C);
        assert!(lambda_obs < 500.0, "Approaching should decrease wavelength: {}", lambda_obs);
    }

    #[test]
    fn test_redshift_z_zero_at_rest() {
        let z = redshift_z(0.0, C);
        assert!(z.abs() < 1e-10);
    }

    #[test]
    fn test_redshift_z_approaches_infinity() {
        let z = redshift_z(0.9999 * C, C);
        assert!(z > 100.0, "z should be very large near c: {}", z);
    }

    #[test]
    fn test_color_from_wavelength_visible() {
        // Red
        let red = color_from_wavelength(650.0);
        assert!(red.x > red.y && red.x > red.z);
        // Green
        let green = color_from_wavelength(520.0);
        assert!(green.y > green.x && green.y > green.z);
        // Blue
        let blue = color_from_wavelength(460.0);
        assert!(blue.z > blue.y);
    }

    #[test]
    fn test_cosmic_redshift() {
        let shifted = cosmic_redshift(1.0, 500.0);
        assert!((shifted - 1000.0).abs() < 1e-10, "z=1 should double wavelength");
    }

    #[test]
    fn test_velocity_from_redshift_roundtrip() {
        let v = 0.6 * C;
        let z = redshift_z(v, C);
        let v_back = velocity_from_redshift(z, C);
        assert!((v - v_back).abs() / v < 1e-6);
    }

    #[test]
    fn test_general_doppler_forward() {
        // theta=0 (approaching) should match relativistic_doppler approaching
        let f1 = general_doppler(1000.0, 0.5 * C, C, 0.0);
        let f2 = relativistic_doppler(1000.0, 0.5 * C, C, true);
        assert!((f1 - f2).abs() < 1e-6);
    }

    #[test]
    fn test_general_doppler_transverse() {
        // theta = pi/2 should match transverse doppler
        let f1 = general_doppler(1000.0, 0.5 * C, C, std::f64::consts::FRAC_PI_2);
        let f2 = transverse_doppler(1000.0, 0.5 * C, C);
        assert!((f1 - f2).abs() < 1e-6);
    }

    #[test]
    fn test_doppler_renderer() {
        let renderer = DopplerRenderer::new(C, Vec3::new(0.0, 0.0, 0.0));
        let ratio = renderer.frequency_ratio(
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(-0.5 * C as f32, 0.0, 0.0), // moving toward observer
        );
        assert!(ratio > 1.0, "Should be blueshifted: {}", ratio);
    }
}
