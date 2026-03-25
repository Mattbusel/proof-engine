//! Prime number distribution, sieves, and rendering utilities.

use glam::{Vec2, Vec3, Vec4};

/// Classic sieve of Eratosthenes returning all primes up to `limit`.
pub fn sieve_of_eratosthenes(limit: u64) -> Vec<u64> {
    if limit < 2 {
        return Vec::new();
    }
    let n = limit as usize;
    let mut is_prime = vec![true; n + 1];
    is_prime[0] = false;
    is_prime[1] = false;
    let mut i = 2;
    while i * i <= n {
        if is_prime[i] {
            let mut j = i * i;
            while j <= n {
                is_prime[j] = false;
                j += i;
            }
        }
        i += 1;
    }
    is_prime
        .iter()
        .enumerate()
        .filter_map(|(idx, &p)| if p { Some(idx as u64) } else { None })
        .collect()
}

/// Deterministic Miller-Rabin witnesses sufficient for all u64.
const WITNESSES: [u64; 7] = [2, 3, 5, 7, 11, 13, 17];

fn mod_pow_u128(mut base: u128, mut exp: u128, modulus: u128) -> u128 {
    let mut result: u128 = 1;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result * base % modulus;
        }
        exp >>= 1;
        base = base * base % modulus;
    }
    result
}

fn miller_rabin_test(n: u64, a: u64) -> bool {
    if a % n == 0 {
        return true;
    }
    let n128 = n as u128;
    let mut d = n - 1;
    let mut r = 0u32;
    while d % 2 == 0 {
        d /= 2;
        r += 1;
    }
    let mut x = mod_pow_u128(a as u128, d as u128, n128);
    if x == 1 || x == n128 - 1 {
        return true;
    }
    for _ in 0..r - 1 {
        x = x * x % n128;
        if x == n128 - 1 {
            return true;
        }
    }
    false
}

/// Primality test. Uses trial division for small n, Miller-Rabin for large n.
pub fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    if n < 25 {
        return true;
    }
    // Small trial division
    let mut i = 5u64;
    while i * i <= n && i < 1000 {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    if i * i > n {
        return true;
    }
    // Miller-Rabin
    for &a in &WITNESSES {
        if a >= n {
            continue;
        }
        if !miller_rabin_test(n, a) {
            return false;
        }
    }
    true
}

/// Returns the n-th prime (1-indexed: nth_prime(1) == 2).
pub fn nth_prime(n: usize) -> u64 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 2;
    }
    let mut count = 1usize;
    let mut candidate = 3u64;
    loop {
        if is_prime(candidate) {
            count += 1;
            if count == n {
                return candidate;
            }
        }
        candidate += 2;
    }
}

/// Prime counting function pi(x): number of primes <= x.
pub fn prime_counting(x: u64) -> usize {
    if x < 2 {
        return 0;
    }
    sieve_of_eratosthenes(x).len()
}

/// Gaps between consecutive primes up to `limit`.
pub fn prime_gaps(limit: u64) -> Vec<u64> {
    let primes = sieve_of_eratosthenes(limit);
    primes.windows(2).map(|w| w[1] - w[0]).collect()
}

/// Twin prime pairs (p, p+2) up to `limit`.
pub fn twin_primes(limit: u64) -> Vec<(u64, u64)> {
    let primes = sieve_of_eratosthenes(limit);
    primes
        .windows(2)
        .filter_map(|w| {
            if w[1] - w[0] == 2 {
                Some((w[0], w[1]))
            } else {
                None
            }
        })
        .collect()
}

/// Prime factorization of n, returned as sorted (prime, exponent) pairs.
pub fn prime_factorization(n: u64) -> Vec<(u64, u32)> {
    if n <= 1 {
        return Vec::new();
    }
    let mut factors = Vec::new();
    let mut remaining = n;

    let mut count = 0u32;
    while remaining % 2 == 0 {
        remaining /= 2;
        count += 1;
    }
    if count > 0 {
        factors.push((2u64, count));
    }

    let mut d = 3u64;
    while d * d <= remaining {
        let mut count = 0u32;
        while remaining % d == 0 {
            remaining /= d;
            count += 1;
        }
        if count > 0 {
            factors.push((d, count));
        }
        d += 2;
    }
    if remaining > 1 {
        factors.push((remaining, 1));
    }
    factors
}

/// Ulam spiral: maps positive integers to a 2D grid position via a spiral walk,
/// highlighting prime positions.
pub struct UlamSpiral {
    pub size: usize,
}

impl UlamSpiral {
    pub fn new(size: usize) -> Self {
        Self { size }
    }

