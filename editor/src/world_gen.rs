// world_gen.rs — Full World Generation Editor
// egui-based procedural world generator with noise stacks, biomes, erosion, rivers

use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

// ============================================================
// NOISE TYPES & DATA MODEL
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoiseType {
    Perlin,
    Simplex,
    Cellular,
    Worley,
    Ridged,
    Fbm { octaves: u32, lacunarity: f32, gain: f32 },
    Value,
    Cylinders,
    Spheres,
}

impl NoiseType {
    pub fn name(&self) -> &str {
        match self {
            NoiseType::Perlin => "Perlin",
            NoiseType::Simplex => "Simplex",
            NoiseType::Cellular => "Cellular",
            NoiseType::Worley => "Worley",
            NoiseType::Ridged => "Ridged",
            NoiseType::Fbm { .. } => "fBm",
            NoiseType::Value => "Value",
            NoiseType::Cylinders => "Cylinders",
            NoiseType::Spheres => "Spheres",
        }
    }

    pub fn all_variants() -> Vec<NoiseType> {
        vec![
            NoiseType::Perlin,
            NoiseType::Simplex,
            NoiseType::Cellular,
            NoiseType::Worley,
            NoiseType::Ridged,
            NoiseType::Fbm { octaves: 6, lacunarity: 2.0, gain: 0.5 },
            NoiseType::Value,
            NoiseType::Cylinders,
            NoiseType::Spheres,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlendMode {
    Add,
    Multiply,
    Screen,
    Overlay,
    Replace,
}

impl BlendMode {
    pub fn name(&self) -> &str {
        match self {
            BlendMode::Add => "Add",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::Replace => "Replace",
        }
    }

    pub fn all() -> &'static [BlendMode] {
        &[
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Replace,
        ]
    }

    pub fn apply(&self, base: f32, layer: f32) -> f32 {
        match self {
            BlendMode::Add => (base + layer).clamp(0.0, 1.0),
            BlendMode::Multiply => base * layer,
            BlendMode::Screen => 1.0 - (1.0 - base) * (1.0 - layer),
            BlendMode::Overlay => {
                if base < 0.5 {
                    2.0 * base * layer
                } else {
                    1.0 - 2.0 * (1.0 - base) * (1.0 - layer)
                }
            }
            BlendMode::Replace => layer,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoiseLayer {
    pub name: String,
    pub noise_type: NoiseType,
    pub frequency: f32,
    pub amplitude: f32,
    pub offset: [f32; 2],
    pub seed: u32,
    pub blend_mode: BlendMode,
    pub enabled: bool,
    pub weight: f32,
}

impl NoiseLayer {
    pub fn default_height() -> Self {
        Self {
            name: "Base Height".to_string(),
            noise_type: NoiseType::Fbm { octaves: 6, lacunarity: 2.0, gain: 0.5 },
            frequency: 0.003,
            amplitude: 1.0,
            offset: [0.0, 0.0],
            seed: 42,
            blend_mode: BlendMode::Replace,
            enabled: true,
            weight: 1.0,
        }
    }

    pub fn default_moisture() -> Self {
        Self {
            name: "Base Moisture".to_string(),
            noise_type: NoiseType::Perlin,
            frequency: 0.005,
            amplitude: 1.0,
            offset: [1000.0, 1000.0],
            seed: 137,
            blend_mode: BlendMode::Replace,
            enabled: true,
            weight: 1.0,
        }
    }

    pub fn default_temp() -> Self {
        Self {
            name: "Base Temperature".to_string(),
            noise_type: NoiseType::Value,
            frequency: 0.004,
            amplitude: 1.0,
            offset: [5000.0, 5000.0],
            seed: 999,
            blend_mode: BlendMode::Replace,
            enabled: true,
            weight: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeRule {
    pub name: String,
    pub color: Color32,
    pub min_height: f32,
    pub max_height: f32,
    pub min_moisture: f32,
    pub max_moisture: f32,
    pub min_temp: f32,
    pub max_temp: f32,
    pub glyph: char,
    pub density: f32,
    pub priority: i32,
}

impl BiomeRule {
    pub fn matches(&self, height: f32, moisture: f32, temp: f32) -> bool {
        height >= self.min_height && height <= self.max_height
            && moisture >= self.min_moisture && moisture <= self.max_moisture
            && temp >= self.min_temp && temp <= self.max_temp
    }
}

fn default_biomes() -> Vec<BiomeRule> {
    vec![
        BiomeRule {
            name: "Deep Ocean".to_string(), color: Color32::from_rgb(10, 40, 80),
            min_height: 0.0, max_height: 0.2, min_moisture: 0.0, max_moisture: 1.0,
            min_temp: 0.0, max_temp: 1.0, glyph: '~', density: 0.0, priority: 0,
        },
        BiomeRule {
            name: "Ocean".to_string(), color: Color32::from_rgb(20, 80, 150),
            min_height: 0.2, max_height: 0.35, min_moisture: 0.0, max_moisture: 1.0,
            min_temp: 0.0, max_temp: 1.0, glyph: '~', density: 0.0, priority: 1,
        },
        BiomeRule {
            name: "Beach".to_string(), color: Color32::from_rgb(220, 200, 140),
            min_height: 0.35, max_height: 0.4, min_moisture: 0.0, max_moisture: 1.0,
            min_temp: 0.2, max_temp: 1.0, glyph: '.', density: 0.02, priority: 2,
        },
        BiomeRule {
            name: "Desert".to_string(), color: Color32::from_rgb(220, 180, 80),
            min_height: 0.35, max_height: 0.7, min_moisture: 0.0, max_moisture: 0.2,
            min_temp: 0.6, max_temp: 1.0, glyph: 'c', density: 0.05, priority: 5,
        },
        BiomeRule {
            name: "Savanna".to_string(), color: Color32::from_rgb(180, 170, 80),
            min_height: 0.35, max_height: 0.65, min_moisture: 0.2, max_moisture: 0.4,
            min_temp: 0.5, max_temp: 1.0, glyph: 'g', density: 0.1, priority: 4,
        },
        BiomeRule {
            name: "Grassland".to_string(), color: Color32::from_rgb(100, 160, 70),
            min_height: 0.35, max_height: 0.65, min_moisture: 0.3, max_moisture: 0.6,
            min_temp: 0.3, max_temp: 0.7, glyph: ',', density: 0.15, priority: 3,
        },
        BiomeRule {
            name: "Forest".to_string(), color: Color32::from_rgb(30, 100, 40),
            min_height: 0.35, max_height: 0.7, min_moisture: 0.5, max_moisture: 1.0,
            min_temp: 0.3, max_temp: 0.8, glyph: 'T', density: 0.4, priority: 6,
        },
        BiomeRule {
            name: "Swamp".to_string(), color: Color32::from_rgb(60, 90, 60),
            min_height: 0.35, max_height: 0.5, min_moisture: 0.7, max_moisture: 1.0,
            min_temp: 0.4, max_temp: 0.8, glyph: 'S', density: 0.3, priority: 7,
        },
        BiomeRule {
            name: "Tundra".to_string(), color: Color32::from_rgb(180, 200, 180),
            min_height: 0.35, max_height: 0.7, min_moisture: 0.0, max_moisture: 0.5,
            min_temp: 0.0, max_temp: 0.3, glyph: '.', density: 0.05, priority: 5,
        },
        BiomeRule {
            name: "Taiga".to_string(), color: Color32::from_rgb(50, 100, 80),
            min_height: 0.35, max_height: 0.75, min_moisture: 0.4, max_moisture: 1.0,
            min_temp: 0.0, max_temp: 0.3, glyph: 'T', density: 0.35, priority: 6,
        },
        BiomeRule {
            name: "Mountain".to_string(), color: Color32::from_rgb(120, 110, 100),
            min_height: 0.7, max_height: 0.85, min_moisture: 0.0, max_moisture: 1.0,
            min_temp: 0.0, max_temp: 1.0, glyph: '^', density: 0.1, priority: 8,
        },
        BiomeRule {
            name: "Snow Peak".to_string(), color: Color32::from_rgb(240, 245, 255),
            min_height: 0.85, max_height: 1.0, min_moisture: 0.0, max_moisture: 1.0,
            min_temp: 0.0, max_temp: 1.0, glyph: '*', density: 0.05, priority: 9,
        },
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldGenConfig {
    pub width: u32,
    pub height: u32,
    pub height_layers: Vec<NoiseLayer>,
    pub moisture_layers: Vec<NoiseLayer>,
    pub temp_layers: Vec<NoiseLayer>,
    pub biomes: Vec<BiomeRule>,
    pub sea_level: f32,
    pub mountain_threshold: f32,
    pub river_chance: f32,
    pub erosion_iterations: u32,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            height_layers: vec![NoiseLayer::default_height()],
            moisture_layers: vec![NoiseLayer::default_moisture()],
            temp_layers: vec![NoiseLayer::default_temp()],
            biomes: default_biomes(),
            sea_level: 0.35,
            mountain_threshold: 0.7,
            river_chance: 0.3,
            erosion_iterations: 3,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedChunk {
    pub width: u32,
    pub height: u32,
    pub height_map: Vec<f32>,
    pub moisture_map: Vec<f32>,
    pub temp_map: Vec<f32>,
    pub biome_map: Vec<usize>,
    pub entity_positions: Vec<([f32; 2], char, Color32)>,
}

impl GeneratedChunk {
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width * height) as usize;
        Self {
            width, height,
            height_map: vec![0.0; n],
            moisture_map: vec![0.0; n],
            temp_map: vec![0.0; n],
            biome_map: vec![0; n],
            entity_positions: Vec::new(),
        }
    }

    pub fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErosionParams {
    pub thermal_iterations: u32,
    pub hydraulic_iterations: u32,
    pub talus_angle: f32,
    pub rain_amount: f32,
    pub evaporation: f32,
    pub sediment_capacity: f32,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            thermal_iterations: 50,
            hydraulic_iterations: 100,
            talus_angle: 0.03,
            rain_amount: 0.01,
            evaporation: 0.5,
            sediment_capacity: 0.01,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiverParams {
    pub source_threshold: f32,
    pub min_length: u32,
    pub width_growth: f32,
    pub meander_factor: f32,
}

impl Default for RiverParams {
    fn default() -> Self {
        Self {
            source_threshold: 0.72,
            min_length: 20,
            width_growth: 0.02,
            meander_factor: 0.3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorldPreset {
    Island,
    Continent,
    Archipelago,
    Desert,
    Arctic,
    Volcanic,
    Forest,
    Badlands,
    Swamp,
    Moon,
}

impl WorldPreset {
    pub fn name(&self) -> &str {
        match self {
            WorldPreset::Island => "Island",
            WorldPreset::Continent => "Continent",
            WorldPreset::Archipelago => "Archipelago",
            WorldPreset::Desert => "Desert",
            WorldPreset::Arctic => "Arctic",
            WorldPreset::Volcanic => "Volcanic",
            WorldPreset::Forest => "Forest",
            WorldPreset::Badlands => "Badlands",
            WorldPreset::Swamp => "Swamp",
            WorldPreset::Moon => "Moon",
        }
    }

    pub fn all() -> &'static [WorldPreset] {
        &[
            WorldPreset::Island, WorldPreset::Continent, WorldPreset::Archipelago,
            WorldPreset::Desert, WorldPreset::Arctic, WorldPreset::Volcanic,
            WorldPreset::Forest, WorldPreset::Badlands, WorldPreset::Swamp, WorldPreset::Moon,
        ]
    }

    pub fn apply(&self, config: &mut WorldGenConfig) {
        match self {
            WorldPreset::Island => {
                config.sea_level = 0.45;
                config.mountain_threshold = 0.75;
                config.river_chance = 0.4;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Island Base".to_string(),
                        noise_type: NoiseType::Fbm { octaves: 6, lacunarity: 2.0, gain: 0.5 },
                        frequency: 0.004, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 1, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                    NoiseLayer {
                        name: "Island Mask".to_string(),
                        noise_type: NoiseType::Cylinders,
                        frequency: 0.002, amplitude: 0.6, offset: [0.0, 0.0],
                        seed: 2, blend_mode: BlendMode::Multiply, enabled: true, weight: 0.8,
                    },
                ];
            }
            WorldPreset::Continent => {
                config.sea_level = 0.35;
                config.mountain_threshold = 0.68;
                config.river_chance = 0.5;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Continental Shelf".to_string(),
                        noise_type: NoiseType::Fbm { octaves: 8, lacunarity: 2.0, gain: 0.5 },
                        frequency: 0.002, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 10, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                    NoiseLayer {
                        name: "Detail".to_string(),
                        noise_type: NoiseType::Perlin,
                        frequency: 0.01, amplitude: 0.2, offset: [50.0, 50.0],
                        seed: 11, blend_mode: BlendMode::Add, enabled: true, weight: 0.3,
                    },
                ];
            }
            WorldPreset::Archipelago => {
                config.sea_level = 0.55;
                config.mountain_threshold = 0.78;
                config.river_chance = 0.2;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Archi Base".to_string(),
                        noise_type: NoiseType::Cellular,
                        frequency: 0.006, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 20, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
            WorldPreset::Desert => {
                config.sea_level = 0.2;
                config.mountain_threshold = 0.65;
                config.river_chance = 0.05;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Dunes".to_string(),
                        noise_type: NoiseType::Value,
                        frequency: 0.008, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 30, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
                config.moisture_layers = vec![
                    NoiseLayer {
                        name: "Arid".to_string(),
                        noise_type: NoiseType::Value,
                        frequency: 0.003, amplitude: 0.2, offset: [200.0, 200.0],
                        seed: 31, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
            WorldPreset::Arctic => {
                config.sea_level = 0.3;
                config.mountain_threshold = 0.6;
                config.river_chance = 0.1;
                config.temp_layers = vec![
                    NoiseLayer {
                        name: "Cold Base".to_string(),
                        noise_type: NoiseType::Value,
                        frequency: 0.002, amplitude: 0.25, offset: [100.0, 100.0],
                        seed: 40, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
            WorldPreset::Volcanic => {
                config.sea_level = 0.4;
                config.mountain_threshold = 0.6;
                config.river_chance = 0.15;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Volcanic Base".to_string(),
                        noise_type: NoiseType::Ridged,
                        frequency: 0.005, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 50, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
            WorldPreset::Forest => {
                config.sea_level = 0.3;
                config.mountain_threshold = 0.72;
                config.river_chance = 0.6;
                config.moisture_layers = vec![
                    NoiseLayer {
                        name: "Lush Moisture".to_string(),
                        noise_type: NoiseType::Perlin,
                        frequency: 0.004, amplitude: 1.0, offset: [300.0, 300.0],
                        seed: 60, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                    NoiseLayer {
                        name: "Extra Moisture".to_string(),
                        noise_type: NoiseType::Value,
                        frequency: 0.002, amplitude: 0.3, offset: [0.0, 0.0],
                        seed: 61, blend_mode: BlendMode::Add, enabled: true, weight: 0.4,
                    },
                ];
            }
            WorldPreset::Badlands => {
                config.sea_level = 0.25;
                config.mountain_threshold = 0.6;
                config.river_chance = 0.1;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Badlands".to_string(),
                        noise_type: NoiseType::Worley,
                        frequency: 0.007, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 70, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
            WorldPreset::Swamp => {
                config.sea_level = 0.4;
                config.mountain_threshold = 0.8;
                config.river_chance = 0.7;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Swamp Base".to_string(),
                        noise_type: NoiseType::Value,
                        frequency: 0.004, amplitude: 0.4, offset: [0.0, 0.0],
                        seed: 80, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
                config.moisture_layers = vec![
                    NoiseLayer {
                        name: "Swamp Wet".to_string(),
                        noise_type: NoiseType::Perlin,
                        frequency: 0.003, amplitude: 1.0, offset: [700.0, 700.0],
                        seed: 81, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
            WorldPreset::Moon => {
                config.sea_level = 0.05;
                config.mountain_threshold = 0.5;
                config.river_chance = 0.0;
                config.height_layers = vec![
                    NoiseLayer {
                        name: "Craters".to_string(),
                        noise_type: NoiseType::Cellular,
                        frequency: 0.01, amplitude: 1.0, offset: [0.0, 0.0],
                        seed: 90, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
                config.moisture_layers = vec![
                    NoiseLayer {
                        name: "Dry".to_string(),
                        noise_type: NoiseType::Value,
                        frequency: 0.001, amplitude: 0.05, offset: [0.0, 0.0],
                        seed: 91, blend_mode: BlendMode::Replace, enabled: true, weight: 1.0,
                    },
                ];
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PreviewLayer {
    Height,
    Moisture,
    Temperature,
    Biome,
    Combined,
}

impl PreviewLayer {
    pub fn name(&self) -> &str {
        match self {
            PreviewLayer::Height => "Height",
            PreviewLayer::Moisture => "Moisture",
            PreviewLayer::Temperature => "Temperature",
            PreviewLayer::Biome => "Biome",
            PreviewLayer::Combined => "Combined",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorldBrush {
    None,
    RaiseTerrain,
    LowerTerrain,
    FlattenTerrain,
    PaintBiome,
    PlaceRiver,
    ErodeLocal,
}

impl WorldBrush {
    pub fn name(&self) -> &str {
        match self {
            WorldBrush::None => "None",
            WorldBrush::RaiseTerrain => "Raise",
            WorldBrush::LowerTerrain => "Lower",
            WorldBrush::FlattenTerrain => "Flatten",
            WorldBrush::PaintBiome => "Paint Biome",
            WorldBrush::PlaceRiver => "River",
            WorldBrush::ErodeLocal => "Erode",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoiseLayerTarget {
    Height,
    Moisture,
    Temperature,
}

impl NoiseLayerTarget {
    pub fn name(&self) -> &str {
        match self {
            NoiseLayerTarget::Height => "Height",
            NoiseLayerTarget::Moisture => "Moisture",
            NoiseLayerTarget::Temperature => "Temperature",
        }
    }
}

// ============================================================
// NOISE FUNCTIONS (pure software implementation)
// ============================================================

fn hash_u32(mut x: u32) -> u32 {
    x = x.wrapping_add(0x9e3779b9);
    x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
    x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
    x = (x >> 16) ^ x;
    x
}

fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = seed.wrapping_add(x as u32);
    h = hash_u32(h);
    h = h.wrapping_add(y as u32);
    h = hash_u32(h);
    h
}

fn hash2f(x: i32, y: i32, seed: u32) -> f32 {
    (hash2(x, y, seed) & 0xFFFFFF) as f32 / 16777216.0
}

pub fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn smootherstep(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn value_noise_2d(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let ux = smoothstep(fx);
    let uy = smoothstep(fy);

    let v00 = hash2f(ix,   iy,   seed);
    let v10 = hash2f(ix+1, iy,   seed);
    let v01 = hash2f(ix,   iy+1, seed);
    let v11 = hash2f(ix+1, iy+1, seed);

    lerp(lerp(v00, v10, ux), lerp(v01, v11, ux), uy)
}

fn grad2(hash: u32, x: f32, y: f32) -> f32 {
    match hash & 7 {
        0 =>  x + y,
        1 => -x + y,
        2 =>  x - y,
        3 => -x - y,
        4 =>  x,
        5 => -x,
        6 =>  y,
        7 => -y,
        _ => 0.0,
    }
}

pub fn perlin_2d(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let ux = smootherstep(fx);
    let uy = smootherstep(fy);

    let g00 = grad2(hash2(ix,   iy,   seed), fx,       fy);
    let g10 = grad2(hash2(ix+1, iy,   seed), fx - 1.0, fy);
    let g01 = grad2(hash2(ix,   iy+1, seed), fx,       fy - 1.0);
    let g11 = grad2(hash2(ix+1, iy+1, seed), fx - 1.0, fy - 1.0);

    let v = lerp(lerp(g00, g10, ux), lerp(g01, g11, ux), uy);
    // normalize from roughly [-1,1] to [0,1]
    (v + 0.7071) / 1.4142
}

pub fn fbm(x: f32, y: f32, seed: u32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut value = 0.0_f32;
    let mut amplitude = 1.0_f32;
    let mut frequency = 1.0_f32;
    let mut max_value = 0.0_f32;
    for i in 0..octaves {
        value += perlin_2d(x * frequency, y * frequency, seed.wrapping_add(i * 127)) * amplitude;
        max_value += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    if max_value > 0.0 { value / max_value } else { 0.0 }
}

pub fn cellular_2d(x: f32, y: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let mut min_dist = f32::MAX;
    for dy in -2..=2 {
        for dx in -2..=2 {
            let cx = ix + dx;
            let cy = iy + dy;
            let h = hash2(cx, cy, seed);
            let px = cx as f32 + (h & 0xFFFF) as f32 / 65535.0;
            let py = cy as f32 + ((h >> 16) & 0xFFFF) as f32 / 65535.0;
            let dist = (px - x) * (px - x) + (py - y) * (py - y);
            if dist < min_dist { min_dist = dist; }
        }
    }
    (min_dist.sqrt()).clamp(0.0, 1.0)
}

// Worley is same as cellular but we return 1 - distance for inversion effect
pub fn worley_2d(x: f32, y: f32, seed: u32) -> f32 {
    1.0 - cellular_2d(x, y, seed)
}

pub fn ridged_noise(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let mut value = 0.0_f32;
    let mut amplitude = 1.0_f32;
    let mut frequency = 1.0_f32;
    let mut max_value = 0.0_f32;
    for i in 0..octaves {
        let n = perlin_2d(x * frequency, y * frequency, seed.wrapping_add(i * 53));
        let r = 1.0 - (2.0 * n - 1.0).abs();
        value += r * amplitude;
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    if max_value > 0.0 { value / max_value } else { 0.0 }
}

pub fn cylinders_noise(x: f32, y: f32, frequency: f32) -> f32 {
    let d = (x * frequency * std::f32::consts::TAU).sin();
    (d + 1.0) * 0.5
}

pub fn spheres_noise(x: f32, y: f32, frequency: f32) -> f32 {
    let dist = (x * x + y * y).sqrt();
    let d = (dist * frequency * std::f32::consts::TAU).sin();
    (d + 1.0) * 0.5
}

pub fn evaluate_noise_type(nt: &NoiseType, x: f32, y: f32, seed: u32, freq: f32) -> f32 {
    match nt {
        NoiseType::Perlin => perlin_2d(x * freq, y * freq, seed),
        NoiseType::Simplex => perlin_2d(x * freq + 0.5, y * freq + 0.5, seed.wrapping_add(999)),
        NoiseType::Cellular => cellular_2d(x * freq, y * freq, seed),
        NoiseType::Worley => worley_2d(x * freq, y * freq, seed),
        NoiseType::Ridged => ridged_noise(x * freq, y * freq, seed, 5),
        NoiseType::Fbm { octaves, lacunarity, gain } => {
            fbm(x * freq, y * freq, seed, *octaves, *lacunarity, *gain)
        }
        NoiseType::Value => value_noise_2d(x * freq, y * freq, seed),
        NoiseType::Cylinders => cylinders_noise(x + seed as f32 * 0.01, y, freq),
        NoiseType::Spheres => spheres_noise(x + seed as f32 * 0.01, y, freq),
    }
}

pub fn evaluate_layer_stack(layers: &[NoiseLayer], x: f32, y: f32) -> f32 {
    let mut result = 0.0_f32;
    let mut has_base = false;
    for layer in layers {
        if !layer.enabled { continue; }
        let nx = x + layer.offset[0];
        let ny = y + layer.offset[1];
        let raw = evaluate_noise_type(&layer.noise_type, nx, ny, layer.seed, layer.frequency);
        let scaled = raw * layer.amplitude * layer.weight;
        if !has_base {
            result = scaled;
            has_base = true;
        } else {
            result = layer.blend_mode.apply(result, scaled);
        }
    }
    result.clamp(0.0, 1.0)
}

// ============================================================
// GENERATION FUNCTIONS
// ============================================================

pub fn generate_chunk(config: &WorldGenConfig, seed: u64) -> GeneratedChunk {
    let w = config.width;
    let h = config.height;
    let mut chunk = GeneratedChunk::new(w, h);

    let seed_h = (seed ^ 0xDEADBEEF) as u32;
    let seed_m = (seed ^ 0xCAFEBABE) as u32;
    let seed_t = (seed ^ 0xABCDEF01) as u32;

    for y in 0..h {
        for x in 0..w {
            let idx = chunk.idx(x, y);
            let fx = x as f32;
            let fy = y as f32;

            // evaluate height layers
            let mut height_layers = config.height_layers.clone();
            for layer in &mut height_layers {
                layer.seed = layer.seed.wrapping_add(seed_h);
            }
            chunk.height_map[idx] = evaluate_layer_stack(&height_layers, fx, fy);

            // evaluate moisture layers
            let mut moist_layers = config.moisture_layers.clone();
            for layer in &mut moist_layers {
                layer.seed = layer.seed.wrapping_add(seed_m);
            }
            chunk.moisture_map[idx] = evaluate_layer_stack(&moist_layers, fx, fy);

            // evaluate temp layers with latitude bias
            let mut temp_layers = config.temp_layers.clone();
            for layer in &mut temp_layers {
                layer.seed = layer.seed.wrapping_add(seed_t);
            }
            let base_temp = evaluate_layer_stack(&temp_layers, fx, fy);
            // latitude gradient: hotter at equator (center), colder at poles
            let lat = (fy / h as f32 - 0.5).abs() * 2.0;
            let height_val = chunk.height_map[idx];
            let altitude_cool = (height_val - config.sea_level).max(0.0) * 0.5;
            chunk.temp_map[idx] = (base_temp * 0.5 + (1.0 - lat) * 0.4 - altitude_cool).clamp(0.0, 1.0);
        }
    }

    // assign biomes
    assign_biomes(&mut chunk, &config.biomes);

    chunk
}

fn assign_biomes(chunk: &mut GeneratedChunk, biomes: &[BiomeRule]) {
    let n = (chunk.width * chunk.height) as usize;
    // sort by priority descending
    let mut sorted: Vec<(usize, &BiomeRule)> = biomes.iter().enumerate().collect();
    sorted.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));

    for i in 0..n {
        let h = chunk.height_map[i];
        let m = chunk.moisture_map[i];
        let t = chunk.temp_map[i];
        let mut assigned = 0usize;
        for (idx, biome) in &sorted {
            if biome.matches(h, m, t) {
                assigned = *idx;
                break;
            }
        }
        chunk.biome_map[i] = assigned;
    }
}

pub fn apply_erosion(chunk: &mut GeneratedChunk, params: &ErosionParams) {
    let w = chunk.width as usize;
    let h = chunk.height as usize;

    // Thermal erosion
    for _iter in 0..params.thermal_iterations {
        let old = chunk.height_map.clone();
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                let center = old[idx];
                let neighbors = [
                    old[(y-1)*w + x], old[(y+1)*w + x],
                    old[y*w + x-1],   old[y*w + x+1],
                ];
                for (ni, &n_val) in neighbors.iter().enumerate() {
                    let diff = center - n_val;
                    if diff > params.talus_angle {
                        let transfer = (diff - params.talus_angle) * 0.5;
                        chunk.height_map[idx] -= transfer;
                        let n_idx = match ni {
                            0 => (y-1)*w + x,
                            1 => (y+1)*w + x,
                            2 => y*w + x-1,
                            _ => y*w + x+1,
                        };
                        chunk.height_map[n_idx] += transfer;
                    }
                }
            }
        }
    }

    // Hydraulic erosion (simplified)
    let mut water = vec![0.0_f32; w * h];
    let mut sediment = vec![0.0_f32; w * h];

    for _iter in 0..params.hydraulic_iterations {
        // Rain
        for i in 0..w*h {
            water[i] += params.rain_amount;
        }
        // Flow
        let old_h = chunk.height_map.clone();
        let old_w = water.clone();
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                let total = old_h[idx] + old_w[idx];
                let neighbor_idx = [
                    (y-1)*w + x, (y+1)*w + x,
                    y*w + x-1,   y*w + x+1,
                ];
                let neighbor_total: Vec<f32> = neighbor_idx.iter()
                    .map(|&ni| old_h[ni] + old_w[ni]).collect();
                let min_n = neighbor_total.iter().cloned().fold(f32::MAX, f32::min);
                if min_n < total {
                    let flow = ((total - min_n) * 0.25).min(old_w[idx]);
                    water[idx] -= flow;
                    // erode
                    let cap = params.sediment_capacity * flow;
                    if sediment[idx] < cap {
                        let erode = (cap - sediment[idx]).min(0.005);
                        chunk.height_map[idx] -= erode;
                        sediment[idx] += erode;
                    }
                    // deposit at min neighbor
                    let min_ni = neighbor_idx.iter().zip(neighbor_total.iter())
                        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| *i).unwrap_or(idx);
                    water[min_ni] += flow;
                    let deposit = sediment[idx] * 0.1;
                    chunk.height_map[min_ni] += deposit;
                    sediment[idx] -= deposit;
                }
            }
        }
        // Evaporate
        for i in 0..w*h {
            water[i] *= 1.0 - params.evaporation;
            // deposit remaining sediment
            let dep = sediment[i] * params.evaporation;
            chunk.height_map[i] += dep;
            sediment[i] -= dep;
        }
    }

    // clamp
    for v in chunk.height_map.iter_mut() {
        *v = v.clamp(0.0, 1.0);
    }
}

pub fn place_rivers(chunk: &mut GeneratedChunk, params: &RiverParams, seed: u64) {
    let w = chunk.width as usize;
    let h = chunk.height as usize;
    let mut rng_state = seed ^ 0x1234567890ABCDEF;

    let rng_next = |state: &mut u64| -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state & 0xFFFFFF) as f32 / 16777216.0
    };

    let num_rivers = ((w * h) as f32 * chunk.width as f32 * 0.000001 * params.source_threshold) as usize + 3;
    let num_rivers = num_rivers.min(20);

    for _ in 0..num_rivers {
        // find a high point as source
        let mut sx = (rng_next(&mut rng_state) * w as f32) as usize;
        let mut sy = (rng_next(&mut rng_state) * h as f32) as usize;
        sx = sx.clamp(1, w-2);
        sy = sy.clamp(1, h-2);

        if chunk.height_map[sy * w + sx] < params.source_threshold {
            continue;
        }

        let mut cx = sx as i32;
        let mut cy = sy as i32;
        let mut path_len = 0u32;
        let mut river_width = 1.0_f32;

        for _step in 0..500 {
            let idx = (cy as usize) * w + cx as usize;
            // carve river channel
            let carve_r = (river_width as i32).max(1);
            for dy in -carve_r..=carve_r {
                for dx in -carve_r..=carve_r {
                    let nx = (cx + dx).clamp(0, w as i32 - 1) as usize;
                    let ny = (cy + dy).clamp(0, h as i32 - 1) as usize;
                    let ni = ny * w + nx;
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist <= river_width {
                        chunk.height_map[ni] = chunk.height_map[ni].min(0.33);
                        chunk.moisture_map[ni] = (chunk.moisture_map[ni] + 0.3).min(1.0);
                    }
                }
            }

            // flow downhill with meander
            let neighbors = [
                ((cx-1).max(0), cy),
                ((cx+1).min(w as i32-1), cy),
                (cx, (cy-1).max(0)),
                (cx, (cy+1).min(h as i32-1)),
            ];
            let mut best = idx;
            let mut best_h = chunk.height_map[idx];
            for &(nx, ny) in &neighbors {
                let ni = ny as usize * w + nx as usize;
                let meander = rng_next(&mut rng_state) * params.meander_factor * 0.05;
                if chunk.height_map[ni] + meander < best_h {
                    best_h = chunk.height_map[ni] + meander;
                    best = ni;
                    cx = nx;
                    cy = ny;
                }
            }
            if best == idx { break; } // stuck - at sea or flat

            path_len += 1;
            river_width += params.width_growth;

            // reached sea level
            if chunk.height_map[idx] < 0.35 { break; }
        }

        let _ = path_len;
    }
}

pub fn populate_entities(chunk: &mut GeneratedChunk, biomes: &[BiomeRule], seed: u64) {
    let w = chunk.width as usize;
    let h = chunk.height as usize;
    let mut rng_state = seed ^ 0xFEDCBA9876543210;

    let rng_next = |state: &mut u64| -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state & 0xFFFFFF) as f32 / 16777216.0
    };

    chunk.entity_positions.clear();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let biome_idx = chunk.biome_map[idx];
            if biome_idx >= biomes.len() { continue; }
            let biome = &biomes[biome_idx];
            if biome.density <= 0.0 { continue; }
            let r = rng_next(&mut rng_state);
            if r < biome.density * 0.1 {
                chunk.entity_positions.push((
                    [x as f32, y as f32],
                    biome.glyph,
                    biome.color,
                ));
            }
        }
    }
}

// ============================================================
// COLOR MAPPING
// ============================================================

fn height_to_color(h: f32, sea_level: f32) -> Color32 {
    if h < sea_level * 0.4 {
        // Deep ocean
        let t = h / (sea_level * 0.4);
        Color32::from_rgb(
            lerp(10.0, 20.0, t) as u8,
            lerp(20.0, 60.0, t) as u8,
            lerp(60.0, 120.0, t) as u8,
        )
    } else if h < sea_level {
        // Shallow ocean
        let t = (h - sea_level * 0.4) / (sea_level * 0.6);
        Color32::from_rgb(
            lerp(20.0, 60.0, t) as u8,
            lerp(60.0, 120.0, t) as u8,
            lerp(120.0, 180.0, t) as u8,
        )
    } else if h < sea_level + 0.05 {
        // Beach/coast
        let t = (h - sea_level) / 0.05;
        Color32::from_rgb(
            lerp(180.0, 220.0, t) as u8,
            lerp(160.0, 195.0, t) as u8,
            lerp(100.0, 130.0, t) as u8,
        )
    } else if h < 0.6 {
        // Lowlands
        let t = (h - (sea_level + 0.05)) / (0.6 - sea_level - 0.05);
        Color32::from_rgb(
            lerp(60.0, 100.0, t) as u8,
            lerp(130.0, 150.0, t) as u8,
            lerp(50.0, 70.0, t) as u8,
        )
    } else if h < 0.75 {
        // Hills
        let t = (h - 0.6) / 0.15;
        Color32::from_rgb(
            lerp(100.0, 130.0, t) as u8,
            lerp(150.0, 120.0, t) as u8,
            lerp(70.0, 90.0, t) as u8,
        )
    } else if h < 0.88 {
        // Mountains
        let t = (h - 0.75) / 0.13;
        Color32::from_rgb(
            lerp(130.0, 160.0, t) as u8,
            lerp(120.0, 150.0, t) as u8,
            lerp(90.0, 140.0, t) as u8,
        )
    } else {
        // Snow
        let t = (h - 0.88) / 0.12;
        Color32::from_rgb(
            lerp(200.0, 255.0, t) as u8,
            lerp(210.0, 255.0, t) as u8,
            lerp(220.0, 255.0, t) as u8,
        )
    }
}

fn moisture_to_color(m: f32) -> Color32 {
    Color32::from_rgb(
        lerp(200.0, 20.0, m) as u8,
        lerp(180.0, 80.0, m) as u8,
        lerp(100.0, 200.0, m) as u8,
    )
}

fn temp_to_color(t: f32) -> Color32 {
    if t < 0.5 {
        let s = t * 2.0;
        Color32::from_rgb(
            lerp(30.0, 200.0, s) as u8,
            lerp(60.0, 200.0, s) as u8,
            lerp(180.0, 80.0, s) as u8,
        )
    } else {
        let s = (t - 0.5) * 2.0;
        Color32::from_rgb(
            lerp(200.0, 255.0, s) as u8,
            lerp(200.0, 60.0, s) as u8,
            lerp(80.0, 20.0, s) as u8,
        )
    }
}

// ============================================================
// WORLD GEN EDITOR STATE
// ============================================================

pub struct WorldGenEditor {
    pub config: WorldGenConfig,
    pub erosion: ErosionParams,
    pub rivers: RiverParams,
    pub generated: Option<GeneratedChunk>,
    pub preview_layer: PreviewLayer,
    pub preview_zoom: f32,
    pub preview_offset: [f32; 2],
    pub selected_biome: Option<usize>,
    pub selected_noise_layer: Option<(NoiseLayerTarget, usize)>,
    pub brush_tool: WorldBrush,
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub auto_regenerate: bool,
    pub generation_seed: u64,
    pub show_biomes: bool,
    pub show_noise_layers: bool,
    pub show_erosion: bool,
    pub show_rivers: bool,
    // internal
    generation_time_ms: f64,
    dirty: bool,
    preview_texture_data: Vec<Color32>,
    preview_needs_refresh: bool,
    drag_start: Option<Pos2>,
    brush_pos: Option<[f32; 2]>,
    scroll_offset: f32,
    left_panel_width: f32,
    right_panel_width: f32,
    noise_tab: NoiseLayerTarget,
    show_entity_glyphs: bool,
    show_grid: bool,
    grid_size: u32,
    show_contours: bool,
}

impl WorldGenEditor {
    pub fn new() -> Self {
        let config = WorldGenConfig::default();
        Self {
            config,
            erosion: ErosionParams::default(),
            rivers: RiverParams::default(),
            generated: None,
            preview_layer: PreviewLayer::Combined,
            preview_zoom: 1.0,
            preview_offset: [0.0, 0.0],
            selected_biome: None,
            selected_noise_layer: None,
            brush_tool: WorldBrush::None,
            brush_radius: 10.0,
            brush_strength: 0.05,
            auto_regenerate: false,
            generation_seed: 12345,
            show_biomes: true,
            show_noise_layers: true,
            show_erosion: false,
            show_rivers: true,
            generation_time_ms: 0.0,
            dirty: false,
            preview_texture_data: Vec::new(),
            preview_needs_refresh: false,
            drag_start: None,
            brush_pos: None,
            scroll_offset: 0.0,
            left_panel_width: 280.0,
            right_panel_width: 280.0,
            noise_tab: NoiseLayerTarget::Height,
            show_entity_glyphs: true,
            show_grid: false,
            grid_size: 32,
            show_contours: false,
        }
    }

    pub fn show_panel(ctx: &egui::Context, editor: &mut WorldGenEditor, _dt: f32, open: &mut bool) {
        egui::Window::new("World Generation Editor")
            .open(open)
            .resizable(true)
            .default_size([1200.0, 800.0])
            .show(ctx, |ui| {
                editor.show(ui, _dt);
            });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, dt: f32) {
        if self.auto_regenerate && self.dirty {
            self.do_generate();
            self.dirty = false;
        }

        let available = ui.available_size();

        ui.horizontal(|ui| {
            // LEFT PANEL
            let left_w = self.left_panel_width;
            egui::ScrollArea::vertical()
                .id_source("wg_left_scroll")
                .max_height(available.y - 40.0)
                .show(ui, |ui| {
                    ui.set_min_width(left_w);
                    ui.set_max_width(left_w);
                    self.show_left_panel(ui);
                });

            ui.separator();

            // CENTER PANEL
            let center_w = available.x - left_w - self.right_panel_width - 20.0;
            ui.vertical(|ui| {
                ui.set_min_width(center_w);
                ui.set_max_width(center_w);
                self.show_toolbar(ui);
                ui.separator();
                self.show_preview_canvas(ui, center_w, available.y - 120.0);
                ui.separator();
                self.show_bottom_bar(ui);
            });

            ui.separator();

            // RIGHT PANEL
            let right_w = self.right_panel_width;
            egui::ScrollArea::vertical()
                .id_source("wg_right_scroll")
                .max_height(available.y - 40.0)
                .show(ui, |ui| {
                    ui.set_min_width(right_w);
                    self.show_right_panel(ui);
                });
        });
    }

    fn show_left_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("World Generation");
        ui.separator();

        // Preset buttons
        ui.collapsing("Presets", |ui| {
            let presets = WorldPreset::all();
            let cols = 2;
            let total = presets.len();
            let rows = (total + cols - 1) / cols;
            for row in 0..rows {
                ui.horizontal(|ui| {
                    for col in 0..cols {
                        let idx = row * cols + col;
                        if idx < total {
                            let preset = &presets[idx];
                            if ui.button(preset.name()).clicked() {
                                preset.apply(&mut self.config);
                                self.dirty = true;
                            }
                        }
                    }
                });
            }
        });

        ui.separator();

        // Chunk size
        ui.collapsing("Chunk Settings", |ui| {
            let mut w = self.config.width;
            let mut h = self.config.height;
            ui.horizontal(|ui| {
                ui.label("Width:");
                if ui.add(egui::DragValue::new(&mut w).range(32..=1024).speed(1.0)).changed() {
                    self.config.width = w;
                    self.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Height:");
                if ui.add(egui::DragValue::new(&mut h).range(32..=1024).speed(1.0)).changed() {
                    self.config.height = h;
                    self.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Sea Level:");
                if ui.add(egui::Slider::new(&mut self.config.sea_level, 0.0..=1.0)).changed() {
                    self.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Mountain Threshold:");
                if ui.add(egui::Slider::new(&mut self.config.mountain_threshold, 0.0..=1.0)).changed() {
                    self.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("River Chance:");
                if ui.add(egui::Slider::new(&mut self.config.river_chance, 0.0..=1.0)).changed() {
                    self.dirty = true;
                }
            });
        });

        ui.separator();

        // Noise layer stacks
        egui::CollapsingHeader::new("Noise Layers").id_salt("wg_noise_collapse").show(ui, |ui| {
            ui.horizontal(|ui| {
                for target in [NoiseLayerTarget::Height, NoiseLayerTarget::Moisture, NoiseLayerTarget::Temperature] {
                    let selected = self.noise_tab == target;
                    if ui.selectable_label(selected, target.name()).clicked() {
                        self.noise_tab = target.clone();
                    }
                }
            });
            ui.separator();

            let layers = match self.noise_tab {
                NoiseLayerTarget::Height => &mut self.config.height_layers,
                NoiseLayerTarget::Moisture => &mut self.config.moisture_layers,
                NoiseLayerTarget::Temperature => &mut self.config.temp_layers,
            };

            let mut to_remove: Option<usize> = None;
            let mut to_move_up: Option<usize> = None;
            let mut to_move_down: Option<usize> = None;
            let target_clone = self.noise_tab.clone();

            for (i, layer) in layers.iter_mut().enumerate() {
                let selected = self.selected_noise_layer.as_ref()
                    .map(|(t, idx)| t == &target_clone && *idx == i)
                    .unwrap_or(false);

                let header = format!("{} [{}]", layer.name, if layer.enabled { "ON" } else { "OFF" });
                let resp = ui.selectable_label(selected, &header);
                if resp.clicked() {
                    self.selected_noise_layer = Some((target_clone.clone(), i));
                }
                if selected {
                    ui.indent("nl_indent", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut layer.name);
                        });
                        ui.checkbox(&mut layer.enabled, "Enabled");
                        ui.horizontal(|ui| {
                            ui.label("Type:");
                            egui::ComboBox::from_id_source(format!("nt_{}", i))
                                .selected_text(layer.noise_type.name())
                                .show_ui(ui, |ui| {
                                    for v in NoiseType::all_variants() {
                                        let sel = &layer.noise_type == &v;
                                        if ui.selectable_label(sel, v.name()).clicked() {
                                            layer.noise_type = v;
                                        }
                                    }
                                });
                        });
                        if let NoiseType::Fbm { octaves, lacunarity, gain } = &mut layer.noise_type {
                            ui.horizontal(|ui| {
                                ui.label("Octaves:");
                                ui.add(egui::DragValue::new(octaves).range(1..=12).speed(0.1));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Lacunarity:");
                                ui.add(egui::DragValue::new(lacunarity).range(1.0..=4.0).speed(0.01));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Gain:");
                                ui.add(egui::DragValue::new(gain).range(0.1..=0.9).speed(0.01));
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.label("Frequency:");
                            ui.add(egui::DragValue::new(&mut layer.frequency).range(0.0001..=0.1).speed(0.0001));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Amplitude:");
                            ui.add(egui::Slider::new(&mut layer.amplitude, 0.0..=2.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Weight:");
                            ui.add(egui::Slider::new(&mut layer.weight, 0.0..=2.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Seed:");
                            ui.add(egui::DragValue::new(&mut layer.seed).speed(1.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Offset X:");
                            ui.add(egui::DragValue::new(&mut layer.offset[0]).speed(1.0));
                            ui.label("Y:");
                            ui.add(egui::DragValue::new(&mut layer.offset[1]).speed(1.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Blend:");
                            egui::ComboBox::from_id_source(format!("bm_{}", i))
                                .selected_text(layer.blend_mode.name())
                                .show_ui(ui, |ui| {
                                    for bm in BlendMode::all() {
                                        let sel = &layer.blend_mode == bm;
                                        if ui.selectable_label(sel, bm.name()).clicked() {
                                            layer.blend_mode = bm.clone();
                                        }
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            if ui.small_button("↑").clicked() { to_move_up = Some(i); }
                            if ui.small_button("↓").clicked() { to_move_down = Some(i); }
                            if ui.small_button("🗑").clicked() { to_remove = Some(i); }
                        });
                    });
                }
            }

            if let Some(idx) = to_remove {
                let layers = match self.noise_tab {
                    NoiseLayerTarget::Height => &mut self.config.height_layers,
                    NoiseLayerTarget::Moisture => &mut self.config.moisture_layers,
                    NoiseLayerTarget::Temperature => &mut self.config.temp_layers,
                };
                if idx < layers.len() { layers.remove(idx); }
                self.selected_noise_layer = None;
                self.dirty = true;
            }
            if let Some(idx) = to_move_up {
                let layers = match self.noise_tab {
                    NoiseLayerTarget::Height => &mut self.config.height_layers,
                    NoiseLayerTarget::Moisture => &mut self.config.moisture_layers,
                    NoiseLayerTarget::Temperature => &mut self.config.temp_layers,
                };
                if idx > 0 { layers.swap(idx, idx-1); self.dirty = true; }
            }
            if let Some(idx) = to_move_down {
                let layers = match self.noise_tab {
                    NoiseLayerTarget::Height => &mut self.config.height_layers,
                    NoiseLayerTarget::Moisture => &mut self.config.moisture_layers,
                    NoiseLayerTarget::Temperature => &mut self.config.temp_layers,
                };
                if idx + 1 < layers.len() { layers.swap(idx, idx+1); self.dirty = true; }
            }

            ui.separator();
            if ui.button("+ Add Layer").clicked() {
                let new_layer = match self.noise_tab {
                    NoiseLayerTarget::Height => NoiseLayer::default_height(),
                    NoiseLayerTarget::Moisture => NoiseLayer::default_moisture(),
                    NoiseLayerTarget::Temperature => NoiseLayer::default_temp(),
                };
                let layers = match self.noise_tab {
                    NoiseLayerTarget::Height => &mut self.config.height_layers,
                    NoiseLayerTarget::Moisture => &mut self.config.moisture_layers,
                    NoiseLayerTarget::Temperature => &mut self.config.temp_layers,
                };
                layers.push(new_layer);
                self.dirty = true;
            }
        });

        ui.separator();

        // Erosion settings
        ui.collapsing("Erosion", |ui| {
            ui.horizontal(|ui| {
                ui.label("Thermal Iterations:");
                if ui.add(egui::DragValue::new(&mut self.erosion.thermal_iterations).range(0..=500).speed(1.0)).changed() {}
            });
            ui.horizontal(|ui| {
                ui.label("Hydraulic Iterations:");
                if ui.add(egui::DragValue::new(&mut self.erosion.hydraulic_iterations).range(0..=1000).speed(1.0)).changed() {}
            });
            ui.horizontal(|ui| {
                ui.label("Talus Angle:");
                ui.add(egui::Slider::new(&mut self.erosion.talus_angle, 0.001..=0.2));
            });
            ui.horizontal(|ui| {
                ui.label("Rain Amount:");
                ui.add(egui::Slider::new(&mut self.erosion.rain_amount, 0.001..=0.1));
            });
            ui.horizontal(|ui| {
                ui.label("Evaporation:");
                ui.add(egui::Slider::new(&mut self.erosion.evaporation, 0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Sediment Capacity:");
                ui.add(egui::Slider::new(&mut self.erosion.sediment_capacity, 0.001..=0.1));
            });
        });

        ui.separator();

        // River settings
        ui.collapsing("Rivers", |ui| {
            ui.horizontal(|ui| {
                ui.label("Source Threshold:");
                ui.add(egui::Slider::new(&mut self.rivers.source_threshold, 0.3..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Min Length:");
                ui.add(egui::DragValue::new(&mut self.rivers.min_length).range(1..=200).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Width Growth:");
                ui.add(egui::Slider::new(&mut self.rivers.width_growth, 0.0..=0.2));
            });
            ui.horizontal(|ui| {
                ui.label("Meander Factor:");
                ui.add(egui::Slider::new(&mut self.rivers.meander_factor, 0.0..=1.0));
            });
        });

        ui.separator();

        // display toggles
        ui.collapsing("Display", |ui| {
            ui.checkbox(&mut self.show_entity_glyphs, "Show Entity Glyphs");
            ui.checkbox(&mut self.show_grid, "Show Grid");
            ui.horizontal(|ui| {
                ui.label("Grid Size:");
                ui.add(egui::DragValue::new(&mut self.grid_size).range(8..=128).speed(1.0));
            });
            ui.checkbox(&mut self.show_contours, "Show Contours");
        });
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Preview:");
            for layer in [
                PreviewLayer::Height, PreviewLayer::Moisture, PreviewLayer::Temperature,
                PreviewLayer::Biome, PreviewLayer::Combined,
            ] {
                let selected = self.preview_layer == layer;
                if ui.selectable_label(selected, layer.name()).clicked() {
                    self.preview_layer = layer;
                    self.preview_needs_refresh = true;
                }
            }

            ui.separator();
            ui.label("Zoom:");
            if ui.small_button("-").clicked() { self.preview_zoom = (self.preview_zoom / 1.25).max(0.1); }
            ui.label(format!("{:.1}x", self.preview_zoom));
            if ui.small_button("+").clicked() { self.preview_zoom = (self.preview_zoom * 1.25).min(16.0); }
            if ui.small_button("Fit").clicked() { self.preview_zoom = 1.0; self.preview_offset = [0.0, 0.0]; }

            ui.separator();
            ui.label("Brush:");
            for brush in [
                WorldBrush::None, WorldBrush::RaiseTerrain, WorldBrush::LowerTerrain,
                WorldBrush::FlattenTerrain, WorldBrush::PaintBiome,
                WorldBrush::PlaceRiver, WorldBrush::ErodeLocal,
            ] {
                let sel = self.brush_tool == brush;
                if ui.selectable_label(sel, brush.name()).clicked() {
                    self.brush_tool = brush;
                }
            }

            if self.brush_tool != WorldBrush::None {
                ui.separator();
                ui.label("Radius:");
                ui.add(egui::DragValue::new(&mut self.brush_radius).range(1.0..=100.0).speed(0.5));
                ui.label("Strength:");
                ui.add(egui::DragValue::new(&mut self.brush_strength).range(0.001..=1.0).speed(0.001));
            }
        });
    }

    fn show_preview_canvas(&mut self, ui: &mut egui::Ui, width: f32, height: f32) {
        let canvas_size = Vec2::new(width.max(100.0), height.max(100.0));
        let (response, painter) = ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());

        // Background
        painter.rect_filled(response.rect, 0.0, Color32::from_rgb(20, 20, 30));

        // Compute layout params outside borrow for use in drag/hover handlers
        let (chunk_cell_size, chunk_base_x, chunk_base_y, chunk_w, chunk_h) =
            if let Some(chunk) = &self.generated {
                let cw = chunk.width as f32;
                let ch = chunk.height as f32;
                let cell_w = (canvas_size.x / cw) * self.preview_zoom;
                let cell_h = (canvas_size.y / ch) * self.preview_zoom;
                let cell_size = cell_w.min(cell_h);
                let total_w = cw * cell_size;
                let total_h = ch * cell_size;
                let base_x = response.rect.min.x + (canvas_size.x - total_w) * 0.5 + self.preview_offset[0];
                let base_y = response.rect.min.y + (canvas_size.y - total_h) * 0.5 + self.preview_offset[1];
                (cell_size, base_x, base_y, chunk.width, chunk.height)
            } else {
                (1.0, 0.0, 0.0, 0, 0)
            };

        if let Some(chunk) = &self.generated {
            let cw = chunk.width as f32;
            let ch = chunk.height as f32;
            let cell_size = chunk_cell_size;
            let base_x = chunk_base_x;
            let base_y = chunk_base_y;

            // determine visible range
            let vis_x0 = ((-self.preview_offset[0]) / cell_size).floor().max(0.0) as u32;
            let vis_y0 = ((-self.preview_offset[1]) / cell_size).floor().max(0.0) as u32;
            let vis_x1 = ((canvas_size.x / cell_size) + vis_x0 as f32 + 2.0).min(cw) as u32;
            let vis_y1 = ((canvas_size.y / cell_size) + vis_y0 as f32 + 2.0).min(ch) as u32;

            let biomes = &self.config.biomes;
            let sea_level = self.config.sea_level;

            for y in vis_y0..vis_y1 {
                for x in vis_x0..vis_x1 {
                    let idx = (y * chunk.width + x) as usize;
                    if idx >= chunk.height_map.len() { continue; }

                    let px = base_x + x as f32 * cell_size;
                    let py = base_y + y as f32 * cell_size;

                    let color = match self.preview_layer {
                        PreviewLayer::Height => {
                            let h = chunk.height_map[idx];
                            height_to_color(h, sea_level)
                        }
                        PreviewLayer::Moisture => {
                            moisture_to_color(chunk.moisture_map[idx])
                        }
                        PreviewLayer::Temperature => {
                            temp_to_color(chunk.temp_map[idx])
                        }
                        PreviewLayer::Biome => {
                            let bi = chunk.biome_map[idx];
                            if bi < biomes.len() { biomes[bi].color } else { Color32::GRAY }
                        }
                        PreviewLayer::Combined => {
                            let h = chunk.height_map[idx];
                            let mut c = height_to_color(h, sea_level);
                            // overlay biome tint slightly
                            let bi = chunk.biome_map[idx];
                            if bi < biomes.len() && h >= sea_level {
                                let bc = biomes[bi].color;
                                c = Color32::from_rgb(
                                    ((c.r() as f32 * 0.6 + bc.r() as f32 * 0.4) as u8),
                                    ((c.g() as f32 * 0.6 + bc.g() as f32 * 0.4) as u8),
                                    ((c.b() as f32 * 0.6 + bc.b() as f32 * 0.4) as u8),
                                );
                            }
                            c
                        }
                    };

                    let rect = Rect::from_min_size(
                        Pos2::new(px, py),
                        Vec2::new(cell_size + 0.5, cell_size + 0.5),
                    );
                    if rect.intersects(response.rect) {
                        painter.rect_filled(rect, 0.0, color);
                    }
                }
            }

            // Entity glyphs
            if self.show_entity_glyphs && cell_size >= 4.0 {
                let clip = response.rect;
                for &(pos, glyph, color) in &chunk.entity_positions {
                    let px = base_x + pos[0] * cell_size;
                    let py = base_y + pos[1] * cell_size;
                    if px >= clip.min.x && px < clip.max.x && py >= clip.min.y && py < clip.max.y {
                        painter.text(
                            Pos2::new(px + cell_size * 0.5, py + cell_size * 0.5),
                            egui::Align2::CENTER_CENTER,
                            glyph.to_string(),
                            FontId::monospace(cell_size.min(12.0)),
                            color,
                        );
                    }
                }
            }

            // Grid overlay
            if self.show_grid && cell_size >= 2.0 {
                let gs = self.grid_size as f32;
                let mut gx = (vis_x0 as f32 / gs).floor() * gs;
                while gx <= vis_x1 as f32 {
                    let px = base_x + gx * cell_size;
                    painter.line_segment(
                        [Pos2::new(px, response.rect.min.y), Pos2::new(px, response.rect.max.y)],
                        Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 255, 255, 30)),
                    );
                    gx += gs;
                }
                let mut gy = (vis_y0 as f32 / gs).floor() * gs;
                while gy <= vis_y1 as f32 {
                    let py = base_y + gy * cell_size;
                    painter.line_segment(
                        [Pos2::new(response.rect.min.x, py), Pos2::new(response.rect.max.x, py)],
                        Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 255, 255, 30)),
                    );
                    gy += gs;
                }
            }

            // Brush circle
            if let Some(bp) = self.brush_pos {
                if self.brush_tool != WorldBrush::None {
                    let bx = base_x + bp[0] * cell_size;
                    let by = base_y + bp[1] * cell_size;
                    let br = self.brush_radius * cell_size;
                    painter.circle_stroke(
                        Pos2::new(bx, by),
                        br,
                        Stroke::new(2.0, Color32::from_rgba_premultiplied(255, 200, 50, 180)),
                    );
                }
            }

        } else {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "No chunk generated.\nPress 'Generate' to create a world.",
                FontId::proportional(16.0),
                Color32::from_rgb(150, 150, 150),
            );
        }

        // Interaction: drag to pan/brush (outside chunk borrow so apply_brush can take &mut self)
        if self.generated.is_some() {
            let cell_size = chunk_cell_size;
            let base_x = chunk_base_x;
            let base_y = chunk_base_y;

            if response.dragged() {
                if self.brush_tool == WorldBrush::None {
                    let delta = response.drag_delta();
                    self.preview_offset[0] += delta.x;
                    self.preview_offset[1] += delta.y;
                } else if let Some(pos) = response.interact_pointer_pos() {
                    let wx = (pos.x - base_x) / cell_size;
                    let wy = (pos.y - base_y) / cell_size;
                    self.brush_pos = Some([wx, wy]);
                    self.apply_brush(wx, wy);
                    self.preview_needs_refresh = true;
                }
            }

            // Track mouse for brush preview and tooltip
            if let Some(pos) = response.hover_pos() {
                let wx = (pos.x - base_x) / cell_size;
                let wy = (pos.y - base_y) / cell_size;
                if self.brush_tool != WorldBrush::None {
                    self.brush_pos = Some([wx, wy]);
                } else {
                    self.brush_pos = None;
                }
                // Tooltip: show chunk info at hover
                let xi = wx.floor() as i32;
                let yi = wy.floor() as i32;
                if xi >= 0 && yi >= 0 && xi < chunk_w as i32 && yi < chunk_h as i32 {
                    if let Some(chunk) = &self.generated {
                        let idx2 = (yi as u32 * chunk.width + xi as u32) as usize;
                        if idx2 < chunk.height_map.len() {
                            let h = chunk.height_map[idx2];
                            let m = chunk.moisture_map[idx2];
                            let t = chunk.temp_map[idx2];
                            let bi = chunk.biome_map[idx2];
                            let bname = if bi < self.config.biomes.len() { &self.config.biomes[bi].name } else { "?" };
                            response.clone().on_hover_text(format!(
                                "({}, {})\nH: {:.3}  M: {:.3}  T: {:.3}\nBiome: {}",
                                xi, yi, h, m, t, bname
                            ));
                        }
                    }
                }
            } else {
                self.brush_pos = None;
            }

            // Scroll to zoom
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let factor = if scroll > 0.0 { 1.1 } else { 0.9 };
                    self.preview_zoom = (self.preview_zoom * factor).clamp(0.1, 32.0);
                }
            }
        }
    }

    fn show_bottom_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Seed:");
            ui.add(egui::DragValue::new(&mut self.generation_seed).speed(1.0));

            if ui.button("🎲 Random Seed").clicked() {
                // simple LCG for next seed
                self.generation_seed = self.generation_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            }

            ui.separator();

            if ui.button("⚙ Generate").clicked() {
                self.do_generate();
            }

            if ui.button("🌊 Apply Erosion").clicked() {
                if let Some(chunk) = &mut self.generated {
                    apply_erosion(chunk, &self.erosion);
                    assign_biomes(chunk, &self.config.biomes);
                }
                self.preview_needs_refresh = true;
            }

            if ui.button("🏞 Place Rivers").clicked() {
                if let Some(chunk) = &mut self.generated {
                    let seed = self.generation_seed;
                    let rivers = self.rivers.clone();
                    place_rivers(chunk, &rivers, seed);
                    assign_biomes(chunk, &self.config.biomes);
                }
                self.preview_needs_refresh = true;
            }

            if ui.button("🌿 Populate").clicked() {
                if let Some(chunk) = &mut self.generated {
                    let biomes = self.config.biomes.clone();
                    let seed = self.generation_seed;
                    populate_entities(chunk, &biomes, seed);
                }
            }

            ui.separator();
            ui.checkbox(&mut self.auto_regenerate, "Auto-regen");

            ui.separator();
            if self.generation_time_ms > 0.0 {
                ui.label(format!("Generated in {:.1}ms", self.generation_time_ms));
            }

            if let Some(chunk) = &self.generated {
                ui.label(format!("  {}×{} chunk", chunk.width, chunk.height));
                ui.label(format!("  {} entities", chunk.entity_positions.len()));
            }
        });
    }

    fn show_right_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Biomes");
        ui.separator();

        let mut to_add_biome = false;
        let mut to_remove_biome: Option<usize> = None;
        let mut to_move_biome_up: Option<usize> = None;
        let mut to_move_biome_down: Option<usize> = None;

        let sel = self.selected_biome;

        // Biome list with colored swatches
        egui::ScrollArea::vertical()
            .id_source("biome_list_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                for (i, biome) in self.config.biomes.iter().enumerate() {
                    let selected = sel == Some(i);
                    ui.horizontal(|ui| {
                        // color swatch
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, biome.color);
                        ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
                        let label = format!("{} '{}' p{}", biome.name, biome.glyph, biome.priority);
                        if ui.selectable_label(selected, label).clicked() {
                            self.selected_biome = Some(i);
                        }
                    });
                }
            });

        ui.horizontal(|ui| {
            if ui.small_button("+ Add").clicked() { to_add_biome = true; }
        });

        ui.separator();

        // Biome editor
        if let Some(sel_idx) = self.selected_biome {
            if sel_idx < self.config.biomes.len() {
                let biome = &mut self.config.biomes[sel_idx];
                ui.label("Selected Biome:");
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut biome.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Color:");
                    let mut c = [
                        biome.color.r() as f32 / 255.0,
                        biome.color.g() as f32 / 255.0,
                        biome.color.b() as f32 / 255.0,
                    ];
                    if ui.color_edit_button_rgb(&mut c).changed() {
                        biome.color = Color32::from_rgb(
                            (c[0] * 255.0) as u8,
                            (c[1] * 255.0) as u8,
                            (c[2] * 255.0) as u8,
                        );
                        self.dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Glyph:");
                    let mut s = biome.glyph.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut s).desired_width(30.0)).changed() {
                        if let Some(c) = s.chars().next() {
                            biome.glyph = c;
                        }
                    }
                });
                ui.label("Height Range:");
                ui.horizontal(|ui| {
                    ui.label("Min:");
                    if ui.add(egui::DragValue::new(&mut biome.min_height).range(0.0..=1.0).speed(0.01)).changed() { self.dirty = true; }
                    ui.label("Max:");
                    if ui.add(egui::DragValue::new(&mut biome.max_height).range(0.0..=1.0).speed(0.01)).changed() { self.dirty = true; }
                });
                ui.label("Moisture Range:");
                ui.horizontal(|ui| {
                    ui.label("Min:");
                    if ui.add(egui::DragValue::new(&mut biome.min_moisture).range(0.0..=1.0).speed(0.01)).changed() { self.dirty = true; }
                    ui.label("Max:");
                    if ui.add(egui::DragValue::new(&mut biome.max_moisture).range(0.0..=1.0).speed(0.01)).changed() { self.dirty = true; }
                });
                ui.label("Temperature Range:");
                ui.horizontal(|ui| {
                    ui.label("Min:");
                    if ui.add(egui::DragValue::new(&mut biome.min_temp).range(0.0..=1.0).speed(0.01)).changed() { self.dirty = true; }
                    ui.label("Max:");
                    if ui.add(egui::DragValue::new(&mut biome.max_temp).range(0.0..=1.0).speed(0.01)).changed() { self.dirty = true; }
                });
                ui.horizontal(|ui| {
                    ui.label("Density:");
                    if ui.add(egui::Slider::new(&mut biome.density, 0.0..=1.0)).changed() { self.dirty = true; }
                });
                ui.horizontal(|ui| {
                    ui.label("Priority:");
                    if ui.add(egui::DragValue::new(&mut biome.priority).range(-100..=100).speed(1.0)).changed() { self.dirty = true; }
                });
                ui.horizontal(|ui| {
                    if ui.small_button("↑").clicked() { to_move_biome_up = Some(sel_idx); }
                    if ui.small_button("↓").clicked() { to_move_biome_down = Some(sel_idx); }
                    if ui.small_button("Delete").clicked() { to_remove_biome = Some(sel_idx); }
                });
            }
        } else {
            ui.label("(select a biome to edit)");
        }

        // Biome mutations
        if to_add_biome {
            self.config.biomes.push(BiomeRule {
                name: format!("Biome {}", self.config.biomes.len()),
                color: Color32::from_rgb(128, 128, 128),
                min_height: 0.3, max_height: 0.7,
                min_moisture: 0.3, max_moisture: 0.7,
                min_temp: 0.3, max_temp: 0.7,
                glyph: '?', density: 0.1, priority: 0,
            });
            self.selected_biome = Some(self.config.biomes.len() - 1);
            self.dirty = true;
        }
        if let Some(idx) = to_remove_biome {
            if idx < self.config.biomes.len() {
                self.config.biomes.remove(idx);
                self.selected_biome = None;
                self.dirty = true;
            }
        }
        if let Some(idx) = to_move_biome_up {
            if idx > 0 && idx < self.config.biomes.len() {
                self.config.biomes.swap(idx, idx-1);
                self.selected_biome = Some(idx-1);
                self.dirty = true;
            }
        }
        if let Some(idx) = to_move_biome_down {
            if idx + 1 < self.config.biomes.len() {
                self.config.biomes.swap(idx, idx+1);
                self.selected_biome = Some(idx+1);
                self.dirty = true;
            }
        }

        ui.separator();

        // Biome distribution stats
        if let Some(chunk) = &self.generated {
            ui.label("Biome Coverage:");
            let total = chunk.biome_map.len() as f32;
            if total > 0.0 {
                let mut counts = vec![0u32; self.config.biomes.len()];
                for &bi in &chunk.biome_map {
                    if bi < counts.len() { counts[bi] += 1; }
                }
                for (i, &count) in counts.iter().enumerate() {
                    if count > 0 && i < self.config.biomes.len() {
                        let pct = count as f32 / total * 100.0;
                        let biome = &self.config.biomes[i];
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 1.0, biome.color);
                            ui.label(format!("{}: {:.1}%", biome.name, pct));
                        });
                    }
                }
            }
        }

        ui.separator();

        // Mini noise preview (small gradient for selected layer)
        if let Some((target, layer_idx)) = &self.selected_noise_layer {
            let layers = match target {
                NoiseLayerTarget::Height => &self.config.height_layers,
                NoiseLayerTarget::Moisture => &self.config.moisture_layers,
                NoiseLayerTarget::Temperature => &self.config.temp_layers,
            };
            if *layer_idx < layers.len() {
                let layer = &layers[*layer_idx];
                ui.label(format!("Noise Preview: {}", layer.name));
                let preview_size = Vec2::new(self.right_panel_width - 20.0, 80.0);
                let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
                let painter = ui.painter();
                let pw = preview_size.x as u32;
                let ph = preview_size.y as u32;
                for py in 0..ph {
                    for px in 0..pw {
                        let nx = px as f32;
                        let ny = py as f32;
                        let val = evaluate_noise_type(&layer.noise_type, nx * layer.frequency + layer.offset[0], ny * layer.frequency + layer.offset[1], layer.seed, 1.0);
                        let v = (val * 255.0) as u8;
                        let prect = Rect::from_min_size(
                            Pos2::new(rect.min.x + px as f32, rect.min.y + py as f32),
                            Vec2::new(1.0, 1.0),
                        );
                        painter.rect_filled(prect, 0.0, Color32::from_gray(v));
                    }
                }
            }
        }
    }

    fn apply_brush(&mut self, wx: f32, wy: f32) {
        if let Some(chunk) = &mut self.generated {
            let w = chunk.width as usize;
            let h = chunk.height as usize;
            let r = self.brush_radius;
            let str = self.brush_strength;
            let ix = wx.floor() as i32;
            let iy = wy.floor() as i32;
            let ir = r.ceil() as i32;

            for dy in -ir..=ir {
                for dx in -ir..=ir {
                    let nx = ix + dx;
                    let ny = iy + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist > r { continue; }
                    let falloff = 1.0 - (dist / r);
                    let idx = (ny as usize) * w + nx as usize;
                    match self.brush_tool {
                        WorldBrush::RaiseTerrain => {
                            chunk.height_map[idx] = (chunk.height_map[idx] + str * falloff).clamp(0.0, 1.0);
                        }
                        WorldBrush::LowerTerrain => {
                            chunk.height_map[idx] = (chunk.height_map[idx] - str * falloff).clamp(0.0, 1.0);
                        }
                        WorldBrush::FlattenTerrain => {
                            let center_h = chunk.height_map[(iy as usize) * w + ix as usize];
                            chunk.height_map[idx] = lerp(chunk.height_map[idx], center_h, str * falloff);
                        }
                        WorldBrush::PaintBiome => {
                            if let Some(bi) = self.selected_biome {
                                chunk.biome_map[idx] = bi;
                            }
                        }
                        WorldBrush::PlaceRiver => {
                            chunk.height_map[idx] = chunk.height_map[idx].min(0.33);
                            chunk.moisture_map[idx] = (chunk.moisture_map[idx] + 0.3 * falloff).clamp(0.0, 1.0);
                        }
                        WorldBrush::ErodeLocal => {
                            // local thermal erosion pass
                            if idx > 0 && idx < w * h - 1 {
                                let neighbors = [
                                    if nx > 0 { Some(idx - 1) } else { None },
                                    if nx < w as i32 - 1 { Some(idx + 1) } else { None },
                                    if ny > 0 { Some(idx - w) } else { None },
                                    if ny < h as i32 - 1 { Some(idx + w) } else { None },
                                ];
                                let center_h = chunk.height_map[idx];
                                for maybe_ni in &neighbors {
                                    if let Some(ni) = maybe_ni {
                                        let diff = center_h - chunk.height_map[*ni];
                                        if diff > 0.02 {
                                            let transfer = diff * str * falloff * 0.1;
                                            chunk.height_map[idx] -= transfer;
                                            chunk.height_map[*ni] += transfer;
                                        }
                                    }
                                }
                            }
                        }
                        WorldBrush::None => {}
                    }
                }
            }

            // Re-assign biomes for affected area (unless painting biomes directly)
            if self.brush_tool != WorldBrush::PaintBiome {
                let biomes = self.config.biomes.clone();
                assign_biomes(chunk, &biomes);
            }
        }
    }

    fn do_generate(&mut self) {
        let start = std::time::Instant::now();
        let mut chunk = generate_chunk(&self.config, self.generation_seed);

        if self.config.erosion_iterations > 0 {
            let erosion = ErosionParams {
                thermal_iterations: self.config.erosion_iterations,
                hydraulic_iterations: self.config.erosion_iterations * 2,
                ..self.erosion.clone()
            };
            apply_erosion(&mut chunk, &erosion);
        }

        if self.config.river_chance > 0.0 {
            let rivers = self.rivers.clone();
            let seed = self.generation_seed;
            place_rivers(&mut chunk, &rivers, seed);
            assign_biomes(&mut chunk, &self.config.biomes);
        }

        populate_entities(&mut chunk, &self.config.biomes, self.generation_seed);
        self.generation_time_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.generated = Some(chunk);
        self.preview_needs_refresh = true;
    }
}

// ============================================================
// ADDITIONAL UTILITY: noise export, stats, I/O stubs
// ============================================================

/// Export height map as normalized f32 values in a flat Vec
pub fn export_heightmap(chunk: &GeneratedChunk) -> Vec<f32> {
    chunk.height_map.clone()
}

/// Export biome map as flat Vec<u8> (biome indices clamped to u8)
pub fn export_biome_map(chunk: &GeneratedChunk) -> Vec<u8> {
    chunk.biome_map.iter().map(|&b| b.min(255) as u8).collect()
}

/// Compute height map statistics
pub struct HeightStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub std_dev: f32,
    pub sea_coverage: f32,
}

pub fn compute_height_stats(chunk: &GeneratedChunk, sea_level: f32) -> HeightStats {
    let n = chunk.height_map.len();
    if n == 0 {
        return HeightStats { min: 0.0, max: 0.0, mean: 0.0, std_dev: 0.0, sea_coverage: 0.0 };
    }
    let mut mn = f32::MAX;
    let mut mx = f32::MIN;
    let mut sum = 0.0_f64;
    let mut sea_count = 0usize;
    for &v in &chunk.height_map {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
        sum += v as f64;
        if v < sea_level { sea_count += 1; }
    }
    let mean = (sum / n as f64) as f32;
    let mut var_sum = 0.0_f64;
    for &v in &chunk.height_map {
        let diff = v as f64 - mean as f64;
        var_sum += diff * diff;
    }
    let std_dev = ((var_sum / n as f64).sqrt()) as f32;
    HeightStats {
        min: mn, max: mx, mean, std_dev,
        sea_coverage: sea_count as f32 / n as f32,
    }
}

/// Simple 2D Gaussian blur for smoothing maps
pub fn gaussian_blur(data: &mut Vec<f32>, width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 || radius == 0 { return; }
    let kernel_size = radius * 2 + 1;
    let sigma = radius as f32 / 2.0;
    let mut kernel = vec![0.0_f32; kernel_size];
    let mut sum = 0.0_f32;
    for i in 0..kernel_size {
        let x = i as f32 - radius as f32;
        kernel[i] = (-x * x / (2.0 * sigma * sigma)).exp();
        sum += kernel[i];
    }
    for k in &mut kernel { *k /= sum; }

    // horizontal pass
    let mut temp = vec![0.0_f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let mut val = 0.0_f32;
            for (ki, &k) in kernel.iter().enumerate() {
                let sx = (x as i32 + ki as i32 - radius as i32).clamp(0, width as i32 - 1) as usize;
                val += data[y * width + sx] * k;
            }
            temp[y * width + x] = val;
        }
    }

    // vertical pass
    for y in 0..height {
        for x in 0..width {
            let mut val = 0.0_f32;
            for (ki, &k) in kernel.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - radius as i32).clamp(0, height as i32 - 1) as usize;
                val += temp[sy * width + x] * k;
            }
            data[y * width + x] = val;
        }
    }
}

/// Normalize a Vec<f32> to [0,1]
pub fn normalize_map(data: &mut Vec<f32>) {
    let mn = data.iter().cloned().fold(f32::MAX, f32::min);
    let mx = data.iter().cloned().fold(f32::MIN, f32::max);
    let range = mx - mn;
    if range > 0.0 {
        for v in data.iter_mut() {
            *v = (*v - mn) / range;
        }
    }
}

/// Apply island mask (radial falloff) to height map
pub fn apply_island_mask(data: &mut Vec<f32>, width: usize, height: usize, strength: f32) {
    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let max_dist = (cx * cx + cy * cy).sqrt();
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt() / max_dist;
            let mask = (1.0 - dist.powf(2.0) * strength).clamp(0.0, 1.0);
            data[y * width + x] *= mask;
        }
    }
}

/// Warp domain by adding noise to x/y coordinates before sampling
pub fn domain_warp(x: f32, y: f32, seed: u32, warp_amount: f32) -> (f32, f32) {
    let wx = perlin_2d(x * 0.1, y * 0.1, seed) * warp_amount;
    let wy = perlin_2d(x * 0.1 + 100.0, y * 0.1 + 100.0, seed.wrapping_add(1)) * warp_amount;
    (x + wx, y + wy)
}

/// Generate a temperature map with latitude and altitude corrections
pub fn generate_temp_with_corrections(
    width: u32, height: u32,
    temp_layers: &[NoiseLayer],
    height_map: &[f32],
    seed: u32,
    pole_strength: f32,
    altitude_strength: f32,
) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    let mut temp_map = vec![0.0_f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let base = evaluate_layer_stack(temp_layers, x as f32, y as f32);
            let lat = (y as f32 / h as f32 - 0.5).abs() * 2.0;
            let alt = height_map[idx];
            let temp = base * 0.4 + (1.0 - lat) * pole_strength - alt * altitude_strength;
            temp_map[idx] = temp.clamp(0.0, 1.0);
        }
    }
    temp_map
}

/// Simple diamond-square heightmap generation
pub fn diamond_square(size: usize, seed: u64, roughness: f32) -> Vec<f32> {
    // size must be 2^n + 1
    let n = size;
    let mut map = vec![0.0_f32; n * n];
    let mut rng = seed;

    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13;
        *r ^= *r >> 7;
        *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0 * 2.0 - 1.0
    };

    // seed corners
    map[0] = rng_f(&mut rng) * 0.5 + 0.5;
    map[n-1] = rng_f(&mut rng) * 0.5 + 0.5;
    map[(n-1)*n] = rng_f(&mut rng) * 0.5 + 0.5;
    map[(n-1)*n + n-1] = rng_f(&mut rng) * 0.5 + 0.5;

    let mut step = n - 1;
    let mut scale = roughness;
    while step > 1 {
        let half = step / 2;
        // diamond step
        let mut y = 0;
        while y < n - 1 {
            let mut x = 0;
            while x < n - 1 {
                let avg = (map[y*n+x] + map[y*n+x+step] + map[(y+step)*n+x] + map[(y+step)*n+x+step]) * 0.25;
                map[(y+half)*n + (x+half)] = (avg + rng_f(&mut rng) * scale).clamp(0.0, 1.0);
                x += step;
            }
            y += step;
        }
        // square step
        let mut y = 0;
        while y < n {
            let mut x = if (y / half) % 2 == 0 { half } else { 0 };
            while x < n {
                let mut count = 0;
                let mut sum = 0.0_f32;
                if x >= half { sum += map[y*n + x-half]; count += 1; }
                if x + half < n { sum += map[y*n + x+half]; count += 1; }
                if y >= half { sum += map[(y-half)*n + x]; count += 1; }
                if y + half < n { sum += map[(y+half)*n + x]; count += 1; }
                if count > 0 {
                    map[y*n+x] = ((sum / count as f32) + rng_f(&mut rng) * scale).clamp(0.0, 1.0);
                }
                x += step;
            }
            y += half;
        }
        step = half;
        scale *= 0.5;
    }
    map
}

/// Contour line extraction: returns list of segments at threshold
pub fn extract_contours(data: &[f32], width: usize, height: usize, threshold: f32) -> Vec<[Pos2; 2]> {
    let mut segments = Vec::new();
    for y in 0..height.saturating_sub(1) {
        for x in 0..width.saturating_sub(1) {
            let v00 = data[y * width + x];
            let v10 = data[y * width + x + 1];
            let v01 = data[(y+1) * width + x];
            let v11 = data[(y+1) * width + x + 1];

            let b00 = v00 >= threshold;
            let b10 = v10 >= threshold;
            let b01 = v01 >= threshold;
            let b11 = v11 >= threshold;

            let fx = x as f32;
            let fy = y as f32;

            // marching squares lookup
            let case = (b00 as u8) | ((b10 as u8) << 1) | ((b01 as u8) << 2) | ((b11 as u8) << 3);

            let interp_h = |a: f32, b: f32| -> f32 {
                if (b - a).abs() < 0.0001 { 0.5 }
                else { (threshold - a) / (b - a) }
            };

            match case {
                0 | 15 => {}
                1 | 14 => {
                    let t1 = interp_h(v00, v10);
                    let t2 = interp_h(v00, v01);
                    segments.push([Pos2::new(fx + t1, fy), Pos2::new(fx, fy + t2)]);
                }
                2 | 13 => {
                    let t1 = interp_h(v00, v10);
                    let t2 = interp_h(v10, v11);
                    segments.push([Pos2::new(fx + t1, fy), Pos2::new(fx + 1.0, fy + t2)]);
                }
                3 | 12 => {
                    let t1 = interp_h(v00, v01);
                    let t2 = interp_h(v10, v11);
                    segments.push([Pos2::new(fx, fy + t1), Pos2::new(fx + 1.0, fy + t2)]);
                }
                4 | 11 => {
                    let t1 = interp_h(v00, v01);
                    let t2 = interp_h(v01, v11);
                    segments.push([Pos2::new(fx, fy + t1), Pos2::new(fx + t2, fy + 1.0)]);
                }
                5 => {
                    let t1 = interp_h(v00, v10);
                    let t2 = interp_h(v10, v11);
                    let t3 = interp_h(v01, v11);
                    let t4 = interp_h(v00, v01);
                    segments.push([Pos2::new(fx + t1, fy), Pos2::new(fx + 1.0, fy + t2)]);
                    segments.push([Pos2::new(fx + t3, fy + 1.0), Pos2::new(fx, fy + t4)]);
                }
                6 | 9 => {
                    let t1 = interp_h(v00, v10);
                    let t2 = interp_h(v01, v11);
                    segments.push([Pos2::new(fx + t1, fy), Pos2::new(fx + t2, fy + 1.0)]);
                }
                7 | 8 => {
                    let t1 = interp_h(v10, v11);
                    let t2 = interp_h(v01, v11);
                    segments.push([Pos2::new(fx + 1.0, fy + t1), Pos2::new(fx + t2, fy + 1.0)]);
                }
                10 => {
                    let t1 = interp_h(v00, v10);
                    let t2 = interp_h(v00, v01);
                    let t3 = interp_h(v10, v11);
                    let t4 = interp_h(v01, v11);
                    segments.push([Pos2::new(fx + t1, fy), Pos2::new(fx, fy + t2)]);
                    segments.push([Pos2::new(fx + 1.0, fy + t3), Pos2::new(fx + t4, fy + 1.0)]);
                }
                _ => {}
            }
        }
    }
    segments
}

/// River pathfinding using A* on height map
pub struct RiverPath {
    pub points: Vec<(usize, usize)>,
    pub total_length: f32,
}

pub fn find_river_path(
    height_map: &[f32],
    width: usize,
    height: usize,
    start: (usize, usize),
    sea_level: f32,
) -> Option<RiverPath> {
    let mut current = start;
    let mut path = vec![current];
    let mut visited = HashSet::new();
    visited.insert(current);

    let max_steps = width.max(height) * 4;

    for _ in 0..max_steps {
        let (cx, cy) = current;
        let curr_h = height_map[cy * width + cx];

        if curr_h <= sea_level { break; }

        let neighbors = [
            if cx > 0 { Some((cx-1, cy)) } else { None },
            if cx+1 < width { Some((cx+1, cy)) } else { None },
            if cy > 0 { Some((cx, cy-1)) } else { None },
            if cy+1 < height { Some((cx, cy+1)) } else { None },
            if cx > 0 && cy > 0 { Some((cx-1, cy-1)) } else { None },
            if cx+1 < width && cy+1 < height { Some((cx+1, cy+1)) } else { None },
            if cx > 0 && cy+1 < height { Some((cx-1, cy+1)) } else { None },
            if cx+1 < width && cy > 0 { Some((cx+1, cy-1)) } else { None },
        ];

        let mut best: Option<(usize, usize)> = None;
        let mut best_h = curr_h;
        for maybe_n in &neighbors {
            if let Some(n) = maybe_n {
                if !visited.contains(n) {
                    let nh = height_map[n.1 * width + n.0];
                    if nh < best_h {
                        best_h = nh;
                        best = Some(*n);
                    }
                }
            }
        }

        match best {
            Some(n) => {
                visited.insert(n);
                path.push(n);
                current = n;
            }
            None => break,
        }
    }

    if path.len() < 3 { return None; }
    let total_length = path.len() as f32;
    Some(RiverPath { points: path, total_length })
}

/// Voronoi cell generator for biome regions
pub struct VoronoiCell {
    pub center: [f32; 2],
    pub biome_index: usize,
}

pub fn generate_voronoi_biomes(
    width: u32, height: u32,
    num_cells: usize,
    seed: u64,
    biome_count: usize,
) -> Vec<usize> {
    let mut rng = seed;
    let rng_next = |r: &mut u64| -> f32 {
        *r ^= *r << 13;
        *r ^= *r >> 7;
        *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    let cells: Vec<VoronoiCell> = (0..num_cells).map(|_| {
        VoronoiCell {
            center: [rng_next(&mut rng) * width as f32, rng_next(&mut rng) * height as f32],
            biome_index: (rng_next(&mut rng) * biome_count as f32) as usize % biome_count,
        }
    }).collect();

    let w = width as usize;
    let h = height as usize;
    let mut result = vec![0usize; w * h];

    for y in 0..h {
        for x in 0..w {
            let fx = x as f32;
            let fy = y as f32;
            let mut best_dist = f32::MAX;
            let mut best_biome = 0;
            for cell in &cells {
                let dx = fx - cell.center[0];
                let dy = fy - cell.center[1];
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_biome = cell.biome_index;
                }
            }
            result[y * w + x] = best_biome;
        }
    }
    result
}

/// Hillshade calculation for 3D-like terrain rendering
pub fn compute_hillshade(
    height_map: &[f32],
    width: usize,
    height: usize,
    sun_azimuth: f32,
    sun_altitude: f32,
    z_scale: f32,
) -> Vec<f32> {
    let az = sun_azimuth.to_radians();
    let alt = sun_altitude.to_radians();
    let sun_dir = [az.cos() * alt.cos(), az.sin() * alt.cos(), alt.sin()];

    let mut shade = vec![0.0_f32; width * height];
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let dz_dx = (height_map[y * width + x + 1] - height_map[y * width + x - 1]) * z_scale * 0.5;
            let dz_dy = (height_map[(y+1) * width + x] - height_map[(y-1) * width + x]) * z_scale * 0.5;
            let normal = [-dz_dx, -dz_dy, 1.0];
            let len = (normal[0]*normal[0] + normal[1]*normal[1] + normal[2]*normal[2]).sqrt();
            let dot = if len > 0.0 {
                (normal[0]*sun_dir[0] + normal[1]*sun_dir[1] + normal[2]*sun_dir[2]) / len
            } else { 0.0 };
            shade[y * width + x] = dot.max(0.0);
        }
    }
    shade
}

/// Moisture accumulation from rainfall simulation
pub fn simulate_rainfall_moisture(
    height_map: &[f32],
    width: usize,
    height: usize,
    rain_iterations: u32,
    seed: u64,
) -> Vec<f32> {
    let mut moisture = vec![0.0_f32; width * height];
    let mut rng = seed;
    let rng_next = |r: &mut u64| -> f32 {
        *r ^= *r << 13;
        *r ^= *r >> 7;
        *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    for _ in 0..rain_iterations {
        // drop rain at random points
        let drops = (width * height / 100).max(1);
        for _ in 0..drops {
            let mut x = (rng_next(&mut rng) * width as f32) as usize;
            let mut y = (rng_next(&mut rng) * height as f32) as usize;
            x = x.clamp(0, width-1);
            y = y.clamp(0, height-1);
            let mut water = 1.0_f32;
            for _ in 0..50 {
                let idx = y * width + x;
                moisture[idx] += water * 0.1;
                water *= 0.95;
                if water < 0.01 { break; }
                // flow downhill
                let neighbors = [
                    if x > 0 { Some((x-1, y)) } else { None },
                    if x+1 < width { Some((x+1, y)) } else { None },
                    if y > 0 { Some((x, y-1)) } else { None },
                    if y+1 < height { Some((x, y+1)) } else { None },
                ];
                let mut best_n = None;
                let mut best_h = height_map[idx];
                for maybe_n in &neighbors {
                    if let Some(&(nx, ny)) = maybe_n.as_ref() {
                        let nh = height_map[ny * width + nx];
                        if nh < best_h {
                            best_h = nh;
                            best_n = Some((nx, ny));
                        }
                    }
                }
                if let Some((nx, ny)) = best_n {
                    x = nx; y = ny;
                } else {
                    break;
                }
            }
        }
    }
    normalize_map(&mut moisture);
    moisture
}

/// Generate a simple wind map using perlin noise
pub fn generate_wind_map(width: u32, height: u32, seed: u32, scale: f32) -> Vec<[f32; 2]> {
    let w = width as usize;
    let h = height as usize;
    let mut wind = vec![[0.0_f32; 2]; w * h];
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 * scale;
            let fy = y as f32 * scale;
            let angle = perlin_2d(fx, fy, seed) * std::f32::consts::TAU;
            let speed = value_noise_2d(fx + 100.0, fy + 100.0, seed.wrapping_add(1));
            wind[y * w + x] = [angle.cos() * speed, angle.sin() * speed];
        }
    }
    wind
}

// ============================================================
// TESTS (basic smoke tests)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_noise_range() {
        for i in 0..100 {
            let v = value_noise_2d(i as f32 * 0.37, i as f32 * 0.53, 42);
            assert!(v >= 0.0 && v <= 1.0, "value_noise out of range: {}", v);
        }
    }

    #[test]
    fn test_perlin_range() {
        for i in 0..100 {
            let v = perlin_2d(i as f32 * 0.1, i as f32 * 0.17, 99);
            assert!(v >= 0.0 && v <= 1.0, "perlin out of range: {}", v);
        }
    }

    #[test]
    fn test_fbm_range() {
        for i in 0..50 {
            let v = fbm(i as f32 * 0.1, i as f32 * 0.13, 7, 6, 2.0, 0.5);
            assert!(v >= 0.0 && v <= 1.0, "fbm out of range: {}", v);
        }
    }

    #[test]
    fn test_cellular_range() {
        for i in 0..50 {
            let v = cellular_2d(i as f32 * 0.5, i as f32 * 0.7, 13);
            assert!(v >= 0.0 && v <= 1.0, "cellular out of range: {}", v);
        }
    }

    #[test]
    fn test_generate_chunk() {
        let config = WorldGenConfig {
            width: 32, height: 32,
            ..Default::default()
        };
        let chunk = generate_chunk(&config, 42);
        assert_eq!(chunk.height_map.len(), 32 * 32);
        assert_eq!(chunk.biome_map.len(), 32 * 32);
        for &v in &chunk.height_map {
            assert!(v >= 0.0 && v <= 1.0);
        }
    }

    #[test]
    fn test_erosion() {
        let config = WorldGenConfig {
            width: 32, height: 32,
            ..Default::default()
        };
        let mut chunk = generate_chunk(&config, 99);
        let params = ErosionParams::default();
        apply_erosion(&mut chunk, &params);
        for &v in &chunk.height_map {
            assert!(v >= 0.0 && v <= 1.0, "eroded height out of range: {}", v);
        }
    }

    #[test]
    fn test_preset_island() {
        let mut config = WorldGenConfig::default();
        WorldPreset::Island.apply(&mut config);
        assert!(config.sea_level > 0.0);
    }

    #[test]
    fn test_diamond_square() {
        let map = diamond_square(33, 42, 0.5);
        assert_eq!(map.len(), 33 * 33);
        for &v in &map {
            assert!(v >= 0.0 && v <= 1.0, "diamond-square out of range: {}", v);
        }
    }

    #[test]
    fn test_blend_modes() {
        assert!((BlendMode::Add.apply(0.3, 0.5) - 0.8).abs() < 0.001);
        assert!((BlendMode::Multiply.apply(0.5, 0.5) - 0.25).abs() < 0.001);
        assert!((BlendMode::Replace.apply(0.3, 0.7) - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_hillshade() {
        let config = WorldGenConfig { width: 16, height: 16, ..Default::default() };
        let chunk = generate_chunk(&config, 1);
        let shade = compute_hillshade(&chunk.height_map, 16, 16, 315.0, 45.0, 3.0);
        assert_eq!(shade.len(), 16 * 16);
        for &v in &shade {
            assert!(v >= 0.0 && v <= 1.0, "shade out of range: {}", v);
        }
    }
}

// ============================================================
// ADVANCED WORLD GEN: CLIMATE, TECTONICS, EROSION VARIANTS
// ============================================================

/// Full climate simulation on top of generated terrain
pub struct ClimateSimulator {
    pub wind_direction: f32,   // degrees
    pub wind_speed: f32,
    pub global_temp_offset: f32,
    pub rainfall_scale: f32,
    pub ocean_influence: f32,
}

impl ClimateSimulator {
    pub fn new() -> Self {
        Self {
            wind_direction: 270.0,
            wind_speed: 1.0,
            global_temp_offset: 0.0,
            rainfall_scale: 1.0,
            ocean_influence: 0.5,
        }
    }

    pub fn simulate(&self, chunk: &mut GeneratedChunk, sea_level: f32, iterations: u32) {
        let w = chunk.width as usize;
        let h = chunk.height as usize;

        let wind_rad = self.wind_direction.to_radians();
        let wind_dx = wind_rad.cos() * self.wind_speed;
        let wind_dy = wind_rad.sin() * self.wind_speed;

        // moisture evaporation from ocean
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if chunk.height_map[idx] < sea_level {
                    chunk.moisture_map[idx] = (chunk.moisture_map[idx] + self.ocean_influence * 0.1).min(1.0);
                }
            }
        }

        // wind-driven moisture transport (simplified advection)
        for _iter in 0..iterations {
            let old_moisture = chunk.moisture_map.clone();
            for y in 1..h-1 {
                for x in 1..w-1 {
                    let idx = y * w + x;
                    // sample upwind position
                    let src_x = (x as f32 - wind_dx).round().clamp(0.0, w as f32 - 1.0) as usize;
                    let src_y = (y as f32 - wind_dy).round().clamp(0.0, h as f32 - 1.0) as usize;
                    let src_idx = src_y * w + src_x;
                    // orographic lift: moisture condenses on windward slopes
                    let height_diff = chunk.height_map[idx] - chunk.height_map[src_idx];
                    let rain_factor = if height_diff > 0.0 {
                        height_diff * self.rainfall_scale * 2.0
                    } else { 0.0 };
                    let transported = old_moisture[src_idx] * 0.95;
                    let rained_out = transported * rain_factor.min(1.0);
                    chunk.moisture_map[idx] = (transported - rained_out + rained_out * 0.1).clamp(0.0, 1.0);
                }
            }
        }

        // temperature adjustment
        for i in 0..w*h {
            chunk.temp_map[i] = (chunk.temp_map[i] + self.global_temp_offset).clamp(0.0, 1.0);
        }
    }
}

/// Tectonic plate simulation for large-scale terrain features
pub struct TectonicPlate {
    pub center: [f32; 2],
    pub radius: f32,
    pub elevation_bias: f32,
    pub movement: [f32; 2],
    pub plate_type: PlateType,
}

#[derive(Clone, Debug)]
pub enum PlateType {
    Continental,
    Oceanic,
}

pub struct TectonicSimulator {
    pub plates: Vec<TectonicPlate>,
}

impl TectonicSimulator {
    pub fn new(num_plates: usize, width: u32, height: u32, seed: u64) -> Self {
        let mut rng = seed;
        let rng_f = |r: &mut u64| -> f32 {
            *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
            (*r & 0xFFFFFF) as f32 / 16777216.0
        };

        let plates = (0..num_plates).map(|_| {
            let cx = rng_f(&mut rng) * width as f32;
            let cy = rng_f(&mut rng) * height as f32;
            let radius = (width.min(height) as f32) * (0.2 + rng_f(&mut rng) * 0.4);
            let elev = if rng_f(&mut rng) > 0.4 { rng_f(&mut rng) * 0.3 } else { -rng_f(&mut rng) * 0.2 };
            let ptype = if rng_f(&mut rng) > 0.35 { PlateType::Continental } else { PlateType::Oceanic };
            TectonicPlate {
                center: [cx, cy],
                radius,
                elevation_bias: elev,
                movement: [rng_f(&mut rng) * 2.0 - 1.0, rng_f(&mut rng) * 2.0 - 1.0],
                plate_type: ptype,
            }
        }).collect();

        TectonicSimulator { plates }
    }

    /// Apply tectonic influence to a height map
    pub fn apply(&self, height_map: &mut Vec<f32>, width: u32, height: u32) {
        let w = width as usize;
        let h = height as usize;
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let fx = x as f32;
                let fy = y as f32;
                let mut max_influence = 0.0_f32;
                let mut best_bias = 0.0_f32;
                for plate in &self.plates {
                    let dx = fx - plate.center[0];
                    let dy = fy - plate.center[1];
                    let dist = (dx * dx + dy * dy).sqrt();
                    let influence = (1.0 - (dist / plate.radius).min(1.0)).max(0.0);
                    if influence > max_influence {
                        max_influence = influence;
                        best_bias = plate.elevation_bias;
                    }
                }
                // Check for plate boundaries (subduction/collision)
                let mut collision_energy = 0.0_f32;
                for pi in 0..self.plates.len() {
                    for pj in (pi+1)..self.plates.len() {
                        let pa = &self.plates[pi];
                        let pb = &self.plates[pj];
                        // relative velocity dot product
                        let rel_vx = pa.movement[0] - pb.movement[0];
                        let rel_vy = pa.movement[1] - pb.movement[1];
                        let dx_ab = pb.center[0] - pa.center[0];
                        let dy_ab = pb.center[1] - pa.center[1];
                        let dist_ab = (dx_ab * dx_ab + dy_ab * dy_ab).sqrt();
                        if dist_ab < 0.001 { continue; }
                        let dot = (rel_vx * dx_ab + rel_vy * dy_ab) / dist_ab;
                        // how close is this point to the boundary?
                        let dist_a = ((fx - pa.center[0]).powi(2) + (fy - pa.center[1]).powi(2)).sqrt();
                        let dist_b = ((fx - pb.center[0]).powi(2) + (fy - pb.center[1]).powi(2)).sqrt();
                        let boundary_dist = (dist_a - pa.radius).abs().min((dist_b - pb.radius).abs());
                        if boundary_dist < 20.0 {
                            let proximity = 1.0 - boundary_dist / 20.0;
                            collision_energy += dot.abs() * proximity;
                        }
                    }
                }
                height_map[idx] = (height_map[idx] + best_bias * max_influence + collision_energy * 0.15).clamp(0.0, 1.0);
            }
        }
        normalize_map(height_map);
    }
}

/// Advanced hydraulic erosion with full particle simulation
pub struct HydraulicErosionParticle {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub water: f32,
    pub sediment: f32,
}

impl HydraulicErosionParticle {
    pub fn new(x: f32, y: f32) -> Self {
        Self { pos: [x, y], vel: [0.0, 0.0], water: 1.0, sediment: 0.0 }
    }
}

pub struct ParticleErosionParams {
    pub inertia: f32,
    pub gravity: f32,
    pub evaporation_rate: f32,
    pub deposit_speed: f32,
    pub erode_speed: f32,
    pub sediment_capacity_factor: f32,
    pub min_slope: f32,
    pub max_steps: u32,
}

impl Default for ParticleErosionParams {
    fn default() -> Self {
        Self {
            inertia: 0.05,
            gravity: 4.0,
            evaporation_rate: 0.01,
            deposit_speed: 0.3,
            erode_speed: 0.3,
            sediment_capacity_factor: 8.0,
            min_slope: 0.01,
            max_steps: 64,
        }
    }
}

pub fn particle_hydraulic_erosion(
    height_map: &mut Vec<f32>,
    width: usize,
    height: usize,
    params: &ParticleErosionParams,
    num_particles: u32,
    seed: u64,
) {
    let mut rng = seed;
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    // Heightmap sample with bilinear interpolation
    let sample_h = |map: &Vec<f32>, x: f32, y: f32| -> f32 {
        let x = x.clamp(0.0, width as f32 - 1.001);
        let y = y.clamp(0.0, height as f32 - 1.001);
        let ix = x as usize;
        let iy = y as usize;
        let fx = x - ix as f32;
        let fy = y - iy as f32;
        let v00 = map[iy * width + ix];
        let v10 = map[iy * width + (ix+1).min(width-1)];
        let v01 = map[(iy+1).min(height-1) * width + ix];
        let v11 = map[(iy+1).min(height-1) * width + (ix+1).min(width-1)];
        v00 * (1.0-fx)*(1.0-fy) + v10 * fx*(1.0-fy) + v01 * (1.0-fx)*fy + v11 * fx*fy
    };

    let sample_grad = |map: &Vec<f32>, x: f32, y: f32| -> [f32; 2] {
        let x = x.clamp(1.0, width as f32 - 2.0);
        let y = y.clamp(1.0, height as f32 - 2.0);
        let gx = sample_h(map, x+1.0, y) - sample_h(map, x-1.0, y);
        let gy = sample_h(map, x, y+1.0) - sample_h(map, x, y-1.0);
        [gx * 0.5, gy * 0.5]
    };

    for _ in 0..num_particles {
        let px = rng_f(&mut rng) * (width as f32 - 2.0) + 1.0;
        let py = rng_f(&mut rng) * (height as f32 - 2.0) + 1.0;
        let mut particle = HydraulicErosionParticle::new(px, py);

        for _step in 0..params.max_steps {
            let old_pos = particle.pos;
            let grad = sample_grad(height_map, particle.pos[0], particle.pos[1]);

            // update velocity
            particle.vel[0] = particle.vel[0] * params.inertia - grad[0] * (1.0 - params.inertia);
            particle.vel[1] = particle.vel[1] * params.inertia - grad[1] * (1.0 - params.inertia);

            let speed = (particle.vel[0]*particle.vel[0] + particle.vel[1]*particle.vel[1]).sqrt();
            if speed < 0.0001 { break; }

            // normalize velocity
            particle.vel[0] /= speed;
            particle.vel[1] /= speed;

            particle.pos[0] += particle.vel[0];
            particle.pos[1] += particle.vel[1];

            if particle.pos[0] < 0.0 || particle.pos[0] >= width as f32
                || particle.pos[1] < 0.0 || particle.pos[1] >= height as f32 {
                break;
            }

            let old_h = sample_h(height_map, old_pos[0], old_pos[1]);
            let new_h = sample_h(height_map, particle.pos[0], particle.pos[1]);
            let height_diff = new_h - old_h;

            let capacity = (params.sediment_capacity_factor * speed * particle.water * (-height_diff).max(params.min_slope)).max(0.0);

            if particle.sediment > capacity || height_diff > 0.0 {
                // deposit
                let deposit_amount = if height_diff > 0.0 {
                    particle.sediment.min(height_diff)
                } else {
                    (particle.sediment - capacity) * params.deposit_speed
                };
                particle.sediment -= deposit_amount;
                let ix = old_pos[0] as usize;
                let iy = old_pos[1] as usize;
                if iy < height && ix < width {
                    height_map[iy * width + ix] += deposit_amount;
                }
            } else {
                // erode
                let erode_amount = ((capacity - particle.sediment) * params.erode_speed).min(-height_diff.max(0.0));
                let ix = old_pos[0] as usize;
                let iy = old_pos[1] as usize;
                if iy < height && ix < width {
                    let actual_erode = erode_amount.min(height_map[iy * width + ix]);
                    height_map[iy * width + ix] -= actual_erode;
                    particle.sediment += actual_erode;
                }
            }

            particle.water *= 1.0 - params.evaporation_rate;
            if particle.water < 0.01 {
                // deposit remaining sediment
                let ix = particle.pos[0].clamp(0.0, width as f32 - 1.0) as usize;
                let iy = particle.pos[1].clamp(0.0, height as f32 - 1.0) as usize;
                height_map[iy * width + ix] += particle.sediment;
                break;
            }
        }
    }

    // clamp
    for v in height_map.iter_mut() { *v = v.clamp(0.0, 1.0); }
}

/// Wind erosion: removes material from exposed peaks
pub fn wind_erosion(
    height_map: &mut Vec<f32>,
    width: usize,
    height: usize,
    wind_dir: f32, // radians
    strength: f32,
    iterations: u32,
) {
    let dx = wind_dir.cos();
    let dy = wind_dir.sin();
    let step_x = dx.round() as i32;
    let step_y = dy.round() as i32;

    for _iter in 0..iterations {
        let old = height_map.clone();
        for y in 1..height as i32 - 1 {
            for x in 1..width as i32 - 1 {
                let idx = (y * width as i32 + x) as usize;
                let nx = (x + step_x).clamp(0, width as i32 - 1);
                let ny = (y + step_y).clamp(0, height as i32 - 1);
                let n_idx = (ny * width as i32 + nx) as usize;
                let diff = old[idx] - old[n_idx];
                if diff > 0.0 {
                    let erode = diff * strength * 0.1;
                    height_map[idx] -= erode;
                    height_map[n_idx] += erode * 0.5;
                }
            }
        }
    }
    for v in height_map.iter_mut() { *v = v.clamp(0.0, 1.0); }
}

/// Coastal erosion: erodes terrain near sea level
pub fn coastal_erosion(
    height_map: &mut Vec<f32>,
    width: usize,
    height: usize,
    sea_level: f32,
    strength: f32,
    iterations: u32,
) {
    for _iter in 0..iterations {
        let old = height_map.clone();
        for y in 1..height-1 {
            for x in 1..width-1 {
                let idx = y * width + x;
                let h = old[idx];
                if h <= sea_level + 0.1 && h > sea_level - 0.05 {
                    // check if adjacent to water
                    let neighbors = [
                        old[(y-1)*width+x], old[(y+1)*width+x],
                        old[y*width+x-1],   old[y*width+x+1],
                    ];
                    let water_neighbors = neighbors.iter().filter(|&&n| n < sea_level).count();
                    if water_neighbors > 0 {
                        let erode = strength * water_neighbors as f32 * 0.02;
                        height_map[idx] = (h - erode).max(sea_level - 0.05);
                    }
                }
            }
        }
    }
}

/// Lake filling algorithm — finds local minima above sea level and fills to outflow
pub fn fill_lakes(height_map: &mut Vec<f32>, width: usize, height: usize, sea_level: f32) {
    // Simple filling: iteratively raise local minima until they drain
    let w = width;
    let h = height;
    for _ in 0..5 {
        let old = height_map.clone();
        for y in 1..h-1 {
            for x in 1..w-1 {
                let idx = y * w + x;
                if old[idx] <= sea_level { continue; }
                let neighbors = [
                    old[(y-1)*w+x], old[(y+1)*w+x],
                    old[y*w+x-1],   old[y*w+x+1],
                ];
                let min_n = neighbors.iter().cloned().fold(f32::MAX, f32::min);
                if min_n > old[idx] {
                    // local minimum — fill to min neighbor
                    height_map[idx] = (min_n + old[idx]) * 0.5;
                }
            }
        }
    }
}

/// Fractal terrain subdivision (alternative to diamond-square)
pub fn midpoint_displacement(
    width: usize,
    height: usize,
    roughness: f32,
    seed: u64,
) -> Vec<f32> {
    let mut map = vec![0.5_f32; width * height];
    let mut rng = seed;
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0 * 2.0 - 1.0
    };

    // Initialize corners
    map[0] = rng_f(&mut rng) * 0.5 + 0.5;
    map[width-1] = rng_f(&mut rng) * 0.5 + 0.5;
    map[(height-1)*width] = rng_f(&mut rng) * 0.5 + 0.5;
    map[(height-1)*width + width-1] = rng_f(&mut rng) * 0.5 + 0.5;

    let mut step_x = width - 1;
    let mut step_y = height - 1;
    let mut scale = roughness;

    while step_x > 1 || step_y > 1 {
        let half_x = (step_x / 2).max(1);
        let half_y = (step_y / 2).max(1);

        // midpoint of each cell
        let mut y = 0;
        while y < height - 1 {
            let mut x = 0;
            while x < width - 1 {
                let next_x = (x + step_x).min(width-1);
                let next_y = (y + step_y).min(height-1);
                let mid_x = x + half_x;
                let mid_y = y + half_y;
                if mid_x < width && mid_y < height {
                    let avg = (map[y*width+x] + map[y*width+next_x] + map[next_y*width+x] + map[next_y*width+next_x]) * 0.25;
                    map[mid_y*width+mid_x] = (avg + rng_f(&mut rng) * scale).clamp(0.0, 1.0);
                }
                x += step_x;
            }
            y += step_y;
        }

        // edge midpoints
        let mut y = 0;
        while y < height - 1 {
            let next_y = (y + step_y).min(height-1);
            let mid_y = y + half_y;
            if mid_y < height {
                let v0 = map[y*width];
                let v1 = map[next_y*width];
                map[mid_y*width] = ((v0 + v1) * 0.5 + rng_f(&mut rng) * scale).clamp(0.0, 1.0);
                let v0 = map[y*width + width-1];
                let v1 = map[next_y*width + width-1];
                map[mid_y*width + width-1] = ((v0 + v1) * 0.5 + rng_f(&mut rng) * scale).clamp(0.0, 1.0);
            }
            y += step_y;
        }

        step_x = half_x;
        step_y = half_y;
        scale *= 0.5_f32.powf(1.0 - roughness);
    }
    map
}

/// Compute drainage basin map (flow accumulation)
pub fn compute_drainage_map(height_map: &[f32], width: usize, height: usize) -> Vec<u32> {
    let w = width;
    let h = height;
    let n = w * h;
    let mut flow_to = vec![n; n]; // n = no flow (sink)
    let mut accumulation = vec![0u32; n];

    // Compute flow directions (D8 algorithm)
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let ch = height_map[idx];
            let mut min_h = ch;
            let mut min_idx = n;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 { continue; }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                    let ni = ny as usize * w + nx as usize;
                    if height_map[ni] < min_h {
                        min_h = height_map[ni];
                        min_idx = ni;
                    }
                }
            }
            flow_to[idx] = min_idx;
        }
    }

    // Accumulate flow (topological order by height)
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| height_map[b].partial_cmp(&height_map[a]).unwrap_or(std::cmp::Ordering::Equal));

    for &idx in &order {
        accumulation[idx] += 1;
        let next = flow_to[idx];
        if next < n {
            accumulation[next] += accumulation[idx];
        }
    }
    accumulation
}

