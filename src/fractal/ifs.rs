//! Iterated Function Systems (IFS) — Sierpinski, Barnsley fern, custom IFS.

use glam::Vec2;

/// 2D affine transformation: p' = A*p + b.
#[derive(Debug, Clone)]
pub struct AffineTx {
    pub a: f32, pub b: f32, pub c: f32, pub d: f32, pub e: f32, pub f: f32,
    pub probability: f32,
    pub color_index: u8,
}

impl AffineTx {
    pub fn apply(&self, p: Vec2) -> Vec2 {
        Vec2::new(self.a * p.x + self.b * p.y + self.e, self.c * p.x + self.d * p.y + self.f)
    }
}

/// An Iterated Function System.
#[derive(Debug, Clone)]
pub struct IfsSystem {
    pub transforms: Vec<AffineTx>,
    pub name: String,
}

/// Rendered IFS fractal.
pub struct IfsFractal {
    pub points: Vec<Vec2>,
    pub colors: Vec<u8>,
}

impl IfsSystem {
    /// Run the chaos game for `iterations` steps.
    pub fn render(&self, iterations: u32, seed: u64) -> IfsFractal {
        let mut points = Vec::with_capacity(iterations as usize);
        let mut colors = Vec::with_capacity(iterations as usize);
        let mut p = Vec2::ZERO;
        let mut rng = seed;
        let total_prob: f32 = self.transforms.iter().map(|t| t.probability).sum();

        for _ in 0..iterations {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = (rng >> 33) as f32 / (u32::MAX >> 1) as f32 * total_prob;
            let mut cumulative = 0.0;
            let mut tx_idx = 0;
            for (i, tx) in self.transforms.iter().enumerate() {
                cumulative += tx.probability;
                if r <= cumulative { tx_idx = i; break; }
            }
            p = self.transforms[tx_idx].apply(p);
            points.push(p);
            colors.push(self.transforms[tx_idx].color_index);
        }
        IfsFractal { points, colors }
    }

    /// Barnsley fern preset.
    pub fn barnsley_fern() -> Self {
        Self {
            name: "Barnsley Fern".to_string(),
            transforms: vec![
                AffineTx { a: 0.0, b: 0.0, c: 0.0, d: 0.16, e: 0.0, f: 0.0, probability: 0.01, color_index: 0 },
                AffineTx { a: 0.85, b: 0.04, c: -0.04, d: 0.85, e: 0.0, f: 1.6, probability: 0.85, color_index: 1 },
                AffineTx { a: 0.2, b: -0.26, c: 0.23, d: 0.22, e: 0.0, f: 1.6, probability: 0.07, color_index: 2 },
                AffineTx { a: -0.15, b: 0.28, c: 0.26, d: 0.24, e: 0.0, f: 0.44, probability: 0.07, color_index: 3 },
            ],
        }
    }

    /// Sierpinski triangle preset.
    pub fn sierpinski() -> Self {
        Self {
            name: "Sierpinski Triangle".to_string(),
            transforms: vec![
                AffineTx { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e: 0.0, f: 0.0, probability: 1.0, color_index: 0 },
                AffineTx { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e: 0.5, f: 0.0, probability: 1.0, color_index: 1 },
                AffineTx { a: 0.5, b: 0.0, c: 0.0, d: 0.5, e: 0.25, f: 0.5, probability: 1.0, color_index: 2 },
            ],
        }
    }

    /// Sierpinski carpet.
    pub fn sierpinski_carpet() -> Self {
        let mut transforms = Vec::new();
        for i in 0..3 { for j in 0..3 {
            if i == 1 && j == 1 { continue; }
            transforms.push(AffineTx {
                a: 1.0/3.0, b: 0.0, c: 0.0, d: 1.0/3.0,
                e: i as f32 / 3.0, f: j as f32 / 3.0,
                probability: 1.0, color_index: (i * 3 + j) as u8,
            });
        }}
        Self { name: "Sierpinski Carpet".to_string(), transforms }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn barnsley_fern_renders() {
        let ifs = IfsSystem::barnsley_fern();
        let result = ifs.render(10000, 42);
        assert_eq!(result.points.len(), 10000);
        // Fern should be roughly bounded
        let max_y = result.points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(max_y > 5.0 && max_y < 15.0, "Fern y range: {max_y}");
    }
    #[test]
    fn sierpinski_bounded() {
        let ifs = IfsSystem::sierpinski();
        let result = ifs.render(5000, 123);
        for p in &result.points {
            assert!(p.x >= -0.1 && p.x <= 1.1 && p.y >= -0.1 && p.y <= 1.1);
        }
    }
}
