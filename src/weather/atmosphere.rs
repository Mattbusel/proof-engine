//! Atmospheric simulation subsystem.
//!
//! Models pressure layers, humidity fields, temperature profiles, 3-D wind vector grids,
//! jet streams, fog density, visibility calculation, and barometric gradients.

use std::collections::HashMap;
use super::{Vec3, lerp, smoothstep, fbm_2d, value_noise_2d};

// ── Constants ────────────────────────────────────────────────────────────────

/// ISA sea-level pressure in Pascals.
pub const ISA_PRESSURE_PA: f32 = 101_325.0;
/// ISA sea-level temperature in Kelvin.
pub const ISA_TEMP_K: f32 = 288.15;
/// Lapse rate K/m (ISA troposphere).
pub const LAPSE_RATE: f32 = 0.006_5;
/// Specific gas constant for dry air.
pub const R_DRY: f32 = 287.058;
/// Gravity m/s².
pub const GRAVITY: f32 = 9.807;
/// Troposphere top altitude (m).
pub const TROPOPAUSE_ALT: f32 = 11_000.0;
/// Stratosphere temperature (K) above tropopause.
pub const STRATO_TEMP_K: f32 = 216.65;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for the atmospheric simulator.
#[derive(Debug, Clone)]
pub struct AtmosphereConfig {
    /// Number of vertical layers to simulate.
    pub layer_count: usize,
    /// Maximum altitude of the simulation (metres).
    pub max_altitude_m: f32,
    /// Horizontal grid resolution in world units.
    pub grid_resolution: f32,
    /// Grid width (cells in X).
    pub grid_width: usize,
    /// Grid depth (cells in Z).
    pub grid_depth: usize,
    /// Turbulence strength multiplier.
    pub turbulence_scale: f32,
    /// Enable large-scale jet-stream simulation.
    pub enable_jet_streams: bool,
    /// Fog scattering coefficient (higher = denser fog).
    pub fog_scatter_coeff: f32,
}

impl Default for AtmosphereConfig {
    fn default() -> Self {
        Self {
            layer_count: 8,
            max_altitude_m: 12_000.0,
            grid_resolution: 200.0,
            grid_width: 32,
            grid_depth: 32,
            turbulence_scale: 0.4,
            enable_jet_streams: true,
            fog_scatter_coeff: 0.02,
        }
    }
}

// ── Pressure ──────────────────────────────────────────────────────────────────

/// A barometric pressure cell covering a horizontal region.
#[derive(Debug, Clone)]
pub struct PressureCell {
    /// Centre of the cell in world-space (x, z).
    pub centre: [f32; 2],
    /// Radius of influence in metres.
    pub radius: f32,
    /// Pressure at the centre in Pascals.
    pub pressure_pa: f32,
    /// Type of system.
    pub kind: PressureCellKind,
    /// Drift velocity [m/s] in world-space (x, z).
    pub drift: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureCellKind {
    HighPressure,
    LowPressure,
    Neutral,
}

impl PressureCell {
    pub fn new_high(cx: f32, cz: f32, radius: f32) -> Self {
        Self {
            centre: [cx, cz],
            radius,
            pressure_pa: ISA_PRESSURE_PA + 2_000.0,
            kind: PressureCellKind::HighPressure,
            drift: [1.5, 0.5],
        }
    }

    pub fn new_low(cx: f32, cz: f32, radius: f32) -> Self {
        Self {
            centre: [cx, cz],
            radius,
            pressure_pa: ISA_PRESSURE_PA - 3_000.0,
            kind: PressureCellKind::LowPressure,
            drift: [2.0, -0.3],
        }
    }

    /// Sample pressure contribution at world pos (x, z).
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= self.radius { return ISA_PRESSURE_PA; }
        let t = smoothstep(self.radius, 0.0, dist);
        lerp(ISA_PRESSURE_PA, self.pressure_pa, t)
    }

    /// Advance cell position by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.centre[0] += self.drift[0] * dt;
        self.centre[1] += self.drift[1] * dt;
    }
}

/// Barometric pressure gradient — the spatial derivative of pressure.
#[derive(Debug, Clone, Copy)]
pub struct BarometricGradient {
    /// Gradient vector [Pa/m] in (x, z).
    pub grad_x: f32,
    pub grad_z: f32,
}

impl BarometricGradient {
    pub fn magnitude(&self) -> f32 {
        (self.grad_x * self.grad_x + self.grad_z * self.grad_z).sqrt()
    }

    /// Derive geostrophic wind from gradient (simplified, f=Coriolis param).
    pub fn geostrophic_wind(&self, air_density: f32, coriolis: f32) -> Vec3 {
        // Geostrophic: V = (1/(ρ·f)) × ∇p rotated 90°
        let factor = 1.0 / (air_density * coriolis.max(1e-5));
        Vec3::new(
             self.grad_z * factor,
            0.0,
            -self.grad_x * factor,
        )
    }
}

