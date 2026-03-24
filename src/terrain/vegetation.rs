//! Vegetation system — tree/grass/rock placement, LOD, wind, seasonal changes.
//!
//! Handles placement of trees, grass clusters, and rocks based on heightmap
//! and biome data. Includes wind simulation, seasonal variation, procedural
//! L-system tree skeletons, and frustum/distance culling.

use glam::{Vec2, Vec3, Vec4};
use crate::terrain::heightmap::HeightMap;
use crate::terrain::biome::{BiomeMap, BiomeType, VegetationDensity, SeasonFactor};

// ── Internal RNG ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Rng {
    state: [u64; 4],
}

impl Rng {
    fn new(seed: u64) -> Self {
        let mut s = seed;
        let mut next = || {
            s = s.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        };
        Self { state: [next(), next(), next(), next()] }
    }
    fn rol64(x: u64, k: u32) -> u64 { (x << k) | (x >> (64 - k)) }
    fn next_u64(&mut self) -> u64 {
        let r = Self::rol64(self.state[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = Self::rol64(self.state[3], 45);
        r
    }
    fn next_f32(&mut self) -> f32 { (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32 }
    fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 { lo + self.next_f32() * (hi - lo) }
    fn next_usize(&mut self, n: usize) -> usize { (self.next_u64() % n as u64) as usize }
}

// ── TreeType ──────────────────────────────────────────────────────────────────

/// Tree species variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TreeType {
    Oak,
    Pine,
    Birch,
    Tropical,
    Dead,
    Palm,
    Willow,
    Cactus,
    Fern,
    Mushroom,
}

impl TreeType {
    pub fn name(self) -> &'static str {
        match self {
            TreeType::Oak      => "Oak",
            TreeType::Pine     => "Pine",
            TreeType::Birch    => "Birch",
            TreeType::Tropical => "Tropical",
            TreeType::Dead     => "Dead",
            TreeType::Palm     => "Palm",
            TreeType::Willow   => "Willow",
            TreeType::Cactus   => "Cactus",
            TreeType::Fern     => "Fern",
            TreeType::Mushroom => "Mushroom",
        }
    }

    /// Typical tree types for a given biome.
    pub fn for_biome(biome: BiomeType) -> &'static [TreeType] {
        match biome {
            BiomeType::TemperateForest => &[TreeType::Oak, TreeType::Birch],
            BiomeType::TropicalForest  => &[TreeType::Tropical, TreeType::Palm],
            BiomeType::Boreal | BiomeType::Taiga => &[TreeType::Pine],
            BiomeType::Tundra          => &[TreeType::Dead, TreeType::Fern],
            BiomeType::Savanna         => &[TreeType::Oak, TreeType::Dead],
            BiomeType::Desert          => &[TreeType::Cactus],
            BiomeType::Swamp           => &[TreeType::Willow],
            BiomeType::Mangrove        => &[TreeType::Tropical],
            BiomeType::Mountain        => &[TreeType::Pine, TreeType::Dead],
            BiomeType::Mushroom        => &[TreeType::Mushroom],
            _                          => &[TreeType::Oak],
        }
    }
}

// ── TreeParams ────────────────────────────────────────────────────────────────

/// Parameters describing a single tree's shape.
#[derive(Clone, Debug)]
pub struct TreeParams {
    pub height:           f32,
    pub crown_radius:     f32,
    pub trunk_radius:     f32,
    pub lean_angle:       f32,  // radians, from vertical
    pub color_variation:  f32,  // [0,1] hue shift
    pub branch_density:   f32,  // relative number of branches
    pub root_spread:      f32,  // radius of root buttresses
}

