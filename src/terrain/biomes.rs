//! Biome placement and ecological simulation.
//!
//! Implements the Whittaker biome classification diagram, biome blending,
//! vegetation density maps, rock/soil type assignment, river carving,
//! lake filling, and coastline detection.

use super::heightmap::HeightMap;

// ── Biome Types ───────────────────────────────────────────────────────────────

/// All biome types derived from the Whittaker biome diagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeKind {
    // Aquatic
    Ocean,
    ShallowWater,
    Lake,
    River,
    // Hot / dry
    Desert,
    HotDesert,
    SemiArid,
    // Tropical
    TropicalRainforest,
    TropicalDryForest,
    Savanna,
    // Subtropical / temperate
    Shrubland,
    TemperateRainforest,
    TemperateSeasonalForest,
    TemperateGrassland,
    // Cold
    BorealForest,    // Taiga
    Tundra,
    Alpine,
    // Special
    Glacier,
    Beach,
    Wetland,
    Mangrove,
    Volcano,
}

impl BiomeKind {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            BiomeKind::Ocean                    => "Ocean",
            BiomeKind::ShallowWater             => "Shallow Water",
            BiomeKind::Lake                     => "Lake",
            BiomeKind::River                    => "River",
            BiomeKind::Desert                   => "Desert",
            BiomeKind::HotDesert                => "Hot Desert",
            BiomeKind::SemiArid                 => "Semi-Arid",
            BiomeKind::TropicalRainforest        => "Tropical Rainforest",
            BiomeKind::TropicalDryForest         => "Tropical Dry Forest",
            BiomeKind::Savanna                  => "Savanna",
            BiomeKind::Shrubland                => "Shrubland",
            BiomeKind::TemperateRainforest       => "Temperate Rainforest",
            BiomeKind::TemperateSeasonalForest   => "Temperate Seasonal Forest",
            BiomeKind::TemperateGrassland        => "Temperate Grassland",
            BiomeKind::BorealForest              => "Boreal Forest (Taiga)",
            BiomeKind::Tundra                   => "Tundra",
            BiomeKind::Alpine                   => "Alpine",
            BiomeKind::Glacier                  => "Glacier",
            BiomeKind::Beach                    => "Beach",
            BiomeKind::Wetland                  => "Wetland",
            BiomeKind::Mangrove                 => "Mangrove",
            BiomeKind::Volcano                  => "Volcano",
        }
    }

    /// Representative RGB color (0–255) for map display.
    pub fn map_color(self) -> [u8; 3] {
        match self {
            BiomeKind::Ocean                    => [0,   60,  120],
            BiomeKind::ShallowWater             => [30,  120, 180],
            BiomeKind::Lake                     => [50,  100, 200],
            BiomeKind::River                    => [50,  130, 220],
            BiomeKind::Desert                   => [230, 200, 130],
            BiomeKind::HotDesert                => [240, 190, 100],
            BiomeKind::SemiArid                 => [210, 185, 130],
            BiomeKind::TropicalRainforest        => [10,  100,  20],
            BiomeKind::TropicalDryForest         => [50,  130,  40],
            BiomeKind::Savanna                  => [160, 180,  60],
            BiomeKind::Shrubland                => [130, 150,  80],
            BiomeKind::TemperateRainforest       => [30,  100,  60],
            BiomeKind::TemperateSeasonalForest   => [60,  130,  50],
            BiomeKind::TemperateGrassland        => [130, 170,  80],
            BiomeKind::BorealForest              => [50,  100,  70],
            BiomeKind::Tundra                   => [160, 170, 140],
            BiomeKind::Alpine                   => [200, 200, 200],
            BiomeKind::Glacier                  => [230, 245, 255],
            BiomeKind::Beach                    => [230, 210, 150],
            BiomeKind::Wetland                  => [60,  130, 100],
            BiomeKind::Mangrove                 => [30,  90,   60],
            BiomeKind::Volcano                  => [90,  30,   20],
        }
    }

    /// Whether the biome is a water body.
    pub fn is_water(self) -> bool {
        matches!(self,
            BiomeKind::Ocean | BiomeKind::ShallowWater |
            BiomeKind::Lake  | BiomeKind::River)
    }

    /// Whether the biome supports forest vegetation.
    pub fn is_forested(self) -> bool {
        matches!(self,
            BiomeKind::TropicalRainforest   |
            BiomeKind::TropicalDryForest    |
            BiomeKind::TemperateRainforest  |
            BiomeKind::TemperateSeasonalForest |
            BiomeKind::BorealForest         |
            BiomeKind::Mangrove)
    }
}

// ── Climate Parameters ────────────────────────────────────────────────────────