// ── Humidity ──────────────────────────────────────────────────────────────────

/// A 2-D humidity map over the simulation grid.
#[derive(Debug, Clone)]
pub struct HumidityMap {
    pub width: usize,
    pub depth: usize,
    /// Relative humidity [0.0, 1.0] stored row-major (z * width + x).
    pub data: Vec<f32>,
}

impl HumidityMap {
    pub fn new(width: usize, depth: usize, base: f32) -> Self {
        Self {
            width,
            depth,
            data: vec![base; width * depth],
        }
    }

    /// Sample humidity at continuous grid coords using bilinear interpolation.
    pub fn sample(&self, gx: f32, gz: f32) -> f32 {
        let x0 = (gx.floor() as usize).min(self.width.saturating_sub(2));
        let z0 = (gz.floor() as usize).min(self.depth.saturating_sub(2));
        let fx = gx - gx.floor();
        let fz = gz - gz.floor();
        let v00 = self.data[z0       * self.width + x0    ];
        let v10 = self.data[z0       * self.width + x0 + 1];
        let v01 = self.data[(z0 + 1) * self.width + x0    ];
        let v11 = self.data[(z0 + 1) * self.width + x0 + 1];
        lerp(lerp(v00, v10, fx), lerp(v01, v11, fx), fz)
    }

    /// Set humidity at integer grid cell.
    pub fn set(&mut self, gx: usize, gz: usize, val: f32) {
        if gx < self.width && gz < self.depth {
            self.data[gz * self.width + gx] = val.clamp(0.0, 1.0);
        }
    }

    /// Advect humidity by a uniform wind (dx, dz) in grid cells/step.
    pub fn advect(&mut self, dx: f32, dz: f32) {
        let old = self.data.clone();
        for z in 0..self.depth {
            for x in 0..self.width {
                let src_x = x as f32 - dx;
                let src_z = z as f32 - dz;
                let clamped_x = src_x.clamp(0.0, (self.width  - 1) as f32);
                let clamped_z = src_z.clamp(0.0, (self.depth  - 1) as f32);
                let x0 = clamped_x.floor() as usize;
                let z0 = clamped_z.floor() as usize;
                let x1 = (x0 + 1).min(self.width  - 1);
                let z1 = (z0 + 1).min(self.depth  - 1);
                let fx = clamped_x - clamped_x.floor();
                let fz = clamped_z - clamped_z.floor();
                let v = lerp(
                    lerp(old[z0 * self.width + x0], old[z0 * self.width + x1], fx),
                    lerp(old[z1 * self.width + x0], old[z1 * self.width + x1], fx),
                    fz,
                );
                self.data[z * self.width + x] = v.clamp(0.0, 1.0);
            }
        }
    }

    /// Evaporation — increase humidity by `rate` everywhere (capped at 1.0).
    pub fn evaporate(&mut self, rate: f32) {
        for v in &mut self.data {
            *v = (*v + rate).min(1.0);
        }
    }

    /// Precipitation sink — decrease humidity proportional to excess above `sat`.
    pub fn precipitate(&mut self, sat: f32, coeff: f32) {
        for v in &mut self.data {
            if *v > sat {
                *v -= (*v - sat) * coeff;
            }
        }
    }
}

// ── Temperature Profile ───────────────────────────────────────────────────────

/// Vertical temperature profile — one value per simulation layer.
#[derive(Debug, Clone)]
pub struct TemperatureProfile {
    /// Altitude of each layer's base (m).
    pub altitudes: Vec<f32>,
    /// Temperature (K) at each layer.
    pub temps_k: Vec<f32>,
}

impl TemperatureProfile {
    /// Build a standard ISA-based profile for `layer_count` layers up to `max_alt_m`.
    pub fn isa(layer_count: usize, max_alt_m: f32, surface_temp_k: f32) -> Self {
        let mut altitudes = Vec::with_capacity(layer_count);
        let mut temps_k   = Vec::with_capacity(layer_count);
        for i in 0..layer_count {
            let alt = i as f32 * max_alt_m / (layer_count - 1).max(1) as f32;
            let t = if alt <= TROPOPAUSE_ALT {
                surface_temp_k - LAPSE_RATE * alt
            } else {
                STRATO_TEMP_K
            };
            altitudes.push(alt);
            temps_k.push(t);
        }
        Self { altitudes, temps_k }
    }

    /// Sample temperature at arbitrary altitude via linear interpolation.
    pub fn sample_at(&self, alt_m: f32) -> f32 {
        if alt_m <= self.altitudes[0] { return self.temps_k[0]; }
        let last = self.altitudes.len() - 1;
        if alt_m >= self.altitudes[last] { return self.temps_k[last]; }
        for i in 0..last {
            if alt_m >= self.altitudes[i] && alt_m <= self.altitudes[i + 1] {
                let t = (alt_m - self.altitudes[i])
                    / (self.altitudes[i + 1] - self.altitudes[i]);
                return lerp(self.temps_k[i], self.temps_k[i + 1], t);
            }
        }
        self.temps_k[last]
    }

