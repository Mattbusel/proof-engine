//! Antenna radiation patterns — Hertzian dipole, half-wave dipole,
//! antenna arrays with beam steering, directivity, and gain.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

/// Speed of light in normalized units.
const C: f32 = 1.0;
/// Impedance of free space (normalized).
const ETA0: f32 = 1.0;

// ── Hertzian Dipole ───────────────────────────────────────────────────────

/// An infinitesimal (Hertzian) dipole antenna.
#[derive(Clone, Debug)]
pub struct HertzianDipole {
    pub position: Vec3,
    pub orientation: Vec3, // direction of the dipole moment (unit)
    pub current_moment: f32, // I * dl (current times length)
    pub frequency: f32,
}

impl HertzianDipole {
    pub fn new(position: Vec3, orientation: Vec3, current_moment: f32, frequency: f32) -> Self {
        Self {
            position,
            orientation: orientation.normalize(),
            frequency,
            current_moment,
        }
    }

    /// Wavenumber k = 2*pi*f/c.
    pub fn wavenumber(&self) -> f32 {
        2.0 * PI * self.frequency / C
    }

    /// Wavelength.
    pub fn wavelength(&self) -> f32 {
        C / self.frequency
    }

    /// Far-field E and H at a given direction and distance.
    /// E_theta = j * k * eta * I*dl * sin(theta) * exp(-jkr) / (4*pi*r)
    /// We return the magnitude (real envelope).
    pub fn far_field(&self, direction: Vec3, distance: f32) -> (Vec3, Vec3) {
        if distance < 1e-10 {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let dir = direction.normalize();
        let k = self.wavenumber();

        // sin(theta) where theta is angle between direction and dipole orientation
        let cos_theta = dir.dot(self.orientation);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

        // E-field magnitude in far field
        let e_mag = k * ETA0 * self.current_moment * sin_theta / (4.0 * PI * distance);

        // E-field direction: theta-hat (perpendicular to r in the r-orientation plane)
        let e_dir = if sin_theta > 1e-10 {
            // theta_hat = cos(theta)*r_hat - orientation (projected perpendicular to r)
            let theta_hat = (cos_theta * dir - self.orientation).normalize();
            theta_hat
        } else {
            Vec3::ZERO
        };

        let e = e_dir * e_mag;
        // H = (1/eta) * r_hat × E
        let h = dir.cross(e) / ETA0;

        (e, h)
    }

    /// Radiation power pattern: P(theta) proportional to sin^2(theta).
    pub fn power_pattern(&self, theta: f32) -> f32 {
        let sin_theta = theta.sin();
        sin_theta * sin_theta
    }

    /// Compute the full radiation pattern.
    /// Returns (theta, phi, power) for each angular sample.
    pub fn radiation_pattern(&self, theta_steps: usize, phi_steps: usize) -> Vec<(f32, f32, f32)> {
        let mut pattern = Vec::with_capacity(theta_steps * phi_steps);
        for i in 0..theta_steps {
            let theta = PI * (i as f32 + 0.5) / theta_steps as f32;
            for j in 0..phi_steps {
                let phi = 2.0 * PI * j as f32 / phi_steps as f32;
                let power = self.power_pattern(theta);
                pattern.push((theta, phi, power));
            }
        }
        pattern
    }

    /// Radiation resistance: R_rad = (2*pi/3) * eta * (k*dl)^2 / (4*pi) (normalized).
    pub fn radiation_resistance(&self) -> f32 {
        let k = self.wavenumber();
        let kdl = k * self.current_moment; // k * I*dl, but dl is embedded in current_moment
        // For a Hertzian dipole: R_rad = (2*pi/3) * eta * (dl/lambda)^2
        // Simplified: proportional to (k*dl)^2
        ETA0 * 2.0 * PI / 3.0 * kdl * kdl / (4.0 * PI)
    }
}

// ── Radiation Pattern Analysis ────────────────────────────────────────────

/// Compute directivity from a radiation pattern.
/// D = 4*pi * max(P) / integral(P * sin(theta) dtheta dphi)
pub fn directivity(pattern: &[(f32, f32, f32)]) -> f32 {
    if pattern.is_empty() {
        return 0.0;
    }

    let max_power = pattern.iter().map(|p| p.2).fold(0.0f32, f32::max);
    if max_power < 1e-10 {
        return 0.0;
    }

    // Approximate the integral using the pattern data
    // Determine step sizes from the pattern
    let theta_values: Vec<f32> = pattern.iter().map(|p| p.0).collect();
    let mut unique_thetas: Vec<f32> = Vec::new();
    for &t in &theta_values {
        if unique_thetas.last().map_or(true, |&last| (last - t).abs() > 1e-6) {
            unique_thetas.push(t);
        }
    }
    let n_theta = unique_thetas.len().max(1);
    let n_phi = pattern.len() / n_theta;
    let d_theta = PI / n_theta as f32;
    let d_phi = 2.0 * PI / n_phi.max(1) as f32;

    let mut total = 0.0f32;
    for &(theta, _phi, power) in pattern {
        total += power * theta.sin() * d_theta * d_phi;
    }

    if total < 1e-10 {
        return 0.0;
    }

    4.0 * PI * max_power / total
}

/// Gain = directivity * efficiency.
pub fn gain(pattern: &[(f32, f32, f32)], efficiency: f32) -> f32 {
    directivity(pattern) * efficiency
}

// ── Half-Wave Dipole ──────────────────────────────────────────────────────

/// A half-wave dipole antenna.
#[derive(Clone, Debug)]
pub struct HalfWaveDipole {
    pub position: Vec3,
    pub orientation: Vec3,
    pub frequency: f32,
}

impl HalfWaveDipole {
    pub fn new(position: Vec3, orientation: Vec3, frequency: f32) -> Self {
        Self {
            position,
            orientation: orientation.normalize(),
            frequency,
        }
    }

