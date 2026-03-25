//! Poisson processes: homogeneous, non-homogeneous, and compound.
//!
//! Models event arrivals where events occur independently at a given rate.

use super::brownian::Rng;

// ---------------------------------------------------------------------------
// PoissonProcess (homogeneous)
// ---------------------------------------------------------------------------

/// Homogeneous Poisson process with constant rate lambda.
pub struct PoissonProcess {
    /// Events per unit time.
    pub rate: f64,
}

impl PoissonProcess {
    pub fn new(rate: f64) -> Self {
        assert!(rate > 0.0, "rate must be positive");
        Self { rate }
    }

    /// Generate the next inter-arrival time (exponential distribution).
    /// T ~ Exp(rate) => T = -ln(U) / rate
    pub fn next_arrival(&self, rng: &mut Rng) -> f64 {
        let u = rng.uniform().max(1e-15);
        -u.ln() / self.rate
    }

    /// Generate all arrival times in [0, duration].
    pub fn arrivals(&self, rng: &mut Rng, duration: f64) -> Vec<f64> {
        let mut times = Vec::new();
        let mut t = 0.0;
        loop {
            t += self.next_arrival(rng);
            if t > duration {
                break;
            }
            times.push(t);
        }
        times
    }

    /// Count distribution: N(t) ~ Poisson(rate * t).
    pub fn count(&self, duration: f64) -> PoissonDistribution {
        PoissonDistribution {
            lambda: self.rate * duration,
        }
    }

    /// Generate a counting process path: (time, count) pairs.
    pub fn counting_path(&self, rng: &mut Rng, duration: f64) -> Vec<(f64, usize)> {
        let arrivals = self.arrivals(rng, duration);
        let mut path = Vec::with_capacity(arrivals.len() + 2);
        path.push((0.0, 0));
        for (i, &t) in arrivals.iter().enumerate() {
            path.push((t, i + 1));
        }
        path.push((duration, arrivals.len()));
        path
    }

    /// Superposition of two independent Poisson processes.
    pub fn superpose(&self, other: &PoissonProcess) -> PoissonProcess {
        PoissonProcess::new(self.rate + other.rate)
    }

    /// Thin this process with probability p to get rate * p.
    pub fn thin(&self, p: f64) -> PoissonProcess {
        assert!(p > 0.0 && p <= 1.0);
        PoissonProcess::new(self.rate * p)
    }
}

// ---------------------------------------------------------------------------
// PoissonDistribution
// ---------------------------------------------------------------------------

/// Poisson distribution with parameter lambda.
pub struct PoissonDistribution {
    pub lambda: f64,
}

impl PoissonDistribution {
    pub fn new(lambda: f64) -> Self {
        Self { lambda }
    }

    /// P(N = k) = e^{-lambda} * lambda^k / k!
    pub fn pmf(&self, k: usize) -> f64 {
        (-self.lambda).exp() * self.lambda.powi(k as i32) / factorial(k) as f64
    }

    /// P(N <= k)
    pub fn cdf(&self, k: usize) -> f64 {
        (0..=k).map(|i| self.pmf(i)).sum()
    }

    /// Expected value: lambda.
    pub fn mean(&self) -> f64 {
        self.lambda
    }

    /// Variance: lambda.
    pub fn variance(&self) -> f64 {
        self.lambda
    }

    /// Sample a Poisson random variable using the inverse transform method.
    pub fn sample(&self, rng: &mut Rng) -> usize {
        // Knuth algorithm
        let l = (-self.lambda).exp();
        let mut k = 0usize;
        let mut p = 1.0;
        loop {
            k += 1;
            p *= rng.uniform();
            if p <= l {
                return k - 1;
            }
        }
    }
}

fn factorial(n: usize) -> f64 {
    if n <= 1 {
        1.0
    } else {
        (2..=n).fold(1.0, |acc, i| acc * i as f64)
    }
}

// ---------------------------------------------------------------------------
// NonHomogeneousPoisson
// ---------------------------------------------------------------------------

/// Non-homogeneous Poisson process with time-varying rate function lambda(t).
pub struct NonHomogeneousPoisson {
    /// Rate function lambda(t).
    pub rate_fn: Box<dyn Fn(f64) -> f64>,
    /// Upper bound on the rate function (for thinning algorithm).
    pub rate_max: f64,
}