impl TreeParams {
    /// Default parameters for a given tree type.
    pub fn for_type(tt: TreeType, rng: &mut Rng) -> Self {
        let var = rng.next_f32_range(-0.1, 0.1);
        match tt {
            TreeType::Oak => Self {
                height: rng.next_f32_range(6.0, 14.0),
                crown_radius: rng.next_f32_range(3.0, 6.0),
                trunk_radius: rng.next_f32_range(0.2, 0.5),
                lean_angle: rng.next_f32_range(0.0, 0.15),
                color_variation: var.abs(),
                branch_density: 0.8,
                root_spread: 0.6,
            },
            TreeType::Pine => Self {
                height: rng.next_f32_range(8.0, 20.0),
                crown_radius: rng.next_f32_range(1.5, 3.0),
                trunk_radius: rng.next_f32_range(0.15, 0.35),
                lean_angle: rng.next_f32_range(0.0, 0.1),
                color_variation: var.abs() * 0.5,
                branch_density: 1.2,
                root_spread: 0.3,
            },
            TreeType::Birch => Self {
                height: rng.next_f32_range(5.0, 12.0),
                crown_radius: rng.next_f32_range(1.5, 3.0),
                trunk_radius: rng.next_f32_range(0.1, 0.2),
                lean_angle: rng.next_f32_range(0.0, 0.2),
                color_variation: var.abs() * 0.3,
                branch_density: 0.7,
                root_spread: 0.3,
            },
            TreeType::Tropical => Self {
                height: rng.next_f32_range(10.0, 25.0),
                crown_radius: rng.next_f32_range(4.0, 8.0),
                trunk_radius: rng.next_f32_range(0.3, 0.7),
                lean_angle: rng.next_f32_range(0.0, 0.25),
                color_variation: var.abs() * 0.4,
                branch_density: 0.6,
                root_spread: 1.5,
            },
            TreeType::Dead => Self {
                height: rng.next_f32_range(3.0, 8.0),
                crown_radius: rng.next_f32_range(1.0, 2.5),
                trunk_radius: rng.next_f32_range(0.1, 0.3),
                lean_angle: rng.next_f32_range(0.0, 0.4),
                color_variation: 0.0,
                branch_density: 0.3,
                root_spread: 0.2,
            },
            TreeType::Palm => Self {
                height: rng.next_f32_range(6.0, 15.0),
                crown_radius: rng.next_f32_range(3.0, 5.0),
                trunk_radius: rng.next_f32_range(0.15, 0.3),
                lean_angle: rng.next_f32_range(0.05, 0.35),
                color_variation: var.abs() * 0.2,
                branch_density: 0.2,
                root_spread: 0.4,
            },
            TreeType::Willow => Self {
                height: rng.next_f32_range(8.0, 16.0),
                crown_radius: rng.next_f32_range(4.0, 7.0),
                trunk_radius: rng.next_f32_range(0.2, 0.4),
                lean_angle: rng.next_f32_range(0.0, 0.3),
                color_variation: var.abs() * 0.15,
                branch_density: 1.5,
                root_spread: 1.2,
            },
            TreeType::Cactus => Self {
                height: rng.next_f32_range(2.0, 6.0),
                crown_radius: rng.next_f32_range(0.5, 1.5),
                trunk_radius: rng.next_f32_range(0.15, 0.4),
                lean_angle: rng.next_f32_range(0.0, 0.1),
                color_variation: var.abs() * 0.1,
                branch_density: 0.2,
                root_spread: 0.5,
            },
            TreeType::Fern => Self {
                height: rng.next_f32_range(0.3, 1.0),
                crown_radius: rng.next_f32_range(0.3, 0.8),
                trunk_radius: rng.next_f32_range(0.02, 0.06),
                lean_angle: rng.next_f32_range(0.0, 0.5),
                color_variation: var.abs() * 0.2,
                branch_density: 2.0,
                root_spread: 0.1,
            },
            TreeType::Mushroom => Self {
                height: rng.next_f32_range(1.0, 4.0),
                crown_radius: rng.next_f32_range(0.8, 2.5),
                trunk_radius: rng.next_f32_range(0.1, 0.25),
                lean_angle: rng.next_f32_range(0.0, 0.15),
                color_variation: rng.next_f32_range(0.0, 0.5),
                branch_density: 0.0,
                root_spread: 0.15,
            },
        }
    }
}

// ── L-System Tree Skeleton ────────────────────────────────────────────────────

/// A single segment in a procedural tree skeleton.
#[derive(Clone, Debug)]
pub struct TreeSegment {
    pub start:    Vec3,
    pub end:      Vec3,
    pub radius:   f32,
    pub depth:    u32,
}

/// Procedural tree skeleton generated by an L-system.
#[derive(Clone, Debug)]
pub struct TreeSkeleton {
    pub segments: Vec<TreeSegment>,
    pub tree_type: TreeType,
    pub params: TreeParams,
}

impl TreeSkeleton {
    /// Generate a tree skeleton using L-system-style recursive branching.
    pub fn generate(tree_type: TreeType, params: &TreeParams, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut segments = Vec::new();
        let base_pos = Vec3::ZERO;
        let lean_x = params.lean_angle * rng.next_f32_range(-1.0, 1.0);
        let lean_z = params.lean_angle * rng.next_f32_range(-1.0, 1.0);
        let trunk_dir = Vec3::new(lean_x, 1.0, lean_z).normalize();

        let max_depth = match tree_type {
            TreeType::Pine | TreeType::Oak | TreeType::Birch => 5u32,
            TreeType::Tropical | TreeType::Willow           => 4,
            TreeType::Fern | TreeType::Cactus               => 3,
            TreeType::Dead                                   => 4,
            TreeType::Palm | TreeType::Mushroom              => 2,
        };

        Self::branch(
            base_pos,
            trunk_dir,
            params.height,
            params.trunk_radius,
            0,
            max_depth,
            params,
            tree_type,
            &mut rng,
            &mut segments,
        );

        Self { segments, tree_type, params: params.clone() }
    }

