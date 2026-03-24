//! Shared terrain types used across submodules.
//!
//! This module defines the core data structures shared between the terrain
//! submodules to avoid circular imports.

use glam::Vec3;
use crate::terrain::heightmap::HeightMap;
use crate::terrain::biome::BiomeMap;
use crate::terrain::vegetation::VegetationSystem;

// ── ChunkCoord ────────────────────────────────────────────────────────────────

/// Grid coordinate for a terrain chunk. (chunk_x, chunk_z) in chunk-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ChunkCoord(pub i32, pub i32);

impl ChunkCoord {
    /// Neighbor coordinates in 4 cardinal directions.
    pub fn neighbors_4(self) -> [ChunkCoord; 4] {
        [
            ChunkCoord(self.0 - 1, self.1),
            ChunkCoord(self.0 + 1, self.1),
            ChunkCoord(self.0, self.1 - 1),
            ChunkCoord(self.0, self.1 + 1),
        ]
    }

    /// Neighbor coordinates in 8 directions (cardinal + diagonal).
    pub fn neighbors_8(self) -> [ChunkCoord; 8] {
        [
            ChunkCoord(self.0 - 1, self.1 - 1),
            ChunkCoord(self.0,     self.1 - 1),
            ChunkCoord(self.0 + 1, self.1 - 1),
            ChunkCoord(self.0 - 1, self.1),
            ChunkCoord(self.0 + 1, self.1),
            ChunkCoord(self.0 - 1, self.1 + 1),
            ChunkCoord(self.0,     self.1 + 1),
            ChunkCoord(self.0 + 1, self.1 + 1),
        ]
    }

    /// Chebyshev distance to another chunk (max of abs differences).
    pub fn chebyshev_distance(self, other: ChunkCoord) -> i32 {
        (self.0 - other.0).abs().max((self.1 - other.1).abs())
    }

    /// Euclidean distance in chunk-space.
    pub fn euclidean_distance(self, other: ChunkCoord) -> f32 {
        let dx = (self.0 - other.0) as f32;
        let dz = (self.1 - other.1) as f32;
        (dx * dx + dz * dz).sqrt()
    }

    /// Convert chunk coord to world-space center position.
    pub fn to_world_pos(self, chunk_size: f32) -> Vec3 {
        Vec3::new(
            (self.0 as f32 + 0.5) * chunk_size,
            0.0,
            (self.1 as f32 + 0.5) * chunk_size,
        )
    }

    /// Distance from this chunk's center to an arbitrary world position.
    pub fn distance_to_world_pos(self, world_pos: Vec3, chunk_size: f32) -> f32 {
        let center = self.to_world_pos(chunk_size);
        let dx = center.x - world_pos.x;
        let dz = center.z - world_pos.z;
        (dx * dx + dz * dz).sqrt()
    }

    /// Chunk coord from world position.
    pub fn from_world_pos(world_pos: Vec3, chunk_size: f32) -> Self {
        ChunkCoord(
            (world_pos.x / chunk_size).floor() as i32,
            (world_pos.z / chunk_size).floor() as i32,
        )
    }

    /// True if this coord is within `radius` chunks of `other`.
    pub fn within_radius(self, other: ChunkCoord, radius: i32) -> bool {
        self.chebyshev_distance(other) <= radius
    }
}

impl std::fmt::Display for ChunkCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

// ── ChunkState ────────────────────────────────────────────────────────────────

/// Current lifecycle state of a terrain chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkState {
    /// Chunk is queued for generation.
    Pending,
    /// Chunk is currently being generated on a worker thread.
    Generating,
    /// Chunk data is ready for use.
    Ready,
    /// Chunk is staged for eviction from cache.
    Evicting,
    /// Chunk has been serialized to disk.
    Serialized,
}

// ── TerrainConfig ─────────────────────────────────────────────────────────────

/// Top-level configuration for the terrain system.
#[derive(Clone, Debug)]
pub struct TerrainConfig {
    /// Size of each chunk in terrain cells (e.g. 64, 128, 256).
    pub chunk_size:    usize,
    /// Number of chunks visible in each direction from the camera.
    pub view_distance: usize,
    /// Number of LOD levels (1 = no LOD, 4 = aggressive).
    pub lod_levels:    usize,
    /// World generation seed.
    pub seed:          u64,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            chunk_size:    64,
            view_distance: 8,
            lod_levels:    4,
            seed:          12345,
        }
    }
}

