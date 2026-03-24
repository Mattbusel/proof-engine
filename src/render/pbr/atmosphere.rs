//! Atmospheric and environmental rendering mathematics.
//!
//! Covers Rayleigh/Mie scattering, physical sky models, volumetric fog,
//! procedural clouds, Gerstner ocean waves, and shadow-mapping helpers.
//!
//! All math is CPU-side, using `glam::{Vec2, Vec3, Vec4, Mat4}`.

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::f32::consts::{FRAC_1_PI, PI};

// ─────────────────────────────────────────────────────────────────────────────
// Rayleigh and Mie scattering coefficients
// ─────────────────────────────────────────────────────────────────────────────

/// Rayleigh scattering parameters for Earth's atmosphere.
///
/// `beta` — scattering coefficient per wavelength channel (m⁻¹).
///          Default: (5.8e-6, 1.35e-5, 3.31e-5) for R, G, B.
/// `scale_height` — characteristic scale height (m).  Default: 8000 m.
#[derive(Debug, Clone)]
pub struct RayleighScattering {
    pub beta: Vec3,
    pub scale_height: f32,
}

impl RayleighScattering {
    /// Earth-atmosphere defaults.
    pub fn earth() -> Self {
        Self {
            beta: Vec3::new(5.8e-6, 1.35e-5, 3.31e-5),
            scale_height: 8000.0,
        }
    }

    /// Density relative to sea level at altitude `h` (metres).
    #[inline]
    pub fn density(&self, h: f32) -> f32 {
        (-h / self.scale_height).exp()
    }

    /// Scattering coefficient at altitude `h`.
    #[inline]
    pub fn beta_at(&self, h: f32) -> Vec3 {
        self.beta * self.density(h)
    }
}

impl Default for RayleighScattering {
    fn default() -> Self {
        Self::earth()
    }
}

/// Mie scattering parameters for Earth's atmosphere.
///
/// `beta` — scalar scattering coefficient (m⁻¹).  Default: 21e-6.
/// `scale_height` — scale height (m).  Default: 1200 m (aerosol layer).
/// `g` — Henyey-Greenstein asymmetry parameter.  Default: 0.758 (forward-scattering).
#[derive(Debug, Clone)]
pub struct MieScattering {
    pub beta: f32,
    pub scale_height: f32,
    /// Asymmetry parameter in (-1, 1).  Positive = forward scattering.
    pub g: f32,
}

impl MieScattering {
    /// Earth-atmosphere defaults.
    pub fn earth() -> Self {
        Self {
            beta: 21e-6,
            scale_height: 1200.0,
            g: 0.758,
        }
    }

    /// Density at altitude `h`.
    #[inline]
    pub fn density(&self, h: f32) -> f32 {
        (-h / self.scale_height).exp()
    }

    /// Scattering coefficient at altitude `h`.
    #[inline]
    pub fn beta_at(&self, h: f32) -> f32 {
        self.beta * self.density(h)
    }
}

impl Default for MieScattering {
    fn default() -> Self {
        Self::earth()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase functions
// ─────────────────────────────────────────────────────────────────────────────

/// Rayleigh phase function.
///
/// `cos_theta` — cosine of the scattering angle.
#[inline]
pub fn rayleigh_phase(cos_theta: f32) -> f32 {
    let ct2 = cos_theta * cos_theta;
    3.0 / (16.0 * PI) * (1.0 + ct2)
}

/// Henyey-Greenstein phase function for Mie scattering.
///
/// `g` — asymmetry parameter.
#[inline]
pub fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let t = 1.0 + g2 - 2.0 * g * cos_theta;
    (1.0 - g2) / (4.0 * PI * t.max(1e-10).powf(1.5))
}

/// Cornette-Shanks phase function — more accurate than HG for large g.
#[inline]
pub fn cornette_shanks_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let t = 1.0 + g2 - 2.0 * g * cos_theta;
    3.0 * (1.0 - g2) * (1.0 + cos_theta * cos_theta)
        / (8.0 * PI * (2.0 + g2) * t.max(1e-10).powf(1.5))
}

// ─────────────────────────────────────────────────────────────────────────────
// Optical depth integration
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the optical depth (column density) from `start` to the atmosphere
/// boundary, and also toward the sun for shadow computation.
///
/// Returns `Vec2(rayleigh_depth, mie_depth)`.
///
/// # Parameters
/// - `start`    — starting position in world space (planet-centred, metres)
/// - `dir`      — ray direction (normalised)
/// - `atm`      — atmosphere parameters
/// - `n_steps`  — number of integration steps
pub fn optical_depth(
    start: Vec3,
    dir: Vec3,
    atm: &AtmosphereParams,
    n_steps: usize,
) -> Vec2 {
    let ray_len = ray_sphere_intersect(start, dir, atm.radius_atm)
        .unwrap_or(0.0)
        .max(0.0);

    if ray_len < 1e-3 {
        return Vec2::ZERO;
    }

    let step_len = ray_len / n_steps as f32;
    let mut rayleigh = 0.0f32;
    let mut mie = 0.0f32;

    for i in 0..n_steps {
        let t = (i as f32 + 0.5) * step_len;
        let pos = start + dir * t;
        let h = (pos.length() - atm.radius_planet).max(0.0);
        rayleigh += atm.rayleigh.density(h) * step_len;
        mie += atm.mie.density(h) * step_len;
    }

    Vec2::new(rayleigh, mie)
}

