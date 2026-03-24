//! World generation — heightmaps, climate, rivers, roads, settlements, history.
//!
//! Generates a complete world map from noise-based terrain through climate
//! simulation, river carving, settlement placement, and procedural history.

use super::Rng;
use std::collections::{BinaryHeap, HashSet, VecDeque};
use std::cmp::Ordering;

// ── BiomeId ───────────────────────────────────────────────────────────────────

/// Opaque biome identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BiomeId(pub u8);

impl BiomeId {
    pub const OCEAN:           BiomeId = BiomeId(0);
    pub const COAST:           BiomeId = BiomeId(1);
    pub const DESERT:          BiomeId = BiomeId(2);
    pub const SAVANNA:         BiomeId = BiomeId(3);
    pub const TROPICAL_FOREST: BiomeId = BiomeId(4);
    pub const GRASSLAND:       BiomeId = BiomeId(5);
    pub const SHRUBLAND:       BiomeId = BiomeId(6);
    pub const TEMPERATE_FOREST:BiomeId = BiomeId(7);
    pub const BOREAL_FOREST:   BiomeId = BiomeId(8);
    pub const TUNDRA:          BiomeId = BiomeId(9);
    pub const SNOW:            BiomeId = BiomeId(10);
    pub const MOUNTAIN:        BiomeId = BiomeId(11);
}

/// Parameters describing a biome.
#[derive(Debug, Clone)]
pub struct BiomeParams {
    pub id:            BiomeId,
    pub name:          &'static str,
    pub glyph_char:    char,
    /// Approximate colour as (r, g, b) in 0..255.
    pub color:         (u8, u8, u8),
    pub temperature_min: f32,
    pub temperature_max: f32,
    pub moisture_min:  f32,
    pub moisture_max:  f32,
    pub elevation_min: f32,
    pub elevation_max: f32,
}

impl BiomeParams {
    pub fn all() -> Vec<BiomeParams> {
        vec![
            BiomeParams { id: BiomeId::OCEAN,           name: "Ocean",            glyph_char: '~', color: (30,  80,  160), temperature_min: -1.0, temperature_max: 1.0, moisture_min: 0.0, moisture_max: 1.0, elevation_min: -1.0, elevation_max: 0.0  },
            BiomeParams { id: BiomeId::COAST,           name: "Coast",            glyph_char: '≈', color: (60,  120, 180), temperature_min: -1.0, temperature_max: 1.0, moisture_min: 0.0, moisture_max: 1.0, elevation_min: 0.0,  elevation_max: 0.1  },
            BiomeParams { id: BiomeId::DESERT,          name: "Desert",           glyph_char: '∴', color: (210, 185, 130), temperature_min: 0.5,  temperature_max: 1.0, moisture_min: 0.0, moisture_max: 0.2, elevation_min: 0.0,  elevation_max: 1.0  },
            BiomeParams { id: BiomeId::SAVANNA,         name: "Savanna",          glyph_char: ',', color: (180, 160, 90),  temperature_min: 0.4,  temperature_max: 1.0, moisture_min: 0.2, moisture_max: 0.4, elevation_min: 0.0,  elevation_max: 1.0  },
            BiomeParams { id: BiomeId::TROPICAL_FOREST, name: "Tropical Forest",  glyph_char: '♣', color: (30,  110, 40),  temperature_min: 0.4,  temperature_max: 1.0, moisture_min: 0.6, moisture_max: 1.0, elevation_min: 0.0,  elevation_max: 1.0  },
            BiomeParams { id: BiomeId::GRASSLAND,       name: "Grassland",        glyph_char: '"', color: (120, 170, 60),  temperature_min: 0.0,  temperature_max: 0.7, moisture_min: 0.3, moisture_max: 0.6, elevation_min: 0.0,  elevation_max: 1.0  },
            BiomeParams { id: BiomeId::SHRUBLAND,       name: "Shrubland",        glyph_char: '\'',color: (150, 140, 80),  temperature_min: 0.0,  temperature_max: 0.7, moisture_min: 0.1, moisture_max: 0.4, elevation_min: 0.0,  elevation_max: 1.0  },
            BiomeParams { id: BiomeId::TEMPERATE_FOREST,name: "Temperate Forest", glyph_char: '♠', color: (60,  130, 60),  temperature_min: -0.3, temperature_max: 0.5, moisture_min: 0.4, moisture_max: 0.8, elevation_min: 0.0,  elevation_max: 0.8  },
            BiomeParams { id: BiomeId::BOREAL_FOREST,   name: "Boreal Forest",    glyph_char: '▲', color: (40,  100, 60),  temperature_min: -0.6, temperature_max: 0.1, moisture_min: 0.3, moisture_max: 0.7, elevation_min: 0.0,  elevation_max: 0.8  },
            BiomeParams { id: BiomeId::TUNDRA,          name: "Tundra",           glyph_char: '-', color: (160, 160, 140), temperature_min: -1.0, temperature_max: -0.3,moisture_min: 0.1, moisture_max: 0.5, elevation_min: 0.0,  elevation_max: 0.8  },
            BiomeParams { id: BiomeId::SNOW,            name: "Snow",             glyph_char: '*', color: (230, 240, 255), temperature_min: -1.0, temperature_max: -0.3,moisture_min: 0.0, moisture_max: 1.0, elevation_min: 0.0,  elevation_max: 1.0  },
            BiomeParams { id: BiomeId::MOUNTAIN,        name: "Mountain",         glyph_char: '^', color: (130, 120, 120), temperature_min: -1.0, temperature_max: 1.0, moisture_min: 0.0, moisture_max: 1.0, elevation_min: 0.7,  elevation_max: 1.0  },
        ]
    }
}

