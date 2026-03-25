//! Statistics and probability: descriptive stats, distributions, hypothesis testing,
//! regression, Bayesian inference, random number generation, information theory.

use std::f64::consts::PI;

// ============================================================
// RANDOM NUMBER GENERATORS
// ============================================================

/// Trait for random number generators.
pub trait Rng {
    fn next_u64(&mut self) -> u64;
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
}

/// Xorshift64 — fast, simple 64-bit RNG.
#[derive(Clone, Debug)]
pub struct Xorshift64 {
    pub state: u64,
}

impl Xorshift64 {
    pub fn new(seed: u64) -> Self { Self { state: seed.max(1) } }
}

impl Rng for Xorshift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

/// PCG32 — Permuted Congruential Generator.
#[derive(Clone, Debug)]
pub struct Pcg32 {
    pub state: u64,
    pub inc: u64,
}

impl Pcg32 {
    pub fn new(seed: u64, seq: u64) -> Self {
        let mut rng = Self { state: 0, inc: (seq << 1) | 1 };
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u64();
        rng
    }
}

impl Rng for Pcg32 {
    fn next_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        lo | (hi << 32)
    }

    fn next_u32(&mut self) -> u32 {
        let old_state = self.state;
        self.state = old_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old_state >> 18) ^ old_state) >> 27) as u32;
        let rot = (old_state >> 59) as u32;
        xorshifted.rotate_right(rot)
    }
}

/// SplitMix64 — fast 64-bit generator suitable as seed scrambler.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    pub state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self { Self { state: seed } }
}

impl Rng for SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

/// Linear Congruential Generator.
#[derive(Clone, Debug)]
pub struct Lcg {
    pub state: u64,
    pub a: u64,
    pub c: u64,
    pub m: u64,
}

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed,
            a: 6_364_136_223_846_793_005,
            c: 1_442_695_040_888_963_407,
            m: u64::MAX,
        }
    }
}

impl Rng for Lcg {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(self.a).wrapping_add(self.c);
        self.state
    }
}

/// Fisher-Yates shuffle.
pub fn shuffle<T>(data: &mut [T], rng: &mut impl Rng) {
    let n = data.len();
    for i in (1..n).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        data.swap(i, j);
    }
}

/// Sample k distinct indices from 0..n without replacement (Knuth's algorithm S).
pub fn sample_without_replacement(n: usize, k: usize, rng: &mut impl Rng) -> Vec<usize> {
    let k = k.min(n);
    let mut result = Vec::with_capacity(k);
    let mut needed = k;
    let mut available = n;
    for i in 0..n {
        let u = rng.next_f64();
        if u < needed as f64 / available as f64 {
            result.push(i);
            needed -= 1;
            if needed == 0 { break; }
        }
        available -= 1;
    }
    result
}

/// Weighted sampling — draw one index proportional to weights.
pub fn weighted_sample(weights: &[f64], rng: &mut impl Rng) -> usize {
    let total: f64 = weights.iter().sum();
    let mut r = rng.next_f64() * total;
    for (i, &w) in weights.iter().enumerate() {
        r -= w;
        if r <= 0.0 { return i; }
    }
    weights.len() - 1
}

// ============================================================
// DESCRIPTIVE STATISTICS
// ============================================================

/// Arithmetic mean.
pub fn mean(data: &[f64]) -> f64 {
    if data.is_empty() { return 0.0; }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Sample variance (Bessel's correction, n-1 denominator).
pub fn variance(data: &[f64]) -> f64 {
    let n = data.len();
    if n < 2 { return 0.0; }
    let m = mean(data);
    data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64
}

/// Sample standard deviation.
pub fn std_dev(data: &[f64]) -> f64 { variance(data).sqrt() }

/// Median (sorts the slice in place).
pub fn median(data: &mut [f64]) -> f64 {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = data.len();
    if n == 0 { return 0.0; }
    if n % 2 == 0 { (data[n / 2 - 1] + data[n / 2]) / 2.0 } else { data[n / 2] }
}

/// Mode(s) — returns all values that appear most frequently.
pub fn mode(data: &[f64]) -> Vec<f64> {
    if data.is_empty() { return vec![]; }
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut modes = Vec::new();
    let mut max_count = 0usize;
    let mut count = 1usize;
    for i in 1..sorted.len() {
        if (sorted[i] - sorted[i - 1]).abs() < 1e-12 {
            count += 1;
        } else {
            if count > max_count { max_count = count; modes.clear(); modes.push(sorted[i - 1]); }
            else if count == max_count { modes.push(sorted[i - 1]); }
            count = 1;
        }
    }
    let last = *sorted.last().unwrap();
    if count > max_count { modes = vec![last]; }
    else if count == max_count { modes.push(last); }
    modes
}

/// p-th percentile (p in [0,100]).
pub fn percentile(data: &mut [f64], p: f64) -> f64 {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = data.len();
    if n == 0 { return 0.0; }
    let idx = (p / 100.0 * (n - 1) as f64).clamp(0.0, (n - 1) as f64);
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let frac = idx - lo as f64;
    data[lo] + frac * (data[hi] - data[lo])
}

/// Interquartile range.
pub fn iqr(data: &mut [f64]) -> f64 {
    let q3 = percentile(data, 75.0);
    let q1 = percentile(data, 25.0);
    q3 - q1
}

/// Sample skewness.
pub fn skewness(data: &[f64]) -> f64 {
    let n = data.len() as f64;
    if n < 3.0 { return 0.0; }
    let m = mean(data);
    let s = std_dev(data);
    if s == 0.0 { return 0.0; }
    let sum: f64 = data.iter().map(|x| ((x - m) / s).powi(3)).sum();
    sum * n / ((n - 1.0) * (n - 2.0))
}

/// Sample excess kurtosis.
pub fn kurtosis(data: &[f64]) -> f64 {
    let n = data.len() as f64;
    if n < 4.0 { return 0.0; }
    let m = mean(data);
    let s = std_dev(data);
    if s == 0.0 { return 0.0; }
    let sum: f64 = data.iter().map(|x| ((x - m) / s).powi(4)).sum();
    let g2 = sum * n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0))
        - 3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0));
    g2
}

