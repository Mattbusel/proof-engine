//! Chunked terrain streaming system.
//!
//! Manages the loading, unloading, caching, and mesh generation of terrain
//! chunks around a moving viewer. Provides:
//!
//! - `ChunkCoord` — integer 2D chunk address
//! - `ChunkState` — lifecycle state machine (Unloaded/Loading/Loaded/Unloading)
//! - Priority queue ordered by distance-to-viewer
//! - Async-style load queue (single-threaded work queue, drain-per-frame)
//! - Mesh cache with LRU eviction
//! - Collision hull generation per chunk

use super::heightmap::{HeightMap, DiamondSquare, HydraulicErosion};
use std::collections::{HashMap, VecDeque, BinaryHeap};
use std::cmp::Ordering;

// ── TerrainMesh ──────────────────────────────────────────────────────────────

/// A simple triangle mesh generated from a heightmap.
#[derive(Clone, Debug)]
pub struct TerrainMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals:  Vec<[f32; 3]>,
    pub indices:  Vec<u32>,
}

// ── ChunkCoord ────────────────────────────────────────────────────────────────

/// Integer 2D address of a terrain chunk in chunk-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const ZERO: Self = Self { x: 0, z: 0 };

    pub fn new(x: i32, z: i32) -> Self { Self { x, z } }

    /// Manhattan distance between two chunk coordinates.
    pub fn manhattan(self, other: ChunkCoord) -> u32 {
        ((self.x - other.x).abs() + (self.z - other.z).abs()) as u32
    }

    /// Euclidean distance (in chunk units) between two chunks.
    pub fn distance(self, other: ChunkCoord) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dz = (self.z - other.z) as f32;
        (dx*dx + dz*dz).sqrt()
    }

    /// World-space center of this chunk given `chunk_size` world units per chunk.
    pub fn world_center(self, chunk_size: f32) -> [f32; 3] {
        let half = chunk_size * 0.5;
        [self.x as f32 * chunk_size + half, 0.0, self.z as f32 * chunk_size + half]
    }

    /// All 8 neighbors (including diagonals).
    pub fn neighbors_8(self) -> [ChunkCoord; 8] {
        [
            ChunkCoord::new(self.x - 1, self.z - 1),
            ChunkCoord::new(self.x,     self.z - 1),
            ChunkCoord::new(self.x + 1, self.z - 1),
            ChunkCoord::new(self.x - 1, self.z),
            ChunkCoord::new(self.x + 1, self.z),
            ChunkCoord::new(self.x - 1, self.z + 1),
            ChunkCoord::new(self.x,     self.z + 1),
            ChunkCoord::new(self.x + 1, self.z + 1),
        ]
    }

    /// Orthogonal neighbors only.
    pub fn neighbors_4(self) -> [ChunkCoord; 4] {
        [
            ChunkCoord::new(self.x,     self.z - 1),
            ChunkCoord::new(self.x,     self.z + 1),
            ChunkCoord::new(self.x - 1, self.z),
            ChunkCoord::new(self.x + 1, self.z),
        ]
    }
}

impl std::fmt::Display for ChunkCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.z)
    }
}

// ── ChunkState ────────────────────────────────────────────────────────────────

/// Lifecycle state of a terrain chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkState {
    /// Not in memory; no data loaded.
    Unloaded,
    /// Queued for loading; data generation has not started.
    Queued,
    /// Actively being generated/loaded (in-flight).
    Loading,
    /// Fully loaded; heightmap and mesh are available.
    Loaded,
    /// Marked for unloading on the next eviction pass.
    Unloading,
}

impl ChunkState {
    pub fn is_usable(self) -> bool {
        self == ChunkState::Loaded
    }

    pub fn is_pending(self) -> bool {
        matches!(self, ChunkState::Queued | ChunkState::Loading)
    }
}

// ── TerrainChunkData ──────────────────────────────────────────────────────────

/// All data for a single loaded terrain chunk.
#[derive(Clone, Debug)]
pub struct TerrainChunkData {
    pub coord:        ChunkCoord,
    pub heightmap:    HeightMap,
    pub vertices:     Vec<[f32; 3]>,
    pub collision:    CollisionHull,
    pub state:        ChunkState,
    pub seed:         u64,
    pub last_used_frame: u64,
}

impl TerrainChunkData {
    pub fn world_aabb(&self, chunk_size: f32, height_scale: f32) -> Aabb {
        let x0 = self.coord.x as f32 * chunk_size;
        let z0 = self.coord.z as f32 * chunk_size;
        let x1 = x0 + chunk_size;
        let z1 = z0 + chunk_size;
        let y_min = self.heightmap.data.iter().cloned().fold(f32::INFINITY,    f32::min) * height_scale;
        let y_max = self.heightmap.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max) * height_scale;
        Aabb { min: [x0, y_min, z0], max: [x1, y_max, z1] }
    }
}

