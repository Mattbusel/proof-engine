//! Biome classification, climate simulation, and biome-driven parameters.
//!
//! Implements a Whittaker biome diagram classifier driven by temperature,
//! humidity, altitude, and slope. Also provides a climate simulator that
//! derives these parameters from a heightmap using atmospheric physics.

use glam::Vec3;
use crate::terrain::heightmap::HeightMap;

// ── BiomeType ─────────────────────────────────────────────────────────────────

/// All supported biome types. Each represents a distinct ecological zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeType {
    Ocean,
    DeepOcean,
    Beach,
    Desert,
    Savanna,
    Grassland,
    Shrubland,
    TemperateForest,
    TropicalForest,
    Boreal,
    Taiga,
    Tundra,
    Arctic,
    Mountain,
    AlpineGlacier,
    Swamp,
    Mangrove,
    Volcanic,
    Badlands,
    Mushroom,
}

impl BiomeType {
    /// Human-readable name of the biome.
    pub fn name(self) -> &'static str {
        match self {
            BiomeType::Ocean           => "Ocean",
            BiomeType::DeepOcean       => "Deep Ocean",
            BiomeType::Beach           => "Beach",
            BiomeType::Desert          => "Desert",
            BiomeType::Savanna         => "Savanna",
            BiomeType::Grassland       => "Grassland",
            BiomeType::Shrubland       => "Shrubland",
            BiomeType::TemperateForest => "Temperate Forest",
            BiomeType::TropicalForest  => "Tropical Forest",
            BiomeType::Boreal          => "Boreal Forest",
            BiomeType::Taiga           => "Taiga",
            BiomeType::Tundra          => "Tundra",
            BiomeType::Arctic          => "Arctic",
            BiomeType::Mountain        => "Mountain",
            BiomeType::AlpineGlacier   => "Alpine Glacier",
            BiomeType::Swamp           => "Swamp",
            BiomeType::Mangrove        => "Mangrove",
            BiomeType::Volcanic        => "Volcanic",
            BiomeType::Badlands        => "Badlands",
            BiomeType::Mushroom        => "Mushroom Island",
        }
    }

    /// Whether this biome is aquatic.
    pub fn is_aquatic(self) -> bool {
        matches!(self, BiomeType::Ocean | BiomeType::DeepOcean | BiomeType::Swamp | BiomeType::Mangrove)
    }

    /// Whether this biome is cold.
    pub fn is_cold(self) -> bool {
        matches!(self, BiomeType::Tundra | BiomeType::Arctic | BiomeType::AlpineGlacier | BiomeType::Taiga)
    }

    /// Whether this biome has trees.
    pub fn has_trees(self) -> bool {
        matches!(self,
            BiomeType::TemperateForest | BiomeType::TropicalForest |
            BiomeType::Boreal | BiomeType::Taiga | BiomeType::Swamp |
            BiomeType::Mangrove | BiomeType::Mushroom
        )
    }

    /// Index for array lookups (0-based, matches enum order).
    pub fn index(self) -> usize {
        self as usize
    }
}

// ── BiomeParams ───────────────────────────────────────────────────────────────

/// Input parameters for biome classification.
#[derive(Clone, Copy, Debug, Default)]
pub struct BiomeParams {
    /// Temperature normalized to [0, 1]. 0 = freezing, 1 = tropical.
    pub temperature: f32,
    /// Humidity (precipitation) normalized to [0, 1]. 0 = arid, 1 = rainforest.
    pub humidity: f32,
    /// Altitude normalized to [0, 1]. 0 = sea level, 1 = highest peak.
    pub altitude: f32,
    /// Slope [0, 1]. 0 = flat, 1 = cliff.
    pub slope: f32,
    /// Distance to nearest coast [0, 1]. 0 = on coast, 1 = deep inland.
    pub coast_distance: f32,
    /// Whether this is near a volcanic hot spot.
    pub volcanic: bool,
}

// ── BiomeClassifier ───────────────────────────────────────────────────────────

/// Classifies a location into a `BiomeType` based on `BiomeParams`.
///
/// Uses a Whittaker-style biome diagram (temperature × precipitation)
/// with altitude and slope overrides.
pub struct BiomeClassifier;

