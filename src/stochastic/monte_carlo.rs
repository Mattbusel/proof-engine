//! Monte Carlo simulation framework.
//!
//! Generic simulation runner, histogram, importance sampling, pi estimation,
//! and numerical integration.

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// Histogram
// ---------------------------------------------------------------------------

/// Histogram with uniform bins.
pub struct Histogram {
    pub bins: Vec<u32>,
    pub min: f64,
    pub max: f64,
    pub bin_count: usize,
}

impl Histogram {
    /// Create from samples with the given number of bins.
    pub fn from_samples(samples: &[f64], bin_count: usize) -> Self {
        if samples.is_empty() || bin_count == 0 {
            return Self {
                bins: vec![0; bin_count.max(1)],
                min: 0.0,
                max: 1.0,
                bin_count: bin_count.max(1),
            };
        }

        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = (max - min).max(1e-15);
        let mut bins = vec![0u32; bin_count];

        for &s in samples {
            let idx = ((s - min) / range * bin_count as f64) as usize;
            let idx = idx.min(bin_count - 1);
            bins[idx] += 1;
        }

        Self { bins, min, max, bin_count }
    }

    /// Create with explicit bounds.
    pub fn from_samples_bounded(samples: &[f64], bin_count: usize, min: f64, max: f64) -> Self {
        let range = (max - min).max(1e-15);
        let mut bins = vec![0u32; bin_count];
        for &s in samples {
            if s >= min && s <= max {
                let idx = ((s - min) / range * bin_count as f64) as usize;
                let idx = idx.min(bin_count - 1);
                bins[idx] += 1;
            }
        }
        Self { bins, min, max, bin_count }
    }

    /// Bin width.
    pub fn bin_width(&self) -> f64 {
        (self.max - self.min) / self.bin_count as f64
    }

    /// Center of bin i.
    pub fn bin_center(&self, i: usize) -> f64 {
        self.min + (i as f64 + 0.5) * self.bin_width()
    }

    /// Normalized density (area = 1) for bin i.
    pub fn density(&self, i: usize) -> f64 {
        let total: u32 = self.bins.iter().sum();
        if total == 0 {
            return 0.0;
        }
        self.bins[i] as f64 / (total as f64 * self.bin_width())
    }

