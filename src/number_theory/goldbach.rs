//! Goldbach's conjecture: partitions, counts, and comet visualization.

use glam::{Vec2, Vec3, Vec4};

/// Find a pair of primes (p, q) such that p + q = n.
/// Returns None if n is odd, less than 4, or no partition is found.
pub fn goldbach_partition(n: u64) -> Option<(u64, u64)> {
    if n < 4 || n % 2 != 0 {
        return None;
    }
    let primes = super::primes::sieve_of_eratosthenes(n);
    let prime_set: std::collections::HashSet<u64> = primes.iter().copied().collect();
    for &p in &primes {
        if p > n / 2 {
            break;
        }
        let q = n - p;
        if prime_set.contains(&q) {
            return Some((p, q));
        }
    }
    None
}

/// Count the number of ways to write n as a sum of two primes (unordered, p <= q).
pub fn goldbach_count(n: u64) -> u32 {
    if n < 4 || n % 2 != 0 {
        return 0;
    }
    let primes = super::primes::sieve_of_eratosthenes(n);
    let prime_set: std::collections::HashSet<u64> = primes.iter().copied().collect();
    let mut count = 0u32;
    for &p in &primes {
        if p > n / 2 {
            break;
        }
        let q = n - p;
        if prime_set.contains(&q) {
            count += 1;
        }
    }
    count
}

/// Generate the "Goldbach comet": for each even n in [4, limit], compute the
/// number of Goldbach partitions. Returns (n, count) pairs.
pub fn goldbach_comet(limit: u64) -> Vec<(u64, u32)> {
    let primes = super::primes::sieve_of_eratosthenes(limit);
    let prime_set: std::collections::HashSet<u64> = primes.iter().copied().collect();

    let mut result = Vec::new();
    let mut n = 4u64;
    while n <= limit {
        let mut count = 0u32;
        for &p in &primes {
            if p > n / 2 {
                break;
            }
            if prime_set.contains(&(n - p)) {
                count += 1;
            }
        }
        result.push((n, count));
        n += 2;
    }
    result
}

// ─── Renderer ───────────────────────────────────────────────────────────────

/// Scatter plot of Goldbach partition counts (the "comet" shape).
pub struct GoldbachRenderer {
    pub origin: Vec3,
    pub x_scale: f32,
    pub y_scale: f32,
}

pub struct GoldbachGlyph {
    pub n: u64,
    pub count: u32,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl GoldbachRenderer {
    pub fn new(origin: Vec3, x_scale: f32, y_scale: f32) -> Self {
        Self { origin, x_scale, y_scale }
    }

    /// Render the comet as a scatter of glyphs.
    pub fn render(&self, comet: &[(u64, u32)]) -> Vec<GoldbachGlyph> {
        let max_count = comet.iter().map(|&(_, c)| c).max().unwrap_or(1) as f32;
        comet
            .iter()
            .enumerate()
            .map(|(i, &(n, count))| {
                let x = i as f32 * self.x_scale;
                let y = count as f32 * self.y_scale;
                let t = count as f32 / max_count;
                GoldbachGlyph {
                    n,
                    count,
                    position: self.origin + Vec3::new(x, y, 0.0),
                    color: Vec4::new(0.2, t, 1.0 - t * 0.5, 1.0),
                    character: if count > (max_count * 0.5) as u32 { '*' } else { '.' },
                }
            })
            .collect()
    }

    /// Highlight a specific partition: render the two primes and their sum.
    pub fn render_partition(&self, n: u64) -> Vec<GoldbachGlyph> {
        let mut glyphs = Vec::new();
        if let Some((p, q)) = goldbach_partition(n) {
            glyphs.push(GoldbachGlyph {
                n: p,
                count: 0,
                position: self.origin + Vec3::new(p as f32 * self.x_scale, 0.0, 0.0),
                color: Vec4::new(1.0, 0.3, 0.3, 1.0),
                character: 'P',
            });
            glyphs.push(GoldbachGlyph {
                n: q,
                count: 0,
                position: self.origin + Vec3::new(q as f32 * self.x_scale, 0.0, 0.0),
                color: Vec4::new(0.3, 1.0, 0.3, 1.0),
                character: 'Q',
            });
            glyphs.push(GoldbachGlyph {
                n,
                count: 1,
                position: self.origin + Vec3::new(n as f32 * self.x_scale, 1.0, 0.0),
                color: Vec4::new(1.0, 1.0, 0.3, 1.0),
                character: '=',
            });
        }
        glyphs
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_basic() {
        assert_eq!(goldbach_partition(4), Some((2, 2)));
        let p10 = goldbach_partition(10).unwrap();
        assert_eq!(p10.0 + p10.1, 10);
        assert!(super::super::primes::is_prime(p10.0));
        assert!(super::super::primes::is_prime(p10.1));
    }

    #[test]
    fn partition_odd_returns_none() {
        assert_eq!(goldbach_partition(7), None);
        assert_eq!(goldbach_partition(3), None);
    }

    #[test]
    fn partition_all_even() {
        // Verify Goldbach for all even n from 4 to 1000
        for n in (4..=1000).step_by(2) {
            assert!(
                goldbach_partition(n).is_some(),
                "Goldbach failed for {}",
                n
            );
        }
    }

    #[test]
    fn count_basic() {
        assert_eq!(goldbach_count(4), 1);  // 2+2
        assert_eq!(goldbach_count(10), 2); // 3+7, 5+5
        assert_eq!(goldbach_count(20), 2); // 3+17, 7+13
    }

    #[test]
    fn count_odd_zero() {
        assert_eq!(goldbach_count(7), 0);
    }

    #[test]
    fn comet_basic() {
        let comet = goldbach_comet(20);
        // Even numbers from 4 to 20: 4,6,8,10,12,14,16,18,20 = 9 entries
        assert_eq!(comet.len(), 9);
        assert_eq!(comet[0].0, 4);
        assert_eq!(comet[0].1, 1); // 2+2
    }

    #[test]
    fn comet_monotonically_positive() {
        let comet = goldbach_comet(200);
        for &(n, count) in &comet {
            assert!(count >= 1, "even {} should have at least one partition", n);
        }
    }

    #[test]
    fn renderer_produces_glyphs() {
        let comet = goldbach_comet(100);
        let r = GoldbachRenderer::new(Vec3::ZERO, 0.1, 1.0);
        let glyphs = r.render(&comet);
        assert_eq!(glyphs.len(), comet.len());
    }

    #[test]
    fn renderer_partition() {
        let r = GoldbachRenderer::new(Vec3::ZERO, 0.1, 1.0);
        let glyphs = r.render_partition(10);
        assert_eq!(glyphs.len(), 3); // P, Q, =
    }
}