impl NonHomogeneousPoisson {
    pub fn new(rate_fn: Box<dyn Fn(f64) -> f64>, rate_max: f64) -> Self {
        Self { rate_fn, rate_max }
    }

    /// Generate arrival times using the thinning (Lewis-Shedler) algorithm.
    ///
    /// 1. Generate candidate arrival from homogeneous Poisson(rate_max)
    /// 2. Accept with probability lambda(t) / rate_max
    pub fn arrivals(&self, rng: &mut Rng, duration: f64) -> Vec<f64> {
        let mut times = Vec::new();
        let mut t = 0.0;
        let homogeneous = PoissonProcess::new(self.rate_max);

        loop {
            t += homogeneous.next_arrival(rng);
            if t > duration {
                break;
            }
            // Accept/reject
            let acceptance_prob = (self.rate_fn)(t) / self.rate_max;
            if rng.uniform() < acceptance_prob {
                times.push(t);
            }
        }
        times
    }

    /// Generate counting process path.
    pub fn counting_path(&self, rng: &mut Rng, duration: f64) -> Vec<(f64, usize)> {
        let arrivals = self.arrivals(rng, duration);
        let mut path = Vec::with_capacity(arrivals.len() + 2);
        path.push((0.0, 0));
        for (i, &t) in arrivals.iter().enumerate() {
            path.push((t, i + 1));
        }
        path.push((duration, arrivals.len()));
        path
    }

    /// Expected number of arrivals in [0, t] = integral of lambda(s) ds.
    /// Computed via simple trapezoidal rule.
    pub fn expected_count(&self, duration: f64, n_points: usize) -> f64 {
        let dx = duration / n_points as f64;
        let mut sum = 0.5 * ((self.rate_fn)(0.0) + (self.rate_fn)(duration));
        for i in 1..n_points {
            sum += (self.rate_fn)(i as f64 * dx);
        }
        sum * dx
    }
}

// ---------------------------------------------------------------------------
// CompoundPoisson
// ---------------------------------------------------------------------------

/// Compound Poisson process: S(t) = sum_{i=1}^{N(t)} X_i
/// where N(t) is Poisson(rate*t) and X_i are iid from jump_distribution.
pub struct CompoundPoisson {
    /// Rate of underlying Poisson process.
    pub rate: f64,
    /// Jump size distribution: function that generates a random jump.
    pub jump_distribution: Box<dyn Fn(&mut Rng) -> f64>,
}

impl CompoundPoisson {
    pub fn new(rate: f64, jump_distribution: Box<dyn Fn(&mut Rng) -> f64>) -> Self {
        Self { rate, jump_distribution }
    }

    /// Create a compound Poisson with normally distributed jumps.
    pub fn normal_jumps(rate: f64, jump_mean: f64, jump_std: f64) -> Self {
        Self {
            rate,
            jump_distribution: Box::new(move |rng: &mut Rng| rng.normal_with(jump_mean, jump_std)),
        }
    }

    /// Create a compound Poisson with exponentially distributed jumps.
    pub fn exponential_jumps(rate: f64, jump_rate: f64) -> Self {
        Self {
            rate,
            jump_distribution: Box::new(move |rng: &mut Rng| {
                let u = rng.uniform().max(1e-15);
                -u.ln() / jump_rate
            }),
        }
    }

    /// Generate the process path: Vec of (time, cumulative_sum).
    pub fn path(&self, rng: &mut Rng, duration: f64) -> Vec<(f64, f64)> {
        let pp = PoissonProcess::new(self.rate);
        let arrivals = pp.arrivals(rng, duration);
        let mut path = Vec::with_capacity(arrivals.len() + 2);
        path.push((0.0, 0.0));
        let mut cumsum = 0.0;
        for &t in &arrivals {
            cumsum += (self.jump_distribution)(rng);
            path.push((t, cumsum));
        }
        path.push((duration, cumsum));
        path
    }