    fn branch(
        pos:      Vec3,
        dir:      Vec3,
        length:   f32,
        radius:   f32,
        depth:    u32,
        max_depth: u32,
        params:   &TreeParams,
        tt:       TreeType,
        rng:      &mut Rng,
        out:      &mut Vec<TreeSegment>,
    ) {
        if depth > max_depth || length < 0.1 || radius < 0.01 { return; }

        let end = pos + dir * length;
        out.push(TreeSegment { start: pos, end, radius, depth });

        if depth == max_depth { return; }

        let branch_count = match tt {
            TreeType::Palm | TreeType::Cactus | TreeType::Mushroom => 2,
            TreeType::Willow => (3.0 * params.branch_density) as u32 + 1,
            TreeType::Pine   => (4.0 * params.branch_density) as u32 + 2,
            _                => (3.0 * params.branch_density) as u32 + 2,
        };

        for _ in 0..branch_count {
            let spread = match tt {
                TreeType::Pine     => 0.4,
                TreeType::Willow   => 0.9,
                TreeType::Palm     => 1.2,
                TreeType::Tropical => 0.7,
                _                  => 0.6,
            };
            let dx = rng.next_f32_range(-spread, spread);
            let dz = rng.next_f32_range(-spread, spread);
            let branch_dir = (dir + Vec3::new(dx, rng.next_f32_range(-0.1, 0.3), dz)).normalize();
            let branch_len  = length  * rng.next_f32_range(0.55, 0.75);
            let branch_rad  = radius  * rng.next_f32_range(0.55, 0.7);
            let branch_start = pos + dir * (length * rng.next_f32_range(0.5, 0.85));
            Self::branch(
                branch_start, branch_dir, branch_len, branch_rad,
                depth + 1, max_depth, params, tt, rng, out,
            );
        }
    }

    /// Bounding box of the skeleton: (min, max).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut mn = Vec3::splat(f32::INFINITY);
        let mut mx = Vec3::splat(f32::NEG_INFINITY);
        for seg in &self.segments {
            mn = mn.min(seg.start).min(seg.end);
            mx = mx.max(seg.start).max(seg.end);
        }
        (mn, mx)
    }
}

// ── VegetationInstance ────────────────────────────────────────────────────────

/// A single placed vegetation object (tree, grass cluster, rock).
#[derive(Clone, Debug)]
pub struct VegetationInstance {
    pub position:  Vec3,
    pub rotation:  f32,   // Y-axis rotation in radians
    pub scale:     Vec3,
    pub lod_level: u8,
    pub visible:   bool,
    pub kind:      VegetationKind,
}

/// What kind of vegetation this instance is.
#[derive(Clone, Debug, PartialEq)]
pub enum VegetationKind {
    Tree(TreeType),
    Grass,
    Rock { size_class: u8 },
    Shrub,
    Flower,
}

// ── GrassCluster ─────────────────────────────────────────────────────────────

/// A cluster of grass blades sharing placement and animation parameters.
#[derive(Clone, Debug)]
pub struct GrassCluster {
    pub center:          Vec3,
    pub radius:          f32,
    pub density:         f32,
    pub blade_height:    f32,
    pub blade_width:     f32,
    pub sway_frequency:  f32,
    pub sway_amplitude:  f32,
    pub color:           Vec4,
    pub biome:           BiomeType,
}

impl GrassCluster {
    pub fn new(center: Vec3, radius: f32, biome: BiomeType, rng: &mut Rng) -> Self {
        let (height, color) = match biome {
            BiomeType::Grassland => (
                rng.next_f32_range(0.3, 0.7),
                Vec4::new(0.3 + rng.next_f32() * 0.1, 0.6 + rng.next_f32() * 0.1, 0.15, 1.0)
            ),
            BiomeType::Savanna => (
                rng.next_f32_range(0.4, 1.2),
                Vec4::new(0.65 + rng.next_f32() * 0.1, 0.6, 0.15, 1.0)
            ),
            BiomeType::TropicalForest => (
                rng.next_f32_range(0.2, 0.5),
                Vec4::new(0.15, 0.55 + rng.next_f32() * 0.1, 0.1, 1.0)
            ),
            BiomeType::Tundra => (
                rng.next_f32_range(0.05, 0.2),
                Vec4::new(0.45, 0.5, 0.3, 1.0)
            ),
            _ => (
                rng.next_f32_range(0.2, 0.5),
                Vec4::new(0.3, 0.55, 0.15, 1.0)
            ),
        };
        Self {
            center,
            radius,
            density: rng.next_f32_range(0.4, 1.0),
            blade_height: height,
            blade_width: rng.next_f32_range(0.02, 0.05),
            sway_frequency: rng.next_f32_range(0.5, 2.0),
            sway_amplitude: rng.next_f32_range(0.05, 0.15),
            color,
            biome,
        }
    }