impl BiomeClassifier {
    /// Classify a location given its biome parameters.
    pub fn classify(p: &BiomeParams) -> BiomeType {
        // Special cases first
        if p.volcanic { return BiomeType::Volcanic; }
        if p.altitude < 0.05 { return if p.altitude < 0.02 { BiomeType::DeepOcean } else { BiomeType::Ocean }; }
        if p.altitude < 0.1 && p.coast_distance < 0.05 { return BiomeType::Beach; }
        if p.altitude < 0.1 && p.humidity > 0.7 && p.temperature > 0.5 { return BiomeType::Mangrove; }

        // Altitude override: high peaks become glaciers or mountains
        if p.altitude > 0.85 {
            if p.temperature < 0.3 || p.altitude > 0.95 { return BiomeType::AlpineGlacier; }
            return BiomeType::Mountain;
        }
        if p.altitude > 0.7 {
            if p.slope > 0.5 { return BiomeType::Mountain; }
            if p.temperature < 0.2 { return BiomeType::AlpineGlacier; }
        }

        // Temperature-based overrides
        if p.temperature < 0.1 { return BiomeType::Arctic; }
        if p.temperature < 0.25 {
            if p.humidity < 0.3 { return BiomeType::Tundra; }
            return BiomeType::Taiga;
        }
        if p.temperature < 0.4 {
            if p.humidity > 0.5 { return BiomeType::Boreal; }
            return BiomeType::Tundra;
        }

        // Whittaker diagram: temp × humidity grid
        // High humidity zone
        if p.humidity > 0.75 {
            if p.temperature > 0.65 { return BiomeType::TropicalForest; }
            if p.temperature > 0.45 { return BiomeType::TemperateForest; }
            return BiomeType::Boreal;
        }

        if p.humidity > 0.55 {
            if p.temperature > 0.65 {
                if p.altitude < 0.15 && p.coast_distance < 0.1 { return BiomeType::Mangrove; }
                return BiomeType::TropicalForest;
            }
            if p.temperature > 0.45 {
                if p.humidity > 0.65 && p.altitude < 0.15 { return BiomeType::Swamp; }
                return BiomeType::TemperateForest;
            }
            return BiomeType::Boreal;
        }

        if p.humidity > 0.35 {
            if p.temperature > 0.65 { return BiomeType::Savanna; }
            if p.temperature > 0.45 { return BiomeType::Grassland; }
            return BiomeType::Shrubland;
        }

        if p.humidity > 0.2 {
            if p.temperature > 0.55 { return BiomeType::Savanna; }
            if p.temperature > 0.4  { return BiomeType::Grassland; }
            return BiomeType::Shrubland;
        }

        // Dry zone
        if p.humidity < 0.15 {
            if p.temperature > 0.5 { return BiomeType::Desert; }
            if p.temperature > 0.3 { return BiomeType::Badlands; }
            return BiomeType::Tundra;
        }

        // Medium humidity, varying temperature
        if p.temperature > 0.6 { return BiomeType::Savanna; }
        if p.temperature > 0.4 { return BiomeType::Shrubland; }
        BiomeType::Tundra
    }

    /// Return a blend weight for each neighboring biome type.
    /// Used for smooth transitions.
    pub fn classify_blended(p: &BiomeParams) -> [(BiomeType, f32); 4] {
        let base = Self::classify(p);
        // Slightly perturbed versions to find neighbors
        let p_warm  = BiomeParams { temperature: p.temperature + 0.05, ..*p };
        let p_wet   = BiomeParams { humidity:    p.humidity    + 0.05, ..*p };
        let p_high  = BiomeParams { altitude:    p.altitude    + 0.05, ..*p };
        let b1 = Self::classify(&p_warm);
        let b2 = Self::classify(&p_wet);
        let b3 = Self::classify(&p_high);
        [
            (base, 0.7),
            (b1, if b1 != base { 0.1 } else { 0.0 }),
            (b2, if b2 != base { 0.1 } else { 0.0 }),
            (b3, if b3 != base { 0.1 } else { 0.0 }),
        ]
    }
}

// ── ClimateSimulator ──────────────────────────────────────────────────────────

/// Simulates climate across a terrain given a heightmap.
///
/// Models temperature gradients, Hadley/Ferrel circulation cells,
/// rain shadow from mountains, and ocean current effects.
pub struct ClimateSimulator {
    /// World latitude range [south, north] in degrees.
    pub latitude_range: (f32, f32),
    /// Global temperature offset in normalized units.
    pub base_temperature: f32,
    /// Global precipitation scale factor.
    pub precipitation_scale: f32,
    /// Prevailing wind direction (normalized x, z).
    pub wind_direction: (f32, f32),
}

impl Default for ClimateSimulator {
    fn default() -> Self {
        Self {
            latitude_range: (-60.0, 60.0),
            base_temperature: 0.5,
            precipitation_scale: 1.0,
            wind_direction: (1.0, 0.0),
        }
    }
}

impl ClimateSimulator {
    pub fn new() -> Self { Self::default() }

    /// Compute temperature at a given normalized position (x, y in [0,1]) and altitude.
    pub fn temperature(&self, nx: f32, ny: f32, altitude: f32) -> f32 {
        // Latitude gradient: equator hot, poles cold
        let (lat_s, lat_n) = self.latitude_range;
        let lat = lat_s + ny * (lat_n - lat_s);
        let lat_factor = (lat.to_radians().cos()).powf(0.5).clamp(0.0, 1.0);

        // Altitude cooling: lapse rate ~6.5°C per 1000m (normalized)
        let altitude_cooling = altitude * 0.5;

        // Hadley cells: tropics (|lat| < 30) hot and dry, ITC convergence
        let hadley_bonus = if lat.abs() < 30.0 {
            (1.0 - lat.abs() / 30.0) * 0.1
        } else {
            0.0
        };

        (self.base_temperature + lat_factor * 0.4 + hadley_bonus - altitude_cooling)
            .clamp(0.0, 1.0)
    }