/// Sample covariance.
pub fn covariance(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 { return 0.0; }
    let mx = mean(x);
    let my = mean(y);
    x.iter().zip(y.iter()).map(|(xi, yi)| (xi - mx) * (yi - my)).sum::<f64>() / (n - 1) as f64
}

/// Pearson correlation coefficient.
pub fn pearson_r(x: &[f64], y: &[f64]) -> f64 {
    let cov = covariance(x, y);
    let sx = std_dev(x);
    let sy = std_dev(y);
    if sx == 0.0 || sy == 0.0 { return 0.0; }
    cov / (sx * sy)
}

/// Spearman rank correlation.
pub fn spearman_rho(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 { return 0.0; }
    let rank = |data: &[f64]| -> Vec<f64> {
        let mut indexed: Vec<(usize, f64)> = data.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let mut ranks = vec![0.0f64; indexed.len()];
        let mut i = 0;
        while i < indexed.len() {
            let mut j = i;
            while j < indexed.len() && (indexed[j].1 - indexed[i].1).abs() < 1e-12 { j += 1; }
            let avg_rank = (i + j - 1) as f64 / 2.0 + 1.0;
            for k in i..j { ranks[indexed[k].0] = avg_rank; }
            i = j;
        }
        ranks
    };
    let rx: Vec<f64> = rank(&x[..n]);
    let ry: Vec<f64> = rank(&y[..n]);
    pearson_r(&rx, &ry)
}

// ============================================================
// SPECIAL FUNCTIONS
// ============================================================

/// Error function erf(x).
pub fn erf(x: f64) -> f64 {
    // Abramowitz & Stegun approximation 7.1.26
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let poly = t * (0.254829592
        + t * (-0.284496736
        + t * (1.421413741
        + t * (-1.453152027
        + t * 1.061405429))));
    let result = 1.0 - poly * (-x * x).exp();
    if x >= 0.0 { result } else { -result }
}

/// Complementary error function erfc(x).
pub fn erfc(x: f64) -> f64 { 1.0 - erf(x) }

/// Natural log of gamma function (Lanczos approximation).
pub fn lgamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];
    if x < 0.5 {
        return (PI / ((PI * x).sin())).ln() - lgamma(1.0 - x);
    }
    let z = x - 1.0;
    let mut t = z + G + 0.5;
    let mut s = C[0];
    for i in 1..9 { s += C[i] / (z + i as f64); }
    0.5 * (2.0 * PI).ln() + s.ln() + (z + 0.5) * t.ln() - t
}

/// Gamma function.
pub fn gamma(x: f64) -> f64 { lgamma(x).exp() }

/// Regularized incomplete gamma function P(a, x) — lower.
pub fn gammainc_lower(a: f64, x: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    if x < a + 1.0 {
        // Series expansion
        let mut term = 1.0 / a;
        let mut sum = term;
        for n in 1..200usize {
            term *= x / (a + n as f64);
            sum += term;
            if term.abs() < sum.abs() * 1e-12 { break; }
        }
        sum * (-x + a * x.ln() - lgamma(a)).exp()
    } else {
        // Continued fraction (Lentz's method)
        let eps = 1e-12;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / 1e-300;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..200i64 {
            let an = -i as f64 * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < 1e-300 { d = 1e-300; }
            c = b + an / c;
            if c.abs() < 1e-300 { c = 1e-300; }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < eps { break; }
        }
        1.0 - (-x + a * x.ln() - lgamma(a)).exp() * h
    }
}

/// Regularized incomplete beta function I_x(a,b).
pub fn betainc(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    if x >= 1.0 { return 1.0; }
    let lbeta = lgamma(a) + lgamma(b) - lgamma(a + b);
    let factor = (a * x.ln() + b * (1.0 - x).ln() - lbeta).exp();
    // Use symmetry relation for convergence
    if x < (a + 1.0) / (a + b + 2.0) {
        factor * betacf(x, a, b) / a
    } else {
        1.0 - factor * betacf(1.0 - x, b, a) / b
    }
}

fn betacf(x: f64, a: f64, b: f64) -> f64 {
    let max_iter = 200;
    let eps = 1e-12;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < 1e-300 { d = 1e-300; }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=max_iter {
        let m = m as f64;
        let m2 = 2.0 * m;
        let mut aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-300 { d = 1e-300; }
        c = 1.0 + aa / c;
        if c.abs() < 1e-300 { c = 1e-300; }
        d = 1.0 / d;
        h *= d * c;
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-300 { d = 1e-300; }
        c = 1.0 + aa / c;
        if c.abs() < 1e-300 { c = 1e-300; }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < eps { break; }
    }
    h
}

/// Inverse normal CDF (probit function) via rational approximation.
pub fn probit(p: f64) -> f64 {
    let p = p.clamp(1e-12, 1.0 - 1e-12);
    let sign = if p < 0.5 { -1.0 } else { 1.0 };
    let q = if p < 0.5 { p } else { 1.0 - p };
    let t = (-2.0 * q.ln()).sqrt();
    const C: [f64; 3] = [2.515517, 0.802853, 0.010328];
    const D: [f64; 3] = [1.432788, 0.189269, 0.001308];
    let num = C[0] + C[1] * t + C[2] * t * t;
    let den = 1.0 + D[0] * t + D[1] * t * t + D[2] * t * t * t;
    sign * (t - num / den)
}

/// Two-tailed p-value from t statistic with df degrees of freedom.
pub fn p_value_from_t(t: f64, df: f64) -> f64 {
    // CDF of t-distribution via regularized incomplete beta
    let x = df / (df + t * t);
    let p_one_tail = 0.5 * betainc(x, df / 2.0, 0.5);
    (2.0 * p_one_tail).min(1.0)
}