    /// Compute air density (kg/m³) at a given altitude using the ideal gas law.
    pub fn density_at(&self, alt_m: f32) -> f32 {
        let temp = self.sample_at(alt_m);
        let pressure = isa_pressure_at_altitude(alt_m);
        pressure / (R_DRY * temp)
    }

    /// Update surface temperature, scaling the whole profile accordingly.
    pub fn set_surface_temp(&mut self, new_temp_k: f32) {
        if self.temps_k.is_empty() { return; }
        let old_surface = self.temps_k[0];
        let delta = new_temp_k - old_surface;
        // Surface perturbation decays exponentially with altitude
        for (i, t) in self.temps_k.iter_mut().enumerate() {
            let alt = self.altitudes[i];
            let decay = (-alt / 3_000.0_f32).exp();
            *t += delta * decay;
        }
    }
}

/// ISA pressure at altitude using barometric formula.
pub fn isa_pressure_at_altitude(alt_m: f32) -> f32 {
    if alt_m <= TROPOPAUSE_ALT {
        let base_ratio = 1.0 - LAPSE_RATE * alt_m / ISA_TEMP_K;
        ISA_PRESSURE_PA * base_ratio.powf(GRAVITY / (LAPSE_RATE * R_DRY))
    } else {
        let p_tropo = isa_pressure_at_altitude(TROPOPAUSE_ALT);
        let delta_alt = alt_m - TROPOPAUSE_ALT;
        p_tropo * (-(GRAVITY * delta_alt) / (R_DRY * STRATO_TEMP_K)).exp()
    }
}

// ── Atmospheric Layers ────────────────────────────────────────────────────────

/// A single simulated atmospheric layer.
#[derive(Debug, Clone)]
pub struct AtmosphericLayer {
    pub altitude_base_m: f32,
    pub altitude_top_m:  f32,
    pub pressure_pa:     f32,
    pub temperature_k:   f32,
    pub density_kg_m3:   f32,
    pub humidity:        f32,   // relative humidity 0–1
    pub cloud_fraction:  f32,   // 0–1 coverage
    pub wind:            Vec3,
}

impl AtmosphericLayer {
    pub fn from_isa(alt_base: f32, alt_top: f32, surface_temp_k: f32, humidity: f32) -> Self {
        let mid = (alt_base + alt_top) * 0.5;
        let temp_k = if mid <= TROPOPAUSE_ALT {
            surface_temp_k - LAPSE_RATE * mid
        } else {
            STRATO_TEMP_K
        };
        let pressure = isa_pressure_at_altitude(mid);
        let density  = pressure / (R_DRY * temp_k.max(1.0));
        Self {
            altitude_base_m: alt_base,
            altitude_top_m:  alt_top,
            pressure_pa:     pressure,
            temperature_k:   temp_k,
            density_kg_m3:   density,
            humidity,
            cloud_fraction:  0.0,
            wind:            Vec3::ZERO,
        }
    }

    pub fn thickness(&self) -> f32 {
        self.altitude_top_m - self.altitude_base_m
    }

    /// Compute dew point (K) using Magnus approximation.
    pub fn dew_point_k(&self) -> f32 {
        let tc = self.temperature_k - 273.15;
        let rh = self.humidity.clamp(0.001, 1.0);
        let a = 17.625_f32;
        let b = 243.04_f32;
        let alpha = (a * tc / (b + tc)) + rh.ln();
        let dp_c = b * alpha / (a - alpha);
        dp_c + 273.15
    }

    /// True if temperature is at or below dew point (condensation occurs).
    pub fn is_saturated(&self) -> bool {
        self.temperature_k <= self.dew_point_k() + 0.5
    }

    /// Cloud likelihood based on humidity and instability.
    pub fn update_cloud_fraction(&mut self) {
        if self.is_saturated() {
            self.cloud_fraction = (self.cloud_fraction + 0.1).min(1.0);
        } else {
            self.cloud_fraction = (self.cloud_fraction - 0.05).max(0.0);
        }
    }
}

// ── Wind Field ────────────────────────────────────────────────────────────────

/// A single wind sample.
#[derive(Debug, Clone, Copy)]
pub struct WindVector {
    pub velocity: Vec3,  // m/s in world space
    pub turbulence: f32, // local turbulence intensity 0–1
}

/// 3-D wind field stored as a grid of [WindVector].
#[derive(Debug, Clone)]
pub struct WindField {
    pub grid_w: usize,
    pub grid_h: usize, // vertical layers
    pub grid_d: usize,
    pub resolution: f32,     // world units per cell
    pub layer_height: f32,   // altitude per vertical cell
    pub data: Vec<WindVector>,
}

