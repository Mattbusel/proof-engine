//! Brownian motion / Wiener process implementations.
//!
//! Provides standard Brownian motion in arbitrary dimensions, 2D convenience
//! wrappers returning `glam::Vec2`, Brownian bridges, fractional Brownian
//! motion, and a glyph-based renderer.

use glam::Vec2;

// ---------------------------------------------------------------------------
// LCG-based RNG
// ---------------------------------------------------------------------------

/// Simple linear congruential generator for reproducible stochastic simulations.
/// Uses the Numerical Recipes constants: a = 1664525, c = 1013904223, m = 2^32.
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a new RNG with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(1) }
    }

    /// Return the next raw u32 value.
    pub fn next_u32(&mut self) -> u32 {
        // LCG step
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.state >> 33) ^ (self.state >> 17)) as u32
    }

    /// Uniform random f64 in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }

    /// Standard normal via Box-Muller transform.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.uniform().max(1e-15);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Normal with given mean and standard deviation.
    pub fn normal_with(&mut self, mean: f64, std_dev: f64) -> f64 {
        mean + std_dev * self.normal()
    }
}

// ---------------------------------------------------------------------------
// BrownianMotion (n-dimensional)
// ---------------------------------------------------------------------------

/// Standard Brownian motion (Wiener process) in arbitrary dimension.
///
/// Each coordinate performs an independent random walk with increments
/// drawn from N(0, variance * dt).
pub struct BrownianMotion {
    /// Number of spatial dimensions.
    pub dimension: usize,
    /// Time step size.
    pub dt: f64,
    /// Per-step variance multiplier (default 1.0 for standard Wiener process).
    pub variance: f64,
}

impl BrownianMotion {
    /// Create a new standard Brownian motion.
    pub fn new(dimension: usize, dt: f64) -> Self {
        Self { dimension, dt, variance: 1.0 }
    }

    /// Create with custom variance.
    pub fn with_variance(dimension: usize, dt: f64, variance: f64) -> Self {
        Self { dimension, dt, variance }
    }

    /// Single step: returns the increment dW as a Vec<f64>.
    pub fn step(&self, rng: &mut Rng) -> Vec<f64> {
        let scale = (self.variance * self.dt).sqrt();
        (0..self.dimension).map(|_| rng.normal() * scale).collect()
    }

    /// Generate a full trajectory of the given number of steps, starting at the origin.
    pub fn path(&self, rng: &mut Rng, steps: usize) -> Vec<Vec<f64>> {
        let mut trajectory = Vec::with_capacity(steps + 1);
        let mut current = vec![0.0; self.dimension];
        trajectory.push(current.clone());
        for _ in 0..steps {
            let dw = self.step(rng);
            for (c, d) in current.iter_mut().zip(dw.iter()) {
                *c += d;
            }
            trajectory.push(current.clone());
        }
        trajectory
    }

    /// Generate a path starting from a given position.
    pub fn path_from(&self, rng: &mut Rng, start: &[f64], steps: usize) -> Vec<Vec<f64>> {
        let mut trajectory = Vec::with_capacity(steps + 1);
        let mut current = start.to_vec();
        trajectory.push(current.clone());
        for _ in 0..steps {
            let dw = self.step(rng);
            for (c, d) in current.iter_mut().zip(dw.iter()) {
                *c += d;
            }
            trajectory.push(current.clone());
        }
        trajectory
    }
}

// ---------------------------------------------------------------------------
// BrownianMotion2D — convenience wrapper returning Vec2
// ---------------------------------------------------------------------------

/// 2D Brownian motion returning `glam::Vec2` positions.
pub struct BrownianMotion2D {
    pub dt: f64,
    pub variance: f64,
}

impl BrownianMotion2D {
    pub fn new(dt: f64) -> Self {
        Self { dt, variance: 1.0 }
    }

    pub fn with_variance(dt: f64, variance: f64) -> Self {
        Self { dt, variance }
    }

    /// Single step increment as Vec2.
    pub fn step(&self, rng: &mut Rng) -> Vec2 {
        let scale = (self.variance * self.dt).sqrt() as f32;
        Vec2::new(rng.normal() as f32 * scale, rng.normal() as f32 * scale)
    }

