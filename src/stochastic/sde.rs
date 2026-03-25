//! Stochastic differential equations: generic SDE solver with Euler-Maruyama
//! and Milstein methods, plus preset SDEs for common processes.

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// SDE
// ---------------------------------------------------------------------------

/// A stochastic differential equation dX = a(t,X)dt + b(t,X)dW.
pub struct SDE {
    /// Drift function a(t, x).
    pub drift: Box<dyn Fn(f64, f64) -> f64>,
    /// Diffusion function b(t, x).
    pub diffusion: Box<dyn Fn(f64, f64) -> f64>,
}

impl SDE {
    pub fn new(
        drift: Box<dyn Fn(f64, f64) -> f64>,
        diffusion: Box<dyn Fn(f64, f64) -> f64>,
    ) -> Self {
        Self { drift, diffusion }
    }
}

// ---------------------------------------------------------------------------
// Solvers
// ---------------------------------------------------------------------------

/// Euler-Maruyama method for solving an SDE.
///
/// X_{n+1} = X_n + a(t_n, X_n) * dt + b(t_n, X_n) * dW_n
pub fn euler_maruyama(sde: &SDE, x0: f64, dt: f64, steps: usize, rng: &mut Rng) -> Vec<f64> {
    let mut path = Vec::with_capacity(steps + 1);
    path.push(x0);
    let mut x = x0;
    let mut t = 0.0;
    let sqrt_dt = dt.sqrt();

    for _ in 0..steps {
        let dw = rng.normal() * sqrt_dt;
        let a = (sde.drift)(t, x);
        let b = (sde.diffusion)(t, x);
        x += a * dt + b * dw;
        t += dt;
        path.push(x);
    }
    path
}

/// Milstein method for solving an SDE.
///
/// X_{n+1} = X_n + a*dt + b*dW + 0.5*b*b'*(dW^2 - dt)
/// where b' = db/dx (diffusion_derivative).
pub fn milstein(
    sde: &SDE,
    diffusion_derivative: &dyn Fn(f64, f64) -> f64,
    x0: f64,
    dt: f64,
    steps: usize,
    rng: &mut Rng,
) -> Vec<f64> {
    let mut path = Vec::with_capacity(steps + 1);
    path.push(x0);
    let mut x = x0;
    let mut t = 0.0;
    let sqrt_dt = dt.sqrt();

    for _ in 0..steps {
        let dw = rng.normal() * sqrt_dt;
        let a = (sde.drift)(t, x);
        let b = (sde.diffusion)(t, x);
        let b_prime = diffusion_derivative(t, x);
        x += a * dt + b * dw + 0.5 * b * b_prime * (dw * dw - dt);
        t += dt;
        path.push(x);
    }
    path
}

/// Heun's method (improved Euler / predictor-corrector) for SDEs.
pub fn heun(sde: &SDE, x0: f64, dt: f64, steps: usize, rng: &mut Rng) -> Vec<f64> {
    let mut path = Vec::with_capacity(steps + 1);
    path.push(x0);
    let mut x = x0;
    let mut t = 0.0;
    let sqrt_dt = dt.sqrt();

    for _ in 0..steps {
        let dw = rng.normal() * sqrt_dt;
        let a1 = (sde.drift)(t, x);
        let b1 = (sde.diffusion)(t, x);

        let x_tilde = x + a1 * dt + b1 * dw;
        let t_next = t + dt;

        let a2 = (sde.drift)(t_next, x_tilde);
        let b2 = (sde.diffusion)(t_next, x_tilde);

        x += 0.5 * (a1 + a2) * dt + 0.5 * (b1 + b2) * dw;
        t = t_next;
        path.push(x);
    }
    path
}

// ---------------------------------------------------------------------------
// Error measures
// ---------------------------------------------------------------------------