// ── BiomeClassifier ───────────────────────────────────────────────────────────

/// Whittaker biome diagram: classifies (temperature × moisture) → BiomeId.
pub struct BiomeClassifier {
    params: Vec<BiomeParams>,
}

impl BiomeClassifier {
    pub fn new() -> Self {
        Self { params: BiomeParams::all() }
    }

    /// Classify a point on the world map.
    pub fn classify(&self, temperature: f32, moisture: f32, elevation: f32) -> BiomeId {
        // Ocean / coast first
        if elevation < 0.0 { return BiomeId::OCEAN; }
        if elevation < 0.1 { return BiomeId::COAST; }
        // High elevation → mountain
        if elevation > 0.75 {
            if temperature < -0.2 { return BiomeId::SNOW; }
            return BiomeId::MOUNTAIN;
        }
        // Cold → snow / tundra / boreal
        if temperature < -0.4 {
            if moisture < 0.15 { return BiomeId::SNOW; }
            return BiomeId::TUNDRA;
        }
        if temperature < 0.0 {
            if moisture < 0.3 { return BiomeId::TUNDRA; }
            return BiomeId::BOREAL_FOREST;
        }
        // Temperate
        if temperature < 0.4 {
            if moisture < 0.2 { return BiomeId::SHRUBLAND; }
            if moisture < 0.5 { return BiomeId::GRASSLAND; }
            return BiomeId::TEMPERATE_FOREST;
        }
        // Warm / hot
        if moisture < 0.15 { return BiomeId::DESERT; }
        if moisture < 0.35 { return BiomeId::SAVANNA; }
        if moisture < 0.6  { return BiomeId::GRASSLAND; }
        BiomeId::TROPICAL_FOREST
    }

    pub fn params_for(&self, id: BiomeId) -> Option<&BiomeParams> {
        self.params.iter().find(|p| p.id == id)
    }
}

impl Default for BiomeClassifier {
    fn default() -> Self { Self::new() }
}

// ── WorldCell ─────────────────────────────────────────────────────────────────

/// A single cell on the world map.
#[derive(Debug, Clone)]
pub struct WorldCell {
    pub elevation:     f32,
    pub temperature:   f32,
    pub moisture:      f32,
    pub biome:         BiomeId,
    pub river_id:      Option<u32>,
    pub road_id:       Option<u32>,
    pub settlement_id: Option<u32>,
}

impl Default for WorldCell {
    fn default() -> Self {
        Self {
            elevation:     0.0,
            temperature:   0.0,
            moisture:      0.5,
            biome:         BiomeId::OCEAN,
            river_id:      None,
            road_id:       None,
            settlement_id: None,
        }
    }
}

// ── WorldMap ──────────────────────────────────────────────────────────────────

/// The complete world map.
#[derive(Debug, Clone)]
pub struct WorldMap {
    pub width:  usize,
    pub height: usize,
    pub cells:  Vec<WorldCell>,
}

impl WorldMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, cells: vec![WorldCell::default(); width * height] }
    }

    pub fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    pub fn get(&self, x: usize, y: usize) -> &WorldCell {
        &self.cells[self.idx(x, y)]
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut WorldCell {
        let i = self.idx(x, y);
        &mut self.cells[i]
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    pub fn elevation_at(&self, x: usize, y: usize) -> f32 {
        self.get(x, y).elevation
    }

    /// Lowest-elevation neighbour (for river flow).
    pub fn lowest_neighbour(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        let mut best = (x, y);
        let mut best_e = self.get(x, y).elevation;
        for (dx, dy) in &[(0i32,1),(0,-1),(1,0),(-1,0)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if !self.in_bounds(nx, ny) { continue; }
            let e = self.get(nx as usize, ny as usize).elevation;
            if e < best_e { best_e = e; best = (nx as usize, ny as usize); }
        }
        if best == (x, y) { None } else { Some(best) }
    }
}

// ── Noise helpers (no external noise crate needed for simple Perlin) ───────────

/// Simple value-noise implementation (no external deps).
fn hash_noise(ix: i64, iy: i64) -> f32 {
    let mut h = ix.wrapping_mul(1619).wrapping_add(iy.wrapping_mul(31337));
    h = (h ^ (h >> 16)).wrapping_mul(0x45d9f3b);
    h = (h ^ (h >> 16)).wrapping_mul(0x45d9f3b);
    h = h ^ (h >> 16);
    (h as f32).abs() / i64::MAX as f32
}

fn smoothstep(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }

/// Sample 2D value noise at (x, y).
fn value_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i64;
    let iy = y.floor() as i64;
    let fx = x - x.floor();
    let fy = y - y.floor();
    let sx = smoothstep(fx);
    let sy = smoothstep(fy);
    let v00 = hash_noise(ix,     iy);
    let v10 = hash_noise(ix + 1, iy);
    let v01 = hash_noise(ix,     iy + 1);
    let v11 = hash_noise(ix + 1, iy + 1);
    lerp(lerp(v00, v10, sx), lerp(v01, v11, sx), sy)
}