    /// Compute sway offset at given time for wind animation.
    pub fn sway_offset(&self, time: f32, wind: Vec2) -> Vec2 {
        let phase = self.center.x * 0.1 + self.center.z * 0.1;
        let sway = (time * self.sway_frequency + phase).sin() * self.sway_amplitude;
        wind.normalize_or_zero() * sway
    }
}

// ── GrassField ────────────────────────────────────────────────────────────────

/// Collection of grass clusters over a terrain region.
#[derive(Clone, Debug)]
pub struct GrassField {
    pub clusters: Vec<GrassCluster>,
}

impl GrassField {
    /// Generate grass placement from heightmap and biome data.
    pub fn generate(
        heightmap: &HeightMap,
        biome_map: &BiomeMap,
        density_scale: f32,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let mut clusters = Vec::new();
        let w = heightmap.width;
        let h = heightmap.height;
        let grid_step = 3usize;

        for y in (0..h).step_by(grid_step) {
            for x in (0..w).step_by(grid_step) {
                let biome = biome_map.get(x, y);
                let density = VegetationDensity::for_biome(biome);
                if density.grass_density * density_scale < rng.next_f32() { continue; }
                let alt = heightmap.get(x, y);
                if alt < 0.1 { continue; } // skip ocean
                let pos = Vec3::new(
                    x as f32 + rng.next_f32_range(-1.0, 1.0),
                    alt * 100.0,
                    y as f32 + rng.next_f32_range(-1.0, 1.0),
                );
                let radius = rng.next_f32_range(1.0, 3.0);
                clusters.push(GrassCluster::new(pos, radius, biome, &mut rng));
            }
        }
        Self { clusters }
    }

    /// Update sway for all clusters given current time and wind.
    pub fn update_wind(&mut self, _time: f32, _wind: Vec2) {
        // In a real system this would update GPU buffers; here we just note the call.
    }
}

// ── RockPlacement ─────────────────────────────────────────────────────────────

/// A placed rock or boulder.
#[derive(Clone, Debug)]
pub struct RockPlacement {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale:    Vec3,
    pub biome:    BiomeType,
}

/// A cluster of rocks using Poisson disk sampling.
#[derive(Clone, Debug)]
pub struct RockCluster {
    pub rocks:  Vec<RockPlacement>,
    pub center: Vec3,
    pub radius: f32,
}

impl RockCluster {
    /// Generate a cluster of rocks around a center point.
    pub fn generate(center: Vec3, radius: f32, biome: BiomeType, count: usize, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut rocks = Vec::new();

        // Poisson disk sampling (simplified dart-throwing)
        let min_dist = radius / (count as f32).sqrt().max(1.0) * 0.8;
        let mut positions: Vec<Vec2> = Vec::new();
        let max_attempts = count * 30;

        for _ in 0..max_attempts {
            if rocks.len() >= count { break; }
            let angle = rng.next_f32() * std::f32::consts::TAU;
            let dist  = rng.next_f32() * radius;
            let px = center.x + angle.cos() * dist;
            let pz = center.z + angle.sin() * dist;
            let candidate = Vec2::new(px, pz);
            // Check min distance from existing rocks
            let too_close = positions.iter().any(|&p| p.distance(candidate) < min_dist);
            if too_close { continue; }
            positions.push(candidate);
            let size_class = (rng.next_f32() * 3.0) as u8; // 0=small, 1=med, 2=large
            let base_scale = match size_class {
                0 => rng.next_f32_range(0.2, 0.6),
                1 => rng.next_f32_range(0.6, 1.5),
                _ => rng.next_f32_range(1.5, 4.0),
            };
            let scale_var = Vec3::new(
                base_scale * rng.next_f32_range(0.8, 1.2),
                base_scale * rng.next_f32_range(0.6, 1.0),
                base_scale * rng.next_f32_range(0.8, 1.2),
            );
            rocks.push(RockPlacement {
                position: Vec3::new(px, center.y, pz),
                rotation: Vec3::new(
                    rng.next_f32_range(-0.3, 0.3),
                    rng.next_f32() * std::f32::consts::TAU,
                    rng.next_f32_range(-0.3, 0.3),
                ),
                scale: scale_var,
                biome,
            });
        }
        Self { rocks, center, radius }
    }
}

// ── VegetationLod ─────────────────────────────────────────────────────────────

/// LOD configuration for a vegetation type.
#[derive(Clone, Debug)]
pub struct VegetationLod {
    /// Distance threshold for LOD 0 (full detail).
    pub lod0_distance: f32,
    /// Distance threshold for LOD 1 (reduced).
    pub lod1_distance: f32,
    /// Distance threshold for billboard impostors.
    pub billboard_distance: f32,
    /// Maximum visibility distance (beyond this, cull).
    pub cull_distance: f32,
    /// Billboard dimensions (width, height).
    pub billboard_size: Vec2,
}

