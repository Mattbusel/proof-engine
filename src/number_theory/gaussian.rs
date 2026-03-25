//! Gaussian integers Z[i]: arithmetic, primes, factorization, and lattice rendering.

use glam::{Vec2, Vec3, Vec4};

/// A Gaussian integer a + bi where a, b are in Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GaussianInt {
    pub re: i64,
    pub im: i64,
}

impl GaussianInt {
    pub const ZERO: Self = Self { re: 0, im: 0 };
    pub const ONE: Self = Self { re: 1, im: 0 };
    pub const I: Self = Self { re: 0, im: 1 };

    pub fn new(re: i64, im: i64) -> Self {
        Self { re, im }
    }

    /// Norm: N(a+bi) = a^2 + b^2.
    pub fn norm(self) -> i64 {
        self.re * self.re + self.im * self.im
    }

    /// Complex conjugate: conj(a+bi) = a - bi.
    pub fn conj(self) -> Self {
        Self { re: self.re, im: -self.im }
    }

    /// Whether this is the zero element.
    pub fn is_zero(self) -> bool {
        self.re == 0 && self.im == 0
    }

    /// Whether this is a unit (norm = 1): {1, -1, i, -i}.
    pub fn is_unit(self) -> bool {
        self.norm() == 1
    }

    /// Gaussian integer division (rounded): returns (quotient, remainder).
    pub fn div_rem(self, other: Self) -> (Self, Self) {
        if other.is_zero() {
            panic!("division by zero in Gaussian integers");
        }
        // (a+bi)/(c+di) = (a+bi)(c-di) / (c^2+d^2)
        let n = other.norm();
        let num = self * other.conj();
        // Round to nearest Gaussian integer
        let qr = div_round(num.re, n);
        let qi = div_round(num.im, n);
        let q = GaussianInt::new(qr, qi);
        let r = self - q * other;
        (q, r)
    }
}

fn div_round(a: i64, b: i64) -> i64 {
    // Round to nearest integer (ties go to even or just use standard rounding)
    let (d, r) = (a / b, a % b);
    if 2 * r.abs() > b.abs() {
        if (a > 0) == (b > 0) { d + 1 } else { d - 1 }
    } else {
        d
    }
}

impl std::ops::Add for GaussianInt {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { re: self.re + rhs.re, im: self.im + rhs.im }
    }
}

impl std::ops::Sub for GaussianInt {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { re: self.re - rhs.re, im: self.im - rhs.im }
    }
}

impl std::ops::Mul for GaussianInt {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::Neg for GaussianInt {
    type Output = Self;
    fn neg(self) -> Self {
        Self { re: -self.re, im: -self.im }
    }
}

impl std::fmt::Display for GaussianInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.im == 0 {
            write!(f, "{}", self.re)
        } else if self.re == 0 {
            if self.im == 1 {
                write!(f, "i")
            } else if self.im == -1 {
                write!(f, "-i")
            } else {
                write!(f, "{}i", self.im)
            }
        } else {
            let sign = if self.im > 0 { "+" } else { "-" };
            let abs_im = self.im.abs();
            if abs_im == 1 {
                write!(f, "{}{}i", self.re, sign)
            } else {
                write!(f, "{}{}{}i", self.re, sign, abs_im)
            }
        }
    }
}

/// GCD of two Gaussian integers via the Euclidean algorithm.
pub fn gcd(mut a: GaussianInt, mut b: GaussianInt) -> GaussianInt {
    while !b.is_zero() {
        let (_, r) = a.div_rem(b);
        a = b;
        b = r;
    }
    // Normalize: prefer associate with positive real part
    normalize(a)
}

/// Normalize a Gaussian integer to its canonical associate
/// (multiply by unit so that the result is in the first quadrant or positive real axis).
fn normalize(z: GaussianInt) -> GaussianInt {
    if z.is_zero() {
        return z;
    }
    // Multiply by i^k to get into "first quadrant": re > 0, im >= 0
    // or re >= 0, im > 0
    let units = [
        GaussianInt::new(1, 0),
        GaussianInt::new(0, 1),
        GaussianInt::new(-1, 0),
        GaussianInt::new(0, -1),
    ];
    for &u in &units {
        let w = z * u;
        if w.re > 0 && w.im >= 0 {
            return w;
        }
    }
    // fallback: just pick re >= 0
    for &u in &units {
        let w = z * u;
        if w.re >= 0 && w.im >= 0 {
            return w;
        }
    }
    z
}