impl WindField {
    pub fn new(w: usize, h: usize, d: usize, resolution: f32, layer_height: f32) -> Self {
        let default_wv = WindVector { velocity: Vec3::ZERO, turbulence: 0.0 };
        Self {
            grid_w: w,
            grid_h: h,
            grid_d: d,
            resolution,
            layer_height,
            data: vec![default_wv; w * h * d],
        }
    }

    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        (y * self.grid_d + z) * self.grid_w + x
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, wv: WindVector) {
        if x < self.grid_w && y < self.grid_h && z < self.grid_d {
            let i = self.index(x, y, z);
            self.data[i] = wv;
        }
    }

    /// Sample wind at world-space position using trilinear interpolation.
    pub fn sample_world(&self, wx: f32, wy: f32, wz: f32) -> WindVector {
        let gx = wx / self.resolution;
        let gy = wy / self.layer_height;
        let gz = wz / self.resolution;

        let x0 = (gx.floor() as usize).min(self.grid_w.saturating_sub(2));
        let y0 = (gy.floor() as usize).min(self.grid_h.saturating_sub(2));
        let z0 = (gz.floor() as usize).min(self.grid_d.saturating_sub(2));
        let x1 = (x0 + 1).min(self.grid_w - 1);
        let y1 = (y0 + 1).min(self.grid_h - 1);
        let z1 = (z0 + 1).min(self.grid_d - 1);

        let fx = gx - gx.floor();
        let fy = gy - gy.floor();
        let fz = gz - gz.floor();

        let lerp_wv = |a: WindVector, b: WindVector, t: f32| -> WindVector {
            WindVector {
                velocity:   a.velocity.lerp(b.velocity, t),
                turbulence: lerp(a.turbulence, b.turbulence, t),
            }
        };

        let c000 = self.data[self.index(x0, y0, z0)];
        let c100 = self.data[self.index(x1, y0, z0)];
        let c010 = self.data[self.index(x0, y1, z0)];
        let c110 = self.data[self.index(x1, y1, z0)];
        let c001 = self.data[self.index(x0, y0, z1)];
        let c101 = self.data[self.index(x1, y0, z1)];
        let c011 = self.data[self.index(x0, y1, z1)];
        let c111 = self.data[self.index(x1, y1, z1)];

        let c00 = lerp_wv(c000, c100, fx);
        let c10 = lerp_wv(c010, c110, fx);
        let c01 = lerp_wv(c001, c101, fx);
        let c11 = lerp_wv(c011, c111, fx);

        let c0 = lerp_wv(c00, c10, fy);
        let c1 = lerp_wv(c01, c11, fy);

        lerp_wv(c0, c1, fz)
    }

    /// Fill the entire field with a base directional wind that varies by layer.
    pub fn fill_from_layers(&mut self, layers: &[AtmosphericLayer]) {
        for y in 0..self.grid_h {
            let alt = y as f32 * self.layer_height;
            // Find the matching layer
            let layer_wind = layers.iter()
                .find(|l| alt >= l.altitude_base_m && alt < l.altitude_top_m)
                .map(|l| l.wind)
                .unwrap_or(Vec3::ZERO);

            for z in 0..self.grid_d {
                for x in 0..self.grid_w {
                    let turb_noise = fbm_2d(
                        x as f32 * 0.3 + alt * 0.001,
                        z as f32 * 0.3,
                        3,
                    ) * 0.5 - 0.25;
                    let turb_vec = Vec3::new(turb_noise, 0.0, turb_noise * 0.7);
                    let wv = WindVector {
                        velocity:   layer_wind.add(turb_vec),
                        turbulence: turb_noise.abs() * 2.0,
                    };
                    self.set(x, y, z, wv);
                }
            }
        }
    }
}

// ── Jet Stream ────────────────────────────────────────────────────────────────

/// A jet stream — a narrow band of fast upper-level wind.
#[derive(Debug, Clone)]
pub struct JetStream {
    /// Core altitude (m).
    pub altitude_m: f32,
    /// Core wind speed (m/s).
    pub core_speed: f32,
    /// Direction (radians from +X axis).
    pub direction: f32,
    /// Vertical half-width (m) — Gaussian roll-off.
    pub vertical_width_m: f32,
    /// Horizontal sinusoidal amplitude for Rossby wave undulation.
    pub wave_amplitude: f32,
    /// Rossby wave number (radians per metre along the stream).
    pub wave_number: f32,
    /// Phase offset (radians).
    pub wave_phase: f32,
    /// Phase advance speed (radians/second).
    pub wave_phase_speed: f32,
}

impl JetStream {
    pub fn polar(northern_hemisphere: bool) -> Self {
        Self {
            altitude_m:        10_000.0,
            core_speed:        if northern_hemisphere { 60.0 } else { 55.0 },
            direction:         if northern_hemisphere { 0.0 } else { std::f32::consts::PI },
            vertical_width_m:  2_000.0,
            wave_amplitude:    400_000.0,
            wave_number:       1.0 / 6_000_000.0,
            wave_phase:        0.0,
            wave_phase_speed:  1e-5,
        }
    }

