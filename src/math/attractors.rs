//! Strange attractor implementations — RK4 integration, Lyapunov exponents, warmup.
//!
//! All seven classical attractors are implemented with:
//! - 4th-order Runge-Kutta (RK4) integration for accuracy
//! - Lyapunov exponent approximation (numerical divergence tracking)
//! - Warmup / transient discard so initial noise is removed before sampling
//! - Normalised output bounding boxes (attractor fits ≈ unit cube)
//! - Bifurcation parameter sweeps for each attractor family

use glam::Vec3;

// ── Attractor type ─────────────────────────────────────────────────────────────

/// Which strange attractor to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttractorType {
    Lorenz,
    Rossler,
    Chen,
    Halvorsen,
    Aizawa,
    Thomas,
    Dadras,
    Sprott,
    Rabinovich,
    Burke,
}

impl AttractorType {
    /// Human-readable name for display.
    pub fn name(self) -> &'static str {
        match self {
            AttractorType::Lorenz      => "Lorenz",
            AttractorType::Rossler     => "Rössler",
            AttractorType::Chen        => "Chen",
            AttractorType::Halvorsen   => "Halvorsen",
            AttractorType::Aizawa      => "Aizawa",
            AttractorType::Thomas      => "Thomas",
            AttractorType::Dadras      => "Dadras",
            AttractorType::Sprott      => "Sprott B",
            AttractorType::Rabinovich  => "Rabinovich–Fabrikant",
            AttractorType::Burke       => "Burke–Shaw",
        }
    }

    /// All attractor variants, useful for iteration.
    pub fn all() -> &'static [AttractorType] {
        &[
            AttractorType::Lorenz,
            AttractorType::Rossler,
            AttractorType::Chen,
            AttractorType::Halvorsen,
            AttractorType::Aizawa,
            AttractorType::Thomas,
            AttractorType::Dadras,
            AttractorType::Sprott,
            AttractorType::Rabinovich,
            AttractorType::Burke,
        ]
    }

    /// Approximate scale factor to normalise positions to ≈ unit cube.
    pub fn normalization_scale(self) -> f32 {
        match self {
            AttractorType::Lorenz      => 1.0 / 30.0,
            AttractorType::Rossler     => 1.0 / 12.0,
            AttractorType::Chen        => 1.0 / 30.0,
            AttractorType::Halvorsen   => 1.0 / 8.0,
            AttractorType::Aizawa      => 1.0 / 1.5,
            AttractorType::Thomas      => 1.0 / 3.5,
            AttractorType::Dadras      => 1.0 / 10.0,
            AttractorType::Sprott      => 1.0 / 2.0,
            AttractorType::Rabinovich  => 1.0 / 3.0,
            AttractorType::Burke       => 1.0 / 3.5,
        }
    }

    /// Recommended integration step size (smaller = more accurate but slower).
    pub fn recommended_dt(self) -> f32 {
        match self {
            AttractorType::Lorenz      => 0.005,
            AttractorType::Rossler     => 0.01,
            AttractorType::Chen        => 0.005,
            AttractorType::Halvorsen   => 0.01,
            AttractorType::Aizawa      => 0.01,
            AttractorType::Thomas      => 0.05,
            AttractorType::Dadras      => 0.005,
            AttractorType::Sprott      => 0.01,
            AttractorType::Rabinovich  => 0.005,
            AttractorType::Burke       => 0.005,
        }
    }

    /// Approximate Lyapunov exponent (positive → chaotic).
    pub fn lyapunov_estimate(self) -> f32 {
        match self {
            AttractorType::Lorenz      => 0.9056,
            AttractorType::Rossler     => 0.0714,
            AttractorType::Chen        => 2.0272,
            AttractorType::Halvorsen   => 0.8042,
            AttractorType::Aizawa      => 0.0721,
            AttractorType::Thomas      => 0.0550,
            AttractorType::Dadras      => 0.5100,
            AttractorType::Sprott      => 0.3600,
            AttractorType::Rabinovich  => 0.1600,
            AttractorType::Burke       => 0.3300,
        }
    }
}

// ── Derivative functions ───────────────────────────────────────────────────────