    /// Generate just the jump values at each arrival.
    pub fn jumps(&self, rng: &mut Rng, duration: f64) -> Vec<(f64, f64)> {
        let pp = PoissonProcess::new(self.rate);
        let arrivals = pp.arrivals(rng, duration);
        arrivals
            .iter()
            .map(|&t| (t, (self.jump_distribution)(rng)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poisson_arrival_positive() {
        let pp = PoissonProcess::new(5.0);
        let mut rng = Rng::new(42);
        for _ in 0..100 {
            let t = pp.next_arrival(&mut rng);
            assert!(t > 0.0, "inter-arrival time should be positive");
        }
    }

    #[test]
    fn test_poisson_mean_count() {
        // E[N(t)] = rate * t
        let rate = 3.0;
        let duration = 10.0;
        let pp = PoissonProcess::new(rate);
        let trials = 5000;
        let mut rng = Rng::new(42);

        let total_count: usize = (0..trials)
            .map(|_| pp.arrivals(&mut rng, duration).len())
            .sum();
        let empirical_mean = total_count as f64 / trials as f64;
        let expected = rate * duration; // 30

        assert!(
            (empirical_mean - expected).abs() < 2.0,
            "mean count {} should be ~{}",
            empirical_mean,
            expected
        );
    }

    #[test]
    fn test_poisson_inter_arrivals_exponential() {
        // Inter-arrival times should have mean 1/rate
        let rate = 4.0;
        let pp = PoissonProcess::new(rate);
        let mut rng = Rng::new(123);
        let n = 10_000;
        let inter_arrivals: Vec<f64> = (0..n).map(|_| pp.next_arrival(&mut rng)).collect();
        let mean = inter_arrivals.iter().sum::<f64>() / n as f64;
        let expected = 1.0 / rate;

        assert!(
            (mean - expected).abs() < 0.05,
            "inter-arrival mean {} should be ~{}",
            mean,
            expected
        );
    }

    #[test]
    fn test_poisson_arrivals_sorted() {
        let pp = PoissonProcess::new(2.0);
        let mut rng = Rng::new(42);
        let arrivals = pp.arrivals(&mut rng, 100.0);
        for w in arrivals.windows(2) {
            assert!(w[0] < w[1], "arrivals should be sorted");
        }
    }

    #[test]
    fn test_poisson_distribution_pmf() {
        let pd = PoissonDistribution::new(3.0);
        // P(0) = e^{-3} ≈ 0.0498
        assert!((pd.pmf(0) - (-3.0_f64).exp()).abs() < 1e-10);
        // Sum of all probabilities should be ~1
        let total: f64 = (0..30).map(|k| pd.pmf(k)).sum();
        assert!((total - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_poisson_distribution_sample_mean() {
        let pd = PoissonDistribution::new(5.0);
        let mut rng = Rng::new(42);
        let n = 10_000;
        let mean: f64 = (0..n).map(|_| pd.sample(&mut rng) as f64).sum::<f64>() / n as f64;
        assert!(
            (mean - 5.0).abs() < 0.3,
            "sample mean {} should be ~5",
            mean
        );
    }

    #[test]
    fn test_nhpp_thinning() {
        // Sinusoidal rate: lambda(t) = 5 + 3*sin(t), max = 8
        let nhpp = NonHomogeneousPoisson::new(
            Box::new(|t: f64| 5.0 + 3.0 * t.sin()),
            8.0,
        );
        let mut rng = Rng::new(42);
        let arrivals = nhpp.arrivals(&mut rng, 10.0);
        // Should have some events
        assert!(!arrivals.is_empty());
        // All within duration
        assert!(arrivals.iter().all(|&t| t >= 0.0 && t <= 10.0));
    }

    #[test]
    fn test_compound_poisson() {
        let cp = CompoundPoisson::normal_jumps(2.0, 1.0, 0.5);
        let mut rng = Rng::new(42);
        let path = cp.path(&mut rng, 10.0);
        assert!(path.len() >= 2); // at least start and end
        assert_eq!(path[0], (0.0, 0.0));
    }

    #[test]
    fn test_superpose_and_thin() {
        let p1 = PoissonProcess::new(3.0);
        let p2 = PoissonProcess::new(5.0);
        let merged = p1.superpose(&p2);
        assert!((merged.rate - 8.0).abs() < 1e-10);

        let thinned = merged.thin(0.5);
        assert!((thinned.rate - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_counting_path() {
        let pp = PoissonProcess::new(2.0);
        let mut rng = Rng::new(42);
        let path = pp.counting_path(&mut rng, 5.0);
        assert_eq!(path[0], (0.0, 0));
        // Count should be monotonically non-decreasing
        for w in path.windows(2) {
            assert!(w[1].1 >= w[0].1);
        }
    }
}