/// Per-cell climate data used for biome classification.
#[derive(Clone, Debug)]
pub struct ClimateCell {
    /// Normalized temperature: 0.0 = arctic, 1.0 = equatorial
    pub temperature: f32,
    /// Normalized annual precipitation: 0.0 = hyperarid, 1.0 = wet
    pub moisture:    f32,
    /// Altitude (normalized 0–1)
    pub altitude:    f32,
    /// Distance to nearest ocean cell (0 = coast, 1 = far inland)
    pub continentality: f32,
}

// ── Whittaker Biome Classifier ────────────────────────────────────────────────

/// Classify a biome using the Whittaker biome diagram.
///
/// Inputs are normalized temperature [0,1] and moisture [0,1].
/// Additional altitude/continentality corrections are applied.
pub struct WhittakerClassifier;

impl WhittakerClassifier {
    /// Primary classification from temperature and moisture.
    pub fn classify(cell: &ClimateCell) -> BiomeKind {
        let t = cell.temperature;
        let m = cell.moisture;
        let a = cell.altitude;

        // Altitude overrides first
        if a > 0.92 { return BiomeKind::Glacier; }
        if a > 0.80 { return BiomeKind::Alpine;  }

        // Ocean / lake handled by caller
        // Very cold
        if t < 0.10 {
            return if a > 0.70 { BiomeKind::Alpine } else { BiomeKind::Glacier };
        }
        if t < 0.20 {
            return BiomeKind::Tundra;
        }

        // Cold
        if t < 0.35 {
            return if m > 0.40 {
                BiomeKind::BorealForest
            } else {
                BiomeKind::Tundra
            };
        }

        // Cool temperate
        if t < 0.50 {
            return if m > 0.70 {
                BiomeKind::TemperateRainforest
            } else if m > 0.40 {
                BiomeKind::TemperateSeasonalForest
            } else if m > 0.20 {
                BiomeKind::TemperateGrassland
            } else {
                BiomeKind::Desert
            };
        }

        // Warm temperate
        if t < 0.65 {
            return if m > 0.65 {
                BiomeKind::TemperateRainforest
            } else if m > 0.40 {
                BiomeKind::TemperateSeasonalForest
            } else if m > 0.25 {
                BiomeKind::Shrubland
            } else if m > 0.12 {
                BiomeKind::TemperateGrassland
            } else {
                BiomeKind::SemiArid
            };
        }

        // Hot (subtropical / tropical)
        if t < 0.80 {
            return if m > 0.70 {
                BiomeKind::TropicalDryForest
            } else if m > 0.40 {
                BiomeKind::Savanna
            } else if m > 0.20 {
                BiomeKind::SemiArid
            } else {
                BiomeKind::HotDesert
            };
        }

        // Very hot (tropical)
        if m > 0.75 {
            BiomeKind::TropicalRainforest
        } else if m > 0.50 {
            BiomeKind::TropicalDryForest
        } else if m > 0.30 {
            BiomeKind::Savanna
        } else if m > 0.15 {
            BiomeKind::SemiArid
        } else {
            BiomeKind::HotDesert
        }
    }
}

// ── Biome Map ─────────────────────────────────────────────────────────────────

/// Full biome classification for a terrain.
#[derive(Clone, Debug)]
pub struct BiomeMap {
    pub width:  usize,
    pub height: usize,
    /// Primary biome per cell.
    pub biomes: Vec<BiomeKind>,
    /// Blend weights for up to 4 neighboring biomes per cell.
    pub blend:  Vec<BiomeBlend>,
    /// Per-cell climate.
    pub climate: Vec<ClimateCell>,
}

/// Blending weights at a cell boundary.
#[derive(Clone, Debug, Default)]
pub struct BiomeBlend {
    /// Up to 4 biomes and their weights (sum to 1.0).
    pub entries: [(BiomeKind, f32); 4],
    pub count:   usize,
}

impl BiomeBlend {
    pub fn new_single(biome: BiomeKind) -> Self {
        let mut b = Self::default();
        b.entries[0] = (biome, 1.0);
        b.count = 1;
        b
    }
}

impl Default for BiomeKind {
    fn default() -> Self { BiomeKind::TemperateGrassland }
}

impl BiomeMap {
    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    pub fn get(&self, x: usize, y: usize) -> BiomeKind { self.biomes[y * self.width + x] }

    pub fn set(&mut self, x: usize, y: usize, b: BiomeKind) {
        self.biomes[y * self.width + x] = b;
    }
}

// ── Climate Generator ─────────────────────────────────────────────────────────

/// Generates climate maps from a heightmap.
///
/// Temperature decreases with altitude and increases toward the equator
/// (modeled as the center of the map). Moisture is generated via
/// a simple prevailing-wind model.
pub struct ClimateGenerator {
    pub equatorial_band:  f32, // 0–1, fraction of map height as equator
    pub moisture_falloff: f32, // How quickly moisture drops inland
    pub seed:             u64,
}

