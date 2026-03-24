//! # Terrain System
//!
//! A complete terrain rendering and simulation system for the Proof Engine.
//!
//! ## Architecture
//!
//! The terrain system is organized into five submodules:
//!
//! - [`heightmap`] — Height field generation, erosion, analysis, and I/O
//! - [`biome`]     — Climate simulation and biome classification
//! - [`vegetation`] — Tree, grass, and rock placement with LOD
//! - [`streaming`] — Async-style chunk loading, caching, and prefetching
//! - [`mod_types`]  — Shared core data types (ChunkCoord, TerrainChunk, etc.)
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use proof_engine::terrain::*;
//!
//! // Configure terrain generation
//! let config = TerrainConfig::new(64, 8, 4, 12345);
//!
//! // Create the manager
//! let mut manager = TerrainManager::new(config);
//!
//! // Update each frame with camera position
//! manager.update(glam::Vec3::new(0.0, 50.0, 0.0));
//!
//! // Query terrain height
//! let h = manager.sample_height(100.0, 200.0);
//! println!("Height at (100, 200) = {h}");
//! ```

pub mod heightmap;
pub mod biome;
pub mod biomes;
pub mod chunks;
pub mod vegetation;
pub mod streaming;
pub mod mod_types;

pub use mod_types::{ChunkCoord, ChunkState, TerrainConfig, TerrainChunk};
pub use heightmap::{
    HeightMap, DiamondSquare, FractalNoise, VoronoiPlates, PerlinTerrain,
    HydraulicErosion, ThermalErosion, WindErosion,
};
pub use biome::{
    BiomeType, BiomeParams, BiomeClassifier, BiomeMap, ClimateMap,
    ClimateSimulator, VegetationDensity, BiomeColor, TransitionZone, SeasonFactor,
};
pub use vegetation::{
    VegetationSystem, VegetationInstance, VegetationKind, VegetationLod,
    TreeType, TreeParams, TreeSkeleton, TreeSegment, GrassCluster, GrassField,
    RockPlacement, RockCluster, VegetationPainter, ImpostorBillboard,
    generate_impostors,
};
pub use streaming::{
    StreamingManager, ChunkCache, LoadQueue, ChunkGenerator, ChunkSerializer,
    StreamingStats, VisibilitySet, LodScheduler, Prefetcher,
};

use glam::Vec3;

// ── TerrainMaterial ───────────────────────────────────────────────────────────

/// Material properties for terrain rendering.
#[derive(Clone, Debug)]
pub struct TerrainMaterial {
    pub albedo:    Vec3,
    pub normal:    Vec3,
    pub roughness: f32,
    pub layers:    Vec<TerrainLayer>,
}

impl Default for TerrainMaterial {
    fn default() -> Self {
        Self {
            albedo:    Vec3::new(0.5, 0.45, 0.3),
            normal:    Vec3::new(0.5, 1.0, 0.5),
            roughness: 0.85,
            layers:    Vec::new(),
        }
    }
}

impl TerrainMaterial {
    pub fn new() -> Self { Self::default() }

    /// Add a terrain layer.
    pub fn add_layer(&mut self, layer: TerrainLayer) {
        self.layers.push(layer);
    }

    /// Sample blended albedo based on altitude and slope.
    pub fn sample_albedo(&self, altitude: f32, slope: f32) -> Vec3 {
        if self.layers.is_empty() { return self.albedo; }
        let mut result = Vec3::ZERO;
        let mut total_weight = 0.0f32;
        for layer in &self.layers {
            let alt_blend = smooth_step(layer.blend_start, layer.blend_end, altitude);
            let slope_ok = slope >= layer.slope_min && slope <= layer.slope_max;
            let weight = alt_blend * if slope_ok { 1.0 } else { 0.0 };
            result += layer.albedo * weight;
            total_weight += weight;
        }
        if total_weight < 1e-6 { self.albedo } else { result / total_weight }
    }
}

fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── TerrainLayer ──────────────────────────────────────────────────────────────

/// A single material layer on terrain (e.g., grass, rock, snow).
#[derive(Clone, Debug)]
pub struct TerrainLayer {
    pub name:          String,
    pub albedo:        Vec3,
    pub texture_scale: f32,
    pub blend_start:   f32,
    pub blend_end:     f32,
    pub slope_min:     f32,
    pub slope_max:     f32,
    pub roughness:     f32,
}

impl TerrainLayer {
    pub fn grass() -> Self {
        Self {
            name: "Grass".to_string(),
            albedo: Vec3::new(0.3, 0.55, 0.15),
            texture_scale: 4.0,
            blend_start: 0.05,
            blend_end: 0.5,
            slope_min: 0.0,
            slope_max: 0.4,
            roughness: 0.9,
        }
    }

    pub fn rock() -> Self {
        Self {
            name: "Rock".to_string(),
            albedo: Vec3::new(0.5, 0.47, 0.44),
            texture_scale: 2.0,
            blend_start: 0.0,
            blend_end: 1.0,
            slope_min: 0.35,
            slope_max: 1.0,
            roughness: 0.75,
        }
    }

    pub fn snow() -> Self {
        Self {
            name: "Snow".to_string(),
            albedo: Vec3::new(0.9, 0.92, 0.95),
            texture_scale: 3.0,
            blend_start: 0.75,
            blend_end: 0.9,
            slope_min: 0.0,
            slope_max: 0.6,
            roughness: 0.3,
        }
    }

    pub fn sand() -> Self {
        Self {
            name: "Sand".to_string(),
            albedo: Vec3::new(0.85, 0.78, 0.55),
            texture_scale: 5.0,
            blend_start: 0.05,
            blend_end: 0.15,
            slope_min: 0.0,
            slope_max: 0.2,
            roughness: 0.95,
        }
    }
}

// ── TerrainCollider ───────────────────────────────────────────────────────────

/// Height-field collision query interface.
pub struct TerrainCollider<'a> {
    heightmap:    &'a HeightMap,
    chunk_size:   f32,
    height_scale: f32,
}

