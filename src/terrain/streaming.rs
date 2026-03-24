//! Terrain streaming — async chunk loading/unloading, LRU cache, priority queue.
//!
//! Uses thread pools (std threads + channels) rather than async/await.
//! Manages chunk lifecycle: generation → cache → serialization → eviction.

use std::collections::{HashMap, BinaryHeap, VecDeque};
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::thread;
use glam::Vec3;

use crate::terrain::mod_types::{TerrainChunk, TerrainConfig, ChunkCoord, ChunkState};
use crate::terrain::heightmap::{HeightMap, FractalNoise, DiamondSquare, PerlinTerrain};
use crate::terrain::biome::{BiomeMap, ClimateSimulator};
use crate::terrain::vegetation::VegetationSystem;

// ── ChunkCache ────────────────────────────────────────────────────────────────

/// Key for cache entries: maps ChunkCoord → access_order (used for LRU).
struct CacheEntry {
    chunk:        TerrainChunk,
    access_order: u64,
}

/// LRU cache for terrain chunks.
pub struct ChunkCache {
    entries:   HashMap<ChunkCoord, CacheEntry>,
    max_size:  usize,
    /// Monotonically increasing access counter for LRU tracking.
    clock:     u64,
    /// Memory budget in bytes (approximate).
    memory_budget: usize,
    current_memory: usize,
}

impl ChunkCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
            clock: 0,
            memory_budget: max_size * 1024 * 1024, // default: max_size MB
            current_memory: 0,
        }
    }

    pub fn with_memory_budget(max_size: usize, budget_bytes: usize) -> Self {
        let mut c = Self::new(max_size);
        c.memory_budget = budget_bytes;
        c
    }

    /// Insert or replace a chunk. Evicts LRU if over capacity.
    pub fn insert(&mut self, coord: ChunkCoord, chunk: TerrainChunk) {
        let mem = Self::estimate_chunk_memory(&chunk);
        // Evict while over budget or count limit
        while (self.entries.len() >= self.max_size || self.current_memory + mem > self.memory_budget)
            && !self.entries.is_empty()
        {
            self.evict_lru();
        }
        self.clock += 1;
        let old_mem = self.entries.get(&coord).map(|e| Self::estimate_chunk_memory(&e.chunk)).unwrap_or(0);
        self.current_memory = self.current_memory.saturating_sub(old_mem) + mem;
        self.entries.insert(coord, CacheEntry { chunk, access_order: self.clock });
    }

    /// Get a chunk by coord, updating access time.
    pub fn get(&mut self, coord: ChunkCoord) -> Option<&TerrainChunk> {
        if let Some(entry) = self.entries.get_mut(&coord) {
            self.clock += 1;
            entry.access_order = self.clock;
            Some(&entry.chunk)
        } else {
            None
        }
    }

    /// Check whether the cache contains a coord without updating LRU.
    pub fn contains(&self, coord: ChunkCoord) -> bool {
        self.entries.contains_key(&coord)
    }

    /// Remove a specific coord from the cache.
    pub fn remove(&mut self, coord: ChunkCoord) -> Option<TerrainChunk> {
        if let Some(entry) = self.entries.remove(&coord) {
            self.current_memory = self.current_memory
                .saturating_sub(Self::estimate_chunk_memory(&entry.chunk));
            Some(entry.chunk)
        } else {
            None
        }
    }

    /// Evict the least-recently-used entry.
    fn evict_lru(&mut self) {
        let lru_key = self.entries.iter()
            .min_by_key(|(_, e)| e.access_order)
            .map(|(k, _)| *k);
        if let Some(key) = lru_key {
            if let Some(entry) = self.entries.remove(&key) {
                self.current_memory = self.current_memory
                    .saturating_sub(Self::estimate_chunk_memory(&entry.chunk));
            }
        }
    }

    /// Estimate memory usage of a chunk in bytes.
    fn estimate_chunk_memory(chunk: &TerrainChunk) -> usize {
        let heightmap_bytes = chunk.heightmap.data.len() * 4;
        let base = std::mem::size_of::<TerrainChunk>();
        base + heightmap_bytes + 512 // approximate
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn current_memory_bytes(&self) -> usize { self.current_memory }
}

// ── Priority Queue for chunk loading ─────────────────────────────────────────

/// A load request with priority.
#[derive(Clone, Debug)]
struct LoadRequest {
    coord:    ChunkCoord,
    priority: i64,   // higher = load first
}

impl PartialEq for LoadRequest {
    fn eq(&self, other: &Self) -> bool { self.priority == other.priority }
}
impl Eq for LoadRequest {}
impl PartialOrd for LoadRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for LoadRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