// ── Axis-Aligned Bounding Box ─────────────────────────────────────────────────

/// Axis-aligned bounding box in world space.
#[derive(Clone, Debug)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn center(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn contains_xz(&self, x: f32, z: f32) -> bool {
        x >= self.min[0] && x <= self.max[0] &&
        z >= self.min[2] && z <= self.max[2]
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min[0] <= other.max[0] && self.max[0] >= other.min[0] &&
        self.min[1] <= other.max[1] && self.max[1] >= other.min[1] &&
        self.min[2] <= other.max[2] && self.max[2] >= other.min[2]
    }
}

// ── Collision Hull ────────────────────────────────────────────────────────────

/// A simplified collision representation for a terrain chunk.
///
/// Uses a low-resolution height grid suitable for physics queries.
#[derive(Clone, Debug)]
pub struct CollisionHull {
    /// Low-res height grid (collision_res x collision_res cells).
    pub heights:        Vec<f32>,
    pub resolution:     usize,
    /// World width/height covered by this hull.
    pub chunk_size:     f32,
    pub height_scale:   f32,
}

impl CollisionHull {
    /// Generate a collision hull from a heightmap.
    ///
    /// `resolution` controls how many cells wide the collision grid is.
    /// Typically 8–32 for performance.
    pub fn generate(heightmap: &HeightMap, chunk_size: f32, height_scale: f32, resolution: usize) -> Self {
        let res = resolution.max(2);
        let mut heights = Vec::with_capacity(res * res);

        for row in 0..res {
            for col in 0..res {
                let fx = col as f32 / (res - 1).max(1) as f32 * (heightmap.width  - 1) as f32;
                let fy = row as f32 / (res - 1).max(1) as f32 * (heightmap.height - 1) as f32;
                heights.push(heightmap.sample_bilinear(fx, fy) * height_scale);
            }
        }

        Self { heights, resolution: res, chunk_size, height_scale }
    }

    /// Sample height at world-space (x, z) relative to chunk origin.
    pub fn height_at_local(&self, lx: f32, lz: f32) -> f32 {
        let fx = (lx / self.chunk_size * (self.resolution - 1) as f32).clamp(0.0, (self.resolution - 1) as f32);
        let fz = (lz / self.chunk_size * (self.resolution - 1) as f32).clamp(0.0, (self.resolution - 1) as f32);
        let col = fx as usize;
        let row = fz as usize;
        let tx = fx - col as f32;
        let tz = fz - row as f32;
        let col1 = (col + 1).min(self.resolution - 1);
        let row1 = (row + 1).min(self.resolution - 1);
        let h00 = self.heights[row  * self.resolution + col];
        let h10 = self.heights[row  * self.resolution + col1];
        let h01 = self.heights[row1 * self.resolution + col];
        let h11 = self.heights[row1 * self.resolution + col1];
        let h0 = h00 + (h10 - h00) * tx;
        let h1 = h01 + (h11 - h01) * tx;
        h0 + (h1 - h0) * tz
    }

    /// Returns a set of triangles for this hull (for physics engine submission).
    pub fn triangles(&self) -> Vec<[[f32; 3]; 3]> {
        let res = self.resolution;
        let cell_w = self.chunk_size / (res - 1).max(1) as f32;
        let mut tris = Vec::with_capacity((res - 1) * (res - 1) * 2);

        for row in 0..res.saturating_sub(1) {
            for col in 0..res.saturating_sub(1) {
                let x0 = col as f32 * cell_w;
                let x1 = (col + 1) as f32 * cell_w;
                let z0 = row as f32 * cell_w;
                let z1 = (row + 1) as f32 * cell_w;
                let h00 = self.heights[row     * res + col];
                let h10 = self.heights[row     * res + col + 1];
                let h01 = self.heights[(row+1) * res + col];
                let h11 = self.heights[(row+1) * res + col + 1];

                tris.push([[x0,h00,z0], [x0,h01,z1], [x1,h10,z0]]);
                tris.push([[x1,h10,z0], [x0,h01,z1], [x1,h11,z1]]);
            }
        }

        tris
    }
}

// ── Priority Queue Item ───────────────────────────────────────────────────────

/// An item in the chunk load priority queue.
#[derive(Clone, Debug)]
struct PriorityItem {
    coord:    ChunkCoord,
    priority: u32, // Higher = more urgent
}

impl PartialEq for PriorityItem {
    fn eq(&self, other: &Self) -> bool { self.priority == other.priority }
}

impl Eq for PriorityItem {}

impl PartialOrd for PriorityItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

