//! Climate zone and seasonal cycle subsystem.
//!
//! Models biome climate zones, seasonal temperature/precipitation cycles,
//! day/night temperature curves, climate interpolation, storm fronts,
//! heat waves, cold snaps, and weather pattern transitions.

use std::collections::HashMap;
use super::{Vec3, lerp, smoothstep, fbm_2d, value_noise_2d};

// ── Season ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Determine season from day-of-year [0,365) and hemisphere.
    pub fn from_day(day: f32, northern: bool) -> Self {
        let d = day.rem_euclid(365.0);
        let raw = if d < 80.0       { Season::Winter }
            else if d < 172.0       { Season::Spring }
            else if d < 264.0       { Season::Summer }
            else if d < 355.0       { Season::Autumn }
            else                    { Season::Winter };
        if northern { raw } else { raw.opposite() }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Spring => Self::Autumn,
            Self::Summer => Self::Winter,
            Self::Autumn => Self::Spring,
            Self::Winter => Self::Summer,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Spring => "Spring",
            Self::Summer => "Summer",
            Self::Autumn => "Autumn",
            Self::Winter => "Winter",
        }
    }

    /// Solar declination factor [-1, 1] for this season (rough).
    pub fn solar_factor(self) -> f32 {
        match self {
            Self::Summer =>  1.0,
            Self::Winter => -1.0,
            Self::Spring =>  0.2,
            Self::Autumn => -0.2,
        }
    }
}

// ── Biome Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeType {
    TropicalRainforest,
    TropicalSavanna,
    HotDesert,
    ColdDesert,
    MediterraneanShrubland,
    TemperateGrassland,
    TemperateDeciduousForest,
    TemperateConiferousForest,
    BorealForest,       // Taiga
    Tundra,
    PolarIce,
    MontaneAlpine,
    CoastalMarine,
    Wetland,
    UrbanHeatIsland,
}

impl BiomeType {
    /// Mean annual temperature (°C).
    pub fn mean_annual_temp_c(self) -> f32 {
        match self {
            Self::TropicalRainforest      =>  26.0,
            Self::TropicalSavanna         =>  24.0,
            Self::HotDesert               =>  25.0,
            Self::ColdDesert              =>   8.0,
            Self::MediterraneanShrubland  =>  15.0,
            Self::TemperateGrassland      =>   8.0,
            Self::TemperateDeciduousForest =>  10.0,
            Self::TemperateConiferousForest =>  7.0,
            Self::BorealForest            =>  -3.0,
            Self::Tundra                  => -10.0,
            Self::PolarIce                => -25.0,
            Self::MontaneAlpine           =>  -2.0,
            Self::CoastalMarine           =>  14.0,
            Self::Wetland                 =>  12.0,
            Self::UrbanHeatIsland         =>  13.0,
        }
    }

    /// Annual temperature range (°C) between coldest and warmest month.
    pub fn annual_temp_range_c(self) -> f32 {
        match self {
            Self::TropicalRainforest      =>  2.0,
            Self::TropicalSavanna         =>  8.0,
            Self::HotDesert               => 20.0,
            Self::ColdDesert              => 35.0,
            Self::MediterraneanShrubland  => 15.0,
            Self::TemperateGrassland      => 30.0,
            Self::TemperateDeciduousForest => 25.0,
            Self::TemperateConiferousForest => 28.0,
            Self::BorealForest            => 40.0,
            Self::Tundra                  => 35.0,
            Self::PolarIce                => 30.0,
            Self::MontaneAlpine           => 22.0,
            Self::CoastalMarine           => 10.0,
            Self::Wetland                 => 20.0,
            Self::UrbanHeatIsland         => 22.0,
        }
    }

    /// Mean annual precipitation (mm).
    pub fn mean_annual_precip_mm(self) -> f32 {
        match self {
            Self::TropicalRainforest      => 2500.0,
            Self::TropicalSavanna         =>  900.0,
            Self::HotDesert               =>   50.0,
            Self::ColdDesert              =>  150.0,
            Self::MediterraneanShrubland  =>  500.0,
            Self::TemperateGrassland      =>  400.0,
            Self::TemperateDeciduousForest => 750.0,
            Self::TemperateConiferousForest => 900.0,
            Self::BorealForest            =>  500.0,
            Self::Tundra                  =>  200.0,
            Self::PolarIce                =>  100.0,
            Self::MontaneAlpine           =>  800.0,
            Self::CoastalMarine           =>  700.0,
            Self::Wetland                 => 1000.0,
            Self::UrbanHeatIsland         =>  600.0,
        }
    }

    /// Diurnal temperature range (°C, day vs night).
    pub fn diurnal_range_c(self) -> f32 {
        match self {
            Self::TropicalRainforest      =>  5.0,
            Self::HotDesert               => 25.0,
            Self::CoastalMarine           =>  6.0,
            Self::PolarIce                =>  8.0,
            _                             => 12.0,
        }
    }
}

// ── Climate Zone ──────────────────────────────────────────────────────────────

/// A climate zone associated with a biome.
#[derive(Debug, Clone)]
pub struct BiomeZone {
    pub biome: BiomeType,
    /// Latitude range [min, max] degrees.
    pub latitude_range: [f32; 2],
    /// Altitude range [min, max] metres.
    pub altitude_range: [f32; 2],
    /// Koppen climate classification code.
    pub koppen: &'static str,
    /// Dominant wind direction (radians from east).
    pub prevailing_wind_dir: f32,
    /// Mean wind speed (m/s).
    pub mean_wind_speed: f32,
}

impl BiomeZone {
    pub fn temperate_deciduous() -> Self {
        Self {
            biome: BiomeType::TemperateDeciduousForest,
            latitude_range: [40.0, 60.0],
            altitude_range: [0.0, 1500.0],
            koppen: "Cfb",
            prevailing_wind_dir: 0.0, // westerlies
            mean_wind_speed: 6.0,
        }
    }

    pub fn tropical_rainforest() -> Self {
        Self {
            biome: BiomeType::TropicalRainforest,
            latitude_range: [-10.0, 10.0],
            altitude_range: [0.0, 1000.0],
            koppen: "Af",
            prevailing_wind_dir: std::f32::consts::PI * 0.25,
            mean_wind_speed: 3.0,
        }
    }

    pub fn hot_desert() -> Self {
        Self {
            biome: BiomeType::HotDesert,
            latitude_range: [20.0, 35.0],
            altitude_range: [0.0, 800.0],
            koppen: "BWh",
            prevailing_wind_dir: std::f32::consts::PI * 0.75,
            mean_wind_speed: 8.0,
        }
    }