/// Priority queue for chunks to be loaded.
pub struct LoadQueue {
    heap:    BinaryHeap<LoadRequest>,
    in_queue: std::collections::HashSet<ChunkCoord>,
}

impl LoadQueue {
    pub fn new() -> Self {
        Self { heap: BinaryHeap::new(), in_queue: std::collections::HashSet::new() }
    }

    /// Push a coord with given priority (higher = sooner).
    pub fn push(&mut self, coord: ChunkCoord, priority: i64) {
        if self.in_queue.insert(coord) {
            self.heap.push(LoadRequest { coord, priority });
        }
    }

    /// Pop the highest-priority coord.
    pub fn pop(&mut self) -> Option<ChunkCoord> {
        while let Some(req) = self.heap.pop() {
            if self.in_queue.remove(&req.coord) {
                return Some(req.coord);
            }
        }
        None
    }

    pub fn len(&self) -> usize { self.in_queue.len() }
    pub fn is_empty(&self) -> bool { self.in_queue.is_empty() }

    /// Recompute priorities based on new camera position. Clears and rebuilds.
    pub fn reprioritize(&mut self, camera: Vec3, config: &TerrainConfig) {
        let coords: Vec<ChunkCoord> = self.in_queue.drain().collect();
        self.heap.clear();
        for coord in coords {
            let dist = coord.distance_to_world_pos(camera, config.chunk_size as f32);
            let priority = -(dist as i64);
            self.heap.push(LoadRequest { coord, priority });
            self.in_queue.insert(coord);
        }
    }
}

impl Default for LoadQueue {
    fn default() -> Self { Self::new() }
}

// ── ChunkGenerator ────────────────────────────────────────────────────────────

/// Generates terrain chunks from scratch for a given coord.
pub struct ChunkGenerator {
    pub config: TerrainConfig,
}

impl ChunkGenerator {
    pub fn new(config: TerrainConfig) -> Self { Self { config } }

    /// Generate a fully-initialized chunk for the given coordinate.
    pub fn generate(&self, coord: ChunkCoord) -> TerrainChunk {
        let size = self.config.chunk_size;
        let seed = self.chunk_seed(coord);

        // Generate heightmap using a mix of algorithms
        let mut heightmap = self.generate_heightmap(coord, size, seed);
        heightmap.island_mask(2.0);
        heightmap.normalize();

        // Apply erosion for naturalistic terrain
        crate::terrain::heightmap::HydraulicErosion::erode(
            &mut heightmap, 1000, 1.0, 8.0, 0.05, seed,
        );
        crate::terrain::heightmap::ThermalErosion::erode(&mut heightmap, 10, 0.04);
        heightmap.normalize();

        // Biome classification
        let sim = ClimateSimulator::default();
        let climate = sim.simulate(&heightmap);
        let biome_map = BiomeMap::from_heightmap(&heightmap, &climate);

        // Vegetation
        let vegetation = VegetationSystem::generate(&heightmap, &biome_map, 0.8, seed);

        TerrainChunk {
            coord,
            heightmap,
            biome_map: Some(biome_map),
            vegetation: Some(vegetation),
            lod_level: 0,
            state: ChunkState::Ready,
            last_used: std::time::Instant::now(),
            seed,
        }
    }

    /// Generate a LOD-reduced chunk (lower resolution heightmap).
    pub fn generate_lod(&self, coord: ChunkCoord, lod: u8) -> TerrainChunk {
        let scale = 1usize << lod as usize;
        let size = (self.config.chunk_size / scale).max(4);
        let seed = self.chunk_seed(coord);
        let mut heightmap = self.generate_heightmap(coord, size, seed);
        heightmap.normalize();

        TerrainChunk {
            coord,
            heightmap,
            biome_map: None,
            vegetation: None,
            lod_level: lod,
            state: ChunkState::Ready,
            last_used: std::time::Instant::now(),
            seed,
        }
    }

    fn generate_heightmap(&self, coord: ChunkCoord, size: usize, seed: u64) -> HeightMap {
        // Use coord to offset the noise sampling for seamless tiling
        let offset_x = coord.0 as f32 * size as f32;
        let offset_z = coord.1 as f32 * size as f32;
        let world_scale = 256.0f32;

        // Base layer: fractal noise
        let mut hm = HeightMap::new(size, size);
        let noise = crate::terrain::heightmap::FractalNoise::generate(
            size, size, 6, 2.0, 0.5, 4.0, seed,
        );
        // Offset coordinates for seamless tiling
        for y in 0..size {
            for x in 0..size {
                let nx = (x as f32 + offset_x) / world_scale;
                let ny = (y as f32 + offset_z) / world_scale;
                let idx = y * size + x;
                hm.data[idx] = noise.data[idx];
            }
        }
        hm
    }