    /// Compute precipitation at a given position, accounting for rain shadow.
    pub fn precipitation(
        &self,
        nx: f32,
        ny: f32,
        altitude: f32,
        heightmap: &HeightMap,
    ) -> f32 {
        let w = heightmap.width as f32;
        let h = heightmap.height as f32;
        let x = nx * w;
        let y = ny * h;

        // Base precipitation from Ferrel/Hadley cells
        let (lat_s, lat_n) = self.latitude_range;
        let lat = lat_s + ny * (lat_n - lat_s);
        let base_precip = {
            // High precipitation at ITCZ (~0°) and mid-latitudes (~50-60°)
            let p1 = (-(lat / 10.0).powi(2)).exp();            // ITCZ
            let p2 = (-(((lat.abs() - 55.0) / 15.0)).powi(2)).exp(); // mid-lat
            // Low at subtropical highs (~30°) and poles
            let desert_suppress = if lat.abs() > 25.0 && lat.abs() < 35.0 { 0.5 } else { 1.0 };
            (p1 * 0.6 + p2 * 0.4) * desert_suppress
        };

        // Orographic lift: upwind slope receives more rain
        let wind_x = self.wind_direction.0;
        let wind_y = self.wind_direction.1;
        let upwind_x = (x - wind_x * 20.0).clamp(0.0, w - 1.0);
        let upwind_y = (y - wind_y * 20.0).clamp(0.0, h - 1.0);
        let upwind_h = heightmap.sample_bilinear(upwind_x, upwind_y);
        let orographic = if altitude > upwind_h + 0.05 {
            // Rising air → more precipitation
            0.2 * ((altitude - upwind_h) / 0.3).clamp(0.0, 1.0)
        } else if altitude < upwind_h - 0.05 {
            // Rain shadow → less precipitation
            -0.3 * ((upwind_h - altitude) / 0.3).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Ocean proximity increases humidity
        let coast_bonus = (1.0 - Self::coast_distance(heightmap, x as usize, y as usize)) * 0.15;

        (base_precip * self.precipitation_scale + orographic + coast_bonus)
            .clamp(0.0, 1.0)
    }

    /// Compute ocean current warming/cooling effect at a coastal point.
    pub fn ocean_current_effect(&self, nx: f32, ny: f32) -> f32 {
        let (lat_s, lat_n) = self.latitude_range;
        let lat = lat_s + ny * (lat_n - lat_s);
        // Western boundary currents: warm on east side of ocean, cold on west
        // Simplified: longitude affects current temperature
        let warm_current = if nx > 0.5 && lat.abs() < 40.0 { 0.1 } else { 0.0 };
        let cold_current = if nx < 0.2 && lat.abs() > 20.0 { -0.08 } else { 0.0 };
        warm_current + cold_current
    }

    /// Compute normalized distance to coast (nearest land-sea boundary).
    fn coast_distance(heightmap: &HeightMap, x: usize, y: usize) -> f32 {
        let sea_level = 0.1;
        let is_land = heightmap.get(x, y) > sea_level;
        let max_search = 32usize;
        for r in 0..max_search {
            for dy in -(r as i32)..=(r as i32) {
                for dx in -(r as i32)..=(r as i32) {
                    if dx.abs() != r as i32 && dy.abs() != r as i32 { continue; }
                    let nx2 = x as i32 + dx;
                    let ny2 = y as i32 + dy;
                    if nx2 < 0 || nx2 >= heightmap.width as i32 || ny2 < 0 || ny2 >= heightmap.height as i32 { continue; }
                    let other_land = heightmap.get(nx2 as usize, ny2 as usize) > sea_level;
                    if other_land != is_land {
                        return r as f32 / max_search as f32;
                    }
                }
            }
        }
        1.0
    }

    /// Generate a full `ClimateMap` from a heightmap.
    pub fn simulate(&self, heightmap: &HeightMap) -> ClimateMap {
        let w = heightmap.width;
        let h = heightmap.height;
        let mut temperature = HeightMap::new(w, h);
        let mut humidity    = HeightMap::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let nx = x as f32 / w as f32;
                let ny = y as f32 / h as f32;
                let alt = heightmap.get(x, y);
                let t = self.temperature(nx, ny, alt)
                    + self.ocean_current_effect(nx, ny);
                let p = self.precipitation(nx, ny, alt, heightmap);
                temperature.set(x, y, t.clamp(0.0, 1.0));
                humidity.set(x, y, p.clamp(0.0, 1.0));
            }
        }
        // Slight spatial smoothing for realism
        temperature.blur(2);
        humidity.blur(2);
        ClimateMap { temperature, humidity }
    }
}

/// Output of the climate simulator: temperature and humidity maps.
#[derive(Clone, Debug)]
pub struct ClimateMap {
    pub temperature: HeightMap,
    pub humidity:    HeightMap,
}

// ── BiomeMap ──────────────────────────────────────────────────────────────────

/// A 2D map of biome assignments.
#[derive(Clone, Debug)]
pub struct BiomeMap {
    pub width:  usize,
    pub height: usize,
    pub biomes: Vec<BiomeType>,
}