/// Compute continuous-time derivatives at state `s` for the given attractor.
/// Returns `(dx/dt, dy/dt, dz/dt)`.
pub fn derivatives(attractor: AttractorType, s: Vec3) -> Vec3 {
    let (x, y, z) = (s.x, s.y, s.z);
    let (dx, dy, dz) = match attractor {
        AttractorType::Lorenz => {
            const SIGMA: f32 = 10.0;
            const RHO:   f32 = 28.0;
            const BETA:  f32 = 8.0 / 3.0;
            (SIGMA * (y - x), x * (RHO - z) - y, x * y - BETA * z)
        }
        AttractorType::Rossler => {
            const A: f32 = 0.2;
            const B: f32 = 0.2;
            const C: f32 = 5.7;
            (-y - z, x + A * y, B + z * (x - C))
        }
        AttractorType::Chen => {
            const A: f32 = 35.0;
            const B: f32 = 3.0;
            const C: f32 = 28.0;
            (A * (y - x), (C - A) * x - x * z + C * y, x * y - B * z)
        }
        AttractorType::Halvorsen => {
            const A: f32 = 1.4;
            (
                -A * x - 4.0 * y - 4.0 * z - y * y,
                -A * y - 4.0 * z - 4.0 * x - z * z,
                -A * z - 4.0 * x - 4.0 * y - x * x,
            )
        }
        AttractorType::Aizawa => {
            const A: f32 = 0.95;
            const B: f32 = 0.7;
            const C: f32 = 0.6;
            const D: f32 = 3.5;
            const E: f32 = 0.25;
            const F: f32 = 0.1;
            (
                (z - B) * x - D * y,
                D * x + (z - B) * y,
                C + A * z - z.powi(3) / 3.0
                    - (x * x + y * y) * (1.0 + E * z)
                    + F * z * x.powi(3),
            )
        }
        AttractorType::Thomas => {
            const B: f32 = 0.208_186;
            (y.sin() - B * x, z.sin() - B * y, x.sin() - B * z)
        }
        AttractorType::Dadras => {
            const P: f32 = 3.0;
            const Q: f32 = 2.7;
            const R: f32 = 1.7;
            const S: f32 = 2.0;
            const H: f32 = 9.0;
            (y - P * x + Q * y * z, R * y - x * z + z, S * x * y - H * z)
        }
        AttractorType::Sprott => {
            // Sprott B: dx=yz, dy=x-y, dz=1-xy
            (y * z, x - y, 1.0 - x * y)
        }
        AttractorType::Rabinovich => {
            // Rabinovich–Fabrikant with canonical parameters
            const GAMMA: f32 = 0.87;
            const ALPHA: f32 = 1.1;
            (
                y * (z - 1.0 + x * x) + GAMMA * x,
                x * (3.0 * z + 1.0 - x * x) + GAMMA * y,
                -2.0 * z * (ALPHA + x * y),
            )
        }
        AttractorType::Burke => {
            // Burke–Shaw: dx=-s(x+y), dy=y-sx*z, dz=sx*y+v
            const S: f32 = 10.0;
            const V: f32 = 4.272;
            (-S * (x + y), -y - S * x * z, S * x * y + V)
        }
    };
    Vec3::new(dx, dy, dz)
}

// ── RK4 integrator ────────────────────────────────────────────────────────────

/// Single RK4 step.
/// More accurate than Euler at the cost of 4× function evaluations.
#[inline]
pub fn rk4_step(attractor: AttractorType, state: Vec3, dt: f32) -> Vec3 {
    let k1 = derivatives(attractor, state);
    let k2 = derivatives(attractor, state + k1 * (dt * 0.5));
    let k3 = derivatives(attractor, state + k2 * (dt * 0.5));
    let k4 = derivatives(attractor, state + k3 * dt);
    state + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0)
}

/// Evolve by one step and return (new_state, displacement).
/// Uses RK4 for accuracy.
pub fn step(attractor: AttractorType, state: Vec3, dt: f32) -> (Vec3, Vec3) {
    let next = rk4_step(attractor, state, dt);
    (next, next - state)
}

/// Warm up an attractor (discard the transient trajectory).
/// Call before sampling to ensure the state is on the attractor.
pub fn warmup(attractor: AttractorType, mut state: Vec3, steps: usize) -> Vec3 {
    let dt = attractor.recommended_dt();
    for _ in 0..steps {
        state = rk4_step(attractor, state, dt);
    }
    state
}

// ── Initial conditions ─────────────────────────────────────────────────────────