    /// Full 2D trajectory starting at the origin.
    pub fn path(&self, rng: &mut Rng, steps: usize) -> Vec<Vec2> {
        let mut trajectory = Vec::with_capacity(steps + 1);
        let mut pos = Vec2::ZERO;
        trajectory.push(pos);
        for _ in 0..steps {
            pos += self.step(rng);
            trajectory.push(pos);
        }
        trajectory
    }

    /// Full 2D trajectory starting from a given position.
    pub fn path_from(&self, rng: &mut Rng, start: Vec2, steps: usize) -> Vec<Vec2> {
        let mut trajectory = Vec::with_capacity(steps + 1);
        let mut pos = start;
        trajectory.push(pos);
        for _ in 0..steps {
            pos += self.step(rng);
            trajectory.push(pos);
        }
        trajectory
    }
}

// ---------------------------------------------------------------------------
// BrownianBridge
// ---------------------------------------------------------------------------

/// Brownian bridge: a Brownian motion conditioned to reach a specified endpoint.
///
/// B(t) = (1 - t/T)*start + (t/T)*end + W(t) - (t/T)*W(T)
/// which is implemented by generating a free Wiener path then pinning it.
pub struct BrownianBridge {
    /// Starting value.
    pub start: f64,
    /// Ending value (the bridge is conditioned to hit this).
    pub end: f64,
    /// Number of discrete steps.
    pub steps: usize,
}

impl BrownianBridge {
    pub fn new(start: f64, end: f64, steps: usize) -> Self {
        Self { start, end, steps }
    }

    /// Generate the bridge path as a vector of length `steps + 1`.
    pub fn path(&self, rng: &mut Rng) -> Vec<f64> {
        let n = self.steps;
        if n == 0 {
            return vec![self.start];
        }

        let dt = 1.0 / n as f64;
        let scale = dt.sqrt();

        // Generate free Wiener path
        let mut w = Vec::with_capacity(n + 1);
        w.push(0.0);
        for _ in 0..n {
            let prev = *w.last().unwrap();
            w.push(prev + rng.normal() * scale);
        }
        let w_t = w[n];

        // Pin: B(k) = start + (end - start) * (k/n) + W(k) - (k/n) * W(T)
        let mut bridge = Vec::with_capacity(n + 1);
        for k in 0..=n {
            let t_frac = k as f64 / n as f64;
            let val = self.start + (self.end - self.start) * t_frac + w[k] - t_frac * w_t;
            bridge.push(val);
        }
        bridge
    }

    /// Generate a 2D bridge path (each coordinate independently bridged).
    pub fn path_2d(&self, rng: &mut Rng, start: Vec2, end: Vec2) -> Vec<Vec2> {
        let bx = BrownianBridge::new(start.x as f64, end.x as f64, self.steps);
        let by = BrownianBridge::new(start.y as f64, end.y as f64, self.steps);
        let px = bx.path(rng);
        let py = by.path(rng);
        px.iter()
            .zip(py.iter())
            .map(|(&x, &y)| Vec2::new(x as f32, y as f32))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Fractional Brownian Motion
// ---------------------------------------------------------------------------

/// Generate fractional Brownian motion with Hurst parameter H in (0, 1).
///
/// Uses the Cholesky decomposition of the covariance matrix:
/// Cov(B_H(s), B_H(t)) = 0.5 * (|s|^{2H} + |t|^{2H} - |t-s|^{2H})
pub fn fractional_brownian(h: f64, steps: usize, rng: &mut Rng) -> Vec<f64> {
    if steps == 0 {
        return vec![0.0];
    }

    let n = steps;
    let two_h = 2.0 * h;

    // Build the covariance matrix
    let mut cov = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            let si = (i + 1) as f64;
            let sj = (j + 1) as f64;
            let diff = (si - sj).abs();
            cov[i][j] = 0.5 * (si.powf(two_h) + sj.powf(two_h) - diff.powf(two_h));
        }
    }