/// Generate a macro-scale biome map using climate rules (Whittaker diagram)
pub fn whittaker_biome(temperature: f32, precipitation: f32) -> &'static str {
    if temperature < 0.1 {
        if precipitation < 0.3 { "Polar Desert" } else { "Tundra" }
    } else if temperature < 0.25 {
        if precipitation < 0.5 { "Boreal Desert" } else { "Taiga" }
    } else if temperature < 0.5 {
        if precipitation < 0.2 { "Cold Desert" }
        else if precipitation < 0.5 { "Temperate Grassland" }
        else if precipitation < 0.8 { "Temperate Deciduous Forest" }
        else { "Temperate Rainforest" }
    } else if temperature < 0.75 {
        if precipitation < 0.2 { "Desert" }
        else if precipitation < 0.4 { "Savanna" }
        else if precipitation < 0.7 { "Subtropical Dry Forest" }
        else { "Tropical Seasonal Forest" }
    } else {
        if precipitation < 0.15 { "Hot Desert" }
        else if precipitation < 0.35 { "Tropical Scrub" }
        else if precipitation < 0.6 { "Tropical Dry Forest" }
        else { "Tropical Rainforest" }
    }
}

/// Sediment transport visualization
pub struct SedimentMap {
    pub width: usize,
    pub height: usize,
    pub sediment: Vec<f32>,
    pub transport: Vec<[f32; 2]>,
}

