//! Cave system generation — 3D cellular automata.
//!
//! Uses 3D cellular automata (B5678/S45678 variant) to create organic
//! cave networks, then connects components and identifies features
//! (chambers, tunnels, lakes).

use super::Rng;

/// A 3D voxel grid for cave generation.
#[derive(Debug, Clone)]
pub struct VoxelGrid {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    /// true = solid rock, false = open space
    pub solid: Vec<bool>,
}

impl VoxelGrid {
    pub fn new(w: usize, h: usize, d: usize) -> Self {
        Self { width: w, height: h, depth: d, solid: vec![true; w * h * d] }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        z * self.width * self.height + y * self.width + x
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> bool {
        if x < 0 || y < 0 || z < 0 { return true; }
        let (ux, uy, uz) = (x as usize, y as usize, z as usize);
        if ux >= self.width || uy >= self.height || uz >= self.depth { return true; }
        self.solid[self.idx(ux, uy, uz)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, solid: bool) {
        let i = self.idx(x, y, z);
        self.solid[i] = solid;
    }

    /// Count solid neighbors in a 3×3×3 cube (26-neighborhood).
    pub fn count_neighbors(&self, x: usize, y: usize, z: usize) -> u32 {
        let mut count = 0u32;
        for dz in -1..=1i32 {
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    if dx == 0 && dy == 0 && dz == 0 { continue; }
                    if self.get(x as i32 + dx, y as i32 + dy, z as i32 + dz) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Count open (air) cells.
    pub fn open_count(&self) -> usize {
        self.solid.iter().filter(|&&s| !s).count()
    }
}

/// A feature detected in the cave.
#[derive(Debug, Clone)]
pub enum CaveFeature {
    /// Large open area.
    Chamber { center: (usize, usize, usize), volume: usize },
    /// Narrow passage.
    Tunnel { cells: Vec<(usize, usize, usize)> },
    /// Water-filled area at the bottom.
    Lake { cells: Vec<(usize, usize, usize)> },
    /// Vertical shaft.
    Shaft { x: usize, z: usize, y_min: usize, y_max: usize },
}

/// A complete cave system.
#[derive(Debug, Clone)]
pub struct CaveSystem {
    pub grid: VoxelGrid,
    pub features: Vec<CaveFeature>,
    /// Entrance positions on the surface.
    pub entrances: Vec<(usize, usize, usize)>,
}

/// Generate cave systems.
pub fn generate(grid_size: usize, num_systems: usize, rng: &mut Rng) -> Vec<CaveSystem> {
    let mut systems = Vec::with_capacity(num_systems);
    let cave_size = grid_size.min(48); // Cap cave size for performance

    for _ in 0..num_systems {
        let system = generate_single(cave_size, cave_size, cave_size / 2, rng);
        systems.push(system);
    }

    systems
}

/// Generate a single cave system.
fn generate_single(w: usize, h: usize, d: usize, rng: &mut Rng) -> CaveSystem {
    let mut grid = VoxelGrid::new(w, h, d);

    // 1. Random fill (~45% air)
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                grid.set(x, y, z, rng.coin(0.55));
            }
        }
    }

    // Force border solid
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                if x == 0 || y == 0 || z == 0 || x == w - 1 || y == h - 1 || z == d - 1 {
                    grid.set(x, y, z, true);
                }
            }
        }
    }

    // 2. Cellular automata (4 iterations)
    for _ in 0..4 {
        let old = grid.solid.clone();
        for z in 1..d - 1 {
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let neighbors = grid.count_neighbors(x, y, z);
                    let idx = grid.idx(x, y, z);
                    grid.solid[idx] = if old[idx] {
                        // Survival: stay solid if >= 13 solid neighbors
                        neighbors >= 13
                    } else {
                        // Birth: become solid if >= 14 solid neighbors
                        neighbors >= 14
                    };
                }
            }
        }
    }

    // 3. Detect features
    let features = detect_features(&grid);

    // 4. Find entrances (top layer openings)
    let mut entrances = Vec::new();
    let top_z = d - 2;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if !grid.get(x as i32, y as i32, top_z as i32) {
                entrances.push((x, y, top_z));
                if entrances.len() >= 4 { break; }
            }
        }
        if entrances.len() >= 4 { break; }
    }

    CaveSystem { grid, features, entrances }
}

/// Simple feature detection.
fn detect_features(grid: &VoxelGrid) -> Vec<CaveFeature> {
    let mut features = Vec::new();
    let w = grid.width;
    let h = grid.height;
    let d = grid.depth;

    // Find chambers: connected open regions larger than threshold
    let mut visited = vec![false; w * h * d];
    for z in 1..d - 1 {
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = grid.idx(x, y, z);
                if grid.solid[idx] || visited[idx] { continue; }

                // Flood fill
                let mut stack = vec![(x, y, z)];
                let mut cells = Vec::new();
                while let Some((cx, cy, cz)) = stack.pop() {
                    let ci = grid.idx(cx, cy, cz);
                    if visited[ci] || grid.solid[ci] { continue; }
                    visited[ci] = true;
                    cells.push((cx, cy, cz));

                    for &(dx, dy, dz) in &[
                        (1i32, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)
                    ] {
                        let nx = cx as i32 + dx;
                        let ny = cy as i32 + dy;
                        let nz = cz as i32 + dz;
                        if nx > 0 && ny > 0 && nz > 0
                            && (nx as usize) < w - 1 && (ny as usize) < h - 1 && (nz as usize) < d - 1
                        {
                            let ni = grid.idx(nx as usize, ny as usize, nz as usize);
                            if !visited[ni] && !grid.solid[ni] {
                                stack.push((nx as usize, ny as usize, nz as usize));
                            }
                        }
                    }
                }

                if cells.len() > 50 {
                    // Compute center
                    let (sx, sy, sz): (usize, usize, usize) = cells.iter()
                        .fold((0, 0, 0), |(ax, ay, az), (x, y, z)| (ax + x, ay + y, az + z));
                    let n = cells.len();
                    features.push(CaveFeature::Chamber {
                        center: (sx / n, sy / n, sz / n),
                        volume: n,
                    });
                }

                // Detect lakes (open cells at z=1)
                let lake_cells: Vec<_> = cells.iter().filter(|&&(_, _, z)| z <= 2).cloned().collect();
                if lake_cells.len() > 10 {
                    features.push(CaveFeature::Lake { cells: lake_cells });
                }
            }
        }
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cave_generation() {
        let mut rng = Rng::new(42);
        let systems = generate(32, 2, &mut rng);
        assert_eq!(systems.len(), 2);
        for sys in &systems {
            assert!(sys.grid.open_count() > 0, "cave should have open spaces");
        }
    }

    #[test]
    fn test_voxel_grid() {
        let mut g = VoxelGrid::new(4, 4, 4);
        assert!(g.get(0, 0, 0)); // default solid
        g.set(2, 2, 2, false);
        assert!(!g.get(2, 2, 2));
    }
}