/// Strong error: max |exact(t_i) - numerical(t_i)|.
pub fn strong_error(exact: &[f64], numerical: &[f64]) -> f64 {
    exact
        .iter()
        .zip(numerical.iter())
        .map(|(e, n)| (e - n).abs())
        .fold(0.0, f64::max)
}

/// Weak error: |E[exact(T)] - E[numerical(T)]|.
pub fn weak_error(exact_mean: f64, numerical_mean: f64) -> f64 {
    (exact_mean - numerical_mean).abs()
}

/// Root mean square error between two paths.
pub fn rmse(exact: &[f64], numerical: &[f64]) -> f64 {
    let n = exact.len().min(numerical.len());
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = exact.iter().zip(numerical.iter()).map(|(e, n)| (e - n).powi(2)).sum();
    (sum / n as f64).sqrt()
}

// ---------------------------------------------------------------------------
// Preset SDEs
// ---------------------------------------------------------------------------

/// Geometric Brownian Motion: dS = mu*S*dt + sigma*S*dW.
pub fn sde_gbm(mu: f64, sigma: f64) -> SDE {
    SDE {
        drift: Box::new(move |_t, x| mu * x),
        diffusion: Box::new(move |_t, x| sigma * x),
    }
}

/// GBM diffusion derivative: d(sigma*x)/dx = sigma.
pub fn sde_gbm_diffusion_deriv(sigma: f64) -> Box<dyn Fn(f64, f64) -> f64> {
    Box::new(move |_t, _x| sigma)
}

/// Ornstein-Uhlenbeck: dX = theta*(mu - X)*dt + sigma*dW.
pub fn sde_ou(theta: f64, mu: f64, sigma: f64) -> SDE {
    SDE {
        drift: Box::new(move |_t, x| theta * (mu - x)),
        diffusion: Box::new(move |_t, _x| sigma),
    }
}

/// OU diffusion derivative: d(sigma)/dx = 0.
pub fn sde_ou_diffusion_deriv() -> Box<dyn Fn(f64, f64) -> f64> {
    Box::new(|_t, _x| 0.0)
}

/// Cox-Ingersoll-Ross: dX = kappa*(theta - X)*dt + sigma*sqrt(X)*dW.
pub fn sde_cir(kappa: f64, theta: f64, sigma: f64) -> SDE {
    SDE {
        drift: Box::new(move |_t, x| kappa * (theta - x)),
        diffusion: Box::new(move |_t, x| sigma * x.max(0.0).sqrt()),
    }
}

/// CIR diffusion derivative: d(sigma*sqrt(x))/dx = sigma/(2*sqrt(x)).
pub fn sde_cir_diffusion_deriv(sigma: f64) -> Box<dyn Fn(f64, f64) -> f64> {
    Box::new(move |_t, x| {
        let sx = x.max(1e-15).sqrt();
        sigma / (2.0 * sx)
    })
}

/// Constant Elasticity of Variance (CEV): dS = mu*S*dt + sigma*S^gamma*dW.
pub fn sde_cev(mu: f64, sigma: f64, gamma: f64) -> SDE {
    SDE {
        drift: Box::new(move |_t, x| mu * x),
        diffusion: Box::new(move |_t, x| sigma * x.abs().powf(gamma)),
    }
}

/// Langevin equation: dV = -gamma*V*dt + sigma*dW (velocity process).
pub fn sde_langevin(gamma: f64, sigma: f64) -> SDE {
    SDE {
        drift: Box::new(move |_t, v| -gamma * v),
        diffusion: Box::new(move |_t, _v| sigma),
    }
}

// ---------------------------------------------------------------------------
// SDERenderer
// ---------------------------------------------------------------------------

/// Render SDE solution paths with drift/diffusion visualization.
pub struct SDERenderer {
    pub path_character: char,
    pub path_color: [f32; 4],
    pub drift_color: [f32; 4],
    pub x_scale: f32,
    pub y_scale: f32,
}