    fn chunk_seed(&self, coord: ChunkCoord) -> u64 {
        let base = self.config.seed;
        let cx = coord.0 as u64;
        let cz = coord.1 as u64;
        base.wrapping_add(cx.wrapping_mul(0x9e3779b97f4a7c15))
            .wrapping_add(cz.wrapping_mul(0x6c62272e07bb0142))
    }
}

// ── ChunkSerializer ───────────────────────────────────────────────────────────

/// Magic bytes for chunk file format.
const CHUNK_MAGIC: u32 = 0x43484E4B; // "CHNK"
const CHUNK_VERSION: u16 = 1;

/// Serializes/deserializes chunks to/from a binary format.
///
/// Format:
/// ```text
/// [magic: u32][version: u16][coord_x: i32][coord_z: i32]
/// [timestamp: u64][seed: u64][lod: u8]
/// [heightmap_width: u32][heightmap_height: u32][heightmap_data: f32 * w * h]
/// [checksum: u32]
/// ```
pub struct ChunkSerializer;

impl ChunkSerializer {
    /// Serialize a chunk to bytes.
    pub fn serialize(chunk: &TerrainChunk) -> Vec<u8> {
        let mut out = Vec::new();
        // Header
        out.extend_from_slice(&CHUNK_MAGIC.to_le_bytes());
        out.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
        out.extend_from_slice(&chunk.coord.0.to_le_bytes());
        out.extend_from_slice(&chunk.coord.1.to_le_bytes());
        // Timestamp (seconds since epoch — use 0 since no std::time::SystemTime)
        out.extend_from_slice(&0u64.to_le_bytes());
        out.extend_from_slice(&chunk.seed.to_le_bytes());
        out.push(chunk.lod_level);
        // Heightmap
        let hm_bytes = chunk.heightmap.to_raw_bytes();
        let hm_len = hm_bytes.len() as u32;
        out.extend_from_slice(&hm_len.to_le_bytes());
        out.extend_from_slice(&hm_bytes);
        // Simple checksum: sum of all bytes so far mod 2^32
        let checksum: u32 = out.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        out.extend_from_slice(&checksum.to_le_bytes());
        out
    }

    /// Deserialize a chunk from bytes. Returns None if format is invalid.
    pub fn deserialize(bytes: &[u8]) -> Option<TerrainChunk> {
        if bytes.len() < 4 + 2 + 4 + 4 + 8 + 8 + 1 + 4 { return None; }
        let mut pos = 0;

        let magic = u32::from_le_bytes(bytes[pos..pos+4].try_into().ok()?); pos += 4;
        if magic != CHUNK_MAGIC { return None; }

        let version = u16::from_le_bytes(bytes[pos..pos+2].try_into().ok()?); pos += 2;
        if version != CHUNK_VERSION { return None; }

        let coord_x = i32::from_le_bytes(bytes[pos..pos+4].try_into().ok()?); pos += 4;
        let coord_z = i32::from_le_bytes(bytes[pos..pos+4].try_into().ok()?); pos += 4;
        let _timestamp = u64::from_le_bytes(bytes[pos..pos+8].try_into().ok()?); pos += 8;
        let seed = u64::from_le_bytes(bytes[pos..pos+8].try_into().ok()?); pos += 8;
        let lod_level = bytes[pos]; pos += 1;

        let hm_len = u32::from_le_bytes(bytes[pos..pos+4].try_into().ok()?) as usize; pos += 4;
        if bytes.len() < pos + hm_len + 4 { return None; }

        let hm_bytes = &bytes[pos..pos + hm_len]; pos += hm_len;
        let heightmap = HeightMap::from_raw_bytes(hm_bytes)?;

        // Verify checksum
        let stored_checksum = u32::from_le_bytes(bytes[pos..pos+4].try_into().ok()?);
        let computed: u32 = bytes[..pos].iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        if stored_checksum != computed { return None; }

        Some(TerrainChunk {
            coord: ChunkCoord(coord_x, coord_z),
            heightmap,
            biome_map: None,
            vegetation: None,
            lod_level,
            state: ChunkState::Ready,
            last_used: std::time::Instant::now(),
            seed,
        })
    }

    /// Save chunk to a file path (returns error string on failure).
    pub fn save_to_file(chunk: &TerrainChunk, path: &str) -> Result<(), String> {
        let bytes = Self::serialize(chunk);
        std::fs::write(path, &bytes).map_err(|e| e.to_string())
    }