impl ClimateGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            equatorial_band:  0.5,
            moisture_falloff: 0.6,
            seed,
        }
    }

    /// Generate climate for each cell of the heightmap.
    pub fn generate(&self, heightmap: &HeightMap, sea_level: f32) -> Vec<ClimateCell> {
        let w = heightmap.width;
        let h = heightmap.height;
        let mut cells = Vec::with_capacity(w * h);

        // Compute ocean distance map (BFS) for continentality
        let ocean_dist = self.ocean_distance_map(heightmap, sea_level);

        // Simple noise for moisture variation
        let noise = MoistureNoise::new(self.seed);

        for y in 0..h {
            for x in 0..w {
                let altitude = heightmap.get(x, y);
                // Latitude: 0 at equator (center), 1 at poles (edges)
                let lat = (y as f32 / h as f32 - self.equatorial_band).abs() * 2.0;
                let lat = lat.clamp(0.0, 1.0);

                // Base temperature: warm at equator, cool at poles
                let base_temp = 1.0 - lat;
                // Altitude lapse: temperature drops with height
                let lapse = altitude * 0.65;
                let temperature = (base_temp - lapse).clamp(0.0, 1.0);

                // Continentality (0 = coast, 1 = interior)
                let max_dist = (w.max(h) / 2) as f32;
                let continentality = (ocean_dist[y * w + x] as f32 / max_dist).clamp(0.0, 1.0);

                // Moisture: high near ocean, decreases inland
                // Also influenced by latitude (ITCZ near equator)
                let itcz = 1.0 - (lat * 2.0 - 0.3).abs().min(1.0);
                let base_moist = (1.0 - continentality * self.moisture_falloff + itcz * 0.3).clamp(0.0, 1.0);
                // Add noise variation
                let nx = x as f32 / w as f32 * 3.7;
                let ny = y as f32 / h as f32 * 3.7;
                let moist_noise = noise.sample(nx, ny) * 0.25;
                let moisture = (base_moist + moist_noise - altitude * 0.15).clamp(0.0, 1.0);

                cells.push(ClimateCell { temperature, moisture, altitude, continentality });
            }
        }

        cells
    }

    /// Flood-fill BFS from ocean cells outward to compute distance.
    fn ocean_distance_map(&self, heightmap: &HeightMap, sea_level: f32) -> Vec<u32> {
        use std::collections::VecDeque;
        let w = heightmap.width;
        let h = heightmap.height;
        let mut dist = vec![u32::MAX; w * h];
        let mut queue = VecDeque::new();

        // Seed ocean cells
        for y in 0..h {
            for x in 0..w {
                if heightmap.get(x, y) < sea_level {
                    dist[y * w + x] = 0;
                    queue.push_back((x, y));
                }
            }
        }

        // BFS
        while let Some((x, y)) = queue.pop_front() {
            let d = dist[y * w + x];
            for (dx, dy) in &[(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i64 + dx;
                let ny = y as i64 + dy;
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 { continue; }
                let idx = ny as usize * w + nx as usize;
                if dist[idx] == u32::MAX {
                    dist[idx] = d + 1;
                    queue.push_back((nx as usize, ny as usize));
                }
            }
        }

        dist
    }
}

/// Minimal value noise for moisture variation.
struct MoistureNoise {
    seed: u64,
}

impl MoistureNoise {
    fn new(seed: u64) -> Self { Self { seed } }

    fn sample(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i64;
        let yi = y.floor() as i64;
        let xf = x - xi as f32;
        let yf = y - yi as f32;
        let ux = xf * xf * (3.0 - 2.0 * xf);
        let uy = yf * yf * (3.0 - 2.0 * yf);
        let r = |ix: i64, iy: i64| -> f32 {
            let mut h = self.seed;
            h ^= ix.unsigned_abs().wrapping_mul(374761393);
            h ^= iy.unsigned_abs().wrapping_mul(668265263);
            h = h.wrapping_mul(0x9e3779b97f4a7c15);
            h ^= h >> 33;
            if ix < 0 { h ^= 0xabcd; }
            if iy < 0 { h ^= 0xef01; }
            (h >> 11) as f32 / (1u64 << 53) as f32
        };
        let v = r(xi, yi) * (1.0-ux)*(1.0-uy)
              + r(xi+1,yi) *    ux  *(1.0-uy)
              + r(xi,yi+1) * (1.0-ux)*  uy
              + r(xi+1,yi+1)*   ux  *   uy;
        v
    }
}

// ── Full Biome Placement System ───────────────────────────────────────────────