// ── Chunk Generator ───────────────────────────────────────────────────────────

/// Configuration for chunk generation.
#[derive(Clone, Debug)]
pub struct ChunkGenConfig {
    /// Number of cells per side (must be 2^n + 1 for diamond-square).
    pub cells_per_chunk:  usize,
    /// World units per chunk.
    pub chunk_size:       f32,
    /// World height scale (multiplier applied to [0,1] heights).
    pub height_scale:     f32,
    /// Diamond-square roughness.
    pub roughness:        f32,
    /// Number of hydraulic erosion iterations.
    pub erosion_iters:    u32,
    /// Number of LOD levels to generate.
    pub lod_levels:       u32,
    /// Resolution of the collision hull.
    pub collision_res:    usize,
    /// Base RNG seed — combined with coord for per-chunk seed.
    pub base_seed:        u64,
}

impl Default for ChunkGenConfig {
    fn default() -> Self {
        Self {
            cells_per_chunk: 65,
            chunk_size:      128.0,
            height_scale:    80.0,
            roughness:       0.55,
            erosion_iters:   200,
            lod_levels:      4,
            collision_res:   16,
            base_seed:       0xdeadbeef_cafebabe,
        }
    }
}

/// Generates terrain chunk data synchronously.
pub struct ChunkGenerator {
    pub config: ChunkGenConfig,
}

impl ChunkGenerator {
    pub fn new(config: ChunkGenConfig) -> Self { Self { config } }

    /// Derive a deterministic seed for a coordinate.
    fn chunk_seed(&self, coord: ChunkCoord) -> u64 {
        let mut h = self.config.base_seed;
        h ^= (coord.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
        h ^= (coord.z as i64 as u64).wrapping_mul(0x6c62272e07bb0142);
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h
    }

    /// Generate a complete chunk.
    pub fn generate(&self, coord: ChunkCoord) -> TerrainChunkData {
        let seed = self.chunk_seed(coord);

        // Generate heightmap via diamond-square
        let mut hm = DiamondSquare::generate(self.config.cells_per_chunk, self.config.roughness, seed);

        // Apply hydraulic erosion
        if self.config.erosion_iters > 0 {
            HydraulicErosion::erode(
                &mut hm,
                self.config.erosion_iters as usize,
                0.01,  // rain_amount
                4.0,   // sediment_capacity
                0.01,  // evaporation
                seed ^ 0x1234,
            );
        }

        let scale = self.config.chunk_size / (self.config.cells_per_chunk - 1) as f32;

        // Generate simple mesh vertices from heightmap
        let mut vertices = Vec::new();
        for y in 0..hm.height {
            for x in 0..hm.width {
                vertices.push([
                    x as f32 * scale,
                    hm.get(x, y) * self.config.height_scale,
                    y as f32 * scale,
                ]);
            }
        }

        // Generate collision hull
        let collision = CollisionHull::generate(
            &hm,
            self.config.chunk_size,
            self.config.height_scale,
            self.config.collision_res,
        );

        TerrainChunkData {
            coord,
            heightmap: hm,
            vertices,
            collision,
            state: ChunkState::Loaded,
            seed,
            last_used_frame: 0,
        }
    }
}

// ── Mesh Cache ────────────────────────────────────────────────────────────────

/// LRU mesh cache for terrain chunks.
///
/// Evicts least-recently-used chunks when capacity is exceeded.
pub struct MeshCache {
    pub capacity: usize,
    chunks:       HashMap<ChunkCoord, TerrainChunkData>,
    /// Access order: front = most recent, back = least recent.
    access_order: VecDeque<ChunkCoord>,
}

impl MeshCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            chunks:       HashMap::with_capacity(capacity + 1),
            access_order: VecDeque::with_capacity(capacity + 1),
        }
    }

    /// Insert or update a chunk. Evicts LRU if over capacity.
    pub fn insert(&mut self, data: TerrainChunkData) {
        let coord = data.coord;
        self.chunks.insert(coord, data);
        self.touch(coord);
        self.evict_if_needed();
    }

    /// Get a reference to a chunk, updating its LRU position.
    pub fn get(&mut self, coord: ChunkCoord) -> Option<&TerrainChunkData> {
        if self.chunks.contains_key(&coord) {
            self.touch(coord);
            self.chunks.get(&coord)
        } else {
            None
        }
    }

    /// Get without updating LRU (read-only peek).
    pub fn peek(&self, coord: ChunkCoord) -> Option<&TerrainChunkData> {
        self.chunks.get(&coord)
    }

    /// Remove a chunk explicitly.
    pub fn remove(&mut self, coord: ChunkCoord) -> Option<TerrainChunkData> {
        self.access_order.retain(|c| *c != coord);
        self.chunks.remove(&coord)
    }

    pub fn contains(&self, coord: ChunkCoord) -> bool {
        self.chunks.contains_key(&coord)
    }

    pub fn len(&self) -> usize { self.chunks.len() }
    pub fn is_empty(&self) -> bool { self.chunks.is_empty() }

    /// Iterate over all loaded chunks.
    pub fn iter(&self) -> impl Iterator<Item = (&ChunkCoord, &TerrainChunkData)> {
        self.chunks.iter()
    }

    fn touch(&mut self, coord: ChunkCoord) {
        self.access_order.retain(|c| *c != coord);
        self.access_order.push_front(coord);
    }

    fn evict_if_needed(&mut self) {
        while self.chunks.len() > self.capacity {
            if let Some(lru) = self.access_order.pop_back() {
                self.chunks.remove(&lru);
            } else {
                break;
            }
        }
    }

    /// Evict all chunks marked as Unloading.
    pub fn evict_unloading(&mut self) {
        let to_remove: Vec<ChunkCoord> = self.chunks.iter()
            .filter(|(_, v)| v.state == ChunkState::Unloading)
            .map(|(k, _)| *k)
            .collect();
        for coord in to_remove {
            self.remove(coord);
        }
    }
}