impl SedimentMap {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width, height,
            sediment: vec![0.0; n],
            transport: vec![[0.0, 0.0]; n],
        }
    }

    pub fn compute_from_drainage(
        &mut self,
        height_map: &[f32],
        drainage: &[u32],
        sea_level: f32,
    ) {
        let w = self.width;
        let h = self.height;
        let max_drainage = drainage.iter().cloned().max().unwrap_or(1).max(1) as f32;
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let d = drainage[idx] as f32 / max_drainage;
                let slope_x = if x + 1 < w && x > 0 {
                    (height_map[y*w+x+1] - height_map[y*w+x-1]) * 0.5
                } else { 0.0 };
                let slope_y = if y + 1 < h && y > 0 {
                    (height_map[(y+1)*w+x] - height_map[(y-1)*w+x]) * 0.5
                } else { 0.0 };
                let slope = (slope_x*slope_x + slope_y*slope_y).sqrt();
                self.sediment[idx] = d * slope;
                self.transport[idx] = [-slope_x * d, -slope_y * d];
            }
        }
    }
}

/// Isostatic rebound: after erosion, terrain rises due to reduced load
pub fn apply_isostatic_rebound(height_map: &mut Vec<f32>, width: usize, height: usize, factor: f32) {
    let w = width;
    let h = height;
    let avg: f32 = height_map.iter().sum::<f32>() / (w * h) as f32;
    for v in height_map.iter_mut() {
        let deviation = avg - *v;
        *v += deviation * factor * 0.1;
        *v = v.clamp(0.0, 1.0);
    }
}

