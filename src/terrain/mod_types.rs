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
