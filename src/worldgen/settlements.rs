//! Settlement generation — L-system road networks + building placement.
//!
//! Places settlements at habitable locations near rivers and resources,
//! then generates internal layout using L-systems for roads and
//! zoning rules for buildings.

use super::{Rng, Grid2D};
use super::biomes::{BiomeMap, Biome};
use super::rivers::RiverNetwork;

/// Settlement size category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettlementSize {
    Hamlet,   // 5-20 buildings
    Village,  // 20-80 buildings
    Town,     // 80-300 buildings
    City,     // 300-1000 buildings
    Capital,  // 1000+ buildings
}

impl SettlementSize {
    pub fn building_range(self) -> (usize, usize) {
        match self {
            Self::Hamlet  => (5, 20),
            Self::Village => (20, 80),
            Self::Town    => (80, 300),
            Self::City    => (300, 1000),
            Self::Capital => (1000, 3000),
        }
    }

    pub fn population_range(self) -> (usize, usize) {
        match self {
            Self::Hamlet  => (10, 100),
            Self::Village => (100, 500),
            Self::Town    => (500, 5000),
            Self::City    => (5000, 50000),
            Self::Capital => (50000, 500000),
        }
    }
}

/// Building type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingType {
    House,
    Farm,
    Market,
    Temple,
    Barracks,
    Wall,
    Gate,
    Tavern,
    Smithy,
    Library,
    Palace,
    Port,
    Mine,
    Warehouse,
    Workshop,
}

/// A building in a settlement.
#[derive(Debug, Clone)]
pub struct Building {
    pub building_type: BuildingType,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub stories: u8,
}

/// A road segment.
#[derive(Debug, Clone)]
pub struct Road {
    pub start: (f32, f32),
    pub end: (f32, f32),
    pub width: f32,
    pub is_main: bool,
}

/// A complete settlement.
#[derive(Debug, Clone)]
pub struct Settlement {
    pub id: u32,
    pub name: String,
    pub grid_x: usize,
    pub grid_y: usize,
    pub size: SettlementSize,
    pub population: usize,
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    pub biome: Biome,
    pub near_river: bool,
    pub near_coast: bool,
    /// Civilization ID that owns this settlement.
    pub owner_civ: Option<u32>,
    /// Founding year.
    pub founded_year: i32,
    /// Resource score.
    pub resources: f32,
    /// Defense score.
    pub defense: f32,
}

/// Place settlements on the map.
pub fn place(
    heightmap: &Grid2D,
    biome_map: &BiomeMap,
    rivers: &RiverNetwork,
    max_settlements: usize,
    rng: &mut Rng,
) -> Vec<Settlement> {
    let w = heightmap.width;
    let h = heightmap.height;

    // Score each cell for settlement suitability
    let mut scores: Vec<(usize, usize, f32)> = Vec::new();
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let biome = biome_map.biome_at(x, y);
            if biome.is_water() { continue; }

            let mut score = biome.habitability();

            // Near river bonus
            if rivers.is_river(x, y) || has_river_neighbor(rivers, x, y, w, h) {
                score += 0.3;
            }

            // Flat terrain bonus
            let (gx, gy) = heightmap.gradient(x, y);
            let slope = (gx * gx + gy * gy).sqrt();
            score += (1.0 - slope * 5.0).max(0.0) * 0.2;

            // Resource bonus
            score += biome.resources() * 0.2;

            if score > 0.3 {
                scores.push((x, y, score));
            }
        }
    }

    // Sort by score (best first)
    scores.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // Place settlements with minimum distance between them
    let min_dist_sq = (w / 8).max(4).pow(2);
    let mut placed: Vec<(usize, usize)> = Vec::new();
    let mut settlements = Vec::new();
    let mut next_id = 0u32;

    for &(x, y, score) in &scores {
        if settlements.len() >= max_settlements { break; }

        // Check minimum distance
        let too_close = placed.iter().any(|&(px, py)| {
            let dx = x as i32 - px as i32;
            let dy = y as i32 - py as i32;
            (dx * dx + dy * dy) < min_dist_sq as i32
        });
        if too_close { continue; }

        let size = if score > 0.85 {
            SettlementSize::City
        } else if score > 0.7 {
            SettlementSize::Town
        } else if score > 0.5 {
            SettlementSize::Village
        } else {
            SettlementSize::Hamlet
        };

        let pop_range = size.population_range();
        let population = rng.range_usize(pop_range.0, pop_range.1);

        let biome = biome_map.biome_at(x, y);
        let near_river = rivers.is_river(x, y) || has_river_neighbor(rivers, x, y, w, h);

        let (buildings, roads) = generate_layout(size, rng);

        settlements.push(Settlement {
            id: next_id,
            name: generate_name(rng),
            grid_x: x,
            grid_y: y,
            size,
            population,
            buildings,
            roads,
            biome,
            near_river,
            near_coast: false, // TODO: compute from biome neighbors
            owner_civ: None,
            founded_year: 0,
            resources: biome.resources(),
            defense: 0.1,
        });

        placed.push((x, y));
        next_id += 1;
    }

    settlements
}