impl TerrainConfig {
    pub fn new(chunk_size: usize, view_distance: usize, lod_levels: usize, seed: u64) -> Self {
        Self { chunk_size, view_distance, lod_levels, seed }
    }
}

// ── TerrainChunk ──────────────────────────────────────────────────────────────

/// A single terrain chunk: one tile of the infinite world grid.
pub struct TerrainChunk {
    pub coord:       ChunkCoord,
    pub heightmap:   HeightMap,
    pub biome_map:   Option<BiomeMap>,
    pub vegetation:  Option<VegetationSystem>,
    pub lod_level:   u8,
    pub state:       ChunkState,
    pub last_used:   std::time::Instant,
    pub seed:        u64,
}

impl TerrainChunk {
    /// World-space bounds: (min, max) corner of this chunk.
    pub fn world_bounds(&self, chunk_size: f32) -> (Vec3, Vec3) {
        let min_x = self.coord.0 as f32 * chunk_size;
        let min_z = self.coord.1 as f32 * chunk_size;
        let max_x = min_x + chunk_size;
        let max_z = min_z + chunk_size;
        let min_h = self.heightmap.min_value() * 100.0;
        let max_h = self.heightmap.max_value() * 100.0;
        (Vec3::new(min_x, min_h, min_z), Vec3::new(max_x, max_h, max_z))
    }

    /// Is the chunk fully ready for rendering?
    pub fn is_ready(&self) -> bool { self.state == ChunkState::Ready }

    /// Seconds since this chunk was last accessed.
    pub fn age_seconds(&self) -> f32 {
        self.last_used.elapsed().as_secs_f32()
    }

    /// Approximate memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        let hm = self.heightmap.data.len() * 4;
        let bm = self.biome_map.as_ref().map(|b| b.biomes.len()).unwrap_or(0) * 1;
        let veg = self.vegetation.as_ref().map(|v| v.instances.len() * 64).unwrap_or(0);
        std::mem::size_of::<TerrainChunk>() + hm + bm + veg
    }
}

impl std::fmt::Debug for TerrainChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerrainChunk")
            .field("coord", &self.coord)
            .field("lod_level", &self.lod_level)
            .field("state", &self.state)
            .field("heightmap_size", &(self.heightmap.width, self.heightmap.height))
            .finish()
    }
}

// ── ChunkGrid ─────────────────────────────────────────────────────────────────

/// A 2D grid of chunk coordinates within a rectangular region.
#[derive(Clone, Debug)]
pub struct ChunkGrid {
    pub origin: ChunkCoord,
    pub width:  i32,
    pub height: i32,
}

impl ChunkGrid {
    /// Create a grid of chunks centred on `center` with given half-extent.
    pub fn around(center: ChunkCoord, half_extent: i32) -> Self {
        Self {
            origin: ChunkCoord(center.0 - half_extent, center.1 - half_extent),
            width:  half_extent * 2 + 1,
            height: half_extent * 2 + 1,
        }
    }

    /// Iterate all coords in the grid.
    pub fn iter(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        let ox = self.origin.0;
        let oy = self.origin.1;
        let w  = self.width;
        let h  = self.height;
        (0..h).flat_map(move |dy| {
            (0..w).map(move |dx| ChunkCoord(ox + dx, oy + dy))
        })
    }

    /// Total number of chunks in this grid.
    pub fn count(&self) -> usize { (self.width * self.height) as usize }

    /// Test if a coord is within this grid.
    pub fn contains(&self, c: ChunkCoord) -> bool {
        c.0 >= self.origin.0
            && c.0 < self.origin.0 + self.width
            && c.1 >= self.origin.1
            && c.1 < self.origin.1 + self.height
    }

    /// Convert a grid-relative (col, row) to ChunkCoord.
    pub fn at(&self, col: i32, row: i32) -> ChunkCoord {
        ChunkCoord(self.origin.0 + col, self.origin.1 + row)
    }