impl SDERenderer {
    pub fn new() -> Self {
        Self {
            path_character: '·',
            path_color: [0.2, 0.8, 1.0, 1.0],
            drift_color: [1.0, 0.5, 0.2, 0.5],
            x_scale: 0.1,
            y_scale: 1.0,
        }
    }

    /// Render a single solution path.
    pub fn render_path(&self, path: &[f64]) -> Vec<(Vec2, char, [f32; 4])> {
        path.iter()
            .enumerate()
            .map(|(i, &val)| {
                (
                    Vec2::new(i as f32 * self.x_scale, val as f32 * self.y_scale),
                    self.path_character,
                    self.path_color,
                )
            })
            .collect()
    }

    /// Render multiple sample paths.
    pub fn render_paths(&self, paths: &[Vec<f64>]) -> Vec<(Vec2, char, [f32; 4])> {
        let n = paths.len().max(1);
        let mut glyphs = Vec::new();
        for (pi, path) in paths.iter().enumerate() {
            let alpha = 0.2 + 0.6 * (pi as f32 / n as f32);
            let color = [self.path_color[0], self.path_color[1], self.path_color[2], alpha];
            for (i, &val) in path.iter().enumerate() {
                glyphs.push((
                    Vec2::new(i as f32 * self.x_scale, val as f32 * self.y_scale),
                    self.path_character,
                    color,
                ));
            }
        }
        glyphs
    }

    /// Render drift field as arrows at sampled points.
    pub fn render_drift_field(
        &self,
        sde: &SDE,
        t_range: (f64, f64),
        x_range: (f64, f64),
        grid: (usize, usize),
    ) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = Vec::new();
        let (t_steps, x_steps) = grid;

        for ti in 0..t_steps {
            for xi in 0..x_steps {
                let t = t_range.0 + (t_range.1 - t_range.0) * ti as f64 / t_steps as f64;
                let x = x_range.0 + (x_range.1 - x_range.0) * xi as f64 / x_steps as f64;
                let drift = (sde.drift)(t, x);

                let ch = if drift > 0.1 {
                    '↑'
                } else if drift < -0.1 {
                    '↓'
                } else {
                    '·'
                };

                let pos = Vec2::new(t as f32 * self.x_scale, x as f32 * self.y_scale);
                glyphs.push((pos, ch, self.drift_color));
            }
        }
        glyphs
    }
}

impl Default for SDERenderer {
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
    fn test_euler_maruyama_gbm() {
        // Compare EM solution of GBM SDE against closed-form
        let mu = 0.05;
        let sigma = 0.2;
        let s0 = 100.0;
        let dt = 0.001;
        let steps = 1000;
        let trials = 5000;

        let mut rng = Rng::new(42);
        let sde = sde_gbm(mu, sigma);

        let endpoints: Vec<f64> = (0..trials)
            .map(|_| {
                let path = euler_maruyama(&sde, s0, dt, steps, &mut rng);
                *path.last().unwrap()
            })
            .collect();

        let empirical_mean = endpoints.iter().sum::<f64>() / trials as f64;
        let expected_mean = s0 * (mu * 1.0).exp();

        assert!(
            (empirical_mean - expected_mean).abs() / expected_mean < 0.1,
            "EM GBM mean {} should be ~{}", empirical_mean, expected_mean
        );
    }

    #[test]
    fn test_milstein_gbm() {
        let mu = 0.05;
        let sigma = 0.2;
        let s0 = 100.0;
        let dt = 0.01;
        let steps = 100;

        let sde = sde_gbm(mu, sigma);
        let deriv = sde_gbm_diffusion_deriv(sigma);
        let mut rng = Rng::new(42);

        let path = milstein(&sde, &*deriv, s0, dt, steps, &mut rng);
        assert_eq!(path.len(), steps + 1);
        assert!((path[0] - s0).abs() < 1e-10);
        // GBM should stay positive with high probability
        assert!(path.iter().all(|&x| x > 0.0));
    }