impl BiomeMap {
    /// Create from explicit biome data.
    pub fn new(width: usize, height: usize, biomes: Vec<BiomeType>) -> Self {
        assert_eq!(biomes.len(), width * height);
        Self { width, height, biomes }
    }

    /// Build a biome map from a heightmap and a precomputed climate map.
    pub fn from_heightmap(heightmap: &HeightMap, climate: &ClimateMap) -> Self {
        let w = heightmap.width;
        let h = heightmap.height;
        let slope_map = heightmap.slope_map();
        let mut biomes = Vec::with_capacity(w * h);

        for y in 0..h {
            for x in 0..w {
                let altitude    = heightmap.get(x, y);
                let temperature = climate.temperature.get(x, y);
                let humidity    = climate.humidity.get(x, y);
                let slope       = slope_map.get(x, y);
                let coast_dist  = ClimateSimulator::coast_distance(heightmap, x, y);

                // Volcanic detection: steep slopes on hot spots (placeholder heuristic)
                let volcanic = altitude > 0.75 && slope > 0.7 && temperature > 0.6;

                let params = BiomeParams {
                    temperature,
                    humidity,
                    altitude,
                    slope,
                    coast_distance: coast_dist,
                    volcanic,
                };
                biomes.push(BiomeClassifier::classify(&params));
            }
        }
        Self { width: w, height: h, biomes }
    }

    /// Get the biome at integer coordinates.
    pub fn get(&self, x: usize, y: usize) -> BiomeType {
        if x < self.width && y < self.height {
            self.biomes[y * self.width + x]
        } else {
            BiomeType::Ocean
        }
    }

    /// Get blend weights for smooth biome transitions at floating-point coordinates.
    /// Returns up to 4 (biome, weight) pairs that sum to ~1.
    pub fn blend_weights(&self, x: f32, z: f32) -> Vec<(BiomeType, f32)> {
        let cx = x.clamp(0.0, (self.width  - 1) as f32);
        let cz = z.clamp(0.0, (self.height - 1) as f32);
        let x0 = cx.floor() as usize;
        let z0 = cz.floor() as usize;
        let x1 = (x0 + 1).min(self.width  - 1);
        let z1 = (z0 + 1).min(self.height - 1);
        let tx = cx - x0 as f32;
        let tz = cz - z0 as f32;

        let b00 = self.get(x0, z0);
        let b10 = self.get(x1, z0);
        let b01 = self.get(x0, z1);
        let b11 = self.get(x1, z1);

        let w00 = (1.0 - tx) * (1.0 - tz);
        let w10 = tx * (1.0 - tz);
        let w01 = (1.0 - tx) * tz;
        let w11 = tx * tz;

        // Merge duplicate biomes
        let mut result: Vec<(BiomeType, f32)> = Vec::new();
        for (b, w) in [(b00, w00), (b10, w10), (b01, w01), (b11, w11)] {
            if let Some(entry) = result.iter_mut().find(|(bt, _)| *bt == b) {
                entry.1 += w;
            } else {
                result.push((b, w));
            }
        }
        result
    }
}

// ── VegetationDensity ─────────────────────────────────────────────────────────

/// Vegetation density parameters for a biome.
#[derive(Clone, Copy, Debug, Default)]
pub struct VegetationDensity {
    /// Trees per unit area [0, 1].
    pub tree_density: f32,
    /// Grass coverage [0, 1].
    pub grass_density: f32,
    /// Rock/boulder frequency [0, 1].
    pub rock_density: f32,
    /// Shrub density [0, 1].
    pub shrub_density: f32,
    /// Flower density [0, 1].
    pub flower_density: f32,
}

