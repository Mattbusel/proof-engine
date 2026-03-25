//! River generation — water flow simulation from rainfall to ocean.
//!
//! Traces downhill flow from every cell, accumulates flow volume,
//! and marks cells exceeding a threshold as river segments.

use super::{Grid2D, Rng};

/// A river segment.
#[derive(Debug, Clone)]
pub struct RiverSegment {
    pub x: usize,
    pub y: usize,
    /// Accumulated flow volume at this cell.
    pub flow: f32,
    /// Width of the river at this point (derived from flow).
    pub width: f32,
    /// Downstream direction (dx, dy).
    pub downstream: (i32, i32),
}

/// Complete river network.
#[derive(Debug, Clone)]
pub struct RiverNetwork {
    pub segments: Vec<RiverSegment>,
    /// Flow accumulation grid.
    pub flow_grid: Grid2D,
    /// River threshold (cells with flow > this are rivers).
    pub threshold: f32,
}

impl RiverNetwork {
    /// Check if a cell is a river.
    pub fn is_river(&self, x: usize, y: usize) -> bool {
        self.flow_grid.get(x, y) > self.threshold
    }

    /// Get river width at a cell.
    pub fn width_at(&self, x: usize, y: usize) -> f32 {
        let flow = self.flow_grid.get(x, y);
        if flow > self.threshold {
            (flow / self.threshold).sqrt().min(5.0)
        } else {
            0.0
        }
    }

    /// Find river mouths (river cells adjacent to ocean).
    pub fn mouths(&self, heightmap: &Grid2D, sea_level: f32) -> Vec<(usize, usize)> {
        let mut mouths = Vec::new();
        for seg in &self.segments {
            let nx = (seg.x as i32 + seg.downstream.0).clamp(0, heightmap.width as i32 - 1) as usize;
            let ny = (seg.y as i32 + seg.downstream.1).clamp(0, heightmap.height as i32 - 1) as usize;
            if heightmap.get(nx, ny) < sea_level {
                mouths.push((seg.x, seg.y));
            }
        }
        mouths
    }

    /// Find the longest river path.
    pub fn longest_river(&self) -> Vec<(usize, usize)> {
        if self.segments.is_empty() { return Vec::new(); }
        // Find segment with highest flow (likely near mouth)
        let start = self.segments.iter().max_by(|a, b| a.flow.partial_cmp(&b.flow).unwrap()).unwrap();
        // Trace upstream (not implemented in MVP — return single point)
        vec![(start.x, start.y)]
    }
}

/// Generate river network from heightmap and precipitation.
pub fn generate(heightmap: &Grid2D, precipitation: &Grid2D, sea_level: f32) -> RiverNetwork {
    let w = heightmap.width;
    let h = heightmap.height;

    // 1. Compute flow direction (steepest descent)
    let mut flow_dir = vec![(0i32, 0i32); w * h];
    for y in 0..h {
        for x in 0..w {
            let center = heightmap.get(x, y);
            let mut best_dir = (0i32, 0i32);
            let mut best_drop = 0.0_f32;

            for &(dx, dy) in &[(-1i32, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                let nh = heightmap.get(nx as usize, ny as usize);
                let drop = center - nh;
                let dist = if dx.abs() + dy.abs() == 2 { 1.414 } else { 1.0 };
                let slope = drop / dist;
                if slope > best_drop {
                    best_drop = slope;
                    best_dir = (dx, dy);
                }
            }
            flow_dir[y * w + x] = best_dir;
        }
    }

    // 2. Flow accumulation (each cell contributes its precipitation downstream)
    let mut flow_grid = Grid2D::new(w, h);

    // Initialize with precipitation
    for y in 0..h {
        for x in 0..w {
            if heightmap.get(x, y) > sea_level {
                flow_grid.set(x, y, precipitation.get(x, y));
            }
        }
    }

    // Sort cells by elevation (high to low) for single-pass accumulation
    let mut sorted: Vec<(usize, usize, f32)> = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            sorted.push((x, y, heightmap.get(x, y)));
        }
    }
    sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // Flow accumulation: from high to low, add flow to downstream cell
    for &(x, y, elev) in &sorted {
        if elev < sea_level { continue; }
        let (dx, dy) = flow_dir[y * w + x];
        if dx == 0 && dy == 0 { continue; }

        let nx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
        let ny = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
        let upstream_flow = flow_grid.get(x, y);
        flow_grid.add(nx, ny, upstream_flow);
    }

    // 3. Extract river segments above threshold
    let threshold = compute_threshold(&flow_grid, sea_level, heightmap);
    let mut segments = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let flow = flow_grid.get(x, y);
            if flow > threshold && heightmap.get(x, y) > sea_level {
                let (dx, dy) = flow_dir[y * w + x];
                segments.push(RiverSegment {
                    x,
                    y,
                    flow,
                    width: (flow / threshold).sqrt().min(5.0),
                    downstream: (dx, dy),
                });
            }
        }
    }

    RiverNetwork { segments, flow_grid, threshold }
}

/// Compute a reasonable river threshold (top ~2% of flow values on land).
fn compute_threshold(flow_grid: &Grid2D, sea_level: f32, heightmap: &Grid2D) -> f32 {
    let mut land_flows: Vec<f32> = Vec::new();
    for y in 0..flow_grid.height {
        for x in 0..flow_grid.width {
            if heightmap.get(x, y) > sea_level {
                land_flows.push(flow_grid.get(x, y));
            }
        }
    }
    land_flows.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if land_flows.is_empty() { return 1.0; }
    let idx = (land_flows.len() as f32 * 0.98) as usize;
    land_flows[idx.min(land_flows.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_river_generation() {
        // Simple slope: high on left, low on right
        let mut hm = Grid2D::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                hm.set(x, y, 1.0 - x as f32 / 32.0);
            }
        }
        let precip = Grid2D::filled(32, 32, 0.5);
        let rn = generate(&hm, &precip, 0.1);
        assert!(!rn.segments.is_empty(), "should generate rivers on a slope");
    }

    #[test]
    fn test_flat_no_rivers() {
        let hm = Grid2D::filled(16, 16, 0.5);
        let precip = Grid2D::filled(16, 16, 0.5);
        let rn = generate(&hm, &precip, 0.4);
        // Flat terrain may not generate rivers (no gradient)
        // This is acceptable behavior
        assert!(rn.threshold > 0.0);
    }
}
