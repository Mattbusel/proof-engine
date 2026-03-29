#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
//  FUNDAMENTAL CONSTANTS
// ============================================================

pub const PI: f32      = std::f32::consts::PI;
pub const TWO_PI: f32  = 2.0 * PI;
pub const HALF_PI: f32 = PI * 0.5;
pub const DEG2RAD: f32 = PI / 180.0;
pub const RAD2DEG: f32 = 180.0 / PI;
pub const SQRT3: f32   = 1.732_050_8;
pub const F3: f32      = 1.0 / 3.0;
pub const G3: f32      = 1.0 / 6.0;

// Atmospheric physics constants
pub const RAYLEIGH_SCALE_HEIGHT: f64 = 8.5;    // km
pub const MIE_SCALE_HEIGHT: f64      = 1.2;    // km
pub const RAYLEIGH_R: f64            = 5.8e-6;
pub const RAYLEIGH_G: f64            = 13.5e-6;
pub const RAYLEIGH_B: f64            = 33.1e-6;
pub const MIE_COEFF: f64             = 21.0e-6;
pub const MIE_G: f64                 = 0.758;  // asymmetry factor
pub const EARTH_RADIUS: f64          = 6371.0; // km
pub const ATMO_RADIUS: f64           = 6471.0; // km (100 km atmosphere)

// Solar
pub const SOLAR_OBLIQUITY: f64 = 23.45; // degrees

// Hydraulic erosion defaults
pub const EROSION_INERTIA: f32            = 0.05;
pub const EROSION_CAPACITY: f32           = 4.0;
pub const EROSION_DEPOSITION: f32         = 0.3;
pub const EROSION_EROSION_SPEED: f32      = 0.3;
pub const EROSION_EVAPORATION: f32        = 0.02;
pub const EROSION_MIN_SLOPE: f32          = 0.01;
pub const EROSION_GRAVITY: f32            = 4.0;
pub const EROSION_MAX_STEPS: usize        = 64;

// ============================================================
//  PERMUTATION TABLE (512 elements for Perlin/Simplex noise)
// ============================================================

pub const PERM: [u8; 512] = [
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
];

/// 3-D gradient vectors (Perlin)
pub const GRAD3: [[f32; 3]; 16] = [
    [ 1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [ 1.0,-1.0, 0.0], [-1.0,-1.0, 0.0],
    [ 1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [ 1.0, 0.0,-1.0], [-1.0, 0.0,-1.0],
    [ 0.0, 1.0, 1.0], [ 0.0,-1.0, 1.0], [ 0.0, 1.0,-1.0], [ 0.0,-1.0,-1.0],
    [ 1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [ 0.0,-1.0, 1.0], [ 0.0,-1.0,-1.0],
];

// Simplex noise 4-D gradient
pub const GRAD4: [[f32; 4]; 32] = [
    [ 0.0, 1.0, 1.0, 1.0],[ 0.0, 1.0, 1.0,-1.0],[ 0.0, 1.0,-1.0, 1.0],[ 0.0, 1.0,-1.0,-1.0],
    [ 0.0,-1.0, 1.0, 1.0],[ 0.0,-1.0, 1.0,-1.0],[ 0.0,-1.0,-1.0, 1.0],[ 0.0,-1.0,-1.0,-1.0],
    [ 1.0, 0.0, 1.0, 1.0],[ 1.0, 0.0, 1.0,-1.0],[ 1.0, 0.0,-1.0, 1.0],[ 1.0, 0.0,-1.0,-1.0],
    [-1.0, 0.0, 1.0, 1.0],[-1.0, 0.0, 1.0,-1.0],[-1.0, 0.0,-1.0, 1.0],[-1.0, 0.0,-1.0,-1.0],
    [ 1.0, 1.0, 0.0, 1.0],[ 1.0, 1.0, 0.0,-1.0],[ 1.0,-1.0, 0.0, 1.0],[ 1.0,-1.0, 0.0,-1.0],
    [-1.0, 1.0, 0.0, 1.0],[-1.0, 1.0, 0.0,-1.0],[-1.0,-1.0, 0.0, 1.0],[-1.0,-1.0, 0.0,-1.0],
    [ 1.0, 1.0, 1.0, 0.0],[ 1.0, 1.0,-1.0, 0.0],[ 1.0,-1.0, 1.0, 0.0],[ 1.0,-1.0,-1.0, 0.0],
    [-1.0, 1.0, 1.0, 0.0],[-1.0, 1.0,-1.0, 0.0],[-1.0,-1.0, 1.0, 0.0],[-1.0,-1.0,-1.0, 0.0],
];

// ============================================================
//  BIOME SYSTEM
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeId {
    TropicalRainforest    = 0,
    TropicalSavanna       = 1,
    HotDesert             = 2,
    ColdDesert            = 3,
    XericShrubland        = 4,
    MediterraneanShrub    = 5,
    TemperateGrassland    = 6,
    TemperateRainforest   = 7,
    TemperateDeciduous    = 8,
    BorealForest          = 9,
    TaigaSpruce           = 10,
    Tundra                = 11,
    ArcticDesert          = 12,
    AlpineMeadow          = 13,
    AlpineTundra          = 14,
    PolarIceCap           = 15,
    Mangrove              = 16,
    Wetland               = 17,
    FloodPlain            = 18,
    VolcanicLandscape     = 19,
    SaltFlat              = 20,
    GlacialValley         = 21,
    CoastalDunes          = 22,
    DeepOceanFloor        = 23,
    CoralReef             = 24,
}

#[derive(Clone, Debug)]
pub struct BiomeDescriptor {
    pub id: BiomeId,
    pub name: &'static str,
    pub temp_min: f32,
    pub temp_max: f32,
    pub humidity_min: f32,
    pub humidity_max: f32,
    pub alt_min: f32,
    pub alt_max: f32,
    pub ground_color: Vec3,
    pub tree_density: f32,
    pub grass_density: f32,
    pub rock_density: f32,
    pub snow_coverage: f32,
    pub rainfall_mm: f32,
    pub wind_speed_ms: f32,
    pub fog_density: f32,
}

impl BiomeDescriptor {
    pub fn classify_point(temp: f32, humidity: f32, altitude: f32) -> BiomeId {
        // Altitude override first
        if altitude > 0.88 {
            return BiomeId::PolarIceCap;
        }
        if altitude > 0.75 {
            return BiomeId::AlpineTundra;
        }
        if altitude > 0.62 {
            return BiomeId::AlpineMeadow;
        }

        // Temperature + humidity classification
        if temp > 24.0 {
            if humidity > 0.80 {
                return BiomeId::TropicalRainforest;
            } else if humidity > 0.50 {
                return BiomeId::TropicalSavanna;
            } else if humidity > 0.25 {
                return BiomeId::XericShrubland;
            } else {
                return BiomeId::HotDesert;
            }
        } else if temp > 10.0 {
            if humidity > 0.70 {
                return BiomeId::TemperateRainforest;
            } else if humidity > 0.50 {
                return BiomeId::TemperateDeciduous;
            } else if humidity > 0.28 {
                return BiomeId::MediterraneanShrub;
            } else {
                return BiomeId::XericShrubland;
            }
        } else if temp > 0.0 {
            if humidity > 0.65 {
                return BiomeId::BorealForest;
            } else if humidity > 0.40 {
                return BiomeId::TemperateGrassland;
            } else {
                return BiomeId::ColdDesert;
            }
        } else if temp > -10.0 {
            if humidity > 0.50 {
                return BiomeId::TaigaSpruce;
            } else {
                return BiomeId::Tundra;
            }
        } else {
            if humidity > 0.30 {
                return BiomeId::Tundra;
            } else {
                return BiomeId::ArcticDesert;
            }
        }
    }

    /// Compute a blend weight for this biome at a given (temp, humidity, altitude)
    pub fn blend_weight(&self, temp: f32, humidity: f32, altitude: f32) -> f32 {
        let temp_w     = gaussian_falloff(temp,     (self.temp_min     + self.temp_max)     * 0.5, (self.temp_max     - self.temp_min)     * 0.5 + 0.5);
        let hum_w      = gaussian_falloff(humidity, (self.humidity_min + self.humidity_max) * 0.5, (self.humidity_max - self.humidity_min) * 0.5 + 0.05);
        let alt_w      = gaussian_falloff(altitude, (self.alt_min      + self.alt_max)      * 0.5, (self.alt_max      - self.alt_min)      * 0.5 + 0.05);
        (temp_w * hum_w * alt_w).max(0.0)
    }
}

fn gaussian_falloff(x: f32, center: f32, sigma: f32) -> f32 {
    let diff = x - center;
    (-(diff * diff) / (2.0 * sigma * sigma)).exp()
}

/// Build the full 25-biome table at runtime
pub fn build_biome_table() -> Vec<BiomeDescriptor> {
    vec![
        BiomeDescriptor {
            id: BiomeId::TropicalRainforest, name: "Tropical Rainforest",
            temp_min: 24.0, temp_max: 36.0, humidity_min: 0.80, humidity_max: 1.00,
            alt_min: 0.00, alt_max: 0.30, ground_color: Vec3::new(0.04, 0.35, 0.06),
            tree_density: 0.95, grass_density: 0.60, rock_density: 0.03,
            snow_coverage: 0.00, rainfall_mm: 3000.0, wind_speed_ms: 2.0, fog_density: 0.15,
        },
        BiomeDescriptor {
            id: BiomeId::TropicalSavanna, name: "Tropical Savanna",
            temp_min: 20.0, temp_max: 35.0, humidity_min: 0.25, humidity_max: 0.55,
            alt_min: 0.00, alt_max: 0.30, ground_color: Vec3::new(0.62, 0.55, 0.14),
            tree_density: 0.18, grass_density: 0.85, rock_density: 0.10,
            snow_coverage: 0.00, rainfall_mm: 900.0, wind_speed_ms: 4.0, fog_density: 0.02,
        },
        BiomeDescriptor {
            id: BiomeId::HotDesert, name: "Hot Desert",
            temp_min: 20.0, temp_max: 52.0, humidity_min: 0.00, humidity_max: 0.18,
            alt_min: 0.00, alt_max: 0.35, ground_color: Vec3::new(0.87, 0.79, 0.41),
            tree_density: 0.01, grass_density: 0.04, rock_density: 0.40,
            snow_coverage: 0.00, rainfall_mm: 80.0, wind_speed_ms: 7.0, fog_density: 0.00,
        },
        BiomeDescriptor {
            id: BiomeId::ColdDesert, name: "Cold Desert",
            temp_min: -10.0, temp_max: 15.0, humidity_min: 0.00, humidity_max: 0.20,
            alt_min: 0.00, alt_max: 0.45, ground_color: Vec3::new(0.70, 0.65, 0.50),
            tree_density: 0.02, grass_density: 0.10, rock_density: 0.50,
            snow_coverage: 0.10, rainfall_mm: 150.0, wind_speed_ms: 8.0, fog_density: 0.01,
        },
        BiomeDescriptor {
            id: BiomeId::XericShrubland, name: "Xeric Shrubland",
            temp_min: 10.0, temp_max: 30.0, humidity_min: 0.10, humidity_max: 0.30,
            alt_min: 0.00, alt_max: 0.40, ground_color: Vec3::new(0.70, 0.65, 0.30),
            tree_density: 0.05, grass_density: 0.40, rock_density: 0.30,
            snow_coverage: 0.00, rainfall_mm: 300.0, wind_speed_ms: 5.0, fog_density: 0.01,
        },
        BiomeDescriptor {
            id: BiomeId::MediterraneanShrub, name: "Mediterranean Shrubland",
            temp_min: 5.0, temp_max: 28.0, humidity_min: 0.25, humidity_max: 0.50,
            alt_min: 0.00, alt_max: 0.40, ground_color: Vec3::new(0.55, 0.62, 0.20),
            tree_density: 0.25, grass_density: 0.55, rock_density: 0.20,
            snow_coverage: 0.00, rainfall_mm: 600.0, wind_speed_ms: 4.0, fog_density: 0.03,
        },
        BiomeDescriptor {
            id: BiomeId::TemperateGrassland, name: "Temperate Grassland",
            temp_min: -5.0, temp_max: 20.0, humidity_min: 0.20, humidity_max: 0.50,
            alt_min: 0.00, alt_max: 0.45, ground_color: Vec3::new(0.50, 0.70, 0.15),
            tree_density: 0.05, grass_density: 0.90, rock_density: 0.05,
            snow_coverage: 0.05, rainfall_mm: 500.0, wind_speed_ms: 5.5, fog_density: 0.05,
        },
        BiomeDescriptor {
            id: BiomeId::TemperateRainforest, name: "Temperate Rainforest",
            temp_min: 5.0, temp_max: 20.0, humidity_min: 0.70, humidity_max: 1.00,
            alt_min: 0.00, alt_max: 0.50, ground_color: Vec3::new(0.10, 0.40, 0.10),
            tree_density: 0.85, grass_density: 0.50, rock_density: 0.08,
            snow_coverage: 0.00, rainfall_mm: 2500.0, wind_speed_ms: 3.0, fog_density: 0.20,
        },
        BiomeDescriptor {
            id: BiomeId::TemperateDeciduous, name: "Temperate Deciduous Forest",
            temp_min: 5.0, temp_max: 22.0, humidity_min: 0.50, humidity_max: 0.75,
            alt_min: 0.00, alt_max: 0.50, ground_color: Vec3::new(0.20, 0.50, 0.10),
            tree_density: 0.70, grass_density: 0.35, rock_density: 0.10,
            snow_coverage: 0.05, rainfall_mm: 1100.0, wind_speed_ms: 3.5, fog_density: 0.08,
        },
        BiomeDescriptor {
            id: BiomeId::BorealForest, name: "Boreal Forest",
            temp_min: -10.0, temp_max: 10.0, humidity_min: 0.45, humidity_max: 0.70,
            alt_min: 0.00, alt_max: 0.55, ground_color: Vec3::new(0.15, 0.35, 0.12),
            tree_density: 0.75, grass_density: 0.20, rock_density: 0.12,
            snow_coverage: 0.25, rainfall_mm: 700.0, wind_speed_ms: 4.0, fog_density: 0.10,
        },
        BiomeDescriptor {
            id: BiomeId::TaigaSpruce, name: "Taiga Spruce",
            temp_min: -20.0, temp_max: 5.0, humidity_min: 0.40, humidity_max: 0.65,
            alt_min: 0.00, alt_max: 0.60, ground_color: Vec3::new(0.12, 0.28, 0.12),
            tree_density: 0.65, grass_density: 0.15, rock_density: 0.15,
            snow_coverage: 0.45, rainfall_mm: 550.0, wind_speed_ms: 5.0, fog_density: 0.12,
        },
        BiomeDescriptor {
            id: BiomeId::Tundra, name: "Tundra",
            temp_min: -25.0, temp_max: 0.0, humidity_min: 0.20, humidity_max: 0.55,
            alt_min: 0.00, alt_max: 0.65, ground_color: Vec3::new(0.45, 0.50, 0.30),
            tree_density: 0.02, grass_density: 0.50, rock_density: 0.30,
            snow_coverage: 0.60, rainfall_mm: 280.0, wind_speed_ms: 8.0, fog_density: 0.15,
        },
        BiomeDescriptor {
            id: BiomeId::ArcticDesert, name: "Arctic Desert",
            temp_min: -40.0, temp_max: -10.0, humidity_min: 0.00, humidity_max: 0.20,
            alt_min: 0.00, alt_max: 0.70, ground_color: Vec3::new(0.80, 0.85, 0.90),
            tree_density: 0.00, grass_density: 0.02, rock_density: 0.20,
            snow_coverage: 0.90, rainfall_mm: 100.0, wind_speed_ms: 12.0, fog_density: 0.10,
        },
        BiomeDescriptor {
            id: BiomeId::AlpineMeadow, name: "Alpine Meadow",
            temp_min: -5.0, temp_max: 12.0, humidity_min: 0.40, humidity_max: 0.75,
            alt_min: 0.58, alt_max: 0.75, ground_color: Vec3::new(0.35, 0.60, 0.20),
            tree_density: 0.10, grass_density: 0.75, rock_density: 0.25,
            snow_coverage: 0.20, rainfall_mm: 800.0, wind_speed_ms: 6.0, fog_density: 0.08,
        },
        BiomeDescriptor {
            id: BiomeId::AlpineTundra, name: "Alpine Tundra",
            temp_min: -15.0, temp_max: 5.0, humidity_min: 0.20, humidity_max: 0.60,
            alt_min: 0.72, alt_max: 0.88, ground_color: Vec3::new(0.40, 0.42, 0.38),
            tree_density: 0.00, grass_density: 0.30, rock_density: 0.60,
            snow_coverage: 0.50, rainfall_mm: 500.0, wind_speed_ms: 10.0, fog_density: 0.12,
        },
        BiomeDescriptor {
            id: BiomeId::PolarIceCap, name: "Polar Ice Cap",
            temp_min: -50.0, temp_max: -5.0, humidity_min: 0.00, humidity_max: 0.30,
            alt_min: 0.85, alt_max: 1.00, ground_color: Vec3::new(0.92, 0.95, 1.00),
            tree_density: 0.00, grass_density: 0.00, rock_density: 0.05,
            snow_coverage: 1.00, rainfall_mm: 50.0, wind_speed_ms: 15.0, fog_density: 0.20,
        },
        BiomeDescriptor {
            id: BiomeId::Mangrove, name: "Mangrove",
            temp_min: 20.0, temp_max: 35.0, humidity_min: 0.70, humidity_max: 1.00,
            alt_min: 0.00, alt_max: 0.08, ground_color: Vec3::new(0.20, 0.35, 0.10),
            tree_density: 0.70, grass_density: 0.30, rock_density: 0.02,
            snow_coverage: 0.00, rainfall_mm: 2000.0, wind_speed_ms: 2.0, fog_density: 0.25,
        },
        BiomeDescriptor {
            id: BiomeId::Wetland, name: "Wetland",
            temp_min: 0.0, temp_max: 25.0, humidity_min: 0.75, humidity_max: 1.00,
            alt_min: 0.00, alt_max: 0.15, ground_color: Vec3::new(0.18, 0.32, 0.10),
            tree_density: 0.30, grass_density: 0.80, rock_density: 0.02,
            snow_coverage: 0.00, rainfall_mm: 1400.0, wind_speed_ms: 2.0, fog_density: 0.30,
        },
        BiomeDescriptor {
            id: BiomeId::FloodPlain, name: "Flood Plain",
            temp_min: 10.0, temp_max: 30.0, humidity_min: 0.55, humidity_max: 0.85,
            alt_min: 0.00, alt_max: 0.12, ground_color: Vec3::new(0.40, 0.55, 0.15),
            tree_density: 0.15, grass_density: 0.85, rock_density: 0.03,
            snow_coverage: 0.00, rainfall_mm: 1200.0, wind_speed_ms: 3.0, fog_density: 0.12,
        },
        BiomeDescriptor {
            id: BiomeId::VolcanicLandscape, name: "Volcanic Landscape",
            temp_min: 5.0, temp_max: 40.0, humidity_min: 0.10, humidity_max: 0.60,
            alt_min: 0.10, alt_max: 0.70, ground_color: Vec3::new(0.12, 0.10, 0.10),
            tree_density: 0.05, grass_density: 0.10, rock_density: 0.85,
            snow_coverage: 0.00, rainfall_mm: 400.0, wind_speed_ms: 6.0, fog_density: 0.20,
        },
        BiomeDescriptor {
            id: BiomeId::SaltFlat, name: "Salt Flat",
            temp_min: 15.0, temp_max: 45.0, humidity_min: 0.00, humidity_max: 0.12,
            alt_min: 0.00, alt_max: 0.10, ground_color: Vec3::new(0.95, 0.95, 0.92),
            tree_density: 0.00, grass_density: 0.03, rock_density: 0.05,
            snow_coverage: 0.00, rainfall_mm: 50.0, wind_speed_ms: 8.0, fog_density: 0.00,
        },
        BiomeDescriptor {
            id: BiomeId::GlacialValley, name: "Glacial Valley",
            temp_min: -20.0, temp_max: 2.0, humidity_min: 0.30, humidity_max: 0.70,
            alt_min: 0.30, alt_max: 0.80, ground_color: Vec3::new(0.55, 0.65, 0.70),
            tree_density: 0.05, grass_density: 0.15, rock_density: 0.60,
            snow_coverage: 0.70, rainfall_mm: 600.0, wind_speed_ms: 7.0, fog_density: 0.15,
        },
        BiomeDescriptor {
            id: BiomeId::CoastalDunes, name: "Coastal Dunes",
            temp_min: 10.0, temp_max: 35.0, humidity_min: 0.15, humidity_max: 0.45,
            alt_min: 0.00, alt_max: 0.10, ground_color: Vec3::new(0.90, 0.85, 0.65),
            tree_density: 0.05, grass_density: 0.30, rock_density: 0.10,
            snow_coverage: 0.00, rainfall_mm: 350.0, wind_speed_ms: 9.0, fog_density: 0.08,
        },
        BiomeDescriptor {
            id: BiomeId::DeepOceanFloor, name: "Deep Ocean Floor",
            temp_min: 2.0, temp_max: 8.0, humidity_min: 1.00, humidity_max: 1.00,
            alt_min: 0.00, alt_max: 0.05, ground_color: Vec3::new(0.05, 0.06, 0.15),
            tree_density: 0.00, grass_density: 0.05, rock_density: 0.20,
            snow_coverage: 0.00, rainfall_mm: 0.0, wind_speed_ms: 0.0, fog_density: 0.90,
        },
        BiomeDescriptor {
            id: BiomeId::CoralReef, name: "Coral Reef",
            temp_min: 22.0, temp_max: 32.0, humidity_min: 0.90, humidity_max: 1.00,
            alt_min: 0.00, alt_max: 0.06, ground_color: Vec3::new(0.90, 0.60, 0.40),
            tree_density: 0.00, grass_density: 0.60, rock_density: 0.30,
            snow_coverage: 0.00, rainfall_mm: 0.0, wind_speed_ms: 0.0, fog_density: 0.30,
        },
    ]
}

#[derive(Clone, Debug)]
pub struct BiomeBlendSample {
    pub weights: [f32; 25],
    pub dominant: BiomeId,
    pub blended_color: Vec3,
    pub blended_tree_density: f32,
    pub blended_grass_density: f32,
    pub blended_rock_density: f32,
    pub blended_snow: f32,
}

pub struct BiomeSystem {
    pub descriptors: Vec<BiomeDescriptor>,
}

impl BiomeSystem {
    pub fn new() -> Self {
        Self { descriptors: build_biome_table() }
    }

    /// Full biome blend at a point using gaussian weighting
    pub fn sample(&self, temp: f32, humidity: f32, altitude: f32) -> BiomeBlendSample {
        let mut weights = [0.0f32; 25];
        let mut weight_sum = 0.0f32;

        for (i, desc) in self.descriptors.iter().enumerate() {
            let w = desc.blend_weight(temp, humidity, altitude);
            weights[i] = w;
            weight_sum += w;
        }

        // Normalize
        if weight_sum < 1e-10 {
            // fallback: use classified biome
            let id = BiomeDescriptor::classify_point(temp, humidity, altitude) as usize;
            weights[id] = 1.0;
            weight_sum = 1.0;
        }
        for w in weights.iter_mut() {
            *w /= weight_sum;
        }

        // Find dominant
        let dominant_idx = weights.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Blend properties
        let mut blended_color       = Vec3::ZERO;
        let mut blended_tree        = 0.0f32;
        let mut blended_grass       = 0.0f32;
        let mut blended_rock        = 0.0f32;
        let mut blended_snow        = 0.0f32;

        for (i, desc) in self.descriptors.iter().enumerate() {
            let w = weights[i];
            blended_color  += desc.ground_color  * w;
            blended_tree   += desc.tree_density  * w;
            blended_grass  += desc.grass_density * w;
            blended_rock   += desc.rock_density  * w;
            blended_snow   += desc.snow_coverage * w;
        }

        let dominant_id = self.descriptors[dominant_idx].id;
        BiomeBlendSample {
            weights,
            dominant: dominant_id,
            blended_color,
            blended_tree_density: blended_tree,
            blended_grass_density: blended_grass,
            blended_rock_density: blended_rock,
            blended_snow,
        }
    }

    /// Classify transition zone between two biomes (returns a blend factor 0..1)
    pub fn transition_factor(&self, biome_a: BiomeId, biome_b: BiomeId,
                              temp: f32, humidity: f32, altitude: f32) -> f32 {
        let wa = self.descriptors[biome_a as usize].blend_weight(temp, humidity, altitude);
        let wb = self.descriptors[biome_b as usize].blend_weight(temp, humidity, altitude);
        if wa + wb < 1e-10 { return 0.5; }
        wa / (wa + wb)
    }

    /// Get wind speed interpolated across biome weights at a sample
    pub fn wind_speed(&self, sample: &BiomeBlendSample) -> f32 {
        let mut speed = 0.0f32;
        for (i, desc) in self.descriptors.iter().enumerate() {
            speed += desc.wind_speed_ms * sample.weights[i];
        }
        speed
    }
}

// ============================================================
//  PROCEDURAL NOISE
// ============================================================

// --- Perlin Noise ---

#[inline]
fn fade(t: f32) -> f32 {
    // Ken Perlin's 6t^5 - 15t^4 + 10t^3
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp_f(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[inline]
fn grad3(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = (hash & 15) as usize;
    let g = &GRAD3[h];
    g[0] * x + g[1] * y + g[2] * z
}

pub fn perlin_noise_3d(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;

    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let zf = z - zi as f32;

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let xi = (xi & 255) as usize;
    let yi = (yi & 255) as usize;
    let zi = (zi & 255) as usize;

    let aaa = PERM[PERM[PERM[xi]     as usize + yi]     as usize + zi]     as u8;
    let aba = PERM[PERM[PERM[xi]     as usize + yi + 1] as usize + zi]     as u8;
    let aab = PERM[PERM[PERM[xi]     as usize + yi]     as usize + zi + 1] as u8;
    let abb = PERM[PERM[PERM[xi]     as usize + yi + 1] as usize + zi + 1] as u8;
    let baa = PERM[PERM[PERM[xi + 1] as usize + yi]     as usize + zi]     as u8;
    let bba = PERM[PERM[PERM[xi + 1] as usize + yi + 1] as usize + zi]     as u8;
    let bab = PERM[PERM[PERM[xi + 1] as usize + yi]     as usize + zi + 1] as u8;
    let bbb = PERM[PERM[PERM[xi + 1] as usize + yi + 1] as usize + zi + 1] as u8;

    let x1 = lerp_f(grad3(aaa, xf,       yf,       zf),       grad3(baa, xf - 1.0, yf,       zf),       u);
    let x2 = lerp_f(grad3(aba, xf,       yf - 1.0, zf),       grad3(bba, xf - 1.0, yf - 1.0, zf),       u);
    let y1 = lerp_f(x1, x2, v);

    let x3 = lerp_f(grad3(aab, xf,       yf,       zf - 1.0), grad3(bab, xf - 1.0, yf,       zf - 1.0), u);
    let x4 = lerp_f(grad3(abb, xf,       yf - 1.0, zf - 1.0), grad3(bbb, xf - 1.0, yf - 1.0, zf - 1.0), u);
    let y2 = lerp_f(x3, x4, v);

    lerp_f(y1, y2, w)
}

pub fn perlin_noise_2d(x: f32, y: f32) -> f32 {
    perlin_noise_3d(x, y, 0.0)
}

// --- Simplex Noise 3D ---

#[inline]
fn simplex_grad3(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = (hash & 15) as usize;
    let g = &GRAD3[h];
    g[0] * x + g[1] * y + g[2] * z
}

pub fn simplex_noise_3d(xin: f32, yin: f32, zin: f32) -> f32 {
    // Simplex noise: Ken Perlin's improved algorithm
    let f3 = 1.0 / 3.0_f32;
    let g3 = 1.0 / 6.0_f32;

    let s = (xin + yin + zin) * f3;
    let i = (xin + s).floor() as i32;
    let j = (yin + s).floor() as i32;
    let k = (zin + s).floor() as i32;

    let t = (i + j + k) as f32 * g3;
    let x0 = xin - (i as f32 - t);
    let y0 = yin - (j as f32 - t);
    let z0 = zin - (k as f32 - t);

    // Determine which simplex we're in
    let (i1, j1, k1, i2, j2, k2);
    if x0 >= y0 {
        if y0 >= z0      { i1=1;j1=0;k1=0; i2=1;j2=1;k2=0; }
        else if x0 >= z0 { i1=1;j1=0;k1=0; i2=1;j2=0;k2=1; }
        else             { i1=0;j1=0;k1=1; i2=1;j2=0;k2=1; }
    } else {
        if y0 < z0       { i1=0;j1=0;k1=1; i2=0;j2=1;k2=1; }
        else if x0 < z0  { i1=0;j1=1;k1=0; i2=0;j2=1;k2=1; }
        else             { i1=0;j1=1;k1=0; i2=1;j2=1;k2=0; }
    }

    let x1 = x0 - i1 as f32 + g3;
    let y1 = y0 - j1 as f32 + g3;
    let z1 = z0 - k1 as f32 + g3;
    let x2 = x0 - i2 as f32 + 2.0 * g3;
    let y2 = y0 - j2 as f32 + 2.0 * g3;
    let z2 = z0 - k2 as f32 + 2.0 * g3;
    let x3 = x0 - 1.0 + 3.0 * g3;
    let y3 = y0 - 1.0 + 3.0 * g3;
    let z3 = z0 - 1.0 + 3.0 * g3;

    let ii = (i & 255) as usize;
    let jj = (j & 255) as usize;
    let kk = (k & 255) as usize;

    let gi0 = PERM[ii  + PERM[jj  + PERM[kk ] as usize] as usize] & 15;
    let gi1 = PERM[ii + i1 as usize + PERM[jj + j1 as usize + PERM[(kk + k1 as usize) & 255] as usize] as usize] & 15;
    let gi2 = PERM[ii + i2 as usize + PERM[jj + j2 as usize + PERM[(kk + k2 as usize) & 255] as usize] as usize] & 15;
    let gi3 = PERM[(ii+1)&255 + PERM[(jj+1)&255 + PERM[(kk+1)&255] as usize] as usize] & 15;

    let t0 = 0.6 - x0*x0 - y0*y0 - z0*z0;
    let n0 = if t0 < 0.0 { 0.0 } else { t0*t0*t0*t0 * simplex_grad3(gi0, x0, y0, z0) };

    let t1 = 0.6 - x1*x1 - y1*y1 - z1*z1;
    let n1 = if t1 < 0.0 { 0.0 } else { t1*t1*t1*t1 * simplex_grad3(gi1, x1, y1, z1) };

    let t2 = 0.6 - x2*x2 - y2*y2 - z2*z2;
    let n2 = if t2 < 0.0 { 0.0 } else { t2*t2*t2*t2 * simplex_grad3(gi2, x2, y2, z2) };

    let t3 = 0.6 - x3*x3 - y3*y3 - z3*z3;
    let n3 = if t3 < 0.0 { 0.0 } else { t3*t3*t3*t3 * simplex_grad3(gi3, x3, y3, z3) };

    32.0 * (n0 + n1 + n2 + n3)
}

// --- Worley / Cellular Noise ---

/// Returns (F1, F2) — the two nearest feature-point distances
pub fn worley_noise_2d(x: f32, y: f32) -> (f32, f32) {
    let cx = x.floor() as i32;
    let cy = y.floor() as i32;

    let mut f1 = f32::MAX;
    let mut f2 = f32::MAX;

    for dx in -2..=2i32 {
        for dy in -2..=2i32 {
            let nx = cx + dx;
            let ny = cy + dy;
            // pseudo-random offset inside this cell
            let hash = worley_hash(nx, ny);
            let fx = nx as f32 + ((hash & 0xFFFF) as f32 / 65535.0);
            let fy = ny as f32 + (((hash >> 16) & 0xFFFF) as f32 / 65535.0);
            let dist = ((fx - x) * (fx - x) + (fy - y) * (fy - y)).sqrt();
            if dist < f1       { f2 = f1; f1 = dist; }
            else if dist < f2  { f2 = dist; }
        }
    }
    (f1, f2)
}

pub fn worley_noise_3d(x: f32, y: f32, z: f32) -> (f32, f32) {
    let cx = x.floor() as i32;
    let cy = y.floor() as i32;
    let cz = z.floor() as i32;

    let mut f1 = f32::MAX;
    let mut f2 = f32::MAX;

    for dx in -1..=1i32 {
        for dy in -1..=1i32 {
            for dz in -1..=1i32 {
                let nx = cx + dx;
                let ny = cy + dy;
                let nz = cz + dz;
                let h = worley_hash_3d(nx, ny, nz);
                let fx = nx as f32 + ((h & 0x3FF) as f32 / 1023.0);
                let fy = ny as f32 + (((h >> 10) & 0x3FF) as f32 / 1023.0);
                let fz = nz as f32 + (((h >> 20) & 0x3FF) as f32 / 1023.0);
                let dist = ((fx-x)*(fx-x) + (fy-y)*(fy-y) + (fz-z)*(fz-z)).sqrt();
                if dist < f1      { f2 = f1; f1 = dist; }
                else if dist < f2 { f2 = dist; }
            }
        }
    }
    (f1, f2)
}

#[inline]
fn worley_hash(x: i32, y: i32) -> u32 {
    let mut h = (x.wrapping_mul(1619).wrapping_add(y.wrapping_mul(31337))) as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h
}

#[inline]
fn worley_hash_3d(x: i32, y: i32, z: i32) -> u32 {
    let mut h = (x.wrapping_mul(1619)
        .wrapping_add(y.wrapping_mul(31337))
        .wrapping_add(z.wrapping_mul(1013))) as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    h = h.wrapping_mul(0xd7e7f3b);
    h ^= h >> 16;
    h
}

// --- Fractal Brownian Motion ---

#[derive(Clone, Debug)]
pub struct FbmParams {
    pub octaves:    usize,
    pub frequency:  f32,
    pub lacunarity: f32,
    pub gain:       f32,
    pub amplitude:  f32,
    pub offset:     f32,
    pub ridge:      bool,
}

impl FbmParams {
    pub fn default_terrain() -> Self {
        FbmParams { octaves: 8, frequency: 1.0, lacunarity: 2.0, gain: 0.5, amplitude: 1.0, offset: 1.0, ridge: false }
    }
    pub fn default_ridge() -> Self {
        FbmParams { octaves: 6, frequency: 1.0, lacunarity: 2.2, gain: 0.6, amplitude: 1.0, offset: 1.0, ridge: true }
    }
    pub fn default_cloud() -> Self {
        FbmParams { octaves: 5, frequency: 2.0, lacunarity: 2.0, gain: 0.45, amplitude: 0.8, offset: 0.0, ridge: false }
    }
}

pub fn fbm_3d(x: f32, y: f32, z: f32, params: &FbmParams) -> f32 {
    let mut freq  = params.frequency;
    let mut amp   = params.amplitude;
    let mut value = 0.0f32;
    let mut weight = 1.0f32;
    let mut prev   = 1.0f32;

    for i in 0..params.octaves {
        let n = perlin_noise_3d(x * freq, y * freq, z * freq);

        if params.ridge {
            let ridged = (params.offset - n.abs()).abs();
            let signal = ridged * ridged * weight;
            weight     = (signal * 2.0).clamp(0.0, 1.0);
            value += signal * amp;
        } else {
            value += n * amp;
        }

        freq *= params.lacunarity;
        amp  *= params.gain;
    }
    value
}

pub fn fbm_2d(x: f32, y: f32, params: &FbmParams) -> f32 {
    fbm_3d(x, y, 0.0, params)
}

/// Turbulence (absolute value FBM)
pub fn turbulence_2d(x: f32, y: f32, octaves: usize, freq: f32, gain: f32, lacunarity: f32) -> f32 {
    let mut f   = freq;
    let mut amp = 1.0f32;
    let mut v   = 0.0f32;
    let mut max = 0.0f32;
    for _ in 0..octaves {
        v   += perlin_noise_2d(x * f, y * f).abs() * amp;
        max += amp;
        f   *= lacunarity;
        amp *= gain;
    }
    if max > 0.0 { v / max } else { 0.0 }
}

/// Domain-warped FBM (Inigo Quilez style)
pub fn domain_warp_fbm_2d(x: f32, y: f32, warp_strength: f32, params: &FbmParams) -> f32 {
    let q_x = fbm_2d(x,             y,             params);
    let q_y = fbm_2d(x + 5.2, y + 1.3, params);
    let r_x = fbm_2d(x + warp_strength * q_x + 1.7, y + warp_strength * q_y + 9.2, params);
    let r_y = fbm_2d(x + warp_strength * q_x + 8.3, y + warp_strength * q_y + 2.8, params);
    fbm_2d(x + warp_strength * r_x, y + warp_strength * r_y, params)
}

// ============================================================
//  HEIGHTMAP + HYDRAULIC EROSION
// ============================================================

#[derive(Clone, Debug)]
pub struct Heightmap {
    pub width:  usize,
    pub height: usize,
    pub data:   Vec<f32>,    // row-major, values 0..1
    pub min_h:  f32,
    pub max_h:  f32,
}

impl Heightmap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
            min_h: 0.0,
            max_h: 1.0,
        }
    }

    #[inline]
    pub fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[self.index(x, y)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        let idx = self.index(x, y);
        self.data[idx] = v;
    }

    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> f32 {
        let cx = x.clamp(0, self.width  as i32 - 1) as usize;
        let cy = y.clamp(0, self.height as i32 - 1) as usize;
        self.get(cx, cy)
    }

    pub fn sample_bilinear(&self, u: f32, v: f32) -> f32 {
        let px = u * (self.width  - 1) as f32;
        let py = v * (self.height - 1) as f32;
        let x0 = px.floor() as i32;
        let y0 = py.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let tx = px - x0 as f32;
        let ty = py - y0 as f32;
        let a = self.get_clamped(x0, y0);
        let b = self.get_clamped(x1, y0);
        let c = self.get_clamped(x0, y1);
        let d = self.get_clamped(x1, y1);
        lerp_f(lerp_f(a, b, tx), lerp_f(c, d, tx), ty)
    }

    /// Compute surface normal at a pixel using central differences
    pub fn normal_at(&self, x: usize, y: usize, cell_size: f32) -> Vec3 {
        let xi = x as i32;
        let yi = y as i32;
        let hL = self.get_clamped(xi - 1, yi);
        let hR = self.get_clamped(xi + 1, yi);
        let hD = self.get_clamped(xi, yi - 1);
        let hU = self.get_clamped(xi, yi + 1);
        let dx = (hR - hL) / (2.0 * cell_size);
        let dz = (hU - hD) / (2.0 * cell_size);
        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    /// Compute slope (radians) at a pixel
    pub fn slope_at(&self, x: usize, y: usize, cell_size: f32) -> f32 {
        let n = self.normal_at(x, y, cell_size);
        n.y.acos()
    }

    /// Compute gradient at a pixel — returns (dh/dx, dh/dy) in normalised coords
    pub fn gradient_at(&self, x: usize, y: usize) -> Vec2 {
        let xi = x as i32;
        let yi = y as i32;
        let dx = (self.get_clamped(xi + 1, yi) - self.get_clamped(xi - 1, yi)) * 0.5;
        let dy = (self.get_clamped(xi, yi + 1) - self.get_clamped(xi, yi - 1)) * 0.5;
        Vec2::new(dx, dy)
    }

    pub fn recompute_minmax(&mut self) {
        self.min_h = self.data.iter().cloned().fold(f32::MAX, f32::min);
        self.max_h = self.data.iter().cloned().fold(f32::MIN, f32::max);
    }

    pub fn normalize_to_01(&mut self) {
        self.recompute_minmax();
        let range = self.max_h - self.min_h;
        if range < 1e-10 { return; }
        for v in self.data.iter_mut() {
            *v = (*v - self.min_h) / range;
        }
        self.min_h = 0.0;
        self.max_h = 1.0;
    }

    /// Generate heightmap from FBM noise
    pub fn generate_fbm(&mut self, params: &FbmParams, seed_offset: Vec2) {
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / self.width  as f32 + seed_offset.x;
                let ny = y as f32 / self.height as f32 + seed_offset.y;
                let h = fbm_2d(nx, ny, params) * 0.5 + 0.5;
                self.set(x, y, h.clamp(0.0, 1.0));
            }
        }
        self.recompute_minmax();
    }

    /// Generate with domain warped FBM for more natural terrain
    pub fn generate_domain_warp(&mut self, params: &FbmParams, warp: f32, seed_offset: Vec2) {
        for y in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / self.width  as f32 + seed_offset.x;
                let ny = y as f32 / self.height as f32 + seed_offset.y;
                let h = domain_warp_fbm_2d(nx, ny, warp, params) * 0.5 + 0.5;
                self.set(x, y, h.clamp(0.0, 1.0));
            }
        }
        self.recompute_minmax();
    }
}

// --- Hydraulic Erosion (particle-based Benes et al. / Sebastian Lague) ---

#[derive(Clone, Debug)]
pub struct ErosionParams {
    pub num_particles:  usize,
    pub inertia:        f32,   // 0..1 — how much particle keeps direction
    pub capacity:       f32,   // max sediment a droplet can carry (proportional to speed)
    pub deposition:     f32,   // fraction deposited when over-capacity
    pub erosion_speed:  f32,   // how quickly terrain is eroded
    pub evaporation:    f32,   // water lost per step
    pub min_slope:      f32,   // prevents flat-area erosion artefacts
    pub gravity:        f32,
    pub max_steps:      usize,
    pub erosion_radius: f32,   // radius for depositing sediment onto neighbours
    pub seed:           u64,
}

impl Default for ErosionParams {
    fn default() -> Self {
        ErosionParams {
            num_particles:  50_000,
            inertia:        EROSION_INERTIA,
            capacity:       EROSION_CAPACITY,
            deposition:     EROSION_DEPOSITION,
            erosion_speed:  EROSION_EROSION_SPEED,
            evaporation:    EROSION_EVAPORATION,
            min_slope:      EROSION_MIN_SLOPE,
            gravity:        EROSION_GRAVITY,
            max_steps:      EROSION_MAX_STEPS,
            erosion_radius: 3.0,
            seed:           0xDEAD_BEEF_1234,
        }
    }
}

struct LcgRng { state: u64 }
impl LcgRng {
    fn new(seed: u64) -> Self { Self { state: seed ^ 0x123456789ABCDEF } }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }
    fn next_f32(&mut self) -> f32 { (self.next_u64() >> 32) as f32 / u32::MAX as f32 }
    fn next_f32_range(&mut self, min: f32, max: f32) -> f32 { min + self.next_f32() * (max - min) }
}

/// Bilinear height from a continuous position on the heightmap
fn hmap_height_bilinear(data: &[f32], width: usize, height: usize, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let clamp_x = |v: i32| -> usize { v.clamp(0, width  as i32 - 1) as usize };
    let clamp_y = |v: i32| -> usize { v.clamp(0, height as i32 - 1) as usize };

    let a = data[clamp_y(y0) * width + clamp_x(x0)];
    let b = data[clamp_y(y0) * width + clamp_x(x1)];
    let c = data[clamp_y(y1) * width + clamp_x(x0)];
    let d = data[clamp_y(y1) * width + clamp_x(x1)];

    lerp_f(lerp_f(a, b, tx), lerp_f(c, d, tx), ty)
}

/// Gradient of the heightmap using bilinear interpolation
fn hmap_gradient(data: &[f32], width: usize, height: usize, x: f32, y: f32) -> Vec2 {
    let gx = hmap_height_bilinear(data, width, height, x + 0.5, y)
           - hmap_height_bilinear(data, width, height, x - 0.5, y);
    let gy = hmap_height_bilinear(data, width, height, x, y + 0.5)
           - hmap_height_bilinear(data, width, height, x, y - 0.5);
    Vec2::new(gx, gy)
}

/// Erode a heightmap using particle-based hydraulic erosion
pub fn hydraulic_erosion(hmap: &mut Heightmap, params: &ErosionParams) {
    let w = hmap.width;
    let h = hmap.height;
    let mut rng = LcgRng::new(params.seed);

    // Pre-compute erosion brush weights (circular kernel)
    let radius = params.erosion_radius;
    let brush_radius = radius.ceil() as i32;
    let mut brush_offsets: Vec<(i32, i32, f32)> = Vec::new();
    let mut brush_weight_sum = 0.0f32;
    for dy in -brush_radius..=brush_radius {
        for dx in -brush_radius..=brush_radius {
            let dist = ((dx*dx + dy*dy) as f32).sqrt();
            if dist <= radius {
                let w_val = 1.0 - dist / radius;
                brush_offsets.push((dx, dy, w_val));
                brush_weight_sum += w_val;
            }
        }
    }
    // normalize brush weights
    for b in brush_offsets.iter_mut() { b.2 /= brush_weight_sum; }

    let data = &mut hmap.data;

    for _particle in 0..params.num_particles {
        // spawn droplet at random position
        let mut pos_x = rng.next_f32_range(0.0, (w - 1) as f32);
        let mut pos_y = rng.next_f32_range(0.0, (h - 1) as f32);
        let mut vel_x = 0.0f32;
        let mut vel_y = 0.0f32;
        let mut speed  = 0.0f32;
        let mut water  = 1.0f32;
        let mut sediment = 0.0f32;

        for _step in 0..params.max_steps {
            let node_x = pos_x.floor() as i32;
            let node_y = pos_y.floor() as i32;

            if node_x < 0 || node_x >= w as i32 - 1 || node_y < 0 || node_y >= h as i32 - 1 {
                break;
            }

            let grad = hmap_gradient(data, w, h, pos_x, pos_y);
            // Update direction (blend with gradient)
            vel_x = vel_x * params.inertia - grad.x * (1.0 - params.inertia);
            vel_y = vel_y * params.inertia - grad.y * (1.0 - params.inertia);

            let vel_len = (vel_x * vel_x + vel_y * vel_y).sqrt();
            if vel_len < 1e-6 {
                break; // droplet stuck
            }
            vel_x /= vel_len;
            vel_y /= vel_len;

            let new_x = pos_x + vel_x;
            let new_y = pos_y + vel_y;

            // Height change
            let old_h = hmap_height_bilinear(data, w, h, pos_x, pos_y);
            let new_h = hmap_height_bilinear(data, w, h, new_x, new_y);
            let delta_h = new_h - old_h;

            // Carrying capacity
            let slope = (-delta_h).max(params.min_slope);
            let carry_capacity = slope * vel_len * water * params.capacity;

            if sediment > carry_capacity || delta_h > 0.0 {
                // Deposit sediment
                let amount = if delta_h > 0.0 {
                    sediment.min(delta_h)
                } else {
                    (sediment - carry_capacity) * params.deposition
                };
                sediment -= amount;

                // Deposit around current position with brush
                for &(bdx, bdy, bw) in &brush_offsets {
                    let bx = node_x + bdx;
                    let by = node_y + bdy;
                    if bx >= 0 && bx < w as i32 && by >= 0 && by < h as i32 {
                        let idx = by as usize * w + bx as usize;
                        data[idx] += amount * bw;
                    }
                }
            } else {
                // Erode terrain
                let erode_amount = ((carry_capacity - sediment) * params.erosion_speed)
                    .min(-delta_h);
                let erode_amount = erode_amount.max(0.0);
                sediment += erode_amount;

                for &(bdx, bdy, bw) in &brush_offsets {
                    let bx = node_x + bdx;
                    let by = node_y + bdy;
                    if bx >= 0 && bx < w as i32 && by >= 0 && by < h as i32 {
                        let idx = by as usize * w + bx as usize;
                        data[idx] -= erode_amount * bw;
                        if data[idx] < 0.0 { data[idx] = 0.0; }
                    }
                }
            }

            speed = ((speed * speed + delta_h * params.gravity).max(0.0)).sqrt();
            water *= 1.0 - params.evaporation;
            pos_x = new_x;
            pos_y = new_y;

            if water < 0.01 { break; }
        }
    }

    hmap.recompute_minmax();
}

/// Thermal erosion — material avalanches if slope exceeds talus angle
pub fn thermal_erosion(hmap: &mut Heightmap, iterations: usize, talus_angle: f32) {
    let w = hmap.width;
    let h = hmap.height;
    let talus = talus_angle.tan(); // in normalised height units per cell

    for _iter in 0..iterations {
        let data_copy = hmap.data.clone();
        for y in 1..h-1 {
            for x in 1..w-1 {
                let center = data_copy[y * w + x];
                let neighbours = [
                    (x+1, y), (x-1, y), (x, y+1), (x, y-1),
                    (x+1, y+1), (x-1, y+1), (x+1, y-1), (x-1, y-1),
                ];
                let mut total_diff = 0.0f32;
                let mut max_diff   = 0.0f32;
                let mut count = 0usize;
                for &(nx, ny) in &neighbours {
                    let diff = center - data_copy[ny * w + nx];
                    if diff > talus {
                        total_diff += diff;
                        if diff > max_diff { max_diff = diff; }
                        count += 1;
                    }
                }
                if count == 0 || total_diff < 1e-8 { continue; }
                let move_frac = 0.5 * (max_diff - talus) / total_diff;
                for &(nx, ny) in &neighbours {
                    let diff = center - data_copy[ny * w + nx];
                    if diff > talus {
                        let transfer = move_frac * diff;
                        hmap.data[y * w + x]   -= transfer;
                        hmap.data[ny * w + nx] += transfer;
                    }
                }
            }
        }
    }
    hmap.recompute_minmax();
}

// ============================================================
//  WATER BODIES — RIVER SIMULATION & LAKE FILLING
// ============================================================

#[derive(Clone, Debug)]
pub struct RiverPath {
    pub points:      Vec<Vec2>,   // (x, y) in heightmap coords
    pub widths:      Vec<f32>,
    pub depths:      Vec<f32>,
    pub flow_rates:  Vec<f32>,
    pub source:      Vec2,
    pub mouth:       Vec2,
    pub total_length: f32,
}

impl RiverPath {
    pub fn new() -> Self {
        RiverPath {
            points:      Vec::new(),
            widths:      Vec::new(),
            depths:      Vec::new(),
            flow_rates:  Vec::new(),
            source:      Vec2::ZERO,
            mouth:       Vec2::ZERO,
            total_length: 0.0,
        }
    }

    pub fn compute_total_length(&mut self) {
        let mut len = 0.0f32;
        for i in 1..self.points.len() {
            len += (self.points[i] - self.points[i-1]).length();
        }
        self.total_length = len;
    }
}

/// Simulate a river starting from a source by following the steepest descent gradient
pub fn simulate_river(hmap: &Heightmap, start: Vec2, min_height: f32) -> RiverPath {
    let mut path = RiverPath::new();
    path.source = start;

    let mut pos = start;
    let mut flow = 1.0f32;
    let mut prev_dir = Vec2::ZERO;

    path.points.push(pos);
    path.widths.push(0.5);
    path.depths.push(0.1);
    path.flow_rates.push(flow);

    let w = hmap.width  as f32;
    let h = hmap.height as f32;

    let mut visited: HashSet<(i32, i32)> = HashSet::new();

    for step in 0..16384usize {
        let ux = (pos.x / w).clamp(0.0, 1.0);
        let uy = (pos.y / h).clamp(0.0, 1.0);
        let current_h = hmap.sample_bilinear(ux, uy);

        if current_h <= min_height { break; }

        // Find steepest descent direction, sampled at 8 neighbours + 8 slightly further out
        let step_size = 0.5f32;
        let mut best_dir  = Vec2::ZERO;
        let mut best_drop = 0.0f32;

        let angles: [f32; 16] = [
            0.0, PI/8.0, PI/4.0, 3.0*PI/8.0, PI/2.0, 5.0*PI/8.0, 3.0*PI/4.0, 7.0*PI/8.0,
            PI, 9.0*PI/8.0, 5.0*PI/4.0, 11.0*PI/8.0, 3.0*PI/2.0, 13.0*PI/8.0, 7.0*PI/4.0, 15.0*PI/8.0,
        ];

        for &angle in &angles {
            let dir = Vec2::new(angle.cos(), angle.sin());
            // Weight toward previous direction (inertia)
            let weighted_dir = if prev_dir.length() > 0.01 {
                (dir * 0.7 + prev_dir * 0.3).normalize()
            } else {
                dir
            };
            let npos = pos + weighted_dir * step_size;
            let nu = (npos.x / w).clamp(0.0, 1.0);
            let nv = (npos.y / h).clamp(0.0, 1.0);
            let nh = hmap.sample_bilinear(nu, nv);
            let drop = current_h - nh;
            if drop > best_drop {
                best_drop = drop;
                best_dir  = weighted_dir;
            }
        }

        if best_drop < 0.0001 && step > 10 {
            // Flat — try to find any lower neighbor
            break;
        }

        if best_dir.length() < 0.01 { break; }
        best_dir = best_dir.normalize();

        pos = pos + best_dir * step_size;
        prev_dir = best_dir;

        // Accumulate flow
        flow += 0.005 * best_drop;
        let width = (flow * 0.3).clamp(0.2, 20.0);
        let depth = (flow * 0.05).clamp(0.05, 5.0);

        path.points.push(pos);
        path.widths.push(width);
        path.depths.push(depth);
        path.flow_rates.push(flow);

        let cell = (pos.x as i32, pos.y as i32);
        if visited.contains(&cell) { break; } // loop detection
        visited.insert(cell);

        // Check world boundary
        if pos.x < 0.5 || pos.y < 0.5 || pos.x > w - 0.5 || pos.y > h - 0.5 {
            break;
        }
    }

    path.mouth = pos;
    path.compute_total_length();
    path
}

/// Lake filling — flood-fill from a seed point up to a given water level
#[derive(Clone, Debug)]
pub struct LakeBody {
    pub cells:      Vec<(usize, usize)>,
    pub water_level: f32,
    pub surface_area: f32,
    pub volume:      f32,
    pub centroid:    Vec2,
}

pub fn fill_lake(hmap: &Heightmap, seed_x: usize, seed_y: usize, max_water_level: f32) -> LakeBody {
    let w = hmap.width;
    let h = hmap.height;
    let mut visited = vec![false; w * h];
    let mut cells   = Vec::new();
    let mut queue   = VecDeque::new();

    let seed_h = hmap.get(seed_x, seed_y);
    let water_level = seed_h.max(max_water_level);

    queue.push_back((seed_x, seed_y));
    visited[seed_y * w + seed_x] = true;

    while let Some((cx, cy)) = queue.pop_front() {
        let ch = hmap.get(cx, cy);
        if ch <= water_level {
            cells.push((cx, cy));
            let neighbours = [
                (cx.wrapping_sub(1), cy), (cx+1, cy),
                (cx, cy.wrapping_sub(1)), (cx, cy+1),
            ];
            for &(nx, ny) in &neighbours {
                if nx < w && ny < h && !visited[ny * w + nx] {
                    visited[ny * w + nx] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    let surface_area = cells.len() as f32;
    let mut cx_sum = 0.0f32;
    let mut cy_sum = 0.0f32;
    let mut volume  = 0.0f32;
    for &(x, y) in &cells {
        cx_sum += x as f32;
        cy_sum += y as f32;
        volume += water_level - hmap.get(x, y);
    }
    let count = cells.len() as f32;
    let centroid = if count > 0.0 {
        Vec2::new(cx_sum / count, cy_sum / count)
    } else {
        Vec2::new(seed_x as f32, seed_y as f32)
    };

    LakeBody { cells, water_level, surface_area, volume, centroid }
}

/// Ocean shore generation — scan for coastline cells (land/water boundary)
#[derive(Clone, Debug)]
pub struct OceanShore {
    pub shore_cells: Vec<(usize, usize)>,
    pub sea_level:   f32,
    pub beach_width: f32,
}

pub fn generate_ocean_shore(hmap: &Heightmap, sea_level: f32, beach_width: f32) -> OceanShore {
    let w = hmap.width;
    let h = hmap.height;
    let mut shore_cells = Vec::new();

    for y in 1..h-1 {
        for x in 1..w-1 {
            let ch = hmap.get(x, y);
            if ch <= sea_level { continue; } // underwater
            // check if any neighbour is underwater
            let neighbours = [(x+1,y),(x-1,y),(x,y+1),(x,y-1)];
            let has_water_nb = neighbours.iter().any(|&(nx, ny)| {
                nx < w && ny < h && hmap.get(nx, ny) <= sea_level
            });
            if has_water_nb {
                shore_cells.push((x, y));
            }
        }
    }

    OceanShore { shore_cells, sea_level, beach_width }
}

/// Check if a point is within beach_width of the shore
pub fn is_beach(shore: &OceanShore, hmap: &Heightmap, x: usize, y: usize) -> bool {
    let h_val = hmap.get(x, y);
    h_val > shore.sea_level && h_val < shore.sea_level + shore.beach_width
}

// ============================================================
//  FOLIAGE PLACEMENT — POISSON DISK SAMPLING (BRIDSON ALGORITHM)
// ============================================================

#[derive(Clone, Debug)]
pub struct FoliageInstance {
    pub position:   Vec3,
    pub rotation:   Quat,
    pub scale:      Vec3,
    pub asset_id:   u32,
    pub biome_id:   u8,
    pub lod_factor: f32,
}

#[derive(Clone, Debug)]
pub struct FoliagePlacementParams {
    pub min_radius:    f32,   // minimum distance between instances
    pub max_instances: usize,
    pub max_slope_rad: f32,   // max surface slope (radians)
    pub min_altitude:  f32,   // normalised altitude 0..1
    pub max_altitude:  f32,
    pub density_scale: f32,
    pub use_density_map: bool,
    pub random_rotation: bool,
    pub scale_variance: f32,
    pub base_scale:    Vec3,
    pub asset_id:      u32,
    pub biome_id:      u8,
    pub align_to_normal: bool,
}

impl Default for FoliagePlacementParams {
    fn default() -> Self {
        FoliagePlacementParams {
            min_radius: 2.0,
            max_instances: 100_000,
            max_slope_rad: 0.7,
            min_altitude: 0.05,
            max_altitude: 0.75,
            density_scale: 1.0,
            use_density_map: false,
            random_rotation: true,
            scale_variance: 0.25,
            base_scale: Vec3::ONE,
            asset_id: 0,
            biome_id: 0,
            align_to_normal: false,
        }
    }
}

/// Bridson's fast Poisson disk sampling in 2D, returns list of (x, y) sample positions
pub fn poisson_disk_2d(
    width: f32,
    height: f32,
    min_dist: f32,
    max_attempts: usize,
    seed: u64,
) -> Vec<Vec2> {
    let cell_size = min_dist / (2.0_f32).sqrt();
    let grid_w    = (width  / cell_size).ceil() as usize + 1;
    let grid_h    = (height / cell_size).ceil() as usize + 1;

    let mut grid: Vec<Option<Vec2>> = vec![None; grid_w * grid_h];
    let mut active_list: Vec<Vec2>  = Vec::new();
    let mut samples:     Vec<Vec2>  = Vec::new();
    let mut rng = LcgRng::new(seed ^ 0xF00D);

    let grid_idx = |p: Vec2| -> usize {
        let gx = (p.x / cell_size) as usize;
        let gy = (p.y / cell_size) as usize;
        gy * grid_w + gx
    };

    // Initial sample
    let first = Vec2::new(
        rng.next_f32() * width,
        rng.next_f32() * height,
    );
    active_list.push(first);
    samples.push(first);
    grid[grid_idx(first)] = Some(first);

    while !active_list.is_empty() {
        let rand_idx = (rng.next_f32() * active_list.len() as f32) as usize;
        let rand_idx = rand_idx.min(active_list.len() - 1);
        let base = active_list[rand_idx];

        let mut found = false;
        for _ in 0..max_attempts {
            // Random point in annulus [r, 2r] around base
            let angle = rng.next_f32() * TWO_PI;
            let rad   = min_dist + rng.next_f32() * min_dist;
            let candidate = Vec2::new(
                base.x + angle.cos() * rad,
                base.y + angle.sin() * rad,
            );

            if candidate.x < 0.0 || candidate.x >= width
            || candidate.y < 0.0 || candidate.y >= height {
                continue;
            }

            // Check neighbours in grid
            let gx0 = ((candidate.x - min_dist) / cell_size).floor() as i32;
            let gy0 = ((candidate.y - min_dist) / cell_size).floor() as i32;
            let gx1 = ((candidate.x + min_dist) / cell_size).ceil()  as i32;
            let gy1 = ((candidate.y + min_dist) / cell_size).ceil()  as i32;

            let gx0u = gx0.max(0) as usize;
            let gy0u = gy0.max(0) as usize;
            let gx1u = (gx1 as usize).min(grid_w - 1);
            let gy1u = (gy1 as usize).min(grid_h - 1);

            let mut ok = true;
            'outer: for gy in gy0u..=gy1u {
                for gx in gx0u..=gx1u {
                    if let Some(p) = grid[gy * grid_w + gx] {
                        if (p - candidate).length() < min_dist {
                            ok = false;
                            break 'outer;
                        }
                    }
                }
            }

            if ok {
                active_list.push(candidate);
                samples.push(candidate);
                grid[grid_idx(candidate)] = Some(candidate);
                found = true;
                break;
            }
        }

        if !found {
            active_list.swap_remove(rand_idx);
        }
    }

    samples
}

pub fn place_foliage(
    hmap:        &Heightmap,
    density_map: Option<&Vec<f32>>,
    params:      &FoliagePlacementParams,
    seed:        u64,
    cell_size:   f32,
) -> Vec<FoliageInstance> {
    let w = hmap.width  as f32;
    let h = hmap.height as f32;

    let candidates = poisson_disk_2d(w, h, params.min_radius, 30, seed);
    let mut rng    = LcgRng::new(seed ^ 0xFACE);
    let mut result = Vec::new();

    for pos2d in &candidates {
        if result.len() >= params.max_instances { break; }

        let ux = (pos2d.x / w).clamp(0.0, 1.0);
        let uy = (pos2d.y / h).clamp(0.0, 1.0);
        let altitude = hmap.sample_bilinear(ux, uy);

        if altitude < params.min_altitude || altitude > params.max_altitude { continue; }

        let xi = pos2d.x as usize;
        let yi = pos2d.y as usize;
        let slope = if xi < hmap.width && yi < hmap.height {
            hmap.slope_at(xi.min(hmap.width-1), yi.min(hmap.height-1), cell_size)
        } else { 0.0 };

        if slope > params.max_slope_rad { continue; }

        // Check density map
        if params.use_density_map {
            if let Some(dmap) = density_map {
                let di = (uy * (hmap.height - 1) as f32) as usize * hmap.width
                       + (ux * (hmap.width  - 1) as f32) as usize;
                let di = di.min(dmap.len() - 1);
                let density = dmap[di] * params.density_scale;
                if rng.next_f32() > density { continue; }
            }
        }

        // Compute rotation
        let rotation = if params.align_to_normal {
            let xi_c = xi.min(hmap.width  - 1);
            let yi_c = yi.min(hmap.height - 1);
            let normal = hmap.normal_at(xi_c, yi_c, cell_size);
            let up = Vec3::Y;
            let axis  = up.cross(normal);
            let angle  = up.dot(normal).acos();
            if axis.length() > 1e-6 {
                Quat::from_axis_angle(axis.normalize(), angle)
            } else {
                Quat::IDENTITY
            }
        } else if params.random_rotation {
            let angle = rng.next_f32() * TWO_PI;
            Quat::from_rotation_y(angle)
        } else {
            Quat::IDENTITY
        };

        // Compute scale
        let sv = 1.0 + (rng.next_f32() * 2.0 - 1.0) * params.scale_variance;
        let scale = params.base_scale * sv;

        let world_y = altitude * hmap.max_h;
        let position = Vec3::new(pos2d.x * cell_size, world_y, pos2d.y * cell_size);

        result.push(FoliageInstance {
            position,
            rotation,
            scale,
            asset_id: params.asset_id,
            biome_id: params.biome_id,
            lod_factor: 1.0,
        });
    }

    result
}

// ============================================================
//  ROAD NETWORK — A* PATHFINDING + CATMULL-ROM SMOOTHING
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GridNode {
    pub x: i32,
    pub y: i32,
}

impl GridNode {
    pub fn new(x: i32, y: i32) -> Self { GridNode { x, y } }
    pub fn to_vec2(&self, cell_size: f32) -> Vec2 {
        Vec2::new(self.x as f32 * cell_size, self.y as f32 * cell_size)
    }
}

#[derive(Clone, Debug)]
pub struct AStarNode {
    pub pos:    GridNode,
    pub g_cost: f32,
    pub h_cost: f32,
    pub parent: Option<GridNode>,
}

impl AStarNode {
    pub fn f_cost(&self) -> f32 { self.g_cost + self.h_cost }
}

fn astar_heuristic(a: &GridNode, b: &GridNode) -> f32 {
    // Octile heuristic — good for 8-directional movement
    let dx = (a.x - b.x).abs() as f32;
    let dy = (a.y - b.y).abs() as f32;
    let (min_d, max_d) = if dx < dy { (dx, dy) } else { (dy, dx) };
    max_d + (1.41421356 - 1.0) * min_d
}

pub struct RoadCostParams {
    pub slope_weight:   f32,
    pub height_weight:  f32,
    pub water_penalty:  f32,
    pub sea_level:      f32,
}

impl Default for RoadCostParams {
    fn default() -> Self {
        RoadCostParams { slope_weight: 5.0, height_weight: 2.0, water_penalty: 100.0, sea_level: 0.1 }
    }
}

fn road_move_cost(hmap: &Heightmap, from: &GridNode, to: &GridNode, cost_params: &RoadCostParams) -> f32 {
    let w = hmap.width  as i32;
    let h = hmap.height as i32;
    if to.x < 0 || to.y < 0 || to.x >= w || to.y >= h { return f32::MAX; }

    let diagonal = from.x != to.x && from.y != to.y;
    let base_cost = if diagonal { 1.41421356 } else { 1.0 };

    let h_from = hmap.get_clamped(from.x, from.y);
    let h_to   = hmap.get_clamped(to.x,   to.y);

    // Water avoidance
    if h_to <= cost_params.sea_level { return base_cost + cost_params.water_penalty; }

    let slope = (h_to - h_from).abs();
    let cost  = base_cost
              + slope * cost_params.slope_weight
              + (h_to - 0.3).abs() * cost_params.height_weight;
    cost
}

/// A* pathfinding on a heightmap grid
pub fn astar_path(
    hmap:        &Heightmap,
    start:       GridNode,
    goal:        GridNode,
    cost_params: &RoadCostParams,
) -> Option<Vec<GridNode>> {
    use std::collections::BinaryHeap;
    use std::cmp::Ordering;

    #[derive(Clone)]
    struct Entry { cost: f32, node: GridNode }
    impl PartialEq for Entry { fn eq(&self, o: &Self) -> bool { self.cost == o.cost } }
    impl Eq for Entry {}
    impl PartialOrd for Entry {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
    }
    impl Ord for Entry {
        fn cmp(&self, o: &Self) -> Ordering {
            o.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
        }
    }

    let mut open_heap:  BinaryHeap<Entry> = BinaryHeap::new();
    let mut g_score:    HashMap<(i32,i32), f32> = HashMap::new();
    let mut came_from:  HashMap<(i32,i32), GridNode> = HashMap::new();
    let mut closed_set: HashSet<(i32,i32)> = HashSet::new();

    let start_key = (start.x, start.y);
    g_score.insert(start_key, 0.0);
    open_heap.push(Entry { cost: astar_heuristic(&start, &goal), node: start.clone() });

    let directions: [(i32,i32); 8] = [
        (1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)
    ];

    let mut iterations = 0usize;
    const MAX_ITER: usize = 200_000;

    while let Some(Entry { node: current, .. }) = open_heap.pop() {
        iterations += 1;
        if iterations > MAX_ITER { return None; }

        let cur_key = (current.x, current.y);

        if current.x == goal.x && current.y == goal.y {
            // Reconstruct path
            let mut path = vec![current.clone()];
            let mut cur  = cur_key;
            while let Some(parent) = came_from.get(&cur) {
                path.push(parent.clone());
                cur = (parent.x, parent.y);
            }
            path.reverse();
            return Some(path);
        }

        if closed_set.contains(&cur_key) { continue; }
        closed_set.insert(cur_key);

        let cur_g = *g_score.get(&cur_key).unwrap_or(&f32::MAX);

        for &(dx, dy) in &directions {
            let nb = GridNode::new(current.x + dx, current.y + dy);
            let nb_key = (nb.x, nb.y);
            if closed_set.contains(&nb_key) { continue; }

            let move_c = road_move_cost(hmap, &current, &nb, cost_params);
            if move_c >= f32::MAX * 0.5 { continue; }

            let tentative_g = cur_g + move_c;
            let old_g = *g_score.get(&nb_key).unwrap_or(&f32::MAX);
            if tentative_g < old_g {
                g_score.insert(nb_key, tentative_g);
                came_from.insert(nb_key, current.clone());
                let f = tentative_g + astar_heuristic(&nb, &goal);
                open_heap.push(Entry { cost: f, node: nb });
            }
        }
    }
    None
}

/// Catmull-Rom spline interpolation between control points
pub fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    // Catmull-Rom matrix (alpha = 0.5)
    let q = 0.5 * (
        (p1 * 2.0)
        + (-p0 + p2) * t
        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
        + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * t3
    );
    q
}

/// Smooth a road path using Catmull-Rom
pub fn smooth_road_path(
    raw_nodes: &[GridNode],
    cell_size: f32,
    samples_per_segment: usize,
) -> Vec<Vec2> {
    if raw_nodes.len() < 2 { return Vec::new(); }
    let pts: Vec<Vec2> = raw_nodes.iter().map(|n| n.to_vec2(cell_size)).collect();
    let n = pts.len();
    let mut result = Vec::with_capacity(n * samples_per_segment);

    for i in 0..n - 1 {
        let p0 = if i == 0     { pts[0] + (pts[0] - pts[1]) }       else { pts[i-1] };
        let p1 = pts[i];
        let p2 = pts[i+1];
        let p3 = if i+2 >= n   { pts[n-1] + (pts[n-1] - pts[n-2]) } else { pts[i+2] };

        for s in 0..samples_per_segment {
            let t = s as f32 / samples_per_segment as f32;
            result.push(catmull_rom(p0, p1, p2, p3, t));
        }
    }
    result.push(*pts.last().unwrap());
    result
}

#[derive(Clone, Debug)]
pub struct RoadSegment {
    pub id:           u32,
    pub control_pts:  Vec<Vec2>,
    pub smoothed_pts: Vec<Vec2>,
    pub width:        f32,
    pub road_type:    RoadType,
    pub start_node:   u32,
    pub end_node:     u32,
    pub length:       f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoadType {
    Dirt,
    Gravel,
    Paved,
    Highway,
    Trail,
}

impl RoadSegment {
    pub fn compute_length(&mut self) {
        let mut len = 0.0f32;
        for i in 1..self.smoothed_pts.len() {
            len += (self.smoothed_pts[i] - self.smoothed_pts[i-1]).length();
        }
        self.length = len;
    }
}

#[derive(Clone, Debug)]
pub struct RoadNetwork {
    pub segments: Vec<RoadSegment>,
    pub nodes:    HashMap<u32, Vec2>,
    pub next_node_id: u32,
    pub next_seg_id:  u32,
}

impl RoadNetwork {
    pub fn new() -> Self {
        RoadNetwork { segments: Vec::new(), nodes: HashMap::new(), next_node_id: 0, next_seg_id: 0 }
    }

    pub fn add_node(&mut self, pos: Vec2) -> u32 {
        let id = self.next_node_id;
        self.nodes.insert(id, pos);
        self.next_node_id += 1;
        id
    }

    pub fn build_road(
        &mut self,
        hmap:        &Heightmap,
        start_world: Vec2,
        end_world:   Vec2,
        cell_size:   f32,
        road_type:   RoadType,
        cost_params: &RoadCostParams,
    ) -> Option<u32> {
        let start_node = GridNode::new(
            (start_world.x / cell_size) as i32,
            (start_world.y / cell_size) as i32,
        );
        let end_node = GridNode::new(
            (end_world.x / cell_size) as i32,
            (end_world.y / cell_size) as i32,
        );

        let path = astar_path(hmap, start_node, end_node, cost_params)?;
        let smoothed = smooth_road_path(&path, cell_size, 8);
        let control_pts: Vec<Vec2> = path.iter().map(|n| n.to_vec2(cell_size)).collect();

        let width = match road_type {
            RoadType::Dirt    => 3.0,
            RoadType::Gravel  => 4.5,
            RoadType::Paved   => 6.0,
            RoadType::Highway => 12.0,
            RoadType::Trail   => 1.5,
        };

        let sid = self.next_seg_id;
        self.next_seg_id += 1;

        let start_nid = self.add_node(start_world);
        let end_nid   = self.add_node(end_world);

        let mut seg = RoadSegment {
            id: sid,
            control_pts,
            smoothed_pts: smoothed,
            width,
            road_type,
            start_node: start_nid,
            end_node:   end_nid,
            length: 0.0,
        };
        seg.compute_length();
        self.segments.push(seg);
        Some(sid)
    }
}

// ============================================================
//  ATMOSPHERE — RAYLEIGH & MIE SCATTERING
// ============================================================

#[derive(Clone, Debug)]
pub struct AtmosphereParams {
    pub rayleigh_scale_height: f64,
    pub mie_scale_height:      f64,
    pub rayleigh_coeff:        [f64; 3], // per wavelength (R,G,B)
    pub mie_coeff:             f64,
    pub mie_asymmetry:         f64,      // g factor
    pub sun_intensity:         f64,
    pub num_view_samples:      usize,
    pub num_light_samples:     usize,
    pub planet_radius:         f64,      // km
    pub atmo_radius:           f64,      // km
}

impl Default for AtmosphereParams {
    fn default() -> Self {
        AtmosphereParams {
            rayleigh_scale_height: RAYLEIGH_SCALE_HEIGHT,
            mie_scale_height:      MIE_SCALE_HEIGHT,
            rayleigh_coeff:        [RAYLEIGH_R, RAYLEIGH_G, RAYLEIGH_B],
            mie_coeff:             MIE_COEFF,
            mie_asymmetry:         MIE_G,
            sun_intensity:         20.0,
            num_view_samples:      16,
            num_light_samples:     8,
            planet_radius:         EARTH_RADIUS,
            atmo_radius:           ATMO_RADIUS,
        }
    }
}

fn ray_sphere_intersection(
    ray_origin: [f64; 3],
    ray_dir:    [f64; 3],
    sphere_radius: f64,
) -> Option<(f64, f64)> {
    let a = dot3(ray_dir, ray_dir);
    let b = 2.0 * dot3(ray_origin, ray_dir);
    let c = dot3(ray_origin, ray_origin) - sphere_radius * sphere_radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 { return None; }
    let sqrt_disc = disc.sqrt();
    let t0 = (-b - sqrt_disc) / (2.0 * a);
    let t1 = (-b + sqrt_disc) / (2.0 * a);
    Some((t0, t1))
}

#[inline]
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0]*b[0] + a[1]*b[1] + a[2]*b[2]
}

#[inline]
fn normalize3(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
    if len < 1e-15 { return [0.0, 1.0, 0.0]; }
    [v[0]/len, v[1]/len, v[2]/len]
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]+b[0], a[1]+b[1], a[2]+b[2]] }
fn scale3(a: [f64; 3], s: f64) -> [f64; 3] { [a[0]*s, a[1]*s, a[2]*s] }
fn mul3_elem(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]*b[0], a[1]*b[1], a[2]*b[2]] }
fn exp3(a: [f64; 3]) -> [f64; 3] { [a[0].exp(), a[1].exp(), a[2].exp()] }
fn neg3(a: [f64; 3]) -> [f64; 3] { [-a[0], -a[1], -a[2]] }

/// Rayleigh phase function
fn phase_rayleigh(cos_theta: f64) -> f64 {
    (3.0 / (16.0 * std::f64::consts::PI)) * (1.0 + cos_theta * cos_theta)
}

/// Mie phase function (Henyey-Greenstein)
fn phase_mie(cos_theta: f64, g: f64) -> f64 {
    let g2 = g * g;
    (1.0 - g2) / (4.0 * std::f64::consts::PI * (1.0 + g2 - 2.0 * g * cos_theta).powf(1.5))
}

/// Compute sky color for a given view direction and sun direction (all in km space)
/// Returns Vec3 (R, G, B) in linear HDR
pub fn compute_sky_color(
    view_dir: Vec3,
    sun_dir:  Vec3,
    params:   &AtmosphereParams,
) -> Vec3 {
    let planet_r = params.planet_radius;
    let atmo_r   = params.atmo_radius;

    // Camera is on the surface at altitude 0
    let camera_pos = [0.0f64, planet_r + 0.1, 0.0f64]; // 100 m above surface
    let vd = [view_dir.x as f64, view_dir.y as f64, view_dir.z as f64];
    let vd = normalize3(vd);
    let sd = [sun_dir.x as f64, sun_dir.y as f64, sun_dir.z as f64];
    let sd = normalize3(sd);

    // Ray-atmosphere intersection
    let (_, t_max_opt) = match ray_sphere_intersection(camera_pos, vd, atmo_r) {
        Some(v) => v,
        None => return Vec3::ZERO,
    };
    let t_max_opt = t_max_opt.max(0.0);

    // If ray hits planet, clip
    let t_max = if let Some((t0, _)) = ray_sphere_intersection(camera_pos, vd, planet_r + 0.001) {
        if t0 > 0.0 { t0 } else { t_max_opt }
    } else {
        t_max_opt
    };

    let cos_theta = dot3(vd, sd);
    let phase_r   = phase_rayleigh(cos_theta);
    let phase_m   = phase_mie(cos_theta, params.mie_asymmetry);

    let mut total_rayleigh = [0.0f64; 3];
    let mut total_mie      = [0.0f64; 3];

    let ns = params.num_view_samples;
    let seg_len = t_max / ns as f64;

    let mut optical_depth_r = 0.0f64;
    let mut optical_depth_m = 0.0f64;

    for i in 0..ns {
        let t_mid = (i as f64 + 0.5) * seg_len;
        let sample_pos = add3(camera_pos, scale3(vd, t_mid));
        let sample_r = (dot3(sample_pos, sample_pos)).sqrt();
        let height = (sample_r - planet_r).max(0.0);

        let hr = (-(height / params.rayleigh_scale_height)).exp();
        let hm = (-(height / params.mie_scale_height)).exp();

        optical_depth_r += hr * seg_len;
        optical_depth_m += hm * seg_len;

        // Light ray integration
        let (_, t_light) = match ray_sphere_intersection(sample_pos, sd, atmo_r) {
            Some(v) => v,
            None => continue,
        };
        let t_light = t_light.max(0.0);
        let nl = params.num_light_samples;
        let light_seg = t_light / nl as f64;
        let mut od_lr = 0.0f64;
        let mut od_lm = 0.0f64;
        let mut above_planet = true;
        for j in 0..nl {
            let lt = (j as f64 + 0.5) * light_seg;
            let lpos = add3(sample_pos, scale3(sd, lt));
            let lr = (dot3(lpos, lpos)).sqrt();
            if lr < planet_r { above_planet = false; break; }
            let lh = (lr - planet_r).max(0.0);
            od_lr += (-(lh / params.rayleigh_scale_height)).exp() * light_seg;
            od_lm += (-(lh / params.mie_scale_height)).exp() * light_seg;
        }
        if !above_planet { continue; }

        let tau_r = [
            params.rayleigh_coeff[0] * (optical_depth_r + od_lr),
            params.rayleigh_coeff[1] * (optical_depth_r + od_lr),
            params.rayleigh_coeff[2] * (optical_depth_r + od_lr),
        ];
        let tau_m_val = 1.1 * params.mie_coeff * (optical_depth_m + od_lm);
        let tau_m = [tau_m_val; 3];

        let attenuation_r = exp3(neg3(tau_r));
        let attenuation_m = exp3(neg3(tau_m));

        let contrib_r = scale3(attenuation_r, hr * seg_len);
        let contrib_m = scale3(attenuation_m, hm * seg_len);

        for k in 0..3 {
            total_rayleigh[k] += contrib_r[k] * params.rayleigh_coeff[k];
            total_mie[k]      += contrib_m[k] * params.mie_coeff;
        }
    }

    let sun_intensity = params.sun_intensity;
    let color = [
        sun_intensity * (phase_r * total_rayleigh[0] + phase_m * total_mie[0]),
        sun_intensity * (phase_r * total_rayleigh[1] + phase_m * total_mie[1]),
        sun_intensity * (phase_r * total_rayleigh[2] + phase_m * total_mie[2]),
    ];

    Vec3::new(color[0] as f32, color[1] as f32, color[2] as f32)
}

/// Render a sun disk contribution on top of sky color
pub fn sun_disk_color(view_dir: Vec3, sun_dir: Vec3, disk_size: f32, sun_color: Vec3) -> Vec3 {
    let cos_angle = view_dir.dot(sun_dir).clamp(-1.0, 1.0);
    let angle = cos_angle.acos();
    if angle < disk_size {
        // Smooth edge using a limb-darkening approximation
        let t     = (angle / disk_size).clamp(0.0, 1.0);
        let limb  = 1.0 - 0.6 * t.sqrt(); // limb darkening coefficient ~0.6
        sun_color * limb
    } else {
        Vec3::ZERO
    }
}

/// Apply tone mapping (ACES filmic approximation)
pub fn aces_tonemap(color: Vec3) -> Vec3 {
    let a = 2.51f32;
    let b = 0.03f32;
    let c = 2.43f32;
    let d = 0.59f32;
    let e = 0.14f32;
    let result = (color * (color * a + Vec3::splat(b)))
        / (color * (color * c + Vec3::splat(d)) + Vec3::splat(e));
    result.clamp(Vec3::ZERO, Vec3::ONE)
}

/// Simple Reinhard tone mapping
pub fn reinhard_tonemap(color: Vec3) -> Vec3 {
    color / (Vec3::ONE + color)
}

// ============================================================
//  DAY / NIGHT — SOLAR MATH
// ============================================================

/// Compute solar declination for a given day of year (1-365)
/// Returns angle in degrees
pub fn solar_declination(day_of_year: f64) -> f64 {
    // Spencer (1971) formula — accurate to ±0.3°
    let b = 360.0 / 365.0 * (day_of_year - 81.0);
    let b_rad = b * std::f64::consts::PI / 180.0;
    SOLAR_OBLIQUITY * b_rad.sin()
}

/// Equation of time (minutes) for a given day of year
pub fn equation_of_time(day_of_year: f64) -> f64 {
    let b = 360.0 / 365.0 * (day_of_year - 81.0);
    let b_rad = b * std::f64::consts::PI / 180.0;
    9.87 * (2.0 * b_rad).sin() - 7.53 * b_rad.cos() - 1.5 * b_rad.sin()
}

/// Compute hour angle (degrees) from longitude, solar time, and equation of time
pub fn hour_angle(longitude_deg: f64, solar_time_hours: f64) -> f64 {
    15.0 * (solar_time_hours - 12.0)
}

/// Compute solar altitude and azimuth angles (degrees) from observer's position and time
/// Returns (altitude_deg, azimuth_deg)
pub fn solar_position(
    latitude_deg:  f64,
    longitude_deg: f64,
    day_of_year:   f64,
    utc_hour:      f64,
) -> (f64, f64) {
    let decl = solar_declination(day_of_year) * std::f64::consts::PI / 180.0;
    let lat  = latitude_deg                   * std::f64::consts::PI / 180.0;
    let eot  = equation_of_time(day_of_year);
    let solar_time = utc_hour + longitude_deg / 15.0 + eot / 60.0;
    let ha   = hour_angle(longitude_deg, solar_time) * std::f64::consts::PI / 180.0;

    let sin_alt = decl.sin() * lat.sin() + decl.cos() * lat.cos() * ha.cos();
    let altitude = sin_alt.asin() * 180.0 / std::f64::consts::PI;

    let cos_az = (decl.sin() * lat.cos() - decl.cos() * lat.sin() * ha.cos())
               / (1.0 - sin_alt * sin_alt).sqrt().max(1e-10);
    let azimuth_rad = cos_az.acos();
    let azimuth = if ha > 0.0 { 360.0 - azimuth_rad * 180.0 / std::f64::consts::PI }
                  else        { azimuth_rad * 180.0 / std::f64::consts::PI };

    (altitude, azimuth)
}

/// Compute sunrise and sunset UTC hours for a given lat/lon and day of year
/// Returns (sunrise_utc, sunset_utc) or None if sun never rises/sets
pub fn sunrise_sunset(
    latitude_deg:  f64,
    longitude_deg: f64,
    day_of_year:   f64,
) -> Option<(f64, f64)> {
    let decl = solar_declination(day_of_year) * std::f64::consts::PI / 180.0;
    let lat  = latitude_deg                   * std::f64::consts::PI / 180.0;
    let eot  = equation_of_time(day_of_year);

    // Hour angle at sunrise/sunset (solar altitude = -0.833° accounting for refraction)
    let h_arg = (-0.01454 - decl.sin() * lat.sin()) / (decl.cos() * lat.cos());
    if h_arg.abs() > 1.0 { return None; } // polar day/night
    let ha0_deg = h_arg.acos() * 180.0 / std::f64::consts::PI;

    let solar_noon_utc = 12.0 - longitude_deg / 15.0 - eot / 60.0;
    let half_day = ha0_deg / 15.0;
    Some((solar_noon_utc - half_day, solar_noon_utc + half_day))
}

/// Convert solar altitude/azimuth to a world-space direction vector
pub fn solar_direction_vec(altitude_deg: f64, azimuth_deg: f64) -> Vec3 {
    let alt = (altitude_deg as f32) * DEG2RAD;
    let az  = (azimuth_deg  as f32) * DEG2RAD;
    let cos_alt = alt.cos();
    Vec3::new(cos_alt * az.sin(), alt.sin(), cos_alt * az.cos())
}

#[derive(Clone, Debug)]
pub struct SolarState {
    pub altitude_deg:  f64,
    pub azimuth_deg:   f64,
    pub direction:     Vec3,
    pub is_day:        bool,
    pub sun_color:     Vec3,
    pub sun_intensity: f32,
    pub sky_color:     Vec3,
    pub ambient_color: Vec3,
}

impl SolarState {
    pub fn compute(
        latitude:  f64,
        longitude: f64,
        doy:       f64,
        utc_hour:  f64,
        atmo:      &AtmosphereParams,
    ) -> Self {
        let (alt, az) = solar_position(latitude, longitude, doy, utc_hour);
        let dir       = solar_direction_vec(alt, az);
        let is_day    = alt > -0.833;

        // Sun color: yellower near horizon (atmospheric reddening approximation)
        let elevation_factor = (alt as f32 / 90.0 + 0.1).clamp(0.0, 1.0);
        let sun_color = Vec3::new(
            1.0,
            0.8 + 0.2 * elevation_factor,
            0.5 + 0.5 * elevation_factor,
        );
        let sun_intensity = if is_day {
            ((alt as f32 * DEG2RAD).sin().max(0.0)).sqrt() * 100.0
        } else {
            0.0
        };

        let sky_color = if is_day {
            let sky = compute_sky_color(dir * -1.0, dir, atmo); // view dir is up
            let view_up = Vec3::Y;
            let raw = compute_sky_color(view_up, dir, atmo);
            aces_tonemap(raw)
        } else {
            Vec3::new(0.005, 0.005, 0.02)
        };

        let ambient_color = sky_color * 0.3 + Vec3::new(0.02, 0.02, 0.04);

        SolarState { altitude_deg: alt, azimuth_deg: az, direction: dir, is_day, sun_color, sun_intensity, sky_color, ambient_color }
    }
}

// ============================================================
//  WEATHER MARKOV CHAIN
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeatherState {
    Clear    = 0,
    Cloudy   = 1,
    Rain     = 2,
    Storm    = 3,
    Snow     = 4,
}

impl WeatherState {
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => WeatherState::Clear,
            1 => WeatherState::Cloudy,
            2 => WeatherState::Rain,
            3 => WeatherState::Storm,
            4 => WeatherState::Snow,
            _ => WeatherState::Clear,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            WeatherState::Clear  => "Clear",
            WeatherState::Cloudy => "Cloudy",
            WeatherState::Rain   => "Rain",
            WeatherState::Storm  => "Storm",
            WeatherState::Snow   => "Snow",
        }
    }
}

/// Transition matrix: rows = current state, cols = next state, values = probability
/// Order: Clear, Cloudy, Rain, Storm, Snow
pub const WEATHER_TRANSITION_MATRIX: [[f32; 5]; 5] = [
    // Clear -> Clear  Cloudy  Rain  Storm  Snow
    [0.60,  0.28,   0.08,  0.02,  0.02],
    // Cloudy
    [0.25,  0.40,   0.25,  0.07,  0.03],
    // Rain
    [0.10,  0.30,   0.40,  0.15,  0.05],
    // Storm
    [0.05,  0.20,   0.35,  0.30,  0.10],
    // Snow
    [0.08,  0.25,   0.10,  0.05,  0.52],
];

#[derive(Clone, Debug)]
pub struct WeatherSnapshot {
    pub state:           WeatherState,
    pub temperature_c:   f32,
    pub wind_speed_ms:   f32,
    pub wind_dir_deg:    f32,
    pub precipitation_mm: f32,
    pub cloud_cover:     f32,   // 0..1
    pub visibility_km:   f32,
    pub humidity:        f32,
    pub pressure_hpa:    f32,
    pub fog_density:     f32,
    pub lightning_chance: f32,
}

impl WeatherSnapshot {
    pub fn clear(temp: f32) -> Self {
        WeatherSnapshot {
            state: WeatherState::Clear,
            temperature_c: temp,
            wind_speed_ms: 2.0,
            wind_dir_deg:  0.0,
            precipitation_mm: 0.0,
            cloud_cover:   0.05,
            visibility_km: 50.0,
            humidity:      0.30,
            pressure_hpa:  1013.25,
            fog_density:   0.0,
            lightning_chance: 0.0,
        }
    }
}

pub struct WeatherSystem {
    pub current:    WeatherSnapshot,
    pub history:    VecDeque<WeatherSnapshot>,
    pub max_history: usize,
    pub rng:        LcgRng,
    pub base_temp:  f32,
    pub season:     f32,  // 0..1, 0 = winter, 0.5 = summer
    pub latitude:   f32,
}

impl WeatherSystem {
    pub fn new(seed: u64, base_temp: f32, latitude: f32) -> Self {
        WeatherSystem {
            current:     WeatherSnapshot::clear(base_temp),
            history:     VecDeque::with_capacity(256),
            max_history: 256,
            rng:         LcgRng::new(seed),
            base_temp,
            season:      0.25, // spring
            latitude,
        }
    }

    /// Advance weather by one time step using Markov chain transitions
    pub fn step(&mut self, hours_elapsed: f32) {
        // Determine how many steps to apply (one step per simulated hour)
        let steps = (hours_elapsed as usize).max(1);

        for _ in 0..steps {
            let cur_idx = self.current.state as usize;
            let row     = &WEATHER_TRANSITION_MATRIX[cur_idx];

            // Weighted random transition
            let r = self.rng.next_f32();
            let mut cum = 0.0f32;
            let mut next_state = self.current.state;
            for (i, &p) in row.iter().enumerate() {
                cum += p;
                if r < cum {
                    next_state = WeatherState::from_index(i);
                    break;
                }
            }

            // Generate consistent meteorological variables for new state
            let temp = self.compute_temperature(next_state);
            let snap = self.generate_snapshot(next_state, temp);

            self.history.push_back(self.current.clone());
            if self.history.len() > self.max_history {
                self.history.pop_front();
            }
            self.current = snap;
        }
    }

    fn compute_temperature(&mut self, state: WeatherState) -> f32 {
        // Seasonal adjustment: ±15°C from base temperature
        let seasonal_bias = (self.season * TWO_PI).sin() * 15.0;
        // Latitude cooling: roughly -0.5°C per degree from equator
        let lat_bias = -(self.latitude.abs() * 0.5);
        let weather_bias = match state {
            WeatherState::Clear  =>  2.0,
            WeatherState::Cloudy => -1.0,
            WeatherState::Rain   => -3.0,
            WeatherState::Storm  => -5.0,
            WeatherState::Snow   => -8.0,
        };
        let noise = (self.rng.next_f32() - 0.5) * 4.0;
        self.base_temp + seasonal_bias + lat_bias + weather_bias + noise
    }

    fn generate_snapshot(&mut self, state: WeatherState, temp: f32) -> WeatherSnapshot {
        let r = |rng: &mut LcgRng| rng.next_f32();
        match state {
            WeatherState::Clear => WeatherSnapshot {
                state,
                temperature_c:    temp,
                wind_speed_ms:    r(&mut self.rng) * 5.0,
                wind_dir_deg:     r(&mut self.rng) * 360.0,
                precipitation_mm: 0.0,
                cloud_cover:      r(&mut self.rng) * 0.15,
                visibility_km:    40.0 + r(&mut self.rng) * 30.0,
                humidity:         0.20 + r(&mut self.rng) * 0.25,
                pressure_hpa:     1015.0 + r(&mut self.rng) * 10.0,
                fog_density:      0.0,
                lightning_chance: 0.0,
            },
            WeatherState::Cloudy => WeatherSnapshot {
                state,
                temperature_c:    temp,
                wind_speed_ms:    2.0 + r(&mut self.rng) * 8.0,
                wind_dir_deg:     r(&mut self.rng) * 360.0,
                precipitation_mm: 0.0,
                cloud_cover:      0.50 + r(&mut self.rng) * 0.40,
                visibility_km:    15.0 + r(&mut self.rng) * 25.0,
                humidity:         0.50 + r(&mut self.rng) * 0.25,
                pressure_hpa:     1005.0 + r(&mut self.rng) * 10.0,
                fog_density:      r(&mut self.rng) * 0.1,
                lightning_chance: 0.0,
            },
            WeatherState::Rain => WeatherSnapshot {
                state,
                temperature_c:    temp,
                wind_speed_ms:    5.0 + r(&mut self.rng) * 10.0,
                wind_dir_deg:     r(&mut self.rng) * 360.0,
                precipitation_mm: 1.0 + r(&mut self.rng) * 8.0,
                cloud_cover:      0.75 + r(&mut self.rng) * 0.25,
                visibility_km:    3.0 + r(&mut self.rng) * 7.0,
                humidity:         0.75 + r(&mut self.rng) * 0.20,
                pressure_hpa:     995.0 + r(&mut self.rng) * 10.0,
                fog_density:      0.1 + r(&mut self.rng) * 0.2,
                lightning_chance: 0.05,
            },
            WeatherState::Storm => WeatherSnapshot {
                state,
                temperature_c:    temp,
                wind_speed_ms:    15.0 + r(&mut self.rng) * 30.0,
                wind_dir_deg:     r(&mut self.rng) * 360.0,
                precipitation_mm: 8.0 + r(&mut self.rng) * 25.0,
                cloud_cover:      0.90 + r(&mut self.rng) * 0.10,
                visibility_km:    0.2 + r(&mut self.rng) * 2.0,
                humidity:         0.90 + r(&mut self.rng) * 0.10,
                pressure_hpa:     975.0 + r(&mut self.rng) * 15.0,
                fog_density:      0.3 + r(&mut self.rng) * 0.4,
                lightning_chance: 0.40 + r(&mut self.rng) * 0.40,
            },
            WeatherState::Snow => WeatherSnapshot {
                state,
                temperature_c:    temp.min(-1.0),
                wind_speed_ms:    3.0 + r(&mut self.rng) * 15.0,
                wind_dir_deg:     r(&mut self.rng) * 360.0,
                precipitation_mm: 0.5 + r(&mut self.rng) * 4.0,
                cloud_cover:      0.70 + r(&mut self.rng) * 0.30,
                visibility_km:    0.5 + r(&mut self.rng) * 4.0,
                humidity:         0.60 + r(&mut self.rng) * 0.30,
                pressure_hpa:     1000.0 + r(&mut self.rng) * 15.0,
                fog_density:      0.15 + r(&mut self.rng) * 0.25,
                lightning_chance: 0.02,
            },
        }
    }

    /// Linearly interpolate two weather snapshots (for smooth transitions)
    pub fn interpolate_snapshots(a: &WeatherSnapshot, b: &WeatherSnapshot, t: f32) -> WeatherSnapshot {
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        WeatherSnapshot {
            state:            if t < 0.5 { a.state } else { b.state },
            temperature_c:    lerp(a.temperature_c,    b.temperature_c),
            wind_speed_ms:    lerp(a.wind_speed_ms,    b.wind_speed_ms),
            wind_dir_deg:     lerp(a.wind_dir_deg,     b.wind_dir_deg),
            precipitation_mm: lerp(a.precipitation_mm, b.precipitation_mm),
            cloud_cover:      lerp(a.cloud_cover,      b.cloud_cover),
            visibility_km:    lerp(a.visibility_km,    b.visibility_km),
            humidity:         lerp(a.humidity,         b.humidity),
            pressure_hpa:     lerp(a.pressure_hpa,     b.pressure_hpa),
            fog_density:      lerp(a.fog_density,      b.fog_density),
            lightning_chance: lerp(a.lightning_chance, b.lightning_chance),
        }
    }

    /// Compute wind vector from speed and direction
    pub fn wind_vector(&self) -> Vec2 {
        let dir_rad = self.current.wind_dir_deg * DEG2RAD;
        Vec2::new(dir_rad.cos(), dir_rad.sin()) * self.current.wind_speed_ms
    }

    /// Check if it's snowing (temperature below 0 with precipitation)
    pub fn is_snowing(&self) -> bool {
        self.current.state == WeatherState::Snow
        || (self.current.temperature_c < 0.0 && self.current.precipitation_mm > 0.5)
    }

    /// Advance season (0 = winter, 1 = winter again after full year)
    pub fn advance_season(&mut self, delta_fraction: f32) {
        self.season = (self.season + delta_fraction) % 1.0;
    }
}

// ============================================================
//  UNDO / REDO SYSTEM
// ============================================================

#[derive(Debug)]
pub enum EditAction {
    SetHeightRegion {
        x: usize, y: usize,
        width: usize, height: usize,
        old_data: Vec<f32>,
        new_data: Vec<f32>,
    },
    PlaceFoliageInstances {
        instances: Vec<FoliageInstance>,
        indices:   Vec<usize>,
    },
    RemoveFoliageInstances {
        indices:   Vec<usize>,
        instances: Vec<FoliageInstance>,
    },
    AddRoadSegment {
        segment_id: u32,
        segment:    RoadSegment,
    },
    RemoveRoadSegment {
        segment_id: u32,
        segment:    RoadSegment,
    },
    SetBiomeOverride {
        x: usize, y: usize,
        old_biome: Option<BiomeId>,
        new_biome: Option<BiomeId>,
    },
    AddWaterBody {
        lake: LakeBody,
        index: usize,
    },
    RemoveWaterBody {
        lake: LakeBody,
        index: usize,
    },
    CompoundAction {
        actions: Vec<EditAction>,
        description: String,
    },
}

pub struct UndoRedoStack {
    pub undo_stack: Vec<EditAction>,
    pub redo_stack: Vec<EditAction>,
    pub max_depth:  usize,
}

impl UndoRedoStack {
    pub fn new(max_depth: usize) -> Self {
        UndoRedoStack { undo_stack: Vec::new(), redo_stack: Vec::new(), max_depth }
    }

    pub fn push(&mut self, action: EditAction) {
        self.redo_stack.clear();
        if self.undo_stack.len() >= self.max_depth {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(action);
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    pub fn pop_undo(&mut self) -> Option<EditAction> {
        let a = self.undo_stack.pop()?;
        Some(a)
    }

    pub fn push_redo(&mut self, action: EditAction) {
        self.redo_stack.push(action);
    }

    pub fn pop_redo(&mut self) -> Option<EditAction> {
        self.redo_stack.pop()
    }
}

// ============================================================
//  SELECTION SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SelectionItem {
    TerrainCell(usize, usize),
    FoliageInstance(usize),
    RoadSegment(u32),
    WaterBody(usize),
    RiverPath(usize),
    BiomeZone(BiomeId),
}

#[derive(Clone, Debug)]
pub struct SelectionState {
    pub items:        HashSet<SelectionItem>,
    pub pivot:        Option<Vec3>,
    pub aabb_min:     Vec3,
    pub aabb_max:     Vec3,
    pub mode:         SelectionMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    Single,
    Multi,
    Box,
    Paint,
}

impl SelectionState {
    pub fn new() -> Self {
        SelectionState {
            items:    HashSet::new(),
            pivot:    None,
            aabb_min: Vec3::splat(f32::MAX),
            aabb_max: Vec3::splat(f32::MIN),
            mode:     SelectionMode::Single,
        }
    }

    pub fn select(&mut self, item: SelectionItem) {
        if self.mode == SelectionMode::Single { self.items.clear(); }
        self.items.insert(item);
    }

    pub fn deselect(&mut self, item: &SelectionItem) {
        self.items.remove(item);
    }

    pub fn toggle(&mut self, item: SelectionItem) {
        if self.items.contains(&item) { self.items.remove(&item); }
        else { self.items.insert(item); }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.pivot = None;
    }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }
    pub fn len(&self) -> usize { self.items.len() }

    /// Box selection — add all terrain cells within a 2D bounding rectangle
    pub fn box_select_terrain(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let (lx, rx) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
        let (ly, ry) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
        for y in ly..=ry {
            for x in lx..=rx {
                self.items.insert(SelectionItem::TerrainCell(x, y));
            }
        }
    }

    pub fn count_terrain_cells(&self) -> usize {
        self.items.iter().filter(|i| matches!(i, SelectionItem::TerrainCell(..)) ).count()
    }
}

// ============================================================
//  SERIALIZATION HELPERS
// ============================================================

#[derive(Clone, Debug)]
pub struct SerializedWorld {
    pub version:    u32,
    pub width:      usize,
    pub height:     usize,
    pub cell_size:  f32,
    pub heightmap:  Vec<f32>,
    pub biome_map:  Vec<u8>,
    pub rivers:     Vec<SerializedRiver>,
    pub lakes:      Vec<SerializedLake>,
    pub roads:      Vec<SerializedRoad>,
    pub foliage:    Vec<SerializedFoliage>,
    pub sea_level:  f32,
    pub world_name: String,
    pub metadata:   HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct SerializedRiver {
    pub points: Vec<[f32; 2]>,
    pub widths: Vec<f32>,
    pub depths: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct SerializedLake {
    pub water_level: f32,
    pub centroid:    [f32; 2],
    pub volume:      f32,
    pub surface_area: f32,
}

#[derive(Clone, Debug)]
pub struct SerializedRoad {
    pub id:        u32,
    pub road_type: u8,
    pub width:     f32,
    pub points:    Vec<[f32; 2]>,
    pub length:    f32,
}

#[derive(Clone, Debug)]
pub struct SerializedFoliage {
    pub asset_id:  u32,
    pub biome_id:  u8,
    pub position:  [f32; 3],
    pub rotation:  [f32; 4],
    pub scale:     [f32; 3],
}

impl SerializedWorld {
    pub fn from_editor(editor: &WorldEditor) -> Self {
        let heightmap = editor.heightmap.data.clone();
        let w = editor.heightmap.width;
        let h = editor.heightmap.height;

        let mut biome_map = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let temp     = editor.temperature_map[y * w + x];
                let humidity = editor.humidity_map[y * w + x];
                let altitude = editor.heightmap.get(x, y);
                let biome    = BiomeDescriptor::classify_point(temp, humidity, altitude);
                biome_map[y * w + x] = biome as u8;
            }
        }

        let rivers: Vec<SerializedRiver> = editor.rivers.iter().map(|r| {
            SerializedRiver {
                points: r.points.iter().map(|p| [p.x, p.y]).collect(),
                widths: r.widths.clone(),
                depths: r.depths.clone(),
            }
        }).collect();

        let lakes: Vec<SerializedLake> = editor.lakes.iter().map(|l| {
            SerializedLake {
                water_level:  l.water_level,
                centroid:     [l.centroid.x, l.centroid.y],
                volume:       l.volume,
                surface_area: l.surface_area,
            }
        }).collect();

        let roads: Vec<SerializedRoad> = editor.road_network.segments.iter().map(|seg| {
            SerializedRoad {
                id:        seg.id,
                road_type: seg.road_type.clone() as u8,
                width:     seg.width,
                points:    seg.smoothed_pts.iter().map(|p| [p.x, p.y]).collect(),
                length:    seg.length,
            }
        }).collect();

        let foliage: Vec<SerializedFoliage> = editor.foliage.iter().map(|fi| {
            SerializedFoliage {
                asset_id: fi.asset_id,
                biome_id: fi.biome_id,
                position: [fi.position.x, fi.position.y, fi.position.z],
                rotation: [fi.rotation.x, fi.rotation.y, fi.rotation.z, fi.rotation.w],
                scale:    [fi.scale.x, fi.scale.y, fi.scale.z],
            }
        }).collect();

        SerializedWorld {
            version: 1,
            width: w,
            height: h,
            cell_size: editor.cell_size,
            heightmap,
            biome_map,
            rivers,
            lakes,
            roads,
            foliage,
            sea_level: editor.sea_level,
            world_name: editor.world_name.clone(),
            metadata: editor.metadata.clone(),
        }
    }

    /// Serialize to bytes (simple binary format)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Header
        buf.extend_from_slice(b"WRLD");
        push_u32(&mut buf, self.version);
        push_u32(&mut buf, self.width  as u32);
        push_u32(&mut buf, self.height as u32);
        push_f32(&mut buf, self.cell_size);
        push_f32(&mut buf, self.sea_level);

        // Heightmap
        push_u32(&mut buf, self.heightmap.len() as u32);
        for &v in &self.heightmap { push_f32(&mut buf, v); }

        // Biome map
        push_u32(&mut buf, self.biome_map.len() as u32);
        buf.extend_from_slice(&self.biome_map);

        // Name
        let name_bytes = self.world_name.as_bytes();
        push_u32(&mut buf, name_bytes.len() as u32);
        buf.extend_from_slice(name_bytes);

        // Rivers
        push_u32(&mut buf, self.rivers.len() as u32);
        for river in &self.rivers {
            push_u32(&mut buf, river.points.len() as u32);
            for &[px, py] in &river.points { push_f32(&mut buf, px); push_f32(&mut buf, py); }
            for &w in &river.widths { push_f32(&mut buf, w); }
            for &d in &river.depths { push_f32(&mut buf, d); }
        }

        // Lakes
        push_u32(&mut buf, self.lakes.len() as u32);
        for lake in &self.lakes {
            push_f32(&mut buf, lake.water_level);
            push_f32(&mut buf, lake.centroid[0]);
            push_f32(&mut buf, lake.centroid[1]);
            push_f32(&mut buf, lake.volume);
            push_f32(&mut buf, lake.surface_area);
        }

        // Roads
        push_u32(&mut buf, self.roads.len() as u32);
        for road in &self.roads {
            push_u32(&mut buf, road.id);
            buf.push(road.road_type);
            push_f32(&mut buf, road.width);
            push_f32(&mut buf, road.length);
            push_u32(&mut buf, road.points.len() as u32);
            for &[px, py] in &road.points { push_f32(&mut buf, px); push_f32(&mut buf, py); }
        }

        // Foliage
        push_u32(&mut buf, self.foliage.len() as u32);
        for fi in &self.foliage {
            push_u32(&mut buf, fi.asset_id);
            buf.push(fi.biome_id);
            for &v in &fi.position { push_f32(&mut buf, v); }
            for &v in &fi.rotation { push_f32(&mut buf, v); }
            for &v in &fi.scale    { push_f32(&mut buf, v); }
        }

        buf
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut cursor = 0usize;

        if data.len() < 4 { return None; }
        if &data[0..4] != b"WRLD" { return None; }
        cursor += 4;

        let version  = read_u32(data, &mut cursor)?;
        let width    = read_u32(data, &mut cursor)? as usize;
        let height   = read_u32(data, &mut cursor)? as usize;
        let cell_size = read_f32(data, &mut cursor)?;
        let sea_level = read_f32(data, &mut cursor)?;

        let hmap_len = read_u32(data, &mut cursor)? as usize;
        let mut heightmap = Vec::with_capacity(hmap_len);
        for _ in 0..hmap_len {
            heightmap.push(read_f32(data, &mut cursor)?);
        }

        let biome_len = read_u32(data, &mut cursor)? as usize;
        if cursor + biome_len > data.len() { return None; }
        let biome_map = data[cursor..cursor + biome_len].to_vec();
        cursor += biome_len;

        let name_len = read_u32(data, &mut cursor)? as usize;
        if cursor + name_len > data.len() { return None; }
        let world_name = String::from_utf8(data[cursor..cursor + name_len].to_vec()).ok()?;
        cursor += name_len;

        // Simplified: skip remaining for brevity in deserialization
        Some(SerializedWorld {
            version,
            width,
            height,
            cell_size,
            heightmap,
            biome_map,
            rivers: Vec::new(),
            lakes: Vec::new(),
            roads: Vec::new(),
            foliage: Vec::new(),
            sea_level,
            world_name,
            metadata: HashMap::new(),
        })
    }
}

fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn push_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_bits().to_le_bytes());
}
fn read_u32(data: &[u8], cursor: &mut usize) -> Option<u32> {
    if *cursor + 4 > data.len() { return None; }
    let v = u32::from_le_bytes(data[*cursor..*cursor+4].try_into().ok()?);
    *cursor += 4;
    Some(v)
}
fn read_f32(data: &[u8], cursor: &mut usize) -> Option<f32> {
    let bits = read_u32(data, cursor)?;
    Some(f32::from_bits(bits))
}

// ============================================================
//  EDITOR TOOL MODES
// ============================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    TerrainRaise,
    TerrainLower,
    TerrainSmooth,
    TerrainFlatten,
    TerrainPaint,
    TerrainErode,
    FoliagePaint,
    FoliageErase,
    RoadDraw,
    WaterPaint,
    BiomePaint,
    MeasureTool,
    ViewOnly,
}

#[derive(Clone, Debug)]
pub struct BrushSettings {
    pub radius:    f32,
    pub strength:  f32,
    pub falloff:   BrushFalloff,
    pub scatter:   f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrushFalloff {
    Linear,
    Smooth,
    Constant,
    Spike,
}

impl BrushSettings {
    pub fn weight_at_radius(&self, dist: f32) -> f32 {
        let t = (dist / self.radius).clamp(0.0, 1.0);
        match self.falloff {
            BrushFalloff::Linear   => (1.0 - t) * self.strength,
            BrushFalloff::Smooth   => { let s = 1.0 - t; s * s * (3.0 - 2.0 * s) * self.strength }
            BrushFalloff::Constant => self.strength,
            BrushFalloff::Spike    => (1.0 - t * t) * self.strength,
        }
    }
}

// ============================================================
//  TERRAIN OPERATIONS (editor-level)
// ============================================================

/// Apply a brush raise/lower operation to the heightmap
pub fn terrain_brush_raise(
    hmap:    &mut Heightmap,
    cx:      f32,
    cy:      f32,
    brush:   &BrushSettings,
    delta:   f32,
) -> EditAction {
    let r  = brush.radius.ceil() as i32;
    let cx_i = cx as i32;
    let cy_i = cy as i32;

    let x0 = (cx_i - r).max(0) as usize;
    let y0 = (cy_i - r).max(0) as usize;
    let x1 = (cx_i + r).min(hmap.width  as i32 - 1) as usize;
    let y1 = (cy_i + r).min(hmap.height as i32 - 1) as usize;

    let width  = x1 - x0 + 1;
    let height = y1 - y0 + 1;

    let mut old_data = Vec::with_capacity(width * height);
    for y in y0..=y1 {
        for x in x0..=x1 {
            old_data.push(hmap.get(x, y));
        }
    }

    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx*dx + dy*dy).sqrt();
            if dist <= brush.radius {
                let w   = brush.weight_at_radius(dist);
                let old = hmap.get(x, y);
                hmap.set(x, y, (old + delta * w).clamp(0.0, 1.0));
            }
        }
    }

    let mut new_data = Vec::with_capacity(width * height);
    for y in y0..=y1 {
        for x in x0..=x1 {
            new_data.push(hmap.get(x, y));
        }
    }

    EditAction::SetHeightRegion { x: x0, y: y0, width, height, old_data, new_data }
}

/// Smooth terrain in brush region (box filter)
pub fn terrain_brush_smooth(
    hmap:  &mut Heightmap,
    cx:    f32,
    cy:    f32,
    brush: &BrushSettings,
    iterations: usize,
) -> EditAction {
    let r    = brush.radius.ceil() as i32;
    let cx_i = cx as i32;
    let cy_i = cy as i32;

    let x0 = (cx_i - r).max(0) as usize;
    let y0 = (cy_i - r).max(0) as usize;
    let x1 = (cx_i + r).min(hmap.width  as i32 - 1) as usize;
    let y1 = (cy_i + r).min(hmap.height as i32 - 1) as usize;

    let width  = x1 - x0 + 1;
    let height_r = y1 - y0 + 1;

    let mut old_data = Vec::with_capacity(width * height_r);
    for y in y0..=y1 {
        for x in x0..=x1 {
            old_data.push(hmap.get(x, y));
        }
    }

    for _iter in 0..iterations {
        let copy = hmap.data.clone();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx*dx + dy*dy).sqrt();
                if dist > brush.radius { continue; }
                let w = brush.weight_at_radius(dist);

                let xi = x as i32;
                let yi = y as i32;
                let sum = copy[hmap.index(x, y)]
                        + hmap.get_clamped(xi-1, yi)
                        + hmap.get_clamped(xi+1, yi)
                        + hmap.get_clamped(xi, yi-1)
                        + hmap.get_clamped(xi, yi+1);
                let avg = sum / 5.0;
                let old = copy[hmap.index(x, y)];
                hmap.set(x, y, old + (avg - old) * w);
            }
        }
    }

    let mut new_data = Vec::with_capacity(width * height_r);
    for y in y0..=y1 {
        for x in x0..=x1 {
            new_data.push(hmap.get(x, y));
        }
    }

    EditAction::SetHeightRegion { x: x0, y: y0, width, height: height_r, old_data, new_data }
}

/// Flatten terrain toward a target height
pub fn terrain_brush_flatten(
    hmap:          &mut Heightmap,
    cx:            f32,
    cy:            f32,
    brush:         &BrushSettings,
    target_height: f32,
) -> EditAction {
    let r    = brush.radius.ceil() as i32;
    let cx_i = cx as i32;
    let cy_i = cy as i32;

    let x0 = (cx_i - r).max(0) as usize;
    let y0 = (cy_i - r).max(0) as usize;
    let x1 = (cx_i + r).min(hmap.width  as i32 - 1) as usize;
    let y1 = (cy_i + r).min(hmap.height as i32 - 1) as usize;

    let width  = x1 - x0 + 1;
    let height = y1 - y0 + 1;

    let mut old_data = Vec::with_capacity(width * height);
    for y in y0..=y1 { for x in x0..=x1 { old_data.push(hmap.get(x, y)); } }

    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx*dx + dy*dy).sqrt();
            if dist > brush.radius { continue; }
            let w   = brush.weight_at_radius(dist);
            let old = hmap.get(x, y);
            hmap.set(x, y, old + (target_height - old) * w);
        }
    }

    let mut new_data = Vec::with_capacity(width * height);
    for y in y0..=y1 { for x in x0..=x1 { new_data.push(hmap.get(x, y)); } }

    EditAction::SetHeightRegion { x: x0, y: y0, width, height, old_data, new_data }
}

/// Apply stamp (add a precomputed height kernel to a region)
pub fn terrain_stamp(
    hmap:    &mut Heightmap,
    cx:      f32,
    cy:      f32,
    stamp:   &[f32],
    sw:      usize,
    sh:      usize,
    scale:   f32,
) -> EditAction {
    let x0 = ((cx - sw as f32 * 0.5) as i32).max(0) as usize;
    let y0 = ((cy - sh as f32 * 0.5) as i32).max(0) as usize;
    let x1 = (x0 + sw).min(hmap.width);
    let y1 = (y0 + sh).min(hmap.height);

    let width  = x1 - x0;
    let height = y1 - y0;

    let mut old_data = Vec::with_capacity(width * height);
    for y in y0..y1 { for x in x0..x1 { old_data.push(hmap.get(x, y)); } }

    for y in y0..y1 {
        for x in x0..x1 {
            let si = (y - y0) * sw + (x - x0);
            if si < stamp.len() {
                let old = hmap.get(x, y);
                hmap.set(x, y, (old + stamp[si] * scale).clamp(0.0, 1.0));
            }
        }
    }

    let mut new_data = Vec::with_capacity(width * height);
    for y in y0..y1 { for x in x0..x1 { new_data.push(hmap.get(x, y)); } }

    EditAction::SetHeightRegion { x: x0, y: y0, width, height, old_data, new_data }
}

// ============================================================
//  TEMPERATURE & HUMIDITY MAP GENERATION
// ============================================================

pub fn generate_temperature_map(
    hmap:        &Heightmap,
    base_temp:   f32,
    latitude:    f32,
    noise_scale: f32,
    seed:        u64,
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut temp_map = vec![0.0f32; w * h];

    // Temperature decreases with altitude (environmental lapse rate: ~6.5°C per km)
    // Assume 1 unit height = 1000 m
    let lapse_rate = 6.5f32;

    // Latitude effect: cooler at poles
    // We treat y axis as N-S gradient
    let lat_range = 60.0f32; // ±60° simulation range

    let fbm_params = FbmParams { octaves: 4, frequency: noise_scale, lacunarity: 2.0, gain: 0.5, amplitude: 5.0, offset: 0.0, ridge: false };

    for y in 0..h {
        for x in 0..w {
            let nx = x as f32 / w as f32 + (seed as f32 * 0.0001);
            let ny = y as f32 / h as f32;
            let altitude = hmap.get(x, y);

            // Latitude gradient: y=0 -> -lat_range, y=h -> +lat_range
            let lat_factor = (ny - 0.5) * 2.0 * lat_range + latitude;
            let lat_temp   = base_temp - lat_factor.abs() * 0.5;

            // Altitude cooling
            let alt_cooling = altitude * lapse_rate * 5.0; // scale factor for normalised heights

            // Noise variation
            let noise_var = fbm_2d(nx * 3.0, ny * 3.0, &fbm_params);

            temp_map[y * w + x] = lat_temp - alt_cooling + noise_var;
        }
    }
    temp_map
}

pub fn generate_humidity_map(
    hmap:        &Heightmap,
    sea_level:   f32,
    noise_scale: f32,
    seed:        u64,
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut hum_map = vec![0.0f32; w * h];

    let fbm_params = FbmParams { octaves: 5, frequency: noise_scale, lacunarity: 2.0, gain: 0.5, amplitude: 1.0, offset: 0.0, ridge: false };

    // Simple humidity: higher near sea, lower far from water, noise variation
    // First pass: mark ocean cells
    let is_ocean: Vec<bool> = (0..w*h).map(|i| hmap.data[i] <= sea_level).collect();

    // Distance-to-ocean approximation using a fast spread (BFS would be ideal but we use noise)
    for y in 0..h {
        for x in 0..w {
            let nx = x as f32 / w as f32 + (seed as f32 * 0.0002 + 0.5);
            let ny = y as f32 / h as f32 + 0.33;
            let altitude = hmap.get(x, y);

            // Base humidity from proximity to sea (approximated by altitude inversion)
            let coast_humidity = if altitude <= sea_level + 0.05 {
                0.85 + fbm_2d(nx * 2.0, ny * 2.0, &fbm_params) * 0.15
            } else {
                let alt_factor = ((altitude - sea_level) / (1.0 - sea_level)).clamp(0.0, 1.0);
                (0.7 - alt_factor * 0.5 + fbm_2d(nx * 4.0, ny * 4.0, &fbm_params) * 0.3).clamp(0.0, 1.0)
            };

            hum_map[y * w + x] = coast_humidity;
        }
    }
    hum_map
}

// ============================================================
//  CLIP PLANES, FRUSTUM CULLING
// ============================================================

#[derive(Clone, Debug)]
pub struct Plane {
    pub normal: Vec3,
    pub d:      f32,
}

impl Plane {
    pub fn new(normal: Vec3, d: f32) -> Self { Plane { normal, d } }
    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Self {
        Plane { normal: normal.normalize(), d: -normal.normalize().dot(point) }
    }
    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.d
    }
    pub fn normalize(&self) -> Self {
        let len = self.normal.length();
        Plane { normal: self.normal / len, d: self.d / len }
    }
}

#[derive(Clone, Debug)]
pub struct Frustum {
    pub planes: [Plane; 6], // near, far, left, right, top, bottom
}

impl Frustum {
    pub fn from_view_proj(vp: Mat4) -> Self {
        let m = vp.to_cols_array();
        // Extract frustum planes from view-projection matrix (Gribb-Hartmann method)
        let planes = [
            Plane::new(Vec3::new(m[3]+m[2], m[7]+m[6], m[11]+m[10]), m[15]+m[14]).normalize(), // near
            Plane::new(Vec3::new(m[3]-m[2], m[7]-m[6], m[11]-m[10]), m[15]-m[14]).normalize(), // far
            Plane::new(Vec3::new(m[3]+m[0], m[7]+m[4], m[11]+m[8]),  m[15]+m[12]).normalize(), // left
            Plane::new(Vec3::new(m[3]-m[0], m[7]-m[4], m[11]-m[8]),  m[15]-m[12]).normalize(), // right
            Plane::new(Vec3::new(m[3]+m[1], m[7]+m[5], m[11]+m[9]),  m[15]+m[13]).normalize(), // top
            Plane::new(Vec3::new(m[3]-m[1], m[7]-m[5], m[11]-m[9]),  m[15]-m[13]).normalize(), // bottom
        ];
        Frustum { planes }
    }

    pub fn test_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Find positive vertex (most positive in plane normal direction)
            let px = if plane.normal.x >= 0.0 { max.x } else { min.x };
            let py = if plane.normal.y >= 0.0 { max.y } else { min.y };
            let pz = if plane.normal.z >= 0.0 { max.z } else { min.z };
            let pv = Vec3::new(px, py, pz);
            if plane.distance_to_point(pv) < 0.0 { return false; }
        }
        true
    }

    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.distance_to_point(center) < -radius { return false; }
        }
        true
    }
}

// ============================================================
//  RAY CASTING AGAINST HEIGHTMAP
// ============================================================

#[derive(Clone, Debug)]
pub struct RayHit {
    pub point:    Vec3,
    pub normal:   Vec3,
    pub t:        f32,
    pub cell_x:   usize,
    pub cell_y:   usize,
    pub altitude: f32,
}

/// Ray-heightmap intersection using adaptive stepping
pub fn ray_heightmap_intersect(
    hmap:      &Heightmap,
    cell_size: f32,
    height_scale: f32,
    ray_origin: Vec3,
    ray_dir:    Vec3,
) -> Option<RayHit> {
    let dir = ray_dir.normalize();
    if dir.y.abs() < 1e-6 { return None; }

    let w = hmap.width  as f32;
    let h = hmap.height as f32;

    let mut t   = 0.0f32;
    let step    = cell_size * 0.5;
    let max_t   = (w * w + h * h + height_scale * height_scale).sqrt() * 2.0;

    let mut prev_pos = ray_origin;
    let mut prev_above = true;

    loop {
        t += step;
        if t > max_t { return None; }

        let pos = ray_origin + dir * t;
        let gx  = pos.x / cell_size;
        let gz  = pos.z / cell_size;

        if gx < 0.0 || gz < 0.0 || gx >= w || gz >= h { continue; }

        let ux = gx / w;
        let uz = gz / h;
        let terrain_h = hmap.sample_bilinear(ux, uz) * height_scale;

        let above = pos.y >= terrain_h;
        if !above && prev_above {
            // Bisect for precise hit
            let mut lo = t - step;
            let mut hi = t;
            for _ in 0..8 {
                let mid = (lo + hi) * 0.5;
                let mpos = ray_origin + dir * mid;
                let mx = mpos.x / cell_size;
                let mz = mpos.z / cell_size;
                if mx < 0.0 || mz < 0.0 || mx >= w || mz >= h { hi = mid; continue; }
                let mu = mx / w;
                let mv = mz / h;
                let mh = hmap.sample_bilinear(mu, mv) * height_scale;
                if mpos.y >= mh { lo = mid; } else { hi = mid; }
            }
            let hit_t   = (lo + hi) * 0.5;
            let hit_pos = ray_origin + dir * hit_t;
            let hx = (hit_pos.x / cell_size) as usize;
            let hz = (hit_pos.z / cell_size) as usize;
            let hx = hx.min(hmap.width  - 1);
            let hz = hz.min(hmap.height - 1);
            let normal = hmap.normal_at(hx, hz, cell_size);
            let altitude = hmap.get(hx, hz);
            return Some(RayHit { point: hit_pos, normal, t: hit_t, cell_x: hx, cell_y: hz, altitude });
        }

        prev_above = above;
        prev_pos   = pos;
    }
}

// ============================================================
//  MAIN WorldEditor STRUCT
// ============================================================

pub struct WorldEditor {
    // Core terrain
    pub heightmap:       Heightmap,
    pub cell_size:       f32,
    pub height_scale:    f32,
    pub sea_level:       f32,
    pub world_name:      String,
    pub metadata:        HashMap<String, String>,

    // Climate maps
    pub temperature_map: Vec<f32>,
    pub humidity_map:    Vec<f32>,

    // Systems
    pub biome_system:    BiomeSystem,
    pub weather:         WeatherSystem,
    pub road_network:    RoadNetwork,
    pub atmosphere:      AtmosphereParams,

    // Water
    pub rivers:     Vec<RiverPath>,
    pub lakes:      Vec<LakeBody>,
    pub shore:      Option<OceanShore>,

    // Foliage
    pub foliage:    Vec<FoliageInstance>,
    pub foliage_params: Vec<FoliagePlacementParams>,

    // Editor state
    pub selection:   SelectionState,
    pub undo_redo:   UndoRedoStack,
    pub active_tool: EditorTool,
    pub brush:       BrushSettings,

    // Time / solar
    pub utc_hour:    f64,
    pub day_of_year: f64,
    pub latitude:    f64,
    pub longitude:   f64,
    pub solar:       SolarState,

    // Statistics cache
    pub stats:       WorldStats,

    // Noise configuration
    pub terrain_fbm_params: FbmParams,
    pub warp_strength:       f32,
    pub erosion_params:      ErosionParams,

    // Seed for procedural generation
    pub master_seed: u64,

    // Dirty flags
    pub heightmap_dirty:     bool,
    pub climate_dirty:       bool,
    pub foliage_dirty:       bool,
    pub water_dirty:         bool,
}

#[derive(Clone, Debug, Default)]
pub struct WorldStats {
    pub total_cells:       usize,
    pub ocean_cells:       usize,
    pub land_cells:        usize,
    pub mountain_cells:    usize,
    pub river_count:       usize,
    pub lake_count:        usize,
    pub road_segments:     usize,
    pub road_total_length: f32,
    pub foliage_count:     usize,
    pub min_height:        f32,
    pub max_height:        f32,
    pub mean_height:       f32,
    pub dominant_biome:    Option<BiomeId>,
}

impl WorldEditor {
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        let heightmap = Heightmap::new(width, height);
        let temp_map  = vec![15.0f32; width * height];
        let hum_map   = vec![0.5f32;  width * height];

        let weather = WeatherSystem::new(12345, 15.0, 45.0);
        let atmo    = AtmosphereParams::default();

        let solar = SolarState::compute(45.0, 0.0, 180.0, 12.0, &atmo);

        WorldEditor {
            heightmap,
            cell_size,
            height_scale: 500.0,
            sea_level: 0.2,
            world_name: String::from("Untitled World"),
            metadata: HashMap::new(),

            temperature_map: temp_map,
            humidity_map:    hum_map,

            biome_system: BiomeSystem::new(),
            weather,
            road_network: RoadNetwork::new(),
            atmosphere: atmo,

            rivers: Vec::new(),
            lakes:  Vec::new(),
            shore:  None,

            foliage: Vec::new(),
            foliage_params: Vec::new(),

            selection:   SelectionState::new(),
            undo_redo:   UndoRedoStack::new(256),
            active_tool: EditorTool::Select,
            brush: BrushSettings {
                radius:   20.0,
                strength: 0.01,
                falloff:  BrushFalloff::Smooth,
                scatter:  0.0,
            },

            utc_hour:    12.0,
            day_of_year: 180.0,
            latitude:    45.0,
            longitude:   0.0,
            solar,

            stats: WorldStats::default(),
            terrain_fbm_params: FbmParams::default_terrain(),
            warp_strength: 0.3,
            erosion_params: ErosionParams::default(),

            master_seed: 0xCAFEBABE,
            heightmap_dirty:  true,
            climate_dirty:    true,
            foliage_dirty:    true,
            water_dirty:      true,
        }
    }

    // ---- Terrain Generation ----

    pub fn generate_terrain(&mut self) {
        let seed_offset = Vec2::new(
            (self.master_seed & 0xFFFF) as f32 / 65536.0,
            ((self.master_seed >> 16) & 0xFFFF) as f32 / 65536.0,
        );

        if self.warp_strength > 0.0 {
            self.heightmap.generate_domain_warp(&self.terrain_fbm_params, self.warp_strength, seed_offset);
        } else {
            self.heightmap.generate_fbm(&self.terrain_fbm_params, seed_offset);
        }

        self.heightmap_dirty = true;
        self.climate_dirty   = true;
        self.water_dirty     = true;
        self.foliage_dirty   = true;
    }

    pub fn apply_erosion(&mut self) {
        hydraulic_erosion(&mut self.heightmap, &self.erosion_params);
        self.heightmap_dirty = true;
        self.water_dirty     = true;
        self.foliage_dirty   = true;
    }

    pub fn apply_thermal_erosion(&mut self, iterations: usize, talus_deg: f32) {
        let talus_rad = talus_deg * DEG2RAD;
        thermal_erosion(&mut self.heightmap, iterations, talus_rad);
        self.heightmap_dirty = true;
    }

    // ---- Climate ----

    pub fn generate_climate(&mut self) {
        self.temperature_map = generate_temperature_map(
            &self.heightmap,
            15.0,
            self.latitude as f32,
            2.0,
            self.master_seed,
        );
        self.humidity_map = generate_humidity_map(
            &self.heightmap,
            self.sea_level,
            2.0,
            self.master_seed ^ 0x55AA,
        );
        self.climate_dirty = false;
    }

    pub fn get_biome_at(&self, x: usize, y: usize) -> BiomeBlendSample {
        let w = self.heightmap.width;
        let idx = y * w + x;
        let temp     = if idx < self.temperature_map.len() { self.temperature_map[idx] } else { 15.0 };
        let humidity = if idx < self.humidity_map.len()    { self.humidity_map[idx]    } else { 0.5  };
        let altitude = self.heightmap.get(x, y);
        self.biome_system.sample(temp, humidity, altitude)
    }

    // ---- Water ----

    pub fn generate_rivers(&mut self, num_rivers: usize) {
        self.rivers.clear();
        let mut rng = LcgRng::new(self.master_seed ^ 0xABCDEF);

        let w = self.heightmap.width  as f32;
        let h = self.heightmap.height as f32;

        for _ in 0..num_rivers {
            // Start from high-altitude random point
            let attempts = 20;
            let mut start = Vec2::ZERO;
            let mut found_start = false;
            for _ in 0..attempts {
                let sx = rng.next_f32() * w;
                let sy = rng.next_f32() * h;
                let ux = sx / w;
                let uy = sy / h;
                let alt = self.heightmap.sample_bilinear(ux, uy);
                if alt > 0.55 {
                    start = Vec2::new(sx, sy);
                    found_start = true;
                    break;
                }
            }
            if !found_start { continue; }

            let river = simulate_river(&self.heightmap, start, self.sea_level);
            if river.points.len() >= 10 {
                self.rivers.push(river);
            }
        }

        self.water_dirty = false;
    }

    pub fn generate_lakes(&mut self, num_lakes: usize, max_water_level: f32) {
        self.lakes.clear();
        let mut rng = LcgRng::new(self.master_seed ^ 0x123123);
        let w = self.heightmap.width;
        let h = self.heightmap.height;

        for _ in 0..num_lakes {
            let sx = (rng.next_f32() * (w - 2) as f32) as usize + 1;
            let sy = (rng.next_f32() * (h - 2) as f32) as usize + 1;
            let base_h = self.heightmap.get(sx, sy);
            if base_h <= self.sea_level || base_h > 0.6 { continue; }
            let water_level = base_h + rng.next_f32() * max_water_level;
            let lake = fill_lake(&self.heightmap, sx, sy, water_level);
            if lake.cells.len() > 4 {
                self.lakes.push(lake);
            }
        }
    }

    pub fn generate_shore(&mut self) {
        self.shore = Some(generate_ocean_shore(&self.heightmap, self.sea_level, 0.02));
    }

    // ---- Foliage ----

    pub fn place_foliage_layer(&mut self, params: FoliagePlacementParams, seed: u64) {
        let new_instances = place_foliage(
            &self.heightmap,
            None,
            &params,
            seed,
            self.cell_size,
        );
        self.foliage.extend(new_instances);
    }

    pub fn clear_foliage(&mut self) {
        self.foliage.clear();
    }

    pub fn cull_foliage(&mut self, frustum: &Frustum) -> Vec<usize> {
        let mut visible = Vec::new();
        for (i, fi) in self.foliage.iter().enumerate() {
            let r = fi.scale.length();
            if frustum.test_sphere(fi.position, r) {
                visible.push(i);
            }
        }
        visible
    }

    // ---- Roads ----

    pub fn build_road(
        &mut self,
        start: Vec2,
        end:   Vec2,
        road_type: RoadType,
    ) -> Option<u32> {
        let cost_params = RoadCostParams::default();
        let seg_id = self.road_network.build_road(
            &self.heightmap,
            start,
            end,
            self.cell_size,
            road_type,
            &cost_params,
        )?;
        Some(seg_id)
    }

    // ---- Solar / Sky ----

    pub fn update_solar(&mut self) {
        self.solar = SolarState::compute(
            self.latitude,
            self.longitude,
            self.day_of_year,
            self.utc_hour,
            &self.atmosphere,
        );
    }

    pub fn advance_time(&mut self, delta_hours: f64) {
        self.utc_hour += delta_hours;
        if self.utc_hour >= 24.0 {
            self.utc_hour -= 24.0;
            self.day_of_year += 1.0;
            if self.day_of_year > 365.0 {
                self.day_of_year = 1.0;
            }
        }
        self.update_solar();
        self.weather.advance_season((delta_hours / 8760.0) as f32);
        self.weather.step(delta_hours as f32);
    }

    // ---- Editing ----

    pub fn raise_terrain(&mut self, cx: f32, cy: f32, delta: f32) {
        let action = terrain_brush_raise(&mut self.heightmap, cx, cy, &self.brush, delta);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    pub fn lower_terrain(&mut self, cx: f32, cy: f32, delta: f32) {
        let action = terrain_brush_raise(&mut self.heightmap, cx, cy, &self.brush, -delta);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    pub fn smooth_terrain(&mut self, cx: f32, cy: f32, iters: usize) {
        let action = terrain_brush_smooth(&mut self.heightmap, cx, cy, &self.brush, iters);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    pub fn flatten_terrain(&mut self, cx: f32, cy: f32, target: f32) {
        let action = terrain_brush_flatten(&mut self.heightmap, cx, cy, &self.brush, target);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    // ---- Undo / Redo ----

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_redo.pop_undo() {
            let redo_action = self.apply_action_inverse(&action);
            self.undo_redo.push_redo(redo_action);
            self.heightmap_dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.undo_redo.pop_redo() {
            let undo_action = self.apply_action_inverse(&action);
            self.undo_redo.push(undo_action);
            self.heightmap_dirty = true;
        }
    }

    fn apply_action_inverse(&mut self, action: &EditAction) -> EditAction {
        match action {
            EditAction::SetHeightRegion { x, y, width, height, old_data, new_data } => {
                for row in 0..*height {
                    for col in 0..*width {
                        let hx = x + col;
                        let hy = y + row;
                        if hx < self.heightmap.width && hy < self.heightmap.height {
                            let i = row * width + col;
                            if i < old_data.len() {
                                self.heightmap.set(hx, hy, old_data[i]);
                            }
                        }
                    }
                }
                EditAction::SetHeightRegion {
                    x: *x, y: *y, width: *width, height: *height,
                    old_data: new_data.clone(),
                    new_data: old_data.clone(),
                }
            }
            EditAction::AddRoadSegment { segment_id, segment } => {
                self.road_network.segments.retain(|s| s.id != *segment_id);
                EditAction::RemoveRoadSegment { segment_id: *segment_id, segment: segment.clone() }
            }
            EditAction::RemoveRoadSegment { segment_id, segment } => {
                self.road_network.segments.push(segment.clone());
                EditAction::AddRoadSegment { segment_id: *segment_id, segment: segment.clone() }
            }
            EditAction::AddWaterBody { lake, index } => {
                if *index < self.lakes.len() { self.lakes.remove(*index); }
                EditAction::RemoveWaterBody { lake: lake.clone(), index: *index }
            }
            EditAction::RemoveWaterBody { lake, index } => {
                let i = (*index).min(self.lakes.len());
                self.lakes.insert(i, lake.clone());
                EditAction::AddWaterBody { lake: lake.clone(), index: *index }
            }
            EditAction::PlaceFoliageInstances { instances, indices } => {
                for &idx in indices.iter().rev() {
                    if idx < self.foliage.len() { self.foliage.remove(idx); }
                }
                EditAction::RemoveFoliageInstances {
                    indices: indices.clone(),
                    instances: instances.clone(),
                }
            }
            EditAction::RemoveFoliageInstances { instances, indices } => {
                for (i, inst) in indices.iter().zip(instances.iter()) {
                    let insert_at = (*i).min(self.foliage.len());
                    self.foliage.insert(insert_at, inst.clone());
                }
                EditAction::PlaceFoliageInstances {
                    instances: instances.clone(),
                    indices: indices.clone(),
                }
            }
            EditAction::SetBiomeOverride { .. } => {
                // Biome overrides not yet stored on editor — return no-op
                action.clone()
            }
            EditAction::CompoundAction { actions, description } => {
                let mut reverse_actions = Vec::with_capacity(actions.len());
                for a in actions.iter().rev() {
                    reverse_actions.push(self.apply_action_inverse(a));
                }
                EditAction::CompoundAction {
                    actions: reverse_actions,
                    description: format!("Undo: {}", description),
                }
            }
        }
    }

    // ---- Ray Casting ----

    pub fn ray_cast(&self, ray_origin: Vec3, ray_dir: Vec3) -> Option<RayHit> {
        ray_heightmap_intersect(
            &self.heightmap,
            self.cell_size,
            self.height_scale,
            ray_origin,
            ray_dir,
        )
    }

    // ---- Statistics ----

    pub fn compute_stats(&mut self) {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        let total = w * h;

        let mut ocean    = 0usize;
        let mut mountain = 0usize;
        let mut sum      = 0.0f64;
        let mut biome_counts = [0usize; 25];

        for y in 0..h {
            for x in 0..w {
                let alt = self.heightmap.get(x, y);
                sum += alt as f64;
                if alt <= self.sea_level { ocean += 1; }
                if alt > 0.7 { mountain += 1; }
                let idx = y * w + x;
                let temp = if idx < self.temperature_map.len() { self.temperature_map[idx] } else { 15.0 };
                let hum  = if idx < self.humidity_map.len()    { self.humidity_map[idx]    } else { 0.5  };
                let biome = BiomeDescriptor::classify_point(temp, hum, alt);
                biome_counts[biome as usize] += 1;
            }
        }

        let dominant_idx = biome_counts.iter().enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| i)
            .unwrap_or(0);

        self.heightmap.recompute_minmax();

        let road_len: f32 = self.road_network.segments.iter().map(|s| s.length).sum();

        self.stats = WorldStats {
            total_cells:       total,
            ocean_cells:       ocean,
            land_cells:        total - ocean,
            mountain_cells:    mountain,
            river_count:       self.rivers.len(),
            lake_count:        self.lakes.len(),
            road_segments:     self.road_network.segments.len(),
            road_total_length: road_len,
            foliage_count:     self.foliage.len(),
            min_height:        self.heightmap.min_h,
            max_height:        self.heightmap.max_h,
            mean_height:       (sum / total as f64) as f32,
            dominant_biome:    Some(BiomeId::TropicalRainforest), // simplified
        };
    }

    // ---- Serialization ----

    pub fn serialize(&self) -> Vec<u8> {
        let sw = SerializedWorld::from_editor(self);
        sw.to_bytes()
    }

    // ---- Full procedural generation pipeline ----

    pub fn generate_full_world(
        &mut self,
        num_rivers: usize,
        num_lakes:  usize,
        foliage_density: f32,
    ) {
        // 1. Generate terrain
        self.generate_terrain();

        // 2. Apply erosion
        self.apply_erosion();
        self.apply_thermal_erosion(5, 35.0);

        // 3. Generate climate maps
        self.generate_climate();

        // 4. Generate water bodies
        self.generate_rivers(num_rivers);
        self.generate_lakes(num_lakes, 0.03);
        self.generate_shore();

        // 5. Place foliage
        let biome_table = build_biome_table();
        for (biome_idx, desc) in biome_table.iter().enumerate() {
            let fp = FoliagePlacementParams {
                min_radius:      2.0 + (1.0 - desc.tree_density) * 8.0,
                max_instances:   (desc.tree_density * foliage_density * 50000.0) as usize,
                max_slope_rad:   0.6,
                min_altitude:    desc.alt_min,
                max_altitude:    desc.alt_max,
                density_scale:   desc.tree_density * foliage_density,
                use_density_map: false,
                random_rotation: true,
                scale_variance:  0.3,
                base_scale:      Vec3::new(1.0, 1.0 + desc.tree_density, 1.0),
                asset_id:        biome_idx as u32,
                biome_id:        biome_idx as u8,
                align_to_normal: false,
            };
            self.place_foliage_layer(fp, self.master_seed ^ (biome_idx as u64 * 997));
        }

        // 6. Update solar state
        self.update_solar();

        // 7. Compute statistics
        self.compute_stats();
    }

    // ---- Camera helpers ----

    pub fn world_to_heightmap(&self, world: Vec3) -> (usize, usize) {
        let x = (world.x / self.cell_size) as usize;
        let z = (world.z / self.cell_size) as usize;
        (x.min(self.heightmap.width - 1), z.min(self.heightmap.height - 1))
    }

    pub fn heightmap_to_world(&self, x: usize, z: usize) -> Vec3 {
        let height = self.heightmap.get(x, z) * self.height_scale;
        Vec3::new(x as f32 * self.cell_size, height, z as f32 * self.cell_size)
    }

    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        let min = Vec3::ZERO;
        let max = Vec3::new(
            self.heightmap.width  as f32 * self.cell_size,
            self.height_scale,
            self.heightmap.height as f32 * self.cell_size,
        );
        (min, max)
    }

    // ---- Gizmo rendering helpers ----

    pub fn get_selection_pivot(&self) -> Vec3 {
        if let Some(p) = self.selection.pivot { return p; }
        // Compute from selected cells
        let mut sum = Vec3::ZERO;
        let mut count = 0;
        for item in &self.selection.items {
            if let SelectionItem::TerrainCell(x, z) = item {
                sum += self.heightmap_to_world(*x, *z);
                count += 1;
            }
        }
        if count > 0 { sum / count as f32 } else { Vec3::ZERO }
    }

    // ---- Measure tool ----

    pub fn measure_distance(&self, a_world: Vec3, b_world: Vec3) -> f32 {
        (b_world - a_world).length()
    }

    pub fn measure_area_of_selection(&self) -> f32 {
        let count = self.selection.count_terrain_cells();
        count as f32 * self.cell_size * self.cell_size
    }

    // ---- LOD helpers ----

    pub fn compute_lod_factor(&self, pos: Vec3, camera_pos: Vec3, lod_distances: &[f32]) -> u8 {
        let dist = (pos - camera_pos).length();
        for (i, &d) in lod_distances.iter().enumerate() {
            if dist < d { return i as u8; }
        }
        lod_distances.len() as u8
    }

    pub fn update_foliage_lod(&mut self, camera_pos: Vec3) {
        let lod_distances = [50.0f32, 150.0, 400.0, 1000.0];
        for fi in self.foliage.iter_mut() {
            let dist = (fi.position - camera_pos).length();
            fi.lod_factor = (dist / lod_distances[lod_distances.len() - 1]).clamp(0.0, 1.0);
        }
    }

    // ---- Debug helpers ----

    pub fn sample_sky_at_direction(&self, dir: Vec3) -> Vec3 {
        let raw = compute_sky_color(dir, self.solar.direction, &self.atmosphere);
        let with_sun = raw + sun_disk_color(dir, self.solar.direction, 0.009, self.solar.sun_color * self.solar.sun_intensity);
        aces_tonemap(with_sun)
    }
}

fn _0_009_rad_equiv_inner() -> f32 { 0.009 }
trait RadEquiv { fn _0_009_rad_equiv(&self) -> f32; }
// Hack to make the compiler happy — use a free fn:
fn zero_point_zero_zero_nine() -> f32 { 0.009 }

// Direct free function reference to avoid method call on literal:
impl WorldEditor {
    pub fn sky_at(&self, view_dir: Vec3) -> Vec3 {
        let raw = compute_sky_color(view_dir, self.solar.direction, &self.atmosphere);
        let sun = sun_disk_color(view_dir, self.solar.direction, 0.009, self.solar.sun_color * self.solar.sun_intensity);
        aces_tonemap(raw + sun)
    }
}

// ============================================================
//  ADDITIONAL MATHEMATICAL UTILITIES
// ============================================================

/// Smooth-step: 3x² - 2x³
#[inline] pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smoother-step: 6x⁵ - 15x⁴ + 10x³
#[inline] pub fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Remap value from [in_min, in_max] to [out_min, out_max]
#[inline] pub fn remap(v: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    out_min + (out_max - out_min) * ((v - in_min) / (in_max - in_min)).clamp(0.0, 1.0)
}

/// Bilinear interpolation of a 2D value grid
pub fn bilinear_sample(data: &[f32], width: usize, height: usize, u: f32, v: f32) -> f32 {
    let px = u * (width  - 1) as f32;
    let py = v * (height - 1) as f32;
    let x0 = px.floor() as usize;
    let y0 = py.floor() as usize;
    let x1 = (x0 + 1).min(width  - 1);
    let y1 = (y0 + 1).min(height - 1);
    let tx = px - x0 as f32;
    let ty = py - y0 as f32;
    let a = data[y0 * width + x0];
    let b = data[y0 * width + x1];
    let c = data[y1 * width + x0];
    let d = data[y1 * width + x1];
    lerp_f(lerp_f(a, b, tx), lerp_f(c, d, tx), ty)
}

/// Build a 2D Gaussian kernel (sigma, kernel_size must be odd)
pub fn gaussian_kernel_2d(sigma: f32, size: usize) -> Vec<f32> {
    let half   = (size / 2) as i32;
    let sigma2 = sigma * sigma;
    let mut k  = vec![0.0f32; size * size];
    let mut sum = 0.0f32;
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let dx = (x - half) as f32;
            let dy = (y - half) as f32;
            let v  = (-(dx*dx + dy*dy) / (2.0 * sigma2)).exp();
            k[(y as usize) * size + (x as usize)] = v;
            sum += v;
        }
    }
    for v in k.iter_mut() { *v /= sum; }
    k
}

/// Apply a separable Gaussian blur to a 2D float map
pub fn gaussian_blur_2d(data: &[f32], width: usize, height: usize, sigma: f32) -> Vec<f32> {
    let radius = (sigma * 3.0).ceil() as i32;
    let size   = (radius * 2 + 1) as usize;
    // 1D kernel
    let mut kernel = vec![0.0f32; size];
    let mut ksum   = 0.0f32;
    for i in 0..size as i32 {
        let d  = (i - radius) as f32;
        let v  = (-(d*d) / (2.0 * sigma * sigma)).exp();
        kernel[i as usize] = v;
        ksum += v;
    }
    for v in kernel.iter_mut() { *v /= ksum; }

    // Horizontal pass
    let mut temp = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let mut acc = 0.0f32;
            for (ki, &kv) in kernel.iter().enumerate() {
                let nx = (x as i32 + ki as i32 - radius).clamp(0, width as i32 - 1) as usize;
                acc += data[y * width + nx] * kv;
            }
            temp[y * width + x] = acc;
        }
    }

    // Vertical pass
    let mut out = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let mut acc = 0.0f32;
            for (ki, &kv) in kernel.iter().enumerate() {
                let ny = (y as i32 + ki as i32 - radius).clamp(0, height as i32 - 1) as usize;
                acc += temp[ny * width + x] * kv;
            }
            out[y * width + x] = acc;
        }
    }
    out
}

/// Diamond-square fractal terrain generation
pub fn diamond_square(size: usize, roughness: f32, seed: u64) -> Vec<f32> {
    // size must be 2^n + 1
    let mut grid = vec![0.0f32; size * size];
    let mut rng  = LcgRng::new(seed);

    // Corner seeds
    grid[0]                   = rng.next_f32();
    grid[size - 1]            = rng.next_f32();
    grid[(size-1)*size]       = rng.next_f32();
    grid[(size-1)*size+size-1]= rng.next_f32();

    let mut step    = size - 1;
    let mut scale   = roughness;
    let half_size   = size as i32;

    while step > 1 {
        let half = step / 2;

        // Diamond step
        let mut y = 0;
        while y < size - 1 {
            let mut x = 0;
            while x < size - 1 {
                let avg = (
                    grid[y        * size + x       ]
                    + grid[y        * size + x + step]
                    + grid[(y+step) * size + x       ]
                    + grid[(y+step) * size + x + step]
                ) / 4.0;
                grid[(y+half) * size + (x+half)] = avg + (rng.next_f32() * 2.0 - 1.0) * scale;
                x += step;
            }
            y += step;
        }

        // Square step
        let mut y = 0i32;
        while y < size as i32 {
            let mut x = (if (y as usize / half) % 2 == 0 { half as i32 } else { 0 });
            while x < size as i32 {
                let mut sum   = 0.0f32;
                let mut count = 0;
                let offsets: [(i32,i32); 4] = [(-(half as i32), 0), (half as i32, 0), (0, -(half as i32)), (0, half as i32)];
                for &(dx, dy) in &offsets {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < size as i32 && ny >= 0 && ny < size as i32 {
                        sum += grid[ny as usize * size + nx as usize];
                        count += 1;
                    }
                }
                grid[y as usize * size + x as usize] = sum / count as f32 + (rng.next_f32() * 2.0 - 1.0) * scale;
                x += step as i32;
            }
            y += half as i32;
        }

        step  /= 2;
        scale *= roughness.powf(1.0);
    }

    // Normalize
    let min_v = grid.iter().cloned().fold(f32::MAX, f32::min);
    let max_v = grid.iter().cloned().fold(f32::MIN, f32::max);
    let range = max_v - min_v;
    if range > 1e-10 {
        for v in grid.iter_mut() { *v = (*v - min_v) / range; }
    }

    grid
}

// ============================================================
//  SLOPE MAP, CURVATURE, FLOW DIRECTION
// ============================================================

/// Compute a slope map (value in radians) for the entire heightmap
pub fn compute_slope_map(hmap: &Heightmap, cell_size: f32) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut slope_map = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            slope_map[y * w + x] = hmap.slope_at(x, y, cell_size);
        }
    }
    slope_map
}

/// Curvature — second derivative of height (Laplacian, approximated)
pub fn compute_curvature_map(hmap: &Heightmap) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut curv_map = vec![0.0f32; w * h];
    for y in 1..h-1 {
        for x in 1..w-1 {
            let center = hmap.get(x, y);
            let d2hdx2 = hmap.get(x+1, y) - 2.0 * center + hmap.get(x-1, y);
            let d2hdy2 = hmap.get(x, y+1) - 2.0 * center + hmap.get(x, y-1);
            curv_map[y * w + x] = d2hdx2 + d2hdy2;
        }
    }
    curv_map
}

/// D8 flow direction (8 neighbors) — returns index 0-7 of steepest descent
pub fn compute_flow_direction(hmap: &Heightmap) -> Vec<u8> {
    let w = hmap.width;
    let h = hmap.height;
    let mut flow = vec![0u8; w * h];
    let dirs: [(i32,i32); 8] = [(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1),(0,-1),(1,-1)];
    for y in 1..h-1 {
        for x in 1..w-1 {
            let center = hmap.get(x, y);
            let mut best_drop = 0.0f32;
            let mut best_dir  = 0u8;
            for (i, &(dx, dy)) in dirs.iter().enumerate() {
                let nh = hmap.get_clamped(x as i32 + dx, y as i32 + dy);
                let drop = center - nh;
                let dist = if dx != 0 && dy != 0 { (2.0f32).sqrt() } else { 1.0 };
                let slope = drop / dist;
                if slope > best_drop { best_drop = slope; best_dir = i as u8; }
            }
            flow[y * w + x] = best_dir;
        }
    }
    flow
}

/// Compute flow accumulation from flow direction map
pub fn compute_flow_accumulation(flow_dir: &[u8], width: usize, height: usize) -> Vec<u32> {
    let mut acc = vec![1u32; width * height]; // each cell starts with 1
    let dirs: [(i32,i32); 8] = [(1,0),(1,1),(0,1),(-1,1),(-1,0),(-1,-1),(0,-1),(1,-1)];

    // Topological sort (simplified): iterate multiple passes
    for _pass in 0..height {
        for y in 1..height-1 {
            for x in 1..width-1 {
                let dir = flow_dir[y * width + x] as usize;
                let (dx, dy) = dirs[dir];
                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;
                if nx < width && ny < height {
                    acc[ny * width + nx] += acc[y * width + x];
                }
            }
        }
    }
    acc
}

// ============================================================
//  AMBIENT OCCLUSION (SSAO-style precompute for terrain)
// ============================================================

/// Compute horizon-based ambient occlusion for each heightmap cell
/// Casts rays in multiple horizontal directions and measures occlusion
pub fn compute_terrain_ao(hmap: &Heightmap, num_rays: usize, max_dist: f32, cell_size: f32) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut ao = vec![1.0f32; w * h];

    let angle_step = TWO_PI / num_rays as f32;

    for y in 0..h {
        for x in 0..w {
            let base_h  = hmap.get(x, y);
            let mut occ = 0.0f32;

            for ray in 0..num_rays {
                let angle   = ray as f32 * angle_step;
                let ray_dx  = angle.cos();
                let ray_dz  = angle.sin();
                let mut max_horizon = 0.0f32; // max elevation angle seen

                let steps = (max_dist / cell_size).ceil() as usize;
                for step in 1..=steps {
                    let t  = step as f32 * cell_size;
                    let nx = x as f32 + ray_dx * t / cell_size;
                    let nz = y as f32 + ray_dz * t / cell_size;
                    if nx < 0.0 || nz < 0.0 || nx >= w as f32 || nz >= h as f32 { break; }

                    let ux = (nx / w as f32).clamp(0.0, 1.0);
                    let uz = (nz / h as f32).clamp(0.0, 1.0);
                    let nh = hmap.sample_bilinear(ux, uz);

                    let elevation_angle = (nh - base_h) / t * cell_size; // approximate
                    if elevation_angle > max_horizon {
                        max_horizon = elevation_angle;
                    }
                }

                // Convert max horizon angle to occlusion
                let horizon_angle = max_horizon.atan();
                occ += (horizon_angle / HALF_PI).clamp(0.0, 1.0);
            }

            ao[y * w + x] = 1.0 - (occ / num_rays as f32).clamp(0.0, 1.0);
        }
    }

    ao
}

// ============================================================
//  CLOUD LAYER SIMULATION
// ============================================================

#[derive(Clone, Debug)]
pub struct CloudLayer {
    pub altitude_km:  f32,
    pub thickness_km: f32,
    pub coverage:     f32,   // 0..1
    pub density:      f32,
    pub wind_vel:     Vec2,
    pub noise_offset: Vec2,
}

impl CloudLayer {
    pub fn new(altitude_km: f32, thickness_km: f32, coverage: f32) -> Self {
        CloudLayer {
            altitude_km,
            thickness_km,
            coverage,
            density: coverage * 0.5,
            wind_vel: Vec2::new(5.0, 2.0),
            noise_offset: Vec2::ZERO,
        }
    }

    /// Compute cloud opacity at a given UV position
    pub fn opacity_at(&self, u: f32, v: f32, time: f32) -> f32 {
        let offset = self.wind_vel * time * 0.0001;
        let su = u + offset.x + self.noise_offset.x;
        let sv = v + offset.y + self.noise_offset.y;

        let cloud_params = FbmParams { octaves: 5, frequency: 2.0, lacunarity: 2.0, gain: 0.5, amplitude: 1.0, offset: 0.0, ridge: false };
        let n  = fbm_2d(su * 3.0, sv * 3.0, &cloud_params) * 0.5 + 0.5;
        let cloud_val = smoothstep(1.0 - self.coverage, 1.0, n);
        cloud_val * self.density
    }

    /// Update cloud layer position by wind
    pub fn update(&mut self, delta_time: f32) {
        self.noise_offset += self.wind_vel * delta_time * 0.00001;
    }
}

pub struct SkySystem {
    pub cloud_layers: Vec<CloudLayer>,
    pub params:       AtmosphereParams,
    pub time:         f32,
}

impl SkySystem {
    pub fn new() -> Self {
        SkySystem {
            cloud_layers: vec![
                CloudLayer::new(2.0, 0.5, 0.4),
                CloudLayer::new(5.0, 1.0, 0.3),
                CloudLayer::new(8.0, 2.0, 0.2),
            ],
            params: AtmosphereParams::default(),
            time: 0.0,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.time += delta_time;
        for layer in self.cloud_layers.iter_mut() {
            layer.update(delta_time);
        }
    }

    /// Sample total cloud coverage (max of all layers) at a UV
    pub fn cloud_coverage_at(&self, u: f32, v: f32) -> f32 {
        self.cloud_layers.iter()
            .map(|l| l.opacity_at(u, v, self.time))
            .fold(0.0f32, f32::max)
    }

    pub fn render_sky(&self, view_dir: Vec3, sun_dir: Vec3) -> Vec3 {
        let sky  = compute_sky_color(view_dir, sun_dir, &self.params);
        let sun  = sun_disk_color(view_dir, sun_dir, 0.009, Vec3::new(10.0, 9.0, 8.0));
        aces_tonemap(sky + sun)
    }
}

// ============================================================
//  TERRAIN LOD QUADTREE
// ============================================================

#[derive(Clone, Debug)]
pub struct QuadtreeNode {
    pub x:        usize,
    pub y:        usize,
    pub size:     usize,
    pub lod:      u8,
    pub children: Option<[Box<QuadtreeNode>; 4]>,
    pub min_h:    f32,
    pub max_h:    f32,
    pub center:   Vec3,
    pub is_leaf:  bool,
}

impl QuadtreeNode {
    pub fn new(x: usize, y: usize, size: usize, cell_size: f32) -> Self {
        let half = size as f32 * 0.5;
        let center = Vec3::new(
            (x as f32 + half) * cell_size,
            0.0,
            (y as f32 + half) * cell_size,
        );
        QuadtreeNode { x, y, size, lod: 0, children: None, min_h: 0.0, max_h: 1.0, center, is_leaf: true }
    }

    pub fn build(hmap: &Heightmap, x: usize, y: usize, size: usize, min_size: usize, cell_size: f32, depth: u8) -> Box<Self> {
        let mut node = QuadtreeNode::new(x, y, size, cell_size);
        node.lod = depth;

        // Compute height range
        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;
        let x1 = (x + size).min(hmap.width);
        let y1 = (y + size).min(hmap.height);
        for cy in y..y1 {
            for cx in x..x1 {
                let h = hmap.get(cx, cy);
                if h < min_h { min_h = h; }
                if h > max_h { max_h = h; }
            }
        }
        node.min_h = min_h;
        node.max_h = max_h;
        node.center.y = (min_h + max_h) * 0.5 * 500.0;

        if size <= min_size {
            node.is_leaf = true;
            return Box::new(node);
        }

        let half = size / 2;
        node.is_leaf = false;
        node.children = Some([
            QuadtreeNode::build(hmap, x,        y,        half, min_size, cell_size, depth + 1),
            QuadtreeNode::build(hmap, x + half, y,        half, min_size, cell_size, depth + 1),
            QuadtreeNode::build(hmap, x,        y + half, half, min_size, cell_size, depth + 1),
            QuadtreeNode::build(hmap, x + half, y + half, half, min_size, cell_size, depth + 1),
        ]);

        Box::new(node)
    }

    /// Collect visible leaf nodes given a frustum and camera position
    pub fn collect_visible<'a>(&'a self, frustum: &Frustum, cam_pos: Vec3, max_lod: u8, out: &mut Vec<&'a QuadtreeNode>, cell_size: f32, height_scale: f32) {
        let aabb_min = Vec3::new(
            self.x as f32 * cell_size,
            self.min_h * height_scale,
            self.y as f32 * cell_size,
        );
        let aabb_max = Vec3::new(
            (self.x + self.size) as f32 * cell_size,
            self.max_h * height_scale,
            (self.y + self.size) as f32 * cell_size,
        );

        if !frustum.test_aabb(aabb_min, aabb_max) { return; }

        if self.is_leaf || self.lod >= max_lod {
            out.push(self);
            return;
        }

        let dist = (self.center - cam_pos).length();
        let lod_size = self.size as f32 * cell_size;
        // Switch to leaf if this node's size is small enough relative to distance
        if lod_size / dist < 0.5 {
            out.push(self);
            return;
        }

        if let Some(ref ch) = self.children {
            for c in ch.iter() {
                c.collect_visible(frustum, cam_pos, max_lod, out, cell_size, height_scale);
            }
        } else {
            out.push(self);
        }
    }
}

// ============================================================
//  VEGETATION DENSITY MAP FROM BIOMES
// ============================================================

pub fn generate_vegetation_density_map(
    hmap:        &Heightmap,
    temp_map:    &[f32],
    hum_map:     &[f32],
    biome_sys:   &BiomeSystem,
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut density = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let idx      = y * w + x;
            let altitude = hmap.get(x, y);
            let temp     = if idx < temp_map.len() { temp_map[idx] } else { 15.0 };
            let humidity = if idx < hum_map.len()  { hum_map[idx]  } else { 0.5  };
            let sample   = biome_sys.sample(temp, humidity, altitude);
            density[idx] = sample.blended_tree_density * sample.blended_grass_density;
        }
    }
    density
}

// ============================================================
//  SUNLIGHT SHADOW MAP (DIRECTIONAL SHADOW APPROXIMATE)
// ============================================================

/// Compute shadow intensity for each heightmap cell from a directional light
/// Returns 0 = fully shadowed, 1 = fully lit
pub fn compute_terrain_shadow_map(
    hmap:       &Heightmap,
    sun_dir:    Vec3,
    cell_size:  f32,
    height_scale: f32,
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut shadow = vec![1.0f32; w * h];

    // Only compute shadows when sun is above horizon
    if sun_dir.y < 0.01 {
        return vec![0.0f32; w * h];
    }

    // For each cell, march toward sun and check if any terrain is in the way
    let sun_horiz = Vec2::new(sun_dir.x, sun_dir.z);
    if sun_horiz.length() < 1e-6 { return shadow; } // sun straight up — no shadows

    let sun_2d     = sun_horiz.normalize();
    let slope_inv  = sun_dir.y / sun_horiz.length();
    let step_dist  = cell_size;
    let max_steps  = ((w + h) / 2) as usize;

    for y in 0..h {
        for x in 0..w {
            let base_h = hmap.get(x, y) * height_scale;
            let mut cur_x = x as f32;
            let mut cur_z = y as f32;
            let mut shadowed = false;

            for step in 1..max_steps {
                cur_x += sun_2d.x * step_dist / cell_size;
                cur_z += sun_2d.y * step_dist / cell_size; // sun_2d.y maps to Z axis
                if cur_x < 0.0 || cur_z < 0.0 || cur_x >= w as f32 || cur_z >= h as f32 { break; }

                let ux = (cur_x / w as f32).clamp(0.0, 1.0);
                let uz = (cur_z / h as f32).clamp(0.0, 1.0);
                let terrain_h = hmap.sample_bilinear(ux, uz) * height_scale;
                let expected_h = base_h + step as f32 * step_dist * slope_inv;

                if terrain_h > expected_h {
                    shadowed = true;
                    break;
                }
            }

            shadow[y * w + x] = if shadowed { 0.0 } else { 1.0 };
        }
    }

    shadow
}

// ============================================================
//  COLOR RAMP FOR VISUALIZATION
// ============================================================

#[derive(Clone, Debug)]
pub struct ColorRamp {
    pub stops: Vec<(f32, Vec3)>, // (position 0..1, color)
}

impl ColorRamp {
    pub fn terrain_default() -> Self {
        ColorRamp {
            stops: vec![
                (0.00, Vec3::new(0.05, 0.15, 0.60)), // deep water
                (0.18, Vec3::new(0.10, 0.40, 0.80)), // shallow water
                (0.22, Vec3::new(0.90, 0.85, 0.65)), // sand/beach
                (0.30, Vec3::new(0.30, 0.55, 0.15)), // lowland grass
                (0.50, Vec3::new(0.20, 0.45, 0.10)), // forest
                (0.65, Vec3::new(0.45, 0.40, 0.30)), // highland
                (0.80, Vec3::new(0.55, 0.50, 0.45)), // rock
                (0.92, Vec3::new(0.80, 0.85, 0.90)), // snow line
                (1.00, Vec3::new(0.95, 0.97, 1.00)), // peak snow
            ],
        }
    }

    pub fn sample(&self, t: f32) -> Vec3 {
        let t = t.clamp(0.0, 1.0);
        if self.stops.is_empty() { return Vec3::ZERO; }
        if self.stops.len() == 1 { return self.stops[0].1; }

        for i in 0..self.stops.len() - 1 {
            let (ta, ca) = self.stops[i];
            let (tb, cb) = self.stops[i + 1];
            if t >= ta && t <= tb {
                let local_t = (t - ta) / (tb - ta);
                let st      = smoothstep(0.0, 1.0, local_t);
                return ca + (cb - ca) * st;
            }
        }

        self.stops.last().unwrap().1
    }
}

// ============================================================
//  EDITOR VIEWPORT CAMERA
// ============================================================

#[derive(Clone, Debug)]
pub struct EditorCamera {
    pub position:    Vec3,
    pub target:      Vec3,
    pub up:          Vec3,
    pub fov_deg:     f32,
    pub aspect:      f32,
    pub near:        f32,
    pub far:         f32,
    pub orbit_yaw:   f32,
    pub orbit_pitch: f32,
    pub orbit_dist:  f32,
}

impl EditorCamera {
    pub fn new(aspect: f32) -> Self {
        EditorCamera {
            position:    Vec3::new(512.0, 200.0, 512.0),
            target:      Vec3::new(512.0, 0.0, 512.0),
            up:          Vec3::Y,
            fov_deg:     60.0,
            aspect,
            near:        1.0,
            far:         50000.0,
            orbit_yaw:   -30.0,
            orbit_pitch: 45.0,
            orbit_dist:  600.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn proj_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_deg * DEG2RAD, self.aspect, self.near, self.far)
    }

    pub fn view_proj(&self) -> Mat4 {
        self.proj_matrix() * self.view_matrix()
    }

    pub fn frustum(&self) -> Frustum {
        Frustum::from_view_proj(self.view_proj())
    }

    /// Update camera position from orbit parameters
    pub fn update_orbit(&mut self) {
        let yaw_rad   = self.orbit_yaw   * DEG2RAD;
        let pitch_rad = self.orbit_pitch * DEG2RAD;

        let x = self.orbit_dist * pitch_rad.cos() * yaw_rad.sin();
        let y = self.orbit_dist * pitch_rad.sin();
        let z = self.orbit_dist * pitch_rad.cos() * yaw_rad.cos();

        self.position = self.target + Vec3::new(x, y, z);
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.orbit_yaw   += delta_yaw;
        self.orbit_pitch  = (self.orbit_pitch + delta_pitch).clamp(5.0, 85.0);
        self.update_orbit();
    }

    pub fn zoom(&mut self, delta: f32) {
        self.orbit_dist = (self.orbit_dist + delta).clamp(10.0, 10000.0);
        self.update_orbit();
    }

    pub fn pan(&mut self, delta: Vec3) {
        self.target   += delta;
        self.position += delta;
    }

    /// Compute a ray from the camera through a screen-space point (ndc -1..1)
    pub fn screen_to_ray(&self, ndc_x: f32, ndc_y: f32) -> (Vec3, Vec3) {
        let inv_vp = self.view_proj().inverse();
        let near_ndc = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let far_ndc  = Vec4::new(ndc_x, ndc_y,  1.0, 1.0);

        let near_world = inv_vp * near_ndc;
        let far_world  = inv_vp * far_ndc;

        let nw = Vec3::new(near_world.x / near_world.w, near_world.y / near_world.w, near_world.z / near_world.w);
        let fw = Vec3::new(far_world.x  / far_world.w,  far_world.y  / far_world.w,  far_world.z  / far_world.w);

        let dir = (fw - nw).normalize();
        (nw, dir)
    }
}

// ============================================================
//  TERRAIN PAINTER (multi-layer blending)
// ============================================================

#[derive(Clone, Debug)]
pub struct TerrainLayer {
    pub id:        usize,
    pub name:      String,
    pub weight_map: Vec<f32>,   // same dimensions as heightmap
    pub tiling:    f32,
    pub normal_strength: f32,
}

impl TerrainLayer {
    pub fn new(id: usize, name: &str, width: usize, height: usize) -> Self {
        TerrainLayer {
            id,
            name: name.to_string(),
            weight_map: vec![0.0; width * height],
            tiling: 10.0,
            normal_strength: 1.0,
        }
    }

    pub fn paint(&mut self, cx: f32, cy: f32, brush: &BrushSettings, width: usize, height: usize) {
        let r  = brush.radius.ceil() as i32;
        let cx_i = cx as i32;
        let cy_i = cy as i32;
        let x0 = (cx_i - r).max(0) as usize;
        let y0 = (cy_i - r).max(0) as usize;
        let x1 = (cx_i + r).min(width  as i32 - 1) as usize;
        let y1 = (cy_i + r).min(height as i32 - 1) as usize;

        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx   = x as f32 - cx;
                let dy   = y as f32 - cy;
                let dist = (dx*dx + dy*dy).sqrt();
                if dist > brush.radius { continue; }
                let w = brush.weight_at_radius(dist);
                let idx = y * width + x;
                self.weight_map[idx] = (self.weight_map[idx] + w).clamp(0.0, 1.0);
            }
        }
    }
}

/// Normalize paint weights across layers so they sum to 1
pub fn normalize_paint_weights(layers: &mut [TerrainLayer], width: usize, height: usize) {
    for i in 0..width * height {
        let total: f32 = layers.iter().map(|l| l.weight_map[i]).sum();
        if total > 1e-6 {
            for l in layers.iter_mut() {
                l.weight_map[i] /= total;
            }
        }
    }
}

// ============================================================
//  WEATHER EFFECTS (PARTICLE RAIN/SNOW SIMULATION)
// ============================================================

#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life:     f32,
    pub max_life: f32,
    pub size:     f32,
    pub color:    Vec4,
}

impl Particle {
    pub fn lifetime_t(&self) -> f32 { 1.0 - self.life / self.max_life }
}

pub struct ParticleSystem {
    pub particles: Vec<Particle>,
    pub max_count: usize,
    rng:           LcgRng,
}

impl ParticleSystem {
    pub fn new(max_count: usize, seed: u64) -> Self {
        ParticleSystem { particles: Vec::with_capacity(max_count), max_count, rng: LcgRng::new(seed) }
    }

    pub fn emit_rain(&mut self, camera_pos: Vec3, wind: Vec2, density: f32) {
        let count = (density * self.max_count as f32) as usize;
        let existing = self.particles.len();
        let to_emit  = (count.saturating_sub(existing)).min(500);

        for _ in 0..to_emit {
            let rx = camera_pos.x + (self.rng.next_f32() - 0.5) * 200.0;
            let rz = camera_pos.z + (self.rng.next_f32() - 0.5) * 200.0;
            let ry = camera_pos.y + 80.0 + self.rng.next_f32() * 40.0;

            self.particles.push(Particle {
                position: Vec3::new(rx, ry, rz),
                velocity: Vec3::new(wind.x * 0.3, -10.0 - self.rng.next_f32() * 5.0, wind.y * 0.3),
                life:     0.5 + self.rng.next_f32() * 2.0,
                max_life: 2.5,
                size:     0.02 + self.rng.next_f32() * 0.01,
                color:    Vec4::new(0.6, 0.7, 0.9, 0.6),
            });
        }
    }

    pub fn emit_snow(&mut self, camera_pos: Vec3, wind: Vec2, density: f32) {
        let count    = (density * self.max_count as f32) as usize;
        let existing = self.particles.len();
        let to_emit  = (count.saturating_sub(existing)).min(300);

        for _ in 0..to_emit {
            let rx = camera_pos.x + (self.rng.next_f32() - 0.5) * 300.0;
            let rz = camera_pos.z + (self.rng.next_f32() - 0.5) * 300.0;
            let ry = camera_pos.y + 60.0 + self.rng.next_f32() * 30.0;

            self.particles.push(Particle {
                position: Vec3::new(rx, ry, rz),
                velocity: Vec3::new(
                    wind.x * 0.5 + (self.rng.next_f32() - 0.5) * 0.5,
                    -1.5 - self.rng.next_f32(),
                    wind.y * 0.5 + (self.rng.next_f32() - 0.5) * 0.5,
                ),
                life:     3.0 + self.rng.next_f32() * 4.0,
                max_life: 7.0,
                size:     0.05 + self.rng.next_f32() * 0.08,
                color:    Vec4::new(0.95, 0.97, 1.0, 0.8),
            });
        }
    }

    pub fn update(&mut self, dt: f32, gravity: f32) {
        self.particles.retain_mut(|p| {
            p.velocity.y -= gravity * dt;
            p.position    += p.velocity * dt;
            p.life        -= dt;
            p.life > 0.0
        });
    }

    pub fn count(&self) -> usize { self.particles.len() }
}

// ============================================================
//  GRID SNAPPING & COORDINATE HELPERS
// ============================================================

pub fn snap_to_grid(pos: Vec3, grid_size: f32) -> Vec3 {
    Vec3::new(
        (pos.x / grid_size).round() * grid_size,
        (pos.y / grid_size).round() * grid_size,
        (pos.z / grid_size).round() * grid_size,
    )
}

pub fn snap_to_terrain(pos: Vec3, hmap: &Heightmap, cell_size: f32, height_scale: f32) -> Vec3 {
    let gx = pos.x / cell_size;
    let gz = pos.z / cell_size;
    let ux = (gx / hmap.width  as f32).clamp(0.0, 1.0);
    let uz = (gz / hmap.height as f32).clamp(0.0, 1.0);
    let h  = hmap.sample_bilinear(ux, uz) * height_scale;
    Vec3::new(pos.x, h, pos.z)
}

pub fn world_to_uv(pos: Vec3, world_width: f32, world_depth: f32) -> Vec2 {
    Vec2::new(
        (pos.x / world_width).clamp(0.0, 1.0),
        (pos.z / world_depth).clamp(0.0, 1.0),
    )
}

// ============================================================
//  SCENE GRAPH PLACEHOLDER (editor objects)
// ============================================================

#[derive(Clone, Debug)]
pub enum EditorObjectKind {
    SpawnPoint,
    Trigger { radius: f32 },
    LightProbe { radius: f32 },
    NavigationMarker,
    CustomMarker { label: String },
}

#[derive(Clone, Debug)]
pub struct EditorObject {
    pub id:        u32,
    pub name:      String,
    pub transform: Mat4,
    pub kind:      EditorObjectKind,
    pub visible:   bool,
    pub locked:    bool,
    pub selected:  bool,
}

impl EditorObject {
    pub fn new(id: u32, name: &str, pos: Vec3, kind: EditorObjectKind) -> Self {
        EditorObject {
            id,
            name:      name.to_string(),
            transform: Mat4::from_translation(pos),
            kind,
            visible:   true,
            locked:    false,
            selected:  false,
        }
    }

    pub fn position(&self) -> Vec3 {
        Vec3::new(self.transform.w_axis.x, self.transform.w_axis.y, self.transform.w_axis.z)
    }
}

// ============================================================
//  MINIMAP / OVERVIEW MAP
// ============================================================

pub struct Minimap {
    pub width:   usize,
    pub height:  usize,
    pub pixels:  Vec<Vec4>,  // RGBA
    pub ramp:    ColorRamp,
    pub dirty:   bool,
}

impl Minimap {
    pub fn new(width: usize, height: usize) -> Self {
        Minimap {
            width,
            height,
            pixels: vec![Vec4::ZERO; width * height],
            ramp:   ColorRamp::terrain_default(),
            dirty:  true,
        }
    }

    /// Render the minimap from the heightmap + biome data
    pub fn update(&mut self, hmap: &Heightmap, ao_map: Option<&[f32]>, sea_level: f32) {
        let tw = hmap.width;
        let th = hmap.height;

        for y in 0..self.height {
            for x in 0..self.width {
                let u  = x as f32 / self.width  as f32;
                let v  = y as f32 / self.height as f32;
                let h  = hmap.sample_bilinear(u, v);
                let mut color = self.ramp.sample(h);

                // Apply AO if available
                if let Some(ao) = ao_map {
                    let ax  = (u * (tw - 1) as f32) as usize;
                    let ay  = (v * (th - 1) as f32) as usize;
                    let ao_val = ao[ay * tw + ax];
                    color *= ao_val * 0.7 + 0.3;
                }

                self.pixels[y * self.width + x] = Vec4::new(color.x, color.y, color.z, 1.0);
            }
        }
        self.dirty = false;
    }

    /// Draw a marker at a world position
    pub fn draw_marker(&mut self, world_x: f32, world_z: f32, world_w: f32, world_h: f32, color: Vec4) {
        let u  = (world_x / world_w).clamp(0.0, 1.0);
        let v  = (world_z / world_h).clamp(0.0, 1.0);
        let px = (u * (self.width  - 1) as f32) as usize;
        let py = (v * (self.height - 1) as f32) as usize;

        let radius = 3usize;
        let x0 = px.saturating_sub(radius);
        let y0 = py.saturating_sub(radius);
        let x1 = (px + radius).min(self.width  - 1);
        let y1 = (py + radius).min(self.height - 1);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let dx = cx as i32 - px as i32;
                let dy = cy as i32 - py as i32;
                if dx*dx + dy*dy <= (radius*radius) as i32 {
                    self.pixels[cy * self.width + cx] = color;
                }
            }
        }
    }
}

// ============================================================
//  EDITOR STATE SNAPSHOT (for save/restore)
// ============================================================

#[derive(Clone, Debug)]
pub struct EditorStateSnapshot {
    pub heightmap_data:  Vec<f32>,
    pub foliage_count:   usize,
    pub lake_count:      usize,
    pub river_count:     usize,
    pub road_count:      usize,
    pub utc_hour:        f64,
    pub day_of_year:     f64,
    pub weather_state:   WeatherState,
    pub sea_level:       f32,
    pub world_name:      String,
}

impl WorldEditor {
    pub fn snapshot(&self) -> EditorStateSnapshot {
        EditorStateSnapshot {
            heightmap_data:  self.heightmap.data.clone(),
            foliage_count:   self.foliage.len(),
            lake_count:      self.lakes.len(),
            river_count:     self.rivers.len(),
            road_count:      self.road_network.segments.len(),
            utc_hour:        self.utc_hour,
            day_of_year:     self.day_of_year,
            weather_state:   self.weather.current.state,
            sea_level:       self.sea_level,
            world_name:      self.world_name.clone(),
        }
    }

    pub fn restore_heightmap(&mut self, snapshot: &EditorStateSnapshot) {
        if snapshot.heightmap_data.len() == self.heightmap.data.len() {
            self.heightmap.data = snapshot.heightmap_data.clone();
            self.heightmap.recompute_minmax();
            self.heightmap_dirty = true;
        }
    }
}

// ============================================================
//  STRESS TESTS / BENCHMARKING HELPERS
// ============================================================

pub fn benchmark_noise(width: usize, height: usize, params: &FbmParams) -> f64 {
    let mut sum = 0.0f64;
    for y in 0..height {
        for x in 0..width {
            let nx = x as f32 / width  as f32;
            let ny = y as f32 / height as f32;
            sum += fbm_2d(nx, ny, params) as f64;
        }
    }
    sum / (width * height) as f64
}

pub fn benchmark_erosion(size: usize) -> Heightmap {
    let mut hmap = Heightmap::new(size, size);
    let params = FbmParams::default_terrain();
    hmap.generate_fbm(&params, Vec2::ZERO);
    let ep = ErosionParams { num_particles: 10_000, ..Default::default() };
    hydraulic_erosion(&mut hmap, &ep);
    hmap
}

pub fn benchmark_pathfinding(hmap: &Heightmap) -> Option<Vec<GridNode>> {
    let w = hmap.width  as i32;
    let h = hmap.height as i32;
    let start = GridNode::new(1, 1);
    let goal  = GridNode::new(w - 2, h - 2);
    astar_path(hmap, start, goal, &RoadCostParams::default())
}

// ============================================================
//  TRAIT IMPLEMENTATIONS
// ============================================================

impl Default for WorldEditor {
    fn default() -> Self {
        WorldEditor::new(512, 512, 1.0)
    }
}

impl std::fmt::Display for WeatherState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::fmt::Display for BiomeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for WorldStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,
            "World: {}x{} cells | Land: {} | Ocean: {} | Rivers: {} | Lakes: {} | Roads: {} segs ({:.0}m) | Foliage: {}",
            (self.total_cells as f32).sqrt() as usize,
            (self.total_cells as f32).sqrt() as usize,
            self.land_cells,
            self.ocean_cells,
            self.river_count,
            self.lake_count,
            self.road_segments,
            self.road_total_length,
            self.foliage_count,
        )
    }
}

impl Clone for EditAction {
    fn clone(&self) -> Self {
        match self {
            EditAction::SetHeightRegion { x, y, width, height, old_data, new_data } =>
                EditAction::SetHeightRegion { x: *x, y: *y, width: *width, height: *height, old_data: old_data.clone(), new_data: new_data.clone() },
            EditAction::PlaceFoliageInstances { instances, indices } =>
                EditAction::PlaceFoliageInstances { instances: instances.clone(), indices: indices.clone() },
            EditAction::RemoveFoliageInstances { indices, instances } =>
                EditAction::RemoveFoliageInstances { indices: indices.clone(), instances: instances.clone() },
            EditAction::AddRoadSegment { segment_id, segment } =>
                EditAction::AddRoadSegment { segment_id: *segment_id, segment: segment.clone() },
            EditAction::RemoveRoadSegment { segment_id, segment } =>
                EditAction::RemoveRoadSegment { segment_id: *segment_id, segment: segment.clone() },
            EditAction::SetBiomeOverride { x, y, old_biome, new_biome } =>
                EditAction::SetBiomeOverride { x: *x, y: *y, old_biome: *old_biome, new_biome: *new_biome },
            EditAction::AddWaterBody { lake, index } =>
                EditAction::AddWaterBody { lake: lake.clone(), index: *index },
            EditAction::RemoveWaterBody { lake, index } =>
                EditAction::RemoveWaterBody { lake: lake.clone(), index: *index },
            EditAction::CompoundAction { actions, description } =>
                EditAction::CompoundAction { actions: actions.clone(), description: description.clone() },
        }
    }
}

// ============================================================
//  ROCK PLACEMENT SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct RockInstance {
    pub position:  Vec3,
    pub rotation:  Quat,
    pub scale:     Vec3,
    pub rock_type: u8,
}

pub fn place_rocks(
    hmap:      &Heightmap,
    cell_size: f32,
    min_slope: f32,  // radians — rocks appear on steep slopes
    max_alt:   f32,
    density:   f32,
    seed:      u64,
) -> Vec<RockInstance> {
    let w   = hmap.width  as f32;
    let h   = hmap.height as f32;
    let min_dist = 3.0 + (1.0 - density) * 7.0;
    let candidates = poisson_disk_2d(w, h, min_dist, 30, seed);
    let mut rng = LcgRng::new(seed ^ 0xB00B);
    let mut result = Vec::new();

    for pos in &candidates {
        let ux = (pos.x / w).clamp(0.0, 1.0);
        let uy = (pos.y / h).clamp(0.0, 1.0);
        let alt   = hmap.sample_bilinear(ux, uy);
        if alt > max_alt { continue; }

        let xi = pos.x as usize;
        let yi = pos.y as usize;
        let xi_c = xi.min(hmap.width  - 1);
        let yi_c = yi.min(hmap.height - 1);
        let slope = hmap.slope_at(xi_c, yi_c, cell_size);
        if slope < min_slope { continue; }

        let angle = rng.next_f32() * TWO_PI;
        let tilt  = slope * 0.5;
        let rot   = Quat::from_rotation_y(angle) * Quat::from_rotation_x(tilt);

        let sv  = 0.5 + rng.next_f32() * 2.0;
        let sxz = 0.7 + rng.next_f32() * 0.6;
        let scale = Vec3::new(sv * sxz, sv, sv * sxz);

        let world_y = alt * 500.0;
        result.push(RockInstance {
            position:  Vec3::new(pos.x * cell_size, world_y, pos.y * cell_size),
            rotation:  rot,
            scale,
            rock_type: (rng.next_f32() * 8.0) as u8,
        });
    }

    result
}

// ============================================================
//  THERMAL GRADIENT MAP (for snow accumulation)
// ============================================================

pub fn compute_snow_accumulation(
    hmap:        &Heightmap,
    temp_map:    &[f32],
    snow_line:   f32,        // normalised altitude above which snow can form
    temp_thresh: f32,        // temperature threshold for snow (°C)
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut snow_map = vec![0.0f32; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let alt  = hmap.get(x, y);
            let temp = if idx < temp_map.len() { temp_map[idx] } else { 15.0 };
            if alt >= snow_line && temp <= temp_thresh {
                // More snow at higher altitudes and colder temperatures
                let alt_factor  = ((alt - snow_line) / (1.0 - snow_line)).clamp(0.0, 1.0);
                let temp_factor = ((temp_thresh - temp) / 30.0).clamp(0.0, 1.0);
                snow_map[idx]   = (alt_factor * 0.6 + temp_factor * 0.4).clamp(0.0, 1.0);
            }
        }
    }
    snow_map
}

// ============================================================
//  LARGE CONSTANT DATA TABLES
// ============================================================

/// Noise permutation indices for higher-quality scrambling (second set)
pub const PERM2: [u8; 256] = [
    198, 11,  59, 119, 138,  22,  40, 216,  69, 175,  89, 201,  90, 142,  76, 250,
    220,  37, 104,  82, 127, 248,  13,  99, 179,  42, 222, 194, 230, 106,  26, 155,
     36,  83,  18,  72,  67,  17, 162, 167, 147, 137,  50, 133,  23, 213,  80, 125,
    200, 192,  29, 180,  10, 218, 146, 183, 234,  60, 215,  38, 244, 239, 169,  91,
     34, 190, 185, 171,  27, 203, 240, 254, 158,  52, 249, 153, 214,  54,  47, 207,
    140,  55, 102, 182, 111, 170, 232, 101,  96, 173, 166, 136,  43,  20,  88, 115,
    129, 156, 126, 233, 221,  74,  62,  48,  86,  35, 109, 224, 165, 131, 187, 246,
     71,  63, 141, 108,  24, 148,  45,  79, 121, 210, 144, 196,  93, 228,  28,   9,
    177, 118, 110, 120, 243,  41, 251, 107,  49, 117, 160,  85, 247,  65,   6,  64,
    189,  58, 132, 235,  75,   7, 163, 205, 188,   3, 139, 197, 208, 150, 116, 168,
     15,  95,  16, 151, 217,  77,  66, 152, 204,  57, 199,  12, 161, 184,  81,  31,
    229, 211,  53,  39,  78, 206, 236,   4,  46,  25, 227, 241, 174, 159,  14, 253,
    154, 191,  73, 238, 135, 209, 181,  33, 226, 123,  68,  32, 130, 193,  21,  84,
    237, 123, 145, 172,  44,   5, 176, 143, 100, 219, 114,  56, 252, 149,  92, 245,
    103, 157,   2,   8,  19,  97, 122, 202, 134, 255, 112,  30,  70, 186,  61,  98,
    105,  94, 113,  87, 231, 178, 164, 124,  51,   1, 212,  76, 128, 242, 223, 195,
];

/// Biome temperature-humidity classification lookup string (for debugging)
pub const BIOME_CLASSIFICATION_TABLE: &str = "\
T >24 H >0.80 -> Tropical Rainforest\n\
T >24 H >0.50 -> Tropical Savanna\n\
T >24 H >0.25 -> Xeric Shrubland\n\
T >24 H     * -> Hot Desert\n\
T >10 H >0.70 -> Temperate Rainforest\n\
T >10 H >0.50 -> Temperate Deciduous\n\
T >10 H >0.28 -> Mediterranean Shrub\n\
T >10 H     * -> Xeric Shrubland\n\
T  >0 H >0.65 -> Boreal Forest\n\
T  >0 H >0.40 -> Temperate Grassland\n\
T  >0 H     * -> Cold Desert\n\
T>-10 H >0.50 -> Taiga Spruce\n\
T>-10 H     * -> Tundra\n\
T    * H >0.30 -> Tundra\n\
T    * H     * -> Arctic Desert\n\
ALT >0.88      -> Polar Ice Cap\n\
ALT >0.75      -> Alpine Tundra\n\
ALT >0.62      -> Alpine Meadow\n";

// ============================================================
//  ADDITIONAL WORLD EDITOR METHODS
// ============================================================

impl WorldEditor {
    /// Generate a complete terrain from a user-provided seed value
    pub fn generate_from_seed(&mut self, seed: u64) {
        self.master_seed = seed;
        let mut rng = LcgRng::new(seed);
        self.terrain_fbm_params.octaves    = 7 + (rng.next_f32() * 3.0) as usize;
        self.terrain_fbm_params.lacunarity = 1.8 + rng.next_f32() * 0.5;
        self.terrain_fbm_params.gain       = 0.45 + rng.next_f32() * 0.15;
        self.warp_strength                 = 0.2  + rng.next_f32() * 0.4;
        self.sea_level                     = 0.15 + rng.next_f32() * 0.15;
        self.generate_terrain();
        self.apply_erosion();
        self.apply_thermal_erosion(3, 30.0 + rng.next_f32() * 15.0);
        self.generate_climate();
        self.generate_rivers(5 + (rng.next_f32() * 10.0) as usize);
        self.generate_lakes(3 + (rng.next_f32() * 7.0) as usize, 0.02);
        self.generate_shore();
        self.compute_stats();
    }

    /// Get height at a world position (x, z in world units)
    pub fn height_at_world(&self, x: f32, z: f32) -> f32 {
        let ux = (x / (self.heightmap.width  as f32 * self.cell_size)).clamp(0.0, 1.0);
        let uz = (z / (self.heightmap.height as f32 * self.cell_size)).clamp(0.0, 1.0);
        self.heightmap.sample_bilinear(ux, uz) * self.height_scale
    }

    /// Get surface normal at a world position
    pub fn normal_at_world(&self, x: f32, z: f32) -> Vec3 {
        let gx = (x / self.cell_size) as usize;
        let gz = (z / self.cell_size) as usize;
        let gx_c = gx.min(self.heightmap.width  - 1);
        let gz_c = gz.min(self.heightmap.height - 1);
        self.heightmap.normal_at(gx_c, gz_c, self.cell_size)
    }

    /// Check if a world position is underwater
    pub fn is_underwater(&self, x: f32, z: f32) -> bool {
        let alt = self.height_at_world(x, z) / self.height_scale;
        alt <= self.sea_level
    }

    /// Get biome at a world position
    pub fn biome_at_world(&self, x: f32, z: f32) -> BiomeId {
        let gx = ((x / self.cell_size) as usize).min(self.heightmap.width  - 1);
        let gz = ((z / self.cell_size) as usize).min(self.heightmap.height - 1);
        let idx  = gz * self.heightmap.width + gx;
        let temp = if idx < self.temperature_map.len() { self.temperature_map[idx] } else { 15.0 };
        let hum  = if idx < self.humidity_map.len()    { self.humidity_map[idx]    } else { 0.5  };
        let alt  = self.heightmap.get(gx, gz);
        BiomeDescriptor::classify_point(temp, hum, alt)
    }

    /// Resize the world (resample heightmap to new dimensions)
    pub fn resize_world(&mut self, new_width: usize, new_height: usize) {
        let mut new_hmap = Heightmap::new(new_width, new_height);
        for y in 0..new_height {
            for x in 0..new_width {
                let u = x as f32 / (new_width  - 1) as f32;
                let v = y as f32 / (new_height - 1) as f32;
                let h = self.heightmap.sample_bilinear(u, v);
                new_hmap.set(x, y, h);
            }
        }
        new_hmap.recompute_minmax();
        self.heightmap        = new_hmap;
        self.temperature_map  = vec![15.0; new_width * new_height];
        self.humidity_map     = vec![0.5;  new_width * new_height];
        self.climate_dirty    = true;
        self.heightmap_dirty  = true;
        self.foliage.clear();
        self.foliage_dirty    = true;
    }

    /// Apply a heightmap from external data
    pub fn import_heightmap(&mut self, data: &[f32], width: usize, height: usize) {
        if data.len() != width * height { return; }
        self.heightmap = Heightmap {
            width,
            height,
            data:  data.to_vec(),
            min_h: 0.0,
            max_h: 1.0,
        };
        self.heightmap.recompute_minmax();
        self.heightmap.normalize_to_01();
        self.heightmap_dirty = true;
        self.climate_dirty   = true;
    }

    /// Export heightmap as 16-bit grayscale image bytes (big-endian per pixel)
    pub fn export_heightmap_u16(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.heightmap.data.len() * 2);
        for &h in &self.heightmap.data {
            let v = (h.clamp(0.0, 1.0) * 65535.0) as u16;
            out.push((v >> 8) as u8);
            out.push((v & 0xFF) as u8);
        }
        out
    }

    /// Export heightmap as 8-bit grayscale image bytes
    pub fn export_heightmap_u8(&self) -> Vec<u8> {
        self.heightmap.data.iter()
            .map(|&h| (h.clamp(0.0, 1.0) * 255.0) as u8)
            .collect()
    }

    /// Compute horizon angle at a point (used for ambient lighting)
    pub fn horizon_angle_at(&self, x: usize, y: usize, direction: f32, max_dist: f32) -> f32 {
        let base_h = self.heightmap.get(x, y) * self.height_scale;
        let dx     = direction.cos();
        let dz     = direction.sin();
        let steps  = (max_dist / self.cell_size) as usize;
        let mut max_elev = 0.0f32;

        for step in 1..=steps {
            let t   = step as f32 * self.cell_size;
            let nx  = x as f32 + dx * t / self.cell_size;
            let nz  = y as f32 + dz * t / self.cell_size;
            let w   = self.heightmap.width  as f32;
            let h   = self.heightmap.height as f32;
            if nx < 0.0 || nz < 0.0 || nx >= w || nz >= h { break; }
            let ux  = nx / w;
            let uz  = nz / h;
            let nh  = self.heightmap.sample_bilinear(ux, uz) * self.height_scale;
            let elev = ((nh - base_h) / t).atan();
            if elev > max_elev { max_elev = elev; }
        }
        max_elev
    }
}

// ============================================================
//  FINAL UTILITY FUNCTIONS
// ============================================================

/// Compute approximate visual radius of the world at a given altitude
pub fn visual_radius_from_altitude(altitude_km: f32) -> f32 {
    // Horizon distance for a sphere: sqrt(2 * R * h + h²) in km
    let r = EARTH_RADIUS as f32;
    let h = altitude_km;
    (2.0 * r * h + h * h).sqrt()
}

/// Convert world height in normalised units to metres
pub fn normalised_to_metres(h: f32, height_scale_m: f32) -> f32 {
    h * height_scale_m
}

/// Great-circle distance between two lat/lon points (Haversine formula), returns km
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r    = EARTH_RADIUS;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a    = (dlat / 2.0).sin().powi(2)
             + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    r * c
}

/// Convert temperature from Celsius to Fahrenheit
#[inline] pub fn c_to_f(c: f32) -> f32 { c * 1.8 + 32.0 }

/// Convert temperature from Fahrenheit to Celsius
#[inline] pub fn f_to_c(f: f32) -> f32 { (f - 32.0) / 1.8 }

/// Dew point from temperature and humidity (Magnus formula)
pub fn dew_point(temp_c: f32, relative_humidity: f32) -> f32 {
    let a = 17.27f32;
    let b = 237.7f32;
    let alpha = (a * temp_c / (b + temp_c)) + (relative_humidity.max(1e-5)).ln();
    b * alpha / (a - alpha)
}

/// Wind chill temperature (Steadman 1971 approximation)
pub fn wind_chill(temp_c: f32, wind_speed_ms: f32) -> f32 {
    if wind_speed_ms < 1.4 || temp_c > 10.0 { return temp_c; }
    let v = wind_speed_ms * 3.6; // to km/h
    13.12 + 0.6215 * temp_c - 11.37 * v.powf(0.16) + 0.3965 * temp_c * v.powf(0.16)
}

/// Heat index (Rothfusz regression)
pub fn heat_index(temp_c: f32, humidity: f32) -> f32 {
    let t = c_to_f(temp_c);
    let r = humidity * 100.0; // percent
    let hi = -42.379
        + 2.04901523 * t
        + 10.14333127 * r
        - 0.22475541 * t * r
        - 0.00683783 * t * t
        - 0.05481717 * r * r
        + 0.00122874 * t * t * r
        + 0.00085282 * t * r * r
        - 0.00000199 * t * t * r * r;
    f_to_c(hi)
}

/// Beaufort wind scale classification
pub fn beaufort_scale(wind_speed_ms: f32) -> u8 {
    match wind_speed_ms as u32 {
        0         => 0,
        1..=2     => 1,
        3..=5     => 2,
        6..=9     => 3,
        10..=14   => 4,
        15..=21   => 5,
        22..=29   => 6,
        30..=38   => 7,
        39..=49   => 8,
        50..=61   => 9,
        62..=74   => 10,
        75..=88   => 11,
        _         => 12,
    }
}

// ============================================================
//  EXTENDED NOISE FUNCTIONS
// ============================================================

/// Value noise (simpler than Perlin, interpolates grid values)
pub fn value_noise_2d(x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let u  = fade(xf);
    let v  = fade(yf);
    let aa = PERM[((PERM[(xi & 255) as usize] as i32 + (yi & 255)) & 255) as usize] as f32 / 255.0;
    let ba = PERM[((PERM[((xi+1) & 255) as usize] as i32 + (yi & 255)) & 255) as usize] as f32 / 255.0;
    let ab = PERM[((PERM[(xi & 255) as usize] as i32 + ((yi+1) & 255)) & 255) as usize] as f32 / 255.0;
    let bb = PERM[((PERM[((xi+1) & 255) as usize] as i32 + ((yi+1) & 255)) & 255) as usize] as f32 / 255.0;
    lerp_f(lerp_f(aa, ba, u), lerp_f(ab, bb, u), v)
}

/// Voronoi noise — returns distance to nearest feature point and feature ID
pub fn voronoi_noise_2d(x: f32, y: f32, jitter: f32) -> (f32, u32) {
    let cx = x.floor() as i32;
    let cy = y.floor() as i32;
    let mut min_dist = f32::MAX;
    let mut min_id   = 0u32;
    for dy in -2..=2i32 {
        for dx in -2..=2i32 {
            let nx = cx + dx;
            let ny = cy + dy;
            let h  = worley_hash(nx, ny);
            let fx = nx as f32 + jitter * ((h & 0xFFFF) as f32 / 65535.0 - 0.5) * 2.0 + 0.5;
            let fy = ny as f32 + jitter * (((h >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5) * 2.0 + 0.5;
            let dist = ((fx - x) * (fx - x) + (fy - y) * (fy - y)).sqrt();
            if dist < min_dist { min_dist = dist; min_id = h; }
        }
    }
    (min_dist, min_id)
}

/// Ridged multifractal noise (mountain ridges)
pub fn ridged_multifractal_2d(x: f32, y: f32, octaves: usize, freq: f32, lacunarity: f32, gain: f32, offset: f32) -> f32 {
    let mut f      = freq;
    let mut amp    = 1.0f32;
    let mut value  = 0.0f32;
    let mut weight = 1.0f32;
    for _ in 0..octaves {
        let n      = (offset - perlin_noise_2d(x * f, y * f).abs()).abs();
        let signal = n * n * weight;
        weight     = (signal * 2.0).clamp(0.0, 1.0);
        value     += signal * amp;
        f         *= lacunarity;
        amp       *= gain;
    }
    value
}

// ============================================================
//  MESH GENERATION FROM HEIGHTMAP
// ============================================================

#[derive(Clone, Debug)]
pub struct TerrainMesh {
    pub vertices:  Vec<Vec3>,
    pub normals:   Vec<Vec3>,
    pub uvs:       Vec<Vec2>,
    pub indices:   Vec<u32>,
    pub lod_level: u8,
}

impl TerrainMesh {
    pub fn new() -> Self {
        TerrainMesh { vertices: Vec::new(), normals: Vec::new(), uvs: Vec::new(), indices: Vec::new(), lod_level: 0 }
    }
}

/// Generate a mesh from a chunk of the heightmap
pub fn generate_terrain_mesh(
    hmap:        &Heightmap,
    chunk_x:     usize,
    chunk_z:     usize,
    chunk_size:  usize,
    cell_size:   f32,
    height_scale: f32,
    lod_step:    usize,
) -> TerrainMesh {
    let step  = lod_step.max(1);
    let x_end = (chunk_x + chunk_size).min(hmap.width  - 1);
    let z_end = (chunk_z + chunk_size).min(hmap.height - 1);
    let mut mesh = TerrainMesh::new();
    let mut vert_idx_map: HashMap<(usize, usize), u32> = HashMap::new();

    let mut xz = chunk_z;
    while xz <= z_end {
        let mut xx = chunk_x;
        while xx <= x_end {
            let h    = hmap.get(xx, xz) * height_scale;
            let pos  = Vec3::new(xx as f32 * cell_size, h, xz as f32 * cell_size);
            let norm = hmap.normal_at(xx, xz, cell_size);
            let uv   = Vec2::new(
                (xx - chunk_x) as f32 / chunk_size as f32,
                (xz - chunk_z) as f32 / chunk_size as f32,
            );
            let idx = mesh.vertices.len() as u32;
            vert_idx_map.insert((xx, xz), idx);
            mesh.vertices.push(pos);
            mesh.normals.push(norm);
            mesh.uvs.push(uv);
            xx += step;
        }
        xz += step;
    }

    let mut xz = chunk_z;
    while xz + step <= z_end {
        let mut xx = chunk_x;
        while xx + step <= x_end {
            let nx = (xx + step).min(x_end);
            let nz = (xz + step).min(z_end);
            if let (Some(&i00), Some(&i10), Some(&i01), Some(&i11)) = (
                vert_idx_map.get(&(xx, xz)),
                vert_idx_map.get(&(nx, xz)),
                vert_idx_map.get(&(xx, nz)),
                vert_idx_map.get(&(nx, nz)),
            ) {
                mesh.indices.extend_from_slice(&[i00, i10, i01, i10, i11, i01]);
            }
            xx += step;
        }
        xz += step;
    }
    mesh
}

/// Compute per-vertex tangents for normal mapping
pub fn compute_tangents(mesh: &mut TerrainMesh) -> Vec<Vec3> {
    let mut tangents = vec![Vec3::ZERO; mesh.vertices.len()];
    let tri_count    = mesh.indices.len() / 3;
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3]     as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        let v0 = mesh.vertices[i0];
        let v1 = mesh.vertices[i1];
        let v2 = mesh.vertices[i2];
        let uv0 = mesh.uvs[i0];
        let uv1 = mesh.uvs[i1];
        let uv2 = mesh.uvs[i2];
        let e1  = v1 - v0;
        let e2  = v2 - v0;
        let du1 = uv1.x - uv0.x;
        let dv1 = uv1.y - uv0.y;
        let du2 = uv2.x - uv0.x;
        let dv2 = uv2.y - uv0.y;
        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-10 { continue; }
        let tang = (e1 * dv2 - e2 * dv1) / det;
        tangents[i0] += tang;
        tangents[i1] += tang;
        tangents[i2] += tang;
    }
    tangents.iter().enumerate().map(|(i, t)| {
        let n = mesh.normals[i];
        (*t - n * n.dot(*t)).normalize_or_zero()
    }).collect()
}

// ============================================================
//  WIND SIMULATION ON TERRAIN
// ============================================================

#[derive(Clone, Debug)]
pub struct WindField {
    pub width:      usize,
    pub height:     usize,
    pub vectors:    Vec<Vec2>,
    pub turbulence: Vec<f32>,
}

impl WindField {
    pub fn new(width: usize, height: usize) -> Self {
        WindField { width, height, vectors: vec![Vec2::ZERO; width*height], turbulence: vec![0.0; width*height] }
    }

    pub fn generate_from_terrain(hmap: &Heightmap, base_wind: Vec2, turbulence_strength: f32, seed: u64) -> Self {
        let w = hmap.width;
        let h = hmap.height;
        let mut field = WindField::new(w, h);
        let fbm_p = FbmParams { octaves: 4, frequency: 2.0, lacunarity: 2.0, gain: 0.5, amplitude: 1.0, offset: 0.0, ridge: false };
        for y in 0..h {
            for x in 0..w {
                let idx   = y * w + x;
                let slope = hmap.slope_at(x, y, 1.0);
                let grad  = hmap.gradient_at(x, y);
                let upslope = base_wind.dot(grad);
                let deflect = -grad * upslope * slope * 2.0;
                let speed_m = 1.0 + (if upslope < 0.0 { 1.0 } else { 0.0 }) * slope * 0.5;
                let nx      = x as f32 / w as f32 + seed as f32 * 1e-5;
                let ny      = y as f32 / h as f32;
                let noise_x = fbm_2d(nx,         ny,         &fbm_p) * turbulence_strength;
                let noise_y = fbm_2d(nx + 100.0, ny + 100.0, &fbm_p) * turbulence_strength;
                field.vectors[idx]    = (base_wind + deflect) * speed_m + Vec2::new(noise_x, noise_y);
                field.turbulence[idx] = noise_x.abs() + noise_y.abs();
            }
        }
        field
    }

    pub fn sample(&self, x: f32, y: f32) -> Vec2 {
        let gx = x.clamp(0.0, (self.width  - 1) as f32);
        let gy = y.clamp(0.0, (self.height - 1) as f32);
        let x0 = gx.floor() as usize;
        let y0 = gy.floor() as usize;
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = gx - x0 as f32;
        let ty = gy - y0 as f32;
        let a  = self.vectors[y0 * self.width + x0];
        let b  = self.vectors[y0 * self.width + x1];
        let c  = self.vectors[y1 * self.width + x0];
        let d  = self.vectors[y1 * self.width + x1];
        a.lerp(b, tx).lerp(c.lerp(d, tx), ty)
    }
}

// ============================================================
//  HEIGHTMAP MASK OPERATIONS
// ============================================================

/// Generate radial falloff mask (circular island shape)
pub fn generate_island_mask(width: usize, height: usize, falloff_exp: f32) -> Vec<f32> {
    let mut mask = vec![0.0f32; width * height];
    let cx   = width  as f32 * 0.5;
    let cy   = height as f32 * 0.5;
    let max_r = cx.min(cy) * 0.95;
    for y in 0..height {
        for x in 0..width {
            let dx   = x as f32 - cx;
            let dy   = y as f32 - cy;
            let t    = ((dx*dx + dy*dy).sqrt() / max_r).clamp(0.0, 1.0);
            mask[y * width + x] = (1.0 - t.powf(falloff_exp)).clamp(0.0, 1.0);
        }
    }
    mask
}

/// Apply mask to heightmap
pub fn apply_mask(hmap: &mut Heightmap, mask: &[f32]) {
    let len = hmap.data.len().min(mask.len());
    for i in 0..len { hmap.data[i] *= mask[i]; }
    hmap.recompute_minmax();
}

#[derive(Clone, Debug)]
pub enum MaskBlendMode { Add, Multiply, Screen, Max, Min, Subtract }

pub fn blend_masks(a: &[f32], b: &[f32], mode: MaskBlendMode) -> Vec<f32> {
    let len = a.len().min(b.len());
    (0..len).map(|i| match mode {
        MaskBlendMode::Add      => (a[i] + b[i]).clamp(0.0, 1.0),
        MaskBlendMode::Multiply => a[i] * b[i],
        MaskBlendMode::Screen   => 1.0 - (1.0 - a[i]) * (1.0 - b[i]),
        MaskBlendMode::Max      => a[i].max(b[i]),
        MaskBlendMode::Min      => a[i].min(b[i]),
        MaskBlendMode::Subtract => (a[i] - b[i]).clamp(0.0, 1.0),
    }).collect()
}

// ============================================================
//  BEZIER CURVE UTILITIES
// ============================================================

/// Cubic Bezier: B(t) = (1-t)³P0 + 3(1-t)²tP1 + 3(1-t)t²P2 + t³P3
pub fn cubic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let mt = 1.0 - t;
    p0 * (mt*mt*mt) + p1 * (3.0*mt*mt*t) + p2 * (3.0*mt*t*t) + p3 * (t*t*t)
}

/// Cubic Bezier tangent
pub fn cubic_bezier_tangent(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let mt = 1.0 - t;
    (p1 - p0) * (3.0*mt*mt) + (p2 - p1) * (6.0*mt*t) + (p3 - p2) * (3.0*t*t)
}

/// Auto-smooth polyline with Catmull-Rom to Bezier conversion
pub fn auto_smooth_polyline(pts: &[Vec2], tension: f32, samples: usize) -> Vec<Vec2> {
    if pts.len() < 2 { return pts.to_vec(); }
    let mut result = Vec::new();
    for i in 0..pts.len() - 1 {
        let p0 = if i == 0 { pts[0] + (pts[0] - pts[1]) } else { pts[i-1] };
        let p1 = pts[i];
        let p2 = pts[i+1];
        let p3 = if i+2 >= pts.len() { pts[pts.len()-1] + (pts[pts.len()-1] - pts[pts.len()-2]) } else { pts[i+2] };
        let cp1 = p1 + (p2 - p0) * (tension / 6.0);
        let cp2 = p2 - (p3 - p1) * (tension / 6.0);
        for s in 0..samples {
            result.push(cubic_bezier(p1, cp1, cp2, p2, s as f32 / samples as f32));
        }
    }
    result.push(*pts.last().unwrap());
    result
}

/// Compute bezier arc length numerically
pub fn bezier_arc_length(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, steps: usize) -> f32 {
    let mut len  = 0.0f32;
    let mut prev = p0;
    for i in 1..=steps {
        let t    = i as f32 / steps as f32;
        let curr = cubic_bezier(p0, p1, p2, p3, t);
        len += (curr - prev).length();
        prev = curr;
    }
    len
}

// ============================================================
//  HEIGHTMAP CROP / TILE / STITCH
// ============================================================

/// Tile a heightmap to a larger grid
pub fn tile_heightmap(source: &Heightmap, tile_x: usize, tile_y: usize) -> Heightmap {
    let new_w = source.width  * tile_x;
    let new_h = source.height * tile_y;
    let mut out = Heightmap::new(new_w, new_h);
    for ty in 0..tile_y {
        for tx in 0..tile_x {
            for y in 0..source.height {
                for x in 0..source.width {
                    out.set(tx * source.width + x, ty * source.height + y, source.get(x, y));
                }
            }
        }
    }
    out.recompute_minmax();
    out
}

/// Crop sub-region of a heightmap
pub fn crop_heightmap(source: &Heightmap, ox: usize, oy: usize, w: usize, h: usize) -> Heightmap {
    let x_end = (ox + w).min(source.width);
    let y_end = (oy + h).min(source.height);
    let out_w = x_end - ox;
    let out_h = y_end - oy;
    let mut out = Heightmap::new(out_w, out_h);
    for ry in 0..out_h { for rx in 0..out_w { out.set(rx, ry, source.get(ox + rx, oy + ry)); } }
    out.recompute_minmax();
    out
}

/// Stitch two heightmaps side-by-side with blended seam
pub fn stitch_heightmaps_horizontal(left: &Heightmap, right: &Heightmap, seam_width: usize) -> Heightmap {
    assert_eq!(left.height, right.height);
    let total_w = left.width + right.width;
    let h       = left.height;
    let mut out = Heightmap::new(total_w, h);
    for y in 0..h {
        for x in 0..left.width  { out.set(x,             y, left.get(x, y));  }
        for x in 0..right.width { out.set(left.width + x, y, right.get(x, y)); }
    }
    let sw = seam_width.min(left.width).min(right.width);
    for y in 0..h {
        for s in 0..sw {
            let t       = smoothstep(0.0, 1.0, s as f32 / sw as f32);
            let lx      = left.width - sw + s;
            let rx      = left.width + s;
            let blended = lerp_f(out.get(lx, y), out.get(rx, y), t);
            out.set(lx, y, blended);
            out.set(rx, y, blended);
        }
    }
    out.recompute_minmax();
    out
}

// ============================================================
//  VERTEX COLOR PAINTING
// ============================================================

#[derive(Clone, Debug)]
pub struct VertexColorMap {
    pub width:  usize,
    pub height: usize,
    pub data:   Vec<Vec4>,
}

impl VertexColorMap {
    pub fn new(width: usize, height: usize, fill: Vec4) -> Self {
        VertexColorMap { width, height, data: vec![fill; width * height] }
    }

    pub fn paint(&mut self, cx: f32, cy: f32, brush: &BrushSettings, color: Vec4) {
        let r    = brush.radius.ceil() as i32;
        let x0   = ((cx as i32 - r).max(0)) as usize;
        let y0   = ((cy as i32 - r).max(0)) as usize;
        let x1   = ((cx as i32 + r).min(self.width  as i32 - 1)) as usize;
        let y1   = ((cy as i32 + r).min(self.height as i32 - 1)) as usize;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                if dist > brush.radius { continue; }
                let w   = brush.weight_at_radius(dist);
                let idx = y * self.width + x;
                let old = self.data[idx];
                let a   = color.w * w;
                self.data[idx] = Vec4::new(
                    lerp_f(old.x, color.x, a),
                    lerp_f(old.y, color.y, a),
                    lerp_f(old.z, color.z, a),
                    lerp_f(old.w, 1.0, a),
                );
            }
        }
    }

    pub fn sample_bilinear(&self, u: f32, v: f32) -> Vec4 {
        let px = u * (self.width  - 1) as f32;
        let py = v * (self.height - 1) as f32;
        let x0 = px.floor() as usize;
        let y0 = py.floor() as usize;
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = px - x0 as f32;
        let ty = py - y0 as f32;
        fn l4(a: Vec4, b: Vec4, t: f32) -> Vec4 { a + (b - a) * t }
        let a = self.data[y0 * self.width + x0];
        let b = self.data[y0 * self.width + x1];
        let c = self.data[y1 * self.width + x0];
        let d = self.data[y1 * self.width + x1];
        l4(l4(a, b, tx), l4(c, d, tx), ty)
    }
}

// ============================================================
//  DECAL SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct Decal {
    pub id:         u32,
    pub position:   Vec3,
    pub rotation_y: f32,
    pub size:       Vec2,
    pub texture_id: u32,
    pub alpha:      f32,
    pub tint:       Vec4,
}

impl Decal {
    pub fn transform(&self) -> Mat4 {
        Mat4::from_translation(self.position)
        * Mat4::from_rotation_y(self.rotation_y)
        * Mat4::from_scale(Vec3::new(self.size.x, 1.0, self.size.y))
    }
    pub fn world_aabb(&self) -> (Vec3, Vec3) {
        let hs = Vec3::new(self.size.x * 0.5, 1.0, self.size.y * 0.5);
        (self.position - hs, self.position + hs)
    }
}

pub struct DecalLayer {
    pub decals:  Vec<Decal>,
    pub next_id: u32,
}

impl DecalLayer {
    pub fn new() -> Self { DecalLayer { decals: Vec::new(), next_id: 0 } }
    pub fn add(&mut self, position: Vec3, rotation_y: f32, size: Vec2, texture_id: u32) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.decals.push(Decal { id, position, rotation_y, size, texture_id, alpha: 1.0, tint: Vec4::ONE });
        id
    }
    pub fn remove(&mut self, id: u32) { self.decals.retain(|d| d.id != id); }
    pub fn query_sphere(&self, center: Vec3, radius: f32) -> Vec<&Decal> {
        self.decals.iter().filter(|d| (d.position - center).length() <= radius + d.size.x.max(d.size.y)).collect()
    }
}

// ============================================================
//  TERRAIN FEATURE DETECTION
// ============================================================

pub fn find_peaks(hmap: &Heightmap, min_height: f32, search_radius: usize) -> Vec<(usize, usize, f32)> {
    let w = hmap.width;
    let h = hmap.height;
    let r = search_radius as i32;
    let mut peaks = Vec::new();
    for y in r as usize..h - r as usize {
        for x in r as usize..w - r as usize {
            let ch = hmap.get(x, y);
            if ch < min_height { continue; }
            let mut is_max = true;
            'chk: for dy in -r..=r { for dx in -r..=r {
                if dx == 0 && dy == 0 { continue; }
                if hmap.get_clamped(x as i32 + dx, y as i32 + dy) > ch { is_max = false; break 'chk; }
            }}
            if is_max { peaks.push((x, y, ch)); }
        }
    }
    peaks
}

pub fn find_cliffs(hmap: &Heightmap, cliff_threshold: f32) -> Vec<(usize, usize)> {
    let w = hmap.width;
    let h = hmap.height;
    let mut cliffs = Vec::new();
    for y in 1..h-1 { for x in 1..w-1 {
        let center = hmap.get(x, y);
        let max_diff = [hmap.get(x+1,y), hmap.get(x-1,y), hmap.get(x,y+1), hmap.get(x,y-1)]
            .iter().map(|&n| (center - n).abs()).fold(0.0f32, f32::max);
        if max_diff >= cliff_threshold { cliffs.push((x, y)); }
    }}
    cliffs
}

// ============================================================
//  LAYER STACK (non-destructive editing)
// ============================================================

#[derive(Clone, Debug)]
pub enum LayerOperation { Add, Multiply, Subtract, Max, Min, Blend(f32) }

#[derive(Clone, Debug)]
pub struct TerrainLayerStack {
    pub base:   Vec<f32>,
    pub layers: Vec<(Vec<f32>, LayerOperation, f32)>,
    pub width:  usize,
    pub height: usize,
}

impl TerrainLayerStack {
    pub fn new(width: usize, height: usize) -> Self {
        TerrainLayerStack { base: vec![0.0; width*height], layers: Vec::new(), width, height }
    }
    pub fn push_layer(&mut self, data: Vec<f32>, op: LayerOperation, strength: f32) {
        self.layers.push((data, op, strength));
    }
    pub fn flatten(&self) -> Vec<f32> {
        let mut result = self.base.clone();
        let n = result.len();
        for (layer_data, op, strength) in &self.layers {
            for i in 0..n.min(layer_data.len()) {
                result[i] = match op {
                    LayerOperation::Add      => (result[i] + layer_data[i] * strength).clamp(0.0, 1.0),
                    LayerOperation::Multiply => result[i] * (1.0 + (layer_data[i] - 0.5) * strength * 2.0),
                    LayerOperation::Subtract => (result[i] - layer_data[i] * strength).clamp(0.0, 1.0),
                    LayerOperation::Max      => result[i].max(layer_data[i]),
                    LayerOperation::Min      => result[i].min(layer_data[i]),
                    LayerOperation::Blend(t) => lerp_f(result[i], layer_data[i], *t * strength),
                };
            }
        }
        result
    }
}



// ============================================================
//  WORLD EDITOR EXTENDED METHODS
// ============================================================

impl WorldEditor {
    pub fn apply_island_mask(&mut self, falloff_exp: f32) {
        let mask = generate_island_mask(self.heightmap.width, self.heightmap.height, falloff_exp);
        apply_mask(&mut self.heightmap, &mask);
        self.heightmap_dirty = true;
    }

    pub fn compute_snow_map(&self, snow_line: f32, temp_thresh: f32) -> Vec<f32> {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        let mut snow = vec![0.0f32; w * h];
        for y in 0..h { for x in 0..w {
            let idx  = y * w + x;
            let alt  = self.heightmap.get(x, y);
            let temp = if idx < self.temperature_map.len() { self.temperature_map[idx] } else { 15.0 };
            if alt >= snow_line && temp <= temp_thresh {
                let af = ((alt - snow_line) / (1.0 - snow_line)).clamp(0.0, 1.0);
                let tf = ((temp_thresh - temp) / 30.0).clamp(0.0, 1.0);
                snow[idx] = (af * 0.6 + tf * 0.4).clamp(0.0, 1.0);
            }
        }}
        snow
    }

    pub fn build_chunk_mesh(&self, chunk_x: usize, chunk_z: usize, chunk_size: usize, lod_step: usize) -> TerrainMesh {
        generate_terrain_mesh(&self.heightmap, chunk_x, chunk_z, chunk_size, self.cell_size, self.height_scale, lod_step)
    }

    pub fn compute_ao(&self, num_rays: usize, max_dist: f32) -> Vec<f32> {
        compute_terrain_ao(&self.heightmap, num_rays, max_dist, self.cell_size)
    }

    pub fn compute_shadow_map(&self) -> Vec<f32> {
        compute_terrain_shadow_map(&self.heightmap, self.solar.direction, self.cell_size, self.height_scale)
    }

    pub fn render_minimap(&self, out_w: usize, out_h: usize) -> Vec<u8> {
        let ramp = ColorRamp::terrain_default();
        let w    = self.heightmap.width;
        let h    = self.heightmap.height;
        let mut bytes = Vec::with_capacity(out_w * out_h * 4);
        for my in 0..out_h {
            for mx in 0..out_w {
                let u   = mx as f32 / out_w as f32;
                let v   = my as f32 / out_h as f32;
                let ht  = self.heightmap.sample_bilinear(u, v);
                let col = ramp.sample(ht);
                bytes.push((col.x * 255.0) as u8);
                bytes.push((col.y * 255.0) as u8);
                bytes.push((col.z * 255.0) as u8);
                bytes.push(255u8);
            }
        }
        bytes
    }

    pub fn find_peaks(&self, min_height: f32, search_radius: usize) -> Vec<(usize, usize, f32)> {
        find_peaks(&self.heightmap, min_height, search_radius)
    }

    pub fn generate_wind_field(&self, turbulence: f32) -> WindField {
        WindField::generate_from_terrain(&self.heightmap, self.weather.wind_vector(), turbulence, self.master_seed ^ 0xABCD0001)
    }

    pub fn stamp_hill(&mut self, world_x: f32, world_z: f32, radius: f32, height: f32, sharpness: f32) {
        let cx  = world_x / self.cell_size;
        let cz  = world_z / self.cell_size;
        let r   = (radius / self.cell_size) as usize;
        let w   = self.heightmap.width;
        let h   = self.heightmap.height;
        let x0  = ((cx as usize).saturating_sub(r)).min(w.saturating_sub(1));
        let y0  = ((cz as usize).saturating_sub(r)).min(h.saturating_sub(1));
        let x1  = ((cx as usize + r + 1)).min(w);
        let y1  = ((cz as usize + r + 1)).min(h);
        let rw  = x1 - x0;
        let rh  = y1 - y0;
        let mut stamp = vec![0.0f32; rw * rh];
        for ry in 0..rh { for rx in 0..rw {
            let dx   = (x0 + rx) as f32 - cx;
            let dz   = (y0 + ry) as f32 - cz;
            let dist = (dx*dx + dz*dz).sqrt() * self.cell_size;
            stamp[ry * rw + rx] = (1.0 - (dist / radius).clamp(0.0, 1.0).powf(sharpness)).max(0.0) * height / self.height_scale;
        }}
        let action = terrain_stamp(&mut self.heightmap, cx, cz, &stamp, rw, rh, 1.0);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }


    pub fn world_report(&self) -> String {
        let s = &self.stats;
        format!(
            "World '{}' {}x{} @ {:.1}m | H: {:.3}..{:.3} | Sea: {:.3}\n\
             Sun: alt={:.1}° az={:.1}° ({}) | DoY: {} Time: {:.1}h\n\
             Weather: {} Temp: {:.1}°C Wind: {:.1}m/s@{:.0}° Cloud: {:.0}%\n\
             Content: {} foliage | {} rivers | {} lakes | {} roads ({:.0}m)",
            self.world_name,
            self.heightmap.width, self.heightmap.height,
            self.cell_size,
            s.min_height, s.max_height, self.sea_level,
            self.solar.altitude_deg, self.solar.azimuth_deg,
            if self.solar.is_day { "day" } else { "night" },
            self.day_of_year as usize, self.utc_hour,
            self.weather.current.state.name(),
            self.weather.current.temperature_c,
            self.weather.current.wind_speed_ms,
            self.weather.current.wind_dir_deg,
            self.weather.current.cloud_cover * 100.0,
            s.foliage_count, s.river_count, s.lake_count,
            s.road_segments, s.road_total_length,
        )
    }

    pub fn validate(&self) -> Vec<String> {
        let mut w = Vec::new();
        if self.heightmap.width  < 16 { w.push("Heightmap width very small".into()); }
        if self.heightmap.height < 16 { w.push("Heightmap height very small".into()); }
        if self.sea_level > 0.9  { w.push("Sea level very high".into()); }
        if self.foliage.len() > 500_000 { w.push("Very large foliage count".into()); }
        for (i, lake) in self.lakes.iter().enumerate() {
            if lake.water_level < self.sea_level { w.push(format!("Lake {} below sea level", i)); }
        }
        w
    }

    pub fn is_navigable(&self, x: usize, y: usize, max_slope_deg: f32) -> bool {
        let alt   = self.heightmap.get(x, y);
        if alt <= self.sea_level { return false; }
        self.heightmap.slope_at(x, y, self.cell_size) * RAD2DEG <= max_slope_deg
    }

    pub fn export_nav_passability(&self, max_slope_deg: f32) -> Vec<bool> {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        (0..h).flat_map(|y| (0..w).map(move |x| self.is_navigable(x, y, max_slope_deg))).collect()
    }

    pub fn day_length_hours(&self) -> f64 {
        match sunrise_sunset(self.latitude, self.longitude, self.day_of_year) {
            Some((rise, set)) => set - rise,
            None => if self.solar.altitude_deg > 0.0 { 24.0 } else { 0.0 },
        }
    }

    pub fn rebuild_all(&mut self) {
        self.generate_climate();
        self.compute_stats();
        self.update_solar();
        self.heightmap_dirty = false;
        self.climate_dirty   = false;
    }

    pub fn sample_sky_color(&self, view_dir: Vec3) -> Vec3 {
        let clear = self.sky_at(view_dir);
        let cloud = Vec3::new(0.8, 0.85, 0.9);
        clear.lerp(cloud, self.weather.current.cloud_cover)
    }


    pub fn compute_viewshed(&self, obs_x: usize, obs_z: usize, obs_height: f32, max_dist: f32) -> Vec<bool> {
        let w     = self.heightmap.width;
        let h     = self.heightmap.height;
        let obs_h = self.heightmap.get(obs_x, obs_z) * self.height_scale + obs_height;
        let mut visible = vec![false; w * h];
        for tz in 0..h { for tx in 0..w {
            let dc = (((tx as i32 - obs_x as i32).pow(2) + (tz as i32 - obs_z as i32).pow(2)) as f32).sqrt();
            if dc * self.cell_size > max_dist { continue; }
            let steps = dc.ceil() as usize;
            if steps == 0 { visible[tz*w+tx] = true; continue; }
            let tgt_h = self.heightmap.get(tx.min(w-1), tz.min(h-1)) * self.height_scale;
            let mut los = true;
            for s in 1..steps {
                let t    = s as f32 / steps as f32;
                let lx   = obs_x as f32 + (tx as f32 - obs_x as f32) * t;
                let lz   = obs_z as f32 + (tz as f32 - obs_z as f32) * t;
                let ux   = (lx / w as f32).clamp(0.0, 1.0);
                let uz   = (lz / h as f32).clamp(0.0, 1.0);
                let th   = self.heightmap.sample_bilinear(ux, uz) * self.height_scale;
                let los_h = obs_h + (tgt_h - obs_h) * t;
                if th > los_h { los = false; break; }
            }
            visible[tz*w+tx] = los;
        }}
        visible
    }
}


// ============================================================
//  HALTON & FIBONACCI SAMPLING
// ============================================================

pub fn halton(index: usize, base: usize) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    let mut i = index;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

pub fn halton_2d(count: usize) -> Vec<Vec2> {
    (0..count).map(|i| Vec2::new(halton(i+1, 2), halton(i+1, 3))).collect()
}

pub fn fibonacci_sphere_points(n: usize) -> Vec<Vec3> {
    let gr = (1.0 + 5.0_f32.sqrt()) * 0.5;
    (0..n).map(|i| {
        let theta = (1.0 - 2.0 * i as f32 / (n as f32 - 1.0)).acos();
        let phi   = TWO_PI * i as f32 / gr;
        Vec3::new(theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos())
    }).collect()
}


pub const BEAUFORT_NAMES: [&str; 13] = [
    "Calm","Light air","Light breeze","Gentle breeze","Moderate breeze",
    "Fresh breeze","Strong breeze","Near gale","Gale","Strong gale",
    "Storm","Violent storm","Hurricane",
];

pub fn classify_cloud(coverage: f32, altitude_km: f32) -> &'static str {
    if coverage < 0.1 { return "Clear"; }
    if altitude_km < 2.0 { if coverage > 0.7 { "Stratus" } else { "Stratocumulus" } }
    else if altitude_km < 6.0 { if coverage > 0.6 { "Altostratus" } else { "Altocumulus" } }
    else { if coverage > 0.5 { "Cirrostratus" } else { "Cirrus" } }
}

pub fn pressure_tendency(history: &VecDeque<WeatherSnapshot>) -> &'static str {
    if history.len() < 3 { return "Steady"; }
    let v: Vec<f32> = history.iter().rev().take(3).map(|s| s.pressure_hpa).collect();
    let trend = v[0] - v[2];
    if trend > 1.5 { "Rising rapidly" } else if trend > 0.5 { "Rising" }
    else if trend < -1.5 { "Falling rapidly" } else if trend < -0.5 { "Falling" }
    else { "Steady" }
}

// ============================================================
//  BIOME WEIGHT MAP GENERATION
// ============================================================

pub fn compute_biome_weight_map(
    hmap:      &Heightmap,
    temp_map:  &[f32],
    hum_map:   &[f32],
    biome_sys: &BiomeSystem,
) -> Vec<[f32; 25]> {
    let w = hmap.width;
    let h = hmap.height;
    let mut wmap = vec![[0.0f32; 25]; w * h];
    for y in 0..h { for x in 0..w {
        let idx  = y * w + x;
        let alt  = hmap.get(x, y);
        let temp = if idx < temp_map.len() { temp_map[idx] } else { 15.0 };
        let hum  = if idx < hum_map.len()  { hum_map[idx]  } else { 0.5  };
        wmap[idx] = biome_sys.sample(temp, hum, alt).weights;
    }}
    wmap
}

pub fn build_biome_id_map(weight_map: &[[f32; 25]]) -> Vec<u8> {
    weight_map.iter().map(|ws|
        ws.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as u8).unwrap_or(0)
    ).collect()
}


// ============================================================
//  SHALLOW WATER EQUATIONS (simplified)
// ============================================================

#[derive(Clone, Debug)]
pub struct ShallowWaterSim {
    pub width:    usize,
    pub height:   usize,
    pub height_h: Vec<f32>,
    pub vel_x:    Vec<f32>,
    pub vel_z:    Vec<f32>,
    pub depth:    Vec<f32>,
    pub cell_size: f32,
    pub gravity:  f32,
    pub friction: f32,
}

impl ShallowWaterSim {
    pub fn new(width: usize, height: usize, cell_size: f32, gravity: f32) -> Self {
        let n = width * height;
        ShallowWaterSim { width, height, height_h: vec![0.0;n], vel_x: vec![0.0;n], vel_z: vec![0.0;n], depth: vec![0.0;n], cell_size, gravity, friction: 0.99 }
    }

    pub fn init_from_heightmap(&mut self, terrain: &Heightmap, sea_level: f32, height_scale: f32) {
        for y in 0..self.height { for x in 0..self.width {
            let th = terrain.get(x.min(terrain.width-1), y.min(terrain.height-1)) * height_scale;
            let wh = sea_level * height_scale;
            let idx = y * self.width + x;
            self.height_h[idx] = wh;
            self.depth[idx]    = (wh - th).max(0.0);
        }}
    }

    pub fn step(&mut self, terrain: &Heightmap, height_scale: f32, dt: f32) {
        let w = self.width; let h = self.height;
        let g = self.gravity; let cs = self.cell_size;
        let hh = self.height_h.clone();
        for y in 1..h-1 { for x in 1..w-1 {
            let idx = y*w+x;
            let depth = self.depth[idx];
            if depth < 0.001 { continue; }
            let dhdx = (hh[y*w+x+1] - hh[y*w+x-1]) / (2.0*cs);
            let dhdz = (hh[(y+1)*w+x] - hh[(y-1)*w+x]) / (2.0*cs);
            self.vel_x[idx] = (self.vel_x[idx] - g*dhdx*dt) * self.friction;
            self.vel_z[idx] = (self.vel_z[idx] - g*dhdz*dt) * self.friction;
        }}
        let vx = self.vel_x.clone();
        let vz = self.vel_z.clone();
        for y in 1..h-1 { for x in 1..w-1 {
            let idx   = y*w+x;
            let depth = self.depth[idx];
            if depth < 0.001 { continue; }
            let fx = vx[idx] * depth * dt / cs;
            let fz = vz[idx] * depth * dt / cs;
            let nx = (x as i32 + fx.signum() as i32).clamp(0, w as i32 - 1) as usize;
            let nz = (y as i32 + fz.signum() as i32).clamp(0, h as i32 - 1) as usize;
            let tx_a = fx.abs().min(depth * 0.5);
            let tz_a = fz.abs().min(depth * 0.5);
            self.height_h[idx]    -= tx_a + tz_a;
            self.height_h[nz*w+x] += tz_a;
            self.height_h[y*w+nx] += tx_a;
            let th = terrain.get(x.min(terrain.width-1), y.min(terrain.height-1)) * height_scale;
            self.depth[idx] = (self.height_h[idx] - th).max(0.0);
        }}
    }
}

// ============================================================
//  PROCEDURAL TEXTURE HELPERS
// ============================================================

pub fn gen_rock_texture(width: usize, height: usize, seed: u64) -> Vec<f32> {
    let params = FbmParams { octaves: 6, frequency: 4.0, lacunarity: 2.1, gain: 0.55, amplitude: 1.0, offset: 1.0, ridge: true };
    let off    = (seed as f32 * 1e-5, seed as f32 * 1e-5 + 50.0);
    let mut out = vec![0.0f32; width * height];
    for y in 0..height { for x in 0..width {
        out[y * width + x] = (fbm_2d(x as f32 / width as f32 + off.0, y as f32 / height as f32 + off.1, &params) * 0.5 + 0.5).clamp(0.0, 1.0);
    }}
    out
}

pub fn gen_soil_texture(width: usize, height: usize, seed: u64) -> Vec<f32> {
    let base_p  = FbmParams { octaves: 4, frequency: 8.0, lacunarity: 2.0, gain: 0.5, amplitude: 1.0, offset: 0.0, ridge: false };
    let crack_p = FbmParams { octaves: 3, frequency: 12.0, lacunarity: 2.5, gain: 0.4, amplitude: 0.5, offset: 0.0, ridge: false };
    let off     = (seed as f32 * 1e-5 + 100.0, seed as f32 * 1e-5 + 200.0);
    let mut out = vec![0.0f32; width * height];
    for y in 0..height { for x in 0..width {
        let nx = x as f32 / width as f32 + off.0;
        let ny = y as f32 / height as f32 + off.1;
        let b  = fbm_2d(nx,       ny,       &base_p)  * 0.5 + 0.5;
        let cr = fbm_2d(nx * 0.5, ny * 0.5, &crack_p).abs();
        out[y * width + x] = (b * 0.7 + cr * 0.3).clamp(0.0, 1.0);
    }}
    out
}


// ============================================================
//  TERRAIN LAYER PAINTER
// ============================================================

#[derive(Clone, Debug)]
pub struct TerrainPaintLayer {
    pub id:         usize,
    pub name:       String,
    pub weight_map: Vec<f32>,
    pub tiling:     f32,
    pub normal_strength: f32,
}

impl TerrainPaintLayer {
    pub fn new(id: usize, name: &str, width: usize, height: usize) -> Self {
        TerrainPaintLayer { id, name: name.into(), weight_map: vec![0.0; width*height], tiling: 10.0, normal_strength: 1.0 }
    }

    pub fn paint(&mut self, cx: f32, cy: f32, brush: &BrushSettings, width: usize, height: usize) {
        let r  = brush.radius.ceil() as i32;
        let x0 = ((cx as i32 - r).max(0)) as usize;
        let y0 = ((cy as i32 - r).max(0)) as usize;
        let x1 = ((cx as i32 + r).min(width  as i32 - 1)) as usize;
        let y1 = ((cy as i32 + r).min(height as i32 - 1)) as usize;
        for y in y0..=y1 { for x in x0..=x1 {
            let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            if dist > brush.radius { continue; }
            let idx = y * width + x;
            self.weight_map[idx] = (self.weight_map[idx] + brush.weight_at_radius(dist)).clamp(0.0, 1.0);
        }}
    }
}

// ============================================================
//  ATMOSPHERIC HAZE
// ============================================================

#[derive(Clone, Debug)]
pub struct AtmosphericHaze {
    pub density:        f32,
    pub haze_color:     Vec3,
    pub fog_start:      f32,
    pub fog_end:        f32,
    pub height_falloff: f32,
}

impl AtmosphericHaze {
    pub fn new(density: f32, color: Vec3) -> Self {
        AtmosphericHaze { density, haze_color: color, fog_start: 100.0, fog_end: 5000.0, height_falloff: 0.002 }
    }
    pub fn fog_factor(&self, distance: f32, height: f32) -> f32 {
        (smoothstep(self.fog_start, self.fog_end, distance) * (-(height * self.height_falloff)).exp() * self.density).clamp(0.0, 1.0)
    }
    pub fn apply(&self, color: Vec3, distance: f32, height: f32) -> Vec3 {
        color.lerp(self.haze_color, self.fog_factor(distance, height))
    }
}


// ============================================================
//  FIND EROSION SOURCES
// ============================================================

pub fn find_erosion_sources(hmap: &Heightmap, count: usize, min_altitude: f32, min_slope: f32, seed: u64) -> Vec<Vec2> {
    let w = hmap.width;
    let h = hmap.height;
    let mut candidates: Vec<(f32, Vec2)> = Vec::new();
    for y in 0..h { for x in 0..w {
        let alt   = hmap.get(x, y);
        let slope = hmap.slope_at(x, y, 1.0);
        if alt >= min_altitude && slope >= min_slope {
            candidates.push((alt * slope, Vec2::new(x as f32, y as f32)));
        }
    }}
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let min_spacing = ((w * h) as f32 / count as f32).sqrt() * 0.3;
    let mut result = Vec::new();
    for (_, pos) in candidates.iter() {
        if result.len() >= count { break; }
        if !result.iter().any(|p: &Vec2| (*p - *pos).length() < min_spacing) {
            result.push(*pos);
        }
    }
    result
}

// ============================================================
//  WARP HEIGHTMAP
// ============================================================

pub fn warp_heightmap(hmap: &Heightmap, disp_x: &[f32], disp_z: &[f32], strength: f32) -> Heightmap {
    let w = hmap.width;
    let h = hmap.height;
    let mut out = Heightmap::new(w, h);
    for y in 0..h { for x in 0..w {
        let idx  = y * w + x;
        let dx   = if idx < disp_x.len() { disp_x[idx] * strength } else { 0.0 };
        let dz   = if idx < disp_z.len() { disp_z[idx] * strength } else { 0.0 };
        let src_x = (x as f32 + dx * w as f32).clamp(0.0, (w-1) as f32);
        let src_z = (y as f32 + dz * h as f32).clamp(0.0, (h-1) as f32);
        out.set(x, y, hmap.sample_bilinear(src_x / (w-1) as f32, src_z / (h-1) as f32));
    }}
    out.recompute_minmax();
    out
}



#[derive(Clone, Debug)]
pub struct FlareElement { pub offset: f32, pub size: f32, pub color: Vec4, pub texture_id: u32 }

#[derive(Clone, Debug)]
pub struct LensFlare {
    pub elements:     Vec<FlareElement>,
    pub intensity:    f32,
    pub streak_count: u8,
    pub streak_size:  f32,
}

impl LensFlare {
    pub fn sun_flare() -> Self {
        LensFlare {
            elements: vec![
                FlareElement { offset: 0.0, size: 0.15, color: Vec4::new(1.0, 0.9, 0.7, 0.8), texture_id: 0 },
                FlareElement { offset: 0.2, size: 0.05, color: Vec4::new(0.8, 0.8, 1.0, 0.4), texture_id: 1 },
                FlareElement { offset: 0.5, size: 0.08, color: Vec4::new(1.0, 0.7, 0.3, 0.3), texture_id: 2 },
                FlareElement { offset: 0.8, size: 0.04, color: Vec4::new(0.7, 1.0, 0.7, 0.2), texture_id: 1 },
                FlareElement { offset: 1.2, size: 0.10, color: Vec4::new(0.6, 0.8, 1.0, 0.2), texture_id: 0 },
            ],
            intensity: 1.0, streak_count: 6, streak_size: 0.4,
        }
    }
    pub fn screen_positions<'a>(&'a self, sun_screen: Vec2, screen_center: Vec2) -> Vec<(Vec2, &'a FlareElement)> {
        let axis = screen_center - sun_screen;
        self.elements.iter().map(|e| (sun_screen + axis * e.offset, e)).collect()
    }
}


#[derive(Clone, Debug)]
pub struct NavCell {
    pub x:          usize,
    pub y:          usize,
    pub passable:   bool,
    pub cost:       f32,
    pub region_id:  u32,
}

#[derive(Clone, Debug)]
pub struct NavGrid {
    pub width:  usize,
    pub height: usize,
    pub cells:  Vec<NavCell>,
}

impl NavGrid {
    pub fn from_heightmap(hmap: &Heightmap, cell_size: f32, sea_level: f32, max_slope_deg: f32) -> Self {
        let w = hmap.width;
        let h = hmap.height;
        let cells: Vec<NavCell> = (0..h).flat_map(|y| (0..w).map(move |x| {
            let alt   = hmap.get(x, y);
            let slope = hmap.slope_at(x.min(w-1), y.min(h-1), cell_size) * RAD2DEG;
            let pass  = alt > sea_level && slope <= max_slope_deg;
            let cost  = 1.0 + slope / max_slope_deg;
            NavCell { x, y, passable: pass, cost, region_id: 0 }
        })).collect();
        NavGrid { width: w, height: h, cells }
    }

    pub fn get(&self, x: usize, y: usize) -> &NavCell {
        &self.cells[y * self.width + x]
    }

    pub fn label_regions(&mut self) {
        let w = self.width;
        let h = self.height;
        let mut region = 0u32;
        let mut visited = vec![false; w * h];

        for sy in 0..h {
            for sx in 0..w {
                if visited[sy * w + sx] || !self.cells[sy * w + sx].passable { continue; }
                region += 1;
                let mut queue = VecDeque::new();
                queue.push_back((sx, sy));
                visited[sy * w + sx] = true;
                while let Some((cx, cy)) = queue.pop_front() {
                    self.cells[cy * w + cx].region_id = region;
                    let neighbors: [(i32, i32); 4] = [(1,0),(-1,0),(0,1),(0,-1)];
                    for &(dx, dy) in &neighbors {
                        let nx = cx as i32 + dx;
                        let ny = cy as i32 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                        let ni = ny as usize * w + nx as usize;
                        if !visited[ni] && self.cells[ni].passable {
                            visited[ni] = true;
                            queue.push_back((nx as usize, ny as usize));
                        }
                    }
                }
            }
        }
    }

    pub fn region_count(&self) -> u32 {
        self.cells.iter().map(|c| c.region_id).max().unwrap_or(0)
    }

    pub fn largest_region_size(&self) -> usize {
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for c in &self.cells { if c.passable { *counts.entry(c.region_id).or_insert(0) += 1; } }
        counts.values().copied().max().unwrap_or(0)
    }
}

// ============================================================
//  HEIGHTMAP STATISTICAL ANALYSIS
// ============================================================

#[derive(Clone, Debug)]
pub struct HeightmapStats {
    pub min:    f32,
    pub max:    f32,
    pub mean:   f32,
    pub median: f32,
    pub stddev: f32,
    pub skewness: f32,
    pub percentile_25: f32,
    pub percentile_75: f32,
    pub histogram: Vec<u32>,  // 256 buckets
}

pub fn compute_heightmap_stats(hmap: &Heightmap) -> HeightmapStats {
    let n = hmap.data.len();
    if n == 0 {
        return HeightmapStats { min:0.0, max:0.0, mean:0.0, median:0.0, stddev:0.0,
            skewness:0.0, percentile_25:0.0, percentile_75:0.0, histogram: vec![0;256] };
    }

    let mut sorted = hmap.data.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min   = *sorted.first().unwrap();
    let max   = *sorted.last().unwrap();
    let sum: f64 = sorted.iter().map(|&v| v as f64).sum();
    let mean  = (sum / n as f64) as f32;
    let median = sorted[n / 2];
    let p25   = sorted[n / 4];
    let p75   = sorted[3 * n / 4];

    let variance: f64 = sorted.iter().map(|&v| { let d = v as f64 - mean as f64; d*d }).sum::<f64>() / n as f64;
    let stddev  = variance.sqrt() as f32;

    let skewness: f64 = if stddev > 1e-10 {
        sorted.iter().map(|&v| { let d = (v as f64 - mean as f64) / stddev as f64; d*d*d }).sum::<f64>() / n as f64
    } else { 0.0 };

    let range = max - min;
    let mut histogram = vec![0u32; 256];
    for &v in &hmap.data {
        if range > 1e-10 {
            let b = ((v - min) / range * 255.0).clamp(0.0, 255.0) as usize;
            histogram[b] += 1;
        }
    }

    HeightmapStats { min, max, mean, median, stddev, skewness: skewness as f32, percentile_25: p25, percentile_75: p75, histogram }
}

// ============================================================
//  EROSION PARAMETER PRESETS
// ============================================================

impl ErosionParams {
    pub fn preset_light() -> Self {
        ErosionParams { num_particles: 20_000, inertia: 0.03, capacity: 3.0,
            deposition: 0.4, erosion_speed: 0.2, evaporation: 0.025, min_slope: 0.005,
            gravity: 3.0, max_steps: 48, erosion_radius: 2.0, seed: 0xDEAD }
    }
    pub fn preset_heavy() -> Self {
        ErosionParams { num_particles: 150_000, inertia: 0.06, capacity: 6.0,
            deposition: 0.2, erosion_speed: 0.5, evaporation: 0.015, min_slope: 0.01,
            gravity: 5.0, max_steps: 80, erosion_radius: 4.0, seed: 0xBEEF }
    }
    pub fn preset_rivers() -> Self {
        ErosionParams { num_particles: 80_000, inertia: 0.08, capacity: 8.0,
            deposition: 0.1, erosion_speed: 0.8, evaporation: 0.01, min_slope: 0.02,
            gravity: 6.0, max_steps: 120, erosion_radius: 5.0, seed: 0xFACE }
    }
}

// ============================================================
//  FBM PRESET LIBRARY
// ============================================================

impl FbmParams {
    pub fn mountains() -> Self {
        FbmParams { octaves: 8, frequency: 1.0, lacunarity: 2.1, gain: 0.52, amplitude: 1.0, offset: 1.0, ridge: true }
    }
    pub fn plains() -> Self {
        FbmParams { octaves: 4, frequency: 0.5, lacunarity: 2.0, gain: 0.6, amplitude: 0.4, offset: 0.0, ridge: false }
    }
    pub fn hills() -> Self {
        FbmParams { octaves: 6, frequency: 1.5, lacunarity: 2.0, gain: 0.55, amplitude: 0.7, offset: 0.0, ridge: false }
    }
    pub fn canyon() -> Self {
        FbmParams { octaves: 5, frequency: 1.2, lacunarity: 2.3, gain: 0.45, amplitude: 1.0, offset: 0.8, ridge: true }
    }
    pub fn island() -> Self {
        FbmParams { octaves: 7, frequency: 1.0, lacunarity: 2.0, gain: 0.5, amplitude: 1.0, offset: 0.0, ridge: false }
    }
}

// ============================================================
//  INTERPOLATION UTILITIES
// ============================================================

/// Cubic hermite interpolation
#[inline]
pub fn cubic_hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a = -0.5*y0 + 1.5*y1 - 1.5*y2 + 0.5*y3;
    let b =  y0 - 2.5*y1 + 2.0*y2 - 0.5*y3;
    let c = -0.5*y0 + 0.5*y2;
    let d =  y1;
    ((a*t + b)*t + c)*t + d
}

/// Quintic interpolation (6th order smooth)
#[inline]
pub fn quintic_interp(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Spherical linear interpolation for quaternions (SLERP)
pub fn slerp(a: Quat, b: Quat, t: f32) -> Quat {
    let dot = a.dot(b).clamp(-1.0, 1.0);
    let b_adj = if dot < 0.0 { Quat::from_array([-b.x,-b.y,-b.z,-b.w]) } else { b };
    let dot_adj = dot.abs();
    if dot_adj > 0.9995 {
        return Quat::from_array([
            a.x + (b_adj.x - a.x) * t,
            a.y + (b_adj.y - a.y) * t,
            a.z + (b_adj.z - a.z) * t,
            a.w + (b_adj.w - a.w) * t,
        ]).normalize();
    }
    let theta_0 = dot_adj.acos();
    let theta    = theta_0 * t;
    let sin_t0   = theta_0.sin();
    let sin_t    = theta.sin();
    let s1 = (theta_0 - theta).sin() / sin_t0;
    let s2 = sin_t / sin_t0;
    Quat::from_array([
        a.x * s1 + b_adj.x * s2,
        a.y * s1 + b_adj.y * s2,
        a.z * s1 + b_adj.z * s2,
        a.w * s1 + b_adj.w * s2,
    ])
}

/// Inverse bilinear interpolation (find UV from world position in quad)
pub fn inverse_bilinear(p: Vec2, a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    // Solve the bilinear system: p = a*(1-u)*(1-v) + b*u*(1-v) + c*(1-u)*v + d*u*v
    let e = b - a;
    let f = c - a;
    let g = a - b - c + d;
    let h = p - a;

    // Quadratic in v
    let k2 = g.perp_dot(e);
    let k1 = e.perp_dot(h) + g.perp_dot(f); // sign correction from standard formulation
    let k0 = f.perp_dot(h);

    let (v, u);
    if k2.abs() < 1e-6 {
        if k1.abs() < 1e-6 { return None; }
        v = -k0 / k1;
        let denom = e.x + g.x * v;
        u = if denom.abs() > 1e-6 { (h.x - f.x * v) / denom } else { (h.y - f.y * v) / (e.y + g.y * v) };
    } else {
        let disc = k1 * k1 - 4.0 * k0 * k2;
        if disc < 0.0 { return None; }
        v = (-k1 - disc.sqrt()) / (2.0 * k2);
        let denom = e.x + g.x * v;
        u = if denom.abs() > 1e-6 { (h.x - f.x * v) / denom } else { (h.y - f.y * v) / (e.y + g.y * v) };
    }

    Some(Vec2::new(u, v))
}

// ============================================================
//  WORLD SEED CATALOG
// ============================================================

#[derive(Clone, Debug)]
pub struct WorldSeedPreset {
    pub name:    &'static str,
    pub seed:    u64,
    pub style:   WorldStyle,
    pub size:    usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldStyle {
    Continental,
    Island,
    Archipelago,
    Mountains,
    Desert,
    Tundra,
    Jungle,
    Mixed,
}

pub const WORLD_SEED_PRESETS: [WorldSeedPreset; 12] = [
    WorldSeedPreset { name: "Verdant Valley",   seed: 0x1A2B3C4D, style: WorldStyle::Continental, size: 512 },
    WorldSeedPreset { name: "Dragon's Peak",    seed: 0xDEAD1234, style: WorldStyle::Mountains,   size: 1024 },
    WorldSeedPreset { name: "Lost Atoll",       seed: 0x42424242, style: WorldStyle::Island,      size: 512 },
    WorldSeedPreset { name: "Frozen North",     seed: 0xCE000001, style: WorldStyle::Tundra,      size: 1024 },
    WorldSeedPreset { name: "Amber Waste",      seed: 0xDEAD5A1D, style: WorldStyle::Desert,      size: 512 },
    WorldSeedPreset { name: "Emerald Canopy",   seed: 0x74726545, style: WorldStyle::Jungle,      size: 1024 },
    WorldSeedPreset { name: "Shattered Isles",  seed: 0xB0CA5501, style: WorldStyle::Archipelago, size: 2048 },
    WorldSeedPreset { name: "Old Frontier",     seed: 0xF121E510, style: WorldStyle::Mixed,       size: 1024 },
    WorldSeedPreset { name: "Crystal Spires",   seed: 0xCCC00DDD, style: WorldStyle::Mountains,   size: 512 },
    WorldSeedPreset { name: "River Delta",      seed: 0xD37741AA, style: WorldStyle::Continental, size: 1024 },
    WorldSeedPreset { name: "Thunder Plains",   seed: 0xBADC0DE1, style: WorldStyle::Mixed,       size: 2048 },
    WorldSeedPreset { name: "Ancient Caldera",  seed: 0xCA1D3EA0, style: WorldStyle::Island,      size: 512 },
];

// ============================================================
//  HEIGHTMAP OPERATIONS — FILL SINKS
// ============================================================

/// Planchon-Darboux sink filling — fill all depressions for hydrological flow
pub fn fill_sinks(hmap: &mut Heightmap, epsilon: f32) {
    let w = hmap.width;
    let h = hmap.height;
    let big = 1e9f32;
    let mut wl = vec![big; w * h];

    // Initialize border cells
    for x in 0..w {
        wl[0 * w + x] = hmap.get(x, 0);
        wl[(h-1) * w + x] = hmap.get(x, h-1);
    }
    for y in 0..h {
        wl[y * w + 0] = hmap.get(0, y);
        wl[y * w + w-1] = hmap.get(w-1, y);
    }

    let dirs: [(i32, i32); 8] = [(1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)];

    // Iteratively lower water level
    let mut changed = true;
    let mut iter = 0;
    while changed && iter < 1000 {
        changed = false;
        iter += 1;
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx  = y * w + x;
                let hval = hmap.data[idx];
                let mut new_wl = wl[idx];
                for &(dx, dy) in &dirs {
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    let nidx = ny * w + nx;
                    let candidate = wl[nidx] + epsilon;
                    if hval >= candidate {
                        new_wl = new_wl.min(hval);
                    } else {
                        new_wl = new_wl.min(candidate);
                    }
                }
                if new_wl < wl[idx] {
                    wl[idx]  = new_wl;
                    changed  = true;
                }
            }
        }
    }

    // Apply water level as new terrain height
    for (i, v) in wl.iter().enumerate() {
        if *v < big * 0.5 {
            hmap.data[i] = hmap.data[i].max(*v);
        }
    }
    hmap.recompute_minmax();
}

// ============================================================
//  GRID UTILITIES
// ============================================================

/// Compute cell neighbors in a 2D grid (8-way or 4-way)
pub fn cell_neighbors_8(x: usize, y: usize, width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(8);
    let xi = x as i32;
    let yi = y as i32;
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if dx == 0 && dy == 0 { continue; }
            let nx = xi + dx;
            let ny = yi + dy;
            if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                result.push((nx as usize, ny as usize));
            }
        }
    }
    result
}

pub fn cell_neighbors_4(x: usize, y: usize, width: usize, height: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(4);
    let xi = x as i32;
    let yi = y as i32;
    for &(dx, dy) in &[(1i32,0i32),(-1,0),(0,1),(0,-1)] {
        let nx = xi + dx;
        let ny = yi + dy;
        if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
            result.push((nx as usize, ny as usize));
        }
    }
    result
}

/// Flood-fill a boolean map starting from a seed cell
pub fn flood_fill_bool(mask: &mut Vec<bool>, width: usize, height: usize, sx: usize, sy: usize, fill_value: bool) {
    let init = mask[sy * width + sx];
    if init == fill_value { return; }
    let mut queue = VecDeque::new();
    queue.push_back((sx, sy));
    mask[sy * width + sx] = fill_value;
    while let Some((cx, cy)) = queue.pop_front() {
        for (nx, ny) in cell_neighbors_4(cx, cy, width, height) {
            if mask[ny * width + nx] != fill_value {
                mask[ny * width + nx] = fill_value;
                queue.push_back((nx, ny));
            }
        }
    }
}

// ============================================================
//  WORLD GENERATION PROFILE
// ============================================================

#[derive(Clone, Debug)]
pub struct WorldGenProfile {
    pub name:              String,
    pub width:             usize,
    pub height:            usize,
    pub cell_size_m:       f32,
    pub height_scale_m:    f32,
    pub sea_level_frac:    f32,
    pub fbm_params:        FbmParams,
    pub warp_strength:     f32,
    pub erosion_params:    ErosionParams,
    pub thermal_iters:     usize,
    pub thermal_talus_deg: f32,
    pub num_rivers:        usize,
    pub num_lakes:         usize,
    pub foliage_density:   f32,
    pub apply_island_mask: bool,
    pub island_falloff:    f32,
    pub fill_sinks:        bool,
    pub latitude:          f64,
    pub longitude:         f64,
    pub start_doy:         f64,
    pub start_utc:         f64,
}

impl WorldGenProfile {
    pub fn default_continental() -> Self {
        WorldGenProfile {
            name: "Continental".into(), width: 1024, height: 1024, cell_size_m: 1.0,
            height_scale_m: 500.0, sea_level_frac: 0.22, fbm_params: FbmParams::default_terrain(),
            warp_strength: 0.35, erosion_params: ErosionParams::default(), thermal_iters: 5,
            thermal_talus_deg: 32.0, num_rivers: 8, num_lakes: 5, foliage_density: 0.8,
            apply_island_mask: false, island_falloff: 2.0, fill_sinks: true,
            latitude: 45.0, longitude: 0.0, start_doy: 180.0, start_utc: 12.0,
        }
    }

    pub fn default_island() -> Self {
        WorldGenProfile {
            name: "Island".into(), width: 512, height: 512, cell_size_m: 1.0,
            height_scale_m: 300.0, sea_level_frac: 0.28, fbm_params: FbmParams::island(),
            warp_strength: 0.4, erosion_params: ErosionParams::preset_light(), thermal_iters: 3,
            thermal_talus_deg: 28.0, num_rivers: 4, num_lakes: 2, foliage_density: 1.0,
            apply_island_mask: true, island_falloff: 2.5, fill_sinks: false,
            latitude: 10.0, longitude: -30.0, start_doy: 80.0, start_utc: 10.0,
        }
    }

    pub fn default_mountains() -> Self {
        WorldGenProfile {
            name: "Mountains".into(), width: 1024, height: 1024, cell_size_m: 1.0,
            height_scale_m: 1000.0, sea_level_frac: 0.12, fbm_params: FbmParams::mountains(),
            warp_strength: 0.2, erosion_params: ErosionParams::preset_heavy(), thermal_iters: 10,
            thermal_talus_deg: 40.0, num_rivers: 12, num_lakes: 6, foliage_density: 0.4,
            apply_island_mask: false, island_falloff: 2.0, fill_sinks: true,
            latitude: 55.0, longitude: 10.0, start_doy: 240.0, start_utc: 8.0,
        }
    }
}

impl WorldEditor {
    /// Apply a complete WorldGenProfile — full procedural pipeline
    pub fn apply_profile(&mut self, profile: &WorldGenProfile, seed: u64) {
        // Resize if needed
        if self.heightmap.width != profile.width || self.heightmap.height != profile.height {
            self.resize_world(profile.width, profile.height);
        }

        self.cell_size      = profile.cell_size_m;
        self.height_scale   = profile.height_scale_m;
        self.sea_level      = profile.sea_level_frac;
        self.latitude       = profile.latitude;
        self.longitude      = profile.longitude;
        self.day_of_year    = profile.start_doy;
        self.utc_hour       = profile.start_utc;
        self.master_seed    = seed;
        self.world_name     = profile.name.clone();
        self.terrain_fbm_params = profile.fbm_params.clone();
        self.warp_strength  = profile.warp_strength;
        self.erosion_params = profile.erosion_params.clone();

        // Generate terrain
        self.generate_terrain();

        // Island mask
        if profile.apply_island_mask {
            self.apply_island_mask(profile.island_falloff);
        }

        // Fill sinks
        if profile.fill_sinks {
            fill_sinks(&mut self.heightmap, 0.0001);
        }

        // Erosion
        self.apply_erosion();
        self.apply_thermal_erosion(profile.thermal_iters, profile.thermal_talus_deg);

        // Climate
        self.generate_climate();

        // Water
        self.generate_rivers(profile.num_rivers);
        self.generate_lakes(profile.num_lakes, 0.025);
        self.generate_shore();

        // Foliage
        let density = profile.foliage_density;
        self.place_foliage_layer(FoliagePlacementParams {
            min_radius: 2.0, max_instances: (density * 100_000.0) as usize,
            max_slope_rad: 0.65, min_altitude: 0.05, max_altitude: 0.75,
            density_scale: density, use_density_map: false, random_rotation: true,
            scale_variance: 0.3, base_scale: Vec3::ONE, asset_id: 0, biome_id: 0,
            align_to_normal: false,
        }, seed ^ 0xF01_1A6E);

        // Solar
        self.update_solar();
        self.compute_stats();
        self.heightmap_dirty = false;
        self.climate_dirty   = false;
    }
}

// ============================================================
//  ADDITIONAL TERRAIN STAMP SHAPES
// ============================================================

/// Generate a volcano stamp (ring with caldera depression)
pub fn volcano_stamp(size: usize, rim_radius: f32, rim_height: f32, caldera_depth: f32) -> Vec<f32> {
    let center = size as f32 * 0.5;
    let mut stamp = vec![0.0f32; size * size];
    for y in 0..size {
        for x in 0..size {
            let dx   = x as f32 - center;
            let dy   = y as f32 - center;
            let dist = (dx*dx + dy*dy).sqrt() / (size as f32 * 0.5);
            let t    = dist / rim_radius;
            let h = if t < 0.6 {
                // caldera
                rim_height - caldera_depth + caldera_depth * smoothstep(0.0, 0.6, t)
            } else if t <= 1.0 {
                // rim
                let rt = (t - 0.6) / 0.4;
                rim_height * (1.0 - smoothstep(0.0, 1.0, rt))
            } else {
                // outer slope
                rim_height * (1.0 - smoothstep(1.0, 2.0, t)).max(0.0)
            };
            stamp[y * size + x] = h.max(0.0);
        }
    }
    stamp
}

/// Generate a mesa stamp (flat top, steep sides)
pub fn mesa_stamp(size: usize, top_radius: f32, cliff_steepness: f32, height: f32) -> Vec<f32> {
    let center = size as f32 * 0.5;
    let mut stamp = vec![0.0f32; size * size];
    for y in 0..size {
        for x in 0..size {
            let dx   = x as f32 - center;
            let dy   = y as f32 - center;
            let dist = (dx*dx + dy*dy).sqrt() / (size as f32 * 0.5);
            let h = if dist <= top_radius {
                height
            } else {
                let edge_dist = (dist - top_radius) / (1.0 - top_radius);
                height * (1.0 - edge_dist.powf(cliff_steepness)).max(0.0)
            };
            stamp[y * size + x] = h.max(0.0);
        }
    }
    stamp
}

/// Generate a crater stamp (impact crater — raised rim, depressed interior)
pub fn crater_stamp(size: usize, crater_radius: f32, rim_height: f32, depth: f32) -> Vec<f32> {
    let center = size as f32 * 0.5;
    let mut stamp = vec![0.0f32; size * size];
    for y in 0..size {
        for x in 0..size {
            let dx   = x as f32 - center;
            let dy   = y as f32 - center;
            let dist = (dx*dx + dy*dy).sqrt() / (size as f32 * 0.5);
            let t    = dist / crater_radius;
            let h = if t < 0.8 {
                // interior floor — depressed
                -depth * (1.0 - smoothstep(0.6, 0.8, t))
            } else if t <= 1.0 {
                // rim
                let rt = (t - 0.8) / 0.2;
                rim_height * (1.0 - (rt * 2.0 - 1.0).powi(2))
            } else {
                // outer falloff
                rim_height * (1.0 - smoothstep(1.0, 1.5, t)).max(0.0)
            };
            stamp[y * size + x] = h;
        }
    }
    stamp
}

// ============================================================
//  VORONOI REGION BUILDER
// ============================================================

#[derive(Clone, Debug)]
pub struct VoronoiCell {
    pub site:    Vec2,
    pub id:      u32,
    pub biome:   BiomeId,
    pub area:    f32,
    pub members: Vec<(usize, usize)>,
}

pub struct VoronoiMap {
    pub width:  usize,
    pub height: usize,
    pub cell_id: Vec<u32>,
    pub cells:  Vec<VoronoiCell>,
}

impl VoronoiMap {
    pub fn generate(width: usize, height: usize, num_sites: usize, seed: u64) -> Self {
        let mut rng   = LcgRng::new(seed);
        let sites: Vec<Vec2> = (0..num_sites).map(|_| {
            Vec2::new(rng.next_f32() * width as f32, rng.next_f32() * height as f32)
        }).collect();

        let mut cell_id = vec![0u32; width * height];
        let mut cells: Vec<VoronoiCell> = (0..num_sites).map(|i| VoronoiCell {
            site: sites[i],
            id: i as u32,
            biome: BiomeId::TemperateGrassland,
            area: 0.0,
            members: Vec::new(),
        }).collect();

        for y in 0..height {
            for x in 0..width {
                let p = Vec2::new(x as f32, y as f32);
                let best_idx = sites.iter().enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let da = (**a - p).length_squared();
                        let db = (**b - p).length_squared();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                cell_id[y * width + x] = best_idx as u32;
                cells[best_idx].members.push((x, y));
                cells[best_idx].area += 1.0;
            }
        }

        VoronoiMap { width, height, cell_id, cells }
    }

    pub fn assign_biomes(&mut self, hmap: &Heightmap, temp_map: &[f32], hum_map: &[f32]) {
        let w = hmap.width;
        for cell in self.cells.iter_mut() {
            let sx  = cell.site.x as usize;
            let sy  = cell.site.y as usize;
            let sx  = sx.min(hmap.width  - 1);
            let sy  = sy.min(hmap.height - 1);
            let idx = sy * w + sx;
            let alt  = hmap.get(sx, sy);
            let temp = if idx < temp_map.len() { temp_map[idx] } else { 15.0 };
            let hum  = if idx < hum_map.len()  { hum_map[idx]  } else { 0.5  };
            cell.biome = BiomeDescriptor::classify_point(temp, hum, alt);
        }
    }

    pub fn biome_at(&self, x: usize, y: usize) -> BiomeId {
        let cid = self.cell_id[y * self.width + x] as usize;
        if cid < self.cells.len() { self.cells[cid].biome } else { BiomeId::TemperateGrassland }
    }
}

// ============================================================
//  TEMPERATURE INVERSION (valley fog)
// ============================================================

/// Compute temperature inversion areas (valley bottoms colder than surroundings)
pub fn compute_temperature_inversion(
    hmap:     &Heightmap,
    temp_map: &[f32],
    acc_map:  &[u32],   // flow accumulation
    threshold: u32,     // flow accumulation threshold for valley detection
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut inv_map = vec![0.0f32; w * h];

    for y in 1..h-1 {
        for x in 1..w-1 {
            let idx = y * w + x;
            if acc_map[idx] < threshold { continue; }
            // This cell has high flow accumulation (valley)
            let alt = hmap.get(x, y);
            // Surrounding cells average height
            let mut sum_h = 0.0f32;
            let mut cnt   = 0;
            for &(dx, dy) in &[(2i32,0i32),(-2,0),(0,2),(0,-2)] {
                let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                sum_h += hmap.get(nx, ny);
                cnt   += 1;
            }
            let avg_h = sum_h / cnt as f32;
            let inv   = (avg_h - alt).max(0.0) * 20.0; // 20°C per unit height inversion
            inv_map[idx] = inv;
        }
    }
    inv_map
}

// ============================================================
//  EROSION SEDIMENT FLUX MAP
// ============================================================

/// Compute sediment flux (how much sediment passes through each cell) from flow acc
pub fn compute_sediment_flux(
    hmap:    &Heightmap,
    acc_map: &[u32],
    k:       f32,   // erodibility constant
    m:       f32,   // drainage area exponent (typically 0.5)
    n:       f32,   // slope exponent (typically 1.0)
    cell_size: f32,
) -> Vec<f32> {
    let w = hmap.width;
    let h = hmap.height;
    let mut flux = vec![0.0f32; w * h];
    for y in 1..h-1 {
        for x in 1..w-1 {
            let idx   = y * w + x;
            let slope = hmap.slope_at(x, y, cell_size);
            let area  = acc_map[idx] as f32 * cell_size * cell_size;
            flux[idx] = k * area.powf(m) * slope.powf(n);
        }
    }
    flux
}

// ============================================================
//  EXTRA MATH FUNCTIONS
// ============================================================

/// Linear congruential pseudo-random on f32 input (useful for hash-based shaders)
#[inline]
pub fn hash_f32(x: f32) -> f32 {
    let mut h = (x.to_bits() ^ 0x9e3779b9u32).wrapping_mul(0x6c62272e);
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    (h as f32) / u32::MAX as f32
}

#[inline]
pub fn hash_vec2(v: Vec2) -> f32 {
    hash_f32(v.x * 127.1 + v.y * 311.7)
}

#[inline]
pub fn hash_vec3(v: Vec3) -> f32 {
    hash_f32(v.x * 127.1 + v.y * 311.7 + v.z * 74.7)
}

/// Integer hash (Wang hash)
#[inline]
pub fn wang_hash(mut n: u32) -> u32 {
    n = (n ^ 61) ^ (n >> 16);
    n = n.wrapping_mul(9);
    n ^= n >> 4;
    n = n.wrapping_mul(0x27d4eb2d);
    n ^= n >> 15;
    n
}

/// Float from integer hash, [0, 1)
#[inline]
pub fn float_from_hash(hash: u32) -> f32 {
    (hash as f32) / 4_294_967_296.0
}

/// Vectorized clamp
#[inline]
pub fn clamp_vec3(v: Vec3, lo: Vec3, hi: Vec3) -> Vec3 {
    Vec3::new(v.x.clamp(lo.x, hi.x), v.y.clamp(lo.y, hi.y), v.z.clamp(lo.z, hi.z))
}

/// Reflect a vector across a normal
#[inline]
pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - n * (2.0 * v.dot(n))
}

/// Refract a vector (Snell's law), returns None for total internal reflection
pub fn refract(v: Vec3, n: Vec3, eta: f32) -> Option<Vec3> {
    let cos_i  = -v.dot(n);
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
    if sin2_t > 1.0 { return None; }
    let cos_t  = (1.0 - sin2_t).sqrt();
    Some(v * eta + n * (eta * cos_i - cos_t))
}

/// Fresnel reflectance (Schlick approximation)
#[inline]
pub fn fresnel_schlick(cos_theta: f32, r0: f32) -> f32 {
    r0 + (1.0 - r0) * (1.0 - cos_theta).powi(5)
}

/// Frenet-Serret frame along a path
pub fn frenet_frame(tangent: Vec3, up_hint: Vec3) -> (Vec3, Vec3, Vec3) {
    let t  = tangent.normalize();
    let b  = t.cross(up_hint).normalize();
    let n  = b.cross(t);
    (t, n, b) // tangent, normal, binormal
}

// ============================================================
//  MATERIAL SYSTEM FOR TERRAIN RENDERING
// ============================================================

#[derive(Clone, Debug)]
pub struct TerrainMaterial {
    pub albedo_color:    Vec4,
    pub roughness:       f32,
    pub metallic:        f32,
    pub normal_strength: f32,
    pub displacement:    f32,
    pub tiling_scale:    Vec2,
    pub texture_ids:     [u32; 4],  // albedo, normal, roughness, displacement
}

impl TerrainMaterial {
    pub fn default_grass() -> Self {
        TerrainMaterial { albedo_color: Vec4::new(0.20, 0.55, 0.12, 1.0), roughness: 0.85, metallic: 0.0,
            normal_strength: 0.8, displacement: 0.05, tiling_scale: Vec2::new(8.0, 8.0), texture_ids: [0,1,2,3] }
    }
    pub fn default_rock() -> Self {
        TerrainMaterial { albedo_color: Vec4::new(0.50, 0.45, 0.40, 1.0), roughness: 0.90, metallic: 0.0,
            normal_strength: 1.2, displacement: 0.15, tiling_scale: Vec2::new(4.0, 4.0), texture_ids: [4,5,6,7] }
    }
    pub fn default_snow() -> Self {
        TerrainMaterial { albedo_color: Vec4::new(0.95, 0.97, 1.0, 1.0), roughness: 0.30, metallic: 0.0,
            normal_strength: 0.3, displacement: 0.02, tiling_scale: Vec2::new(6.0, 6.0), texture_ids: [8,9,10,11] }
    }
    pub fn default_sand() -> Self {
        TerrainMaterial { albedo_color: Vec4::new(0.87, 0.79, 0.55, 1.0), roughness: 0.95, metallic: 0.0,
            normal_strength: 0.5, displacement: 0.08, tiling_scale: Vec2::new(10.0, 10.0), texture_ids: [12,13,14,15] }
    }
    pub fn default_water() -> Self {
        TerrainMaterial { albedo_color: Vec4::new(0.10, 0.35, 0.65, 0.85), roughness: 0.05, metallic: 0.0,
            normal_strength: 1.5, displacement: 0.0, tiling_scale: Vec2::new(20.0, 20.0), texture_ids: [16,17,18,19] }
    }

    /// Blend two materials by a weight [0..1]
    pub fn blend(&self, other: &TerrainMaterial, t: f32) -> TerrainMaterial {
        let lf = |a: f32, b: f32| a + (b - a) * t;
        let lv4 = |a: Vec4, b: Vec4| a + (b - a) * t;
        let lv2 = |a: Vec2, b: Vec2| a + (b - a) * t;
        TerrainMaterial {
            albedo_color:    lv4(self.albedo_color, other.albedo_color),
            roughness:       lf(self.roughness, other.roughness),
            metallic:        lf(self.metallic, other.metallic),
            normal_strength: lf(self.normal_strength, other.normal_strength),
            displacement:    lf(self.displacement, other.displacement),
            tiling_scale:    lv2(self.tiling_scale, other.tiling_scale),
            texture_ids:     if t < 0.5 { self.texture_ids } else { other.texture_ids },
        }
    }
}

// ============================================================
//  EXTRA EDITOR OBJECT METHODS
// ============================================================

impl WorldEditor {
    /// Place a volcano at a world position
    pub fn place_volcano(&mut self, world_x: f32, world_z: f32, radius: f32, rim_height: f32, caldera_depth: f32) {
        let stamp_size = (radius * 2.0 / self.cell_size) as usize + 4;
        let stamp = volcano_stamp(stamp_size, 0.6, rim_height / self.height_scale, caldera_depth / self.height_scale);
        let cx = world_x / self.cell_size;
        let cz = world_z / self.cell_size;
        let action = terrain_stamp(&mut self.heightmap, cx, cz, &stamp, stamp_size, stamp_size, 1.0);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    /// Place a mesa (flat-topped plateau) at a world position
    pub fn place_mesa(&mut self, world_x: f32, world_z: f32, radius: f32, height: f32, steepness: f32) {
        let stamp_size = (radius * 2.5 / self.cell_size) as usize + 4;
        let stamp = mesa_stamp(stamp_size, 0.5, steepness, height / self.height_scale);
        let cx = world_x / self.cell_size;
        let cz = world_z / self.cell_size;
        let action = terrain_stamp(&mut self.heightmap, cx, cz, &stamp, stamp_size, stamp_size, 1.0);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    /// Place an impact crater at a world position
    pub fn place_crater(&mut self, world_x: f32, world_z: f32, radius: f32, rim_height: f32, depth: f32) {
        let stamp_size = (radius * 3.0 / self.cell_size) as usize + 4;
        let stamp = crater_stamp(stamp_size, 0.55, rim_height / self.height_scale, depth / self.height_scale);
        let cx = world_x / self.cell_size;
        let cz = world_z / self.cell_size;
        let action = terrain_stamp(&mut self.heightmap, cx, cz, &stamp, stamp_size, stamp_size, 1.0);
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    /// Fill terrain sinks (hydrological preprocessing)
    pub fn fill_terrain_sinks(&mut self) {
        fill_sinks(&mut self.heightmap, 0.0001);
        self.heightmap_dirty = true;
    }

    /// Compute statistics for a specific region
    pub fn stats_for_region(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> WorldStats {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        let x1 = x1.min(w);
        let y1 = y1.min(h);
        let total = (x1 - x0) * (y1 - y0);
        let mut ocean = 0usize;
        let mut mountain = 0usize;
        let mut sum = 0.0f64;
        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;
        for y in y0..y1 { for x in x0..x1 {
            let ht = self.heightmap.get(x, y);
            sum += ht as f64;
            if ht < min_h { min_h = ht; }
            if ht > max_h { max_h = ht; }
            if ht <= self.sea_level { ocean += 1; }
            if ht > 0.7 { mountain += 1; }
        }}
        WorldStats {
            total_cells: total, ocean_cells: ocean, land_cells: total - ocean, mountain_cells: mountain,
            river_count: 0, lake_count: 0, road_segments: 0, road_total_length: 0.0,
            foliage_count: 0, min_height: min_h, max_height: max_h,
            mean_height: (sum / total as f64) as f32, dominant_biome: None,
        }
    }

    /// Smooth the entire heightmap with a Gaussian filter
    pub fn gaussian_smooth(&mut self, sigma: f32) {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        let old = self.heightmap.data.clone();
        let smoothed = gaussian_blur_2d(&old, w, h, sigma);
        let x0 = 0; let y0 = 0;
        let action = EditAction::SetHeightRegion {
            x: 0, y: 0, width: w, height: h,
            old_data: old,
            new_data: smoothed.clone(),
        };
        self.heightmap.data = smoothed;
        self.heightmap.recompute_minmax();
        self.undo_redo.push(action);
        self.heightmap_dirty = true;
    }

    /// Generate full navigation data for the world
    pub fn build_nav_grid(&self, max_slope_deg: f32) -> NavGrid {
        let mut grid = NavGrid::from_heightmap(&self.heightmap, self.cell_size, self.sea_level, max_slope_deg);
        grid.label_regions();
        grid
    }

    /// Get biome statistics (percentage of each biome type)
    pub fn biome_percentages(&self) -> [(BiomeId, f32); 25] {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        let total = (w * h) as f32;
        let mut counts = [0usize; 25];
        for y in 0..h { for x in 0..w {
            let idx  = y * w + x;
            let alt  = self.heightmap.get(x, y);
            let temp = if idx < self.temperature_map.len() { self.temperature_map[idx] } else { 15.0 };
            let hum  = if idx < self.humidity_map.len()    { self.humidity_map[idx]    } else { 0.5  };
            let biome = BiomeDescriptor::classify_point(temp, hum, alt);
            counts[biome as usize] += 1;
        }}
        [
            (BiomeId::TropicalRainforest,  counts[0]  as f32 / total),
            (BiomeId::TropicalSavanna,     counts[1]  as f32 / total),
            (BiomeId::HotDesert,           counts[2]  as f32 / total),
            (BiomeId::ColdDesert,          counts[3]  as f32 / total),
            (BiomeId::XericShrubland,      counts[4]  as f32 / total),
            (BiomeId::MediterraneanShrub,  counts[5]  as f32 / total),
            (BiomeId::TemperateGrassland,  counts[6]  as f32 / total),
            (BiomeId::TemperateRainforest, counts[7]  as f32 / total),
            (BiomeId::TemperateDeciduous,  counts[8]  as f32 / total),
            (BiomeId::BorealForest,        counts[9]  as f32 / total),
            (BiomeId::TaigaSpruce,         counts[10] as f32 / total),
            (BiomeId::Tundra,              counts[11] as f32 / total),
            (BiomeId::ArcticDesert,        counts[12] as f32 / total),
            (BiomeId::AlpineMeadow,        counts[13] as f32 / total),
            (BiomeId::AlpineTundra,        counts[14] as f32 / total),
            (BiomeId::PolarIceCap,         counts[15] as f32 / total),
            (BiomeId::Mangrove,            counts[16] as f32 / total),
            (BiomeId::Wetland,             counts[17] as f32 / total),
            (BiomeId::FloodPlain,          counts[18] as f32 / total),
            (BiomeId::VolcanicLandscape,   counts[19] as f32 / total),
            (BiomeId::SaltFlat,            counts[20] as f32 / total),
            (BiomeId::GlacialValley,       counts[21] as f32 / total),
            (BiomeId::CoastalDunes,        counts[22] as f32 / total),
            (BiomeId::DeepOceanFloor,      counts[23] as f32 / total),
            (BiomeId::CoralReef,           counts[24] as f32 / total),
        ]
    }
}

// ============================================================
//  ATMOSPHERIC SCATTERING LOOKUP TABLE
// ============================================================

/// Pre-bake transmittance table for faster sky rendering
pub struct TransmittanceLut {
    pub width:  usize,
    pub height: usize,
    pub data:   Vec<[f32; 3]>,  // RGB transmittance per sample
}

impl TransmittanceLut {
    pub fn bake(params: &AtmosphereParams, width: usize, height: usize) -> Self {
        let mut data = vec![[0.0f32; 3]; width * height];

        for v_idx in 0..height {
            for u_idx in 0..width {
                // u = altitude fraction [0..1], v = cos(zenith angle) [-1..1]
                let u         = u_idx as f64 / (width  - 1) as f64;
                let v         = v_idx as f64 / (height - 1) as f64 * 2.0 - 1.0;
                let altitude_km = u * (params.atmo_radius - params.planet_radius);
                let cos_zenith  = v;

                let h = altitude_km;
                let hr = (-(h / params.rayleigh_scale_height)).exp();
                let hm = (-(h / params.mie_scale_height)).exp();

                // Simple path length approximation
                let path_len = if cos_zenith.abs() < 1e-6 {
                    params.atmo_radius - params.planet_radius
                } else {
                    ((params.atmo_radius * params.atmo_radius
                      - (params.planet_radius + h) * (params.planet_radius + h) * (1.0 - cos_zenith * cos_zenith)).sqrt()
                     - (params.planet_radius + h) * cos_zenith).max(0.0)
                };

                let tau_r = [
                    params.rayleigh_coeff[0] * hr * path_len,
                    params.rayleigh_coeff[1] * hr * path_len,
                    params.rayleigh_coeff[2] * hr * path_len,
                ];
                let tau_m_val = 1.1 * params.mie_coeff * hm * path_len;

                data[v_idx * width + u_idx] = [
                    (-(tau_r[0] + tau_m_val)).exp() as f32,
                    (-(tau_r[1] + tau_m_val)).exp() as f32,
                    (-(tau_r[2] + tau_m_val)).exp() as f32,
                ];
            }
        }

        TransmittanceLut { width, height, data }
    }

    pub fn sample(&self, altitude_norm: f32, cos_zenith: f32) -> Vec3 {
        let u = altitude_norm.clamp(0.0, 1.0) * (self.width  - 1) as f32;
        let v = ((cos_zenith + 1.0) * 0.5).clamp(0.0, 1.0) * (self.height - 1) as f32;
        let x0 = u.floor() as usize;
        let y0 = v.floor() as usize;
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = u - x0 as f32;
        let ty = v - y0 as f32;
        let s = |xi: usize, yi: usize| { let d = self.data[yi * self.width + xi]; Vec3::new(d[0], d[1], d[2]) };
        let a = s(x0, y0).lerp(s(x1, y0), tx);
        let b = s(x0, y1).lerp(s(x1, y1), tx);
        a.lerp(b, ty)
    }
}

// ============================================================
//  FINAL CONSTANTS AND VERSION INFO
// ============================================================

pub const WORLD_EDITOR_VERSION: &str = "0.1.0";
pub const WORLD_EDITOR_BUILD: u32    = 10001;


/// Returns a short version string
pub fn editor_version() -> String {
    format!("WorldEditor v{} (build {})", WORLD_EDITOR_VERSION, WORLD_EDITOR_BUILD)
}

/// Distance between two grid cells in world units
#[inline]
pub fn grid_distance(x0: usize, y0: usize, x1: usize, y1: usize, cell_size: f32) -> f32 {
    let dx = (x1 as i32 - x0 as i32) as f32;
    let dy = (y1 as i32 - y0 as i32) as f32;
    (dx*dx + dy*dy).sqrt() * cell_size
}

/// Check if two AABB volumes overlap
#[inline]
pub fn aabb_overlap(min_a: Vec3, max_a: Vec3, min_b: Vec3, max_b: Vec3) -> bool {
    min_a.x <= max_b.x && max_a.x >= min_b.x &&
    min_a.y <= max_b.y && max_a.y >= min_b.y &&
    min_a.z <= max_b.z && max_a.z >= min_b.z
}

/// Compute the area of a triangle given three 2D vertices
#[inline]
pub fn triangle_area_2d(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    ((b - a).perp_dot(c - a)).abs() * 0.5
}

/// Barycentric coordinates of point P in triangle (A, B, C)
pub fn barycentric(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> Vec3 {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    Vec3::new(1.0 - u - v, v, u)
}

/// Point-in-triangle test using barycentric coordinates
#[inline]
pub fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let bary = barycentric(p, a, b, c);
    bary.x >= 0.0 && bary.y >= 0.0 && bary.z >= 0.0
}

/// Clamp a point to the boundary of an AABB
#[inline]
pub fn clamp_to_aabb(p: Vec3, min: Vec3, max: Vec3) -> Vec3 {
    Vec3::new(p.x.clamp(min.x, max.x), p.y.clamp(min.y, max.y), p.z.clamp(min.z, max.z))
}

/// Signed distance from a point to a plane
#[inline]
pub fn signed_distance_to_plane(point: Vec3, plane_normal: Vec3, plane_d: f32) -> f32 {
    plane_normal.dot(point) + plane_d
}

// ============================================================
//  SPLINE PATH EDITOR TOOL
// ============================================================

#[derive(Clone, Debug)]
pub struct SplinePath {
    pub id:           u32,
    pub control_pts:  Vec<Vec3>,
    pub name:         String,
    pub closed:       bool,
    pub tangents:     Vec<Vec3>,
}

impl SplinePath {
    pub fn new(id: u32, name: &str) -> Self {
        SplinePath { id, control_pts: Vec::new(), name: name.into(), closed: false, tangents: Vec::new() }
    }

    pub fn add_point(&mut self, p: Vec3) {
        self.control_pts.push(p);
        self.recompute_tangents();
    }

    pub fn remove_point(&mut self, idx: usize) {
        if idx < self.control_pts.len() {
            self.control_pts.remove(idx);
            self.recompute_tangents();
        }
    }

    pub fn move_point(&mut self, idx: usize, new_pos: Vec3) {
        if idx < self.control_pts.len() {
            self.control_pts[idx] = new_pos;
            self.recompute_tangents();
        }
    }

    pub fn recompute_tangents(&mut self) {
        let n = self.control_pts.len();
        self.tangents = vec![Vec3::ZERO; n];
        if n < 2 { return; }
        for i in 0..n {
            let prev = if i == 0 { self.control_pts[0] } else { self.control_pts[i - 1] };
            let next = if i == n-1 { self.control_pts[n-1] } else { self.control_pts[i + 1] };
            self.tangents[i] = (next - prev).normalize_or_zero();
        }
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.control_pts.len();
        if n == 0 { return Vec3::ZERO; }
        if n == 1 { return self.control_pts[0]; }
        let total_t = if self.closed { n as f32 } else { (n - 1) as f32 };
        let t_clamped = t.clamp(0.0, 1.0) * total_t;
        let seg = t_clamped.floor() as usize;
        let local_t = t_clamped - seg as f32;
        let i0 = seg.min(n - 1);
        let i1 = (seg + 1).min(n - 1);
        let p0 = self.control_pts[i0];
        let p1 = self.control_pts[i1];
        let tan0 = self.tangents[i0] * (p1 - p0).length() * 0.3;
        let tan1 = self.tangents[i1] * (p1 - p0).length() * 0.3;
        // Hermite interpolation
        let h00 =  2.0 * local_t.powi(3) - 3.0 * local_t.powi(2) + 1.0;
        let h10 =       local_t.powi(3) - 2.0 * local_t.powi(2) + local_t;
        let h01 = -2.0 * local_t.powi(3) + 3.0 * local_t.powi(2);
        let h11 =       local_t.powi(3) -       local_t.powi(2);
        p0 * h00 + tan0 * h10 + p1 * h01 + tan1 * h11
    }

    pub fn arc_length(&self, steps_per_seg: usize) -> f32 {
        let n = self.control_pts.len();
        if n < 2 { return 0.0; }
        let total_steps = (n - 1) * steps_per_seg;
        let mut len  = 0.0f32;
        let mut prev = self.evaluate(0.0);
        for i in 1..=total_steps {
            let t    = i as f32 / total_steps as f32;
            let curr = self.evaluate(t);
            len += (curr - prev).length();
            prev = curr;
        }
        len
    }

    /// Sample evenly-spaced points along the spline
    pub fn sample_uniform(&self, count: usize) -> Vec<Vec3> {
        if count == 0 { return Vec::new(); }
        if count == 1 { return vec![self.evaluate(0.5)]; }
        (0..count).map(|i| self.evaluate(i as f32 / (count - 1) as f32)).collect()
    }
}

// ============================================================
//  HEIGHTMAP OPERATION QUEUE (async-style)
// ============================================================

#[derive(Clone, Debug)]
pub enum HeightmapOp {
    Noise      { params: FbmParams, offset: Vec2 },
    Erosion    { params: ErosionParams },
    Thermal    { iterations: usize, talus_deg: f32 },
    Blur       { sigma: f32 },
    Normalize,
    Clamp      { min: f32, max: f32 },
    Multiply   { factor: f32 },
    Add        { value: f32 },
    FillSinks,
    IslandMask { falloff: f32 },
}

pub struct HeightmapOpQueue {
    pub ops:   VecDeque<HeightmapOp>,
    pub dirty: bool,
}

impl HeightmapOpQueue {
    pub fn new() -> Self { HeightmapOpQueue { ops: VecDeque::new(), dirty: false } }

    pub fn push(&mut self, op: HeightmapOp) { self.ops.push_back(op); self.dirty = true; }

    pub fn execute_all(&mut self, hmap: &mut Heightmap) {
        while let Some(op) = self.ops.pop_front() {
            match op {
                HeightmapOp::Noise { params, offset } => {
                    hmap.generate_fbm(&params, offset);
                }
                HeightmapOp::Erosion { params } => {
                    hydraulic_erosion(hmap, &params);
                }
                HeightmapOp::Thermal { iterations, talus_deg } => {
                    thermal_erosion(hmap, iterations, talus_deg * DEG2RAD);
                }
                HeightmapOp::Blur { sigma } => {
                    let w = hmap.width; let h = hmap.height;
                    let blurred = gaussian_blur_2d(&hmap.data.clone(), w, h, sigma);
                    hmap.data = blurred;
                    hmap.recompute_minmax();
                }
                HeightmapOp::Normalize => {
                    hmap.normalize_to_01();
                }
                HeightmapOp::Clamp { min, max } => {
                    for v in hmap.data.iter_mut() { *v = v.clamp(min, max); }
                    hmap.recompute_minmax();
                }
                HeightmapOp::Multiply { factor } => {
                    for v in hmap.data.iter_mut() { *v = (*v * factor).clamp(0.0, 1.0); }
                    hmap.recompute_minmax();
                }
                HeightmapOp::Add { value } => {
                    for v in hmap.data.iter_mut() { *v = (*v + value).clamp(0.0, 1.0); }
                    hmap.recompute_minmax();
                }
                HeightmapOp::FillSinks => {
                    fill_sinks(hmap, 0.0001);
                }
                HeightmapOp::IslandMask { falloff } => {
                    let mask = generate_island_mask(hmap.width, hmap.height, falloff);
                    apply_mask(hmap, &mask);
                }
            }
        }
        self.dirty = false;
    }
}

// ============================================================
//  SOUND SOURCE SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct SoundSource {
    pub id:           u32,
    pub position:     Vec3,
    pub max_dist:     f32,
    pub base_volume:  f32,
    pub sound_id:     u32,
    pub looping:      bool,
    pub terrain_occ:  bool,
}

impl SoundSource {
    pub fn volume_at(&self, listener: Vec3, hmap: &Heightmap, cell_size: f32, height_scale: f32) -> f32 {
        let dist = (self.position - listener).length();
        if dist >= self.max_dist { return 0.0; }
        let atten  = (1.0 - dist / self.max_dist).powi(2);
        if !self.terrain_occ { return (self.base_volume * atten).clamp(0.0, 1.0); }

        let dir   = (listener - self.position).normalize();
        let steps = (dist / cell_size) as usize;
        let blocked = (1..steps).any(|s| {
            let p  = self.position + dir * s as f32 * cell_size;
            let ux = (p.x / (hmap.width  as f32 * cell_size)).clamp(0.0, 1.0);
            let uz = (p.z / (hmap.height as f32 * cell_size)).clamp(0.0, 1.0);
            hmap.sample_bilinear(ux, uz) * height_scale > p.y + 2.0
        });
        (self.base_volume * atten * if blocked { 0.15 } else { 1.0 }).clamp(0.0, 1.0)
    }
}

// ============================================================
//  WATER CAUSTICS TEXTURE GENERATION
// ============================================================

/// Generate an animated water caustics pattern using interference of waves
pub fn generate_caustics_pattern(width: usize, height: usize, time: f32, wave_count: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; width * height];
    let mut rng = LcgRng::new(0xCA05710C);

    let waves: Vec<(f32, f32, f32, f32)> = (0..wave_count).map(|_| {
        let angle = rng.next_f32() * TWO_PI;
        let freq  = 3.0 + rng.next_f32() * 8.0;
        let phase = rng.next_f32() * TWO_PI;
        let amp   = 0.5 + rng.next_f32() * 0.5;
        (angle, freq, phase, amp)
    }).collect();

    for y in 0..height {
        for x in 0..width {
            let ux = x as f32 / width  as f32;
            let uy = y as f32 / height as f32;
            let mut v = 0.0f32;
            for &(angle, freq, phase, amp) in &waves {
                let proj = ux * angle.cos() + uy * angle.sin();
                v += amp * (proj * freq * TWO_PI + phase + time * 2.0).sin();
            }
            v = v / wave_count as f32 * 0.5 + 0.5;
            out[y * width + x] = v.powi(2); // sharpen caustics
        }
    }
    out
}

// ============================================================
//  VEGETATION DISTRIBUTION BY SLOPE AND ALTITUDE
// ============================================================

#[derive(Clone, Debug)]
pub struct VegetationRule {
    pub asset_id:    u32,
    pub name:        &'static str,
    pub min_alt:     f32,
    pub max_alt:     f32,
    pub min_slope:   f32,
    pub max_slope:   f32,
    pub min_temp:    f32,
    pub max_temp:    f32,
    pub min_hum:     f32,
    pub max_hum:     f32,
    pub density:     f32,
    pub min_radius:  f32,
}

pub fn build_default_vegetation_rules() -> Vec<VegetationRule> {
    vec![
        VegetationRule { asset_id:0,  name:"Oak Tree",      min_alt:0.05, max_alt:0.60, min_slope:0.0, max_slope:0.5, min_temp:5.0,  max_temp:25.0, min_hum:0.40, max_hum:0.80, density:0.5, min_radius:4.0 },
        VegetationRule { asset_id:1,  name:"Pine Tree",     min_alt:0.20, max_alt:0.75, min_slope:0.0, max_slope:0.6, min_temp:-5.0, max_temp:15.0, min_hum:0.35, max_hum:0.75, density:0.6, min_radius:3.5 },
        VegetationRule { asset_id:2,  name:"Palm Tree",     min_alt:0.00, max_alt:0.20, min_slope:0.0, max_slope:0.3, min_temp:20.0, max_temp:40.0, min_hum:0.30, max_hum:0.80, density:0.4, min_radius:5.0 },
        VegetationRule { asset_id:3,  name:"Spruce",        min_alt:0.30, max_alt:0.70, min_slope:0.0, max_slope:0.5, min_temp:-15.0, max_temp:8.0, min_hum:0.40, max_hum:0.80, density:0.7, min_radius:3.0 },
        VegetationRule { asset_id:4,  name:"Cactus",        min_alt:0.00, max_alt:0.40, min_slope:0.0, max_slope:0.4, min_temp:15.0, max_temp:50.0, min_hum:0.00, max_hum:0.20, density:0.2, min_radius:2.0 },
        VegetationRule { asset_id:5,  name:"Birch",         min_alt:0.05, max_alt:0.55, min_slope:0.0, max_slope:0.5, min_temp:-5.0, max_temp:20.0, min_hum:0.45, max_hum:0.75, density:0.5, min_radius:3.5 },
        VegetationRule { asset_id:6,  name:"Bamboo",        min_alt:0.02, max_alt:0.35, min_slope:0.0, max_slope:0.4, min_temp:15.0, max_temp:35.0, min_hum:0.60, max_hum:1.00, density:0.8, min_radius:1.5 },
        VegetationRule { asset_id:7,  name:"Fern Shrub",    min_alt:0.00, max_alt:0.50, min_slope:0.0, max_slope:0.6, min_temp:5.0,  max_temp:30.0, min_hum:0.50, max_hum:1.00, density:0.7, min_radius:1.0 },
        VegetationRule { asset_id:8,  name:"Bush",          min_alt:0.00, max_alt:0.60, min_slope:0.0, max_slope:0.5, min_temp:0.0,  max_temp:35.0, min_hum:0.25, max_hum:0.75, density:0.6, min_radius:1.5 },
        VegetationRule { asset_id:9,  name:"Tundra Grass",  min_alt:0.00, max_alt:0.65, min_slope:0.0, max_slope:0.4, min_temp:-25.0, max_temp:5.0, min_hum:0.20, max_hum:0.60, density:0.5, min_radius:0.5 },
        VegetationRule { asset_id:10, name:"Tall Grass",    min_alt:0.00, max_alt:0.45, min_slope:0.0, max_slope:0.4, min_temp:5.0,  max_temp:30.0, min_hum:0.30, max_hum:0.70, density:0.9, min_radius:0.3 },
        VegetationRule { asset_id:11, name:"Reed",          min_alt:0.00, max_alt:0.10, min_slope:0.0, max_slope:0.1, min_temp:5.0,  max_temp:35.0, min_hum:0.75, max_hum:1.00, density:0.8, min_radius:0.5 },
        VegetationRule { asset_id:12, name:"Mangrove Root", min_alt:0.00, max_alt:0.08, min_slope:0.0, max_slope:0.1, min_temp:20.0, max_temp:36.0, min_hum:0.75, max_hum:1.00, density:0.6, min_radius:3.0 },
    ]
}

pub fn apply_vegetation_rules(
    hmap:     &Heightmap,
    temp_map: &[f32],
    hum_map:  &[f32],
    rules:    &[VegetationRule],
    cell_size: f32,
    seed:     u64,
) -> Vec<FoliageInstance> {
    let mut result    = Vec::new();
    let w = hmap.width  as f32;
    let h = hmap.height as f32;

    for (ri, rule) in rules.iter().enumerate() {
        let candidates = poisson_disk_2d(w, h, rule.min_radius, 30, seed ^ (ri as u64 * 31337));
        let mut rng    = LcgRng::new(seed ^ ri as u64 * 997);

        for pos in &candidates {
            let ux  = (pos.x / w).clamp(0.0, 1.0);
            let uy  = (pos.y / h).clamp(0.0, 1.0);
            let alt = hmap.sample_bilinear(ux, uy);
            if alt < rule.min_alt || alt > rule.max_alt { continue; }

            let xi    = (pos.x as usize).min(hmap.width  - 1);
            let yi    = (pos.y as usize).min(hmap.height - 1);
            let slope = hmap.slope_at(xi, yi, cell_size);
            if slope < rule.min_slope || slope > rule.max_slope { continue; }

            let idx  = yi * hmap.width + xi;
            let temp = if idx < temp_map.len() { temp_map[idx] } else { 15.0 };
            let hum  = if idx < hum_map.len()  { hum_map[idx]  } else { 0.5  };
            if temp < rule.min_temp || temp > rule.max_temp { continue; }
            if hum  < rule.min_hum  || hum  > rule.max_hum  { continue; }

            if rng.next_f32() > rule.density { continue; }

            let angle = rng.next_f32() * TWO_PI;
            let sv    = 0.75 + rng.next_f32() * 0.5;
            result.push(FoliageInstance {
                position:  Vec3::new(pos.x * cell_size, alt * 500.0, pos.y * cell_size),
                rotation:  Quat::from_rotation_y(angle),
                scale:     Vec3::new(sv, sv * (0.8 + rng.next_f32() * 0.4), sv),
                asset_id:  rule.asset_id,
                biome_id:  0,
                lod_factor: 1.0,
            });
        }
    }
    result
}

// ============================================================
//  TERRAIN LEVEL-OF-DETAIL DISTANCE BANDS
// ============================================================

#[derive(Clone, Debug)]
pub struct LodBand {
    pub max_distance: f32,
    pub mesh_step:    usize,  // 1 = full, 2 = half, 4 = quarter
    pub texture_lod:  u8,
    pub foliage:      bool,
    pub shadows:      bool,
}

pub const LOD_BANDS: [LodBand; 5] = [
    LodBand { max_distance:   50.0, mesh_step: 1, texture_lod: 0, foliage: true,  shadows: true  },
    LodBand { max_distance:  150.0, mesh_step: 1, texture_lod: 0, foliage: true,  shadows: true  },
    LodBand { max_distance:  400.0, mesh_step: 2, texture_lod: 1, foliage: true,  shadows: false },
    LodBand { max_distance: 1000.0, mesh_step: 4, texture_lod: 2, foliage: false, shadows: false },
    LodBand { max_distance: 3000.0, mesh_step: 8, texture_lod: 3, foliage: false, shadows: false },
];

pub fn select_lod_band(distance: f32) -> &'static LodBand {
    for band in &LOD_BANDS {
        if distance < band.max_distance { return band; }
    }
    &LOD_BANDS[LOD_BANDS.len() - 1]
}

// ============================================================
//  WATER SHIMMER / SPECULAR HIGHLIGHT COMPUTATION
// ============================================================

/// Compute water specular highlight intensity for a view and sun direction
pub fn water_specular(view_dir: Vec3, sun_dir: Vec3, water_normal: Vec3, roughness: f32) -> f32 {
    let half_vec    = (view_dir + sun_dir).normalize();
    let n_dot_h     = water_normal.dot(half_vec).max(0.0);
    let alpha       = roughness * roughness;
    let alpha2      = alpha * alpha;
    let denom       = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    let ggx_ndf     = alpha2 / (PI * denom * denom);
    let n_dot_l     = water_normal.dot(sun_dir).max(0.0);
    let n_dot_v     = water_normal.dot(view_dir).max(0.0);
    let r0          = 0.02; // water Fresnel R0
    let fresnel     = fresnel_schlick(n_dot_v, r0);
    ggx_ndf * fresnel * n_dot_l
}

/// Gerstner wave displacement for water surface
pub fn gerstner_wave(pos: Vec2, amplitude: f32, wavelength: f32, direction: Vec2, speed: f32, steepness: f32, time: f32) -> Vec3 {
    let k    = TWO_PI / wavelength;
    let c    = speed;
    let d    = direction.normalize();
    let f    = k * d.dot(pos) - c * time;
    let q   = steepness / (k * amplitude);
    Vec3::new(
        q * amplitude * d.x * f.cos(),
        amplitude * f.sin(),
        q * amplitude * d.y * f.cos(),
    )
}

/// Sum multiple Gerstner waves for realistic water surface
pub fn gerstner_wave_sum(pos: Vec2, time: f32) -> Vec3 {
    let waves: [(f32, f32, Vec2, f32, f32); 4] = [
        (0.15, 8.0,  Vec2::new(1.0, 0.3).normalize(), 1.5, 0.3),
        (0.08, 5.0,  Vec2::new(0.5, 1.0).normalize(), 2.0, 0.25),
        (0.05, 3.0,  Vec2::new(-0.3, 1.0).normalize(), 2.5, 0.2),
        (0.03, 2.0,  Vec2::new(0.8, -0.5).normalize(), 3.0, 0.15),
    ];
    let mut disp = Vec3::ZERO;
    for &(amp, wl, dir, speed, steep) in &waves {
        disp += gerstner_wave(pos, amp, wl, dir, speed, steep, time);
    }
    disp
}

// ============================================================
//  EXTRA WORLD EDITOR METHODS — FINAL BATCH
// ============================================================

impl WorldEditor {
    /// Place vegetation according to rules database
    pub fn place_vegetation_by_rules(&mut self, seed: u64) {
        let rules = build_default_vegetation_rules();
        let instances = apply_vegetation_rules(
            &self.heightmap,
            &self.temperature_map,
            &self.humidity_map,
            &rules,
            self.cell_size,
            seed,
        );
        self.foliage.extend(instances);
        self.foliage_dirty = false;
    }

    /// Build a Voronoi biome map
    pub fn build_voronoi_biome_map(&self, num_sites: usize) -> VoronoiMap {
        let mut vmap = VoronoiMap::generate(self.heightmap.width, self.heightmap.height, num_sites, self.master_seed ^ 0x707010);
        vmap.assign_biomes(&self.heightmap, &self.temperature_map, &self.humidity_map);
        vmap
    }

    /// Get a material for a terrain cell based on slope/altitude/biome
    pub fn terrain_material_at(&self, x: usize, y: usize) -> TerrainMaterial {
        let alt   = self.heightmap.get(x, y);
        let slope = self.heightmap.slope_at(x, y, self.cell_size) * RAD2DEG;
        let snow  = self.compute_snow_map(0.75, 2.0);
        let w     = self.heightmap.width;
        let snow_v = if y * w + x < snow.len() { snow[y * w + x] } else { 0.0 };

        let rock_blend   = smoothstep(25.0, 45.0, slope);
        let snow_blend   = snow_v;
        let water_blend  = if alt <= self.sea_level { 1.0 } else { 0.0 };

        let grass = TerrainMaterial::default_grass();
        let rock  = TerrainMaterial::default_rock();
        let snow  = TerrainMaterial::default_snow();
        let water = TerrainMaterial::default_water();

        if water_blend > 0.5 { return water; }
        let base = grass.blend(&rock, rock_blend);
        base.blend(&snow, snow_blend)
    }

    /// Add metadata to the world
    pub fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Simulate a single rain event (increases humidity, may trigger erosion)
    pub fn simulate_rain_event(&mut self, intensity: f32, duration_hours: f32) {
        // Increase humidity temporarily
        for v in self.humidity_map.iter_mut() {
            *v = (*v + intensity * 0.3).clamp(0.0, 1.0);
        }
        // Apply light erosion proportional to intensity
        if intensity > 0.5 {
            let mut ep = self.erosion_params.clone();
            ep.num_particles = (ep.num_particles as f32 * intensity * 0.5) as usize;
            hydraulic_erosion(&mut self.heightmap, &ep);
            self.heightmap_dirty = true;
        }
        // Weather state update
        self.weather.current.precipitation_mm += intensity * 10.0 * duration_hours;
        self.weather.current.humidity = (self.weather.current.humidity + intensity * 0.2).clamp(0.0, 1.0);
    }

    /// Move all foliage onto the current terrain surface (after terrain edit)
    pub fn reseat_foliage_to_terrain(&mut self) {
        let new_ys: Vec<f32> = self.foliage.iter()
            .map(|fi| self.height_at_world(fi.position.x, fi.position.z))
            .collect();
        for (fi, new_y) in self.foliage.iter_mut().zip(new_ys) {
            fi.position.y = new_y;
        }
    }

    /// Remove all foliage below sea level (e.g. after sea level change)
    pub fn cull_underwater_foliage(&mut self) {
        let sea_h = self.sea_level * self.height_scale;
        self.foliage.retain(|fi| fi.position.y >= sea_h - 0.5);
    }

    /// Compute the total number of triangles in the terrain mesh at full LOD
    pub fn terrain_triangle_count(&self) -> usize {
        let w = self.heightmap.width;
        let h = self.heightmap.height;
        (w - 1) * (h - 1) * 2
    }

    /// Estimate terrain memory usage in bytes
    pub fn terrain_memory_bytes(&self) -> usize {
        let hmap_bytes     = self.heightmap.data.len() * 4;
        let temp_bytes     = self.temperature_map.len() * 4;
        let hum_bytes      = self.humidity_map.len() * 4;
        let foliage_bytes  = self.foliage.len() * std::mem::size_of::<FoliageInstance>();
        hmap_bytes + temp_bytes + hum_bytes + foliage_bytes
    }

    /// Recalculate all rivers from scratch using current heightmap
    pub fn recalculate_rivers(&mut self, num_rivers: usize) {
        self.rivers.clear();
        self.generate_rivers(num_rivers);
    }

    /// Serialise all editor state to bytes
    pub fn full_save(&self) -> Vec<u8> {
        self.serialize()
    }
}

// ============================================================
//  PERLIN NOISE 2D DERIVATIVE (for slope-based operations)
// ============================================================

/// Returns (value, dvalue/dx, dvalue/dy) for analytical gradient
pub fn perlin_noise_2d_deriv(x: f32, y: f32) -> (f32, f32, f32) {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let u  = fade(xf);
    let v  = fade(yf);
    // fade derivative: 30t^4 - 60t^3 + 30t^2
    let du = 30.0 * xf * xf * (xf * xf - 2.0 * xf + 1.0);
    let dv = 30.0 * yf * yf * (yf * yf - 2.0 * yf + 1.0);

    let xi_u = (xi & 255) as usize;
    let yi_u = (yi & 255) as usize;
    let a  = PERM[xi_u     + PERM[yi_u    ] as usize];
    let b  = PERM[xi_u + 1 + PERM[yi_u    ] as usize];
    let c  = PERM[xi_u     + PERM[yi_u + 1] as usize];
    let d  = PERM[xi_u + 1 + PERM[yi_u + 1] as usize];

    fn g2(h: u8, x: f32, y: f32) -> f32 {
        let hh = (h & 7) as usize;
        let gx: f32 = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, 0.0][hh];
        let gy: f32 = [0.0,  0.0, 1.0,  1.0,-1.0, -1.0, 1.0,-1.0][hh];
        gx * x + gy * y
    }

    let a00 = g2(a, xf,       yf);
    let b00 = g2(b, xf - 1.0, yf);
    let a10 = g2(c, xf,       yf - 1.0);
    let b10 = g2(d, xf - 1.0, yf - 1.0);

    let val  = lerp_f(lerp_f(a00, b00, u), lerp_f(a10, b10, u), v);
    let dx   = du * lerp_f(b00 - a00, b10 - a10, v)
             + u  * lerp_f(0.0,       0.0,        dv); // simplified
    let dy   = dv * (lerp_f(a10, b10, u) - lerp_f(a00, b00, u));

    (val, dx, dy)
}

// ============================================================
//  ADDITIONAL EDITOR CAMERA PRESETS
// ============================================================

impl EditorCamera {
    pub fn preset_top_down(center: Vec3) -> Self {
        EditorCamera {
            position:    center + Vec3::new(0.0, 1000.0, 0.0),
            target:      center,
            up:          Vec3::new(0.0, 0.0, -1.0),
            fov_deg:     45.0,
            aspect:      16.0 / 9.0,
            near:        1.0,
            far:         20000.0,
            orbit_yaw:   0.0,
            orbit_pitch: 90.0,
            orbit_dist:  1000.0,
        }
    }

    pub fn preset_horizon(center: Vec3) -> Self {
        let mut cam = EditorCamera {
            position:    center + Vec3::new(0.0, 200.0, 800.0),
            target:      center,
            up:          Vec3::Y,
            fov_deg:     70.0,
            aspect:      16.0 / 9.0,
            near:        0.5,
            far:         50000.0,
            orbit_yaw:   0.0,
            orbit_pitch: 15.0,
            orbit_dist:  800.0,
        };
        cam.update_orbit();
        cam
    }

    pub fn clamp_to_terrain(&mut self, hmap: &Heightmap, cell_size: f32, height_scale: f32, min_height_above: f32) {
        let ux = (self.position.x / (hmap.width  as f32 * cell_size)).clamp(0.0, 1.0);
        let uz = (self.position.z / (hmap.height as f32 * cell_size)).clamp(0.0, 1.0);
        let terrain_h = hmap.sample_bilinear(ux, uz) * height_scale;
        if self.position.y < terrain_h + min_height_above {
            let diff = terrain_h + min_height_above - self.position.y;
            self.position.y += diff;
            self.target.y   += diff;
        }
    }
}


// ============================================================
//  RENDER SETTINGS
// ============================================================

#[derive(Clone, Debug)]
pub struct WorldRenderSettings {
    pub enable_shadows:    bool,
    pub shadow_distance:   f32,
    pub shadow_cascades:   u8,
    pub enable_ao:         bool,
    pub ao_radius:         f32,
    pub ao_samples:        u32,
    pub enable_fog:        bool,
    pub fog_start:         f32,
    pub fog_end:           f32,
    pub fog_color:         Vec3,
    pub enable_bloom:      bool,
    pub bloom_threshold:   f32,
    pub bloom_intensity:   f32,
    pub exposure:          f32,
    pub gamma:             f32,
    pub tonemap_mode:      TonemapMode,
    pub enable_ssao:       bool,
    pub enable_motion_blur: bool,
    pub motion_blur_amount: f32,
    pub enable_vignette:   bool,
    pub vignette_strength: f32,
    pub enable_chromatic:  bool,
    pub chromatic_amount:  f32,
    pub water_tessellation: u8,
    pub terrain_max_lod:   u8,
    pub foliage_distance:  f32,
    pub foliage_density_scale: f32,
    pub sky_samples:       u32,
    pub render_wireframe:  bool,
    pub render_colliders:  bool,
    pub render_navmesh:    bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TonemapMode {
    Linear,
    Reinhard,
    ACES,
    Filmic,
    Uncharted2,
}

impl Default for WorldRenderSettings {
    fn default() -> Self {
        WorldRenderSettings {
            enable_shadows: true, shadow_distance: 500.0, shadow_cascades: 4,
            enable_ao: true, ao_radius: 2.0, ao_samples: 16,
            enable_fog: true, fog_start: 200.0, fog_end: 4000.0, fog_color: Vec3::new(0.7, 0.8, 0.9),
            enable_bloom: true, bloom_threshold: 1.2, bloom_intensity: 0.4,
            exposure: 1.0, gamma: 2.2, tonemap_mode: TonemapMode::ACES,
            enable_ssao: true, enable_motion_blur: false, motion_blur_amount: 0.5,
            enable_vignette: true, vignette_strength: 0.3,
            enable_chromatic: false, chromatic_amount: 0.003,
            water_tessellation: 4, terrain_max_lod: 4,
            foliage_distance: 500.0, foliage_density_scale: 1.0,
            sky_samples: 16, render_wireframe: false, render_colliders: false, render_navmesh: false,
        }
    }
}

// ============================================================
//  FINAL TRAIT IMPLEMENTATIONS AND MISC
// ============================================================


impl std::fmt::Display for BiomeDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Biome[{}] '{}' T:{:.0}..{:.0}°C H:{:.0}..{:.0}%",
            self.id as usize, self.name,
            self.temp_min, self.temp_max,
            self.humidity_min * 100.0, self.humidity_max * 100.0)
    }
}

impl std::fmt::Display for RoadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            RoadType::Dirt    => "Dirt",
            RoadType::Gravel  => "Gravel",
            RoadType::Paved   => "Paved",
            RoadType::Highway => "Highway",
            RoadType::Trail   => "Trail",
        })
    }
}

impl std::fmt::Display for EditorTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Utility: convert a normalised height value to a color using default terrain ramp
pub fn height_to_color(h: f32) -> Vec3 {
    ColorRamp::terrain_default().sample(h)
}

/// Debug visualization: create a checkerboard pattern
pub fn checkerboard_pattern(width: usize, height: usize, cell_size: usize) -> Vec<f32> {
    (0..height).flat_map(|y| (0..width).map(move |x| {
        let cx = x / cell_size;
        let cy = y / cell_size;
        if (cx + cy) % 2 == 0 { 1.0 } else { 0.0 }
    })).collect()
}

/// Compute bounding sphere of a set of points
pub fn bounding_sphere(points: &[Vec3]) -> (Vec3, f32) {
    if points.is_empty() { return (Vec3::ZERO, 0.0); }
    let center = points.iter().fold(Vec3::ZERO, |acc, &p| acc + p) / points.len() as f32;
    let radius = points.iter().map(|&p| (p - center).length()).fold(0.0f32, f32::max);
    (center, radius)
}

/// Compute axis-aligned bounding box of a set of points
pub fn bounding_aabb(points: &[Vec3]) -> (Vec3, Vec3) {
    if points.is_empty() { return (Vec3::ZERO, Vec3::ZERO); }
    let mut mn = Vec3::splat(f32::MAX);
    let mut mx = Vec3::splat(f32::MIN);
    for &p in points {
        mn.x = mn.x.min(p.x); mn.y = mn.y.min(p.y); mn.z = mn.z.min(p.z);
        mx.x = mx.x.max(p.x); mx.y = mx.y.max(p.y); mx.z = mx.z.max(p.z);
    }
    (mn, mx)
}

/// Uniform random point on unit sphere
pub fn random_on_sphere(rng: &mut LcgRng) -> Vec3 {
    loop {
        let v = Vec3::new(
            rng.next_f32() * 2.0 - 1.0,
            rng.next_f32() * 2.0 - 1.0,
            rng.next_f32() * 2.0 - 1.0,
        );
        let len = v.length();
        if len > 0.0001 && len <= 1.0 { return v / len; }
    }
}

/// Random point on unit disk
pub fn random_on_disk(rng: &mut LcgRng) -> Vec2 {
    loop {
        let v = Vec2::new(rng.next_f32() * 2.0 - 1.0, rng.next_f32() * 2.0 - 1.0);
        if v.length_squared() <= 1.0 { return v; }
    }
}

// ============================================================
//  END OF FILE
// ============================================================