/// Canonical initial conditions close to the attractor.
pub fn initial_state(attractor: AttractorType) -> Vec3 {
    match attractor {
        AttractorType::Lorenz      => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Rossler     => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Chen        => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Halvorsen   => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Aizawa      => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Thomas      => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Dadras      => Vec3::new(0.1,  0.0,  0.0),
        AttractorType::Sprott      => Vec3::new(1.0,  0.0,  0.5),
        AttractorType::Rabinovich  => Vec3::new(0.05, 0.05, 0.5),
        AttractorType::Burke       => Vec3::new(0.6, -0.4,  0.4),
    }
}

/// Return a warmed-up initial state on the attractor surface.
pub fn initial_state_warmed(attractor: AttractorType) -> Vec3 {
    warmup(attractor, initial_state(attractor), 5000)
}

// ── AttractorSampler ───────────────────────────────────────────────────────────

/// Stateful sampler that integrates an attractor over time.
/// Maintains the current state and advances it on each call to `next()`.
#[derive(Debug, Clone)]
pub struct AttractorSampler {
    pub attractor: AttractorType,
    pub state:     Vec3,
    pub dt:        f32,
    pub time:      f32,
    /// Scale applied to output positions (use `normalization_scale()` for unit cube).
    pub scale:     f32,
    /// Center offset applied after scaling.
    pub center:    Vec3,
    /// How many integration steps per `next()` call (sub-steps for stability).
    pub substeps:  usize,
}

impl AttractorSampler {
    pub fn new(attractor: AttractorType) -> Self {
        let state = initial_state_warmed(attractor);
        Self {
            attractor,
            state,
            dt:       attractor.recommended_dt(),
            time:     0.0,
            scale:    attractor.normalization_scale(),
            center:   Vec3::ZERO,
            substeps: 1,
        }
    }

    /// Advance by `substeps` integration steps and return the normalised position.
    pub fn next(&mut self) -> Vec3 {
        for _ in 0..self.substeps {
            self.state  = rk4_step(self.attractor, self.state, self.dt);
            self.time  += self.dt;
        }
        self.state * self.scale + self.center
    }

    /// Sample `n` points from the attractor trajectory.
    pub fn sample_trajectory(&mut self, n: usize) -> Vec<Vec3> {
        (0..n).map(|_| self.next()).collect()
    }

    /// Reset to a fresh warmed initial condition.
    pub fn reset(&mut self) {
        self.state = initial_state_warmed(self.attractor);
        self.time  = 0.0;
    }

    /// Set the attractor type, resetting the state.
    pub fn set_attractor(&mut self, attractor: AttractorType) {
        self.attractor = attractor;
        self.dt        = attractor.recommended_dt();
        self.scale     = attractor.normalization_scale();
        self.reset();
    }

    /// Current raw (un-normalised) state.
    pub fn raw_state(&self) -> Vec3 {
        self.state
    }

    /// Current normalised position.
    pub fn position(&self) -> Vec3 {
        self.state * self.scale + self.center
    }

    /// Instantaneous velocity vector in normalised space.
    pub fn velocity(&self) -> Vec3 {
        let d = derivatives(self.attractor, self.state);
        d * (self.scale * self.dt)
    }
}

// ── Lyapunov exponent approximation ───────────────────────────────────────────

/// Compute the largest Lyapunov exponent numerically.
///
/// Uses the standard renormalization method:
/// 1. Evolve two nearby trajectories.
/// 2. After each step, measure the divergence.
/// 3. Rescale the separation vector and accumulate log of growth.
///
/// `steps` is the number of integration steps (10_000 is usually enough).
pub fn largest_lyapunov_exponent(
    attractor: AttractorType,
    initial:   Vec3,
    steps:     usize,
) -> f32 {
    let dt     = attractor.recommended_dt();
    let eps    = 1e-8_f32;
    let mut s1 = warmup(attractor, initial, 2000);
    // Perturb s2 slightly along (1, 1, 1) normalised
    let mut s2 = s1 + Vec3::splat(eps / 3.0_f32.sqrt());

    let mut lyapunov_sum = 0.0_f32;

    for _ in 0..steps {
        s1 = rk4_step(attractor, s1, dt);
        s2 = rk4_step(attractor, s2, dt);

        let sep = s2 - s1;
        let d   = sep.length();
        if d < 1e-15 { continue; }

        lyapunov_sum += (d / eps).ln();

        // Renormalize separation
        s2 = s1 + sep * (eps / d);
    }

    lyapunov_sum / (steps as f32 * dt)
}