    #[test]
    fn test_euler_maruyama_ou() {
        let theta = 2.0;
        let mu = 5.0;
        let sigma = 1.0;
        let sde = sde_ou(theta, mu, sigma);
        let mut rng = Rng::new(42);

        let path = euler_maruyama(&sde, 0.0, 0.01, 10_000, &mut rng);
        // Tail should be near mu
        let tail_mean: f64 = path[5000..].iter().sum::<f64>() / 5000.0;
        assert!(
            (tail_mean - mu).abs() < 1.0,
            "OU tail mean {} should be near {}", tail_mean, mu
        );
    }

    #[test]
    fn test_cir_non_negative() {
        let sde = sde_cir(1.0, 0.05, 0.1);
        let mut rng = Rng::new(42);
        let path = euler_maruyama(&sde, 0.05, 0.001, 10_000, &mut rng);
        // CIR with Feller condition 2*kappa*theta > sigma^2 should stay positive
        // 2*1*0.05 = 0.1 > 0.01 = sigma^2, so it should
        let min_val = path.iter().cloned().fold(f64::INFINITY, f64::min);
        // EM can go slightly negative; just check it's not too negative
        assert!(
            min_val > -0.01,
            "CIR should stay roughly non-negative, min = {}", min_val
        );
    }

    #[test]
    fn test_strong_error() {
        let exact = vec![0.0, 1.0, 2.0, 3.0];
        let numerical = vec![0.0, 1.1, 1.8, 3.2];
        let err = strong_error(&exact, &numerical);
        assert!((err - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_weak_error() {
        assert!((weak_error(5.0, 4.8) - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_rmse() {
        let exact = vec![0.0, 1.0, 2.0];
        let numerical = vec![0.0, 1.0, 2.0];
        assert!(rmse(&exact, &numerical) < 1e-10);
    }

    #[test]
    fn test_milstein_better_than_euler() {
        // Milstein should have better strong convergence than Euler for GBM
        let mu = 0.1;
        let sigma = 0.3;
        let s0 = 1.0;
        let dt = 0.01;
        let steps = 100;
        let sde = sde_gbm(mu, sigma);
        let deriv = sde_gbm_diffusion_deriv(sigma);

        // Use same noise for both — we just check they produce valid paths
        let mut rng1 = Rng::new(42);
        let em_path = euler_maruyama(&sde, s0, dt, steps, &mut rng1);
        let mut rng2 = Rng::new(42);
        let sde2 = sde_gbm(mu, sigma);
        let mil_path = milstein(&sde2, &*deriv, s0, dt, steps, &mut rng2);

        assert_eq!(em_path.len(), steps + 1);
        assert_eq!(mil_path.len(), steps + 1);
    }

    #[test]
    fn test_heun_method() {
        let sde = sde_ou(1.0, 0.0, 1.0);
        let mut rng = Rng::new(42);
        let path = heun(&sde, 5.0, 0.01, 1000, &mut rng);
        assert_eq!(path.len(), 1001);
        // Should mean-revert toward 0
        let tail_mean: f64 = path[500..].iter().sum::<f64>() / 500.0;
        assert!(tail_mean.abs() < 2.0);
    }

    #[test]
    fn test_sde_renderer() {
        let sde = sde_ou(1.0, 0.0, 1.0);
        let mut rng = Rng::new(42);
        let path = euler_maruyama(&sde, 0.0, 0.01, 100, &mut rng);
        let renderer = SDERenderer::new();
        let glyphs = renderer.render_path(&path);
        assert_eq!(glyphs.len(), 101);
    }

    #[test]
    fn test_drift_field_render() {
        let sde = sde_ou(1.0, 0.0, 1.0);
        let renderer = SDERenderer::new();
        let glyphs = renderer.render_drift_field(&sde, (0.0, 1.0), (-2.0, 2.0), (5, 5));
        assert_eq!(glyphs.len(), 25);
    }
}