    pub fn subtropical() -> Self {
        Self {
            altitude_m:        12_000.0,
            core_speed:        45.0,
            direction:         0.1,
            vertical_width_m:  1_500.0,
            wave_amplitude:    200_000.0,
            wave_number:       1.0 / 4_000_000.0,
            wave_phase:        1.0,
            wave_phase_speed:  8e-6,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.wave_phase += self.wave_phase_speed * dt;
    }

    /// Sample jet-stream wind contribution at world pos (wx, wy, wz).
    pub fn sample(&self, wx: f32, wy: f32, wz: f32) -> Vec3 {
        // Vertical Gaussian roll-off from core altitude
        let dalt = wy - self.altitude_m;
        let vert_factor = (-0.5 * (dalt / self.vertical_width_m).powi(2)).exp();
        if vert_factor < 1e-4 { return Vec3::ZERO; }

        // Rossby wave undulation shifts the stream laterally
        let undulation = (wx * self.wave_number + self.wave_phase).sin() * self.wave_amplitude;
        let _effective_z = wz - undulation;

        let speed = self.core_speed * vert_factor;
        Vec3::new(
            speed * self.direction.cos(),
            0.0,
            speed * self.direction.sin(),
        )
    }
}

// ── Fog ───────────────────────────────────────────────────────────────────────

/// A fog layer — horizontally uniform, vertically parameterised.
#[derive(Debug, Clone)]
pub struct FogLayer {
    /// Altitude of fog base (m).
    pub base_m: f32,
    /// Altitude of fog top (m).
    pub top_m: f32,
    /// Peak extinction coefficient (m⁻¹).
    pub extinction: f32,
    /// Fog type.
    pub kind: FogKind,
    /// Current visibility through the peak (metres).
    pub visibility_m: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogKind {
    Radiation,
    Advection,
    Upslope,
    Freezing,
}

impl FogLayer {
    pub fn radiation(base: f32, top: f32, extinction: f32) -> Self {
        let vis = if extinction > 0.0 { 3.912 / extinction } else { 50_000.0 };
        Self { base_m: base, top_m: top, extinction, kind: FogKind::Radiation, visibility_m: vis }
    }

    pub fn advection(base: f32, top: f32, extinction: f32) -> Self {
        let vis = if extinction > 0.0 { 3.912 / extinction } else { 50_000.0 };
        Self { base_m: base, top_m: top, extinction, kind: FogKind::Advection, visibility_m: vis }
    }

    /// Sample extinction at altitude `y`.
    pub fn extinction_at(&self, y: f32) -> f32 {
        if y < self.base_m || y > self.top_m { return 0.0; }
        let t = (y - self.base_m) / (self.top_m - self.base_m).max(0.1);
        // Bell-shaped profile peaking in the middle
        let bell = smoothstep(0.0, 0.5, t) * smoothstep(1.0, 0.5, t) * 2.0;
        self.extinction * bell
    }

    /// Dissipate fog over time.
    pub fn dissipate(&mut self, rate: f32) {
        self.extinction = (self.extinction - rate).max(0.0);
        self.visibility_m = if self.extinction > 0.0 { 3.912 / self.extinction } else { 50_000.0 };
    }

    /// Intensify fog.
    pub fn intensify(&mut self, rate: f32) {
        self.extinction = (self.extinction + rate).min(0.5);
        self.visibility_m = if self.extinction > 0.0 { 3.912 / self.extinction } else { 50_000.0 };
    }
}

// ── Visibility ────────────────────────────────────────────────────────────────

/// Result of a visibility computation.
#[derive(Debug, Clone, Copy)]
pub struct VisibilityResult {
    /// Meteorological visibility (distance at which contrast = 0.05).
    pub distance_m: f32,
    /// Optical depth along the line of sight.
    pub optical_depth: f32,
    /// Dominant limiting factor.
    pub limiting_factor: VisibilityLimiter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityLimiter {
    Clear,
    Fog,
    Precipitation,
    Haze,
    Smoke,
    Dust,
}

// ── Main Atmosphere Struct ────────────────────────────────────────────────────

/// The primary atmospheric simulation state.
#[derive(Debug, Clone)]
pub struct Atmosphere {
    pub config: AtmosphereConfig,
    pub layers: Vec<AtmosphericLayer>,
    pub temperature_profile: TemperatureProfile,
    pub humidity_map: HumidityMap,
    pub wind_field: WindField,
    pub pressure_cells: Vec<PressureCell>,
    pub jet_streams: Vec<JetStream>,
    pub fog_layers: Vec<FogLayer>,
    pub surface_altitude_m: f32,
    /// Background aerosol extinction (haze), m⁻¹.
    pub haze_extinction: f32,
    /// Time accumulator for slow dynamics.
    time_accum: f32,
    /// Noise offset drifting over time for turbulence variety.
    noise_offset: f32,
}

impl Atmosphere {
    pub fn new(surface_altitude_m: f32) -> Self {
        Self::with_config(surface_altitude_m, AtmosphereConfig::default())
    }