/// Generates a full biome map from a heightmap.
pub struct BiomePlacer {
    pub sea_level:          f32,
    pub shallow_water_depth: f32,
    pub beach_width:        f32,
    pub glacier_altitude:   f32,
    pub enable_rivers:      bool,
    pub enable_lakes:       bool,
    pub enable_mangroves:   bool,
    pub river_count:        usize,
    pub seed:               u64,
}

impl BiomePlacer {
    pub fn new(seed: u64) -> Self {
        Self {
            sea_level:           0.35,
            shallow_water_depth: 0.08,
            beach_width:         0.02,
            glacier_altitude:    0.92,
            enable_rivers:       true,
            enable_lakes:        true,
            enable_mangroves:    true,
            river_count:         8,
            seed,
        }
    }

    /// Generate a complete BiomeMap from a heightmap.
    pub fn generate(&self, heightmap: &HeightMap) -> BiomeMap {
        let w = heightmap.width;
        let h = heightmap.height;

        // Generate climate
        let climate_gen = ClimateGenerator::new(self.seed);
        let climate = climate_gen.generate(heightmap, self.sea_level);

        // Initial biome classification
        let mut biomes = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let alt = heightmap.get(x, y);
                let cell = &climate[y * w + x];
                let biome = if alt < self.sea_level - self.shallow_water_depth {
                    BiomeKind::Ocean
                } else if alt < self.sea_level {
                    BiomeKind::ShallowWater
                } else if alt < self.sea_level + self.beach_width {
                    BiomeKind::Beach
                } else {
                    WhittakerClassifier::classify(cell)
                };
                biomes.push(biome);
            }
        }

        let mut map = BiomeMap {
            width: w,
            height: h,
            biomes,
            blend: vec![BiomeBlend::new_single(BiomeKind::TemperateGrassland); w * h],
            climate,
        };

        // Post-processing passes
        if self.enable_rivers {
            self.carve_rivers(&mut map, heightmap);
        }
        if self.enable_lakes {
            self.fill_lakes(&mut map, heightmap);
        }
        if self.enable_mangroves {
            self.place_mangroves(&mut map, heightmap);
        }

        // Compute blending at biome boundaries
        self.compute_biome_blending(&mut map);

        map
    }

    // ── River Carving ─────────────────────────────────────────────────────────

    fn carve_rivers(&self, map: &mut BiomeMap, heightmap: &HeightMap) {
        let w = heightmap.width;
        let h = heightmap.height;
        let mut rng = crate::terrain::heightmap::Rng::new(self.seed ^ 0xbeef);

        for _ in 0..self.river_count {
            // Find a high-altitude starting point that isn't ocean
            let mut tries = 0;
            let (mut x, mut y) = loop {
                let cx = rng.next_i32_range(2, w as i32 - 2) as usize;
                let cy = rng.next_i32_range(2, h as i32 - 2) as usize;
                let alt = heightmap.get(cx, cy);
                if alt > 0.65 && map.get(cx, cy) != BiomeKind::Ocean {
                    break (cx, cy);
                }
                tries += 1;
                if tries > 200 { break (cx, cy); }
            };

            // Flow downhill
            let max_steps = (w + h) * 2;
            for _ in 0..max_steps {
                map.set(x, y, BiomeKind::River);

                // Find the lowest neighbor (steepest descent)
                let mut best_h = heightmap.get(x, y);
                let mut best = None;
                for (dx, dy) in &[(-1i64, 0i64), (1, 0), (0, -1), (0, 1),
                                   (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let nx = x as i64 + dx;
                    let ny = y as i64 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 { continue; }
                    let nh = heightmap.get(nx as usize, ny as usize);
                    if nh < best_h {
                        best_h = nh;
                        best = Some((nx as usize, ny as usize));
                    }
                }

                match best {
                    Some((nx, ny)) => {
                        // Stop if we reach the sea
                        if map.get(nx, ny) == BiomeKind::Ocean
                        || map.get(nx, ny) == BiomeKind::ShallowWater {
                            break;
                        }
                        x = nx;
                        y = ny;
                    }
                    None => break, // Local minimum — fill as lake
                }
            }
        }
    }

    // ── Lake Filling ──────────────────────────────────────────────────────────

    fn fill_lakes(&self, map: &mut BiomeMap, heightmap: &HeightMap) {
        let w = heightmap.width;
        let h = heightmap.height;

        // Find local depressions (cells lower than all 4 orthogonal neighbors)
        // that are not already water or river, then flood-fill with lake.
        let mut visited = vec![false; w * h];

        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                if visited[y * w + x] { continue; }
                let biome = map.get(x, y);
                if biome.is_water() { continue; }

                let center_h = heightmap.get(x, y);
                let is_depression = [(0i64, -1i64), (0, 1), (-1, 0), (1, 0)]
                    .iter()
                    .all(|(dx, dy)| {
                        let nx = (x as i64 + dx) as usize;
                        let ny = (y as i64 + dy) as usize;
                        heightmap.get(nx, ny) > center_h
                    });

                if is_depression && heightmap.get(x, y) < self.sea_level + 0.15 {
                    // Flood fill a small lake around this depression
                    self.flood_fill_lake(map, heightmap, x, y, &mut visited);
                }
            }
        }
    }

    fn flood_fill_lake(
        &self,
        map: &mut BiomeMap,
        heightmap: &HeightMap,
        sx: usize,
        sy: usize,
        visited: &mut Vec<bool>,
    ) {
        use std::collections::VecDeque;
        let w = heightmap.width;
        let h = heightmap.height;
        let seed_h = heightmap.get(sx, sy);
        let lake_level = seed_h + 0.03; // small lake — fill slightly above depression

        let mut queue = VecDeque::new();
        queue.push_back((sx, sy));
        visited[sy * w + sx] = true;

        let mut cells_filled = 0usize;
        let max_lake_cells = 500;

        while let Some((x, y)) = queue.pop_front() {
            if cells_filled >= max_lake_cells { break; }
            if !map.get(x, y).is_water() {
                map.set(x, y, BiomeKind::Lake);
                cells_filled += 1;
            }

            for (dx, dy) in &[(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i64 + dx;
                let ny = y as i64 + dy;
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 { continue; }
                let ni = ny as usize * w + nx as usize;
                if visited[ni] { continue; }
                let nh = heightmap.get(nx as usize, ny as usize);
                if nh <= lake_level && !map.get(nx as usize, ny as usize).is_water() {
                    visited[ni] = true;
                    queue.push_back((nx as usize, ny as usize));
                }
            }
        }
    }

    // ── Mangrove Placement ────────────────────────────────────────────────────

    fn place_mangroves(&self, map: &mut BiomeMap, heightmap: &HeightMap) {
        let w = heightmap.width;
        let h = heightmap.height;

        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let biome = map.get(x, y);
                if biome != BiomeKind::Beach { continue; }
                let climate = &map.climate[y * w + x];
                // Mangroves only in tropical/subtropical zones
                if climate.temperature < 0.65 { continue; }
                // Must be adjacent to shallow water
                let adj_water = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .any(|(dx, dy)| {
                        let nx = (x as i64 + dx) as usize;
                        let ny = (y as i64 + dy) as usize;
                        matches!(map.get(nx, ny), BiomeKind::Ocean | BiomeKind::ShallowWater)
                    });
                if adj_water {
                    map.set(x, y, BiomeKind::Mangrove);
                }
            }
        }
    }

    // ── Biome Boundary Blending ───────────────────────────────────────────────

    fn compute_biome_blending(&self, map: &mut BiomeMap) {
        let w = map.width;
        let h = map.height;
        let biomes_snap = map.biomes.clone();

        for y in 0..h {
            for x in 0..w {
                let center = biomes_snap[y * w + x];
                let mut blend_map: std::collections::HashMap<BiomeKindKey, f32> =
                    std::collections::HashMap::new();

                // Sample a 3x3 neighborhood
                let mut total = 0.0f32;
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let nx = x as i64 + dx;
                        let ny = y as i64 + dy;
                        let biome = if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            center
                        } else {
                            biomes_snap[ny as usize * w + nx as usize]
                        };
                        let weight = if dx == 0 && dy == 0 { 4.0 }
                                     else if dx == 0 || dy == 0 { 2.0 }
                                     else { 1.0 };
                        *blend_map.entry(BiomeKindKey(biome)).or_insert(0.0) += weight;
                        total += weight;
                    }
                }

                // Sort by weight descending, keep top 4
                let mut entries: Vec<(BiomeKind, f32)> = blend_map
                    .into_iter()
                    .map(|(k, w)| (k.0, w / total))
                    .collect();
                entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                entries.truncate(4);

                // Renormalize after truncation
                let sum: f32 = entries.iter().map(|e| e.1).sum();
                let mut blend = BiomeBlend::default();
                blend.count = entries.len();
                for (i, (biome, weight)) in entries.into_iter().enumerate() {
                    blend.entries[i] = (biome, if sum > 0.0 { weight / sum } else { 0.0 });
                }

                map.blend[y * w + x] = blend;
            }
        }
    }
}

