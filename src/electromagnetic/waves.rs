//! Electromagnetic wave propagation — plane waves, spherical waves,
//! Gaussian beams, wave packets, interference, diffraction, and Snell's law.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

/// Speed of light in normalized units.
const C: f32 = 1.0;

// ── Plane Wave ────────────────────────────────────────────────────────────

/// Plane electromagnetic wave: E and B perpendicular to propagation direction.
#[derive(Clone, Debug)]
pub struct PlaneWave {
    pub direction: Vec3,     // propagation direction (unit)
    pub polarization: Vec3,  // E-field polarization direction (unit, ⊥ direction)
    pub frequency: f32,
    pub amplitude: f32,
    pub phase: f32,
}

impl PlaneWave {
    pub fn new(direction: Vec3, polarization: Vec3, frequency: f32, amplitude: f32) -> Self {
        Self {
            direction: direction.normalize(),
            polarization: polarization.normalize(),
            frequency,
            amplitude,
            phase: 0.0,
        }
    }

    /// Evaluate E and B fields at a position and time.
    /// E = E0 * sin(k·r - ωt + φ) * polarization
    /// B = (1/c) * direction × E
    pub fn evaluate(&self, pos: Vec3, time: f32) -> (Vec3, Vec3) {
        let omega = 2.0 * PI * self.frequency;
        let k = omega / C;
        let k_vec = self.direction * k;
        let phase = k_vec.dot(pos) - omega * time + self.phase;
        let e_mag = self.amplitude * phase.sin();
        let e = self.polarization * e_mag;
        let b = self.direction.cross(e) / C;
        (e, b)
    }

    /// Wavelength = c / frequency.
    pub fn wavelength(&self) -> f32 {
        C / self.frequency
    }

    /// Wave vector magnitude.
    pub fn wavenumber(&self) -> f32 {
        2.0 * PI * self.frequency / C
    }

    /// Angular frequency.
    pub fn angular_frequency(&self) -> f32 {
        2.0 * PI * self.frequency
    }

    /// Intensity (time-averaged): I = 0.5 * E0^2 / (mu0*c), normalized to 0.5*E0^2.
    pub fn intensity(&self) -> f32 {
        0.5 * self.amplitude * self.amplitude
    }
}

// ── Spherical Wave ────────────────────────────────────────────────────────

/// Spherical electromagnetic wave emanating from a point source with 1/r decay.
#[derive(Clone, Debug)]
pub struct SphericalWave {
    pub origin: Vec3,
    pub frequency: f32,
    pub amplitude: f32,
}

impl SphericalWave {
    pub fn new(origin: Vec3, frequency: f32, amplitude: f32) -> Self {
        Self { origin, frequency, amplitude }
    }

    /// Evaluate the scalar field amplitude at a point and time.
    /// A(r,t) = (A0/r) * sin(kr - ωt)
    pub fn evaluate_scalar(&self, pos: Vec3, time: f32) -> f32 {
        let r_vec = pos - self.origin;
        let r = r_vec.length();
        if r < 1e-10 {
            return 0.0;
        }
        let omega = 2.0 * PI * self.frequency;
        let k = omega / C;
        (self.amplitude / r) * (k * r - omega * time).sin()
    }

    /// Evaluate E and B vectors (polarized radially outward for simplicity).
    pub fn evaluate(&self, pos: Vec3, time: f32) -> (Vec3, Vec3) {
        let r_vec = pos - self.origin;
        let r = r_vec.length();
        if r < 1e-10 {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let r_hat = r_vec / r;
        let omega = 2.0 * PI * self.frequency;
        let k = omega / C;
        let scalar = (self.amplitude / r) * (k * r - omega * time).sin();

        // E perpendicular to r_hat; pick an arbitrary perpendicular direction
        let e_dir = if r_hat.x.abs() < 0.9 {
            Vec3::X.cross(r_hat).normalize()
        } else {
            Vec3::Y.cross(r_hat).normalize()
        };
        let e = e_dir * scalar;
        let b = r_hat.cross(e) / C;
        (e, b)
    }
}

// ── Gaussian Beam ─────────────────────────────────────────────────────────

/// Gaussian beam with a finite waist (focused beam).
#[derive(Clone, Debug)]
pub struct GaussianBeam {
    pub origin: Vec3,
    pub direction: Vec3,
    pub waist: f32,      // beam waist (minimum radius) w0
    pub wavelength: f32,
}

impl GaussianBeam {
    pub fn new(origin: Vec3, direction: Vec3, waist: f32, wavelength: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
            waist,
            wavelength,
        }
    }