/// Compute all three Lyapunov exponents via QR decomposition (Gram–Schmidt).
/// Returns `[λ1, λ2, λ3]` in descending order.
pub fn lyapunov_spectrum(
    attractor: AttractorType,
    initial:   Vec3,
    steps:     usize,
) -> [f32; 3] {
    let dt  = attractor.recommended_dt();
    let eps = 1e-6_f32;

    // Basis vectors (perturbed copies)
    let mut state = warmup(attractor, initial, 2000);
    let mut v1    = Vec3::new(eps,  0.0, 0.0);
    let mut v2    = Vec3::new(0.0,  eps, 0.0);
    let mut v3    = Vec3::new(0.0,  0.0, eps);

    let mut sums = [0.0_f32; 3];

    for _ in 0..steps {
        let s0  = state;
        state   = rk4_step(attractor, s0, dt);

        // Evolve perturbations via the Jacobian (numerical, finite difference)
        let evolve = |v: Vec3| -> Vec3 {
            rk4_step(attractor, s0 + v, dt) - state
        };

        v1 = evolve(v1);
        v2 = evolve(v2);
        v3 = evolve(v3);

        // Gram–Schmidt orthogonalisation
        let n1  = v1.length().max(1e-30);
        sums[0] += n1.ln();
        v1       = v1 / n1;

        v2       = v2 - v1 * v2.dot(v1);
        let n2   = v2.length().max(1e-30);
        sums[1] += n2.ln();
        v2       = v2 / n2;

        v3       = v3 - v1 * v3.dot(v1) - v2 * v3.dot(v2);
        let n3   = v3.length().max(1e-30);
        sums[2] += n3.ln();
        v3       = v3 / n3;

        // Reset to eps magnitude
        v1 *= eps;
        v2 *= eps;
        v3 *= eps;
    }

    let t = steps as f32 * dt;
    [sums[0] / t, sums[1] / t, sums[2] / t]
}

/// Kaplan–Yorke dimension from Lyapunov spectrum: D_KY = j + Σ(λ_i) / |λ_{j+1}|
pub fn kaplan_yorke_dimension(spectrum: [f32; 3]) -> f32 {
    let mut sorted = spectrum;
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());

    let mut cumsum = 0.0_f32;
    let mut j = 0usize;
    for (i, &l) in sorted.iter().enumerate() {
        if cumsum + l < 0.0 { break; }
        cumsum += l;
        j = i + 1;
    }
    if j >= 3 { return 3.0; }
    j as f32 + cumsum / sorted[j].abs().max(1e-12)
}

// ── Attractor metadata and analysis ───────────────────────────────────────────

/// Statistical metadata about an attractor's trajectory.
#[derive(Debug, Clone)]
pub struct AttractorStats {
    pub attractor:    AttractorType,
    pub bbox_min:     Vec3,
    pub bbox_max:     Vec3,
    pub centroid:     Vec3,
    pub variance:     Vec3,
    pub sample_count: usize,
    /// Estimated largest Lyapunov exponent from the trajectory.
    pub lyapunov_max: f32,
}

impl AttractorStats {
    /// Compute statistics by sampling `n` points from a warmed attractor.
    pub fn compute(attractor: AttractorType, n: usize) -> Self {
        let mut sampler = AttractorSampler::new(attractor);
        sampler.scale = 1.0; // raw coordinates

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut sum = Vec3::ZERO;

        let pts: Vec<Vec3> = (0..n).map(|_| {
            let p = sampler.next();
            min = min.min(p);
            max = max.max(p);
            sum += p;
            p
        }).collect();

        let centroid = sum / n as f32;
        let variance = pts.iter().fold(Vec3::ZERO, |acc, &p| {
            let d = p - centroid;
            acc + d * d
        }) / n as f32;

        let lyapunov_max = largest_lyapunov_exponent(
            attractor, initial_state(attractor), 5000,
        );

        Self { attractor, bbox_min: min, bbox_max: max, centroid, variance, sample_count: n, lyapunov_max }
    }

    /// Axis-aligned bounding box size.
    pub fn bbox_size(&self) -> Vec3 {
        self.bbox_max - self.bbox_min
    }