// ── Load Queue ────────────────────────────────────────────────────────────────

/// Async-style load queue: accepts load requests, drains N per frame.
pub struct LoadQueue {
    heap:       BinaryHeap<PriorityItem>,
    in_queue:   HashMap<ChunkCoord, u32>, // coord → priority
}

impl LoadQueue {
    pub fn new() -> Self {
        Self {
            heap:     BinaryHeap::new(),
            in_queue: HashMap::new(),
        }
    }

    /// Enqueue a chunk for loading with a given priority (higher = sooner).
    pub fn enqueue(&mut self, coord: ChunkCoord, priority: u32) {
        if let Some(existing) = self.in_queue.get_mut(&coord) {
            if priority <= *existing { return; }
            *existing = priority;
        } else {
            self.in_queue.insert(coord, priority);
        }
        self.heap.push(PriorityItem { coord, priority });
    }

    /// Dequeue up to `max_count` chunks for loading this frame.
    pub fn drain(&mut self, max_count: usize) -> Vec<ChunkCoord> {
        let mut result = Vec::with_capacity(max_count);
        while result.len() < max_count {
            match self.heap.pop() {
                None => break,
                Some(item) => {
                    // Skip stale entries (priority changed)
                    let current = self.in_queue.get(&item.coord).copied().unwrap_or(0);
                    if current != item.priority { continue; }
                    self.in_queue.remove(&item.coord);
                    result.push(item.coord);
                }
            }
        }
        result
    }

    /// Remove a chunk from the queue (e.g., it moved out of range).
    pub fn cancel(&mut self, coord: ChunkCoord) {
        self.in_queue.remove(&coord);
        // We leave stale entries in the heap; they'll be skipped in drain()
    }

    pub fn len(&self) -> usize { self.in_queue.len() }
    pub fn is_empty(&self) -> bool { self.in_queue.is_empty() }
}

impl Default for LoadQueue {
    fn default() -> Self { Self::new() }
}

// ── LOD Scheduler ─────────────────────────────────────────────────────────────

/// Determines which LOD level a chunk should be rendered at.
pub struct LodScheduler {
    /// Distance thresholds for each LOD level.
    /// `lod_distances[0]` = max distance for LOD 0 (highest detail).
    pub lod_distances: Vec<f32>,
}

impl LodScheduler {
    pub fn new(chunk_size: f32, max_lod: u32) -> Self {
        let mut dists = Vec::with_capacity(max_lod as usize + 1);
        for i in 0..=max_lod {
            dists.push(chunk_size * 2.0f32.powi(i as i32 + 1));
        }
        Self { lod_distances: dists }
    }

    /// Select the appropriate LOD for a chunk at `distance` world units.
    pub fn select_lod(&self, distance: f32) -> u32 {
        for (i, &threshold) in self.lod_distances.iter().enumerate() {
            if distance <= threshold {
                return i as u32;
            }
        }
        self.lod_distances.len() as u32
    }
}

// ── Visibility Set ─────────────────────────────────────────────────────────────

/// Tracks which chunks are visible this frame.
#[derive(Default)]
pub struct VisibilitySet {
    visible:     HashMap<ChunkCoord, u32>, // coord → LOD level
}

impl VisibilitySet {
    pub fn new() -> Self { Self::default() }

    pub fn mark_visible(&mut self, coord: ChunkCoord, lod: u32) {
        self.visible.insert(coord, lod);
    }

