//! Ornstein-Uhlenbeck process: a mean-reverting stochastic process.
//!
//! dX = theta * (mu - X) * dt + sigma * dW
//!
//! Commonly used for interest rates, temperature models, and any quantity
//! that tends to revert to a long-term mean.

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// OrnsteinUhlenbeck
// ---------------------------------------------------------------------------

/// Ornstein-Uhlenbeck process parameters.
pub struct OrnsteinUhlenbeck {
    /// Mean-reversion speed. Higher = faster reversion.
    pub theta: f64,
    /// Long-term mean.
    pub mu: f64,
    /// Volatility.
    pub sigma: f64,
    /// Time step.
    pub dt: f64,
}

impl OrnsteinUhlenbeck {
    pub fn new(theta: f64, mu: f64, sigma: f64, dt: f64) -> Self {
        Self { theta, mu, sigma, dt }
    }

    /// Single Euler-Maruyama step from `current`.
    /// X(t+dt) = X(t) + theta*(mu - X(t))*dt + sigma*sqrt(dt)*Z
    pub fn step(&self, rng: &mut Rng, current: f64) -> f64 {
        let z = rng.normal();
        current + self.theta * (self.mu - current) * self.dt + self.sigma * self.dt.sqrt() * z
    }

    /// Exact step using the analytical transition distribution.
    /// X(t+dt) | X(t) ~ N(mu + (X(t)-mu)*exp(-theta*dt), sigma²/(2*theta)*(1-exp(-2*theta*dt)))
    pub fn exact_step(&self, rng: &mut Rng, current: f64) -> f64 {
        let decay = (-self.theta * self.dt).exp();
        let mean = self.mu + (current - self.mu) * decay;
        let var = (self.sigma * self.sigma / (2.0 * self.theta)) * (1.0 - (-2.0 * self.theta * self.dt).exp());
        mean + rng.normal() * var.sqrt()
    }

    /// Generate a path of `steps` using Euler-Maruyama, starting at `x0`.
    pub fn path(&self, rng: &mut Rng, steps: usize, x0: f64) -> Vec<f64> {
        let mut values = Vec::with_capacity(steps + 1);
        values.push(x0);
        let mut current = x0;
        for _ in 0..steps {
            current = self.step(rng, current);
            values.push(current);
        }
        values
    }

    /// Generate a path using exact sampling.
    pub fn exact_path(&self, rng: &mut Rng, steps: usize, x0: f64) -> Vec<f64> {
        let mut values = Vec::with_capacity(steps + 1);
        values.push(x0);
        let mut current = x0;
        for _ in 0..steps {
            current = self.exact_step(rng, current);
            values.push(current);
        }
        values
    }

    /// Stationary variance: sigma² / (2 * theta).
    pub fn stationary_variance(&self) -> f64 {
        self.sigma * self.sigma / (2.0 * self.theta)
    }

    /// Stationary mean (equals mu).
    pub fn stationary_mean(&self) -> f64 {
        self.mu
    }

    /// Autocorrelation at lag tau: exp(-theta * tau).
    pub fn autocorrelation(&self, lag: f64) -> f64 {
        (-self.theta * lag).exp()
    }

    /// Half-life of mean reversion: ln(2) / theta.
    pub fn half_life(&self) -> f64 {
        (2.0_f64).ln() / self.theta
    }

    /// Conditional mean at time t given X(0) = x0.
    pub fn conditional_mean(&self, x0: f64, t: f64) -> f64 {
        self.mu + (x0 - self.mu) * (-self.theta * t).exp()
    }

    /// Conditional variance at time t.
    pub fn conditional_variance(&self, t: f64) -> f64 {
        (self.sigma * self.sigma / (2.0 * self.theta)) * (1.0 - (-2.0 * self.theta * t).exp())
    }
}

// ---------------------------------------------------------------------------
// Multi-dimensional OU
// ---------------------------------------------------------------------------

/// Multi-dimensional Ornstein-Uhlenbeck process (independent coordinates).
pub struct OUMultiDim {
    pub processes: Vec<OrnsteinUhlenbeck>,
}