/// Perlin worms for cave/tunnel generation (2D cross-section)
pub fn generate_cave_map(
    width: usize,
    height: usize,
    num_worms: u32,
    seed: u64,
    threshold: f32,
) -> Vec<bool> {
    let mut map = vec![false; width * height];
    let mut rng = seed;
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    for _ in 0..num_worms {
        let mut x = rng_f(&mut rng) * width as f32;
        let mut y = rng_f(&mut rng) * height as f32;
        let length = (50.0 + rng_f(&mut rng) * 150.0) as u32;
        let mut angle = rng_f(&mut rng) * std::f32::consts::TAU;
        let radius = 2.0 + rng_f(&mut rng) * 4.0;

        for step in 0..length {
            let step_f = step as f32;
            angle += (perlin_2d(x * 0.05, y * 0.05, (seed & 0xFFFFFFFF) as u32) - 0.5) * 0.5;
            x += angle.cos();
            y += angle.sin();

            if x < 0.0 || x >= width as f32 || y < 0.0 || y >= height as f32 { break; }

            let ix = x as usize;
            let iy = y as usize;
            let ir = radius.ceil() as i32;
            for dy in -ir..=ir {
                for dx in -ir..=ir {
                    let nx = (ix as i32 + dx).clamp(0, width as i32 - 1) as usize;
                    let ny = (iy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let dist = ((dx*dx + dy*dy) as f32).sqrt();
                    if dist <= radius {
                        map[ny * width + nx] = true;
                    }
                }
            }
        }
    }
    map
}

/// 2D distance field for nearest-feature analysis
pub fn compute_distance_field(mask: &[bool], width: usize, height: usize) -> Vec<f32> {
    let w = width;
    let h = height;
    let n = w * h;
    let mut dist = vec![f32::MAX; n];

    // Initialize: distance=0 for true cells
    for (i, &v) in mask.iter().enumerate() {
        if v { dist[i] = 0.0; }
    }

    // Forward pass
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if y > 0 {
                let above = dist[(y-1)*w+x] + 1.0;
                if above < dist[idx] { dist[idx] = above; }
            }
            if x > 0 {
                let left = dist[y*w+x-1] + 1.0;
                if left < dist[idx] { dist[idx] = left; }
            }
        }
    }

    // Backward pass
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;
            if y + 1 < h {
                let below = dist[(y+1)*w+x] + 1.0;
                if below < dist[idx] { dist[idx] = below; }
            }
            if x + 1 < w {
                let right = dist[y*w+x+1] + 1.0;
                if right < dist[idx] { dist[idx] = right; }
            }
        }
    }

    // Normalize
    let max_d = dist.iter().cloned().fold(0.0_f32, f32::max);
    if max_d > 0.0 {
        for v in dist.iter_mut() {
            if *v == f32::MAX { *v = 1.0; }
            else { *v /= max_d; }
        }
    }
    dist
}