impl<'a> TerrainCollider<'a> {
    pub fn new(heightmap: &'a HeightMap, chunk_size: f32, height_scale: f32) -> Self {
        Self { heightmap, chunk_size, height_scale }
    }

    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let lx = (x / self.chunk_size * self.heightmap.width as f32)
            .clamp(0.0, self.heightmap.width as f32 - 1.0);
        let lz = (z / self.chunk_size * self.heightmap.height as f32)
            .clamp(0.0, self.heightmap.height as f32 - 1.0);
        self.heightmap.sample_bilinear(lx, lz) * self.height_scale
    }

    pub fn normal_at(&self, x: f32, z: f32) -> Vec3 {
        let lx = (x / self.chunk_size * self.heightmap.width as f32) as usize;
        let lz = (z / self.chunk_size * self.heightmap.height as f32) as usize;
        self.heightmap.normal_at(
            lx.min(self.heightmap.width  - 1),
            lz.min(self.heightmap.height - 1),
        )
    }

    pub fn is_below_surface(&self, x: f32, y: f32, z: f32) -> bool {
        y < self.height_at(x, z)
    }

    /// Cast a ray against the heightfield; returns distance, or None.
    pub fn ray_cast(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> Option<f32> {
        let dir = direction.normalize();
        let step = self.chunk_size / self.heightmap.width as f32;
        let mut t = 0.0f32;
        let mut above = !self.is_below_surface(origin.x, origin.y, origin.z);
        while t < max_dist {
            let p = origin + dir * t;
            let h = self.height_at(p.x, p.z);
            let now_above = p.y > h;
            if above && !now_above {
                let mut lo = t - step;
                let mut hi = t;
                for _ in 0..8 {
                    let mid = (lo + hi) * 0.5;
                    let pm = origin + dir * mid;
                    if pm.y > self.height_at(pm.x, pm.z) { lo = mid; } else { hi = mid; }
                }
                return Some((lo + hi) * 0.5);
            }
            above = now_above;
            t += step;
        }
        None
    }

    /// Test if an AABB (center, half-extents) intersects the heightfield.
    pub fn aabb_intersects(&self, center: Vec3, half_extents: Vec3) -> bool {
        let corners = [
            (center.x - half_extents.x, center.z - half_extents.z),
            (center.x + half_extents.x, center.z - half_extents.z),
            (center.x - half_extents.x, center.z + half_extents.z),
            (center.x + half_extents.x, center.z + half_extents.z),
            (center.x, center.z),
        ];
        let min_y = center.y - half_extents.y;
        corners.iter().any(|&(x, z)| self.height_at(x, z) >= min_y)
    }
}

// ── TerrainQuery ──────────────────────────────────────────────────────────────

/// High-level query API for sampling terrain properties at world positions.
pub struct TerrainQuery<'a> {
    chunks: &'a mut StreamingManager,
    config: TerrainConfig,
}

impl<'a> TerrainQuery<'a> {
    pub fn new(chunks: &'a mut StreamingManager, config: TerrainConfig) -> Self {
        Self { chunks, config }
    }

    pub fn sample_height(&mut self, x: f32, z: f32) -> f32 {
        self.chunks.sample_height_world(x, z)
    }