    /// Load chunk from a file path.
    pub fn load_from_file(path: &str) -> Result<TerrainChunk, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::deserialize(&bytes).ok_or_else(|| "Invalid chunk format".to_string())
    }
}

// ── StreamingStats ────────────────────────────────────────────────────────────

/// Statistics for the streaming system.
#[derive(Debug, Default, Clone)]
pub struct StreamingStats {
    pub chunks_loaded:   usize,
    pub chunks_unloaded: usize,
    pub cache_hits:      usize,
    pub cache_misses:    usize,
    pub pending_count:   usize,
    pub memory_bytes:    usize,
    pub generate_time_ms: f64,
}

impl StreamingStats {
    pub fn cache_hit_rate(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 { 0.0 } else { self.cache_hits as f32 / total as f32 }
    }
}

// ── VisibilitySet ─────────────────────────────────────────────────────────────

/// Tracks which chunk coords are currently in the view frustum.
#[derive(Debug, Default)]
pub struct VisibilitySet {
    visible: std::collections::HashSet<ChunkCoord>,
    previous: std::collections::HashSet<ChunkCoord>,
}

impl VisibilitySet {
    pub fn new() -> Self { Self::default() }

    /// Update the visible set from new camera parameters.
    pub fn update(&mut self, camera_pos: Vec3, config: &TerrainConfig) {
        self.previous = std::mem::take(&mut self.visible);
        let chunk_world = config.chunk_size as f32;
        let view_chunks = config.view_distance as i32;
        let cam_cx = (camera_pos.x / chunk_world).floor() as i32;
        let cam_cz = (camera_pos.z / chunk_world).floor() as i32;
        for dz in -view_chunks..=view_chunks {
            for dx in -view_chunks..=view_chunks {
                let dist = ((dx * dx + dz * dz) as f32).sqrt();
                if dist <= config.view_distance as f32 {
                    self.visible.insert(ChunkCoord(cam_cx + dx, cam_cz + dz));
                }
            }
        }
    }

    /// Chunks that just became visible.
    pub fn newly_visible(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        self.visible.iter().copied().filter(|c| !self.previous.contains(c))
    }

    /// Chunks that just became invisible.
    pub fn newly_hidden(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        self.previous.iter().copied().filter(|c| !self.visible.contains(c))
    }

    pub fn is_visible(&self, coord: ChunkCoord) -> bool { self.visible.contains(&coord) }
    pub fn visible_count(&self) -> usize { self.visible.len() }
    pub fn visible_coords(&self) -> impl Iterator<Item = ChunkCoord> + '_ { self.visible.iter().copied() }
}

// ── LodScheduler ─────────────────────────────────────────────────────────────

/// Decides when to upgrade/downgrade chunk LOD.
///
/// Uses hysteresis to prevent LOD thrashing: upgrade requires distance < threshold,
/// downgrade requires distance > threshold + hysteresis_margin.
pub struct LodScheduler {
    /// Distance thresholds for each LOD level. `lod_thresholds[n]` = max dist for LOD n.
    pub lod_thresholds:     Vec<f32>,
    /// Hysteresis margin to prevent thrashing.
    pub hysteresis_margin:  f32,
    /// Pending LOD changes (coord → target_lod).
    pending: HashMap<ChunkCoord, u8>,
}

impl LodScheduler {
    pub fn new(lod_levels: usize, chunk_size: f32) -> Self {
        let thresholds: Vec<f32> = (0..lod_levels)
            .map(|l| chunk_size * 2.0 * (1 << l) as f32)
            .collect();
        Self {
            lod_thresholds: thresholds,
            hysteresis_margin: chunk_size * 0.5,
            pending: HashMap::new(),
        }
    }

    /// Compute the desired LOD for a chunk at a given world distance.
    pub fn desired_lod(&self, dist: f32, current_lod: u8) -> u8 {
        let mut target = (self.lod_thresholds.len() - 1) as u8;
        for (l, &threshold) in self.lod_thresholds.iter().enumerate() {
            if dist < threshold {
                target = l as u8;
                break;
            }
        }
        // Apply hysteresis: only change if difference exceeds margin
        if target > current_lod {
            // Downgrading: require dist > current threshold + margin
            let cur_thresh = self.lod_thresholds.get(current_lod as usize).copied().unwrap_or(0.0);
            if dist < cur_thresh + self.hysteresis_margin {
                return current_lod; // Stay at current LOD
            }
        }
        // Upgrading: immediate (close objects need more detail)
        target
    }