impl VegetationDensity {
    /// Return the vegetation density parameters for a given biome.
    pub fn for_biome(biome: BiomeType) -> Self {
        match biome {
            BiomeType::Ocean | BiomeType::DeepOcean => Self::default(),
            BiomeType::Beach => Self {
                grass_density: 0.05, rock_density: 0.1,
                ..Default::default()
            },
            BiomeType::Desert => Self {
                tree_density: 0.02, rock_density: 0.3, shrub_density: 0.05,
                ..Default::default()
            },
            BiomeType::Savanna => Self {
                tree_density: 0.1, grass_density: 0.7, shrub_density: 0.1,
                flower_density: 0.05, ..Default::default()
            },
            BiomeType::Grassland => Self {
                tree_density: 0.05, grass_density: 0.9,
                flower_density: 0.15, rock_density: 0.05, ..Default::default()
            },
            BiomeType::Shrubland => Self {
                tree_density: 0.1, grass_density: 0.4, shrub_density: 0.6,
                rock_density: 0.1, ..Default::default()
            },
            BiomeType::TemperateForest => Self {
                tree_density: 0.7, grass_density: 0.3, shrub_density: 0.2,
                flower_density: 0.1, rock_density: 0.05,
            },
            BiomeType::TropicalForest => Self {
                tree_density: 0.9, grass_density: 0.2, shrub_density: 0.4,
                flower_density: 0.3, rock_density: 0.02,
            },
            BiomeType::Boreal => Self {
                tree_density: 0.6, grass_density: 0.1, shrub_density: 0.15,
                rock_density: 0.1, ..Default::default()
            },
            BiomeType::Taiga => Self {
                tree_density: 0.5, grass_density: 0.05, shrub_density: 0.1,
                rock_density: 0.15, ..Default::default()
            },
            BiomeType::Tundra => Self {
                tree_density: 0.01, grass_density: 0.3, shrub_density: 0.15,
                rock_density: 0.3, flower_density: 0.05,
            },
            BiomeType::Arctic => Self {
                rock_density: 0.4, ..Default::default()
            },
            BiomeType::Mountain => Self {
                tree_density: 0.15, grass_density: 0.2, rock_density: 0.6,
                shrub_density: 0.1, ..Default::default()
            },
            BiomeType::AlpineGlacier => Self {
                rock_density: 0.2, ..Default::default()
            },
            BiomeType::Swamp => Self {
                tree_density: 0.5, grass_density: 0.4, shrub_density: 0.3,
                flower_density: 0.05, rock_density: 0.01,
            },
            BiomeType::Mangrove => Self {
                tree_density: 0.6, grass_density: 0.1, shrub_density: 0.2,
                ..Default::default()
            },
            BiomeType::Volcanic => Self {
                rock_density: 0.8, ..Default::default()
            },
            BiomeType::Badlands => Self {
                grass_density: 0.05, rock_density: 0.5, shrub_density: 0.05,
                ..Default::default()
            },
            BiomeType::Mushroom => Self {
                tree_density: 0.05, grass_density: 0.6, shrub_density: 0.2,
                flower_density: 0.4, rock_density: 0.05,
            },
        }
    }
}

// ── BiomeColor ────────────────────────────────────────────────────────────────

/// Color palette for a biome.
#[derive(Clone, Copy, Debug)]
pub struct BiomeColor {
    /// Primary ground/soil color.
    pub ground: Vec3,
    /// Grass tint color.
    pub grass:  Vec3,
    /// Sky tint color (affects fog/atmosphere).
    pub sky:    Vec3,
    /// Water color (if applicable).
    pub water:  Vec3,
    /// Rock color.
    pub rock:   Vec3,
}

