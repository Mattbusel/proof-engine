#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const MAX_MAP_WIDTH: usize = 4096;
const MAX_MAP_HEIGHT: usize = 4096;
const MAX_LAYERS: usize = 32;
const BLOB_TILE_COUNT: usize = 47;
const A_STAR_MAX_ITERATIONS: usize = 1_000_000;
const FOG_CHUNK_SIZE: usize = 8;
const MINIMAP_MAX_SIZE: usize = 512;
const MAX_UNDO_HISTORY: usize = 256;
const PATROL_MAX_WAYPOINTS: usize = 64;
const ENCOUNTER_MAX_SPAWNS: usize = 32;
const BSP_MIN_ROOM_SIZE: usize = 4;
const BSP_MAX_DEPTH: usize = 8;
const AO_RADIUS: usize = 2;
const LIGHT_FALLOFF_TABLE_SIZE: usize = 256;
const JUMP_POINT_SEARCH_MAX: usize = 512;
const CORRIDOR_WIDTH: usize = 2;

// ============================================================
// TILE DEFINITIONS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileSurface {
    Solid,
    Floor,
    Water,
    Lava,
    Ice,
    Mud,
    Sand,
    Void,
    Bridge,
    Stairs,
}

#[derive(Clone, Debug)]
pub struct TileDefinition {
    pub id: u32,
    pub name: String,
    pub surface: TileSurface,
    pub walkable: bool,
    pub swimmable: bool,
    pub flyable: bool,
    pub solid_collision: bool,
    pub damage_per_second: f32,
    pub friction: f32,
    pub movement_cost: f32,
    pub texture_id: u32,
    pub overlay_texture_id: Option<u32>,
    pub light_blocking: bool,
    pub light_emission: f32,
    pub auto_tile_group: Option<u32>,
    pub height_offset: f32,
    pub destructible: bool,
    pub health: f32,
    pub sound_id: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tile {
    pub tile_id: u32,
    pub variant: u8,
    pub auto_tile_mask: u8,
    pub flags: u16,
    pub ambient_occlusion: u8,
    pub light_level: u8,
    pub entity_id: u32,
    pub height: i16,
}

impl Tile {
    pub fn new(tile_id: u32) -> Self { Self { tile_id, ..Default::default() } }
    pub fn with_id(tile_id: u32) -> Self { Self { tile_id, ..Default::default() } }
    pub fn empty() -> Self { Self::default() }
}

pub struct TileDatabase {
    pub definitions: HashMap<u32, TileDefinition>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerType {
    Background,
    Collision,
    Decoration,
    Entity,
    FogOfWar,
    Lighting,
    Navigation,
    Overlay,
    Ground,
    Tile,
}

#[derive(Clone, Debug)]
pub struct MapLayer {
    pub name: String,
    pub layer_type: LayerType,
    pub tiles: Vec<Tile>,
    pub width: usize,
    pub height: usize,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub z_offset: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MapProjection {
    TopDown,
    Isometric,
    IsometricStaggered,
    Hexagonal,
}

#[derive(Clone, Debug)]
pub struct TileMap {
    pub width: usize,
    pub height: usize,
    pub tile_width: u32,
    pub tile_height: u32,
    pub layers: Vec<MapLayer>,
    pub projection: MapProjection,
    pub background_color: Vec4,
    pub ambient_light: f32,
}

pub const BLOB_MAPPING: [u8; 16] = [
    0, 1, 2, 3, 4, 5, 6, 7,
    8, 9, 10, 11, 12, 13, 14, 15,
];

// 8-bit neighbor mask → 47-tile frame
pub fn blob_8bit_to_47(mask: u8) -> u8 {
    // Canonical 47-tile blob: use lookup table
    // mask bits: NW=0, N=1, NE=2, W=3, E=4, SW=5, S=6, SE=7
    const BLOB47: [u8; 256] = compute_blob47_table();
    BLOB47[mask as usize]
}

const fn compute_blob47_table() -> [u8; 256] {
    let mut table = [0u8; 256];
    let mut i = 0usize;
    while i < 256 {
        let n  = (i >> 1) & 1;
        let e  = (i >> 4) & 1;
        let s  = (i >> 6) & 1;
        let w  = (i >> 3) & 1;
        let nw = (i >> 0) & 1;
        let ne = (i >> 2) & 1;
        let sw = (i >> 5) & 1;
        let se = (i >> 7) & 1;
        // Suppress diagonals if adjacent cardinals absent
        let real_nw = if n == 1 && w == 1 { nw } else { 0 };
        let real_ne = if n == 1 && e == 1 { ne } else { 0 };
        let real_sw = if s == 1 && w == 1 { sw } else { 0 };
        let real_se = if s == 1 && e == 1 { se } else { 0 };
        let canonical = n | (e << 1) | (s << 2) | (w << 3)
            | (real_nw << 4) | (real_ne << 5) | (real_sw << 6) | (real_se << 7);
        table[i] = canonical as u8;
        i += 1;
    }
    table
}

pub struct AutoTiler;

#[derive(Clone, Debug, PartialEq)]
pub enum RoomShape {
    Rectangular,
    LShaped,
    Circular,
    IrregularPolygon,
    Cross,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoomType {
    Normal,
    Boss,
    Treasure,
    Spawn,
    Exit,
    Shop,
    Secret,
    Corridor,
    Safe,
}

#[derive(Clone, Debug)]
pub struct Doorway {
    pub x: usize,
    pub y: usize,
    pub direction: CardinalDir,
    pub width: usize,
    pub is_locked: bool,
    pub key_id: Option<u32>,
    pub connected_room_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CardinalDir {
    North,
    South,
    East,
    West,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub id: u32,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub shape: RoomShape,
    pub room_type: RoomType,
    pub doorways: Vec<Doorway>,
    pub connected_rooms: Vec<u32>,
    pub polygon_points: Vec<(i32, i32)>,
    pub metadata: HashMap<String, String>,
}

pub struct RoomGraph {
    pub nodes: HashMap<u32, Room>,
    pub edges: HashMap<u32, Vec<u32>>,
}

pub struct BspNode {
    pub rect: (usize, usize, usize, usize),  // x, y, w, h
    pub left: Option<Box<BspNode>>,
    pub right: Option<Box<BspNode>>,
    pub room: Option<Room>,
}

pub struct BspDungeonGenerator {
    pub rng: MapRng,
    pub room_padding: usize,
    pub min_room_size: usize,
    pub winding_factor: f32,
}

pub struct MapRng {
    pub state: u64,
}
impl MapRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0x853c49e6748fea9b } }
    pub fn next_u64(&mut self) -> u64 { self.state ^= self.state << 13; self.state ^= self.state >> 7; self.state ^= self.state << 17; self.state }
    pub fn next_f32(&mut self) -> f32 { (self.next_u64() >> 33) as f32 / 2147483648.0 }
    pub fn next_range(&mut self, min: u32, max: u32) -> u32 { if min >= max { return min; } min + (self.next_u64() % (max - min) as u64) as u32 }
}

#[derive(Clone, Debug)]
pub struct SpawnTableEntry {
    pub entity_type_id: u32,
    pub weight: f32,
    pub min_level: u32,
    pub max_level: u32,
    pub group_size_min: u32,
    pub group_size_max: u32,
}

#[derive(Clone, Debug)]
pub struct SpawnTable {
    pub entries: Vec<SpawnTableEntry>,
    pub total_weight: f32,
}

pub struct EncounterZone {
    pub id: u32,
    pub polygon: Vec<(i32, i32)>,
    pub spawn_table: SpawnTable,
    pub min_level: u32,
    pub max_level: u32,
    pub respawn_time_secs: f32,
    pub trigger_conditions: Vec<TriggerCondition>,
    pub is_active: bool,
    pub encounter_name: String,
}

pub enum TriggerCondition {
    OnEnter,
    OnPlayerNearby { radius: f32 },
    OnQuestFlag { flag_id: u32, value: bool },
    OnTime { time_of_day_min: f32, time_of_day_max: f32 },
    OnEnemyCount { count: u32, comparison: Comparison },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Comparison {
    LessThan,
    LessEqual,
    Equal,
    GreaterEqual,
    GreaterThan,
}

// ============================================================
// MINIMAP
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FogState {
    Hidden,
    Explored,
    Visible,
}

#[derive(Clone, Debug)]
pub struct MinimapData {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[u8; 4]>,
    pub fog_states: Vec<FogState>,
    pub scale_x: f32,
    pub scale_y: f32,
}

pub struct MinimapGenerator;

#[derive(Clone, Debug, PartialEq)]
pub struct PathNode {
    pub x: u32,
    pub y: u32,
    pub g: f32,
    pub f: f32,
}

impl Eq for PathNode {}

impl std::cmp::PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.f.partial_cmp(&self.f)
    }
}

pub struct AStarPathfinder;

pub fn octile_heuristic(ax: usize, ay: usize, bx: usize, by: usize) -> f32 {
    let dx = (ax as i32 - bx as i32).unsigned_abs() as f32;
    let dy = (ay as i32 - by as i32).unsigned_abs() as f32;
    let (d_min, d_max) = if dx < dy { (dx, dy) } else { (dy, dx) };
    d_max + (1.414 - 1.0) * d_min
}

// ============================================================
// JUMP POINT SEARCH
// ============================================================

pub struct JumpPointSearch;

pub struct NavRegion {
    pub id: u32,
    pub min_x: usize,
    pub min_y: usize,
    pub max_x: usize,
    pub max_y: usize,
    pub connections: Vec<u32>,
}

pub struct NavMeshGenerator;

pub struct TileLighter;

pub struct PointLight {
    pub x: usize,
    pub y: usize,
    pub intensity: f32,
    pub radius: usize,
    pub color: Vec3,
}

// ============================================================
// MAP EVENTS
// ============================================================

#[derive(Clone, Debug)]
pub struct EventZone {
    pub id: u32,
    pub rect: (usize, usize, usize, usize),
    pub trigger: EventTrigger,
    pub script_id: u32,
    pub one_shot: bool,
    pub triggered: bool,
    pub cooldown_secs: f32,
    pub last_triggered: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventTrigger {
    OnEnter,
    OnExit,
    OnStay { duration_secs: f32 },
    OnInteract,
    OnCombatEnd,
}

#[derive(Clone, Debug)]
pub struct ScriptedEvent {
    pub time: f32,
    pub action: ScriptedAction,
}

#[derive(Clone, Debug)]
pub enum ScriptedAction {
    SpawnEntity { entity_type: u32, x: usize, y: usize },
    PlayAudio { sound_id: u32 },
    ShowSubtitle { text: String, duration: f32 },
    SetTile { layer: String, x: usize, y: usize, tile_id: u32 },
    TriggerAnimation { entity_id: u32, anim_id: u32 },
    ShowCutscene { cutscene_id: u32 },
    GiveItem { entity_id: u32, item_id: u32, count: u32 },
    FadeInOut { duration: f32, color: Vec4 },
}

#[derive(Clone, Debug)]
pub struct ScriptedSequence {
    pub id: u32,
    pub name: String,
    pub events: Vec<ScriptedEvent>,
    pub is_looping: bool,
    pub current_time: f32,
    pub is_running: bool,
}

pub struct PatrolPath {
    pub id: u32,
    pub waypoints: Vec<(usize, usize)>,
    pub current_idx: usize,
    pub loop_type: PatrolLoopType,
    pub direction: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PatrolLoopType {
    Loop,
    PingPong,
    OneShot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrushTool {
    Pencil,
    Fill,
    Line,
    Rectangle,
    Circle,
    FloodFill,
    Erase,
    Select,
    EyeDropper,
}

#[derive(Clone, Debug)]
pub struct BrushState {
    pub tool: BrushTool,
    pub tile_id: u32,
    pub size: usize,
    pub layer_idx: usize,
    pub start_x: Option<usize>,
    pub start_y: Option<usize>,
    pub is_painting: bool,
}

pub struct BrushEngine;

pub fn bresenham_line(x0: i32, y0: i32, x1: i32, y1: i32, out: &mut Vec<(i32, i32)>) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    loop {
        out.push((x, y));
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 > -dy { err -= dy; x += sx; }
        if e2 < dx { err += dx; y += sy; }
    }
}

// ============================================================
// COPY/PASTE
// ============================================================

#[derive(Clone, Debug)]
pub struct TileRegion {
    pub tiles: Vec<Vec<Tile>>,
    pub width: usize,
    pub height: usize,
    pub layer_idx: usize,
}

pub struct Clipboard {
    pub data: Option<TileRegion>,
}

pub enum MapEditAction {
    PaintTile { layer_idx: usize, x: usize, y: usize, old_tile: Tile, new_tile: Tile },
    PaintRegion { layer_idx: usize, x: usize, y: usize, old_tiles: Vec<Vec<Tile>>, new_tiles: Vec<Vec<Tile>> },
    AddLayer { layer_idx: usize, layer: MapLayer },
    RemoveLayer { layer_idx: usize, layer: MapLayer },
    ResizeMap { old_w: usize, old_h: usize, new_w: usize, new_h: usize },
    AddRoom { room: Room },
    RemoveRoom { room_id: u32, room: Room },
}

pub struct MapUndoStack {
    pub history: VecDeque<Vec<MapEditAction>>,
    pub redo_stack: VecDeque<Vec<MapEditAction>>,
    pub max_size: usize,
}

pub struct RoomFiller;

pub enum MapEditorMode {
    Idle,
    PaintingTiles,
    SelectingRegion,
    PlacingRoom,
    PlacingEvent,
    PlacingPatrol,
    PlacingEncounter,
    GeneratingDungeon,
    PaintingLighting,
}

#[derive(Clone, Debug)]
pub struct SelectionRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub is_active: bool,
}

pub struct MapEditor {
    pub map: TileMap,
    pub layers: Vec<MapLayer>,
    pub db: TileDatabase,
    pub undo_stack: MapUndoStack,
    pub brush: BrushState,
    pub clipboard: Clipboard,
    pub selection: SelectionRect,
    pub rooms: RoomGraph,
    pub encounter_zones: Vec<EncounterZone>,
    pub patrol_paths: Vec<PatrolPath>,
    pub event_zones: Vec<EventZone>,
    pub scripted_sequences: Vec<ScriptedSequence>,
    pub lights: Vec<PointLight>,
    pub minimap: MinimapData,
    pub mode: MapEditorMode,
    pub current_layer: usize,
    pub rng: MapRng,
    pub pending_actions: Vec<MapEditAction>,
    pub grid_visible: bool,
    pub show_collision: bool,
    pub show_entities: bool,
    pub show_lighting: bool,
    pub show_patrol_paths: bool,
    pub show_encounter_zones: bool,
    pub show_minimap: bool,
}

pub fn serialize_map(map: &TileMap) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"TILEMAP1");
    let w_bytes = (map.width as u32).to_le_bytes();
    let h_bytes = (map.height as u32).to_le_bytes();
    let tw_bytes = map.tile_width.to_le_bytes();
    let th_bytes = map.tile_height.to_le_bytes();
    bytes.extend_from_slice(&w_bytes);
    bytes.extend_from_slice(&h_bytes);
    bytes.extend_from_slice(&tw_bytes);
    bytes.extend_from_slice(&th_bytes);
    bytes.extend_from_slice(&(map.layers.len() as u32).to_le_bytes());
    for layer in &map.layers {
        let name_bytes = layer.name.as_bytes();
        bytes.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name_bytes);
        bytes.push(layer.layer_type as u8);
        bytes.push(if layer.visible { 1 } else { 0 });
        bytes.push(if layer.locked { 1 } else { 0 });
        bytes.extend_from_slice(&layer.opacity.to_le_bytes());
        let tile_count = layer.tiles.len();
        bytes.extend_from_slice(&(tile_count as u32).to_le_bytes());
        for tile in &layer.tiles {
            bytes.extend_from_slice(&tile.tile_id.to_le_bytes());
            bytes.push(tile.variant);
            bytes.push(tile.auto_tile_mask);
            bytes.push(tile.ambient_occlusion);
            bytes.push(tile.light_level);
            bytes.extend_from_slice(&tile.entity_id.to_le_bytes());
        }
    }
    bytes
}

pub fn deserialize_map(bytes: &[u8]) -> Option<TileMap> {
    if bytes.len() < 8 || &bytes[..8] != b"TILEMAP1" { return None; }
    let mut pos = 8usize;
    let read_u32 = |b: &[u8], p: &mut usize| -> Option<u32> {
        if *p + 4 > b.len() { return None; }
        let v = u32::from_le_bytes(b[*p..*p+4].try_into().ok()?);
        *p += 4;
        Some(v)
    };
    let read_u8 = |b: &[u8], p: &mut usize| -> Option<u8> {
        if *p >= b.len() { return None; }
        let v = b[*p];
        *p += 1;
        Some(v)
    };
    let read_f32 = |b: &[u8], p: &mut usize| -> Option<f32> {
        if *p + 4 > b.len() { return None; }
        let v = f32::from_le_bytes(b[*p..*p+4].try_into().ok()?);
        *p += 4;
        Some(v)
    };
    let w = read_u32(bytes, &mut pos)? as usize;
    let h = read_u32(bytes, &mut pos)? as usize;
    let tw = read_u32(bytes, &mut pos)?;
    let th = read_u32(bytes, &mut pos)?;
    let mut map = TileMap::new(w, h, tw, th);
    map.layers.clear();
    let layer_count = read_u32(bytes, &mut pos)? as usize;
    for _ in 0..layer_count {
        let name_len = read_u32(bytes, &mut pos)? as usize;
        if pos + name_len > bytes.len() { return None; }
        let name = std::str::from_utf8(&bytes[pos..pos+name_len]).ok()?.to_string();
        pos += name_len;
        let lt_byte = read_u8(bytes, &mut pos)?;
        let layer_type = match lt_byte {
            0 => LayerType::Background,
            1 => LayerType::Collision,
            2 => LayerType::Decoration,
            3 => LayerType::Entity,
            4 => LayerType::FogOfWar,
            5 => LayerType::Lighting,
            6 => LayerType::Navigation,
            _ => LayerType::Overlay,
        };
        let visible = read_u8(bytes, &mut pos)? != 0;
        let locked = read_u8(bytes, &mut pos)? != 0;
        let opacity = read_f32(bytes, &mut pos)?;
        let tile_count = read_u32(bytes, &mut pos)? as usize;
        let mut layer = MapLayer::new(&name, layer_type, w, h);
        layer.visible = visible;
        layer.locked = locked;
        layer.opacity = opacity;
        for i in 0..tile_count.min(layer.tiles.len()) {
            let tile_id = read_u32(bytes, &mut pos)?;
            let variant = read_u8(bytes, &mut pos)?;
            let auto_tile_mask = read_u8(bytes, &mut pos)?;
            let ao = read_u8(bytes, &mut pos)?;
            let light = read_u8(bytes, &mut pos)?;
            let entity_id = read_u32(bytes, &mut pos)?;
            layer.tiles[i] = Tile { tile_id, variant, auto_tile_mask, flags: 0, ambient_occlusion: ao, light_level: light, entity_id, height: 0 };
        }
        map.layers.push(layer);
    }
    Some(map)
}

// ============================================================
// MAP STATISTICS
// ============================================================

#[derive(Clone, Debug)]
pub struct MapStats {
    pub total_tiles: usize,
    pub walkable_tiles: usize,
    pub solid_tiles: usize,
    pub water_tiles: usize,
    pub room_count: usize,
    pub connected_rooms: bool,
    pub encounter_zones: usize,
    pub nav_regions: usize,
    pub layer_count: usize,
    pub total_lights: usize,
    pub patrol_paths: usize,
    pub scripted_sequences: usize,
}

pub fn compute_map_stats(editor: &MapEditor) -> MapStats {
    let nav_regions = editor.build_nav_regions();
    MapStats {
        total_tiles: editor.map.width * editor.map.height,
        walkable_tiles: editor.total_walkable_tiles(),
        solid_tiles: editor.total_solid_tiles(),
        water_tiles: {
            let mut cnt = 0;
            for layer in &editor.map.layers {
                for t in &layer.tiles {
                    if editor.db.get(t.tile_id).map(|d| d.surface == TileSurface::Water).unwrap_or(false) {
                        cnt += 1;
                    }
                }
            }
            cnt
        },
        room_count: editor.rooms.nodes.len(),
        connected_rooms: editor.rooms.is_connected(),
        encounter_zones: editor.encounter_zones.len(),
        nav_regions: nav_regions.len(),
        layer_count: editor.map.layers.len(),
        total_lights: editor.lights.len(),
        patrol_paths: editor.patrol_paths.len(),
        scripted_sequences: editor.scripted_sequences.len(),
    }
}

// ============================================================
// TILESET MANAGEMENT
// ============================================================

#[derive(Clone, Debug)]
pub struct Tileset {
    pub id: u32,
    pub name: String,
    pub texture_id: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles_per_row: u32,
    pub total_tiles: u32,
    pub tile_definitions: Vec<TileDefinition>,
}

pub struct TilesetManager {
    pub tilesets: HashMap<u32, Tileset>,
}

pub struct CaveGenerator {
    pub rng: MapRng,
    pub initial_fill_prob: f32,
    pub smoothing_iterations: usize,
    pub birth_threshold: usize,
    pub death_threshold: usize,
}

pub fn generate_noise_map(width: usize, height: usize, freq: f32, seed: u64) -> Vec<f32> {
    let mut rng = MapRng::new(seed);
    let mut noise = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let nx = x as f32 * freq;
            let ny = y as f32 * freq;
            noise[y * width + x] = simple_value_noise_2d(nx, ny);
        }
    }
    noise
}

pub fn simple_value_noise_2d(x: f32, y: f32) -> f32 {
    let ix = x as i32;
    let iy = y as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let n00 = hash_2d(ix, iy);
    let n10 = hash_2d(ix + 1, iy);
    let n01 = hash_2d(ix, iy + 1);
    let n11 = hash_2d(ix + 1, iy + 1);
    let nx0 = n00 + (n10 - n00) * ux;
    let nx1 = n01 + (n11 - n01) * ux;
    nx0 + (nx1 - nx0) * uy
}

pub fn hash_2d(x: i32, y: i32) -> f32 {
    let n = (x.wrapping_mul(1619).wrapping_add(y.wrapping_mul(31337))) as u32;
    let n = n ^ (n >> 16);
    let n = n.wrapping_mul(0x45d9f3b);
    let n = n ^ (n >> 16);
    (n & 0xffffff) as f32 / 0xffffff as f32
}

pub fn apply_heightmap_to_tiles(map: &mut TileMap, heightmap: &[f32], water_threshold: f32, wall_threshold: f32, water_tile: u32, floor_tile: u32, wall_tile: u32) {
    let bg_idx = map.layers.iter().position(|l| l.layer_type == LayerType::Background).unwrap_or(0);
    let col_idx = map.layers.iter().position(|l| l.layer_type == LayerType::Collision).unwrap_or(1);
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = y * map.width + x;
            if idx >= heightmap.len() { continue; }
            let h = heightmap[idx];
            if h < water_threshold {
                map.layers[bg_idx].set(x, y, Tile::with_id(water_tile));
            } else if h > wall_threshold {
                map.layers[col_idx].set(x, y, Tile::with_id(wall_tile));
            } else {
                map.layers[bg_idx].set(x, y, Tile::with_id(floor_tile));
                map.layers[col_idx].set(x, y, Tile::with_id(0));
            }
        }
    }
}

// ============================================================
// MAP TESTS
// ============================================================

pub fn run_all_map_tests() -> bool {
    // Test tilemap creation
    {
        let map = TileMap::new(32, 32, 16, 16);
        assert_eq!(map.width, 32);
        assert_eq!(map.height, 32);
        assert!(!map.layers.is_empty());
    }
    // Test paint and undo
    {
        let mut editor = MapEditor::new(32, 32, 16, 16);
        editor.brush.tile_id = 1;
        editor.start_stroke(5, 5);
        editor.paint_tile(5, 5);
        editor.end_stroke();
        let t = editor.map.layers[0].get(5, 5).copied().unwrap_or(Tile::empty());
        assert_eq!(t.tile_id, 1);
        editor.undo();
        let t2 = editor.map.layers[0].get(5, 5).copied().unwrap_or(Tile::empty());
        assert_eq!(t2.tile_id, 0);
    }
    // Test flood fill
    {
        let mut map = TileMap::new(16, 16, 16, 16);
        for y in 0..16usize { for x in 0..16usize { map.layers[0].set(x, y, Tile::with_id(2)); } }
        BrushEngine::flood_fill(&mut map, 0, 8, 8, 5);
        let t = map.layers[0].get(8, 8).copied().unwrap_or(Tile::empty());
        assert_eq!(t.tile_id, 5);
    }
    // Test A*
    {
        let mut map = TileMap::new(10, 10, 16, 16);
        let db = TileDatabase::new();
        let path = AStarPathfinder::find_path(&map, &db, 0, 0, 9, 9, true, 0);
        assert!(path.is_some());
    }
    // Test BSP
    {
        let mut gen = BspDungeonGenerator::new(42);
        let (rooms, corridors) = gen.generate(64, 64, 4);
        assert!(!rooms.is_empty());
    }
    // Test minimap
    {
        let map = TileMap::new(32, 32, 16, 16);
        let db = TileDatabase::new();
        let mm = MinimapGenerator::generate(&map, &db, 16, 16);
        assert_eq!(mm.width, 16);
        assert_eq!(mm.height, 16);
    }
    // Test serialization round-trip
    {
        let mut editor = MapEditor::new(8, 8, 16, 16);
        editor.map.layers[0].set(3, 3, Tile::with_id(7));
        let bytes = editor.serialize();
        let mut editor2 = MapEditor::new(8, 8, 16, 16);
        let ok = editor2.deserialize_into(&bytes);
        assert!(ok);
        assert_eq!(editor2.map.layers[0].get(3, 3).map(|t| t.tile_id), Some(7));
    }
    // Test patrol path
    {
        let mut path = PatrolPath::new(0);
        path.add_waypoint((0, 0));
        path.add_waypoint((5, 0));
        path.add_waypoint((5, 5));
        path.loop_type = PatrolLoopType::Loop;
        assert_eq!(path.current_target(), Some((0, 0)));
        path.advance();
        assert_eq!(path.current_target(), Some((5, 0)));
        path.advance();
        path.advance();
        assert_eq!(path.current_target(), Some((0, 0))); // looped
    }
    // Test encounter zone containment
    {
        let mut zone = EncounterZone {
            id: 0,
            polygon: vec![(0, 0), (10, 0), (10, 10), (0, 10)],
            spawn_table: SpawnTable::new(),
            min_level: 1,
            max_level: 5,
            respawn_time_secs: 60.0,
            trigger_conditions: Vec::new(),
            is_active: true,
            encounter_name: "test".to_string(),
        };
        assert!(zone.contains_point(5, 5));
        assert!(!zone.contains_point(15, 15));
    }
    true
}

// ============================================================
// TILEMAP QUERY UTILITIES
// ============================================================

pub fn find_tiles_of_type(map: &TileMap, db: &TileDatabase, surface: TileSurface) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    for layer in &map.layers {
        for y in 0..map.height {
            for x in 0..map.width {
                if let Some(t) = layer.get(x, y) {
                    if let Some(def) = db.get(t.tile_id) {
                        if def.surface == surface {
                            results.push((x, y));
                        }
                    }
                }
            }
        }
    }
    results
}

pub fn count_surface(map: &TileMap, db: &TileDatabase, surface: TileSurface) -> usize {
    find_tiles_of_type(map, db, surface).len()
}

pub fn find_room_at_point(rooms: &RoomGraph, x: i32, y: i32) -> Option<&Room> {
    rooms.nodes.values().find(|r| r.contains_point(x, y))
}

pub fn get_rooms_of_type(rooms: &RoomGraph, room_type: RoomType) -> Vec<&Room> {
    rooms.nodes.values().filter(|r| r.room_type == room_type).collect()
}

pub fn map_bounding_box(map: &TileMap) -> (Vec2, Vec2) {
    let min = Vec2::ZERO;
    let max = Vec2::new(
        map.width as f32 * map.tile_width as f32,
        map.height as f32 * map.tile_height as f32,
    );
    (min, max)
}

pub fn spawn_points(map: &TileMap, db: &TileDatabase) -> Vec<(usize, usize)> {
    let mut pts = Vec::new();
    for layer in &map.layers {
        if layer.layer_type != LayerType::Entity { continue; }
        for y in 0..map.height {
            for x in 0..map.width {
                if let Some(t) = layer.get(x, y) {
                    if t.entity_id != 0 { pts.push((x, y)); }
                }
            }
        }
    }
    pts
}

pub fn weighted_random_room_type(rng: &mut MapRng) -> RoomType {
    let roll = rng.next_range(0, 100);
    match roll {
        0..=5 => RoomType::Boss,
        6..=12 => RoomType::Treasure,
        13..=15 => RoomType::Shop,
        16..=17 => RoomType::Secret,
        18..=20 => RoomType::Safe,
        21..=23 => RoomType::Spawn,
        24..=25 => RoomType::Exit,
        _ => RoomType::Normal,
    }
}

pub fn assign_room_types(rooms: &mut RoomGraph, rng: &mut MapRng) {
    let ids: Vec<u32> = rooms.nodes.keys().cloned().collect();
    for id in ids {
        if let Some(room) = rooms.nodes.get_mut(&id) {
            room.room_type = weighted_random_room_type(rng);
        }
    }
}

pub fn build_default_spawn_table() -> SpawnTable {
    let mut table = SpawnTable::new();
    table.add(SpawnTableEntry { entity_type_id: 1, weight: 30.0, min_level: 1, max_level: 5, group_size_min: 1, group_size_max: 3 });
    table.add(SpawnTableEntry { entity_type_id: 2, weight: 20.0, min_level: 2, max_level: 8, group_size_min: 1, group_size_max: 2 });
    table.add(SpawnTableEntry { entity_type_id: 3, weight: 10.0, min_level: 4, max_level: 10, group_size_min: 1, group_size_max: 1 });
    table.add(SpawnTableEntry { entity_type_id: 4, weight: 5.0, min_level: 6, max_level: 15, group_size_min: 1, group_size_max: 1 });
    table
}

// ============================================================
// FULL SAMPLE DUNGEON BUILDER
// ============================================================

pub fn build_sample_dungeon(width: usize, height: usize, seed: u64) -> MapEditor {
    let mut editor = MapEditor::new(width, height, 16, 16);
    editor.generate_dungeon(BSP_MAX_DEPTH, 2, 1);
    assign_room_types(&mut editor.rooms, &mut editor.rng);
    // Place lights in rooms
    let room_ids: Vec<u32> = editor.rooms.nodes.keys().cloned().collect();
    for id in &room_ids {
        if let Some(room) = editor.rooms.nodes.get(id) {
            let (cx, cy) = room.center();
            editor.lights.push(PointLight {
                x: cx, y: cy,
                intensity: 0.8,
                radius: room.width.max(room.height) / 2 + 2,
                color: Vec3::new(1.0, 0.9, 0.7),
            });
        }
    }
    editor.bake_lights();
    // Add encounter zones per room
    let spawn_table = build_default_spawn_table();
    let mut ez_id = 0u32;
    for id in &room_ids {
        if let Some(room) = editor.rooms.nodes.get(id) {
            if room.room_type == RoomType::Normal {
                let poly = vec![
                    (room.x as i32, room.y as i32),
                    ((room.x + room.width) as i32, room.y as i32),
                    ((room.x + room.width) as i32, (room.y + room.height) as i32),
                    (room.x as i32, (room.y + room.height) as i32),
                ];
                let mut table = SpawnTable::new();
                for e in &spawn_table.entries {
                    table.add(e.clone());
                }
                editor.encounter_zones.push(EncounterZone {
                    id: ez_id,
                    polygon: poly,
                    spawn_table: table,
                    min_level: 1,
                    max_level: 10,
                    respawn_time_secs: 120.0,
                    trigger_conditions: vec![TriggerCondition::OnEnter],
                    is_active: true,
                    encounter_name: format!("room_{}_encounter", id),
                });
                ez_id += 1;
            }
        }
    }
    editor.rebuild_minimap();
    editor
}

// ============================================================
// MAP REGION SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RegionType { Town, Dungeon, Wilderness, Special, Tutorial, Boss, Hub }

#[derive(Debug, Clone)]
pub struct MapRegion {
    pub id: u32,
    pub name: String,
    pub region_type: RegionType,
    pub bounds: (i32, i32, i32, i32), // x, y, w, h
    pub connected_regions: Vec<u32>,
    pub level_range: (u32, u32),
    pub background_music: Option<String>,
    pub ambient_sounds: Vec<String>,
    pub weather: String,
    pub discovered: bool,
    pub completion: f32,
}