    /// Rayleigh range: z_R = pi * w0^2 / lambda
    pub fn rayleigh_range(&self) -> f32 {
        PI * self.waist * self.waist / self.wavelength
    }

    /// Beam radius at distance z from waist: w(z) = w0 * sqrt(1 + (z/z_R)^2)
    pub fn beam_radius(&self, z: f32) -> f32 {
        let zr = self.rayleigh_range();
        self.waist * (1.0 + (z / zr).powi(2)).sqrt()
    }

    /// Evaluate the beam intensity profile at a point.
    pub fn intensity_at(&self, pos: Vec3) -> f32 {
        let to_point = pos - self.origin;
        let z = to_point.dot(self.direction);
        let perp = to_point - z * self.direction;
        let rho = perp.length();

        let w = self.beam_radius(z);
        let w0 = self.waist;
        let intensity = (w0 / w) * (w0 / w) * (-2.0 * rho * rho / (w * w)).exp();
        intensity
    }

    /// Evaluate the complex amplitude (scalar, real part) at a point and time.
    pub fn evaluate(&self, pos: Vec3, time: f32) -> f32 {
        let to_point = pos - self.origin;
        let z = to_point.dot(self.direction);
        let perp = to_point - z * self.direction;
        let rho = perp.length();

        let zr = self.rayleigh_range();
        let w = self.beam_radius(z);
        let k = 2.0 * PI / self.wavelength;
        let omega = 2.0 * PI * C / self.wavelength;

        let amplitude = (self.waist / w) * (-rho * rho / (w * w)).exp();
        let gouy_phase = (z / zr).atan();
        let phase = k * z - omega * time + k * rho * rho * z / (2.0 * (z * z + zr * zr)) - gouy_phase;

        amplitude * phase.cos()
    }
}

// ── Wave Packet ───────────────────────────────────────────────────────────

/// Wave packet with a finite bandwidth (group velocity envelope).
#[derive(Clone, Debug)]
pub struct WavePacket {
    pub center_freq: f32,
    pub bandwidth: f32,
    pub group_velocity: f32,
}

impl WavePacket {
    pub fn new(center_freq: f32, bandwidth: f32, group_velocity: f32) -> Self {
        Self { center_freq, bandwidth, group_velocity }
    }

    /// Evaluate the wave packet amplitude at position x and time t.
    /// Gaussian envelope modulating a carrier wave.
    pub fn evaluate(&self, x: f32, t: f32) -> f32 {
        let omega0 = 2.0 * PI * self.center_freq;
        let k0 = omega0 / C;
        let sigma = 1.0 / (2.0 * PI * self.bandwidth);

        // Envelope moves at group velocity
        let xi = x - self.group_velocity * t;
        let envelope = (-xi * xi / (2.0 * sigma * sigma)).exp();

        // Carrier wave
        let carrier = (k0 * x - omega0 * t).cos();

        envelope * carrier
    }

    /// Phase velocity.
    pub fn phase_velocity(&self) -> f32 {
        C // In vacuum, phase velocity = c
    }

    /// Spatial width (1/e amplitude) of the packet.
    pub fn spatial_width(&self) -> f32 {
        1.0 / (2.0 * PI * self.bandwidth)
    }
}

// ── Standing Wave ─────────────────────────────────────────────────────────