/// Intersect a ray with a sphere centred at the origin.
/// Returns the far intersection distance `t`, or `None` if no intersection.
fn ray_sphere_intersect(origin: Vec3, dir: Vec3, radius: f32) -> Option<f32> {
    let b = origin.dot(dir);
    let c = origin.dot(origin) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t1 = -b - sq;
    let t2 = -b + sq;
    if t2 < 0.0 {
        None
    } else {
        Some(t2.max(0.0))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AtmosphereParams + SkyModel
// ─────────────────────────────────────────────────────────────────────────────

/// Full atmosphere parameter set for physical sky rendering.
#[derive(Debug, Clone)]
pub struct AtmosphereParams {
    /// Planet (ground) radius in metres.  Default: 6_371_000 m (Earth).
    pub radius_planet: f32,
    /// Atmosphere outer radius in metres.  Default: 6_471_000 m.
    pub radius_atm: f32,
    pub rayleigh: RayleighScattering,
    pub mie: MieScattering,
    /// Irradiance of the sun disc (W/m²/sr).  Default: 20.
    pub sun_intensity: f32,
    /// Unit vector pointing toward the sun.
    pub sun_dir: Vec3,
}

impl AtmosphereParams {
    pub fn earth_default(sun_dir: Vec3) -> Self {
        Self {
            radius_planet: 6_371_000.0,
            radius_atm: 6_471_000.0,
            rayleigh: RayleighScattering::earth(),
            mie: MieScattering::earth(),
            sun_intensity: 20.0,
            sun_dir: sun_dir.normalize(),
        }
    }
}

impl Default for AtmosphereParams {
    fn default() -> Self {
        Self::earth_default(Vec3::new(0.0, 1.0, 0.0))
    }
}

/// Physical sky colour model using single-scattering path integrals.
pub struct SkyModel {
    pub params: AtmosphereParams,
}

impl SkyModel {
    pub fn new(params: AtmosphereParams) -> Self {
        Self { params }
    }

    /// Compute sky colour for a view ray `ray_dir` (unit vector).
    ///
    /// Uses `n_steps` integration steps along the view ray, and `n_light_steps`
    /// for each secondary ray toward the sun.
    pub fn sky_color(&self, ray_dir: Vec3, n_steps: usize) -> Vec3 {
        let p = &self.params;
        let origin = Vec3::new(0.0, p.radius_planet + 1.0, 0.0);
        let ray_dir = ray_dir.normalize();

        let ray_len = match ray_sphere_intersect(origin, ray_dir, p.radius_atm) {
            Some(t) => t,
            None => return Vec3::ZERO,
        };

        // If ray hits the planet, shorten the path
        let ray_len = if let Some(t_ground) = ray_sphere_intersect(origin, ray_dir, p.radius_planet) {
            ray_len.min(t_ground)
        } else {
            ray_len
        };

        let step_len = ray_len / n_steps as f32;
        let mut rayleigh_sum = Vec3::ZERO;
        let mut mie_sum = Vec3::ZERO;

        // Accumulated optical depth along view ray
        let mut od_view_r = 0.0f32;
        let mut od_view_m = 0.0f32;

        let n_light = 8;

        for i in 0..n_steps {
            let t = (i as f32 + 0.5) * step_len;
            let pos = origin + ray_dir * t;
            let h = (pos.length() - p.radius_planet).max(0.0);

            let rho_r = p.rayleigh.density(h);
            let rho_m = p.mie.density(h);

            od_view_r += rho_r * step_len;
            od_view_m += rho_m * step_len;

            // Light ray optical depth to sun
            let od_light = optical_depth(pos, p.sun_dir, p, n_light);

            // Combined optical depth
            let tau_r = p.rayleigh.beta * (od_view_r + od_light.x);
            let tau_m = Vec3::splat(p.mie.beta * 1.1) * (od_view_m + od_light.y);
            let attenuation = (-(tau_r + tau_m)).exp();

            rayleigh_sum += attenuation * rho_r * step_len;
            mie_sum += attenuation * Vec3::splat(rho_m * step_len);
        }

        let cos_theta = ray_dir.dot(p.sun_dir);
        let phase_r = rayleigh_phase(cos_theta);
        let phase_m = mie_phase(cos_theta, p.mie.g);

        p.sun_intensity
            * (p.rayleigh.beta * phase_r * rayleigh_sum
                + Vec3::splat(p.mie.beta * phase_m) * mie_sum)
    }

    /// Compute transmittance along a ray from `origin` toward the sun.
    pub fn sun_transmittance(&self, ray_dir: Vec3) -> Vec3 {
        let p = &self.params;
        let origin = Vec3::new(0.0, p.radius_planet + 1.0, 0.0);
        let od = optical_depth(origin, ray_dir.normalize(), p, 32);
        let tau_r = p.rayleigh.beta * od.x;
        let tau_m = Vec3::splat(p.mie.beta * 1.1 * od.y);
        (-(tau_r + tau_m)).exp()
    }

    /// Approximate sky irradiance on a surface with the given `normal`.
    ///
    /// Integrates the sky radiance over the upper hemisphere and weights by
    /// NdotL.  Uses a coarse stratified sample.
    pub fn sky_irradiance(&self, normal: Vec3) -> Vec3 {
        let n_theta = 8;
        let n_phi = 16;
        let mut irradiance = Vec3::ZERO;
        let n = normal.normalize();

        for it in 0..n_theta {
            let theta = (it as f32 + 0.5) / n_theta as f32 * std::f32::consts::FRAC_PI_2;
            let sin_t = theta.sin();
            let cos_t = theta.cos();
            for ip in 0..n_phi {
                let phi = (ip as f32 + 0.5) / n_phi as f32 * 2.0 * PI;
                let dir = Vec3::new(sin_t * phi.cos(), cos_t, sin_t * phi.sin());
                let n_dot_l = n.dot(dir).max(0.0);
                let d_omega = sin_t
                    * (std::f32::consts::FRAC_PI_2 / n_theta as f32)
                    * (2.0 * PI / n_phi as f32);
                irradiance += self.sky_color(dir, 6) * n_dot_l * d_omega;
            }
        }
        irradiance
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Preetham analytic sky model
// ─────────────────────────────────────────────────────────────────────────────

/// Simplified Preetham sky model.
pub struct Preetham;

impl Preetham {
    /// Zenith luminance (kcd/m²) given turbidity `t` and sun elevation `theta_s` (radians).
    pub fn zenith_luminance(turbidity: f32, theta_s: f32) -> f32 {
        let chi = (4.0 / 9.0 - turbidity / 120.0) * (PI - 2.0 * theta_s);
        (4.0453 * turbidity - 4.9710) * chi.tan() - 0.2155 * turbidity + 2.4192
    }

    /// Preetham distribution function F.
    fn perez(theta: f32, gamma: f32, a: f32, b: f32, c: f32, d: f32, e: f32) -> f32 {
        (1.0 + a * (-b / theta.cos().max(1e-3)).exp())
            * (1.0 + c * (-d * gamma).exp() + e * gamma.cos() * gamma.cos())
    }

    /// Compute sky luminance (Y, x, y in Yxy colour space) for a view direction.
    ///
    /// `view_dir` — unit vector pointing from camera into sky
    /// `sun_dir`  — unit vector toward sun
    /// `turbidity` — atmosphere turbidity (1 = pure air, 10 = heavy haze)
    pub fn sky_luminance(view_dir: Vec3, sun_dir: Vec3, turbidity: f32) -> Vec3 {
        let view_dir = view_dir.normalize();
        let sun_dir = sun_dir.normalize();

        // Elevation angle of sun
        let theta_s = sun_dir.y.clamp(-1.0, 1.0).acos();

        // Perez coefficients for Y, x, y
        let t = turbidity;
        let ay = 0.1787 * t - 1.4630;
        let by = -0.3554 * t + 0.4275;
        let cy = -0.0227 * t + 5.3251;
        let dy = 0.1206 * t - 2.5771;
        let ey = -0.0670 * t + 0.3703;

        let ax = -0.0193 * t - 0.2592;
        let bx = -0.0665 * t + 0.0008;
        let cx = -0.0004 * t + 0.2125;
        let dx = -0.0641 * t - 0.8989;
        let ex = -0.0033 * t + 0.0452;

        let az = -0.0167 * t - 0.2608;
        let bz = -0.0950 * t + 0.0092;
        let cz = -0.0079 * t + 0.2102;
        let dz = -0.0441 * t - 1.6537;
        let ez = -0.0109 * t + 0.0529;

        // Angle between view and zenith
        let theta = view_dir.y.clamp(-1.0, 1.0).acos();
        // Angle between view and sun
        let cos_gamma = view_dir.dot(sun_dir).clamp(-1.0, 1.0);
        let gamma = cos_gamma.acos();

        let yz = Self::zenith_luminance(t, theta_s);
        let xz = t * (0.0026 + 0.00008 * t - 0.000021 * theta_s * theta_s)
            + (-0.0669 + 0.00209 * t + 0.000028 * theta_s * theta_s);
        let zz = t * (-0.0065 + 0.0 + 0.000001 * theta_s * theta_s)
            + (0.0659 - 0.0015 * t + 0.000047 * theta_s * theta_s);

        let y_val = yz
            * Self::perez(theta, gamma, ay, by, cy, dy, ey)
            / Self::perez(0.0, theta_s, ay, by, cy, dy, ey);
        let x_val = xz
            * Self::perez(theta, gamma, ax, bx, cx, dx, ex)
            / Self::perez(0.0, theta_s, ax, bx, cx, dx, ex);
        let z_val = zz
            * Self::perez(theta, gamma, az, bz, cz, dz, ez)
            / Self::perez(0.0, theta_s, az, bz, cz, dz, ez);

        Vec3::new(y_val, x_val, z_val)
    }

    /// Convert CIE Yxy to linear sRGB.
    pub fn yxy_to_rgb(yxy: Vec3) -> Vec3 {
        let y = yxy.x;
        let x_chrom = yxy.y;
        let y_chrom = yxy.z;

        if y < 1e-5 {
            return Vec3::ZERO;
        }

        // Yxy to XYZ
        let xyz_x = x_chrom / y_chrom * y;
        let xyz_y = y;
        let xyz_z = (1.0 - x_chrom - y_chrom) / y_chrom * y;

        // XYZ to linear sRGB (D65 illuminant)
        let r = 3.2406 * xyz_x - 1.5372 * xyz_y - 0.4986 * xyz_z;
        let g = -0.9689 * xyz_x + 1.8758 * xyz_y + 0.0415 * xyz_z;
        let b = 0.0557 * xyz_x - 0.2040 * xyz_y + 1.0570 * xyz_z;

        Vec3::new(r, g, b).max(Vec3::ZERO)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transmittance LUT (Bruneton-style precomputed atmosphere)
// ─────────────────────────────────────────────────────────────────────────────

/// Precomputed transmittance lookup table.
///
/// Parameterised by `(cos_zenith, altitude)` where:
/// - `cos_zenith` in [-1, 1] — cosine of zenith angle
/// - `altitude` in [0, radius_atm - radius_planet] — height above surface (m)
pub struct TransmittanceLut {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Vec3>,
    pub params: AtmosphereParams,
}

impl TransmittanceLut {
    /// Compute the transmittance LUT.
    pub fn compute(params: &AtmosphereParams) -> Self {
        let width = 256;
        let height = 64;
        let mut data = vec![Vec3::ONE; width * height];

        for row in 0..height {
            let altitude = row as f32 / (height - 1) as f32
                * (params.radius_atm - params.radius_planet);
            let h = params.radius_planet + altitude;

            for col in 0..width {
                let cos_z = col as f32 / (width - 1) as f32 * 2.0 - 1.0;
                let sin_z = (1.0 - cos_z * cos_z).max(0.0).sqrt();
                let dir = Vec3::new(sin_z, cos_z, 0.0);
                let origin = Vec3::new(0.0, h, 0.0);

                let od = optical_depth(origin, dir, params, 32);
                let tau_r = params.rayleigh.beta * od.x;
                let tau_m = Vec3::splat(params.mie.beta * 1.1 * od.y);
                let transmittance = (-(tau_r + tau_m)).exp();

                data[row * width + col] = transmittance;
            }
        }

        Self {
            width,
            height,
            data,
            params: params.clone(),
        }
    }

    /// Bilinearly sample the LUT.
    ///
    /// `cos_zenith` in [-1, 1], `altitude` in metres [0, atm_thickness].
    pub fn sample(&self, cos_zenith: f32, altitude: f32) -> Vec3 {
        let atm_thickness = self.params.radius_atm - self.params.radius_planet;
        let alt_norm = (altitude / atm_thickness).clamp(0.0, 1.0);
        let cz_norm = (cos_zenith * 0.5 + 0.5).clamp(0.0, 1.0);

        let row_f = alt_norm * (self.height - 1) as f32;
        let col_f = cz_norm * (self.width - 1) as f32;

        let row0 = (row_f as usize).min(self.height - 1);
        let col0 = (col_f as usize).min(self.width - 1);
        let row1 = (row0 + 1).min(self.height - 1);
        let col1 = (col0 + 1).min(self.width - 1);

        let tr = row_f - row0 as f32;
        let tc = col_f - col0 as f32;

        let s00 = self.data[row0 * self.width + col0];
        let s10 = self.data[row0 * self.width + col1];
        let s01 = self.data[row1 * self.width + col0];
        let s11 = self.data[row1 * self.width + col1];

        s00.lerp(s10, tc).lerp(s01.lerp(s11, tc), tr)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Volumetric fog
// ─────────────────────────────────────────────────────────────────────────────

/// Homogeneous volumetric fog parameters.
#[derive(Debug, Clone)]
pub struct VolumetricFog {
    /// Extinction (scattering + absorption) density coefficient.
    pub density: f32,
    /// Per-channel absorption coefficient (κ_a).
    pub absorption: Vec3,
    /// Per-channel scattering coefficient (κ_s).
    pub scattering: Vec3,
    /// Henyey-Greenstein g parameter for phase function.
    pub anisotropy: f32,
}

impl VolumetricFog {
    pub fn new(density: f32, absorption: Vec3, scattering: Vec3, anisotropy: f32) -> Self {
        Self {
            density,
            absorption,
            scattering,
            anisotropy,
        }
    }

    /// Uniform white fog (scattering-only).
    pub fn uniform_white(density: f32) -> Self {
        Self::new(density, Vec3::ZERO, Vec3::ONE, 0.0)
    }

    /// Extinction coefficient.
    pub fn extinction(&self) -> Vec3 {
        (self.absorption + self.scattering) * self.density
    }
}

impl Default for VolumetricFog {
    fn default() -> Self {
        Self::uniform_white(0.02)
    }
}

/// Beer-Lambert transmittance through homogeneous fog of `distance`.
pub fn fog_transmittance(distance: f32, fog: &VolumetricFog) -> f32 {
    let ext = fog.extinction().length(); // scalar approximation
    (-ext * distance).exp()
}

/// In-scattering colour accumulated along a ray through volumetric fog toward a
/// single directional light.
///
/// Returns the inscattered radiance contribution.
pub fn fog_inscattering(
    start: Vec3,
    end: Vec3,
    light_dir: Vec3,
    light_color: Vec3,
    fog: &VolumetricFog,
    n_steps: usize,
) -> Vec3 {
    let seg = end - start;
    let dist = seg.length();
    if dist < 1e-5 {
        return Vec3::ZERO;
    }
    let dir = seg / dist;
    let step_len = dist / n_steps as f32;

    let cos_theta = dir.dot(light_dir.normalize());
    let phase = mie_phase(cos_theta, fog.anisotropy);
    let ext = fog.extinction();

    let mut inscattered = Vec3::ZERO;
    for i in 0..n_steps {
        let t = (i as f32 + 0.5) * step_len;
        // Transmittance from start to sample point
        let t_view = (-ext * t).exp();
        // Transmittance from sample point toward light (assume the same fog, no scene geometry)
        let t_light = (-ext * step_len).exp();
        let sample_inscatter = fog.scattering * fog.density * phase * light_color * t_view * t_light * step_len;
        inscattered += sample_inscatter;
    }
    inscattered
}

/// Exponential height-fog density at world position `pos`.
///
/// `density`    — base fog density at `fog_height`
/// `falloff`    — controls how quickly density decreases above `fog_height`
/// `fog_height` — altitude (world Y) at which density == `density`
pub fn exponential_height_fog(pos: Vec3, density: f32, falloff: f32, fog_height: f32) -> f32 {
    let h = pos.y - fog_height;
    density * (-falloff * h).exp()
}

// ─────────────────────────────────────────────────────────────────────────────
// Clouds
// ─────────────────────────────────────────────────────────────────────────────

/// A single cloud layer definition.
#[derive(Debug, Clone)]
pub struct CloudLayer {
    /// Base altitude (metres above sea level).
    pub altitude: f32,
    /// Vertical extent of the layer (metres).
    pub thickness: f32,
    /// Cloud coverage factor [0, 1].
    pub coverage: f32,
    /// Maximum density within the layer.
    pub density: f32,
    /// Wind vector (moves the cloud pattern over time).
    pub wind: Vec3,
}

impl CloudLayer {
    pub fn new(altitude: f32, thickness: f32, coverage: f32, density: f32, wind: Vec3) -> Self {
        Self {
            altitude,
            thickness,
            coverage,
            density,
            wind,
        }
    }

    /// Cumulus layer preset.
    pub fn cumulus() -> Self {
        Self::new(2000.0, 1500.0, 0.5, 0.8, Vec3::new(10.0, 0.0, 5.0))
    }

    /// Cirrus layer preset.
    pub fn cirrus() -> Self {
        Self::new(8000.0, 500.0, 0.3, 0.4, Vec3::new(30.0, 0.0, 15.0))
    }
}

/// Evaluate procedural cloud density at `pos` using layered FBM noise.
///
/// `time` advances the wind offset.
pub fn sample_cloud_density(pos: Vec3, time: f32, layer: &CloudLayer) -> f32 {
    // Height gradient: density falls off at layer boundaries
    let rel_h = (pos.y - layer.altitude) / layer.thickness.max(1.0);
    if rel_h < 0.0 || rel_h > 1.0 {
        return 0.0;
    }
    // Gaussian height profile
    let h_weight = {
        let x = rel_h * 2.0 - 1.0;
        (-x * x * 4.0).exp()
    };

    // Wind offset
    let wind_offset = layer.wind * time;
    let sample_pos = pos + wind_offset;

    // Multi-octave hash-based noise (no external crate)
    let density_noise = fbm_noise(sample_pos, 5);

    // Remap and apply coverage
    let raw = (density_noise - (1.0 - layer.coverage)).max(0.0) / layer.coverage.max(1e-4);
    raw * h_weight * layer.density
}

/// Simple 3D hash-based FBM noise (no hardware textures).
fn fbm_noise(p: Vec3, octaves: usize) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 0.5f32;
    let mut frequency = 1.0f32;

    for _ in 0..octaves {
        value += amplitude * hash31(p * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    value
}

/// Hash function Vec3 → [0,1].
fn hash31(p: Vec3) -> f32 {
    let mut h = p.x * 127.1 + p.y * 311.7 + p.z * 74.7;
    h = (h.sin() * 43758.5453123).fract();
    h.abs()
}

/// Raymarche through cloud layers and return `Vec4(rgb_color, alpha)`.
///
/// `ray_origin` — world-space ray start
/// `ray_dir`    — normalised ray direction
/// `layers`     — cloud layer definitions
/// `sun_dir`    — direction toward the sun
/// `n_steps`    — number of marching steps
pub fn raymarch_cloud(
    ray_origin: Vec3,
    ray_dir: Vec3,
    layers: &[CloudLayer],
    sun_dir: Vec3,
    n_steps: usize,
) -> Vec4 {
    if layers.is_empty() {
        return Vec4::ZERO;
    }
    let ray_dir = ray_dir.normalize();

    // Find the altitude range of all layers
    let min_alt = layers.iter().map(|l| l.altitude).fold(f32::INFINITY, f32::min);
    let max_alt = layers.iter().map(|l| l.altitude + l.thickness).fold(0.0f32, f32::max);

    // Compute entry and exit t values for the altitude slab
    if ray_dir.y.abs() < 1e-5 {
        return Vec4::ZERO; // Ray is horizontal — skip
    }
    let t_min = (min_alt - ray_origin.y) / ray_dir.y;
    let t_max = (max_alt - ray_origin.y) / ray_dir.y;
    let (t_enter, t_exit) = if t_min < t_max {
        (t_min.max(0.0), t_max.max(0.0))
    } else {
        (t_max.max(0.0), t_min.max(0.0))
    };

    if t_enter >= t_exit {
        return Vec4::ZERO;
    }

    let step_len = (t_exit - t_enter) / n_steps as f32;
    let mut accumulated_density = 0.0f32;
    let mut light_energy = Vec3::ZERO;

    for i in 0..n_steps {
        let t = t_enter + (i as f32 + 0.5) * step_len;
        let pos = ray_origin + ray_dir * t;

        let mut total_density = 0.0f32;
        for layer in layers {
            total_density += sample_cloud_density(pos, 0.0, layer);
        }
        if total_density < 1e-4 {
            continue;
        }

        // Beer's law transmittance along view ray so far
        let transmittance_view = (-accumulated_density * 0.1).exp();

        // Cheap light ray — step toward sun
        let mut sun_density = 0.0f32;
        for ls in 0..4 {
            let sun_pos = pos + sun_dir * (ls as f32 * 200.0);
            for layer in layers {
                sun_density += sample_cloud_density(sun_pos, 0.0, layer);
            }
        }
        let transmittance_sun = (-sun_density * 0.1 * 200.0).exp();

        // Henyey-Greenstein phase toward sun
        let cos_theta = ray_dir.dot(sun_dir.normalize());
        let phase = mie_phase(cos_theta, 0.3);

        // White sun colour
        let sun_col = Vec3::new(1.0, 0.95, 0.8) * 3.0;

        light_energy += sun_col * transmittance_sun * transmittance_view * phase * total_density * step_len;
        accumulated_density += total_density * step_len;
    }

    let alpha = 1.0 - (-accumulated_density * 0.1).exp();
    let color = light_energy + Vec3::new(0.8, 0.85, 0.95) * (1.0 - alpha); // sky tint

    Vec4::new(color.x, color.y, color.z, alpha.clamp(0.0, 1.0))
}

// ─────────────────────────────────────────────────────────────────────────────
// Ocean / water waves
// ─────────────────────────────────────────────────────────────────────────────

/// A single Gerstner wave component.
#[derive(Debug, Clone)]
pub struct OceanWave {
    /// Wave amplitude (metres).
    pub amplitude: f32,
    /// Wavelength (metres).
    pub wavelength: f32,
    /// Horizontal direction of propagation (unit Vec2).
    pub direction: Vec2,
    /// Phase speed (m/s).  Deep-water: `sqrt(g * wavelength / 2π)`.
    pub phase_speed: f32,
}

impl OceanWave {
    pub fn new(amplitude: f32, wavelength: f32, direction: Vec2, phase_speed: f32) -> Self {
        Self {
            amplitude,
            wavelength,
            direction: direction.normalize(),
            phase_speed,
        }
    }

    /// Create a physically-based deep-water wave.
    pub fn deep_water(amplitude: f32, wavelength: f32, direction: Vec2) -> Self {
        let g = 9.81_f32;
        let phase_speed = (g * wavelength / (2.0 * PI)).sqrt();
        Self::new(amplitude, wavelength, direction, phase_speed)
    }

    /// Wave number k = 2π / λ.
    #[inline]
    pub fn wave_number(&self) -> f32 {
        2.0 * PI / self.wavelength.max(1e-5)
    }

    /// Angular frequency ω = k * c.
    #[inline]
    pub fn omega(&self) -> f32 {
        self.wave_number() * self.phase_speed
    }
}

/// Compute Gerstner wave displacement at horizontal position `pos` and time `time`.
///
/// Returns a Vec3 where (x, y, z) correspond to (horizontal offset x, height y,
/// horizontal offset z).
pub fn gerstner_wave(pos: Vec2, time: f32, waves: &[OceanWave]) -> Vec3 {
    let mut displacement = Vec3::ZERO;

    for wave in waves {
        let k = wave.wave_number();
        let omega = wave.omega();
        let phase = wave.direction.dot(pos) * k - omega * time;
        let sin_phase = phase.sin();
        let cos_phase = phase.cos();

        // Steepness Q — limits wave crest sharpness (0 = sine wave, 1 = trochoidal)
        let q = (wave.amplitude * k).min(0.9);

        displacement.x += q * wave.amplitude * wave.direction.x * cos_phase;
        displacement.y += wave.amplitude * sin_phase;
        displacement.z += q * wave.amplitude * wave.direction.y * cos_phase;
    }

    displacement
}

/// Compute the analytic surface normal of a Gerstner wave field.
///
/// Returns a unit normal pointing upward from the ocean surface.
pub fn ocean_normal(pos: Vec2, time: f32, waves: &[OceanWave]) -> Vec3 {
    let mut b = Vec3::ZERO; // ∂P/∂x (tangent x)
    let mut t = Vec3::ZERO; // ∂P/∂z (tangent z)

    for wave in waves {
        let k = wave.wave_number();
        let omega = wave.omega();
        let phase = wave.direction.dot(pos) * k - omega * time;
        let sin_phase = phase.sin();
        let cos_phase = phase.cos();

        let q = (wave.amplitude * k).min(0.9);
        let wa = wave.amplitude * k;

        // Partial derivatives of Gerstner displacement
        b.x += -q * wa * wave.direction.x * wave.direction.x * sin_phase;
        b.y += wa * wave.direction.x * cos_phase;
        b.z += -q * wa * wave.direction.x * wave.direction.y * sin_phase;

        t.x += -q * wa * wave.direction.x * wave.direction.y * sin_phase;
        t.y += wa * wave.direction.y * cos_phase;
        t.z += -q * wa * wave.direction.y * wave.direction.y * sin_phase;
    }

    // Tangent x: (1, 0, 0) + b; Tangent z: (0, 0, 1) + t
    let tx = Vec3::new(1.0 + b.x, b.y, b.z);
    let tz = Vec3::new(t.x, t.y, 1.0 + t.z);
    tx.cross(tz).normalize()
}

/// Fresnel reflectance for water (IOR = 1.333) using Schlick approximation.
///
/// `view_angle` — angle between view ray and surface normal (radians).
pub fn water_fresnel(view_angle: f32) -> f32 {
    let cos_theta = view_angle.cos().clamp(0.0, 1.0);
    let ior = 1.333_f32;
    let f0_val = {
        let t = (ior - 1.0) / (ior + 1.0);
        t * t
    };
    f0_val + (1.0 - f0_val) * (1.0 - cos_theta).powi(5)
}

/// Simple deep-water colour model.
///
/// Composites subsurface scattering, sun reflectance, and depth absorption.
pub fn water_color(depth: f32, sun_pos: Vec3, view_dir: Vec3) -> Vec3 {
    let view_dir = view_dir.normalize();
    let sun_dir = sun_pos.normalize();

    // Absorption coefficient (red attenuates fastest)
    let absorption = Vec3::new(0.45, 0.12, 0.05);
    let transmitted = (-absorption * depth.max(0.0)).exp();

    // Deep-water base colour (blue-green)
    let deep_color = Vec3::new(0.01, 0.12, 0.25);
    let subsurface = deep_color * transmitted;

    // Specular sun reflection
    let n = Vec3::Y; // assume flat water
    let h = (sun_dir - view_dir).normalize();
    let n_dot_h = n.dot(h).max(0.0);
    let sun_spec = Vec3::splat(200.0 * n_dot_h.powf(256.0));

    // Fresnel blend: above water surface
    let cos_theta = (-view_dir).dot(n).clamp(0.0, 1.0);
    let fresnel = water_fresnel(cos_theta.acos());

    subsurface * (1.0 - fresnel) + sun_spec * fresnel
}

// ─────────────────────────────────────────────────────────────────────────────
// Shadow mapping helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute cascade split distances for Cascaded Shadow Maps (CSM).
///
/// Uses a blend between linear and logarithmic splits controlled by `lambda`.
///
/// `near`, `far`   — camera frustum near/far planes
/// `n_cascades`    — number of cascade levels
/// `lambda`        — blend factor: 0 = uniform, 1 = logarithmic
///
/// Returns a `Vec<f32>` of length `n_cascades + 1` (near…far boundaries).
pub fn cascade_shadow_splits(near: f32, far: f32, n_cascades: usize, lambda: f32) -> Vec<f32> {
    let n = n_cascades as f32;
    let mut splits = Vec::with_capacity(n_cascades + 1);
    splits.push(near);

    for i in 1..n_cascades {
        let p = i as f32 / n;
        let log_split = near * (far / near).powf(p);
        let uni_split = near + (far - near) * p;
        splits.push(lambda * log_split + (1.0 - lambda) * uni_split);
    }
    splits.push(far);
    splits
}

/// Compute a light-space view matrix and ortho projection for a directional
/// light, tightly fitted to a scene AABB.
///
/// Returns `(view, proj)` matrices.
pub fn light_space_matrix(
    light_dir: Vec3,
    scene_aabb: (Vec3, Vec3),
) -> (Mat4, Mat4) {
    let (aabb_min, aabb_max) = scene_aabb;
    let center = (aabb_min + aabb_max) * 0.5;
    let extent = aabb_max - aabb_min;
    let radius = extent.length() * 0.5;

    let light_dir = light_dir.normalize();
    // Pick an up vector not parallel to light_dir
    let up = if light_dir.y.abs() < 0.99 {
        Vec3::Y
    } else {
        Vec3::Z
    };

    let eye = center - light_dir * radius;
    let view = Mat4::look_at_rh(eye, center, up);

    // Tight ortho based on AABB extent
    let half = radius * 1.05; // small margin
    let proj = Mat4::orthographic_rh(-half, half, -half, half, -half * 2.0, half * 2.0);

    (view, proj)
}

/// Bias matrix to map NDC [-1,1] to UV [0,1] and depth [0,1].
pub fn bias_matrix() -> Mat4 {
    Mat4::from_cols_array(&[
        0.5, 0.0, 0.0, 0.0, // col 0
        0.0, 0.5, 0.0, 0.0, // col 1
        0.0, 0.0, 0.5, 0.0, // col 2
        0.5, 0.5, 0.5, 1.0, // col 3 (translation)
    ])
}

/// Generate a Poisson-disc PCF kernel of `size` taps.
///
/// Returns UV offsets in `[-1, 1]²`.
pub fn pcf_kernel(size: usize) -> Vec<Vec2> {
    // Use a deterministic Poisson disc via jittered grid
    let mut samples = Vec::with_capacity(size);
    let sqrt_size = (size as f32).sqrt().ceil() as usize;

    for i in 0..sqrt_size {
        for j in 0..sqrt_size {
            if samples.len() >= size {
                break;
            }
            let u = (i as f32 + 0.5) / sqrt_size as f32;
            let v = (j as f32 + 0.5) / sqrt_size as f32;
            // Jitter with hash
            let jx = hash31(Vec3::new(i as f32, j as f32, 0.3)) * 0.5 / sqrt_size as f32;
            let jy = hash31(Vec3::new(j as f32, i as f32, 0.7)) * 0.5 / sqrt_size as f32;
            samples.push(Vec2::new(u + jx, v + jy) * 2.0 - Vec2::ONE);
        }
        if samples.len() >= size {
            break;
        }
    }

    samples.truncate(size);
    samples
}

/// Compute the PCSS blocker-search UV radius.
///
/// `light_size_uv` — angular size of the light in shadow-map UV space
/// `depth`         — receiver depth in [0, 1]
pub fn pcss_blocker_search_uv(light_size_uv: f32, depth: f32) -> f32 {
    // Penumbra estimate: larger when receiver is far from blocker
    light_size_uv * depth / (1.0 - depth).max(0.001)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn rayleigh_phase_integrates_to_one() {
        // ∫ rayleigh_phase(cos θ) dΩ = 1
        // dΩ = sin θ dθ dφ; integrate over sphere
        let n = 1000;
        let mut sum = 0.0f32;
        for i in 0..n {
            let theta = (i as f32 + 0.5) / n as f32 * PI;
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            sum += rayleigh_phase(cos_t) * sin_t * (PI / n as f32) * 2.0 * PI;
        }
        assert!((sum - 1.0).abs() < 0.01, "Rayleigh phase integral = {sum}");
    }

    #[test]
    fn mie_phase_integrates_to_one() {
        let n = 1000;
        let g = 0.7;
        let mut sum = 0.0f32;
        for i in 0..n {
            let theta = (i as f32 + 0.5) / n as f32 * PI;
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            sum += mie_phase(cos_t, g) * sin_t * (PI / n as f32) * 2.0 * PI;
        }
        assert!((sum - 1.0).abs() < 0.02, "Mie phase integral = {sum}");
    }

    #[test]
    fn sky_color_is_non_negative() {
        let sky = SkyModel::new(AtmosphereParams::default());
        let color = sky.sky_color(Vec3::Y, 16);
        assert!(color.x >= 0.0 && color.y >= 0.0 && color.z >= 0.0);
    }

    #[test]
    fn sky_color_is_blue_biased() {
        let sky = SkyModel::new(AtmosphereParams::default());
        let color = sky.sky_color(Vec3::new(0.2, 0.8, 0.0).normalize(), 16);
        // Sky should be more blue than red
        assert!(color.z >= color.x, "Sky should be blue-biased: {color:?}");
    }

    #[test]
    fn transmittance_lut_values_in_range() {
        let params = AtmosphereParams::default();
        let lut = TransmittanceLut::compute(&params);
        for &t in &lut.data {
            assert!(t.x >= 0.0 && t.x <= 1.0, "Transmittance out of [0,1]: {t:?}");
        }
    }

    #[test]
    fn fog_transmittance_zero_distance_is_one() {
        let fog = VolumetricFog::default();
        let t = fog_transmittance(0.0, &fog);
        assert!((t - 1.0).abs() < 1e-5);
    }

    #[test]
    fn fog_transmittance_decreases_with_distance() {
        let fog = VolumetricFog::default();
        let t1 = fog_transmittance(10.0, &fog);
        let t2 = fog_transmittance(100.0, &fog);
        assert!(t1 > t2, "Transmittance should decrease with distance");
    }

    #[test]
    fn gerstner_wave_zero_time_is_finite() {
        let waves = vec![OceanWave::deep_water(0.5, 10.0, Vec2::X)];
        let d = gerstner_wave(Vec2::new(5.0, 5.0), 0.0, &waves);
        assert!(d.is_finite(), "Gerstner displacement should be finite: {d:?}");
    }

    #[test]
    fn ocean_normal_is_unit_length() {
        let waves = vec![
            OceanWave::deep_water(0.3, 8.0, Vec2::X),
            OceanWave::deep_water(0.2, 5.0, Vec2::Y),
        ];
        let n = ocean_normal(Vec2::new(3.0, 7.0), 2.5, &waves);
        assert!((n.length() - 1.0).abs() < 1e-4, "Normal should be unit: {n:?}");
    }

    #[test]
    fn water_fresnel_at_grazing_is_one() {
        let f = water_fresnel(std::f32::consts::FRAC_PI_2);
        assert!((f - 1.0).abs() < 0.01, "Grazing fresnel ~1, got {f}");
    }

    #[test]
    fn cascade_splits_monotone() {
        let splits = cascade_shadow_splits(0.1, 1000.0, 4, 0.5);
        assert_eq!(splits.len(), 5);
        for w in splits.windows(2) {
            assert!(w[1] > w[0], "Splits should be monotone increasing");
        }
    }

    #[test]
    fn bias_matrix_maps_neg1_to_0() {
        let bm = bias_matrix();
        let p = bm.transform_point3(Vec3::new(-1.0, -1.0, -1.0));
        assert!((p - Vec3::ZERO).length() < 1e-4, "Bias matrix: {p:?}");
    }

    #[test]
    fn pcf_kernel_correct_count() {
        let k = pcf_kernel(16);
        assert_eq!(k.len(), 16);
    }

    #[test]
    fn exponential_height_fog_decreases_with_height() {
        let f0 = exponential_height_fog(Vec3::new(0.0, 0.0, 0.0), 0.1, 0.5, 0.0);
        let f1 = exponential_height_fog(Vec3::new(0.0, 100.0, 0.0), 0.1, 0.5, 0.0);
        assert!(f0 > f1, "Fog should be denser near the ground");
    }

    #[test]
    fn cloud_density_outside_layer_is_zero() {
        let layer = CloudLayer::cumulus();
        let pos_below = Vec3::new(100.0, layer.altitude - 100.0, 100.0);
        let d = sample_cloud_density(pos_below, 0.0, &layer);
        assert_eq!(d, 0.0, "Cloud density below layer should be 0");
    }

    #[test]
    fn pcss_blocker_search_grows_with_depth() {
        let uv0 = pcss_blocker_search_uv(0.01, 0.3);
        let uv1 = pcss_blocker_search_uv(0.01, 0.7);
        assert!(uv1 > uv0, "PCSS blocker radius should grow with depth");
    }

    #[test]
    fn preetham_sky_luminance_is_finite() {
        let view_dir = Vec3::new(0.3, 0.8, 0.1).normalize();
        let sun_dir = Vec3::new(0.5, 0.6, 0.3).normalize();
        let yxy = Preetham::sky_luminance(view_dir, sun_dir, 2.5);
        assert!(
            yxy.x.is_finite() && yxy.y.is_finite() && yxy.z.is_finite(),
            "Preetham luminance should be finite: {yxy:?}"
        );
    }
}