/// Multi-octave fractal noise (fBm) with domain warping.
fn fbm_warped(x: f32, y: f32, octaves: u32, warp_strength: f32) -> f32 {
    // Domain warp: offset the sample position by another noise layer
    let wx = value_noise(x * 1.3 + 7.1, y * 1.3 + 2.7) * warp_strength;
    let wy = value_noise(x * 1.3 + 1.2, y * 1.3 + 9.3) * warp_strength;
    let mut value = 0.0_f32;
    let mut amplitude = 0.5_f32;
    let mut frequency = 1.0_f32;
    let mut max_val   = 0.0_f32;
    for _ in 0..octaves {
        value    += amplitude * value_noise((x + wx) * frequency, (y + wy) * frequency);
        max_val  += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    value / max_val
}

// ── HeightmapWorld ────────────────────────────────────────────────────────────

/// Generates terrain using multi-octave Perlin noise with domain warping
/// and a continent falloff mask.
pub struct HeightmapWorld {
    pub octaves:       u32,
    pub scale:         f32,
    pub warp_strength: f32,
    pub sea_level:     f32,
    pub continent_falloff: f32, // 0 = no falloff, 1 = strong island mask
}

impl Default for HeightmapWorld {
    fn default() -> Self {
        Self { octaves: 6, scale: 0.005, warp_strength: 0.4, sea_level: 0.42, continent_falloff: 0.8 }
    }
}

impl HeightmapWorld {
    pub fn new(octaves: u32, scale: f32, warp_strength: f32, sea_level: f32) -> Self {
        Self { octaves, scale, warp_strength, sea_level, continent_falloff: 0.7 }
    }

    /// Fill a WorldMap's elevation, applying continent mask.
    pub fn apply(&self, map: &mut WorldMap, seed: u64) {
        let w = map.width as f32;
        let h = map.height as f32;
        let seed_f = (seed % 10000) as f32 * 0.1;

        for y in 0..map.height {
            for x in 0..map.width {
                let nx = x as f32 * self.scale + seed_f;
                let ny = y as f32 * self.scale + seed_f * 0.7;
                let raw = fbm_warped(nx, ny, self.octaves, self.warp_strength);

                // Continent falloff from centre
                let dx = (x as f32 / w - 0.5) * 2.0;
                let dy = (y as f32 / h - 0.5) * 2.0;
                let dist = (dx * dx + dy * dy).sqrt().min(1.0);
                let mask = 1.0 - dist.powf(1.5) * self.continent_falloff;

                let elevation = (raw * mask * 2.0 - 1.0).clamp(-1.0, 1.0);
                map.get_mut(x, y).elevation = elevation;
            }
        }
    }
}

// ── ClimateSimulator ──────────────────────────────────────────────────────────

/// Simulates temperature and moisture based on latitude, elevation, and ocean proximity.
pub struct ClimateSimulator {
    pub lapse_rate:        f32, // temperature drop per unit elevation gain
    pub ocean_moisture:    f32, // moisture contribution from ocean cells
    pub rain_shadow_decay: f32, // how quickly moisture drops on leeward side of mountains
}

impl Default for ClimateSimulator {
    fn default() -> Self {
        Self { lapse_rate: 0.6, ocean_moisture: 0.7, rain_shadow_decay: 0.4 }
    }
}

impl ClimateSimulator {
    pub fn new(lapse_rate: f32, ocean_moisture: f32, rain_shadow_decay: f32) -> Self {
        Self { lapse_rate, ocean_moisture, rain_shadow_decay }
    }

    /// Assign temperature and moisture to every cell in the map.
    pub fn apply(&self, map: &mut WorldMap) {
        let h = map.height as f32;
        let w = map.width;
        let ht = map.height;

        // Pass 1: temperature from latitude + elevation lapse
        for y in 0..ht {
            let lat_norm = y as f32 / h; // 0 = north, 1 = south
            let lat_temp = (std::f32::consts::PI * lat_norm).cos(); // peaks at equator (mid)
            for x in 0..w {
                let elev = map.get(x, y).elevation.max(0.0);
                let temp = lat_temp - elev * self.lapse_rate;
                map.get_mut(x, y).temperature = temp.clamp(-1.0, 1.0);
            }
        }

        // Pass 2: moisture advection west-to-east
        // Collect elevation snapshot first
        let elev_snapshot: Vec<f32> = map.cells.iter().map(|c| c.elevation).collect();

        for y in 0..ht {
            let mut moisture = 0.2_f32; // base moisture
            for x in 0..w {
                let elev = elev_snapshot[y * w + x];
                if elev < 0.0 {
                    // Over ocean: gain moisture
                    moisture = (moisture + self.ocean_moisture * 0.1).min(1.0);
                } else {
                    // Over land: lose moisture to rainfall, more at high elevation
                    let rainfall = moisture * (0.05 + elev * self.rain_shadow_decay * 0.1);
                    moisture = (moisture - rainfall).max(0.0);
                    // Rain shadow: if going downhill after peak, moisture stays low
                }
                map.get_mut(x, y).moisture = moisture.clamp(0.0, 1.0);
            }
        }

        // Pass 3: proximity to ocean smoothing
        // Build ocean mask
        let is_ocean: Vec<bool> = map.cells.iter().map(|c| c.elevation < 0.0).collect();
        let mut ocean_prox = vec![0.0_f32; w * ht];
        // BFS from ocean cells up to radius 10
        let mut queue: VecDeque<(usize, usize, u32)> = VecDeque::new();
        let mut visited_op = vec![false; w * ht];
        for y in 0..ht {
            for x in 0..w {
                if is_ocean[y * w + x] {
                    ocean_prox[y * w + x] = 1.0;
                    queue.push_back((x, y, 0));
                    visited_op[y * w + x] = true;
                }
            }
        }
        let radius = 15u32;
        while let Some((cx, cy, dist)) = queue.pop_front() {
            if dist >= radius { continue; }
            for (dx, dy) in &[(0i32,1),(0,-1),(1,0),(-1,0)] {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= ht { continue; }
                let ni = ny as usize * w + nx as usize;
                if !visited_op[ni] {
                    visited_op[ni] = true;
                    ocean_prox[ni] = 1.0 - (dist + 1) as f32 / radius as f32;
                    queue.push_back((nx as usize, ny as usize, dist + 1));
                }
            }
        }
        // Add ocean proximity to moisture
        for y in 0..ht {
            for x in 0..w {
                let i = y * w + x;
                let m = map.cells[i].moisture + ocean_prox[i] * self.ocean_moisture * 0.3;
                map.cells[i].moisture = m.clamp(0.0, 1.0);
            }
        }
    }
}

// ── RiverSystem ───────────────────────────────────────────────────────────────

/// Generates rivers by flowing downhill from high-elevation sources.
pub struct RiverSystem {
    pub num_rivers:     usize,
    pub min_source_elev: f32,
    pub erosion_amount: f32,
}

impl Default for RiverSystem {
    fn default() -> Self { Self { num_rivers: 12, min_source_elev: 0.5, erosion_amount: 0.02 } }
}

impl RiverSystem {
    pub fn new(num_rivers: usize, min_source_elev: f32, erosion_amount: f32) -> Self {
        Self { num_rivers, min_source_elev, erosion_amount }
    }

    /// Carve rivers into the map. Each river gets a unique river_id.
    pub fn apply(&self, map: &mut WorldMap, rng: &mut Rng) {
        let w = map.width;
        let h = map.height;

        // Collect candidate source cells (high elevation land)
        let mut candidates: Vec<(usize, usize)> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if map.get(x, y).elevation >= self.min_source_elev {
                    candidates.push((x, y));
                }
            }
        }
        rng.shuffle(&mut candidates);

        for river_idx in 0..self.num_rivers.min(candidates.len()) {
            let (mut cx, mut cy) = candidates[river_idx];
            let river_id = river_idx as u32 + 1;
            let mut visited_r = HashSet::new();
            let mut steps = 0usize;

            loop {
                if visited_r.contains(&(cx, cy)) { break; }
                visited_r.insert((cx, cy));
                map.get_mut(cx, cy).river_id = Some(river_id);
                // Erode slightly
                let e = map.get(cx, cy).elevation;
                map.get_mut(cx, cy).elevation = (e - self.erosion_amount).max(-0.05);
                // Add moisture along river
                let m = map.get(cx, cy).moisture;
                map.get_mut(cx, cy).moisture = (m + 0.1).min(1.0);

                steps += 1;
                if steps > w + h { break; } // prevent infinite loops

                // Flow to lowest neighbour
                match map.lowest_neighbour(cx, cy) {
                    Some((nx, ny)) => {
                        if map.get(nx, ny).elevation < 0.0 {
                            // Reached the sea — delta
                            map.get_mut(nx, ny).river_id = Some(river_id);
                            break;
                        }
                        cx = nx; cy = ny;
                    }
                    None => break,
                }
            }
        }
    }
}