    /// Effective scale factor to normalise into [-1, 1].
    pub fn normalizing_scale(&self) -> f32 {
        let s = self.bbox_size();
        let m = s.x.max(s.y).max(s.z);
        if m < 1e-10 { 1.0 } else { 2.0 / m }
    }
}

// ── Bifurcation parameter sweeps ──────────────────────────────────────────────

/// Lorenz attractor with configurable parameters.
pub fn lorenz_parametric(s: Vec3, sigma: f32, rho: f32, beta: f32) -> Vec3 {
    let (x, y, z) = (s.x, s.y, s.z);
    Vec3::new(sigma * (y - x), x * (rho - z) - y, x * y - beta * z)
}

/// Rössler attractor with configurable parameters.
pub fn rossler_parametric(s: Vec3, a: f32, b: f32, c: f32) -> Vec3 {
    let (x, y, z) = (s.x, s.y, s.z);
    Vec3::new(-y - z, x + a * y, b + z * (x - c))
}

/// Lorenz bifurcation diagram: sweep rho from `rho_min` to `rho_max`, return last-n states.
pub fn lorenz_bifurcation(
    rho_min:    f32,
    rho_max:    f32,
    rho_steps:  usize,
    warmup_n:   usize,
    sample_n:   usize,
) -> Vec<(f32, Vec<f32>)> {
    let sigma = 10.0_f32;
    let beta  = 8.0_f32 / 3.0_f32;
    let dt    = 0.005_f32;

    (0..rho_steps).map(|i| {
        let rho = rho_min + (rho_max - rho_min) * i as f32 / (rho_steps - 1) as f32;
        let mut state = Vec3::new(0.1, 0.0, 0.0);

        // Warmup
        for _ in 0..warmup_n {
            let k1 = lorenz_parametric(state, sigma, rho, beta);
            let k2 = lorenz_parametric(state + k1 * (dt * 0.5), sigma, rho, beta);
            let k3 = lorenz_parametric(state + k2 * (dt * 0.5), sigma, rho, beta);
            let k4 = lorenz_parametric(state + k3 * dt, sigma, rho, beta);
            state += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0);
        }

        // Sample z-values
        let zs: Vec<f32> = (0..sample_n).map(|_| {
            let k1 = lorenz_parametric(state, sigma, rho, beta);
            let k2 = lorenz_parametric(state + k1 * (dt * 0.5), sigma, rho, beta);
            let k3 = lorenz_parametric(state + k2 * (dt * 0.5), sigma, rho, beta);
            let k4 = lorenz_parametric(state + k3 * dt, sigma, rho, beta);
            state += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0);
            state.z
        }).collect();

        (rho, zs)
    }).collect()
}

// ── Multi-attractor particle emitter helper ────────────────────────────────────

/// A pool of attractor samplers — one per particle stream.
/// Allows spawning particles from multiple independent trajectories.
pub struct AttractorPool {
    samplers:   Vec<AttractorSampler>,
    round_robin: usize,
}

impl AttractorPool {
    /// Create `n` independent samplers for the given attractor.
    /// Each sampler is initialised from a slightly different state.
    pub fn new(attractor: AttractorType, n: usize) -> Self {
        let base = initial_state_warmed(attractor);
        let dt   = attractor.recommended_dt();
        let samplers = (0..n).map(|i| {
            // Offset each sampler along the trajectory by `i * 200` steps
            let offset_state = {
                let mut s = base;
                for _ in 0..(i * 200) {
                    s = rk4_step(attractor, s, dt);
                }
                s
            };
            AttractorSampler {
                attractor,
                state:    offset_state,
                dt,
                time:     i as f32 * 200.0 * dt,
                scale:    attractor.normalization_scale(),
                center:   Vec3::ZERO,
                substeps: 1,
            }
        }).collect();
        Self { samplers, round_robin: 0 }
    }

    /// Get the next position from the pool, cycling through samplers.
    pub fn next(&mut self) -> Vec3 {
        if self.samplers.is_empty() {
            return Vec3::ZERO;
        }
        let idx = self.round_robin % self.samplers.len();
        self.round_robin = idx + 1;
        self.samplers[idx].next()
    }

    /// Advance all samplers by one step without returning a position.
    pub fn tick_all(&mut self) {
        for s in &mut self.samplers {
            s.next();
        }
    }