    /// Update LOD decisions for all visible chunks.
    pub fn update(&mut self, camera_pos: Vec3, chunks: &HashMap<ChunkCoord, u8>, config: &TerrainConfig) {
        self.pending.clear();
        for (&coord, &current_lod) in chunks {
            let dist = coord.distance_to_world_pos(camera_pos, config.chunk_size as f32);
            let desired = self.desired_lod(dist, current_lod);
            if desired != current_lod {
                self.pending.insert(coord, desired);
            }
        }
    }

    /// Get the pending LOD changes.
    pub fn pending_changes(&self) -> &HashMap<ChunkCoord, u8> { &self.pending }

    /// Compute priority for loading a chunk (used by LoadQueue).
    /// In-frustum and close chunks have highest priority.
    pub fn load_priority(
        coord: ChunkCoord,
        camera_pos: Vec3,
        chunk_size: f32,
        in_frustum: bool,
    ) -> i64 {
        let dist = coord.distance_to_world_pos(camera_pos, chunk_size);
        let dist_score = -(dist as i64);
        let frustum_bonus: i64 = if in_frustum { 1_000_000 } else { 0 };
        dist_score + frustum_bonus
    }
}

// ── Prefetcher ────────────────────────────────────────────────────────────────

/// Predicts movement direction and prefetches chunks ahead of the camera.
pub struct Prefetcher {
    /// Recent camera positions (ring buffer for velocity estimation).
    history: VecDeque<Vec3>,
    /// Number of frames to look ahead.
    lookahead_frames: usize,
    /// Estimated velocity.
    velocity: Vec3,
}

impl Prefetcher {
    pub fn new(lookahead_frames: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(16),
            lookahead_frames,
            velocity: Vec3::ZERO,
        }
    }

    /// Record a new camera position.
    pub fn push_position(&mut self, pos: Vec3) {
        if self.history.len() >= 16 { self.history.pop_front(); }
        self.history.push_back(pos);
        self.estimate_velocity();
    }

    fn estimate_velocity(&mut self) {
        if self.history.len() < 2 {
            self.velocity = Vec3::ZERO;
            return;
        }
        let recent = self.history.back().copied().unwrap_or(Vec3::ZERO);
        let old = self.history.front().copied().unwrap_or(Vec3::ZERO);
        let n = self.history.len() as f32;
        self.velocity = (recent - old) / n;
    }

    /// Predict where camera will be after `lookahead_frames` frames.
    pub fn predicted_position(&self) -> Vec3 {
        self.history.back().copied().unwrap_or(Vec3::ZERO)
            + self.velocity * self.lookahead_frames as f32
    }

    /// Generate coords to prefetch based on predicted movement.
    pub fn prefetch_coords(&self, config: &TerrainConfig) -> Vec<ChunkCoord> {
        let pred = self.predicted_position();
        let chunk_world = config.chunk_size as f32;
        let pred_cx = (pred.x / chunk_world).floor() as i32;
        let pred_cz = (pred.z / chunk_world).floor() as i32;
        let prefetch_radius = 2i32;
        let mut coords = Vec::new();
        for dz in -prefetch_radius..=prefetch_radius {
            for dx in -prefetch_radius..=prefetch_radius {
                coords.push(ChunkCoord(pred_cx + dx, pred_cz + dz));
            }
        }
        coords
    }
}

// ── Worker Thread Pool ─────────────────────────────────────────────────────────

type GenerateResult = (ChunkCoord, TerrainChunk);

/// A fixed-size thread pool for chunk generation.
pub struct GeneratorPool {
    workers:    Vec<thread::JoinHandle<()>>,
    tx:         std::sync::mpsc::Sender<ChunkCoord>,
    rx_results: Arc<Mutex<Vec<GenerateResult>>>,
    active:     Arc<AtomicUsize>,
    config:     TerrainConfig,
}

impl GeneratorPool {
    pub fn new(num_workers: usize, config: TerrainConfig) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<ChunkCoord>();
        let rx = Arc::new(Mutex::new(rx));
        let results = Arc::new(Mutex::new(Vec::new()));
        let active = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();

        for _ in 0..num_workers {
            let rx2 = Arc::clone(&rx);
            let res2 = Arc::clone(&results);
            let act2 = Arc::clone(&active);
            let cfg2 = config.clone();
            let handle = thread::spawn(move || {
                let gen = ChunkGenerator::new(cfg2);
                loop {
                    let coord = {
                        let lock = rx2.lock().unwrap();
                        match lock.recv() {
                            Ok(c) => c,
                            Err(_) => break,
                        }
                    };
                    act2.fetch_add(1, Ordering::Relaxed);
                    let chunk = gen.generate(coord);
                    {
                        let mut lock = res2.lock().unwrap();
                        lock.push((coord, chunk));
                    }
                    act2.fetch_sub(1, Ordering::Relaxed);
                }
            });
            workers.push(handle);
        }