    pub fn wavelength(&self) -> f32 {
        C / self.frequency
    }

    pub fn wavenumber(&self) -> f32 {
        2.0 * PI * self.frequency / C
    }

    /// Half-wave dipole pattern: P(theta) = [cos(pi/2 * cos(theta)) / sin(theta)]^2
    pub fn power_pattern(&self, theta: f32) -> f32 {
        let sin_theta = theta.sin();
        if sin_theta.abs() < 1e-10 {
            return 0.0;
        }
        let numerator = ((PI / 2.0) * theta.cos()).cos();
        let val = numerator / sin_theta;
        val * val
    }

    /// Far field of the half-wave dipole.
    pub fn far_field(&self, direction: Vec3, distance: f32) -> (Vec3, Vec3) {
        if distance < 1e-10 {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let dir = direction.normalize();
        let cos_theta = dir.dot(self.orientation);
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

        let pattern_val = if sin_theta > 1e-10 {
            ((PI / 2.0) * cos_theta).cos() / sin_theta
        } else {
            0.0
        };

        let k = self.wavenumber();
        let e_mag = ETA0 * pattern_val / (2.0 * PI * distance);

        let e_dir = if sin_theta > 1e-10 {
            (cos_theta * dir - self.orientation).normalize()
        } else {
            Vec3::ZERO
        };

        let e = e_dir * e_mag;
        let h = dir.cross(e) / ETA0;
        (e, h)
    }

    /// Radiation pattern.
    pub fn radiation_pattern(&self, theta_steps: usize, phi_steps: usize) -> Vec<(f32, f32, f32)> {
        let mut pattern = Vec::with_capacity(theta_steps * phi_steps);
        for i in 0..theta_steps {
            let theta = PI * (i as f32 + 0.5) / theta_steps as f32;
            for j in 0..phi_steps {
                let phi = 2.0 * PI * j as f32 / phi_steps as f32;
                let power = self.power_pattern(theta);
                pattern.push((theta, phi, power));
            }
        }
        pattern
    }

    /// Directivity of a half-wave dipole ≈ 1.64 (2.15 dBi).
    pub fn theoretical_directivity(&self) -> f32 {
        1.64
    }
}

// ── Antenna Array ─────────────────────────────────────────────────────────

/// An array of Hertzian dipole antennas with programmable phase shifts for beam forming.
#[derive(Clone, Debug)]
pub struct AntennaArray {
    pub elements: Vec<HertzianDipole>,
    pub phase_shifts: Vec<f32>,
}

impl AntennaArray {
    pub fn new(elements: Vec<HertzianDipole>, phase_shifts: Vec<f32>) -> Self {
        assert_eq!(elements.len(), phase_shifts.len());
        Self { elements, phase_shifts }
    }

    /// Create a uniform linear array along a given axis.
    pub fn uniform_linear(
        n: usize,
        spacing: f32,
        axis: Vec3,
        orientation: Vec3,
        frequency: f32,
        current_moment: f32,
    ) -> Self {
        let axis_norm = axis.normalize();
        let start = -axis_norm * spacing * (n - 1) as f32 * 0.5;
        let elements: Vec<HertzianDipole> = (0..n)
            .map(|i| {
                let pos = start + axis_norm * spacing * i as f32;
                HertzianDipole::new(pos, orientation, current_moment, frequency)
            })
            .collect();
        let phase_shifts = vec![0.0; n];
        Self { elements, phase_shifts }
    }