    pub fn is_visible(&self, coord: ChunkCoord) -> bool {
        self.visible.contains_key(&coord)
    }

    pub fn lod_for(&self, coord: ChunkCoord) -> Option<u32> {
        self.visible.get(&coord).copied()
    }

    pub fn clear(&mut self) { self.visible.clear(); }

    pub fn iter(&self) -> impl Iterator<Item = (&ChunkCoord, &u32)> {
        self.visible.iter()
    }

    pub fn len(&self) -> usize { self.visible.len() }
}

// ── Chunk Streaming Manager ───────────────────────────────────────────────────

/// Configuration for the streaming manager.
#[derive(Clone, Debug)]
pub struct StreamingConfig {
    /// Radius in chunks around the viewer to keep loaded.
    pub load_radius:      u32,
    /// Radius at which chunks are unloaded (should be > load_radius).
    pub unload_radius:    u32,
    /// Maximum chunks to generate per frame.
    pub loads_per_frame:  usize,
    /// Maximum chunks in the mesh cache.
    pub cache_capacity:   usize,
    /// Chunk generation configuration.
    pub gen_config:       ChunkGenConfig,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            load_radius:     6,
            unload_radius:   10,
            loads_per_frame: 2,
            cache_capacity:  200,
            gen_config:      ChunkGenConfig::default(),
        }
    }
}

/// Runtime statistics for the streaming system.
#[derive(Clone, Debug, Default)]
pub struct StreamingStats {
    pub loaded_chunks:    usize,
    pub queued_chunks:    usize,
    pub chunks_loaded_this_frame: usize,
    pub chunks_unloaded_this_frame: usize,
    pub cache_hits:       u64,
    pub cache_misses:     u64,
    pub frame_count:      u64,
}

/// The main terrain streaming manager.
///
/// Call `update(viewer_pos)` every frame to drive chunk loading/unloading.
/// Query `height_at(world_x, world_z)` for terrain height at any position.
pub struct ChunkStreamingManager {
    pub config:     StreamingConfig,
    pub stats:      StreamingStats,
    cache:          MeshCache,
    load_queue:     LoadQueue,
    lod_scheduler:  LodScheduler,
    visibility:     VisibilitySet,
    generator:      ChunkGenerator,
    chunk_states:   HashMap<ChunkCoord, ChunkState>,
    current_frame:  u64,
}

impl ChunkStreamingManager {
    pub fn new(config: StreamingConfig) -> Self {
        let gen = ChunkGenerator::new(config.gen_config.clone());
        let lod = LodScheduler::new(
            config.gen_config.chunk_size,
            config.gen_config.lod_levels,
        );
        let cache_cap = config.cache_capacity;
        Self {
            config,
            stats:         StreamingStats::default(),
            cache:         MeshCache::new(cache_cap),
            load_queue:    LoadQueue::new(),
            lod_scheduler: lod,
            visibility:    VisibilitySet::new(),
            generator:     gen,
            chunk_states:  HashMap::new(),
            current_frame: 0,
        }
    }

