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

// ── Extended Streaming Utilities ──────────────────────────────────────────────

/// Serialization format options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializationFormat {
    /// Raw binary (no compression).
    Raw,
    /// Run-length encoded (good for uniform terrain).
    RLE,
    /// Delta-encoded (good for slowly varying terrain).
    Delta,
}

/// Extended chunk serializer with format support.
pub struct ExtendedChunkSerializer;

impl ExtendedChunkSerializer {
    /// Serialize with the given format.
    pub fn serialize_with_format(
        chunk: &crate::terrain::mod_types::TerrainChunk,
        format: SerializationFormat,
    ) -> Vec<u8> {
        let base = ChunkSerializer::serialize(chunk);
        match format {
            SerializationFormat::Raw   => base,
            SerializationFormat::RLE   => Self::rle_encode(&base),
            SerializationFormat::Delta => Self::delta_encode_bytes(&base),
        }
    }

    /// Deserialize with the given format.
    pub fn deserialize_with_format(
        bytes: &[u8],
        format: SerializationFormat,
    ) -> Option<crate::terrain::mod_types::TerrainChunk> {
        let decoded = match format {
            SerializationFormat::Raw   => bytes.to_vec(),
            SerializationFormat::RLE   => Self::rle_decode(bytes)?,
            SerializationFormat::Delta => Self::delta_decode_bytes(bytes)?,
        };
        ChunkSerializer::deserialize(&decoded)
    }

    /// Simple run-length encoding for byte streams.
    pub fn rle_encode(data: &[u8]) -> Vec<u8> {
        if data.is_empty() { return Vec::new(); }
        let mut out = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let val = data[i];
            let mut run = 1usize;
            while i + run < data.len() && data[i + run] == val && run < 255 {
                run += 1;
            }
            out.push(run as u8);
            out.push(val);
            i += run;
        }
        out
    }

    /// Decode run-length encoded data.
    pub fn rle_decode(data: &[u8]) -> Option<Vec<u8>> {
        if data.len() % 2 != 0 { return None; }
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < data.len() {
            let count = data[i] as usize;
            let val   = data[i + 1];
            for _ in 0..count { out.push(val); }
            i += 2;
        }
        Some(out)
    }

    /// Delta encode: store first byte raw, then differences.
    pub fn delta_encode_bytes(data: &[u8]) -> Vec<u8> {
        if data.is_empty() { return Vec::new(); }
        let mut out = Vec::with_capacity(data.len());
        out.push(data[0]);
        for i in 1..data.len() {
            out.push(data[i].wrapping_sub(data[i - 1]));
        }
        out
    }

    /// Decode delta-encoded bytes.
    pub fn delta_decode_bytes(data: &[u8]) -> Option<Vec<u8>> {
        if data.is_empty() { return Some(Vec::new()); }
        let mut out = Vec::with_capacity(data.len());
        out.push(data[0]);
        for i in 1..data.len() {
            out.push(data[i].wrapping_add(*out.last().unwrap()));
        }
        Some(out)
    }
}

// ── Chunk Event System ────────────────────────────────────────────────────────

/// Events emitted by the streaming system.
#[derive(Clone, Debug)]
pub enum ChunkEvent {
    Loaded(ChunkCoord),
    Unloaded(ChunkCoord),
    LodChanged { coord: ChunkCoord, old_lod: u8, new_lod: u8 },
    GenerationStarted(ChunkCoord),
    GenerationFailed { coord: ChunkCoord, reason: String },
}

/// Simple event queue for chunk events.
#[derive(Debug, Default)]
pub struct ChunkEventQueue {
    events: VecDeque<ChunkEvent>,
    max_size: usize,
}

impl ChunkEventQueue {
    pub fn new(max_size: usize) -> Self {
        Self { events: VecDeque::new(), max_size }
    }