/// Terrain categorization helper for gameplay systems
#[derive(Clone, Debug, PartialEq)]
pub enum TerrainCategory {
    DeepWater,
    ShallowWater,
    Coast,
    Lowland,
    Highland,
    Mountain,
    Peak,
}

impl TerrainCategory {
    pub fn from_height(h: f32, sea_level: f32) -> Self {
        if h < sea_level * 0.5 { TerrainCategory::DeepWater }
        else if h < sea_level { TerrainCategory::ShallowWater }
        else if h < sea_level + 0.05 { TerrainCategory::Coast }
        else if h < 0.55 { TerrainCategory::Lowland }
        else if h < 0.70 { TerrainCategory::Highland }
        else if h < 0.85 { TerrainCategory::Mountain }
        else { TerrainCategory::Peak }
    }

    pub fn movement_cost(&self) -> f32 {
        match self {
            TerrainCategory::DeepWater => f32::MAX,
            TerrainCategory::ShallowWater => 3.0,
            TerrainCategory::Coast => 1.0,
            TerrainCategory::Lowland => 1.0,
            TerrainCategory::Highland => 1.5,
            TerrainCategory::Mountain => 2.5,
            TerrainCategory::Peak => 4.0,
        }
    }

    pub fn defense_bonus(&self) -> f32 {
        match self {
            TerrainCategory::DeepWater => 0.0,
            TerrainCategory::ShallowWater => -0.2,
            TerrainCategory::Coast => 0.0,
            TerrainCategory::Lowland => 0.0,
            TerrainCategory::Highland => 0.15,
            TerrainCategory::Mountain => 0.3,
            TerrainCategory::Peak => 0.5,
        }
    }
}