pub struct WorldMap {
    pub regions: Vec<MapRegion>,
    pub width: u32,
    pub height: u32,
    pub world_name: String,
    pub starting_region: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemRarity { Common, Uncommon, Rare, Epic, Legendary, Artifact, VeryRare }

#[derive(Debug, Clone)]
pub struct TreasureItem {
    pub id: u32,
    pub name: String,
    pub rarity: ItemRarity,
    pub item_type: String,
    pub base_value: u32,
    pub weight: f32,
    pub level_req: u32,
}

impl TreasureItem {
    pub fn new(id: u32, name: &str, rarity: ItemRarity, item_type: &str, base_value: u32) -> Self { Self { id, name: name.to_string(), rarity, item_type: item_type.to_string(), base_value, weight: 1.0, level_req: 1 } }
}

pub struct TreasureChest {
    pub id: u32,
    pub position: (i32, i32),
    pub items: Vec<TreasureItem>,
    pub gold_amount: u32,
    pub chest_type: String,
    pub is_locked: bool,
    pub key_id: Option<u32>,
    pub is_trapped: bool,
    pub trap_damage: u32,
    pub opened: bool,
}

pub enum NpcRole { Merchant, QuestGiver, Guard, Enemy, Ally, Neutral, Boss }

#[derive(Debug, Clone)]
pub struct NpcDialogueNode {
    pub id: u32,
    pub text: String,
    pub speaker: String,
    pub responses: Vec<(String, u32)>,
    pub conditions: Vec<String>,
    pub effects: Vec<String>,
}

pub struct NpcDefinition {
    pub id: u32,
    pub name: String,
    pub role: NpcRole,
    pub position: (i32, i32),
    pub level: u32,
    pub faction: String,
    pub dialogue_root: u32,
    pub dialogue_nodes: HashMap<u32, NpcDialogueNode>,
    pub schedule: Vec<(u32, (i32, i32))>,
    pub inventory: Vec<TreasureItem>,
    pub aggression_range: f32,
    pub is_essential: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestStatus { NotStarted, Active, Completed, Failed, Hidden, Abandoned }

#[derive(Debug, Clone)]
pub enum QuestObjectiveKind {
    KillEnemy { enemy_type: String, count: u32, progress: u32 },
    CollectItem { item_id: u32, count: u32, progress: u32 },
    ReachLocation { region_id: u32, reached: bool },
    TalkToNpc { npc_id: u32, talked: bool },
    ActivateObject { object_id: u32, activated: bool },
    EscortNpc { npc_id: u32, start: (i32,i32), end: (i32,i32), reached: bool },
}

pub struct QuestReward {
    pub experience: u32,
    pub gold: u32,
    pub items: Vec<u32>,
    pub reputation: HashMap<String, i32>,
}

pub struct Quest {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub status: QuestStatus,
    pub objectives: Vec<QuestObjectiveKind>,
    pub reward: QuestReward,
    pub prereq_quests: Vec<u32>,
    pub journal_entries: Vec<(u32, String)>,
    pub is_main_quest: bool,
    pub chapter: u32,
}

pub struct QuestJournal {
    pub quests: HashMap<u32, Quest>,
    pub active_quest_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrapType { SpikePit, SwingingBlade, PressurePlate, MagicGlyph, NetTrap, PoisonDart, BoulderRoll, FireJet }

#[derive(Debug, Clone)]
pub struct TrapDefinition {
    pub id: u32,
    pub trap_type: TrapType,
    pub position: (i32, i32),
    pub damage: u32,
    pub damage_type: String,
    pub triggered: bool,
    pub rearm_time: f32,
    pub time_since_trigger: f32,
    pub visible: bool,
    pub disarm_skill: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DoorState { Open, Closed, Locked, Barred, Broken }

#[derive(Debug, Clone)]
pub struct DoorDefinition {
    pub id: u32,
    pub position: (i32, i32),
    pub facing: u8,
    pub state: DoorState,
    pub key_id: Option<u32>,
    pub lock_difficulty: u32,
    pub hp: i32,
    pub material: String,
    pub auto_close_time: Option<f32>,
    pub is_secret: bool,
}

pub struct DestructibleObject {
    pub id: u32,
    pub position: (i32, i32),
    pub object_type: String,
    pub hp: i32,
    pub max_hp: i32,
    pub destroyed: bool,
    pub blocking: bool,
    pub drops: Vec<TreasureItem>,
    pub destruction_effect: String,
    pub respawn_time: Option<f32>,
}

pub struct WaterBody {
    pub id: u32,
    pub tiles: HashSet<(i32, i32)>,
    pub depth: f32,
    pub current_dir: (f32, f32),
    pub water_type: String,
    pub is_swimmable: bool,
    pub damage_per_turn: u32,
}

pub struct FogOfWar {
    pub width: u32,
    pub height: u32,
    pub revealed: Vec<bool>,
    pub visible: Vec<bool>,
    pub los_radius: u32,
}

pub struct LevelTransition {
    pub id: u32,
    pub position: (i32, i32),
    pub target_map: String,
    pub target_position: (i32, i32),
    pub direction: u8,
    pub requires_item: Option<u32>,
    pub requires_quest: Option<u32>,
    pub transition_type: String,
    pub is_bidirectional: bool,
}

pub struct DungeonTheme {
    pub name: String,
    pub wall_tile_ids: Vec<u32>,
    pub floor_tile_ids: Vec<u32>,
    pub door_tile_id: u32,
    pub chest_frequency: f32,
    pub trap_frequency: f32,
    pub enemy_types: Vec<String>,
    pub ambient_light: (u8, u8, u8),
    pub torch_frequency: f32,
    pub special_tiles: HashMap<String, u32>,
    pub music_track: String,
}

pub struct HeightMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

pub struct OverworldGenerator {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub sea_level: f32,
    pub mountain_threshold: f32,
    pub forest_threshold: f32,
}

pub fn run_map_editor_integration_tests() {
    // World map test
    let mut world = WorldMap::new("Eldoria", 1000, 1000);
    let mut r1 = MapRegion::new(1, "Starter Town", RegionType::Town, (0, 0, 100, 100));
    let mut r2 = MapRegion::new(2, "Dark Forest", RegionType::Wilderness, (100, 0, 200, 200));
    let r3 = MapRegion::new(3, "Ancient Dungeon", RegionType::Dungeon, (300, 0, 100, 100));
    r1.connect(2); r2.connect(1); r2.connect(3);
    world.add_region(r1); world.add_region(r2); world.add_region(r3);
    world.discover_region(1);
    assert!(world.region_by_id(1).unwrap().discovered);
    assert!(!world.region_by_id(2).unwrap().discovered);
    let path = world.find_region_path(1, 3);
    assert!(path.contains(&1) && path.contains(&3));

    // Treasure test
    let mut chest = TreasureChest::new(1, 5, 5);
    chest.add_item(TreasureItem::new(1, "Iron Sword", ItemRarity::Common, "weapon", 50));
    chest.add_item(TreasureItem::new(2, "Magic Staff", ItemRarity::Rare, "weapon", 200));
    chest.gold_amount = 100;
    assert!(chest.total_value() > 0);
    chest.set_trap(15);
    assert!(chest.is_trapped);
    let items = chest.open();
    assert_eq!(items.len(), 2);

    // NPC test
    let mut npc = NpcDefinition::new(1, "Aldric the Merchant", NpcRole::Merchant, (10, 10));
    let mut dialogue = NpcDialogueNode::new(0, "Aldric", "Welcome, traveler! What can I do for you?");
    dialogue.add_response("Buy", 1);
    dialogue.add_response("Sell", 2);
    dialogue.add_response("Goodbye", u32::MAX);
    npc.add_dialogue(dialogue);
    npc.add_to_schedule(8, (10, 10));
    npc.add_to_schedule(18, (15, 15));
    assert_eq!(npc.position_at_hour(12), (10, 10));
    assert_eq!(npc.position_at_hour(20), (15, 15));
    assert!(npc.is_merchant());

    // Quest test
    let mut quest = Quest::new(1, "The Lost Sword", "Find the ancient sword in the dungeon");
    quest.add_objective(QuestObjectiveKind::KillEnemy { enemy_type: "skeleton".into(), count: 5, progress: 5 });
    quest.add_objective(QuestObjectiveKind::ReachLocation { region_id: 3, reached: true });
    quest.start();
    quest.check_completion();
    assert_eq!(quest.status, QuestStatus::Completed);
    assert!((quest.progress_percentage() - 100.0).abs() < 0.1);

    let mut journal = QuestJournal::new();
    journal.add_quest(Quest::new(2, "Main Quest 1", "Begin your journey"));
    journal.quests.get_mut(&2).unwrap().is_main_quest = true;
    journal.start_quest(2);
    assert_eq!(journal.active_quests().len(), 1);

    // Trap test
    let mut trap = TrapDefinition::new(1, TrapType::SwingingBlade, (3, 3));
    let dmg = trap.trigger();
    assert_eq!(dmg, 25);
    assert!(trap.triggered);
    trap.update(4.0);
    assert!(!trap.triggered);

    // Door test
    let mut door = DoorDefinition::new(1, (5, 0), 0);
    door.lock(42, 8);
    assert!(!door.try_open(false, 5));
    assert!(door.try_open(true, 0));

    // Destructible test
    let mut barrel = DestructibleObject::new(1, (7, 7), "barrel", 20);
    barrel.take_damage(15);
    assert!(!barrel.destroyed);
    barrel.take_damage(10);
    assert!(barrel.destroyed);
    assert!(!barrel.blocking);

    // Fog of war test
    let mut fog = FogOfWar::new(64, 64, 8);
    fog.reveal_radius(32, 32);
    assert!(fog.is_revealed(32, 32));
    assert!(fog.exploration_percentage() > 0.0);

    // Level transition test
    let trans = LevelTransition::new(1, (0, 50), "dungeon_01", (1, 1));
    assert!(trans.can_use(|_| true, |_| true));
    assert!(!trans.can_use(|_| false, |_| true)); // won't matter since no req set

    // Height map test
    let mut hm = HeightMap::new(64, 64);
    hm.generate_fbm(4, 12345);
    assert!(*hm.data.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap() >= 0.0);
    hm.smooth(2);
    hm.apply_erosion(2);

    // Overworld generator test
    let gen = OverworldGenerator::new(128, 128, 999);
    let (hm2, tiles) = gen.generate();
    assert_eq!(tiles.len(), 128);
    let towns = gen.place_towns(&hm2, 5);
    assert!(!towns.is_empty());
    let river = gen.generate_river(&hm2, towns[0]);
    assert!(!river.is_empty());

    // Dungeon theme test
    let theme = DungeonTheme::crypt();
    let mut rng = MapRng::new(777);
    let wall = theme.random_wall_tile(&mut rng);
    assert!(theme.wall_tile_ids.contains(&wall));

    // Water body test
    let mut lake = WaterBody::new(1, "lake");
    for x in 0..10 { for y in 0..10 { lake.add_tile(x, y); } }
    assert_eq!(lake.area(), 100);
    assert!(lake.contains(5, 5));
    assert!(!lake.contains(15, 15));
}

// ============================================================
// TILE ANIMATION SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct TileAnimation {
    pub tile_id: u32,
    pub frames: Vec<u32>,
    pub frame_durations_ms: Vec<u32>,
    pub current_frame: usize,
    pub elapsed_ms: u32,
    pub loop_animation: bool,
}

pub struct TileAnimationManager {
    pub animations: HashMap<u32, TileAnimation>,
}

pub struct TileObject {
    pub id: u32,
    pub object_type: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub rotation: u8,
    pub flip_x: bool,
    pub flip_y: bool,
    pub layer: u32,
    pub properties: HashMap<String, String>,
    pub collidable: bool,
    pub sprite_id: u32,
}

pub struct ObjectLayer {
    pub id: u32,
    pub name: String,
    pub objects: Vec<TileObject>,
    pub visible: bool,
    pub locked: bool,
}

pub struct TmxExporter { pub version: String }

pub struct CollisionMap {
    pub width: u32, pub height: u32, pub cells: Vec<u8>,
}

pub struct SpawnPoint {
    pub id: u32, pub position: (i32, i32), pub spawn_type: String,
    pub max_concurrent: u32, pub current_count: u32, pub enabled: bool, pub respawn_delay_secs: f32,
}

pub struct SpawnManager {
    pub spawn_points: Vec<SpawnPoint>,
    pub global_spawn_enabled: bool,
}

pub struct MapChunk {
    pub chunk_x: i32, pub chunk_y: i32, pub tile_data: Vec<Vec<u32>>,
    pub is_loaded: bool, pub last_access_frame: u64, pub dirty: bool,
}

pub struct ChunkStreamingManager {
    pub loaded_chunks: HashMap<(i32, i32), MapChunk>,
    pub stream_radius: i32,
    pub current_frame: u64,
}

pub fn run_map_editor_extended_tests() {
    // Tile animation
    let mut anim = TileAnimation::new(5, vec![5, 6, 7, 8], 100);
    assert_eq!(anim.current_tile_id(), 5);
    anim.update(150); assert_eq!(anim.current_tile_id(), 6);
    anim.reset(); assert_eq!(anim.current_tile_id(), 5);
    assert_eq!(anim.total_duration_ms(), 400);

    let mut amgr = TileAnimationManager::new();
    amgr.register(TileAnimationManager::standard_water_animation());
    amgr.update_all(175);
    let _ = amgr.resolve_tile(5);

    // Object layer
    let mut ol = ObjectLayer::new(1, "objects");
    let mut t1 = TileObject::new(1, "tree", (5, 5));
    t1.set_property("shade", "large");
    let t2 = TileObject::new(2, "rock", (5, 5));
    ol.add_object(t1); ol.add_object(t2);
    assert!(ol.object_at(5, 5).is_some());
    assert!(!ol.find_overlaps().is_empty());
    assert_eq!(ol.objects[0].get_property("shade"), Some("large"));

    // TMX export
    let tmx = TmxExporter::new();
    let m = TileMap::new(8, 8, 16, 16);
    let xml = tmx.export_map(&m, "test");
    assert!(xml.contains("<?xml"));

    // Collision map
    let mut coll = CollisionMap::new(16, 16);
    coll.set(3, 3, CollisionMap::SOLID);
    assert!(coll.is_solid(3, 3));
    assert!(coll.is_passable(4, 4));
    let nbrs = coll.passable_neighbors(4, 4);
    assert!(nbrs.len() >= 3);

    // Spawn manager
    let mut sm = SpawnManager::new();
    let mut sp = SpawnPoint::new(1, (5, 5), "goblin");
    sp.max_concurrent = 2;
    sm.add_spawn_point(sp);
    let av = sm.available_spawns_for_type("goblin");
    assert_eq!(av.len(), 1);
    assert_eq!(sm.total_capacity(), 2);

    // Chunk streaming
    let mut csm = ChunkStreamingManager::new(2);
    csm.ensure_loaded(0, 0);
    assert!(csm.loaded_chunk_count() > 0);
    csm.current_frame = 1000;
    csm.evict_distant_chunks(1, 0);
    // Only chunks with recent access remain
    let _ = csm.get_tile_at(5, 5);

    // World map and quests
    let mut world = WorldMap::new("Veldoria", 200, 200);
    let mut r1 = MapRegion::new(10, "Hometown", RegionType::Town, (0,0,20,20));
    let mut r2 = MapRegion::new(11, "Wilderness", RegionType::Wilderness, (20,0,40,40));
    r1.connect(11); r2.connect(10);
    world.add_region(r1); world.add_region(r2);
    world.discover_region(10);
    assert_eq!(world.discovered_regions().len(), 1);
    let path = world.find_region_path(10, 11);
    assert!(path.len() >= 2);

    // Treasure
    let mut chest = TreasureChest::new(99, 3, 3);
    chest.add_item(TreasureItem::new(1, "Sword", ItemRarity::Rare, "weapon", 100));
    chest.gold_amount = 50;
    assert!(chest.total_value() > 0);
    let items = chest.open();
    assert_eq!(items.len(), 1);
    assert!(chest.opened);

    // Door
    let mut door = DoorDefinition::new(1, (0, 5), 0);
    door.lock(7, 10);
    assert!(!door.try_open(false, 5));
    assert!(door.try_open(false, 10));
    assert_eq!(door.state, DoorState::Open);

    // Trap
    let mut trap = TrapDefinition::new(1, TrapType::PoisonDart, (1, 1));
    let dmg = trap.trigger();
    assert!(dmg > 0);
    trap.update(10.0);
    assert!(!trap.triggered);

    // Fog of war
    let mut fog = FogOfWar::new(32, 32, 6);
    fog.reveal_radius(16, 16);
    assert!(fog.exploration_percentage() > 0.0);
    fog.clear_visible();
    assert!(fog.is_revealed(16, 16));
    assert!(!fog.is_visible(16, 16));

    // Level transition
    let lt = LevelTransition::new(1, (0, 10), "next_level", (1, 1));
    assert!(lt.can_use(|_| true, |_| true));

    // Height map full test
    let mut hm = HeightMap::new(32, 32);
    hm.generate_fbm(5, 99);
    let min_h = hm.data.iter().cloned().fold(f32::MAX, f32::min);
    let max_h = hm.data.iter().cloned().fold(f32::MIN, f32::max);
    assert!(min_h >= 0.0 && max_h <= 1.0);
    hm.smooth(2); hm.apply_erosion(3);
    let db = TileDatabase::new();
    let tiles = hm.to_terrain_tiles(&db, 0.3, 0.75);
    assert_eq!(tiles.len(), 32);

    // Overworld
    let gen = OverworldGenerator::new(64, 64, 555);
    let (hm2, _tiles) = gen.generate();
    let towns = gen.place_towns(&hm2, 3);
    assert!(!towns.is_empty());
    if !towns.is_empty() {
        let river = gen.generate_river(&hm2, towns[0]);
        assert!(!river.is_empty());
    }
}

// ============================================================
// MAP DIALOGUE TRIGGER SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerCondition2 { PlayerEnter, PlayerExit, Interact, TimePassed(f32), ItemPickedUp(u32), QuestStage(u32, u32) }

#[derive(Debug, Clone)]
pub struct MapTrigger {
    pub id: u32,
    pub region: (i32, i32, i32, i32),
    pub condition: TriggerCondition2,
    pub fired: bool,
    pub one_shot: bool,
    pub cooldown: f32,
    pub elapsed_cooldown: f32,
    pub script: String,
}

pub struct TriggerManager2 {
    pub triggers: Vec<MapTrigger>,
}

pub struct WarpPoint {
    pub id: u32,
    pub name: String,
    pub position: (i32, i32),
    pub target_map: String,
    pub target_position: (i32, i32),
    pub unlock_condition: Option<String>,
    pub is_unlocked: bool,
    pub warp_type: String,
    pub effect_name: String,
}

pub struct WarpNetwork {
    pub warp_points: Vec<WarpPoint>,
}

pub struct SoundZone {
    pub id: u32,
    pub polygon: Vec<(i32, i32)>,
    pub ambient_track: String,
    pub volume: f32,
    pub loop_audio: bool,
    pub fade_in_secs: f32,
    pub fade_out_secs: f32,
    pub reverb_preset: String,
    pub priority: i32,
}

pub struct SoundZoneManager {
    pub zones: Vec<SoundZone>,
    pub current_zone_id: Option<u32>,
    pub transition_progress: f32,
}

pub struct AoeZone {
    pub id: u32,
    pub center: (i32, i32),
    pub radius: f32,
    pub effect_type: String,
    pub damage_per_second: f32,
    pub duration_remaining: f32,
    pub team_mask: u32,
    pub visual_effect: String,
}

pub struct AoeManager {
    pub zones: Vec<AoeZone>,
}

pub struct StaticLight {
    pub id: u32,
    pub position: (i32, i32),
    pub color: (u8, u8, u8),
    pub radius: f32,
    pub intensity: f32,
    pub cast_shadows: bool,
    pub flicker: bool,
    pub flicker_speed: f32,
    pub flicker_time: f32,
}

pub struct DynamicLightMap {
    pub width: u32,
    pub height: u32,
    pub light_values: Vec<f32>,
    pub ambient_level: f32,
}

pub struct MapEditorStats {
    pub total_tiles_placed: u64,
    pub total_tiles_erased: u64,
    pub undo_operations: u32,
    pub redo_operations: u32,
    pub layers_created: u32,
    pub objects_placed: u32,
    pub rooms_generated: u32,
    pub maps_saved: u32,
    pub maps_loaded: u32,
    pub session_start: String,
    pub editing_time_secs: f64,
}

pub trait MapEditorPlugin: std::fmt::Debug {
    fn name(&self) -> &str;
    fn on_tile_placed(&mut self, x: i32, y: i32, tile_id: u32);
    fn on_selection_changed(&mut self, tiles: &[(i32, i32)]);
    fn on_layer_changed(&mut self, layer_name: &str);
}

#[derive(Debug)]
pub struct AutoTilePlugin { pub name: String, pub last_updated: u32 }
#[derive(Debug)]
pub struct CollisionPlugin { pub name: String, pub auto_build: bool }
pub struct MapExportConfig {
    pub format: String,
    pub include_collision: bool,
    pub include_nav_mesh: bool,
    pub include_spawns: bool,
    pub include_metadata: bool,
    pub compress: bool,
    pub output_path: String,
    pub atlas_size: (u32, u32),
}

pub struct MapExporter {
    pub config: MapExportConfig,
    pub bytes_written: usize,
}

pub fn run_map_comprehensive_tests() {
    // Trigger system
    let mut tm = TriggerManager2::new();
    tm.add(MapTrigger::on_enter(1, 5, 5, 10, 10, "start_dialogue_01"));
    tm.add(MapTrigger::on_enter(2, 20, 20, 5, 5, "boss_cutscene"));
    let scripts = tm.check_enter(8, 8);
    assert!(!scripts.is_empty());
    assert!(scripts[0].contains("dialogue"));
    let scripts2 = tm.check_enter(8, 8);
    assert!(scripts2.is_empty()); // one_shot
    assert_eq!(tm.fired_count(), 1);

    // Warp network
    let mut wn = WarpNetwork::new();
    wn.add(WarpPoint::new(1, "Town Warp", (10, 10), "town_map", (5, 5)));
    wn.add(WarpPoint::new(2, "Dungeon Warp", (50, 50), "dungeon_01", (1, 1)));
    wn.warp_points[1].is_unlocked = false;
    assert_eq!(wn.unlocked_count(), 1);
    let nearest = wn.nearest_unlocked((12, 12));
    assert!(nearest.is_some());
    assert_eq!(nearest.unwrap().name, "Town Warp");
    wn.unlock_all();
    assert_eq!(wn.unlocked_count(), 2);

    // Sound zone
    let mut szm = SoundZoneManager::new();
    let cave_zone = SoundZone::new(1, vec![(0,0),(20,0),(20,20),(0,20)], "cave_ambience");
    szm.add_zone(cave_zone);
    szm.update_player_position(10, 10, 1.0);
    assert_eq!(szm.current_zone_id, Some(1));
    assert!(szm.current_volume() > 0.0);
    szm.update_player_position(50, 50, 0.1);
    assert_eq!(szm.current_zone_id, None);

    // AoE manager
    let mut aoe = AoeManager::new();
    aoe.add_zone(AoeZone::fire_pillar(1, 10, 10, 5.0, 3.0));
    aoe.add_zone(AoeZone::frost_zone(2, 10, 10, 3.0, 2.0));
    assert_eq!(aoe.zones_at(10, 10).len(), 2);
    let dmg = aoe.total_damage_at(10, 10, 0.1);
    assert!(dmg > 0.0);
    aoe.update(2.5);
    assert_eq!(aoe.zones.len(), 1); // frost expired
    aoe.update(1.0);
    assert!(aoe.zones.is_empty());

    // Static lights + light map
    let lights = vec![StaticLight::torch(1, 10, 10), StaticLight::lamp(2, 20, 20), StaticLight::torch(3, 5, 5)];
    let mut light_map = DynamicLightMap::new(32, 32, 0.05);
    light_map.bake_static_lights(&lights);
    let il = light_map.get(10, 10);
    assert!(il > 0.05);
    let avg = light_map.average_illuminance();
    assert!(avg > 0.05);
    let dark_count = light_map.dark_tile_count(0.1);
    assert!(dark_count > 0);

    // Stats tracker
    let mut stats = MapEditorStats::new();
    stats.record_tile_place(150);
    stats.record_tile_erase(30);
    stats.advance_time(60.0);
    assert_eq!(stats.net_tiles(), 120);
    assert!(stats.tiles_per_minute() > 0.0);
    let summary = stats.summary();
    assert!(summary.contains("150"));

    // Map exporter
    let config = MapExportConfig::default_config("level_01.map");
    let mut exporter = MapExporter::new(config);
    let test_map = TileMap::new(32, 32, 16, 16);
    let header = exporter.export_header(&test_map);
    assert!(header.starts_with(b"MAPF"));
    assert_eq!(header.len(), 20);
    let json = exporter.export_map_json(&test_map, "test_level");
    assert!(json.contains("test_level"));
    assert!(json.contains("32"));
    let binary = exporter.export_map_binary(&TileMap::new(8, 8, 16, 16));
    assert!(!binary.is_empty());

    // Plugin system
    let mut atp = AutoTilePlugin::new();
    atp.on_tile_placed(5, 5, 42);
    assert_eq!(atp.last_updated, 1);
    assert_eq!(atp.name(), "AutoTile");
    let cp = CollisionPlugin::new();
    assert_eq!(cp.name(), "CollisionBuilder");
    assert!(cp.auto_build);

    // Full dungeon generation + stats tracking
    let dungeon_editor = build_sample_dungeon(64, 64, 12345);
    assert!(!dungeon_editor.map.layers.is_empty());
    stats.rooms_generated += dungeon_editor.encounter_zones.len() as u32;
    stats.record_tile_place(dungeon_editor.map.layers[0].tiles.len() as u64);

    // Dynamic light map with flickering torch
    let mut torch = StaticLight::torch(99, 15, 15);
    for _ in 0..60 { torch.update(1.0/60.0); }
    let intensity = torch.current_intensity();
    assert!(intensity > 0.5 && intensity < 2.0);

    // Fog complete workflow
    let mut fog = FogOfWar::new(64, 64, 8);
    let waypoints = vec![(8,8), (16,16), (32,32), (48,48), (56,56)];
    for &(wx, wy) in &waypoints { fog.reveal_radius(wx, wy); }
    let explored_pct = fog.exploration_percentage();
    assert!(explored_pct > 20.0);

    // Overworld with roads connecting towns
    let gen = OverworldGenerator::new(256, 256, 42);
    let (hm, _) = gen.generate();
    let towns = gen.place_towns(&hm, 8);
    assert!(!towns.is_empty());
    let mut rivers = Vec::new();
    for &town in &towns { rivers.push(gen.generate_river(&hm, town)); }
    assert_eq!(rivers.len(), towns.len());
    // All rivers should start at town positions
    for (i, river) in rivers.iter().enumerate() { assert!(!river.is_empty(), "River {} is empty", i); }
}

// ============================================================
// SECTION: Dungeon Room Placement System
// ============================================================

#[derive(Debug, Clone)]
pub struct DungeonRoom {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub room_type: DungeonRoomType,
    pub is_cleared: bool,
    pub connections: Vec<u32>,
    pub loot_level: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DungeonRoomType {
    Entrance, Exit, Combat, Treasure, BossRoom, Corridor, Shop, Rest, Puzzle, Secret,
}

impl DungeonRoom {
    pub fn area(&self) -> i32 { self.width * self.height }
}

pub struct DungeonGenerator {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub min_room_size: i32,
    pub max_room_size: i32,
    pub max_rooms: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TilePos { pub x: i32, pub y: i32 }

pub struct PathfindingGrid {
    pub width: i32,
    pub height: i32,
    pub passable: Vec<bool>,
    pub tile_cost: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapEventType {
    PlayerEnter, PlayerExit, MobSpawn, MobKill, ChestOpen, TrapTriggered,
    LevelTransition, QuestStart, QuestComplete, BossDefeated, SecretFound,
}

#[derive(Debug, Clone)]
pub struct MapEvent {
    pub id: u32,
    pub event_type: MapEventType,
    pub position: (i32, i32),
    pub data: HashMap<String, String>,
    pub fired: bool,
    pub cooldown_s: f32,
    pub last_fire_time: f32,
}

pub struct MapEventSystem {
    pub events: Vec<MapEvent>,
    pub current_time: f32,
}

#[derive(Debug, Clone)]
pub struct MinimapCell {
    pub explored: bool,
    pub visible: bool,
    pub tile_type: u8,
    pub has_entity: bool,
}

pub struct Minimap {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<MinimapCell>,
    pub scale: u32, // minimap pixels per world tile
}

#[derive(Clone, Debug)]
pub enum PatrolState { Idle, Walking, Alert, Searching, Returning }

#[derive(Debug, Clone)]
pub struct PatrolWaypoint {
    pub position: (i32, i32),
    pub wait_time_s: f32,
    pub animation_hint: String,
}

pub struct PatrolAi {
    pub npc_id: u32,
    pub waypoints: Vec<PatrolWaypoint>,
    pub current_waypoint: usize,
    pub state: PatrolState,
    pub position: (f32, f32),
    pub move_speed: f32,
    pub alert_radius: f32,
    pub search_timer: f32,
    pub wait_timer: f32,
    pub home_position: (f32, f32),
}

pub struct MapAnnotation {
    pub id: u32,
    pub tile_pos: (i32, i32),
    pub text: String,
    pub annotation_type: String,
    pub color: (u8, u8, u8),
    pub visible: bool,
    pub created_time: f32,
}

pub struct MapAnnotationLayer {
    pub annotations: Vec<MapAnnotation>,
    pub visible: bool,
}

pub fn run_dungeon_generator_tests() {
    let gen = DungeonGenerator::new(100, 100, 42);
    let rooms = gen.generate();
    assert!(!rooms.is_empty());
    assert_eq!(rooms[0].room_type, DungeonRoomType::Entrance);
    // At least some rooms should be connected
    let connected = rooms.iter().filter(|r| !r.connections.is_empty() || r.room_type == DungeonRoomType::Entrance).count();
    assert!(connected > 0);
    // Boss room should exist in larger dungeons
    let has_boss = rooms.iter().any(|r| r.room_type == DungeonRoomType::BossRoom);
    if rooms.len() >= 4 { assert!(has_boss); }
    let treasure = rooms.iter().find(|r| r.room_type == DungeonRoomType::Treasure);
    assert!(rooms.len() < 3 || treasure.is_some());
    // All rooms should have non-zero area
    for room in &rooms { assert!(room.area() > 0); }
}

pub fn run_pathfinding_tests() {
    let mut grid = PathfindingGrid::new(20, 20);
    // Block a wall
    for y in 5..15 { grid.set_passable(10, y, false); }
    let path = grid.find_path_astar(TilePos::new(5, 10), TilePos::new(15, 10), false);
    assert!(path.is_some());
    let p = path.unwrap();
    assert!(!p.is_empty());
    assert_eq!(*p.first().unwrap(), TilePos::new(5, 10));
    assert_eq!(*p.last().unwrap(), TilePos::new(15, 10));
    // Verify path avoids blocked tiles
    for pos in &p { assert!(grid.is_passable(pos)); }

    // Octile distance
    let a = TilePos::new(0, 0);
    let b = TilePos::new(3, 4);
    assert!(a.octile_distance(&b) > 0.0);
    assert!(a.manhattan_distance(&b) == 7);
}

pub fn run_map_event_tests() {
    let mut sys = MapEventSystem::new();
    sys.add_event(MapEvent::new(1, MapEventType::PlayerEnter, 5, 5).with_data("zone", "town"));
    sys.add_event(MapEvent::new(2, MapEventType::TrapTriggered, 5, 5).with_cooldown(10.0));
    sys.add_event(MapEvent::new(3, MapEventType::ChestOpen, 8, 3));

    let events_at_55 = sys.events_at(5, 5);
    assert_eq!(events_at_55.len(), 2);

    let fired = sys.trigger_at(5, 5);
    assert_eq!(fired.len(), 2);

    let trap_events = sys.events_of_type(&MapEventType::TrapTriggered);
    assert_eq!(trap_events.len(), 1);

    sys.reset_all();
    assert!(sys.events[0].can_fire(0.0));
}

pub fn run_minimap_tests() {
    let mut minimap = Minimap::new(64, 64, 2);
    assert_eq!(minimap.exploration_percent(), 0.0);
    minimap.reveal_radius(32, 32, 8);
    assert!(minimap.exploration_percent() > 0.0);
    assert!(minimap.is_explored(32, 32));
    assert!(!minimap.is_explored(0, 0));
    let (mx, my) = minimap.world_to_minimap(10.5, 20.3);
    assert!(mx < minimap.width && my < minimap.height);
}

pub fn run_patrol_ai_tests() {
    let mut ai = PatrolAi::new(1, (0.0, 0.0), 3.0);
    ai.add_waypoint(PatrolWaypoint::new(10, 0).with_wait(1.0));
    ai.add_waypoint(PatrolWaypoint::new(10, 10));
    ai.add_waypoint(PatrolWaypoint::new(0, 10));
    assert_eq!(ai.state, PatrolState::Idle);
    ai.update(0.016);
    assert_eq!(ai.state, PatrolState::Walking);
    ai.alert();
    assert_eq!(ai.state, PatrolState::Alert);
    ai.update(0.016);
    assert_eq!(ai.state, PatrolState::Searching);
    // After search timer expires
    ai.update(6.0);
    assert_eq!(ai.state, PatrolState::Returning);
}

pub fn run_annotation_tests() {
    let mut layer = MapAnnotationLayer::new();
    layer.add(MapAnnotation::new(1, 5, 5, "Town entrance"));
    layer.add(MapAnnotation::warning(2, 10, 10, "Danger zone"));
    layer.add(MapAnnotation::secret(3, 20, 20, "Hidden passage"));
    assert_eq!(layer.visible_count(), 3);
    let at_55 = layer.at_tile(5, 5);
    assert_eq!(at_55.len(), 1);
    let search = layer.search("Hidden");
    assert_eq!(search.len(), 1);
    let notes = layer.export_notes();
    assert!(notes.contains("Town entrance"));
    layer.remove(1);
    assert_eq!(layer.annotations.len(), 2);
}

pub fn run_all_map_editor_tests_extended() {
    run_dungeon_generator_tests();
    run_pathfinding_tests();
    run_map_event_tests();
    run_minimap_tests();
    run_patrol_ai_tests();
    run_annotation_tests();
    run_map_editor_integration_tests();
    run_map_editor_extended_tests();
}

// ============================================================
// SECTION: Map Tile Autotiling
// ============================================================

/// Wang tile autotiling — 16-tile blob tileset index
pub fn wang_blob_index(n: bool, e: bool, s: bool, w: bool, ne: bool, se: bool, sw: bool, nw: bool) -> u8 {
    let mut idx = 0u8;
    if n { idx |= 1; }
    if e { idx |= 2; }
    if s { idx |= 4; }
    if w { idx |= 8; }
    // Corners only count if both adjacent cardinals are also solid
    if nw && n && w { idx |= 16; }
    if ne && n && e { idx |= 32; }
    if se && s && e { idx |= 64; }
    if sw && s && w { idx |= 128; }
    idx
}

/// Returns a descriptive name for a Wang blob tile index
pub fn wang_blob_name(idx: u8) -> &'static str {
    match idx {
        0 => "isolated", 2 => "edge_east", 8 => "edge_west",
        10 => "corridor_horizontal", 1 => "edge_north", 4 => "edge_south",
        5 => "corridor_vertical", 15 => "cross", 255 => "full_interior",
        _ => "complex",
    }
}

#[derive(Debug, Clone)]
pub struct AutotileResult {
    pub x: i32,
    pub y: i32,
    pub tile_index: u8,
}

#[derive(Debug, Clone)]
pub struct AutotileLayer {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<u8>,  // 0=empty, 1=solid
    pub result: Vec<AutotileResult>,
}

pub struct LootEntry {
    pub item_id: u32,
    pub item_name: String,
    pub weight: u32,
    pub min_quantity: u32,
    pub max_quantity: u32,
    pub min_level: u32,
    pub condition: Option<String>,
}

pub struct LootTable {
    pub id: u32,
    pub name: String,
    pub entries: Vec<LootEntry>,
    pub guaranteed_entries: Vec<LootEntry>,
    pub rolls_min: u32,
    pub rolls_max: u32,
}

pub struct LootTableRegistry {
    pub tables: HashMap<u32, LootTable>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TileEffect {
    None, Slippery, Slow, Damage, Heal, Teleport, Invisible, Burning, Poisoned,
}

#[derive(Debug, Clone)]
pub struct TileEffectMap {
    pub width: i32,
    pub height: i32,
    pub effects: Vec<TileEffect>,
    pub effect_params: Vec<f32>, // damage amount, heal amount, etc.
}

pub enum MapLayerType { Tile, Object, Image, Group }

pub struct MapLayerStack {
    pub layers: Vec<MapLayer>,
    pub width: i32,
    pub height: i32,
}

pub struct MapProgressionData {
    pub map_id: u32,
    pub total_secrets: u32,
    pub found_secrets: u32,
    pub total_enemies: u32,
    pub killed_enemies: u32,
    pub total_chests: u32,
    pub opened_chests: u32,
    pub boss_defeated: bool,
    pub time_spent_s: f32,
    pub completion_flags: HashSet<String>,
}

pub fn run_autotile_tests() {
    let mut layer = AutotileLayer::new(20, 20);
    layer.fill_rect(5, 5, 10, 10);
    assert!(layer.solid_count() > 0);
    layer.compute_autotile();
    assert!(!layer.result.is_empty());
    // Center tile should be full interior (255)
    let center = layer.result.iter().find(|r| r.x == 7 && r.y == 7);
    assert!(center.is_some());
    assert_eq!(center.unwrap().tile_index, 255);
    // Edge tile (top-left corner)
    let edge = layer.result.iter().find(|r| r.x == 5 && r.y == 5);
    assert!(edge.is_some());
    assert!(wang_blob_name(0).len() > 0);
}

pub fn run_loot_table_tests() {
    let mut registry = LootTableRegistry::new();
    registry.register(LootTable::goblin_treasure());
    registry.register(LootTable::boss_hoard());
    assert_eq!(registry.table_count(), 2);

    let loot = registry.roll_table(1, 12345, 5);
    assert!(!loot.is_empty()); // guaranteed drop
    let boss_loot = registry.roll_table(2, 99999, 10);
    assert!(!boss_loot.is_empty());
    // Boss table weight
    let boss = LootTable::boss_hoard();
    assert!(boss.total_weight() > 0.0);
}

pub fn run_tile_effect_tests() {
    let mut emap = TileEffectMap::new(20, 20);
    emap.set_effect(5, 5, TileEffect::Damage, 10.0);
    emap.set_effect(6, 6, TileEffect::Heal, 5.0);
    emap.set_effect(7, 7, TileEffect::Slow, 0.5);

    let mut hp = 100.0f32;
    let mut speed = 1.0f32;
    emap.apply_to_entity(5, 5, &mut hp, &mut speed);
    assert_eq!(hp, 90.0);

    emap.apply_to_entity(6, 6, &mut hp, &mut speed);
    assert_eq!(hp, 95.0);

    emap.apply_to_entity(7, 7, &mut hp, &mut speed);
    assert!((speed - 0.5).abs() < 0.001);

    assert_eq!(emap.effect_at(5, 5), &TileEffect::Damage);
    assert_eq!(emap.effect_at(0, 0), &TileEffect::None);
    let counts = emap.effects_count_by_type();
    assert!(counts.contains_key("Damage"));
}

pub fn run_layer_stack_tests() {
    let mut stack = MapLayerStack::standard_rpg_layers(32, 32);
    assert_eq!(stack.layer_count(), 6);
    assert_eq!(stack.visible_layer_count(), 6);

    let terrain = stack.layer_by_name_mut("terrain").unwrap();
    terrain.fill(1);
    terrain.set_tile(5, 5, 42);

    let composited = stack.composite_tile_at(5, 5);
    // Objects layer is above terrain, check terrain tile if no object
    assert!(composited > 0);

    let layer = stack.layer_by_name("terrain").unwrap();
    assert_eq!(layer.tile_count(42), 1);
    assert!(layer.non_empty_count() > 0);
}

pub fn run_progression_tests() {
    let mut prog = MapProgressionData::new(1);
    prog.total_enemies = 20; prog.total_chests = 5; prog.total_secrets = 3;
    prog.killed_enemies = 20; prog.opened_chests = 5; prog.found_secrets = 3;
    prog.boss_defeated = true;
    assert!(prog.is_100_percent());
    assert_eq!(prog.grade(), 'S');
    prog.set_flag("found_shortcut");
    assert!(prog.has_flag("found_shortcut"));

    let mut partial = MapProgressionData::new(2);
    partial.total_enemies = 10; partial.killed_enemies = 5;
    partial.total_chests = 4; partial.opened_chests = 2;
    assert!(partial.completion_percent() < 100.0);
    assert!(partial.completion_percent() > 0.0);
}

pub fn map_editor_run_all() {
    run_all_map_editor_tests_extended();
    run_autotile_tests();
    run_loot_table_tests();
    run_tile_effect_tests();
    run_layer_stack_tests();
    run_progression_tests();
}

// ============================================================
// SECTION: Module Constants
// ============================================================

pub const MAP_EDITOR_MODULE_VERSION: &str = "3.0.0";
pub const MAX_DUNGEON_ROOMS: u32 = 100;
pub const MAX_DUNGEON_ROOM_SIZE: i32 = 32;
pub const MIN_DUNGEON_ROOM_SIZE: i32 = 4;
pub const DEFAULT_TILE_SIZE_PX: u32 = 16;
pub const MAX_MAP_LAYERS: usize = 32;
pub const MAX_ANNOTATION_TEXT_LEN: usize = 256;
pub const MAX_PATROL_WAYPOINTS: usize = 64;
pub const NPC_ALERT_RADIUS_DEFAULT: f32 = 5.0;
pub const NPC_SEARCH_DURATION_S: f32 = 5.0;
pub const LOOT_TABLE_MAX_ENTRIES: usize = 128;
pub const LOOT_TABLE_MAX_ROLLS: u32 = 10;
pub const PATHFINDING_MAX_ITERATIONS: u32 = 100_000;
pub const MINIMAP_DEFAULT_SCALE: u32 = 2;
pub const FOG_OF_WAR_DEFAULT_RADIUS: i32 = 8;
pub const MAP_EVENT_MAX_COOLDOWN_S: f32 = 3600.0;
pub const AUTOTILE_WANG_BLOB_VARIANTS: u32 = 256;
pub const MAP_EXPORT_MAGIC: u32 = 0x4D415045; // "MAPE"
pub const MAP_EXPORT_VERSION: u32 = 3;

pub fn map_editor_module_info() -> HashMap<String, String> {
    let mut info = HashMap::new();
    info.insert("module".into(), "map_editor".into());
    info.insert("version".into(), MAP_EDITOR_MODULE_VERSION.into());
    info.insert("max_map_size".into(), format!("{}x{}", MAX_MAP_WIDTH, MAX_MAP_HEIGHT));
    info.insert("max_dungeon_rooms".into(), format!("{}", MAX_DUNGEON_ROOMS));
    info.insert("default_tile_size".into(), format!("{}px", DEFAULT_TILE_SIZE_PX));
    info.insert("max_layers".into(), format!("{}", MAX_MAP_LAYERS));
    info.insert("export_magic".into(), format!("{:#010X}", MAP_EXPORT_MAGIC));
    info.insert("pathfinding".into(), "A* with octile heuristic".into());
    info.insert("dungeon_gen".into(), "BSP + Prim MST".into());
    info.insert("autotile".into(), "Wang blob 256-variant".into());
    info
}

// ============================================================
// SECTION: Procedural Dungeon Content Placement
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ContentType { Enemy, Chest, Trap, Decoration, Light, Exit, Spawn, Boss }

#[derive(Debug, Clone)]
pub struct DungeonContent {
    pub position: (i32, i32),
    pub content_type: ContentType,
    pub enemies: Vec<(i32, i32, String)>,
    pub chests: Vec<(i32, i32, u32)>,
    pub traps: Vec<(i32, i32, String)>,
    pub data: HashMap<String, String>,
}

pub struct DungeonContentPlacer {
    pub rng: MapRng,
    pub enemy_density: f32,
    pub chest_density: f32,
    pub trap_density: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WeatherType { Clear, Cloudy, Rain, HeavyRain, Thunderstorm, Snow, Fog, Blizzard, Sandstorm, Hail, Overcast, LightRain, LightSnow, HeavySnow, HeavyFog, Sleet }

#[derive(Debug, Clone)]
pub struct MapWeatherState {
    pub weather_type: String,
    pub intensity: f32,
    pub wind_speed_ms: f32,
    pub wind_direction_deg: f32,
    pub temperature_c: f32,
    pub visibility_m: f32,
    pub precipitation_mm_hr: f32,
}

pub struct WeatherTransition {
    pub from: WeatherType,
    pub to: WeatherType,
    pub duration_s: f32,
    pub elapsed_s: f32,
}

pub struct MapSaveState {
    pub map_id: u32,
    pub player_x: f32,
    pub player_y: f32,
    pub visited_chunks: HashSet<(i32, i32)>,
    pub cleared_rooms: HashSet<u32>,
    pub opened_chests: HashSet<u32>,
    pub killed_enemies: HashSet<u64>,
    pub world_time: f64,
    pub save_version: u32,
}

pub fn run_dungeon_content_tests() {
    let gen = DungeonGenerator::new(80, 80, 1337);
    let rooms = gen.generate();
    assert!(!rooms.is_empty());
    let mut placer = DungeonContentPlacer::new(12345);
    let content = placer.populate_dungeon(&rooms);
    assert!(!content.is_empty());
    // Should have a spawn point
    let has_spawn = content.iter().any(|c| c.content_type == ContentType::Spawn);
    assert!(has_spawn || rooms[0].room_type == DungeonRoomType::Entrance);
    // Boss room if enough rooms
    if rooms.len() >= 4 {
        let has_boss = content.iter().any(|c| c.content_type == ContentType::Boss);
        assert!(has_boss);
    }
}

pub fn run_weather_tests() {
    let clear = MapWeatherState::clear();
    assert_eq!(clear.movement_speed_modifier(), 1.0);
    assert!(!clear.is_dangerous());
    assert!((clear.ambient_light_factor() - 1.0).abs() < 0.001);

    let storm = MapWeatherState::thunderstorm();
    assert!(storm.is_dangerous());
    assert!(storm.movement_speed_modifier() < 1.0);
    assert!(storm.ambient_light_factor() < 1.0);

    let rain = MapWeatherState::rain(0.8);
    assert!(rain.precipitation_mm_hr > 0.0);
    assert!(rain.visibility_m < 10_000.0);

    let snow = MapWeatherState::snow(0.5);
    assert!(snow.temperature_c < 0.0);
    assert!(snow.movement_speed_modifier() < 1.0);

    let mut transition = WeatherTransition::new(WeatherType::Clear, WeatherType::Rain, 60.0);
    transition.update(30.0);
    assert!((transition.progress() - 0.5).abs() < 0.001);
    assert!(!transition.is_complete());
    transition.update(30.0);
    assert!(transition.is_complete());
}

pub fn run_save_state_tests() {
    let mut state = MapSaveState::new(42);
    state.mark_chunk_visited(0, 0); state.mark_chunk_visited(1, 0); state.mark_chunk_visited(0, 1);
    assert_eq!(state.visited_chunk_count(), 3);
    assert!(state.is_chunk_visited(1, 0));
    assert!(!state.is_chunk_visited(5, 5));

    state.mark_room_cleared(1);
    state.mark_chest_opened(101);
    state.mark_enemy_killed(999_000_001);
    assert!(state.is_room_cleared(1));
    assert!(state.is_chest_opened(101));
    assert!(state.is_enemy_killed(999_000_001));

    state.advance_time(3600.0);
    assert!(state.world_time > 0.0);
    assert!(state.time_of_day_fraction() >= 0.0 && state.time_of_day_fraction() < 1.0);

    let header = state.serialize_header();
    assert_eq!(&header[0..4], b"MSAV");
    assert_eq!(header.len(), 20); // 4+4+4+4+4
}

pub fn map_editor_complete_suite() {
    map_editor_run_all();
    run_dungeon_content_tests();
    run_weather_tests();
    run_save_state_tests();
    let info = map_editor_module_info();
    assert!(info.contains_key("version"));
    assert!(info.contains_key("pathfinding"));
}

// ============================================================
// SECTION: Map Editor Selection & Clipboard
// ============================================================

#[derive(Debug, Clone)]
pub struct TileRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug)]
pub struct TileClipboard {
    pub tiles: Vec<Vec<u32>>,  // [row][col]
    pub width: usize,
    pub height: usize,
}

pub struct MapEditorSelection {
    pub selection: Option<TileRect>,
    pub clipboard: TileClipboard,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintTool { Pencil, Eraser, Fill, BoxFill, Line, Circle }

#[derive(Debug, Clone)]
pub struct TilePainter {
    pub active_tool: PaintTool,
    pub active_tile_id: u32,
    pub brush_size: u32,
    pub layer_name: String,
    pub paint_history: Vec<(i32, i32, u32, u32)>, // (x, y, old_tile, new_tile)
}

#[derive(Debug, Clone)]
pub struct TileChange { pub x: i32, pub y: i32, pub layer: String, pub before: u32, pub after: u32 }

#[derive(Debug, Clone)]
pub struct MapUndoAction {
    pub description: String,
    pub changes: Vec<TileChange>,
}

pub struct MapUndoHistory {
    pub actions: VecDeque<MapUndoAction>,
    pub redo_stack: Vec<MapUndoAction>,
    pub max_history: usize,
}

impl MapLayer {
    pub fn set_tile(&mut self, x: usize, y: usize, tile_id: u32) {
        if x < self.width && y < self.height { self.tiles[y * self.width + x].tile_id = tile_id; }
    }
    pub fn tile_at(&self, x: usize, y: usize) -> u32 {
        self.get(x, y).map(|t| t.tile_id).unwrap_or(0)
    }
    pub fn new(name: impl Into<String>, layer_type: LayerType, width: usize, height: usize) -> Self {
        Self { name: name.into(), layer_type, tiles: vec![Tile::default(); width * height], width, height, visible: true, locked: false, opacity: 1.0, z_offset: 0.0 }
    }
    pub fn set(&mut self, x: usize, y: usize, tile: Tile) {
        if x < self.width && y < self.height { self.tiles[y * self.width + x] = tile; }
    }
    pub fn get(&self, x: usize, y: usize) -> Option<&Tile> {
        if x < self.width && y < self.height { Some(&self.tiles[y * self.width + x]) } else { None }
    }
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, tile: Tile) { for ty in y..y+h { for tx in x..x+w { self.set(tx, ty, tile.clone()); } } }
    pub fn fill(&mut self, tile_id: u32) { for t in &mut self.tiles { t.tile_id = tile_id; } }
    pub fn fill_with(&mut self, tile_id: u32) { for t in &mut self.tiles { t.tile_id = tile_id; } }
    pub fn tile_count(&self, tile_id: u32) -> usize { self.tiles.iter().filter(|t| t.tile_id == tile_id).count() }
    pub fn non_empty_count(&self) -> usize { self.tiles.iter().filter(|t| t.tile_id != 0).count() }
}

impl TileRect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self { Self { x, y, width: w, height: h } }
    pub fn contains(&self, x: i32, y: i32) -> bool { x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height }
    pub fn intersects(&self, other: &TileRect) -> bool { self.x < other.x + other.width && self.x + self.width > other.x && self.y < other.y + other.height && self.y + self.height > other.y }
    pub fn area(&self) -> i32 { self.width * self.height }
}

impl TileClipboard {
    pub fn flip_horizontal(&mut self) { for row in &mut self.tiles { row.reverse(); } }
    pub fn flip_vertical(&mut self) { self.tiles.reverse(); }
}

impl MapEditorSelection {
    pub fn new() -> Self { Self { selection: None, clipboard: TileClipboard { tiles: Vec::new(), width: 0, height: 0 }, is_active: false } }
    pub fn select(&mut self, rect: TileRect) { self.selection = Some(rect); self.is_active = true; }
    pub fn selection_area(&self) -> i32 { self.selection.as_ref().map(|r| r.width * r.height).unwrap_or(0) }
    pub fn has_clipboard(&self) -> bool { !self.clipboard.tiles.is_empty() }
    pub fn copy(&mut self, layer: &MapLayer) {
        if let Some(rect) = &self.selection {
            let (w, h, sx, sy) = (rect.width as usize, rect.height as usize, rect.x as usize, rect.y as usize);
            self.clipboard = TileClipboard { width: w, height: h, tiles: (0..h).map(|dy| (0..w).map(|dx| layer.tile_at(sx+dx, sy+dy)).collect()).collect() };
        }
    }
    pub fn paste(&self, layer: &mut MapLayer, tx: usize, ty: usize) {
        for (dy, row) in self.clipboard.tiles.iter().enumerate() {
            for (dx, &tile_id) in row.iter().enumerate() { layer.set_tile(tx+dx, ty+dy, tile_id); }
        }
    }
}

impl TilePainter {
    pub fn new() -> Self { Self { active_tool: PaintTool::Pencil, active_tile_id: 0, brush_size: 1, layer_name: String::new(), paint_history: Vec::new() } }
    pub fn paint_at(&mut self, layer: &mut MapLayer, x: usize, y: usize) {
        let old = layer.tile_at(x, y);
        layer.set_tile(x, y, self.active_tile_id);
        self.paint_history.push((x as i32, y as i32, old, self.active_tile_id));
    }
    pub fn paint_line(&mut self, layer: &mut MapLayer, x0: usize, y0: usize, x1: usize, y1: usize) {
        let steps = ((x1 as i32 - x0 as i32).abs().max((y1 as i32 - y0 as i32).abs())) as usize + 1;
        for i in 0..=steps { let t = if steps == 0 { 0.0 } else { i as f32 / steps as f32 }; let x = (x0 as f32 + t * (x1 as f32 - x0 as f32)) as usize; let y = (y0 as f32 + t * (y1 as f32 - y0 as f32)) as usize; self.paint_at(layer, x, y); }
    }
    pub fn paint_box(&mut self, layer: &mut MapLayer, rect: &TileRect) {
        for dy in 0..rect.height as usize { for dx in 0..rect.width as usize { self.paint_at(layer, rect.x as usize+dx, rect.y as usize+dy); } }
    }
    pub fn history_count(&self) -> usize { self.paint_history.len() }
    pub fn clear_history(&mut self) { self.paint_history.clear(); }
}

pub fn run_clipboard_tests() {
    let mut layer = MapLayer::new("test", LayerType::Background, 20, 20);
    for y in 0..5 { for x in 0..5 { layer.set_tile(x, y, (y * 5 + x + 1) as u32); } }

    let mut sel = MapEditorSelection::new();
    sel.select(TileRect::new(0, 0, 5, 5));
    assert_eq!(sel.selection_area(), 25);
    sel.copy(&layer);
    assert!(sel.has_clipboard());

    let mut target = MapLayer::new("target", LayerType::Background, 20, 20);
    sel.paste(&mut target, 5, 5);
    assert_eq!(target.tile_at(5, 5), layer.tile_at(0, 0));

    // Flip
    let mut cb = sel.clipboard.clone();
    let original_00 = cb.tiles[0][0];
    cb.flip_horizontal();
    assert_ne!(cb.tiles[0][0], original_00);

    let rect = TileRect::new(2, 2, 4, 4);
    assert!(rect.contains(3, 3));
    assert!(!rect.contains(0, 0));
    let r2 = TileRect::new(4, 4, 4, 4);
    assert!(rect.intersects(&r2));
}

pub fn run_painter_tests() {
    let mut layer = MapLayer::new("paint", LayerType::Background, 20, 20);
    let mut painter = TilePainter::new();
    painter.active_tile_id = 5;
    painter.paint_at(&mut layer, 5, 5);
    assert_eq!(layer.tile_at(5, 5), 5);
    painter.paint_line(&mut layer, 0, 0, 10, 10);
    assert_eq!(layer.tile_at(0, 0), 5);
    assert_eq!(layer.tile_at(10, 10), 5);
    painter.paint_box(&mut layer, &TileRect::new(1, 1, 3, 3));
    assert_eq!(layer.tile_at(1, 1), 5);
    assert!(painter.history_count() > 0);
    painter.clear_history();
    assert_eq!(painter.history_count(), 0);
}

pub fn run_undo_history_tests() {
    let mut history = MapUndoHistory::new(50);
    assert!(!history.can_undo());
    let mut action = MapUndoAction::new("paint tiles");
    action.add_change(5, 5, "terrain", 0, 1);
    action.add_change(6, 5, "terrain", 0, 1);
    history.push(action);
    assert!(history.can_undo());
    let undone = history.undo();
    assert!(undone.is_some());
    assert!(history.can_redo());
    let redone = history.redo();
    assert!(redone.is_some());
    assert!(!history.can_redo());
}

pub fn map_editor_final() {
    map_editor_complete_suite();
    run_clipboard_tests();
    run_painter_tests();
    run_undo_history_tests();
    let info = map_editor_module_info();
    assert_eq!(info["module"], "map_editor");
}

// ============================================================
// SECTION: Tile Variant & Biome System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BiomeType {
    Grassland, Forest, Desert, Tundra, Swamp, Volcanic, Ocean, Mountains,
    Jungle, Savanna, Taiga, Cave, Underground, Ruins, Magical,
}

#[derive(Debug, Clone)]
pub struct BiomeDefinition {
    pub biome_type: BiomeType,
    pub name: String,
    pub base_floor_tile: u32,
    pub base_wall_tile: u32,
    pub base_water_tile: u32,
    pub deco_tiles: Vec<u32>,
    pub enemy_table_id: u32,
    pub ambient_color: [u8; 3],
    pub fog_density: f32,
    pub temperature_c: f32,
    pub precipitation_mm: f32,
}

impl BiomeDefinition {
    pub fn grassland() -> Self {
        BiomeDefinition { biome_type: BiomeType::Grassland, name: "Grassland".into(),
            base_floor_tile: 1, base_wall_tile: 10, base_water_tile: 20,
            deco_tiles: vec![30, 31, 32, 33], enemy_table_id: 1,
            ambient_color: [180, 220, 140], fog_density: 0.05, temperature_c: 18.0, precipitation_mm: 600.0 }
    }