/// Standing wave: superposition of two counter-propagating waves.
pub fn standing_wave(pos: f32, time: f32, wavelength: f32, amplitude: f32) -> f32 {
    let k = 2.0 * PI / wavelength;
    let omega = 2.0 * PI * C / wavelength;
    // 2*A*cos(kx)*cos(wt) — the product of spatial and temporal oscillations
    2.0 * amplitude * (k * pos).cos() * (omega * time).cos()
}

// ── Interference ──────────────────────────────────────────────────────────

/// Compute the interference pattern of two plane waves at a set of points.
pub fn interference_pattern(wave1: &PlaneWave, wave2: &PlaneWave, points: &[Vec3], time: f32) -> Vec<f32> {
    points.iter().map(|&pos| {
        let (e1, _) = wave1.evaluate(pos, time);
        let (e2, _) = wave2.evaluate(pos, time);
        let total = e1 + e2;
        total.length_squared() // intensity ∝ |E|^2
    }).collect()
}

// ── Diffraction ───────────────────────────────────────────────────────────

/// Single-slit diffraction intensity pattern (Fraunhofer).
/// Returns normalized intensity at angle `angle` for given slit width and wavelength.
pub fn diffraction_single_slit(slit_width: f32, wavelength: f32, angle: f32) -> f32 {
    let beta = PI * slit_width * angle.sin() / wavelength;
    if beta.abs() < 1e-8 {
        return 1.0; // Central maximum
    }
    let sinc = beta.sin() / beta;
    sinc * sinc
}

// ── Snell's Law ───────────────────────────────────────────────────────────

/// Compute the refracted angle using Snell's law: n1*sin(θ_i) = n2*sin(θ_t).
/// Returns None for total internal reflection.
pub fn snells_law(n1: f32, n2: f32, theta_i: f32) -> Option<f32> {
    let sin_t = n1 * theta_i.sin() / n2;
    if sin_t.abs() > 1.0 {
        None // Total internal reflection
    } else {
        Some(sin_t.asin())
    }
}

/// Fresnel coefficients for reflection and transmission (s-polarization, intensity).
/// Returns (reflectance, transmittance).
pub fn fresnel_coefficients(n1: f32, n2: f32, theta_i: f32) -> (f32, f32) {
    let cos_i = theta_i.cos();
    let sin_t = n1 * theta_i.sin() / n2;
    if sin_t.abs() > 1.0 {
        return (1.0, 0.0); // Total internal reflection
    }
    let cos_t = (1.0 - sin_t * sin_t).sqrt();

    // s-polarization (TE)
    let rs = (n1 * cos_i - n2 * cos_t) / (n1 * cos_i + n2 * cos_t);
    // p-polarization (TM)
    let rp = (n2 * cos_i - n1 * cos_t) / (n2 * cos_i + n1 * cos_t);

    // Average reflectance for unpolarized light
    let reflectance = 0.5 * (rs * rs + rp * rp);
    let transmittance = 1.0 - reflectance;
    (reflectance, transmittance)
}

// ── Wave Renderer ─────────────────────────────────────────────────────────

/// Renderer for EM waves as animated color/brightness patterns.
pub struct WaveRenderer {
    pub color_positive: Vec4,
    pub color_negative: Vec4,
    pub brightness_scale: f32,
}

impl WaveRenderer {
    pub fn new() -> Self {
        Self {
            color_positive: Vec4::new(1.0, 0.8, 0.2, 1.0),
            color_negative: Vec4::new(0.2, 0.4, 1.0, 1.0),
            brightness_scale: 1.0,
        }
    }

    /// Color for a wave amplitude value.
    pub fn color_for_amplitude(&self, amplitude: f32) -> Vec4 {
        let normalized = (amplitude * self.brightness_scale).clamp(-1.0, 1.0);
        let t = (normalized + 1.0) * 0.5; // map [-1,1] to [0,1]
        let color = self.color_negative * (1.0 - t) + self.color_positive * t;
        let alpha = normalized.abs().max(0.05);
        Vec4::new(color.x, color.y, color.z, alpha)
    }

