//! Lévy flights: heavy-tailed random walks using alpha-stable distributions.
//!
//! Implements the Chambers-Mallows-Stuck algorithm for stable random variates,
//! 2D Lévy flights, and the Cauchy flight special case.

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// Stable distribution sampling
// ---------------------------------------------------------------------------

/// Generate a random variate from a symmetric alpha-stable distribution
/// using the Chambers-Mallows-Stuck (CMS) algorithm.
///
/// * `alpha` - stability parameter in (0, 2]. alpha=2 is Gaussian, alpha=1 is Cauchy.
/// * `rng` - random number generator
///
/// Returns a sample from S(alpha, 0, 1, 0) (symmetric, unit scale).
pub fn stable_random(alpha: f64, rng: &mut Rng) -> f64 {
    assert!(alpha > 0.0 && alpha <= 2.0, "alpha must be in (0, 2]");

    if (alpha - 2.0).abs() < 1e-10 {
        // Gaussian case
        return rng.normal() * std::f64::consts::SQRT_2;
    }

    if (alpha - 1.0).abs() < 1e-10 {
        // Cauchy case: tan(U) where U ~ Uniform(-pi/2, pi/2)
        let u = (rng.uniform() - 0.5) * std::f64::consts::PI;
        return u.tan();
    }

    // General CMS algorithm for symmetric alpha-stable (beta=0)
    let u = (rng.uniform() - 0.5) * std::f64::consts::PI; // U ~ Uniform(-pi/2, pi/2)
    let w = -rng.uniform().max(1e-15).ln(); // W ~ Exp(1)

    let factor = (alpha * u).sin() / (u.cos().powf(1.0 / alpha));
    let tail = ((1.0 - alpha) * u).cos() / w;
    factor * tail.powf((1.0 - alpha) / alpha)
}

/// Generate a stable random variate with given scale parameter.
pub fn stable_random_scaled(alpha: f64, scale: f64, rng: &mut Rng) -> f64 {
    scale * stable_random(alpha, rng)
}

// ---------------------------------------------------------------------------
// LevyFlight
// ---------------------------------------------------------------------------

/// Lévy flight with alpha-stable step lengths.
pub struct LevyFlight {
    /// Stability parameter in (0, 2]. Smaller = heavier tails.
    pub alpha: f64,
    /// Scale parameter controlling typical step size.
    pub scale: f64,
}

impl LevyFlight {
    pub fn new(alpha: f64, scale: f64) -> Self {
        assert!(alpha > 0.0 && alpha <= 2.0);
        Self { alpha, scale }
    }

    /// Generate a single step length (absolute value of stable variate).
    pub fn step_length(&self, rng: &mut Rng) -> f64 {
        stable_random_scaled(self.alpha, self.scale, rng).abs()
    }

    /// Generate a 2D step: random direction with Lévy-distributed length.
    pub fn step_2d(&self, rng: &mut Rng) -> Vec2 {
        let length = self.step_length(rng) as f32;
        let angle = rng.uniform() as f32 * 2.0 * std::f32::consts::PI;
        Vec2::new(length * angle.cos(), length * angle.sin())
    }

    /// Generate a 2D path starting at the origin.
    pub fn path_2d(&self, rng: &mut Rng, steps: usize) -> Vec<Vec2> {
        let mut path = Vec::with_capacity(steps + 1);
        let mut pos = Vec2::ZERO;
        path.push(pos);
        for _ in 0..steps {
            pos += self.step_2d(rng);
            path.push(pos);
        }
        path
    }

    /// Generate a 2D path starting from a given position.
    pub fn path_2d_from(&self, rng: &mut Rng, start: Vec2, steps: usize) -> Vec<Vec2> {
        let mut path = Vec::with_capacity(steps + 1);
        let mut pos = start;
        path.push(pos);
        for _ in 0..steps {
            pos += self.step_2d(rng);
            path.push(pos);
        }
        path
    }

    /// Generate a 1D path.
    pub fn path_1d(&self, rng: &mut Rng, steps: usize) -> Vec<f64> {
        let mut path = Vec::with_capacity(steps + 1);
        let mut x = 0.0;
        path.push(x);
        for _ in 0..steps {
            x += stable_random_scaled(self.alpha, self.scale, rng);
            path.push(x);
        }
        path
    }

    /// Compute step lengths for a path (useful for analysis).
    pub fn step_lengths(&self, rng: &mut Rng, count: usize) -> Vec<f64> {
        (0..count).map(|_| self.step_length(rng)).collect()
    }
}

// ---------------------------------------------------------------------------
// CauchyFlight (alpha = 1)
// ---------------------------------------------------------------------------