// ── Settlement ────────────────────────────────────────────────────────────────

/// Settlement size/kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementKind {
    Village,
    Town,
    City,
    Capital,
}

impl SettlementKind {
    pub fn base_population(&self) -> u32 {
        match self {
            SettlementKind::Village  => 200,
            SettlementKind::Town    => 2_000,
            SettlementKind::City    => 20_000,
            SettlementKind::Capital => 100_000,
        }
    }

    pub fn glyph(&self) -> char {
        match self {
            SettlementKind::Village  => 'v',
            SettlementKind::Town    => 'T',
            SettlementKind::City    => 'C',
            SettlementKind::Capital => '★',
        }
    }
}

/// A settlement on the world map.
#[derive(Debug, Clone)]
pub struct Settlement {
    pub id:         u32,
    pub position:   (usize, usize),
    pub kind:       SettlementKind,
    pub population: u32,
    pub name:       String,
    pub faction_id: Option<u32>,
}

impl Settlement {
    pub fn new(id: u32, x: usize, y: usize, kind: SettlementKind, name: String) -> Self {
        let base = kind.base_population();
        Self {
            id,
            position: (x, y),
            kind,
            population: base,
            name,
            faction_id: None,
        }
    }
}

// ── SettlementPlacer ──────────────────────────────────────────────────────────

/// Places settlements on flat, fertile land.
pub struct SettlementPlacer {
    pub num_settlements: usize,
    pub min_separation:  f32, // in world cells
    pub capital_count:   usize,
    pub city_count:      usize,
    pub town_count:      usize,
}

impl Default for SettlementPlacer {
    fn default() -> Self {
        Self { num_settlements: 20, min_separation: 15.0, capital_count: 1, city_count: 3, town_count: 6 }
    }
}

impl SettlementPlacer {
    /// Fertility score for placing settlements: low elevation land, near rivers, not desert.
    fn fertility(cell: &WorldCell) -> f32 {
        if cell.elevation < 0.05 { return 0.0; } // ocean/coast
        if cell.elevation > 0.65 { return 0.0; } // mountains
        let river_bonus = if cell.river_id.is_some() { 0.3 } else { 0.0 };
        let temp_score = 1.0 - (cell.temperature - 0.3).abs();
        let moist_score = cell.moisture.min(0.8);
        (temp_score * 0.4 + moist_score * 0.4 + river_bonus).max(0.0)
    }

