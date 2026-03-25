//! Progressive rendering — low-res first, refine over multiple frames.

use super::mandelbrot::{MandelbrotRenderer, MandelbrotParams, FractalPixel};

/// Progressive fractal renderer: renders at increasing resolution over multiple frames.
pub struct ProgressiveRenderer {
    pub target_width: u32,
    pub target_height: u32,
    pub current_level: u32,
    pub max_level: u32,
    pub pixels: Vec<FractalPixel>,
    pub complete: bool,
}

impl ProgressiveRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let max_level = ((width.max(height) as f32).log2().ceil() as u32).max(2);
        Self { target_width: width, target_height: height, current_level: 0, max_level, pixels: Vec::new(), complete: false }
    }

    /// Render the next refinement level. Returns true if rendering is complete.
    pub fn step(&mut self, params: &MandelbrotParams) -> bool {
        if self.complete { return true; }

        let scale = 1u32 << (self.max_level - self.current_level);
        let w = (self.target_width / scale).max(1);
        let h = (self.target_height / scale).max(1);

        let level_params = MandelbrotParams { width: w, height: h, ..params.clone() };
        self.pixels = MandelbrotRenderer::render(&level_params);

        self.current_level += 1;
        self.complete = self.current_level >= self.max_level;
        self.complete
    }

    /// Current resolution being rendered.
    pub fn current_resolution(&self) -> (u32, u32) {
        let scale = 1u32 << (self.max_level - self.current_level.min(self.max_level));
        ((self.target_width / scale).max(1), (self.target_height / scale).max(1))
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f32 { self.current_level as f32 / self.max_level as f32 }

    /// Reset to start rendering from scratch.
    pub fn reset(&mut self) { self.current_level = 0; self.complete = false; self.pixels.clear(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progressive_refines() {
        let mut pr = ProgressiveRenderer::new(64, 64);
        let params = MandelbrotParams { width: 64, height: 64, max_iter: 50, ..Default::default() };
        let (w1, h1) = pr.current_resolution();
        pr.step(&params);
        let (w2, h2) = pr.current_resolution();
        assert!(w2 >= w1, "Resolution should increase: {w1} → {w2}");
    }
    #[test]
    fn progressive_completes() {
        let mut pr = ProgressiveRenderer::new(16, 16);
        let params = MandelbrotParams { width: 16, height: 16, max_iter: 20, ..Default::default() };
        for _ in 0..20 { if pr.step(&params) { break; } }
        assert!(pr.complete);
    }
}