impl BiomeColor {
    /// Color palette for a given biome type.
    pub fn for_biome(biome: BiomeType) -> Self {
        match biome {
            BiomeType::Ocean => Self {
                ground: Vec3::new(0.05, 0.1,  0.3),
                grass:  Vec3::new(0.0,  0.3,  0.5),
                sky:    Vec3::new(0.4,  0.65, 0.9),
                water:  Vec3::new(0.0,  0.2,  0.8),
                rock:   Vec3::new(0.3,  0.3,  0.4),
            },
            BiomeType::DeepOcean => Self {
                ground: Vec3::new(0.02, 0.04, 0.2),
                grass:  Vec3::new(0.0,  0.1,  0.3),
                sky:    Vec3::new(0.3,  0.5,  0.8),
                water:  Vec3::new(0.0,  0.1,  0.6),
                rock:   Vec3::new(0.2,  0.2,  0.3),
            },
            BiomeType::Beach => Self {
                ground: Vec3::new(0.87, 0.80, 0.55),
                grass:  Vec3::new(0.7,  0.75, 0.3),
                sky:    Vec3::new(0.5,  0.75, 0.95),
                water:  Vec3::new(0.1,  0.5,  0.9),
                rock:   Vec3::new(0.6,  0.55, 0.45),
            },
            BiomeType::Desert => Self {
                ground: Vec3::new(0.85, 0.65, 0.3),
                grass:  Vec3::new(0.7,  0.6,  0.25),
                sky:    Vec3::new(0.9,  0.75, 0.45),
                water:  Vec3::new(0.3,  0.5,  0.8),
                rock:   Vec3::new(0.75, 0.55, 0.35),
            },
            BiomeType::Savanna => Self {
                ground: Vec3::new(0.75, 0.6,  0.25),
                grass:  Vec3::new(0.7,  0.65, 0.2),
                sky:    Vec3::new(0.7,  0.8,  0.9),
                water:  Vec3::new(0.2,  0.5,  0.8),
                rock:   Vec3::new(0.65, 0.55, 0.4),
            },
            BiomeType::Grassland => Self {
                ground: Vec3::new(0.45, 0.5,  0.2),
                grass:  Vec3::new(0.35, 0.6,  0.15),
                sky:    Vec3::new(0.5,  0.7,  0.95),
                water:  Vec3::new(0.15, 0.45, 0.8),
                rock:   Vec3::new(0.5,  0.5,  0.45),
            },
            BiomeType::Shrubland => Self {
                ground: Vec3::new(0.5,  0.45, 0.25),
                grass:  Vec3::new(0.4,  0.5,  0.2),
                sky:    Vec3::new(0.55, 0.7,  0.9),
                water:  Vec3::new(0.1,  0.4,  0.75),
                rock:   Vec3::new(0.55, 0.5,  0.4),
            },
            BiomeType::TemperateForest => Self {
                ground: Vec3::new(0.3,  0.35, 0.15),
                grass:  Vec3::new(0.25, 0.55, 0.15),
                sky:    Vec3::new(0.45, 0.65, 0.85),
                water:  Vec3::new(0.1,  0.35, 0.7),
                rock:   Vec3::new(0.45, 0.45, 0.4),
            },
            BiomeType::TropicalForest => Self {
                ground: Vec3::new(0.2,  0.3,  0.1),
                grass:  Vec3::new(0.15, 0.55, 0.1),
                sky:    Vec3::new(0.5,  0.7,  0.75),
                water:  Vec3::new(0.05, 0.4,  0.6),
                rock:   Vec3::new(0.35, 0.4,  0.3),
            },
            BiomeType::Boreal => Self {
                ground: Vec3::new(0.3,  0.35, 0.2),
                grass:  Vec3::new(0.2,  0.45, 0.2),
                sky:    Vec3::new(0.55, 0.65, 0.8),
                water:  Vec3::new(0.1,  0.3,  0.65),
                rock:   Vec3::new(0.4,  0.42, 0.38),
            },
            BiomeType::Taiga => Self {
                ground: Vec3::new(0.35, 0.35, 0.25),
                grass:  Vec3::new(0.25, 0.4,  0.25),
                sky:    Vec3::new(0.6,  0.65, 0.8),
                water:  Vec3::new(0.1,  0.3,  0.6),
                rock:   Vec3::new(0.45, 0.45, 0.4),
            },
            BiomeType::Tundra => Self {
                ground: Vec3::new(0.55, 0.5,  0.4),
                grass:  Vec3::new(0.5,  0.55, 0.3),
                sky:    Vec3::new(0.7,  0.75, 0.85),
                water:  Vec3::new(0.1,  0.3,  0.6),
                rock:   Vec3::new(0.55, 0.52, 0.48),
            },
            BiomeType::Arctic => Self {
                ground: Vec3::new(0.9,  0.92, 0.95),
                grass:  Vec3::new(0.85, 0.88, 0.92),
                sky:    Vec3::new(0.7,  0.8,  0.95),
                water:  Vec3::new(0.6,  0.75, 0.9),
                rock:   Vec3::new(0.6,  0.62, 0.65),
            },
            BiomeType::Mountain => Self {
                ground: Vec3::new(0.5,  0.48, 0.44),
                grass:  Vec3::new(0.35, 0.45, 0.25),
                sky:    Vec3::new(0.55, 0.65, 0.85),
                water:  Vec3::new(0.1,  0.3,  0.7),
                rock:   Vec3::new(0.55, 0.52, 0.48),
            },
            BiomeType::AlpineGlacier => Self {
                ground: Vec3::new(0.85, 0.9,  0.95),
                grass:  Vec3::new(0.8,  0.85, 0.9),
                sky:    Vec3::new(0.65, 0.75, 0.95),
                water:  Vec3::new(0.7,  0.85, 0.95),
                rock:   Vec3::new(0.6,  0.62, 0.65),
            },
            BiomeType::Swamp => Self {
                ground: Vec3::new(0.25, 0.3,  0.15),
                grass:  Vec3::new(0.2,  0.4,  0.15),
                sky:    Vec3::new(0.45, 0.55, 0.65),
                water:  Vec3::new(0.1,  0.2,  0.25),
                rock:   Vec3::new(0.3,  0.32, 0.28),
            },
            BiomeType::Mangrove => Self {
                ground: Vec3::new(0.3,  0.35, 0.2),
                grass:  Vec3::new(0.2,  0.5,  0.15),
                sky:    Vec3::new(0.5,  0.65, 0.8),
                water:  Vec3::new(0.1,  0.3,  0.5),
                rock:   Vec3::new(0.35, 0.38, 0.3),
            },
            BiomeType::Volcanic => Self {
                ground: Vec3::new(0.15, 0.1,  0.08),
                grass:  Vec3::new(0.2,  0.18, 0.1),
                sky:    Vec3::new(0.5,  0.35, 0.25),
                water:  Vec3::new(0.8,  0.4,  0.05),
                rock:   Vec3::new(0.1,  0.08, 0.07),
            },
            BiomeType::Badlands => Self {
                ground: Vec3::new(0.75, 0.45, 0.25),
                grass:  Vec3::new(0.6,  0.45, 0.2),
                sky:    Vec3::new(0.8,  0.65, 0.45),
                water:  Vec3::new(0.25, 0.45, 0.75),
                rock:   Vec3::new(0.7,  0.5,  0.3),
            },
            BiomeType::Mushroom => Self {
                ground: Vec3::new(0.55, 0.3,  0.55),
                grass:  Vec3::new(0.5,  0.2,  0.6),
                sky:    Vec3::new(0.6,  0.5,  0.8),
                water:  Vec3::new(0.4,  0.2,  0.7),
                rock:   Vec3::new(0.45, 0.3,  0.5),
            },
        }
    }
}

// ── TransitionZone ────────────────────────────────────────────────────────────