/// Check if a Gaussian integer is a Gaussian prime.
pub fn is_gaussian_prime(z: GaussianInt) -> bool {
    if z.is_zero() {
        return false;
    }
    let n = z.norm();
    if n <= 1 {
        return false; // units are not primes
    }
    // z is a Gaussian prime iff:
    // 1. z = a + 0i (or 0 + bi) and |a| (or |b|) is an ordinary prime p ≡ 3 (mod 4)
    // 2. N(z) = a^2 + b^2 is an ordinary prime
    super::primes::is_prime(n as u64)
        || (z.im == 0 && {
            let p = z.re.unsigned_abs();
            super::primes::is_prime(p) && p % 4 == 3
        })
        || (z.re == 0 && {
            let p = z.im.unsigned_abs();
            super::primes::is_prime(p) && p % 4 == 3
        })
}

/// Factorize a Gaussian integer into Gaussian primes.
/// Returns a list of (not necessarily normalized) prime factors.
pub fn factorize(z: GaussianInt) -> Vec<GaussianInt> {
    if z.is_zero() || z.is_unit() {
        return vec![];
    }

    let mut factors = Vec::new();
    let mut remaining = z;

    // Factor out units to make norm analysis cleaner
    let n = remaining.norm();

    // Factor the norm in the ordinary integers
    let norm_factors = super::primes::prime_factorization(n as u64);

    for (p, exp) in norm_factors {
        let p_val = p as i64;
        if p == 2 {
            // 2 = -i * (1+i)^2, and (1+i) is a Gaussian prime
            let pi = GaussianInt::new(1, 1);
            for _ in 0..exp {
                let (q, r) = remaining.div_rem(pi);
                if r.is_zero() {
                    factors.push(pi);
                    remaining = q;
                } else {
                    break;
                }
            }
        } else if p % 4 == 3 {
            // p stays prime in Z[i]; it divides as p itself
            let gp = GaussianInt::new(p_val, 0);
            let pairs = exp / 2; // norm contribution is p^2 per factor of p in Z[i]
            for _ in 0..pairs {
                let (q, r) = remaining.div_rem(gp);
                if r.is_zero() {
                    factors.push(gp);
                    remaining = q;
                } else {
                    break;
                }
            }
        } else {
            // p ≡ 1 (mod 4): splits as p = pi * conj(pi)
            // Find a+bi such that a^2 + b^2 = p
            if let Some((a, b)) = sum_of_two_squares(p) {
                let pi1 = GaussianInt::new(a as i64, b as i64);
                let pi2 = pi1.conj();
                for pi in &[pi1, pi2] {
                    loop {
                        let (q, r) = remaining.div_rem(*pi);
                        if r.is_zero() {
                            factors.push(*pi);
                            remaining = q;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    // remaining should be a unit; if not unit, include it
    if !remaining.is_unit() && !remaining.is_zero() {
        factors.push(remaining);
    }

    factors
}

/// Find a, b >= 0 such that a^2 + b^2 = p (for prime p ≡ 1 mod 4).
fn sum_of_two_squares(p: u64) -> Option<(u64, u64)> {
    if p == 2 {
        return Some((1, 1));
    }
    if p % 4 != 1 {
        return None;
    }
    // Find a square root of -1 mod p
    let mut r = 2u64;
    loop {
        let candidate = super::modular::mod_pow(r, (p - 1) / 4, p);
        if (candidate as u128 * candidate as u128) % p as u128 == p as u128 - 1 {
            // Use Cornacchia / Fermat descent
            return fermat_descent(p, candidate);
        }
        r += 1;
        if r >= p {
            return None;
        }
    }
}

fn fermat_descent(p: u64, mut a: u64) -> Option<(u64, u64)> {
    let mut b = p;
    let limit = (p as f64).sqrt() as u64;
    while a > limit {
        let t = b % a;
        b = a;
        a = t;
    }
    let remainder = p - a * a;
    let b_sq = (remainder as f64).sqrt().round() as u64;
    if b_sq * b_sq == remainder {
        Some((a.max(b_sq), a.min(b_sq)))
    } else {
        None
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

/// Render Gaussian integer lattice points with primes highlighted.
pub struct GaussianLatticeRenderer {
    pub range: i64,
    pub origin: Vec3,
    pub scale: f32,
}

pub struct LatticeGlyph {
    pub z: GaussianInt,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
    pub is_prime: bool,
}

impl GaussianLatticeRenderer {
    pub fn new(range: i64, origin: Vec3, scale: f32) -> Self {
        Self { range, origin, scale }
    }

    /// Generate lattice glyphs for all Gaussian integers with |re|, |im| <= range.
    pub fn render(&self) -> Vec<LatticeGlyph> {
        let mut glyphs = Vec::new();
        for a in -self.range..=self.range {
            for b in -self.range..=self.range {
                let z = GaussianInt::new(a, b);
                let prime = is_gaussian_prime(z);
                let norm = z.norm() as f32;
                let max_norm = (2 * self.range * self.range) as f32;
                let t = (norm / max_norm).min(1.0);
                let color = if prime {
                    Vec4::new(1.0, 0.2, 0.2, 1.0)
                } else if z.is_unit() {
                    Vec4::new(1.0, 1.0, 0.0, 1.0)
                } else {
                    Vec4::new(0.3, 0.3 + t * 0.5, 0.8, 0.6)
                };
                let ch = if prime {
                    '*'
                } else if z.is_zero() {
                    'O'
                } else if z.is_unit() {
                    'U'
                } else {
                    '.'
                };
                glyphs.push(LatticeGlyph {
                    z,
                    position: self.origin
                        + Vec3::new(a as f32 * self.scale, b as f32 * self.scale, 0.0),
                    color,
                    character: ch,
                    is_prime: prime,
                });
            }
        }
        glyphs
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        let a = GaussianInt::new(3, 4);
        let b = GaussianInt::new(1, -2);
        assert_eq!(a + b, GaussianInt::new(4, 2));
        assert_eq!(a - b, GaussianInt::new(2, 6));
        // (3+4i)(1-2i) = 3 -6i +4i -8i^2 = 11 - 2i
        assert_eq!(a * b, GaussianInt::new(11, -2));
    }

    #[test]
    fn norm_and_conj() {
        let z = GaussianInt::new(3, 4);
        assert_eq!(z.norm(), 25);
        assert_eq!(z.conj(), GaussianInt::new(3, -4));
        assert_eq!(z * z.conj(), GaussianInt::new(25, 0));
    }

    #[test]
    fn units() {
        assert!(GaussianInt::ONE.is_unit());
        assert!(GaussianInt::I.is_unit());
        assert!((-GaussianInt::ONE).is_unit());
        assert!((-GaussianInt::I).is_unit());
        assert!(!GaussianInt::new(1, 1).is_unit());
    }

    #[test]
    fn div_rem_exact() {
        let a = GaussianInt::new(11, -2);
        let b = GaussianInt::new(1, -2);
        let (q, r) = a.div_rem(b);
        assert_eq!(a, q * b + r);
        assert!(r.norm() <= b.norm()); // remainder has smaller norm (approximately)
    }

    #[test]
    fn gcd_test() {
        let a = GaussianInt::new(3, 4);
        let b = GaussianInt::new(1, 2);
        let g = gcd(a, b);
        // gcd should divide both
        let (_, r1) = a.div_rem(g);
        let (_, r2) = b.div_rem(g);
        assert!(r1.is_zero(), "gcd should divide a, remainder = {:?}", r1);
        assert!(r2.is_zero(), "gcd should divide b, remainder = {:?}", r2);
    }

    #[test]
    fn gaussian_primes() {
        // 1+i has norm 2 which is prime => Gaussian prime
        assert!(is_gaussian_prime(GaussianInt::new(1, 1)));
        // 3 is ordinary prime, 3 ≡ 3 mod 4 => stays prime
        assert!(is_gaussian_prime(GaussianInt::new(3, 0)));
        // 5 = (2+i)(2-i), norm(2+i) = 5 which is prime => 2+i is Gaussian prime
        assert!(is_gaussian_prime(GaussianInt::new(2, 1)));
        // 5 itself: norm = 25, not prime => not a Gaussian prime
        assert!(!is_gaussian_prime(GaussianInt::new(5, 0)));
        // Units are not prime
        assert!(!is_gaussian_prime(GaussianInt::ONE));
        assert!(!is_gaussian_prime(GaussianInt::I));
    }

    #[test]
    fn factorize_5() {
        // 5 = (2+i)(2-i) up to units
        let z = GaussianInt::new(5, 0);
        let factors = factorize(z);
        // Product of factors * some unit should equal z
        let product = factors.iter().fold(GaussianInt::ONE, |acc, &f| acc * f);
        assert_eq!(product.norm(), z.norm());
        assert!(factors.len() >= 2);
    }

    #[test]
    fn factorize_prime_3() {
        // 3 ≡ 3 mod 4, stays prime in Z[i]
        let z = GaussianInt::new(3, 0);
        let factors = factorize(z);
        assert_eq!(factors.len(), 1);
        assert_eq!(factors[0].norm(), 9); // norm of 3 is 9
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", GaussianInt::new(3, 4)), "3+4i");
        assert_eq!(format!("{}", GaussianInt::new(3, -4)), "3-4i");
        assert_eq!(format!("{}", GaussianInt::new(0, 1)), "i");
        assert_eq!(format!("{}", GaussianInt::new(5, 0)), "5");
    }

    #[test]
    fn lattice_renderer() {
        let r = GaussianLatticeRenderer::new(3, Vec3::ZERO, 1.0);
        let glyphs = r.render();
        assert_eq!(glyphs.len(), 7 * 7); // -3..=3 in both dims
        let prime_count = glyphs.iter().filter(|g| g.is_prime).count();
        assert!(prime_count > 0);
    }
}
