//! Euler's totient function, divisor functions, and heatmap rendering.

use glam::{Vec2, Vec3, Vec4};

/// Euler's totient: phi(n) = number of integers in [1, n] coprime to n.
pub fn totient(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    let factors = super::primes::prime_factorization(n);
    let mut result = n;
    for &(p, _) in &factors {
        result = result / p * (p - 1);
    }
    result
}

/// Compute phi(k) for all k in [0, limit] using a sieve.
pub fn totient_sieve(limit: u64) -> Vec<u64> {
    let n = limit as usize;
    let mut phi: Vec<u64> = (0..=n as u64).collect();
    for i in 2..=n {
        if phi[i] == i as u64 {
            // i is prime
            for j in (i..=n).step_by(i) {
                phi[j] = phi[j] / i as u64 * (i as u64 - 1);
            }
        }
    }
    phi
}

/// Sum of phi(k) for k = 1..=n.
pub fn totient_sum(n: u64) -> u64 {
    let phi = totient_sieve(n);
    phi.iter().skip(1).sum()
}

/// Divisor sigma function: sigma_k(n) = sum of d^k for all divisors d of n.
pub fn sigma(n: u64, k: u32) -> u64 {
    if n == 0 {
        return 0;
    }
    let factors = super::primes::prime_factorization(n);
    let mut result = 1u64;
    for &(p, e) in &factors {
        // sigma_k(p^e) = (p^{k(e+1)} - 1) / (p^k - 1) when k > 0
        // sigma_0(p^e) = e + 1
        if k == 0 {
            result *= (e + 1) as u64;
        } else {
            let pk = p.pow(k);
            let mut sum = 0u64;
            let mut power = 1u64;
            for _ in 0..=e {
                sum += power;
                power = power.saturating_mul(pk);
            }
            result *= sum;
        }
    }
    result
}

/// Number of divisors: tau(n) = sigma_0(n).
pub fn tau(n: u64) -> u64 {
    sigma(n, 0)
}