/// Describes the blend zone between two adjacent biomes.
#[derive(Clone, Debug)]
pub struct TransitionZone {
    pub biome_a: BiomeType,
    pub biome_b: BiomeType,
    /// Blend width in world units.
    pub blend_width: f32,
    /// Whether the transition has a distinct visual marker (e.g. treeline).
    pub sharp_boundary: bool,
}

impl TransitionZone {
    pub fn new(biome_a: BiomeType, biome_b: BiomeType, blend_width: f32) -> Self {
        let sharp = matches!(
            (biome_a, biome_b),
            (BiomeType::Grassland, BiomeType::Desert)   |
            (BiomeType::Desert,    BiomeType::Grassland) |
            (BiomeType::Mountain,  BiomeType::AlpineGlacier) |
            (BiomeType::AlpineGlacier, BiomeType::Mountain)
        );
        Self { biome_a, biome_b, blend_width, sharp_boundary: sharp }
    }

    /// Compute the blend factor from biome_a to biome_b.
    /// `position` is 0.0 at biome_a center and 1.0 at biome_b center.
    pub fn blend_factor(&self, position: f32) -> f32 {
        let t = position.clamp(0.0, 1.0);
        if self.sharp_boundary {
            if t < 0.5 { 0.0 } else { 1.0 }
        } else {
            // Smooth sigmoid
            let x = t * 2.0 - 1.0;
            0.5 + x * (1.0 - x.abs() * 0.5) * 0.5
        }
    }
}

// ── Seasonal Variation ────────────────────────────────────────────────────────

/// Describes how a biome changes by season.
#[derive(Clone, Copy, Debug)]
pub struct SeasonFactor {
    /// Color multiplier for vegetation (0 = dead/snow-covered, 1 = full green).
    pub vegetation_green:  f32,
    /// Color shift toward autumn browns/oranges.
    pub autumn_shift:      f32,
    /// Snow cover [0, 1].
    pub snow_cover:        f32,
    /// Effective vegetation density multiplier.
    pub density_scale:     f32,
}