/// Newtype for HashMap key since BiomeKind doesn't implement Hash by default
/// (it does via derive, but we need Eq too).
#[derive(PartialEq, Eq, Hash)]
struct BiomeKindKey(BiomeKind);

// ── Vegetation Density Map ────────────────────────────────────────────────────

/// Per-cell vegetation density values.
#[derive(Clone, Debug)]
pub struct VegetationDensityMap {
    pub width:   usize,
    pub height:  usize,
    /// Tree density [0, 1]
    pub trees:   Vec<f32>,
    /// Grass density [0, 1]
    pub grass:   Vec<f32>,
    /// Shrub density [0, 1]
    pub shrubs:  Vec<f32>,
    /// Rock density [0, 1]
    pub rocks:   Vec<f32>,
}

impl VegetationDensityMap {
    pub fn generate(biome_map: &BiomeMap, heightmap: &HeightMap) -> Self {
        let w = biome_map.width;
        let h = biome_map.height;
        let size = w * h;

        let mut trees  = vec![0.0f32; size];
        let mut grass  = vec![0.0f32; size];
        let mut shrubs = vec![0.0f32; size];
        let mut rocks  = vec![0.0f32; size];

        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let slope = heightmap.slope_at(x, y);
                let biome = biome_map.get(x, y);
                let (t, g, s, r) = vegetation_densities(biome, slope);
                trees[i]  = t;
                grass[i]  = g;
                shrubs[i] = s;
                rocks[i]  = r;
            }
        }

        Self { width: w, height: h, trees, grass, shrubs, rocks }
    }

    pub fn tree_density_at(&self, x: usize, y: usize) -> f32 {
        self.trees[y * self.width + x]
    }

    pub fn grass_density_at(&self, x: usize, y: usize) -> f32 {
        self.grass[y * self.width + x]
    }
}

