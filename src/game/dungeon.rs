//! Procedural floor/dungeon generation for the Chaos RPG.
//!
//! Provides full dungeon generation using BSP (Binary Space Partition), room
//! population, enemy scaling, fog of war, minimap rendering, and floor
//! progression across 100 floors of increasing difficulty and chaos.

use std::collections::HashMap;
use glam::IVec2;

use crate::procedural::{Rng, DungeonFloor as ProceduralFloor};
use crate::procedural::dungeon::{
    IRect, BspSplitter, DungeonGraph, DungeonTheme,
    Room as ProceduralRoom, Corridor as ProceduralCorridor,
};

// ══════════════════════════════════════════════════════════════════════════════
// Floor Biome
// ══════════════════════════════════════════════════════════════════════════════

/// Biome determines the visual theme, hazards, and ambient feel of a floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloorBiome {
    Ruins,
    Crypt,
    Library,
    Forge,
    Garden,
    Void,
    Chaos,
    Abyss,
    Cathedral,
    Laboratory,
}

/// Static properties for a biome.
#[derive(Debug, Clone)]
pub struct BiomeProperties {
    pub wall_char: char,
    pub floor_char: char,
    pub accent_color: (u8, u8, u8),
    pub ambient_light: f32,
    pub music_vibe: &'static str,
    pub hazard_type: HazardType,
    pub flavor_text: &'static str,
}

/// Types of environmental hazards per biome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardType {
    None,
    Crumble,
    Poison,
    Fire,
    Ice,
    Thorns,
    VoidRift,
    ChaosBurst,
    Darkness,
    Acid,
}

impl FloorBiome {
    /// Return the static properties for this biome.
    pub fn properties(self) -> BiomeProperties {
        match self {
            FloorBiome::Ruins => BiomeProperties {
                wall_char: '#',
                floor_char: '.',
                accent_color: (180, 160, 120),
                ambient_light: 0.6,
                music_vibe: "melancholy_strings",
                hazard_type: HazardType::Crumble,
                flavor_text: "Shattered walls echo with the memory of civilization.",
            },
            FloorBiome::Crypt => BiomeProperties {
                wall_char: '\u{2593}',
                floor_char: ',',
                accent_color: (100, 100, 130),
                ambient_light: 0.3,
                music_vibe: "somber_choir",
                hazard_type: HazardType::Poison,
                flavor_text: "The dead stir in their alcoves, whispering warnings.",
            },
            FloorBiome::Library => BiomeProperties {
                wall_char: '\u{2588}',
                floor_char: ':',
                accent_color: (140, 100, 60),
                ambient_light: 0.5,
                music_vibe: "quiet_ambient",
                hazard_type: HazardType::None,
                flavor_text: "Tomes of forbidden knowledge line the endless shelves.",
            },
            FloorBiome::Forge => BiomeProperties {
                wall_char: '%',
                floor_char: '=',
                accent_color: (220, 120, 40),
                ambient_light: 0.7,
                music_vibe: "industrial_rhythm",
                hazard_type: HazardType::Fire,
                flavor_text: "Molten metal pours from ancient crucibles, still burning.",
            },
            FloorBiome::Garden => BiomeProperties {
                wall_char: '&',
                floor_char: '"',
                accent_color: (60, 180, 80),
                ambient_light: 0.8,
                music_vibe: "ethereal_wind",
                hazard_type: HazardType::Thorns,
                flavor_text: "Overgrown vines conceal both beauty and peril.",
            },
            FloorBiome::Void => BiomeProperties {
                wall_char: '\u{2591}',
                floor_char: '\u{00B7}',
                accent_color: (30, 10, 60),
                ambient_light: 0.15,
                music_vibe: "deep_drone",
                hazard_type: HazardType::VoidRift,
                flavor_text: "Reality thins here. The darkness between worlds seeps in.",
            },
            FloorBiome::Chaos => BiomeProperties {
                wall_char: '?',
                floor_char: '~',
                accent_color: (200, 50, 200),
                ambient_light: 0.4,
                music_vibe: "discordant_pulse",
                hazard_type: HazardType::ChaosBurst,
                flavor_text: "The laws of nature are merely suggestions on this floor.",
            },
            FloorBiome::Abyss => BiomeProperties {
                wall_char: '\u{2592}',
                floor_char: ' ',
                accent_color: (15, 5, 15),
                ambient_light: 0.05,
                music_vibe: "silence_with_heartbeat",
                hazard_type: HazardType::Darkness,
                flavor_text: "An endless expanse of nothing. Even sound fears to travel.",
            },
            FloorBiome::Cathedral => BiomeProperties {
                wall_char: '\u{2502}',
                floor_char: '+',
                accent_color: (200, 180, 220),
                ambient_light: 0.9,
                music_vibe: "grand_organ",
                hazard_type: HazardType::None,
                flavor_text: "Stained glass casts prismatic light across the nave.",
            },
            FloorBiome::Laboratory => BiomeProperties {
                wall_char: '\u{2554}',
                floor_char: '.',
                accent_color: (80, 200, 100),
                ambient_light: 0.6,
                music_vibe: "electronic_hum",
                hazard_type: HazardType::Acid,
                flavor_text: "Bubbling vials and crackling arcs of energy fill the air.",
            },
        }
    }

    /// Convert a DungeonTheme from the procedural module to the closest biome.
    pub fn from_dungeon_theme(theme: DungeonTheme) -> Self {
        match theme {
            DungeonTheme::Cave => FloorBiome::Ruins,
            DungeonTheme::Cathedral => FloorBiome::Cathedral,
            DungeonTheme::Laboratory => FloorBiome::Laboratory,
            DungeonTheme::Temple => FloorBiome::Library,
            DungeonTheme::Ruins => FloorBiome::Ruins,
            DungeonTheme::Void => FloorBiome::Void,
        }
    }