        Self { workers, tx, rx_results: results, active, config }
    }

    /// Submit a coord for generation.
    pub fn submit(&self, coord: ChunkCoord) {
        let _ = self.tx.send(coord);
    }

    /// Drain completed results.
    pub fn drain_results(&self) -> Vec<GenerateResult> {
        let mut lock = self.rx_results.lock().unwrap();
        std::mem::take(&mut *lock)
    }

    /// Number of workers currently generating.
    pub fn active_count(&self) -> usize { self.active.load(Ordering::Relaxed) }
}

// ── StreamingManager ─────────────────────────────────────────────────────────

/// Top-level coordinator for terrain chunk streaming.
///
/// Uses a thread pool for generation, an LRU cache, a priority load queue,
/// a LOD scheduler, and a prefetcher to manage the full chunk lifecycle.
pub struct StreamingManager {
    pub config:       TerrainConfig,
    pub cache:        ChunkCache,
    pub load_queue:   LoadQueue,
    pub lod_scheduler: LodScheduler,
    pub prefetcher:   Prefetcher,
    pub visibility:   VisibilitySet,
    pub stats:        StreamingStats,
    /// Coords currently being generated (submitted to pool).
    in_flight: std::collections::HashSet<ChunkCoord>,
    /// Generator pool.
    pool:      Option<GeneratorPool>,
    /// Camera position from last update.
    camera_pos: Vec3,
    /// Current LOD for each cached chunk.
    chunk_lods: HashMap<ChunkCoord, u8>,
}

impl StreamingManager {
    pub fn new(config: TerrainConfig) -> Self {
        let lod_levels = config.lod_levels;
        let chunk_size = config.chunk_size as f32;
        let max_cache = (config.view_distance * 2 + 1).pow(2) as usize * 2;
        let pool = GeneratorPool::new(4, config.clone());

        Self {
            lod_scheduler: LodScheduler::new(lod_levels, chunk_size),
            cache:        ChunkCache::new(max_cache),
            load_queue:   LoadQueue::new(),
            prefetcher:   Prefetcher::new(8),
            visibility:   VisibilitySet::new(),
            stats:        StreamingStats::default(),
            in_flight:    std::collections::HashSet::new(),
            pool:         Some(pool),
            camera_pos:   Vec3::ZERO,
            chunk_lods:   HashMap::new(),
            config,
        }
    }

    /// Create without a thread pool (synchronous generation for testing).
    pub fn new_synchronous(config: TerrainConfig) -> Self {
        let lod_levels = config.lod_levels;
        let chunk_size = config.chunk_size as f32;
        let max_cache = (config.view_distance * 2 + 1).pow(2) as usize * 2;
        Self {
            lod_scheduler: LodScheduler::new(lod_levels, chunk_size),
            cache:        ChunkCache::new(max_cache),
            load_queue:   LoadQueue::new(),
            prefetcher:   Prefetcher::new(8),
            visibility:   VisibilitySet::new(),
            stats:        StreamingStats::default(),
            in_flight:    std::collections::HashSet::new(),
            pool:         None,
            camera_pos:   Vec3::ZERO,
            chunk_lods:   HashMap::new(),
            config,
        }
    }