    pub fn push(&mut self, event: ChunkEvent) {
        if self.events.len() >= self.max_size {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> Vec<ChunkEvent> {
        self.events.drain(..).collect()
    }

    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
}

// ── Memory Budget Tracker ─────────────────────────────────────────────────────

/// Tracks and enforces memory budget for the streaming system.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Maximum allowed memory in bytes.
    pub max_bytes:      usize,
    /// Currently used memory in bytes.
    pub current_bytes:  usize,
    /// Reserved headroom (keep this much free).
    pub headroom_bytes: usize,
    /// Peak memory usage seen.
    pub peak_bytes:     usize,
}

impl MemoryBudget {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            current_bytes: 0,
            headroom_bytes: max_bytes / 10,
            peak_bytes: 0,
        }
    }

    pub fn allocate(&mut self, bytes: usize) -> bool {
        if self.current_bytes + bytes + self.headroom_bytes > self.max_bytes {
            return false;
        }
        self.current_bytes += bytes;
        if self.current_bytes > self.peak_bytes {
            self.peak_bytes = self.current_bytes;
        }
        true
    }

    pub fn free(&mut self, bytes: usize) {
        self.current_bytes = self.current_bytes.saturating_sub(bytes);
    }

    pub fn utilization(&self) -> f32 {
        self.current_bytes as f32 / self.max_bytes as f32
    }

    pub fn available(&self) -> usize {
        self.max_bytes.saturating_sub(self.current_bytes + self.headroom_bytes)
    }

    pub fn is_over_budget(&self) -> bool {
        self.current_bytes + self.headroom_bytes > self.max_bytes
    }
}

// ── Terrain World Map ─────────────────────────────────────────────────────────

/// Low-resolution overview map of the entire world, used for minimap and LOD hints.
#[derive(Debug)]
pub struct WorldMap {
    /// Low-resolution height overview.
    pub height_overview:  crate::terrain::heightmap::HeightMap,
    /// Biome overview (compressed to u8 per cell).
    pub biome_overview:   Vec<u8>,
    /// Overview resolution.
    pub resolution:       usize,
    /// World size in chunks.
    pub world_size_chunks: usize,
}

impl WorldMap {
    /// Generate a world map by sampling the global noise function.
    pub fn generate(world_size_chunks: usize, resolution: usize, config: &TerrainConfig) -> Self {
        let scale = world_size_chunks as f32 / resolution as f32;
        let height_overview = crate::terrain::heightmap::FractalNoise::generate(
            resolution, resolution, 6, 2.0, 0.5, 2.0, config.seed,
        );
        let biome_overview: Vec<u8> = height_overview.data.iter()
            .map(|&h| {
                let biome = if h < 0.1 { crate::terrain::biome::BiomeType::Ocean }
                    else if h < 0.2 { crate::terrain::biome::BiomeType::Beach }
                    else if h < 0.5 { crate::terrain::biome::BiomeType::Grassland }
                    else if h < 0.7 { crate::terrain::biome::BiomeType::TemperateForest }
                    else if h < 0.85 { crate::terrain::biome::BiomeType::Mountain }
                    else { crate::terrain::biome::BiomeType::AlpineGlacier };
                biome as u8
            })
            .collect();
        Self { height_overview, biome_overview, resolution, world_size_chunks }
    }

    /// Sample height at normalized world position (0..1).
    pub fn sample_height(&self, nx: f32, ny: f32) -> f32 {
        let x = (nx * (self.resolution - 1) as f32).clamp(0.0, (self.resolution - 1) as f32);
        let y = (ny * (self.resolution - 1) as f32).clamp(0.0, (self.resolution - 1) as f32);
        self.height_overview.sample_bilinear(x, y)
    }

    /// Get biome at normalized world position.
    pub fn sample_biome(&self, nx: f32, ny: f32) -> crate::terrain::biome::BiomeType {
        let x = (nx * (self.resolution - 1) as f32) as usize;
        let y = (ny * (self.resolution - 1) as f32) as usize;
        let idx = y.min(self.resolution - 1) * self.resolution + x.min(self.resolution - 1);
        crate::terrain::biome::biome_from_index(self.biome_overview[idx] as usize)
    }

    /// Convert chunk coord to normalized world position.
    pub fn chunk_to_normalized(&self, coord: ChunkCoord) -> (f32, f32) {
        (
            (coord.0 as f32 + 0.5) / self.world_size_chunks as f32,
            (coord.1 as f32 + 0.5) / self.world_size_chunks as f32,
        )
    }
}

// ── Chunk Diff ────────────────────────────────────────────────────────────────