/// p-value from chi-squared statistic with k degrees of freedom.
pub fn p_value_from_chi2(chi2: f64, k: usize) -> f64 {
    if chi2 <= 0.0 { return 1.0; }
    1.0 - gammainc_lower(k as f64 / 2.0, chi2 / 2.0)
}

// ============================================================
// PROBABILITY DISTRIBUTIONS
// ============================================================

/// Normal (Gaussian) distribution.
#[derive(Clone, Debug)]
pub struct NormalDist {
    pub mean: f64,
    pub std_dev: f64,
}

impl NormalDist {
    pub fn pdf(&self, x: f64) -> f64 {
        let z = (x - self.mean) / self.std_dev;
        (-0.5 * z * z).exp() / (self.std_dev * (2.0 * PI).sqrt())
    }
    pub fn cdf(&self, x: f64) -> f64 {
        0.5 * (1.0 + erf((x - self.mean) / (self.std_dev * 2.0f64.sqrt())))
    }
    pub fn inv_cdf(&self, p: f64) -> f64 {
        self.mean + self.std_dev * probit(p)
    }
    /// Box-Muller sampling. Returns two independent samples.
    pub fn sample_pair(&self, rng: &mut impl Rng) -> (f64, f64) {
        let u1 = rng.next_f64().max(1e-300);
        let u2 = rng.next_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * PI * u2;
        let z0 = r * theta.cos();
        let z1 = r * theta.sin();
        (self.mean + self.std_dev * z0, self.mean + self.std_dev * z1)
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 { self.sample_pair(rng).0 }
}

/// Continuous uniform distribution.
#[derive(Clone, Debug)]
pub struct UniformDist {
    pub min: f64,
    pub max: f64,
}

impl UniformDist {
    pub fn pdf(&self, x: f64) -> f64 {
        if x >= self.min && x <= self.max { 1.0 / (self.max - self.min) } else { 0.0 }
    }
    pub fn cdf(&self, x: f64) -> f64 {
        ((x - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }
    pub fn inv_cdf(&self, p: f64) -> f64 { self.min + p * (self.max - self.min) }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 { self.inv_cdf(rng.next_f64()) }
}

/// Exponential distribution.
#[derive(Clone, Debug)]
pub struct ExponentialDist {
    pub lambda: f64,
}

impl ExponentialDist {
    pub fn pdf(&self, x: f64) -> f64 {
        if x < 0.0 { 0.0 } else { self.lambda * (-self.lambda * x).exp() }
    }
    pub fn cdf(&self, x: f64) -> f64 {
        if x < 0.0 { 0.0 } else { 1.0 - (-self.lambda * x).exp() }
    }
    pub fn inv_cdf(&self, p: f64) -> f64 { -((1.0 - p).max(1e-300)).ln() / self.lambda }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 { self.inv_cdf(rng.next_f64()) }
}

/// Poisson distribution.
#[derive(Clone, Debug)]
pub struct PoissonDist {
    pub lambda: f64,
}

impl PoissonDist {
    pub fn pmf(&self, k: u64) -> f64 {
        (-self.lambda).exp() * self.lambda.powi(k as i32) / gamma(k as f64 + 1.0)
    }
    pub fn cdf(&self, k: u64) -> f64 {
        (0..=k).map(|i| self.pmf(i)).sum()
    }
    /// Knuth algorithm for Poisson sampling.
    pub fn sample(&self, rng: &mut impl Rng) -> u64 {
        let l = (-self.lambda).exp();
        let mut k = 0u64;
        let mut p = 1.0;
        loop {
            k += 1;
            p *= rng.next_f64();
            if p <= l { break; }
        }
        k - 1
    }
}

/// Binomial distribution.
#[derive(Clone, Debug)]
pub struct BinomialDist {
    pub n: u64,
    pub p: f64,
}

impl BinomialDist {
    pub fn pmf(&self, k: u64) -> f64 {
        if k > self.n { return 0.0; }
        let log_coeff = lgamma(self.n as f64 + 1.0)
            - lgamma(k as f64 + 1.0)
            - lgamma((self.n - k) as f64 + 1.0);
        (log_coeff + k as f64 * self.p.ln() + (self.n - k) as f64 * (1.0 - self.p).ln()).exp()
    }
    pub fn cdf(&self, k: u64) -> f64 {
        (0..=k).map(|i| self.pmf(i)).sum()
    }
    pub fn sample(&self, rng: &mut impl Rng) -> u64 {
        (0..self.n).filter(|_| rng.next_f64() < self.p).count() as u64
    }
}

/// Beta distribution (Johnk's method for sampling).
#[derive(Clone, Debug)]
pub struct BetaDist {
    pub alpha: f64,
    pub beta: f64,
}

impl BetaDist {
    pub fn pdf(&self, x: f64) -> f64 {
        if x <= 0.0 || x >= 1.0 { return 0.0; }
        let lbeta = lgamma(self.alpha) + lgamma(self.beta) - lgamma(self.alpha + self.beta);
        ((self.alpha - 1.0) * x.ln() + (self.beta - 1.0) * (1.0 - x).ln() - lbeta).exp()
    }
    pub fn cdf(&self, x: f64) -> f64 { betainc(x, self.alpha, self.beta) }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 {
        // Johnk's method
        loop {
            let u = rng.next_f64();
            let v = rng.next_f64();
            let x = u.powf(1.0 / self.alpha);
            let y = v.powf(1.0 / self.beta);
            if x + y <= 1.0 { return x / (x + y); }
        }
    }
}

/// Gamma distribution (Marsaglia-Tsang method for alpha >= 1).
#[derive(Clone, Debug)]
pub struct GammaDist {
    pub shape: f64,  // alpha / k
    pub scale: f64,  // theta
}

impl GammaDist {
    pub fn pdf(&self, x: f64) -> f64 {
        if x <= 0.0 { return 0.0; }
        let log_scale = self.scale.ln();
        ((self.shape - 1.0) * x.ln() - x / self.scale - self.shape * log_scale - lgamma(self.shape)).exp()
    }
    pub fn cdf(&self, x: f64) -> f64 {
        if x <= 0.0 { return 0.0; }
        gammainc_lower(self.shape, x / self.scale)
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 {
        let alpha = self.shape;
        let s = if alpha >= 1.0 {
            // Marsaglia-Tsang
            let d = alpha - 1.0 / 3.0;
            let c = 1.0 / (9.0 * d).sqrt();
            let norm = NormalDist { mean: 0.0, std_dev: 1.0 };
            loop {
                let x = norm.sample(rng);
                let v = (1.0 + c * x).powi(3);
                if v <= 0.0 { continue; }
                let u = rng.next_f64();
                if u < 1.0 - 0.0331 * (x * x).powi(2) { break d * v; }
                if u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) { break d * v; }
            }
        } else {
            // alpha < 1: use alpha+1 and scale
            let d = alpha + 1.0 - 1.0 / 3.0;
            let c = 1.0 / (9.0 * d).sqrt();
            let norm = NormalDist { mean: 0.0, std_dev: 1.0 };
            let s_plus1 = loop {
                let x = norm.sample(rng);
                let v = (1.0 + c * x).powi(3);
                if v <= 0.0 { continue; }
                let u = rng.next_f64();
                if u < 1.0 - 0.0331 * (x * x).powi(2) { break d * v; }
                if u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) { break d * v; }
            };
            s_plus1 * rng.next_f64().powf(1.0 / alpha)
        };
        s * self.scale
    }
}

/// Log-Normal distribution.
#[derive(Clone, Debug)]
pub struct LogNormalDist {
    pub mu: f64,
    pub sigma: f64,
}

impl LogNormalDist {
    pub fn pdf(&self, x: f64) -> f64 {
        if x <= 0.0 { return 0.0; }
        let z = (x.ln() - self.mu) / self.sigma;
        (-0.5 * z * z).exp() / (x * self.sigma * (2.0 * PI).sqrt())
    }
    pub fn cdf(&self, x: f64) -> f64 {
        if x <= 0.0 { return 0.0; }
        0.5 * (1.0 + erf((x.ln() - self.mu) / (self.sigma * 2.0f64.sqrt())))
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 {
        let norm = NormalDist { mean: self.mu, std_dev: self.sigma };
        norm.sample(rng).exp()
    }
}

/// Weibull distribution.
#[derive(Clone, Debug)]
pub struct WeibullDist {
    pub shape: f64,   // k
    pub scale: f64,   // lambda
}

impl WeibullDist {
    pub fn pdf(&self, x: f64) -> f64 {
        if x < 0.0 { return 0.0; }
        let k = self.shape; let l = self.scale;
        (k / l) * (x / l).powf(k - 1.0) * (-(x / l).powf(k)).exp()
    }
    pub fn cdf(&self, x: f64) -> f64 {
        if x < 0.0 { return 0.0; }
        1.0 - (-(x / self.scale).powf(self.shape)).exp()
    }
    pub fn inv_cdf(&self, p: f64) -> f64 {
        self.scale * (-(1.0 - p).ln()).powf(1.0 / self.shape)
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 { self.inv_cdf(rng.next_f64()) }
}

/// Cauchy distribution.
#[derive(Clone, Debug)]
pub struct CauchyDist {
    pub location: f64,
    pub scale: f64,
}

impl CauchyDist {
    pub fn pdf(&self, x: f64) -> f64 {
        let z = (x - self.location) / self.scale;
        1.0 / (PI * self.scale * (1.0 + z * z))
    }
    pub fn cdf(&self, x: f64) -> f64 {
        0.5 + ((x - self.location) / self.scale).atan() / PI
    }
    pub fn inv_cdf(&self, p: f64) -> f64 {
        self.location + self.scale * (PI * (p - 0.5)).tan()
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 { self.inv_cdf(rng.next_f64()) }
}

/// Student's t-distribution.
#[derive(Clone, Debug)]
pub struct StudentTDist {
    pub degrees_of_freedom: f64,
}

impl StudentTDist {
    pub fn pdf(&self, t: f64) -> f64 {
        let nu = self.degrees_of_freedom;
        let coeff = gamma((nu + 1.0) / 2.0) / (gamma(nu / 2.0) * (nu * PI).sqrt());
        coeff * (1.0 + t * t / nu).powf(-(nu + 1.0) / 2.0)
    }
    pub fn cdf(&self, t: f64) -> f64 {
        let nu = self.degrees_of_freedom;
        let x = nu / (nu + t * t);
        let ib = betainc(x, nu / 2.0, 0.5) / 2.0;
        if t > 0.0 { 1.0 - ib } else { ib }
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 {
        let z = NormalDist { mean: 0.0, std_dev: 1.0 }.sample(rng);
        let chi2 = GammaDist { shape: self.degrees_of_freedom / 2.0, scale: 2.0 }.sample(rng);
        z / (chi2 / self.degrees_of_freedom).sqrt()
    }
}

/// Chi-squared distribution.
#[derive(Clone, Debug)]
pub struct ChiSquaredDist {
    pub k: f64,
}

impl ChiSquaredDist {
    pub fn pdf(&self, x: f64) -> f64 {
        GammaDist { shape: self.k / 2.0, scale: 2.0 }.pdf(x)
    }
    pub fn cdf(&self, x: f64) -> f64 {
        if x <= 0.0 { return 0.0; }
        gammainc_lower(self.k / 2.0, x / 2.0)
    }
    pub fn sample(&self, rng: &mut impl Rng) -> f64 {
        GammaDist { shape: self.k / 2.0, scale: 2.0 }.sample(rng)
    }
}

// ============================================================
// HYPOTHESIS TESTING
// ============================================================

/// One-sample t-test against mu0.
/// Returns (t-statistic, two-tailed p-value).
pub fn t_test_one_sample(data: &[f64], mu0: f64) -> (f64, f64) {
    let n = data.len() as f64;
    if n < 2.0 { return (0.0, 1.0); }
    let xbar = mean(data);
    let s = std_dev(data);
    if s == 0.0 { return (0.0, 1.0); }
    let t = (xbar - mu0) / (s / n.sqrt());
    let p = p_value_from_t(t, n - 1.0);
    (t, p)
}

/// Welch's two-sample t-test.
/// Returns (t-statistic, two-tailed p-value).
pub fn t_test_two_sample(a: &[f64], b: &[f64]) -> (f64, f64) {
    let na = a.len() as f64;
    let nb = b.len() as f64;
    if na < 2.0 || nb < 2.0 { return (0.0, 1.0); }
    let ma = mean(a);
    let mb = mean(b);
    let sa2 = variance(a);
    let sb2 = variance(b);
    let se = (sa2 / na + sb2 / nb).sqrt();
    if se == 0.0 { return (0.0, 1.0); }
    let t = (ma - mb) / se;
    // Welch-Satterthwaite degrees of freedom
    let df = (sa2 / na + sb2 / nb).powi(2)
        / ((sa2 / na).powi(2) / (na - 1.0) + (sb2 / nb).powi(2) / (nb - 1.0));
    let p = p_value_from_t(t, df);
    (t, p)
}

/// Chi-squared goodness-of-fit test.
/// Returns (chi2-statistic, p-value).
pub fn chi_squared_test(observed: &[f64], expected: &[f64]) -> (f64, f64) {
    let chi2: f64 = observed
        .iter()
        .zip(expected.iter())
        .map(|(o, e)| if *e > 0.0 { (o - e).powi(2) / e } else { 0.0 })
        .sum();
    let df = (observed.len() - 1).max(1);
    let p = p_value_from_chi2(chi2, df);
    (chi2, p)
}

/// Kolmogorov-Smirnov test against a theoretical CDF.
/// Returns (D-statistic, approximate p-value).
pub fn ks_test(data: &[f64], cdf: impl Fn(f64) -> f64) -> (f64, f64) {
    let n = data.len();
    if n == 0 { return (0.0, 1.0); }
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut d = 0.0f64;
    for (i, &x) in sorted.iter().enumerate() {
        let empirical_upper = (i + 1) as f64 / n as f64;
        let empirical_lower = i as f64 / n as f64;
        let theoretical = cdf(x);
        d = d.max((empirical_upper - theoretical).abs());
        d = d.max((empirical_lower - theoretical).abs());
    }
    // Approximate p-value using Kolmogorov distribution
    let sqrt_n = (n as f64).sqrt();
    let z = (sqrt_n + 0.12 + 0.11 / sqrt_n) * d;
    // Two-tailed KS p-value approximation
    let p = if z <= 0.0 { 1.0 } else {
        let mut sum = 0.0;
        for k in 1..50i64 {
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            sum += sign * (-2.0 * (k as f64).powi(2) * z * z).exp();
        }
        (2.0 * sum).clamp(0.0, 1.0)
    };
    (d, p)
}

/// Mann-Whitney U test (non-parametric, two-sample).
/// Returns (U-statistic, approximate two-tailed p-value).
pub fn mann_whitney_u(a: &[f64], b: &[f64]) -> (f64, f64) {
    let na = a.len();
    let nb = b.len();
    let mut u = 0.0f64;
    for &ai in a {
        for &bi in b {
            if ai > bi { u += 1.0; }
            else if ai == bi { u += 0.5; }
        }
    }
    let mean_u = na as f64 * nb as f64 / 2.0;
    let std_u = ((na as f64 * nb as f64 * (na + nb + 1) as f64) / 12.0).sqrt();
    if std_u == 0.0 { return (u, 1.0); }
    let z = (u - mean_u) / std_u;
    let norm = NormalDist { mean: 0.0, std_dev: 1.0 };
    let p = 2.0 * (1.0 - norm.cdf(z.abs()));
    (u, p)
}

/// Shapiro-Wilk test statistic W for normality.
/// Uses first 20 a-coefficients approximation.
pub fn shapiro_wilk_stat(data: &[f64]) -> f64 {
    let n = data.len();
    if n < 3 { return 1.0; }
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = mean(&sorted);
    let ss: f64 = sorted.iter().map(|x| (x - m).powi(2)).sum();
    if ss == 0.0 { return 1.0; }
    // Approximate a coefficients using expected normal order statistics
    let norm = NormalDist { mean: 0.0, std_dev: 1.0 };
    let half = n / 2;
    let mut b = 0.0f64;
    for i in 0..half {
        let expected_i = norm.inv_cdf((i as f64 + 0.625) / (n as f64 + 0.25));
        let expected_n_i = norm.inv_cdf((n as f64 - 1.0 - i as f64 + 0.625) / (n as f64 + 0.25));
        let a_i = expected_n_i - expected_i;
        b += a_i * (sorted[n - 1 - i] - sorted[i]);
    }
    b * b / ss
}

// ============================================================
// REGRESSION
// ============================================================

/// Simple linear regression: y = slope * x + intercept.
pub fn linear_regression(x: &[f64], y: &[f64]) -> (f64, f64) {
    let n = x.len().min(y.len()) as f64;
    if n < 2.0 { return (0.0, 0.0); }
    let mx = mean(x);
    let my = mean(y);
    let ss_xx: f64 = x.iter().map(|xi| (xi - mx).powi(2)).sum();
    let ss_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| (xi - mx) * (yi - my)).sum();
    if ss_xx == 0.0 { return (0.0, my); }
    let slope = ss_xy / ss_xx;
    let intercept = my - slope * mx;
    (slope, intercept)
}

/// Polynomial regression of given degree. Returns coefficients [a0, a1, ..., a_deg].
pub fn polynomial_regression(x: &[f64], y: &[f64], degree: usize) -> Vec<f64> {
    let n = x.len().min(y.len());
    let d = degree + 1;
    // Build Vandermonde matrix X
    let mut xmat = vec![vec![0.0f64; d]; n];
    for i in 0..n {
        for j in 0..d {
            xmat[i][j] = x[i].powi(j as i32);
        }
    }
    // X^T X
    let mut xtx = vec![vec![0.0f64; d]; d];
    for r in 0..d {
        for c in 0..d {
            for i in 0..n { xtx[r][c] += xmat[i][r] * xmat[i][c]; }
        }
    }
    // X^T y
    let mut xty = vec![0.0f64; d];
    for r in 0..d {
        for i in 0..n { xty[r] += xmat[i][r] * y[i]; }
    }
    // Solve xtx * coeffs = xty via Gaussian elimination
    solve_system(&mut xtx, &mut xty).unwrap_or_else(|| vec![0.0; d])
}

fn solve_system(a: &mut Vec<Vec<f64>>, b: &mut Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for k in 0..n {
        let mut max_val = a[k][k].abs();
        let mut max_row = k;
        for i in k + 1..n {
            if a[i][k].abs() > max_val { max_val = a[i][k].abs(); max_row = i; }
        }
        if max_val < 1e-12 { return None; }
        a.swap(k, max_row);
        b.swap(k, max_row);
        let pivot = a[k][k];
        for j in k..n { a[k][j] /= pivot; }
        b[k] /= pivot;
        for i in 0..n {
            if i != k {
                let factor = a[i][k];
                for j in k..n { a[i][j] -= factor * a[k][j]; }
                b[i] -= factor * b[k];
            }
        }
    }
    Some(b.clone())
}

/// Multiple linear regression (OLS). X is n_samples × n_features.
/// Returns coefficient vector (including intercept as first element).
pub fn multiple_linear_regression(x: &[Vec<f64>], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    if n == 0 { return vec![]; }
    let p = x[0].len() + 1; // +1 for intercept
    // Build design matrix with intercept column
    let mut xmat = vec![vec![0.0f64; p]; n];
    for i in 0..n {
        xmat[i][0] = 1.0;
        for j in 1..p { xmat[i][j] = x[i][j - 1]; }
    }
    // X^T X
    let mut xtx = vec![vec![0.0f64; p]; p];
    for r in 0..p {
        for c in 0..p {
            for i in 0..n { xtx[r][c] += xmat[i][r] * xmat[i][c]; }
        }
    }
    // X^T y
    let mut xty = vec![0.0f64; p];
    for r in 0..p {
        for i in 0..n { xty[r] += xmat[i][r] * y[i]; }
    }
    solve_system(&mut xtx, &mut xty).unwrap_or_else(|| vec![0.0; p])
}

/// R-squared coefficient of determination.
pub fn r_squared(y_true: &[f64], y_pred: &[f64]) -> f64 {
    let n = y_true.len().min(y_pred.len());
    if n == 0 { return 0.0; }
    let mean_true = mean(y_true);
    let ss_res: f64 = y_true.iter().zip(y_pred.iter()).map(|(y, yh)| (y - yh).powi(2)).sum();
    let ss_tot: f64 = y_true.iter().map(|y| (y - mean_true).powi(2)).sum();
    if ss_tot == 0.0 { return 1.0; }
    1.0 - ss_res / ss_tot
}

/// Ridge regression (L2 regularized OLS). Returns coefficients.
pub fn ridge_regression(x: &[Vec<f64>], y: &[f64], lambda: f64) -> Vec<f64> {
    let n = x.len().min(y.len());
    if n == 0 { return vec![]; }
    let p = x[0].len() + 1;
    let mut xmat = vec![vec![0.0f64; p]; n];
    for i in 0..n {
        xmat[i][0] = 1.0;
        for j in 1..p { xmat[i][j] = x[i][j - 1]; }
    }
    let mut xtx = vec![vec![0.0f64; p]; p];
    for r in 0..p {
        for c in 0..p {
            for i in 0..n { xtx[r][c] += xmat[i][r] * xmat[i][c]; }
        }
    }
    // Add lambda * I (skip intercept at index 0)
    for j in 1..p { xtx[j][j] += lambda; }
    let mut xty = vec![0.0f64; p];
    for r in 0..p {
        for i in 0..n { xty[r] += xmat[i][r] * y[i]; }
    }
    solve_system(&mut xtx, &mut xty).unwrap_or_else(|| vec![0.0; p])
}

/// Logistic regression via gradient descent.
/// `x` is n_samples × n_features, `y` is bool labels.
/// Returns weight vector (n_features + 1, including intercept).
pub fn logistic_regression(x: &[Vec<f64>], y: &[bool], lr: f64, epochs: usize) -> Vec<f64> {
    let n = x.len().min(y.len());
    if n == 0 { return vec![]; }
    let p = x[0].len() + 1;
    let mut w = vec![0.0f64; p];
    let sigmoid = |z: f64| 1.0 / (1.0 + (-z).exp());
    for _ in 0..epochs {
        let mut grad = vec![0.0f64; p];
        for i in 0..n {
            let mut z = w[0];
            for j in 1..p { z += w[j] * x[i][j - 1]; }
            let pred = sigmoid(z);
            let target = if y[i] { 1.0 } else { 0.0 };
            let err = pred - target;
            grad[0] += err;
            for j in 1..p { grad[j] += err * x[i][j - 1]; }
        }
        for j in 0..p { w[j] -= lr * grad[j] / n as f64; }
    }
    w
}

// ============================================================
// BAYESIAN INFERENCE
// ============================================================

/// Beta-Bernoulli conjugate model.
#[derive(Clone, Debug)]
pub struct BetaBernoulli {
    pub alpha: f64,
    pub beta: f64,
}

/// Update Beta prior with new Bernoulli observations.
pub fn update_beta_bernoulli(prior: BetaBernoulli, successes: u32, failures: u32) -> BetaBernoulli {
    BetaBernoulli {
        alpha: prior.alpha + successes as f64,
        beta: prior.beta + failures as f64,
    }
}

/// Posterior mean of Beta-Bernoulli model.
pub fn posterior_mean(dist: &BetaBernoulli) -> f64 {
    dist.alpha / (dist.alpha + dist.beta)
}

/// Equal-tailed credible interval for Beta distribution.
pub fn credible_interval(dist: &BetaBernoulli, level: f64) -> (f64, f64) {
    let tail = (1.0 - level) / 2.0;
    let beta = BetaDist { alpha: dist.alpha, beta: dist.beta };
    // Numerical inversion of beta CDF
    let inv_beta_cdf = |p: f64| -> f64 {
        let mut lo = 0.0f64;
        let mut hi = 1.0f64;
        for _ in 0..100 {
            let mid = (lo + hi) * 0.5;
            if beta.cdf(mid) < p { lo = mid; } else { hi = mid; }
        }
        (lo + hi) * 0.5
    };
    (inv_beta_cdf(tail), inv_beta_cdf(1.0 - tail))
}

/// Gaussian-Gaussian conjugate model (known variance).
#[derive(Clone, Debug)]
pub struct GaussianGaussian {
    pub prior_mean: f64,
    pub prior_variance: f64,
    pub likelihood_variance: f64,
}

impl GaussianGaussian {
    /// Update posterior given n observations with sample mean.
    pub fn update(&self, sample_mean: f64, n: usize) -> (f64, f64) {
        let n = n as f64;
        let lv = self.likelihood_variance;
        let pv = self.prior_variance;
        let post_var = 1.0 / (1.0 / pv + n / lv);
        let post_mean = post_var * (self.prior_mean / pv + n * sample_mean / lv);
        (post_mean, post_var)
    }
}

/// Bayesian Information Criterion.
pub fn bayesian_information_criterion(log_likelihood: f64, n_params: usize, n_samples: usize) -> f64 {
    -2.0 * log_likelihood + n_params as f64 * (n_samples as f64).ln()
}

/// Akaike Information Criterion.
pub fn akaike_information_criterion(log_likelihood: f64, n_params: usize) -> f64 {
    -2.0 * log_likelihood + 2.0 * n_params as f64
}

// ============================================================
// INFORMATION THEORY
// ============================================================

/// Shannon entropy in nats (natural log base).
pub fn entropy(probs: &[f64]) -> f64 {
    probs.iter()
        .filter(|&&p| p > 0.0)
        .map(|&p| -p * p.ln())
        .sum()
}

/// Cross-entropy H(P, Q) = -sum_x P(x) log Q(x).
pub fn cross_entropy(p: &[f64], q: &[f64]) -> f64 {
    p.iter()
        .zip(q.iter())
        .filter(|(&pi, &qi)| pi > 0.0 && qi > 0.0)
        .map(|(&pi, &qi)| -pi * qi.ln())
        .sum()
}

/// KL divergence D_KL(P || Q) = sum_x P(x) log(P(x)/Q(x)).
pub fn kl_divergence(p: &[f64], q: &[f64]) -> f64 {
    p.iter()
        .zip(q.iter())
        .filter(|(&pi, &qi)| pi > 0.0 && qi > 0.0)
        .map(|(&pi, &qi)| pi * (pi / qi).ln())
        .sum()
}

/// Mutual information I(X;Y) from joint probability matrix.
pub fn mutual_information(joint: &[Vec<f64>]) -> f64 {
    let rows = joint.len();
    if rows == 0 { return 0.0; }
    let cols = joint[0].len();
    let px: Vec<f64> = (0..rows).map(|i| joint[i].iter().sum()).collect();
    let py: Vec<f64> = (0..cols).map(|j| joint.iter().map(|row| row[j]).sum()).collect();
    let mut mi = 0.0;
    for i in 0..rows {
        for j in 0..cols {
            let pij = joint[i][j];
            if pij > 0.0 && px[i] > 0.0 && py[j] > 0.0 {
                mi += pij * (pij / (px[i] * py[j])).ln();
            }
        }
    }
    mi
}

/// Jensen-Shannon divergence — symmetric, bounded [0, ln(2)].
pub fn jensen_shannon_divergence(p: &[f64], q: &[f64]) -> f64 {
    let m: Vec<f64> = p.iter().zip(q.iter()).map(|(pi, qi)| (pi + qi) * 0.5).collect();
    0.5 * kl_divergence(p, &m) + 0.5 * kl_divergence(q, &m)
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((mean(&data) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_variance() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((variance(&data) - 4.571428571428571).abs() < 1e-8);
    }

    #[test]
    fn test_std_dev() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((std_dev(&data) - 2.138).abs() < 0.001);
    }

    #[test]
    fn test_median_odd() {
        let mut data = vec![3.0, 1.0, 4.0, 1.0, 5.0];
        assert_eq!(median(&mut data), 3.0);
    }

    #[test]
    fn test_median_even() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0];
        assert!((median(&mut data) - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_percentile() {
        let mut data: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        assert!((percentile(&mut data, 50.0) - 50.5).abs() < 0.5);
    }

    #[test]
    fn test_pearson_r_perfect() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y: Vec<f64> = x.iter().map(|xi| 2.0 * xi + 1.0).collect();
        assert!((pearson_r(&x, &y) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_spearman_rho() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        assert!((spearman_rho(&x, &y) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_normal_dist_cdf() {
        let n = NormalDist { mean: 0.0, std_dev: 1.0 };
        assert!((n.cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((n.cdf(1.96) - 0.975).abs() < 0.001);
    }

    #[test]
    fn test_normal_dist_sample() {
        let n = NormalDist { mean: 5.0, std_dev: 2.0 };
        let mut rng = Xorshift64::new(42);
        let samples: Vec<f64> = (0..10000).map(|_| n.sample(&mut rng)).collect();
        let m = mean(&samples);
        assert!((m - 5.0).abs() < 0.1, "mean {} far from 5.0", m);
    }

    #[test]
    fn test_exponential_sample() {
        let e = ExponentialDist { lambda: 2.0 };
        let mut rng = Xorshift64::new(42);
        let samples: Vec<f64> = (0..10000).map(|_| e.sample(&mut rng)).collect();
        let m = mean(&samples);
        assert!((m - 0.5).abs() < 0.05, "mean {} far from 0.5", m);
    }

    #[test]
    fn test_poisson_sample() {
        let p = PoissonDist { lambda: 3.0 };
        let mut rng = Xorshift64::new(42);
        let samples: Vec<f64> = (0..10000).map(|_| p.sample(&mut rng) as f64).collect();
        let m = mean(&samples);
        assert!((m - 3.0).abs() < 0.1, "mean {} far from 3.0", m);
    }

    #[test]
    fn test_gamma_sample() {
        let g = GammaDist { shape: 2.0, scale: 3.0 };
        let mut rng = Xorshift64::new(99);
        let samples: Vec<f64> = (0..10000).map(|_| g.sample(&mut rng)).collect();
        let m = mean(&samples);
        // Expected mean = shape * scale = 6
        assert!((m - 6.0).abs() < 0.2, "mean {} far from 6.0", m);
    }

    #[test]
    fn test_t_test_one_sample() {
        let data = vec![10.0, 11.0, 9.5, 10.5, 10.2, 9.8, 10.1, 9.9, 10.3, 10.4];
        let (t, p) = t_test_one_sample(&data, 10.0);
        assert!(p > 0.05, "should not reject null at 10.0; p={}", p);
        let _ = t;
    }

    #[test]
    fn test_t_test_two_sample() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![6.0, 7.0, 8.0, 9.0, 10.0];
        let (_t, p) = t_test_two_sample(&a, &b);
        assert!(p < 0.05, "should reject null; p={}", p);
    }

    #[test]
    fn test_chi_squared_test() {
        let obs = vec![10.0, 20.0, 30.0];
        let exp = vec![10.0, 20.0, 30.0];
        let (chi2, p) = chi_squared_test(&obs, &exp);
        assert!(chi2.abs() < 1e-10);
        assert!(p > 0.9);
    }

    #[test]
    fn test_linear_regression() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y: Vec<f64> = x.iter().map(|xi| 3.0 * xi + 1.0).collect();
        let (slope, intercept) = linear_regression(&x, &y);
        assert!((slope - 3.0).abs() < 1e-10);
        assert!((intercept - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_polynomial_regression() {
        let x = vec![0.0, 1.0, 2.0, 3.0];
        let y: Vec<f64> = x.iter().map(|xi| xi * xi + 2.0 * xi + 1.0).collect();
        let coeffs = polynomial_regression(&x, &y, 2);
        assert_eq!(coeffs.len(), 3);
        assert!((coeffs[0] - 1.0).abs() < 1e-6);
        assert!((coeffs[1] - 2.0).abs() < 1e-6);
        assert!((coeffs[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_r_squared() {
        let y_true = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y_pred = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((r_squared(&y_true, &y_pred) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_logistic_regression_separable() {
        let x = vec![vec![-2.0], vec![-1.0], vec![1.0], vec![2.0]];
        let y = vec![false, false, true, true];
        let w = logistic_regression(&x, &y, 0.5, 500);
        let sigmoid = |z: f64| 1.0 / (1.0 + (-z).exp());
        let pred_neg = sigmoid(w[0] + w[1] * (-2.0));
        let pred_pos = sigmoid(w[0] + w[1] * 2.0);
        assert!(pred_neg < 0.5, "negative class should have prob < 0.5");
        assert!(pred_pos > 0.5, "positive class should have prob > 0.5");
    }

    #[test]
    fn test_bayesian_update() {
        let prior = BetaBernoulli { alpha: 1.0, beta: 1.0 };
        let posterior = update_beta_bernoulli(prior, 6, 4);
        assert!((posterior.alpha - 7.0).abs() < 1e-10);
        assert!((posterior.beta - 5.0).abs() < 1e-10);
        assert!((posterior_mean(&posterior) - 7.0 / 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_entropy() {
        let uniform = vec![0.25, 0.25, 0.25, 0.25];
        assert!((entropy(&uniform) - (4.0f64).ln()).abs() < 1e-10);
    }

    #[test]
    fn test_kl_divergence() {
        let p = vec![0.5, 0.5];
        let q = vec![0.5, 0.5];
        assert!(kl_divergence(&p, &q).abs() < 1e-10);
    }

    #[test]
    fn test_jsd() {
        let p = vec![1.0, 0.0];
        let q = vec![0.0, 1.0];
        let jsd = jensen_shannon_divergence(&p, &q);
        assert!((jsd - 2.0f64.ln()).abs() < 1e-10);
    }

    #[test]
    fn test_pcg32() {
        let mut rng = Pcg32::new(42, 1);
        let v: Vec<f64> = (0..1000).map(|_| rng.next_f64()).collect();
        let m = mean(&v);
        assert!((m - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_splitmix64() {
        let mut rng = SplitMix64::new(12345);
        let v: Vec<f64> = (0..1000).map(|_| rng.next_f64()).collect();
        let m = mean(&v);
        assert!((m - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_shuffle() {
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let original = data.clone();
        let mut rng = Xorshift64::new(7);
        shuffle(&mut data, &mut rng);
        // Not necessarily different but should contain same elements
        let mut sorted = data.clone();
        sorted.sort();
        assert_eq!(sorted, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let _ = original;
    }

    #[test]
    fn test_weighted_sample() {
        let weights = vec![0.0, 1.0, 0.0]; // must pick index 1
        let mut rng = Xorshift64::new(42);
        let idx = weighted_sample(&weights, &mut rng);
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_sample_without_replacement() {
        let mut rng = Xorshift64::new(42);
        let sample = sample_without_replacement(100, 10, &mut rng);
        assert_eq!(sample.len(), 10);
        // All unique
        let mut s = sample.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), 10);
    }
}