    pub fn desert() -> Self {
        BiomeDefinition { biome_type: BiomeType::Desert, name: "Desert".into(),
            base_floor_tile: 5, base_wall_tile: 15, base_water_tile: 20,
            deco_tiles: vec![40, 41, 42], enemy_table_id: 2,
            ambient_color: [240, 210, 150], fog_density: 0.02, temperature_c: 38.0, precipitation_mm: 50.0 }
    }

    pub fn cave() -> Self {
        BiomeDefinition { biome_type: BiomeType::Cave, name: "Cave".into(),
            base_floor_tile: 8, base_wall_tile: 18, base_water_tile: 21,
            deco_tiles: vec![50, 51, 52, 53, 54], enemy_table_id: 3,
            ambient_color: [80, 80, 100], fog_density: 0.2, temperature_c: 10.0, precipitation_mm: 0.0 }
    }

    pub fn is_cold(&self) -> bool { self.temperature_c < 0.0 }
    pub fn is_hot(&self) -> bool { self.temperature_c > 30.0 }
    pub fn is_wet(&self) -> bool { self.precipitation_mm > 1000.0 }
    pub fn is_dark(&self) -> bool { self.fog_density > 0.15 }

    pub fn movement_modifier(&self) -> f32 {
        match self.biome_type {
            BiomeType::Swamp => 0.6,
            BiomeType::Desert => 0.85,
            BiomeType::Mountains => 0.7,
            BiomeType::Forest => 0.9,
            _ => 1.0,
        }
    }

    pub fn survival_difficulty(&self) -> u8 {
        match self.biome_type {
            BiomeType::Volcanic | BiomeType::Swamp => 5,
            BiomeType::Desert | BiomeType::Tundra => 4,
            BiomeType::Mountains | BiomeType::Cave => 3,
            BiomeType::Forest | BiomeType::Jungle => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BiomeMap {
    pub width: u32,
    pub height: u32,
    pub biome_ids: Vec<u8>,
    pub biome_registry: Vec<BiomeDefinition>,
}

impl BiomeMap {
    pub fn new(width: u32, height: u32) -> Self {
        BiomeMap { width, height, biome_ids: vec![0; (width * height) as usize], biome_registry: Vec::new() }
    }

    pub fn register_biome(&mut self, biome: BiomeDefinition) { self.biome_registry.push(biome); }

    fn idx(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height { Some((y * self.width + x) as usize) } else { None }
    }

    pub fn set_biome(&mut self, x: u32, y: u32, biome_id: u8) {
        if let Some(i) = self.idx(x, y) { self.biome_ids[i] = biome_id; }
    }

    pub fn biome_at(&self, x: u32, y: u32) -> Option<&BiomeDefinition> {
        let id = self.idx(x, y).map(|i| self.biome_ids[i] as usize).unwrap_or(0);
        self.biome_registry.get(id)
    }

    pub fn biome_coverage(&self, biome_id: u8) -> f32 {
        let count = self.biome_ids.iter().filter(|&&b| b == biome_id).count();
        count as f32 / self.biome_ids.len() as f32 * 100.0
    }
}

// ============================================================
// SECTION: Encounter Zone System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum EncounterTriggerType { OnEnter, OnStep, OnTime, OnLowHealth, OnCombatEnd }

#[derive(Debug, Clone)]
pub struct EncounterGroup {
    pub group_id: u32,
    pub enemy_ids: Vec<u32>,
    pub level_range: (u32, u32),
    pub formation_radius: f32,
    pub is_elite: bool,
    pub loot_table_id: u32,
}

impl EncounterGroup {
    pub fn new(id: u32, enemies: Vec<u32>, level_min: u32, level_max: u32) -> Self {
        EncounterGroup { group_id: id, enemy_ids: enemies, level_range: (level_min, level_max),
            formation_radius: 2.0, is_elite: false, loot_table_id: 1 }
    }

    pub fn enemy_count(&self) -> usize { self.enemy_ids.len() }
    pub fn is_boss_group(&self) -> bool { self.enemy_count() == 1 && self.is_elite }
    pub fn xp_reward(&self) -> u32 { self.enemy_ids.len() as u32 * (self.level_range.1 + self.level_range.0) * 25 }
}

#[derive(Debug, Clone)]
pub struct EncounterZoneRecord {
    pub zone_id: u32,
    pub bounds: (i32, i32, i32, i32), // x1, y1, x2, y2
    pub trigger_type: EncounterTriggerType,
    pub encounter_groups: Vec<EncounterGroup>,
    pub respawn_time_s: f32,
    pub last_cleared_time: f32,
    pub difficulty_scale: f32,
}

impl EncounterZoneRecord {
    pub fn new(id: u32, x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        EncounterZoneRecord { zone_id: id, bounds: (x1, y1, x2, y2),
            trigger_type: EncounterTriggerType::OnEnter,
            encounter_groups: Vec::new(), respawn_time_s: 300.0,
            last_cleared_time: -999.0, difficulty_scale: 1.0 }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.bounds.0 && y >= self.bounds.1 && x <= self.bounds.2 && y <= self.bounds.3
    }

    pub fn add_group(&mut self, group: EncounterGroup) { self.encounter_groups.push(group); }

    pub fn can_respawn(&self, current_time: f32) -> bool {
        current_time - self.last_cleared_time >= self.respawn_time_s
    }

    pub fn total_enemy_count(&self) -> usize { self.encounter_groups.iter().map(|g| g.enemy_count()).sum() }

    pub fn total_xp_reward(&self) -> u32 {
        (self.encounter_groups.iter().map(|g| g.xp_reward()).sum::<u32>() as f32 * self.difficulty_scale) as u32
    }
}

// ============================================================
// SECTION: Map Region Connection & Graph
// ============================================================

#[derive(Debug, Clone)]
pub struct MapRegionRecord {
    pub region_id: u32,
    pub name: String,
    pub bounds: (i32, i32, i32, i32),
    pub region_type: BiomeType,
    pub connections: Vec<u32>,
    pub level_range: (u32, u32),
    pub is_unlocked: bool,
    pub discovery_flags: HashSet<String>,
}

impl MapRegionRecord {
    pub fn new(id: u32, name: &str, bounds: (i32, i32, i32, i32), region_type: BiomeType) -> Self {
        MapRegionRecord { region_id: id, name: name.to_string(), bounds, region_type,
            connections: Vec::new(), level_range: (1, 10), is_unlocked: false, discovery_flags: HashSet::new() }
    }

    pub fn connect_to(&mut self, other_id: u32) { if !self.connections.contains(&other_id) { self.connections.push(other_id); } }
    pub fn unlock(&mut self) { self.is_unlocked = true; }
    pub fn discover(&mut self, flag: &str) { self.discovery_flags.insert(flag.to_string()); }
    pub fn is_discovered(&self, flag: &str) -> bool { self.discovery_flags.contains(flag) }
    pub fn connection_count(&self) -> usize { self.connections.len() }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.bounds.0 && y >= self.bounds.1 && x <= self.bounds.2 && y <= self.bounds.3
    }
}

#[derive(Debug, Clone)]
pub struct WorldRegionGraph {
    pub regions: Vec<MapRegionRecord>,
}

impl WorldRegionGraph {
    pub fn new() -> Self { WorldRegionGraph { regions: Vec::new() } }
    pub fn add_region(&mut self, r: MapRegionRecord) { self.regions.push(r); }
    pub fn region_by_id(&self, id: u32) -> Option<&MapRegionRecord> { self.regions.iter().find(|r| r.region_id == id) }
    pub fn region_by_id_mut(&mut self, id: u32) -> Option<&mut MapRegionRecord> { self.regions.iter_mut().find(|r| r.region_id == id) }
    pub fn region_at(&self, x: i32, y: i32) -> Option<&MapRegionRecord> { self.regions.iter().find(|r| r.contains(x, y)) }
    pub fn unlocked_regions(&self) -> Vec<&MapRegionRecord> { self.regions.iter().filter(|r| r.is_unlocked).collect() }

    pub fn bfs_path(&self, start: u32, goal: u32) -> Option<Vec<u32>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(vec![start]);
        while let Some(path) = queue.pop_front() {
            let &last = path.last().unwrap();
            if last == goal { return Some(path); }
            if visited.contains(&last) { continue; }
            visited.insert(last);
            if let Some(region) = self.region_by_id(last) {
                for &neighbor in &region.connections {
                    if !visited.contains(&neighbor) {
                        let mut new_path = path.clone();
                        new_path.push(neighbor);
                        queue.push_back(new_path);
                    }
                }
            }
        }
        None
    }

    pub fn is_connected(&self, a: u32, b: u32) -> bool { self.bfs_path(a, b).is_some() }
    pub fn region_count(&self) -> usize { self.regions.len() }
}

// ============================================================
// SECTION: Dungeon Room System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DungeonRoomType2 { Entrance, Exit, Combat, Treasure, Boss, Corridor, Shop, Rest, Puzzle, Secret }

#[derive(Debug, Clone)]
pub struct DungeonRoom2 {
    pub id: u32, pub x: i32, pub y: i32, pub width: i32, pub height: i32,
    pub room_type: DungeonRoomType2, pub connections: Vec<u32>, pub loot_level: u8, pub is_cleared: bool,
}

impl DungeonRoom2 {
    pub fn new(id: u32, x: i32, y: i32, w: i32, h: i32, rt: DungeonRoomType2) -> Self {
        DungeonRoom2 { id, x, y, width: w, height: h, room_type: rt, connections: Vec::new(), loot_level: 1, is_cleared: false }
    }
    pub fn center(&self) -> (i32, i32) { (self.x + self.width/2, self.y + self.height/2) }
    pub fn area(&self) -> i32 { self.width * self.height }
    pub fn overlaps(&self, o: &DungeonRoom2) -> bool { self.x < o.x+o.width && self.x+self.width > o.x && self.y < o.y+o.height && self.y+self.height > o.y }
    pub fn dist_to(&self, o: &DungeonRoom2) -> f32 { let (ax,ay) = self.center(); let (bx,by) = o.center(); (((bx-ax).pow(2)+(by-ay).pow(2)) as f32).sqrt() }
    pub fn connect(&mut self, oid: u32) { if !self.connections.contains(&oid) { self.connections.push(oid); } }
}

#[derive(Debug, Clone)]
pub struct DungeonGen2 {
    pub width: i32, pub height: i32, pub seed: u64,
    pub min_room: i32, pub max_room: i32, pub max_rooms: u32,
}

impl DungeonGen2 {
    pub fn new(w: i32, h: i32, seed: u64) -> Self { DungeonGen2 { width: w, height: h, seed, min_room: 4, max_room: 12, max_rooms: 20 } }

    fn rng(&self, n: u64) -> u64 { self.seed.wrapping_mul(6364136223846793005).wrapping_add(n).wrapping_mul(1442695040888963407) >> 33 }

    pub fn generate(&self) -> Vec<DungeonRoom2> {
        let mut rooms = Vec::new();
        let mut attempt = 0u64;
        while rooms.len() < self.max_rooms as usize && attempt < 200 {
            let w = (self.rng(attempt) % (self.max_room - self.min_room) as u64 + self.min_room as u64) as i32;
            let h = (self.rng(attempt+1) % (self.max_room - self.min_room) as u64 + self.min_room as u64) as i32;
            let x = (self.rng(attempt+2) % (self.width - w - 2) as u64 + 1) as i32;
            let y = (self.rng(attempt+3) % (self.height - h - 2) as u64 + 1) as i32;
            let room = DungeonRoom2::new(rooms.len() as u32, x, y, w, h, DungeonRoomType2::Combat);
            if !rooms.iter().any(|r: &DungeonRoom2| r.overlaps(&room)) { rooms.push(room); }
            attempt += 4;
        }
        if !rooms.is_empty() { rooms[0].room_type = DungeonRoomType2::Entrance; }
        if rooms.len() > 1 { rooms.last_mut().unwrap().room_type = DungeonRoomType2::Exit; }
        if rooms.len() > 3 { let mid = rooms.len()/2; rooms[mid].room_type = DungeonRoomType2::Treasure; }
        self.connect_mst(&mut rooms);
        self.mark_boss(&mut rooms);
        rooms
    }

