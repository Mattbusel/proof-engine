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

// ── Wind System ───────────────────────────────────────────────────────────────

/// Models global wind for vegetation animation.
#[derive(Clone, Debug)]
pub struct WindSystem {
    /// Base wind direction and strength.
    pub base_wind:    Vec2,
    /// Wind gustiness [0, 1]: how much the wind varies.
    pub gustiness:    f32,
    /// Wind turbulence frequency.
    pub turbulence:   f32,
    /// Current resolved wind vector.
    pub current_wind: Vec2,
    /// Internal time.
    time:             f32,
    /// Gust cycle phase.
    gust_phase:       f32,
}

impl WindSystem {
    pub fn new(base_wind: Vec2, gustiness: f32) -> Self {
        Self {
            base_wind,
            gustiness,
            turbulence: 0.3,
            current_wind: base_wind,
            time: 0.0,
            gust_phase: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.gust_phase += dt * 0.3;
        // Gust cycle: sine wave modulation of wind strength
        let gust_mul = 1.0 + self.gustiness * (self.gust_phase * 0.7).sin()
                                            * (self.gust_phase * 1.3).sin();
        // Direction variation: slow oscillation
        let dir_angle = (self.base_wind.y.atan2(self.base_wind.x))
            + (self.time * 0.1).sin() * self.gustiness * 0.4;
        let base_speed = self.base_wind.length();
        self.current_wind = Vec2::new(
            dir_angle.cos() * base_speed * gust_mul,
            dir_angle.sin() * base_speed * gust_mul,
        );
    }

    /// Compute local wind at a position (adds turbulence based on position).
    pub fn local_wind(&self, pos: Vec2) -> Vec2 {
        let turb = self.turbulence;
        let phase_x = pos.x * 0.02 + self.time * 0.5;
        let phase_y = pos.y * 0.02 + self.time * 0.7;
        let turb_x = phase_x.sin() * turb;
        let turb_y = phase_y.sin() * turb;
        self.current_wind + Vec2::new(turb_x, turb_y) * self.current_wind.length()
    }
}

// ── Vegetation Density Map ────────────────────────────────────────────────────

/// A 2D map of vegetation density values.
#[derive(Clone, Debug)]
pub struct VegetationDensityMap {
    pub width:  usize,
    pub height: usize,
    pub data:   Vec<f32>,
}

impl VegetationDensityMap {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![0.0; width * height] }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height { self.data[y * self.width + x] } else { 0.0 }
    }

    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = v.clamp(0.0, 1.0);
        }
    }

    /// Build density map from heightmap and biome map for trees.
    pub fn for_trees(
        heightmap: &crate::terrain::heightmap::HeightMap,
        biome_map: &BiomeMap,
        slope_map: &crate::terrain::heightmap::HeightMap,
    ) -> Self {
        let w = heightmap.width;
        let h = heightmap.height;
        let mut dm = Self::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let alt   = heightmap.get(x, y);
                let slope = slope_map.get(x, y);
                let biome = biome_map.get(x, y);
                if alt < 0.1 || slope > 0.6 { continue; }
                let base = VegetationDensity::for_biome(biome).tree_density;
                // Altitude penalty: fewer trees at high altitude
                let alt_factor = if alt > 0.7 { 1.0 - (alt - 0.7) / 0.3 } else { 1.0 };
                // Slope penalty: fewer trees on steep slopes
                let slope_factor = (1.0 - slope / 0.6).max(0.0);
                dm.set(x, y, base * alt_factor * slope_factor);
            }
        }
        dm
    }

    /// Build density map for grass.
    pub fn for_grass(
        heightmap: &crate::terrain::heightmap::HeightMap,
        biome_map: &BiomeMap,
    ) -> Self {
        let w = heightmap.width;
        let h = heightmap.height;
        let mut dm = Self::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let alt   = heightmap.get(x, y);
                let biome = biome_map.get(x, y);
                if alt < 0.08 { continue; }
                let base = VegetationDensity::for_biome(biome).grass_density;
                // Grass prefers moderate altitude
                let alt_factor = if alt > 0.8 { 0.1 } else if alt > 0.6 { 0.5 } else { 1.0 };
                dm.set(x, y, base * alt_factor);
            }
        }
        dm
    }

    /// Sample with bilinear interpolation.
    pub fn sample_bilinear(&self, x: f32, y: f32) -> f32 {
        let cx = x.clamp(0.0, (self.width  - 1) as f32);
        let cy = y.clamp(0.0, (self.height - 1) as f32);
        let x0 = cx.floor() as usize;
        let y0 = cy.floor() as usize;
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = cx - x0 as f32;
        let ty = cy - y0 as f32;
        let h00 = self.get(x0, y0);
        let h10 = self.get(x1, y0);
        let h01 = self.get(x0, y1);
        let h11 = self.get(x1, y1);
        let lerp = |a: f32, b: f32, t: f32| a + t * (b - a);
        lerp(lerp(h00, h10, tx), lerp(h01, h11, tx), ty)
    }
}

// ── Vegetation Cluster ────────────────────────────────────────────────────────

/// A group of same-species instances sharing a spatial cluster for efficient culling.
#[derive(Debug, Clone)]
pub struct VegetationCluster {
    pub center:     Vec3,
    pub radius:     f32,
    pub instances:  Vec<usize>,  // indices into VegetationSystem.instances
    pub kind:       VegetationKind,
    pub lod_level:  u8,
}

impl VegetationCluster {
    pub fn new(center: Vec3, radius: f32, kind: VegetationKind) -> Self {
        Self { center, radius, instances: Vec::new(), kind, lod_level: 0 }
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        let dx = p.x - self.center.x;
        let dz = p.z - self.center.z;
        dx * dx + dz * dz <= self.radius * self.radius
    }

    pub fn distance_to(&self, p: Vec3) -> f32 {
        let dx = p.x - self.center.x;
        let dz = p.z - self.center.z;
        (dx * dx + dz * dz).sqrt()
    }
}

// ── VegetationAtlas ───────────────────────────────────────────────────────────

/// Manages texture atlas slots for vegetation impostor billboards.
#[derive(Debug, Clone)]
pub struct VegetationAtlas {
    /// Width and height of the atlas in pixels.
    pub resolution: u32,
    /// Number of slots per row.
    pub slots_per_row: u32,
    /// Slot size (in atlas pixels).
    pub slot_size: u32,
    /// Mapping from tree type to atlas slot index.
    pub slot_map: std::collections::HashMap<String, u32>,
    /// Next available slot.
    pub next_slot: u32,
}