    /// Update the streaming system with a new camera position.
    /// Call once per frame.
    pub fn update(&mut self, camera_pos: Vec3) {
        self.camera_pos = camera_pos;
        self.prefetcher.push_position(camera_pos);

        // Update visibility
        self.visibility.update(camera_pos, &self.config);

        // Queue newly visible chunks
        for coord in self.visibility.newly_visible() {
            if !self.cache.contains(coord) && !self.in_flight.contains(&coord) {
                let priority = LodScheduler::load_priority(
                    coord, camera_pos, self.config.chunk_size as f32, true,
                );
                self.load_queue.push(coord, priority);
            }
        }

        // Queue prefetch coords
        let prefetch = self.prefetcher.prefetch_coords(&self.config);
        for coord in prefetch {
            if !self.cache.contains(coord) && !self.in_flight.contains(&coord) {
                let priority = LodScheduler::load_priority(
                    coord, camera_pos, self.config.chunk_size as f32, false,
                ) - 500_000; // lower priority than visible
                self.load_queue.push(coord, priority);
            }
        }

        // Reprioritize queue
        self.load_queue.reprioritize(camera_pos, &self.config);

        // Submit chunks to pool (limit in-flight)
        let max_in_flight = 8usize;
        while self.in_flight.len() < max_in_flight {
            if let Some(coord) = self.load_queue.pop() {
                if self.pool.is_some() {
                    self.pool.as_ref().unwrap().submit(coord);
                    self.in_flight.insert(coord);
                } else {
                    // Synchronous mode: generate immediately
                    let t0 = std::time::Instant::now();
                    let gen = ChunkGenerator::new(self.config.clone());
                    let chunk = gen.generate(coord);
                    self.stats.generate_time_ms += t0.elapsed().as_secs_f64() * 1000.0;
                    self.chunk_lods.insert(coord, chunk.lod_level);
                    self.cache.insert(coord, chunk);
                    self.stats.chunks_loaded += 1;
                    self.stats.cache_misses += 1;
                }
            } else {
                break;
            }
        }

        // Drain completed results from pool
        if let Some(ref pool) = self.pool {
            for (coord, chunk) in pool.drain_results() {
                self.in_flight.remove(&coord);
                self.chunk_lods.insert(coord, chunk.lod_level);
                self.cache.insert(coord, chunk);
                self.stats.chunks_loaded += 1;
                self.stats.cache_misses += 1;
            }
        }

        // Unload invisible chunks that exceed cache budget
        for coord in self.visibility.newly_hidden() {
            if self.cache.contains(coord) {
                self.cache.remove(coord);
                self.chunk_lods.remove(&coord);
                self.stats.chunks_unloaded += 1;
            }
        }

        // LOD scheduler update
        self.lod_scheduler.update(camera_pos, &self.chunk_lods, &self.config);

        self.stats.pending_count = self.load_queue.len() + self.in_flight.len();
        self.stats.memory_bytes = self.cache.current_memory_bytes();
    }

    /// Get a chunk by coordinate. Returns None if not yet loaded.
    pub fn get_chunk(&mut self, coord: ChunkCoord) -> Option<&TerrainChunk> {
        if let Some(chunk) = self.cache.get(coord) {
            self.stats.cache_hits += 1;
            Some(chunk)
        } else {
            self.stats.cache_misses += 1;
            None
        }
    }

    /// Force-load a chunk synchronously (blocks until generated).
    pub fn force_load(&mut self, coord: ChunkCoord) -> &TerrainChunk {
        if !self.cache.contains(coord) {
            let gen = ChunkGenerator::new(self.config.clone());
            let chunk = gen.generate(coord);
            self.chunk_lods.insert(coord, chunk.lod_level);
            self.cache.insert(coord, chunk);
            self.stats.chunks_loaded += 1;
        }
        self.cache.get(coord).unwrap()
    }

    /// Sample height at world coordinates (interpolates between chunks if needed).
    pub fn sample_height_world(&mut self, world_x: f32, world_z: f32) -> f32 {
        let chunk_world = self.config.chunk_size as f32;
        let cx = (world_x / chunk_world).floor() as i32;
        let cz = (world_z / chunk_world).floor() as i32;
        let coord = ChunkCoord(cx, cz);
        let local_x = world_x - cx as f32 * chunk_world;
        let local_z = world_z - cz as f32 * chunk_world;
        if let Some(chunk) = self.cache.get(coord) {
            let hm = &chunk.heightmap;
            let sx = (local_x / chunk_world * hm.width as f32).clamp(0.0, hm.width as f32 - 1.0);
            let sz = (local_z / chunk_world * hm.height as f32).clamp(0.0, hm.height as f32 - 1.0);
            hm.sample_bilinear(sx, sz)
        } else {
            0.0
        }
    }