    // Cholesky decomposition (covariance matrix is symmetric positive definite)
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i][k] * l[j][k];
            }
            if i == j {
                let diag = cov[i][i] - sum;
                l[i][j] = if diag > 0.0 { diag.sqrt() } else { 0.0 };
            } else {
                l[i][j] = if l[j][j].abs() > 1e-15 {
                    (cov[i][j] - sum) / l[j][j]
                } else {
                    0.0
                };
            }
        }
    }

    // Generate independent normals
    let z: Vec<f64> = (0..n).map(|_| rng.normal()).collect();

    // Multiply L * z to get correlated increments, then cumsum
    let mut path = Vec::with_capacity(n + 1);
    path.push(0.0);
    for i in 0..n {
        let mut val = 0.0;
        for j in 0..=i {
            val += l[i][j] * z[j];
        }
        path.push(val);
    }
    path
}

// ---------------------------------------------------------------------------
// BrownianRenderer
// ---------------------------------------------------------------------------

/// Render a Brownian path as a connected trail of glyphs.
pub struct BrownianRenderer {
    /// Character to use for the trail.
    pub character: char,
    /// Base color (r, g, b, a).
    pub color: [f32; 4],
    /// Glow radius for each glyph.
    pub glow_radius: f32,
}

impl BrownianRenderer {
    pub fn new() -> Self {
        Self {
            character: '·',
            color: [0.4, 0.8, 1.0, 1.0],
            glow_radius: 0.6,
        }
    }

    pub fn with_character(mut self, ch: char) -> Self {
        self.character = ch;
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    /// Convert a 2D path into glyph data: Vec of (position, character, color).
    pub fn render_path_2d(&self, path: &[Vec2]) -> Vec<(Vec2, char, [f32; 4])> {
        let len = path.len();
        if len == 0 {
            return Vec::new();
        }

        let mut glyphs = Vec::with_capacity(len);
        for (i, &pos) in path.iter().enumerate() {
            // Fade alpha along the trail
            let alpha = (i as f32 / len as f32) * self.color[3];
            let color = [self.color[0], self.color[1], self.color[2], alpha];
            glyphs.push((pos, self.character, color));
        }
        glyphs
    }

    /// Convert a generic n-dimensional path (projected to first 2 dims) into glyph data.
    pub fn render_path_nd(&self, path: &[Vec<f64>]) -> Vec<(Vec2, char, [f32; 4])> {
        let len = path.len();
        if len == 0 {
            return Vec::new();
        }

        let mut glyphs = Vec::with_capacity(len);
        for (i, point) in path.iter().enumerate() {
            let x = point.first().copied().unwrap_or(0.0) as f32;
            let y = point.get(1).copied().unwrap_or(0.0) as f32;
            let pos = Vec2::new(x, y);
            let alpha = (i as f32 / len as f32) * self.color[3];
            let color = [self.color[0], self.color[1], self.color[2], alpha];
            glyphs.push((pos, self.character, color));
        }
        glyphs
    }

    /// Render a 1D path as a line chart (index on x-axis, value on y-axis).
    pub fn render_path_1d(&self, path: &[f64], x_scale: f32, y_scale: f32) -> Vec<(Vec2, char, [f32; 4])> {
        let len = path.len();
        if len == 0 {
            return Vec::new();
        }

        let mut glyphs = Vec::with_capacity(len);
        for (i, &val) in path.iter().enumerate() {
            let pos = Vec2::new(i as f32 * x_scale, val as f32 * y_scale);
            let alpha = ((i as f32 + 1.0) / len as f32) * self.color[3];
            let color = [self.color[0], self.color[1], self.color[2], alpha];
            glyphs.push((pos, self.character, color));
        }
        glyphs
    }
}

impl Default for BrownianRenderer {
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
    fn test_rng_uniform_range() {
        let mut rng = Rng::new(42);
        for _ in 0..1000 {
            let u = rng.uniform();
            assert!(u >= 0.0 && u < 1.0, "uniform out of range: {}", u);
        }
    }

    #[test]
    fn test_rng_normal_mean_and_variance() {
        let mut rng = Rng::new(123);
        let n = 50_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "normal mean too far from 0: {}", mean);
        assert!((var - 1.0).abs() < 0.1, "normal variance too far from 1: {}", var);
    }