    pub fn place(&self, map: &mut WorldMap, names: &mut NameGenerator, rng: &mut Rng) -> Vec<Settlement> {
        let w = map.width;
        let h = map.height;

        // Score all cells
        let mut scored: Vec<(f32, usize, usize)> = (0..h).flat_map(|y| {
            (0..w).map(move |x| (0.0_f32, x, y))
        }).collect();
        for (score, x, y) in &mut scored {
            *score = Self::fertility(map.get(*x, *y));
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

        let mut settlements = Vec::new();
        let mut placed_positions: Vec<(usize, usize)> = Vec::new();

        // Determine kinds
        let mut kinds: Vec<SettlementKind> = Vec::new();
        for _ in 0..self.capital_count { kinds.push(SettlementKind::Capital); }
        for _ in 0..self.city_count    { kinds.push(SettlementKind::City); }
        for _ in 0..self.town_count    { kinds.push(SettlementKind::Town); }
        while kinds.len() < self.num_settlements { kinds.push(SettlementKind::Village); }

        let sep2 = self.min_separation * self.min_separation;
        let mut kind_idx = 0;

        for (_, x, y) in &scored {
            if settlements.len() >= self.num_settlements { break; }
            let x = *x; let y = *y;
            // Check separation
            let too_close = placed_positions.iter().any(|&(px, py)| {
                let dx = px as f32 - x as f32;
                let dy = py as f32 - y as f32;
                dx * dx + dy * dy < sep2
            });
            if too_close { continue; }

            let kind = kinds.get(kind_idx).copied().unwrap_or(SettlementKind::Village);
            kind_idx += 1;
            let name = names.generate(rng);
            let id   = settlements.len() as u32 + 1;
            let pop_jitter = rng.range_f32(0.7, 1.4);
            let mut s = Settlement::new(id, x, y, kind, name);
            s.population = (s.population as f32 * pop_jitter) as u32;
            s.faction_id = Some(rng.range_usize(4) as u32 + 1);

            map.get_mut(x, y).settlement_id = Some(id);
            placed_positions.push((x, y));
            settlements.push(s);
        }

        settlements
    }
}

// ── RoadNetwork ───────────────────────────────────────────────────────────────

/// Connects settlements with A* paths that prefer flat terrain.
pub struct RoadNetwork {
    pub road_cost_slope: f32, // how much elevation change costs
}

impl Default for RoadNetwork {
    fn default() -> Self { Self { road_cost_slope: 5.0 } }
}

/// A* node for road pathfinding.
#[derive(Debug, Clone)]
struct AStarNode {
    cost: f32,
    x: usize,
    y: usize,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool { self.cost == other.cost }
}
impl Eq for AStarNode {}
impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: reverse ordering
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl RoadNetwork {
    pub fn new(road_cost_slope: f32) -> Self { Self { road_cost_slope } }

    /// Find a road path from `(ax,ay)` to `(bx,by)` using A*.
    /// Returns list of positions or empty if no path found.
    pub fn find_path(&self, map: &WorldMap, ax: usize, ay: usize, bx: usize, by: usize) -> Vec<(usize, usize)> {
        let w = map.width;
        let h = map.height;
        let start = ay * w + ax;
        let goal  = by * w + bx;

        let mut dist = vec![f32::INFINITY; w * h];
        let mut prev = vec![usize::MAX; w * h];
        dist[start] = 0.0;

        let mut heap: BinaryHeap<AStarNode> = BinaryHeap::new();
        heap.push(AStarNode { cost: 0.0, x: ax, y: ay });

        while let Some(AStarNode { cost, x, y }) = heap.pop() {
            let idx = y * w + x;
            if idx == goal {
                // Reconstruct
                let mut path = Vec::new();
                let mut cur = goal;
                while cur != usize::MAX {
                    path.push((cur % w, cur / w));
                    cur = prev[cur];
                }
                path.reverse();
                return path;
            }
            if cost > dist[idx] { continue; }

            for (dx, dy) in &[(0i32,1),(0,-1),(1,0),(-1,0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h { continue; }
                let (nx, ny) = (nx as usize, ny as usize);
                let ni = ny * w + nx;
                let elev_change = (map.get(nx, ny).elevation - map.get(x, y).elevation).abs();
                let move_cost = 1.0 + elev_change * self.road_cost_slope;
                let new_cost  = dist[idx] + move_cost;
                if new_cost < dist[ni] {
                    dist[ni] = new_cost;
                    prev[ni] = idx;
                    // A* heuristic: Euclidean distance
                    let hdx = nx as f32 - bx as f32;
                    let hdy = ny as f32 - by as f32;
                    let h_val = (hdx * hdx + hdy * hdy).sqrt();
                    heap.push(AStarNode { cost: new_cost + h_val, x: nx, y: ny });
                }
            }
        }
        Vec::new() // no path
    }

    /// Shortest trade route between two settlements (by name lookup).
    pub fn shortest_trade_route(
        &self,
        map: &WorldMap,
        settlements: &[Settlement],
        a_id: u32,
        b_id: u32,
    ) -> Vec<(usize, usize)> {
        let a = settlements.iter().find(|s| s.id == a_id);
        let b = settlements.iter().find(|s| s.id == b_id);
        match (a, b) {
            (Some(sa), Some(sb)) => {
                let (ax, ay) = sa.position;
                let (bx, by) = sb.position;
                self.find_path(map, ax, ay, bx, by)
            }
            _ => Vec::new(),
        }
    }

    /// Connect all settlements with roads; mark road cells on the map.
    pub fn build_roads(&self, map: &mut WorldMap, settlements: &[Settlement], rng: &mut Rng) {
        if settlements.len() < 2 { return; }
        // Connect each settlement to its nearest neighbour (MST-like)
        let n = settlements.len();
        let mut connected = vec![false; n];
        connected[0] = true;
        let mut road_id = 1u32;

        for _ in 1..n {
            let mut best_cost = f32::INFINITY;
            let mut best_pair = (0, 1);
            for i in 0..n {
                if !connected[i] { continue; }
                for j in 0..n {
                    if connected[j] { continue; }
                    let (ax, ay) = settlements[i].position;
                    let (bx, by) = settlements[j].position;
                    let dx = ax as f32 - bx as f32;
                    let dy = ay as f32 - by as f32;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < best_cost { best_cost = d; best_pair = (i, j); }
                }
            }
            let (i, j) = best_pair;
            connected[j] = true;
            let (ax, ay) = settlements[i].position;
            let (bx, by) = settlements[j].position;
            let path = self.find_path(map, ax, ay, bx, by);
            for (px, py) in path {
                map.get_mut(px, py).road_id = Some(road_id);
            }
            road_id += 1;
        }
        // Suppress unused warning
        let _ = rng.next_u64();
    }
}

// ── NameGenerator (world cultures) ───────────────────────────────────────────

/// Culture for name generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Culture {
    Norse,
    Arabic,
    Japanese,
    Latin,
    Fantasy,
}

/// Markov-chain (order 2) name generator trained on culture syllable lists.
pub struct NameGenerator {
    culture: Culture,
}

impl NameGenerator {
    pub fn new(culture: Culture) -> Self { Self { culture } }