    /// Array factor: AF(direction) = sum_i exp(j*(k*d_i·direction + phase_i))
    /// Returns the magnitude |AF|^2.
    pub fn array_factor(&self, direction: Vec3) -> f32 {
        let dir = direction.normalize();
        let mut real_sum = 0.0f32;
        let mut imag_sum = 0.0f32;

        for (i, elem) in self.elements.iter().enumerate() {
            let k = elem.wavenumber();
            let phase = k * elem.position.dot(dir) + self.phase_shifts[i];
            real_sum += phase.cos();
            imag_sum += phase.sin();
        }

        real_sum * real_sum + imag_sum * imag_sum
    }

    /// Total radiation pattern: element pattern × array factor.
    pub fn total_pattern(&self, theta: f32, phi: f32) -> f32 {
        let direction = Vec3::new(
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        );

        // Element pattern (assuming all elements are identical)
        let element_power = if !self.elements.is_empty() {
            self.elements[0].power_pattern(theta)
        } else {
            0.0
        };

        element_power * self.array_factor(direction)
    }

    /// Compute phase shifts to steer the beam toward a target direction.
    pub fn beam_steering(&mut self, target_direction: Vec3) {
        let dir = target_direction.normalize();
        for (i, elem) in self.elements.iter().enumerate() {
            let k = elem.wavenumber();
            // Phase shift = -k * d_i · target_direction
            self.phase_shifts[i] = -k * elem.position.dot(dir);
        }
    }

    /// Full radiation pattern of the array.
    pub fn radiation_pattern(&self, theta_steps: usize, phi_steps: usize) -> Vec<(f32, f32, f32)> {
        let mut pattern = Vec::with_capacity(theta_steps * phi_steps);
        for i in 0..theta_steps {
            let theta = PI * (i as f32 + 0.5) / theta_steps as f32;
            for j in 0..phi_steps {
                let phi = 2.0 * PI * j as f32 / phi_steps as f32;
                let power = self.total_pattern(theta, phi);
                pattern.push((theta, phi, power));
            }
        }
        pattern
    }
}

// ── Antenna Renderer ──────────────────────────────────────────────────────

/// Renderer for antenna radiation patterns.
pub struct AntennaRenderer {
    pub pattern_color: Vec4,
    pub element_color: Vec4,
    pub scale: f32,
}

impl AntennaRenderer {
    pub fn new() -> Self {
        Self {
            pattern_color: Vec4::new(0.2, 0.8, 0.4, 0.7),
            element_color: Vec4::new(1.0, 0.5, 0.1, 1.0),
            scale: 1.0,
        }
    }

    /// Render a 2D polar plot of the radiation pattern (theta slice at phi=0).
    pub fn render_polar_plot(
        &self,
        pattern: &[(f32, f32, f32)],
        phi_slice: f32,
        num_points: usize,
    ) -> Vec<(Vec2, Vec4)> {
        let mut result = Vec::new();

        // Find max power for normalization
        let max_power = pattern.iter().map(|p| p.2).fold(0.0f32, f32::max).max(1e-10);

        // Extract the phi slice
        let tolerance = PI / num_points as f32;
        for &(theta, phi, power) in pattern {
            if (phi - phi_slice).abs() < tolerance {
                let r = (power / max_power).sqrt() * self.scale;
                let x = r * theta.sin();
                let y = r * theta.cos();
                let brightness = (power / max_power).sqrt();
                let color = self.pattern_color * brightness;
                result.push((Vec2::new(x, y), color));
            }
        }

        result
    }

    /// Render a 3D pattern as a set of (position, brightness) for glyph rendering.
    pub fn render_3d_pattern(
        &self,
        pattern: &[(f32, f32, f32)],
    ) -> Vec<(Vec3, f32)> {
        let max_power = pattern.iter().map(|p| p.2).fold(0.0f32, f32::max).max(1e-10);

        pattern.iter().map(|&(theta, phi, power)| {
            let r = (power / max_power).sqrt() * self.scale;
            let pos = Vec3::new(
                r * theta.sin() * phi.cos(),
                r * theta.sin() * phi.sin(),
                r * theta.cos(),
            );
            let brightness = (power / max_power).sqrt();
            (pos, brightness)
        }).collect()
    }