    /// Get the DungeonTheme most closely matching this biome.
    pub fn to_dungeon_theme(self) -> DungeonTheme {
        match self {
            FloorBiome::Ruins => DungeonTheme::Ruins,
            FloorBiome::Crypt => DungeonTheme::Cathedral,
            FloorBiome::Library => DungeonTheme::Temple,
            FloorBiome::Forge => DungeonTheme::Laboratory,
            FloorBiome::Garden => DungeonTheme::Cave,
            FloorBiome::Void => DungeonTheme::Void,
            FloorBiome::Chaos => DungeonTheme::Void,
            FloorBiome::Abyss => DungeonTheme::Void,
            FloorBiome::Cathedral => DungeonTheme::Cathedral,
            FloorBiome::Laboratory => DungeonTheme::Laboratory,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Room Types
// ══════════════════════════════════════════════════════════════════════════════

/// Room types in the Chaos RPG dungeon.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomType {
    Normal,
    Combat,
    Treasure,
    Shop,
    Shrine,
    Trap,
    Puzzle,
    MiniBoss,
    Boss,
    ChaosRift,
    Rest,
    Secret,
    Library,
    Forge,
}

impl RoomType {
    /// Weighted random selection of room type for a given floor depth.
    pub fn random_for_floor(floor: u32, rng: &mut Rng) -> Self {
        let mut weights: Vec<(RoomType, f32)> = vec![
            (RoomType::Normal, 30.0),
            (RoomType::Combat, 25.0),
            (RoomType::Treasure, 10.0),
            (RoomType::Trap, 8.0),
            (RoomType::Rest, 6.0),
            (RoomType::Shrine, 5.0),
            (RoomType::Puzzle, 5.0),
            (RoomType::Shop, 4.0),
            (RoomType::Library, 3.0),
            (RoomType::Forge, 2.0),
            (RoomType::Secret, 1.5),
            (RoomType::ChaosRift, 0.5),
        ];
        // Higher floors increase combat/trap, decrease rest
        if floor > 25 {
            for entry in &mut weights {
                match entry.0 {
                    RoomType::Combat => entry.1 += 10.0,
                    RoomType::Trap => entry.1 += 5.0,
                    RoomType::Rest => entry.1 = (entry.1 - 2.0).max(1.0),
                    RoomType::ChaosRift => entry.1 += 3.0,
                    _ => {}
                }
            }
        }
        if floor > 50 {
            for entry in &mut weights {
                match entry.0 {
                    RoomType::ChaosRift => entry.1 += 5.0,
                    RoomType::Normal => entry.1 = (entry.1 - 10.0).max(5.0),
                    _ => {}
                }
            }
        }
        let total: f32 = weights.iter().map(|(_, w)| *w).sum();
        let mut r = rng.next_f32() * total;
        for (rt, w) in &weights {
            r -= w;
            if r <= 0.0 {
                return rt.clone();
            }
        }
        RoomType::Normal
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Room Shape
// ══════════════════════════════════════════════════════════════════════════════

/// Shape of the room within its bounding rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomShape {
    Rectangle,
    LShaped,
    Circular,
    Irregular,
}

impl RoomShape {
    /// Pick a room shape based on corruption level.
    pub fn random(corruption: u32, rng: &mut Rng) -> Self {
        let irregular_chance = (corruption as f32 / 100.0).min(0.3);
        let circular_chance = 0.08;
        let l_shaped_chance = 0.15;
        let roll = rng.next_f32();
        if roll < irregular_chance {
            RoomShape::Irregular
        } else if roll < irregular_chance + circular_chance {
            RoomShape::Circular
        } else if roll < irregular_chance + circular_chance + l_shaped_chance {
            RoomShape::LShaped
        } else {
            RoomShape::Rectangle
        }
    }

    /// Carve tiles for this shape within a given rect onto a tile grid.
    pub fn carve(&self, rect: &IRect, tiles: &mut Vec<Tile>, map_width: usize, rng: &mut Rng) {
        let x0 = rect.x as usize;
        let y0 = rect.y as usize;
        let w = rect.w as usize;
        let h = rect.h as usize;
        match self {
            RoomShape::Rectangle => {
                for dy in 0..h {
                    for dx in 0..w {
                        let idx = (y0 + dy) * map_width + (x0 + dx);
                        if idx < tiles.len() {
                            tiles[idx] = Tile::Floor;
                        }
                    }
                }
            }
            RoomShape::LShaped => {
                // Main body
                let half_w = w / 2;
                let half_h = h / 2;
                for dy in 0..h {
                    for dx in 0..w {
                        if dx < half_w || dy < half_h {
                            let idx = (y0 + dy) * map_width + (x0 + dx);
                            if idx < tiles.len() {
                                tiles[idx] = Tile::Floor;
                            }
                        }
                    }
                }
            }
            RoomShape::Circular => {
                let cx = w as f32 / 2.0;
                let cy = h as f32 / 2.0;
                let rx = cx - 0.5;
                let ry = cy - 0.5;
                for dy in 0..h {
                    for dx in 0..w {
                        let fx = dx as f32 - cx + 0.5;
                        let fy = dy as f32 - cy + 0.5;
                        if (fx * fx) / (rx * rx) + (fy * fy) / (ry * ry) <= 1.0 {
                            let idx = (y0 + dy) * map_width + (x0 + dx);
                            if idx < tiles.len() {
                                tiles[idx] = Tile::Floor;
                            }
                        }
                    }
                }
            }
            RoomShape::Irregular => {
                // Cellular automata blob
                let mut grid = vec![false; w * h];
                // Seed ~55% filled
                for cell in grid.iter_mut() {
                    *cell = rng.next_f32() < 0.55;
                }
                // 3 smoothing iterations
                for _ in 0..3 {
                    let mut next = grid.clone();
                    for dy in 0..h {
                        for dx in 0..w {
                            let mut alive = 0;
                            for ny in dy.saturating_sub(1)..=(dy + 1).min(h - 1) {
                                for nx in dx.saturating_sub(1)..=(dx + 1).min(w - 1) {
                                    if grid[ny * w + nx] {
                                        alive += 1;
                                    }
                                }
                            }
                            next[dy * w + dx] = alive >= 5;
                        }
                    }
                    grid = next;
                }
                for dy in 0..h {
                    for dx in 0..w {
                        if grid[dy * w + dx] {
                            let idx = (y0 + dy) * map_width + (x0 + dx);
                            if idx < tiles.len() {
                                tiles[idx] = Tile::Floor;
                            }
                        }
                    }
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Corridor Style
// ══════════════════════════════════════════════════════════════════════════════

/// How corridors are generated between rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorridorStyle {
    Straight,
    Winding,
    Organic,
}

impl CorridorStyle {
    /// Carve a corridor path into the tile grid.
    pub fn carve_corridor(
        &self,
        from: IVec2,
        to: IVec2,
        tiles: &mut Vec<Tile>,
        map_width: usize,
        map_height: usize,
        rng: &mut Rng,
    ) -> Vec<IVec2> {
        match self {
            CorridorStyle::Straight => {
                Self::carve_l_bend(from, to, tiles, map_width, rng)
            }
            CorridorStyle::Winding => {
                Self::carve_winding(from, to, tiles, map_width, map_height, rng)
            }
            CorridorStyle::Organic => {
                Self::carve_organic(from, to, tiles, map_width, map_height, rng)
            }
        }
    }

    fn carve_l_bend(
        from: IVec2,
        to: IVec2,
        tiles: &mut Vec<Tile>,
        map_width: usize,
        rng: &mut Rng,
    ) -> Vec<IVec2> {
        let mut path = Vec::new();
        let bend = if rng.chance(0.5) {
            IVec2::new(to.x, from.y)
        } else {
            IVec2::new(from.x, to.y)
        };
        // from -> bend
        let mut cur = from;
        while cur != bend {
            Self::set_tile(cur, tiles, map_width, Tile::Corridor);
            path.push(cur);
            if cur.x < bend.x { cur.x += 1; }
            else if cur.x > bend.x { cur.x -= 1; }
            if cur.y < bend.y { cur.y += 1; }
            else if cur.y > bend.y { cur.y -= 1; }
        }
        // bend -> to
        while cur != to {
            Self::set_tile(cur, tiles, map_width, Tile::Corridor);
            path.push(cur);
            if cur.x < to.x { cur.x += 1; }
            else if cur.x > to.x { cur.x -= 1; }
            if cur.y < to.y { cur.y += 1; }
            else if cur.y > to.y { cur.y -= 1; }
        }
        Self::set_tile(to, tiles, map_width, Tile::Corridor);
        path.push(to);
        path
    }

    fn carve_winding(
        from: IVec2,
        to: IVec2,
        tiles: &mut Vec<Tile>,
        map_width: usize,
        map_height: usize,
        rng: &mut Rng,
    ) -> Vec<IVec2> {
        // Perlin-like displacement: walk from->to with random perpendicular jitter
        let mut path = Vec::new();
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let steps = (dx.abs() + dy.abs()).max(1) as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let base_x = from.x as f32 + dx as f32 * t;
            let base_y = from.y as f32 + dy as f32 * t;
            // Perpendicular displacement using pseudo-Perlin noise
            let phase = t * std::f32::consts::PI * 3.0 + rng.next_f32() * 0.3;
            let amplitude = 2.0 + rng.next_f32() * 1.5;
            let norm_len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
            let perp_x = -(dy as f32) / norm_len;
            let perp_y = (dx as f32) / norm_len;
            let disp = phase.sin() * amplitude;
            let px = (base_x + perp_x * disp).round() as i32;
            let py = (base_y + perp_y * disp).round() as i32;
            let clamped = IVec2::new(
                px.clamp(1, map_width as i32 - 2),
                py.clamp(1, map_height as i32 - 2),
            );
            Self::set_tile(clamped, tiles, map_width, Tile::Corridor);
            path.push(clamped);
        }
        path
    }

    fn carve_organic(
        from: IVec2,
        to: IVec2,
        tiles: &mut Vec<Tile>,
        map_width: usize,
        map_height: usize,
        rng: &mut Rng,
    ) -> Vec<IVec2> {
        // Random walk biased towards target, then cellular automata smoothing
        let mut path = Vec::new();
        let mut cur = from;
        let max_steps = ((to.x - from.x).abs() + (to.y - from.y).abs()) as usize * 3 + 20;
        for _ in 0..max_steps {
            Self::set_tile(cur, tiles, map_width, Tile::Corridor);
            path.push(cur);
            if cur == to {
                break;
            }
            let dx = (to.x - cur.x).signum();
            let dy = (to.y - cur.y).signum();
            // 60% move toward target, 40% random walk
            if rng.chance(0.6) {
                if rng.chance(0.5) && dx != 0 {
                    cur.x += dx;
                } else if dy != 0 {
                    cur.y += dy;
                } else {
                    cur.x += dx;
                }
            } else {
                match rng.range_usize(4) {
                    0 => cur.x += 1,
                    1 => cur.x -= 1,
                    2 => cur.y += 1,
                    _ => cur.y -= 1,
                }
            }
            cur.x = cur.x.clamp(1, map_width as i32 - 2);
            cur.y = cur.y.clamp(1, map_height as i32 - 2);
        }
        // Widen the path slightly with adjacent tiles
        let widen: Vec<IVec2> = path.clone();
        for p in &widen {
            for offset in &[IVec2::new(1, 0), IVec2::new(0, 1)] {
                let adj = *p + *offset;
                if adj.x > 0
                    && adj.x < map_width as i32 - 1
                    && adj.y > 0
                    && adj.y < map_height as i32 - 1
                {
                    if rng.chance(0.3) {
                        Self::set_tile(adj, tiles, map_width, Tile::Corridor);
                    }
                }
            }
        }
        path
    }

    fn set_tile(pos: IVec2, tiles: &mut [Tile], map_width: usize, tile: Tile) {
        let idx = pos.y as usize * map_width + pos.x as usize;
        if idx < tiles.len() && tiles[idx] == Tile::Wall {
            tiles[idx] = tile;
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tile
// ══════════════════════════════════════════════════════════════════════════════

/// Extended tile types for the Chaos RPG floor map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    Corridor,
    Door,
    StairsDown,
    StairsUp,
    Trap,
    Chest,
    Shrine,
    ShopCounter,
    Void,
    SecretWall,
    Water,
    Lava,
    Ice,
}

/// Physical properties of a tile.
#[derive(Debug, Clone)]
pub struct TileProperties {
    pub walkable: bool,
    pub blocks_sight: bool,
    pub damage_on_step: Option<f32>,
    pub slow_factor: f32,
    pub element: Option<Element>,
}

/// Elemental affinity for tiles and attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Fire,
    Ice,
    Poison,
    Lightning,
    Void,
    Chaos,
    Holy,
    Dark,
}

impl Tile {
    /// Get physical properties for this tile type.
    pub fn properties(self) -> TileProperties {
        match self {
            Tile::Floor => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Wall => TileProperties {
                walkable: false, blocks_sight: true,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Corridor => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Door => TileProperties {
                walkable: true, blocks_sight: true,
                damage_on_step: None, slow_factor: 0.8, element: None,
            },
            Tile::StairsDown | Tile::StairsUp => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Trap => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: Some(10.0), slow_factor: 0.5, element: None,
            },
            Tile::Chest => TileProperties {
                walkable: false, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Shrine => TileProperties {
                walkable: false, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.0, element: Some(Element::Holy),
            },
            Tile::ShopCounter => TileProperties {
                walkable: false, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Void => TileProperties {
                walkable: false, blocks_sight: true,
                damage_on_step: Some(999.0), slow_factor: 0.0, element: Some(Element::Void),
            },
            Tile::SecretWall => TileProperties {
                walkable: false, blocks_sight: true,
                damage_on_step: None, slow_factor: 1.0, element: None,
            },
            Tile::Water => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: None, slow_factor: 0.5, element: Some(Element::Ice),
            },
            Tile::Lava => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: Some(25.0), slow_factor: 0.3, element: Some(Element::Fire),
            },
            Tile::Ice => TileProperties {
                walkable: true, blocks_sight: false,
                damage_on_step: None, slow_factor: 1.5, element: Some(Element::Ice),
            },
        }
    }

    /// Whether entities can walk through this tile.
    pub fn is_walkable(self) -> bool {
        self.properties().walkable
    }

    /// Whether this tile blocks line of sight.
    pub fn blocks_sight(self) -> bool {
        self.properties().blocks_sight
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Floor Config
// ══════════════════════════════════════════════════════════════════════════════

/// Configuration for generating a single dungeon floor.
#[derive(Debug, Clone)]
pub struct FloorConfig {
    pub floor_number: u32,
    pub room_count_range: (u32, u32),
    pub corridor_style: CorridorStyle,
    pub difficulty_mult: f32,
    pub biome: FloorBiome,
    pub corruption_level: u32,
    pub special_rooms: Vec<RoomType>,
    pub boss_floor: bool,
}

impl FloorConfig {
    /// Generate a config automatically based on floor number.
    pub fn for_floor(floor_number: u32) -> Self {
        let theme = FloorTheme::for_floor(floor_number);
        let boss_floor = floor_number % 10 == 0 || floor_number == 100;
        let room_min = if boss_floor { 5 } else { 6 + (floor_number / 15).min(6) };
        let room_max = if boss_floor { 8 } else { 10 + (floor_number / 10).min(10) };
        let corridor_style = if floor_number < 11 {
            CorridorStyle::Straight
        } else if floor_number < 51 {
            CorridorStyle::Winding
        } else {
            CorridorStyle::Organic
        };
        let corruption = (floor_number.saturating_sub(25) * 2).min(200);
        let mut special_rooms = Vec::new();
        if boss_floor {
            special_rooms.push(RoomType::Boss);
        }
        if floor_number % 5 == 0 {
            special_rooms.push(RoomType::Shop);
        }
        special_rooms.push(RoomType::Rest);

        FloorConfig {
            floor_number,
            room_count_range: (room_min, room_max),
            corridor_style,
            difficulty_mult: theme.difficulty_mult,
            biome: theme.biome,
            corruption_level: corruption,
            special_rooms,
            boss_floor,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Floor Theme
// ══════════════════════════════════════════════════════════════════════════════

/// Defines the feel of each floor range.
#[derive(Debug, Clone)]
pub struct FloorTheme {
    pub biome: FloorBiome,
    pub difficulty_mult: f32,
    pub palette: ThemePalette,
    pub description: &'static str,
    pub traps_enabled: bool,
    pub puzzles_enabled: bool,
    pub min_safe_rooms: u32,
}

/// Color palette hint for a theme.
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub primary: (u8, u8, u8),
    pub secondary: (u8, u8, u8),
    pub accent: (u8, u8, u8),
}

impl FloorTheme {
    /// Determine the theme for a given floor number.
    pub fn for_floor(floor: u32) -> Self {
        match floor {
            1..=10 => FloorTheme {
                biome: FloorBiome::Ruins,
                difficulty_mult: 1.0,
                palette: ThemePalette {
                    primary: (180, 140, 100),
                    secondary: (140, 110, 80),
                    accent: (220, 180, 120),
                },
                description: "The crumbling entrance. Warm torchlight guides the way.",
                traps_enabled: false,
                puzzles_enabled: false,
                min_safe_rooms: 3,
            },
            11..=25 => FloorTheme {
                biome: FloorBiome::Crypt,
                difficulty_mult: 1.5,
                palette: ThemePalette {
                    primary: (100, 100, 140),
                    secondary: (70, 70, 110),
                    accent: (150, 130, 180),
                },
                description: "Ancient burial grounds. Traps protect the forgotten dead.",
                traps_enabled: true,
                puzzles_enabled: false,
                min_safe_rooms: 2,
            },
            26..=50 => FloorTheme {
                biome: FloorBiome::Forge,
                difficulty_mult: 2.0,
                palette: ThemePalette {
                    primary: (180, 120, 60),
                    secondary: (60, 160, 80),
                    accent: (200, 200, 80),
                },
                description: "The workshop depths. Puzzles guard ancient knowledge.",
                traps_enabled: true,
                puzzles_enabled: true,
                min_safe_rooms: 2,
            },
            51..=75 => FloorTheme {
                biome: FloorBiome::Void,
                difficulty_mult: 3.0,
                palette: ThemePalette {
                    primary: (40, 20, 80),
                    secondary: (20, 10, 50),
                    accent: (180, 60, 200),
                },
                description: "Reality fractures. Corruption seeps through every crack.",
                traps_enabled: true,
                puzzles_enabled: true,
                min_safe_rooms: 1,
            },
            76..=99 => FloorTheme {
                biome: FloorBiome::Abyss,
                difficulty_mult: 4.5,
                palette: ThemePalette {
                    primary: (20, 15, 20),
                    secondary: (10, 5, 10),
                    accent: (60, 40, 60),
                },
                description: "The bottomless dark. Few safe havens remain.",
                traps_enabled: true,
                puzzles_enabled: true,
                min_safe_rooms: 0,
            },
            100 => FloorTheme {
                biome: FloorBiome::Cathedral,
                difficulty_mult: 6.0,
                palette: ThemePalette {
                    primary: (200, 180, 220),
                    secondary: (160, 140, 180),
                    accent: (255, 220, 255),
                },
                description: "The Cathedral of the Algorithm. The final reckoning.",
                traps_enabled: false,
                puzzles_enabled: false,
                min_safe_rooms: 0,
            },
            _ => FloorTheme {
                biome: FloorBiome::Chaos,
                difficulty_mult: 5.0 + (floor as f32 - 100.0) * 0.1,
                palette: ThemePalette {
                    primary: (200, 50, 200),
                    secondary: (100, 30, 150),
                    accent: (255, 100, 255),
                },
                description: "Beyond the Algorithm. Pure chaos reigns.",
                traps_enabled: true,
                puzzles_enabled: true,
                min_safe_rooms: 0,
            },
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Floor Map
// ══════════════════════════════════════════════════════════════════════════════

/// A room on the Chaos RPG floor.
#[derive(Debug, Clone)]
pub struct DungeonRoom {
    pub id: usize,
    pub rect: IRect,
    pub room_type: RoomType,
    pub shape: RoomShape,
    pub connections: Vec<usize>,
    pub spawn_points: Vec<IVec2>,
    pub items: Vec<RoomItem>,
    pub enemies: Vec<EnemySpawn>,
    pub visited: bool,
    pub cleared: bool,
}

/// An item placed in a room.
#[derive(Debug, Clone)]
pub struct RoomItem {
    pub pos: IVec2,
    pub kind: RoomItemKind,
}

/// Types of interactable items in rooms.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomItemKind {
    Chest { trapped: bool, loot_tier: u32 },
    HealingShrine,
    BuffShrine { buff_name: String, floors_remaining: u32 },
    RiskShrine,
    Merchant { item_count: u32, price_mult: f32 },
    Campfire,
    ForgeAnvil,
    LoreBook { entry_id: u32 },
    SpellScroll { spell_name: String },
    PuzzleBlock { target: IVec2 },
}

/// An enemy to be spawned in a room.
#[derive(Debug, Clone)]
pub struct EnemySpawn {
    pub pos: IVec2,
    pub stats: ScaledStats,
    pub name: String,
    pub element: Option<Element>,
    pub is_elite: bool,
    pub abilities: Vec<String>,
}

/// A corridor in the floor map.
#[derive(Debug, Clone)]
pub struct DungeonCorridor {
    pub from_room: usize,
    pub to_room: usize,
    pub path: Vec<IVec2>,
    pub style: CorridorStyle,
    pub has_door: bool,
}

/// The full tile map for a single floor.
#[derive(Debug, Clone)]
pub struct FloorMap {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Tile>,
    pub rooms: Vec<DungeonRoom>,
    pub corridors: Vec<DungeonCorridor>,
    pub player_start: IVec2,
    pub exit_point: IVec2,
    pub biome: FloorBiome,
    pub floor_number: u32,
}

impl FloorMap {
    /// Get the tile at (x, y), or Wall if out of bounds.
    pub fn get_tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Tile::Wall;
        }
        self.tiles[y as usize * self.width + x as usize]
    }

    /// Set the tile at (x, y) if in bounds.
    pub fn set_tile(&mut self, x: i32, y: i32, tile: Tile) {
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            self.tiles[y as usize * self.width + x as usize] = tile;
        }
    }

    /// Find which room contains the given position.
    pub fn room_at(&self, pos: IVec2) -> Option<&DungeonRoom> {
        self.rooms.iter().find(|r| r.rect.contains(pos.x, pos.y))
    }

    /// Find which room contains the given position (mutable).
    pub fn room_at_mut(&mut self, pos: IVec2) -> Option<&mut DungeonRoom> {
        self.rooms.iter_mut().find(|r| r.rect.contains(pos.x, pos.y))
    }

    /// Return all walkable neighbor positions of `pos`.
    pub fn walkable_neighbors(&self, pos: IVec2) -> Vec<IVec2> {
        let offsets = [
            IVec2::new(1, 0), IVec2::new(-1, 0),
            IVec2::new(0, 1), IVec2::new(0, -1),
        ];
        offsets
            .iter()
            .map(|o| pos + *o)
            .filter(|p| self.get_tile(p.x, p.y).is_walkable())
            .collect()
    }

    /// Total number of tiles.
    pub fn tile_count(&self) -> usize {
        self.width * self.height
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Floor (wraps FloorMap + metadata)
// ══════════════════════════════════════════════════════════════════════════════

/// A complete dungeon floor with map, fog, and metadata.
#[derive(Debug, Clone)]
pub struct Floor {
    pub map: FloorMap,
    pub fog: FogOfWar,
    pub seed: u64,
    pub config: FloorConfig,
    pub enemies_alive: u32,
    pub total_enemies: u32,
    pub cleared: bool,
}

impl Floor {
    /// Check if the floor has been fully cleared of enemies.
    pub fn check_cleared(&mut self) -> bool {
        self.enemies_alive = self.map.rooms.iter()
            .flat_map(|r| r.enemies.iter())
            .count() as u32;
        self.cleared = self.enemies_alive == 0;
        self.cleared
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Fog of War
// ══════════════════════════════════════════════════════════════════════════════

/// Visibility state of a single tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Unseen,
    Seen,
    Visible,
}

/// Fog of war tracking for a floor.
#[derive(Debug, Clone)]
pub struct FogOfWar {
    pub width: usize,
    pub height: usize,
    pub visibility: Vec<Visibility>,
}

impl FogOfWar {
    /// Create a fully unseen fog.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            visibility: vec![Visibility::Unseen; width * height],
        }
    }

    /// Get visibility at a position.
    pub fn get(&self, x: i32, y: i32) -> Visibility {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Visibility::Unseen;
        }
        self.visibility[y as usize * self.width + x as usize]
    }

    /// Set visibility at a position.
    pub fn set(&mut self, x: i32, y: i32, vis: Visibility) {
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            self.visibility[y as usize * self.width + x as usize] = vis;
        }
    }

    /// Demote all Visible tiles to Seen (called before recalculating LOS).
    pub fn fade_visible(&mut self) {
        for v in self.visibility.iter_mut() {
            if *v == Visibility::Visible {
                *v = Visibility::Seen;
            }
        }
    }

    /// Reveal tiles around `center` using raycast-based line of sight.
    pub fn reveal_around(&mut self, center: IVec2, radius: i32, floor_map: &FloorMap) {
        self.fade_visible();
        // Cast rays in all directions
        let steps = (radius * 8).max(32);
        for i in 0..steps {
            let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let dx = angle.cos();
            let dy = angle.sin();
            let mut cx = center.x as f32 + 0.5;
            let mut cy = center.y as f32 + 0.5;
            for _ in 0..=radius {
                let tx = cx as i32;
                let ty = cy as i32;
                if tx < 0 || ty < 0 || tx >= self.width as i32 || ty >= self.height as i32 {
                    break;
                }
                self.set(tx, ty, Visibility::Visible);
                if floor_map.get_tile(tx, ty).blocks_sight() && (tx != center.x || ty != center.y)
                {
                    break;
                }
                cx += dx;
                cy += dy;
            }
        }
    }

    /// Count how many tiles have been seen or are visible.
    pub fn explored_count(&self) -> usize {
        self.visibility
            .iter()
            .filter(|v| **v != Visibility::Unseen)
            .count()
    }

    /// Fraction of map explored.
    pub fn explored_fraction(&self) -> f32 {
        let total = self.visibility.len();
        if total == 0 {
            return 0.0;
        }
        self.explored_count() as f32 / total as f32
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Enemy Scaling
// ══════════════════════════════════════════════════════════════════════════════

/// Base stats for an enemy before scaling.
#[derive(Debug, Clone)]
pub struct BaseStats {
    pub hp: f32,
    pub damage: f32,
    pub defense: f32,
    pub speed: f32,
    pub xp_value: u32,
}

/// Stats after floor/corruption scaling.
#[derive(Debug, Clone)]
pub struct ScaledStats {
    pub hp: f32,
    pub damage: f32,
    pub defense: f32,
    pub speed: f32,
    pub xp_value: u32,
    pub level: u32,
}

/// Scales enemy stats based on floor depth and corruption.
pub struct EnemyScaler;

impl EnemyScaler {
    /// Scale base stats for a given floor and corruption level.
    ///
    /// - HP scales: base * (1.0 + floor * 0.08)
    /// - Damage scales: base * (1.0 + floor * 0.05)
    /// - Corruption adds +1% per 10 corruption to all stats
    /// - Every 10 floors: enemies gain a new ability
    /// - Every 25 floors: new enemy types unlock
    /// - Floor 50+: elite variants
    pub fn scale_enemy(base: &BaseStats, floor: u32, corruption: u32) -> ScaledStats {
        let floor_hp_mult = 1.0 + floor as f32 * 0.08;
        let floor_dmg_mult = 1.0 + floor as f32 * 0.05;
        let floor_def_mult = 1.0 + floor as f32 * 0.03;
        let floor_spd_mult = 1.0 + floor as f32 * 0.01;

        let corruption_mult = 1.0 + (corruption as f32 / 10.0) * 0.01;

        let hp = base.hp * floor_hp_mult * corruption_mult;
        let damage = base.damage * floor_dmg_mult * corruption_mult;
        let defense = base.defense * floor_def_mult * corruption_mult;
        let speed = base.speed * floor_spd_mult * corruption_mult;
        let xp_mult = 1.0 + floor as f32 * 0.04;
        let xp_value = (base.xp_value as f32 * xp_mult * corruption_mult) as u32;

        ScaledStats {
            hp,
            damage,
            defense,
            speed,
            xp_value,
            level: floor,
        }
    }

    /// Determine how many abilities an enemy should have based on floor.
    pub fn ability_count(floor: u32) -> u32 {
        floor / 10
    }

    /// Determine if new enemy types are unlocked at this floor.
    pub fn new_types_unlocked(floor: u32) -> bool {
        floor % 25 == 0 && floor > 0
    }

    /// Determine the name prefix for elite enemies on higher floors.
    pub fn elite_prefix(floor: u32, rng: &mut Rng) -> Option<&'static str> {
        if floor < 50 {
            return None;
        }
        let prefixes = ["Corrupted", "Ancient", "Void-touched"];
        rng.pick(&prefixes).copied()
    }

    /// Generate abilities for an enemy at a given floor.
    pub fn generate_abilities(floor: u32, rng: &mut Rng) -> Vec<String> {
        let count = Self::ability_count(floor) as usize;
        let pool = [
            "Charge", "Enrage", "Shield Bash", "Poison Strike", "Teleport",
            "Summon Minion", "Life Drain", "Fire Breath", "Frost Nova",
            "Shadow Step", "Berserk", "Heal Pulse", "Void Bolt", "Chain Lightning",
            "Earthquake", "Mirror Image", "Petrify Gaze", "Soul Rend",
        ];
        let mut abilities = Vec::new();
        let mut indices: Vec<usize> = (0..pool.len()).collect();
        rng.shuffle(&mut indices);
        for &i in indices.iter().take(count.min(pool.len())) {
            abilities.push(pool[i].to_string());
        }
        abilities
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Room Populator
// ══════════════════════════════════════════════════════════════════════════════

/// Populates rooms with enemies, items, and interactables based on type.
pub struct RoomPopulator;

impl RoomPopulator {
    /// Populate a room based on its type, floor depth, and biome.
    pub fn populate(
        room: &mut DungeonRoom,
        floor: u32,
        corruption: u32,
        biome: FloorBiome,
        rng: &mut Rng,
    ) {
        match room.room_type {
            RoomType::Combat => Self::populate_combat(room, floor, corruption, biome, rng),
            RoomType::Treasure => Self::populate_treasure(room, floor, rng),
            RoomType::Shop => Self::populate_shop(room, floor, rng),
            RoomType::Shrine => Self::populate_shrine(room, rng),
            RoomType::Trap => Self::populate_trap(room, floor, rng),
            RoomType::Puzzle => Self::populate_puzzle(room, rng),
            RoomType::MiniBoss => Self::populate_miniboss(room, floor, corruption, biome, rng),
            RoomType::Boss => Self::populate_boss(room, floor, corruption, biome, rng),
            RoomType::ChaosRift => Self::populate_chaos_rift(room, floor, corruption, rng),
            RoomType::Rest => Self::populate_rest(room, rng),
            RoomType::Secret => Self::populate_secret(room, floor, rng),
            RoomType::Library => Self::populate_library(room, floor, rng),
            RoomType::Forge => Self::populate_forge(room, rng),
            RoomType::Normal => Self::populate_normal(room, floor, corruption, biome, rng),
        }
    }

    fn random_pos_in_room(room: &DungeonRoom, rng: &mut Rng) -> IVec2 {
        let r = &room.rect;
        IVec2::new(
            rng.range_i32(r.x + 1, (r.x + r.w - 2).max(r.x + 1)),
            rng.range_i32(r.y + 1, (r.y + r.h - 2).max(r.y + 1)),
        )
    }

    fn element_for_biome(biome: FloorBiome, rng: &mut Rng) -> Option<Element> {
        let options = match biome {
            FloorBiome::Forge => vec![Element::Fire],
            FloorBiome::Crypt => vec![Element::Dark, Element::Poison],
            FloorBiome::Garden => vec![Element::Poison],
            FloorBiome::Void => vec![Element::Void],
            FloorBiome::Chaos => vec![Element::Chaos, Element::Void, Element::Fire, Element::Lightning],
            FloorBiome::Abyss => vec![Element::Dark, Element::Void],
            FloorBiome::Cathedral => vec![Element::Holy, Element::Lightning],
            FloorBiome::Laboratory => vec![Element::Lightning, Element::Poison],
            FloorBiome::Library => vec![Element::Fire],
            FloorBiome::Ruins => return None,
        };
        if options.is_empty() {
            return None;
        }
        Some(options[rng.range_usize(options.len())])
    }

    fn make_enemy(
        name: &str,
        pos: IVec2,
        floor: u32,
        corruption: u32,
        element: Option<Element>,
        rng: &mut Rng,
    ) -> EnemySpawn {
        let base = BaseStats {
            hp: 30.0,
            damage: 8.0,
            defense: 3.0,
            speed: 1.0,
            xp_value: 10,
        };
        let stats = EnemyScaler::scale_enemy(&base, floor, corruption);
        let is_elite = floor >= 50 && rng.chance(0.2);
        let prefix = if is_elite {
            EnemyScaler::elite_prefix(floor, rng).unwrap_or("Elite")
        } else {
            ""
        };
        let full_name = if is_elite {
            format!("{} {}", prefix, name)
        } else {
            name.to_string()
        };
        let abilities = EnemyScaler::generate_abilities(floor, rng);
        EnemySpawn {
            pos,
            stats,
            name: full_name,
            element,
            is_elite,
            abilities,
        }
    }

    fn populate_combat(
        room: &mut DungeonRoom,
        floor: u32,
        corruption: u32,
        biome: FloorBiome,
        rng: &mut Rng,
    ) {
        let count = rng.range_i32(2, 6) as usize;
        let element = Self::element_for_biome(biome, rng);
        let enemy_names = ["Shade", "Wraith", "Golem", "Serpent", "Husk", "Warden"];
        for _ in 0..count {
            let pos = Self::random_pos_in_room(room, rng);
            let name = enemy_names[rng.range_usize(enemy_names.len())];
            room.enemies.push(Self::make_enemy(name, pos, floor, corruption, element, rng));
        }
    }

    fn populate_treasure(room: &mut DungeonRoom, floor: u32, rng: &mut Rng) {
        let count = rng.range_i32(1, 3) as usize;
        for _ in 0..count {
            let pos = Self::random_pos_in_room(room, rng);
            let trapped = rng.chance(0.25);
            let tier = 1 + floor / 10;
            room.items.push(RoomItem {
                pos,
                kind: RoomItemKind::Chest { trapped, loot_tier: tier },
            });
        }
    }

    fn populate_shop(room: &mut DungeonRoom, floor: u32, rng: &mut Rng) {
        let center = room.rect.center();
        let item_count = rng.range_i32(4, 8) as u32;
        let price_mult = 1.0 + floor as f32 * 0.05;
        room.items.push(RoomItem {
            pos: center,
            kind: RoomItemKind::Merchant { item_count, price_mult },
        });
    }

    fn populate_shrine(room: &mut DungeonRoom, rng: &mut Rng) {
        let pos = room.rect.center();
        let roll = rng.next_f32();
        let kind = if roll < 0.4 {
            RoomItemKind::HealingShrine
        } else if roll < 0.75 {
            let buffs = ["Fortitude", "Swiftness", "Might", "Arcane Sight", "Iron Skin"];
            let buff_name = buffs[rng.range_usize(buffs.len())].to_string();
            RoomItemKind::BuffShrine {
                buff_name,
                floors_remaining: 5,
            }
        } else {
            RoomItemKind::RiskShrine
        };
        room.items.push(RoomItem { pos, kind });
    }

    fn populate_trap(room: &mut DungeonRoom, floor: u32, rng: &mut Rng) {
        let count = rng.range_i32(2, 4) as usize;
        let trap_names = ["Pendulum", "Spikes", "Arrows", "Flames"];
        for _ in 0..count {
            let pos = Self::random_pos_in_room(room, rng);
            room.spawn_points.push(pos);
        }
        // Also place a small reward for surviving
        if rng.chance(0.5) {
            let pos = Self::random_pos_in_room(room, rng);
            let tier = 1 + floor / 15;
            room.items.push(RoomItem {
                pos,
                kind: RoomItemKind::Chest { trapped: false, loot_tier: tier },
            });
        }
        let _ = trap_names; // Used by TrapSystem in runtime
    }

    fn populate_puzzle(room: &mut DungeonRoom, rng: &mut Rng) {
        // Place 2-3 puzzle blocks with target positions
        let count = rng.range_i32(2, 3) as usize;
        for _ in 0..count {
            let pos = Self::random_pos_in_room(room, rng);
            let target = Self::random_pos_in_room(room, rng);
            room.items.push(RoomItem {
                pos,
                kind: RoomItemKind::PuzzleBlock { target },
            });
        }
    }

    fn populate_miniboss(
        room: &mut DungeonRoom,
        floor: u32,
        corruption: u32,
        biome: FloorBiome,
        rng: &mut Rng,
    ) {
        let element = Self::element_for_biome(biome, rng);
        let pos = room.rect.center();
        let base = BaseStats {
            hp: 120.0,
            damage: 25.0,
            defense: 10.0,
            speed: 0.8,
            xp_value: 80,
        };
        let stats = EnemyScaler::scale_enemy(&base, floor, corruption);
        let abilities = EnemyScaler::generate_abilities(floor, rng);
        let boss_names = ["Guardian", "Sentinel", "Revenant", "Behemoth", "Archon"];
        let name = boss_names[rng.range_usize(boss_names.len())].to_string();
        room.enemies.push(EnemySpawn {
            pos,
            stats,
            name,
            element,
            is_elite: true,
            abilities,
        });
    }

    fn populate_boss(
        room: &mut DungeonRoom,
        floor: u32,
        corruption: u32,
        biome: FloorBiome,
        rng: &mut Rng,
    ) {
        let element = Self::element_for_biome(biome, rng);
        let pos = room.rect.center();
        let base = BaseStats {
            hp: 500.0,
            damage: 50.0,
            defense: 25.0,
            speed: 0.6,
            xp_value: 500,
        };
        let stats = EnemyScaler::scale_enemy(&base, floor, corruption);
        let mut abilities = EnemyScaler::generate_abilities(floor, rng);
        // Bosses always have at least 3 abilities
        let extra = ["Phase Shift", "Devastating Slam", "Summon Elites"];
        for a in &extra {
            if abilities.len() < 3 {
                abilities.push(a.to_string());
            }
        }
        let boss_name = if floor == 100 {
            "The Algorithm Reborn".to_string()
        } else {
            let titles = [
                "The Hollow King", "Archlich Verath", "Ironclad Titan",
                "The Void Weaver", "Chaos Incarnate", "The Silent Dread",
            ];
            titles[rng.range_usize(titles.len())].to_string()
        };
        room.enemies.push(EnemySpawn {
            pos,
            stats,
            name: boss_name,
            element,
            is_elite: true,
            abilities,
        });
    }

    fn populate_chaos_rift(
        room: &mut DungeonRoom,
        floor: u32,
        corruption: u32,
        rng: &mut Rng,
    ) {
        // Escalating waves: start with 1 enemy, can scale
        let initial_count = rng.range_i32(1, 3) as usize;
        for _ in 0..initial_count {
            let pos = Self::random_pos_in_room(room, rng);
            let names = ["Rift Spawn", "Chaos Wisp", "Void Tendril"];
            let name = names[rng.range_usize(names.len())];
            room.enemies.push(Self::make_enemy(name, pos, floor, corruption, Some(Element::Chaos), rng));
        }
    }

    fn populate_rest(room: &mut DungeonRoom, rng: &mut Rng) {
        let pos = room.rect.center();
        room.items.push(RoomItem {
            pos,
            kind: RoomItemKind::Campfire,
        });
        // Sometimes a small heal shrine too
        if rng.chance(0.3) {
            let pos2 = Self::random_pos_in_room(room, rng);
            room.items.push(RoomItem {
                pos: pos2,
                kind: RoomItemKind::HealingShrine,
            });
        }
    }

    fn populate_secret(room: &mut DungeonRoom, floor: u32, rng: &mut Rng) {
        // Always rare loot
        let pos = room.rect.center();
        let tier = 3 + floor / 10;
        room.items.push(RoomItem {
            pos,
            kind: RoomItemKind::Chest { trapped: false, loot_tier: tier },
        });
        // Bonus spell scroll
        if rng.chance(0.5) {
            let pos2 = Self::random_pos_in_room(room, rng);
            let spells = ["Meteor", "Time Stop", "Mass Heal", "Void Gate", "Chain Bolt"];
            room.items.push(RoomItem {
                pos: pos2,
                kind: RoomItemKind::SpellScroll {
                    spell_name: spells[rng.range_usize(spells.len())].to_string(),
                },
            });
        }
    }

    fn populate_library(room: &mut DungeonRoom, floor: u32, rng: &mut Rng) {
        let book_count = rng.range_i32(1, 3) as usize;
        for i in 0..book_count {
            let pos = Self::random_pos_in_room(room, rng);
            room.items.push(RoomItem {
                pos,
                kind: RoomItemKind::LoreBook { entry_id: floor * 10 + i as u32 },
            });
        }
        // Spell scroll chance
        if rng.chance(0.4) {
            let pos = Self::random_pos_in_room(room, rng);
            let spells = ["Fireball", "Frost Shield", "Lightning Arc", "Shadow Cloak"];
            room.items.push(RoomItem {
                pos,
                kind: RoomItemKind::SpellScroll {
                    spell_name: spells[rng.range_usize(spells.len())].to_string(),
                },
            });
        }
    }

    fn populate_forge(room: &mut DungeonRoom, rng: &mut Rng) {
        let pos = room.rect.center();
        room.items.push(RoomItem {
            pos,
            kind: RoomItemKind::ForgeAnvil,
        });
    }

    fn populate_normal(
        room: &mut DungeonRoom,
        floor: u32,
        corruption: u32,
        biome: FloorBiome,
        rng: &mut Rng,
    ) {
        // Small chance of a wandering enemy
        if rng.chance(0.3) {
            let pos = Self::random_pos_in_room(room, rng);
            let element = Self::element_for_biome(biome, rng);
            room.enemies.push(Self::make_enemy("Wanderer", pos, floor, corruption, element, rng));
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Floor Generator (BSP-based)
// ══════════════════════════════════════════════════════════════════════════════

/// Generates complete dungeon floors using BSP subdivision.
pub struct FloorGenerator;

impl FloorGenerator {
    /// Generate a complete floor from config and seed.
    pub fn generate(config: &FloorConfig, seed: u64) -> Floor {
        let mut rng = Rng::new(seed ^ (config.floor_number as u64).wrapping_mul(0xCAFEBABE));

        // Determine map dimensions based on floor
        let base_w = 60 + (config.floor_number as usize * 4).min(140);
        let base_h = 40 + (config.floor_number as usize * 3).min(80);
        let map_width = base_w.min(200);
        let map_height = base_h.min(120);

        // Initialize tile grid
        let mut tiles = vec![Tile::Wall; map_width * map_height];

        // BSP split to get room rects
        let min_room = 7;
        let max_depth = 4 + config.floor_number / 15;
        let bsp = BspSplitter::new(min_room, 0.2, max_depth.min(8));
        let graph = bsp.generate(map_width as i32, map_height as i32, &mut rng);

        // Convert procedural rooms to dungeon rooms and carve
        let mut rooms: Vec<DungeonRoom> = Vec::new();
        for (i, proc_room) in graph.rooms.iter().enumerate() {
            // Clamp room size to max 15x15
            let mut rect = proc_room.rect;
            if rect.w > 15 { rect.w = 15; }
            if rect.h > 15 { rect.h = 15; }

            let shape = RoomShape::random(config.corruption_level, &mut rng);
            shape.carve(&rect, &mut tiles, map_width, &mut rng);

            let room_type = Self::assign_room_type(i, graph.rooms.len(), config, &mut rng);
            let mut dungeon_room = DungeonRoom {
                id: i,
                rect,
                room_type,
                shape,
                connections: proc_room.connections.clone(),
                spawn_points: proc_room.spawns.clone(),
                items: Vec::new(),
                enemies: Vec::new(),
                visited: false,
                cleared: false,
            };

            // Populate room contents
            RoomPopulator::populate(
                &mut dungeon_room,
                config.floor_number,
                config.corruption_level,
                config.biome,
                &mut rng,
            );
            rooms.push(dungeon_room);
        }

        // Carve corridors
        let mut corridors: Vec<DungeonCorridor> = Vec::new();
        for proc_corr in &graph.corridors {
            let from_center = if proc_corr.from < rooms.len() {
                rooms[proc_corr.from].rect.center()
            } else {
                IVec2::ZERO
            };
            let to_center = if proc_corr.to < rooms.len() {
                rooms[proc_corr.to].rect.center()
            } else {
                IVec2::ZERO
            };
            let path = config.corridor_style.carve_corridor(
                from_center,
                to_center,
                &mut tiles,
                map_width,
                map_height,
                &mut rng,
            );
            corridors.push(DungeonCorridor {
                from_room: proc_corr.from,
                to_room: proc_corr.to,
                path,
                style: config.corridor_style,
                has_door: proc_corr.has_door,
            });
        }

        // Place doors at corridor-room junctions
        for corr in &corridors {
            if corr.has_door {
                if let Some(first) = corr.path.first() {
                    let idx = first.y as usize * map_width + first.x as usize;
                    if idx < tiles.len() && tiles[idx] == Tile::Corridor {
                        tiles[idx] = Tile::Door;
                    }
                }
                if let Some(last) = corr.path.last() {
                    let idx = last.y as usize * map_width + last.x as usize;
                    if idx < tiles.len() && tiles[idx] == Tile::Corridor {
                        tiles[idx] = Tile::Door;
                    }
                }
            }
        }

        // Place stairs
        let player_start = if !rooms.is_empty() {
            rooms[0].rect.center()
        } else {
            IVec2::new(map_width as i32 / 2, map_height as i32 / 2)
        };
        let exit_point = if rooms.len() > 1 {
            rooms[rooms.len() - 1].rect.center()
        } else {
            player_start
        };

        // Mark stairs tiles
        {
            let idx = player_start.y as usize * map_width + player_start.x as usize;
            if idx < tiles.len() {
                tiles[idx] = Tile::StairsUp;
            }
        }
        {
            let idx = exit_point.y as usize * map_width + exit_point.x as usize;
            if idx < tiles.len() {
                tiles[idx] = Tile::StairsDown;
            }
        }

        // Place special tiles from items
        for room in &rooms {
            for item in &room.items {
                let idx = item.pos.y as usize * map_width + item.pos.x as usize;
                if idx < tiles.len() {
                    match &item.kind {
                        RoomItemKind::Chest { .. } => tiles[idx] = Tile::Chest,
                        RoomItemKind::HealingShrine
                        | RoomItemKind::BuffShrine { .. }
                        | RoomItemKind::RiskShrine => tiles[idx] = Tile::Shrine,
                        RoomItemKind::Merchant { .. } => tiles[idx] = Tile::ShopCounter,
                        _ => {}
                    }
                }
            }
        }

        // Place secret walls for secret rooms
        for room in &rooms {
            if room.room_type == RoomType::Secret {
                // Replace one wall adjacent to the room with a SecretWall
                let candidates = [
                    IVec2::new(room.rect.x - 1, room.rect.y + room.rect.h / 2),
                    IVec2::new(room.rect.x + room.rect.w, room.rect.y + room.rect.h / 2),
                    IVec2::new(room.rect.x + room.rect.w / 2, room.rect.y - 1),
                    IVec2::new(room.rect.x + room.rect.w / 2, room.rect.y + room.rect.h),
                ];
                for c in &candidates {
                    let ci = c.y as usize * map_width + c.x as usize;
                    if ci < tiles.len() && tiles[ci] == Tile::Wall {
                        tiles[ci] = Tile::SecretWall;
                        break;
                    }
                }
            }
        }

        // Add biome hazard tiles
        Self::place_hazard_tiles(config, &mut tiles, map_width, map_height, &mut rng);

        let total_enemies = rooms.iter().map(|r| r.enemies.len() as u32).sum();

        let floor_map = FloorMap {
            width: map_width,
            height: map_height,
            tiles,
            rooms,
            corridors,
            player_start,
            exit_point,
            biome: config.biome,
            floor_number: config.floor_number,
        };

        let fog = FogOfWar::new(map_width, map_height);

        Floor {
            map: floor_map,
            fog,
            seed,
            config: config.clone(),
            enemies_alive: total_enemies,
            total_enemies,
            cleared: total_enemies == 0,
        }
    }

    /// Assign room type: first room is always Rest, last is exit-related,
    /// boss floors get a boss room, and specials from config are placed.
    fn assign_room_type(
        index: usize,
        total: usize,
        config: &FloorConfig,
        rng: &mut Rng,
    ) -> RoomType {
        if index == 0 {
            return RoomType::Rest; // Safe start
        }
        if index == total - 1 && config.boss_floor {
            return RoomType::Boss;
        }
        if index == total - 1 {
            return RoomType::Normal;
        }
        // Place special rooms from config
        let special_index = index.saturating_sub(1);
        if special_index < config.special_rooms.len() {
            return config.special_rooms[special_index].clone();
        }
        // Random assignment
        RoomType::random_for_floor(config.floor_number, rng)
    }

    /// Place hazard tiles (water, lava, ice) based on biome.
    fn place_hazard_tiles(
        config: &FloorConfig,
        tiles: &mut Vec<Tile>,
        width: usize,
        height: usize,
        rng: &mut Rng,
    ) {
        let props = config.biome.properties();
        let hazard_tile = match props.hazard_type {
            HazardType::Fire => Some(Tile::Lava),
            HazardType::Ice => Some(Tile::Ice),
            HazardType::Acid | HazardType::Poison => Some(Tile::Water),
            HazardType::VoidRift | HazardType::Darkness => Some(Tile::Void),
            _ => None,
        };
        if let Some(ht) = hazard_tile {
            let count = rng.range_i32(3, 8 + config.floor_number as i32 / 5) as usize;
            for _ in 0..count {
                let x = rng.range_i32(2, width as i32 - 3);
                let y = rng.range_i32(2, height as i32 - 3);
                let idx = y as usize * width + x as usize;
                if idx < tiles.len() && tiles[idx] == Tile::Floor {
                    tiles[idx] = ht;
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Minimap
// ══════════════════════════════════════════════════════════════════════════════

/// A glyph for the minimap display.
#[derive(Debug, Clone)]
pub struct MinimapGlyph {
    pub x: i32,
    pub y: i32,
    pub ch: char,
    pub color: (u8, u8, u8),
}

/// Renders a compact minimap of the floor.
pub struct Minimap;

impl Minimap {
    /// Render the minimap given the floor map, player position, and fog.
    /// Returns a list of glyphs that represent the minimap at reduced scale.
    pub fn render_minimap(
        floor: &FloorMap,
        player_pos: IVec2,
        fog: &FogOfWar,
    ) -> Vec<MinimapGlyph> {
        let scale = 4; // Each minimap cell = 4x4 tiles
        let mw = (floor.width + scale - 1) / scale;
        let mh = (floor.height + scale - 1) / scale;
        let biome_props = floor.biome.properties();

        let mut glyphs = Vec::with_capacity(mw * mh);

        for my in 0..mh {
            for mx in 0..mw {
                let tx = (mx * scale) as i32;
                let ty = (my * scale) as i32;

                // Sample the dominant tile in this minimap cell
                let mut floor_count = 0u32;
                let mut wall_count = 0u32;
                let mut special = false;
                let mut any_visible = false;

                for dy in 0..scale as i32 {
                    for dx in 0..scale as i32 {
                        let sx = tx + dx;
                        let sy = ty + dy;
                        let vis = fog.get(sx, sy);
                        if vis == Visibility::Unseen {
                            continue;
                        }
                        any_visible = true;
                        let tile = floor.get_tile(sx, sy);
                        match tile {
                            Tile::Floor | Tile::Corridor => floor_count += 1,
                            Tile::Wall | Tile::Void => wall_count += 1,
                            _ => {
                                special = true;
                                floor_count += 1;
                            }
                        }
                    }
                }

                if !any_visible {
                    continue; // Skip unseen areas
                }

                let (ch, color) = if special {
                    ('!', (255, 255, 100))
                } else if floor_count > wall_count {
                    ('.', biome_props.accent_color)
                } else {
                    ('#', (80, 80, 80))
                };

                glyphs.push(MinimapGlyph {
                    x: mx as i32,
                    y: my as i32,
                    ch,
                    color,
                });
            }
        }

        // Player position marker
        let px = player_pos.x as usize / scale;
        let py = player_pos.y as usize / scale;
        glyphs.push(MinimapGlyph {
            x: px as i32,
            y: py as i32,
            ch: '@',
            color: (255, 255, 255),
        });

        // Exit marker (if seen)
        let ex = floor.exit_point.x;
        let ey = floor.exit_point.y;
        if fog.get(ex, ey) != Visibility::Unseen {
            glyphs.push(MinimapGlyph {
                x: ex as i32 / scale as i32,
                y: ey as i32 / scale as i32,
                ch: '>',
                color: (100, 255, 100),
            });
        }

        glyphs
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Dungeon Manager
// ══════════════════════════════════════════════════════════════════════════════

/// Top-level manager for dungeon exploration across multiple floors.
pub struct DungeonManager {
    seed: u64,
    current_floor_number: u32,
    current_floor: Option<Floor>,
    floor_history: HashMap<u32, Floor>,
    max_floor_reached: u32,
}

impl DungeonManager {
    /// Create a new dungeon manager with a given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            current_floor_number: 0,
            current_floor: None,
            floor_history: HashMap::new(),
            max_floor_reached: 0,
        }
    }

    /// Start the dungeon run, generating floor 1.
    pub fn start(&mut self) {
        self.current_floor_number = 1;
        let config = FloorConfig::for_floor(1);
        let floor = FloorGenerator::generate(&config, self.seed);
        self.current_floor = Some(floor);
        self.max_floor_reached = 1;
    }

    /// Descend to the next floor.
    pub fn descend(&mut self) {
        // Store current floor in history
        if let Some(floor) = self.current_floor.take() {
            self.floor_history.insert(self.current_floor_number, floor);
        }
        self.current_floor_number += 1;

        // Check history first (for backtracking)
        if let Some(existing) = self.floor_history.remove(&self.current_floor_number) {
            self.current_floor = Some(existing);
        } else {
            let config = FloorConfig::for_floor(self.current_floor_number);
            let floor_seed = self.seed.wrapping_add(self.current_floor_number as u64 * 0x517cc1b727220a95);
            let floor = FloorGenerator::generate(&config, floor_seed);
            self.current_floor = Some(floor);
        }

        if self.current_floor_number > self.max_floor_reached {
            self.max_floor_reached = self.current_floor_number;
        }
    }

    /// Ascend to the previous floor.
    pub fn ascend(&mut self) {
        if self.current_floor_number <= 1 {
            return;
        }
        if let Some(floor) = self.current_floor.take() {
            self.floor_history.insert(self.current_floor_number, floor);
        }
        self.current_floor_number -= 1;
        if let Some(existing) = self.floor_history.remove(&self.current_floor_number) {
            self.current_floor = Some(existing);
        }
    }

    /// Get a reference to the current floor.
    pub fn current_floor(&self) -> Option<&Floor> {
        self.current_floor.as_ref()
    }

    /// Get a mutable reference to the current floor.
    pub fn current_floor_mut(&mut self) -> Option<&mut Floor> {
        self.current_floor.as_mut()
    }

    /// Get the room at a given position on the current floor.
    pub fn get_room_at(&self, pos: IVec2) -> Option<&DungeonRoom> {
        self.current_floor.as_ref().and_then(|f| f.map.room_at(pos))
    }

    /// Reveal tiles around a position on the current floor.
    pub fn reveal_around(&mut self, pos: IVec2, radius: i32) {
        if let Some(floor) = &mut self.current_floor {
            floor.fog.reveal_around(pos, radius, &floor.map);
        }
    }

    /// Get the current floor number.
    pub fn floor_number(&self) -> u32 {
        self.current_floor_number
    }

    /// Get the maximum floor reached.
    pub fn max_floor(&self) -> u32 {
        self.max_floor_reached
    }

    /// Get the seed for this dungeon run.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// How many floors are stored in history.
    pub fn history_size(&self) -> usize {
        self.floor_history.len()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bsp_generation_produces_rooms() {
        let config = FloorConfig::for_floor(1);
        let floor = FloorGenerator::generate(&config, 42);
        assert!(!floor.map.rooms.is_empty(), "BSP should generate at least one room");
        assert!(floor.map.rooms.len() >= 2, "Floor should have at least 2 rooms");
    }

    #[test]
    fn test_bsp_generation_deterministic() {
        let config = FloorConfig::for_floor(5);
        let floor_a = FloorGenerator::generate(&config, 12345);
        let floor_b = FloorGenerator::generate(&config, 12345);
        assert_eq!(floor_a.map.rooms.len(), floor_b.map.rooms.len());
        assert_eq!(floor_a.map.tiles, floor_b.map.tiles);
    }

    #[test]
    fn test_floor_has_start_and_exit() {
        let config = FloorConfig::for_floor(1);
        let floor = FloorGenerator::generate(&config, 99);
        let start_tile = floor.map.get_tile(floor.map.player_start.x, floor.map.player_start.y);
        let exit_tile = floor.map.get_tile(floor.map.exit_point.x, floor.map.exit_point.y);
        assert_eq!(start_tile, Tile::StairsUp);
        assert_eq!(exit_tile, Tile::StairsDown);
    }

    #[test]
    fn test_room_population_combat() {
        let mut rng = Rng::new(42);
        let rect = IRect::new(5, 5, 10, 10);
        let mut room = DungeonRoom {
            id: 0,
            rect,
            room_type: RoomType::Combat,
            shape: RoomShape::Rectangle,
            connections: vec![],
            spawn_points: vec![],
            items: vec![],
            enemies: vec![],
            visited: false,
            cleared: false,
        };
        RoomPopulator::populate(&mut room, 10, 0, FloorBiome::Ruins, &mut rng);
        assert!(
            room.enemies.len() >= 2 && room.enemies.len() <= 6,
            "Combat room should have 2-6 enemies, got {}",
            room.enemies.len()
        );
    }

    #[test]
    fn test_room_population_shop() {
        let mut rng = Rng::new(42);
        let rect = IRect::new(5, 5, 10, 10);
        let mut room = DungeonRoom {
            id: 0,
            rect,
            room_type: RoomType::Shop,
            shape: RoomShape::Rectangle,
            connections: vec![],
            spawn_points: vec![],
            items: vec![],
            enemies: vec![],
            visited: false,
            cleared: false,
        };
        RoomPopulator::populate(&mut room, 10, 0, FloorBiome::Ruins, &mut rng);
        assert!(!room.items.is_empty(), "Shop room should have merchant");
        let has_merchant = room.items.iter().any(|i| matches!(i.kind, RoomItemKind::Merchant { .. }));
        assert!(has_merchant, "Shop room should contain a Merchant item");
    }

    #[test]
    fn test_enemy_scaling() {
        let base = BaseStats {
            hp: 100.0,
            damage: 20.0,
            defense: 5.0,
            speed: 1.0,
            xp_value: 10,
        };
        let scaled_f1 = EnemyScaler::scale_enemy(&base, 1, 0);
        let scaled_f50 = EnemyScaler::scale_enemy(&base, 50, 0);
        assert!(
            scaled_f50.hp > scaled_f1.hp,
            "Higher floor enemies should have more HP"
        );
        assert!(
            scaled_f50.damage > scaled_f1.damage,
            "Higher floor enemies should do more damage"
        );

        // With corruption
        let scaled_corrupt = EnemyScaler::scale_enemy(&base, 50, 100);
        assert!(
            scaled_corrupt.hp > scaled_f50.hp,
            "Corruption should increase HP"
        );
    }

    #[test]
    fn test_enemy_scaling_formula() {
        let base = BaseStats {
            hp: 100.0,
            damage: 20.0,
            defense: 5.0,
            speed: 1.0,
            xp_value: 10,
        };
        let scaled = EnemyScaler::scale_enemy(&base, 10, 0);
        let expected_hp = 100.0 * (1.0 + 10.0 * 0.08);
        assert!(
            (scaled.hp - expected_hp).abs() < 0.01,
            "HP should be base * (1 + floor * 0.08)"
        );
        let expected_dmg = 20.0 * (1.0 + 10.0 * 0.05);
        assert!(
            (scaled.damage - expected_dmg).abs() < 0.01,
            "Damage should be base * (1 + floor * 0.05)"
        );
    }

    #[test]
    fn test_enemy_abilities_scale_with_floor() {
        assert_eq!(EnemyScaler::ability_count(5), 0);
        assert_eq!(EnemyScaler::ability_count(10), 1);
        assert_eq!(EnemyScaler::ability_count(30), 3);
    }

    #[test]
    fn test_elite_prefix_only_after_floor_50() {
        let mut rng = Rng::new(42);
        assert!(EnemyScaler::elite_prefix(10, &mut rng).is_none());
        assert!(EnemyScaler::elite_prefix(49, &mut rng).is_none());
        // Floor 50+ should return some prefix
        let mut found = false;
        for _ in 0..20 {
            if EnemyScaler::elite_prefix(50, &mut rng).is_some() {
                found = true;
                break;
            }
        }
        assert!(found, "Floor 50+ should eventually produce an elite prefix");
    }

    #[test]
    fn test_fog_of_war_initial_unseen() {
        let fog = FogOfWar::new(10, 10);
        for y in 0..10i32 {
            for x in 0..10i32 {
                assert_eq!(fog.get(x, y), Visibility::Unseen);
            }
        }
    }

    #[test]
    fn test_fog_of_war_reveal() {
        let config = FloorConfig::for_floor(1);
        let floor = FloorGenerator::generate(&config, 42);
        let mut fog = FogOfWar::new(floor.map.width, floor.map.height);
        let center = floor.map.player_start;
        fog.reveal_around(center, 5, &floor.map);
        assert_eq!(fog.get(center.x, center.y), Visibility::Visible);
        assert!(fog.explored_count() > 0);
    }

    #[test]
    fn test_fog_fade_visible_to_seen() {
        let mut fog = FogOfWar::new(5, 5);
        fog.set(2, 2, Visibility::Visible);
        assert_eq!(fog.get(2, 2), Visibility::Visible);
        fog.fade_visible();
        assert_eq!(fog.get(2, 2), Visibility::Seen);
    }

    #[test]
    fn test_tile_properties() {
        assert!(Tile::Floor.is_walkable());
        assert!(!Tile::Wall.is_walkable());
        assert!(Tile::Lava.is_walkable());
        assert!(Tile::Lava.properties().damage_on_step.is_some());
        assert!(Tile::Wall.blocks_sight());
        assert!(!Tile::Floor.blocks_sight());
        assert!(Tile::Door.blocks_sight());
        assert_eq!(Tile::Water.properties().element, Some(Element::Ice));
    }

    #[test]
    fn test_floor_config_auto_generation() {
        let c1 = FloorConfig::for_floor(1);
        assert_eq!(c1.biome, FloorBiome::Ruins);
        assert!(!c1.boss_floor);

        let c10 = FloorConfig::for_floor(10);
        assert!(c10.boss_floor);

        let c100 = FloorConfig::for_floor(100);
        assert!(c100.boss_floor);
        assert_eq!(c100.biome, FloorBiome::Cathedral);
    }

    #[test]
    fn test_floor_theme_ranges() {
        let t5 = FloorTheme::for_floor(5);
        assert_eq!(t5.biome, FloorBiome::Ruins);
        assert!(!t5.traps_enabled);

        let t20 = FloorTheme::for_floor(20);
        assert_eq!(t20.biome, FloorBiome::Crypt);
        assert!(t20.traps_enabled);

        let t40 = FloorTheme::for_floor(40);
        assert_eq!(t40.biome, FloorBiome::Forge);

        let t60 = FloorTheme::for_floor(60);
        assert_eq!(t60.biome, FloorBiome::Void);

        let t80 = FloorTheme::for_floor(80);
        assert_eq!(t80.biome, FloorBiome::Abyss);

        let t100 = FloorTheme::for_floor(100);
        assert_eq!(t100.biome, FloorBiome::Cathedral);
    }

    #[test]
    fn test_biome_properties_populated() {
        for biome in &[
            FloorBiome::Ruins, FloorBiome::Crypt, FloorBiome::Library,
            FloorBiome::Forge, FloorBiome::Garden, FloorBiome::Void,
            FloorBiome::Chaos, FloorBiome::Abyss, FloorBiome::Cathedral,
            FloorBiome::Laboratory,
        ] {
            let props = biome.properties();
            assert!(props.ambient_light >= 0.0 && props.ambient_light <= 1.0);
            assert!(!props.flavor_text.is_empty());
            assert!(!props.music_vibe.is_empty());
        }
    }

    #[test]
    fn test_dungeon_manager_lifecycle() {
        let mut mgr = DungeonManager::new(42);
        assert!(mgr.current_floor().is_none());

        mgr.start();
        assert_eq!(mgr.floor_number(), 1);
        assert!(mgr.current_floor().is_some());

        mgr.descend();
        assert_eq!(mgr.floor_number(), 2);
        assert!(mgr.current_floor().is_some());

        mgr.ascend();
        assert_eq!(mgr.floor_number(), 1);
        assert!(mgr.current_floor().is_some());
    }

    #[test]
    fn test_dungeon_manager_backtrack_preserves_floor() {
        let mut mgr = DungeonManager::new(42);
        mgr.start();

        // Get room count on floor 1
        let f1_rooms = mgr.current_floor().unwrap().map.rooms.len();

        mgr.descend();
        mgr.ascend();

        // Floor 1 should have the same layout
        let f1_rooms_after = mgr.current_floor().unwrap().map.rooms.len();
        assert_eq!(f1_rooms, f1_rooms_after);
    }

    #[test]
    fn test_minimap_render() {
        let config = FloorConfig::for_floor(1);
        let floor = FloorGenerator::generate(&config, 42);
        let mut fog = FogOfWar::new(floor.map.width, floor.map.height);
        fog.reveal_around(floor.map.player_start, 10, &floor.map);
        let glyphs = Minimap::render_minimap(&floor.map, floor.map.player_start, &fog);
        assert!(!glyphs.is_empty(), "Minimap should produce glyphs");
        let has_player = glyphs.iter().any(|g| g.ch == '@');
        assert!(has_player, "Minimap should contain player marker");
    }

    #[test]
    fn test_room_shape_carve() {
        let rect = IRect::new(2, 2, 8, 8);
        let mut tiles = vec![Tile::Wall; 20 * 20];
        let mut rng = Rng::new(42);
        RoomShape::Rectangle.carve(&rect, &mut tiles, 20, &mut rng);
        // Check center of room is floor
        let center_idx = 6 * 20 + 6;
        assert_eq!(tiles[center_idx], Tile::Floor);
    }

    #[test]
    fn test_boss_floor_100_has_algorithm_reborn() {
        let config = FloorConfig::for_floor(100);
        let floor = FloorGenerator::generate(&config, 42);
        let boss_room = floor.map.rooms.iter().find(|r| r.room_type == RoomType::Boss);
        assert!(boss_room.is_some(), "Floor 100 should have a boss room");
        if let Some(br) = boss_room {
            let has_algo = br.enemies.iter().any(|e| e.name == "The Algorithm Reborn");
            assert!(has_algo, "Floor 100 boss should be The Algorithm Reborn");
        }
    }

    #[test]
    fn test_floor_map_walkable_neighbors() {
        let config = FloorConfig::for_floor(1);
        let floor = FloorGenerator::generate(&config, 42);
        // Player start should have at least one walkable neighbor
        let neighbors = floor.map.walkable_neighbors(floor.map.player_start);
        assert!(!neighbors.is_empty(), "Player start should have walkable neighbors");
    }

    #[test]
    fn test_corridor_style_variants() {
        // Just ensure all three styles can be constructed without panic
        let styles = [CorridorStyle::Straight, CorridorStyle::Winding, CorridorStyle::Organic];
        let mut tiles = vec![Tile::Wall; 50 * 50];
        let mut rng = Rng::new(42);
        let from = IVec2::new(5, 5);
        let to = IVec2::new(20, 20);
        for style in &styles {
            let mut t = tiles.clone();
            let path = style.carve_corridor(from, to, &mut t, 50, 50, &mut rng);
            assert!(!path.is_empty(), "Corridor {:?} should produce a path", style);
        }
    }
}