    pub fn boreal() -> Self {
        Self {
            biome: BiomeType::BorealForest,
            latitude_range: [50.0, 70.0],
            altitude_range: [0.0, 800.0],
            koppen: "Dfc",
            prevailing_wind_dir: 0.0,
            mean_wind_speed: 5.0,
        }
    }

    /// Is the given latitude inside this zone's range?
    pub fn contains_lat(&self, lat: f32) -> bool {
        lat >= self.latitude_range[0] && lat <= self.latitude_range[1]
    }

    /// Is the given altitude inside this zone's range?
    pub fn contains_alt(&self, alt: f32) -> bool {
        alt >= self.altitude_range[0] && alt <= self.altitude_range[1]
    }
}

// ── Temperature Range ─────────────────────────────────────────────────────────

/// Min/max temperature over a period.
#[derive(Debug, Clone, Copy)]
pub struct TemperatureRange {
    pub min_c: f32,
    pub max_c: f32,
}

impl TemperatureRange {
    pub fn new(min_c: f32, max_c: f32) -> Self { Self { min_c, max_c } }
    pub fn mean(&self) -> f32 { (self.min_c + self.max_c) * 0.5 }
    pub fn amplitude(&self) -> f32 { self.max_c - self.min_c }
    pub fn contains(&self, t: f32) -> bool { t >= self.min_c && t <= self.max_c }
}

// ── Seasonal Cycle ────────────────────────────────────────────────────────────

/// Seasonal temperature and precipitation data for a location.
#[derive(Debug, Clone)]
pub struct SeasonalCycle {
    pub biome: BiomeType,
    /// Monthly mean temperatures (°C) — 12 values Jan–Dec.
    pub monthly_temp_c: [f32; 12],
    /// Monthly precipitation (mm) — 12 values.
    pub monthly_precip_mm: [f32; 12],
    /// Monthly mean humidity [0,1].
    pub monthly_humidity: [f32; 12],
    /// Monthly sunshine hours (average per day).
    pub monthly_sunshine_h: [f32; 12],
}

impl SeasonalCycle {
    /// Build a simple sinusoidal cycle from biome parameters.
    pub fn from_biome(biome: BiomeType, latitude: f32) -> Self {
        let base_temp = biome.mean_annual_temp_c();
        let amplitude = biome.annual_temp_range_c() * 0.5;
        let northern  = latitude >= 0.0;
        // Northern hemisphere: coldest Jan (month 0), warmest Jul (month 6)
        let mut monthly_temp_c = [0.0_f32; 12];
        let mut monthly_precip_mm = [0.0_f32; 12];
        let mut monthly_humidity = [0.0_f32; 12];
        let mut monthly_sunshine_h = [0.0_f32; 12];

        let annual_precip = biome.mean_annual_precip_mm();
        for m in 0..12 {
            // Temperature: cosine with phase shift
            let phase = if northern {
                (m as f32 - 6.5) / 12.0 * 2.0 * std::f32::consts::PI
            } else {
                (m as f32 - 0.5) / 12.0 * 2.0 * std::f32::consts::PI
            };
            monthly_temp_c[m] = base_temp - amplitude * phase.cos();

            // Precipitation: varies by biome type
            let precip_phase = match biome {
                BiomeType::MediterraneanShrubland => {
                    // Wet winters, dry summers
                    let p = (m as f32 - 0.5) / 12.0 * 2.0 * std::f32::consts::PI;
                    1.0 + p.cos() // peak in Jan
                }
                BiomeType::TropicalSavanna => {
                    // Wet summer, dry winter
                    let p = (m as f32 - 6.5) / 12.0 * 2.0 * std::f32::consts::PI;
                    1.0 + p.cos()
                }
                _ => 1.0, // uniform
            };
            monthly_precip_mm[m] = annual_precip / 12.0 * precip_phase;

            // Humidity: inverse of temperature for most biomes
            let temp_norm = (monthly_temp_c[m] - (base_temp - amplitude))
                / (2.0 * amplitude).max(1.0);
            monthly_humidity[m] = match biome {
                BiomeType::TropicalRainforest => 0.85 + temp_norm * 0.1,
                BiomeType::HotDesert | BiomeType::ColdDesert => 0.15 + (1.0 - temp_norm) * 0.15,
                _ => 0.55 + (1.0 - temp_norm) * 0.2,
            }.clamp(0.1, 1.0);

            // Sunshine: longer days in summer
            monthly_sunshine_h[m] = 8.0 + (monthly_temp_c[m] - base_temp) / amplitude.max(1.0) * 4.0;
        }

        Self {
            biome,
            monthly_temp_c,
            monthly_precip_mm,
            monthly_humidity,
            monthly_sunshine_h,
        }
    }

    /// Interpolated temperature for fractional month (0.0 = Jan 1, 11.999 = Dec 31).
    pub fn temperature_at_month(&self, month_frac: f32) -> f32 {
        let m0 = (month_frac.floor() as usize) % 12;
        let m1 = (m0 + 1) % 12;
        let t  = month_frac - month_frac.floor();
        lerp(self.monthly_temp_c[m0], self.monthly_temp_c[m1], t)
    }

    /// Convert day-of-year [0,365) to fractional month index.
    pub fn day_to_month_frac(day: f32) -> f32 {
        (day / 365.0 * 12.0).rem_euclid(12.0)
    }

    pub fn humidity_at_month(&self, month_frac: f32) -> f32 {
        let m0 = (month_frac.floor() as usize) % 12;
        let m1 = (m0 + 1) % 12;
        let t  = month_frac - month_frac.floor();
        lerp(self.monthly_humidity[m0], self.monthly_humidity[m1], t)
    }

    pub fn precipitation_at_month(&self, month_frac: f32) -> f32 {
        let m0 = (month_frac.floor() as usize) % 12;
        let m1 = (m0 + 1) % 12;
        let t  = month_frac - month_frac.floor();
        lerp(self.monthly_precip_mm[m0], self.monthly_precip_mm[m1], t)
    }
}

// ── Day/Night Temperature Curve ───────────────────────────────────────────────

/// Models the diurnal temperature variation throughout a 24-hour cycle.
#[derive(Debug, Clone)]
pub struct DayNightCurve {
    /// Temperature at sunrise (°C).
    pub sunrise_temp_c: f32,
    /// Temperature at daily maximum (°C).
    pub max_temp_c: f32,
    /// Temperature at sunset (°C).
    pub sunset_temp_c: f32,
    /// Temperature at daily minimum (°C), typically before sunrise.
    pub min_temp_c: f32,
    /// Hour of sunrise (0–24).
    pub sunrise_h: f32,
    /// Hour of maximum temperature (0–24), typically 2–3 h after solar noon.
    pub max_temp_h: f32,
    /// Hour of sunset (0–24).
    pub sunset_h: f32,
    /// Hour of minimum temperature (0–24), typically just before sunrise.
    pub min_temp_h: f32,
}