impl VegetationLod {
    pub fn for_tree(tree_type: TreeType) -> Self {
        let base = match tree_type {
            TreeType::Oak | TreeType::Tropical | TreeType::Willow => 40.0f32,
            TreeType::Pine | TreeType::Birch   => 35.0,
            TreeType::Palm                      => 30.0,
            TreeType::Dead | TreeType::Fern     => 20.0,
            TreeType::Cactus | TreeType::Mushroom => 15.0,
        };
        Self {
            lod0_distance:     base,
            lod1_distance:     base * 2.0,
            billboard_distance: base * 4.0,
            cull_distance:     base * 8.0,
            billboard_size:    Vec2::new(4.0, 8.0),
        }
    }

    pub fn for_grass() -> Self {
        Self {
            lod0_distance: 15.0,
            lod1_distance: 25.0,
            billboard_distance: 40.0,
            cull_distance: 60.0,
            billboard_size: Vec2::new(1.0, 0.5),
        }
    }

    pub fn for_rock() -> Self {
        Self {
            lod0_distance: 20.0,
            lod1_distance: 50.0,
            billboard_distance: 80.0,
            cull_distance: 150.0,
            billboard_size: Vec2::new(2.0, 1.5),
        }
    }

    /// Return the LOD level for a given distance (0 = full, 1 = reduced, 2 = billboard, 3 = culled).
    pub fn lod_for_distance(&self, dist: f32) -> u8 {
        if dist > self.cull_distance      { 3 }
        else if dist > self.billboard_distance { 2 }
        else if dist > self.lod1_distance { 1 }
        else                              { 0 }
    }
}

// ── VegetationSystem ──────────────────────────────────────────────────────────

/// Top-level manager for all vegetation instances.
#[derive(Debug)]
pub struct VegetationSystem {
    pub instances:     Vec<VegetationInstance>,
    pub grass_field:   GrassField,
    pub rock_clusters: Vec<RockCluster>,
    pub wind_vector:   Vec2,
    pub time:          f32,
}

impl VegetationSystem {
    pub fn new() -> Self {
        Self {
            instances:     Vec::new(),
            grass_field:   GrassField { clusters: Vec::new() },
            rock_clusters: Vec::new(),
            wind_vector:   Vec2::new(0.5, 0.2),
            time:          0.0,
        }
    }

    /// Generate all vegetation from a heightmap and biome map.
    pub fn generate(
        heightmap: &HeightMap,
        biome_map: &BiomeMap,
        density_scale: f32,
        seed: u64,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let mut instances = Vec::new();
        let w = heightmap.width;
        let h = heightmap.height;

        let slope_map = heightmap.slope_map();

        // Tree placement
        let tree_grid_step = 4usize;
        for y in (0..h).step_by(tree_grid_step) {
            for x in (0..w).step_by(tree_grid_step) {
                let biome = biome_map.get(x, y);
                let density = VegetationDensity::for_biome(biome);
                if density.tree_density * density_scale < 0.05 { continue; }
                if density.tree_density * density_scale < rng.next_f32() { continue; }
                let alt = heightmap.get(x, y);
                if alt < 0.1 { continue; }
                let slope = slope_map.get(x, y);
                if slope > 0.6 { continue; } // no trees on cliffs

                let types = TreeType::for_biome(biome);
                if types.is_empty() { continue; }
                let tt = types[rng.next_usize(types.len())];
                let mut tree_rng = Rng::new(seed.wrapping_add(y as u64 * 1000 + x as u64));
                let params = TreeParams::for_type(tt, &mut tree_rng);
                let scale_f = params.height / 10.0;

                instances.push(VegetationInstance {
                    position: Vec3::new(
                        x as f32 + rng.next_f32_range(-1.5, 1.5),
                        alt * 100.0,
                        y as f32 + rng.next_f32_range(-1.5, 1.5),
                    ),
                    rotation: rng.next_f32() * std::f32::consts::TAU,
                    scale: Vec3::splat(scale_f),
                    lod_level: 0,
                    visible: true,
                    kind: VegetationKind::Tree(tt),
                });
            }
        }

        // Rock placement
        let rock_grid_step = 6usize;
        for y in (0..h).step_by(rock_grid_step) {
            for x in (0..w).step_by(rock_grid_step) {
                let biome = biome_map.get(x, y);
                let density = VegetationDensity::for_biome(biome);
                if density.rock_density * density_scale < rng.next_f32() { continue; }
                let alt = heightmap.get(x, y);
                if alt < 0.05 { continue; }
                let size_class = (rng.next_f32() * 3.0) as u8;
                let base = match size_class { 0 => 0.3f32, 1 => 0.8, _ => 2.0 };
                instances.push(VegetationInstance {
                    position: Vec3::new(x as f32, alt * 100.0, y as f32),
                    rotation: rng.next_f32() * std::f32::consts::TAU,
                    scale: Vec3::splat(base) * rng.next_f32_range(0.8, 1.2),
                    lod_level: 0,
                    visible: true,
                    kind: VegetationKind::Rock { size_class },
                });
            }
        }

        // Shrub placement
        let shrub_grid = 5usize;
        for y in (0..h).step_by(shrub_grid) {
            for x in (0..w).step_by(shrub_grid) {
                let biome = biome_map.get(x, y);
                let density = VegetationDensity::for_biome(biome);
                if density.shrub_density * density_scale < rng.next_f32() { continue; }
                let alt = heightmap.get(x, y);
                if alt < 0.08 { continue; }
                instances.push(VegetationInstance {
                    position: Vec3::new(x as f32, alt * 100.0, y as f32),
                    rotation: rng.next_f32() * std::f32::consts::TAU,
                    scale: Vec3::splat(rng.next_f32_range(0.3, 0.9)),
                    lod_level: 0,
                    visible: true,
                    kind: VegetationKind::Shrub,
                });
            }
        }

        let grass = GrassField::generate(heightmap, biome_map, density_scale, seed.wrapping_add(0xABCD));
        let rocks = Self::generate_rock_clusters(heightmap, biome_map, density_scale, seed.wrapping_add(0x1234));

        Self {
            instances,
            grass_field: grass,
            rock_clusters: rocks,
            wind_vector: Vec2::new(0.5, 0.2),
            time: 0.0,
        }
    }