    pub fn with_config(surface_altitude_m: f32, config: AtmosphereConfig) -> Self {
        let n = config.layer_count;
        let max_alt = config.max_altitude_m;
        let surface_temp_k = ISA_TEMP_K;

        // Build vertical layers
        let layers: Vec<AtmosphericLayer> = (0..n).map(|i| {
            let base = i as f32 * max_alt / n as f32;
            let top  = (i + 1) as f32 * max_alt / n as f32;
            AtmosphericLayer::from_isa(base, top, surface_temp_k, 0.5)
        }).collect();

        let temp_profile = TemperatureProfile::isa(n, max_alt, surface_temp_k);

        let humidity_map = HumidityMap::new(
            config.grid_width,
            config.grid_depth,
            0.55,
        );

        let layer_height = max_alt / n as f32;
        let mut wind_field = WindField::new(
            config.grid_width,
            n,
            config.grid_depth,
            config.grid_resolution,
            layer_height,
        );
        wind_field.fill_from_layers(&layers);

        let pressure_cells = vec![
            PressureCell::new_high(0.0, 0.0, 500_000.0),
            PressureCell::new_low(200_000.0, 100_000.0, 300_000.0),
        ];

        let jet_streams = if config.enable_jet_streams {
            vec![JetStream::polar(true), JetStream::subtropical()]
        } else {
            vec![]
        };

        Self {
            config,
            layers,
            temperature_profile: temp_profile,
            humidity_map,
            wind_field,
            pressure_cells,
            jet_streams,
            fog_layers: vec![],
            surface_altitude_m,
            haze_extinction: 0.002,
            time_accum: 0.0,
            noise_offset: 0.0,
        }
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    pub fn tick(&mut self, dt: f32, surface_temp_k: f32, time_of_day: f32) {
        self.time_accum  += dt;
        self.noise_offset += dt * 0.003;

        // Update temperature profile
        self.temperature_profile.set_surface_temp(surface_temp_k);

        // Sync layer temperatures
        for layer in &mut self.layers {
            let mid = (layer.altitude_base_m + layer.altitude_top_m) * 0.5;
            layer.temperature_k = self.temperature_profile.sample_at(mid);
            layer.pressure_pa   = isa_pressure_at_altitude(mid);
            layer.density_kg_m3 = layer.pressure_pa / (R_DRY * layer.temperature_k.max(1.0));
            layer.update_cloud_fraction();
        }

        // Advance pressure cells
        for cell in &mut self.pressure_cells {
            cell.tick(dt);
        }

        // Advance jet streams
        for js in &mut self.jet_streams {
            js.tick(dt);
        }

        // Wind: build layer winds from pressure gradient + jet stream
        self.rebuild_layer_winds(time_of_day);

        // Refill wind field every few seconds
        if self.time_accum >= 5.0 {
            self.time_accum = 0.0;
            let layers_snapshot = self.layers.clone();
            self.wind_field.fill_from_layers(&layers_snapshot);
        }

        // Humidity advection — use surface wind
        let surf_wind = self.surface_wind();
        let adv_scale = dt / self.config.grid_resolution;
        self.humidity_map.advect(surf_wind.x * adv_scale, surf_wind.z * adv_scale);

        // Evaporation from surface (stronger in daytime)
        let day_factor = smoothstep(6.0, 12.0, time_of_day) * smoothstep(20.0, 15.0, time_of_day);
        self.humidity_map.evaporate(0.00002 * day_factor * dt);

        // Precipitation sink where humidity is high
        self.humidity_map.precipitate(0.85, 0.001 * dt);

        // Fog lifecycle
        self.update_fog(surface_temp_k, dt);
    }

    fn rebuild_layer_winds(&mut self, _time_of_day: f32) {
        let sample_x = 0.0_f32;
        let sample_z = 0.0_f32;

        // Compute pressure gradient at origin
        let dp_dx = self.pressure_gradient_x(sample_x, sample_z);
        let dp_dz = self.pressure_gradient_z(sample_x, sample_z);
        let grad = BarometricGradient { grad_x: dp_dx, grad_z: dp_dz };

        for layer in &mut self.layers {
            let alt = (layer.altitude_base_m + layer.altitude_top_m) * 0.5;
            let density = layer.density_kg_m3.max(0.01);
            // Simplified Coriolis parameter (mid-latitude)
            let f_cor = 1e-4_f32;
            let geo_wind = grad.geostrophic_wind(density, f_cor);

            // Add turbulence noise
            let t_scale = alt * 0.0001 + 0.5;
            let turb = value_noise_2d(t_scale, 0.1) * 2.0 - 1.0;

            layer.wind = Vec3::new(
                geo_wind.x + turb * 1.5,
                0.0,
                geo_wind.z + turb * 0.8,
            );
        }

        // Add jet-stream contribution to upper layers
        for layer in &mut self.layers {
            let alt = (layer.altitude_base_m + layer.altitude_top_m) * 0.5;
            let mut js_contrib = Vec3::ZERO;
            for js in &self.jet_streams {
                js_contrib = js_contrib.add(js.sample(sample_x, alt, sample_z));
            }
            layer.wind = layer.wind.add(js_contrib);
        }
    }

    fn pressure_gradient_x(&self, x: f32, z: f32) -> f32 {
        let dx = 1_000.0_f32;
        let p1 = self.total_pressure_at(x + dx, z);
        let p0 = self.total_pressure_at(x - dx, z);
        (p1 - p0) / (2.0 * dx)
    }

    fn pressure_gradient_z(&self, x: f32, z: f32) -> f32 {
        let dz = 1_000.0_f32;
        let p1 = self.total_pressure_at(x, z + dz);
        let p0 = self.total_pressure_at(x, z - dz);
        (p1 - p0) / (2.0 * dz)
    }

    fn total_pressure_at(&self, x: f32, z: f32) -> f32 {
        let mut p = ISA_PRESSURE_PA;
        for cell in &self.pressure_cells {
            let sample = cell.sample(x, z);
            p += sample - ISA_PRESSURE_PA;
        }
        // Add noise-based mesoscale variation
        p += (fbm_2d(x * 0.000_01, z * 0.000_01, 3) - 0.5) * 400.0;
        p
    }

    fn update_fog(&mut self, surface_temp_k: f32, dt: f32) {
        let surface_humidity = self.surface_humidity();
        let dew_point = {
            let tc = surface_temp_k - 273.15;
            let rh = surface_humidity.clamp(0.001, 1.0);
            let a = 17.625_f32;
            let b = 243.04_f32;
            let alpha = (a * tc / (b + tc)) + rh.ln();
            b * alpha / (a - alpha) + 273.15
        };

        // Radiation fog forms when surface temp ≈ dew point at night
        let temp_spread = surface_temp_k - dew_point;
        if temp_spread < 2.5 && surface_humidity > 0.8 {
            if self.fog_layers.is_empty() {
                self.fog_layers.push(FogLayer::radiation(0.0, 150.0, 0.01));
            } else {
                for fog in &mut self.fog_layers {
                    fog.intensify(0.0005 * dt);
                }
            }
        } else {
            for fog in &mut self.fog_layers {
                fog.dissipate(0.0003 * dt);
            }
            self.fog_layers.retain(|f| f.extinction > 1e-5);
        }
    }

    // ── Public Query Methods ──────────────────────────────────────────────────

    /// Return surface (layer 0) relative humidity [0,1].
    pub fn surface_humidity(&self) -> f32 {
        self.humidity_map.sample(
            (self.config.grid_width / 2) as f32,
            (self.config.grid_depth / 2) as f32,
        )
    }

    /// Return surface pressure (Pa).
    pub fn surface_pressure(&self) -> f32 {
        self.total_pressure_at(0.0, 0.0)
    }

    /// Return surface wind velocity (m/s).
    pub fn surface_wind(&self) -> Vec3 {
        if self.layers.is_empty() { return Vec3::ZERO; }
        self.layers[0].wind
    }

    /// Sample wind at world-space position.
    pub fn wind_at(&self, wx: f32, wy: f32, wz: f32) -> [f32; 3] {
        let base_wv = self.wind_field.sample_world(wx, wy, wz);
        let mut wind = base_wv.velocity;
        // Add jet stream
        for js in &self.jet_streams {
            wind = wind.add(js.sample(wx, wy, wz));
        }
        // Turbulence
        let t = self.noise_offset;
        let turb = Vec3::new(
            (fbm_2d(wx * 0.0005 + t, wz * 0.0005) * 2.0 - 1.0) * self.config.turbulence_scale,
            0.0,
            (fbm_2d(wz * 0.0005 + t + 100.0, wx * 0.0005) * 2.0 - 1.0) * self.config.turbulence_scale * 0.5,
        );
        wind = wind.add(turb);
        [wind.x, wind.y, wind.z]
    }

    /// Compute meteorological visibility from the surface.
    pub fn compute_visibility(&self) -> VisibilityResult {
        // Total extinction coefficient = haze + fog
        let mut ext = self.haze_extinction;
        let mut limiter = VisibilityLimiter::Haze;

        for fog in &self.fog_layers {
            let fog_ext = fog.extinction_at(self.surface_altitude_m + 1.5); // observer height
            if fog_ext > ext {
                ext = fog_ext;
                limiter = VisibilityLimiter::Fog;
            }
        }

        if ext < 1e-8 {
            return VisibilityResult {
                distance_m: 50_000.0,
                optical_depth: 0.0,
                limiting_factor: VisibilityLimiter::Clear,
            };
        }

        // Koschmieder's law: V = 3.912 / σ
        let dist = (3.912 / ext).min(50_000.0);
        let opt_depth = ext * dist;
        VisibilityResult { distance_m: dist, optical_depth: opt_depth, limiting_factor: limiter }
    }

    /// Return the cloud fraction at a given altitude.
    pub fn cloud_fraction_at(&self, alt_m: f32) -> f32 {
        for layer in &self.layers {
            if alt_m >= layer.altitude_base_m && alt_m < layer.altitude_top_m {
                return layer.cloud_fraction;
            }
        }
        0.0
    }

    /// Integrate optical depth between two altitudes (for sky rendering).
    pub fn optical_depth_between(&self, alt_low: f32, alt_high: f32) -> f32 {
        let steps = 8usize;
        let dh = (alt_high - alt_low) / steps as f32;
        let mut od = 0.0_f32;
        for i in 0..steps {
            let h = alt_low + (i as f32 + 0.5) * dh;
            let cloud = self.cloud_fraction_at(h) * 0.05;
            let haze = self.haze_extinction * (-h / 8_500.0_f32).exp();
            let fog_ext: f32 = self.fog_layers.iter().map(|f| f.extinction_at(h)).sum();
            od += (cloud + haze + fog_ext) * dh;
        }
        od
    }

    /// Return temperature (K) at world altitude.
    pub fn temperature_at(&self, alt_m: f32) -> f32 {
        self.temperature_profile.sample_at(alt_m + self.surface_altitude_m)
    }
}

// ── AtmosphereSimulator ───────────────────────────────────────────────────────

/// Higher-level simulator that manages an Atmosphere and exposes query helpers.
#[derive(Debug, Clone)]
pub struct AtmosphereSimulator {
    pub atmo: Atmosphere,
    /// Cached per-frame visibility result.
    pub last_visibility: VisibilityResult,
    /// Accumulated precipitation potential from humidity excess.
    pub precip_potential: f32,
    /// Named weather stations for point queries.
    stations: HashMap<String, [f32; 3]>,
}

impl AtmosphereSimulator {
    pub fn new(surface_alt: f32) -> Self {
        let atmo = Atmosphere::new(surface_alt);
        let vis  = atmo.compute_visibility();
        Self {
            atmo,
            last_visibility: vis,
            precip_potential: 0.0,
            stations: HashMap::new(),
        }
    }

