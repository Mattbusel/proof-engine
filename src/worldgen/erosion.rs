//! Hydraulic and thermal erosion simulation.
//!
//! Hydraulic erosion: virtual raindrops flow downhill, picking up sediment
//! from steep slopes and depositing it in flat areas. Creates realistic
//! valleys, ridges, and drainage patterns.
//!
//! Thermal erosion: material slumps from steep slopes to neighbors when
//! the slope exceeds a talus angle threshold.

use super::{Grid2D, Rng};

/// Erosion parameters.
#[derive(Debug, Clone)]
pub struct ErosionParams {
    /// Fraction of steps that are hydraulic (vs thermal).
    pub hydraulic_ratio: f32,
    /// Maximum steps per raindrop before it stops.
    pub max_drop_lifetime: usize,
    /// Sediment capacity multiplier.
    pub capacity_mult: f32,
    /// Erosion rate (how fast material is picked up).
    pub erosion_rate: f32,
    /// Deposition rate (how fast sediment is dropped).
    pub deposition_rate: f32,
    /// Gravity strength.
    pub gravity: f32,
    /// Evaporation rate per step.
    pub evaporation: f32,
    /// Minimum slope for erosion to occur.
    pub min_slope: f32,
    /// Talus angle for thermal erosion (radians).
    pub talus_angle: f32,
    /// Thermal erosion rate.
    pub thermal_rate: f32,
    /// Inertia (how much the drop retains its previous direction).
    pub inertia: f32,
    /// Initial water volume per drop.
    pub initial_water: f32,
    /// Initial velocity per drop.
    pub initial_speed: f32,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            hydraulic_ratio: 0.8,
            max_drop_lifetime: 64,
            capacity_mult: 8.0,
            erosion_rate: 0.3,
            deposition_rate: 0.3,
            gravity: 4.0,
            evaporation: 0.01,
            min_slope: 0.01,
            talus_angle: 0.6, // ~34 degrees
            thermal_rate: 0.5,
            inertia: 0.3,
            initial_water: 1.0,
            initial_speed: 1.0,
        }
    }
}

/// A simulated raindrop for hydraulic erosion.
struct Drop {
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    speed: f32,
    water: f32,
    sediment: f32,
}

/// Run erosion on a heightmap.
pub fn erode(mut heightmap: Grid2D, iterations: usize, rng: &mut Rng) -> Grid2D {
    let params = ErosionParams::default();
    let w = heightmap.width;
    let h = heightmap.height;

    let hydraulic_iters = (iterations as f32 * params.hydraulic_ratio) as usize;
    let thermal_iters = iterations - hydraulic_iters;

    // Hydraulic erosion
    for _ in 0..hydraulic_iters {
        hydraulic_step(&mut heightmap, rng, &params);
    }

    // Thermal erosion
    for _ in 0..thermal_iters {
        thermal_step(&mut heightmap, &params);
    }

    heightmap
}

/// Run erosion with custom parameters.
pub fn erode_with_params(mut heightmap: Grid2D, iterations: usize, params: &ErosionParams, rng: &mut Rng) -> Grid2D {
    let hydraulic_iters = (iterations as f32 * params.hydraulic_ratio) as usize;
    let thermal_iters = iterations - hydraulic_iters;

    for _ in 0..hydraulic_iters {
        hydraulic_step(&mut heightmap, rng, params);
    }
    for _ in 0..thermal_iters {
        thermal_step(&mut heightmap, params);
    }

    heightmap
}