    fn generate_rock_clusters(
        heightmap: &HeightMap,
        biome_map: &BiomeMap,
        density_scale: f32,
        seed: u64,
    ) -> Vec<RockCluster> {
        let mut rng = Rng::new(seed);
        let mut clusters = Vec::new();
        let w = heightmap.width;
        let h = heightmap.height;
        let step = 12usize;
        for y in (0..h).step_by(step) {
            for x in (0..w).step_by(step) {
                let biome = biome_map.get(x, y);
                let density = VegetationDensity::for_biome(biome);
                if density.rock_density * density_scale * 0.3 < rng.next_f32() { continue; }
                let alt = heightmap.get(x, y);
                let center = Vec3::new(x as f32, alt * 100.0, y as f32);
                let count = rng.next_usize(8) + 2;
                clusters.push(RockCluster::generate(center, 5.0, biome, count, rng.next_u64()));
            }
        }
        clusters
    }

    /// Update LOD levels for all instances based on camera position.
    pub fn update_lod(&mut self, camera_pos: Vec3) {
        for inst in self.instances.iter_mut() {
            let dist = (inst.position - camera_pos).length();
            let lod = match &inst.kind {
                VegetationKind::Tree(tt) => VegetationLod::for_tree(*tt).lod_for_distance(dist),
                VegetationKind::Grass    => VegetationLod::for_grass().lod_for_distance(dist),
                VegetationKind::Rock {..} => VegetationLod::for_rock().lod_for_distance(dist),
                VegetationKind::Shrub    => VegetationLod::for_rock().lod_for_distance(dist),
                VegetationKind::Flower   => VegetationLod::for_grass().lod_for_distance(dist),
            };
            inst.lod_level = lod;
            inst.visible   = lod < 3;
        }
    }

    /// Frustum cull instances. `planes` is 6 plane normal+distance pairs.
    pub fn frustum_cull(&mut self, planes: &[(Vec3, f32); 6]) {
        for inst in self.instances.iter_mut() {
            if !inst.visible { continue; }
            let p = inst.position;
            let inside = planes.iter().all(|(normal, dist)| {
                normal.dot(p) + dist >= 0.0
            });
            inst.visible = inside;
        }
    }

    /// Apply seasonal variation to all instances.
    pub fn apply_season(&mut self, month: u32) {
        for inst in self.instances.iter_mut() {
            let biome = match &inst.kind {
                VegetationKind::Tree(tt) => {
                    // Approximate biome from tree type
                    match tt {
                        TreeType::Pine | TreeType::Fern => BiomeType::Taiga,
                        TreeType::Tropical | TreeType::Palm => BiomeType::TropicalForest,
                        TreeType::Willow => BiomeType::Swamp,
                        _ => BiomeType::TemperateForest,
                    }
                }
                _ => BiomeType::Grassland,
            };
            let sf = SeasonFactor::season_factor(biome, month);
            // Scale by density and season
            inst.scale = inst.scale * sf.density_scale;
            inst.visible = inst.visible && sf.density_scale > 0.05;
        }
    }

    /// Update wind simulation for the given time step.
    pub fn update(&mut self, dt: f32, wind: Vec2) {
        self.time += dt;
        self.wind_vector = wind;
        self.grass_field.update_wind(self.time, wind);
    }

    /// Count visible instances.
    pub fn visible_count(&self) -> usize {
        self.instances.iter().filter(|i| i.visible).count()
    }

    /// Get all visible instances at a given LOD level.
    pub fn instances_at_lod(&self, lod: u8) -> impl Iterator<Item = &VegetationInstance> {
        self.instances.iter().filter(move |i| i.visible && i.lod_level == lod)
    }
}