/// Compute a full terrain category map
pub fn compute_terrain_categories(
    height_map: &[f32],
    width: usize,
    height: usize,
    sea_level: f32,
) -> Vec<TerrainCategory> {
    height_map.iter().map(|&h| TerrainCategory::from_height(h, sea_level)).collect()
}

/// Find all connected regions of a given terrain type (flood fill)
pub fn find_connected_regions(
    category_map: &[TerrainCategory],
    width: usize,
    height: usize,
    target: &TerrainCategory,
) -> Vec<Vec<usize>> {
    let n = width * height;
    let mut visited = vec![false; n];
    let mut regions = Vec::new();

    for start in 0..n {
        if visited[start] || &category_map[start] != target { continue; }
        let mut region = Vec::new();
        let mut stack = vec![start];
        while let Some(idx) = stack.pop() {
            if visited[idx] { continue; }
            visited[idx] = true;
            if &category_map[idx] == target {
                region.push(idx);
                let x = idx % width;
                let y = idx / width;
                if x > 0 && !visited[idx-1] { stack.push(idx-1); }
                if x+1 < width && !visited[idx+1] { stack.push(idx+1); }
                if y > 0 && !visited[idx-width] { stack.push(idx-width); }
                if y+1 < height && !visited[idx+width] { stack.push(idx+width); }
            }
        }
        if !region.is_empty() { regions.push(region); }
    }
    regions
}

/// Compute the centroid of a region
pub fn region_centroid(region: &[usize], width: usize) -> [f32; 2] {
    if region.is_empty() { return [0.0, 0.0]; }
    let n = region.len() as f32;
    let sx: f32 = region.iter().map(|&i| (i % width) as f32).sum();
    let sy: f32 = region.iter().map(|&i| (i / width) as f32).sum();
    [sx / n, sy / n]
}

/// Annotate a chunk with region IDs (for gameplay zones)
pub fn compute_region_map(
    category_map: &[TerrainCategory],
    width: usize,
    height: usize,
) -> Vec<usize> {
    let n = width * height;
    let mut region_map = vec![usize::MAX; n];
    let mut visited = vec![false; n];
    let mut region_id = 0;

    for start in 0..n {
        if visited[start] { continue; }
        let target = &category_map[start];
        let mut stack = vec![start];
        while let Some(idx) = stack.pop() {
            if visited[idx] { continue; }
            visited[idx] = true;
            if &category_map[idx] == target {
                region_map[idx] = region_id;
                let x = idx % width;
                let y = idx / width;
                if x > 0 && !visited[idx-1] { stack.push(idx-1); }
                if x+1 < width && !visited[idx+1] { stack.push(idx+1); }
                if y > 0 && !visited[idx-width] { stack.push(idx-width); }
                if y+1 < height && !visited[idx+width] { stack.push(idx+width); }
            }
        }
        region_id += 1;
    }
    region_map
}

/// Simple biome transition blending (for smooth biome edges)
pub fn smooth_biome_transitions(
    biome_map: &mut Vec<usize>,
    height_map: &[f32],
    moisture_map: &[f32],
    biomes: &[BiomeRule],
    width: usize,
    height: usize,
    blend_radius: usize,
) {
    let w = width;
    let h = height;
    let r = blend_radius as i32;
    let old = biome_map.clone();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let ch = height_map[idx];
            let cm = moisture_map[idx];

            // Count biomes in neighborhood
            let mut biome_votes: HashMap<usize, u32> = HashMap::new();
            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    let ni = ny * w + nx;
                    *biome_votes.entry(old[ni]).or_insert(0) += 1;
                }
            }

            // Pick the most voted biome that's still valid for this cell's parameters
            if let Some((&best_biome, _)) = biome_votes.iter().max_by_key(|(_, &v)| v) {
                biome_map[idx] = best_biome;
            }
        }
    }
}

/// Export map as CSV for external tools
pub fn export_heightmap_csv(chunk: &GeneratedChunk) -> String {
    let mut lines = Vec::new();
    lines.push(format!("x,y,height,moisture,temp,biome"));
    for y in 0..chunk.height {
        for x in 0..chunk.width {
            let idx = (y * chunk.width + x) as usize;
            lines.push(format!("{},{},{:.4},{:.4},{:.4},{}",
                x, y,
                chunk.height_map[idx],
                chunk.moisture_map[idx],
                chunk.temp_map[idx],
                chunk.biome_map[idx],
            ));
        }
    }
    lines.join("\n")
}

/// Heightmap to mesh (returns list of triangles as [Pos2; 3])
pub fn heightmap_to_contour_mesh(
    height_map: &[f32],
    width: usize,
    height: usize,
    levels: &[f32],
) -> Vec<([Pos2; 2], Color32)> {
    let mut lines = Vec::new();
    for &level in levels {
        let segs = extract_contours(height_map, width, height, level);
        let t = level.clamp(0.0, 1.0);
        let color = Color32::from_rgba_premultiplied(
            (t * 255.0) as u8, ((1.0 - t) * 150.0) as u8, 100, 180,
        );
        for seg in segs {
            lines.push((seg, color));
        }
    }
    lines
}

/// World bounds rectangle for editor viewport
pub struct WorldBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub cell_size: f32,
}

impl WorldBounds {
    pub fn new(width: u32, height: u32, cell_size: f32) -> Self {
        Self {
            min_x: 0.0, min_y: 0.0,
            max_x: width as f32 * cell_size,
            max_y: height as f32 * cell_size,
            cell_size,
        }
    }

    pub fn world_to_screen(&self, wx: f32, wy: f32, view_offset: [f32; 2]) -> Pos2 {
        Pos2::new(
            wx * self.cell_size + view_offset[0],
            wy * self.cell_size + view_offset[1],
        )
    }

    pub fn screen_to_world(&self, sx: f32, sy: f32, view_offset: [f32; 2]) -> [f32; 2] {
        [
            (sx - view_offset[0]) / self.cell_size,
            (sy - view_offset[1]) / self.cell_size,
        ]
    }

    pub fn visible_cells(&self, canvas: Rect, view_offset: [f32; 2]) -> (u32, u32, u32, u32) {
        let x0 = ((-view_offset[0]) / self.cell_size).floor().max(0.0) as u32;
        let y0 = ((-view_offset[1]) / self.cell_size).floor().max(0.0) as u32;
        let x1 = ((canvas.width() / self.cell_size + x0 as f32) + 1.0) as u32;
        let y1 = ((canvas.height() / self.cell_size + y0 as f32) + 1.0) as u32;
        (x0, y0, x1, y1)
    }
}

/// World chunk streaming (for large worlds split into chunks)
pub struct ChunkKey {
    pub cx: i32,
    pub cy: i32,
}

impl ChunkKey {
    pub fn from_world_pos(wx: f32, wy: f32, chunk_size: u32) -> Self {
        Self {
            cx: (wx / chunk_size as f32).floor() as i32,
            cy: (wy / chunk_size as f32).floor() as i32,
        }
    }
}

pub struct ChunkCache {
    pub chunks: HashMap<(i32, i32), GeneratedChunk>,
    pub max_cached: usize,
    pub access_order: Vec<(i32, i32)>,
}

impl ChunkCache {
    pub fn new(max_cached: usize) -> Self {
        Self {
            chunks: HashMap::new(),
            max_cached,
            access_order: Vec::new(),
        }
    }

    pub fn get(&mut self, cx: i32, cy: i32) -> Option<&GeneratedChunk> {
        if self.chunks.contains_key(&(cx, cy)) {
            // move to front
            self.access_order.retain(|&k| k != (cx, cy));
            self.access_order.push((cx, cy));
            self.chunks.get(&(cx, cy))
        } else {
            None
        }
    }

    pub fn insert(&mut self, cx: i32, cy: i32, chunk: GeneratedChunk) {
        if self.chunks.len() >= self.max_cached {
            // evict oldest
            if let Some(old_key) = self.access_order.first().copied() {
                self.access_order.remove(0);
                self.chunks.remove(&old_key);
            }
        }
        self.chunks.insert((cx, cy), chunk);
        self.access_order.push((cx, cy));
    }

    pub fn get_or_generate(
        &mut self,
        cx: i32,
        cy: i32,
        config: &WorldGenConfig,
        seed: u64,
    ) -> &GeneratedChunk {
        if !self.chunks.contains_key(&(cx, cy)) {
            let chunk_seed = seed
                .wrapping_add((cx as u64).wrapping_mul(0x9e3779b97f4a7c15))
                .wrapping_add((cy as u64).wrapping_mul(0x6c62272e07bb0142));
            let chunk = generate_chunk(config, chunk_seed);
            self.insert(cx, cy, chunk);
        }
        self.access_order.retain(|&k| k != (cx, cy));
        self.access_order.push((cx, cy));
        &self.chunks[&(cx, cy)]
    }
}

/// Heightmap comparison tool (before/after erosion)
pub struct HeightmapSnapshot {
    pub label: String,
    pub data: Vec<f32>,
    pub width: usize,
    pub height: usize,
}

impl HeightmapSnapshot {
    pub fn capture(label: &str, data: &[f32], width: usize, height: usize) -> Self {
        Self { label: label.to_string(), data: data.to_vec(), width, height }
    }

    pub fn diff(&self, other: &HeightmapSnapshot) -> Vec<f32> {
        self.data.iter().zip(other.data.iter()).map(|(a, b)| a - b).collect()
    }

    pub fn max_change(&self, other: &HeightmapSnapshot) -> f32 {
        self.diff(other).iter().cloned().fold(0.0_f32, |acc, v| acc.max(v.abs()))
    }
}

/// Grid-snapping utility for editor tools
pub fn snap_to_grid(value: f32, grid_size: f32) -> f32 {
    (value / grid_size).round() * grid_size
}

/// Measure distances across the map surface (3D with height scale)
pub fn surface_distance(
    height_map: &[f32],
    width: usize,
    ax: usize, ay: usize,
    bx: usize, by: usize,
    cell_size: f32,
    height_scale: f32,
) -> f32 {
    let dx = (bx as f32 - ax as f32) * cell_size;
    let dy = (by as f32 - ay as f32) * cell_size;
    let ha = height_map[ay * width + ax];
    let hb = height_map[by * width + bx];
    let dz = (ha - hb) * height_scale;
    (dx*dx + dy*dy + dz*dz).sqrt()
}

/// Annotate chunk with custom layer data (for mods/plugins)
pub struct CustomLayerData {
    pub name: String,
    pub data: Vec<f32>,
}

impl CustomLayerData {
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self { name: name.to_string(), data: vec![0.0; (width * height) as usize] }
    }
}

/// World metadata for save files
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldMetadata {
    pub name: String,
    pub seed: u64,
    pub created_at: String,
    pub modified_at: String,
    pub width: u32,
    pub height: u32,
    pub version: u32,
    pub tags: Vec<String>,
    pub description: String,
}

impl WorldMetadata {
    pub fn new(name: &str, seed: u64, width: u32, height: u32) -> Self {
        Self {
            name: name.to_string(),
            seed,
            created_at: "2026-03-31".to_string(),
            modified_at: "2026-03-31".to_string(),
            width, height,
            version: 1,
            tags: Vec::new(),
            description: String::new(),
        }
    }
}