/// Cauchy flight: special case of Lévy flight with alpha = 1.
/// The Cauchy distribution has undefined mean and variance (extremely heavy tails).
pub struct CauchyFlight {
    pub scale: f64,
}

impl CauchyFlight {
    pub fn new(scale: f64) -> Self {
        Self { scale }
    }

    /// Generate a Cauchy-distributed random variate.
    pub fn sample(&self, rng: &mut Rng) -> f64 {
        let u = (rng.uniform() - 0.5) * std::f64::consts::PI;
        self.scale * u.tan()
    }

    /// Generate a 2D step.
    pub fn step_2d(&self, rng: &mut Rng) -> Vec2 {
        let length = self.sample(rng).abs() as f32;
        let angle = rng.uniform() as f32 * 2.0 * std::f32::consts::PI;
        Vec2::new(length * angle.cos(), length * angle.sin())
    }

    /// Generate a 2D path.
    pub fn path_2d(&self, rng: &mut Rng, steps: usize) -> Vec<Vec2> {
        let mut path = Vec::with_capacity(steps + 1);
        let mut pos = Vec2::ZERO;
        path.push(pos);
        for _ in 0..steps {
            pos += self.step_2d(rng);
            path.push(pos);
        }
        path
    }

    /// Generate a 1D path.
    pub fn path_1d(&self, rng: &mut Rng, steps: usize) -> Vec<f64> {
        let mut path = Vec::with_capacity(steps + 1);
        let mut x = 0.0;
        path.push(x);
        for _ in 0..steps {
            x += self.sample(rng);
            path.push(x);
        }
        path
    }
}

// ---------------------------------------------------------------------------
// LevyRenderer
// ---------------------------------------------------------------------------

/// Render a Lévy flight path with step-size-dependent glyph size.
pub struct LevyRenderer {
    pub small_character: char,
    pub large_character: char,
    pub color: [f32; 4],
    /// Step length threshold to switch from small to large glyph.
    pub size_threshold: f32,
}

impl LevyRenderer {
    pub fn new() -> Self {
        Self {
            small_character: '·',
            large_character: '●',
            color: [0.9, 0.5, 0.2, 1.0],
            size_threshold: 2.0,
        }
    }

    /// Render a 2D path, using larger glyphs for longer steps.
    pub fn render_path_2d(&self, path: &[Vec2]) -> Vec<(Vec2, char, [f32; 4])> {
        if path.is_empty() {
            return Vec::new();
        }

        let mut glyphs = Vec::with_capacity(path.len());

        // First point
        glyphs.push((path[0], self.small_character, self.color));

        for i in 1..path.len() {
            let step_len = (path[i] - path[i - 1]).length();
            let ch = if step_len > self.size_threshold {
                self.large_character
            } else {
                self.small_character
            };

            // Color intensity based on step size
            let intensity = (step_len / (self.size_threshold * 5.0)).min(1.0);
            let color = [
                self.color[0],
                self.color[1] * (1.0 - intensity * 0.5),
                self.color[2] * (1.0 - intensity),
                self.color[3],
            ];

            glyphs.push((path[i], ch, color));
        }
        glyphs
    }

    /// Render connecting lines between consecutive points.
    pub fn render_path_2d_with_lines(&self, path: &[Vec2]) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = self.render_path_2d(path);

        // Add intermediate points along each segment
        for i in 1..path.len() {
            let from = path[i - 1];
            let to = path[i];
            let dist = (to - from).length();
            let segments = (dist / 0.5).ceil() as usize;
            if segments > 1 && segments < 100 {
                let line_color = [self.color[0] * 0.5, self.color[1] * 0.5, self.color[2] * 0.5, 0.3];
                for s in 1..segments {
                    let t = s as f32 / segments as f32;
                    let pos = from.lerp(to, t);
                    glyphs.push((pos, '·', line_color));
                }
            }
        }
        glyphs
    }

    /// Render a step-length histogram.
    pub fn render_step_histogram(&self, steps: &[f64], bins: usize) -> Vec<(Vec2, char, [f32; 4])> {
        use super::monte_carlo::Histogram;
        let hist = Histogram::from_samples(steps, bins);
        let max_count = hist.max_count().max(1) as f32;
        let mut glyphs = Vec::new();

        for (i, &count) in hist.bins.iter().enumerate() {
            let x = i as f32 * 0.5;
            let height = (count as f32 / max_count * 15.0) as usize;
            for h in 0..height {
                glyphs.push((Vec2::new(x, h as f32 * 0.3), '█', self.color));
            }
        }
        glyphs
    }
}