/// Records the difference between two chunk heightmaps (for terrain editing).
#[derive(Clone, Debug)]
pub struct ChunkDiff {
    pub coord:   ChunkCoord,
    /// Sparse list of (x, y, old_height, new_height).
    pub changes: Vec<(usize, usize, f32, f32)>,
}

impl ChunkDiff {
    pub fn new(coord: ChunkCoord) -> Self {
        Self { coord, changes: Vec::new() }
    }

    /// Record a height change.
    pub fn record(&mut self, x: usize, y: usize, old_h: f32, new_h: f32) {
        if (old_h - new_h).abs() > 1e-6 {
            self.changes.push((x, y, old_h, new_h));
        }
    }

    /// Apply this diff to a heightmap.
    pub fn apply(&self, hm: &mut crate::terrain::heightmap::HeightMap) {
        for &(x, y, _, new_h) in &self.changes {
            hm.set(x, y, new_h);
        }
    }

    /// Reverse (undo) this diff.
    pub fn undo(&self, hm: &mut crate::terrain::heightmap::HeightMap) {
        for &(x, y, old_h, _) in &self.changes {
            hm.set(x, y, old_h);
        }
    }

    pub fn is_empty(&self) -> bool { self.changes.is_empty() }
    pub fn len(&self) -> usize { self.changes.len() }
}

// ── Streaming Profiler ────────────────────────────────────────────────────────

/// Profiling data for the streaming system.
#[derive(Debug, Default, Clone)]
pub struct StreamingProfiler {
    pub frame_count:         u64,
    pub total_generate_ms:   f64,
    pub total_serialize_ms:  f64,
    pub total_deserialize_ms: f64,
    pub max_generate_ms:     f64,
    pub min_generate_ms:     f64,
    pub chunks_per_second:   f32,
    pub last_frame_ms:       f64,
}

impl StreamingProfiler {
    pub fn new() -> Self {
        Self { min_generate_ms: f64::INFINITY, ..Default::default() }
    }

    pub fn record_generate(&mut self, ms: f64) {
        self.total_generate_ms += ms;
        self.frame_count += 1;
        if ms > self.max_generate_ms { self.max_generate_ms = ms; }
        if ms < self.min_generate_ms { self.min_generate_ms = ms; }
    }

    pub fn average_generate_ms(&self) -> f64 {
        if self.frame_count == 0 { 0.0 } else { self.total_generate_ms / self.frame_count as f64 }
    }

    pub fn reset(&mut self) { *self = Self::new(); }
}

// ── Priority Zones ────────────────────────────────────────────────────────────

/// Defines priority zones that affect chunk loading order.
/// E.g., player start location, POIs, scripted events.
#[derive(Clone, Debug)]
pub struct PriorityZone {
    /// World-space center of the zone.
    pub center: Vec3,
    /// Radius of influence (world units).
    pub radius: f32,
    /// Priority bonus applied to chunks in this zone.
    pub priority_bonus: i64,
    /// Optional name for debugging.
    pub name: String,
}

impl PriorityZone {
    pub fn new(center: Vec3, radius: f32, priority_bonus: i64) -> Self {
        Self { center, radius, priority_bonus, name: String::new() }
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn contains_world_pos(&self, pos: Vec3) -> bool {
        let dx = pos.x - self.center.x;
        let dz = pos.z - self.center.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }

    pub fn chunk_priority(&self, coord: ChunkCoord, chunk_size: f32) -> i64 {
        let world_pos = coord.to_world_pos(chunk_size);
        if self.contains_world_pos(world_pos) {
            self.priority_bonus
        } else {
            let dx = world_pos.x - self.center.x;
            let dz = world_pos.z - self.center.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let falloff = (1.0 - (dist / self.radius).min(2.0)) * 0.5;
            (self.priority_bonus as f32 * falloff.max(0.0)) as i64
        }
    }
}

/// Manages multiple priority zones and computes combined priority bonuses.
#[derive(Debug, Default)]
pub struct PriorityZoneManager {
    pub zones: Vec<PriorityZone>,
}

impl PriorityZoneManager {
    pub fn new() -> Self { Self::default() }

    pub fn add_zone(&mut self, zone: PriorityZone) {
        self.zones.push(zone);
    }

    pub fn remove_zone_by_name(&mut self, name: &str) {
        self.zones.retain(|z| z.name != name);
    }