impl DayNightCurve {
    pub fn new(
        mean_c: f32,
        amplitude_c: f32,
        day_length_h: f32,
        solar_noon: f32,
    ) -> Self {
        let half_day = day_length_h * 0.5;
        let sunrise_h = (solar_noon - half_day + 24.0).rem_euclid(24.0);
        let sunset_h  = (solar_noon + half_day).rem_euclid(24.0);
        let max_temp_h = (solar_noon + 2.5).rem_euclid(24.0);
        let min_temp_h = (sunrise_h - 1.5 + 24.0).rem_euclid(24.0);
        Self {
            sunrise_temp_c: mean_c - amplitude_c * 0.6,
            max_temp_c:     mean_c + amplitude_c * 0.5,
            sunset_temp_c:  mean_c - amplitude_c * 0.2,
            min_temp_c:     mean_c - amplitude_c * 0.5,
            sunrise_h,
            max_temp_h,
            sunset_h,
            min_temp_h,
        }
    }

    /// Sample temperature (°C) at time of day `h` (0–24).
    pub fn temperature_at(&self, h: f32) -> f32 {
        // Use piecewise cosine interpolation between key points
        let h = h.rem_euclid(24.0);
        // Key points in order: min, sunrise, max, sunset, (back to min next day)
        struct Kp { h: f32, t: f32 }
        let mut kps = [
            Kp { h: self.min_temp_h,    t: self.min_temp_c    },
            Kp { h: self.sunrise_h,     t: self.sunrise_temp_c},
            Kp { h: self.max_temp_h,    t: self.max_temp_c    },
            Kp { h: self.sunset_h,      t: self.sunset_temp_c },
        ];
        // Sort by hour for consistent interpolation
        kps.sort_by(|a, b| a.h.partial_cmp(&b.h).unwrap_or(std::cmp::Ordering::Equal));

        // Find bracketing points
        for i in 0..kps.len() {
            let next = (i + 1) % kps.len();
            let h0 = kps[i].h;
            let h1 = if next == 0 { kps[next].h + 24.0 } else { kps[next].h };
            let hh  = if h < h0 { h + 24.0 } else { h };
            if hh >= h0 && hh <= h1 {
                let span = (h1 - h0).max(1e-4);
                let t    = (hh - h0) / span;
                // Cosine interpolation
                let tc = (1.0 - (t * std::f32::consts::PI).cos()) * 0.5;
                return lerp(kps[i].t, kps[next].t, tc);
            }
        }
        self.min_temp_c
    }

    /// Compute day-length hours from latitude and day-of-year.
    pub fn day_length_from_lat(lat_deg: f32, day: f32) -> f32 {
        let lat_rad = lat_deg.to_radians();
        // Solar declination
        let dec = (-23.45_f32 * ((day + 10.0) / 365.0 * 2.0 * std::f32::consts::PI).cos()).to_radians();
        let cos_ha = -(lat_rad.tan() * dec.tan());
        if cos_ha < -1.0 { return 24.0; }
        if cos_ha >  1.0 { return 0.0; }
        let ha_rad = cos_ha.acos();
        ha_rad.to_degrees() / 7.5 // hours = degrees / 15 * 2 (for full arc)
    }
}

// ── Climate Interpolator ──────────────────────────────────────────────────────

/// Blends between two climate zones based on a parameter.
#[derive(Debug, Clone)]
pub struct ClimateInterpolator {
    pub zone_a: BiomeZone,
    pub zone_b: BiomeZone,
    pub cycle_a: SeasonalCycle,
    pub cycle_b: SeasonalCycle,
    /// Blend factor [0,1]; 0 = pure A, 1 = pure B.
    pub blend: f32,
    /// Target blend (for smooth transitions).
    pub target_blend: f32,
    /// Blend transition speed (units/second).
    pub transition_speed: f32,
}

impl ClimateInterpolator {
    pub fn new(zone_a: BiomeZone, zone_b: BiomeZone, lat: f32) -> Self {
        let cycle_a = SeasonalCycle::from_biome(zone_a.biome, lat);
        let cycle_b = SeasonalCycle::from_biome(zone_b.biome, lat);
        Self {
            zone_a,
            zone_b,
            cycle_a,
            cycle_b,
            blend: 0.0,
            target_blend: 0.0,
            transition_speed: 1e-5,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let delta = self.target_blend - self.blend;
        let step  = self.transition_speed * dt;
        if delta.abs() < step {
            self.blend = self.target_blend;
        } else {
            self.blend += step * delta.signum();
        }
    }

    pub fn temperature_at_month(&self, month_frac: f32) -> f32 {
        let ta = self.cycle_a.temperature_at_month(month_frac);
        let tb = self.cycle_b.temperature_at_month(month_frac);
        lerp(ta, tb, self.blend)
    }

    pub fn humidity_at_month(&self, month_frac: f32) -> f32 {
        let ha = self.cycle_a.humidity_at_month(month_frac);
        let hb = self.cycle_b.humidity_at_month(month_frac);
        lerp(ha, hb, self.blend)
    }

    pub fn prevailing_wind_speed(&self) -> f32 {
        lerp(self.zone_a.mean_wind_speed, self.zone_b.mean_wind_speed, self.blend)
    }
}

// ── Weather Pattern ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherPatternKind {
    HighPressureRidge,
    LowPressureTrough,
    ColdFront,
    WarmFront,
    OccludedFront,
    StationaryFront,
    Anticyclone,
    Cyclone,
    Monsoon,
    TradeWinds,
    BlockingHigh,
}

/// A large-scale weather pattern.
#[derive(Debug, Clone)]
pub struct WeatherPattern {
    pub kind: WeatherPatternKind,
    /// Position of the pattern centre (world x, z).
    pub centre: [f32; 2],
    /// Radius of influence (m).
    pub radius: f32,
    /// Pattern intensity [0,1].
    pub intensity: f32,
    /// Drift velocity (m/s).
    pub drift: [f32; 2],
    /// Remaining lifetime (seconds).
    pub lifetime: f32,
    /// Associated pressure anomaly (Pa, positive = high, negative = low).
    pub pressure_anomaly_pa: f32,
    /// Associated temperature anomaly (°C).
    pub temp_anomaly_c: f32,
    /// Associated precipitation modifier [0,2]; 1 = no change.
    pub precip_modifier: f32,
}