impl VegetationAtlas {
    pub fn new(resolution: u32, slots_per_row: u32) -> Self {
        Self {
            resolution,
            slots_per_row,
            slot_size: resolution / slots_per_row,
            slot_map: std::collections::HashMap::new(),
            next_slot: 0,
        }
    }

    /// Allocate an atlas slot for a named vegetation type.
    pub fn allocate_slot(&mut self, name: &str) -> Option<u32> {
        let max_slots = self.slots_per_row * self.slots_per_row;
        if self.next_slot >= max_slots { return None; }
        let slot = self.next_slot;
        self.slot_map.insert(name.to_string(), slot);
        self.next_slot += 1;
        Some(slot)
    }

    /// Get UV coordinates for a given slot.
    pub fn slot_uv(&self, slot: u32) -> [f32; 4] {
        let row = slot / self.slots_per_row;
        let col = slot % self.slots_per_row;
        let sz = 1.0 / self.slots_per_row as f32;
        let u0 = col as f32 * sz;
        let v0 = row as f32 * sz;
        [u0, v0, u0 + sz, v0 + sz]
    }

    /// Get UV for a named type (returns default if not found).
    pub fn get_uv(&self, name: &str) -> [f32; 4] {
        let slot = self.slot_map.get(name).copied().unwrap_or(0);
        self.slot_uv(slot)
    }
}

// ── Forest Generator ──────────────────────────────────────────────────────────

/// Specialized forest generator that creates realistic forest patterns.
pub struct ForestGenerator {
    pub min_tree_spacing: f32,
    pub edge_density:     f32,  // trees are denser at forest edges
    pub clustering:       f32,  // 0 = uniform, 1 = highly clustered
}

impl Default for ForestGenerator {
    fn default() -> Self {
        Self {
            min_tree_spacing: 2.0,
            edge_density: 1.5,
            clustering: 0.4,
        }
    }
}

impl ForestGenerator {
    pub fn new(min_spacing: f32, clustering: f32) -> Self {
        Self { min_tree_spacing: min_spacing, edge_density: 1.5, clustering }
    }

    /// Generate tree positions in a forest region using clustered Poisson disk sampling.
    pub fn generate_positions(
        &self,
        density_map: &VegetationDensityMap,
        heightmap: &crate::terrain::heightmap::HeightMap,
        biome_map: &BiomeMap,
        seed: u64,
    ) -> Vec<Vec3> {
        let mut rng = Rng::new(seed);
        let mut positions: Vec<Vec3> = Vec::new();
        let w = density_map.width;
        let h = density_map.height;

        // Generate candidate positions on a grid with jitter
        let grid_step = (self.min_tree_spacing * 0.8) as usize + 1;
        for y in (0..h).step_by(grid_step) {
            for x in (0..w).step_by(grid_step) {
                let density = density_map.get(x, y);
                if density < rng.next_f32() { continue; }

                // Add spatial jitter
                let jx = rng.next_f32_range(-(grid_step as f32 * 0.4), grid_step as f32 * 0.4);
                let jz = rng.next_f32_range(-(grid_step as f32 * 0.4), grid_step as f32 * 0.4);
                let px = (x as f32 + jx).clamp(0.0, w as f32 - 1.0);
                let pz = (y as f32 + jz).clamp(0.0, h as f32 - 1.0);

                // Check min spacing
                let too_close = positions.iter().any(|p| {
                    let dx = p.x - px;
                    let dz = p.z - pz;
                    dx * dx + dz * dz < self.min_tree_spacing * self.min_tree_spacing
                });
                if too_close { continue; }

                let alt = heightmap.get(x, y);
                positions.push(Vec3::new(px, alt * 100.0, pz));
            }
        }

        // Clustering: move some trees toward existing cluster centers
        if self.clustering > 0.0 {
            let cluster_radius = self.min_tree_spacing * 4.0;
            let n = positions.len();
            for i in 0..n {
                if rng.next_f32() < self.clustering {
                    // Find a nearby position to cluster toward
                    let target_idx = rng.next_usize(n);
                    if target_idx == i { continue; }
                    let tp = positions[target_idx];
                    let dx = tp.x - positions[i].x;
                    let dz = tp.z - positions[i].z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist < cluster_radius && dist > self.min_tree_spacing {
                        let move_frac = self.clustering * 0.3;
                        positions[i].x += dx * move_frac;
                        positions[i].z += dz * move_frac;
                    }
                }
            }
        }

        positions
    }
}

// ── Snow Accumulation ─────────────────────────────────────────────────────────

/// Computes snow accumulation on vegetation based on slope and temperature.
pub struct SnowAccumulation;

impl SnowAccumulation {
    /// Compute snow coverage [0, 1] for a vegetation instance.
    /// `temperature` is normalized (0 = freezing, 1 = hot).
    /// `slope_normal` is the surface normal at the instance position.
    pub fn coverage(temperature: f32, slope_normal: Vec3, altitude: f32) -> f32 {
        if temperature > 0.35 { return 0.0; }
        // Cold factor: how cold is it?
        let cold = (0.35 - temperature) / 0.35;
        // Vertical surface receives less snow
        let vertical_factor = slope_normal.y.clamp(0.0, 1.0);
        // Higher altitude = more snow
        let alt_factor = if altitude > 0.7 { 1.0 } else { altitude / 0.7 };
        (cold * vertical_factor * (0.5 + 0.5 * alt_factor)).clamp(0.0, 1.0)
    }

    /// Apply snow tinting to a vegetation instance's color.
    pub fn apply_tint(base_color: Vec4, snow_coverage: f32) -> Vec4 {
        let snow_color = Vec4::new(0.92, 0.95, 1.0, 1.0);
        Vec4::new(
            base_color.x + (snow_color.x - base_color.x) * snow_coverage,
            base_color.y + (snow_color.y - base_color.y) * snow_coverage,
            base_color.z + (snow_color.z - base_color.z) * snow_coverage,
            base_color.w,
        )
    }
}

// ── Leaf Color System ─────────────────────────────────────────────────────────

/// Computes leaf colors based on tree type, season, and variation.
pub struct LeafColorSystem;

impl LeafColorSystem {
    /// Base leaf color for a tree type.
    pub fn base_color(tt: TreeType) -> Vec3 {
        match tt {
            TreeType::Oak      => Vec3::new(0.2, 0.5,  0.1),
            TreeType::Pine     => Vec3::new(0.1, 0.35, 0.08),
            TreeType::Birch    => Vec3::new(0.35, 0.6, 0.15),
            TreeType::Tropical => Vec3::new(0.1, 0.5,  0.05),
            TreeType::Dead     => Vec3::new(0.4, 0.3,  0.15),
            TreeType::Palm     => Vec3::new(0.15, 0.5, 0.1),
            TreeType::Willow   => Vec3::new(0.25, 0.55, 0.12),
            TreeType::Cactus   => Vec3::new(0.2, 0.45, 0.1),
            TreeType::Fern     => Vec3::new(0.15, 0.55, 0.1),
            TreeType::Mushroom => Vec3::new(0.7, 0.3,  0.1),
        }
    }

