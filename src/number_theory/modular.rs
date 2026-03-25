//! Modular arithmetic, CRT, discrete logarithm, and visualization.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

/// Modular exponentiation: (base^exp) mod modulus.
pub fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result = 1u128;
    let m = modulus as u128;
    base %= modulus;
    let mut b = base as u128;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result * b % m;
        }
        exp >>= 1;
        b = b * b % m;
    }
    result as u64
}

/// Extended Euclidean algorithm returning (gcd, x, y) such that a*x + b*y = gcd.
fn extended_gcd(a: i64, b: i64) -> (i64, i64, i64) {
    if a == 0 {
        return (b, 0, 1);
    }
    let (g, x1, y1) = extended_gcd(b % a, a);
    (g, y1 - (b / a) * x1, x1)
}

/// Modular inverse of a mod m, if it exists (gcd(a, m) == 1).
pub fn mod_inverse(a: u64, m: u64) -> Option<u64> {
    let (g, x, _) = extended_gcd(a as i64, m as i64);
    if g != 1 {
        return None;
    }
    Some(((x % m as i64 + m as i64) % m as i64) as u64)
}

/// Chinese Remainder Theorem: given residues[i] and moduli[i],
/// find x such that x ≡ residues[i] (mod moduli[i]) for all i.
/// Returns None if the system is inconsistent.
pub fn chinese_remainder_theorem(residues: &[u64], moduli: &[u64]) -> Option<u64> {
    if residues.len() != moduli.len() || residues.is_empty() {
        return None;
    }
    let mut r = residues[0] as i64;
    let mut m = moduli[0] as i64;

    for i in 1..residues.len() {
        let r2 = residues[i] as i64;
        let m2 = moduli[i] as i64;
        let (g, p, _) = extended_gcd(m, m2);
        if (r2 - r) % g != 0 {
            return None; // inconsistent
        }
        let lcm = m / g * m2;
        let diff = r2 - r;
        let adjust = diff / g * p % (m2 / g);
        r = ((r as i128 + m as i128 * adjust as i128) % lcm as i128 + lcm as i128) as i64
            % lcm as i64;
        m = lcm;
    }
    Some(((r % m + m) % m) as u64)
}

/// Euler's totient (needed for primitive roots).
fn euler_totient(mut n: u64) -> u64 {
    let mut result = n;
    let mut p = 2u64;
    while p * p <= n {
        if n % p == 0 {
            while n % p == 0 {
                n /= p;
            }
            result -= result / p;
        }
        p += 1;
    }
    if n > 1 {
        result -= result / n;
    }
    result
}