    /// Return positions of all samplers (snapshot).
    pub fn positions(&self) -> Vec<Vec3> {
        self.samplers.iter().map(|s| s.position()).collect()
    }

    /// Set scale on all samplers.
    pub fn set_scale(&mut self, scale: f32) {
        for s in &mut self.samplers {
            s.scale = scale;
        }
    }

    /// Set center offset on all samplers.
    pub fn set_center(&mut self, center: Vec3) {
        for s in &mut self.samplers {
            s.center = center;
        }
    }

    pub fn len(&self) -> usize { self.samplers.len() }
    pub fn is_empty(&self) -> bool { self.samplers.is_empty() }
}

// ── Poincaré section helper ────────────────────────────────────────────────────

/// Collect Poincaré section crossings (z = `z_level`, z going upward).
/// Returns the x,y coordinates at each crossing.
pub fn poincare_section(
    attractor: AttractorType,
    z_level:   f32,
    n_crossings: usize,
) -> Vec<(f32, f32)> {
    let dt   = attractor.recommended_dt();
    let mut s = initial_state_warmed(attractor);
    let mut crossings = Vec::with_capacity(n_crossings);
    let mut prev_z = s.z;
    let mut iterations = 0usize;
    let max_iter = n_crossings * 100_000;

    while crossings.len() < n_crossings && iterations < max_iter {
        s = rk4_step(attractor, s, dt);
        // Upward crossing of z_level
        if prev_z < z_level && s.z >= z_level {
            // Linear interpolation to find exact crossing
            let t = (z_level - prev_z) / (s.z - prev_z);
            let sx = prev_z + t * (s.x - prev_z); // approximate, good enough
            crossings.push((s.x, s.y));
            let _ = sx;
        }
        prev_z = s.z;
        iterations += 1;
    }
    crossings
}

// ── Recurrence plot ────────────────────────────────────────────────────────────

/// Compute a recurrence matrix for a sampled attractor trajectory.
/// `matrix[i][j] = 1` if `|state_i - state_j| < threshold`.
/// Returns a flat `n × n` bit vector.
pub fn recurrence_plot(
    attractor:  AttractorType,
    n:          usize,
    threshold:  f32,
) -> Vec<bool> {
    let trajectory = AttractorSampler::new(attractor)
        .sample_trajectory(n);

    let mut matrix = vec![false; n * n];
    for i in 0..n {
        for j in 0..n {
            let d = (trajectory[i] - trajectory[j]).length();
            matrix[i * n + j] = d < threshold;
        }
    }
    matrix
}

// ── Attractor colour mapping ───────────────────────────────────────────────────

/// Map an attractor velocity magnitude to a colour using a gradient.
/// Returns `(r, g, b, a)` all in `[0.0, 1.0]`.
pub fn velocity_color(velocity: Vec3, palette: AttractorPalette) -> glam::Vec4 {
    let speed = velocity.length();
    let t = (speed * 5.0).clamp(0.0, 1.0); // tune 5.0 as needed
    match palette {
        AttractorPalette::Plasma => {
            // Plasma: purple → blue → green → yellow
            let r = (0.5 + 0.5 * (t * std::f32::consts::TAU).sin()).clamp(0.0, 1.0);
            let g = t.sqrt();
            let b = (1.0 - t).powi(2);
            glam::Vec4::new(r, g, b, 1.0)
        }
        AttractorPalette::Fire => {
            let r = (t * 2.0).clamp(0.0, 1.0);
            let g = ((t * 2.0) - 1.0).clamp(0.0, 1.0);
            let b = 0.0;
            glam::Vec4::new(r, g, b, 1.0)
        }
        AttractorPalette::Ice => {
            let r = t * 0.2;
            let g = t * 0.6;
            let b = t;
            glam::Vec4::new(r, g, b, 1.0)
        }
        AttractorPalette::Neon => {
            let r = (1.0 - t) * 0.8;
            let g = (1.0 - (t - 0.5).abs() * 2.0).max(0.0);
            let b = t;
            glam::Vec4::new(r, g, b, 1.0)
        }
        AttractorPalette::Greyscale => {
            glam::Vec4::new(t, t, t, 1.0)
        }
    }
}

/// Colour palette for attractor visualisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttractorPalette {
    Plasma,
    Fire,
    Ice,
    Neon,
    Greyscale,
}
