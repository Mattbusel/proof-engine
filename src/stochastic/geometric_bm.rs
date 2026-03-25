//! Geometric Brownian Motion (GBM) for modelling stock prices and
//! multiplicative stochastic processes.
//!
//! dS = mu * S * dt + sigma * S * dW
//! Closed-form: S(t) = S(0) * exp((mu - sigma²/2)*t + sigma * W(t))

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// GeometricBM
// ---------------------------------------------------------------------------

/// Geometric Brownian Motion parameters.
pub struct GeometricBM {
    /// Drift rate (annualised expected return).
    pub mu: f64,
    /// Volatility (annualised standard deviation).
    pub sigma: f64,
    /// Initial value S(0).
    pub s0: f64,
    /// Time step size.
    pub dt: f64,
}

impl GeometricBM {
    pub fn new(mu: f64, sigma: f64, s0: f64, dt: f64) -> Self {
        Self { mu, sigma, s0, dt }
    }

    /// Single step: advance from `current` to next value.
    /// S(t+dt) = S(t) * exp((mu - sigma²/2)*dt + sigma*sqrt(dt)*Z)
    pub fn step(&self, rng: &mut Rng, current: f64) -> f64 {
        let z = rng.normal();
        let drift = (self.mu - 0.5 * self.sigma * self.sigma) * self.dt;
        let diffusion = self.sigma * self.dt.sqrt() * z;
        current * (drift + diffusion).exp()
    }

    /// Generate a full price path of length `steps + 1`.
    pub fn path(&self, rng: &mut Rng, steps: usize) -> Vec<f64> {
        let mut prices = Vec::with_capacity(steps + 1);
        prices.push(self.s0);
        let mut current = self.s0;
        for _ in 0..steps {
            current = self.step(rng, current);
            prices.push(current);
        }
        prices
    }

    /// Expected value at time t: E[S(t)] = S(0) * exp(mu * t).
    pub fn expected_value(&self, t: f64) -> f64 {
        self.s0 * (self.mu * t).exp()
    }

    /// Variance at time t: Var[S(t)] = S(0)² * exp(2*mu*t) * (exp(sigma²*t) - 1).
    pub fn variance(&self, t: f64) -> f64 {
        let s0_sq = self.s0 * self.s0;
        s0_sq * (2.0 * self.mu * t).exp() * ((self.sigma * self.sigma * t).exp() - 1.0)
    }

    /// Generate multiple independent paths (e.g. for Monte Carlo pricing).
    pub fn paths(&self, rng: &mut Rng, steps: usize, count: usize) -> Vec<Vec<f64>> {
        (0..count).map(|_| self.path(rng, steps)).collect()
    }
}

// ---------------------------------------------------------------------------
// Black-Scholes option pricing
// ---------------------------------------------------------------------------

/// Cumulative distribution function of the standard normal (approximation).
fn normal_cdf(x: f64) -> f64 {
    // Abramowitz and Stegun approximation 26.2.17
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs();
    let t = 1.0 / (1.0 + p * x_abs);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x_abs * x_abs / 2.0).exp();

    0.5 * (1.0 + sign * y)
}

/// Black-Scholes European call option price.
///
/// * `s` - Current stock price
/// * `k` - Strike price
/// * `r` - Risk-free interest rate (annualised)
/// * `sigma` - Volatility (annualised)
/// * `t` - Time to expiration (years)
pub fn black_scholes_call(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 {
        return (s - k).max(0.0);
    }
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    s * normal_cdf(d1) - k * (-r * t).exp() * normal_cdf(d2)
}

/// Black-Scholes European put option price.
///
/// * `s` - Current stock price
/// * `k` - Strike price
/// * `r` - Risk-free interest rate (annualised)
/// * `sigma` - Volatility (annualised)
/// * `t` - Time to expiration (years)
pub fn black_scholes_put(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 {
        return (k - s).max(0.0);
    }
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    k * (-r * t).exp() * normal_cdf(-d2) - s * normal_cdf(-d1)
}