impl WeatherPattern {
    pub fn new_high_pressure(cx: f32, cz: f32) -> Self {
        Self {
            kind: WeatherPatternKind::HighPressureRidge,
            centre: [cx, cz],
            radius: 800_000.0,
            intensity: 0.8,
            drift: [1.0, 0.3],
            lifetime: 7_200.0,
            pressure_anomaly_pa: 2_500.0,
            temp_anomaly_c: 3.0,
            precip_modifier: 0.1,
        }
    }

    pub fn new_low_pressure(cx: f32, cz: f32) -> Self {
        Self {
            kind: WeatherPatternKind::LowPressureTrough,
            centre: [cx, cz],
            radius: 600_000.0,
            intensity: 0.9,
            drift: [3.0, -0.5],
            lifetime: 5_400.0,
            pressure_anomaly_pa: -3_000.0,
            temp_anomaly_c: -2.0,
            precip_modifier: 2.5,
        }
    }

    pub fn new_monsoon(cx: f32, cz: f32) -> Self {
        Self {
            kind: WeatherPatternKind::Monsoon,
            centre: [cx, cz],
            radius: 1_500_000.0,
            intensity: 1.0,
            drift: [0.5, 0.1],
            lifetime: 86_400.0 * 90.0, // ~90 days
            pressure_anomaly_pa: -1_500.0,
            temp_anomaly_c: 1.5,
            precip_modifier: 5.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.centre[0] += self.drift[0] * dt;
        self.centre[1] += self.drift[1] * dt;
        self.lifetime  -= dt;
        // Decay near end of life
        if self.lifetime < 600.0 {
            self.intensity *= 1.0 - dt / 600.0;
        }
    }

    /// Influence on temperature at world position.
    pub fn temp_influence(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= self.radius { return 0.0; }
        smoothstep(self.radius, 0.0, dist) * self.temp_anomaly_c * self.intensity
    }

    /// Influence on precipitation multiplier at world position.
    pub fn precip_influence(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= self.radius { return 1.0; }
        let t = smoothstep(self.radius, 0.0, dist);
        lerp(1.0, self.precip_modifier, t * self.intensity)
    }

    pub fn is_alive(&self) -> bool { self.lifetime > 0.0 && self.intensity > 1e-4 }
}

// ── Storm Front ───────────────────────────────────────────────────────────────

/// A weather front — boundary between air masses.
#[derive(Debug, Clone)]
pub struct StormFront {
    pub kind: WeatherPatternKind,
    /// Origin point in world (x, z).
    pub origin: [f32; 2],
    /// Direction of movement (radians).
    pub direction: f32,
    /// Speed (m/s).
    pub speed: f32,
    /// Half-width of the frontal zone (m).
    pub width: f32,
    /// Length of the front (m).
    pub length: f32,
    /// Age (seconds).
    pub age: f32,
    /// Maximum lifetime (seconds).
    pub max_lifetime: f32,
    /// Temperature change across the front (°C, positive = warmer air behind).
    pub temp_gradient_c: f32,
    /// Precipitation intensity on the leading edge [0,1].
    pub leading_precip: f32,
    /// Precipitation intensity on the trailing edge [0,1].
    pub trailing_precip: f32,
    /// Whether the front has become occluded.
    pub occluded: bool,
}

impl StormFront {
    pub fn cold_front(ox: f32, oz: f32, direction: f32) -> Self {
        Self {
            kind: WeatherPatternKind::ColdFront,
            origin: [ox, oz],
            direction,
            speed: 8.0,
            width: 50_000.0,
            length: 2_000_000.0,
            age: 0.0,
            max_lifetime: 86_400.0,
            temp_gradient_c: -8.0,
            leading_precip: 0.7,
            trailing_precip: 0.1,
            occluded: false,
        }
    }