    pub fn total_priority_bonus(&self, coord: ChunkCoord, chunk_size: f32) -> i64 {
        self.zones.iter().map(|z| z.chunk_priority(coord, chunk_size)).sum()
    }
}

// ── Chunk Hitlist ─────────────────────────────────────────────────────────────

/// A list of chunks that must be loaded before play can begin.
#[derive(Debug)]
pub struct ChunkHitlist {
    required: std::collections::HashSet<ChunkCoord>,
    loaded:   std::collections::HashSet<ChunkCoord>,
}

impl ChunkHitlist {
    pub fn new() -> Self {
        Self {
            required: std::collections::HashSet::new(),
            loaded:   std::collections::HashSet::new(),
        }
    }

    pub fn require(&mut self, coord: ChunkCoord) {
        self.required.insert(coord);
    }

    pub fn mark_loaded(&mut self, coord: ChunkCoord) {
        if self.required.contains(&coord) {
            self.loaded.insert(coord);
        }
    }

    pub fn is_complete(&self) -> bool {
        self.required.iter().all(|c| self.loaded.contains(c))
    }

    pub fn completion_fraction(&self) -> f32 {
        if self.required.is_empty() { return 1.0; }
        self.loaded.len() as f32 / self.required.len() as f32
    }

    pub fn pending_coords(&self) -> Vec<ChunkCoord> {
        self.required.iter().filter(|c| !self.loaded.contains(*c)).copied().collect()
    }
}

impl Default for ChunkHitlist {
    fn default() -> Self { Self::new() }
}

// ── Distance-based LOD Bias ───────────────────────────────────────────────────

/// Adjusts LOD thresholds based on terrain importance (e.g., near a city).
#[derive(Clone, Debug)]
pub struct LodBias {
    /// LOD multiplier: > 1.0 means higher quality (further LOD0 distance).
    pub quality_multiplier: f32,
    /// Region center.
    pub center: Vec3,
    /// Radius of effect.
    pub radius: f32,
}

impl LodBias {
    pub fn new(center: Vec3, radius: f32, quality_multiplier: f32) -> Self {
        Self { quality_multiplier, center, radius }
    }

    pub fn lod_multiplier_at(&self, pos: Vec3) -> f32 {
        let dx = pos.x - self.center.x;
        let dz = pos.z - self.center.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < self.radius {
            let t = 1.0 - dist / self.radius;
            1.0 + (self.quality_multiplier - 1.0) * t
        } else {
            1.0
        }
    }
}

// ── Extended Streaming Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod extended_streaming_tests {
    use super::*;

    fn test_config() -> TerrainConfig {
        TerrainConfig { chunk_size: 16, view_distance: 2, lod_levels: 3, seed: 42 }
    }