fn has_river_neighbor(rivers: &RiverNetwork, x: usize, y: usize, w: usize, h: usize) -> bool {
    for &(dx, dy) in &[(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
        let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
        let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
        if rivers.is_river(nx, ny) { return true; }
    }
    false
}

/// Generate settlement layout (buildings + roads) using L-system roads.
fn generate_layout(size: SettlementSize, rng: &mut Rng) -> (Vec<Building>, Vec<Road>) {
    let (min_b, max_b) = size.building_range();
    let num_buildings = rng.range_usize(min_b, max_b);

    let mut buildings = Vec::with_capacity(num_buildings);
    let mut roads = Vec::new();

    // Main road
    let main_len = num_buildings as f32 * 0.3;
    roads.push(Road {
        start: (-main_len * 0.5, 0.0),
        end: (main_len * 0.5, 0.0),
        width: 2.0,
        is_main: true,
    });

    // Cross roads (L-system branching)
    let num_cross = (num_buildings / 20).max(1);
    for i in 0..num_cross {
        let t = (i as f32 + 0.5) / num_cross as f32;
        let cx = -main_len * 0.5 + main_len * t;
        let branch_len = main_len * rng.range_f32(0.2, 0.5);
        roads.push(Road {
            start: (cx, 0.0),
            end: (cx, branch_len),
            width: 1.5,
            is_main: false,
        });
        roads.push(Road {
            start: (cx, 0.0),
            end: (cx, -branch_len),
            width: 1.5,
            is_main: false,
        });
    }

    // Place buildings along roads
    for i in 0..num_buildings {
        let road_idx = i % roads.len();
        let road = &roads[road_idx];
        let t = rng.next_f32();
        let rx = road.start.0 + (road.end.0 - road.start.0) * t;
        let ry = road.start.1 + (road.end.1 - road.start.1) * t;
        let offset = if rng.coin(0.5) { road.width + 1.0 } else { -(road.width + 1.0) };

        let btype = pick_building_type(i, num_buildings, rng);
        let (bw, bh) = building_size(btype);

        buildings.push(Building {
            building_type: btype,
            x: rx + if road.start.0 == road.end.0 { offset } else { 0.0 },
            y: ry + if road.start.1 == road.end.1 { offset } else { 0.0 },
            width: bw,
            height: bh,
            stories: if matches!(btype, BuildingType::Palace | BuildingType::Temple | BuildingType::Library) { 3 } else { rng.range_u32(1, 3) as u8 },
        });
    }

    (buildings, roads)
}

fn pick_building_type(index: usize, total: usize, rng: &mut Rng) -> BuildingType {
    if index == 0 { return BuildingType::Market; }
    if index == 1 { return BuildingType::Temple; }
    if index == 2 && total > 50 { return BuildingType::Palace; }
    match rng.range_u32(0, 10) {
        0..=4 => BuildingType::House,
        5 => BuildingType::Farm,
        6 => BuildingType::Tavern,
        7 => BuildingType::Smithy,
        8 => BuildingType::Workshop,
        _ => BuildingType::Warehouse,
    }
}

fn building_size(btype: BuildingType) -> (f32, f32) {
    match btype {
        BuildingType::House     => (2.0, 2.0),
        BuildingType::Farm      => (4.0, 3.0),
        BuildingType::Market    => (5.0, 5.0),
        BuildingType::Temple    => (4.0, 6.0),
        BuildingType::Palace    => (8.0, 8.0),
        BuildingType::Barracks  => (5.0, 4.0),
        BuildingType::Tavern    => (3.0, 3.0),
        BuildingType::Smithy    => (3.0, 2.5),
        BuildingType::Library   => (4.0, 4.0),
        BuildingType::Port      => (6.0, 3.0),
        BuildingType::Mine      => (3.0, 3.0),
        BuildingType::Warehouse => (4.0, 3.0),
        BuildingType::Workshop  => (3.0, 3.0),
        BuildingType::Wall      => (1.0, 1.0),
        BuildingType::Gate      => (2.0, 2.0),
    }
}

/// Generate a simple settlement name.
fn generate_name(rng: &mut Rng) -> String {
    let prefixes = ["Ash", "Oak", "Iron", "Storm", "Frost", "Shadow", "Gold", "Silver",
        "Red", "Blue", "Green", "White", "Black", "Stone", "River", "Lake",
        "Moon", "Sun", "Star", "Wind", "Fire", "Ice", "Dark", "Light"];
    let suffixes = ["ford", "dale", "holm", "bury", "bridge", "gate", "keep",
        "haven", "port", "vale", "fell", "crest", "wood", "field", "ton",
        "wick", "march", "mire", "shore", "cliff"];

    let prefix = prefixes[rng.next_u64() as usize % prefixes.len()];
    let suffix = suffixes[rng.next_u64() as usize % suffixes.len()];
    format!("{}{}", prefix, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settlement_name() {
        let mut rng = Rng::new(42);
        let name = generate_name(&mut rng);
        assert!(!name.is_empty());
    }

    #[test]
    fn test_layout_generation() {
        let mut rng = Rng::new(42);
        let (buildings, roads) = generate_layout(SettlementSize::Village, &mut rng);
        assert!(!buildings.is_empty());
        assert!(!roads.is_empty());
    }
}
