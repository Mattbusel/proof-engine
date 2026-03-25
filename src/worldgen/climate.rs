//! Climate simulation — atmospheric circulation driven by heat equation PDE.
//!
//! Simulates temperature distribution (latitude + altitude + ocean currents)
//! and precipitation (orographic rainfall, moisture transport).

use super::{Grid2D, Rng};
use std::f32::consts::PI;

/// Climate parameters.
#[derive(Debug, Clone)]
pub struct ClimateParams {
    /// Base equatorial temperature (°C).
    pub equator_temp: f32,
    /// Polar temperature (°C).
    pub polar_temp: f32,
    /// Temperature lapse rate per unit elevation (°C per elevation unit).
    pub lapse_rate: f32,
    /// Ocean thermal inertia (moderates coastal temperatures).
    pub ocean_moderation: f32,
    /// Base wind direction (0 = east, PI/2 = north).
    pub prevailing_wind: f32,
    /// Wind speed.
    pub wind_speed: f32,
    /// Moisture pickup rate over ocean.
    pub moisture_rate: f32,
    /// Orographic precipitation factor.
    pub orographic_factor: f32,
    /// Thermal diffusion rate.
    pub diffusion: f32,
    /// Sea level (below this = ocean).
    pub sea_level: f32,
}

impl Default for ClimateParams {
    fn default() -> Self {
        Self {
            equator_temp: 30.0,
            polar_temp: -20.0,
            lapse_rate: 40.0,
            ocean_moderation: 0.3,
            prevailing_wind: 0.0,
            wind_speed: 1.0,
            moisture_rate: 0.05,
            orographic_factor: 3.0,
            diffusion: 0.1,
            sea_level: 0.4,
        }
    }
}

/// Simulate climate and return (temperature, precipitation) grids.
pub fn simulate(heightmap: &Grid2D, iterations: usize, rng: &mut Rng) -> (Grid2D, Grid2D) {
    let params = ClimateParams::default();
    simulate_with_params(heightmap, iterations, &params, rng)
}

/// Simulate with custom parameters.
pub fn simulate_with_params(
    heightmap: &Grid2D,
    iterations: usize,
    params: &ClimateParams,
    _rng: &mut Rng,
) -> (Grid2D, Grid2D) {
    let w = heightmap.width;
    let h = heightmap.height;

    // 1. Base temperature from latitude + altitude
    let mut temperature = Grid2D::new(w, h);
    for y in 0..h {
        let latitude = (y as f32 / h as f32 - 0.5).abs() * 2.0; // 0 at equator, 1 at poles
        let lat_temp = params.equator_temp + (params.polar_temp - params.equator_temp) * latitude;

        for x in 0..w {
            let elevation = heightmap.get(x, y);
            let alt_offset = if elevation > params.sea_level {
                -(elevation - params.sea_level) * params.lapse_rate
            } else {
                0.0
            };

            // Ocean cells moderate toward ocean temp
            let is_ocean = elevation < params.sea_level;
            let base = lat_temp + alt_offset;
            let temp = if is_ocean {
                base * (1.0 - params.ocean_moderation) + lat_temp * params.ocean_moderation
            } else {
                base
            };

            temperature.set(x, y, temp);
        }
    }

    // 2. Thermal diffusion (iterate heat equation)
    for _ in 0..iterations {
        diffuse_temperature(&mut temperature, params.diffusion);
    }

    // 3. Precipitation from moisture transport
    let mut precipitation = Grid2D::new(w, h);

    // Wind-driven moisture transport
    let wind_dx = params.prevailing_wind.cos();
    let wind_dy = params.prevailing_wind.sin();

    // Simulate moisture advection from multiple wind directions (Hadley cells)
    for wind_band in 0..3 {
        let band_angle = match wind_band {
            0 => params.prevailing_wind,           // Trade winds
            1 => params.prevailing_wind + PI,       // Westerlies
            _ => params.prevailing_wind + PI * 0.5, // Polar easterlies
        };
        let wdx = band_angle.cos() * params.wind_speed;
        let wdy = band_angle.sin() * params.wind_speed;

        // Band latitude range
        let (y_min, y_max) = match wind_band {
            0 => (h / 3, 2 * h / 3),  // Tropics
            1 => (0, h / 3),           // Mid-latitudes (north)
            _ => (2 * h / 3, h),       // Mid-latitudes (south)
        };

        let mut moisture = Grid2D::new(w, h);

        // Advect moisture
        for step in 0..w.max(h) {
            for y in y_min..y_max {
                for x in 0..w {
                    let sx = (x as f32 - wdx * step as f32).rem_euclid(w as f32) as usize;
                    let sy = (y as f32 - wdy * step as f32).clamp(0.0, h as f32 - 1.0) as usize;

                    let elev = heightmap.get(x, y);
                    let is_ocean = elev < params.sea_level;

                    if is_ocean {
                        // Pick up moisture over ocean
                        moisture.add(x, y, params.moisture_rate);
                    } else {
                        // Orographic precipitation: upslope forces moisture out
                        let (gx, gy) = heightmap.gradient(x, y);
                        let upslope = gx * wdx + gy * wdy;
                        if upslope > 0.0 {
                            let rain = moisture.get(x, y) * upslope * params.orographic_factor;
                            let rain = rain.min(moisture.get(x, y));
                            precipitation.add(x, y, rain);
                            moisture.add(x, y, -rain);
                        }
                    }
                }
            }
        }

        // Base precipitation from remaining moisture
        for y in y_min..y_max {
            for x in 0..w {
                precipitation.add(x, y, moisture.get(x, y) * 0.1);
            }
        }
    }

    // Add latitude-based baseline precipitation (ITCZ at equator)
    for y in 0..h {
        let lat = (y as f32 / h as f32 - 0.5).abs() * 2.0;
        let itcz = (1.0 - lat * 3.0).max(0.0); // High rainfall near equator
        for x in 0..w {
            precipitation.add(x, y, itcz * 0.5);
        }
    }

    // Normalize precipitation to [0, 1]
    precipitation.normalize();

    (temperature, precipitation)
}

/// One step of thermal diffusion (2D heat equation).
fn diffuse_temperature(temp: &mut Grid2D, rate: f32) {
    let w = temp.width;
    let h = temp.height;
    let old = temp.data.clone();

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = y * w + x;
            let laplacian = old[idx - 1] + old[idx + 1]
                + old[idx - w] + old[idx + w]
                - 4.0 * old[idx];
            temp.data[idx] += laplacian * rate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_latitude() {
        let hm = Grid2D::filled(32, 32, 0.5); // flat land
        let mut rng = Rng::new(42);
        let (temp, _) = simulate(&hm, 10, &mut rng);
        // Equator (y=16) should be warmer than poles (y=0 or y=31)
        let equator_t = temp.get(16, 16);
        let pole_t = temp.get(16, 0);
        assert!(equator_t > pole_t, "equator should be warmer: eq={equator_t}, pole={pole_t}");
    }

    #[test]
    fn test_precipitation_exists() {
        let hm = Grid2D::filled(32, 32, 0.3); // mostly ocean
        let mut rng = Rng::new(42);
        let (_, precip) = simulate(&hm, 10, &mut rng);
        let max_p = precip.max_value();
        assert!(max_p > 0.0, "should have some precipitation");
    }
}