    pub fn sample_normal(&mut self, x: f32, z: f32) -> Vec3 {
        let chunk_world = self.config.chunk_size as f32;
        let cx = (x / chunk_world).floor() as i32;
        let cz = (z / chunk_world).floor() as i32;
        let coord = ChunkCoord(cx, cz);
        let lx = (x - cx as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32;
        let lz = (z - cz as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32;
        if let Some(chunk) = self.chunks.get_chunk(coord) {
            let xi = (lx as usize).min(chunk.heightmap.width  - 1);
            let zi = (lz as usize).min(chunk.heightmap.height - 1);
            chunk.heightmap.normal_at(xi, zi)
        } else {
            Vec3::Y
        }
    }

    pub fn get_biome(&mut self, x: f32, z: f32) -> BiomeType {
        let chunk_world = self.config.chunk_size as f32;
        let cx = (x / chunk_world).floor() as i32;
        let cz = (z / chunk_world).floor() as i32;
        let coord = ChunkCoord(cx, cz);
        let lx = ((x - cx as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32) as usize;
        let lz = ((z - cz as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32) as usize;
        if let Some(chunk) = self.chunks.get_chunk(coord) {
            if let Some(ref bm) = chunk.biome_map {
                return bm.get(lx.min(bm.width - 1), lz.min(bm.height - 1));
            }
        }
        BiomeType::Grassland
    }

    pub fn is_underwater(&mut self, x: f32, z: f32) -> bool {
        self.chunks.sample_height_world(x, z) < 0.1
    }
}

// ── TerrainManager ────────────────────────────────────────────────────────────

/// Top-level terrain system coordinator.
pub struct TerrainManager {
    pub config:        TerrainConfig,
    pub streaming:     StreamingManager,
    pub material:      TerrainMaterial,
    camera_pos:        Vec3,
    current_month:     u32,
}

impl TerrainManager {
    pub fn new(config: TerrainConfig) -> Self {
        let streaming = StreamingManager::new(config.clone());
        Self {
            streaming,
            material: Self::default_material(),
            config,
            camera_pos: Vec3::ZERO,
            current_month: 0,
        }
    }

    pub fn new_synchronous(config: TerrainConfig) -> Self {
        let streaming = StreamingManager::new_synchronous(config.clone());
        Self {
            streaming,
            material: Self::default_material(),
            config,
            camera_pos: Vec3::ZERO,
            current_month: 0,
        }
    }

    fn default_material() -> TerrainMaterial {
        let mut mat = TerrainMaterial::new();
        mat.add_layer(TerrainLayer::sand());
        mat.add_layer(TerrainLayer::grass());
        mat.add_layer(TerrainLayer::rock());
        mat.add_layer(TerrainLayer::snow());
        mat
    }

    pub fn update(&mut self, camera_pos: Vec3) {
        self.camera_pos = camera_pos;
        self.streaming.update(camera_pos);
    }

    pub fn set_month(&mut self, month: u32) {
        self.current_month = month % 12;
    }

    pub fn sample_height(&mut self, x: f32, z: f32) -> f32 {
        self.streaming.sample_height_world(x, z)
    }

    pub fn sample_normal(&mut self, x: f32, z: f32) -> Vec3 {
        let chunk_world = self.config.chunk_size as f32;
        let cx = (x / chunk_world).floor() as i32;
        let cz = (z / chunk_world).floor() as i32;
        let coord = ChunkCoord(cx, cz);
        let lx = ((x - cx as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32) as usize;
        let lz = ((z - cz as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32) as usize;
        if let Some(chunk) = self.streaming.get_chunk(coord) {
            let xi = lx.min(chunk.heightmap.width  - 1);
            let zi = lz.min(chunk.heightmap.height - 1);
            chunk.heightmap.normal_at(xi, zi)
        } else {
            Vec3::Y
        }
    }

    pub fn get_biome(&mut self, x: f32, z: f32) -> BiomeType {
        let chunk_world = self.config.chunk_size as f32;
        let cx = (x / chunk_world).floor() as i32;
        let cz = (z / chunk_world).floor() as i32;
        let coord = ChunkCoord(cx, cz);
        let lx = ((x - cx as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32) as usize;
        let lz = ((z - cz as f32 * chunk_world) / chunk_world * self.config.chunk_size as f32) as usize;
        if let Some(chunk) = self.streaming.get_chunk(coord) {
            if let Some(ref bm) = chunk.biome_map {
                return bm.get(lx.min(bm.width - 1), lz.min(bm.height - 1));
            }
        }
        BiomeType::Grassland
    }

    pub fn is_underwater(&mut self, x: f32, z: f32) -> bool {
        self.sample_height(x, z) < 0.1
    }

    pub fn stats(&self) -> &StreamingStats { self.streaming.stats() }
    pub fn loaded_chunk_count(&self) -> usize { self.streaming.cache_size() }

    pub fn ensure_loaded(&mut self, x: f32, z: f32) {
        let chunk_world = self.config.chunk_size as f32;
        let coord = ChunkCoord(
            (x / chunk_world).floor() as i32,
            (z / chunk_world).floor() as i32,
        );
        self.streaming.force_load(coord);
    }

    pub fn collider_for_chunk<'a>(chunk: &'a TerrainChunk, config: &TerrainConfig) -> TerrainCollider<'a> {
        TerrainCollider::new(&chunk.heightmap, config.chunk_size as f32, 100.0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_config() -> TerrainConfig {
        TerrainConfig { chunk_size: 16, view_distance: 1, lod_levels: 2, seed: 42 }
    }

    #[test]
    fn test_chunk_coord_neighbors() {
        let c = ChunkCoord(0, 0);
        let n4 = c.neighbors_4();
        assert!(n4.contains(&ChunkCoord(-1, 0)));
        assert!(n4.contains(&ChunkCoord(1,  0)));
        assert!(n4.contains(&ChunkCoord(0, -1)));
        assert!(n4.contains(&ChunkCoord(0,  1)));
    }

    #[test]
    fn test_chunk_coord_chebyshev() {
        assert_eq!(ChunkCoord(0, 0).chebyshev_distance(ChunkCoord(3, 2)), 3);
        assert_eq!(ChunkCoord(0, 0).chebyshev_distance(ChunkCoord(0, 0)), 0);
    }

    #[test]
    fn test_chunk_coord_euclidean() {
        let d = ChunkCoord(0, 0).euclidean_distance(ChunkCoord(3, 4));
        assert!((d - 5.0).abs() < 1e-4);
    }

    #[test]
    fn test_chunk_coord_world_pos() {
        let c = ChunkCoord(2, 3);
        let p = c.to_world_pos(64.0);
        assert!((p.x - 160.0).abs() < 1e-4);
        assert!((p.z - 224.0).abs() < 1e-4);
    }

    #[test]
    fn test_chunk_coord_from_world_pos() {
        let p = Vec3::new(130.0, 0.0, 200.0);
        let c = ChunkCoord::from_world_pos(p, 64.0);
        assert_eq!(c, ChunkCoord(2, 3));
    }

    #[test]
    fn test_chunk_coord_within_radius() {
        assert!( ChunkCoord(0, 0).within_radius(ChunkCoord(2, 2), 3));
        assert!(!ChunkCoord(0, 0).within_radius(ChunkCoord(5, 0), 3));
    }

    #[test]
    fn test_terrain_material_sample_albedo() {
        let mut mat = TerrainMaterial::new();
        mat.add_layer(TerrainLayer::grass());
        mat.add_layer(TerrainLayer::snow());
        let snow_color  = mat.sample_albedo(0.9, 0.1);
        let grass_color = mat.sample_albedo(0.2, 0.1);
        assert!(snow_color.x > grass_color.x || snow_color.y > grass_color.y);
    }

    #[test]
    fn test_terrain_collider_height_at() {
        let mut hm = HeightMap::new(64, 64);
        for i in 0..(64 * 64) { hm.data[i] = 0.5; }
        let col = TerrainCollider::new(&hm, 64.0, 100.0);
        let h = col.height_at(32.0, 32.0);
        assert!((h - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_terrain_collider_is_below() {
        let mut hm = HeightMap::new(64, 64);
        for i in 0..(64 * 64) { hm.data[i] = 0.5; }
        let col = TerrainCollider::new(&hm, 64.0, 100.0);
        assert!( col.is_below_surface(32.0, 10.0, 32.0));
        assert!(!col.is_below_surface(32.0, 80.0, 32.0));
    }

    #[test]
    fn test_terrain_collider_ray_cast() {
        let mut hm = HeightMap::new(64, 64);
        for i in 0..(64 * 64) { hm.data[i] = 0.5; }
        let col = TerrainCollider::new(&hm, 64.0, 100.0);
        let hit = col.ray_cast(
            Vec3::new(32.0, 200.0, 32.0),
            Vec3::new(0.0, -1.0, 0.0),
            300.0,
        );
        assert!(hit.is_some(), "Ray should hit flat terrain");
        let dist = hit.unwrap();
        assert!((dist - 150.0).abs() < 5.0);
    }

    #[test]
    fn test_terrain_collider_aabb() {
        let mut hm = HeightMap::new(64, 64);
        for i in 0..(64 * 64) { hm.data[i] = 0.5; }
        let col = TerrainCollider::new(&hm, 64.0, 100.0);
        assert!( col.aabb_intersects(Vec3::new(32.0,  50.0, 32.0), Vec3::new(5.0, 5.0, 5.0)));
        assert!(!col.aabb_intersects(Vec3::new(32.0, 500.0, 32.0), Vec3::new(5.0, 5.0, 5.0)));
    }

    #[test]
    fn test_terrain_manager_creation() {
        let config = simple_config();
        let manager = TerrainManager::new_synchronous(config);
        assert_eq!(manager.loaded_chunk_count(), 0);
    }

    #[test]
    fn test_terrain_manager_update() {
        let config = simple_config();
        let mut manager = TerrainManager::new_synchronous(config);
        manager.update(Vec3::new(0.0, 50.0, 0.0));
        let _s = manager.stats();
    }

    #[test]
    fn test_terrain_manager_ensure_loaded() {
        let config = simple_config();
        let mut manager = TerrainManager::new_synchronous(config);
        manager.ensure_loaded(0.0, 0.0);
        assert_eq!(manager.loaded_chunk_count(), 1);
    }

    #[test]
    fn test_terrain_layers_valid() {
        for layer in &[TerrainLayer::sand(), TerrainLayer::grass(), TerrainLayer::rock(), TerrainLayer::snow()] {
            assert!(layer.albedo.x >= 0.0 && layer.albedo.x <= 1.0);
            assert!(layer.albedo.y >= 0.0 && layer.albedo.y <= 1.0);
            assert!(layer.albedo.z >= 0.0 && layer.albedo.z <= 1.0);
            assert!(layer.blend_start <= layer.blend_end);
        }
    }
}

// ── Terrain Painter ───────────────────────────────────────────────────────────

/// A brush tool for real-time terrain sculpting.
#[derive(Clone, Debug)]
pub struct TerrainPainter {
    pub brush_radius:   f32,
    pub brush_strength: f32,
    pub brush_falloff:  BrushFalloff,
    pub mode:           PaintMode,
}

/// Brush falloff shape.
#[derive(Clone, Copy, Debug)]
pub enum BrushFalloff {
    Linear,
    Smooth,
    Constant,
    Gaussian,
}

/// What operation the terrain painter performs.
#[derive(Clone, Copy, Debug)]
pub enum PaintMode {
    Raise,
    Lower,
    Flatten { target: f32 },
    Smooth,
    Noise { seed: u64, scale: f32 },
}

impl TerrainPainter {
    pub fn new(radius: f32, strength: f32) -> Self {
        Self {
            brush_radius: radius,
            brush_strength: strength,
            brush_falloff: BrushFalloff::Smooth,
            mode: PaintMode::Raise,
        }
    }

    fn falloff(&self, dist_normalized: f32) -> f32 {
        match self.brush_falloff {
            BrushFalloff::Linear    => (1.0 - dist_normalized).max(0.0),
            BrushFalloff::Smooth    => {
                let t = (1.0 - dist_normalized).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }
            BrushFalloff::Constant  => if dist_normalized < 1.0 { 1.0 } else { 0.0 },
            BrushFalloff::Gaussian  => {
                let sigma = 0.4f32;
                (-(dist_normalized * dist_normalized) / (2.0 * sigma * sigma)).exp()
            }
        }
    }

    /// Apply brush at world position (cx, cz) to a heightmap.
    pub fn apply(&self, hm: &mut HeightMap, cx: f32, cz: f32) {
        let r = self.brush_radius;
        let x0 = ((cx - r).floor() as i32).max(0) as usize;
        let z0 = ((cz - r).floor() as i32).max(0) as usize;
        let x1 = ((cx + r).ceil()  as i32).min(hm.width  as i32 - 1) as usize;
        let z1 = ((cz + r).ceil()  as i32).min(hm.height as i32 - 1) as usize;

        for z in z0..=z1 {
            for x in x0..=x1 {
                let dx = x as f32 - cx;
                let dz = z as f32 - cz;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist >= r { continue; }
                let falloff = self.falloff(dist / r);
                let delta = self.brush_strength * falloff;
                let cur = hm.get(x, z);
                let new_val = match self.mode {
                    PaintMode::Raise         => cur + delta,
                    PaintMode::Lower         => cur - delta,
                    PaintMode::Flatten { target } => cur + (target - cur) * delta,
                    PaintMode::Smooth        => {
                        let n = hm.normal_at(x, z);
                        // Smooth toward neighborhood average
                        let neighbors = [
                            hm.get(x.saturating_sub(1), z),
                            hm.get((x+1).min(hm.width-1), z),
                            hm.get(x, z.saturating_sub(1)),
                            hm.get(x, (z+1).min(hm.height-1)),
                        ];
                        let avg = neighbors.iter().sum::<f32>() / 4.0;
                        cur + (avg - cur) * delta
                    }
                    PaintMode::Noise { seed, scale } => {
                        let noise = heightmap::GradientNoisePublic::new(seed);
                        let n = noise.noise2d(x as f32 * scale, z as f32 * scale);
                        cur + (n * 2.0 - 1.0) * delta
                    }
                };
                hm.set(x, z, new_val.clamp(0.0, 1.0));
            }
        }
    }

    /// Compute brush preview: returns a list of (x, z, intensity) samples.
    pub fn preview_samples(&self, cx: f32, cz: f32, sample_count: usize) -> Vec<(f32, f32, f32)> {
        let mut samples = Vec::new();
        let r = self.brush_radius;
        for i in 0..sample_count {
            let angle = i as f32 * std::f32::consts::TAU / sample_count as f32;
            for dist_step in 0..=4 {
                let dist = r * dist_step as f32 / 4.0;
                let x = cx + angle.cos() * dist;
                let z = cz + angle.sin() * dist;
                let intensity = self.falloff(dist / r.max(0.001));
                samples.push((x, z, intensity));
            }
        }
        samples
    }
}

// ── Terrain Heightmap Builder ─────────────────────────────────────────────────

/// A builder for compositing multiple terrain generation steps.
pub struct TerrainHeightmapBuilder {
    width:    usize,
    height:   usize,
    steps:    Vec<BuildStep>,
}

enum BuildStep {
    Diamond   { roughness: f32, seed: u64 },
    Fractal   { octaves: usize, lacunarity: f32, persistence: f32, scale: f32, seed: u64 },
    Voronoi   { num_plates: usize, seed: u64 },
    Perlin    { octaves: usize, scale: f32, seed: u64 },
    Erode     { kind: ErosionKind, iterations: usize },
    Terrace   { levels: usize },
    IslandMask{ falloff: f32 },
    Normalize,
    Blur      { radius: usize },
    Sharpen   { amount: f32 },
}

enum ErosionKind {
    Hydraulic { rain: f32, capacity: f32, evaporation: f32, seed: u64 },
    Thermal   { talus: f32 },
    Wind      { dir: glam::Vec2 },
}

impl TerrainHeightmapBuilder {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, steps: Vec::new() }
    }

    pub fn diamond_square(mut self, roughness: f32, seed: u64) -> Self {
        self.steps.push(BuildStep::Diamond { roughness, seed });
        self
    }

    pub fn fractal_noise(mut self, octaves: usize, lacunarity: f32, persistence: f32, scale: f32, seed: u64) -> Self {
        self.steps.push(BuildStep::Fractal { octaves, lacunarity, persistence, scale, seed });
        self
    }

    pub fn voronoi_plates(mut self, num_plates: usize, seed: u64) -> Self {
        self.steps.push(BuildStep::Voronoi { num_plates, seed });
        self
    }

    pub fn perlin(mut self, octaves: usize, scale: f32, seed: u64) -> Self {
        self.steps.push(BuildStep::Perlin { octaves, scale, seed });
        self
    }

    pub fn hydraulic_erosion(mut self, iterations: usize, rain: f32, capacity: f32, evap: f32, seed: u64) -> Self {
        self.steps.push(BuildStep::Erode { kind: ErosionKind::Hydraulic { rain, capacity, evaporation: evap, seed }, iterations });
        self
    }

    pub fn thermal_erosion(mut self, iterations: usize, talus: f32) -> Self {
        self.steps.push(BuildStep::Erode { kind: ErosionKind::Thermal { talus }, iterations });
        self
    }

    pub fn wind_erosion(mut self, iterations: usize, dir: glam::Vec2) -> Self {
        self.steps.push(BuildStep::Erode { kind: ErosionKind::Wind { dir }, iterations });
        self
    }

    pub fn terrace(mut self, levels: usize) -> Self {
        self.steps.push(BuildStep::Terrace { levels });
        self
    }

    pub fn island_mask(mut self, falloff: f32) -> Self {
        self.steps.push(BuildStep::IslandMask { falloff });
        self
    }

    pub fn normalize(mut self) -> Self {
        self.steps.push(BuildStep::Normalize);
        self
    }

    pub fn blur(mut self, radius: usize) -> Self {
        self.steps.push(BuildStep::Blur { radius });
        self
    }

    pub fn sharpen(mut self, amount: f32) -> Self {
        self.steps.push(BuildStep::Sharpen { amount });
        self
    }

    /// Execute the build pipeline and return the resulting heightmap.
    pub fn build(self) -> HeightMap {
        let mut hm: Option<HeightMap> = None;
        let w = self.width;
        let h = self.height;

        for step in self.steps {
            match step {
                BuildStep::Diamond { roughness, seed } => {
                    let size = w.max(h).next_power_of_two();
                    let generated = DiamondSquare::generate(size, roughness, seed);
                    let resampled = generated.resample(w, h);
                    hm = Some(Self::merge(hm, resampled));
                }
                BuildStep::Fractal { octaves, lacunarity, persistence, scale, seed } => {
                    let generated = FractalNoise::generate(w, h, octaves, lacunarity, persistence, scale, seed);
                    hm = Some(Self::merge(hm, generated));
                }
                BuildStep::Voronoi { num_plates, seed } => {
                    let generated = VoronoiPlates::generate(w, h, num_plates, seed);
                    hm = Some(Self::merge(hm, generated));
                }
                BuildStep::Perlin { octaves, scale, seed } => {
                    let generated = PerlinTerrain::generate(w, h, octaves, scale, seed);
                    hm = Some(Self::merge(hm, generated));
                }
                BuildStep::Erode { kind, iterations } => {
                    if let Some(ref mut m) = hm {
                        match kind {
                            ErosionKind::Hydraulic { rain, capacity, evaporation, seed } => {
                                HydraulicErosion::erode(m, iterations, rain, capacity, evaporation, seed);
                            }
                            ErosionKind::Thermal { talus } => {
                                ThermalErosion::erode(m, iterations, talus);
                            }
                            ErosionKind::Wind { dir } => {
                                WindErosion::erode(m, dir, iterations);
                            }
                        }
                    }
                }
                BuildStep::Terrace { levels } => {
                    if let Some(ref mut m) = hm { m.terrace(levels); }
                }
                BuildStep::IslandMask { falloff } => {
                    if let Some(ref mut m) = hm { m.island_mask(falloff); }
                }
                BuildStep::Normalize => {
                    if let Some(ref mut m) = hm { m.normalize(); }
                }
                BuildStep::Blur { radius } => {
                    if let Some(ref mut m) = hm { m.blur(radius); }
                }
                BuildStep::Sharpen { amount } => {
                    if let Some(ref mut m) = hm { m.sharpen(amount); }
                }
            }
        }

        hm.unwrap_or_else(|| HeightMap::new(w, h))
    }

    /// Merge two heightmaps (average if both exist, use new if only new exists).
    fn merge(existing: Option<HeightMap>, new: HeightMap) -> HeightMap {
        match existing {
            None => new,
            Some(mut e) => {
                for (a, &b) in e.data.iter_mut().zip(new.data.iter()) {
                    *a = (*a + b) * 0.5;
                }
                e
            }
        }
    }
}

// (HeightMap::resample is defined in heightmap.rs)

// ── Terrain LOD System ────────────────────────────────────────────────────────

/// Parameters for heightmap LOD (level-of-detail) scaling.
#[derive(Clone, Debug)]
pub struct TerrainLodParams {
    /// Number of LOD levels.
    pub num_levels: usize,
    /// Distance threshold per level (in world units).
    pub thresholds: Vec<f32>,
    /// Resolution divisor per level (1 = full, 2 = half, 4 = quarter, …).
    pub divisors:   Vec<usize>,
}

impl TerrainLodParams {
    pub fn new(num_levels: usize, base_threshold: f32, chunk_size: f32) -> Self {
        let thresholds: Vec<f32> = (0..num_levels)
            .map(|l| base_threshold * (1 << l) as f32)
            .collect();
        let divisors: Vec<usize> = (0..num_levels)
            .map(|l| 1 << l)
            .collect();
        Self { num_levels, thresholds, divisors }
    }

    /// Determine LOD level for a given distance.
    pub fn lod_for_distance(&self, dist: f32) -> usize {
        for (l, &thresh) in self.thresholds.iter().enumerate() {
            if dist < thresh { return l; }
        }
        self.num_levels - 1
    }

    /// Resolution for a given LOD level and base chunk size.
    pub fn resolution(&self, lod: usize, base_size: usize) -> usize {
        let div = self.divisors.get(lod).copied().unwrap_or(1 << (self.num_levels - 1));
        (base_size / div).max(4)
    }
}

// ── Terrain Metadata ──────────────────────────────────────────────────────────

/// Metadata about a generated terrain world.
#[derive(Clone, Debug)]
pub struct TerrainMetadata {
    pub seed:            u64,
    pub world_name:      String,
    pub generation_time: f32,
    pub total_chunks:    usize,
    pub sea_level:       f32,
    pub max_height:      f32,
    pub land_fraction:   f32,
    pub biome_counts:    [usize; 20],
}

impl TerrainMetadata {
    pub fn new(seed: u64, world_name: &str) -> Self {
        Self {
            seed,
            world_name: world_name.to_string(),
            generation_time: 0.0,
            total_chunks: 0,
            sea_level: 0.1,
            max_height: 100.0,
            land_fraction: 0.0,
            biome_counts: [0usize; 20],
        }
    }

    /// Compute land fraction from a world overview heightmap.
    pub fn compute_land_fraction(hm: &HeightMap, sea_level: f32) -> f32 {
        let land = hm.data.iter().filter(|&&v| v > sea_level).count();
        land as f32 / hm.data.len() as f32
    }
}

// ── Terrain Raycast System ────────────────────────────────────────────────────

/// System for batched terrain raycasts.
pub struct TerrainRaycastSystem;

impl TerrainRaycastSystem {
    /// Cast multiple rays against a heightmap. Returns vec of (hit_dist, hit_pos) or None per ray.
    pub fn batch_raycast(
        hm:     &HeightMap,
        chunk_size: f32,
        height_scale: f32,
        rays:   &[(Vec3, Vec3)],  // (origin, direction) pairs
        max_dist: f32,
    ) -> Vec<Option<(f32, Vec3)>> {
        let collider = TerrainCollider::new(hm, chunk_size, height_scale);
        rays.iter().map(|&(origin, dir)| {
            collider.ray_cast(origin, dir, max_dist)
                .map(|d| (d, origin + dir.normalize() * d))
        }).collect()
    }

    /// Find the first ray that hits terrain. Returns index and hit info.
    pub fn first_hit(
        hm:     &HeightMap,
        chunk_size: f32,
        height_scale: f32,
        rays:   &[(Vec3, Vec3)],
        max_dist: f32,
    ) -> Option<(usize, f32, Vec3)> {
        let collider = TerrainCollider::new(hm, chunk_size, height_scale);
        for (i, &(origin, dir)) in rays.iter().enumerate() {
            if let Some(d) = collider.ray_cast(origin, dir, max_dist) {
                return Some((i, d, origin + dir.normalize() * d));
            }
        }
        None
    }
}

// ── Terrain Water ─────────────────────────────────────────────────────────────

/// Represents bodies of water on the terrain.
#[derive(Clone, Debug)]
pub struct TerrainWater {
    pub sea_level:    f32,   // normalized height of sea level
    pub river_width:  f32,
    /// Precomputed water mask (1 = water, 0 = land).
    pub water_mask:   HeightMap,
}

impl TerrainWater {
    pub fn new(heightmap: &HeightMap, sea_level: f32) -> Self {
        let mut water_mask = HeightMap::new(heightmap.width, heightmap.height);
        for (i, &h) in heightmap.data.iter().enumerate() {
            water_mask.data[i] = if h <= sea_level { 1.0 } else { 0.0 };
        }
        Self { sea_level, river_width: 2.0, water_mask }
    }

    /// Is a world position underwater?
    pub fn is_underwater(&self, x: f32, z: f32) -> bool {
        let lx = x.clamp(0.0, (self.water_mask.width  - 1) as f32);
        let lz = z.clamp(0.0, (self.water_mask.height - 1) as f32);
        self.water_mask.sample_bilinear(lx, lz) > 0.5
    }

    /// Fraction of the terrain covered by water.
    pub fn water_coverage(&self) -> f32 {
        self.water_mask.data.iter().filter(|&&v| v > 0.5).count() as f32
            / self.water_mask.data.len() as f32
    }

    /// Depth below sea level at a given normalized height.
    pub fn depth(&self, height: f32) -> f32 {
        (self.sea_level - height).max(0.0)
    }
}

// ── Extended mod.rs Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod extended_mod_tests {
    use super::*;

    #[test]
    fn test_terrain_painter_raise() {
        let mut hm = HeightMap::new(64, 64);
        let painter = TerrainPainter {
            brush_radius: 10.0,
            brush_strength: 0.5,
            brush_falloff: BrushFalloff::Smooth,
            mode: PaintMode::Raise,
        };
        painter.apply(&mut hm, 32.0, 32.0);
        assert!(hm.get(32, 32) > 0.0);
        assert_eq!(hm.get(0, 0), 0.0);
    }

    #[test]
    fn test_terrain_painter_lower() {
        let mut hm = HeightMap::new(64, 64);
        for v in hm.data.iter_mut() { *v = 0.5; }
        let painter = TerrainPainter {
            brush_radius: 10.0,
            brush_strength: 0.3,
            brush_falloff: BrushFalloff::Linear,
            mode: PaintMode::Lower,
        };
        painter.apply(&mut hm, 32.0, 32.0);
        assert!(hm.get(32, 32) < 0.5);
    }

    #[test]
    fn test_terrain_painter_flatten() {
        let mut hm = HeightMap::new(64, 64);
        for v in hm.data.iter_mut() { *v = 0.8; }
        let painter = TerrainPainter {
            brush_radius: 20.0,
            brush_strength: 1.0,
            brush_falloff: BrushFalloff::Constant,
            mode: PaintMode::Flatten { target: 0.4 },
        };
        painter.apply(&mut hm, 32.0, 32.0);
        // Center should be close to target
        assert!((hm.get(32, 32) - 0.4).abs() < 0.05);
    }

    #[test]
    fn test_terrain_heightmap_builder() {
        let hm = TerrainHeightmapBuilder::new(32, 32)
            .fractal_noise(4, 2.0, 0.5, 3.0, 42)
            .normalize()
            .blur(1)
            .build();
        assert_eq!(hm.width, 32);
        assert_eq!(hm.height, 32);
        let mn = hm.min_value();
        let mx = hm.max_value();
        assert!(mn >= 0.0 && mx <= 1.0);
    }

    #[test]
    fn test_terrain_heightmap_builder_multi_step() {
        let hm = TerrainHeightmapBuilder::new(32, 32)
            .fractal_noise(4, 2.0, 0.5, 3.0, 42)
            .island_mask(2.0)
            .normalize()
            .terrace(4)
            .build();
        assert_eq!(hm.data.len(), 32 * 32);
    }

    #[test]
    fn test_terrain_lod_params() {
        let lod = TerrainLodParams::new(4, 50.0, 64.0);
        assert_eq!(lod.lod_for_distance(10.0),   0);
        assert_eq!(lod.lod_for_distance(60.0),   1);
        assert_eq!(lod.lod_for_distance(120.0),  2);
        assert_eq!(lod.resolution(0, 64),        64);
        assert_eq!(lod.resolution(1, 64),        32);
        assert_eq!(lod.resolution(2, 64),        16);
    }

    #[test]
    fn test_terrain_water() {
        let hm = heightmap::FractalNoise::generate(32, 32, 4, 2.0, 0.5, 3.0, 42);
        let water = TerrainWater::new(&hm, 0.15);
        let coverage = water.water_coverage();
        assert!(coverage >= 0.0 && coverage <= 1.0);
    }

    #[test]
    fn test_terrain_metadata() {
        let hm = heightmap::FractalNoise::generate(32, 32, 4, 2.0, 0.5, 3.0, 42);
        let frac = TerrainMetadata::compute_land_fraction(&hm, 0.15);
        assert!(frac >= 0.0 && frac <= 1.0);
    }

    #[test]
    fn test_terrain_raycast_system_batch() {
        let mut hm = HeightMap::new(64, 64);
        for v in hm.data.iter_mut() { *v = 0.5; }
        let rays = vec![
            (Vec3::new(32.0, 200.0, 32.0), Vec3::new(0.0, -1.0, 0.0)),
            (Vec3::new(10.0, 200.0, 10.0), Vec3::new(0.0, -1.0, 0.0)),
        ];
        let results = TerrainRaycastSystem::batch_raycast(&hm, 64.0, 100.0, &rays, 300.0);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_some());
    }

    #[test]
    fn test_smooth_step() {
        assert!((smooth_step(0.0, 1.0, 0.0) - 0.0).abs() < 1e-5);
        assert!((smooth_step(0.0, 1.0, 1.0) - 1.0).abs() < 1e-5);
        assert!((smooth_step(0.0, 1.0, 0.5) - 0.5).abs() < 1e-5);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Terrain event system
// ─────────────────────────────────────────────────────────────────────────────

/// Discrete events that the terrain system can emit during simulation.
#[derive(Debug, Clone)]
pub enum TerrainEvent {
    ChunkLoaded { coord: mod_types::ChunkCoord },
    ChunkUnloaded { coord: mod_types::ChunkCoord },
    HeightmapModified { coord: mod_types::ChunkCoord, affected_cells: u32 },
    BiomeTransitionDetected { from: biome::BiomeType, to: biome::BiomeType },
    ErosionCycleCompleted { chunk: mod_types::ChunkCoord, delta_energy: f32 },
    WaterLevelChanged { old: f32, new: f32 },
    LodChanged { coord: mod_types::ChunkCoord, old_lod: u8, new_lod: u8 },
}

/// Simple single-producer/single-consumer event queue for terrain events.
#[derive(Debug, Default)]
pub struct TerrainEventQueue {
    events: std::collections::VecDeque<TerrainEvent>,
    pub max_capacity: usize,
}

impl TerrainEventQueue {
    pub fn new(capacity: usize) -> Self {
        Self { events: std::collections::VecDeque::new(), max_capacity: capacity }
    }

    pub fn push(&mut self, event: TerrainEvent) {
        if self.events.len() >= self.max_capacity {
            self.events.pop_front();  // drop oldest
        }
        self.events.push_back(event);
    }

    pub fn pop(&mut self) -> Option<TerrainEvent> {
        self.events.pop_front()
    }

    pub fn drain_all(&mut self) -> Vec<TerrainEvent> {
        self.events.drain(..).collect()
    }

    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Terrain snapshot / diff
// ─────────────────────────────────────────────────────────────────────────────

/// Immutable snapshot of a heightmap for undo/redo support.
#[derive(Debug, Clone)]
pub struct TerrainSnapshot {
    pub width: usize,
    pub height: usize,
    data: Vec<f32>,
    pub timestamp: u64,
}

impl TerrainSnapshot {
    pub fn capture(hm: &heightmap::HeightMap, timestamp: u64) -> Self {
        Self { width: hm.width, height: hm.height, data: hm.data.clone(), timestamp }
    }

    pub fn restore_to(&self, hm: &mut heightmap::HeightMap) {
        if hm.width == self.width && hm.height == self.height {
            hm.data.copy_from_slice(&self.data);
        }
    }

    pub fn byte_size(&self) -> usize {
        self.data.len() * 4
    }
}

/// Stores the per-cell difference between two snapshots for compact undo.
#[derive(Debug, Clone)]
pub struct TerrainDiff {
    pub width: usize,
    pub height: usize,
    /// (cell_index, old_value, new_value)
    pub changes: Vec<(u32, f32, f32)>,
}

impl TerrainDiff {
    pub fn compute(before: &TerrainSnapshot, after: &TerrainSnapshot) -> Self {
        assert_eq!(before.data.len(), after.data.len());
        let changes = before.data.iter().zip(after.data.iter()).enumerate()
            .filter_map(|(i, (&old, &new))| {
                if (old - new).abs() > 1e-7 { Some((i as u32, old, new)) } else { None }
            })
            .collect();
        Self { width: before.width, height: before.height, changes }
    }

    pub fn apply(&self, hm: &mut heightmap::HeightMap) {
        for &(idx, _old, new) in &self.changes {
            hm.data[idx as usize] = new;
        }
    }

    pub fn revert(&self, hm: &mut heightmap::HeightMap) {
        for &(idx, old, _new) in &self.changes {
            hm.data[idx as usize] = old;
        }
    }

    pub fn changed_cell_count(&self) -> usize {
        self.changes.len()
    }
}

/// Stack-based undo/redo manager for terrain edits.
pub struct TerrainUndoStack {
    undo: Vec<TerrainDiff>,
    redo: Vec<TerrainDiff>,
    pub max_depth: usize,
}

impl TerrainUndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), max_depth }
    }

    pub fn push(&mut self, diff: TerrainDiff) {
        if self.undo.len() >= self.max_depth {
            self.undo.remove(0);
        }
        self.undo.push(diff);
        self.redo.clear();
    }

    pub fn undo(&mut self, hm: &mut heightmap::HeightMap) -> bool {
        if let Some(diff) = self.undo.pop() {
            diff.revert(hm);
            self.redo.push(diff);
            true
        } else { false }
    }

    pub fn redo(&mut self, hm: &mut heightmap::HeightMap) -> bool {
        if let Some(diff) = self.redo.pop() {
            diff.apply(hm);
            self.undo.push(diff);
            true
        } else { false }
    }

    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Terrain statistics aggregator
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime statistics for a full terrain world.
#[derive(Debug, Default, Clone)]
pub struct TerrainWorldStats {
    pub total_chunks: u32,
    pub loaded_chunks: u32,
    pub visible_chunks: u32,
    pub total_triangles: u64,
    pub vegetation_instances: u64,
    pub memory_bytes: u64,
    pub last_update_ms: f64,
}

impl TerrainWorldStats {
    pub fn memory_mb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn load_ratio(&self) -> f32 {
        if self.total_chunks == 0 { 0.0 } else {
            self.loaded_chunks as f32 / self.total_chunks as f32
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "Chunks: {}/{} loaded ({} visible), Tris: {}, Veg: {}, Mem: {:.1} MB",
            self.loaded_chunks, self.total_chunks, self.visible_chunks,
            self.total_triangles, self.vegetation_instances,
            self.memory_mb(),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Terrain config presets
// ─────────────────────────────────────────────────────────────────────────────

/// Named presets for common terrain configurations.
pub struct TerrainPresets;

impl TerrainPresets {
    pub fn flat_plains() -> mod_types::TerrainConfig {
        mod_types::TerrainConfig {
            chunk_size: 64,
            lod_levels: 3,
            view_distance: 8,
            seed: 1,
        }
    }

    pub fn mountainous() -> mod_types::TerrainConfig {
        mod_types::TerrainConfig {
            chunk_size: 128,
            lod_levels: 5,
            view_distance: 12,
            seed: 42,
        }
    }

    pub fn ocean_archipelago() -> mod_types::TerrainConfig {
        mod_types::TerrainConfig {
            chunk_size: 128,
            lod_levels: 4,
            view_distance: 16,
            seed: 777,
        }
    }

    pub fn desert_dunes() -> mod_types::TerrainConfig {
        mod_types::TerrainConfig {
            chunk_size: 64,
            lod_levels: 3,
            view_distance: 10,
            seed: 314,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Additional tests for new types
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod extended_terrain_tests {
    use super::*;

    #[test]
    fn test_terrain_event_queue_capacity() {
        let mut q = TerrainEventQueue::new(3);
        for i in 0..5 {
            q.push(TerrainEvent::WaterLevelChanged { old: i as f32, new: i as f32 + 1.0 });
        }
        // Should hold at most 3
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn test_terrain_event_queue_drain() {
        let mut q = TerrainEventQueue::new(10);
        q.push(TerrainEvent::WaterLevelChanged { old: 0.0, new: 1.0 });
        q.push(TerrainEvent::WaterLevelChanged { old: 1.0, new: 2.0 });
        let events = q.drain_all();
        assert_eq!(events.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn test_terrain_snapshot_restore() {
        let mut hm = HeightMap::new(16, 16);
        for v in hm.data.iter_mut() { *v = 0.7; }
        let snap = TerrainSnapshot::capture(&hm, 1000);
        for v in hm.data.iter_mut() { *v = 0.2; }
        snap.restore_to(&mut hm);
        assert!((hm.data[0] - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_terrain_diff_apply_revert() {
        let mut hm = HeightMap::new(16, 16);
        for v in hm.data.iter_mut() { *v = 0.5; }
        let before = TerrainSnapshot::capture(&hm, 0);
        hm.data[10] = 0.9;
        let after = TerrainSnapshot::capture(&hm, 1);
        let diff = TerrainDiff::compute(&before, &after);
        assert_eq!(diff.changed_cell_count(), 1);
        diff.revert(&mut hm);
        assert!((hm.data[10] - 0.5).abs() < 1e-5);
        diff.apply(&mut hm);
        assert!((hm.data[10] - 0.9).abs() < 1e-5);
    }

    #[test]
    fn test_undo_stack() {
        let mut hm = HeightMap::new(16, 16);
        for v in hm.data.iter_mut() { *v = 0.5; }
        let before = TerrainSnapshot::capture(&hm, 0);
        hm.data[5] = 0.8;
        let after = TerrainSnapshot::capture(&hm, 1);
        let diff = TerrainDiff::compute(&before, &after);
        let mut stack = TerrainUndoStack::new(10);
        stack.push(diff);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert!(stack.undo(&mut hm));
        assert!((hm.data[5] - 0.5).abs() < 1e-5);
        assert!(stack.can_redo());
        assert!(stack.redo(&mut hm));
        assert!((hm.data[5] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_terrain_world_stats() {
        let stats = TerrainWorldStats {
            total_chunks: 100,
            loaded_chunks: 40,
            visible_chunks: 25,
            total_triangles: 500_000,
            vegetation_instances: 12_000,
            memory_bytes: 64 * 1024 * 1024,
            last_update_ms: 16.7,
        };
        assert!((stats.memory_mb() - 64.0).abs() < 0.01);
        assert!((stats.load_ratio() - 0.4).abs() < 1e-4);
        let desc = stats.describe();
        assert!(desc.contains("40/100"));
    }

    #[test]
    fn test_terrain_presets() {
        let plains = TerrainPresets::flat_plains();
        assert_eq!(plains.chunk_size, 64);
        let mountains = TerrainPresets::mountainous();
        assert!(mountains.lod_levels >= 5);
        let desert = TerrainPresets::desert_dunes();
        assert_eq!(desert.seed, 314);
    }
}