impl Default for LevyRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_gaussian_case() {
        // alpha = 2 should be Gaussian * sqrt(2)
        let mut rng = Rng::new(42);
        let n = 10_000;
        let samples: Vec<f64> = (0..n).map(|_| stable_random(2.0, &mut rng)).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.1, "Gaussian stable mean should be ~0, got {}", mean);
        // Variance should be ~2 (since we return sqrt(2) * N(0,1))
        assert!((var - 2.0).abs() < 0.3, "Gaussian stable variance should be ~2, got {}", var);
    }

    #[test]
    fn test_cauchy_heavy_tails() {
        // Cauchy distribution should have some very large values
        let cf = CauchyFlight::new(1.0);
        let mut rng = Rng::new(42);
        let samples: Vec<f64> = (0..10_000).map(|_| cf.sample(&mut rng)).collect();
        let max_abs = samples.iter().map(|x| x.abs()).fold(0.0, f64::max);
        assert!(
            max_abs > 10.0,
            "Cauchy should have heavy tails, max |x| = {}",
            max_abs
        );
    }

    #[test]
    fn test_levy_flight_heavy_tails() {
        // With alpha < 2, we should see occasional very large steps
        let lf = LevyFlight::new(1.5, 1.0);
        let mut rng = Rng::new(42);
        let lengths = lf.step_lengths(&mut rng, 10_000);

        let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
        let max_step = lengths.iter().cloned().fold(0.0, f64::max);

        // Max step should be much larger than mean (heavy tails)
        assert!(
            max_step > mean * 5.0,
            "heavy tails: max step {} should be >> mean {}",
            max_step,
            mean
        );
    }

    #[test]
    fn test_levy_2d_path() {
        let lf = LevyFlight::new(1.5, 1.0);
        let mut rng = Rng::new(42);
        let path = lf.path_2d(&mut rng, 100);
        assert_eq!(path.len(), 101);
        assert_eq!(path[0], Vec2::ZERO);
    }

    #[test]
    fn test_levy_1d_path() {
        let lf = LevyFlight::new(1.5, 1.0);
        let mut rng = Rng::new(42);
        let path = lf.path_1d(&mut rng, 100);
        assert_eq!(path.len(), 101);
        assert_eq!(path[0], 0.0);
    }

    #[test]
    fn test_cauchy_path() {
        let cf = CauchyFlight::new(1.0);
        let mut rng = Rng::new(42);
        let path = cf.path_2d(&mut rng, 50);
        assert_eq!(path.len(), 51);
        assert_eq!(path[0], Vec2::ZERO);
    }

    #[test]
    fn test_levy_vs_gaussian_variance() {
        // alpha = 2 (Gaussian) should have finite, bounded increments
        // alpha = 1.2 should have much larger extreme increments
        let mut rng = Rng::new(42);
        let gauss_steps: Vec<f64> = (0..5000)
            .map(|_| stable_random(2.0, &mut rng).abs())
            .collect();
        let levy_steps: Vec<f64> = (0..5000)
            .map(|_| stable_random(1.2, &mut rng).abs())
            .collect();

        let gauss_max = gauss_steps.iter().cloned().fold(0.0, f64::max);
        let levy_max = levy_steps.iter().cloned().fold(0.0, f64::max);

        assert!(
            levy_max > gauss_max,
            "Lévy max {} should generally exceed Gaussian max {}",
            levy_max,
            gauss_max
        );
    }

    #[test]
    fn test_renderer() {
        let lf = LevyFlight::new(1.5, 1.0);
        let mut rng = Rng::new(42);
        let path = lf.path_2d(&mut rng, 50);
        let renderer = LevyRenderer::new();
        let glyphs = renderer.render_path_2d(&path);
        assert_eq!(glyphs.len(), 51);
    }

    #[test]
    fn test_renderer_with_lines() {
        let lf = LevyFlight::new(1.5, 0.5);
        let mut rng = Rng::new(42);
        let path = lf.path_2d(&mut rng, 20);
        let renderer = LevyRenderer::new();
        let glyphs = renderer.render_path_2d_with_lines(&path);
        // Should have more glyphs than just the path points due to line interpolation
        assert!(glyphs.len() >= 21);
    }

    #[test]
    fn test_step_histogram() {
        let lf = LevyFlight::new(1.5, 1.0);
        let mut rng = Rng::new(42);
        let lengths = lf.step_lengths(&mut rng, 500);
        let renderer = LevyRenderer::new();
        let glyphs = renderer.render_step_histogram(&lengths, 20);
        assert!(!glyphs.is_empty());
    }
}