/// Returns (tree, grass, shrub, rock) density for a biome + slope.
fn vegetation_densities(biome: BiomeKind, slope: f32) -> (f32, f32, f32, f32) {
    // Slope penalty: steep terrain → fewer plants, more rocks
    let slope_pen = (slope * 3.0).clamp(0.0, 1.0);
    let rock_bonus = slope_pen * 0.8;

    let (t, g, s, r) = match biome {
        BiomeKind::TropicalRainforest        => (0.90, 0.70, 0.40, 0.05),
        BiomeKind::TropicalDryForest         => (0.65, 0.50, 0.30, 0.10),
        BiomeKind::Savanna                   => (0.15, 0.80, 0.20, 0.10),
        BiomeKind::TemperateRainforest        => (0.85, 0.60, 0.35, 0.08),
        BiomeKind::TemperateSeasonalForest    => (0.70, 0.50, 0.25, 0.10),
        BiomeKind::TemperateGrassland         => (0.05, 0.90, 0.15, 0.08),
        BiomeKind::BorealForest               => (0.75, 0.20, 0.20, 0.12),
        BiomeKind::Shrubland                  => (0.10, 0.50, 0.70, 0.15),
        BiomeKind::Tundra                     => (0.00, 0.30, 0.10, 0.25),
        BiomeKind::Alpine                     => (0.00, 0.10, 0.05, 0.70),
        BiomeKind::Glacier                    => (0.00, 0.00, 0.00, 0.15),
        BiomeKind::Desert | BiomeKind::HotDesert => (0.00, 0.05, 0.08, 0.20),
        BiomeKind::SemiArid                   => (0.03, 0.25, 0.25, 0.18),
        BiomeKind::Wetland                    => (0.20, 0.80, 0.30, 0.02),
        BiomeKind::Mangrove                   => (0.80, 0.30, 0.10, 0.02),
        BiomeKind::Beach                      => (0.00, 0.10, 0.05, 0.20),
        BiomeKind::Ocean | BiomeKind::ShallowWater |
        BiomeKind::Lake  | BiomeKind::River   => (0.00, 0.00, 0.00, 0.00),
        BiomeKind::Volcano                    => (0.00, 0.00, 0.00, 0.80),
    };

    let t = (t * (1.0 - slope_pen)).clamp(0.0, 1.0);
    let g = (g * (1.0 - slope_pen * 0.5)).clamp(0.0, 1.0);
    let s = (s * (1.0 - slope_pen * 0.7)).clamp(0.0, 1.0);
    let r = (r + rock_bonus).clamp(0.0, 1.0);

    (t, g, s, r)
}

// ── Soil / Rock Type ──────────────────────────────────────────────────────────

/// Rock and soil type for a terrain cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoilType {
    Bedrock,
    Granite,
    Limestone,
    Sandstone,
    Basalt,
    Soil,
    LoamySoil,
    ClayRich,
    SandyLoam,
    PeatBog,
    Permafrost,
    VolcanicAsh,
}