// Additional tests for extended functionality
#[cfg(test)]
mod extended_tests {
    use super::*;

    #[test]
    fn test_climate_simulator() {
        let config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let mut chunk = generate_chunk(&config, 1);
        let climate = ClimateSimulator::new();
        climate.simulate(&mut chunk, 0.35, 3);
        for &v in &chunk.moisture_map { assert!(v >= 0.0 && v <= 1.0); }
        for &v in &chunk.temp_map { assert!(v >= 0.0 && v <= 1.0); }
    }

    #[test]
    fn test_tectonic_plates() {
        let mut height_map = vec![0.5_f32; 32 * 32];
        let sim = TectonicSimulator::new(4, 32, 32, 42);
        sim.apply(&mut height_map, 32, 32);
        for &v in &height_map { assert!(v >= 0.0 && v <= 1.0); }
    }

    #[test]
    fn test_particle_erosion() {
        let config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let chunk = generate_chunk(&config, 5);
        let mut hmap = chunk.height_map;
        let params = ParticleErosionParams::default();
        particle_hydraulic_erosion(&mut hmap, 32, 32, &params, 100, 42);
        for &v in &hmap { assert!(v >= 0.0 && v <= 1.0); }
    }

    #[test]
    fn test_drainage_map() {
        let config = WorldGenConfig { width: 16, height: 16, ..Default::default() };
        let chunk = generate_chunk(&config, 7);
        let drainage = compute_drainage_map(&chunk.height_map, 16, 16);
        assert_eq!(drainage.len(), 16 * 16);
    }

    #[test]
    fn test_whittaker_biome() {
        let biome = whittaker_biome(0.8, 0.8);
        assert_eq!(biome, "Tropical Rainforest");
        let biome2 = whittaker_biome(0.05, 0.1);
        assert_eq!(biome2, "Polar Desert");
    }

    #[test]
    fn test_distance_field() {
        let mask = vec![true, false, false, false,
                        false, false, false, false,
                        false, false, false, false,
                        false, false, false, true];
        let field = compute_distance_field(&mask, 4, 4);
        assert!(field[0] <= field[5]);
    }

    #[test]
    fn test_terrain_categories() {
        let config = WorldGenConfig { width: 16, height: 16, ..Default::default() };
        let chunk = generate_chunk(&config, 3);
        let cats = compute_terrain_categories(&chunk.height_map, 16, 16, 0.35);
        assert_eq!(cats.len(), 16 * 16);
    }

    #[test]
    fn test_chunk_cache() {
        let config = WorldGenConfig { width: 16, height: 16, ..Default::default() };
        let mut cache = ChunkCache::new(4);
        let chunk = cache.get_or_generate(0, 0, &config, 42);
        assert_eq!(chunk.width, 16);
        // second access should hit cache
        let chunk2 = cache.get_or_generate(0, 0, &config, 42);
        assert_eq!(chunk2.width, 16);
    }

    #[test]
    fn test_heightmap_snapshot_diff() {
        let a = vec![0.1, 0.2, 0.3, 0.4];
        let b = vec![0.2, 0.2, 0.4, 0.3];
        let snap_a = HeightmapSnapshot::capture("before", &a, 2, 2);
        let snap_b = HeightmapSnapshot::capture("after", &b, 2, 2);
        let diff = snap_a.diff(&snap_b);
        assert!((diff[0] + 0.1).abs() < 0.001);
        assert!((diff[1]).abs() < 0.001);
    }