impl Default for VegetationSystem {
    fn default() -> Self { Self::new() }
}

// ── VegetationPainter ─────────────────────────────────────────────────────────

/// Manual vegetation painting API for terrain editors.
#[derive(Debug, Clone)]
pub struct VegetationPainter {
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub brush_kind: VegetationKind,
}

impl VegetationPainter {
    pub fn new(radius: f32, strength: f32, kind: VegetationKind) -> Self {
        Self { brush_radius: radius, brush_strength: strength, brush_kind: kind }
    }

    /// Add vegetation instances within brush circle around `center`.
    pub fn paint(
        &self,
        center: Vec3,
        system: &mut VegetationSystem,
        heightmap: &HeightMap,
        seed: u64,
    ) {
        let mut rng = Rng::new(seed.wrapping_add(center.x.to_bits() as u64).wrapping_add(center.z.to_bits() as u64));
        let count = (self.brush_radius * self.brush_strength * 2.0) as usize + 1;
        for _ in 0..count {
            let angle = rng.next_f32() * std::f32::consts::TAU;
            let dist  = rng.next_f32() * self.brush_radius;
            let px = center.x + angle.cos() * dist;
            let pz = center.z + angle.sin() * dist;
            let xi = px as usize;
            let zi = pz as usize;
            let alt = heightmap.get(
                xi.min(heightmap.width.saturating_sub(1)),
                zi.min(heightmap.height.saturating_sub(1)),
            );
            system.instances.push(VegetationInstance {
                position: Vec3::new(px, alt * 100.0, pz),
                rotation: rng.next_f32() * std::f32::consts::TAU,
                scale: Vec3::splat(rng.next_f32_range(0.8, 1.2)),
                lod_level: 0,
                visible: true,
                kind: self.brush_kind.clone(),
            });
        }
    }

    /// Remove vegetation instances within brush circle around `center`.
    pub fn erase(&self, center: Vec3, system: &mut VegetationSystem) {
        let r2 = self.brush_radius * self.brush_radius;
        system.instances.retain(|inst| {
            let dx = inst.position.x - center.x;
            let dz = inst.position.z - center.z;
            dx * dx + dz * dz > r2
        });
    }

    /// Resize brush radius.
    pub fn resize(&mut self, new_radius: f32) {
        self.brush_radius = new_radius.max(0.1);
    }
}

// ── Impostor Billboard Generation ─────────────────────────────────────────────

/// Represents a billboard impostor for far-distance rendering.
#[derive(Clone, Debug)]
pub struct ImpostorBillboard {
    pub position:   Vec3,
    pub size:       Vec2,
    pub lod_level:  u8,
    pub atlas_uv:   [f32; 4],  // u0, v0, u1, v1
}