    /// Render a 1D wave along an axis.
    pub fn render_1d_wave(
        &self,
        wave: &PlaneWave,
        x_range: (f32, f32),
        num_points: usize,
        time: f32,
    ) -> Vec<(f32, Vec4)> {
        let mut result = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let t = i as f32 / (num_points - 1).max(1) as f32;
            let x = x_range.0 + t * (x_range.1 - x_range.0);
            let pos = wave.direction * x;
            let (e, _) = wave.evaluate(pos, time);
            let amp = e.dot(wave.polarization);
            result.push((x, self.color_for_amplitude(amp)));
        }
        result
    }

    /// Glyph for wave amplitude.
    pub fn glyph_for_amplitude(amplitude: f32) -> char {
        let a = amplitude.abs();
        if a > 0.8 {
            '█'
        } else if a > 0.6 {
            '▓'
        } else if a > 0.4 {
            '▒'
        } else if a > 0.2 {
            '░'
        } else if a > 0.05 {
            '·'
        } else {
            ' '
        }
    }
}

impl Default for WaveRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispersion_relation() {
        // omega = c * k
        let wave = PlaneWave::new(Vec3::X, Vec3::Y, 2.0, 1.0);
        let omega = wave.angular_frequency();
        let k = wave.wavenumber();
        assert!((omega - C * k).abs() < 1e-6, "Dispersion relation: omega={}, ck={}", omega, C * k);
    }

    #[test]
    fn test_plane_wave_orthogonality() {
        let wave = PlaneWave::new(Vec3::X, Vec3::Y, 1.0, 1.0);
        let (e, b) = wave.evaluate(Vec3::new(0.25, 0.0, 0.0), 0.0);
        // E ⊥ B
        let dot = e.dot(b);
        assert!(dot.abs() < 1e-6, "E·B should be 0, got {}", dot);
        // E ⊥ k
        let dot_ek = e.dot(wave.direction);
        assert!(dot_ek.abs() < 1e-6, "E·k should be 0, got {}", dot_ek);
        // B ⊥ k
        let dot_bk = b.dot(wave.direction);
        assert!(dot_bk.abs() < 1e-6, "B·k should be 0, got {}", dot_bk);
    }

    #[test]
    fn test_plane_wave_ratio() {
        // |E| / |B| = c
        let wave = PlaneWave::new(Vec3::X, Vec3::Y, 1.0, 3.0);
        let (e, b) = wave.evaluate(Vec3::new(0.3, 0.0, 0.0), 0.1);
        if b.length() > 1e-10 {
            let ratio = e.length() / b.length();
            assert!((ratio - C).abs() < 0.01, "|E|/|B| should be c, got {}", ratio);
        }
    }

    #[test]
    fn test_standing_wave_nodes() {
        let wavelength = 2.0;
        // Nodes of cos(kx) at x = lambda/4, 3*lambda/4, etc.
        let node_x = wavelength / 4.0;
        let val = standing_wave(node_x, 0.0, wavelength, 1.0);
        assert!(val.abs() < 1e-5, "Standing wave should have node at lambda/4: {}", val);
    }

    #[test]
    fn test_standing_wave_antinode() {
        let wavelength = 2.0;
        // Antinode at x=0 (cos(0)=1), t=0 (cos(0)=1)
        let val = standing_wave(0.0, 0.0, wavelength, 1.0);
        assert!((val - 2.0).abs() < 1e-5, "Antinode amplitude should be 2A: {}", val);
    }

    #[test]
    fn test_snells_law_normal_incidence() {
        let theta_t = snells_law(1.0, 1.5, 0.0).unwrap();
        assert!(theta_t.abs() < 1e-6, "Normal incidence should give 0 refraction angle");
    }

    #[test]
    fn test_snells_law_tir() {
        // Going from glass (n=1.5) to air (n=1.0) at steep angle
        let critical = (1.0 / 1.5_f32).asin();
        // Angle greater than critical should give TIR
        let result = snells_law(1.5, 1.0, critical + 0.1);
        assert!(result.is_none(), "Should have total internal reflection");
    }

    #[test]
    fn test_snells_law_symmetry() {
        let theta_i = 0.5;
        let theta_t = snells_law(1.0, 1.5, theta_i).unwrap();
        let theta_back = snells_law(1.5, 1.0, theta_t).unwrap();
        assert!((theta_back - theta_i).abs() < 1e-5, "Snell's law should be reversible");
    }

    #[test]
    fn test_fresnel_normal_incidence() {
        let (r, t) = fresnel_coefficients(1.0, 1.5, 0.0);
        // At normal incidence: R = ((n1-n2)/(n1+n2))^2
        let expected_r = ((1.0 - 1.5) / (1.0 + 1.5)).powi(2);
        assert!((r - expected_r).abs() < 0.01, "R={}, expected={}", r, expected_r);
        assert!((r + t - 1.0).abs() < 0.01, "R+T should be 1");
    }

    #[test]
    fn test_diffraction_central_maximum() {
        let intensity = diffraction_single_slit(1.0, 0.5, 0.0);
        assert!((intensity - 1.0).abs() < 1e-6, "Central max should be 1.0");
    }

    #[test]
    fn test_diffraction_first_minimum() {
        // First minimum at sin(θ) = λ/a
        let a = 2.0;
        let lambda = 0.5;
        let angle = (lambda / a).asin();
        let intensity = diffraction_single_slit(a, lambda, angle);
        assert!(intensity < 0.001, "First minimum should be ~0, got {}", intensity);
    }

    #[test]
    fn test_spherical_wave_decay() {
        let wave = SphericalWave::new(Vec3::ZERO, 1.0, 1.0);
        let a1 = wave.evaluate_scalar(Vec3::new(1.0, 0.0, 0.0), 0.0).abs();
        let a2 = wave.evaluate_scalar(Vec3::new(2.0, 0.0, 0.0), 0.0).abs();
        // 1/r decay: ratio should be ~2
        if a2 > 1e-10 {
            let ratio = a1 / a2;
            assert!((ratio - 2.0).abs() < 0.1, "1/r decay ratio: {}", ratio);
        }
    }

    #[test]
    fn test_gaussian_beam_waist() {
        let beam = GaussianBeam::new(Vec3::ZERO, Vec3::X, 1.0, 0.5);
        // At z=0, beam radius = w0
        assert!((beam.beam_radius(0.0) - 1.0).abs() < 1e-6);
        // At z=z_R, beam radius = w0*sqrt(2)
        let zr = beam.rayleigh_range();
        let w_zr = beam.beam_radius(zr);
        assert!((w_zr - 1.0 * 2.0_f32.sqrt()).abs() < 0.01);
    }

    #[test]
    fn test_wave_packet_moves() {
        let wp = WavePacket::new(1.0, 0.1, 0.5);
        // Envelope peak should be at x = v_g * t
        let t = 10.0;
        let peak_x = wp.group_velocity * t;
        // Sample around the peak
        let at_peak = wp.evaluate(peak_x, t).abs();
        let away = wp.evaluate(peak_x + 100.0, t).abs();
        assert!(at_peak > away, "Packet should be centered at group velocity position");
    }

    #[test]
    fn test_interference() {
        let w1 = PlaneWave::new(Vec3::X, Vec3::Y, 1.0, 1.0);
        let w2 = PlaneWave { phase: PI, ..w1.clone() }; // opposite phase
        let points = vec![Vec3::ZERO];
        let pattern = interference_pattern(&w1, &w2, &points, 0.0);
        // Two waves with π phase difference should destructively interfere at origin
        // (both have sin(0+phase) at origin, t=0)
        // w1: sin(0) = 0, w2: sin(pi) = 0. So both are 0 at origin, t=0
        // Let's test at a specific point where it matters
        let points2 = vec![Vec3::new(0.1, 0.0, 0.0)];
        let p = interference_pattern(&w1, &w2, &points2, 0.0);
        // With phase diff of PI, should have reduced intensity
        assert!(p[0] < 1.0);
    }
}