    pub fn warm_front(ox: f32, oz: f32, direction: f32) -> Self {
        Self {
            kind: WeatherPatternKind::WarmFront,
            origin: [ox, oz],
            direction,
            speed: 4.0,
            width: 120_000.0,
            length: 1_500_000.0,
            age: 0.0,
            max_lifetime: 72_000.0,
            temp_gradient_c: 6.0,
            leading_precip: 0.4,
            trailing_precip: 0.6,
            occluded: false,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.age += dt;
        self.origin[0] += self.direction.cos() * self.speed * dt;
        self.origin[1] += self.direction.sin() * self.speed * dt;
        // Fronts slow and widen (occlude) with age
        if self.age > self.max_lifetime * 0.7 && !self.occluded {
            self.occluded = true;
            self.speed   *= 0.3;
            self.kind     = WeatherPatternKind::OccludedFront;
        }
    }

    /// Signed distance from point (x,z) to the frontal boundary.
    /// Negative = ahead of the front, positive = behind.
    pub fn signed_distance(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.origin[0];
        let dz = z - self.origin[1];
        // Project onto front normal (direction the front is moving)
        -(dx * self.direction.cos() + dz * self.direction.sin())
    }

    /// Temperature anomaly at world position.
    pub fn temp_at(&self, x: f32, z: f32) -> f32 {
        let sd = self.signed_distance(x, z);
        if sd.abs() > self.width { return 0.0; }
        let t = (sd / self.width).clamp(-1.0, 1.0);
        self.temp_gradient_c * t * (1.0 - self.age / self.max_lifetime).max(0.0)
    }

    /// Precipitation multiplier at world position.
    pub fn precip_at(&self, x: f32, z: f32) -> f32 {
        let sd = self.signed_distance(x, z);
        if sd.abs() > self.width { return 0.0; }
        let t = ((sd / self.width) + 1.0) * 0.5; // 0 = behind, 1 = ahead
        lerp(self.trailing_precip, self.leading_precip, t)
    }

    pub fn is_alive(&self) -> bool { self.age < self.max_lifetime }
}

// ── Heat Wave ─────────────────────────────────────────────────────────────────

/// A heat wave event — sustained anomalously high temperatures.
#[derive(Debug, Clone)]
pub struct HeatWave {
    /// Remaining duration (seconds).
    pub duration_s: f32,
    /// Peak temperature anomaly (°C).
    pub peak_anomaly_c: f32,
    /// Current anomaly (°C) — ramps up and down.
    pub current_anomaly_c: f32,
    /// Position of the hot dome centre (world x, z).
    pub centre: [f32; 2],
    /// Radius of influence (m).
    pub radius: f32,
    /// Maximum intensity (occurs at mid-event).
    intensity: f32,
    elapsed_s: f32,
}

impl HeatWave {
    pub fn new(cx: f32, cz: f32, peak_anomaly_c: f32, duration_s: f32) -> Self {
        Self {
            duration_s,
            peak_anomaly_c,
            current_anomaly_c: 0.0,
            centre: [cx, cz],
            radius: 500_000.0,
            intensity: 0.0,
            elapsed_s: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed_s += dt;
        let progress = (self.elapsed_s / self.duration_s).clamp(0.0, 1.0);
        // Bell-shaped intensity curve
        self.intensity = smoothstep(0.0, 0.3, progress) * smoothstep(1.0, 0.7, progress);
        self.current_anomaly_c = self.peak_anomaly_c * self.intensity;
    }

    /// Temperature anomaly at world position.
    pub fn temp_anomaly(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= self.radius { return 0.0; }
        smoothstep(self.radius, 0.0, dist) * self.current_anomaly_c
    }

    pub fn is_active(&self) -> bool { self.elapsed_s < self.duration_s }
    pub fn progress(&self) -> f32 { (self.elapsed_s / self.duration_s).clamp(0.0, 1.0) }
}

// ── Cold Snap ─────────────────────────────────────────────────────────────────

/// A cold snap event — sustained anomalously low temperatures.
#[derive(Debug, Clone)]
pub struct ColdSnap {
    pub duration_s: f32,
    pub peak_anomaly_c: f32,   // Negative value
    pub current_anomaly_c: f32,
    pub centre: [f32; 2],
    pub radius: f32,
    /// Wind chill enhancement factor.
    pub wind_chill_factor: f32,
    elapsed_s: f32,
    intensity: f32,
}

impl ColdSnap {
    pub fn new(cx: f32, cz: f32, peak_anomaly_c: f32, duration_s: f32) -> Self {
        assert!(peak_anomaly_c <= 0.0, "Cold snap anomaly must be negative or zero");
        Self {
            duration_s,
            peak_anomaly_c,
            current_anomaly_c: 0.0,
            centre: [cx, cz],
            radius: 600_000.0,
            wind_chill_factor: 1.5,
            elapsed_s: 0.0,
            intensity: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed_s += dt;
        let progress = (self.elapsed_s / self.duration_s).clamp(0.0, 1.0);
        self.intensity = smoothstep(0.0, 0.25, progress) * smoothstep(1.0, 0.75, progress);
        self.current_anomaly_c = self.peak_anomaly_c * self.intensity;
    }

    pub fn temp_anomaly(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= self.radius { return 0.0; }
        smoothstep(self.radius, 0.0, dist) * self.current_anomaly_c
    }

    /// Wind-chill corrected temperature at position given wind speed.
    pub fn apparent_temp(&self, x: f32, z: f32, base_temp_c: f32, wind_speed_ms: f32) -> f32 {
        let anomaly = self.temp_anomaly(x, z);
        let actual = base_temp_c + anomaly;
        // Siple-Passel wind chill formula (simplified)
        if wind_speed_ms < 1.4 || actual >= 10.0 { return actual; }
        let v = wind_speed_ms;
        13.12 + 0.6215 * actual - 11.37 * v.powf(0.16) + 0.3965 * actual * v.powf(0.16)
    }

    pub fn is_active(&self) -> bool { self.elapsed_s < self.duration_s }
}

// ── Weather Transition ────────────────────────────────────────────────────────

/// A smooth transition between two weather states.
#[derive(Debug, Clone)]
pub struct WeatherTransition {
    pub from_pattern: WeatherPatternKind,
    pub to_pattern: WeatherPatternKind,
    /// Transition progress [0,1].
    pub progress: f32,
    /// Total transition duration (seconds).
    pub duration_s: f32,
    elapsed_s: f32,
}

impl WeatherTransition {
    pub fn new(from: WeatherPatternKind, to: WeatherPatternKind, duration_s: f32) -> Self {
        Self {
            from_pattern: from,
            to_pattern: to,
            progress: 0.0,
            duration_s,
            elapsed_s: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed_s = (self.elapsed_s + dt).min(self.duration_s);
        self.progress = smoothstep(0.0, self.duration_s, self.elapsed_s);
    }

    pub fn is_complete(&self) -> bool { self.elapsed_s >= self.duration_s }
}

// ── Precipitation Chance ──────────────────────────────────────────────────────

/// Probability of precipitation for a given period.
#[derive(Debug, Clone, Copy)]
pub struct PrecipitationChance {
    pub probability: f32,     // 0–1
    pub expected_intensity: f32, // 0–1
    pub duration_hours: f32,
}

impl PrecipitationChance {
    pub fn none() -> Self {
        Self { probability: 0.0, expected_intensity: 0.0, duration_hours: 0.0 }
    }
    pub fn from_humidity(humidity: f32, temp_c: f32) -> Self {
        let prob = smoothstep(0.6, 0.95, humidity) * (1.0 - smoothstep(35.0, 45.0, temp_c));
        let intens = smoothstep(0.7, 1.0, humidity);
        Self {
            probability: prob,
            expected_intensity: intens,
            duration_hours: prob * 3.0,
        }
    }
}

// ── Wind Pattern ──────────────────────────────────────────────────────────────

/// Prevailing wind pattern for a region.
#[derive(Debug, Clone)]
pub struct WindPattern {
    /// Direction (radians from east).
    pub direction: f32,
    /// Speed (m/s).
    pub speed: f32,
    /// Variability (standard deviation in direction, radians).
    pub direction_variability: f32,
    /// Speed variability (m/s).
    pub speed_variability: f32,
    /// Gust factor (peak/mean ratio).
    pub gust_factor: f32,
}

impl WindPattern {
    pub fn westerlies() -> Self {
        Self { direction: 0.0, speed: 7.0, direction_variability: 0.4, speed_variability: 2.5, gust_factor: 1.8 }
    }
    pub fn trade_winds() -> Self {
        Self { direction: std::f32::consts::PI * 1.25, speed: 6.0, direction_variability: 0.2, speed_variability: 1.5, gust_factor: 1.4 }
    }
    pub fn doldrums() -> Self {
        Self { direction: 0.0, speed: 0.5, direction_variability: 2.0, speed_variability: 1.0, gust_factor: 2.5 }
    }
    pub fn polar_easterlies() -> Self {
        Self { direction: std::f32::consts::PI, speed: 9.0, direction_variability: 0.5, speed_variability: 4.0, gust_factor: 2.2 }
    }

    /// Sample wind given a noise value `n` in [0,1].
    pub fn sample(&self, n: f32) -> Vec3 {
        let dir = self.direction + (n * 2.0 - 1.0) * self.direction_variability;
        let spd = (self.speed + (n - 0.5) * self.speed_variability * 2.0).max(0.0);
        Vec3::new(dir.cos() * spd, 0.0, dir.sin() * spd)
    }