    /// Coords sorted by distance to center.
    pub fn sorted_by_distance(&self, center: ChunkCoord) -> Vec<ChunkCoord> {
        let mut coords: Vec<ChunkCoord> = self.iter().collect();
        coords.sort_by(|a, b| {
            let da = a.euclidean_distance(center);
            let db = b.euclidean_distance(center);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        coords
    }
}

// ── ChunkBounds ───────────────────────────────────────────────────────────────

/// World-space AABB of a chunk.
#[derive(Clone, Copy, Debug)]
pub struct ChunkBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl ChunkBounds {
    pub fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }

    pub fn from_chunk(coord: ChunkCoord, chunk_size: f32, height_scale: f32) -> Self {
        let x0 = coord.0 as f32 * chunk_size;
        let z0 = coord.1 as f32 * chunk_size;
        Self {
            min: Vec3::new(x0, 0.0, z0),
            max: Vec3::new(x0 + chunk_size, height_scale, z0 + chunk_size),
        }
    }

    /// Center of the bounding box.
    pub fn center(&self) -> Vec3 { (self.min + self.max) * 0.5 }

    /// Half-extents of the bounding box.
    pub fn half_extents(&self) -> Vec3 { (self.max - self.min) * 0.5 }

    /// True if this box intersects another.
    pub fn intersects(&self, other: &ChunkBounds) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// True if a point is inside this box.
    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    /// Signed distance from a point to this box (negative = inside).
    pub fn sdf(&self, p: Vec3) -> f32 {
        let q = Vec3::new(
            (p.x - self.center().x).abs() - self.half_extents().x,
            (p.y - self.center().y).abs() - self.half_extents().y,
            (p.z - self.center().z).abs() - self.half_extents().z,
        );
        let max_q = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0));
        max_q.length() + q.x.max(q.y).max(q.z).min(0.0)
    }
}

// ── ChunkHandle ───────────────────────────────────────────────────────────────

/// A lightweight reference-counted handle to a chunk (for use outside the cache).
#[derive(Clone, Debug)]
pub struct ChunkHandle {
    pub coord:     ChunkCoord,
    pub lod_level: u8,
    pub state:     ChunkState,
    /// Version counter for detecting stale handles.
    pub version:   u32,
}

impl ChunkHandle {
    pub fn new(coord: ChunkCoord, lod_level: u8) -> Self {
        Self { coord, lod_level, state: ChunkState::Pending, version: 0 }
    }

    pub fn is_ready(&self) -> bool { self.state == ChunkState::Ready }

    pub fn advance_version(&mut self) { self.version = self.version.wrapping_add(1); }
}

// ── TerrainRegion ─────────────────────────────────────────────────────────────

/// Describes a named rectangular region of the world.
#[derive(Clone, Debug)]
pub struct TerrainRegion {
    pub name:   String,
    pub min:    ChunkCoord,
    pub max:    ChunkCoord,
    pub biome_hint: crate::terrain::biome::BiomeType,
}

impl TerrainRegion {
    pub fn new(name: &str, min: ChunkCoord, max: ChunkCoord) -> Self {
        Self {
            name: name.to_string(),
            min, max,
            biome_hint: crate::terrain::biome::BiomeType::Grassland,
        }
    }

    pub fn contains(&self, coord: ChunkCoord) -> bool {
        coord.0 >= self.min.0 && coord.0 <= self.max.0
            && coord.1 >= self.min.1 && coord.1 <= self.max.1
    }

    pub fn area(&self) -> usize {
        let w = (self.max.0 - self.min.0 + 1).max(0) as usize;
        let h = (self.max.1 - self.min.1 + 1).max(0) as usize;
        w * h
    }

    pub fn center(&self) -> ChunkCoord {
        ChunkCoord(
            (self.min.0 + self.max.0) / 2,
            (self.min.1 + self.max.1) / 2,
        )
    }
}

// ── WorldSeed ─────────────────────────────────────────────────────────────────

/// A structured seed for reproducible world generation.
#[derive(Clone, Debug)]
pub struct WorldSeed {
    pub base_seed:   u64,
    pub terrain_seed: u64,
    pub biome_seed:  u64,
    pub vegetation_seed: u64,
    pub weather_seed: u64,
    pub name:        String,
}