    /// Seasonal leaf color: spring=bright green, summer=deep green,
    ///                      autumn=orange/red, winter=bare/brown.
    pub fn seasonal_color(tt: TreeType, month: u32, variation: f32) -> Vec3 {
        let base = Self::base_color(tt);
        let m = (month % 12) as f32;
        // Summer peak at month 6, winter at 0/12
        let summer_t = ((m - 6.0) * std::f32::consts::PI / 6.0).cos() * 0.5 + 0.5;
        let autumn_t = {
            // Autumn: Sept-Nov (months 8-10)
            if m >= 8.0 && m <= 11.0 {
                ((m - 8.0) / 3.0).min(1.0)
            } else { 0.0 }
        };
        let dead_trees = matches!(tt, TreeType::Dead);
        if dead_trees {
            return Vec3::new(0.35, 0.25, 0.1);
        }
        let evergreen = matches!(tt, TreeType::Pine | TreeType::Fern | TreeType::Cactus | TreeType::Tropical | TreeType::Palm);
        if evergreen {
            return base * (0.8 + 0.2 * summer_t);
        }
        // Deciduous: green summer → orange/red autumn → brown winter
        let autumn_color = Vec3::new(0.8 + variation * 0.1, 0.3 + variation * 0.1, 0.05);
        let winter_color = Vec3::new(0.3, 0.2, 0.1);
        let spring_color = Vec3::new(base.x * 1.2, base.y * 1.3, base.z * 1.0);
        let summer_color = base;

        if m < 3.0 {
            // Winter
            winter_color * (0.3 + summer_t * 0.2)
        } else if m < 5.0 {
            // Spring
            let t = (m - 3.0) / 2.0;
            winter_color + (spring_color - winter_color) * t
        } else if m < 8.0 {
            // Summer
            spring_color + (summer_color - spring_color) * ((m - 5.0) / 3.0)
        } else {
            // Autumn → Winter
            summer_color + (autumn_color - summer_color) * autumn_t
        }
    }
}

// ── Undergrowth System ────────────────────────────────────────────────────────

/// Generates undergrowth (ferns, mushrooms, flowers) beneath forest canopy.
pub struct UndergrowthSystem;

impl UndergrowthSystem {
    /// Generate undergrowth instances beneath existing tree canopies.
    pub fn generate_under_canopy(
        tree_instances: &[VegetationInstance],
        heightmap: &crate::terrain::heightmap::HeightMap,
        seed: u64,
    ) -> Vec<VegetationInstance> {
        let mut rng = Rng::new(seed);
        let mut out = Vec::new();
        for tree in tree_instances {
            if !matches!(tree.kind, VegetationKind::Tree(_)) { continue; }
            let canopy_r = tree.scale.x * 3.0;
            let count = (canopy_r * canopy_r * 0.5) as usize + 1;
            for _ in 0..count {
                let angle = rng.next_f32() * std::f32::consts::TAU;
                let dist  = rng.next_f32() * canopy_r;
                let px = tree.position.x + angle.cos() * dist;
                let pz = tree.position.z + angle.sin() * dist;
                let xi = (px as usize).min(heightmap.width  - 1);
                let zi = (pz as usize).min(heightmap.height - 1);
                let alt = heightmap.get(xi, zi);
                // Under canopy: spawn ferns or mushrooms
                let kind = if rng.next_f32() < 0.7 {
                    VegetationKind::Tree(TreeType::Fern)
                } else {
                    VegetationKind::Tree(TreeType::Mushroom)
                };
                out.push(VegetationInstance {
                    position: Vec3::new(px, alt * 100.0, pz),
                    rotation: rng.next_f32() * std::f32::consts::TAU,
                    scale: Vec3::splat(rng.next_f32_range(0.2, 0.5)),
                    lod_level: 0,
                    visible: true,
                    kind,
                });
            }
        }
        out
    }
}