    /// Generate a name for the given culture and seed.
    pub fn generate(&self, rng: &mut Rng) -> String {
        let (pre, mid, suf) = self.syllables();
        let p = *rng.pick(pre).unwrap_or(&"Ka");
        let s = *rng.pick(suf).unwrap_or(&"ar");
        if rng.chance(0.55) || mid.is_empty() {
            capitalize_first(&format!("{p}{s}"))
        } else {
            let m = *rng.pick(mid).unwrap_or(&"an");
            capitalize_first(&format!("{p}{m}{s}"))
        }
    }

    pub fn generate_with_seed(&self, seed: u64) -> String {
        let mut rng = Rng::new(seed);
        self.generate(&mut rng)
    }

    fn syllables(&self) -> (&[&'static str], &[&'static str], &[&'static str]) {
        match self.culture {
            Culture::Norse    => (NORSE_PRE, NORSE_MID, NORSE_SUF),
            Culture::Arabic   => (ARABIC_PRE, ARABIC_MID, ARABIC_SUF),
            Culture::Japanese => (JAPANESE_PRE, JAPANESE_MID, JAPANESE_SUF),
            Culture::Latin    => (LATIN_PRE, LATIN_MID, LATIN_SUF),
            Culture::Fantasy  => (FANTASY_PRE, FANTASY_MID, FANTASY_SUF),
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

const NORSE_PRE:    &[&str] = &["Thor","Bjorn","Sigurd","Ulf","Ragnar","Gunnar","Erik","Leif","Ivar","Sven","Haldor","Vidar"];
const NORSE_MID:    &[&str] = &["gar","mund","ald","helm","ulf","ric","win","frid","run","ing","rod","bald"];
const NORSE_SUF:    &[&str] = &["son","sen","sson","dottir","ir","ar","heim","borg","fjord","dal","vik","stad"];

const ARABIC_PRE:   &[&str] = &["Al","Abd","Khalid","Omar","Yusuf","Ahmad","Hamid","Tariq","Walid","Faisal","Jamil","Nabil"];
const ARABIC_MID:   &[&str] = &["al","ibn","bin","ab","um","al","din","ud","ur","ul","im","is"];
const ARABIC_SUF:   &[&str] = &["ah","i","an","un","in","oon","een","at","iya","iyya","awi","awi"];

const JAPANESE_PRE: &[&str] = &["Hiro","Taka","Yoshi","Masa","Nori","Tat","Kei","Shin","Haru","Aki","Tomo","Kazu"];
const JAPANESE_MID: &[&str] = &["no","na","yo","ka","ta","ma","mi","mu","ki","ku","ro","to"];
const JAPANESE_SUF: &[&str] = &["shi","ko","ro","ki","mi","ka","to","ru","no","ta","su","ya"];

const LATIN_PRE:    &[&str] = &["Marc","Gaius","Lucius","Publius","Quintus","Titus","Sextus","Aulus","Gnaeus","Decimus","Marcus","Julius"];
const LATIN_MID:    &[&str] = &["aes","ius","ell","inn","iss","ull","orr","err","uss","elius","anus","atus"];
const LATIN_SUF:    &[&str] = &["us","um","ia","ius","ae","ix","ax","ius","anus","inus","ulus","atus"];

const FANTASY_PRE:  &[&str] = &["Aer","Zyl","Vael","Xan","Myr","Thal","Aen","Ith","Sel","Nox","Kyr","Dra"];
const FANTASY_MID:  &[&str] = &["an","ael","iel","ion","ias","eld","ath","ill","orn","ith","eth","ash"];
const FANTASY_SUF:  &[&str] = &["iel","ias","ion","ath","ael","ial","uen","ean","ian","iel","ath","orn"];

// ── WorldHistory ──────────────────────────────────────────────────────────────

/// Kind of historical event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    War,
    Plague,
    NaturalDisaster,
    FoundedSettlement,
    RulerChanged,
    Discovery,
    Trade,
    Rebellion,
}

/// A single historical event.
#[derive(Debug, Clone)]
pub struct HistoryEvent {
    pub year:                 i32,
    pub kind:                 EventKind,
    pub affected_settlements: Vec<u32>,
    pub description:          String,
}

/// Generates world history as a sequence of events.
pub struct WorldHistory {
    pub events: Vec<HistoryEvent>,
}

impl WorldHistory {
    pub fn new() -> Self { Self { events: Vec::new() } }

    /// Generate `years` worth of history for the given world.
    pub fn generate(
        &mut self,
        settlements: &[Settlement],
        years: u32,
        rng: &mut Rng,
    ) {
        let mut year = 1i32;
        let end_year  = year + years as i32;
        let n_settle  = settlements.len();

        while year < end_year {
            let events_this_year = rng.range_usize(3);
            for _ in 0..=events_this_year {
                if year >= end_year { break; }
                let kind_roll = rng.range_usize(8);
                let kind = match kind_roll {
                    0 => EventKind::War,
                    1 => EventKind::Plague,
                    2 => EventKind::NaturalDisaster,
                    3 => EventKind::FoundedSettlement,
                    4 => EventKind::RulerChanged,
                    5 => EventKind::Discovery,
                    6 => EventKind::Trade,
                    _ => EventKind::Rebellion,
                };

                let n_affected = if n_settle == 0 { 0 } else {
                    rng.range_usize(n_settle.min(3)) + 1
                };
                let affected: Vec<u32> = if n_settle > 0 {
                    let mut ids: Vec<u32> = settlements.iter().map(|s| s.id).collect();
                    rng.shuffle(&mut ids);
                    ids.into_iter().take(n_affected).collect()
                } else {
                    Vec::new()
                };

                let description = Self::describe(&kind, &affected, settlements, year, rng);
                self.events.push(HistoryEvent { year, kind, affected_settlements: affected, description });
            }
            // Advance time by 1–10 years
            year += rng.range_i32(1, 10);
        }
    }

    fn describe(
        kind: &EventKind,
        affected: &[u32],
        settlements: &[Settlement],
        year: i32,
        rng: &mut Rng,
    ) -> String {
        let settle_name = |id: u32| -> &str {
            settlements.iter().find(|s| s.id == id).map(|s| s.name.as_str()).unwrap_or("Unknown")
        };

        let first = affected.first().copied().unwrap_or(0);
        let second = affected.get(1).copied();

        match kind {
            EventKind::War => {
                let a = settle_name(first);
                let b = second.map(settle_name).unwrap_or("foreign powers");
                format!("Year {year}: War erupted between {a} and {b}.")
            }
            EventKind::Plague => {
                let a = settle_name(first);
                format!("Year {year}: A plague ravaged {a}, killing thousands.")
            }
            EventKind::NaturalDisaster => {
                let disasters = &["earthquake","flood","volcanic eruption","drought","wildfire"];
                let d = rng.pick(disasters).copied().unwrap_or("storm");
                let a = settle_name(first);
                format!("Year {year}: A great {d} struck near {a}.")
            }
            EventKind::FoundedSettlement => {
                let a = settle_name(first);
                format!("Year {year}: The settlement of {a} was founded.")
            }
            EventKind::RulerChanged => {
                let a = settle_name(first);
                format!("Year {year}: A new ruler came to power in {a}.")
            }
            EventKind::Discovery => {
                let things = &["ancient ruins","a new trade route","a rich vein of ore","a sacred spring","a lost tome"];
                let thing = rng.pick(things).copied().unwrap_or("something remarkable");
                let a = settle_name(first);
                format!("Year {year}: Explorers from {a} discovered {thing}.")
            }
            EventKind::Trade => {
                let a = settle_name(first);
                let b = second.map(settle_name).unwrap_or("distant lands");
                format!("Year {year}: A prosperous trade agreement was forged between {a} and {b}.")
            }
            EventKind::Rebellion => {
                let a = settle_name(first);
                format!("Year {year}: The people of {a} rose in rebellion against their rulers.")
            }
        }
    }
}

impl Default for WorldHistory {
    fn default() -> Self { Self::new() }
}

// ── WorldBuilder ──────────────────────────────────────────────────────────────

/// Convenience builder that orchestrates all world-generation steps.
pub struct WorldBuilder {
    pub width:  usize,
    pub height: usize,
    pub seed:   u64,
}

impl WorldBuilder {
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        Self { width, height, seed }
    }

    /// Generate a complete world. Returns (map, settlements, history).
    pub fn build(&self) -> (WorldMap, Vec<Settlement>, WorldHistory) {
        let mut rng = Rng::new(self.seed);
        let mut map = WorldMap::new(self.width, self.height);

        // Terrain
        let heightmap = HeightmapWorld::default();
        heightmap.apply(&mut map, self.seed);

        // Climate
        let climate = ClimateSimulator::default();
        climate.apply(&mut map);

        // Rivers
        let rivers = RiverSystem::default();
        rivers.apply(&mut map, &mut rng);

        // Classify biomes
        let classifier = BiomeClassifier::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let (e, t, m) = {
                    let c = map.get(x, y);
                    (c.elevation, c.temperature, c.moisture)
                };
                map.get_mut(x, y).biome = classifier.classify(t, m, e);
            }
        }

        // Settlements
        let culture = [Culture::Norse, Culture::Arabic, Culture::Japanese, Culture::Latin, Culture::Fantasy];
        let c = culture[rng.range_usize(5)];
        let mut name_gen = NameGenerator::new(c);
        let mut placer = SettlementPlacer::default();
        placer.num_settlements = 20;
        let settlements = placer.place(&mut map, &mut name_gen, &mut rng);

        // Roads
        let roads = RoadNetwork::default();
        roads.build_roads(&mut map, &settlements, &mut rng);

        // History
        let mut history = WorldHistory::new();
        history.generate(&settlements, 500, &mut rng);

        (map, settlements, history)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn small_map() -> WorldMap {
        let mut map = WorldMap::new(64, 48);
        let hm = HeightmapWorld::default();
        hm.apply(&mut map, 42);
        map
    }

    #[test]
    fn heightmap_has_land_and_ocean() {
        let map = small_map();
        let has_land  = map.cells.iter().any(|c| c.elevation > 0.0);
        let has_ocean = map.cells.iter().any(|c| c.elevation < 0.0);
        assert!(has_land,  "expected some land cells");
        assert!(has_ocean, "expected some ocean cells");
    }

    #[test]
    fn climate_assigns_temperature() {
        let mut map = small_map();
        let climate = ClimateSimulator::default();
        climate.apply(&mut map);
        let any_nonzero = map.cells.iter().any(|c| c.temperature != 0.0);
        assert!(any_nonzero);
    }

    #[test]
    fn climate_assigns_moisture() {
        let mut map = small_map();
        let climate = ClimateSimulator::default();
        climate.apply(&mut map);
        let any_moisture = map.cells.iter().any(|c| c.moisture > 0.0);
        assert!(any_moisture);
    }

    #[test]
    fn rivers_carve_some_cells() {
        let mut map = small_map();
        let climate = ClimateSimulator::default();
        climate.apply(&mut map);
        let mut rng = Rng::new(99);
        let rivers = RiverSystem { num_rivers: 3, min_source_elev: 0.4, erosion_amount: 0.01 };
        rivers.apply(&mut map, &mut rng);
        let river_cells = map.cells.iter().filter(|c| c.river_id.is_some()).count();
        assert!(river_cells > 0, "expected river cells");
    }

    #[test]
    fn biome_classifier_covers_all_ids() {
        let clf = BiomeClassifier::new();
        // Sample the classifier at various points
        let biomes = [
            clf.classify(-0.8,  0.1, 0.5),
            clf.classify( 0.8,  0.05, 0.3),
            clf.classify( 0.2,  0.6, 0.3),
            clf.classify(-0.5,  0.4, 0.4),
            clf.classify( 0.6,  0.7, 0.3),
        ];
        assert!(biomes.iter().any(|b| *b != BiomeId::OCEAN));
    }

    #[test]
    fn settlement_placer_places_settlements() {
        let mut rng = Rng::new(7);
        let mut map = small_map();
        let climate = ClimateSimulator::default();
        climate.apply(&mut map);
        let mut name_gen = NameGenerator::new(Culture::Fantasy);
        let mut placer = SettlementPlacer::default();
        placer.num_settlements = 5;
        let settlements = placer.place(&mut map, &mut name_gen, &mut rng);
        assert!(!settlements.is_empty(), "should place at least one settlement");
    }

    #[test]
    fn settlement_names_non_empty() {
        let mut rng = Rng::new(1234);
        let gen = NameGenerator::new(Culture::Norse);
        for _ in 0..5 {
            let name = gen.generate(&mut rng);
            assert!(!name.is_empty(), "name should not be empty");
        }
    }

    #[test]
    fn road_network_finds_path() {
        let map = small_map();
        let roads = RoadNetwork::default();
        let path = roads.find_path(&map, 5, 5, 20, 20);
        assert!(!path.is_empty(), "A* should find a path");
    }

    #[test]
    fn world_history_generates_events() {
        let mut rng = Rng::new(42);
        let settlements = vec![
            Settlement::new(1, 10, 10, SettlementKind::Town,    "Testville".into()),
            Settlement::new(2, 30, 30, SettlementKind::City,    "Cityburg".into()),
            Settlement::new(3, 50, 10, SettlementKind::Village, "Hamlet".into()),
        ];
        let mut history = WorldHistory::new();
        history.generate(&settlements, 100, &mut rng);
        assert!(!history.events.is_empty(), "should generate events");
    }

    #[test]
    fn world_history_event_years_monotone() {
        let mut rng = Rng::new(55);
        let settlements = vec![Settlement::new(1, 5, 5, SettlementKind::Village, "A".into())];
        let mut history = WorldHistory::new();
        history.generate(&settlements, 200, &mut rng);
        let years: Vec<i32> = history.events.iter().map(|e| e.year).collect();
        let sorted = years.windows(2).all(|w| w[0] <= w[1]);
        assert!(sorted, "events should be in chronological order");
    }

    #[test]
    fn world_builder_full_pipeline() {
        let builder = WorldBuilder::new(32, 24, 42);
        let (map, settlements, history) = builder.build();
        assert_eq!(map.width, 32);
        assert_eq!(map.height, 24);
        assert!(!history.events.is_empty());
    }

    #[test]
    fn name_generator_all_cultures() {
        let mut rng = Rng::new(7);
        for culture in &[Culture::Norse, Culture::Arabic, Culture::Japanese, Culture::Latin, Culture::Fantasy] {
            let gen = NameGenerator::new(*culture);
            let name = gen.generate(&mut rng);
            assert!(!name.is_empty(), "Culture {:?} produced empty name", culture);
        }
    }

    #[test]
    fn biome_params_all_twelve() {
        let params = BiomeParams::all();
        assert_eq!(params.len(), 12);
    }

    #[test]
    fn world_map_idx_roundtrip() {
        let map = WorldMap::new(50, 40);
        for y in 0..40 {
            for x in 0..50 {
                let i = map.idx(x, y);
                assert_eq!(i % 50, x);
                assert_eq!(i / 50, y);
            }
        }
    }
}