    /// Map integer n (starting at 1 in center) to grid (x, y).
    pub fn position(n: u64) -> Vec2 {
        if n == 1 {
            return Vec2::ZERO;
        }
        // Layer k: numbers from (2k-1)^2+1 to (2k+1)^2
        let k = ((((n as f64).sqrt() - 1.0) / 2.0).ceil()) as i64;
        let side_len = 2 * k;
        let start = (2 * k - 1) * (2 * k - 1) + 1;
        let offset = (n as i64) - start;

        let (x, y) = if offset < side_len {
            // right side going up
            (k, -k + 1 + offset)
        } else if offset < 2 * side_len {
            // top going left
            (k - 1 - (offset - side_len), k)
        } else if offset < 3 * side_len {
            // left going down
            (-k, k - 1 - (offset - 2 * side_len))
        } else {
            // bottom going right
            (-k + 1 + (offset - 3 * side_len), -k)
        };
        Vec2::new(x as f32, y as f32)
    }

    /// Generate all spiral positions up to size*size, returning (position, is_prime).
    pub fn generate(&self) -> Vec<(Vec2, bool)> {
        let total = (self.size * self.size) as u64;
        let primes_set: std::collections::HashSet<u64> =
            sieve_of_eratosthenes(total).into_iter().collect();
        (1..=total)
            .map(|n| (Self::position(n), primes_set.contains(&n)))
            .collect()
    }
}

/// Sacks spiral: polar plot where integer n is at angle sqrt(n) * 2*pi, radius sqrt(n).
pub struct SacksSpiral;

impl SacksSpiral {
    /// Convert integer n to polar coordinates then to Vec2.
    pub fn position(n: u64) -> Vec2 {
        let r = (n as f64).sqrt();
        let theta = r * std::f64::consts::TAU;
        Vec2::new((r * theta.cos()) as f32, (r * theta.sin()) as f32)
    }

    /// Generate points for integers 1..=limit, returning (position, is_prime).
    pub fn generate(limit: u64) -> Vec<(Vec2, bool)> {
        let primes_set: std::collections::HashSet<u64> =
            sieve_of_eratosthenes(limit).into_iter().collect();
        (1..=limit)
            .map(|n| (SacksSpiral::position(n), primes_set.contains(&n)))
            .collect()
    }
}

/// Maps primes to glyph positions, sizes, and colors for engine rendering.
pub struct PrimeDistributionRenderer {
    pub origin: Vec3,
    pub scale: f32,
}

/// A renderable glyph descriptor for a prime.
pub struct PrimeGlyph {
    pub value: u64,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl PrimeDistributionRenderer {
    pub fn new(origin: Vec3, scale: f32) -> Self {
        Self { origin, scale }
    }

    /// Render primes on an Ulam spiral as glyphs.
    pub fn ulam_glyphs(&self, size: usize) -> Vec<PrimeGlyph> {
        let spiral = UlamSpiral::new(size);
        let data = spiral.generate();
        data.into_iter()
            .enumerate()
            .filter_map(|(i, (pos, is_p))| {
                if !is_p {
                    return None;
                }
                let n = (i + 1) as u64;
                let brightness = 1.0 - (pos.length() / (size as f32)).min(1.0);
                Some(PrimeGlyph {
                    value: n,
                    position: self.origin
                        + Vec3::new(pos.x * self.scale, pos.y * self.scale, 0.0),
                    color: Vec4::new(brightness, 0.7, 1.0 - brightness, 1.0),
                    character: prime_char(n),
                })
            })
            .collect()
    }

    /// Render primes on a Sacks spiral as glyphs.
    pub fn sacks_glyphs(&self, limit: u64) -> Vec<PrimeGlyph> {
        let data = SacksSpiral::generate(limit);
        data.into_iter()
            .enumerate()
            .filter_map(|(i, (pos, is_p))| {
                if !is_p {
                    return None;
                }
                let n = (i + 1) as u64;
                let r = (n as f32).sqrt();
                let hue = (r / (limit as f32).sqrt()).min(1.0);
                Some(PrimeGlyph {
                    value: n,
                    position: self.origin
                        + Vec3::new(pos.x * self.scale, pos.y * self.scale, 0.0),
                    color: Vec4::new(hue, 1.0 - hue, 0.5, 1.0),
                    character: prime_char(n),
                })
            })
            .collect()
    }

    /// Linear layout: primes along a horizontal line.
    pub fn linear_glyphs(&self, limit: u64) -> Vec<PrimeGlyph> {
        let primes = sieve_of_eratosthenes(limit);
        primes
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let t = i as f32 / primes.len().max(1) as f32;
                PrimeGlyph {
                    value: p,
                    position: self.origin + Vec3::new(i as f32 * self.scale, 0.0, 0.0),
                    color: Vec4::new(t, 0.3, 1.0 - t, 1.0),
                    character: prime_char(p),
                }
            })
            .collect()
    }
}