    /// Gust wind at given noise.
    pub fn gust(&self, n: f32) -> Vec3 {
        let base = self.sample(n);
        base.scale(if n > 0.85 { self.gust_factor } else { 1.0 })
    }
}

// ── Climate Event ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ClimateEvent {
    HeatWaveStarted { anomaly_c: f32, duration_s: f32 },
    ColdSnapStarted { anomaly_c: f32, duration_s: f32 },
    StormFrontApproaching { kind: WeatherPatternKind, eta_s: f32 },
    SeasonChanged { from: Season, to: Season },
    DustStorm { origin: [f32; 2], intensity: f32 },
    Blizzard { snow_rate_mm_h: f32, wind_speed_ms: f32 },
}

// ── Climate Cell ──────────────────────────────────────────────────────────────

/// A single cell in a coarse climate model grid.
#[derive(Debug, Clone)]
pub struct ClimateCell {
    pub lat: f32,
    pub lon: f32,
    pub biome: BiomeType,
    pub current_temp_c: f32,
    pub current_humidity: f32,
    pub current_pressure_pa: f32,
    pub wind: Vec3,
    pub cloud_cover: f32,
    pub snow_cover_fraction: f32,
}

impl ClimateCell {
    pub fn new(lat: f32, lon: f32, biome: BiomeType) -> Self {
        Self {
            lat,
            lon,
            biome,
            current_temp_c: biome.mean_annual_temp_c(),
            current_humidity: 0.5,
            current_pressure_pa: 101_325.0,
            wind: Vec3::ZERO,
            cloud_cover: 0.3,
            snow_cover_fraction: 0.0,
        }
    }
}

// ── Climate Config ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClimateConfig {
    /// Latitude of the simulation origin (degrees).
    pub latitude: f32,
    /// Longitude of the simulation origin (degrees).
    pub longitude: f32,
    /// Scale factor for time acceleration (1.0 = real time).
    pub time_scale: f32,
    /// How often (seconds) to potentially trigger extreme events.
    pub event_check_interval: f32,
    /// Probability of a heat wave per check.
    pub heat_wave_probability: f32,
    /// Probability of a cold snap per check.
    pub cold_snap_probability: f32,
    /// Probability of a storm front per check.
    pub storm_probability: f32,
    /// Maximum simultaneous weather patterns.
    pub max_patterns: usize,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            latitude: 50.0,
            longitude: 0.0,
            time_scale: 1.0,
            event_check_interval: 3_600.0,
            heat_wave_probability: 0.02,
            cold_snap_probability: 0.03,
            storm_probability: 0.05,
            max_patterns: 8,
        }
    }
}

// ── Climate System ────────────────────────────────────────────────────────────

/// The main climate simulation state.
#[derive(Debug, Clone)]
pub struct ClimateSystem {
    pub config: ClimateConfig,
    pub primary_biome: BiomeType,
    pub seasonal_cycle: SeasonalCycle,
    pub day_night_curve: DayNightCurve,
    pub interpolator: Option<ClimateInterpolator>,
    pub weather_patterns: Vec<WeatherPattern>,
    pub storm_fronts: Vec<StormFront>,
    pub heat_waves: Vec<HeatWave>,
    pub cold_snaps: Vec<ColdSnap>,
    pub active_transitions: Vec<WeatherTransition>,
    pub pending_events: Vec<ClimateEvent>,
    pub wind_pattern: WindPattern,
    /// Grid of climate cells for spatial variation.
    pub cells: Vec<ClimateCell>,
    pub grid_w: usize,
    pub grid_d: usize,
    /// Current surface temperature (°C) at simulation origin.
    current_temp_c: f32,
    /// Current humidity [0,1] at simulation origin.
    current_humidity: f32,
    /// Noise offset for pseudo-random weather variation.
    noise_t: f32,
    /// Time since last event check (s).
    event_check_accum: f32,
    /// Current season.
    cached_season: Season,
    /// Previous season (for detecting transitions).
    prev_season: Season,
}

impl ClimateSystem {
    pub fn new(latitude: f32) -> Self {
        Self::with_config(ClimateConfig { latitude, ..ClimateConfig::default() })
    }

