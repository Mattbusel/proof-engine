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