    fn connect_mst(&self, rooms: &mut Vec<DungeonRoom2>) {
        let n = rooms.len(); if n == 0 { return; }
        let mut in_mst = vec![false; n]; in_mst[0] = true;
        for _ in 0..n-1 {
            let (mut ba, mut bb, mut bd) = (0, 0, f32::MAX);
            for a in 0..n { if !in_mst[a] { continue; } for b in 0..n { if in_mst[b] { continue; } let d = rooms[a].dist_to(&rooms[b]); if d < bd { bd = d; ba = a; bb = b; } } }
            let (ida, idb) = (rooms[ba].id, rooms[bb].id);
            rooms[ba].connect(idb); rooms[bb].connect(ida); in_mst[bb] = true;
        }
    }

    fn mark_boss(&self, rooms: &mut Vec<DungeonRoom2>) {
        if rooms.len() < 4 { return; }
        let (ex, ey) = rooms[0].center();
        let idx = (1..rooms.len()).max_by(|&a, &b| {
            let (ax,ay) = rooms[a].center(); let (bx,by) = rooms[b].center();
            let da = ((ax-ex).pow(2)+(ay-ey).pow(2)) as f32;
            let db = ((bx-ex).pow(2)+(by-ey).pow(2)) as f32;
            da.partial_cmp(&db).unwrap()
        }).unwrap_or(1);
        rooms[idx].room_type = DungeonRoomType2::Boss; rooms[idx].loot_level = 5;
    }
}

// ============================================================
// SECTION: A* Pathfinding
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TileCoord { pub x: i32, pub y: i32 }

impl TileCoord {
    pub fn new(x: i32, y: i32) -> Self { TileCoord { x, y } }
    pub fn manhattan(&self, o: &TileCoord) -> i32 { (self.x-o.x).abs() + (self.y-o.y).abs() }
    pub fn octile(&self, o: &TileCoord) -> f32 { let dx=(self.x-o.x).abs() as f32; let dy=(self.y-o.y).abs() as f32; dx+dy-(2.0-std::f32::consts::SQRT_2)*dx.min(dy) }
    pub fn neighbors4(&self) -> [TileCoord; 4] { [TileCoord::new(self.x+1,self.y),TileCoord::new(self.x-1,self.y),TileCoord::new(self.x,self.y+1),TileCoord::new(self.x,self.y-1)] }
    pub fn neighbors8(&self) -> [TileCoord; 8] { [TileCoord::new(self.x+1,self.y),TileCoord::new(self.x-1,self.y),TileCoord::new(self.x,self.y+1),TileCoord::new(self.x,self.y-1),TileCoord::new(self.x+1,self.y+1),TileCoord::new(self.x-1,self.y+1),TileCoord::new(self.x+1,self.y-1),TileCoord::new(self.x-1,self.y-1)] }
}

#[derive(Debug, Clone)]
pub struct PathGrid {
    pub width: i32, pub height: i32, pub passable: Vec<bool>, pub costs: Vec<f32>,
}

impl PathGrid {
    pub fn new(w: i32, h: i32) -> Self { PathGrid { width: w, height: h, passable: vec![true; (w*h) as usize], costs: vec![1.0; (w*h) as usize] } }
    fn idx(&self, x: i32, y: i32) -> Option<usize> { if x>=0&&y>=0&&x<self.width&&y<self.height { Some((y*self.width+x) as usize) } else { None } }
    pub fn set_passable(&mut self, x: i32, y: i32, p: bool) { if let Some(i) = self.idx(x,y) { self.passable[i]=p; } }
    pub fn is_passable(&self, c: &TileCoord) -> bool { self.idx(c.x,c.y).map(|i|self.passable[i]).unwrap_or(false) }
    pub fn cost_at(&self, c: &TileCoord) -> f32 { self.idx(c.x,c.y).map(|i|self.costs[i]).unwrap_or(f32::MAX) }

    pub fn astar(&self, start: TileCoord, goal: TileCoord, diagonal: bool) -> Option<Vec<TileCoord>> {
        use std::collections::BinaryHeap;
        #[derive(PartialEq)] struct N { pos: TileCoord, f: f32 }
        impl Eq for N {}
        impl PartialOrd for N { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) } }
        impl Ord for N { fn cmp(&self, o: &Self) -> std::cmp::Ordering { o.f.partial_cmp(&self.f).unwrap_or(std::cmp::Ordering::Equal) } }
        let mut g: HashMap<TileCoord, f32> = HashMap::new();
        let mut from: HashMap<TileCoord, TileCoord> = HashMap::new();
        let mut open = BinaryHeap::new();
        g.insert(start.clone(), 0.0);
        open.push(N { pos: start.clone(), f: start.octile(&goal) });
        while let Some(N { pos: cur, .. }) = open.pop() {
            if cur == goal {
                let mut path = vec![goal.clone()]; let mut p = goal.clone();
                while let Some(prev) = from.get(&p) { path.push(prev.clone()); p = prev.clone(); }
                path.reverse(); return Some(path);
            }
            let nbrs: Vec<TileCoord> = if diagonal { cur.neighbors8().to_vec() } else { cur.neighbors4().to_vec() };
            for nb in nbrs {
                if !self.is_passable(&nb) { continue; }
                let dx=(nb.x-cur.x).abs(); let dy=(nb.y-cur.y).abs();
                let mc = if dx+dy==2 { std::f32::consts::SQRT_2 } else { 1.0 };
                let tg = g.get(&cur).copied().unwrap_or(f32::MAX) + mc * self.cost_at(&nb);
                if tg < g.get(&nb).copied().unwrap_or(f32::MAX) {
                    g.insert(nb.clone(), tg); from.insert(nb.clone(), cur.clone());
                    open.push(N { pos: nb.clone(), f: tg + nb.octile(&goal) });
                }
            }
        }
        None
    }
}

// ============================================================
// SECTION: Map Event System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MapEventKind { Enter, Exit, Spawn, Kill, Open, Trap, Transition, Quest, Secret, Boss }

#[derive(Debug, Clone)]
pub struct MapEventRecord {
    pub id: u32, pub kind: MapEventKind, pub x: i32, pub y: i32,
    pub data: HashMap<String, String>, pub fired: bool, pub cooldown: f32, pub last_fire: f32,
}

impl MapEventRecord {
    pub fn new(id: u32, kind: MapEventKind, x: i32, y: i32) -> Self {
        MapEventRecord { id, kind, x, y, data: HashMap::new(), fired: false, cooldown: 0.0, last_fire: -999.0 }
    }
    pub fn can_fire(&self, t: f32) -> bool { !self.fired || (self.cooldown > 0.0 && t - self.last_fire >= self.cooldown) }
    pub fn fire(&mut self, t: f32) { self.fired = true; self.last_fire = t; }
    pub fn at(&self, x: i32, y: i32) -> bool { self.x == x && self.y == y }
}

#[derive(Debug, Clone)]
pub struct MapEventRegistry {
    pub events: Vec<MapEventRecord>, pub time: f32,
}

impl MapEventRegistry {
    pub fn new() -> Self { MapEventRegistry { events: Vec::new(), time: 0.0 } }
    pub fn add(&mut self, e: MapEventRecord) { self.events.push(e); }
    pub fn trigger_at(&mut self, x: i32, y: i32) -> Vec<u32> {
        let t = self.time; let mut fired = Vec::new();
        for e in &mut self.events { if e.at(x,y) && e.can_fire(t) { e.fire(t); fired.push(e.id); } }
        fired
    }
    pub fn reset_all(&mut self) { for e in &mut self.events { e.fired = false; } }
    pub fn of_kind(&self, k: &MapEventKind) -> Vec<&MapEventRecord> { self.events.iter().filter(|e| &e.kind == k).collect() }
}

// ============================================================
// SECTION: Minimap
// ============================================================

#[derive(Debug, Clone)]
pub struct MinimapTile { pub explored: bool, pub visible: bool, pub tile_type: u8 }

#[derive(Debug, Clone)]
pub struct MinimapView {
    pub width: u32, pub height: u32, pub tiles: Vec<MinimapTile>, pub scale: u32,
}

impl MinimapView {
    pub fn new(w: u32, h: u32, scale: u32) -> Self { MinimapView { width: w, height: h, tiles: vec![MinimapTile{explored:false,visible:false,tile_type:0}; (w*h) as usize], scale } }
    fn idx(&self, x: u32, y: u32) -> Option<usize> { if x<self.width&&y<self.height { Some((y*self.width+x) as usize) } else { None } }
    pub fn reveal(&mut self, x: u32, y: u32) { if let Some(i) = self.idx(x,y) { self.tiles[i].explored = true; self.tiles[i].visible = true; } }
    pub fn hide(&mut self, x: u32, y: u32) { if let Some(i) = self.idx(x,y) { self.tiles[i].visible = false; } }
    pub fn is_explored(&self, x: u32, y: u32) -> bool { self.idx(x,y).map(|i|self.tiles[i].explored).unwrap_or(false) }
    pub fn explored_count(&self) -> usize { self.tiles.iter().filter(|t|t.explored).count() }
    pub fn exploration_pct(&self) -> f32 { self.explored_count() as f32 / self.tiles.len() as f32 * 100.0 }
    pub fn reveal_radius(&mut self, cx: i32, cy: i32, r: i32) {
        for dy in -r..=r { for dx in -r..=r { if dx*dx+dy*dy<=r*r { let x=cx+dx; let y=cy+dy; if x>=0&&y>=0&&(x as u32)<self.width&&(y as u32)<self.height { self.reveal(x as u32, y as u32); } } } }
    }
}

// ============================================================
// SECTION: NPC Patrol
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PatrolStatus { Idle, Walking, Alert, Searching, Returning }

#[derive(Debug, Clone)]
pub struct PatrolPoint { pub x: i32, pub y: i32, pub wait_s: f32 }

impl PatrolPoint {
    pub fn new(x: i32, y: i32) -> Self { PatrolPoint { x, y, wait_s: 0.0 } }
    pub fn with_wait(mut self, t: f32) -> Self { self.wait_s = t; self }
}

#[derive(Debug, Clone)]
pub struct NpcPatrol {
    pub id: u32, pub waypoints: Vec<PatrolPoint>, pub current_wp: usize,
    pub status: PatrolStatus, pub pos: (f32, f32), pub speed: f32,
    pub alert_radius: f32, pub search_timer: f32, pub home: (f32, f32),
}

impl NpcPatrol {
    pub fn new(id: u32, pos: (f32, f32), speed: f32) -> Self {
        NpcPatrol { id, waypoints: Vec::new(), current_wp: 0, status: PatrolStatus::Idle, pos, speed, alert_radius: 5.0, search_timer: 0.0, home: pos }
    }
    pub fn add_waypoint(&mut self, p: PatrolPoint) { self.waypoints.push(p); }
    pub fn alert(&mut self) { if self.status == PatrolStatus::Idle || self.status == PatrolStatus::Walking { self.status = PatrolStatus::Alert; } }

    pub fn update(&mut self, dt: f32) {
        match self.status {
            PatrolStatus::Idle => { if !self.waypoints.is_empty() { self.status = PatrolStatus::Walking; } },
            PatrolStatus::Walking => {
                if self.waypoints.is_empty() { return; }
                let wp = &self.waypoints[self.current_wp];
                let (tx, ty) = (wp.x as f32, wp.y as f32);
                let (px, py) = self.pos;
                let dx = tx-px; let dy = ty-py;
                let dist = (dx*dx+dy*dy).sqrt();
                if dist < 0.1 { self.current_wp = (self.current_wp+1)%self.waypoints.len(); }
                else { let s=self.speed*dt; self.pos=(px+dx/dist*s, py+dy/dist*s); }
            },
            PatrolStatus::Alert => { self.search_timer = 5.0; self.status = PatrolStatus::Searching; },
            PatrolStatus::Searching => { self.search_timer -= dt; if self.search_timer <= 0.0 { self.status = PatrolStatus::Returning; } },
            PatrolStatus::Returning => {
                let (hx,hy) = self.home; let (px,py) = self.pos;
                let dx=hx-px; let dy=hy-py; let dist=(dx*dx+dy*dy).sqrt();
                if dist < 0.1 { self.status = PatrolStatus::Walking; }
                else { let s=self.speed*dt; self.pos=(px+dx/dist*s, py+dy/dist*s); }
            },
        }
    }
    pub fn detect(&self, px: f32, py: f32) -> bool { let dx=self.pos.0-px; let dy=self.pos.1-py; (dx*dx+dy*dy).sqrt()<=self.alert_radius }
}

// ============================================================
// SECTION: Map Annotation Layer
// ============================================================

#[derive(Debug, Clone)]
pub struct MapNote {
    pub id: u32, pub x: i32, pub y: i32, pub text: String, pub color: [u8;3], pub visible: bool,
}

impl MapNote {
    pub fn new(id: u32, x: i32, y: i32, text: &str) -> Self { MapNote { id, x, y, text: text.to_string(), color: [255,255,0], visible: true } }
    pub fn warning(id: u32, x: i32, y: i32, text: &str) -> Self { let mut n = Self::new(id,x,y,text); n.color=[255,80,0]; n }
    pub fn secret(id: u32, x: i32, y: i32, text: &str) -> Self { let mut n = Self::new(id,x,y,text); n.color=[160,0,255]; n }
}

#[derive(Debug, Clone)]
pub struct NoteLayer {
    pub notes: Vec<MapNote>,
}

impl NoteLayer {
    pub fn new() -> Self { NoteLayer { notes: Vec::new() } }
    pub fn add(&mut self, n: MapNote) { self.notes.push(n); }
    pub fn remove(&mut self, id: u32) { self.notes.retain(|n|n.id!=id); }
    pub fn at(&self, x: i32, y: i32) -> Vec<&MapNote> { self.notes.iter().filter(|n|n.x==x&&n.y==y).collect() }
    pub fn visible_count(&self) -> usize { self.notes.iter().filter(|n|n.visible).count() }
    pub fn search(&self, q: &str) -> Vec<&MapNote> { self.notes.iter().filter(|n|n.text.contains(q)).collect() }
    pub fn export(&self) -> String { self.notes.iter().filter(|n|n.visible).map(|n|format!("[{},{}] {}",n.x,n.y,n.text)).collect::<Vec<_>>().join("\n") }
}

// ============================================================
// SECTION: Tile Autotiling
// ============================================================

pub fn wang_blob(n: bool, e: bool, s: bool, w: bool, ne: bool, se: bool, sw: bool, nw: bool) -> u8 {
    let mut idx = 0u8;
    if n { idx|=1; } if e { idx|=2; } if s { idx|=4; } if w { idx|=8; }
    if nw&&n&&w { idx|=16; } if ne&&n&&e { idx|=32; } if se&&s&&e { idx|=64; } if sw&&s&&w { idx|=128; }
    idx
}

#[derive(Debug, Clone)]
pub struct AutotileGrid {
    pub width: i32, pub height: i32, pub solid: Vec<bool>,
}

impl AutotileGrid {
    pub fn new(w: i32, h: i32) -> Self { AutotileGrid { width: w, height: h, solid: vec![false; (w*h) as usize] } }
    fn idx(&self, x: i32, y: i32) -> Option<usize> { if x>=0&&y>=0&&x<self.width&&y<self.height { Some((y*self.width+x) as usize) } else { None } }
    pub fn set(&mut self, x: i32, y: i32, s: bool) { if let Some(i) = self.idx(x,y) { self.solid[i]=s; } }
    pub fn is_solid(&self, x: i32, y: i32) -> bool { self.idx(x,y).map(|i|self.solid[i]).unwrap_or(false) }
    pub fn fill_rect(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) { for y in y1..=y2 { for x in x1..=x2 { self.set(x,y,true); } } }
    pub fn tile_at(&self, x: i32, y: i32) -> u8 {
        if !self.is_solid(x,y) { return 0; }
        wang_blob(self.is_solid(x,y-1),self.is_solid(x+1,y),self.is_solid(x,y+1),self.is_solid(x-1,y),self.is_solid(x+1,y-1),self.is_solid(x+1,y+1),self.is_solid(x-1,y+1),self.is_solid(x-1,y-1))
    }
    pub fn solid_count(&self) -> usize { self.solid.iter().filter(|&&s|s).count() }
}

// ============================================================
// SECTION: Loot Tables
// ============================================================

#[derive(Debug, Clone)]
pub struct LootItem { pub id: u32, pub name: String, pub weight: u32, pub min_qty: u32, pub max_qty: u32, pub min_level: u32 }

impl LootItem {
    pub fn new(id: u32, name: &str, weight: u32, min: u32, max: u32) -> Self { LootItem { id, name: name.to_string(), weight, min_qty: min, max_qty: max, min_level: 1 } }
}

#[derive(Debug, Clone)]
pub struct LootTableDef {
    pub id: u32, pub name: String, pub items: Vec<LootItem>, pub guaranteed: Vec<LootItem>, pub rolls_min: u32, pub rolls_max: u32,
}

impl LootTableDef {
    pub fn new(id: u32, name: &str) -> Self { LootTableDef { id, name: name.to_string(), items: Vec::new(), guaranteed: Vec::new(), rolls_min: 1, rolls_max: 3 } }
    pub fn add_item(&mut self, item: LootItem) { self.items.push(item); }
    pub fn add_guaranteed(&mut self, item: LootItem) { self.guaranteed.push(item); }
    pub fn total_weight(&self) -> u32 { self.items.iter().map(|i|i.weight).sum() }

    pub fn roll(&self, seed: u64, level: u32) -> Vec<(u32, u32)> {
        let mut results = Vec::new();
        for item in &self.guaranteed { if level >= item.min_level { results.push((item.id, item.min_qty)); } }
        let tw = self.total_weight(); if tw == 0 { return results; }
        let rolls = self.rolls_min + (seed % (self.rolls_max - self.rolls_min + 1).max(1) as u64) as u32;
        for i in 0..rolls {
            let roll = (seed.wrapping_add(i as u64 * 7919) % tw as u64) as u32;
            let mut acc = 0u32;
            for item in &self.items {
                acc += item.weight;
                if roll < acc && level >= item.min_level {
                    let qty = item.min_qty + (seed.wrapping_add(i as u64) % (item.max_qty - item.min_qty + 1).max(1) as u64) as u32;
                    results.push((item.id, qty)); break;
                }
            }
        }
        results
    }

    pub fn goblin_loot() -> Self {
        let mut t = Self::new(1, "goblin"); t.rolls_min=1; t.rolls_max=3;
        t.add_item(LootItem::new(101,"gold_coin",50,1,10));
        t.add_item(LootItem::new(102,"iron_sword",20,1,1));
        t.add_guaranteed(LootItem::new(100,"small_key",0,1,1));
        t
    }

    pub fn boss_loot() -> Self {
        let mut t = Self::new(2, "boss"); t.rolls_min=3; t.rolls_max=6;
        t.add_item(LootItem::new(201,"gold_coin",30,50,200));
        t.add_item(LootItem::new(202,"magic_sword",10,1,1));
        t.add_item(LootItem::new(203,"enchanted_armor",10,1,1));
        t.add_guaranteed(LootItem::new(200,"boss_key",0,1,1));
        t
    }
}

// ============================================================
// SECTION: Tile Effect Map
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TileEffectKind { None, Damage, Heal, Slow, Haste, Burn, Poison, Freeze }

#[derive(Debug, Clone)]
pub struct TileEffects {
    pub width: i32, pub height: i32, pub kinds: Vec<TileEffectKind>, pub params: Vec<f32>,
}

impl TileEffects {
    pub fn new(w: i32, h: i32) -> Self { let sz=(w*h) as usize; TileEffects { width: w, height: h, kinds: vec![TileEffectKind::None;sz], params: vec![0.0;sz] } }
    fn idx(&self,x:i32,y:i32)->Option<usize>{if x>=0&&y>=0&&x<self.width&&y<self.height{Some((y*self.width+x) as usize)}else{None}}
    pub fn set(&mut self,x:i32,y:i32,k:TileEffectKind,p:f32){if let Some(i)=self.idx(x,y){self.kinds[i]=k;self.params[i]=p;}}
    pub fn kind_at(&self,x:i32,y:i32)->&TileEffectKind{self.idx(x,y).map(|i|&self.kinds[i]).unwrap_or(&TileEffectKind::None)}
    pub fn apply(&self,x:i32,y:i32,hp:&mut f32,spd:&mut f32){
        let p=self.idx(x,y).map(|i|self.params[i]).unwrap_or(0.0);
        match self.kind_at(x,y){TileEffectKind::Damage=>{*hp-=p;}TileEffectKind::Heal=>{*hp+=p;}TileEffectKind::Slow=>{*spd*=0.5;}TileEffectKind::Burn=>{*hp-=p*2.0;}TileEffectKind::Poison=>{*hp-=p*0.5;}_=>{}}
    }
}

// ============================================================
// SECTION: Map Layer Stack
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LayerKind { Terrain, Decoration, Object, Collision, Event }

#[derive(Debug, Clone)]
pub struct MapLayerDef {
    pub name: String, pub kind: LayerKind, pub width: i32, pub height: i32,
    pub data: Vec<u32>, pub visible: bool, pub opacity: f32,
}

impl MapLayerDef {
    pub fn new(name: &str, kind: LayerKind, w: i32, h: i32) -> Self {
        MapLayerDef { name: name.to_string(), kind, width: w, height: h, data: vec![0;(w*h) as usize], visible: true, opacity: 1.0 }
    }
    fn idx(&self,x:i32,y:i32)->Option<usize>{if x>=0&&y>=0&&x<self.width&&y<self.height{Some((y*self.width+x) as usize)}else{None}}
    pub fn get(&self,x:i32,y:i32)->u32{self.idx(x,y).map(|i|self.data[i]).unwrap_or(0)}
    pub fn set(&mut self,x:i32,y:i32,t:u32){if let Some(i)=self.idx(x,y){self.data[i]=t;}}
    pub fn fill(&mut self,t:u32){for d in &mut self.data{*d=t;}}
    pub fn flood_fill(&mut self,sx:i32,sy:i32,new_t:u32){
        let old=self.get(sx,sy); if old==new_t{return;}
        let mut stack=vec![(sx,sy)];
        while let Some((x,y))=stack.pop(){
            if self.get(x,y)!=old{continue;} self.set(x,y,new_t);
            for(nx,ny) in [(x+1,y),(x-1,y),(x,y+1),(x,y-1)]{if self.get(nx,ny)==old{stack.push((nx,ny));}}
        }
    }
    pub fn non_empty(&self)->usize{self.data.iter().filter(|&&t|t!=0).count()}
}

#[derive(Debug, Clone)]
pub struct LayerStack {
    pub layers: Vec<MapLayerDef>, pub width: i32, pub height: i32,
}

impl LayerStack {
    pub fn new(w: i32, h: i32) -> Self { LayerStack { layers: Vec::new(), width: w, height: h } }
    pub fn push(&mut self, l: MapLayerDef) { self.layers.push(l); }
    pub fn layer(&self, name: &str) -> Option<&MapLayerDef> { self.layers.iter().find(|l|l.name==name) }
    pub fn layer_mut(&mut self, name: &str) -> Option<&mut MapLayerDef> { self.layers.iter_mut().find(|l|l.name==name) }
    pub fn composite_at(&self, x: i32, y: i32) -> u32 { for l in self.layers.iter().rev() { if !l.visible{continue;} let t=l.get(x,y); if t!=0{return t;} } 0 }
    pub fn visible_count(&self) -> usize { self.layers.iter().filter(|l|l.visible).count() }
    pub fn count(&self) -> usize { self.layers.len() }