/// Find all primitive roots modulo n (if they exist).
/// Primitive roots exist iff n is 1, 2, 4, p^k, or 2*p^k for odd prime p.
pub fn primitive_roots(n: u64) -> Vec<u64> {
    if n <= 1 {
        return vec![];
    }
    if n == 2 {
        return vec![1];
    }
    let phi = euler_totient(n);
    // Factor phi to check orders
    let phi_factors = factor_small(phi);

    let mut roots = Vec::new();
    for g in 2..n {
        if gcd(g, n) != 1 {
            continue;
        }
        let mut is_root = true;
        for &(p, _) in &phi_factors {
            if mod_pow(g, phi / p, n) == 1 {
                is_root = false;
                break;
            }
        }
        if is_root {
            roots.push(g);
        }
    }
    roots
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn factor_small(mut n: u64) -> Vec<(u64, u32)> {
    let mut factors = Vec::new();
    let mut d = 2u64;
    while d * d <= n {
        let mut count = 0u32;
        while n % d == 0 {
            n /= d;
            count += 1;
        }
        if count > 0 {
            factors.push((d, count));
        }
        d += 1;
    }
    if n > 1 {
        factors.push((n, 1));
    }
    factors
}

/// Baby-step giant-step discrete logarithm:
/// find x such that base^x ≡ target (mod modulus).
pub fn discrete_log(base: u64, target: u64, modulus: u64) -> Option<u64> {
    if modulus <= 1 {
        return None;
    }
    let m = (modulus as f64).sqrt().ceil() as u64;

    // Baby step: base^j for j = 0..m
    let mut table: HashMap<u64, u64> = HashMap::new();
    let mut power = 1u64;
    for j in 0..m {
        table.insert(power, j);
        power = (power as u128 * base as u128 % modulus as u128) as u64;
    }

    // Giant step factor: base^{-m} mod modulus
    let base_inv_m = match mod_inverse(mod_pow(base, m, modulus), modulus) {
        Some(v) => v,
        None => return None,
    };

    let mut gamma = target;
    for i in 0..m {
        if let Some(&j) = table.get(&gamma) {
            return Some(i * m + j);
        }
        gamma = (gamma as u128 * base_inv_m as u128 % modulus as u128) as u64;
    }
    None
}

// ─── Visualization ──────────────────────────────────────────────────────────

/// Renders residue classes as positions on a circle (clock face).
pub struct ClockVisualization {
    pub modulus: u64,
    pub center: Vec2,
    pub radius: f32,
}

pub struct ClockGlyph {
    pub residue: u64,
    pub position: Vec2,
    pub color: Vec4,
    pub character: char,
}

impl ClockVisualization {
    pub fn new(modulus: u64, center: Vec2, radius: f32) -> Self {
        Self { modulus, center, radius }
    }

    /// Place residues 0..modulus around a circle.
    pub fn generate(&self) -> Vec<ClockGlyph> {
        let n = self.modulus;
        (0..n)
            .map(|r| {
                let angle =
                    std::f32::consts::FRAC_PI_2 - (r as f32 / n as f32) * std::f32::consts::TAU;
                let pos = self.center + Vec2::new(angle.cos(), angle.sin()) * self.radius;
                let hue = r as f32 / n as f32;
                ClockGlyph {
                    residue: r,
                    position: pos,
                    color: Vec4::new(hue, 1.0 - hue, 0.5, 1.0),
                    character: std::char::from_digit(r as u32 % 36, 36).unwrap_or('?'),
                }
            })
            .collect()
    }

    /// Highlight a specific residue class with connections.
    pub fn residue_class(&self, value: u64) -> Vec<Vec2> {
        let r = value % self.modulus;
        let angle = std::f32::consts::FRAC_PI_2
            - (r as f32 / self.modulus as f32) * std::f32::consts::TAU;
        vec![
            self.center,
            self.center + Vec2::new(angle.cos(), angle.sin()) * self.radius,
        ]
    }
}

/// Render multiplication tables mod n as glyph grids.
pub struct ResiduePattern {
    pub modulus: u64,
    pub cell_size: f32,
}

pub struct PatternCell {
    pub row: u64,
    pub col: u64,
    pub value: u64,
    pub position: Vec2,
    pub color: Vec4,
}

impl ResiduePattern {
    pub fn new(modulus: u64, cell_size: f32) -> Self {
        Self { modulus, cell_size }
    }

    /// Generate the full multiplication table mod n.
    pub fn multiplication_table(&self) -> Vec<PatternCell> {
        let n = self.modulus;
        let mut cells = Vec::with_capacity((n * n) as usize);
        for r in 0..n {
            for c in 0..n {
                let val = (r * c) % n;
                let brightness = val as f32 / n as f32;
                cells.push(PatternCell {
                    row: r,
                    col: c,
                    value: val,
                    position: Vec2::new(c as f32 * self.cell_size, r as f32 * self.cell_size),
                    color: Vec4::new(brightness, 0.2, 1.0 - brightness, 1.0),
                });
            }
        }
        cells
    }

    /// Generate the addition table mod n.
    pub fn addition_table(&self) -> Vec<PatternCell> {
        let n = self.modulus;
        let mut cells = Vec::with_capacity((n * n) as usize);
        for r in 0..n {
            for c in 0..n {
                let val = (r + c) % n;
                let brightness = val as f32 / n as f32;
                cells.push(PatternCell {
                    row: r,
                    col: c,
                    value: val,
                    position: Vec2::new(c as f32 * self.cell_size, r as f32 * self.cell_size),
                    color: Vec4::new(0.2, brightness, 1.0 - brightness, 1.0),
                });
            }
        }
        cells
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_pow() {
        assert_eq!(mod_pow(2, 10, 1000), 24);
        assert_eq!(mod_pow(3, 0, 7), 1);
        assert_eq!(mod_pow(2, 10, 1024), 0);
        assert_eq!(mod_pow(5, 3, 13), 8); // 125 % 13 = 8
    }

    #[test]
    fn test_mod_inverse() {
        assert_eq!(mod_inverse(3, 7), Some(5)); // 3*5 = 15 ≡ 1 (mod 7)
        assert_eq!(mod_inverse(2, 4), None); // gcd(2,4) = 2
        assert_eq!(mod_inverse(1, 5), Some(1));
        let inv = mod_inverse(17, 43).unwrap();
        assert_eq!((17 * inv) % 43, 1);
    }

    #[test]
    fn test_crt() {
        // x ≡ 2 (mod 3), x ≡ 3 (mod 5), x ≡ 2 (mod 7)
        let result = chinese_remainder_theorem(&[2, 3, 2], &[3, 5, 7]).unwrap();
        assert_eq!(result % 3, 2);
        assert_eq!(result % 5, 3);
        assert_eq!(result % 7, 2);
        assert_eq!(result, 23);
    }

    #[test]
    fn test_crt_inconsistent() {
        // x ≡ 1 (mod 2), x ≡ 0 (mod 2) — impossible
        assert!(chinese_remainder_theorem(&[1, 0], &[2, 2]).is_none());
    }

    #[test]
    fn test_primitive_roots() {
        let roots = primitive_roots(7);
        assert!(roots.contains(&3));
        assert!(roots.contains(&5));
        // Verify they generate all residues
        for &g in &roots {
            let mut seen = std::collections::HashSet::new();
            let mut val = 1u64;
            for _ in 0..6 {
                seen.insert(val);
                val = val * g % 7;
            }
            assert_eq!(seen.len(), 6);
        }
    }

    #[test]
    fn test_primitive_roots_11() {
        let roots = primitive_roots(11);
        assert!(roots.contains(&2));
        // phi(11) = 10, number of primitive roots = phi(phi(11)) = phi(10) = 4
        assert_eq!(roots.len(), 4);
    }

    #[test]
    fn test_discrete_log() {
        // 2^x ≡ 8 (mod 13) => x = 3
        assert_eq!(discrete_log(2, 8, 13), Some(3));
        // 3^x ≡ 1 (mod 7) => x = 0 (or 6)
        let x = discrete_log(3, 1, 7).unwrap();
        assert_eq!(mod_pow(3, x, 7), 1);
    }

    #[test]
    fn test_discrete_log_larger() {
        // 5^x ≡ 12 (mod 23)
        if let Some(x) = discrete_log(5, 12, 23) {
            assert_eq!(mod_pow(5, x, 23), 12);
        }
    }

    #[test]
    fn test_clock_visualization() {
        let clock = ClockVisualization::new(12, Vec2::ZERO, 5.0);
        let glyphs = clock.generate();
        assert_eq!(glyphs.len(), 12);
        // residue 0 should be at top (angle = pi/2)
        let top = &glyphs[0];
        assert!((top.position.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_residue_pattern() {
        let pat = ResiduePattern::new(5, 1.0);
        let table = pat.multiplication_table();
        assert_eq!(table.len(), 25);
        // 3 * 4 mod 5 = 2
        let cell = &table[3 * 5 + 4]; // row 3, col 4
        assert_eq!(cell.value, 2);
    }

    #[test]
    fn test_mod_pow_large() {
        // 2^64 mod 1000000007
        let result = mod_pow(2, 64, 1_000_000_007);
        // 2^64 = 18446744073709551616, mod 10^9+7 = 582344008
        assert_eq!(result, 582344008);
    }
}