    /// Update the streaming system for the current frame.
    ///
    /// `viewer_world_pos` is the viewer's world-space position.
    pub fn update(&mut self, viewer_world_pos: [f32; 3]) {
        self.current_frame += 1;
        let chunk_size = self.config.gen_config.chunk_size;

        // Determine viewer chunk coordinate
        let viewer_chunk = ChunkCoord::new(
            (viewer_world_pos[0] / chunk_size).floor() as i32,
            (viewer_world_pos[2] / chunk_size).floor() as i32,
        );

        // ── 1. Visibility culling & LOD selection ─────────────────────────────
        self.visibility.clear();
        let lr = self.config.load_radius as i32;
        for dz in -lr..=lr {
            for dx in -lr..=lr {
                let coord = ChunkCoord::new(viewer_chunk.x + dx, viewer_chunk.z + dz);
                let dist = viewer_chunk.distance(coord) * chunk_size;
                let lod = self.lod_scheduler.select_lod(dist);
                self.visibility.mark_visible(coord, lod);
            }
        }

        // ── 2. Enqueue missing chunks ─────────────────────────────────────────
        let mut to_enqueue = Vec::new();
        for (&coord, _) in self.visibility.iter() {
            let state = self.chunk_states.get(&coord).copied().unwrap_or(ChunkState::Unloaded);
            if state == ChunkState::Unloaded {
                let dist = viewer_chunk.distance(coord);
                // Priority: inverse distance, integer scaled
                let priority = (1000.0 / (dist + 1.0)) as u32;
                to_enqueue.push((coord, priority));
            }
        }
        for (coord, priority) in to_enqueue {
            self.load_queue.enqueue(coord, priority);
            self.chunk_states.insert(coord, ChunkState::Queued);
        }

        // ── 3. Generate chunks from queue ─────────────────────────────────────
        let to_load = self.load_queue.drain(self.config.loads_per_frame);
        let mut loaded_count = 0usize;
        for coord in to_load {
            let state = self.chunk_states.get(&coord).copied().unwrap_or(ChunkState::Unloaded);
            if state != ChunkState::Queued { continue; }

            self.chunk_states.insert(coord, ChunkState::Loading);
            let mut data = self.generator.generate(coord);
            data.last_used_frame = self.current_frame;
            self.cache.insert(data);
            self.chunk_states.insert(coord, ChunkState::Loaded);
            loaded_count += 1;
        }

        // ── 4. Mark distant chunks for unloading ──────────────────────────────
        let unload_r = self.config.unload_radius as i32;
        let mut to_unload = Vec::new();
        for (&coord, &state) in &self.chunk_states {
            if state == ChunkState::Loaded {
                let dist = (viewer_chunk.x - coord.x).abs().max((viewer_chunk.z - coord.z).abs());
                if dist > unload_r {
                    to_unload.push(coord);
                }
            }
        }
        let unloaded_count = to_unload.len();
        for coord in to_unload {
            if let Some(chunk) = self.cache.chunks.get_mut(&coord) {
                chunk.state = ChunkState::Unloading;
            }
            self.chunk_states.insert(coord, ChunkState::Unloading);
        }

        // ── 5. Evict Unloading chunks ─────────────────────────────────────────
        self.cache.evict_unloading();
        self.chunk_states.retain(|_, s| *s != ChunkState::Unloading);

        // ── 6. Update stats ───────────────────────────────────────────────────
        self.stats.loaded_chunks = self.cache.len();
        self.stats.queued_chunks = self.load_queue.len();
        self.stats.chunks_loaded_this_frame   = loaded_count;
        self.stats.chunks_unloaded_this_frame = unloaded_count;
        self.stats.frame_count = self.current_frame;
    }

    /// Query the terrain height at a world-space position.
    ///
    /// Returns `None` if the chunk containing this position is not loaded.
    pub fn height_at(&mut self, world_x: f32, world_z: f32) -> Option<f32> {
        let chunk_size = self.config.gen_config.chunk_size;
        let cx = (world_x / chunk_size).floor() as i32;
        let cz = (world_z / chunk_size).floor() as i32;
        let coord = ChunkCoord::new(cx, cz);

        if let Some(chunk) = self.cache.get(coord) {
            self.stats.cache_hits += 1;
            let local_x = world_x - cx as f32 * chunk_size;
            let local_z = world_z - cz as f32 * chunk_size;
            Some(chunk.collision.height_at_local(local_x, local_z))
        } else {
            self.stats.cache_misses += 1;
            None
        }
    }

    /// Get the vertex data for a loaded chunk.
    pub fn mesh_for(&mut self, coord: ChunkCoord) -> Option<&[[f32; 3]]> {
        if let Some(chunk) = self.cache.get(coord) {
            Some(&chunk.vertices)
        } else {
            None
        }
    }

    /// Force-load a chunk immediately (bypasses the queue).
    pub fn force_load(&mut self, coord: ChunkCoord) {
        if self.cache.contains(coord) { return; }
        let mut data = self.generator.generate(coord);
        data.last_used_frame = self.current_frame;
        self.cache.insert(data);
        self.chunk_states.insert(coord, ChunkState::Loaded);
    }

    /// Returns a reference to a loaded chunk, if present.
    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<&TerrainChunkData> {
        self.cache.peek(coord)
    }

    /// Returns all currently visible chunk coordinates.
    pub fn visible_coords(&self) -> Vec<ChunkCoord> {
        self.visibility.iter().map(|(&c, _)| c).collect()
    }

    /// Returns the state of a chunk.
    pub fn chunk_state(&self, coord: ChunkCoord) -> ChunkState {
        self.chunk_states.get(&coord).copied().unwrap_or(ChunkState::Unloaded)
    }
}

// ── Prefetcher ────────────────────────────────────────────────────────────────

/// Predicts where the viewer will be and pre-queues chunks.
pub struct Prefetcher {
    /// How many frames ahead to predict.
    pub lookahead_frames: u32,
    prev_chunk: Option<ChunkCoord>,
}

impl Prefetcher {
    pub fn new(lookahead_frames: u32) -> Self {
        Self { lookahead_frames, prev_chunk: None }
    }