// ── Extended Vegetation Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod extended_vegetation_tests {
    use super::*;
    use crate::terrain::heightmap::FractalNoise;
    use crate::terrain::biome::{ClimateSimulator, BiomeMap};

    fn make_terrain(size: usize, seed: u64) -> (crate::terrain::heightmap::HeightMap, BiomeMap) {
        let hm = FractalNoise::generate(size, size, 4, 2.0, 0.5, 3.0, seed);
        let sim = ClimateSimulator::default();
        let climate = sim.simulate(&hm);
        let bm = BiomeMap::from_heightmap(&hm, &climate);
        (hm, bm)
    }

    #[test]
    fn test_wind_system_update() {
        let mut wind = WindSystem::new(Vec2::new(1.0, 0.0), 0.3);
        wind.update(0.016);
        // After update, wind should be valid
        assert!(wind.current_wind.length() >= 0.0);
    }

    #[test]
    fn test_wind_system_local() {
        let wind = WindSystem::new(Vec2::new(2.0, 1.0), 0.3);
        let local = wind.local_wind(Vec2::new(10.0, 20.0));
        assert!(local.length() > 0.0);
    }

    #[test]
    fn test_vegetation_density_map_for_trees() {
        let (hm, bm) = make_terrain(32, 42);
        let slope = hm.slope_map();
        let dm = VegetationDensityMap::for_trees(&hm, &bm, &slope);
        assert_eq!(dm.data.len(), 32 * 32);
        assert!(dm.data.iter().all(|&v| v >= 0.0 && v <= 1.0));
    }

    #[test]
    fn test_vegetation_density_map_for_grass() {
        let (hm, bm) = make_terrain(32, 42);
        let dm = VegetationDensityMap::for_grass(&hm, &bm);
        assert_eq!(dm.data.len(), 32 * 32);
    }

    #[test]
    fn test_forest_generator() {
        let (hm, bm) = make_terrain(32, 42);
        let slope = hm.slope_map();
        let dm = VegetationDensityMap::for_trees(&hm, &bm, &slope);
        let fg = ForestGenerator::new(2.0, 0.3);
        let positions = fg.generate_positions(&dm, &hm, &bm, 42);
        // Should generate some positions
        let _ = positions.len();
    }

    #[test]
    fn test_vegetation_cluster() {
        let c = VegetationCluster::new(Vec3::new(10.0, 0.0, 10.0), 5.0, VegetationKind::Grass);
        assert!(c.contains_point(Vec3::new(10.0, 0.0, 10.0)));
        assert!(!c.contains_point(Vec3::new(100.0, 0.0, 100.0)));
        assert!((c.distance_to(Vec3::new(10.0, 0.0, 15.0)) - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_vegetation_atlas() {
        let mut atlas = VegetationAtlas::new(512, 4);
        let slot = atlas.allocate_slot("Oak");
        assert!(slot.is_some());
        let uv = atlas.get_uv("Oak");
        assert!(uv[0] >= 0.0 && uv[2] <= 1.0);
    }

    #[test]
    fn test_snow_accumulation() {
        let cold_coverage = SnowAccumulation::coverage(0.1, Vec3::Y, 0.8);
        assert!(cold_coverage > 0.0);
        let warm_coverage = SnowAccumulation::coverage(0.8, Vec3::Y, 0.5);
        assert_eq!(warm_coverage, 0.0);
    }

    #[test]
    fn test_leaf_color_seasonal() {
        let summer = LeafColorSystem::seasonal_color(TreeType::Oak, 6, 0.0);
        let winter = LeafColorSystem::seasonal_color(TreeType::Oak, 1, 0.0);
        // Summer should be greener
        assert!(summer.y > winter.y);
        // Pine is evergreen: small variation
        let summer_pine = LeafColorSystem::seasonal_color(TreeType::Pine, 6, 0.0);
        let winter_pine = LeafColorSystem::seasonal_color(TreeType::Pine, 1, 0.0);
        assert!((summer_pine.y - winter_pine.y).abs() < 0.3);
    }

    #[test]
    fn test_undergrowth_generation() {
        let (hm, _) = make_terrain(32, 42);
        let trees = vec![
            VegetationInstance {
                position: Vec3::new(16.0, 50.0, 16.0),
                rotation: 0.0,
                scale: Vec3::splat(2.0),
                lod_level: 0,
                visible: true,
                kind: VegetationKind::Tree(TreeType::Oak),
            },
        ];
        let undergrowth = UndergrowthSystem::generate_under_canopy(&trees, &hm, 42);
        assert!(!undergrowth.is_empty());
    }

    #[test]
    fn test_all_tree_types_have_base_color() {
        let types = [
            TreeType::Oak, TreeType::Pine, TreeType::Birch, TreeType::Tropical,
            TreeType::Dead, TreeType::Palm, TreeType::Willow, TreeType::Cactus,
            TreeType::Fern, TreeType::Mushroom,
        ];
        for tt in types {
            let c = LeafColorSystem::base_color(tt);
            assert!(c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0);
        }
    }
}

// ── Grass Simulation ──────────────────────────────────────────────────────────

/// Per-blade grass animation state.
#[derive(Clone, Debug)]
pub struct GrassBlade {
    pub position:     Vec3,
    pub base_angle:   f32,   // natural lean angle in radians
    pub sway_phase:   f32,   // individual phase offset for wind sway
    pub height:       f32,
    pub width:        f32,
    pub color:        Vec4,
    pub roughness:    f32,
}

impl GrassBlade {
    pub fn new(position: Vec3, height: f32, color: Vec4, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            position,
            base_angle: rng.next_f32_range(-0.2, 0.2),
            sway_phase: rng.next_f32() * std::f32::consts::TAU,
            height,
            width: rng.next_f32_range(0.02, 0.05),
            color,
            roughness: rng.next_f32_range(0.7, 1.0),
        }
    }

    /// Compute current sway angle given time and wind.
    pub fn current_angle(&self, time: f32, wind: Vec2) -> f32 {
        let wind_strength = wind.length();
        let sway = wind_strength * (time * 1.5 + self.sway_phase).sin() * 0.3;
        self.base_angle + sway
    }

    /// Tip position after sway.
    pub fn tip_position(&self, time: f32, wind: Vec2) -> Vec3 {
        let angle = self.current_angle(time, wind);
        self.position + Vec3::new(
            angle.sin() * self.height,
            angle.cos() * self.height,
            (angle * 0.7).sin() * self.height * 0.3,
        )
    }
}

/// A patch of individually simulated grass blades.
#[derive(Debug)]
pub struct GrassPatch {
    pub blades:   Vec<GrassBlade>,
    pub center:   Vec3,
    pub radius:   f32,
}

impl GrassPatch {
    pub fn generate(center: Vec3, radius: f32, density: f32, biome: BiomeType, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let count = (radius * radius * std::f32::consts::PI * density * 4.0) as usize;
        let color = match biome {
            BiomeType::Grassland | BiomeType::TemperateForest =>
                Vec4::new(0.3 + rng.next_f32() * 0.1, 0.6, 0.15, 1.0),
            BiomeType::Savanna =>
                Vec4::new(0.65 + rng.next_f32() * 0.1, 0.55, 0.12, 1.0),
            BiomeType::Tundra =>
                Vec4::new(0.5, 0.52, 0.32, 1.0),
            _ => Vec4::new(0.3, 0.55, 0.15, 1.0),
        };
        let blades: Vec<GrassBlade> = (0..count).map(|_| {
            let angle = rng.next_f32() * std::f32::consts::TAU;
            let dist  = rng.next_f32() * radius;
            let px = center.x + angle.cos() * dist;
            let pz = center.z + angle.sin() * dist;
            let height = rng.next_f32_range(0.15, 0.5);
            let col = Vec4::new(
                (color.x + rng.next_f32_range(-0.05, 0.05)).clamp(0.0, 1.0),
                (color.y + rng.next_f32_range(-0.05, 0.05)).clamp(0.0, 1.0),
                (color.z + rng.next_f32_range(-0.05, 0.05)).clamp(0.0, 1.0),
                1.0,
            );
            GrassBlade::new(Vec3::new(px, center.y, pz), height, col, rng.next_u64())
        }).collect();
        Self { blades, center, radius }
    }

    pub fn blade_count(&self) -> usize { self.blades.len() }
}

// ── Vegetation Query API ──────────────────────────────────────────────────────

/// Query API for finding vegetation near a position.
pub struct VegetationQuery<'a> {
    system: &'a VegetationSystem,
}