impl SoilType {
    /// Drainage rate [0, 1]: 0 = waterlogged, 1 = free-draining.
    pub fn drainage(self) -> f32 {
        match self {
            SoilType::Bedrock      => 0.9,
            SoilType::Granite      => 0.85,
            SoilType::Limestone    => 0.75,
            SoilType::Sandstone    => 0.80,
            SoilType::Basalt       => 0.70,
            SoilType::Soil         => 0.55,
            SoilType::LoamySoil    => 0.50,
            SoilType::ClayRich     => 0.15,
            SoilType::SandyLoam    => 0.80,
            SoilType::PeatBog      => 0.05,
            SoilType::Permafrost   => 0.02,
            SoilType::VolcanicAsh  => 0.65,
        }
    }

    /// Fertility [0, 1]: higher means more vegetation potential.
    pub fn fertility(self) -> f32 {
        match self {
            SoilType::Bedrock      => 0.0,
            SoilType::Granite      => 0.05,
            SoilType::Limestone    => 0.30,
            SoilType::Sandstone    => 0.20,
            SoilType::Basalt       => 0.40,
            SoilType::Soil         => 0.70,
            SoilType::LoamySoil    => 0.90,
            SoilType::ClayRich     => 0.60,
            SoilType::SandyLoam    => 0.50,
            SoilType::PeatBog      => 0.55,
            SoilType::Permafrost   => 0.10,
            SoilType::VolcanicAsh  => 0.80,
        }
    }
}

/// Assign soil types based on altitude and biome.
pub struct SoilAssigner;

impl SoilAssigner {
    pub fn assign(biome_map: &BiomeMap, heightmap: &HeightMap) -> Vec<SoilType> {
        let w = heightmap.width;
        let h = heightmap.height;
        let mut out = Vec::with_capacity(w * h);

        for y in 0..h {
            for x in 0..w {
                let alt   = heightmap.get(x, y);
                let biome = biome_map.get(x, y);
                let slope = heightmap.slope_at(x, y);

                let soil = Self::classify(biome, alt, slope);
                out.push(soil);
            }
        }

        out
    }

    fn classify(biome: BiomeKind, altitude: f32, slope: f32) -> SoilType {
        if altitude > 0.90 { return SoilType::Bedrock; }
        if slope > 0.60    { return SoilType::Granite; }

        match biome {
            BiomeKind::Glacier               => SoilType::Permafrost,
            BiomeKind::Alpine                => SoilType::Bedrock,
            BiomeKind::Tundra                => SoilType::Permafrost,
            BiomeKind::BorealForest          => SoilType::PeatBog,
            BiomeKind::TemperateRainforest |
            BiomeKind::TropicalRainforest    => SoilType::LoamySoil,
            BiomeKind::TemperateSeasonalForest |
            BiomeKind::TropicalDryForest     => SoilType::Soil,
            BiomeKind::TemperateGrassland |
            BiomeKind::Savanna               => SoilType::LoamySoil,
            BiomeKind::Shrubland             => SoilType::SandyLoam,
            BiomeKind::Desert |
            BiomeKind::HotDesert             => SoilType::Sandstone,
            BiomeKind::SemiArid              => SoilType::SandyLoam,
            BiomeKind::Wetland               => SoilType::PeatBog,
            BiomeKind::Mangrove              => SoilType::ClayRich,
            BiomeKind::Beach                 => SoilType::Sandstone,
            BiomeKind::Volcano               => SoilType::VolcanicAsh,
            BiomeKind::Ocean |
            BiomeKind::ShallowWater          => SoilType::Basalt,
            BiomeKind::Lake |
            BiomeKind::River                 => SoilType::ClayRich,
        }
    }
}

// ── Coastline Detection ───────────────────────────────────────────────────────

/// A single point on a coastline contour.
#[derive(Clone, Debug)]
pub struct CoastlinePoint {
    pub x: usize,
    pub y: usize,
    pub facing: CoastFacing,
}

/// Which direction the coast faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoastFacing {
    North, South, East, West,
}

/// Detect coastline cells (land cells adjacent to ocean).
pub fn detect_coastline(map: &BiomeMap) -> Vec<CoastlinePoint> {
    let w = map.width;
    let h = map.height;
    let mut points = Vec::new();

    for y in 0..h {
        for x in 0..w {
            if map.get(x, y).is_water() { continue; }
            // Check 4 orthogonal neighbors
            let neighbors = [
                (x as i64,     y as i64 - 1, CoastFacing::North),
                (x as i64,     y as i64 + 1, CoastFacing::South),
                (x as i64 + 1, y as i64,     CoastFacing::East),
                (x as i64 - 1, y as i64,     CoastFacing::West),
            ];
            for (nx, ny, facing) in &neighbors {
                if *nx < 0 || *ny < 0 || *nx >= w as i64 || *ny >= h as i64 { continue; }
                let nb = map.get(*nx as usize, *ny as usize);
                if nb == BiomeKind::Ocean || nb == BiomeKind::ShallowWater {
                    points.push(CoastlinePoint { x, y, facing: *facing });
                    break; // one entry per cell
                }
            }
        }
    }

    points
}

