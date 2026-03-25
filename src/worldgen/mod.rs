//! Procedural world generation framework.
//!
//! Generates entire mathematical realities: tectonic plates, erosion, climate,
//! biomes, rivers, caves, settlements, history, languages, mythology, artifacts,
//! and genetics — all from equations.

pub mod tectonics;
pub mod erosion;
pub mod climate;
pub mod biomes;
pub mod rivers;
pub mod caves;
pub mod settlements;
pub mod history;
pub mod language;
pub mod mythology;
pub mod artifacts;
pub mod genetics;

/// Master world seed — all generation derives from this.
#[derive(Debug, Clone, Copy)]
pub struct WorldSeed(pub u64);

impl WorldSeed {
    pub fn new(seed: u64) -> Self { Self(seed) }
    /// Derive a sub-seed for a specific system.
    pub fn derive(&self, system_id: u32) -> u64 {
        let mut h = self.0;
        h ^= system_id as u64;
        h = h.wrapping_mul(0x517CC1B727220A95);
        h ^= h >> 32;
        h
    }
}

/// Simple deterministic RNG (xoshiro256**).
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut s = [seed, seed ^ 0xDEADBEEF, seed.wrapping_mul(6364136223846793005), seed ^ 0xCAFEBABE];
        // Warm up
        let mut r = Self { s };
        for _ in 0..20 { r.next_u64(); }
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if min >= max { return min; }
        min + (self.next_u64() % (max - min) as u64) as u32
    }

    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    pub fn range_usize(&mut self, min: usize, max: usize) -> usize {
        if min >= max { return min; }
        min + (self.next_u64() as usize % (max - min))
    }

    pub fn gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-15);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.next_u64() as usize % (i + 1);
            slice.swap(i, j);
        }
    }

    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() { return None; }
        Some(&slice[self.next_u64() as usize % slice.len()])
    }

    pub fn coin(&mut self, probability: f32) -> bool {
        self.next_f32() < probability
    }
}

/// 2D grid coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub fn new(x: i32, y: i32) -> Self { Self { x, y } }

    pub fn neighbors4(&self) -> [GridPos; 4] {
        [
            GridPos::new(self.x - 1, self.y),
            GridPos::new(self.x + 1, self.y),
            GridPos::new(self.x, self.y - 1),
            GridPos::new(self.x, self.y + 1),
        ]
    }

    pub fn neighbors8(&self) -> [GridPos; 8] {
        [
            GridPos::new(self.x - 1, self.y - 1),
            GridPos::new(self.x, self.y - 1),
            GridPos::new(self.x + 1, self.y - 1),
            GridPos::new(self.x - 1, self.y),
            GridPos::new(self.x + 1, self.y),
            GridPos::new(self.x - 1, self.y + 1),
            GridPos::new(self.x, self.y + 1),
            GridPos::new(self.x + 1, self.y + 1),
        ]
    }

    pub fn distance_sq(&self, other: &GridPos) -> i32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

/// A 2D heightfield grid used across worldgen systems.
#[derive(Debug, Clone)]
pub struct Grid2D {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Grid2D {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![0.0; width * height] }
    }

    pub fn filled(width: usize, height: usize, value: f32) -> Self {
        Self { width, height, data: vec![value; width * height] }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        self.data[y * self.width + x] = v;
    }

    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> f32 {
        let cx = x.clamp(0, self.width as i32 - 1) as usize;
        let cy = y.clamp(0, self.height as i32 - 1) as usize;
        self.data[cy * self.width + cx]
    }

    pub fn add(&mut self, x: usize, y: usize, v: f32) {
        self.data[y * self.width + x] += v;
    }

    pub fn min_value(&self) -> f32 { self.data.iter().cloned().fold(f32::MAX, f32::min) }
    pub fn max_value(&self) -> f32 { self.data.iter().cloned().fold(f32::MIN, f32::max) }

    pub fn normalize(&mut self) {
        let min = self.min_value();
        let max = self.max_value();
        let range = (max - min).max(1e-9);
        for v in &mut self.data { *v = (*v - min) / range; }
    }

    /// Bilinear sample at fractional coordinates.
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let x0 = (x.floor() as i32).clamp(0, self.width as i32 - 2) as usize;
        let y0 = (y.floor() as i32).clamp(0, self.height as i32 - 2) as usize;
        let tx = x.fract().clamp(0.0, 1.0);
        let ty = y.fract().clamp(0.0, 1.0);
        let v00 = self.get(x0, y0);
        let v10 = self.get(x0 + 1, y0);
        let v01 = self.get(x0, y0 + 1);
        let v11 = self.get(x0 + 1, y0 + 1);
        let a = v00 + tx * (v10 - v00);
        let b = v01 + tx * (v11 - v01);
        a + ty * (b - a)
    }

    /// Gradient at a point (finite differences).
    pub fn gradient(&self, x: usize, y: usize) -> (f32, f32) {
        let left = self.get_clamped(x as i32 - 1, y as i32);
        let right = self.get_clamped(x as i32 + 1, y as i32);
        let down = self.get_clamped(x as i32, y as i32 - 1);
        let up = self.get_clamped(x as i32, y as i32 + 1);
        ((right - left) * 0.5, (up - down) * 0.5)
    }
}