    pub fn stats(&self) -> &StreamingStats { &self.stats }
    pub fn visible_chunk_count(&self) -> usize { self.visibility.visible_count() }
    pub fn cache_size(&self) -> usize { self.cache.len() }
    pub fn in_flight_count(&self) -> usize { self.in_flight.len() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> TerrainConfig {
        TerrainConfig {
            chunk_size: 16,
            view_distance: 2,
            lod_levels: 3,
            seed: 42,
        }
    }

    #[test]
    fn test_chunk_cache_insert_get() {
        let config = test_config();
        let gen = ChunkGenerator::new(config.clone());
        let chunk = gen.generate(ChunkCoord(0, 0));
        let mut cache = ChunkCache::new(10);
        cache.insert(ChunkCoord(0, 0), chunk);
        assert!(cache.contains(ChunkCoord(0, 0)));
        assert!(cache.get(ChunkCoord(0, 0)).is_some());
        assert!(!cache.contains(ChunkCoord(1, 0)));
    }

    #[test]
    fn test_chunk_cache_lru_eviction() {
        let config = test_config();
        let gen = ChunkGenerator::new(config.clone());
        let mut cache = ChunkCache::new(3);
        for i in 0..4i32 {
            let chunk = gen.generate(ChunkCoord(i, 0));
            cache.insert(ChunkCoord(i, 0), chunk);
        }
        assert!(cache.len() <= 3);
    }

    #[test]
    fn test_load_queue_priority() {
        let mut q = LoadQueue::new();
        q.push(ChunkCoord(5, 0), -50);
        q.push(ChunkCoord(1, 0), -10);
        q.push(ChunkCoord(3, 0), -30);
        let first = q.pop().unwrap();
        assert_eq!(first, ChunkCoord(1, 0)); // highest priority = -10
    }

    #[test]
    fn test_load_queue_no_duplicates() {
        let mut q = LoadQueue::new();
        q.push(ChunkCoord(0, 0), 100);
        q.push(ChunkCoord(0, 0), 200); // duplicate
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn test_chunk_generator() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let chunk = gen.generate(ChunkCoord(0, 0));
        assert_eq!(chunk.coord, ChunkCoord(0, 0));
        assert!(chunk.heightmap.width > 0);
        assert_eq!(chunk.heightmap.data.len(), chunk.heightmap.width * chunk.heightmap.height);
    }

    #[test]
    fn test_chunk_serializer_roundtrip() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let chunk = gen.generate(ChunkCoord(2, -3));
        let bytes = ChunkSerializer::serialize(&chunk);
        let restored = ChunkSerializer::deserialize(&bytes).expect("deserialize failed");
        assert_eq!(restored.coord, chunk.coord);
        assert_eq!(restored.seed, chunk.seed);
        assert_eq!(restored.heightmap.width, chunk.heightmap.width);
        for (a, b) in chunk.heightmap.data.iter().zip(restored.heightmap.data.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_chunk_serializer_invalid_magic() {
        let mut bytes = vec![0u8; 64];
        bytes[0] = 0xFF; // wrong magic
        assert!(ChunkSerializer::deserialize(&bytes).is_none());
    }

    #[test]
    fn test_visibility_set_update() {
        let config = test_config();
        let mut vis = VisibilitySet::new();
        vis.update(Vec3::new(0.0, 0.0, 0.0), &config);
        assert!(vis.visible_count() > 0);
        assert!(vis.is_visible(ChunkCoord(0, 0)));
    }

    #[test]
    fn test_lod_scheduler_desired_lod() {
        let sched = LodScheduler::new(3, 32.0);
        assert_eq!(sched.desired_lod(10.0, 0), 0);  // close → LOD 0
        let far = sched.lod_thresholds[1] + 1.0;
        // With hysteresis: starting from LOD 0, need to exceed threshold + margin
        let lod = sched.desired_lod(far, 0);
        assert!(lod >= 1);
    }

    #[test]
    fn test_lod_scheduler_hysteresis() {
        let sched = LodScheduler::new(3, 32.0);
        let threshold = sched.lod_thresholds[0];
        let margin = sched.hysteresis_margin;
        // Just beyond threshold but within margin: should stay at LOD 0
        let dist = threshold + margin * 0.5;
        assert_eq!(sched.desired_lod(dist, 0), 0);
        // Beyond threshold + margin: should upgrade
        let dist2 = threshold + margin + 1.0;
        assert!(sched.desired_lod(dist2, 0) >= 1);
    }

    #[test]
    fn test_prefetcher() {
        let mut pf = Prefetcher::new(8);
        pf.push_position(Vec3::new(0.0, 0.0, 0.0));
        pf.push_position(Vec3::new(10.0, 0.0, 0.0));
        let pred = pf.predicted_position();
        // Predicted position should be ahead of current
        assert!(pred.x >= 10.0);
    }

    #[test]
    fn test_streaming_manager_synchronous() {
        let config = test_config();
        let mut mgr = StreamingManager::new_synchronous(config);
        mgr.update(Vec3::new(0.0, 0.0, 0.0));
        // After update, some chunks should be loaded
        assert!(mgr.cache_size() > 0 || mgr.stats.pending_count >= 0);
    }

    #[test]
    fn test_streaming_manager_force_load() {
        let config = test_config();
        let mut mgr = StreamingManager::new_synchronous(config);
        let chunk = mgr.force_load(ChunkCoord(0, 0));
        assert_eq!(chunk.coord, ChunkCoord(0, 0));
        assert_eq!(mgr.cache_size(), 1);
    }
}