/// Compute the implied volatility for a call option using bisection.
pub fn implied_volatility_call(s: f64, k: f64, r: f64, t: f64, market_price: f64) -> f64 {
    let mut lo = 0.001;
    let mut hi = 5.0;
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        let price = black_scholes_call(s, k, r, mid, t);
        if price < market_price {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Greeks for a European call option.
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

/// Compute Greeks for a European call.
pub fn call_greeks(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> Greeks {
    let sqrt_t = t.sqrt();
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
    let d2 = d1 - sigma * sqrt_t;
    let pdf_d1 = (-0.5 * d1 * d1).exp() / (2.0 * std::f64::consts::PI).sqrt();

    let delta = normal_cdf(d1);
    let gamma = pdf_d1 / (s * sigma * sqrt_t);
    let theta = -(s * pdf_d1 * sigma) / (2.0 * sqrt_t) - r * k * (-r * t).exp() * normal_cdf(d2);
    let vega = s * pdf_d1 * sqrt_t;
    let rho = k * t * (-r * t).exp() * normal_cdf(d2);

    Greeks { delta, gamma, theta, vega, rho }
}

// ---------------------------------------------------------------------------
// GBMRenderer
// ---------------------------------------------------------------------------

/// Render GBM price paths as glyph line charts.
pub struct GBMRenderer {
    pub character: char,
    pub color: [f32; 4],
    pub x_scale: f32,
    pub y_scale: f32,
}

impl GBMRenderer {
    pub fn new() -> Self {
        Self {
            character: '█',
            color: [0.2, 1.0, 0.3, 1.0],
            x_scale: 0.05,
            y_scale: 0.01,
        }
    }

    pub fn with_scales(mut self, x_scale: f32, y_scale: f32) -> Self {
        self.x_scale = x_scale;
        self.y_scale = y_scale;
        self
    }

    /// Render a single price path as positioned glyphs.
    pub fn render_path(&self, path: &[f64]) -> Vec<(Vec2, char, [f32; 4])> {
        path.iter()
            .enumerate()
            .map(|(i, &price)| {
                let pos = Vec2::new(i as f32 * self.x_scale, price as f32 * self.y_scale);
                (pos, self.character, self.color)
            })
            .collect()
    }

    /// Render multiple paths with varying alpha for a fan chart effect.
    pub fn render_fan(&self, paths: &[Vec<f64>]) -> Vec<(Vec2, char, [f32; 4])> {
        let n = paths.len().max(1);
        let mut glyphs = Vec::new();
        for (pi, path) in paths.iter().enumerate() {
            let alpha = 0.1 + 0.3 * (pi as f32 / n as f32);
            let color = [self.color[0], self.color[1], self.color[2], alpha];
            for (i, &price) in path.iter().enumerate() {
                let pos = Vec2::new(i as f32 * self.x_scale, price as f32 * self.y_scale);
                glyphs.push((pos, self.character, color));
            }
        }
        glyphs
    }
}

impl Default for GBMRenderer {
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
    fn test_gbm_always_positive() {
        let gbm = GeometricBM::new(0.05, 0.2, 100.0, 0.01);
        let mut rng = Rng::new(42);
        let path = gbm.path(&mut rng, 1000);
        assert!(path.iter().all(|&p| p > 0.0), "GBM should always be positive");
    }

    #[test]
    fn test_gbm_expected_value() {
        // E[S(t)] = S0 * exp(mu*t)
        let mu = 0.05;
        let sigma = 0.3;
        let s0 = 100.0;
        let dt = 0.001;
        let steps = 1000; // t = 1.0
        let trials = 5000;
        let gbm = GeometricBM::new(mu, sigma, s0, dt);
        let mut rng = Rng::new(12345);

        let sum: f64 = (0..trials)
            .map(|_| {
                let path = gbm.path(&mut rng, steps);
                *path.last().unwrap()
            })
            .sum();
        let empirical_mean = sum / trials as f64;
        let expected = s0 * (mu * 1.0).exp(); // ~105.13

        assert!(
            (empirical_mean - expected).abs() / expected < 0.1,
            "empirical mean {} should be close to expected {}",
            empirical_mean,
            expected
        );
    }

    #[test]
    fn test_gbm_log_normal() {
        // ln(S(t)/S(0)) should be normally distributed with
        // mean (mu - sigma²/2)*t and variance sigma²*t
        let mu = 0.1;
        let sigma = 0.2;
        let s0 = 100.0;
        let dt = 0.01;
        let steps = 100; // t = 1.0
        let trials = 10_000;
        let gbm = GeometricBM::new(mu, sigma, s0, dt);
        let mut rng = Rng::new(777);

        let log_returns: Vec<f64> = (0..trials)
            .map(|_| {
                let path = gbm.path(&mut rng, steps);
                (path.last().unwrap() / s0).ln()
            })
            .collect();

        let mean = log_returns.iter().sum::<f64>() / trials as f64;
        let var = log_returns.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / trials as f64;
        let expected_mean = (mu - 0.5 * sigma * sigma) * 1.0;
        let expected_var = sigma * sigma * 1.0;

        assert!(
            (mean - expected_mean).abs() < 0.05,
            "log-return mean {} should be ~{}",
            mean,
            expected_mean
        );
        assert!(
            (var - expected_var).abs() < 0.02,
            "log-return variance {} should be ~{}",
            var,
            expected_var
        );
    }

    #[test]
    fn test_black_scholes_put_call_parity() {
        // C - P = S - K*exp(-rT)
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let sigma = 0.2;
        let t = 1.0;

        let c = black_scholes_call(s, k, r, sigma, t);
        let p = black_scholes_put(s, k, r, sigma, t);
        let parity = s - k * (-r * t).exp();

        assert!(
            (c - p - parity).abs() < 1e-10,
            "Put-call parity violated: C={}, P={}, S-Ke^-rT={}",
            c,
            p,
            parity
        );
    }

    #[test]
    fn test_black_scholes_call_value() {
        // Known approximate value: S=100, K=100, r=5%, sigma=20%, T=1 => C ≈ 10.45
        let c = black_scholes_call(100.0, 100.0, 0.05, 0.2, 1.0);
        assert!(
            (c - 10.45).abs() < 0.5,
            "BS call should be ~10.45, got {}",
            c
        );
    }

    #[test]
    fn test_black_scholes_at_expiry() {
        assert!((black_scholes_call(110.0, 100.0, 0.05, 0.2, 0.0) - 10.0).abs() < 1e-10);
        assert!((black_scholes_call(90.0, 100.0, 0.05, 0.2, 0.0) - 0.0).abs() < 1e-10);
        assert!((black_scholes_put(90.0, 100.0, 0.05, 0.2, 0.0) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_implied_volatility() {
        let sigma = 0.25;
        let price = black_scholes_call(100.0, 100.0, 0.05, sigma, 1.0);
        let iv = implied_volatility_call(100.0, 100.0, 0.05, 1.0, price);
        assert!(
            (iv - sigma).abs() < 0.001,
            "implied vol {} should be ~{}",
            iv,
            sigma
        );
    }

    #[test]
    fn test_greeks_delta_range() {
        let g = call_greeks(100.0, 100.0, 0.05, 0.2, 1.0);
        assert!(g.delta > 0.0 && g.delta < 1.0, "delta should be in (0,1)");
        assert!(g.gamma > 0.0, "gamma should be positive");
        assert!(g.vega > 0.0, "vega should be positive");
    }

    #[test]
    fn test_gbm_renderer() {
        let renderer = GBMRenderer::new();
        let gbm = GeometricBM::new(0.05, 0.2, 100.0, 0.01);
        let mut rng = Rng::new(42);
        let path = gbm.path(&mut rng, 50);
        let glyphs = renderer.render_path(&path);
        assert_eq!(glyphs.len(), 51);
    }
}