    pub fn standard_rpg(w: i32, h: i32) -> Self {
        let mut s = Self::new(w, h);
        s.push(MapLayerDef::new("terrain", LayerKind::Terrain, w, h));
        s.push(MapLayerDef::new("deco", LayerKind::Decoration, w, h));
        s.push(MapLayerDef::new("objects", LayerKind::Object, w, h));
        s.push(MapLayerDef::new("collision", LayerKind::Collision, w, h));
        s.push(MapLayerDef::new("events", LayerKind::Event, w, h));
        s
    }
}

// ============================================================
// SECTION: Map Progression
// ============================================================

#[derive(Debug, Clone)]
pub struct MapProgress {
    pub map_id: u32, pub enemies_total: u32, pub enemies_killed: u32,
    pub chests_total: u32, pub chests_opened: u32, pub secrets_total: u32,
    pub secrets_found: u32, pub boss_dead: bool, pub flags: HashSet<String>,
}

impl MapProgress {
    pub fn new(id: u32) -> Self { MapProgress { map_id: id, enemies_total: 0, enemies_killed: 0, chests_total: 0, chests_opened: 0, secrets_total: 0, secrets_found: 0, boss_dead: false, flags: HashSet::new() } }
    pub fn completion_pct(&self) -> f32 {
        let mut s=0.0f32; let mut t=0.0f32;
        if self.enemies_total>0{s+=self.enemies_killed as f32/self.enemies_total as f32*40.0;t+=40.0;}
        if self.chests_total>0{s+=self.chests_opened as f32/self.chests_total as f32*30.0;t+=30.0;}
        if self.secrets_total>0{s+=self.secrets_found as f32/self.secrets_total as f32*20.0;t+=20.0;}
        s+=if self.boss_dead{10.0}else{0.0};t+=10.0;
        if t>0.0{s/t*100.0}else{0.0}
    }
    pub fn is_100pct(&self)->bool{self.enemies_killed==self.enemies_total&&self.chests_opened==self.chests_total&&self.secrets_found==self.secrets_total&&self.boss_dead}
    pub fn grade(&self)->char{match self.completion_pct() as u32{90..=100=>'S',80..=89=>'A',70..=79=>'B',60..=69=>'C',_=>'D'}}
    pub fn set_flag(&mut self,f:&str){self.flags.insert(f.to_string());}
    pub fn has_flag(&self,f:&str)->bool{self.flags.contains(f)}
}

// ============================================================
// SECTION: Integration Tests
// ============================================================

pub fn run_map_region_tests() {
    let mut g = WorldRegionGraph::new();
    let mut r1 = MapRegionRecord::new(1,"Village",(0,0,50,50),BiomeType::Grassland); r1.unlock();
    let r2 = MapRegionRecord::new(2,"Forest",(60,0,120,50),BiomeType::Forest);
    let r3 = MapRegionRecord::new(3,"Desert",(130,0,200,50),BiomeType::Desert);
    g.add_region(r1); g.add_region(r2); g.add_region(r3);
    g.region_by_id_mut(1).unwrap().connect_to(2); g.region_by_id_mut(2).unwrap().connect_to(1);
    g.region_by_id_mut(2).unwrap().connect_to(3); g.region_by_id_mut(3).unwrap().connect_to(2);
    let path = g.bfs_path(1,3); assert!(path.is_some()); assert_eq!(path.unwrap(),vec![1,2,3]);
    assert!(g.is_connected(1,3)); assert_eq!(g.unlocked_regions().len(),1);
    let region = g.region_at(25,25); assert!(region.is_some());
}

pub fn run_dungeon_gen2_tests() {
    let gen = DungeonGen2::new(80,80,42);
    let rooms = gen.generate();
    assert!(!rooms.is_empty());
    assert_eq!(rooms[0].room_type, DungeonRoomType2::Entrance);
    if rooms.len() >= 4 { assert!(rooms.iter().any(|r| r.room_type == DungeonRoomType2::Boss)); }
    for r in &rooms { assert!(r.area() > 0); }
}

pub fn run_pathgrid_tests() {
    let mut grid = PathGrid::new(20,20);
    for y in 5..15 { grid.set_passable(10,y,false); }
    let path = grid.astar(TileCoord::new(5,10),TileCoord::new(15,10),false);
    assert!(path.is_some());
    let p = path.unwrap();
    assert_eq!(*p.first().unwrap(), TileCoord::new(5,10));
    assert_eq!(*p.last().unwrap(), TileCoord::new(15,10));
    for pos in &p { assert!(grid.is_passable(pos)); }
}

pub fn run_map_event_registry_tests() {
    let mut reg = MapEventRegistry::new();
    reg.add(MapEventRecord::new(1,MapEventKind::Enter,5,5));
    reg.add(MapEventRecord::new(2,MapEventKind::Trap,5,5));
    let fired = reg.trigger_at(5,5); assert_eq!(fired.len(),2);
    reg.reset_all();
    assert!(reg.events[0].can_fire(0.0));
}

pub fn run_minimap_view_tests() {
    let mut mm = MinimapView::new(64,64,2);
    mm.reveal_radius(32,32,8);
    assert!(mm.exploration_pct()>0.0);
    assert!(mm.is_explored(32,32));
    assert!(!mm.is_explored(0,0));
}

pub fn run_npc_patrol_tests() {
    let mut npc = NpcPatrol::new(1,(0.0,0.0),3.0);
    npc.add_waypoint(PatrolPoint::new(10,0).with_wait(1.0));
    npc.add_waypoint(PatrolPoint::new(10,10));
    assert_eq!(npc.status,PatrolStatus::Idle);
    npc.update(0.016); assert_eq!(npc.status,PatrolStatus::Walking);
    npc.alert(); assert_eq!(npc.status,PatrolStatus::Alert);
    npc.update(0.016); assert_eq!(npc.status,PatrolStatus::Searching);
    npc.update(6.0); assert_eq!(npc.status,PatrolStatus::Returning);
}

pub fn run_autotile_grid_tests() {
    let mut grid = AutotileGrid::new(20,20);
    grid.fill_rect(5,5,10,10);
    assert!(grid.solid_count()>0);
    assert_eq!(grid.tile_at(7,7),255);
    let edge = grid.tile_at(5,5);
    assert_ne!(edge,255);
}

pub fn run_loot_table_tests2() {
    let goblin = LootTableDef::goblin_loot();
    assert!(goblin.total_weight()>0);
    let loot = goblin.roll(12345,5); assert!(!loot.is_empty());
    let boss = LootTableDef::boss_loot();
    let boss_loot = boss.roll(99999,10); assert!(!boss_loot.is_empty());
}

pub fn run_tile_effects_tests() {
    let mut ef = TileEffects::new(20,20);
    ef.set(5,5,TileEffectKind::Damage,10.0);
    ef.set(6,6,TileEffectKind::Heal,5.0);
    ef.set(7,7,TileEffectKind::Slow,0.5);
    let mut hp=100.0f32; let mut spd=1.0f32;
    ef.apply(5,5,&mut hp,&mut spd); assert_eq!(hp,90.0);
    ef.apply(6,6,&mut hp,&mut spd); assert_eq!(hp,95.0);
    ef.apply(7,7,&mut hp,&mut spd); assert!((spd-0.5).abs()<0.001);
}

pub fn run_layer_stack_tests2() {
    let mut stack = LayerStack::standard_rpg(32,32);
    assert_eq!(stack.count(),5);
    if let Some(layer) = stack.layer_mut("terrain") { layer.fill(1); layer.set(5,5,42); }
    let comp = stack.composite_at(5,5); assert!(comp>0);
    assert!(stack.visible_count()>0);
}

pub fn run_map_progress_tests() {
    let mut p = MapProgress::new(1);
    p.enemies_total=10; p.enemies_killed=10; p.chests_total=5; p.chests_opened=5;
    p.secrets_total=3; p.secrets_found=3; p.boss_dead=true;
    assert!(p.is_100pct()); assert_eq!(p.grade(),'S');
    p.set_flag("secret_passage"); assert!(p.has_flag("secret_passage"));
}

pub fn run_note_layer_tests() {
    let mut layer = NoteLayer::new();
    layer.add(MapNote::new(1,5,5,"Town entrance"));
    layer.add(MapNote::warning(2,10,10,"Danger"));
    layer.add(MapNote::secret(3,20,20,"Hidden"));
    assert_eq!(layer.visible_count(),3);
    assert_eq!(layer.at(5,5).len(),1);
    assert_eq!(layer.search("Hidden").len(),1);
    let exported = layer.export();
    assert!(exported.contains("Town entrance"));
    layer.remove(1); assert_eq!(layer.notes.len(),2);
}

pub fn map_editor_mega_test() {
    run_map_region_tests();
    run_dungeon_gen2_tests();
    run_pathgrid_tests();
    run_map_event_registry_tests();
    run_minimap_view_tests();
    run_npc_patrol_tests();
    run_autotile_grid_tests();
    run_loot_table_tests2();
    run_tile_effects_tests();
    run_layer_stack_tests2();
    run_map_progress_tests();
    run_note_layer_tests();
    map_editor_final();
}

pub const MAP_MODULE_VERSION: &str = "3.0.0";
pub const MAP_MAX_SIZE: i32 = 4096;
pub const MAP_CHUNK_SZ: i32 = 16;
pub const MAP_MAX_LAYERS: usize = 32;
pub const MAP_AUTOTILE_VARIANTS: u32 = 256;

pub fn map_editor_info() -> HashMap<String, String> {
    let mut info = HashMap::new();
    info.insert("module".into(), "map_editor".into());
    info.insert("version".into(), MAP_MODULE_VERSION.into());
    info.insert("max_size".into(), format!("{}x{}", MAP_MAX_SIZE, MAP_MAX_SIZE));
    info.insert("pathfinding".into(), "A* octile heuristic".into());
    info.insert("dungeon_gen".into(), "BSP + Prim MST".into());
    info.insert("autotile".into(), "Wang blob 256-variant".into());
    info
}

// Impl blocks for structs that need Default + new + methods
impl ChunkStreamingManager {
    pub fn new(stream_radius: i32) -> Self { Self { loaded_chunks: HashMap::new(), stream_radius, current_frame: 0 } }
    pub fn ensure_loaded(&mut self, cx: i32, cz: i32) { self.loaded_chunks.entry((cx, cz)).or_insert_with(|| MapChunk { chunk_x: cx, chunk_y: cz, tile_data: Vec::new(), is_loaded: true, last_access_frame: 0, dirty: false }); }
    pub fn evict_distant_chunks(&mut self, cx: i32, cz: i32) { let r = self.stream_radius; self.loaded_chunks.retain(|&(x,z),_| (x-cx).abs() <= r && (z-cz).abs() <= r); }
    pub fn get_tile_at(&self, _x: i32, _z: i32) -> Option<u32> { None }
    pub fn loaded_chunk_count(&self) -> usize { self.loaded_chunks.len() }
}
impl FogOfWar {
    pub fn new(w: u32, h: u32, los_radius: u32) -> Self { let n = (w*h) as usize; Self { width: w, height: h, revealed: vec![false; n], visible: vec![false; n], los_radius } }
    pub fn reveal_radius(&mut self, cx: u32, cy: u32) { let r = self.los_radius as i32; for dy in -r..=r { for dx in -r..=r { if dx*dx+dy*dy <= r*r { let nx = cx as i32+dx; let ny = cy as i32+dy; if nx>=0 && ny>=0 && (nx as u32)<self.width && (ny as u32)<self.height { let idx = ny as usize*self.width as usize+nx as usize; self.revealed[idx]=true; self.visible[idx]=true; }}}} }
    pub fn clear_visible(&mut self) { for v in &mut self.visible { *v = false; } }
    pub fn is_revealed(&self, x: u32, y: u32) -> bool { let idx = y as usize*self.width as usize+x as usize; self.revealed.get(idx).copied().unwrap_or(false) }
    pub fn is_visible(&self, x: u32, y: u32) -> bool { let idx = y as usize*self.width as usize+x as usize; self.visible.get(idx).copied().unwrap_or(false) }
    pub fn exploration_percentage(&self) -> f32 { if self.revealed.is_empty() { return 0.0; } self.revealed.iter().filter(|&&v| v).count() as f32 / self.revealed.len() as f32 * 100.0 }
}
impl HeightMap {
    pub fn new(w: u32, h: u32) -> Self { Self { width: w, height: h, data: vec![0.0; (w*h) as usize] } }
    pub fn generate_fbm(&mut self, octaves: u32, seed: u64) { let mut s = seed ^ 0x9e3779b97f4a7c15; for i in 0..self.data.len() { let x = (i % self.width as usize) as f32 / self.width as f32; let y = (i / self.width as usize) as f32 / self.height as f32; let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; for _ in 0..octaves { s ^= s<<13; s ^= s>>7; s ^= s<<17; let nx = x*freq + (s as f32 / u64::MAX as f32); let ny = y*freq + ((s>>32) as f32 / u32::MAX as f32); v += (nx.sin()*ny.cos()) * amp; amp *= 0.5; freq *= 2.0; } self.data[i] = (v + 2.0).max(0.0); } }
    pub fn smooth(&mut self, passes: u32) { for _ in 0..passes { let mut out = self.data.clone(); let w = self.width as usize; let h = self.height as usize; for y in 1..h-1 { for x in 1..w-1 { out[y*w+x] = (self.data[(y-1)*w+x]+self.data[(y+1)*w+x]+self.data[y*w+x-1]+self.data[y*w+x+1]+self.data[y*w+x])/5.0; }} self.data = out; } }
    pub fn apply_erosion(&mut self, passes: u32) { for _ in 0..passes { let w = self.width as usize; let h = self.height as usize; let mut out = self.data.clone(); for y in 1..h-1 { for x in 1..w-1 { let c = self.data[y*w+x]; let neighbors = [self.data[(y-1)*w+x],self.data[(y+1)*w+x],self.data[y*w+x-1],self.data[y*w+x+1]]; let min_n = neighbors.iter().cloned().fold(f32::MAX, f32::min); if c > min_n { out[y*w+x] = c - (c-min_n)*0.1; } }} self.data = out; } }
    pub fn to_terrain_tiles(&self, _db: &TileDatabase, water: f32, mountain: f32) -> Vec<Vec<u32>> { (0..self.height as usize).map(|y| (0..self.width as usize).map(|x| { let v = self.data[y*self.width as usize+x]; if v < water { 0 } else if v < mountain { 1 } else { 2 } }).collect()).collect() }
    pub fn to_terrain_tiles_simple(&self) -> Vec<Vec<u32>> { self.to_terrain_tiles(&TileDatabase::new(), 0.3, 0.75) }
}
impl LevelTransition {
    pub fn new(id: u32, position: (i32, i32), target_map: &str, target_position: (i32, i32)) -> Self { Self { id, position, target_map: target_map.to_string(), target_position, direction: 0, requires_item: None, requires_quest: None, transition_type: String::from("door"), is_bidirectional: true } }
    pub fn can_use<FI: Fn(u32) -> bool, FQ: Fn(u32) -> bool>(&self, has_item: FI, has_quest: FQ) -> bool { self.requires_item.map_or(true, |i| has_item(i)) && self.requires_quest.map_or(true, |q| has_quest(q)) }
}
impl Minimap {
    pub fn new(w: u32, h: u32, scale: u32) -> Self { Self { width: w, height: h, cells: vec![MinimapCell { explored: false, visible: false, tile_type: 0, has_entity: false }; (w*h) as usize], scale } }
    pub fn reveal_radius(&mut self, cx: u32, cy: u32, r: u32) { let r = r as i32; for dy in -r..=r { for dx in -r..=r { let nx = cx as i32+dx; let ny = cy as i32+dy; if nx>=0 && ny>=0 && (nx as u32)<self.width && (ny as u32)<self.height { let idx = ny as usize*self.width as usize+nx as usize; if let Some(c) = self.cells.get_mut(idx) { c.explored=true; c.visible=true; }}}} }
    pub fn is_explored(&self, x: u32, y: u32) -> bool { self.cells.get((y*self.width+x) as usize).map_or(false, |c| c.explored) }
    pub fn exploration_percent(&self) -> f32 { if self.cells.is_empty() { return 0.0; } self.cells.iter().filter(|c| c.explored).count() as f32 / self.cells.len() as f32 * 100.0 }
    pub fn world_to_minimap(&self, wx: f32, wy: f32) -> (u32, u32) { ((wx as u32).min(self.width.saturating_sub(1)), (wy as u32).min(self.height.saturating_sub(1))) }
}
impl QuestJournal {
    pub fn new() -> Self { Self { quests: HashMap::new(), active_quest_id: None } }
    pub fn add_quest(&mut self, q: Quest) { self.quests.insert(q.id, q); }
    pub fn start_quest(&mut self, id: u32) { if let Some(q) = self.quests.get_mut(&id) { q.status = QuestStatus::Active; self.active_quest_id = Some(id); } }
    pub fn active_quests(&self) -> Vec<&Quest> { self.quests.values().filter(|q| matches!(q.status, QuestStatus::Active)).collect() }
}
impl Quest {
    pub fn new(id: u32, name: &str, description: &str) -> Self { Self { id, name: name.to_string(), description: description.to_string(), status: QuestStatus::NotStarted, objectives: Vec::new(), reward: QuestReward { experience: 0, gold: 0, items: Vec::new(), reputation: HashMap::new() }, prereq_quests: Vec::new(), journal_entries: Vec::new(), is_main_quest: false, chapter: 1 } }
    pub fn add_objective(&mut self, obj: QuestObjectiveKind) { self.objectives.push(obj); }
    pub fn start(&mut self) { self.status = QuestStatus::Active; }
    pub fn check_completion(&mut self) { let done = self.objectives.iter().all(|o| match o { QuestObjectiveKind::KillEnemy { count, progress, .. } => progress >= count, QuestObjectiveKind::CollectItem { count, progress, .. } => progress >= count, QuestObjectiveKind::ReachLocation { reached, .. } => *reached, QuestObjectiveKind::TalkToNpc { talked, .. } => *talked, QuestObjectiveKind::ActivateObject { activated, .. } => *activated, QuestObjectiveKind::EscortNpc { reached, .. } => *reached, }); if done { self.status = QuestStatus::Completed; } }
    pub fn progress_percentage(&self) -> f32 { if self.objectives.is_empty() { return 100.0; } self.objectives.iter().map(|o| match o { QuestObjectiveKind::KillEnemy { count, progress, .. } => (*progress as f32 / *count as f32).min(1.0), QuestObjectiveKind::CollectItem { count, progress, .. } => (*progress as f32 / *count as f32).min(1.0), QuestObjectiveKind::ReachLocation { reached, .. } => if *reached { 1.0 } else { 0.0 }, QuestObjectiveKind::TalkToNpc { talked, .. } => if *talked { 1.0 } else { 0.0 }, QuestObjectiveKind::ActivateObject { activated, .. } => if *activated { 1.0 } else { 0.0 }, QuestObjectiveKind::EscortNpc { reached, .. } => if *reached { 1.0 } else { 0.0 }, }).sum::<f32>() / self.objectives.len() as f32 * 100.0 }
}
impl SpawnTable {
    pub fn new() -> Self { Self { entries: Vec::new(), total_weight: 0.0 } }
    pub fn add(&mut self, entry: SpawnTableEntry) { self.total_weight += entry.weight; self.entries.push(entry); }
}
impl TreasureChest {
    pub fn new(id: u32, x: i32, y: i32) -> Self { Self { id, position: (x, y), items: Vec::new(), gold_amount: 0, chest_type: String::from("wooden"), is_locked: false, key_id: None, is_trapped: false, trap_damage: 0, opened: false } }
    pub fn add_item(&mut self, item: TreasureItem) { self.items.push(item); }
    pub fn total_value(&self) -> u32 { self.gold_amount + self.items.iter().map(|i| i.base_value).sum::<u32>() }
    pub fn set_trap(&mut self, damage: u32) { self.is_trapped = true; self.trap_damage = damage; }
    pub fn open(&mut self) -> Vec<TreasureItem> { self.opened = true; self.items.drain(..).collect() }
}
impl WeatherTransition {
    pub fn new(from: WeatherType, to: WeatherType, duration_s: f32) -> Self { Self { from, to, duration_s, elapsed_s: 0.0 } }
    pub fn update(&mut self, dt: f32) { self.elapsed_s = (self.elapsed_s + dt).min(self.duration_s); }
    pub fn progress(&self) -> f32 { if self.duration_s <= 0.0 { 1.0 } else { self.elapsed_s / self.duration_s } }
    pub fn is_complete(&self) -> bool { self.elapsed_s >= self.duration_s }
    pub fn movement_speed_modifier(&self) -> f32 { match self.to { WeatherType::Snow | WeatherType::Thunderstorm => 0.7, WeatherType::Rain => 0.9, _ => 1.0 } }
}
impl WorldMap {
    pub fn new(name: &str, width: u32, height: u32) -> Self { Self { regions: Vec::new(), width, height, world_name: name.to_string(), starting_region: 0 } }
    pub fn add_region(&mut self, r: MapRegion) { self.regions.push(r); }
    pub fn discover_region(&mut self, id: u32) { if let Some(r) = self.regions.iter_mut().find(|r| r.id == id) { r.discovered = true; } }
    pub fn region_by_id(&self, id: u32) -> Option<&MapRegion> { self.regions.iter().find(|r| r.id == id) }
    pub fn discovered_regions(&self) -> Vec<&MapRegion> { self.regions.iter().filter(|r| r.discovered).collect() }
    pub fn find_region_path(&self, from: u32, to: u32) -> Vec<u32> { let mut visited = std::collections::HashSet::new(); let mut queue = std::collections::VecDeque::new(); queue.push_back(vec![from]); while let Some(path) = queue.pop_front() { let cur = *path.last().unwrap(); if cur == to { return path; } if visited.contains(&cur) { continue; } visited.insert(cur); if let Some(r) = self.regions.iter().find(|r| r.id == cur) { for &nb in &r.connected_regions { let mut np = path.clone(); np.push(nb); queue.push_back(np); } } } Vec::new() }
}
impl AStarPathfinder {
    pub fn find_path(_map: &TileMap, _db: &TileDatabase, _sx: u32, _sy: u32, _ex: u32, _ey: u32, _allow_diag: bool, _layer: usize) -> Option<Vec<(u32, u32)>> { Some(vec![(_sx, _sy), (_ex, _ey)]) }
}

impl Room {
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        let x = x as usize; let y = y as usize;
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}
impl LootTable {
    pub fn boss_hoard() -> Self { Self { id: 9001, name: String::from("BossHoard"), entries: Vec::new(), guaranteed_entries: Vec::new(), rolls_min: 3, rolls_max: 7 } }
    pub fn goblin_treasure() -> Self { Self { id: 9002, name: String::from("GoblinTreasure"), entries: Vec::new(), guaranteed_entries: Vec::new(), rolls_min: 1, rolls_max: 3 } }
    pub fn total_weight(&self) -> f32 { self.entries.iter().map(|e| e.weight as f32).sum() }
}
impl DungeonTheme {
    pub fn crypt() -> Self { Self { name: String::from("Crypt"), wall_tile_ids: vec![10,11], floor_tile_ids: vec![12,13], door_tile_id: 14, chest_frequency: 0.1, trap_frequency: 0.2, enemy_types: vec![String::from("Skeleton"),String::from("Zombie")], ambient_light: (20,20,30), torch_frequency: 0.3, special_tiles: std::collections::HashMap::new(), music_track: String::from("crypt_ambient") } }
    pub fn random_wall_tile(&self, rng: &mut MapRng) -> u32 { rng.next_u64(); self.wall_tile_ids[rng.state as usize % self.wall_tile_ids.len()] }
}


// ============================================================
// DUNGEON ROOM GENERATOR
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ExtRoomShape {
    Rectangular,
    LShape,
    TShape,
    CrossShape,
    Circular,
    Irregular,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoomTheme {
    Stone,
    Wood,
    Cave,
    Crypt,
    Library,
    Throne,
    Dungeon,
    Sewers,
    Ruins,
    Temple,
}

#[derive(Debug, Clone)]
pub struct ExtDungeonRoom {
    pub id: u32,
    pub name: String,
    pub shape: ExtRoomShape,
    pub theme: RoomTheme,
    pub width: u32,
    pub height: u32,
    pub position: Vec2,
    pub connections: Vec<u32>,
    pub loot_table_id: Option<u32>,
    pub monster_group_id: Option<u32>,
    pub trap_ids: Vec<u32>,
    pub secret: bool,
    pub boss_room: bool,
    pub visited: bool,
    pub light_level: f32,
    pub ambient_sound: Option<String>,
}

impl ExtDungeonRoom {
    pub fn new(id: u32, shape: ExtRoomShape, theme: RoomTheme, w: u32, h: u32) -> Self {
        ExtDungeonRoom {
            id,
            name: format!("Room {}", id),
            shape, theme, width: w, height: h,
            position: Vec2::ZERO,
            connections: Vec::new(),
            loot_table_id: None,
            monster_group_id: None,
            trap_ids: Vec::new(),
            secret: false,
            boss_room: false,
            visited: false,
            light_level: 0.5,
            ambient_sound: None,
        }
    }

    pub fn area(&self) -> f32 {
        match self.shape {
            ExtRoomShape::Rectangular => (self.width * self.height) as f32,
            ExtRoomShape::LShape => (self.width * self.height) as f32 * 0.75,
            ExtRoomShape::TShape => (self.width * self.height) as f32 * 0.75,
            ExtRoomShape::CrossShape => (self.width * self.height) as f32 * 0.6,
            ExtRoomShape::Circular => {
                let r = self.width.min(self.height) as f32 * 0.5;
                std::f32::consts::PI * r * r
            },
            ExtRoomShape::Irregular => (self.width * self.height) as f32 * 0.65,
        }
    }

    pub fn connect(&mut self, other_id: u32) {
        if !self.connections.contains(&other_id) {
            self.connections.push(other_id);
        }
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        (self.position, self.position + Vec2::new(self.width as f32, self.height as f32))
    }

    pub fn center(&self) -> Vec2 {
        self.position + Vec2::new(self.width as f32 * 0.5, self.height as f32 * 0.5)
    }
}

#[derive(Debug, Clone)]
pub struct DungeonFloor {
    pub floor_number: i32,
    pub rooms: HashMap<u32, ExtDungeonRoom>,
    pub corridors: Vec<(u32, u32)>,
    pub entrance_room_id: u32,
    pub exit_room_id: u32,
    pub width: u32,
    pub height: u32,
    pub difficulty: f32,
}

impl DungeonFloor {
    pub fn new(floor_number: i32, width: u32, height: u32) -> Self {
        DungeonFloor {
            floor_number, rooms: HashMap::new(),
            corridors: Vec::new(),
            entrance_room_id: 0, exit_room_id: 0,
            width, height, difficulty: 1.0,
        }
    }

    pub fn add_room(&mut self, room: ExtDungeonRoom) {
        self.rooms.insert(room.id, room);
    }

    pub fn connect_rooms(&mut self, a: u32, b: u32) {
        if let Some(room_a) = self.rooms.get_mut(&a) {
            room_a.connect(b);
        }
        if let Some(room_b) = self.rooms.get_mut(&b) {
            room_b.connect(a);
        }
        self.corridors.push((a, b));
    }

    pub fn room_count(&self) -> usize { self.rooms.len() }

    pub fn boss_rooms(&self) -> Vec<&ExtDungeonRoom> {
        self.rooms.values().filter(|r| r.boss_room).collect()
    }

    pub fn secret_rooms(&self) -> Vec<&ExtDungeonRoom> {
        self.rooms.values().filter(|r| r.secret).collect()
    }

    pub fn bfs_distance(&self, from: u32, to: u32) -> Option<u32> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((from, 0));
        visited.insert(from);
        while let Some((curr, dist)) = queue.pop_front() {
            if curr == to { return Some(dist); }
            if let Some(room) = self.rooms.get(&curr) {
                for &next in &room.connections {
                    if !visited.contains(&next) {
                        visited.insert(next);
                        queue.push_back((next, dist + 1));
                    }
                }
            }
        }
        None
    }
}

// ============================================================
// PROCEDURAL DUNGEON GENERATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtDungeonGenParams {
    pub width: u32,
    pub height: u32,
    pub min_rooms: u32,
    pub max_rooms: u32,
    pub min_room_size: u32,
    pub max_room_size: u32,
    pub corridor_width: u32,
    pub boss_room_probability: f32,
    pub secret_room_probability: f32,
    pub seed: u64,
}

impl Default for ExtDungeonGenParams {
    fn default() -> Self {
        ExtDungeonGenParams {
            width: 100, height: 100,
            min_rooms: 8, max_rooms: 20,
            min_room_size: 5, max_room_size: 15,
            corridor_width: 2,
            boss_room_probability: 0.1,
            secret_room_probability: 0.15,
            seed: 12345,
        }
    }
}

pub struct ExtDungeonGenerator {
    pub params: ExtDungeonGenParams,
    pub rng_state: u64,
}

impl ExtDungeonGenerator {
    pub fn new(params: ExtDungeonGenParams) -> Self {
        let seed = params.seed;
        ExtDungeonGenerator { params, rng_state: seed }
    }

    fn next_rand(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    fn rand_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max { return min; }
        (self.next_rand() as u32 % (max - min)) + min
    }

    fn rand_f32(&mut self) -> f32 {
        (self.next_rand() & 0xFFFFFF) as f32 / 0xFFFFFF as f32
    }

    pub fn generate(&mut self, floor_number: i32) -> DungeonFloor {
        let mut floor = DungeonFloor::new(floor_number, self.params.width, self.params.height);
        let num_rooms = self.rand_range(self.params.min_rooms, self.params.max_rooms);

        for i in 0..num_rooms {
            let w = self.rand_range(self.params.min_room_size, self.params.max_room_size);
            let h = self.rand_range(self.params.min_room_size, self.params.max_room_size);
            let x = self.rand_range(1, self.params.width.saturating_sub(w + 1));
            let y = self.rand_range(1, self.params.height.saturating_sub(h + 1));
            let theme_idx = (self.next_rand() % 10) as usize;
            let themes = [
                RoomTheme::Stone, RoomTheme::Wood, RoomTheme::Cave,
                RoomTheme::Crypt, RoomTheme::Library, RoomTheme::Throne,
                RoomTheme::Dungeon, RoomTheme::Sewers, RoomTheme::Ruins, RoomTheme::Temple,
            ];
            let mut room = ExtDungeonRoom::new(i, ExtRoomShape::Rectangular, themes[theme_idx].clone(), w, h);
            room.position = Vec2::new(x as f32, y as f32);
            room.boss_room = self.rand_f32() < self.params.boss_room_probability;
            room.secret = self.rand_f32() < self.params.secret_room_probability;
            floor.add_room(room);
        }

        // Connect rooms in a simple chain
        let room_ids: Vec<u32> = floor.rooms.keys().copied().collect();
        for i in 1..room_ids.len() {
            floor.connect_rooms(room_ids[i-1], room_ids[i]);
        }
        if !room_ids.is_empty() {
            floor.entrance_room_id = room_ids[0];
            floor.exit_room_id = *room_ids.last().unwrap();
        }
        floor
    }
}

// ============================================================
// WORLD MAP REGION SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ExtBiomeType {
    Plains,
    Forest,
    Desert,
    Tundra,
    Swamp,
    Mountains,
    Jungle,
    Coast,
    Ocean,
    Volcanic,
    Magical,
    Corrupted,
}

#[derive(Debug, Clone)]
pub struct WorldRegion {
    pub id: u32,
    pub name: String,
    pub biome: ExtBiomeType,
    pub center: Vec2,
    pub radius: f32,
    pub elevation: f32,
    pub temperature: f32,
    pub rainfall: f32,
    pub danger_level: u32,
    pub discovered: bool,
    pub controlled_by_faction: Option<u32>,
    pub resources: HashMap<String, f32>,
    pub adjacent_region_ids: Vec<u32>,
    pub dungeons: Vec<u32>,
    pub settlements: Vec<u32>,
}

impl WorldRegion {
    pub fn new(id: u32, name: &str, biome: ExtBiomeType) -> Self {
        WorldRegion {
            id, name: name.to_string(), biome,
            center: Vec2::ZERO, radius: 50.0,
            elevation: 0.0, temperature: 15.0, rainfall: 500.0,
            danger_level: 1, discovered: false,
            controlled_by_faction: None,
            resources: HashMap::new(),
            adjacent_region_ids: Vec::new(),
            dungeons: Vec::new(), settlements: Vec::new(),
        }
    }

    pub fn is_habitable(&self) -> bool {
        self.temperature > -20.0 && self.temperature < 45.0
            && self.rainfall > 100.0
            && !matches!(self.biome, ExtBiomeType::Volcanic | ExtBiomeType::Corrupted)
    }

    pub fn traversal_cost(&self) -> f32 {
        match self.biome {
            ExtBiomeType::Plains => 1.0,
            ExtBiomeType::Forest => 1.5,
            ExtBiomeType::Desert => 2.0,
            ExtBiomeType::Tundra => 2.5,
            ExtBiomeType::Swamp => 3.0,
            ExtBiomeType::Mountains => 4.0,
            ExtBiomeType::Jungle => 3.5,
            ExtBiomeType::Coast => 1.2,
            ExtBiomeType::Ocean => 5.0,
            ExtBiomeType::Volcanic => 6.0,
            ExtBiomeType::Magical => 1.8,
            ExtBiomeType::Corrupted => 3.0,
        }
    }

    pub fn add_resource(&mut self, name: &str, amount: f32) {
        *self.resources.entry(name.to_string()).or_insert(0.0) += amount;
    }
}

#[derive(Debug, Clone)]
pub struct ExtWorldMap {
    pub name: String,
    pub regions: HashMap<u32, WorldRegion>,
    pub width: f32,
    pub height: f32,
    pub sea_level: f32,
    pub calendar: String,
}

impl ExtWorldMap {
    pub fn new(name: &str, w: f32, h: f32) -> Self {
        ExtWorldMap {
            name: name.to_string(),
            regions: HashMap::new(),
            width: w, height: h,
            sea_level: 0.0,
            calendar: "Imperial Calendar".to_string(),
        }
    }

    pub fn add_region(&mut self, region: WorldRegion) {
        self.regions.insert(region.id, region);
    }

    pub fn regions_near(&self, pos: Vec2, radius: f32) -> Vec<&WorldRegion> {
        self.regions.values()
            .filter(|r| (r.center - pos).length() <= radius)
            .collect()
    }

    pub fn region_at(&self, pos: Vec2) -> Option<&WorldRegion> {
        self.regions.values()
            .filter(|r| (r.center - pos).length() <= r.radius)
            .min_by(|a, b| {
                (a.center - pos).length().partial_cmp(&(b.center - pos).length()).unwrap()
            })
    }

    pub fn pathfind_regions(&self, from: u32, to: u32) -> Option<Vec<u32>> {
        if from == to { return Some(vec![from]); }
        let mut dist: HashMap<u32, f32> = HashMap::new();
        let mut prev: HashMap<u32, u32> = HashMap::new();
        let mut unvisited: HashSet<u32> = self.regions.keys().copied().collect();
        dist.insert(from, 0.0);
        for &id in &unvisited {
            if id != from { dist.insert(id, f32::INFINITY); }
        }
        while !unvisited.is_empty() {
            let curr = *unvisited.iter()
                .min_by(|&&a, &&b| dist[&a].partial_cmp(&dist[&b]).unwrap())?;
            if curr == to { break; }
            let curr_dist = dist[&curr];
            if curr_dist.is_infinite() { break; }
            unvisited.remove(&curr);
            if let Some(region) = self.regions.get(&curr) {
                for &adj in &region.adjacent_region_ids {
                    if !unvisited.contains(&adj) { continue; }
                    if let Some(adj_region) = self.regions.get(&adj) {
                        let new_dist = curr_dist + adj_region.traversal_cost();
                        if new_dist < *dist.get(&adj).unwrap_or(&f32::INFINITY) {
                            dist.insert(adj, new_dist);
                            prev.insert(adj, curr);
                        }
                    }
                }
            }
        }
        let mut path = Vec::new();
        let mut curr = to;
        while let Some(&p) = prev.get(&curr) {
            path.push(curr);
            curr = p;
        }
        path.push(from);
        path.reverse();
        if path[0] == from { Some(path) } else { None }
    }
}

// ============================================================
// FACTION AND POLITICS SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum FactionType {
    Kingdom,
    Empire,
    Republic,
    Tribe,
    Guild,
    Church,
    Cult,
    MercenaryBand,
    WizardOrder,
    Criminal,
}

#[derive(Debug, Clone)]
pub struct Faction {
    pub id: u32,
    pub name: String,
    pub faction_type: FactionType,
    pub leader_name: String,
    pub influence: f32,
    pub military_strength: f32,
    pub wealth: f32,
    pub reputation: f32,
    pub controlled_regions: Vec<u32>,
    pub relations: HashMap<u32, f32>,
    pub description: String,
    pub color: Vec4,
}

impl Faction {
    pub fn new(id: u32, name: &str, faction_type: FactionType) -> Self {
        Faction {
            id, name: name.to_string(), faction_type,
            leader_name: String::new(),
            influence: 50.0, military_strength: 50.0,
            wealth: 1000.0, reputation: 0.0,
            controlled_regions: Vec::new(),
            relations: HashMap::new(),
            description: String::new(),
            color: Vec4::new(0.5, 0.5, 0.5, 1.0),
        }
    }

    pub fn relation_to(&self, other_id: u32) -> f32 {
        self.relations.get(&other_id).copied().unwrap_or(0.0)
    }

    pub fn is_allied_with(&self, other_id: u32) -> bool {
        self.relation_to(other_id) >= 50.0
    }

    pub fn is_enemy_of(&self, other_id: u32) -> bool {
        self.relation_to(other_id) <= -50.0
    }

    pub fn set_relation(&mut self, other_id: u32, value: f32) {
        self.relations.insert(other_id, value.clamp(-100.0, 100.0));
    }

    pub fn total_power(&self) -> f32 {
        self.influence * 0.3 + self.military_strength * 0.5 + self.wealth * 0.0001 * 0.2
    }
}

// ============================================================
// SETTLEMENT / CITY SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SettlementType {
    Hamlet,
    Village,
    Town,
    City,
    Metropolis,
    Fortress,
    Port,
    Capital,
    Ruin,
}

#[derive(Debug, Clone)]
pub struct Settlement {
    pub id: u32,
    pub name: String,
    pub settlement_type: SettlementType,
    pub position: Vec2,
    pub population: u32,
    pub faction_id: Option<u32>,
    pub services: Vec<String>,
    pub prosperity: f32,
    pub defense_rating: f32,
    pub trade_goods: HashMap<String, f32>,
    pub quests_available: u32,
    pub inn_quality: u32,
}

impl Settlement {
    pub fn new(id: u32, name: &str, st: SettlementType) -> Self {
        Settlement {
            id, name: name.to_string(), settlement_type: st,
            position: Vec2::ZERO, population: 100,
            faction_id: None,
            services: Vec::new(),
            prosperity: 50.0, defense_rating: 10.0,
            trade_goods: HashMap::new(),
            quests_available: 0, inn_quality: 2,
        }
    }

    pub fn has_service(&self, service: &str) -> bool {
        self.services.iter().any(|s| s == service)
    }

    pub fn is_major_city(&self) -> bool {
        matches!(self.settlement_type, SettlementType::City | SettlementType::Metropolis | SettlementType::Capital)
    }

    pub fn add_service(&mut self, service: &str) {
        if !self.has_service(service) {
            self.services.push(service.to_string());
        }
    }
}

// ============================================================
// QUEST SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum QuestType {
    MainStory,
    SideQuest,
    Bounty,
    Fetch,
    Escort,
    Explore,
    Defend,
    Assassinate,
    Investigate,
    Craft,
    Delivery,
}


#[derive(Debug, Clone)]
pub struct QuestObjective {
    pub id: u32,
    pub description: String,
    pub current: u32,
    pub required: u32,
    pub optional: bool,
    pub complete: bool,
}

impl QuestObjective {
    pub fn progress_pct(&self) -> f32 {
        if self.required == 0 { return 1.0; }
        self.current as f32 / self.required as f32
    }
}

#[derive(Debug, Clone)]
pub struct ExtQuestReward {
    pub experience: u32,
    pub gold: u32,
    pub items: Vec<String>,
    pub reputation_changes: HashMap<u32, f32>,
    pub unlock_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExtQuest {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub quest_type: QuestType,
    pub status: QuestStatus,
    pub level_requirement: u32,
    pub objectives: Vec<QuestObjective>,
    pub rewards: ExtQuestReward,
    pub giver_id: Option<u32>,
    pub location_hint: Option<Vec2>,
    pub time_limit_seconds: Option<f32>,
    pub prerequisite_quest_ids: Vec<u32>,
    pub tags: Vec<String>,
}

impl ExtQuest {
    pub fn new(id: u32, name: &str, quest_type: QuestType) -> Self {
        ExtQuest {
            id, name: name.to_string(), description: String::new(),
            quest_type, status: QuestStatus::NotStarted,
            level_requirement: 1,
            objectives: Vec::new(),
            rewards: ExtQuestReward { experience: 0, gold: 0, items: Vec::new(), reputation_changes: HashMap::new(), unlock_ids: Vec::new() },
            giver_id: None, location_hint: None, time_limit_seconds: None,
            prerequisite_quest_ids: Vec::new(), tags: Vec::new(),
        }
    }

    pub fn add_objective(&mut self, obj: QuestObjective) {
        self.objectives.push(obj);
    }

    pub fn is_complete(&self) -> bool {
        self.objectives.iter()
            .filter(|o| !o.optional)
            .all(|o| o.complete)
    }

    pub fn overall_progress(&self) -> f32 {
        let required: Vec<&QuestObjective> = self.objectives.iter().filter(|o| !o.optional).collect();
        if required.is_empty() { return 1.0; }
        required.iter().map(|o| o.progress_pct()).sum::<f32>() / required.len() as f32
    }

    pub fn start(&mut self) {
        if self.status == QuestStatus::NotStarted {
            self.status = QuestStatus::Active;
        }
    }

    pub fn complete_quest(&mut self) {
        self.status = QuestStatus::Completed;
    }
}

// ============================================================
// ENCOUNTER / COMBAT SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum EncounterDifficulty {
    Trivial, Easy, Medium, Hard, Deadly, Legendary,
}

#[derive(Debug, Clone)]
pub struct EncounterEnemy {
    pub monster_id: u32,
    pub count: u32,
    pub level_override: Option<u32>,
    pub loot_bonus: f32,
}

#[derive(Debug, Clone)]
pub struct Encounter {
    pub id: u32,
    pub name: String,
    pub difficulty: EncounterDifficulty,
    pub enemies: Vec<EncounterEnemy>,
    pub location_id: u32,
    pub trigger_radius: f32,
    pub respawn_seconds: Option<f32>,
    pub is_scripted: bool,
    pub ambush: bool,
    pub xp_multiplier: f32,
}

impl Encounter {
    pub fn new(id: u32, name: &str, difficulty: EncounterDifficulty, location_id: u32) -> Self {
        Encounter {
            id, name: name.to_string(), difficulty,
            enemies: Vec::new(), location_id,
            trigger_radius: 5.0,
            respawn_seconds: Some(300.0),
            is_scripted: false, ambush: false, xp_multiplier: 1.0,
        }
    }

    pub fn total_enemy_count(&self) -> u32 {
        self.enemies.iter().map(|e| e.count).sum()
    }

    pub fn threat_rating(&self) -> f32 {
        let base = match self.difficulty {
            EncounterDifficulty::Trivial => 1.0,
            EncounterDifficulty::Easy => 2.0,
            EncounterDifficulty::Medium => 4.0,
            EncounterDifficulty::Hard => 8.0,
            EncounterDifficulty::Deadly => 16.0,
            EncounterDifficulty::Legendary => 32.0,
        };
        base * self.total_enemy_count() as f32 * self.xp_multiplier
    }
}

// ============================================================
// MAP EDITOR TOOLS
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MapToolMode {
    Select,
    PlaceRoom,
    PlaceCorridor,
    PlaceObject,
    Paint,
    Erase,
    FloodFill,
    MeasureDistance,
    PlaceSpawn,
    PlaceWaypoint,
    EditLight,
    EditTrigger,
}

#[derive(Debug, Clone)]
pub struct MapEditorState {
    pub tool_mode: MapToolMode,
    pub selected_room_ids: HashSet<u32>,
    pub brush_size: f32,
    pub paint_tile_id: u32,
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub show_grid: bool,
    pub show_collision: bool,
    pub show_navmesh: bool,
    pub show_lighting: bool,
    pub zoom: f32,
    pub camera_pos: Vec2,
    pub undo_stack: VecDeque<String>,
    pub redo_stack: VecDeque<String>,
    pub max_undo: usize,
}

impl MapEditorState {
    pub fn new() -> Self {
        MapEditorState {
            tool_mode: MapToolMode::Select,
            selected_room_ids: HashSet::new(),
            brush_size: 1.0,
            paint_tile_id: 0,
            grid_size: 1.0,
            snap_to_grid: true,
            show_grid: true,
            show_collision: false,
            show_navmesh: false,
            show_lighting: true,
            zoom: 1.0,
            camera_pos: Vec2::ZERO,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_undo: 100,
        }
    }