impl OUMultiDim {
    pub fn new(processes: Vec<OrnsteinUhlenbeck>) -> Self {
        Self { processes }
    }

    /// Create a 2D OU with identical parameters.
    pub fn uniform_2d(theta: f64, mu: f64, sigma: f64, dt: f64) -> Self {
        Self {
            processes: vec![
                OrnsteinUhlenbeck::new(theta, mu, sigma, dt),
                OrnsteinUhlenbeck::new(theta, mu, sigma, dt),
            ],
        }
    }

    pub fn step(&self, rng: &mut Rng, current: &[f64]) -> Vec<f64> {
        self.processes
            .iter()
            .zip(current.iter())
            .map(|(ou, &x)| ou.step(rng, x))
            .collect()
    }

    pub fn path(&self, rng: &mut Rng, steps: usize, x0: &[f64]) -> Vec<Vec<f64>> {
        let mut trajectory = Vec::with_capacity(steps + 1);
        let mut current = x0.to_vec();
        trajectory.push(current.clone());
        for _ in 0..steps {
            current = self.step(rng, &current);
            trajectory.push(current.clone());
        }
        trajectory
    }
}

// ---------------------------------------------------------------------------
// OURenderer
// ---------------------------------------------------------------------------

/// Render an OU path with mean line and variance bands.
pub struct OURenderer {
    pub character: char,
    pub path_color: [f32; 4],
    pub mean_color: [f32; 4],
    pub band_color: [f32; 4],
    pub x_scale: f32,
    pub y_scale: f32,
}

impl OURenderer {
    pub fn new() -> Self {
        Self {
            character: '·',
            path_color: [0.3, 0.7, 1.0, 1.0],
            mean_color: [1.0, 0.8, 0.2, 0.8],
            band_color: [0.5, 0.5, 0.5, 0.3],
            x_scale: 0.1,
            y_scale: 1.0,
        }
    }

    /// Render the path, mean line, and +/- 2 sigma bands.
    pub fn render(&self, ou: &OrnsteinUhlenbeck, path: &[f64]) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = Vec::new();
        let mean = ou.stationary_mean();
        let std_dev = ou.stationary_variance().sqrt();

        for (i, &val) in path.iter().enumerate() {
            let x = i as f32 * self.x_scale;

            // Path point
            glyphs.push((
                Vec2::new(x, val as f32 * self.y_scale),
                self.character,
                self.path_color,
            ));

            // Mean line
            glyphs.push((
                Vec2::new(x, mean as f32 * self.y_scale),
                '─',
                self.mean_color,
            ));

            // Upper and lower bands (+/- 2 sigma)
            glyphs.push((
                Vec2::new(x, (mean + 2.0 * std_dev) as f32 * self.y_scale),
                '┄',
                self.band_color,
            ));
            glyphs.push((
                Vec2::new(x, (mean - 2.0 * std_dev) as f32 * self.y_scale),
                '┄',
                self.band_color,
            ));
        }

        glyphs
    }

    /// Render only the path as a simple line.
    pub fn render_path_only(&self, path: &[f64]) -> Vec<(Vec2, char, [f32; 4])> {
        path.iter()
            .enumerate()
            .map(|(i, &val)| {
                (
                    Vec2::new(i as f32 * self.x_scale, val as f32 * self.y_scale),
                    self.character,
                    self.path_color,
                )
            })
            .collect()
    }
}

impl Default for OURenderer {
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
    fn test_ou_mean_reversion() {
        // Start far from mean, verify it comes back
        let ou = OrnsteinUhlenbeck::new(5.0, 0.0, 0.5, 0.01);
        let mut rng = Rng::new(42);
        let path = ou.path(&mut rng, 2000, 10.0);

        // After enough time, should be close to mu=0
        let tail_mean: f64 = path[1500..].iter().sum::<f64>() / path[1500..].len() as f64;
        assert!(
            tail_mean.abs() < 1.0,
            "OU should revert to mean ~0, tail mean = {}",
            tail_mean
        );
    }