/// Generate billboard impostors from a set of tree instances.
pub fn generate_impostors(
    instances: &[VegetationInstance],
    camera_pos: Vec3,
    max_distance: f32,
) -> Vec<ImpostorBillboard> {
    instances.iter()
        .filter(|i| i.visible && i.lod_level == 2)
        .filter(|i| (i.position - camera_pos).length() < max_distance)
        .enumerate()
        .map(|(idx, inst)| {
            // Atlas UV depends on tree type slot
            let slot = match &inst.kind {
                VegetationKind::Tree(tt) => *tt as usize,
                _ => 0,
            };
            let u0 = (slot % 4) as f32 * 0.25;
            let v0 = (slot / 4) as f32 * 0.25;
            ImpostorBillboard {
                position: inst.position,
                size: Vec2::new(4.0 * inst.scale.x, 8.0 * inst.scale.y),
                lod_level: inst.lod_level,
                atlas_uv: [u0, v0, u0 + 0.25, v0 + 0.25],
            }
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::heightmap::FractalNoise;
    use crate::terrain::biome::{ClimateSimulator, BiomeMap};

    fn make_test_terrain(size: usize, seed: u64) -> (HeightMap, BiomeMap) {
        let hm = FractalNoise::generate(size, size, 4, 2.0, 0.5, 3.0, seed);
        let sim = ClimateSimulator::default();
        let climate = sim.simulate(&hm);
        let bm = BiomeMap::from_heightmap(&hm, &climate);
        (hm, bm)
    }

    #[test]
    fn test_tree_params_all_types() {
        let types = [
            TreeType::Oak, TreeType::Pine, TreeType::Birch, TreeType::Tropical,
            TreeType::Dead, TreeType::Palm, TreeType::Willow, TreeType::Cactus,
            TreeType::Fern, TreeType::Mushroom,
        ];
        let mut rng = Rng::new(42);
        for tt in types {
            let p = TreeParams::for_type(tt, &mut rng);
            assert!(p.height > 0.0);
            assert!(p.crown_radius > 0.0);
        }
    }

    #[test]
    fn test_tree_skeleton_generates_segments() {
        let mut rng = Rng::new(42);
        let p = TreeParams::for_type(TreeType::Oak, &mut rng);
        let skel = TreeSkeleton::generate(TreeType::Oak, &p, 42);
        assert!(!skel.segments.is_empty());
    }

    #[test]
    fn test_tree_skeleton_bounds() {
        let mut rng = Rng::new(42);
        let p = TreeParams::for_type(TreeType::Pine, &mut rng);
        let skel = TreeSkeleton::generate(TreeType::Pine, &p, 42);
        let (mn, mx) = skel.bounds();
        assert!(mx.y > mn.y, "tree should have positive height");
    }

    #[test]
    fn test_grass_cluster_creation() {
        let mut rng = Rng::new(42);
        let center = Vec3::new(5.0, 0.0, 5.0);
        let cluster = GrassCluster::new(center, 2.0, BiomeType::Grassland, &mut rng);
        assert!(cluster.blade_height > 0.0);
        assert!(cluster.density > 0.0);
    }

    #[test]
    fn test_grass_field_generation() {
        let (hm, bm) = make_test_terrain(32, 42);
        let field = GrassField::generate(&hm, &bm, 1.0, 42);
        // Should produce some clusters (terrain has mixed land/water)
        // Just verify it doesn't panic
        let _ = field.clusters.len();
    }

    #[test]
    fn test_rock_cluster_poisson() {
        let center = Vec3::new(10.0, 0.0, 10.0);
        let cluster = RockCluster::generate(center, 5.0, BiomeType::Mountain, 10, 99);
        assert!(!cluster.rocks.is_empty());
        // All rocks within radius (with tolerance)
        for rock in &cluster.rocks {
            let dx = rock.position.x - center.x;
            let dz = rock.position.z - center.z;
            assert!(dx * dx + dz * dz <= 5.0 * 5.0 + 0.01);
        }
    }

    #[test]
    fn test_vegetation_lod_distances() {
        let lod = VegetationLod::for_tree(TreeType::Oak);
        assert_eq!(lod.lod_for_distance(10.0), 0);
        assert_eq!(lod.lod_for_distance(lod.lod1_distance + 1.0), 1);
        assert_eq!(lod.lod_for_distance(lod.billboard_distance + 1.0), 2);
        assert_eq!(lod.lod_for_distance(lod.cull_distance + 1.0), 3);
    }

    #[test]
    fn test_vegetation_system_generation() {
        let (hm, bm) = make_test_terrain(32, 42);
        let sys = VegetationSystem::generate(&hm, &bm, 1.0, 42);
        // Should produce instances
        let _ = sys.instances.len();
        let _ = sys.grass_field.clusters.len();
    }

    #[test]
    fn test_vegetation_system_lod_update() {
        let (hm, bm) = make_test_terrain(32, 42);
        let mut sys = VegetationSystem::generate(&hm, &bm, 1.0, 42);
        sys.update_lod(Vec3::new(16.0, 50.0, 16.0));
        // All instances should have a valid LOD
        for inst in &sys.instances {
            assert!(inst.lod_level <= 3);
        }
    }

    #[test]
    fn test_vegetation_painter_paint() {
        let (hm, bm) = make_test_terrain(32, 42);
        let mut sys = VegetationSystem::generate(&hm, &bm, 0.1, 42);
        let painter = VegetationPainter::new(5.0, 1.0, VegetationKind::Tree(TreeType::Oak));
        let before = sys.instances.len();
        painter.paint(Vec3::new(16.0, 0.0, 16.0), &mut sys, &hm, 1234);
        assert!(sys.instances.len() > before);
    }

    #[test]
    fn test_vegetation_painter_erase() {
        let (hm, bm) = make_test_terrain(32, 42);
        let mut sys = VegetationSystem::generate(&hm, &bm, 1.0, 42);
        let painter = VegetationPainter::new(100.0, 1.0, VegetationKind::Grass);
        painter.erase(Vec3::new(16.0, 0.0, 16.0), &mut sys);
        // After erasing with huge radius, should be empty
        assert_eq!(sys.instances.len(), 0);
    }

    #[test]
    fn test_generate_impostors() {
        let mut instances = vec![
            VegetationInstance {
                position: Vec3::new(5.0, 0.0, 5.0),
                rotation: 0.0,
                scale: Vec3::ONE,
                lod_level: 2,
                visible: true,
                kind: VegetationKind::Tree(TreeType::Oak),
            },
            VegetationInstance {
                position: Vec3::new(100.0, 0.0, 100.0),
                rotation: 0.0,
                scale: Vec3::ONE,
                lod_level: 2,
                visible: true,
                kind: VegetationKind::Tree(TreeType::Pine),
            },
        ];
        let billboards = generate_impostors(&instances, Vec3::ZERO, 50.0);
        // Only the first (distance 7) is within 50.0
        assert_eq!(billboards.len(), 1);
    }
}