    pub fn push_undo(&mut self, description: String) {
        if self.undo_stack.len() >= self.max_undo {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(description);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<String> {
        if let Some(action) = self.undo_stack.pop_back() {
            self.redo_stack.push_back(action.clone());
            Some(action)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<String> {
        if let Some(action) = self.redo_stack.pop_back() {
            self.undo_stack.push_back(action.clone());
            Some(action)
        } else {
            None
        }
    }

    pub fn snap_position(&self, pos: Vec2) -> Vec2 {
        if !self.snap_to_grid { return pos; }
        let g = self.grid_size;
        Vec2::new((pos.x / g).round() * g, (pos.y / g).round() * g)
    }

    pub fn world_to_screen(&self, world_pos: Vec2, viewport_size: Vec2) -> Vec2 {
        (world_pos - self.camera_pos) * self.zoom + viewport_size * 0.5
    }

    pub fn screen_to_world(&self, screen_pos: Vec2, viewport_size: Vec2) -> Vec2 {
        (screen_pos - viewport_size * 0.5) / self.zoom + self.camera_pos
    }
}

// ============================================================
// NAVMESH GENERATION (SIMPLIFIED 2D)
// ============================================================

#[derive(Debug, Clone)]
pub struct NavPoly {
    pub id: u32,
    pub vertices: Vec<Vec2>,
    pub neighbors: Vec<u32>,
    pub walkable: bool,
    pub cost: f32,
    pub flags: u32,
}

impl NavPoly {
    pub fn new(id: u32) -> Self {
        NavPoly { id, vertices: Vec::new(), neighbors: Vec::new(), walkable: true, cost: 1.0, flags: 0 }
    }

    pub fn center(&self) -> Vec2 {
        if self.vertices.is_empty() { return Vec2::ZERO; }
        let sum = self.vertices.iter().fold(Vec2::ZERO, |a, &b| a + b);
        sum / self.vertices.len() as f32
    }

    pub fn area(&self) -> f32 {
        let n = self.vertices.len();
        if n < 3 { return 0.0; }
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += self.vertices[i].x * self.vertices[j].y;
            area -= self.vertices[j].x * self.vertices[i].y;
        }
        area.abs() * 0.5
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        let n = self.vertices.len();
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let vi = self.vertices[i];
            let vj = self.vertices[j];
            if ((vi.y > p.y) != (vj.y > p.y))
                && (p.x < (vj.x - vi.x) * (p.y - vi.y) / (vj.y - vi.y) + vi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

#[derive(Debug, Clone)]
pub struct NavMesh2D {
    pub polys: HashMap<u32, NavPoly>,
    pub next_id: u32,
}

impl NavMesh2D {
    pub fn new() -> Self {
        NavMesh2D { polys: HashMap::new(), next_id: 0 }
    }

    pub fn add_poly(&mut self, poly: NavPoly) {
        let id = poly.id;
        self.polys.insert(id, poly);
        if id >= self.next_id { self.next_id = id + 1; }
    }

    pub fn poly_at_point(&self, p: Vec2) -> Option<&NavPoly> {
        self.polys.values().find(|poly| poly.walkable && poly.contains_point(p))
    }

    pub fn astar_path(&self, start: Vec2, end: Vec2) -> Option<Vec<Vec2>> {
        let start_poly = self.poly_at_point(start)?;
        let end_poly = self.poly_at_point(end)?;
        if start_poly.id == end_poly.id {
            return Some(vec![start, end]);
        }
        let mut open: Vec<u32> = vec![start_poly.id];
        let mut came_from: HashMap<u32, u32> = HashMap::new();
        let mut g_score: HashMap<u32, f32> = HashMap::new();
        g_score.insert(start_poly.id, 0.0);
        while !open.is_empty() {
            let curr = *open.iter().min_by(|&&a, &&b| {
                let fa = g_score.get(&a).unwrap_or(&f32::INFINITY);
                let fb = g_score.get(&b).unwrap_or(&f32::INFINITY);
                fa.partial_cmp(fb).unwrap()
            })?;
            if curr == end_poly.id {
                let mut path_polys = vec![curr];
                let mut c = curr;
                while let Some(&p) = came_from.get(&c) {
                    path_polys.push(p);
                    c = p;
                }
                path_polys.reverse();
                let mut waypoints: Vec<Vec2> = path_polys.iter()
                    .filter_map(|id| self.polys.get(id))
                    .map(|p| p.center())
                    .collect();
                if !waypoints.is_empty() { waypoints[0] = start; }
                waypoints.push(end);
                return Some(waypoints);
            }
            open.retain(|&id| id != curr);
            if let Some(poly) = self.polys.get(&curr) {
                let curr_g = *g_score.get(&curr).unwrap_or(&f32::INFINITY);
                for &nbr in &poly.neighbors {
                    let nbr_poly = match self.polys.get(&nbr) { Some(p) => p, None => continue };
                    if !nbr_poly.walkable { continue; }
                    let tentative = curr_g + nbr_poly.cost
                        + (nbr_poly.center() - poly.center()).length();
                    if tentative < *g_score.get(&nbr).unwrap_or(&f32::INFINITY) {
                        g_score.insert(nbr, tentative);
                        came_from.insert(nbr, curr);
                        if !open.contains(&nbr) { open.push(nbr); }
                    }
                }
            }
        }
        None
    }
}

// ============================================================
// TILE PALETTE AND TILESET
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtTileDef {
    pub id: u32,
    pub name: String,
    pub sprite_x: u32,
    pub sprite_y: u32,
    pub sprite_width: u32,
    pub sprite_height: u32,
    pub passable: bool,
    pub blocks_sight: bool,
    pub light_transmission: f32,
    pub movement_cost: f32,
    pub tags: Vec<String>,
    pub autotile_group: Option<u32>,
}

impl ExtTileDef {
    pub fn new(id: u32, name: &str) -> Self {
        ExtTileDef {
            id, name: name.to_string(),
            sprite_x: 0, sprite_y: 0,
            sprite_width: 16, sprite_height: 16,
            passable: true, blocks_sight: false,
            light_transmission: 1.0,
            movement_cost: 1.0,
            tags: Vec::new(),
            autotile_group: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtTileset {
    pub id: u32,
    pub name: String,
    pub texture_path: String,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles: HashMap<u32, ExtTileDef>,
}

impl ExtTileset {
    pub fn new(id: u32, name: &str, texture: &str, tw: u32, th: u32) -> Self {
        ExtTileset { id, name: name.to_string(), texture_path: texture.to_string(), tile_width: tw, tile_height: th, tiles: HashMap::new() }
    }

    pub fn add_tile(&mut self, tile: ExtTileDef) {
        self.tiles.insert(tile.id, tile);
    }

    pub fn tile_by_name(&self, name: &str) -> Option<&ExtTileDef> {
        self.tiles.values().find(|t| t.name == name)
    }
}

// ============================================================
// MAP OBJECT PLACEMENT
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MapObjectType {
    Door, Chest, Trap, Altar, Statue, Torch, Barrel, Bookshelf,
    Bed, Chair, Table, Pillar, Wall, Floor, Ceiling,
    NpcSpawn, EnemySpawn, BossSpawn, PlayerSpawn,
    Portal, Trigger, Light, Sound, Particle,
    Lever, Button, Pressure_Plate, Key,
}

#[derive(Debug, Clone)]
pub struct MapObject {
    pub id: u32,
    pub object_type: MapObjectType,
    pub position: Vec3,
    pub rotation_deg: f32,
    pub scale: Vec3,
    pub asset_id: String,
    pub properties: HashMap<String, String>,
    pub interactive: bool,
    pub visible: bool,
    pub layer: u32,
}

impl MapObject {
    pub fn new(id: u32, object_type: MapObjectType, pos: Vec3, asset: &str) -> Self {
        MapObject {
            id, object_type, position: pos,
            rotation_deg: 0.0, scale: Vec3::ONE,
            asset_id: asset.to_string(),
            properties: HashMap::new(),
            interactive: false, visible: true, layer: 0,
        }
    }

    pub fn set_property(&mut self, key: &str, value: &str) {
        self.properties.insert(key.to_string(), value.to_string());
    }

    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    pub fn transform_matrix(&self) -> Mat4 {
        let rot = Mat4::from_rotation_y(self.rotation_deg.to_radians());
        let trans = Mat4::from_translation(self.position);
        let scale = Mat4::from_scale(self.scale);
        trans * rot * scale
    }
}

// ============================================================
// LIGHT BAKING / RADIOSITY (SIMPLIFIED)
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LightType {
    Point, Spot, Directional, Area, Ambient,
}

#[derive(Debug, Clone)]
pub struct SceneLight {
    pub id: u32,
    pub light_type: LightType,
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius: f32,
    pub inner_cone_deg: f32,
    pub outer_cone_deg: f32,
    pub cast_shadows: bool,
    pub shadow_bias: f32,
    pub baked: bool,
    pub static_light: bool,
}

impl SceneLight {
    pub fn new_point(id: u32, pos: Vec3, color: Vec3, intensity: f32, radius: f32) -> Self {
        SceneLight {
            id, light_type: LightType::Point, position: pos,
            direction: Vec3::NEG_Y, color, intensity, radius,
            inner_cone_deg: 30.0, outer_cone_deg: 45.0,
            cast_shadows: true, shadow_bias: 0.005,
            baked: false, static_light: false,
        }
    }

    pub fn new_directional(id: u32, dir: Vec3, color: Vec3, intensity: f32) -> Self {
        SceneLight {
            id, light_type: LightType::Directional, position: Vec3::ZERO,
            direction: dir.normalize(), color, intensity, radius: f32::INFINITY,
            inner_cone_deg: 0.0, outer_cone_deg: 180.0,
            cast_shadows: true, shadow_bias: 0.002,
            baked: false, static_light: true,
        }
    }

    pub fn attenuation(&self, distance: f32) -> f32 {
        match self.light_type {
            LightType::Point | LightType::Spot => {
                let d = distance.max(0.001);
                (1.0 - (d / self.radius).clamp(0.0, 1.0)).powi(2)
            },
            LightType::Directional | LightType::Ambient | LightType::Area => 1.0,
        }
    }

    pub fn illuminate(&self, point: Vec3, normal: Vec3) -> Vec3 {
        match self.light_type {
            LightType::Directional => {
                let ndotl = normal.dot(-self.direction).max(0.0);
                self.color * self.intensity * ndotl
            },
            LightType::Point => {
                let diff = self.position - point;
                let dist = diff.length();
                let dir = diff / dist.max(0.001);
                let ndotl = normal.dot(dir).max(0.0);
                let att = self.attenuation(dist);
                self.color * self.intensity * ndotl * att
            },
            _ => Vec3::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LightmapPatch {
    pub position: Vec3,
    pub normal: Vec3,
    pub baked_color: Vec3,
    pub direct_color: Vec3,
    pub indirect_color: Vec3,
}

pub struct LightBaker {
    pub lights: Vec<SceneLight>,
    pub ambient: Vec3,
    pub num_bounces: u32,
}

impl LightBaker {
    pub fn new(ambient: Vec3) -> Self {
        LightBaker { lights: Vec::new(), ambient, num_bounces: 2 }
    }

    pub fn bake_patch(&self, patch: &LightmapPatch) -> Vec3 {
        let mut total = self.ambient;
        for light in &self.lights {
            total += light.illuminate(patch.position, patch.normal);
        }
        total
    }

    pub fn bake_patches(&self, patches: &mut Vec<LightmapPatch>) {
        for patch in patches.iter_mut() {
            patch.baked_color = self.bake_patch(patch);
        }
    }
}

// ============================================================
// TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_map_additions {
    use super::*;

    #[test]
    fn test_dungeon_room_area() {
        let room = ExtDungeonRoom::new(1, ExtRoomShape::Rectangular, RoomTheme::Stone, 10, 8);
        assert_eq!(room.area(), 80.0);
    }

    #[test]
    fn test_dungeon_floor_bfs() {
        let mut floor = DungeonFloor::new(1, 100, 100);
        floor.add_room(ExtDungeonRoom::new(0, ExtRoomShape::Rectangular, RoomTheme::Stone, 5, 5));
        floor.add_room(ExtDungeonRoom::new(1, ExtRoomShape::Rectangular, RoomTheme::Stone, 5, 5));
        floor.add_room(ExtDungeonRoom::new(2, ExtRoomShape::Rectangular, RoomTheme::Stone, 5, 5));
        floor.connect_rooms(0, 1);
        floor.connect_rooms(1, 2);
        assert_eq!(floor.bfs_distance(0, 2), Some(2));
        assert_eq!(floor.bfs_distance(0, 0), Some(0));
    }

    #[test]
    fn test_dungeon_generator() {
        let params = ExtDungeonGenParams { min_rooms: 5, max_rooms: 10, ..Default::default() };
        let mut gen = ExtDungeonGenerator::new(params);
        let floor = gen.generate(1);
        assert!(floor.room_count() >= 5);
        assert!(floor.room_count() <= 10);
    }

    #[test]
    fn test_world_region_traversal() {
        let plains = WorldRegion::new(1, "Greenfield", ExtBiomeType::Plains);
        let mountains = WorldRegion::new(2, "Stonepeaks", ExtBiomeType::Mountains);
        assert!(plains.traversal_cost() < mountains.traversal_cost());
        assert!(plains.is_habitable());
    }

    #[test]
    fn test_faction_relations() {
        let mut f = Faction::new(1, "Kingdom of Arath", FactionType::Kingdom);
        f.set_relation(2, 75.0);
        assert!(f.is_allied_with(2));
        f.set_relation(3, -80.0);
        assert!(f.is_enemy_of(3));
    }

    #[test]
    fn test_quest_progress() {
        let mut q = ExtQuest::new(1, "Kill 10 Wolves", QuestType::Bounty);
        q.add_objective(QuestObjective { id: 0, description: "Kill wolves".to_string(), current: 7, required: 10, optional: false, complete: false });
        assert!((q.overall_progress() - 0.7).abs() < 0.01);
        assert!(!q.is_complete());
    }

    #[test]
    fn test_nav_poly_contains() {
        let mut poly = NavPoly::new(0);
        poly.vertices = vec![
            Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0),
        ];
        assert!(poly.contains_point(Vec2::new(2.0, 2.0)));
        assert!(!poly.contains_point(Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn test_map_editor_snap() {
        let state = MapEditorState::new();
        let snapped = state.snap_position(Vec2::new(1.4, 2.7));
        assert!((snapped.x - 1.0).abs() < 0.001);
        assert!((snapped.y - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_light_attenuation() {
        let light = SceneLight::new_point(1, Vec3::ZERO, Vec3::ONE, 100.0, 10.0);
        let att_near = light.attenuation(1.0);
        let att_far = light.attenuation(9.0);
        assert!(att_near > att_far);
    }

    #[test]
    fn test_encounter_threat() {
        let mut enc = Encounter::new(1, "Wolf Pack", EncounterDifficulty::Medium, 5);
        enc.enemies.push(EncounterEnemy { monster_id: 10, count: 5, level_override: None, loot_bonus: 1.0 });
        assert_eq!(enc.total_enemy_count(), 5);
        assert!(enc.threat_rating() > 0.0);
    }
}

pub fn map_editor_module_version() -> &'static str { "2.5.0" }
pub fn map_editor_features() -> &'static [&'static str] {
    &["dungeon_gen", "world_map", "factions", "quests", "encounters", "navmesh", "lighting", "tiles"]
}


// ============================================================
// WEATHER AND ENVIRONMENT SYSTEM
// ============================================================


#[derive(Debug, Clone)]
pub struct WeatherState {
    pub weather_type: WeatherType,
    pub temperature_celsius: f32,
    pub wind_speed_kph: f32,
    pub wind_direction_deg: f32,
    pub humidity_pct: f32,
    pub visibility_km: f32,
    pub precipitation_mm_hr: f32,
    pub cloud_cover_pct: f32,
    pub fog_density: f32,
}

impl WeatherState {
    pub fn clear_day() -> Self {
        WeatherState {
            weather_type: WeatherType::Clear,
            temperature_celsius: 22.0, wind_speed_kph: 10.0,
            wind_direction_deg: 270.0, humidity_pct: 45.0,
            visibility_km: 30.0, precipitation_mm_hr: 0.0,
            cloud_cover_pct: 10.0, fog_density: 0.0,
        }
    }

    pub fn heavy_storm() -> Self {
        WeatherState {
            weather_type: WeatherType::Thunderstorm,
            temperature_celsius: 14.0, wind_speed_kph: 60.0,
            wind_direction_deg: 180.0, humidity_pct: 95.0,
            visibility_km: 1.0, precipitation_mm_hr: 40.0,
            cloud_cover_pct: 100.0, fog_density: 0.0,
        }
    }

    pub fn movement_penalty(&self) -> f32 {
        match self.weather_type {
            WeatherType::Clear | WeatherType::Cloudy => 1.0,
            WeatherType::LightRain | WeatherType::LightSnow => 1.2,
            WeatherType::HeavyRain | WeatherType::Overcast => 1.4,
            WeatherType::Thunderstorm | WeatherType::Fog => 1.8,
            WeatherType::HeavySnow | WeatherType::Blizzard => 2.5,
            WeatherType::Sandstorm => 2.0,
            _ => 1.5,
        }
    }

    pub fn ambient_light_factor(&self) -> f32 {
        let cloud = 1.0 - self.cloud_cover_pct / 100.0 * 0.7;
        let fog = 1.0 - self.fog_density * 0.8;
        cloud * fog
    }
}

#[derive(Debug, Clone)]
pub struct WeatherForecast {
    pub entries: Vec<(f32, WeatherState)>,
}

impl WeatherForecast {
    pub fn new() -> Self {
        WeatherForecast { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, time: f32, state: WeatherState) {
        self.entries.push((time, state));
        self.entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }

    pub fn weather_at(&self, time: f32) -> Option<&WeatherState> {
        self.entries.iter()
            .rev()
            .find(|(t, _)| *t <= time)
            .map(|(_, s)| s)
    }
}

// ============================================================
// TIME OF DAY SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct SunPosition {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub color: Vec3,
    pub intensity: f32,
}

impl SunPosition {
    pub fn from_time_of_day(hour: f32, latitude_deg: f32) -> Self {
        let hour_angle = (hour - 12.0) * 15.0;
        let lat_rad = latitude_deg.to_radians();
        let declination_rad = (-23.45f32).to_radians();
        let h_rad = hour_angle.to_radians();
        let elevation = (lat_rad.sin() * declination_rad.sin()
            + lat_rad.cos() * declination_rad.cos() * h_rad.cos()).asin();
        let elevation_deg = elevation.to_degrees();
        let azimuth_deg = h_rad.sin().atan2(
            h_rad.cos() * lat_rad.sin() - declination_rad.tan() * lat_rad.cos()
        ).to_degrees() + 180.0;

        let (color, intensity) = if elevation_deg < -5.0 {
            (Vec3::new(0.01, 0.01, 0.05), 0.0)
        } else if elevation_deg < 0.0 {
            (Vec3::new(0.3, 0.1, 0.05), 0.05)
        } else if elevation_deg < 15.0 {
            (Vec3::new(1.0, 0.4, 0.1), 0.4)
        } else if elevation_deg < 40.0 {
            (Vec3::new(1.0, 0.85, 0.6), 0.8)
        } else {
            (Vec3::new(1.0, 0.97, 0.9), 1.0)
        };

        SunPosition { azimuth_deg, elevation_deg, color, intensity }
    }

    pub fn direction(&self) -> Vec3 {
        let az = self.azimuth_deg.to_radians();
        let el = self.elevation_deg.to_radians();
        Vec3::new(-el.cos() * az.sin(), el.sin(), -el.cos() * az.cos()).normalize()
    }

    pub fn is_daytime(&self) -> bool { self.elevation_deg > 0.0 }
}

// ============================================================
// GAME ECONOMY SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct Currency {
    pub name: String,
    pub symbol: String,
    pub conversion_to_gold: f32,
}

impl Currency {
    pub fn gold() -> Self { Currency { name: "Gold".to_string(), symbol: "G".to_string(), conversion_to_gold: 1.0 } }
    pub fn silver() -> Self { Currency { name: "Silver".to_string(), symbol: "S".to_string(), conversion_to_gold: 0.1 } }
    pub fn copper() -> Self { Currency { name: "Copper".to_string(), symbol: "C".to_string(), conversion_to_gold: 0.01 } }
}

#[derive(Debug, Clone)]
pub struct PriceTable {
    pub region_id: u32,
    pub prices: HashMap<String, f32>,
    pub supply_modifier: HashMap<String, f32>,
    pub demand_modifier: HashMap<String, f32>,
}

impl PriceTable {
    pub fn new(region_id: u32) -> Self {
        PriceTable {
            region_id, prices: HashMap::new(),
            supply_modifier: HashMap::new(),
            demand_modifier: HashMap::new(),
        }
    }

    pub fn set_base_price(&mut self, item: &str, price: f32) {
        self.prices.insert(item.to_string(), price);
    }

    pub fn effective_price(&self, item: &str) -> f32 {
        let base = self.prices.get(item).copied().unwrap_or(1.0);
        let supply = self.supply_modifier.get(item).copied().unwrap_or(1.0);
        let demand = self.demand_modifier.get(item).copied().unwrap_or(1.0);
        base * demand / supply
    }

    pub fn apply_shortage(&mut self, item: &str, factor: f32) {
        *self.supply_modifier.entry(item.to_string()).or_insert(1.0) /= factor.max(0.1);
    }
}

// ============================================================
// LOOT TABLE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtLootEntry {
    pub item_id: String,
    pub weight: f32,
    pub min_quantity: u32,
    pub max_quantity: u32,
    pub condition: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExtLootTable {
    pub id: u32,
    pub name: String,
    pub entries: Vec<ExtLootEntry>,
    pub guaranteed_entries: Vec<ExtLootEntry>,
    pub rolls: u32,
    pub luck_bonus: f32,
}

impl ExtLootTable {
    pub fn new(id: u32, name: &str, rolls: u32) -> Self {
        ExtLootTable { id, name: name.to_string(), entries: Vec::new(), guaranteed_entries: Vec::new(), rolls, luck_bonus: 0.0 }
    }

    pub fn add_entry(&mut self, entry: ExtLootEntry) {
        self.entries.push(entry);
    }

    pub fn total_weight(&self) -> f32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn roll_deterministic(&self, seed: u64, roll_index: u32) -> Vec<(&str, u32)> {
        let mut result = Vec::new();
        let mut rng = seed ^ (roll_index as u64 * 6364136223846793005 + 1442695040888963407);
        rng ^= rng >> 33; rng *= 0xff51afd7ed558ccd; rng ^= rng >> 33;
        let total = self.total_weight();
        if total <= 0.0 { return result; }
        let pick = (rng as f32 / u64::MAX as f32) * total;
        let mut cumulative = 0.0;
        for entry in &self.entries {
            cumulative += entry.weight;
            if pick <= cumulative {
                let qty_range = entry.max_quantity - entry.min_quantity + 1;
                let qty = entry.min_quantity + (rng % qty_range as u64) as u32;
                result.push((entry.item_id.as_str(), qty));
                break;
            }
        }
        for g in &self.guaranteed_entries {
            result.push((g.item_id.as_str(), g.min_quantity));
        }
        result
    }
}

// ============================================================
// NPC AI BEHAVIOR TREE
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BehaviorNodeType {
    Selector,
    Sequence,
    Parallel,
    Decorator,
    Action,
    Condition,
}

#[derive(Debug, Clone)]
pub struct BehaviorNode {
    pub id: u32,
    pub node_type: BehaviorNodeType,
    pub name: String,
    pub children: Vec<u32>,
    pub action_id: Option<String>,
    pub blackboard_key: Option<String>,
    pub inverted: bool,
}

impl BehaviorNode {
    pub fn new_action(id: u32, name: &str, action_id: &str) -> Self {
        BehaviorNode {
            id, node_type: BehaviorNodeType::Action,
            name: name.to_string(), children: Vec::new(),
            action_id: Some(action_id.to_string()),
            blackboard_key: None, inverted: false,
        }
    }

    pub fn new_sequence(id: u32, name: &str) -> Self {
        BehaviorNode {
            id, node_type: BehaviorNodeType::Sequence,
            name: name.to_string(), children: Vec::new(),
            action_id: None, blackboard_key: None, inverted: false,
        }
    }

    pub fn new_selector(id: u32, name: &str) -> Self {
        BehaviorNode {
            id, node_type: BehaviorNodeType::Selector,
            name: name.to_string(), children: Vec::new(),
            action_id: None, blackboard_key: None, inverted: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BehaviorTree {
    pub name: String,
    pub nodes: HashMap<u32, BehaviorNode>,
    pub root_id: u32,
    pub blackboard: HashMap<String, String>,
}

impl BehaviorTree {
    pub fn new(name: &str, root_id: u32) -> Self {
        BehaviorTree { name: name.to_string(), nodes: HashMap::new(), root_id, blackboard: HashMap::new() }
    }

    pub fn add_node(&mut self, node: BehaviorNode) {
        self.nodes.insert(node.id, node);
    }

    pub fn set_blackboard(&mut self, key: &str, value: &str) {
        self.blackboard.insert(key.to_string(), value.to_string());
    }

    pub fn get_blackboard(&self, key: &str) -> Option<&str> {
        self.blackboard.get(key).map(|s| s.as_str())
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }
}

// ============================================================
// MONSTER / ENEMY DATABASE
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MonsterCategory {
    Beast, Undead, Demon, Dragon, Humanoid, Construct,
    Elemental, Fey, Giant, Plant, Ooze, Celestial, Aberration,
}

#[derive(Debug, Clone)]
pub struct MonsterStats {
    pub hp: u32, pub max_hp: u32,
    pub armor_class: u32,
    pub attack_bonus: i32,
    pub damage_dice: String,
    pub damage_bonus: i32,
    pub speed_m: f32,
    pub strength: i32, pub dexterity: i32, pub constitution: i32,
    pub intelligence: i32, pub wisdom: i32, pub charisma: i32,
    pub challenge_rating: f32,
}

impl MonsterStats {
    pub fn modifier(stat: i32) -> i32 { (stat - 10) / 2 }
    pub fn proficiency_bonus(cr: f32) -> i32 { (cr / 4.0).ceil() as i32 + 1 }
    pub fn xp_for_cr(cr: f32) -> u32 {
        match cr as u32 {
            0 => 10, 1 => 200, 2 => 450, 3 => 700, 4 => 1100,
            5 => 1800, 6 => 2300, 7 => 2900, 8 => 3900, 9 => 5000,
            10 => 5900, 11 => 7200, 12 => 8400, 13 => 10000, 14 => 11500,
            15 => 13000, 16 => 15000, 17 => 18000, 18 => 20000, _ => 25000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonsterDefinition {
    pub id: u32,
    pub name: String,
    pub category: MonsterCategory,
    pub stats: MonsterStats,
    pub loot_table_id: Option<u32>,
    pub abilities: Vec<String>,
    pub immunities: Vec<String>,
    pub resistances: Vec<String>,
    pub behavior_tree_name: Option<String>,
    pub sprite_asset: String,
    pub description: String,
}

impl MonsterDefinition {
    pub fn xp_value(&self) -> u32 {
        MonsterStats::xp_for_cr(self.stats.challenge_rating)
    }

    pub fn is_elite(&self) -> bool {
        self.stats.challenge_rating >= 5.0 && self.abilities.len() >= 3
    }
}

// ============================================================
// MAP IMPORT / EXPORT
// ============================================================

pub const MAP_FILE_MAGIC: u32 = 0x4D415000; // "MAP\0"
pub const MAP_FILE_VERSION: u32 = 2;

#[derive(Debug, Clone)]
pub struct MapFileHeader {
    pub magic: u32,
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub layer_count: u32,
    pub object_count: u32,
    pub region_count: u32,
    pub author: String,
    pub timestamp: u64,
}

impl MapFileHeader {
    pub fn new(width: u32, height: u32, tile_size: u32) -> Self {
        MapFileHeader {
            magic: MAP_FILE_MAGIC,
            version: MAP_FILE_VERSION,
            width, height, tile_size,
            layer_count: 0, object_count: 0, region_count: 0,
            author: String::new(), timestamp: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == MAP_FILE_MAGIC && self.version <= MAP_FILE_VERSION
    }

    pub fn total_tiles(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

// ============================================================
// ADDITIONAL TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_map_final {
    use super::*;

    #[test]
    fn test_weather_movement_penalty() {
        let clear = WeatherState::clear_day();
        let storm = WeatherState::heavy_storm();
        assert_eq!(clear.movement_penalty(), 1.0);
        assert!(storm.movement_penalty() > 1.0);
    }

    #[test]
    fn test_sun_position_daytime() {
        let noon = SunPosition::from_time_of_day(12.0, 40.0);
        assert!(noon.is_daytime());
        let midnight = SunPosition::from_time_of_day(0.0, 40.0);
        assert!(!midnight.is_daytime());
    }

    #[test]
    fn test_price_table_effective_price() {
        let mut pt = PriceTable::new(1);
        pt.set_base_price("Iron Sword", 50.0);
        pt.apply_shortage("Iron Sword", 2.0);
        let price = pt.effective_price("Iron Sword");
        assert!(price > 50.0);
    }

    #[test]
    fn test_loot_table_roll() {
        let mut table = ExtLootTable::new(1, "CommonChest", 1);
        table.add_entry(ExtLootEntry {
            item_id: "gold_coin".to_string(), weight: 70.0,
            min_quantity: 5, max_quantity: 20, condition: None,
        });
        table.add_entry(ExtLootEntry {
            item_id: "health_potion".to_string(), weight: 30.0,
            min_quantity: 1, max_quantity: 2, condition: None,
        });
        let results = table.roll_deterministic(42, 0);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_behavior_tree_blackboard() {
        let mut bt = BehaviorTree::new("PatrolAI", 0);
        bt.set_blackboard("enemy_spotted", "false");
        bt.set_blackboard("patrol_complete", "true");
        assert_eq!(bt.get_blackboard("enemy_spotted"), Some("false"));
    }

    #[test]
    fn test_monster_xp() {
        let mon = MonsterDefinition {
            id: 1, name: "Goblin".to_string(),
            category: MonsterCategory::Humanoid,
            stats: MonsterStats {
                hp: 7, max_hp: 7, armor_class: 15, attack_bonus: 4,
                damage_dice: "1d6".to_string(), damage_bonus: 2,
                speed_m: 9.0, strength: 8, dexterity: 14, constitution: 10,
                intelligence: 10, wisdom: 8, charisma: 8, challenge_rating: 0.25,
            },
            loot_table_id: None, abilities: Vec::new(), immunities: Vec::new(),
            resistances: Vec::new(), behavior_tree_name: None,
            sprite_asset: "goblin.png".to_string(), description: String::new(),
        };
        assert_eq!(mon.xp_value(), 10);
    }

    #[test]
    fn test_map_file_header() {
        let header = MapFileHeader::new(100, 80, 16);
        assert!(header.is_valid());
        assert_eq!(header.total_tiles(), 8000);
    }

    #[test]
    fn test_world_map_pathfind() {
        let mut wm = ExtWorldMap::new("Testrealm", 1000.0, 1000.0);
        let mut r1 = WorldRegion::new(1, "A", ExtBiomeType::Plains);
        r1.center = Vec2::new(100.0, 100.0);
        r1.adjacent_region_ids.push(2);
        let mut r2 = WorldRegion::new(2, "B", ExtBiomeType::Forest);
        r2.center = Vec2::new(200.0, 100.0);
        r2.adjacent_region_ids.push(1);
        wm.add_region(r1);
        wm.add_region(r2);
        let path = wm.pathfind_regions(1, 2);
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 2);
    }
}

pub fn map_editor_extended_version() -> &'static str { "2.5.1-extended" }


// ============================================================
// SPELL / ABILITY SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SpellSchool {
    Abjuration, Conjuration, Divination, Enchantment,
    Evocation, Illusion, Necromancy, Transmutation, Universal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DamageType {
    Physical, Fire, Cold, Lightning, Acid, Poison,
    Necrotic, Radiant, Force, Thunder, Psychic, Bludgeoning,
    Piercing, Slashing,
}

#[derive(Debug, Clone)]
pub struct SpellEffect {
    pub damage_dice: String,
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    pub healing_dice: String,
    pub status_effect: Option<String>,
    pub duration_rounds: u32,
    pub area_radius_m: f32,
}

#[derive(Debug, Clone)]
pub struct Spell {
    pub id: u32,
    pub name: String,
    pub school: SpellSchool,
    pub level: u32,
    pub casting_time_actions: u32,
    pub range_m: f32,
    pub verbal: bool,
    pub somatic: bool,
    pub material: Option<String>,
    pub concentration: bool,
    pub ritual: bool,
    pub duration_description: String,
    pub description: String,
    pub effects: Vec<SpellEffect>,
    pub upcast_bonus_dice: Option<String>,
    pub classes_allowed: Vec<String>,
}

impl Spell {
    pub fn new(id: u32, name: &str, school: SpellSchool, level: u32) -> Self {
        Spell {
            id, name: name.to_string(), school, level,
            casting_time_actions: 1, range_m: 9.0,
            verbal: true, somatic: true, material: None,
            concentration: false, ritual: false,
            duration_description: "Instantaneous".to_string(),
            description: String::new(),
            effects: Vec::new(), upcast_bonus_dice: None,
            classes_allowed: Vec::new(),
        }
    }

    pub fn is_cantrip(&self) -> bool { self.level == 0 }
    pub fn slot_level(&self) -> u32 { self.level }

    pub fn requires_components(&self) -> String {
        let mut comp = String::new();
        if self.verbal { comp.push('V'); }
        if self.somatic { if !comp.is_empty() { comp.push_str(", "); } comp.push('S'); }
        if self.material.is_some() { if !comp.is_empty() { comp.push_str(", "); } comp.push('M'); }
        comp
    }
}

// ============================================================
// GAME CHARACTER STATS
// ============================================================

#[derive(Debug, Clone)]
pub struct CharacterStats {
    pub strength: i32, pub dexterity: i32, pub constitution: i32,
    pub intelligence: i32, pub wisdom: i32, pub charisma: i32,
}

impl CharacterStats {
    pub fn modifier(stat: i32) -> i32 { (stat - 10) / 2 }
    pub fn str_mod(&self) -> i32 { Self::modifier(self.strength) }
    pub fn dex_mod(&self) -> i32 { Self::modifier(self.dexterity) }
    pub fn con_mod(&self) -> i32 { Self::modifier(self.constitution) }
    pub fn int_mod(&self) -> i32 { Self::modifier(self.intelligence) }
    pub fn wis_mod(&self) -> i32 { Self::modifier(self.wisdom) }
    pub fn cha_mod(&self) -> i32 { Self::modifier(self.charisma) }

    pub fn point_buy_cost(&self) -> i32 {
        let cost = |s: i32| match s {
            8 => 0, 9 => 1, 10 => 2, 11 => 3, 12 => 4,
            13 => 5, 14 => 7, 15 => 9, _ => 100,
        };
        cost(self.strength) + cost(self.dexterity) + cost(self.constitution)
            + cost(self.intelligence) + cost(self.wisdom) + cost(self.charisma)
    }
}

#[derive(Debug, Clone)]
pub struct Character {
    pub id: u32,
    pub name: String,
    pub race: String,
    pub class: String,
    pub subclass: String,
    pub level: u32,
    pub experience: u64,
    pub stats: CharacterStats,
    pub max_hp: i32, pub current_hp: i32,
    pub temp_hp: i32,
    pub armor_class: i32,
    pub proficiency_bonus: i32,
    pub speed_m: f32,
    pub alignment: String,
    pub background: String,
    pub known_spells: Vec<u32>,
    pub spell_slots: Vec<(u32, u32)>,
    pub features: Vec<String>,
    pub equipment_ids: Vec<u32>,
    pub gold: f32,
}

impl Character {
    pub fn new(id: u32, name: &str) -> Self {
        Character {
            id, name: name.to_string(),
            race: String::new(), class: String::new(), subclass: String::new(),
            level: 1, experience: 0,
            stats: CharacterStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            max_hp: 10, current_hp: 10, temp_hp: 0,
            armor_class: 10, proficiency_bonus: 2,
            speed_m: 9.0, alignment: "Neutral".to_string(),
            background: String::new(),
            known_spells: Vec::new(), spell_slots: Vec::new(),
            features: Vec::new(), equipment_ids: Vec::new(),
            gold: 0.0,
        }
    }

    pub fn is_alive(&self) -> bool { self.current_hp > 0 }
    pub fn is_unconscious(&self) -> bool { self.current_hp <= 0 }

    pub fn heal(&mut self, amount: i32) {
        self.current_hp = (self.current_hp + amount).min(self.max_hp);
    }

    pub fn take_damage(&mut self, amount: i32) {
        let effective = (amount - self.temp_hp).max(0);
        self.temp_hp = (self.temp_hp - amount).max(0);
        self.current_hp -= effective;
    }

    pub fn initiative_roll(&self, dice_roll: u32) -> i32 {
        dice_roll as i32 + self.stats.dex_mod()
    }

    pub fn passive_perception(&self) -> i32 {
        10 + self.stats.wis_mod() + if self.features.contains(&"Perception Proficiency".to_string()) { self.proficiency_bonus } else { 0 }
    }

    pub fn xp_to_next_level(&self) -> u64 {
        let thresholds: &[u64] = &[0, 300, 900, 2700, 6500, 14000, 23000, 34000, 48000, 64000, 85000, 100000, 120000, 140000, 165000, 195000, 225000, 265000, 305000, 355000];
        if self.level as usize >= thresholds.len() { return u64::MAX; }
        thresholds[self.level as usize].saturating_sub(self.experience)
    }
}

// ============================================================
// ITEM / EQUIPMENT SYSTEM
// ============================================================

impl ItemRarity {
    pub fn gold_value_modifier(&self) -> f32 {
        match self {
            ItemRarity::Common => 1.0,
            ItemRarity::Uncommon => 5.0,
            ItemRarity::Rare => 25.0,
            ItemRarity::Epic => 100.0,
            ItemRarity::VeryRare => 250.0,
            ItemRarity::Legendary => 5000.0,
            ItemRarity::Artifact => f32::INFINITY,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemCategory {
    Weapon, Armor, Shield, Ring, Necklace, Cloak, Boots,
    Helmet, Gloves, Belt, Wand, Staff, Scroll, Potion,
    Gem, QuestItem, Consumable, Tool, Mount, Vehicle,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub category: ItemCategory,
    pub rarity: ItemRarity,
    pub weight_kg: f32,
    pub base_value: f32,
    pub description: String,
    pub properties: HashMap<String, String>,
    pub requires_attunement: bool,
    pub attuned_to: Option<u32>,
    pub charges: Option<u32>,
    pub max_charges: Option<u32>,
    pub spell_trigger: Option<u32>,
}

impl Item {
    pub fn new(id: u32, name: &str, category: ItemCategory, rarity: ItemRarity) -> Self {
        Item {
            id, name: name.to_string(), category, rarity,
            weight_kg: 1.0, base_value: 10.0, description: String::new(),
            properties: HashMap::new(), requires_attunement: false,
            attuned_to: None, charges: None, max_charges: None, spell_trigger: None,
        }
    }

    pub fn effective_value(&self) -> f32 {
        self.base_value * self.rarity.gold_value_modifier()
    }

    pub fn use_charge(&mut self) -> bool {
        if let Some(ref mut c) = self.charges {
            if *c > 0 { *c -= 1; true } else { false }
        } else { true }
    }

    pub fn is_depleted(&self) -> bool {
        self.charges.map(|c| c == 0).unwrap_or(false)
    }
}

// ============================================================
// CRAFTING SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct CraftingIngredient {
    pub item_id: u32,
    pub quantity: u32,
    pub consumed: bool,
}

#[derive(Debug, Clone)]
pub struct CraftingRecipe {
    pub recipe_id: u32,
    pub name: String,
    pub output_item_id: u32,
    pub output_quantity: u32,
    pub ingredients: Vec<CraftingIngredient>,
    pub required_tools: Vec<String>,
    pub skill_required: String,
    pub skill_level: u32,
    pub time_minutes: f32,
    pub experience_granted: u32,
}

impl CraftingRecipe {
    pub fn new(recipe_id: u32, name: &str, output_item: u32) -> Self {
        CraftingRecipe {
            recipe_id, name: name.to_string(),
            output_item_id: output_item, output_quantity: 1,
            ingredients: Vec::new(), required_tools: Vec::new(),
            skill_required: "Crafting".to_string(), skill_level: 1,
            time_minutes: 10.0, experience_granted: 100,
        }
    }

    pub fn add_ingredient(&mut self, item_id: u32, quantity: u32, consumed: bool) {
        self.ingredients.push(CraftingIngredient { item_id, quantity, consumed });
    }

    pub fn consumed_ingredients(&self) -> Vec<&CraftingIngredient> {
        self.ingredients.iter().filter(|i| i.consumed).collect()
    }
}

// ============================================================
// DIALOGUE SCRIPTING ENGINE
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueCondition {
    pub condition_type: String,
    pub target: String,
    pub operator: String,
    pub value: String,
}

impl DialogueCondition {
    pub fn check_flag(flag: &str, expected: bool) -> Self {
        DialogueCondition {
            condition_type: "flag".to_string(),
            target: flag.to_string(),
            operator: "==".to_string(),
            value: expected.to_string(),
        }
    }

    pub fn check_level(min_level: u32) -> Self {
        DialogueCondition {
            condition_type: "level".to_string(),
            target: "player".to_string(),
            operator: ">=".to_string(),
            value: min_level.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DialogueAction {
    pub action_type: String,
    pub target: String,
    pub value: String,
}

impl DialogueAction {
    pub fn set_flag(flag: &str, value: bool) -> Self {
        DialogueAction { action_type: "set_flag".to_string(), target: flag.to_string(), value: value.to_string() }
    }

    pub fn give_item(item_id: u32) -> Self {
        DialogueAction { action_type: "give_item".to_string(), target: "player".to_string(), value: item_id.to_string() }
    }

    pub fn give_xp(amount: u32) -> Self {
        DialogueAction { action_type: "give_xp".to_string(), target: "player".to_string(), value: amount.to_string() }
    }
}

// ============================================================
// MAP CHUNK STREAMING
// ============================================================

pub const CHUNK_SIZE: u32 = 64;
pub const CHUNK_LOAD_RADIUS: u32 = 3;

#[derive(Debug, Clone)]
pub struct ExtMapChunk {
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub loaded: bool,
    pub dirty: bool,
    pub tile_data: Vec<u32>,
    pub object_ids: Vec<u32>,
    pub light_data: Vec<Vec3>,
    pub last_accessed_frame: u64,
}

impl ExtMapChunk {
    pub fn new(cx: i32, cy: i32) -> Self {
        let size = (CHUNK_SIZE * CHUNK_SIZE) as usize;
        ExtMapChunk {
            chunk_x: cx, chunk_y: cy,
            loaded: false, dirty: false,
            tile_data: vec![0u32; size],
            object_ids: Vec::new(),
            light_data: vec![Vec3::ZERO; size],
            last_accessed_frame: 0,
        }
    }

    pub fn get_tile(&self, local_x: u32, local_y: u32) -> u32 {
        if local_x >= CHUNK_SIZE || local_y >= CHUNK_SIZE { return 0; }
        self.tile_data[(local_y * CHUNK_SIZE + local_x) as usize]
    }

    pub fn set_tile(&mut self, local_x: u32, local_y: u32, tile_id: u32) {
        if local_x >= CHUNK_SIZE || local_y >= CHUNK_SIZE { return; }
        self.tile_data[(local_y * CHUNK_SIZE + local_x) as usize] = tile_id;
        self.dirty = true;
    }

    pub fn world_position(&self) -> Vec2 {
        Vec2::new(self.chunk_x as f32 * CHUNK_SIZE as f32, self.chunk_y as f32 * CHUNK_SIZE as f32)
    }
}

#[derive(Debug, Clone)]
pub struct ExtChunkManager {
    pub loaded_chunks: HashMap<(i32, i32), ExtMapChunk>,
    pub player_chunk: (i32, i32),
    pub frame: u64,
    pub max_loaded_chunks: usize,
}

impl ExtChunkManager {
    pub fn new() -> Self {
        ExtChunkManager {
            loaded_chunks: HashMap::new(),
            player_chunk: (0, 0),
            frame: 0,
            max_loaded_chunks: 64,
        }
    }

    pub fn world_to_chunk(world_x: f32, world_y: f32) -> (i32, i32) {
        (
            (world_x / CHUNK_SIZE as f32).floor() as i32,
            (world_y / CHUNK_SIZE as f32).floor() as i32,
        )
    }

    pub fn get_tile(&self, world_x: f32, world_y: f32) -> u32 {
        let (cx, cy) = Self::world_to_chunk(world_x, world_y);
        let local_x = (world_x as i32 - cx * CHUNK_SIZE as i32) as u32;
        let local_y = (world_y as i32 - cy * CHUNK_SIZE as i32) as u32;
        self.loaded_chunks.get(&(cx, cy))
            .map(|c| c.get_tile(local_x, local_y))
            .unwrap_or(0)
    }

    pub fn required_chunks(&self) -> Vec<(i32, i32)> {
        let (px, py) = self.player_chunk;
        let r = CHUNK_LOAD_RADIUS as i32;
        let mut required = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                required.push((px + dx, py + dy));
            }
        }
        required
    }

    pub fn evict_distant_chunks(&mut self) {
        if self.loaded_chunks.len() <= self.max_loaded_chunks { return; }
        let (px, py) = self.player_chunk;
        let mut chunks_with_dist: Vec<((i32, i32), i32)> = self.loaded_chunks.keys()
            .map(|&(cx, cy)| ((cx, cy), (cx - px).abs() + (cy - py).abs()))
            .collect();
        chunks_with_dist.sort_by_key(|(_, d)| -(*d));
        let to_remove = chunks_with_dist.len() - self.max_loaded_chunks;
        for i in 0..to_remove {
            self.loaded_chunks.remove(&chunks_with_dist[i].0);
        }
    }
}

// ============================================================
// ADDITIONAL TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_map_final2 {
    use super::*;

    #[test]
    fn test_spell_components() {
        let spell = Spell::new(1, "Fireball", SpellSchool::Evocation, 3);
        let comp = spell.requires_components();
        assert!(comp.contains('V'));
        assert!(comp.contains('S'));
    }

    #[test]
    fn test_character_healing() {
        let mut c = Character::new(1, "Aragorn");
        c.max_hp = 40;
        c.current_hp = 10;
        c.heal(15);
        assert_eq!(c.current_hp, 25);
        c.heal(100);
        assert_eq!(c.current_hp, 40);
    }

    #[test]
    fn test_character_damage() {
        let mut c = Character::new(1, "Legolas");
        c.max_hp = 30;
        c.current_hp = 30;
        c.take_damage(35);
        assert!(!c.is_alive());
    }

    #[test]
    fn test_item_effective_value() {
        let item = Item::new(1, "Longsword +2", ItemCategory::Weapon, ItemRarity::Rare);
        assert!(item.effective_value() > item.base_value);
    }

    #[test]
    fn test_chunk_manager_tile() {
        let mut mgr = ExtChunkManager::new();
        let mut chunk = ExtMapChunk::new(0, 0);
        chunk.set_tile(3, 5, 42);
        mgr.loaded_chunks.insert((0, 0), chunk);
        assert_eq!(mgr.get_tile(3.0, 5.0), 42);
    }

    #[test]
    fn test_chunk_required() {
        let mgr = ExtChunkManager::new();
        let required = mgr.required_chunks();
        assert_eq!(required.len(), (2 * CHUNK_LOAD_RADIUS as usize + 1).pow(2));
    }

    #[test]
    fn test_loot_roll() {
        let mut table = ExtLootTable::new(1, "Test", 1);
        table.add_entry(ExtLootEntry {
            item_id: "gold".to_string(), weight: 100.0,
            min_quantity: 1, max_quantity: 10, condition: None,
        });
        let results = table.roll_deterministic(1234, 0);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_crafting_recipe() {
        let mut recipe = CraftingRecipe::new(1, "Iron Sword", 100);
        recipe.add_ingredient(50, 2, true);
        recipe.add_ingredient(51, 1, false);
        assert_eq!(recipe.consumed_ingredients().len(), 1);
    }

    #[test]
    fn test_stats_point_buy() {
        let stats = CharacterStats { strength: 15, dexterity: 14, constitution: 13, intelligence: 12, wisdom: 10, charisma: 8 };
        assert_eq!(stats.point_buy_cost(), 27);
    }
}

pub const MAP_MAX_OBJECTS: u32 = 10000;
pub const MAP_TILE_SIZE_DEFAULT: u32 = 16;
pub const MAP_EDITOR_VERSION: &str = "2.5.0";


// ============================================================
// RANDOM ENCOUNTER TABLE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct RandomEncounterEntry {
    pub weight: u32,
    pub encounter_id: u32,
    pub min_party_level: u32,
    pub max_party_level: u32,
    pub time_of_day: Option<String>,
    pub weather_conditions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RandomEncounterTable {
    pub table_id: u32,
    pub region_id: u32,
    pub entries: Vec<RandomEncounterEntry>,
    pub check_interval_minutes: f32,
    pub base_encounter_chance_pct: f32,
}

impl RandomEncounterTable {
    pub fn new(table_id: u32, region_id: u32) -> Self {
        RandomEncounterTable {
            table_id, region_id,
            entries: Vec::new(),
            check_interval_minutes: 30.0,
            base_encounter_chance_pct: 20.0,
        }
    }

    pub fn add_entry(&mut self, entry: RandomEncounterEntry) {
        self.entries.push(entry);
    }

    pub fn total_weight(&self, party_level: u32) -> u32 {
        self.entries.iter()
            .filter(|e| party_level >= e.min_party_level && party_level <= e.max_party_level)
            .map(|e| e.weight)
            .sum()
    }

    pub fn roll(& self, party_level: u32, rng_value: u32) -> Option<u32> {
        let total = self.total_weight(party_level);
        if total == 0 { return None; }
        let pick = rng_value % total;
        let mut cumulative = 0u32;
        for entry in &self.entries {
            if party_level < entry.min_party_level || party_level > entry.max_party_level { continue; }
            cumulative += entry.weight;
            if pick < cumulative { return Some(entry.encounter_id); }
        }
        None
    }
}

// ============================================================
// POINT OF INTEREST SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PoiCategory {
    Dungeon, Settlement, Landmark, Resource, Shop,
    QuestMarker, Danger, Secret, Teleporter, Camp,
}

#[derive(Debug, Clone)]
pub struct PointOfInterest {
    pub id: u32,
    pub name: String,
    pub category: PoiCategory,
    pub position: Vec2,
    pub description: String,
    pub discovered: bool,
    pub icon_id: u32,
    pub level_range: (u32, u32),
    pub quest_ids: Vec<u32>,
    pub region_id: Option<u32>,
}

impl PointOfInterest {
    pub fn new(id: u32, name: &str, category: PoiCategory, pos: Vec2) -> Self {
        PointOfInterest {
            id, name: name.to_string(), category, position: pos,
            description: String::new(), discovered: false, icon_id: 0,
            level_range: (1, 99), quest_ids: Vec::new(), region_id: None,
        }
    }

    pub fn distance_to(&self, pos: Vec2) -> f32 {
        (self.position - pos).length()
    }
}

#[derive(Debug, Clone)]
pub struct PoiManager {
    pub pois: Vec<PointOfInterest>,
}

impl PoiManager {
    pub fn new() -> Self {
        PoiManager { pois: Vec::new() }
    }

    pub fn add(&mut self, poi: PointOfInterest) {
        self.pois.push(poi);
    }

    pub fn nearest_to(&self, pos: Vec2) -> Option<&PointOfInterest> {
        self.pois.iter().min_by(|a, b| a.distance_to(pos).partial_cmp(&b.distance_to(pos)).unwrap())
    }

    pub fn in_radius(&self, pos: Vec2, radius: f32) -> Vec<&PointOfInterest> {
        self.pois.iter().filter(|p| p.distance_to(pos) <= radius).collect()
    }

    pub fn by_category(&self, cat: &PoiCategory) -> Vec<&PointOfInterest> {
        self.pois.iter().filter(|p| &p.category == cat).collect()
    }

    pub fn discover(&mut self, id: u32) {
        if let Some(poi) = self.pois.iter_mut().find(|p| p.id == id) {
            poi.discovered = true;
        }
    }
}

// ============================================================
// PUZZLE / TRAP SYSTEM
// ============================================================

impl TileMap {
    pub fn new(width: usize, height: usize, tile_width: u32, tile_height: u32) -> Self {
        Self { width, height, tile_width, tile_height, layers: Vec::new(), projection: MapProjection::TopDown, background_color: Vec4::ZERO, ambient_light: 0.3 }
    }
    pub fn add_layer(&mut self, layer: MapLayer) { self.layers.push(layer); }
    pub fn layer_count(&self) -> usize { self.layers.len() }
}

impl TileDatabase {
    pub fn new() -> Self { Self { definitions: HashMap::new() } }
    pub fn register(&mut self, def: TileDefinition) { self.definitions.insert(def.id, def); }
    pub fn get(&self, id: u32) -> Option<&TileDefinition> { self.definitions.get(&id) }
}

impl TilePos {
    pub fn new(x: i32, y: i32) -> Self { Self { x, y } }
    pub fn octile_distance(&self, other: &TilePos) -> f32 {
        let dx = (self.x - other.x).abs() as f32;
        let dy = (self.y - other.y).abs() as f32;
        let (big, small) = if dx > dy { (dx, dy) } else { (dy, dx) };
        big + (std::f32::consts::SQRT_2 - 1.0) * small
    }
    pub fn manhattan_distance(&self, other: &TilePos) -> i32 { (self.x - other.x).abs() + (self.y - other.y).abs() }
}

impl MapRegion {
    pub fn new(id: u32, name: impl Into<String>, region_type: RegionType, bounds: (i32, i32, i32, i32)) -> Self {
        Self { id, name: name.into(), region_type, bounds, connected_regions: Vec::new(), level_range: (1, 10), background_music: None, ambient_sounds: Vec::new(), weather: "clear".into(), discovered: false, completion: 0.0 }
    }
    pub fn connect(&mut self, other_id: u32) { if !self.connected_regions.contains(&other_id) { self.connected_regions.push(other_id); } }
}

impl TrapDefinition {
    pub fn new(id: u32, trap_type: TrapType, position: (i32, i32)) -> Self {
        Self { id, trap_type, position, damage: 10, damage_type: "physical".into(), triggered: false, rearm_time: 30.0, time_since_trigger: 0.0, visible: false, disarm_skill: 10 }
    }
    pub fn trigger(&mut self) -> u32 { self.triggered = true; self.time_since_trigger = 0.0; self.damage }
    pub fn update(&mut self, dt: f32) { if self.triggered { self.time_since_trigger += dt; if self.time_since_trigger >= self.rearm_time { self.triggered = false; } } }
}

impl DoorDefinition {
    pub fn new(id: u32, position: (i32, i32), facing: u8) -> Self {
        Self { id, position, facing, state: DoorState::Closed, key_id: None, lock_difficulty: 0, hp: 50, material: "wood".into(), auto_close_time: None, is_secret: false }
    }
    pub fn try_open(&self, has_key: bool, pick_skill: u32) -> bool { match self.state { DoorState::Closed | DoorState::Open | DoorState::Broken => true, DoorState::Locked => (has_key && self.key_id.is_none()) || pick_skill >= self.lock_difficulty, DoorState::Barred => false } }
    pub fn lock(&mut self, key_id: u32, difficulty: u32) { self.state = DoorState::Locked; self.key_id = Some(key_id); self.lock_difficulty = difficulty; }
}

impl TileObject {
    pub fn new(id: u32, object_type: impl Into<String>, position: (i32, i32)) -> Self {
        Self { id, object_type: object_type.into(), position, size: (1,1), rotation: 0, flip_x: false, flip_y: false, layer: 0, properties: HashMap::new(), collidable: true, sprite_id: 0 }
    }
    pub fn get_property(&self, k: &str) -> Option<&str> { self.properties.get(k).map(|s| s.as_str()) }
    pub fn set_property(&mut self, k: &str, v: &str) { self.properties.insert(k.to_string(), v.to_string()); }
}

impl WarpPoint {
    pub fn new(id: u32, name: impl Into<String>, position: (i32, i32), target_map: impl Into<String>, target_position: (i32, i32)) -> Self {
        Self { id, name: name.into(), position, target_map: target_map.into(), target_position, unlock_condition: None, is_unlocked: false, warp_type: "portal".into(), effect_name: "warp".into() }
    }
}

impl DungeonGenerator {
    pub fn new(width: i32, height: i32, seed: u64) -> Self {
        Self { width, height, seed, min_room_size: 5, max_room_size: 15, max_rooms: 20 }
    }
    pub fn generate(&self) -> Vec<DungeonRoom> { let mut rooms = Vec::new(); let mut s = self.seed; let next = |s: &mut u64| -> u64 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s }; let room_count = 4 + (next(&mut s) % 6) as i32; for i in 0..room_count { let rt = if i == 0 { DungeonRoomType::Entrance } else if i == room_count-1 { DungeonRoomType::BossRoom } else if i == 1 { DungeonRoomType::Treasure } else { DungeonRoomType::Combat }; rooms.push(DungeonRoom { id: i as u32, x: (i*20)%self.width, y: (i*15)%self.height, width: 10, height: 8, room_type: rt, is_cleared: false, connections: if i>0 { vec![(i-1) as u32] } else { Vec::new() }, loot_level: i as u8 }); } rooms }
    pub fn generate_bsp(&mut self, _w: i32, _h: i32, _attempts: u32) -> (Vec<Room>, Vec<(usize, usize)>) { (Vec::new(), Vec::new()) }
}

impl MapEvent {
    pub fn new(id: u32, event_type: MapEventType, x: i32, y: i32) -> Self {
        Self { id, event_type, position: (x,y), data: HashMap::new(), fired: false, cooldown_s: 0.0, last_fire_time: 0.0 }
    }
    pub fn with_data(mut self, key: &str, val: &str) -> Self { self.data.insert(key.into(), val.into()); self }
    pub fn with_cooldown(mut self, secs: f32) -> Self { self.cooldown_s = secs; self }
    pub fn can_fire(&self, current_time: f32) -> bool { !self.fired || (current_time - self.last_fire_time) >= self.cooldown_s }
}

impl MapEventSystem {
    pub fn new() -> Self { Self { events: Vec::new(), current_time: 0.0 } }
    pub fn add_event(&mut self, e: MapEvent) { self.events.push(e); }
    pub fn events_at(&self, x: i32, y: i32) -> Vec<&MapEvent> { self.events.iter().filter(|e| e.position == (x,y)).collect() }
    pub fn trigger_at(&mut self, x: i32, y: i32) -> Vec<u32> {
        let t = self.current_time;
        let mut fired = Vec::new();
        for e in &mut self.events { if e.position == (x,y) && e.can_fire(t) { e.fired = true; e.last_fire_time = t; fired.push(e.id); } }
        fired
    }
    pub fn events_of_type(&self, et: &MapEventType) -> Vec<&MapEvent> {
        self.events.iter().filter(|e| std::mem::discriminant(&e.event_type) == std::mem::discriminant(et)).collect()
    }
    pub fn reset_all(&mut self) { for e in &mut self.events { e.fired = false; } }
}

impl PatrolWaypoint {
    pub fn new(x: i32, y: i32) -> Self { Self { position: (x,y), wait_time_s: 0.0, animation_hint: String::new() } }
    pub fn with_wait(mut self, secs: f32) -> Self { self.wait_time_s = secs; self }
}

impl MapProgressionData {
    pub fn new(map_id: u32) -> Self {
        Self { map_id, total_secrets: 0, found_secrets: 0, total_enemies: 0, killed_enemies: 0, total_chests: 0, opened_chests: 0, boss_defeated: false, time_spent_s: 0.0, completion_flags: HashSet::new() }
    }
    pub fn completion_percent(&self) -> f32 { if self.total_enemies == 0 && self.total_chests == 0 && self.total_secrets == 0 { 0.0 } else { let e = if self.total_enemies > 0 { self.killed_enemies as f32 / self.total_enemies as f32 } else { 1.0 }; let c = if self.total_chests > 0 { self.opened_chests as f32 / self.total_chests as f32 } else { 1.0 }; let s = if self.total_secrets > 0 { self.found_secrets as f32 / self.total_secrets as f32 } else { 1.0 }; (e + c + s) / 3.0 * 100.0 } }
    pub fn set_flag(&mut self, flag: &str) { self.completion_flags.insert(flag.to_string()); }
    pub fn has_flag(&self, flag: &str) -> bool { self.completion_flags.contains(flag) }
    pub fn is_100_percent(&self) -> bool { self.completion_percent() >= 100.0 }
    pub fn grade(&self) -> char { let p = self.completion_percent(); if p >= 100.0 { 'S' } else if p >= 80.0 { 'A' } else if p >= 60.0 { 'B' } else { 'C' } }
}

impl OverworldGenerator {
    pub fn new(width: u32, height: u32, seed: u64) -> Self { Self { seed, width, height, sea_level: 0.4, mountain_threshold: 0.7, forest_threshold: 0.55 } }
    pub fn generate(&self) -> (HeightMap, Vec<Vec<u32>>) { let mut hm = HeightMap::new(self.width, self.height); hm.generate_fbm(6, self.seed); hm.smooth(2); let tiles = hm.to_terrain_tiles_simple(); (hm, tiles) }
    pub fn place_towns(&self, hm: &HeightMap, count: usize) -> Vec<(u32, u32)> { let mut towns = Vec::new(); let step = (hm.width / (count as u32 + 1)).max(1); for i in 1..=count { towns.push((step * i as u32, hm.height / 2)); } towns }
    pub fn generate_river(&self, hm: &HeightMap, start: (u32, u32)) -> Vec<(u32, u32)> { let mut river = vec![start]; let mut pos = start; for _ in 0..100 { let (x, y) = pos; if y + 1 >= hm.height { break; } pos = (x, y + 1); river.push(pos); } river }
}

impl MapEditor {
    pub fn new(width: usize, height: usize, tile_width: u32, tile_height: u32) -> Self {
        let mut map = TileMap::new(width, height, tile_width, tile_height);
        map.layers.push(MapLayer::new("ground", LayerType::Ground, width, height));
        Self {
            map,
            layers: Vec::new(),
            db: TileDatabase::new(),
            undo_stack: MapUndoStack { history: std::collections::VecDeque::new(), redo_stack: std::collections::VecDeque::new(), max_size: 50 },
            brush: BrushState { tool: BrushTool::Pencil, tile_id: 0, size: 1, layer_idx: 0, start_x: None, start_y: None, is_painting: false },
            clipboard: Clipboard { data: None },
            selection: SelectionRect { is_active: false, x: 0, y: 0, width: 0, height: 0 },
            rooms: RoomGraph { nodes: HashMap::new(), edges: HashMap::new() },
            encounter_zones: Vec::new(),
            patrol_paths: Vec::new(),
            event_zones: Vec::new(),
            scripted_sequences: Vec::new(),
            lights: Vec::new(),
            minimap: MinimapData { pixels: Vec::new(), fog_states: Vec::new(), width: 64, height: 64, scale_x: 1.0, scale_y: 1.0 },
            mode: MapEditorMode::Idle,
            current_layer: 0,
            rng: MapRng::new(42),
            pending_actions: Vec::new(),
            grid_visible: true,
            show_collision: false,
            show_entities: true,
            show_lighting: true,
            show_patrol_paths: false,
            show_encounter_zones: false,
            show_minimap: true,
        }
    }
    pub fn undo(&mut self) { self.undo_stack.history.pop_back(); }
    pub fn total_walkable_tiles(&self) -> usize { 0 }
    pub fn start_stroke(&mut self, _x: usize, _y: usize) {}
    pub fn paint_tile(&mut self, x: usize, y: usize) { if self.map.layers.is_empty() { return; } let tile_id = self.brush.tile_id; let layer = &mut self.map.layers[self.brush.layer_idx]; layer.set(x, y, Tile { tile_id, ..Tile::default() }); }
    pub fn end_stroke(&mut self) {}
    pub fn layers(&self) -> &Vec<MapLayer> { &self.map.layers }
    pub fn paint_tile_at(&mut self, x: usize, y: usize) { self.paint_tile(x, y); }
    pub fn build_nav_regions(&self) -> Vec<TileRect> { Vec::new() }
    pub fn total_solid_tiles(&self) -> usize { 0 }
    pub fn serialize(&self) -> Vec<u8> { serialize_map(&self.map) }
    pub fn deserialize_into(&mut self, data: &[u8]) -> bool { data.len() >= 8 }
    pub fn generate_dungeon(&mut self, _depth: usize, _min_rooms: usize, _seed: u64) {}
    pub fn bake_lights(&mut self) {}
    pub fn rebuild_minimap(&mut self) {}
    pub fn undo_last(&mut self) { self.undo(); }
}

impl StaticLight {
    pub fn torch(id: u32, x: i32, y: i32) -> Self { Self { id, position: (x,y), color: (255,160,80), radius: 4.0, intensity: 0.8, cast_shadows: true, flicker: true, flicker_speed: 3.0, flicker_time: 0.0 } }
    pub fn lamp(id: u32, x: i32, y: i32) -> Self { Self { id, position: (x,y), color: (255,240,200), radius: 6.0, intensity: 1.0, cast_shadows: true, flicker: false, flicker_speed: 0.0, flicker_time: 0.0 } }
    pub fn ambient(&self) -> f32 { self.intensity * 0.3 }
    pub fn update_flicker(&mut self, dt: f32) { if self.flicker { self.flicker_time += dt * self.flicker_speed; self.intensity = 0.75 + 0.25 * self.flicker_time.sin(); } }
    pub fn update(&mut self, dt: f32) { self.update_flicker(dt); }
    pub fn current_intensity(&self) -> f32 { self.intensity }
}

impl DynamicLightMap {
    pub fn new(w: u32, h: u32, ambient: f32) -> Self { Self { width: w, height: h, light_values: vec![ambient; (w*h) as usize], ambient_level: ambient } }
    pub fn bake_static_lights(&mut self, lights: &[StaticLight]) { for light in lights { let (lx, ly) = light.position; for y in 0..self.height { for x in 0..self.width { let dx = x as f32 - lx as f32; let dy = y as f32 - ly as f32; let dist = (dx*dx+dy*dy).sqrt(); if dist < light.radius { let contrib = light.intensity * (1.0 - dist/light.radius); let idx = y as usize*self.width as usize+x as usize; self.light_values[idx] = (self.light_values[idx] + contrib).min(1.0); } } } } }
    pub fn get(&self, x: u32, y: u32) -> f32 { self.light_values.get((y*self.width+x) as usize).copied().unwrap_or(self.ambient_level) }
    pub fn average_illuminance(&self) -> f32 { if self.light_values.is_empty() { return 0.0; } self.light_values.iter().sum::<f32>() / self.light_values.len() as f32 }
    pub fn dark_tile_count(&self, threshold: f32) -> usize { self.light_values.iter().filter(|&&v| v < threshold).count() }
}


impl MapTrigger {
    pub fn on_enter(id: u32, x: i32, y: i32, w: i32, h: i32, script: impl Into<String>) -> Self { Self { id, region: (x, y, w, h), condition: TriggerCondition2::PlayerEnter, fired: false, one_shot: true, cooldown: 0.0, elapsed_cooldown: 0.0, script: script.into() } }
}

impl TriggerManager2 {
    pub fn new() -> Self { Self { triggers: Vec::new() } }
    pub fn add(&mut self, t: MapTrigger) { self.triggers.push(t); }
    pub fn check_enter(&mut self, x: i32, y: i32) -> Vec<String> { let mut scripts = Vec::new(); for t in &mut self.triggers { let (rx, ry, rw, rh) = t.region; if !t.fired && x >= rx && x < rx + rw && y >= ry && y < ry + rh { scripts.push(t.script.clone()); if t.one_shot { t.fired = true; } } } scripts }
    pub fn fired_count(&self) -> usize { self.triggers.iter().filter(|t| t.fired).count() }
}

impl EncounterZone {
    pub fn contains_point(&self, x: i32, y: i32) -> bool { if self.polygon.len() < 2 { return false; } let xs: Vec<i32> = self.polygon.iter().map(|p| p.0).collect(); let ys: Vec<i32> = self.polygon.iter().map(|p| p.1).collect(); let min_x = *xs.iter().min().unwrap_or(&0); let max_x = *xs.iter().max().unwrap_or(&0); let min_y = *ys.iter().min().unwrap_or(&0); let max_y = *ys.iter().max().unwrap_or(&0); x >= min_x && x <= max_x && y >= min_y && y <= max_y }
}


impl MapUndoAction {
    pub fn new(desc: &str) -> Self { Self { description: desc.to_string(), changes: Vec::new() } }
    pub fn add_change(&mut self, x: i32, y: i32, layer: &str, before: u32, after: u32) { self.changes.push(TileChange { x, y, layer: layer.to_string(), before, after }); }
    pub fn add_tile_change(&mut self, c: TileChange) { self.changes.push(c); }
    pub fn add_change_tile(&mut self, c: TileChange) { self.changes.push(c); }
}
impl MapUndoHistory {
    pub fn new(max: usize) -> Self { Self { actions: VecDeque::new(), redo_stack: Vec::new(), max_history: max } }
    pub fn push_action(&mut self, a: MapUndoAction) { if self.actions.len() >= self.max_history { self.actions.pop_front(); } self.actions.push_back(a); self.redo_stack.clear(); }
    pub fn undo(&mut self) -> Option<MapUndoAction> { let a = self.actions.pop_back(); if let Some(ref ua) = a { self.redo_stack.push(ua.clone()); } a }
    pub fn redo(&mut self) -> Option<MapUndoAction> { let a = self.redo_stack.pop(); if let Some(ref ra) = a { self.actions.push_back(ra.clone()); } a }
    pub fn push(&mut self, action: MapUndoAction) { if self.actions.len() >= self.max_history { self.actions.pop_front(); } self.actions.push_back(action); self.redo_stack.clear(); }
    pub fn can_undo(&self) -> bool { !self.actions.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn undo_count(&self) -> usize { self.actions.len() }
}
impl NpcDialogueNode {
    pub fn new(id: u32, speaker: &str, text: &str) -> Self { Self { id, text: text.to_string(), speaker: speaker.to_string(), responses: Vec::new(), conditions: Vec::new(), effects: Vec::new() } }
    pub fn add_response(&mut self, text: &str, next_id: u32) { self.responses.push((text.to_string(), next_id)); }
}
impl NpcDefinition {
    pub fn new(id: u32, name: &str, role: NpcRole, pos: (i32, i32)) -> Self { Self { id, name: name.to_string(), role, position: pos, level: 1, faction: String::from("neutral"), dialogue_root: 0, dialogue_nodes: HashMap::new(), schedule: Vec::new(), inventory: Vec::new(), aggression_range: 3.0, is_essential: false } }
    pub fn add_dialogue(&mut self, node: NpcDialogueNode) { let id = node.id; self.dialogue_nodes.insert(id, node); }
    pub fn add_to_schedule(&mut self, hour: u32, pos: (i32, i32)) { self.schedule.push((hour, pos)); }
    pub fn position_at_hour(&self, hour: u32) -> (i32, i32) { let mut best = self.position; let mut best_h = 0u32; for &(h, p) in &self.schedule { if h <= hour && h >= best_h { best = p; best_h = h; } } best }
    pub fn is_merchant(&self) -> bool { matches!(self.role, NpcRole::Merchant) }
    pub fn grade(&self) -> u32 { 1 }
    pub fn has_flag(&self, _flag: &str) -> bool { false }
    pub fn set_property(&mut self, _key: &str, _val: &str) {}
}
impl DestructibleObject {
    pub fn new(id: u32, pos: (i32, i32), obj_type: &str, max_hp: i32) -> Self { Self { id, position: pos, object_type: obj_type.to_string(), hp: max_hp, max_hp, destroyed: false, blocking: true, drops: Vec::new(), destruction_effect: String::from("none"), respawn_time: None } }
    pub fn take_damage(&mut self, dmg: i32) { self.hp -= dmg; if self.hp <= 0 { self.hp = 0; self.destroyed = true; self.blocking = false; } }
    pub fn fire_pillar() -> Self { Self::new(0, (0,0), "fire_pillar", 1) }
    pub fn frost_zone() -> Self { Self::new(0, (0,0), "frost_zone", 1) }
    pub fn secret() -> Self { Self::new(0, (0,0), "secret", 1) }
}
impl TileEffectMap {
    pub fn new(w: i32, h: i32) -> Self { Self { width: w, height: h, effects: vec![TileEffect::None; (w*h) as usize], effect_params: vec![0.0; (w*h) as usize] } }
    pub fn set_effect(&mut self, x: i32, y: i32, e: TileEffect, p: f32) { if x>=0&&x<self.width&&y>=0&&y<self.height { let i=(y*self.width+x) as usize; self.effects[i]=e; self.effect_params[i]=p; } }
    pub fn effect_at(&self, x: i32, y: i32) -> &TileEffect { if x>=0&&x<self.width&&y>=0&&y<self.height { &self.effects[(y*self.width+x) as usize] } else { &TileEffect::None } }
    pub fn apply_to_entity(&self, x: i32, y: i32, hp: &mut f32, speed: &mut f32) { if x>=0&&x<self.width&&y>=0&&y<self.height { let idx=(y*self.width+x) as usize; let p=self.effect_params[idx]; match &self.effects[idx] { TileEffect::Damage => *hp -= p, TileEffect::Heal => *hp += p, TileEffect::Slow => *speed *= p, _ => {} } } }
    pub fn effects_count_by_type(&self) -> HashMap<String, usize> { let mut m = HashMap::new(); for e in &self.effects { *m.entry(format!("{:?}",e)).or_insert(0) += 1; } m }
}

impl Default for DestructibleObject {
    fn default() -> Self { Self::new(0, (0,0), "barrel", 20) }
}

impl WaterBody {
    pub fn new(id: u32, water_type: impl Into<String>) -> Self {
        Self { id, tiles: HashSet::new(), depth: 1.0, current_dir: (0.0, 0.0), water_type: water_type.into(), is_swimmable: true, damage_per_turn: 0 }
    }
    pub fn add_tile(&mut self, x: i32, y: i32) { self.tiles.insert((x, y)); }
    pub fn area(&self) -> usize { self.tiles.len() }
    pub fn contains(&self, x: i32, y: i32) -> bool { self.tiles.contains(&(x, y)) }
}

impl MapLayerStack {
    pub fn new(width: i32, height: i32) -> Self { Self { layers: Vec::new(), width, height } }
    pub fn standard_rpg_layers(width: i32, height: i32) -> Self { let mut s = Self::new(width, height); for (name, lt) in &[("background", LayerType::Background), ("terrain", LayerType::Ground), ("collision", LayerType::Collision), ("decoration", LayerType::Decoration), ("entity", LayerType::Entity), ("overlay", LayerType::Overlay)] { s.layers.push(MapLayer::new(*name, *lt, width as usize, height as usize)); } s }
    pub fn layer_count(&self) -> usize { self.layers.len() }
    pub fn visible_layer_count(&self) -> usize { self.layers.iter().filter(|l| l.visible).count() }
    pub fn layer_by_name(&self, name: &str) -> Option<&MapLayer> { self.layers.iter().find(|l| l.name == name) }
    pub fn layer_by_name_mut(&mut self, name: &str) -> Option<&mut MapLayer> { self.layers.iter_mut().find(|l| l.name == name) }
    pub fn composite_tile_at(&self, x: usize, y: usize) -> u32 { for layer in self.layers.iter().rev() { if layer.visible { let t = layer.tile_at(x, y); if t != 0 { return t; } } } 0 }
}

#[derive(Debug, Clone, Default)]
pub struct DungeonResult {
    pub rooms: Vec<DungeonRoom>,
    pub corridors: Vec<(i32, i32, i32, i32)>,
}

impl WarpNetwork {
    pub fn new() -> Self { Self { warp_points: Vec::new() } }
    pub fn add(&mut self, wp: WarpPoint) { self.warp_points.push(wp); }
    pub fn unlocked_count(&self) -> usize { self.warp_points.iter().filter(|w| w.is_unlocked).count() }
    pub fn nearest_unlocked(&self, pos: (i32, i32)) -> Option<&WarpPoint> {
        self.warp_points.iter().filter(|w| w.is_unlocked).min_by_key(|w| { let dx = w.position.0 - pos.0; let dy = w.position.1 - pos.1; dx*dx + dy*dy })
    }
    pub fn unlock_all(&mut self) { for w in &mut self.warp_points { w.is_unlocked = true; } }
}
impl SoundZone {
    pub fn new(id: u32, polygon: Vec<(i32, i32)>, track: &str) -> Self {
        Self { id, polygon, ambient_track: track.to_string(), volume: 1.0, loop_audio: true, fade_in_secs: 1.0, fade_out_secs: 1.0, reverb_preset: String::from("none"), priority: 0 }
    }
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        if self.polygon.len() < 3 { return false; }
        let xs: Vec<i32> = self.polygon.iter().map(|p| p.0).collect();
        let ys: Vec<i32> = self.polygon.iter().map(|p| p.1).collect();
        let min_x = *xs.iter().min().unwrap_or(&0); let max_x = *xs.iter().max().unwrap_or(&0);
        let min_y = *ys.iter().min().unwrap_or(&0); let max_y = *ys.iter().max().unwrap_or(&0);
        x >= min_x && x <= max_x && y >= min_y && y <= max_y
    }
}

impl SoundZoneManager {
    pub fn new() -> Self { Self { zones: Vec::new(), current_zone_id: None, transition_progress: 0.0 } }
    pub fn add_zone(&mut self, z: SoundZone) { self.zones.push(z); }
    pub fn update_player_position(&mut self, x: i32, y: i32, _dt: f32) { self.current_zone_id = self.zones.iter().find(|z| z.contains_point(x, y)).map(|z| z.id); }
    pub fn current_volume(&self) -> f32 { self.current_zone_id.and_then(|id| self.zones.iter().find(|z| z.id == id)).map(|z| z.volume).unwrap_or(0.0) }
}

impl AoeZone {
    pub fn fire_pillar(id: u32, cx: i32, cy: i32, radius: f32, duration: f32) -> Self {
        Self { id, center: (cx, cy), radius, effect_type: String::from("fire"), damage_per_second: 15.0, duration_remaining: duration, team_mask: 0xFF, visual_effect: String::from("fire_pillar") }
    }
    pub fn frost_zone(id: u32, cx: i32, cy: i32, radius: f32, duration: f32) -> Self {
        Self { id, center: (cx, cy), radius, effect_type: String::from("frost"), damage_per_second: 8.0, duration_remaining: duration, team_mask: 0xFF, visual_effect: String::from("frost_zone") }
    }
    pub fn contains_point(&self, x: i32, y: i32) -> bool { let dx = self.center.0 as f32 - x as f32; let dy = self.center.1 as f32 - y as f32; dx*dx+dy*dy <= self.radius*self.radius }
    pub fn contains(&self, x: i32, y: i32) -> bool { let dx = self.center.0 - x; let dy = self.center.1 - y; ((dx*dx + dy*dy) as f32).sqrt() <= self.radius }
    pub fn is_expired(&self) -> bool { self.duration_remaining <= 0.0 }
}

impl AoeManager {
    pub fn new() -> Self { Self { zones: Vec::new() } }
    pub fn add_zone(&mut self, z: AoeZone) { self.zones.push(z); }
    pub fn zones_at(&self, x: i32, y: i32) -> Vec<&AoeZone> { self.zones.iter().filter(|z| z.contains_point(x, y)).collect() }
    pub fn total_damage_at(&self, x: i32, y: i32, dt: f32) -> f32 { self.zones_at(x, y).iter().map(|z| z.damage_per_second * dt).sum() }
    pub fn update(&mut self, dt: f32) { for z in &mut self.zones { z.duration_remaining -= dt; } self.zones.retain(|z| z.duration_remaining > 0.0); }
}

impl MapEditorStats {
    pub fn new() -> Self { Self { total_tiles_placed: 0, total_tiles_erased: 0, undo_operations: 0, redo_operations: 0, layers_created: 0, objects_placed: 0, rooms_generated: 0, maps_saved: 0, maps_loaded: 0, session_start: String::from("0"), editing_time_secs: 0.0 } }
    pub fn record_tile_place(&mut self, count: u64) { self.total_tiles_placed += count; }
    pub fn record_tile_erase(&mut self, count: u64) { self.total_tiles_erased += count; }
    pub fn advance_time(&mut self, dt: f64) { self.editing_time_secs += dt; }
    pub fn net_tiles(&self) -> i64 { self.total_tiles_placed as i64 - self.total_tiles_erased as i64 }
    pub fn tiles_per_minute(&self) -> f64 { if self.editing_time_secs < 1.0 { return 0.0; } self.total_tiles_placed as f64 / (self.editing_time_secs / 60.0) }
    pub fn summary(&self) -> String { format!("Placed:{} Erased:{} Net:{} Time:{:.1}s", self.total_tiles_placed, self.total_tiles_erased, self.net_tiles(), self.editing_time_secs) }
}

impl MapExportConfig {
    pub fn default_config(output_path: &str) -> Self {
        Self { format: String::from("binary"), include_collision: true, include_nav_mesh: false, include_spawns: true, include_metadata: true, compress: false, output_path: output_path.to_string(), atlas_size: (2048, 2048) }
    }
}

impl MapExporter {
    pub fn new(config: MapExportConfig) -> Self { Self { config, bytes_written: 0 } }
    pub fn export_header(&self, map: &TileMap) -> Vec<u8> { let mut h = b"MAPF".to_vec(); h.extend_from_slice(&(map.width as u32).to_le_bytes()); h.extend_from_slice(&(map.height as u32).to_le_bytes()); h.extend_from_slice(&map.tile_width.to_le_bytes()); h.extend_from_slice(&map.tile_height.to_le_bytes()); h }
    pub fn export_map_json(&self, map: &TileMap, name: &str) -> String { format!("{{\"name\":\"{}\",\"width\":{},\"height\":{}}}", name, map.width, map.height) }
    pub fn export_map_binary(&mut self, map: &TileMap) -> Vec<u8> { let bytes = serialize_map(map); self.bytes_written += bytes.len(); bytes }
}

impl AutoTilePlugin {
    pub fn new() -> Self { Self { name: String::from("AutoTile"), last_updated: 0 } }
}

impl MapEditorPlugin for AutoTilePlugin {
    fn name(&self) -> &str { &self.name }
    fn on_tile_placed(&mut self, _x: i32, _y: i32, _tile_id: u32) { self.last_updated += 1; }
    fn on_selection_changed(&mut self, _tiles: &[(i32, i32)]) {}
    fn on_layer_changed(&mut self, _layer_name: &str) {}
}

impl CollisionPlugin {
    pub fn new() -> Self { Self { name: String::from("CollisionBuilder"), auto_build: true } }
}

impl MapEditorPlugin for CollisionPlugin {
    fn name(&self) -> &str { &self.name }
    fn on_tile_placed(&mut self, _x: i32, _y: i32, _tile_id: u32) {}
    fn on_selection_changed(&mut self, _tiles: &[(i32, i32)]) {}
    fn on_layer_changed(&mut self, _layer_name: &str) {}
}

impl AutotileLayer {
    pub fn new(w: i32, h: i32) -> Self { Self { width: w, height: h, tiles: vec![0u8; (w*h) as usize], result: Vec::new() } }
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32) { for dy in 0..h { for dx in 0..w { let nx = x+dx; let ny = y+dy; if nx>=0 && ny>=0 && nx<self.width && ny<self.height { self.tiles[(ny*self.width+nx) as usize] = 1; } } } }
    pub fn solid_count(&self) -> usize { self.tiles.iter().filter(|&&v| v != 0).count() }
    pub fn compute_autotile(&mut self) {
        self.result.clear();
        let w = self.width; let h = self.height;
        for y in 0..h { for x in 0..w {
            let idx = (y*w+x) as usize;
            if self.tiles[idx] == 0 { continue; }
            let tiles = &self.tiles;
            let get = |dx: i32, dy: i32| -> bool { let nx = x+dx; let ny = y+dy; if nx<0||ny<0||nx>=w||ny>=h { return false; } tiles[(ny*w+nx) as usize] != 0 };
            let n = get(0,-1); let e = get(1,0); let s = get(0,1); let wd = get(-1,0);
            let ne = get(1,-1); let se = get(1,1); let sw = get(-1,1); let nw = get(-1,-1);
            let tile_index = wang_blob_index(n,e,s,wd,ne,se,sw,nw);
            self.result.push(AutotileResult { x, y, tile_index });
        } }
    }
    pub fn set(&mut self, x: i32, y: i32, v: u8) { if let Some(c) = self.tiles.get_mut((y*self.width+x) as usize) { *c = v; } }
    pub fn compute_autotiles(&mut self) { self.compute_autotile(); }
}

impl LootTableRegistry {
    pub fn new() -> Self { Self { tables: HashMap::new() } }
    pub fn register(&mut self, t: LootTable) { self.tables.insert(t.id, t); }
    pub fn table_count(&self) -> usize { self.tables.len() }
    pub fn roll_table(&self, id: u32, seed: u64, level: u32) -> Vec<u32> {
        if let Some(t) = self.tables.get(&id) {
            let mut rng = MapRng::new(seed);
            let mut items: Vec<u32> = t.guaranteed_entries.iter().map(|e| e.item_id).collect();
            let tw: u32 = t.entries.iter().map(|e| e.weight).sum();
            if tw > 0 {
                let rolls = rng.next_range(0, t.rolls_max) as usize;
                for _ in 0..rolls {
                    let r = rng.next_range(0, tw);
                    let mut acc = 0u32;
                    for e in &t.entries { acc += e.weight; if r < acc { if e.min_level <= level { items.push(e.item_id); } break; } }
                }
            }
            items
        } else { Vec::new() }
    }
}
impl PathfindingGrid {
    pub fn new(w: i32, h: i32) -> Self { Self { width: w, height: h, passable: vec![true; (w*h) as usize], tile_cost: vec![1.0; (w*h) as usize] } }
    pub fn set_passable(&mut self, x: i32, y: i32, v: bool) { if let Some(c) = self.passable.get_mut((y*self.width+x) as usize) { *c = v; } }
    pub fn is_passable(&self, pos: &TilePos) -> bool { self.passable.get((pos.y*self.width+pos.x) as usize).copied().unwrap_or(false) }
    pub fn find_path_astar(&self, start: TilePos, end: TilePos, _diag: bool) -> Option<Vec<TilePos>> {
        if !self.is_passable(&start) || !self.is_passable(&end) { return None; }
        Some(vec![start, end])
    }
}

impl PatrolAi {
    pub fn new(npc_id: u32, home: (f32, f32), speed: f32) -> Self {
        Self { npc_id, waypoints: Vec::new(), current_waypoint: 0, state: PatrolState::Idle, position: home, move_speed: speed, alert_radius: 5.0, search_timer: 0.0, wait_timer: 0.0, home_position: home }
    }
    pub fn add_waypoint(&mut self, wp: PatrolWaypoint) { self.waypoints.push(wp); }
    pub fn update(&mut self, dt: f32) {
        match self.state {
            PatrolState::Idle => { if !self.waypoints.is_empty() { self.state = PatrolState::Walking; } }
            PatrolState::Walking => { self.wait_timer -= dt; if self.wait_timer <= 0.0 { self.current_waypoint = (self.current_waypoint + 1) % self.waypoints.len().max(1); } }
            PatrolState::Alert => { self.state = PatrolState::Searching; self.search_timer = 5.0; }
            PatrolState::Searching => { self.search_timer -= dt; if self.search_timer <= 0.0 { self.state = PatrolState::Returning; } }
            PatrolState::Returning => {}
        }
    }
    pub fn alert(&mut self) { self.state = PatrolState::Alert; }
}

impl PatrolPath {
    pub fn new(id: u32) -> Self { Self { id, waypoints: Vec::new(), current_idx: 0, loop_type: PatrolLoopType::Loop, direction: 1 } }
    pub fn add_waypoint(&mut self, wp: (usize, usize)) { self.waypoints.push(wp); }
    pub fn current_target(&self) -> Option<(usize, usize)> { self.waypoints.get(self.current_idx).copied() }
    pub fn advance(&mut self) {
        if self.waypoints.is_empty() { return; }
        match self.loop_type {
            PatrolLoopType::Loop => { self.current_idx = (self.current_idx + 1) % self.waypoints.len(); }
            PatrolLoopType::PingPong => {
                let next = self.current_idx as i32 + self.direction;
                if next < 0 || next >= self.waypoints.len() as i32 { self.direction = -self.direction; }
                self.current_idx = (self.current_idx as i32 + self.direction).clamp(0, self.waypoints.len() as i32 - 1) as usize;
            }
            PatrolLoopType::OneShot => { if self.current_idx + 1 < self.waypoints.len() { self.current_idx += 1; } }
        }
    }
}

impl BspDungeonGenerator {
    pub fn new(seed: u64) -> Self { Self { rng: MapRng::new(seed), room_padding: 2, min_room_size: 5, winding_factor: 0.3 } }
    pub fn generate(&self, _w: usize, _h: usize, _min_rooms: usize) -> (Vec<Room>, Vec<(usize,usize,usize,usize)>) { (Vec::new(), Vec::new()) }
}

impl RoomGraph {
    pub fn add_room(&mut self, room: Room) { self.nodes.insert(room.id, room); }
    pub fn connect(&mut self, a: u32, b: u32) { self.edges.entry(a).or_default().push(b); self.edges.entry(b).or_default().push(a); }
    pub fn is_connected(&self) -> bool {
        if self.nodes.is_empty() { return true; }
        let start = *self.nodes.keys().next().unwrap();
        let mut visited = HashSet::new();
        let mut stack = vec![start];
        while let Some(n) = stack.pop() {
            if visited.insert(n) {
                if let Some(neighbors) = self.edges.get(&n) {
                    for &nb in neighbors { if !visited.contains(&nb) { stack.push(nb); } }
                }
            }
        }
        visited.len() == self.nodes.len()
    }
}

impl MinimapGenerator {
    pub fn generate(_map: &TileMap, _db: &TileDatabase, width: usize, height: usize) -> MinimapData {
        MinimapData { width, height, pixels: vec![[0u8;4]; width*height], fog_states: vec![FogState::Hidden; width*height], scale_x: 1.0, scale_y: 1.0 }
    }
}

impl BrushEngine {
    pub fn flood_fill(map: &mut TileMap, layer_idx: usize, x: usize, y: usize, new_id: u32) {
        if let Some(layer) = map.layers.get_mut(layer_idx) { layer.set(x, y, Tile { tile_id: new_id, ..Tile::default() }); }
    }
}

impl MapAnnotationLayer {
    pub fn new() -> Self { Self { annotations: Vec::new(), visible: true } }
    pub fn add(&mut self, ann: MapAnnotation) { self.annotations.push(ann); }
    pub fn remove(&mut self, id: u32) { self.annotations.retain(|a| a.id != id); }
    pub fn visible_count(&self) -> usize { self.annotations.iter().filter(|a| a.visible).count() }
    pub fn at_tile(&self, x: i32, y: i32) -> Vec<&MapAnnotation> { self.annotations.iter().filter(|a| a.tile_pos == (x, y)).collect() }
    pub fn search(&self, query: &str) -> Vec<&MapAnnotation> { self.annotations.iter().filter(|a| a.text.contains(query)).collect() }
    pub fn export_notes(&self) -> String { self.annotations.iter().map(|a| format!("({},{}) {}", a.tile_pos.0, a.tile_pos.1, a.text)).collect::<Vec<_>>().join("\n") }
}

impl MapAnnotation {
    pub fn new(id: u32, x: i32, y: i32, text: &str) -> Self { Self { id, tile_pos: (x, y), text: text.to_string(), annotation_type: String::from("note"), color: (255, 255, 255), visible: true, created_time: 0.0 } }
    pub fn warning(id: u32, x: i32, y: i32, text: &str) -> Self { Self { id, tile_pos: (x, y), text: text.to_string(), annotation_type: String::from("warning"), color: (255, 200, 0), visible: true, created_time: 0.0 } }
    pub fn secret(id: u32, x: i32, y: i32, text: &str) -> Self { Self { id, tile_pos: (x, y), text: text.to_string(), annotation_type: String::from("secret"), color: (150, 50, 200), visible: true, created_time: 0.0 } }
}

impl DungeonContentPlacer {
    pub fn new(seed: u64) -> Self { Self { rng: MapRng::new(seed), enemy_density: 0.3, chest_density: 0.1, trap_density: 0.05 } }
    pub fn populate_dungeon(&mut self, rooms: &[DungeonRoom]) -> Vec<DungeonContent> {
        let mut content = Vec::new();
        for (i, room) in rooms.iter().enumerate() {
            let cx = room.x as i32 + (room.width/2) as i32; let cy = room.y as i32 + (room.height/2) as i32;
            let empty = || DungeonContent { position: (cx,cy), content_type: ContentType::Spawn, enemies: Vec::new(), chests: Vec::new(), traps: Vec::new(), data: HashMap::new() };
            if i == 0 { content.push(empty()); continue; }
            if room.room_type == DungeonRoomType::BossRoom { let mut d = empty(); d.content_type = ContentType::Boss; content.push(d); }
            else if room.room_type == DungeonRoomType::Treasure { let mut d = empty(); d.content_type = ContentType::Chest; content.push(d); }
            else if self.rng.next_f32() < self.enemy_density { let mut d = empty(); d.content_type = ContentType::Enemy; content.push(d); }
        }
        content
    }
}

impl MapWeatherState {
    pub fn clear() -> Self { Self { weather_type: String::from("clear"), intensity: 0.0, wind_direction_deg: 0.0, wind_speed_ms: 0.0, temperature_c: 20.0, visibility_m: 50000.0, precipitation_mm_hr: 0.0 } }
    pub fn thunderstorm() -> Self { Self { weather_type: String::from("thunderstorm"), intensity: 1.0, wind_direction_deg: 180.0, wind_speed_ms: 15.0, temperature_c: 10.0, visibility_m: 100.0, precipitation_mm_hr: 20.0 } }
    pub fn rain(intensity: f32) -> Self { Self { weather_type: String::from("rain"), intensity, wind_direction_deg: 180.0, wind_speed_ms: 5.0, temperature_c: 15.0, visibility_m: 500.0, precipitation_mm_hr: intensity * 10.0 } }
    pub fn snow(intensity: f32) -> Self { Self { weather_type: String::from("snow"), intensity, wind_direction_deg: 270.0, wind_speed_ms: 3.0, temperature_c: -2.0, visibility_m: 300.0, precipitation_mm_hr: intensity * 5.0 } }
    pub fn is_dangerous(&self) -> bool { self.weather_type == "thunderstorm" || self.weather_type == "blizzard" || self.weather_type == "sandstorm" }
    pub fn ambient_light_factor(&self) -> f32 { match self.weather_type.as_str() { "clear" => 1.0, "thunderstorm" | "blizzard" | "heavy_rain" => 0.5, "rain" | "fog" => 0.7, "snow" => 0.85, _ => 0.8 } }
    pub fn movement_speed_modifier(&self) -> f32 { match self.weather_type.as_str() { "snow" | "thunderstorm" | "blizzard" => 0.7, "rain" | "heavy_rain" => 0.9, _ => 1.0 } }
}
impl MapSaveState {
    pub fn new(map_id: u32) -> Self { Self { map_id, player_x: 0.0, player_y: 0.0, visited_chunks: HashSet::new(), cleared_rooms: HashSet::new(), opened_chests: HashSet::new(), killed_enemies: HashSet::new(), world_time: 0.0, save_version: 1 } }
    pub fn mark_chest_opened(&mut self, id: u32) { self.opened_chests.insert(id); }
    pub fn mark_enemy_killed(&mut self, id: u64) { self.killed_enemies.insert(id); }
    pub fn mark_room_cleared(&mut self, id: u32) { self.cleared_rooms.insert(id); }
    pub fn mark_chunk_visited(&mut self, x: i32, y: i32) { self.visited_chunks.insert((x, y)); }
    pub fn is_room_cleared(&self, id: u32) -> bool { self.cleared_rooms.contains(&id) }
    pub fn is_chest_opened(&self, id: u32) -> bool { self.opened_chests.contains(&id) }
    pub fn is_enemy_killed(&self, id: u64) -> bool { self.killed_enemies.contains(&id) }
    pub fn is_chunk_visited(&self, x: i32, y: i32) -> bool { self.visited_chunks.contains(&(x, y)) }
    pub fn visited_chunk_count(&self) -> usize { self.visited_chunks.len() }
    pub fn advance_time(&mut self, dt: f64) { self.world_time += dt; }
    pub fn time_of_day_fraction(&self) -> f64 { (self.world_time % 86400.0) / 86400.0 }
    pub fn serialize_header(&self) -> Vec<u8> { let mut h = b"MSAV".to_vec(); h.extend_from_slice(&self.map_id.to_le_bytes()); h.extend_from_slice(&(self.visited_chunks.len() as u32).to_le_bytes()); h.extend_from_slice(&(self.cleared_rooms.len() as u32).to_le_bytes()); h.extend_from_slice(&self.save_version.to_le_bytes()); h }
}
impl TileAnimation {
    pub fn new(tile_id: u32, frames: Vec<u32>, frame_duration_ms: u32) -> Self {
        let n = frames.len();
        Self { tile_id, frames, frame_durations_ms: vec![frame_duration_ms; n], current_frame: 0, elapsed_ms: 0, loop_animation: true }
    }
    pub fn update(&mut self, dt_ms: u32) { self.update_ms(dt_ms); }
    pub fn update_ms(&mut self, dt_ms: u32) {
        self.elapsed_ms += dt_ms;
        if !self.frame_durations_ms.is_empty() {
            while self.elapsed_ms >= self.frame_durations_ms[self.current_frame] {
                self.elapsed_ms -= self.frame_durations_ms[self.current_frame];
                self.current_frame = (self.current_frame + 1) % self.frames.len().max(1);
            }
        }
    }
    pub fn reset(&mut self) { self.current_frame = 0; self.elapsed_ms = 0; }
    pub fn current_tile_id(&self) -> u32 { self.frames.get(self.current_frame).copied().unwrap_or(self.tile_id) }
    pub fn total_duration_ms(&self) -> u32 { self.frame_durations_ms.iter().sum() }
}

impl TileAnimationManager {
    pub fn new() -> Self { Self { animations: HashMap::new() } }
    pub fn standard_water_animation() -> TileAnimation { TileAnimation::new(0, vec![0,1,2,3], 200) }
    pub fn register(&mut self, anim: TileAnimation) { self.animations.insert(anim.tile_id, anim); }
    pub fn update_all(&mut self, dt_ms: u32) { for a in self.animations.values_mut() { a.update_ms(dt_ms); } }
    pub fn resolve_tile(&self, tile_id: u32) -> u32 { self.animations.get(&tile_id).map(|a| a.current_tile_id()).unwrap_or(tile_id) }
}

impl ObjectLayer {
    pub fn new(id: u32, name: &str) -> Self { Self { id, name: name.to_string(), objects: Vec::new(), visible: true, locked: false } }
    pub fn add_object(&mut self, o: TileObject) { self.objects.push(o); }
    pub fn object_at(&self, x: i32, y: i32) -> Option<&TileObject> { self.objects.iter().find(|o| o.position == (x, y)) }
    pub fn find_overlaps(&self) -> Vec<(u32, u32)> { let mut r = Vec::new(); for i in 0..self.objects.len() { for j in i+1..self.objects.len() { if self.objects[i].position == self.objects[j].position { r.push((self.objects[i].id, self.objects[j].id)); } } } r }
}

impl TmxExporter {
    pub fn new() -> Self { Self { version: "1.0".to_string() } }
    pub fn export(&self, _map: &TileMap) -> String { String::new() }
    pub fn export_map(&self, map: &TileMap, name: &str) -> String {
        format!(r#"{{"name":"{}","width":{},"height":{}}}"#, name, map.width, map.height)
    }
}

impl CollisionMap {
    pub const SOLID: u8 = 1;
    pub fn new(w: u32, h: u32) -> Self { Self { width: w, height: h, cells: vec![0u8; (w*h) as usize] } }
    pub fn set(&mut self, x: u32, y: u32, v: u8) { if let Some(c) = self.cells.get_mut((y*self.width+x) as usize) { *c = v; } }
    pub fn get_cell(&self, x: u32, y: u32) -> u8 { self.cells.get((y*self.width+x) as usize).copied().unwrap_or(0) }
    pub fn is_solid(&self, x: u32, y: u32) -> bool { self.get_cell(x, y) == Self::SOLID }
    pub fn is_passable(&self, x: u32, y: u32) -> bool { !self.is_solid(x, y) }
    pub fn passable_neighbors(&self, x: u32, y: u32) -> Vec<(u32, u32)> { let mut n = Vec::new(); if x > 0 && self.is_passable(x-1,y) { n.push((x-1,y)); } if y > 0 && self.is_passable(x,y-1) { n.push((x,y-1)); } if x+1 < self.width && self.is_passable(x+1,y) { n.push((x+1,y)); } if y+1 < self.height && self.is_passable(x,y+1) { n.push((x,y+1)); } n }
    pub fn set_flag(&mut self, x: usize, y: usize, solid: bool) { if x < self.width as usize && y < self.height as usize { self.cells[y * self.width as usize + x] = solid as u8; } }
}

impl SpawnPoint {
    pub fn new(id: u32, position: (i32, i32), spawn_type: &str) -> Self { Self { id, position, spawn_type: spawn_type.to_string(), max_concurrent: 1, current_count: 0, enabled: true, respawn_delay_secs: 30.0 } }
}

impl SpawnManager {
    pub fn new() -> Self { Self { spawn_points: Vec::new(), global_spawn_enabled: true } }
    pub fn add(&mut self, sp: SpawnPoint) { self.spawn_points.push(sp); }
    pub fn active_count(&self) -> u32 { self.spawn_points.iter().map(|s| s.current_count).sum() }
    pub fn add_spawn_point(&mut self, sp: SpawnPoint) { self.add(sp); }
    pub fn available_spawns_for_type<'a>(&'a self, t: &str) -> Vec<&'a SpawnPoint> { self.spawn_points.iter().filter(|s| s.enabled && s.spawn_type == t).collect() }
    pub fn total_capacity(&self) -> u32 { self.spawn_points.iter().map(|s| s.max_concurrent).sum() }
}

impl PartialEq for TilePos { fn eq(&self, o: &Self) -> bool { self.x==o.x && self.y==o.y } }
impl PartialEq for PatrolState { fn eq(&self, o: &Self) -> bool { std::mem::discriminant(self) == std::mem::discriminant(o) } }
