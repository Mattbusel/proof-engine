//! Tectonic plate simulation — rigid body plates on a 2D grid.
//!
//! Plates are generated via Voronoi partitioning, then drift according to
//! velocity vectors. Collisions at plate boundaries generate mountains
//! (convergent), rifts (divergent), or transform faults (shear).

use super::{Grid2D, Rng, GridPos};
use std::collections::HashMap;

/// A tectonic plate.
#[derive(Debug, Clone)]
pub struct Plate {
    pub id: u32,
    /// Velocity vector (cells per step).
    pub velocity: (f32, f32),
    /// Whether this is an oceanic (true) or continental (false) plate.
    pub oceanic: bool,
    /// Average elevation (before collision effects).
    pub base_elevation: f32,
    /// Area in grid cells.
    pub area: usize,
    /// Density (oceanic is denser → subducts under continental).
    pub density: f32,
}

/// The plate assignment map (which plate each cell belongs to).
#[derive(Debug, Clone)]
pub struct PlateMap {
    pub width: usize,
    pub height: usize,
    /// Plate ID per cell.
    pub assignment: Vec<u32>,
    /// All plates.
    pub plates: Vec<Plate>,
}

impl PlateMap {
    pub fn plate_at(&self, x: usize, y: usize) -> u32 {
        self.assignment[y * self.width + x]
    }

    /// Get boundary cells (cells adjacent to a different plate).
    pub fn boundary_cells(&self) -> Vec<(usize, usize, u32, u32)> {
        let mut boundaries = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let pid = self.plate_at(x, y);
                for &(nx, ny) in &[(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                    if nx < self.width && ny < self.height {
                        let npid = self.plate_at(nx, ny);
                        if npid != pid {
                            boundaries.push((x, y, pid, npid));
                            break;
                        }
                    }
                }
            }
        }
        boundaries
    }
}

/// Boundary interaction type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoundaryType {
    /// Plates colliding → mountains.
    Convergent,
    /// Plates separating → rifts/volcanos.
    Divergent,
    /// Plates sliding past → transform faults.
    Transform,
}

/// Generate tectonic plates and base heightmap.
pub fn generate(grid_size: usize, num_plates: usize, rng: &mut Rng) -> (Grid2D, PlateMap) {
    let w = grid_size;
    let h = grid_size;

    // 1. Generate plate seeds (Voronoi centers)
    let mut centers: Vec<(usize, usize)> = Vec::with_capacity(num_plates);
    for _ in 0..num_plates {
        centers.push((rng.range_usize(0, w), rng.range_usize(0, h)));
    }

    // 2. Voronoi assignment (each cell → nearest plate center)
    let mut assignment = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut best_dist = i32::MAX;
            let mut best_plate = 0u32;
            for (i, &(cx, cy)) in centers.iter().enumerate() {
                // Wrap-around distance for toroidal topology
                let dx = wrap_dist(x as i32, cx as i32, w as i32);
                let dy = wrap_dist(y as i32, cy as i32, h as i32);
                let d = dx * dx + dy * dy;
                if d < best_dist {
                    best_dist = d;
                    best_plate = i as u32;
                }
            }
            assignment[y * w + x] = best_plate;
        }
    }

    // 3. Create plate structs
    let mut plates: Vec<Plate> = (0..num_plates)
        .map(|i| {
            let oceanic = rng.coin(0.6);
            Plate {
                id: i as u32,
                velocity: (rng.range_f32(-1.0, 1.0), rng.range_f32(-1.0, 1.0)),
                oceanic,
                base_elevation: if oceanic { rng.range_f32(-0.3, 0.1) } else { rng.range_f32(0.2, 0.6) },
                area: 0,
                density: if oceanic { 3.0 } else { 2.7 },
            }
        })
        .collect();

    // Count areas
    for &pid in &assignment {
        plates[pid as usize].area += 1;
    }

    let plate_map = PlateMap { width: w, height: h, assignment, plates: plates.clone() };

    // 4. Generate heightmap from plate collision
    let mut heightmap = Grid2D::new(w, h);

    // Base elevation from plate type
    for y in 0..h {
        for x in 0..w {
            let pid = plate_map.plate_at(x, y) as usize;
            heightmap.set(x, y, plates[pid].base_elevation);
        }
    }

    // Boundary effects
    let boundaries = plate_map.boundary_cells();
    for &(x, y, pid_a, pid_b) in &boundaries {
        let pa = &plates[pid_a as usize];
        let pb = &plates[pid_b as usize];
        let bt = classify_boundary(pa, pb);

        let elevation_boost = match bt {
            BoundaryType::Convergent => {
                // Mountain building
                if pa.oceanic && !pb.oceanic {
                    0.4 // Subduction → volcanic arc
                } else if !pa.oceanic && !pb.oceanic {
                    0.6 // Continental collision → high mountains
                } else {
                    0.2 // Ocean-ocean → island arc
                }
            }
            BoundaryType::Divergent => {
                -0.2 // Rift valley / mid-ocean ridge
            }
            BoundaryType::Transform => {
                0.05 // Slight uplift
            }
        };

        // Apply with falloff
        let radius = 5;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let nx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                let ny = (y as i32 + dy).rem_euclid(h as i32) as usize;
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                let falloff = (1.0 - dist / radius as f32).max(0.0);
                heightmap.add(nx, ny, elevation_boost * falloff * falloff);
            }
        }
    }

    // Add fractal noise for detail
    add_fractal_noise(&mut heightmap, rng, 6, 0.3);

    heightmap.normalize();

    (heightmap, plate_map)
}