/// Single hydraulic erosion step: simulate one raindrop.
fn hydraulic_step(grid: &mut Grid2D, rng: &mut Rng, params: &ErosionParams) {
    let w = grid.width as f32;
    let h = grid.height as f32;

    let mut drop = Drop {
        x: rng.range_f32(1.0, w - 2.0),
        y: rng.range_f32(1.0, h - 2.0),
        dir_x: 0.0,
        dir_y: 0.0,
        speed: params.initial_speed,
        water: params.initial_water,
        sediment: 0.0,
    };

    for _ in 0..params.max_drop_lifetime {
        let ix = drop.x as usize;
        let iy = drop.y as usize;
        if ix < 1 || iy < 1 || ix >= grid.width - 1 || iy >= grid.height - 1 {
            break;
        }

        // Compute gradient
        let (gx, gy) = grid.gradient(ix, iy);

        // Update direction with inertia
        drop.dir_x = drop.dir_x * params.inertia - gx * (1.0 - params.inertia);
        drop.dir_y = drop.dir_y * params.inertia - gy * (1.0 - params.inertia);

        // Normalize direction
        let len = (drop.dir_x * drop.dir_x + drop.dir_y * drop.dir_y).sqrt();
        if len < 1e-6 {
            break; // No gradient → pool
        }
        drop.dir_x /= len;
        drop.dir_y /= len;

        // Move
        let new_x = drop.x + drop.dir_x;
        let new_y = drop.y + drop.dir_y;

        if new_x < 1.0 || new_y < 1.0 || new_x >= w - 2.0 || new_y >= h - 2.0 {
            break;
        }

        // Height difference
        let old_h = grid.sample(drop.x, drop.y);
        let new_h = grid.sample(new_x, new_y);
        let dh = new_h - old_h;

        // Sediment capacity
        let slope = (-dh).max(params.min_slope);
        let capacity = slope * drop.speed * drop.water * params.capacity_mult;

        if drop.sediment > capacity || dh > 0.0 {
            // Deposit sediment
            let deposit = if dh > 0.0 {
                // Flowing uphill: deposit enough to fill the pit
                dh.min(drop.sediment)
            } else {
                (drop.sediment - capacity) * params.deposition_rate
            };
            drop.sediment -= deposit;
            grid.add(ix, iy, deposit);
        } else {
            // Erode
            let erode_amount = ((capacity - drop.sediment) * params.erosion_rate).min(-dh);
            drop.sediment += erode_amount;
            grid.add(ix, iy, -erode_amount);
        }

        // Update speed
        drop.speed = (drop.speed * drop.speed + dh.abs() * params.gravity).sqrt();
        drop.water *= 1.0 - params.evaporation;

        drop.x = new_x;
        drop.y = new_y;

        if drop.water < 0.01 {
            break;
        }
    }
}

/// Single thermal erosion step over the entire grid.
fn thermal_step(grid: &mut Grid2D, params: &ErosionParams) {
    let w = grid.width;
    let h = grid.height;
    let talus = params.talus_angle.tan(); // convert angle to max height difference per cell

    let mut transfers = Vec::new();

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let center = grid.get(x, y);
            let mut max_diff = 0.0_f32;
            let mut total_diff = 0.0_f32;
            let mut neighbors = Vec::new();

            for &(nx, ny) in &[(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                let nh = grid.get(nx, ny);
                let diff = center - nh;
                if diff > talus {
                    max_diff = max_diff.max(diff);
                    total_diff += diff - talus;
                    neighbors.push((nx, ny, diff - talus));
                }
            }

            if total_diff > 0.0 {
                let transfer = max_diff * params.thermal_rate * 0.5;
                for (nx, ny, weight) in neighbors {
                    let frac = weight / total_diff;
                    transfers.push((x, y, nx, ny, transfer * frac));
                }
            }
        }
    }

    for (sx, sy, dx, dy, amount) in transfers {
        grid.add(sx, sy, -amount);
        grid.add(dx, dy, amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hill() -> Grid2D {
        let mut g = Grid2D::new(32, 32);
        // Gaussian hill in center
        for y in 0..32 {
            for x in 0..32 {
                let dx = x as f32 - 16.0;
                let dy = y as f32 - 16.0;
                g.set(x, y, (-0.01 * (dx * dx + dy * dy)).exp());
            }
        }
        g
    }

    #[test]
    fn test_hydraulic_erodes() {
        let before = test_hill();
        let peak_before = before.get(16, 16);
        let mut rng = Rng::new(42);
        let after = erode(before, 5000, &mut rng);
        let peak_after = after.get(16, 16);
        assert!(peak_after < peak_before, "erosion should lower the peak");
    }

    #[test]
    fn test_thermal_smooths() {
        let mut g = Grid2D::new(8, 8);
        g.set(4, 4, 10.0); // spike
        let params = ErosionParams::default();
        for _ in 0..100 {
            thermal_step(&mut g, &params);
        }
        assert!(g.get(4, 4) < 10.0, "thermal erosion should reduce spike");
        assert!(g.get(3, 4) > 0.0, "neighbors should gain material");
    }
}