    #[test]
    fn test_ou_stationary_distribution() {
        // Run for a long time, check empirical variance matches sigma²/(2*theta)
        let theta = 2.0;
        let mu = 3.0;
        let sigma = 1.0;
        let dt = 0.01;
        let ou = OrnsteinUhlenbeck::new(theta, mu, sigma, dt);
        let mut rng = Rng::new(123);

        let path = ou.path(&mut rng, 100_000, mu);
        let tail = &path[50_000..]; // discard burn-in
        let n = tail.len() as f64;
        let empirical_mean = tail.iter().sum::<f64>() / n;
        let empirical_var = tail.iter().map(|x| (x - empirical_mean).powi(2)).sum::<f64>() / n;

        let expected_var = ou.stationary_variance(); // 0.25

        assert!(
            (empirical_mean - mu).abs() < 0.1,
            "stationary mean {} should be ~{}",
            empirical_mean,
            mu
        );
        assert!(
            (empirical_var - expected_var).abs() < 0.1,
            "stationary variance {} should be ~{}",
            empirical_var,
            expected_var
        );
    }

    #[test]
    fn test_ou_stationary_values() {
        let ou = OrnsteinUhlenbeck::new(2.0, 5.0, 1.0, 0.01);
        assert_eq!(ou.stationary_mean(), 5.0);
        assert!((ou.stationary_variance() - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_ou_autocorrelation() {
        let ou = OrnsteinUhlenbeck::new(2.0, 0.0, 1.0, 0.01);
        assert!((ou.autocorrelation(0.0) - 1.0).abs() < 1e-10);
        assert!(ou.autocorrelation(1.0) < 1.0);
        assert!(ou.autocorrelation(1.0) > 0.0);
        // exp(-2) ≈ 0.135
        assert!((ou.autocorrelation(1.0) - (-2.0_f64).exp()).abs() < 1e-10);
    }

    #[test]
    fn test_ou_half_life() {
        let ou = OrnsteinUhlenbeck::new(2.0, 0.0, 1.0, 0.01);
        let hl = ou.half_life();
        assert!((hl - 2.0_f64.ln() / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_ou_exact_vs_euler() {
        // Both methods should give similar stationary statistics
        let ou = OrnsteinUhlenbeck::new(1.0, 0.0, 1.0, 0.01);
        let mut rng1 = Rng::new(42);
        let mut rng2 = Rng::new(42);
        let euler = ou.path(&mut rng1, 10_000, 0.0);
        let exact = ou.exact_path(&mut rng2, 10_000, 0.0);

        let n = 5000.0;
        let euler_mean = euler[5000..].iter().sum::<f64>() / n;
        let exact_mean = exact[5000..].iter().sum::<f64>() / n;

        assert!(euler_mean.abs() < 0.5);
        assert!(exact_mean.abs() < 0.5);
    }

    #[test]
    fn test_ou_conditional_mean() {
        let ou = OrnsteinUhlenbeck::new(1.0, 5.0, 1.0, 0.01);
        let cm = ou.conditional_mean(10.0, 0.0);
        assert!((cm - 10.0).abs() < 1e-10);
        // As t -> infinity, conditional mean -> mu
        let cm_large = ou.conditional_mean(10.0, 100.0);
        assert!((cm_large - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_ou_renderer() {
        let ou = OrnsteinUhlenbeck::new(1.0, 0.0, 1.0, 0.01);
        let renderer = OURenderer::new();
        let mut rng = Rng::new(42);
        let path = ou.path(&mut rng, 50, 0.0);
        let glyphs = renderer.render(&ou, &path);
        // Each point produces 4 glyphs: path, mean, upper band, lower band
        assert_eq!(glyphs.len(), 51 * 4);
    }

    #[test]
    fn test_ou_multidim() {
        let ou = OUMultiDim::uniform_2d(1.0, 0.0, 1.0, 0.01);
        let mut rng = Rng::new(42);
        let path = ou.path(&mut rng, 100, &[5.0, -5.0]);
        assert_eq!(path.len(), 101);
        assert_eq!(path[0], vec![5.0, -5.0]);
    }
}