fn prime_char(p: u64) -> char {
    match p % 6 {
        1 => '*',
        5 => '+',
        _ => '.',
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sieve_small() {
        let primes = sieve_of_eratosthenes(30);
        assert_eq!(primes, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29]);
    }

    #[test]
    fn sieve_edge() {
        assert!(sieve_of_eratosthenes(0).is_empty());
        assert!(sieve_of_eratosthenes(1).is_empty());
        assert_eq!(sieve_of_eratosthenes(2), vec![2]);
    }

    #[test]
    fn primality() {
        assert!(!is_prime(0));
        assert!(!is_prime(1));
        assert!(is_prime(2));
        assert!(is_prime(3));
        assert!(!is_prime(4));
        assert!(is_prime(7919));
        assert!(!is_prime(7917));
        // Large prime
        assert!(is_prime(104729));
        assert!(!is_prime(104730));
    }

    #[test]
    fn nth_prime_test() {
        assert_eq!(nth_prime(1), 2);
        assert_eq!(nth_prime(2), 3);
        assert_eq!(nth_prime(5), 11);
        assert_eq!(nth_prime(10), 29);
        assert_eq!(nth_prime(100), 541);
    }

    #[test]
    fn counting() {
        assert_eq!(prime_counting(10), 4);
        assert_eq!(prime_counting(100), 25);
        assert_eq!(prime_counting(1000), 168);
    }

    #[test]
    fn gaps() {
        let g = prime_gaps(20);
        // primes: 2,3,5,7,11,13,17,19  gaps: 1,2,2,4,2,4,2
        assert_eq!(g, vec![1, 2, 2, 4, 2, 4, 2]);
    }

    #[test]
    fn twins() {
        let t = twin_primes(50);
        assert!(t.contains(&(3, 5)));
        assert!(t.contains(&(5, 7)));
        assert!(t.contains(&(11, 13)));
        assert!(t.contains(&(29, 31)));
        assert!(t.contains(&(41, 43)));
    }

    #[test]
    fn factorization() {
        assert_eq!(prime_factorization(1), vec![]);
        assert_eq!(prime_factorization(2), vec![(2, 1)]);
        assert_eq!(prime_factorization(12), vec![(2, 2), (3, 1)]);
        assert_eq!(prime_factorization(360), vec![(2, 3), (3, 2), (5, 1)]);
        assert_eq!(
            prime_factorization(2 * 3 * 5 * 7 * 11),
            vec![(2, 1), (3, 1), (5, 1), (7, 1), (11, 1)]
        );
    }

    #[test]
    fn ulam_center() {
        let p = UlamSpiral::position(1);
        assert_eq!(p, Vec2::ZERO);
    }

    #[test]
    fn ulam_generate() {
        let spiral = UlamSpiral::new(5);
        let data = spiral.generate();
        assert_eq!(data.len(), 25);
        // 2 is prime
        assert!(data[1].1);
        // 4 is not
        assert!(!data[3].1);
    }

    #[test]
    fn sacks_positions() {
        let p1 = SacksSpiral::position(1);
        // radius = 1, angle = 2*pi => (1, 0) approximately
        assert!((p1.x - 1.0).abs() < 0.01);
        assert!(p1.y.abs() < 0.01);
    }

    #[test]
    fn renderer_produces_glyphs() {
        let r = PrimeDistributionRenderer::new(Vec3::ZERO, 1.0);
        let glyphs = r.ulam_glyphs(5);
        assert!(!glyphs.is_empty());
        // All glyphs should be primes
        for g in &glyphs {
            assert!(is_prime(g.value));
        }
    }

    #[test]
    fn renderer_sacks() {
        let r = PrimeDistributionRenderer::new(Vec3::ZERO, 0.5);
        let glyphs = r.sacks_glyphs(100);
        assert!(!glyphs.is_empty());
        for g in &glyphs {
            assert!(is_prime(g.value));
        }
    }

    #[test]
    fn renderer_linear() {
        let r = PrimeDistributionRenderer::new(Vec3::ZERO, 1.0);
        let glyphs = r.linear_glyphs(50);
        assert_eq!(glyphs.len(), prime_counting(50));
    }

    #[test]
    fn miller_rabin_large() {
        // Known large primes
        assert!(is_prime(999999999989));
        assert!(!is_prime(999999999990));
    }
}