/// Master world generation parameters.
#[derive(Debug, Clone)]
pub struct WorldGenParams {
    pub seed: WorldSeed,
    pub grid_size: usize,
    pub num_plates: usize,
    pub erosion_iterations: usize,
    pub climate_iterations: usize,
    pub history_years: usize,
    pub num_civilizations: usize,
    pub num_languages: usize,
    pub sea_level: f32,
}

impl Default for WorldGenParams {
    fn default() -> Self {
        Self {
            seed: WorldSeed(42),
            grid_size: 256,
            num_plates: 12,
            erosion_iterations: 50000,
            climate_iterations: 100,
            history_years: 5000,
            num_civilizations: 8,
            num_languages: 6,
            sea_level: 0.4,
        }
    }
}

/// The complete generated world.
#[derive(Debug, Clone)]
pub struct GeneratedWorld {
    pub params: WorldGenParams,
    pub heightmap: Grid2D,
    pub plates: tectonics::PlateMap,
    pub temperature: Grid2D,
    pub precipitation: Grid2D,
    pub biome_map: biomes::BiomeMap,
    pub river_network: rivers::RiverNetwork,
    pub cave_systems: Vec<caves::CaveSystem>,
    pub settlements: Vec<settlements::Settlement>,
    pub civilizations: Vec<history::Civilization>,
    pub languages: Vec<language::Language>,
    pub myths: Vec<mythology::Myth>,
    pub artifacts: Vec<artifacts::Artifact>,
}

/// Generate a complete world from parameters.
pub fn generate_world(params: WorldGenParams) -> GeneratedWorld {
    let mut rng = Rng::new(params.seed.0);
    let sz = params.grid_size;

    // 1. Tectonic plates → base heightmap
    let (heightmap, plates) = tectonics::generate(sz, params.num_plates, &mut rng);

    // 2. Erosion sculpts the terrain
    let heightmap = erosion::erode(heightmap, params.erosion_iterations, &mut rng);

    // 3. Climate from heat equation
    let (temperature, precipitation) = climate::simulate(&heightmap, params.climate_iterations, &mut rng);

    // 4. Biomes from climate
    let biome_map = biomes::classify(&heightmap, &temperature, &precipitation, params.sea_level);

    // 5. Rivers from rainfall
    let river_network = rivers::generate(&heightmap, &precipitation, params.sea_level);

    // 6. Cave systems
    let cave_systems = caves::generate(sz, 5, &mut rng);

    // 7. Settlements
    let settlements = settlements::place(
        &heightmap, &biome_map, &river_network, params.num_civilizations * 3, &mut rng,
    );

    // 8. History
    let civilizations = history::simulate(
        &settlements, &biome_map, params.history_years, params.num_civilizations, &mut rng,
    );

    // 9. Languages
    let languages = language::generate(params.num_languages, &civilizations, &mut rng);

    // 10. Mythology
    let myths = mythology::generate(&civilizations, &languages, &mut rng);

    // 11. Artifacts
    let artifacts = artifacts::generate(&civilizations, &myths, &mut rng);

    GeneratedWorld {
        params,
        heightmap,
        plates,
        temperature,
        precipitation,
        biome_map,
        river_network,
        cave_systems,
        settlements,
        civilizations,
        languages,
        myths,
        artifacts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn test_rng_range() {
        let mut r = Rng::new(123);
        for _ in 0..1000 {
            let v = r.range_f32(0.0, 1.0);
            assert!(v >= 0.0 && v <= 1.0);
        }
    }

    #[test]
    fn test_grid2d() {
        let mut g = Grid2D::new(4, 4);
        g.set(2, 3, 1.0);
        assert_eq!(g.get(2, 3), 1.0);
        assert_eq!(g.get(0, 0), 0.0);
    }

    #[test]
    fn test_grid2d_normalize() {
        let mut g = Grid2D::new(4, 4);
        g.set(0, 0, -5.0);
        g.set(3, 3, 10.0);
        g.normalize();
        assert!((g.get(0, 0) - 0.0).abs() < 0.01);
        assert!((g.get(3, 3) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_world_seed_derive() {
        let seed = WorldSeed(42);
        let a = seed.derive(1);
        let b = seed.derive(2);
        assert_ne!(a, b);
    }
}