// ── Biome Query Helpers ───────────────────────────────────────────────────────

/// Summary statistics for a biome map.
#[derive(Clone, Debug, Default)]
pub struct BiomeStats {
    pub ocean_fraction:    f32,
    pub land_fraction:     f32,
    pub forest_fraction:   f32,
    pub desert_fraction:   f32,
    pub unique_biomes:     usize,
}

impl BiomeStats {
    pub fn compute(map: &BiomeMap) -> Self {
        let total = (map.width * map.height) as f32;
        let mut ocean = 0usize;
        let mut forest = 0usize;
        let mut desert = 0usize;
        let mut kinds = std::collections::HashSet::new();

        for &b in &map.biomes {
            kinds.insert(b as u8);
            if b.is_water()                    { ocean  += 1; }
            if b.is_forested()                 { forest += 1; }
            if matches!(b, BiomeKind::Desert | BiomeKind::HotDesert |
                           BiomeKind::SemiArid) { desert += 1; }
        }

        Self {
            ocean_fraction:  ocean  as f32 / total,
            land_fraction:   (total as usize - ocean) as f32 / total,
            forest_fraction: forest as f32 / total,
            desert_fraction: desert as f32 / total,
            unique_biomes:   kinds.len(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::heightmap::DiamondSquare;

    fn make_test_heightmap() -> HeightMap {
        let ds = DiamondSquare::new(42, 0.5);
        ds.generate(33)
    }

    #[test]
    fn test_whittaker_hot_wet() {
        let cell = ClimateCell {
            temperature:    0.90,
            moisture:       0.90,
            altitude:       0.40,
            continentality: 0.10,
        };
        assert_eq!(WhittakerClassifier::classify(&cell), BiomeKind::TropicalRainforest);
    }

    #[test]
    fn test_whittaker_cold_dry() {
        let cell = ClimateCell {
            temperature:    0.15,
            moisture:       0.10,
            altitude:       0.30,
            continentality: 0.80,
        };
        assert_eq!(WhittakerClassifier::classify(&cell), BiomeKind::Tundra);
    }

    #[test]
    fn test_whittaker_high_altitude() {
        let cell = ClimateCell {
            temperature:    0.50,
            moisture:       0.50,
            altitude:       0.95,
            continentality: 0.50,
        };
        assert_eq!(WhittakerClassifier::classify(&cell), BiomeKind::Glacier);
    }

    #[test]
    fn test_biome_placer_coverage() {
        let hm = make_test_heightmap();
        let placer = BiomePlacer::new(1234);
        let bmap = placer.generate(&hm);
        assert_eq!(bmap.biomes.len(), hm.data.len());
    }

    #[test]
    fn test_biome_blend_weights_sum_to_1() {
        let hm = make_test_heightmap();
        let placer = BiomePlacer::new(9999);
        let bmap = placer.generate(&hm);
        for blend in &bmap.blend {
            let sum: f32 = blend.entries[..blend.count].iter().map(|e| e.1).sum();
            assert!((sum - 1.0).abs() < 1e-4 || blend.count == 0,
                "blend weights don't sum to 1: {sum}");
        }
    }

    #[test]
    fn test_vegetation_density_water_is_zero() {
        let (t, g, s, r) = vegetation_densities(BiomeKind::Ocean, 0.0);
        assert_eq!(t, 0.0);
        assert_eq!(g, 0.0);
        assert_eq!(s, 0.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn test_soil_assigner_count() {
        let hm = make_test_heightmap();
        let placer = BiomePlacer::new(0);
        let bmap = placer.generate(&hm);
        let soils = SoilAssigner::assign(&bmap, &hm);
        assert_eq!(soils.len(), hm.data.len());
    }

    #[test]
    fn test_coastline_detection() {
        let hm = make_test_heightmap();
        let placer = BiomePlacer::new(5555);
        let bmap = placer.generate(&hm);
        let coast = detect_coastline(&bmap);
        // Should find at least some coastline if map has both land and ocean
        let has_ocean = bmap.biomes.iter().any(|b| *b == BiomeKind::Ocean);
        let has_land  = bmap.biomes.iter().any(|b| !b.is_water());
        if has_ocean && has_land {
            assert!(!coast.is_empty(), "expected coastline points");
        }
    }

    #[test]
    fn test_biome_stats() {
        let hm = make_test_heightmap();
        let placer = BiomePlacer::new(42);
        let bmap = placer.generate(&hm);
        let stats = BiomeStats::compute(&bmap);
        assert!(stats.land_fraction + stats.ocean_fraction > 0.0);
        assert!(stats.unique_biomes > 0);
    }
}