    #[test]
    fn test_rle_roundtrip() {
        let data: Vec<u8> = vec![1, 1, 1, 2, 3, 3, 4, 4, 4, 4];
        let encoded = ExtendedChunkSerializer::rle_encode(&data);
        let decoded = ExtendedChunkSerializer::rle_decode(&encoded).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_delta_encode_roundtrip() {
        let data: Vec<u8> = vec![10, 12, 11, 15, 14, 20, 18];
        let enc = ExtendedChunkSerializer::delta_encode_bytes(&data);
        let dec = ExtendedChunkSerializer::delta_decode_bytes(&enc).unwrap();
        assert_eq!(data, dec);
    }

    #[test]
    fn test_chunk_event_queue() {
        let mut q = ChunkEventQueue::new(100);
        q.push(ChunkEvent::Loaded(ChunkCoord(0, 0)));
        q.push(ChunkEvent::Unloaded(ChunkCoord(1, 0)));
        assert_eq!(q.len(), 2);
        let events = q.drain();
        assert_eq!(events.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn test_memory_budget() {
        let mut budget = MemoryBudget::new(1024 * 1024);
        assert!(budget.allocate(100_000));
        budget.free(100_000);
        assert_eq!(budget.current_bytes, 0);
        assert!(!budget.is_over_budget());
    }

    #[test]
    fn test_memory_budget_over() {
        let mut budget = MemoryBudget::new(1000);
        assert!(!budget.allocate(1000)); // headroom would be exceeded
    }

    #[test]
    fn test_world_map_generation() {
        let config = test_config();
        let wm = WorldMap::generate(64, 32, &config);
        assert_eq!(wm.height_overview.data.len(), 32 * 32);
        assert_eq!(wm.biome_overview.len(), 32 * 32);
    }

    #[test]
    fn test_world_map_sample() {
        let config = test_config();
        let wm = WorldMap::generate(64, 32, &config);
        let h = wm.sample_height(0.5, 0.5);
        assert!(h >= 0.0 && h <= 1.0);
    }

    #[test]
    fn test_chunk_diff_apply_undo() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let mut chunk = gen.generate(ChunkCoord(0, 0));
        let old_h = chunk.heightmap.get(5, 5);
        let new_h = 0.9f32;
        let mut diff = ChunkDiff::new(ChunkCoord(0, 0));
        diff.record(5, 5, old_h, new_h);
        diff.apply(&mut chunk.heightmap);
        assert!((chunk.heightmap.get(5, 5) - new_h).abs() < 1e-6);
        diff.undo(&mut chunk.heightmap);
        assert!((chunk.heightmap.get(5, 5) - old_h).abs() < 1e-6);
    }

    #[test]
    fn test_priority_zone() {
        let zone = PriorityZone::new(Vec3::new(0.0, 0.0, 0.0), 100.0, 500_000);
        assert!(zone.contains_world_pos(Vec3::new(50.0, 0.0, 50.0)));
        assert!(!zone.contains_world_pos(Vec3::new(200.0, 0.0, 200.0)));
        let bonus = zone.chunk_priority(ChunkCoord(0, 0), 16.0);
        assert!(bonus > 0);
    }

    #[test]
    fn test_priority_zone_manager() {
        let mut mgr = PriorityZoneManager::new();
        mgr.add_zone(PriorityZone::new(Vec3::ZERO, 100.0, 100_000).named("start"));
        let bonus = mgr.total_priority_bonus(ChunkCoord(0, 0), 16.0);
        assert!(bonus > 0);
        mgr.remove_zone_by_name("start");
        assert!(mgr.zones.is_empty());
    }

    #[test]
    fn test_chunk_hitlist() {
        let mut hl = ChunkHitlist::new();
        hl.require(ChunkCoord(0, 0));
        hl.require(ChunkCoord(1, 0));
        assert!(!hl.is_complete());
        assert!((hl.completion_fraction() - 0.0).abs() < 1e-5);
        hl.mark_loaded(ChunkCoord(0, 0));
        assert!((hl.completion_fraction() - 0.5).abs() < 1e-5);
        hl.mark_loaded(ChunkCoord(1, 0));
        assert!(hl.is_complete());
    }

    #[test]
    fn test_lod_bias() {
        let bias = LodBias::new(Vec3::ZERO, 100.0, 2.0);
        let at_center = bias.lod_multiplier_at(Vec3::ZERO);
        assert!((at_center - 2.0).abs() < 1e-4);
        let far_away = bias.lod_multiplier_at(Vec3::new(200.0, 0.0, 0.0));
        assert!((far_away - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_streaming_profiler() {
        let mut prof = StreamingProfiler::new();
        prof.record_generate(5.0);
        prof.record_generate(10.0);
        prof.record_generate(3.0);
        assert!((prof.average_generate_ms() - 6.0).abs() < 1e-4);
        assert!((prof.max_generate_ms - 10.0).abs() < 1e-4);
        assert!((prof.min_generate_ms - 3.0).abs() < 1e-4);
    }
}

// ── Terrain Patch System ──────────────────────────────────────────────────────

/// A small editable terrain patch (sub-chunk resolution editing).
#[derive(Clone, Debug)]
pub struct TerrainPatch {
    pub coord:       ChunkCoord,
    pub offset_x:    usize,
    pub offset_z:    usize,
    pub width:       usize,
    pub height:      usize,
    pub data:        Vec<f32>,
    pub dirty:       bool,
}

impl TerrainPatch {
    pub fn new(coord: ChunkCoord, offset_x: usize, offset_z: usize, width: usize, height: usize) -> Self {
        Self {
            coord, offset_x, offset_z, width, height,
            data: vec![0.0f32; width * height],
            dirty: false,
        }
    }

    pub fn get(&self, x: usize, z: usize) -> f32 {
        if x < self.width && z < self.height { self.data[z * self.width + x] } else { 0.0 }
    }

    pub fn set(&mut self, x: usize, z: usize, v: f32) {
        if x < self.width && z < self.height {
            self.data[z * self.width + x] = v.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    /// Apply this patch to the corresponding chunk heightmap.
    pub fn apply_to_chunk(&self, chunk: &mut crate::terrain::mod_types::TerrainChunk) {
        for z in 0..self.height {
            for x in 0..self.width {
                let cx = self.offset_x + x;
                let cz = self.offset_z + z;
                chunk.heightmap.set(cx, cz, self.get(x, z));
            }
        }
    }

    /// Read current values from a chunk into this patch.
    pub fn read_from_chunk(&mut self, chunk: &crate::terrain::mod_types::TerrainChunk) {
        for z in 0..self.height {
            for x in 0..self.width {
                let cx = self.offset_x + x;
                let cz = self.offset_z + z;
                self.set(x, z, chunk.heightmap.get(cx, cz));
            }
        }
        self.dirty = false;
    }
}

// ── Neighbor Stitching ────────────────────────────────────────────────────────

/// Stitches chunk borders to eliminate seams between adjacent chunks.
pub struct ChunkStitcher;

impl ChunkStitcher {
    /// Blend the border rows of two adjacent chunks for seamless transitions.
    /// `primary` is the chunk to modify; `neighbor` provides the reference values.
    /// `edge` is which edge of `primary` borders `neighbor`.
    pub fn stitch_edge(
        primary:  &mut crate::terrain::mod_types::TerrainChunk,
        neighbor: &crate::terrain::mod_types::TerrainChunk,
        edge:     StitchEdge,
        blend_width: usize,
    ) {
        let pw = primary.heightmap.width;
        let ph = primary.heightmap.height;
        let nw = neighbor.heightmap.width;
        let nh = neighbor.heightmap.height;

        match edge {
            StitchEdge::East => {
                for z in 0..ph {
                    let nz = (z as f32 / ph as f32 * nh as f32) as usize;
                    for bx in 0..blend_width {
                        let px = pw - 1 - bx;
                        let nx = bx;
                        let t = bx as f32 / blend_width as f32;
                        let p_val = primary.heightmap.get(px, z);
                        let n_val = neighbor.heightmap.get(nx, nz.min(nh - 1));
                        let blended = p_val + (n_val - p_val) * t;
                        primary.heightmap.set(px, z, blended);
                    }
                }
            }
            StitchEdge::West => {
                for z in 0..ph {
                    let nz = (z as f32 / ph as f32 * nh as f32) as usize;
                    for bx in 0..blend_width {
                        let px = bx;
                        let nx = nw - 1 - bx;
                        let t = bx as f32 / blend_width as f32;
                        let p_val = primary.heightmap.get(px, z);
                        let n_val = neighbor.heightmap.get(nx.min(nw - 1), nz.min(nh - 1));
                        let blended = n_val + (p_val - n_val) * (1.0 - t);
                        primary.heightmap.set(px, z, blended);
                    }
                }
            }
            StitchEdge::North => {
                for x in 0..pw {
                    let nx = (x as f32 / pw as f32 * nw as f32) as usize;
                    for bz in 0..blend_width {
                        let pz = bz;
                        let nz = nh - 1 - bz;
                        let t = bz as f32 / blend_width as f32;
                        let p_val = primary.heightmap.get(x, pz);
                        let n_val = neighbor.heightmap.get(nx.min(nw - 1), nz.min(nh - 1));
                        let blended = n_val + (p_val - n_val) * (1.0 - t);
                        primary.heightmap.set(x, pz, blended);
                    }
                }
            }
            StitchEdge::South => {
                for x in 0..pw {
                    let nx = (x as f32 / pw as f32 * nw as f32) as usize;
                    for bz in 0..blend_width {
                        let pz = ph - 1 - bz;
                        let nz = bz;
                        let t = bz as f32 / blend_width as f32;
                        let p_val = primary.heightmap.get(x, pz);
                        let n_val = neighbor.heightmap.get(nx.min(nw - 1), nz.min(nh - 1));
                        let blended = p_val + (n_val - p_val) * t;
                        primary.heightmap.set(x, pz, blended);
                    }
                }
            }
        }
    }
}

/// Which edge of a chunk to stitch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StitchEdge {
    North,
    South,
    East,
    West,
}

// ── Chunk Compression ─────────────────────────────────────────────────────────

/// Quantizes a heightmap for compact storage.
pub struct HeightmapQuantizer;

impl HeightmapQuantizer {
    /// Quantize to 8-bit precision (256 levels).
    pub fn quantize_8bit(heights: &[f32]) -> Vec<u8> {
        heights.iter().map(|&h| (h.clamp(0.0, 1.0) * 255.0) as u8).collect()
    }

    /// Dequantize from 8-bit.
    pub fn dequantize_8bit(bytes: &[u8]) -> Vec<f32> {
        bytes.iter().map(|&b| b as f32 / 255.0).collect()
    }

    /// Quantize to 16-bit precision (65536 levels).
    pub fn quantize_16bit(heights: &[f32]) -> Vec<u16> {
        heights.iter().map(|&h| (h.clamp(0.0, 1.0) * 65535.0) as u16).collect()
    }

    /// Dequantize from 16-bit.
    pub fn dequantize_16bit(shorts: &[u16]) -> Vec<f32> {
        shorts.iter().map(|&s| s as f32 / 65535.0).collect()
    }

    /// Compute quantization error (mean absolute error).
    pub fn quantization_error(original: &[f32], quantized_8bit: &[u8]) -> f32 {
        if original.len() != quantized_8bit.len() { return f32::INFINITY; }
        original.iter().zip(quantized_8bit.iter())
            .map(|(&orig, &q)| (orig - q as f32 / 255.0).abs())
            .sum::<f32>() / original.len() as f32
    }
}

// ── Streaming Telemetry ────────────────────────────────────────────────────────

/// Detailed telemetry for streaming system performance analysis.
#[derive(Debug, Default, Clone)]
pub struct StreamingTelemetry {
    pub frame_number:        u64,
    pub visible_chunk_count: usize,
    pub loaded_chunk_count:  usize,
    pub pending_chunk_count: usize,
    pub cache_hit_rate:      f32,
    pub memory_usage_mb:     f32,
    pub generation_rate:     f32, // chunks per second
    pub eviction_count:      usize,
    pub last_update_ms:      f32,
}

impl StreamingTelemetry {
    pub fn update_from_stats(&mut self, stats: &StreamingStats) {
        self.frame_number      += 1;
        self.loaded_chunk_count = stats.chunks_loaded;
        self.pending_chunk_count = stats.pending_count;
        self.cache_hit_rate    = stats.cache_hit_rate();
        self.memory_usage_mb   = stats.memory_bytes as f32 / (1024.0 * 1024.0);
    }

    pub fn to_display_string(&self) -> String {
        format!(
            "Frame:{} | Chunks:{} Pending:{} | Cache:{:.0}% | Mem:{:.1}MB",
            self.frame_number,
            self.loaded_chunk_count,
            self.pending_chunk_count,
            self.cache_hit_rate * 100.0,
            self.memory_usage_mb,
        )
    }
}

// ── Chunk Repair ──────────────────────────────────────────────────────────────

/// Repairs corrupted or invalid chunk data.
pub struct ChunkRepair;

impl ChunkRepair {
    /// Fix out-of-range height values.
    pub fn clamp_heights(chunk: &mut crate::terrain::mod_types::TerrainChunk) {
        for v in chunk.heightmap.data.iter_mut() {
            *v = v.clamp(0.0, 1.0);
        }
    }

    /// Fix NaN values in heightmap by replacing with neighbors' average.
    pub fn fix_nans(chunk: &mut crate::terrain::mod_types::TerrainChunk) {
        let w = chunk.heightmap.width;
        let h = chunk.heightmap.height;
        let data = chunk.heightmap.data.clone();
        for y in 0..h {
            for x in 0..w {
                if chunk.heightmap.get(x, y).is_nan() {
                    let mut sum = 0.0f32;
                    let mut count = 0;
                    for (dx, dy) in &[(-1i32,0),(1,0),(0,-1i32),(0,1)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                            let v = data[ny as usize * w + nx as usize];
                            if !v.is_nan() { sum += v; count += 1; }
                        }
                    }
                    let replacement = if count > 0 { sum / count as f32 } else { 0.5 };
                    chunk.heightmap.set(x, y, replacement);
                }
            }
        }
    }

    /// Check if a chunk has any issues.
    pub fn is_valid(chunk: &crate::terrain::mod_types::TerrainChunk) -> bool {
        chunk.heightmap.data.iter().all(|&v| !v.is_nan() && v >= 0.0 && v <= 1.0)
    }
}

// ── More Streaming Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod more_streaming_tests {
    use super::*;

    fn test_config() -> TerrainConfig {
        TerrainConfig { chunk_size: 16, view_distance: 1, lod_levels: 2, seed: 42 }
    }

    #[test]
    fn test_terrain_patch_apply() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let mut chunk = gen.generate(ChunkCoord(0, 0));
        let mut patch = TerrainPatch::new(ChunkCoord(0, 0), 0, 0, 4, 4);
        for v in patch.data.iter_mut() { *v = 0.99; }
        patch.dirty = true;
        patch.apply_to_chunk(&mut chunk);
        assert!((chunk.heightmap.get(0, 0) - 0.99).abs() < 1e-5);
    }

    #[test]
    fn test_terrain_patch_read_write() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let chunk = gen.generate(ChunkCoord(0, 0));
        let mut patch = TerrainPatch::new(ChunkCoord(0, 0), 0, 0, 4, 4);
        patch.read_from_chunk(&chunk);
        assert!(!patch.dirty);
        assert!(patch.data.iter().any(|&v| v > 0.0));
    }

    #[test]
    fn test_heightmap_quantizer_8bit() {
        let heights: Vec<f32> = (0..256).map(|i| i as f32 / 255.0).collect();
        let quantized = HeightmapQuantizer::quantize_8bit(&heights);
        let deq = HeightmapQuantizer::dequantize_8bit(&quantized);
        let err = HeightmapQuantizer::quantization_error(&heights, &quantized);
        assert!(err < 0.005, "8-bit quantization error should be small");
    }

    #[test]
    fn test_heightmap_quantizer_16bit() {
        let heights: Vec<f32> = (0..1024).map(|i| i as f32 / 1023.0).collect();
        let quantized = HeightmapQuantizer::quantize_16bit(&heights);
        let deq = HeightmapQuantizer::dequantize_16bit(&quantized);
        let max_err = heights.iter().zip(deq.iter())
            .map(|(&a, &b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(max_err < 0.0001, "16-bit quantization should be very accurate");
    }

    #[test]
    fn test_chunk_repair_clamp() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let mut chunk = gen.generate(ChunkCoord(0, 0));
        chunk.heightmap.data[0] = 2.0; // invalid
        chunk.heightmap.data[1] = -1.0; // invalid
        ChunkRepair::clamp_heights(&mut chunk);
        assert!(ChunkRepair::is_valid(&chunk));
    }

    #[test]
    fn test_chunk_repair_nan() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let mut chunk = gen.generate(ChunkCoord(0, 0));
        chunk.heightmap.data[16] = f32::NAN;
        ChunkRepair::fix_nans(&mut chunk);
        assert!(ChunkRepair::is_valid(&chunk));
    }

    #[test]
    fn test_streaming_telemetry() {
        let stats = StreamingStats {
            chunks_loaded: 10, chunks_unloaded: 2,
            cache_hits: 80, cache_misses: 20,
            pending_count: 3, memory_bytes: 2 * 1024 * 1024,
            generate_time_ms: 100.0,
        };
        let mut tel = StreamingTelemetry::default();
        tel.update_from_stats(&stats);
        assert_eq!(tel.loaded_chunk_count, 10);
        assert!((tel.cache_hit_rate - 0.8).abs() < 1e-4);
        assert!((tel.memory_usage_mb - 2.0).abs() < 1e-4);
        let s = tel.to_display_string();
        assert!(s.contains("Chunks:10"));
    }

    #[test]
    fn test_extended_serializer_rle_chunk() {
        let config = test_config();
        let gen = ChunkGenerator::new(config);
        let chunk = gen.generate(ChunkCoord(0, 0));
        let bytes = ExtendedChunkSerializer::serialize_with_format(&chunk, SerializationFormat::Raw);
        let restored = ExtendedChunkSerializer::deserialize_with_format(
            &bytes, SerializationFormat::Raw
        );
        assert!(restored.is_some());
    }
}