/// Classify boundary type from plate velocities.
fn classify_boundary(a: &Plate, b: &Plate) -> BoundaryType {
    // Relative velocity
    let rel_vx = a.velocity.0 - b.velocity.0;
    let rel_vy = a.velocity.1 - b.velocity.1;
    let speed = (rel_vx * rel_vx + rel_vy * rel_vy).sqrt();

    if speed < 0.3 {
        BoundaryType::Transform
    } else {
        // Dot product with boundary normal (approximated)
        let convergence = -(rel_vx * rel_vx + rel_vy * rel_vy).sqrt();
        if convergence < -0.2 {
            BoundaryType::Convergent
        } else {
            BoundaryType::Divergent
        }
    }
}

/// Wrap-around distance for toroidal grid.
fn wrap_dist(a: i32, b: i32, size: i32) -> i32 {
    let d = (a - b).abs();
    d.min(size - d)
}

/// Add fractal Brownian motion noise to a grid.
fn add_fractal_noise(grid: &mut Grid2D, rng: &mut Rng, octaves: usize, amplitude: f32) {
    let w = grid.width;
    let h = grid.height;
    let base_seed = rng.next_u64();

    for octave in 0..octaves {
        let freq = (1 << octave) as f32;
        let amp = amplitude / (1 << octave) as f32;
        let seed = base_seed.wrapping_add(octave as u64 * 1000);

        for y in 0..h {
            for x in 0..w {
                let nx = x as f32 * freq / w as f32;
                let ny = y as f32 * freq / h as f32;
                let noise = value_noise_2d(nx, ny, seed);
                grid.add(x, y, noise * amp);
            }
        }
    }
}

/// Simple value noise.
fn value_noise_2d(x: f32, y: f32, seed: u64) -> f32 {
    let xi = x.floor() as i64;
    let yi = y.floor() as i64;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let tx = xf * xf * (3.0 - 2.0 * xf);
    let ty = yf * yf * (3.0 - 2.0 * yf);

    let hash = |ix: i64, iy: i64| -> f32 {
        let n = (ix.wrapping_mul(374761393) + iy.wrapping_mul(668265263) + seed as i64) as u64;
        let n = n.wrapping_mul(0x5851F42D4C957F2D);
        let n = n ^ (n >> 32);
        (n & 0x00FF_FFFF) as f32 / 0x0080_0000 as f32 - 1.0
    };

    let v00 = hash(xi, yi);
    let v10 = hash(xi + 1, yi);
    let v01 = hash(xi, yi + 1);
    let v11 = hash(xi + 1, yi + 1);
    let a = v00 + tx * (v10 - v00);
    let b = v01 + tx * (v11 - v01);
    a + ty * (b - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_plates() {
        let mut rng = Rng::new(42);
        let (hm, pm) = generate(64, 6, &mut rng);
        assert_eq!(hm.width, 64);
        assert_eq!(pm.plates.len(), 6);
        // All cells assigned
        assert!(pm.assignment.iter().all(|&p| (p as usize) < 6));
    }

    #[test]
    fn test_boundaries_exist() {
        let mut rng = Rng::new(42);
        let (_, pm) = generate(64, 6, &mut rng);
        let boundaries = pm.boundary_cells();
        assert!(!boundaries.is_empty(), "should have plate boundaries");
    }

    #[test]
    fn test_heightmap_normalized() {
        let mut rng = Rng::new(42);
        let (hm, _) = generate(64, 6, &mut rng);
        let min = hm.min_value();
        let max = hm.max_value();
        assert!(min >= -0.01, "min should be ~0: {min}");
        assert!(max <= 1.01, "max should be ~1: {max}");
    }
}