    pub fn tick(&mut self, dt: f32, surface_temp_k: f32, time_of_day: f32) {
        self.atmo.tick(dt, surface_temp_k, time_of_day);
        self.last_visibility = self.atmo.compute_visibility();
        // Accumulate precip potential
        let hum = self.atmo.surface_humidity();
        if hum > 0.85 {
            self.precip_potential = (self.precip_potential + (hum - 0.85) * dt * 0.1).min(1.0);
        } else {
            self.precip_potential = (self.precip_potential - dt * 0.005).max(0.0);
        }
    }

    /// Register a named weather station.
    pub fn add_station(&mut self, name: impl Into<String>, x: f32, y: f32, z: f32) {
        self.stations.insert(name.into(), [x, y, z]);
    }

    /// Query all weather parameters at a named station.
    pub fn station_report(&self, name: &str) -> Option<StationReport> {
        let pos = self.stations.get(name)?;
        let [x, y, z] = *pos;
        let wind = self.atmo.wind_at(x, y, z);
        let temp_k = self.atmo.temperature_at(y);
        let pressure = isa_pressure_at_altitude(y + self.atmo.surface_altitude_m);
        let vis = self.last_visibility.distance_m;
        let cloud = self.atmo.cloud_fraction_at(y);
        Some(StationReport { x, y, z, wind, temp_k, pressure_pa: pressure, visibility_m: vis, cloud_fraction: cloud })
    }