impl WorldSeed {
    pub fn from_u64(seed: u64) -> Self {
        Self {
            base_seed:       seed,
            terrain_seed:    seed.wrapping_mul(0x9e3779b97f4a7c15),
            biome_seed:      seed.wrapping_mul(0x6c62272e07bb0142),
            vegetation_seed: seed.wrapping_mul(0xbf58476d1ce4e5b9),
            weather_seed:    seed.wrapping_mul(0x94d049bb133111eb),
            name:            format!("World-{:016X}", seed),
        }
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Derive a per-chunk seed.
    pub fn chunk_seed(&self, coord: ChunkCoord) -> u64 {
        let cx = coord.0 as u64;
        let cz = coord.1 as u64;
        self.terrain_seed
            .wrapping_add(cx.wrapping_mul(0x9e3779b97f4a7c15))
            .wrapping_add(cz.wrapping_mul(0x6c62272e07bb0142))
    }

    /// Derive a per-biome seed.
    pub fn biome_chunk_seed(&self, coord: ChunkCoord) -> u64 {
        let cx = coord.0 as u64;
        let cz = coord.1 as u64;
        self.biome_seed
            .wrapping_add(cx.wrapping_mul(0xbf58476d1ce4e5b9))
            .wrapping_add(cz.wrapping_mul(0x94d049bb133111eb))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod mod_types_tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_chunk_grid_count() {
        let grid = ChunkGrid::around(ChunkCoord(0, 0), 2);
        assert_eq!(grid.count(), 25); // 5x5
    }

    #[test]
    fn test_chunk_grid_iter() {
        let grid = ChunkGrid::around(ChunkCoord(0, 0), 1);
        let coords: Vec<ChunkCoord> = grid.iter().collect();
        assert_eq!(coords.len(), 9); // 3x3
        assert!(coords.contains(&ChunkCoord(0, 0)));
        assert!(coords.contains(&ChunkCoord(-1, -1)));
        assert!(coords.contains(&ChunkCoord(1, 1)));
    }

    #[test]
    fn test_chunk_grid_contains() {
        let grid = ChunkGrid::around(ChunkCoord(5, 5), 2);
        assert!(grid.contains(ChunkCoord(5, 5)));
        assert!(grid.contains(ChunkCoord(3, 3)));
        assert!(!grid.contains(ChunkCoord(0, 0)));
    }

    #[test]
    fn test_chunk_grid_sorted_by_distance() {
        let grid = ChunkGrid::around(ChunkCoord(0, 0), 2);
        let sorted = grid.sorted_by_distance(ChunkCoord(0, 0));
        assert_eq!(sorted[0], ChunkCoord(0, 0));
    }

    #[test]
    fn test_chunk_bounds_center() {
        let b = ChunkBounds::from_chunk(ChunkCoord(0, 0), 64.0, 100.0);
        let c = b.center();
        assert!((c.x - 32.0).abs() < 1e-4);
        assert!((c.z - 32.0).abs() < 1e-4);
    }

    #[test]
    fn test_chunk_bounds_intersects() {
        let b1 = ChunkBounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(64.0, 100.0, 64.0));
        let b2 = ChunkBounds::new(Vec3::new(32.0, 0.0, 32.0), Vec3::new(96.0, 100.0, 96.0));
        let b3 = ChunkBounds::new(Vec3::new(200.0, 0.0, 200.0), Vec3::new(264.0, 100.0, 264.0));
        assert!(b1.intersects(&b2));
        assert!(!b1.intersects(&b3));
    }

    #[test]
    fn test_chunk_bounds_contains_point() {
        let b = ChunkBounds::from_chunk(ChunkCoord(0, 0), 64.0, 100.0);
        assert!(b.contains_point(Vec3::new(32.0, 50.0, 32.0)));
        assert!(!b.contains_point(Vec3::new(100.0, 50.0, 32.0)));
    }

    #[test]
    fn test_chunk_handle() {
        let mut h = ChunkHandle::new(ChunkCoord(3, 4), 0);
        assert!(!h.is_ready());
        h.state = ChunkState::Ready;
        assert!(h.is_ready());
        h.advance_version();
        assert_eq!(h.version, 1);
    }

    #[test]
    fn test_terrain_region() {
        let r = TerrainRegion::new("Forest", ChunkCoord(0, 0), ChunkCoord(9, 9));
        assert_eq!(r.area(), 100);
        assert!(r.contains(ChunkCoord(5, 5)));
        assert!(!r.contains(ChunkCoord(10, 10)));
        assert_eq!(r.center(), ChunkCoord(4, 4));
    }

    #[test]
    fn test_world_seed() {
        let ws = WorldSeed::from_u64(12345).named("TestWorld");
        assert_eq!(ws.name, "TestWorld");
        assert_ne!(ws.terrain_seed, ws.biome_seed);
        let s1 = ws.chunk_seed(ChunkCoord(0, 0));
        let s2 = ws.chunk_seed(ChunkCoord(1, 0));
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_terrain_config_default() {
        let c = TerrainConfig::default();
        assert_eq!(c.chunk_size, 64);
        assert_eq!(c.view_distance, 8);
        assert_eq!(c.lod_levels, 4);
    }

    #[test]
    fn test_chunk_state_variants() {
        let states = [
            ChunkState::Pending, ChunkState::Generating, ChunkState::Ready,
            ChunkState::Evicting, ChunkState::Serialized,
        ];
        for s in states { let _ = s; }
    }
}