    #[test]
    fn test_brownian_motion_dimensions() {
        let bm = BrownianMotion::new(3, 0.01);
        let mut rng = Rng::new(7);
        let step = bm.step(&mut rng);
        assert_eq!(step.len(), 3);

        let path = bm.path(&mut rng, 100);
        assert_eq!(path.len(), 101);
        assert_eq!(path[0], vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_brownian_variance_scales_with_time() {
        // For standard BM, Var(W(t)) = t. After N steps of size dt, t = N*dt.
        let dt = 0.01;
        let steps = 1000; // t = 10.0
        let trials = 2000;
        let mut rng = Rng::new(999);
        let bm = BrownianMotion::new(1, dt);

        let mut endpoints = Vec::with_capacity(trials);
        for _ in 0..trials {
            let path = bm.path(&mut rng, steps);
            endpoints.push(path[steps][0]);
        }

        let mean = endpoints.iter().sum::<f64>() / trials as f64;
        let var = endpoints.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / trials as f64;
        let expected_var = dt * steps as f64; // = 10.0

        assert!(mean.abs() < 0.5, "BM mean should be ~0, got {}", mean);
        assert!(
            (var - expected_var).abs() < 2.0,
            "BM variance should be ~{}, got {}",
            expected_var,
            var
        );
    }

    #[test]
    fn test_brownian_motion_2d() {
        let bm = BrownianMotion2D::new(0.01);
        let mut rng = Rng::new(55);
        let path = bm.path(&mut rng, 200);
        assert_eq!(path.len(), 201);
        assert_eq!(path[0], Vec2::ZERO);
    }

    #[test]
    fn test_brownian_bridge_endpoints() {
        let bridge = BrownianBridge::new(0.0, 5.0, 500);
        let mut rng = Rng::new(42);
        let path = bridge.path(&mut rng);
        assert_eq!(path.len(), 501);
        assert!((path[0] - 0.0).abs() < 1e-10, "bridge should start at 0");
        assert!((path[500] - 5.0).abs() < 1e-10, "bridge should end at 5, got {}", path[500]);
    }

    #[test]
    fn test_fractional_brownian_standard() {
        // H = 0.5 should behave like standard Brownian motion
        let mut rng = Rng::new(42);
        let path = fractional_brownian(0.5, 100, &mut rng);
        assert_eq!(path.len(), 101);
        assert_eq!(path[0], 0.0);
    }

    #[test]
    fn test_fractional_brownian_persistent() {
        // H > 0.5 should produce positively correlated increments (persistent)
        let mut rng = Rng::new(42);
        let path = fractional_brownian(0.8, 200, &mut rng);
        assert_eq!(path.len(), 201);
        assert_eq!(path[0], 0.0);
        // Just verify it runs and produces values
        assert!(path.iter().any(|&v| v != 0.0));
    }

    #[test]
    fn test_fractional_brownian_antipersistent() {
        // H < 0.5 should produce negatively correlated increments
        let mut rng = Rng::new(42);
        let path = fractional_brownian(0.2, 200, &mut rng);
        assert_eq!(path.len(), 201);
        assert_eq!(path[0], 0.0);
    }

    #[test]
    fn test_brownian_renderer() {
        let renderer = BrownianRenderer::new().with_character('*');
        let bm = BrownianMotion2D::new(0.01);
        let mut rng = Rng::new(10);
        let path = bm.path(&mut rng, 50);
        let glyphs = renderer.render_path_2d(&path);
        assert_eq!(glyphs.len(), 51);
        assert_eq!(glyphs[0].1, '*');
    }

    #[test]
    fn test_brownian_bridge_2d() {
        let bridge = BrownianBridge::new(0.0, 0.0, 100);
        let mut rng = Rng::new(77);
        let start = Vec2::new(1.0, 2.0);
        let end = Vec2::new(5.0, 8.0);
        let path = bridge.path_2d(&mut rng, start, end);
        assert_eq!(path.len(), 101);
        assert!((path[0].x - 1.0).abs() < 1e-4);
        assert!((path[0].y - 2.0).abs() < 1e-4);
        assert!((path[100].x - 5.0).abs() < 1e-4);
        assert!((path[100].y - 8.0).abs() < 1e-4);
    }

    #[test]
    fn test_render_path_1d() {
        let renderer = BrownianRenderer::new();
        let path = vec![0.0, 1.0, -0.5, 2.0];
        let glyphs = renderer.render_path_1d(&path, 1.0, 1.0);
        assert_eq!(glyphs.len(), 4);
        assert!((glyphs[0].0.x - 0.0).abs() < 1e-5);
        assert!((glyphs[2].0.y - (-0.5)).abs() < 1e-5);
    }
}