    /// Glyph based on pattern intensity.
    pub fn intensity_glyph(intensity: f32) -> char {
        if intensity > 0.8 { '█' }
        else if intensity > 0.6 { '▓' }
        else if intensity > 0.4 { '▒' }
        else if intensity > 0.2 { '░' }
        else if intensity > 0.05 { '·' }
        else { ' ' }
    }
}

impl Default for AntennaRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hertzian_dipole_sin2_pattern() {
        let dipole = HertzianDipole::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        // At theta=pi/2 (equator), pattern should be maximum (sin^2(90°) = 1)
        let p_equator = dipole.power_pattern(PI / 2.0);
        assert!((p_equator - 1.0).abs() < 1e-6);
        // At theta=0 (along axis), pattern should be zero (sin^2(0) = 0)
        let p_pole = dipole.power_pattern(0.0);
        assert!(p_pole.abs() < 1e-6);
        // At theta=pi/4, should be sin^2(45°) = 0.5
        let p_45 = dipole.power_pattern(PI / 4.0);
        assert!((p_45 - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_hertzian_dipole_far_field() {
        let dipole = HertzianDipole::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        // Far field at equator
        let (e, h) = dipole.far_field(Vec3::X, 10.0);
        assert!(e.length() > 0.0, "E should be nonzero at equator");
        assert!(h.length() > 0.0, "H should be nonzero at equator");
        // E and H should be perpendicular
        let dot = e.dot(h);
        assert!(dot.abs() < 1e-6, "E·H should be 0");
    }

    #[test]
    fn test_hertzian_dipole_1_over_r() {
        let dipole = HertzianDipole::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        let (e1, _) = dipole.far_field(Vec3::X, 10.0);
        let (e2, _) = dipole.far_field(Vec3::X, 20.0);
        let ratio = e1.length() / e2.length();
        assert!((ratio - 2.0).abs() < 0.01, "Far field should decay as 1/r: ratio={}", ratio);
    }

    #[test]
    fn test_directivity_hertzian() {
        let dipole = HertzianDipole::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        let pattern = dipole.radiation_pattern(90, 36);
        let d = directivity(&pattern);
        // Hertzian dipole directivity = 1.5
        assert!((d - 1.5).abs() < 0.2, "Directivity should be ~1.5: {}", d);
    }

    #[test]
    fn test_half_wave_dipole_pattern() {
        let hw = HalfWaveDipole::new(Vec3::ZERO, Vec3::Z, 1.0);
        // Maximum at equator
        let p_eq = hw.power_pattern(PI / 2.0);
        assert!(p_eq > 0.9, "Max at equator: {}", p_eq);
        // Zero at poles
        let p_pole = hw.power_pattern(0.01);
        assert!(p_pole < 0.1, "Should be near zero at pole: {}", p_pole);
    }

    #[test]
    fn test_array_factor_broadside() {
        // 4-element array, half-wavelength spacing, no phase shift → broadside radiation
        let freq = 1.0;
        let lambda = C / freq;
        let spacing = lambda / 2.0;
        let array = AntennaArray::uniform_linear(4, spacing, Vec3::X, Vec3::Z, freq, 1.0);

        // Broadside (perpendicular to array axis) should have max AF
        let af_broadside = array.array_factor(Vec3::Z);
        // End-fire (along array axis)
        let af_endfire = array.array_factor(Vec3::X);

        // For uniform array with zero phase shift, broadside should be N^2 = 16
        assert!((af_broadside - 16.0).abs() < 1.0, "Broadside AF should be ~16: {}", af_broadside);
    }

    #[test]
    fn test_array_factor_periodicity() {
        let freq = 1.0;
        let lambda = C / freq;
        let spacing = lambda / 2.0;
        let array = AntennaArray::uniform_linear(4, spacing, Vec3::X, Vec3::Z, freq, 1.0);

        // AF should have same value for opposite directions at broadside
        let af1 = array.array_factor(Vec3::Z);
        let af2 = array.array_factor(-Vec3::Z);
        assert!((af1 - af2).abs() < 0.1, "Pattern should be symmetric: {} vs {}", af1, af2);
    }

    #[test]
    fn test_beam_steering() {
        let freq = 1.0;
        let lambda = C / freq;
        let spacing = lambda / 2.0;
        let mut array = AntennaArray::uniform_linear(8, spacing, Vec3::X, Vec3::Z, freq, 1.0);

        // Steer toward +X direction
        array.beam_steering(Vec3::X);

        let af_target = array.array_factor(Vec3::X);
        let n = array.elements.len() as f32;
        // Steered array should have AF ≈ N^2 in the target direction
        assert!((af_target - n * n).abs() < 1.0, "Steered AF should be ~N^2: {}", af_target);
    }

    #[test]
    fn test_gain() {
        let dipole = HertzianDipole::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        let pattern = dipole.radiation_pattern(60, 24);
        let g = gain(&pattern, 0.9);
        let d = directivity(&pattern);
        assert!((g - d * 0.9).abs() < 0.01);
    }

    #[test]
    fn test_renderer() {
        let renderer = AntennaRenderer::new();
        assert_eq!(AntennaRenderer::intensity_glyph(0.9), '█');
        assert_eq!(AntennaRenderer::intensity_glyph(0.01), ' ');
    }

    #[test]
    fn test_radiation_pattern_size() {
        let dipole = HertzianDipole::new(Vec3::ZERO, Vec3::Z, 1.0, 1.0);
        let pattern = dipole.radiation_pattern(10, 20);
        assert_eq!(pattern.len(), 200);
    }
}