    /// Maximum bin count.
    pub fn max_count(&self) -> u32 {
        self.bins.iter().cloned().max().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// MonteCarloResult
// ---------------------------------------------------------------------------

/// Result of a Monte Carlo simulation.
pub struct MonteCarloResult<T> {
    pub mean: f64,
    pub variance: f64,
    pub std_dev: f64,
    pub confidence_interval: (f64, f64),
    pub histogram: Histogram,
    pub samples: Vec<T>,
}

impl<T> MonteCarloResult<T> {
    /// Standard error of the mean.
    pub fn standard_error(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.std_dev / (self.samples.len() as f64).sqrt()
    }
}

// ---------------------------------------------------------------------------
// MonteCarloSim
// ---------------------------------------------------------------------------

/// Generic Monte Carlo simulation framework.
pub struct MonteCarloSim<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone> MonteCarloSim<T> {
    /// Run `trials` independent simulations, extracting a f64 value from each.
    /// Returns full statistics.
    pub fn run<F>(trials: usize, mut trial_fn: F) -> MonteCarloResult<T>
    where
        F: FnMut(usize) -> (T, f64),
    {
        let mut samples = Vec::with_capacity(trials);
        let mut values = Vec::with_capacity(trials);

        for i in 0..trials {
            let (sample, value) = trial_fn(i);
            samples.push(sample);
            values.push(value);
        }

        let n = trials as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        // 95% confidence interval using z = 1.96
        let se = std_dev / n.sqrt();
        let confidence_interval = (mean - 1.96 * se, mean + 1.96 * se);

        let histogram = Histogram::from_samples(&values, 50);

        MonteCarloResult {
            mean,
            variance,
            std_dev,
            confidence_interval,
            histogram,
            samples,
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience functions
// ---------------------------------------------------------------------------

/// Estimate pi using the classic Monte Carlo method:
/// sample uniform points in [0,1]^2, count fraction inside unit circle.
pub fn estimate_pi(trials: usize, rng: &mut Rng) -> f64 {
    let mut inside = 0usize;
    for _ in 0..trials {
        let x = rng.uniform();
        let y = rng.uniform();
        if x * x + y * y <= 1.0 {
            inside += 1;
        }
    }
    4.0 * inside as f64 / trials as f64
}

/// Monte Carlo integration of f over [a, b].
/// Estimate = (b - a) * mean(f(x)) for x uniform in [a, b].
pub fn integrate(f: &dyn Fn(f64) -> f64, a: f64, b: f64, trials: usize, rng: &mut Rng) -> f64 {
    let range = b - a;
    let sum: f64 = (0..trials)
        .map(|_| {
            let x = a + rng.uniform() * range;
            f(x)
        })
        .sum();
    range * sum / trials as f64
}

/// Monte Carlo integration in 2D: integral of f over [a1,b1] x [a2,b2].
pub fn integrate_2d(
    f: &dyn Fn(f64, f64) -> f64,
    a1: f64, b1: f64,
    a2: f64, b2: f64,
    trials: usize,
    rng: &mut Rng,
) -> f64 {
    let area = (b1 - a1) * (b2 - a2);
    let sum: f64 = (0..trials)
        .map(|_| {
            let x = a1 + rng.uniform() * (b1 - a1);
            let y = a2 + rng.uniform() * (b2 - a2);
            f(x, y)
        })
        .sum();
    area * sum / trials as f64
}

/// Importance sampling: estimate E_target[h(x)] using proposal distribution.
///
/// * `target_density` - target PDF (unnormalized is fine if ratio is used)
/// * `proposal_sample` - function that samples from proposal distribution
/// * `proposal_density` - proposal PDF
/// * `h` - function to compute expectation of
/// * `trials` - number of samples
pub fn importance_sampling(
    target_density: &dyn Fn(f64) -> f64,
    proposal_sample: &dyn Fn(&mut Rng) -> f64,
    proposal_density: &dyn Fn(f64) -> f64,
    h: &dyn Fn(f64) -> f64,
    trials: usize,
    rng: &mut Rng,
) -> f64 {
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for _ in 0..trials {
        let x = proposal_sample(rng);
        let w = target_density(x) / proposal_density(x).max(1e-15);
        weighted_sum += w * h(x);
        weight_sum += w;
    }

    if weight_sum.abs() < 1e-15 {
        0.0
    } else {
        weighted_sum / weight_sum
    }
}

/// Compute running mean for convergence analysis.
pub fn running_mean(values: &[f64]) -> Vec<f64> {
    let mut means = Vec::with_capacity(values.len());
    let mut sum = 0.0;
    for (i, &v) in values.iter().enumerate() {
        sum += v;
        means.push(sum / (i + 1) as f64);
    }
    means
}

// ---------------------------------------------------------------------------
// MonteCarloRenderer
// ---------------------------------------------------------------------------

/// Render histogram and convergence plot as glyphs.
pub struct MonteCarloRenderer {
    pub bar_character: char,
    pub bar_color: [f32; 4],
    pub convergence_character: char,
    pub convergence_color: [f32; 4],
    pub x_scale: f32,
    pub y_scale: f32,
}

impl MonteCarloRenderer {
    pub fn new() -> Self {
        Self {
            bar_character: '█',
            bar_color: [0.3, 0.8, 0.5, 0.9],
            convergence_character: '·',
            convergence_color: [1.0, 0.6, 0.2, 1.0],
            x_scale: 0.5,
            y_scale: 0.1,
        }
    }

    /// Render a histogram as vertical bars of glyphs.
    pub fn render_histogram(&self, hist: &Histogram) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = Vec::new();
        let max_count = hist.max_count().max(1) as f32;

        for (i, &count) in hist.bins.iter().enumerate() {
            let x = i as f32 * self.x_scale;
            let height = (count as f32 / max_count * 20.0) as usize;
            for h in 0..height {
                glyphs.push((
                    Vec2::new(x, h as f32 * self.y_scale),
                    self.bar_character,
                    self.bar_color,
                ));
            }
        }
        glyphs
    }

    /// Render a convergence plot (running mean vs. trial number).
    pub fn render_convergence(
        &self,
        running_means: &[f64],
        target: f64,
    ) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = Vec::new();
        let n = running_means.len();
        let x_step = if n > 500 { n / 500 } else { 1 };

        for i in (0..n).step_by(x_step) {
            let x = i as f32 * 0.01;
            let y = running_means[i] as f32;
            glyphs.push((Vec2::new(x, y), self.convergence_character, self.convergence_color));
        }

        // Target line
        let target_color = [1.0, 0.2, 0.2, 0.5];
        for i in (0..n).step_by(x_step.max(1) * 2) {
            let x = i as f32 * 0.01;
            glyphs.push((Vec2::new(x, target as f32), '─', target_color));
        }

        glyphs
    }
}

impl Default for MonteCarloRenderer {
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
    fn test_estimate_pi() {
        let mut rng = Rng::new(42);
        let pi = estimate_pi(100_000, &mut rng);
        assert!(
            (pi - std::f64::consts::PI).abs() < 0.05,
            "pi estimate {} should be close to {}",
            pi,
            std::f64::consts::PI
        );
    }

    #[test]
    fn test_integrate_constant() {
        let mut rng = Rng::new(42);
        // integral of 5 from 0 to 2 = 10
        let result = integrate(&|_x| 5.0, 0.0, 2.0, 50_000, &mut rng);
        assert!(
            (result - 10.0).abs() < 0.1,
            "integral of 5 over [0,2] should be 10, got {}",
            result
        );
    }

    #[test]
    fn test_integrate_linear() {
        let mut rng = Rng::new(42);
        // integral of x from 0 to 1 = 0.5
        let result = integrate(&|x| x, 0.0, 1.0, 100_000, &mut rng);
        assert!(
            (result - 0.5).abs() < 0.01,
            "integral of x over [0,1] should be 0.5, got {}",
            result
        );
    }

    #[test]
    fn test_integrate_quadratic() {
        let mut rng = Rng::new(42);
        // integral of x^2 from 0 to 1 = 1/3
        let result = integrate(&|x| x * x, 0.0, 1.0, 100_000, &mut rng);
        assert!(
            (result - 1.0 / 3.0).abs() < 0.01,
            "integral of x^2 over [0,1] should be 1/3, got {}",
            result
        );
    }

    #[test]
    fn test_integrate_2d() {
        let mut rng = Rng::new(42);
        // integral of 1 over [0,1]x[0,1] = 1
        let result = integrate_2d(&|_x, _y| 1.0, 0.0, 1.0, 0.0, 1.0, 50_000, &mut rng);
        assert!((result - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_monte_carlo_sim() {
        let mut rng = Rng::new(42);
        let result = MonteCarloSim::<f64>::run(10_000, |_i| {
            let x = rng.uniform();
            (x, x)
        });
        // Mean of U[0,1] = 0.5
        assert!(
            (result.mean - 0.5).abs() < 0.02,
            "mean should be ~0.5, got {}",
            result.mean
        );
        // Variance of U[0,1] = 1/12 ≈ 0.0833
        assert!(
            (result.variance - 1.0 / 12.0).abs() < 0.01,
            "variance should be ~1/12, got {}",
            result.variance
        );
    }

    #[test]
    fn test_histogram() {
        let samples: Vec<f64> = (0..1000).map(|i| i as f64 / 1000.0).collect();
        let hist = Histogram::from_samples(&samples, 10);
        assert_eq!(hist.bins.len(), 10);
        // Each bin should have ~100 samples
        for &count in &hist.bins {
            assert!(count >= 80 && count <= 120, "bin count {} out of range", count);
        }
    }

    #[test]
    fn test_histogram_density_integrates_to_one() {
        let mut rng = Rng::new(42);
        let samples: Vec<f64> = (0..10_000).map(|_| rng.normal()).collect();
        let hist = Histogram::from_samples(&samples, 50);
        let total: f64 = (0..hist.bin_count)
            .map(|i| hist.density(i) * hist.bin_width())
            .sum();
        assert!(
            (total - 1.0).abs() < 0.05,
            "histogram density should integrate to ~1, got {}",
            total
        );
    }

    #[test]
    fn test_running_mean() {
        let values = vec![1.0, 3.0, 5.0, 7.0];
        let means = running_mean(&values);
        assert!((means[0] - 1.0).abs() < 1e-10);
        assert!((means[1] - 2.0).abs() < 1e-10);
        assert!((means[2] - 3.0).abs() < 1e-10);
        assert!((means[3] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_importance_sampling() {
        let mut rng = Rng::new(42);
        // Estimate E[x] where x ~ N(0,1), using proposal N(0,2)
        let result = importance_sampling(
            &|x| (-0.5 * x * x).exp(), // target: standard normal (unnormalized)
            &|rng: &mut Rng| rng.normal() * 2.0, // proposal: N(0, 4)
            &|x| (-x * x / 8.0).exp(), // proposal density (unnormalized)
            &|x| x * x, // h(x) = x^2, so E[x^2] = 1
            50_000,
            &mut rng,
        );
        assert!(
            (result - 1.0).abs() < 0.2,
            "IS estimate of E[X^2] should be ~1, got {}",
            result
        );
    }

    #[test]
    fn test_confidence_interval() {
        let mut rng = Rng::new(42);
        let result = MonteCarloSim::<f64>::run(10_000, |_| {
            let x = rng.normal();
            (x, x)
        });
        // 95% CI should contain 0
        assert!(result.confidence_interval.0 < 0.0);
        assert!(result.confidence_interval.1 > 0.0);
    }

    #[test]
    fn test_renderer_histogram() {
        let hist = Histogram::from_samples(&[1.0, 2.0, 3.0, 2.0, 2.0], 3);
        let renderer = MonteCarloRenderer::new();
        let glyphs = renderer.render_histogram(&hist);
        assert!(!glyphs.is_empty());
    }

    #[test]
    fn test_pi_convergence() {
        let mut rng = Rng::new(42);
        let values: Vec<f64> = (0..10_000)
            .map(|_| {
                let x = rng.uniform();
                let y = rng.uniform();
                if x * x + y * y <= 1.0 { 4.0 } else { 0.0 }
            })
            .collect();
        let means = running_mean(&values);
        // Later estimates should be closer to pi than earlier ones
        let early_error = (means[100] - std::f64::consts::PI).abs();
        let late_error = (means[9999] - std::f64::consts::PI).abs();
        assert!(
            late_error <= early_error + 0.5,
            "convergence: late error {} should generally be <= early error {}",
            late_error,
            early_error
        );
    }
}