    /// Return true if conditions favour precipitation.
    pub fn precipitation_likely(&self) -> bool {
        self.precip_potential > 0.3 && self.atmo.surface_humidity() > 0.75
    }
}

/// A weather report at a named station.
#[derive(Debug, Clone, Copy)]
pub struct StationReport {
    pub x: f32, pub y: f32, pub z: f32,
    pub wind: [f32; 3],
    pub temp_k: f32,
    pub pressure_pa: f32,
    pub visibility_m: f32,
    pub cloud_fraction: f32,
}

impl StationReport {
    pub fn temp_celsius(&self) -> f32 { self.temp_k - 273.15 }
    pub fn wind_speed_ms(&self) -> f32 {
        let [wx, wy, wz] = self.wind;
        (wx * wx + wy * wy + wz * wz).sqrt()
    }
    pub fn beaufort_number(&self) -> u8 {
        let spd = self.wind_speed_ms();
        match spd as u32 {
            0     => 0,
            1..=5  => 1,
            6..=11 => 2,
            12..=19 => 3,
            20..=28 => 4,
            29..=38 => 5,
            39..=49 => 6,
            50..=61 => 7,
            62..=74 => 8,
            75..=88 => 9,
            89..=102 => 10,
            103..=117 => 11,
            _ => 12,
        }
    }
}