    /// Given the current viewer chunk, compute which chunks to prefetch.
    pub fn prefetch_coords(&mut self, current: ChunkCoord, radius: u32) -> Vec<ChunkCoord> {
        let velocity = match self.prev_chunk {
            None    => (0, 0),
            Some(p) => (current.x - p.x, current.z - p.z),
        };
        self.prev_chunk = Some(current);

        let look = self.lookahead_frames as i32;
        let predicted = ChunkCoord::new(
            current.x + velocity.0 * look,
            current.z + velocity.1 * look,
        );

        let r = radius as i32;
        let mut coords = Vec::new();
        for dz in -r..=r {
            for dx in -r..=r {
                coords.push(ChunkCoord::new(predicted.x + dx, predicted.z + dz));
            }
        }
        coords
    }
}

// ── Chunk Serializer ──────────────────────────────────────────────────────────

/// Simple in-memory serializer/deserializer for heightmaps.
/// (In a real engine, this would write to disk.)
pub struct ChunkSerializer;

impl ChunkSerializer {
    /// Serialize a heightmap to a compact byte buffer (f32 little-endian).
    pub fn serialize_heights(hm: &HeightMap) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + hm.data.len() * 4);
        // Header: width (u32 LE), height (u32 LE)
        out.extend_from_slice(&(hm.width  as u32).to_le_bytes());
        out.extend_from_slice(&(hm.height as u32).to_le_bytes());
        for &v in &hm.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Deserialize a heightmap from bytes produced by `serialize_heights`.
    pub fn deserialize_heights(bytes: &[u8]) -> Option<HeightMap> {
        if bytes.len() < 8 { return None; }
        let w = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
        let h = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
        let expected = 8 + w * h * 4;
        if bytes.len() < expected { return None; }
        let mut data = Vec::with_capacity(w * h);
        for i in 0..w*h {
            let off = 8 + i * 4;
            let v = f32::from_le_bytes(bytes[off..off+4].try_into().ok()?);
            data.push(v);
        }
        Some(HeightMap { width: w, height: h, data })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_coord_distance() {
        let a = ChunkCoord::new(0, 0);
        let b = ChunkCoord::new(3, 4);
        assert!((a.distance(b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_chunk_coord_neighbors() {
        let c = ChunkCoord::new(5, 5);
        let n4 = c.neighbors_4();
        assert!(n4.contains(&ChunkCoord::new(5, 4)));
        assert!(n4.contains(&ChunkCoord::new(5, 6)));
        assert!(n4.contains(&ChunkCoord::new(4, 5)));
        assert!(n4.contains(&ChunkCoord::new(6, 5)));
    }

    #[test]
    fn test_load_queue_drain() {
        let mut q = LoadQueue::new();
        q.enqueue(ChunkCoord::new(0, 0), 100);
        q.enqueue(ChunkCoord::new(1, 0), 50);
        q.enqueue(ChunkCoord::new(0, 1), 200);
        let batch = q.drain(2);
        assert_eq!(batch.len(), 2);
        // Highest priority first
        assert_eq!(batch[0], ChunkCoord::new(0, 1));
        assert_eq!(batch[1], ChunkCoord::new(0, 0));
    }

    #[test]
    fn test_load_queue_cancel() {
        let mut q = LoadQueue::new();
        q.enqueue(ChunkCoord::new(0, 0), 100);
        q.cancel(ChunkCoord::new(0, 0));
        let batch = q.drain(10);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_mesh_cache_lru_eviction() {
        let mut cache = MeshCache::new(2);
        let gen = ChunkGenerator::new(ChunkGenConfig {
            cells_per_chunk: 9,
            erosion_iters:   0,
            lod_levels:      1,
            ..ChunkGenConfig::default()
        });
        cache.insert(gen.generate(ChunkCoord::new(0, 0)));
        cache.insert(gen.generate(ChunkCoord::new(1, 0)));
        // Access (0,0) to make it MRU
        cache.get(ChunkCoord::new(0, 0));
        // Insert third → (1,0) should be evicted
        cache.insert(gen.generate(ChunkCoord::new(2, 0)));
        assert_eq!(cache.len(), 2);
        assert!(cache.peek(ChunkCoord::new(0, 0)).is_some(), "0,0 should still be in cache");
        assert!(cache.peek(ChunkCoord::new(1, 0)).is_none(), "1,0 should have been evicted");
    }

    #[test]
    fn test_collision_hull_height() {
        let hm = crate::terrain::heightmap::DiamondSquare::generate(8, 0.5, 7);
        let hull = CollisionHull::generate(&hm, 64.0, 50.0, 8);
        let h = hull.height_at_local(32.0, 32.0);
        assert!(h >= 0.0 && h <= 50.0, "height out of range: {h}");
    }

    #[test]
    fn test_chunk_generator_output() {
        let gen = ChunkGenerator::new(ChunkGenConfig {
            cells_per_chunk: 9,
            erosion_iters:   0,
            lod_levels:      2,
            ..ChunkGenConfig::default()
        });
        let chunk = gen.generate(ChunkCoord::new(3, -2));
        assert_eq!(chunk.coord, ChunkCoord::new(3, -2));
        assert_eq!(chunk.state, ChunkState::Loaded);
        assert!(!chunk.vertices.is_empty());
    }

    #[test]
    fn test_streaming_manager_basic() {
        let cfg = StreamingConfig {
            load_radius:    2,
            unload_radius:  4,
            loads_per_frame: 50,
            cache_capacity: 100,
            gen_config: ChunkGenConfig {
                cells_per_chunk: 9,
                erosion_iters:   0,
                lod_levels:      1,
                ..ChunkGenConfig::default()
            },
        };
        let mut mgr = ChunkStreamingManager::new(cfg);
        mgr.update([0.0, 0.0, 0.0]);
        mgr.update([0.0, 0.0, 0.0]);
        assert!(mgr.stats.loaded_chunks > 0, "should have loaded some chunks");
    }

    #[test]
    fn test_height_query_after_force_load() {
        let cfg = StreamingConfig {
            gen_config: ChunkGenConfig {
                cells_per_chunk: 9,
                erosion_iters:   0,
                lod_levels:      1,
                ..ChunkGenConfig::default()
            },
            ..StreamingConfig::default()
        };
        let mut mgr = ChunkStreamingManager::new(cfg);
        let coord = ChunkCoord::new(0, 0);
        mgr.force_load(coord);
        let h = mgr.height_at(64.0, 64.0);
        assert!(h.is_some(), "height query should succeed after force load");
        let h = h.unwrap();
        assert!(h >= 0.0, "height should be non-negative: {h}");
    }

    #[test]
    fn test_serializer_roundtrip() {
        let hm = crate::terrain::heightmap::DiamondSquare::generate(16, 0.5, 42);
        let bytes = ChunkSerializer::serialize_heights(&hm);
        let hm2 = ChunkSerializer::deserialize_heights(&bytes).expect("deserialize failed");
        assert_eq!(hm.width,  hm2.width);
        assert_eq!(hm.height, hm2.height);
        for (a, b) in hm.data.iter().zip(hm2.data.iter()) {
            assert!((a - b).abs() < 1e-6, "roundtrip mismatch: {a} vs {b}");
        }
    }

    #[test]
    fn test_lod_scheduler() {
        let sched = LodScheduler::new(128.0, 3);
        assert_eq!(sched.select_lod(100.0),  0);
        assert_eq!(sched.select_lod(300.0),  1);
        assert_eq!(sched.select_lod(600.0),  2);
        assert_eq!(sched.select_lod(1200.0), 3);
    }

    #[test]
    fn test_prefetcher() {
        let mut pf = Prefetcher::new(3);
        // No velocity yet
        let c0 = pf.prefetch_coords(ChunkCoord::new(0, 0), 1);
        assert!(!c0.is_empty());
        // Move right: velocity = (1, 0)
        let c1 = pf.prefetch_coords(ChunkCoord::new(1, 0), 1);
        // Predicted = (1 + 1*3, 0) = (4, 0); with radius 1 → 9 chunks around (4, 0)
        assert_eq!(c1.len(), 9);
        assert!(c1.contains(&ChunkCoord::new(4, 0)));
    }

    #[test]
    fn test_collision_hull_triangles() {
        let hm = crate::terrain::heightmap::DiamondSquare::generate(8, 0.4, 5);
        let hull = CollisionHull::generate(&hm, 64.0, 50.0, 4);
        let tris = hull.triangles();
        // 4x4 grid → 3x3 quads → 9*2 = 18 triangles
        assert_eq!(tris.len(), 18);
    }

    #[test]
    fn test_visibility_set() {
        let mut vis = VisibilitySet::new();
        vis.mark_visible(ChunkCoord::new(1, 2), 0);
        vis.mark_visible(ChunkCoord::new(3, 4), 2);
        assert!(vis.is_visible(ChunkCoord::new(1, 2)));
        assert!(!vis.is_visible(ChunkCoord::new(0, 0)));
        assert_eq!(vis.lod_for(ChunkCoord::new(3, 4)), Some(2));
        vis.clear();
        assert!(!vis.is_visible(ChunkCoord::new(1, 2)));
    }

    #[test]
    fn test_aabb_intersects() {
        let a = Aabb { min: [0.0, 0.0, 0.0], max: [10.0, 10.0, 10.0] };
        let b = Aabb { min: [5.0, 5.0, 5.0], max: [15.0, 15.0, 15.0] };
        let c = Aabb { min: [20.0, 0.0, 0.0], max: [30.0, 10.0, 10.0] };
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }
}
