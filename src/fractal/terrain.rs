//! Fractal terrain generation — diamond-square, midpoint displacement, fBm.

/// Terrain generation parameters.
#[derive(Debug, Clone)]
pub struct TerrainParams {
    pub size: u32,       // Must be 2^n + 1
    pub roughness: f32,  // 0.0 = smooth, 1.0 = very rough
    pub amplitude: f32,  // Initial displacement range
    pub seed: u64,
}
impl Default for TerrainParams {
    fn default() -> Self { Self { size: 129, roughness: 0.5, amplitude: 1.0, seed: 42 } }
}

/// Generated fractal terrain heightmap.
#[derive(Debug, Clone)]
pub struct FractalTerrain {
    pub heightmap: Vec<f32>,
    pub size: u32,
    pub min_height: f32,
    pub max_height: f32,
}

impl FractalTerrain {
    /// Generate terrain using the diamond-square algorithm.
    pub fn diamond_square(params: &TerrainParams) -> Self {
        let size = params.size as usize;
        let mut map = vec![0.0f32; size * size];
        let mut rng = params.seed;

        // Seed corners
        map[0] = rand(&mut rng) * params.amplitude;
        map[size - 1] = rand(&mut rng) * params.amplitude;
        map[(size - 1) * size] = rand(&mut rng) * params.amplitude;
        map[(size - 1) * size + size - 1] = rand(&mut rng) * params.amplitude;

        let mut step = size - 1;
        let mut scale = params.amplitude;

        while step > 1 {
            let half = step / 2;

            // Diamond step
            for y in (0..size - 1).step_by(step) {
                for x in (0..size - 1).step_by(step) {
                    let avg = (map[y * size + x] + map[y * size + x + step]
                        + map[(y + step) * size + x] + map[(y + step) * size + x + step]) * 0.25;
                    map[(y + half) * size + x + half] = avg + rand(&mut rng) * scale;
                }
            }

            // Square step
            for y in (0..size).step_by(half) {
                let x_start = if (y / half) % 2 == 0 { half } else { 0 };
                for x in (x_start..size).step_by(step) {
                    let mut sum = 0.0f32;
                    let mut count = 0;
                    if y >= half { sum += map[(y - half) * size + x]; count += 1; }
                    if y + half < size { sum += map[(y + half) * size + x]; count += 1; }
                    if x >= half { sum += map[y * size + x - half]; count += 1; }
                    if x + half < size { sum += map[y * size + x + half]; count += 1; }
                    map[y * size + x] = sum / count as f32 + rand(&mut rng) * scale;
                }
            }

            step = half;
            scale *= 2.0_f32.powf(-params.roughness);
        }

        let min_h = map.iter().copied().fold(f32::MAX, f32::min);
        let max_h = map.iter().copied().fold(f32::MIN, f32::max);

        Self { heightmap: map, size: params.size, min_height: min_h, max_height: max_h }
    }

    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.heightmap[(y * self.size + x) as usize]
    }

    /// Normalize heights to [0, 1].
    pub fn normalize(&mut self) {
        let range = (self.max_height - self.min_height).max(1e-6);
        for v in &mut self.heightmap { *v = (*v - self.min_height) / range; }
        self.min_height = 0.0;
        self.max_height = 1.0;
    }

    /// Sample height with bilinear interpolation at fractional coordinates.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let fx = u * (self.size - 1) as f32;
        let fy = v * (self.size - 1) as f32;
        let x0 = (fx.floor() as u32).min(self.size - 2);
        let y0 = (fy.floor() as u32).min(self.size - 2);
        let fx = fx.fract();
        let fy = fy.fract();
        let h00 = self.get(x0, y0);
        let h10 = self.get(x0 + 1, y0);
        let h01 = self.get(x0, y0 + 1);
        let h11 = self.get(x0 + 1, y0 + 1);
        let h0 = h00 + (h10 - h00) * fx;
        let h1 = h01 + (h11 - h01) * fx;
        h0 + (h1 - h0) * fy
    }
}

fn rand(rng: &mut u64) -> f32 {
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*rng >> 33) as f32 / (u32::MAX >> 1) as f32 * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diamond_square_generates() {
        let t = FractalTerrain::diamond_square(&TerrainParams { size: 33, ..Default::default() });
        assert_eq!(t.heightmap.len(), 33 * 33);
        assert!(t.max_height > t.min_height);
    }
    #[test]
    fn terrain_sample_in_range() {
        let mut t = FractalTerrain::diamond_square(&TerrainParams::default());
        t.normalize();
        let h = t.sample(0.5, 0.5);
        assert!(h >= 0.0 && h <= 1.0, "h={h}");
    }
}