/// Mobius function: mu(n).
/// mu(1) = 1
/// mu(n) = 0 if n has a squared prime factor
/// mu(n) = (-1)^k if n is a product of k distinct primes
pub fn mobius(n: u64) -> i8 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    let factors = super::primes::prime_factorization(n);
    for &(_, e) in &factors {
        if e > 1 {
            return 0;
        }
    }
    if factors.len() % 2 == 0 {
        1
    } else {
        -1
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

/// Render phi(n)/n as a height field / heatmap.
pub struct TotientLandscape {
    pub origin: Vec3,
    pub scale: f32,
    pub width: usize,
}

pub struct TotientGlyph {
    pub n: u64,
    pub phi: u64,
    pub ratio: f32,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl TotientLandscape {
    pub fn new(origin: Vec3, scale: f32, width: usize) -> Self {
        Self { origin, scale, width }
    }

    /// Render totient landscape for integers 1..=limit laid out in a grid.
    pub fn render(&self, limit: u64) -> Vec<TotientGlyph> {
        let phi_values = totient_sieve(limit);
        let mut glyphs = Vec::new();

        for n in 1..=limit {
            let phi_n = phi_values[n as usize];
            let ratio = phi_n as f32 / n as f32;
            let idx = (n - 1) as usize;
            let col = idx % self.width;
            let row = idx / self.width;
            let x = col as f32 * self.scale;
            let y = row as f32 * self.scale;
            let z = ratio * self.scale;

            // Color by ratio: low ratio = red (many factors), high = green (prime-like)
            let color = Vec4::new(1.0 - ratio, ratio, 0.3, 1.0);
            let ch = if phi_n == n - 1 {
                'P' // prime
            } else if ratio < 0.4 {
                '#' // highly composite
            } else {
                '.'
            };

            glyphs.push(TotientGlyph {
                n,
                phi: phi_n,
                ratio,
                position: self.origin + Vec3::new(x, y, z),
                color,
                character: ch,
            });
        }
        glyphs
    }

    /// Render just the totient values as a 1D height graph.
    pub fn render_1d(&self, limit: u64) -> Vec<TotientGlyph> {
        let phi_values = totient_sieve(limit);
        (1..=limit)
            .map(|n| {
                let phi_n = phi_values[n as usize];
                let ratio = phi_n as f32 / n as f32;
                TotientGlyph {
                    n,
                    phi: phi_n,
                    ratio,
                    position: self.origin + Vec3::new(n as f32 * self.scale, ratio * self.scale * 5.0, 0.0),
                    color: Vec4::new(1.0 - ratio, ratio, 0.3, 1.0),
                    character: '|',
                }
            })
            .collect()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totient_basic() {
        assert_eq!(totient(1), 1);
        assert_eq!(totient(2), 1);
        assert_eq!(totient(6), 2);
        assert_eq!(totient(12), 4);
        assert_eq!(totient(7), 6); // prime
        assert_eq!(totient(10), 4);
        assert_eq!(totient(36), 12);
    }

    #[test]
    fn totient_prime() {
        // For prime p, phi(p) = p-1
        for &p in &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31] {
            assert_eq!(totient(p), p - 1, "phi({}) should be {}", p, p - 1);
        }
    }

    #[test]
    fn totient_power_of_prime() {
        // phi(p^k) = p^k - p^{k-1} = p^{k-1}(p-1)
        assert_eq!(totient(8), 4);  // 2^3: 2^2 * 1 = 4
        assert_eq!(totient(9), 6);  // 3^2: 3^1 * 2 = 6
        assert_eq!(totient(27), 18); // 3^3: 3^2 * 2 = 18
    }

    #[test]
    fn sieve_matches_individual() {
        let sieved = totient_sieve(100);
        for n in 1..=100 {
            assert_eq!(
                sieved[n as usize],
                totient(n),
                "sieve mismatch at n={}",
                n
            );
        }
    }

    #[test]
    fn totient_sum_formula() {
        // sum_{k=1}^{n} phi(k) should be approximately 3n^2/pi^2 for large n
        // For exact: sum phi(k) for k=1..10 = 1+1+2+2+4+2+6+4+6+4 = 32
        let s = totient_sum(10);
        assert_eq!(s, 32);
    }

    #[test]
    fn sigma_basic() {
        // sigma_1(6) = 1+2+3+6 = 12
        assert_eq!(sigma(6, 1), 12);
        // sigma_1(12) = 1+2+3+4+6+12 = 28
        assert_eq!(sigma(12, 1), 28);
        // sigma_0(12) = 6 divisors
        assert_eq!(sigma(12, 0), 6);
    }

    #[test]
    fn tau_basic() {
        assert_eq!(tau(1), 1);
        assert_eq!(tau(6), 4);  // 1,2,3,6
        assert_eq!(tau(12), 6); // 1,2,3,4,6,12
        assert_eq!(tau(7), 2);  // prime
    }

    #[test]
    fn sigma_2() {
        // sigma_2(6) = 1^2 + 2^2 + 3^2 + 6^2 = 1+4+9+36 = 50
        assert_eq!(sigma(6, 2), 50);
    }

    #[test]
    fn mobius_basic() {
        assert_eq!(mobius(1), 1);
        assert_eq!(mobius(2), -1);    // one prime factor
        assert_eq!(mobius(6), 1);     // 2*3, two distinct primes
        assert_eq!(mobius(4), 0);     // 2^2, squared factor
        assert_eq!(mobius(30), -1);   // 2*3*5, three distinct primes
        assert_eq!(mobius(12), 0);    // 2^2 * 3
    }

    #[test]
    fn mobius_sum_property() {
        // sum_{d|n} mu(d) = 0 for n > 1, = 1 for n = 1
        for n in 1..=50u64 {
            let mut sum = 0i64;
            for d in 1..=n {
                if n % d == 0 {
                    sum += mobius(d) as i64;
                }
            }
            if n == 1 {
                assert_eq!(sum, 1);
            } else {
                assert_eq!(sum, 0, "Mobius sum property failed for n={}", n);
            }
        }
    }

    #[test]
    fn totient_mobius_relationship() {
        // phi(n) = n * sum_{d|n} mu(d)/d
        // Equivalently: phi(n) = sum_{d|n} mu(n/d) * d
        for n in 1..=50u64 {
            let mut sum = 0i64;
            for d in 1..=n {
                if n % d == 0 {
                    sum += mobius(n / d) as i64 * d as i64;
                }
            }
            assert_eq!(sum as u64, totient(n), "phi-mu relationship failed at n={}", n);
        }
    }

    #[test]
    fn landscape_render() {
        let land = TotientLandscape::new(Vec3::ZERO, 1.0, 10);
        let glyphs = land.render(50);
        assert_eq!(glyphs.len(), 50);
        // Primes should have ratio = (p-1)/p
        let g7 = &glyphs[6]; // n=7
        assert_eq!(g7.n, 7);
        assert!((g7.ratio - 6.0 / 7.0).abs() < 1e-6);
        assert_eq!(g7.character, 'P');
    }

    #[test]
    fn landscape_1d() {
        let land = TotientLandscape::new(Vec3::ZERO, 1.0, 10);
        let glyphs = land.render_1d(20);
        assert_eq!(glyphs.len(), 20);
    }
}