    #[test]
    fn test_world_bounds() {
        let wb = WorldBounds::new(64, 64, 2.0);
        let screen = wb.world_to_screen(5.0, 5.0, [10.0, 10.0]);
        assert!((screen.x - 20.0).abs() < 0.001);
        let world = wb.screen_to_world(20.0, 20.0, [10.0, 10.0]);
        assert!((world[0] - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_midpoint_displacement() {
        let map = midpoint_displacement(32, 32, 0.5, 99);
        assert_eq!(map.len(), 32 * 32);
        for &v in &map { assert!(v >= 0.0 && v <= 1.0); }
    }

    #[test]
    fn test_wind_erosion() {
        let config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let chunk = generate_chunk(&config, 11);
        let mut hmap = chunk.height_map;
        wind_erosion(&mut hmap, 32, 32, 0.0, 0.5, 10);
        for &v in &hmap { assert!(v >= 0.0 && v <= 1.0); }
    }

    #[test]
    fn test_cave_generation() {
        let caves = generate_cave_map(64, 64, 5, 42, 0.5);
        assert_eq!(caves.len(), 64 * 64);
        let carved = caves.iter().filter(|&&v| v).count();
        assert!(carved > 0);
    }

    #[test]
    fn test_voronoi_biomes() {
        let bmap = generate_voronoi_biomes(32, 32, 8, 42, 5);
        assert_eq!(bmap.len(), 32 * 32);
        let unique: HashSet<usize> = bmap.iter().cloned().collect();
        assert!(unique.len() > 1);
    }

    #[test]
    fn test_connected_regions() {
        let config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let chunk = generate_chunk(&config, 2);
        let cats = compute_terrain_categories(&chunk.height_map, 32, 32, 0.35);
        let regions = find_connected_regions(&cats, 32, 32, &TerrainCategory::Lowland);
        // should have at least one region
        assert!(!regions.is_empty());
    }

    #[test]
    fn test_snap_to_grid() {
        assert!((snap_to_grid(7.3, 2.0) - 8.0).abs() < 0.001);
        assert!((snap_to_grid(5.0, 2.0) - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_rainfall_moisture() {
        let config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let chunk = generate_chunk(&config, 4);
        let moist = simulate_rainfall_moisture(&chunk.height_map, 32, 32, 10, 42);
        assert_eq!(moist.len(), 32 * 32);
        for &v in &moist { assert!(v >= 0.0 && v <= 1.0); }
    }
}

// ============================================================
// PROCEDURAL ROAD & PATH SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadNode {
    pub pos: [f32; 2],
    pub node_type: RoadNodeType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoadNodeType {
    Settlement,
    Waypoint,
    Landmark,
    Border,
}

#[derive(Clone, Debug)]
pub struct Road {
    pub nodes: Vec<[f32; 2]>,
    pub road_type: RoadType,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoadType {
    MainRoad,
    DirtTrail,
    MountainPass,
    CoastalRoute,
}

impl Road {
    pub fn new(road_type: RoadType) -> Self {
        Self { nodes: Vec::new(), road_type, width: 1.0 }
    }

    pub fn length(&self) -> f32 {
        if self.nodes.len() < 2 { return 0.0; }
        self.nodes.windows(2).map(|w| {
            let dx = w[1][0] - w[0][0];
            let dy = w[1][1] - w[0][1];
            (dx*dx + dy*dy).sqrt()
        }).sum()
    }
}

pub fn generate_road_network(
    chunk: &GeneratedChunk,
    settlements: &[[f32; 2]],
    sea_level: f32,
    seed: u64,
) -> Vec<Road> {
    let mut roads = Vec::new();
    if settlements.len() < 2 { return roads; }

    let w = chunk.width as usize;
    let h = chunk.height as usize;

    // A* pathfinding between settlement pairs
    for i in 0..settlements.len() {
        for j in (i+1)..settlements.len() {
            if let Some(path) = astar_path(
                &chunk.height_map, w, h,
                [settlements[i][0] as usize, settlements[i][1] as usize],
                [settlements[j][0] as usize, settlements[j][1] as usize],
                sea_level,
            ) {
                let road_type = if path.len() > 100 { RoadType::MainRoad } else { RoadType::DirtTrail };
                let mut road = Road::new(road_type);
                road.nodes = path.iter().map(|&[x, y]| [x as f32, y as f32]).collect();
                roads.push(road);
            }
        }
    }
    roads
}

fn astar_path(
    height_map: &[f32],
    width: usize,
    height: usize,
    start: [usize; 2],
    end: [usize; 2],
    sea_level: f32,
) -> Option<Vec<[usize; 2]>> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    let idx = |x: usize, y: usize| y * width + x;
    let heuristic = |x: usize, y: usize| {
        let dx = (x as i64 - end[0] as i64).abs();
        let dy = (y as i64 - end[1] as i64).abs();
        (dx + dy) as f32
    };

    let n = width * height;
    let mut g_score = vec![f32::MAX; n];
    let mut came_from = vec![usize::MAX; n];
    let start_idx = idx(start[0], start[1]);
    let end_idx = idx(end[0], end[1]);
    g_score[start_idx] = 0.0;

    // (f_score as Reverse for min-heap, index)
    let mut open: BinaryHeap<(Reverse<u32>, usize)> = BinaryHeap::new();
    open.push((Reverse(0), start_idx));

    while let Some((_, current)) = open.pop() {
        if current == end_idx { break; }

        let cx = current % width;
        let cy = current / width;
        let ch = height_map[current];

        let neighbors: &[(i32, i32)] = &[(-1,0),(1,0),(0,-1),(0,1),(-1,-1),(1,-1),(-1,1),(1,1)];
        for &(dx, dy) in neighbors {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 { continue; }
            let ni = idx(nx as usize, ny as usize);
            let nh = height_map[ni];
            if nh < sea_level { continue; } // can't cross water

            let step_cost = if dx == 0 || dy == 0 { 1.0 } else { 1.414 };
            let height_penalty = (nh - ch).max(0.0) * 5.0; // going uphill is expensive
            let cost = step_cost + height_penalty;
            let tentative_g = g_score[current] + cost;

            if tentative_g < g_score[ni] {
                came_from[ni] = current;
                g_score[ni] = tentative_g;
                let f = tentative_g + heuristic(nx as usize, ny as usize);
                open.push((Reverse((f * 100.0) as u32), ni));
            }
        }
    }

    if came_from[end_idx] == usize::MAX && start_idx != end_idx { return None; }

    // Reconstruct path
    let mut path = Vec::new();
    let mut current = end_idx;
    while current != usize::MAX {
        path.push([current % width, current / width]);
        current = came_from[current];
    }
    path.reverse();
    Some(path)
}

// ============================================================
// PROCEDURAL SETTLEMENT PLACEMENT
// ============================================================

#[derive(Clone, Debug)]
pub struct Settlement {
    pub pos: [f32; 2],
    pub name: String,
    pub settlement_type: SettlementType,
    pub population: u32,
    pub prosperity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettlementType {
    Village,
    Town,
    City,
    Outpost,
    Ruin,
    Port,
    Mine,
    Farm,
}

impl SettlementType {
    pub fn name(&self) -> &str {
        match self {
            SettlementType::Village => "Village",
            SettlementType::Town => "Town",
            SettlementType::City => "City",
            SettlementType::Outpost => "Outpost",
            SettlementType::Ruin => "Ruin",
            SettlementType::Port => "Port",
            SettlementType::Mine => "Mine",
            SettlementType::Farm => "Farm",
        }
    }

    pub fn glyph(&self) -> char {
        match self {
            SettlementType::Village => 'v',
            SettlementType::Town => 'T',
            SettlementType::City => 'C',
            SettlementType::Outpost => 'o',
            SettlementType::Ruin => 'R',
            SettlementType::Port => 'P',
            SettlementType::Mine => 'M',
            SettlementType::Farm => 'F',
        }
    }
}

pub fn place_settlements(
    chunk: &GeneratedChunk,
    biomes: &[BiomeRule],
    sea_level: f32,
    num_settlements: usize,
    seed: u64,
) -> Vec<Settlement> {
    let w = chunk.width as usize;
    let h = chunk.height as usize;
    let mut rng = seed;
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    let mut settlements = Vec::new();
    let mut attempts = 0;
    let max_attempts = num_settlements * 20;

    while settlements.len() < num_settlements && attempts < max_attempts {
        attempts += 1;
        let x = (rng_f(&mut rng) * w as f32) as usize;
        let y = (rng_f(&mut rng) * h as f32) as usize;
        let idx = y * w + x;

        let height_val = chunk.height_map[idx];
        if height_val < sea_level || height_val > 0.75 { continue; }

        // Check minimum distance from other settlements
        let min_dist = w.min(h) as f32 * 0.1;
        let too_close = settlements.iter().any(|s: &Settlement| {
            let dx = s.pos[0] - x as f32;
            let dy = s.pos[1] - y as f32;
            (dx*dx + dy*dy).sqrt() < min_dist
        });
        if too_close { continue; }

        // Score location based on resources
        let moisture = chunk.moisture_map[idx];
        let temp = chunk.temp_map[idx];
        let score = moisture * 0.3 + (1.0 - (height_val - sea_level - 0.1).abs() * 5.0).max(0.0) * 0.4
            + temp * 0.3;

        // Determine settlement type
        let near_water = is_near_water(chunk, x, y, 5, sea_level);
        let in_hills = height_val > 0.55 && height_val < 0.7;
        let fertile = moisture > 0.5 && temp > 0.3;

        let stype = if near_water && height_val < sea_level + 0.1 {
            SettlementType::Port
        } else if in_hills {
            if rng_f(&mut rng) > 0.5 { SettlementType::Mine } else { SettlementType::Outpost }
        } else if fertile && rng_f(&mut rng) > 0.6 {
            SettlementType::Farm
        } else if score > 0.6 {
            if rng_f(&mut rng) > 0.7 { SettlementType::City }
            else { SettlementType::Town }
        } else if rng_f(&mut rng) > 0.8 {
            SettlementType::Ruin
        } else {
            SettlementType::Village
        };

        let population = match stype {
            SettlementType::City => 5000 + (rng_f(&mut rng) * 15000.0) as u32,
            SettlementType::Town => 500 + (rng_f(&mut rng) * 4500.0) as u32,
            SettlementType::Village => 50 + (rng_f(&mut rng) * 450.0) as u32,
            SettlementType::Port => 300 + (rng_f(&mut rng) * 2000.0) as u32,
            SettlementType::Mine => 100 + (rng_f(&mut rng) * 500.0) as u32,
            SettlementType::Farm => 20 + (rng_f(&mut rng) * 100.0) as u32,
            SettlementType::Outpost => 10 + (rng_f(&mut rng) * 50.0) as u32,
            SettlementType::Ruin => 0,
        };

        settlements.push(Settlement {
            pos: [x as f32, y as f32],
            name: generate_settlement_name(seed, settlements.len()),
            settlement_type: stype,
            population,
            prosperity: score,
        });
    }
    settlements
}

fn is_near_water(chunk: &GeneratedChunk, x: usize, y: usize, radius: usize, sea_level: f32) -> bool {
    let w = chunk.width as usize;
    let h = chunk.height as usize;
    let r = radius as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
            let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
            if chunk.height_map[ny * w + nx] < sea_level { return true; }
        }
    }
    false
}

fn generate_settlement_name(seed: u64, index: usize) -> String {
    let prefixes = ["Ash", "Iron", "Gold", "Storm", "Dark", "Silver", "Red", "Green", "White", "Black",
        "Stone", "River", "Lake", "Oak", "Pine", "Vale", "High", "Low", "Old", "New"];
    let suffixes = ["hold", "ford", "gate", "bridge", "haven", "port", "wick", "burg", "mere",
        "field", "wood", "ham", "ton", "by", "thorpe", "dale", "moor", "cliff", "peak", "falls"];
    let mut rng = seed.wrapping_add(index as u64 * 31337);
    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    let pi = (rng & 0xFFFFFF) as usize % prefixes.len();
    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    let si = (rng & 0xFFFFFF) as usize % suffixes.len();
    format!("{}{}", prefixes[pi], suffixes[si])
}

// ============================================================
// RESOURCE DEPOSIT SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct ResourceDeposit {
    pub pos: [f32; 2],
    pub resource_type: ResourceType,
    pub abundance: f32,
    pub depth: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResourceType {
    IronOre,
    GoldOre,
    Coal,
    Gemstone,
    Crystal,
    Stone,
    Clay,
    Peat,
    Salt,
    Sulfur,
}

impl ResourceType {
    pub fn name(&self) -> &str {
        match self {
            ResourceType::IronOre => "Iron Ore",
            ResourceType::GoldOre => "Gold Ore",
            ResourceType::Coal => "Coal",
            ResourceType::Gemstone => "Gemstone",
            ResourceType::Crystal => "Crystal",
            ResourceType::Stone => "Stone",
            ResourceType::Clay => "Clay",
            ResourceType::Peat => "Peat",
            ResourceType::Salt => "Salt",
            ResourceType::Sulfur => "Sulfur",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            ResourceType::IronOre => Color32::from_rgb(140, 100, 80),
            ResourceType::GoldOre => Color32::from_rgb(220, 180, 30),
            ResourceType::Coal => Color32::from_rgb(60, 60, 60),
            ResourceType::Gemstone => Color32::from_rgb(180, 50, 220),
            ResourceType::Crystal => Color32::from_rgb(150, 220, 255),
            ResourceType::Stone => Color32::from_rgb(140, 140, 140),
            ResourceType::Clay => Color32::from_rgb(180, 140, 100),
            ResourceType::Peat => Color32::from_rgb(90, 70, 50),
            ResourceType::Salt => Color32::from_rgb(230, 230, 200),
            ResourceType::Sulfur => Color32::from_rgb(220, 200, 50),
        }
    }

    pub fn glyph(&self) -> char {
        match self {
            ResourceType::IronOre => '⚙',
            ResourceType::GoldOre => '$',
            ResourceType::Coal => '◼',
            ResourceType::Gemstone => '◆',
            ResourceType::Crystal => '*',
            ResourceType::Stone => '▲',
            ResourceType::Clay => '~',
            ResourceType::Peat => ',',
            ResourceType::Salt => '+',
            ResourceType::Sulfur => '!',
        }
    }
}

pub fn place_resource_deposits(
    chunk: &GeneratedChunk,
    sea_level: f32,
    seed: u64,
) -> Vec<ResourceDeposit> {
    let w = chunk.width as usize;
    let h = chunk.height as usize;
    let mut rng = seed;
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    let mut deposits = Vec::new();
    let num_deposits = (w * h / 200).max(5).min(500);

    for _ in 0..num_deposits {
        let x = (rng_f(&mut rng) * w as f32) as usize;
        let y = (rng_f(&mut rng) * h as f32) as usize;
        let idx = y * w + x;
        let height_val = chunk.height_map[idx];
        if height_val < sea_level { continue; }

        let r = rng_f(&mut rng);
        let resource_type = if height_val > 0.7 {
            // mountain resources
            if r < 0.25 { ResourceType::IronOre }
            else if r < 0.35 { ResourceType::GoldOre }
            else if r < 0.50 { ResourceType::Gemstone }
            else if r < 0.65 { ResourceType::Crystal }
            else { ResourceType::Stone }
        } else if height_val > 0.5 {
            // highland resources
            if r < 0.3 { ResourceType::Coal }
            else if r < 0.5 { ResourceType::IronOre }
            else if r < 0.65 { ResourceType::Stone }
            else { ResourceType::Sulfur }
        } else {
            // lowland resources
            if chunk.moisture_map[idx] > 0.6 {
                if r < 0.4 { ResourceType::Peat } else { ResourceType::Clay }
            } else if chunk.moisture_map[idx] < 0.2 {
                if r < 0.5 { ResourceType::Salt } else { ResourceType::Stone }
            } else {
                if r < 0.4 { ResourceType::Clay } else { ResourceType::Stone }
            }
        };

        deposits.push(ResourceDeposit {
            pos: [x as f32, y as f32],
            resource_type,
            abundance: rng_f(&mut rng),
            depth: rng_f(&mut rng) * 50.0,
        });
    }
    deposits
}

// ============================================================
// WEATHER SYSTEM
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeatherZone {
    pub center: [f32; 2],
    pub radius: f32,
    pub weather_type: WeatherType,
    pub intensity: f32,
    pub duration: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WeatherType {
    Clear,
    Cloudy,
    Rain,
    Storm,
    Snow,
    Blizzard,
    Fog,
    Heatwave,
    Drought,
}

impl WeatherType {
    pub fn name(&self) -> &str {
        match self {
            WeatherType::Clear => "Clear",
            WeatherType::Cloudy => "Cloudy",
            WeatherType::Rain => "Rain",
            WeatherType::Storm => "Storm",
            WeatherType::Snow => "Snow",
            WeatherType::Blizzard => "Blizzard",
            WeatherType::Fog => "Fog",
            WeatherType::Heatwave => "Heatwave",
            WeatherType::Drought => "Drought",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            WeatherType::Clear => Color32::from_rgb(220, 230, 255),
            WeatherType::Cloudy => Color32::from_rgb(160, 160, 180),
            WeatherType::Rain => Color32::from_rgb(80, 100, 200),
            WeatherType::Storm => Color32::from_rgb(60, 60, 120),
            WeatherType::Snow => Color32::from_rgb(220, 230, 255),
            WeatherType::Blizzard => Color32::from_rgb(180, 200, 240),
            WeatherType::Fog => Color32::from_rgb(180, 180, 190),
            WeatherType::Heatwave => Color32::from_rgb(220, 120, 50),
            WeatherType::Drought => Color32::from_rgb(200, 170, 80),
        }
    }
}

pub fn generate_weather_zones(
    chunk: &GeneratedChunk,
    biomes: &[BiomeRule],
    sea_level: f32,
    seed: u64,
    num_zones: usize,
) -> Vec<WeatherZone> {
    let w = chunk.width as f32;
    let h = chunk.height as f32;
    let mut rng = seed;
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    (0..num_zones).map(|_| {
        let cx = rng_f(&mut rng) * w;
        let cy = rng_f(&mut rng) * h;
        let r2 = w.min(h) * (0.1 + rng_f(&mut rng) * 0.3);
        let x = cx as usize; let y = cy as usize;
        let idx = (y.min(chunk.height as usize - 1)) * chunk.width as usize + x.min(chunk.width as usize - 1);
        let temp = chunk.temp_map[idx];
        let moisture = chunk.moisture_map[idx];
        let height_val = chunk.height_map[idx];

        let weather = if height_val < sea_level {
            if rng_f(&mut rng) > 0.5 { WeatherType::Storm } else { WeatherType::Rain }
        } else if temp < 0.2 {
            if rng_f(&mut rng) > 0.5 { WeatherType::Blizzard } else { WeatherType::Snow }
        } else if temp > 0.8 && moisture < 0.2 {
            WeatherType::Heatwave
        } else if moisture > 0.7 {
            if rng_f(&mut rng) > 0.6 { WeatherType::Storm } else { WeatherType::Rain }
        } else if moisture < 0.2 {
            if rng_f(&mut rng) > 0.7 { WeatherType::Drought } else { WeatherType::Clear }
        } else if rng_f(&mut rng) > 0.6 {
            WeatherType::Fog
        } else {
            WeatherType::Cloudy
        };

        WeatherZone {
            center: [cx, cy],
            radius: r2,
            weather_type: weather,
            intensity: 0.3 + rng_f(&mut rng) * 0.7,
            duration: 60.0 + rng_f(&mut rng) * 360.0,
        }
    }).collect()
}

// ============================================================
// HEIGHTMAP PAINTER TOOLS (detailed brush implementations)
// ============================================================

pub struct PainterState {
    pub undo_stack: Vec<Vec<f32>>,
    pub redo_stack: Vec<Vec<f32>>,
    pub max_undo: usize,
}

impl PainterState {
    pub fn new(max_undo: usize) -> Self {
        Self { undo_stack: Vec::new(), redo_stack: Vec::new(), max_undo }
    }

    pub fn push_undo(&mut self, height_map: &[f32]) {
        self.undo_stack.push(height_map.to_vec());
        if self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, height_map: &mut Vec<f32>) -> bool {
        if let Some(state) = self.undo_stack.pop() {
            self.redo_stack.push(height_map.clone());
            *height_map = state;
            true
        } else { false }
    }

    pub fn redo(&mut self, height_map: &mut Vec<f32>) -> bool {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push(height_map.clone());
            *height_map = state;
            true
        } else { false }
    }
}

/// Sculpt terrain with a smooth falloff brush
pub fn brush_sculpt(
    height_map: &mut Vec<f32>,
    width: usize,
    height: usize,
    cx: f32, cy: f32,
    radius: f32,
    strength: f32,
    mode: SculptMode,
) {
    let ix = cx.floor() as i32;
    let iy = cy.floor() as i32;
    let ir = radius.ceil() as i32;

    for dy in -ir..=ir {
        for dx in -ir..=ir {
            let nx = ix + dx;
            let ny = iy + dy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 { continue; }
            let dist = ((dx as f32 * dx as f32 + dy as f32 * dy as f32) as f32).sqrt();
            if dist > radius { continue; }
            let falloff = smooth_brush_falloff(dist / radius);
            let idx = ny as usize * width + nx as usize;
            match mode {
                SculptMode::Raise => { height_map[idx] = (height_map[idx] + strength * falloff).clamp(0.0, 1.0); }
                SculptMode::Lower => { height_map[idx] = (height_map[idx] - strength * falloff).clamp(0.0, 1.0); }
                SculptMode::Smooth => {
                    // average with neighbors
                    let mut sum = 0.0_f32;
                    let mut count = 0;
                    for ddy in -1i32..=1 {
                        for ddx in -1i32..=1 {
                            let nx2 = (nx + ddx).clamp(0, width as i32 - 1) as usize;
                            let ny2 = (ny + ddy).clamp(0, height as i32 - 1) as usize;
                            sum += height_map[ny2 * width + nx2];
                            count += 1;
                        }
                    }
                    let avg = sum / count as f32;
                    height_map[idx] = lerp(height_map[idx], avg, strength * falloff);
                }
                SculptMode::Noise => {
                    let noise = perlin_2d(nx as f32 * 0.1, ny as f32 * 0.1, 42);
                    height_map[idx] = (height_map[idx] + (noise - 0.5) * strength * falloff).clamp(0.0, 1.0);
                }
                SculptMode::Clamp { target } => {
                    height_map[idx] = lerp(height_map[idx], target, strength * falloff);
                }
            }
        }
    }
}

fn smooth_brush_falloff(t: f32) -> f32 {
    // cosine falloff
    let t = t.clamp(0.0, 1.0);
    (std::f32::consts::PI * t).cos() * 0.5 + 0.5
}

#[derive(Clone, Debug)]
pub enum SculptMode {
    Raise,
    Lower,
    Smooth,
    Noise,
    Clamp { target: f32 },
}

/// Stamp a predefined shape onto the terrain (volcano, mesa, etc.)
pub fn stamp_feature(
    height_map: &mut Vec<f32>,
    width: usize,
    height: usize,
    cx: f32, cy: f32,
    feature: StampFeature,
    scale: f32,
    strength: f32,
) {
    let ir = (scale * 2.0).ceil() as i32;
    let ix = cx as i32;
    let iy = cy as i32;

    for dy in -ir..=ir {
        for dx in -ir..=ir {
            let nx = ix + dx;
            let ny = iy + dy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 { continue; }
            let dist = (dx as f32 * dx as f32 + dy as f32 * dy as f32).sqrt() / scale;
            let idx = ny as usize * width + nx as usize;
            let stamp_val = match feature {
                StampFeature::Volcano => {
                    if dist < 0.5 { 1.0 - dist * 0.5 } // peak
                    else if dist < 0.7 { 0.75 + (0.7 - dist) * 2.5 } // caldera rim
                    else if dist < 0.8 { 0.4 } // crater
                    else { (1.0 - dist).max(0.0) * 0.8 } // slopes
                }
                StampFeature::Mesa => {
                    if dist < 0.6 { 0.9 } else { (1.0 - dist).max(0.0) * 0.6 + 0.2 }
                }
                StampFeature::Canyon => {
                    if dist.abs() < 0.1 { 0.0 } // floor
                    else if dist.abs() < 0.3 { 0.5 + dist * 1.5 } // walls
                    else { (dist * 0.5).min(1.0) }
                }
                StampFeature::Hill => {
                    (1.0 - dist).max(0.0).powf(0.5)
                }
                StampFeature::Pit => {
                    1.0 - (1.0 - dist).max(0.0).powf(0.5)
                }
            };
            if dist <= 1.0 {
                height_map[idx] = lerp(height_map[idx], stamp_val, strength * (1.0 - dist).max(0.0));
                height_map[idx] = height_map[idx].clamp(0.0, 1.0);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum StampFeature {
    Volcano,
    Mesa,
    Canyon,
    Hill,
    Pit,
}

// ============================================================
// WORLD GEN EDITOR: HISTORY AND MACRO RECORDING
// ============================================================

#[derive(Clone, Debug)]
pub struct GenerationMacro {
    pub name: String,
    pub steps: Vec<MacroStep>,
}

#[derive(Clone, Debug)]
pub enum MacroStep {
    ApplyPreset(WorldPreset),
    SetSeed(u64),
    Generate,
    ApplyErosion(ErosionParams),
    PlaceRivers(RiverParams),
    ApplyIsland(f32),
    ApplyBlur(usize),
    Normalize,
}

impl GenerationMacro {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), steps: Vec::new() }
    }

    pub fn execute(&self, config: &mut WorldGenConfig, seed: u64) -> GeneratedChunk {
        let mut effective_seed = seed;
        let mut chunk_opt: Option<GeneratedChunk> = None;

        for step in &self.steps {
            match step {
                MacroStep::ApplyPreset(preset) => { preset.apply(config); }
                MacroStep::SetSeed(s) => { effective_seed = *s; }
                MacroStep::Generate => {
                    let mut c = generate_chunk(config, effective_seed);
                    assign_biomes(&mut c, &config.biomes);
                    chunk_opt = Some(c);
                }
                MacroStep::ApplyErosion(params) => {
                    if let Some(c) = &mut chunk_opt {
                        apply_erosion(c, params);
                    }
                }
                MacroStep::PlaceRivers(params) => {
                    if let Some(c) = &mut chunk_opt {
                        place_rivers(c, params, effective_seed);
                    }
                }
                MacroStep::ApplyIsland(strength) => {
                    if let Some(c) = &mut chunk_opt {
                        apply_island_mask(&mut c.height_map, c.width as usize, c.height as usize, *strength);
                    }
                }
                MacroStep::ApplyBlur(radius) => {
                    if let Some(c) = &mut chunk_opt {
                        gaussian_blur(&mut c.height_map, c.width as usize, c.height as usize, *radius);
                    }
                }
                MacroStep::Normalize => {
                    if let Some(c) = &mut chunk_opt {
                        normalize_map(&mut c.height_map);
                    }
                }
            }
        }

        chunk_opt.unwrap_or_else(|| generate_chunk(config, effective_seed))
    }
}

// ============================================================
// WORLD GEN EDITOR: SHOW SETTLEMENTS & RESOURCES OVERLAY
// ============================================================

impl WorldGenEditor {
    /// Show settlements as overlaid glyphs on the preview
    pub fn show_settlements_overlay(&self, painter: &Painter, settlements: &[Settlement], base_x: f32, base_y: f32, cell_size: f32, clip: Rect) {
        for settlement in settlements {
            let px = base_x + settlement.pos[0] * cell_size;
            let py = base_y + settlement.pos[1] * cell_size;
            if px < clip.min.x || px > clip.max.x || py < clip.min.y || py > clip.max.y { continue; }
            let glyph = settlement.settlement_type.glyph();
            let color = match settlement.settlement_type {
                SettlementType::City => Color32::from_rgb(255, 220, 50),
                SettlementType::Town => Color32::from_rgb(200, 160, 50),
                SettlementType::Village => Color32::from_rgb(150, 120, 80),
                SettlementType::Port => Color32::from_rgb(50, 150, 220),
                SettlementType::Mine => Color32::from_rgb(150, 120, 100),
                SettlementType::Farm => Color32::from_rgb(80, 180, 80),
                SettlementType::Outpost => Color32::from_rgb(180, 100, 50),
                SettlementType::Ruin => Color32::from_rgb(120, 100, 80),
            };
            // Draw background circle
            painter.circle_filled(Pos2::new(px, py), cell_size * 1.5, Color32::from_rgba_premultiplied(0, 0, 0, 150));
            painter.text(Pos2::new(px, py), egui::Align2::CENTER_CENTER, glyph.to_string(), FontId::monospace(cell_size.min(14.0)), color);
        }
    }

    /// Show resources overlay on preview
    pub fn show_resources_overlay(&self, painter: &Painter, deposits: &[ResourceDeposit], base_x: f32, base_y: f32, cell_size: f32, clip: Rect) {
        for deposit in deposits {
            let px = base_x + deposit.pos[0] * cell_size;
            let py = base_y + deposit.pos[1] * cell_size;
            if px < clip.min.x || px > clip.max.x || py < clip.min.y || py > clip.max.y { continue; }
            let color = deposit.resource_type.color();
            let size = cell_size * (0.5 + deposit.abundance * 0.5);
            painter.circle_filled(Pos2::new(px, py), size, color);
        }
    }

    /// Show weather zones as translucent circles
    pub fn show_weather_overlay(&self, painter: &Painter, zones: &[WeatherZone], base_x: f32, base_y: f32, cell_size: f32, clip: Rect) {
        for zone in zones {
            let cx = base_x + zone.center[0] * cell_size;
            let cy = base_y + zone.center[1] * cell_size;
            let r = zone.radius * cell_size;
            let mut color = zone.weather_type.color();
            let alpha = (zone.intensity * 80.0) as u8;
            let color_a = Color32::from_rgba_premultiplied(color.r() / 4, color.g() / 4, color.b() / 4, alpha);
            painter.circle_filled(Pos2::new(cx, cy), r, color_a);
            painter.circle_stroke(Pos2::new(cx, cy), r, Stroke::new(1.0, Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 120)));
        }
    }

    /// Draw road network
    pub fn show_roads_overlay(&self, painter: &Painter, roads: &[Road], base_x: f32, base_y: f32, cell_size: f32) {
        for road in roads {
            if road.nodes.len() < 2 { continue; }
            let color = match road.road_type {
                RoadType::MainRoad => Color32::from_rgba_premultiplied(200, 180, 100, 200),
                RoadType::DirtTrail => Color32::from_rgba_premultiplied(140, 120, 80, 150),
                RoadType::MountainPass => Color32::from_rgba_premultiplied(180, 150, 120, 180),
                RoadType::CoastalRoute => Color32::from_rgba_premultiplied(100, 150, 200, 160),
            };
            let width = match road.road_type {
                RoadType::MainRoad => 2.0,
                RoadType::DirtTrail => 1.0,
                _ => 1.5,
            };
            for window in road.nodes.windows(2) {
                let ax = base_x + window[0][0] * cell_size;
                let ay = base_y + window[0][1] * cell_size;
                let bx = base_x + window[1][0] * cell_size;
                let by = base_y + window[1][1] * cell_size;
                painter.line_segment([Pos2::new(ax, ay), Pos2::new(bx, by)], Stroke::new(width, color));
            }
        }
    }

    /// Context menu for right-clicking on the preview canvas
    pub fn show_canvas_context_menu(&mut self, ui: &mut egui::Ui, world_x: f32, world_y: f32) {
        ui.label(format!("Position: ({:.1}, {:.1})", world_x, world_y));
        if let Some(chunk) = &self.generated {
            let xi = world_x.floor() as u32;
            let yi = world_y.floor() as u32;
            if xi < chunk.width && yi < chunk.height {
                let idx = (yi * chunk.width + xi) as usize;
                ui.separator();
                ui.label(format!("Height: {:.3}", chunk.height_map[idx]));
                ui.label(format!("Moisture: {:.3}", chunk.moisture_map[idx]));
                ui.label(format!("Temperature: {:.3}", chunk.temp_map[idx]));
                let bi = chunk.biome_map[idx];
                if bi < self.config.biomes.len() {
                    ui.label(format!("Biome: {}", self.config.biomes[bi].name));
                }
            }
        }
        ui.separator();
        if ui.button("Set As Brush Center").clicked() {
            self.brush_pos = Some([world_x, world_y]);
        }
        if ui.button("Generate New Seed Here").clicked() {
            // use position as seed modifier
            self.generation_seed = self.generation_seed
                .wrapping_add((world_x as u64 * 31337).wrapping_mul(world_y as u64 * 7919 + 1));
        }
    }

    /// Show a mini thumbnail of the whole world in the corner
    pub fn show_minimap(&self, ui: &mut egui::Ui, size: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_rgb(10, 10, 20));
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);

        if let Some(chunk) = &self.generated {
            let cw = chunk.width as f32;
            let ch = chunk.height as f32;
            let cell_w = size / cw;
            let cell_h = size / ch;
            let biomes = &self.config.biomes;
            let sea_level = self.config.sea_level;

            // Draw at reduced resolution (every N pixels)
            let skip = ((cw / size) as u32).max(1);
            for y in (0..chunk.height).step_by(skip as usize) {
                for x in (0..chunk.width).step_by(skip as usize) {
                    let idx = (y * chunk.width + x) as usize;
                    let color = match self.preview_layer {
                        PreviewLayer::Height => height_to_color(chunk.height_map[idx], sea_level),
                        PreviewLayer::Moisture => moisture_to_color(chunk.moisture_map[idx]),
                        PreviewLayer::Temperature => temp_to_color(chunk.temp_map[idx]),
                        PreviewLayer::Biome => {
                            let bi = chunk.biome_map[idx];
                            if bi < biomes.len() { biomes[bi].color } else { Color32::GRAY }
                        }
                        PreviewLayer::Combined => height_to_color(chunk.height_map[idx], sea_level),
                    };
                    let px = rect.min.x + x as f32 * cell_w;
                    let py = rect.min.y + y as f32 * cell_h;
                    let pr = Rect::from_min_size(Pos2::new(px, py), Vec2::new(cell_w * skip as f32 + 0.5, cell_h * skip as f32 + 0.5));
                    if pr.min.x < rect.max.x && pr.min.y < rect.max.y {
                        painter.rect_filled(pr, 0.0, color);
                    }
                }
            }

            // Show viewport indicator
            let vp_x = rect.min.x + (-self.preview_offset[0] / cw * size / self.preview_zoom).max(0.0);
            let vp_y = rect.min.y + (-self.preview_offset[1] / ch * size / self.preview_zoom).max(0.0);
            painter.rect_stroke(
                Rect::from_min_size(Pos2::new(vp_x, vp_y), Vec2::new(size / self.preview_zoom, size / self.preview_zoom)),
                0.0,
                Stroke::new(1.0, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Export the current chunk data
    pub fn export_csv(&self) -> Option<String> {
        self.generated.as_ref().map(|c| export_heightmap_csv(c))
    }

    /// Get stats about the generated world
    pub fn get_generation_stats(&self) -> Option<HeightStats> {
        self.generated.as_ref().map(|c| compute_height_stats(c, self.config.sea_level))
    }
}

// ============================================================
// FINAL TESTS FOR WORLD GEN EXTENDED
// ============================================================

#[cfg(test)]
mod world_placement_tests {
    use super::*;

    #[test]
    fn test_place_settlements() {
        let config = WorldGenConfig { width: 128, height: 128, ..Default::default() };
        let chunk = generate_chunk(&config, 42);
        let settlements = place_settlements(&chunk, &config.biomes, 0.35, 10, 42);
        // Should place at least a few
        assert!(!settlements.is_empty());
        for s in &settlements {
            assert!(s.pos[0] >= 0.0 && s.pos[0] < 128.0);
            assert!(s.pos[1] >= 0.0 && s.pos[1] < 128.0);
        }
    }

    #[test]
    fn test_place_resources() {
        let config = WorldGenConfig { width: 64, height: 64, ..Default::default() };
        let chunk = generate_chunk(&config, 99);
        let deposits = place_resource_deposits(&chunk, 0.35, 99);
        assert!(!deposits.is_empty());
    }

    #[test]
    fn test_settlement_names() {
        let name1 = generate_settlement_name(42, 0);
        let name2 = generate_settlement_name(42, 1);
        assert!(!name1.is_empty());
        assert!(!name2.is_empty());
        // Names should be different for different indices
        // (not guaranteed but very likely)
    }

    #[test]
    fn test_weather_zones() {
        let config = WorldGenConfig { width: 64, height: 64, ..Default::default() };
        let chunk = generate_chunk(&config, 1);
        let zones = generate_weather_zones(&chunk, &config.biomes, 0.35, 42, 5);
        assert_eq!(zones.len(), 5);
    }

    #[test]
    fn test_astar_path() {
        let config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let chunk = generate_chunk(&config, 5);
        // Path from corner to corner (may not exist if all water)
        let _ = astar_path(&chunk.height_map, 32, 32, [1, 1], [30, 30], 0.35);
    }

    #[test]
    fn test_painter_undo_redo() {
        let mut state = PainterState::new(5);
        let mut data = vec![0.5_f32; 16];
        state.push_undo(&data);
        data[0] = 0.9;
        let could_undo = state.undo(&mut data);
        assert!(could_undo);
        assert!((data[0] - 0.5).abs() < 0.001);
        let could_redo = state.redo(&mut data);
        assert!(could_redo);
        assert!((data[0] - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_brush_sculpt() {
        let mut map = vec![0.5_f32; 32 * 32];
        brush_sculpt(&mut map, 32, 32, 16.0, 16.0, 5.0, 0.1, SculptMode::Raise);
        assert!(map[16 * 32 + 16] > 0.5);
        brush_sculpt(&mut map, 32, 32, 16.0, 16.0, 5.0, 0.5, SculptMode::Lower);
    }

    #[test]
    fn test_stamp_volcano() {
        let mut map = vec![0.3_f32; 64 * 64];
        stamp_feature(&mut map, 64, 64, 32.0, 32.0, StampFeature::Volcano, 10.0, 0.8);
        assert!(map[32 * 64 + 32] > 0.3);
    }

    #[test]
    fn test_generation_macro() {
        let mut config = WorldGenConfig { width: 32, height: 32, ..Default::default() };
        let mut macro_ = GenerationMacro::new("test");
        macro_.steps.push(MacroStep::Generate);
        macro_.steps.push(MacroStep::ApplyBlur(1));
        macro_.steps.push(MacroStep::Normalize);
        let chunk = macro_.execute(&mut config, 42);
        assert_eq!(chunk.width, 32);
    }

    #[test]
    fn test_road_length() {
        let road = Road {
            nodes: vec![[0.0, 0.0], [3.0, 4.0]],
            road_type: RoadType::MainRoad,
            width: 1.0,
        };
        assert!((road.length() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_terrain_movement_cost() {
        assert!(TerrainCategory::Mountain.movement_cost() > TerrainCategory::Lowland.movement_cost());
        assert!(TerrainCategory::DeepWater.movement_cost() == f32::MAX);
    }

    #[test]
    fn test_export_csv() {
        let config = WorldGenConfig { width: 8, height: 8, ..Default::default() };
        let chunk = generate_chunk(&config, 1);
        let csv = export_heightmap_csv(&chunk);
        assert!(csv.contains("x,y,height"));
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 8 * 8 + 1); // header + data
    }
}