    pub fn with_config(config: ClimateConfig) -> Self {
        let biome = Self::biome_for_latitude(config.latitude);
        let cycle = SeasonalCycle::from_biome(biome, config.latitude);
        let mean_temp = biome.mean_annual_temp_c();
        let amp = biome.diurnal_range_c();
        let day_len = DayNightCurve::day_length_from_lat(config.latitude, 172.0); // summer solstice
        let curve = DayNightCurve::new(mean_temp, amp, day_len, 12.0);

        let wind = if config.latitude.abs() > 60.0 {
            WindPattern::polar_easterlies()
        } else if config.latitude.abs() < 20.0 {
            WindPattern::trade_winds()
        } else {
            WindPattern::westerlies()
        };

        // Build a small grid of climate cells
        let grid_w = 8usize;
        let grid_d = 8usize;
        let mut cells = Vec::with_capacity(grid_w * grid_d);
        for gz in 0..grid_d {
            for gx in 0..grid_w {
                let lat_offset = (gz as f32 - grid_d as f32 * 0.5) * 0.5;
                let lon_offset = (gx as f32 - grid_w as f32 * 0.5) * 0.5;
                let cell_lat = config.latitude + lat_offset;
                let cell_biome = Self::biome_for_latitude(cell_lat);
                cells.push(ClimateCell::new(cell_lat, config.longitude + lon_offset, cell_biome));
            }
        }

        let northern = config.latitude >= 0.0;
        let season = Season::from_day(0.0, northern);

        Self {
            config,
            primary_biome: biome,
            seasonal_cycle: cycle,
            day_night_curve: curve,
            interpolator: None,
            weather_patterns: Vec::new(),
            storm_fronts: Vec::new(),
            heat_waves: Vec::new(),
            cold_snaps: Vec::new(),
            active_transitions: Vec::new(),
            pending_events: Vec::new(),
            wind_pattern: wind,
            cells,
            grid_w,
            grid_d,
            current_temp_c: biome.mean_annual_temp_c(),
            current_humidity: 0.55,
            noise_t: 0.0,
            event_check_accum: 0.0,
            cached_season: season,
            prev_season: season,
        }
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    pub fn tick(&mut self, dt: f32, day_of_year: f32, time_of_day: f32) {
        self.noise_t += dt * 0.001;
        let scaled_dt = dt * self.config.time_scale;

        // Season
        let northern = self.config.latitude >= 0.0;
        self.prev_season = self.cached_season;
        self.cached_season = Season::from_day(day_of_year, northern);
        if self.cached_season != self.prev_season {
            self.pending_events.push(ClimateEvent::SeasonChanged {
                from: self.prev_season,
                to: self.cached_season,
            });
        }

        // Update day/night curve for current day length
        let day_len = DayNightCurve::day_length_from_lat(self.config.latitude, day_of_year);
        let month   = SeasonalCycle::day_to_month_frac(day_of_year);
        let mean_tc = self.seasonal_cycle.temperature_at_month(month);
        let amp     = self.primary_biome.diurnal_range_c();
        self.day_night_curve = DayNightCurve::new(mean_tc, amp, day_len, 12.0);

        // Base temperature from day/night cycle
        let mut temp_c = self.day_night_curve.temperature_at(time_of_day);

        // Base humidity from seasonal cycle
        let mut humidity = self.seasonal_cycle.humidity_at_month(month);

        // Apply weather pattern anomalies
        for pat in &self.weather_patterns {
            temp_c   += pat.temp_influence(0.0, 0.0);
            humidity *= pat.precip_influence(0.0, 0.0).clamp(0.1, 3.0);
        }

        // Apply storm fronts
        for front in &self.storm_fronts {
            temp_c   += front.temp_at(0.0, 0.0);
            humidity  = (humidity + front.precip_at(0.0, 0.0) * 0.3).clamp(0.0, 1.0);
        }

        // Apply heat waves
        for hw in &self.heat_waves {
            temp_c += hw.temp_anomaly(0.0, 0.0);
        }

        // Apply cold snaps
        for cs in &self.cold_snaps {
            temp_c += cs.temp_anomaly(0.0, 0.0);
        }

        // Small random variation (mesoscale noise)
        let noise_temp = (value_noise_2d(self.noise_t, 0.0) * 2.0 - 1.0) * 0.5;
        let noise_hum  = (value_noise_2d(0.0, self.noise_t + 3.5) * 2.0 - 1.0) * 0.02;
        temp_c   += noise_temp;
        humidity  = (humidity + noise_hum).clamp(0.0, 1.0);

        self.current_temp_c   = temp_c;
        self.current_humidity = humidity;

        // Update all dynamic patterns
        for pat in &mut self.weather_patterns { pat.tick(scaled_dt); }
        self.weather_patterns.retain(|p| p.is_alive());

        for front in &mut self.storm_fronts { front.tick(scaled_dt); }
        self.storm_fronts.retain(|f| f.is_alive());

        for hw in &mut self.heat_waves { hw.tick(scaled_dt); }
        self.heat_waves.retain(|h| h.is_active());

        for cs in &mut self.cold_snaps { cs.tick(scaled_dt); }
        self.cold_snaps.retain(|c| c.is_active());

        for tr in &mut self.active_transitions { tr.tick(scaled_dt); }
        self.active_transitions.retain(|t| !t.is_complete());

        if let Some(ref mut interp) = self.interpolator { interp.tick(scaled_dt); }

        // Stochastic event checking
        self.event_check_accum += scaled_dt;
        if self.event_check_accum >= self.config.event_check_interval {
            self.event_check_accum = 0.0;
            self.check_extreme_events();
        }

        // Update grid cells
        self.update_cells(day_of_year, time_of_day, month);
    }

    fn check_extreme_events(&mut self) {
        let rng = value_noise_2d(self.noise_t, self.noise_t * 1.3);

        // Heat wave
        if rng < self.config.heat_wave_probability
            && self.heat_waves.is_empty()
            && self.current_temp_c > 15.0
        {
            let anomaly = 8.0 + rng * 10.0;
            let dur     = 86_400.0 * (3.0 + rng * 7.0);
            let cx = (value_noise_2d(self.noise_t * 2.0, 0.0) * 2.0 - 1.0) * 200_000.0;
            let cz = (value_noise_2d(0.0, self.noise_t * 2.0 + 1.0) * 2.0 - 1.0) * 200_000.0;
            self.heat_waves.push(HeatWave::new(cx, cz, anomaly, dur));
            self.pending_events.push(ClimateEvent::HeatWaveStarted { anomaly_c: anomaly, duration_s: dur });
        }

        // Cold snap
        let rng2 = value_noise_2d(self.noise_t * 1.7, self.noise_t * 0.9 + 5.0);
        if rng2 < self.config.cold_snap_probability
            && self.cold_snaps.is_empty()
            && self.current_temp_c < 10.0
        {
            let anomaly = -(6.0 + rng2 * 15.0);
            let dur     = 86_400.0 * (2.0 + rng2 * 5.0);
            let cx = (value_noise_2d(self.noise_t * 0.8, 0.0) * 2.0 - 1.0) * 300_000.0;
            let cz = (value_noise_2d(0.0, self.noise_t * 0.8 + 2.3) * 2.0 - 1.0) * 300_000.0;
            self.cold_snaps.push(ColdSnap::new(cx, cz, anomaly, dur));
            self.pending_events.push(ClimateEvent::ColdSnapStarted { anomaly_c: anomaly, duration_s: dur });
        }

        // Storm front
        let rng3 = value_noise_2d(self.noise_t * 2.3, self.noise_t * 1.5 + 8.0);
        if rng3 < self.config.storm_probability
            && self.storm_fronts.len() < 3
            && self.weather_patterns.len() < self.config.max_patterns
        {
            let dir = rng3 * std::f32::consts::TAU;
            let kind_rng = value_noise_2d(self.noise_t + 11.0, 0.3);
            let front = if kind_rng < 0.5 {
                StormFront::cold_front(-500_000.0, -500_000.0, dir)
            } else {
                StormFront::warm_front(-300_000.0, -300_000.0, dir)
            };
            let kind = front.kind;
            self.storm_fronts.push(front);
            self.pending_events.push(ClimateEvent::StormFrontApproaching {
                kind,
                eta_s: 500_000.0 / 8.0,
            });
        }

        // Blizzard — only in winter with cold + moisture
        let rng4 = value_noise_2d(self.noise_t * 3.1, 0.77);
        if self.current_temp_c < -2.0
            && self.current_humidity > 0.7
            && rng4 < 0.04
        {
            self.pending_events.push(ClimateEvent::Blizzard {
                snow_rate_mm_h: 10.0 + rng4 * 40.0,
                wind_speed_ms:  8.0 + rng4 * 20.0,
            });
            // Add a low pressure pattern to drive it
            if self.weather_patterns.len() < self.config.max_patterns {
                let cx = (value_noise_2d(self.noise_t, 4.4) * 2.0 - 1.0) * 400_000.0;
                let cz = (value_noise_2d(4.4, self.noise_t) * 2.0 - 1.0) * 400_000.0;
                self.weather_patterns.push(WeatherPattern::new_low_pressure(cx, cz));
            }
        }
    }

    fn update_cells(&mut self, day_of_year: f32, time_of_day: f32, month: f32) {
        for (i, cell) in self.cells.iter_mut().enumerate() {
            let n  = value_noise_2d(i as f32 * 0.17 + self.noise_t, i as f32 * 0.11);
            let cycle = SeasonalCycle::from_biome(cell.biome, cell.lat);
            let amp   = cell.biome.diurnal_range_c();
            let day_l = DayNightCurve::day_length_from_lat(cell.lat, day_of_year);
            let curve = DayNightCurve::new(cycle.temperature_at_month(month), amp, day_l, 12.0);
            cell.current_temp_c    = curve.temperature_at(time_of_day) + (n * 2.0 - 1.0) * 0.5;
            cell.current_humidity  = (cycle.humidity_at_month(month) + (n - 0.5) * 0.05).clamp(0.0, 1.0);
            cell.cloud_cover = smoothstep(0.5, 0.85, cell.current_humidity);
            cell.wind = self.wind_pattern.sample(n);
            // Snow cover
            if cell.current_temp_c < 0.0 {
                cell.snow_cover_fraction = (cell.snow_cover_fraction + 0.0001).min(1.0);
            } else {
                cell.snow_cover_fraction = (cell.snow_cover_fraction - 0.0002).max(0.0);
            }
        }
    }

    // ── Public Queries ────────────────────────────────────────────────────────

    /// Current surface temperature at the simulation origin.
    pub fn surface_temperature(&self, time_of_day: f32, day_of_year: f32) -> f32 {
        let month = SeasonalCycle::day_to_month_frac(day_of_year);
        let mean_tc = self.seasonal_cycle.temperature_at_month(month);
        let amp = self.primary_biome.diurnal_range_c();
        let day_len = DayNightCurve::day_length_from_lat(self.config.latitude, day_of_year);
        let curve = DayNightCurve::new(mean_tc, amp, day_len, 12.0);
        let base = curve.temperature_at(time_of_day);
        // Apply current anomalies
        let hw_anom: f32 = self.heat_waves.iter().map(|h| h.current_anomaly_c).sum();
        let cs_anom: f32 = self.cold_snaps.iter().map(|c| c.current_anomaly_c).sum();
        base + hw_anom + cs_anom
    }

    /// Return Kelvin surface temperature (convenience wrapper).
    pub fn surface_temperature_k(&self, time_of_day: f32, day_of_year: f32) -> f32 {
        self.surface_temperature(time_of_day, day_of_year) + 273.15
    }

    pub fn current_season(&self, day_of_year: f32) -> Season {
        Season::from_day(day_of_year, self.config.latitude >= 0.0)
    }

    /// Current humidity at origin, incorporating weather patterns.
    pub fn current_humidity(&self) -> f32 { self.current_humidity }

    /// Current temperature at origin.
    pub fn current_temperature_c(&self) -> f32 { self.current_temp_c }

    /// Drain pending events (calling code should consume these each frame).
    pub fn drain_events(&mut self) -> Vec<ClimateEvent> {
        let mut out = Vec::new();
        std::mem::swap(&mut self.pending_events, &mut out);
        out
    }

    /// Manually trigger a heat wave.
    pub fn trigger_heat_wave(&mut self, peak_c: f32, duration_s: f32) {
        self.heat_waves.push(HeatWave::new(0.0, 0.0, peak_c, duration_s));
        self.pending_events.push(ClimateEvent::HeatWaveStarted { anomaly_c: peak_c, duration_s });
    }

    /// Manually trigger a cold snap.
    pub fn trigger_cold_snap(&mut self, peak_c: f32, duration_s: f32) {
        debug_assert!(peak_c <= 0.0);
        self.cold_snaps.push(ColdSnap::new(0.0, 0.0, peak_c, duration_s));
        self.pending_events.push(ClimateEvent::ColdSnapStarted { anomaly_c: peak_c, duration_s });
    }

    /// Manually add a weather pattern.
    pub fn add_pattern(&mut self, pat: WeatherPattern) {
        if self.weather_patterns.len() < self.config.max_patterns {
            self.weather_patterns.push(pat);
        }
    }

    /// Add a storm front.
    pub fn add_storm_front(&mut self, front: StormFront) {
        self.storm_fronts.push(front);
    }

    /// Return a climate summary for UI or logging.
    pub fn summary(&self) -> ClimateSummary {
        ClimateSummary {
            biome: self.primary_biome,
            season: self.cached_season,
            temp_c: self.current_temp_c,
            humidity: self.current_humidity,
            active_heat_waves: self.heat_waves.len(),
            active_cold_snaps: self.cold_snaps.len(),
            active_storm_fronts: self.storm_fronts.len(),
            active_patterns: self.weather_patterns.len(),
        }
    }

    /// Select a plausible biome for the given latitude.
    fn biome_for_latitude(lat: f32) -> BiomeType {
        let abs_lat = lat.abs();
        if abs_lat < 10.0      { BiomeType::TropicalRainforest }
        else if abs_lat < 20.0 { BiomeType::TropicalSavanna    }
        else if abs_lat < 30.0 { BiomeType::HotDesert          }
        else if abs_lat < 40.0 { BiomeType::MediterraneanShrubland }
        else if abs_lat < 55.0 { BiomeType::TemperateDeciduousForest }
        else if abs_lat < 65.0 { BiomeType::BorealForest       }
        else if abs_lat < 75.0 { BiomeType::Tundra             }
        else                   { BiomeType::PolarIce           }
    }
}

impl Default for ClimateSystem {
    fn default() -> Self { Self::new(51.5) }
}

// ── Climate Summary ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct ClimateSummary {
    pub biome: BiomeType,
    pub season: Season,
    pub temp_c: f32,
    pub humidity: f32,
    pub active_heat_waves: usize,
    pub active_cold_snaps: usize,
    pub active_storm_fronts: usize,
    pub active_patterns: usize,
}

impl ClimateSummary {
    pub fn describe(&self) -> &'static str {
        if self.active_heat_waves > 0 { return "Heat wave"; }
        if self.active_cold_snaps > 0 { return "Cold snap"; }
        if self.active_storm_fronts > 0 { return "Stormy"; }
        match (self.season, self.temp_c as i32) {
            (Season::Summer, t) if t > 28 => "Hot and sunny",
            (Season::Winter, t) if t < 0  => "Cold and clear",
            (_, _) if self.humidity > 0.8  => "Humid and overcast",
            (Season::Spring, _)            => "Mild spring",
            (Season::Autumn, _)            => "Crisp autumn",
            _                              => "Temperate",
        }
    }
}