impl SeasonFactor {
    /// Compute seasonal factor for a biome in month 0–11.
    pub fn season_factor(biome: BiomeType, month: u32) -> Self {
        let month = (month % 12) as f32;
        // Northern hemisphere seasons: summer peak = month 6
        let summer_t = ((month - 6.0) * std::f32::consts::PI / 6.0).cos() * 0.5 + 0.5;
        // summer_t: 1.0 = peak summer, 0.0 = peak winter
        let winter_t = 1.0 - summer_t;

        match biome {
            BiomeType::TemperateForest | BiomeType::Boreal => Self {
                vegetation_green: 0.2 + summer_t * 0.8,
                autumn_shift: if month > 7.0 && month < 11.0 { (month - 7.0) * 0.25 } else { 0.0 },
                snow_cover: (winter_t - 0.6).max(0.0) * 2.5,
                density_scale: 0.3 + summer_t * 0.7,
            },
            BiomeType::Taiga | BiomeType::Tundra => Self {
                vegetation_green: 0.1 + summer_t * 0.7,
                autumn_shift: 0.0,
                snow_cover: winter_t * 0.9,
                density_scale: 0.1 + summer_t * 0.6,
            },
            BiomeType::Arctic | BiomeType::AlpineGlacier => Self {
                vegetation_green: summer_t * 0.2,
                autumn_shift: 0.0,
                snow_cover: 0.5 + winter_t * 0.5,
                density_scale: summer_t * 0.15,
            },
            BiomeType::Grassland | BiomeType::Savanna => Self {
                vegetation_green: 0.4 + summer_t * 0.5,
                autumn_shift: (winter_t - 0.3).max(0.0) * 0.5,
                snow_cover: (winter_t - 0.8).max(0.0) * 2.0,
                density_scale: 0.5 + summer_t * 0.5,
            },
            BiomeType::Desert | BiomeType::Badlands => Self {
                vegetation_green: 0.1,
                autumn_shift: 0.0,
                snow_cover: 0.0,
                density_scale: 0.8 + summer_t * 0.2,
            },
            // Tropical/equatorial biomes: minimal seasonality
            BiomeType::TropicalForest | BiomeType::Mangrove | BiomeType::Swamp => Self {
                vegetation_green: 0.9,
                autumn_shift: 0.0,
                snow_cover: 0.0,
                density_scale: 1.0,
            },
            _ => Self {
                vegetation_green: 0.5 + summer_t * 0.5,
                autumn_shift: 0.0,
                snow_cover: winter_t * 0.3,
                density_scale: 0.6 + summer_t * 0.4,
            },
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::heightmap::FractalNoise;

    #[test]
    fn test_biome_type_names() {
        assert_eq!(BiomeType::Desert.name(), "Desert");
        assert_eq!(BiomeType::TropicalForest.name(), "Tropical Forest");
        assert_eq!(BiomeType::AlpineGlacier.name(), "Alpine Glacier");
    }

    #[test]
    fn test_biome_type_properties() {
        assert!(BiomeType::Ocean.is_aquatic());
        assert!(!BiomeType::Desert.is_aquatic());
        assert!(BiomeType::Arctic.is_cold());
        assert!(!BiomeType::Desert.is_cold());
        assert!(BiomeType::TropicalForest.has_trees());
        assert!(!BiomeType::Arctic.has_trees());
    }

    #[test]
    fn test_biome_classifier_desert() {
        let p = BiomeParams {
            temperature: 0.8, humidity: 0.1, altitude: 0.3, slope: 0.05,
            coast_distance: 0.9, volcanic: false,
        };
        assert_eq!(BiomeClassifier::classify(&p), BiomeType::Desert);
    }

    #[test]
    fn test_biome_classifier_ocean() {
        let p = BiomeParams {
            temperature: 0.5, humidity: 0.8, altitude: 0.01, slope: 0.0,
            coast_distance: 0.0, volcanic: false,
        };
        assert!(matches!(
            BiomeClassifier::classify(&p),
            BiomeType::Ocean | BiomeType::DeepOcean
        ));
    }

    #[test]
    fn test_biome_classifier_alpine() {
        let p = BiomeParams {
            temperature: 0.2, humidity: 0.3, altitude: 0.96, slope: 0.3,
            coast_distance: 0.8, volcanic: false,
        };
        assert_eq!(BiomeClassifier::classify(&p), BiomeType::AlpineGlacier);
    }

    #[test]
    fn test_biome_classifier_tropical() {
        let p = BiomeParams {
            temperature: 0.9, humidity: 0.9, altitude: 0.4, slope: 0.05,
            coast_distance: 0.5, volcanic: false,
        };
        assert_eq!(BiomeClassifier::classify(&p), BiomeType::TropicalForest);
    }

    #[test]
    fn test_biome_classifier_volcanic() {
        let p = BiomeParams {
            temperature: 0.7, humidity: 0.2, altitude: 0.8, slope: 0.75,
            coast_distance: 0.7, volcanic: true,
        };
        assert_eq!(BiomeClassifier::classify(&p), BiomeType::Volcanic);
    }

    #[test]
    fn test_climate_simulator() {
        let hm = FractalNoise::generate(32, 32, 4, 2.0, 0.5, 3.0, 42);
        let sim = ClimateSimulator::default();
        let climate = sim.simulate(&hm);
        assert_eq!(climate.temperature.data.len(), 32 * 32);
        assert_eq!(climate.humidity.data.len(), 32 * 32);
        assert!(climate.temperature.min_value() >= 0.0);
        assert!(climate.temperature.max_value() <= 1.0);
    }

    #[test]
    fn test_biome_map_from_heightmap() {
        let hm = FractalNoise::generate(32, 32, 4, 2.0, 0.5, 3.0, 42);
        let sim = ClimateSimulator::default();
        let climate = sim.simulate(&hm);
        let bm = BiomeMap::from_heightmap(&hm, &climate);
        assert_eq!(bm.biomes.len(), 32 * 32);
    }

    #[test]
    fn test_biome_map_blend_weights() {
        let hm = FractalNoise::generate(32, 32, 4, 2.0, 0.5, 3.0, 42);
        let sim = ClimateSimulator::default();
        let climate = sim.simulate(&hm);
        let bm = BiomeMap::from_heightmap(&hm, &climate);
        let weights = bm.blend_weights(16.5, 16.5);
        let total: f32 = weights.iter().map(|(_, w)| w).sum();
        assert!((total - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_vegetation_density() {
        let d = VegetationDensity::for_biome(BiomeType::TropicalForest);
        assert!(d.tree_density > 0.5);
        let d2 = VegetationDensity::for_biome(BiomeType::Arctic);
        assert!(d2.tree_density < 0.1);
    }

    #[test]
    fn test_biome_colors_all_defined() {
        let all = [
            BiomeType::Ocean, BiomeType::DeepOcean, BiomeType::Beach,
            BiomeType::Desert, BiomeType::Savanna, BiomeType::Grassland,
            BiomeType::Shrubland, BiomeType::TemperateForest, BiomeType::TropicalForest,
            BiomeType::Boreal, BiomeType::Taiga, BiomeType::Tundra,
            BiomeType::Arctic, BiomeType::Mountain, BiomeType::AlpineGlacier,
            BiomeType::Swamp, BiomeType::Mangrove, BiomeType::Volcanic,
            BiomeType::Badlands, BiomeType::Mushroom,
        ];
        for biome in all {
            let color = BiomeColor::for_biome(biome);
            // All color components in [0, 1]
            assert!(color.ground.x >= 0.0 && color.ground.x <= 1.0);
        }
    }

    #[test]
    fn test_season_factor() {
        let summer = SeasonFactor::season_factor(BiomeType::TemperateForest, 6);
        let winter = SeasonFactor::season_factor(BiomeType::TemperateForest, 0);
        assert!(summer.vegetation_green > winter.vegetation_green);
        assert!(winter.snow_cover >= summer.snow_cover);
    }

    #[test]
    fn test_transition_zone() {
        let tz = TransitionZone::new(BiomeType::Grassland, BiomeType::Desert, 10.0);
        assert!((tz.blend_factor(0.0) - 0.0).abs() < 0.01);
        let mid = tz.blend_factor(0.5);
        assert!(mid > 0.0 && mid < 1.0);
    }
}