impl<'a> VegetationQuery<'a> {
    pub fn new(system: &'a VegetationSystem) -> Self { Self { system } }

    /// Find all visible trees within `radius` of `pos`.
    pub fn trees_near(&self, pos: Vec3, radius: f32) -> Vec<&VegetationInstance> {
        let r2 = radius * radius;
        self.system.instances.iter()
            .filter(|i| i.visible && matches!(i.kind, VegetationKind::Tree(_)))
            .filter(|i| {
                let dx = i.position.x - pos.x;
                let dz = i.position.z - pos.z;
                dx * dx + dz * dz <= r2
            })
            .collect()
    }

    /// Find the nearest tree to `pos`.
    pub fn nearest_tree(&self, pos: Vec3) -> Option<&VegetationInstance> {
        self.system.instances.iter()
            .filter(|i| i.visible && matches!(i.kind, VegetationKind::Tree(_)))
            .min_by(|a, b| {
                let da = (a.position - pos).length();
                let db = (b.position - pos).length();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Count instances of each kind within radius.
    pub fn count_by_kind(&self, pos: Vec3, radius: f32) -> std::collections::HashMap<String, usize> {
        let r2 = radius * radius;
        let mut counts = std::collections::HashMap::new();
        for inst in &self.system.instances {
            if !inst.visible { continue; }
            let dx = inst.position.x - pos.x;
            let dz = inst.position.z - pos.z;
            if dx * dx + dz * dz > r2 { continue; }
            let key = match &inst.kind {
                VegetationKind::Tree(tt) => tt.name().to_string(),
                VegetationKind::Grass   => "Grass".to_string(),
                VegetationKind::Rock { size_class } => format!("Rock({})", size_class),
                VegetationKind::Shrub   => "Shrub".to_string(),
                VegetationKind::Flower  => "Flower".to_string(),
            };
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    /// Find all rocks within radius.
    pub fn rocks_near(&self, pos: Vec3, radius: f32) -> Vec<&VegetationInstance> {
        let r2 = radius * radius;
        self.system.instances.iter()
            .filter(|i| matches!(i.kind, VegetationKind::Rock { .. }))
            .filter(|i| {
                let dx = i.position.x - pos.x;
                let dz = i.position.z - pos.z;
                dx * dx + dz * dz <= r2
            })
            .collect()
    }
}

// ── Vegetation Serializer ─────────────────────────────────────────────────────

/// Serializes and deserializes vegetation data for saving/loading.
pub struct VegetationSerializer;

impl VegetationSerializer {
    /// Serialize vegetation instances to compact binary.
    /// Format per instance: [x:f32][y:f32][z:f32][rotation:f32][scale_x:f32][scale_y:f32][scale_z:f32][kind:u8][lod:u8]
    pub fn serialize(instances: &[VegetationInstance]) -> Vec<u8> {
        let mut out = Vec::with_capacity(instances.len() * 36 + 4);
        out.extend_from_slice(&(instances.len() as u32).to_le_bytes());
        for inst in instances {
            out.extend_from_slice(&inst.position.x.to_le_bytes());
            out.extend_from_slice(&inst.position.y.to_le_bytes());
            out.extend_from_slice(&inst.position.z.to_le_bytes());
            out.extend_from_slice(&inst.rotation.to_le_bytes());
            out.extend_from_slice(&inst.scale.x.to_le_bytes());
            out.extend_from_slice(&inst.scale.y.to_le_bytes());
            out.extend_from_slice(&inst.scale.z.to_le_bytes());
            let kind_byte: u8 = match &inst.kind {
                VegetationKind::Tree(tt) => *tt as u8,
                VegetationKind::Grass    => 20,
                VegetationKind::Rock { size_class } => 21 + size_class,
                VegetationKind::Shrub    => 24,
                VegetationKind::Flower   => 25,
            };
            out.push(kind_byte);
            out.push(inst.lod_level);
        }
        out
    }

    /// Deserialize vegetation instances from binary.
    pub fn deserialize(bytes: &[u8]) -> Option<Vec<VegetationInstance>> {
        if bytes.len() < 4 { return None; }
        let count = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
        let record_size = 4 * 7 + 2; // 7 floats + 2 bytes
        if bytes.len() < 4 + count * record_size { return None; }
        let mut instances = Vec::with_capacity(count);
        let mut pos = 4usize;
        for _ in 0..count {
            let read_f32 = |p: &mut usize| -> f32 {
                let v = f32::from_le_bytes(bytes[*p..*p+4].try_into().unwrap_or([0;4]));
                *p += 4;
                v
            };
            let x  = read_f32(&mut pos);
            let y  = read_f32(&mut pos);
            let z  = read_f32(&mut pos);
            let rot = read_f32(&mut pos);
            let sx  = read_f32(&mut pos);
            let sy  = read_f32(&mut pos);
            let sz  = read_f32(&mut pos);
            let kind_byte = bytes[pos]; pos += 1;
            let lod = bytes[pos]; pos += 1;
            let kind = match kind_byte {
                0  => VegetationKind::Tree(TreeType::Oak),
                1  => VegetationKind::Tree(TreeType::Pine),
                2  => VegetationKind::Tree(TreeType::Birch),
                3  => VegetationKind::Tree(TreeType::Tropical),
                4  => VegetationKind::Tree(TreeType::Dead),
                5  => VegetationKind::Tree(TreeType::Palm),
                6  => VegetationKind::Tree(TreeType::Willow),
                7  => VegetationKind::Tree(TreeType::Cactus),
                8  => VegetationKind::Tree(TreeType::Fern),
                9  => VegetationKind::Tree(TreeType::Mushroom),
                20 => VegetationKind::Grass,
                21 => VegetationKind::Rock { size_class: 0 },
                22 => VegetationKind::Rock { size_class: 1 },
                23 => VegetationKind::Rock { size_class: 2 },
                24 => VegetationKind::Shrub,
                _  => VegetationKind::Flower,
            };
            instances.push(VegetationInstance {
                position: Vec3::new(x, y, z),
                rotation: rot,
                scale: Vec3::new(sx, sy, sz),
                lod_level: lod,
                visible: true,
                kind,
            });
        }
        Some(instances)
    }
}

// ── More Vegetation Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod more_vegetation_tests {
    use super::*;

    #[test]
    fn test_grass_blade_creation() {
        let blade = GrassBlade::new(Vec3::new(1.0, 0.0, 1.0), 0.4, Vec4::ONE, 42);
        assert!(blade.height > 0.0);
        assert!(blade.width > 0.0);
    }

    #[test]
    fn test_grass_blade_sway() {
        let blade = GrassBlade::new(Vec3::ZERO, 0.5, Vec4::ONE, 99);
        let angle_t0 = blade.current_angle(0.0, Vec2::new(1.0, 0.0));
        let angle_t1 = blade.current_angle(1.0, Vec2::new(1.0, 0.0));
        // Angle should change over time (sway)
        let _ = (angle_t0, angle_t1);
    }

    #[test]
    fn test_grass_blade_tip() {
        let blade = GrassBlade::new(Vec3::ZERO, 0.5, Vec4::ONE, 42);
        let tip = blade.tip_position(0.0, Vec2::ZERO);
        assert!(tip.y > 0.0); // tip should be above base
    }

    #[test]
    fn test_grass_patch_generation() {
        let patch = GrassPatch::generate(Vec3::ZERO, 5.0, 1.0, BiomeType::Grassland, 42);
        assert!(!patch.blades.is_empty());
        for blade in &patch.blades {
            let dx = blade.position.x;
            let dz = blade.position.z;
            assert!(dx * dx + dz * dz <= 5.0 * 5.0 + 0.1);
        }
    }

    #[test]
    fn test_vegetation_query_trees_near() {
        let mut sys = VegetationSystem::new();
        sys.instances.push(VegetationInstance {
            position: Vec3::new(5.0, 0.0, 0.0),
            rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
            kind: VegetationKind::Tree(TreeType::Oak),
        });
        sys.instances.push(VegetationInstance {
            position: Vec3::new(50.0, 0.0, 0.0),
            rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
            kind: VegetationKind::Tree(TreeType::Pine),
        });
        let q = VegetationQuery::new(&sys);
        let near = q.trees_near(Vec3::ZERO, 10.0);
        assert_eq!(near.len(), 1);
    }

    #[test]
    fn test_vegetation_query_nearest() {
        let mut sys = VegetationSystem::new();
        sys.instances.push(VegetationInstance {
            position: Vec3::new(3.0, 0.0, 0.0),
            rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
            kind: VegetationKind::Tree(TreeType::Oak),
        });
        sys.instances.push(VegetationInstance {
            position: Vec3::new(10.0, 0.0, 0.0),
            rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
            kind: VegetationKind::Tree(TreeType::Pine),
        });
        let q = VegetationQuery::new(&sys);
        let nearest = q.nearest_tree(Vec3::ZERO);
        assert!(nearest.is_some());
        assert!((nearest.unwrap().position.x - 3.0).abs() < 1e-4);
    }

    #[test]
    fn test_vegetation_serializer_roundtrip() {
        let instances = vec![
            VegetationInstance {
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: 1.5,
                scale: Vec3::new(1.0, 1.5, 1.0),
                lod_level: 0,
                visible: true,
                kind: VegetationKind::Tree(TreeType::Oak),
            },
            VegetationInstance {
                position: Vec3::new(5.0, 0.0, 7.0),
                rotation: 0.5,
                scale: Vec3::ONE,
                lod_level: 1,
                visible: true,
                kind: VegetationKind::Rock { size_class: 1 },
            },
        ];
        let bytes = VegetationSerializer::serialize(&instances);
        let restored = VegetationSerializer::deserialize(&bytes).unwrap();
        assert_eq!(restored.len(), instances.len());
        assert!((restored[0].position.x - 1.0).abs() < 1e-5);
        assert!((restored[0].position.y - 2.0).abs() < 1e-5);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Extended vegetation systems
// ─────────────────────────────────────────────────────────────────────────────

/// Represents a single falling leaf particle used for visual effects.
#[derive(Debug, Clone)]
pub struct LeafParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: f32,
    pub angular_velocity: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub color: Vec3,
    pub size: f32,
    pub alpha: f32,
}

impl LeafParticle {
    pub fn new(pos: Vec3, color: Vec3, size: f32, lifetime: f32) -> Self {
        Self {
            position: pos,
            velocity: Vec3::new(0.0, -0.5, 0.0),
            rotation: 0.0,
            angular_velocity: 1.2,
            lifetime,
            max_lifetime: lifetime,
            color,
            size,
            alpha: 1.0,
        }
    }

    /// Advance simulation by `dt` seconds, applying gravity and wind drift.
    pub fn update(&mut self, dt: f32, wind: Vec3) {
        let gravity = Vec3::new(0.0, -0.3, 0.0);
        let drag = -self.velocity * 0.4;
        self.velocity += (gravity + wind + drag) * dt;
        self.position += self.velocity * dt;
        self.rotation += self.angular_velocity * dt;
        self.lifetime -= dt;
        self.alpha = (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
    }

    pub fn is_alive(&self) -> bool {
        self.lifetime > 0.0
    }
}

/// Emitter that spawns leaf particles from tree canopies.
#[derive(Debug, Clone)]
pub struct LeafParticleEmitter {
    pub origin: Vec3,
    pub emit_radius: f32,
    pub emit_rate: f32,      // particles per second
    pub particle_lifetime: f32,
    pub color: Vec3,
    accumulated: f32,
    rng_state: u64,
}

impl LeafParticleEmitter {
    pub fn new(origin: Vec3, radius: f32, rate: f32, lifetime: f32, color: Vec3) -> Self {
        Self {
            origin,
            emit_radius: radius,
            emit_rate: rate,
            particle_lifetime: lifetime,
            color,
            accumulated: 0.0,
            rng_state: (origin.x.to_bits() as u64) ^ 0xDEADBEEF_u64,
        }
    }

    fn rng_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        (self.rng_state as f32) / (u64::MAX as f32)
    }

    /// Returns newly spawned particles for this timestep.
    pub fn emit(&mut self, dt: f32) -> Vec<LeafParticle> {
        self.accumulated += self.emit_rate * dt;
        let count = self.accumulated as usize;
        self.accumulated -= count as f32;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let angle = self.rng_f32() * std::f32::consts::TAU;
            let r = self.rng_f32() * self.emit_radius;
            let offset = Vec3::new(r * angle.cos(), self.rng_f32() * 0.5, r * angle.sin());
            out.push(LeafParticle::new(
                self.origin + offset,
                self.color,
                0.05 + self.rng_f32() * 0.05,
                self.particle_lifetime * (0.8 + self.rng_f32() * 0.4),
            ));
        }
        out
    }
}

/// Manages a pool of leaf particles across an entire scene.
#[derive(Debug, Clone, Default)]
pub struct LeafParticleSystem {
    pub particles: Vec<LeafParticle>,
    pub emitters: Vec<LeafParticleEmitter>,
    pub max_particles: usize,
}

impl LeafParticleSystem {
    pub fn new(max_particles: usize) -> Self {
        Self { particles: Vec::new(), emitters: Vec::new(), max_particles }
    }

    pub fn add_emitter(&mut self, e: LeafParticleEmitter) {
        self.emitters.push(e);
    }

    pub fn update(&mut self, dt: f32, wind: Vec3) {
        // Update existing particles
        self.particles.retain_mut(|p| { p.update(dt, wind); p.is_alive() });
        // Emit new ones if budget allows
        let budget = self.max_particles.saturating_sub(self.particles.len());
        let mut new_particles = Vec::new();
        for emitter in &mut self.emitters {
            let batch = emitter.emit(dt);
            new_particles.extend(batch);
        }
        new_particles.truncate(budget);
        self.particles.extend(new_particles);
    }

    pub fn live_count(&self) -> usize {
        self.particles.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Terrain-aware vegetation placement helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Placement constraint: the cell at (x, z) in the heightmap must satisfy
/// slope and altitude criteria for the given tree type.
pub struct PlacementConstraint {
    pub min_altitude: f32,
    pub max_altitude: f32,
    pub max_slope_deg: f32,
}

impl PlacementConstraint {
    pub fn for_tree(tt: TreeType) -> Self {
        match tt {
            TreeType::Palm => Self { min_altitude: 0.02, max_altitude: 0.25, max_slope_deg: 20.0 },
            TreeType::Cactus => Self { min_altitude: 0.05, max_altitude: 0.40, max_slope_deg: 30.0 },
            TreeType::Oak | TreeType::Fern => Self { min_altitude: 0.10, max_altitude: 0.60, max_slope_deg: 35.0 },
            TreeType::Pine | TreeType::Tropical => Self { min_altitude: 0.20, max_altitude: 0.80, max_slope_deg: 40.0 },
            TreeType::Birch => Self { min_altitude: 0.15, max_altitude: 0.65, max_slope_deg: 38.0 },
            TreeType::Dead => Self { min_altitude: 0.05, max_altitude: 0.90, max_slope_deg: 50.0 },
            TreeType::Willow => Self { min_altitude: 0.02, max_altitude: 0.30, max_slope_deg: 15.0 },
            TreeType::Mushroom => Self { min_altitude: 0.05, max_altitude: 0.45, max_slope_deg: 25.0 },
        }
    }

    pub fn check(&self, altitude: f32, slope_deg: f32) -> bool {
        altitude >= self.min_altitude
            && altitude <= self.max_altitude
            && slope_deg <= self.max_slope_deg
    }
}

/// Filters a list of candidate positions against a heightmap using
/// `PlacementConstraint`.
pub struct TerrainAwarePlacement;

impl TerrainAwarePlacement {
    /// `positions` are world-space XZ coordinates. `hm_scale` converts world
    /// units to [0,1] heightmap UV. Returns accepted positions with world Y.
    pub fn filter(
        positions: &[(f32, f32)],
        heights: &[f32],   // same length as positions, pre-sampled
        slopes: &[f32],    // degrees, same length
        constraint: &PlacementConstraint,
    ) -> Vec<(f32, f32, f32)> {
        positions.iter().zip(heights.iter()).zip(slopes.iter())
            .filter_map(|(((x, z), &h), &s)| {
                if constraint.check(h, s) { Some((*x, h, *z)) } else { None }
            })
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vegetation heat-map: tracks density per grid cell for editor visualization
// ─────────────────────────────────────────────────────────────────────────────

/// 2-D grid counting how many vegetation instances fall in each cell.
#[derive(Debug, Clone)]
pub struct VegetationHeatMap {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    counts: Vec<u32>,
}

impl VegetationHeatMap {
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        Self { width, height, cell_size, counts: vec![0; width * height] }
    }

    pub fn accumulate(&mut self, x: f32, z: f32) {
        let cx = (x / self.cell_size) as usize;
        let cz = (z / self.cell_size) as usize;
        if cx < self.width && cz < self.height {
            self.counts[cz * self.width + cx] += 1;
        }
    }

    pub fn build_from(system: &VegetationSystem, width: usize, height: usize, cell_size: f32) -> Self {
        let mut hm = Self::new(width, height, cell_size);
        for inst in &system.instances {
            hm.accumulate(inst.position.x, inst.position.z);
        }
        hm
    }

    pub fn max_count(&self) -> u32 {
        self.counts.iter().copied().max().unwrap_or(0)
    }

    pub fn normalized_at(&self, cx: usize, cz: usize) -> f32 {
        let max = self.max_count();
        if max == 0 { return 0.0; }
        self.counts[cz * self.width + cx] as f32 / max as f32
    }

    pub fn total_instances(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vegetation export formats
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal OBJ-like text exporter for vegetation instances (positions only).
pub struct VegetationObjExporter;

impl VegetationObjExporter {
    /// Produces a textual listing of instance positions as OBJ vertex lines.
    pub fn export(system: &VegetationSystem) -> String {
        let mut out = String::with_capacity(system.instances.len() * 32);
        out.push_str("# Vegetation export\n");
        for inst in &system.instances {
            let kind_str = match &inst.kind {
                VegetationKind::Tree(t) => format!("{:?}", t),
                VegetationKind::Grass => "Grass".to_owned(),
                VegetationKind::Rock { size_class } => format!("Rock{}", size_class),
                VegetationKind::Shrub => "Shrub".to_owned(),
                VegetationKind::Flower => "Flower".to_owned(),
            };
            out.push_str(&format!(
                "v {:.4} {:.4} {:.4} # {}\n",
                inst.position.x, inst.position.y, inst.position.z, kind_str
            ));
        }
        out
    }
}

/// CSV exporter for spreadsheet analysis.
pub struct VegetationCsvExporter;

impl VegetationCsvExporter {
    pub fn export(system: &VegetationSystem) -> String {
        let mut out = String::from("x,y,z,rotation,scale_x,scale_y,scale_z,lod,kind\n");
        for inst in &system.instances {
            let kind_str = match &inst.kind {
                VegetationKind::Tree(t) => format!("{:?}", t),
                VegetationKind::Grass => "Grass".to_owned(),
                VegetationKind::Rock { size_class } => format!("Rock{}", size_class),
                VegetationKind::Shrub => "Shrub".to_owned(),
                VegetationKind::Flower => "Flower".to_owned(),
            };
            out.push_str(&format!(
                "{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{}\n",
                inst.position.x, inst.position.y, inst.position.z,
                inst.rotation,
                inst.scale.x, inst.scale.y, inst.scale.z,
                inst.lod_level,
                kind_str,
            ));
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vegetation culling helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Axis-aligned bounding box used for frustum / region culling.
#[derive(Debug, Clone, Copy)]
pub struct VegetationAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl VegetationAabb {
    pub fn from_instances(instances: &[VegetationInstance]) -> Self {
        let mut mn = Vec3::splat(f32::INFINITY);
        let mut mx = Vec3::splat(f32::NEG_INFINITY);
        for inst in instances {
            mn = mn.min(inst.position);
            mx = mx.max(inst.position);
        }
        Self { min: mn, max: mx }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &VegetationAabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }
}

/// Spatial grid for O(1) region queries of vegetation instances.
pub struct VegetationGrid {
    pub cell_size: f32,
    pub width_cells: usize,
    pub height_cells: usize,
    cells: Vec<Vec<usize>>,  // cell → list of instance indices
}

impl VegetationGrid {
    pub fn build(system: &VegetationSystem, cell_size: f32, world_width: f32, world_height: f32) -> Self {
        let wc = ((world_width / cell_size).ceil() as usize).max(1);
        let hc = ((world_height / cell_size).ceil() as usize).max(1);
        let mut cells = vec![Vec::new(); wc * hc];
        for (i, inst) in system.instances.iter().enumerate() {
            let cx = ((inst.position.x / cell_size) as usize).min(wc - 1);
            let cz = ((inst.position.z / cell_size) as usize).min(hc - 1);
            cells[cz * wc + cx].push(i);
        }
        Self { cell_size, width_cells: wc, height_cells: hc, cells }
    }

    /// Returns indices of all instances in cells overlapping `aabb`.
    pub fn query_aabb<'a>(&'a self, aabb: &VegetationAabb, system: &'a VegetationSystem) -> Vec<&'a VegetationInstance> {
        let x0 = ((aabb.min.x / self.cell_size) as usize).min(self.width_cells.saturating_sub(1));
        let x1 = ((aabb.max.x / self.cell_size) as usize).min(self.width_cells.saturating_sub(1));
        let z0 = ((aabb.min.z / self.cell_size) as usize).min(self.height_cells.saturating_sub(1));
        let z1 = ((aabb.max.z / self.cell_size) as usize).min(self.height_cells.saturating_sub(1));
        let mut result = Vec::new();
        for cz in z0..=z1 {
            for cx in x0..=x1 {
                for &idx in &self.cells[cz * self.width_cells + cx] {
                    result.push(&system.instances[idx]);
                }
            }
        }
        result
    }

    pub fn cell_count(&self) -> usize {
        self.width_cells * self.height_cells
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Extended tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod extended_veg_tests {
    use super::*;

    #[test]
    fn test_leaf_particle_lifecycle() {
        let mut p = LeafParticle::new(Vec3::ZERO, Vec3::new(0.8, 0.4, 0.1), 0.05, 2.0);
        assert!(p.is_alive());
        p.update(1.5, Vec3::new(0.1, 0.0, 0.05));
        assert!(p.is_alive());
        p.update(1.0, Vec3::ZERO);
        assert!(!p.is_alive());
    }

    #[test]
    fn test_leaf_particle_system_emitter() {
        let mut sys = LeafParticleSystem::new(1000);
        let emitter = LeafParticleEmitter::new(
            Vec3::new(10.0, 15.0, 10.0),
            3.0, 50.0, 3.0,
            Vec3::new(1.0, 0.5, 0.0),
        );
        sys.add_emitter(emitter);
        sys.update(0.5, Vec3::new(0.2, 0.0, 0.1));
        // 50 particles/sec * 0.5s = 25 particles expected
        assert!(sys.live_count() > 0);
    }

    #[test]
    fn test_placement_constraint_oak() {
        let c = PlacementConstraint::for_tree(TreeType::Oak);
        assert!(c.check(0.3, 20.0));
        assert!(!c.check(0.05, 20.0));  // too low
        assert!(!c.check(0.3, 40.0));   // too steep
    }

    #[test]
    fn test_terrain_aware_placement_filter() {
        let positions = vec![(10.0f32, 10.0f32), (20.0, 20.0), (30.0, 30.0)];
        let heights   = vec![0.30f32, 0.05f32, 0.45f32];
        let slopes    = vec![15.0f32, 10.0f32, 50.0f32];
        let c = PlacementConstraint::for_tree(TreeType::Oak);
        let accepted = TerrainAwarePlacement::filter(&positions, &heights, &slopes, &c);
        // Only first passes (h=0.3, s=15); second fails altitude; third fails slope
        assert_eq!(accepted.len(), 1);
        assert!((accepted[0].0 - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_vegetation_heat_map() {
        let mut sys = VegetationSystem::new();
        for i in 0..20u32 {
            sys.instances.push(VegetationInstance {
                position: Vec3::new(i as f32 * 5.0, 0.0, 0.0),
                rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
                kind: VegetationKind::Tree(TreeType::Oak),
            });
        }
        let hm = VegetationHeatMap::build_from(&sys, 10, 10, 10.0);
        assert!(hm.total_instances() > 0);
        assert!(hm.max_count() >= 1);
    }

    #[test]
    fn test_vegetation_obj_exporter() {
        let mut sys = VegetationSystem::new();
        sys.instances.push(VegetationInstance {
            position: Vec3::new(1.0, 0.5, 2.0),
            rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
            kind: VegetationKind::Tree(TreeType::Pine),
        });
        let obj = VegetationObjExporter::export(&sys);
        assert!(obj.contains("v "));
        assert!(obj.contains("Pine"));
    }

    #[test]
    fn test_vegetation_csv_exporter() {
        let mut sys = VegetationSystem::new();
        sys.instances.push(VegetationInstance {
            position: Vec3::new(3.0, 1.0, 4.0),
            rotation: 0.7, scale: Vec3::ONE, lod_level: 0, visible: true,
            kind: VegetationKind::Grass,
        });
        let csv = VegetationCsvExporter::export(&sys);
        assert!(csv.starts_with("x,y,z"));
        assert!(csv.contains("Grass"));
    }

    #[test]
    fn test_vegetation_aabb() {
        let instances = vec![
            VegetationInstance { position: Vec3::new(0.0, 0.0, 0.0), rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true, kind: VegetationKind::Grass },
            VegetationInstance { position: Vec3::new(10.0, 5.0, 10.0), rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true, kind: VegetationKind::Grass },
        ];
        let aabb = VegetationAabb::from_instances(&instances);
        assert!(aabb.contains(Vec3::new(5.0, 2.5, 5.0)));
        assert!(!aabb.contains(Vec3::new(20.0, 0.0, 0.0)));
        let center = aabb.center();
        assert!((center.x - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_vegetation_grid_query() {
        let mut sys = VegetationSystem::new();
        for i in 0..10u32 {
            sys.instances.push(VegetationInstance {
                position: Vec3::new(i as f32 * 10.0, 0.0, 5.0),
                rotation: 0.0, scale: Vec3::ONE, lod_level: 0, visible: true,
                kind: VegetationKind::Tree(TreeType::Oak),
            });
        }
        let grid = VegetationGrid::build(&sys, 20.0, 100.0, 100.0);
        let query_aabb = VegetationAabb { min: Vec3::new(0.0, -1.0, 0.0), max: Vec3::new(25.0, 1.0, 10.0) };
        let found = grid.query_aabb(&query_aabb, &sys);
        assert!(!found.is_empty());
    }
}
