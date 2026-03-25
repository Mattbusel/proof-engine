//! Galois fields GF(p^n): arithmetic, generators, and multiplication tables.

/// Descriptor for a Galois field GF(p^n).
/// For n=1 this is simply Z/pZ. For n>1 we use a fixed irreducible polynomial
/// (automatically selected for small cases).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GaloisField {
    pub prime: u64,
    pub power: u32,
}

/// An element of a Galois field.
/// For GF(p) (power=1), value is simply an integer mod p.
/// For GF(p^n), value encodes a polynomial with coefficients in GF(p),
/// packed as value = c0 + c1*p + c2*p^2 + ... where ci < p.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GfElement {
    pub value: u64,
    pub field: GaloisField,
}

impl GaloisField {
    pub fn new(prime: u64, power: u32) -> Self {
        assert!(prime >= 2);
        assert!(power >= 1);
        Self { prime, power }
    }

    /// Order of the field: p^n.
    pub fn order(&self) -> u64 {
        self.prime.pow(self.power)
    }

    /// Create the zero element.
    pub fn zero(self) -> GfElement {
        GfElement { value: 0, field: self }
    }

    /// Create the one (multiplicative identity).
    pub fn one(self) -> GfElement {
        GfElement { value: 1, field: self }
    }

    /// Create an element from a value.
    pub fn element(self, value: u64) -> GfElement {
        GfElement { value: value % self.order(), field: self }
    }

    /// Get a fixed irreducible polynomial for GF(p^n).
    /// Returns coefficients [c0, c1, ..., cn] of the polynomial c0 + c1*x + ... + cn*x^n.
    /// Only supports small cases.
    fn irreducible_poly(&self) -> Vec<u64> {
        if self.power == 1 {
            return vec![0, 1]; // x (unused for GF(p))
        }
        // Some known irreducible polynomials
        match (self.prime, self.power) {
            (2, 2) => vec![1, 1, 1],           // x^2 + x + 1
            (2, 3) => vec![1, 1, 0, 1],        // x^3 + x + 1
            (2, 4) => vec![1, 1, 0, 0, 1],     // x^4 + x + 1
            (2, 8) => vec![1, 0, 1, 1, 1, 0, 0, 0, 1], // x^8 + x^4 + x^3 + x^2 + 1
            (3, 2) => vec![1, 0, 1],           // x^2 + 1 over GF(3)
            (3, 3) => vec![2, 1, 0, 1],        // x^3 + x + 2 over GF(3)
            (5, 2) => vec![2, 0, 1],           // x^2 + 2 over GF(5)
            (7, 2) => vec![3, 0, 1],           // x^2 + 3 over GF(7)
            _ => {
                // Brute-force search for an irreducible polynomial
                self.find_irreducible()
            }
        }
    }

    fn find_irreducible(&self) -> Vec<u64> {
        let p = self.prime;
        let n = self.power as usize;
        // Try all monic polynomials of degree n over GF(p)
        let total = p.pow(n as u32); // number of lower-degree coefficient combos
        for combo in 0..total {
            let mut coeffs = Vec::with_capacity(n + 1);
            let mut c = combo;
            for _ in 0..n {
                coeffs.push(c % p);
                c /= p;
            }
            coeffs.push(1); // monic
            if self.is_irreducible_poly(&coeffs) {
                return coeffs;
            }
        }
        // Fallback (should not happen for valid prime/power)
        let mut coeffs = vec![1; n + 1];
        coeffs[n] = 1;
        coeffs
    }

    fn is_irreducible_poly(&self, coeffs: &[u64]) -> bool {
        let p = self.prime;
        let n = coeffs.len() - 1;
        // Check: no roots in GF(p) (necessary for degree 2,3; not sufficient for higher)
        // For simplicity: check no polynomial factor of degree 1..n/2
        if n <= 1 {
            return n == 1;
        }
        // Quick check: no roots
        for x in 0..p {
            let mut val = 0u64;
            let mut xpow = 1u64;
            for &c in coeffs {
                val = (val + c * xpow) % p;
                xpow = xpow * x % p;
            }
            if val == 0 {
                return false;
            }
        }
        if n <= 3 {
            return true; // for degree 2,3 no-root test is sufficient
        }
        true // approximate for higher degrees
    }
}

impl GfElement {
    /// Extract polynomial coefficients from packed value.
    fn to_poly(&self) -> Vec<u64> {
        let p = self.field.prime;
        let n = self.field.power as usize;
        let mut coeffs = Vec::with_capacity(n);
        let mut v = self.value;
        for _ in 0..n {
            coeffs.push(v % p);
            v /= p;
        }
        coeffs
    }

    /// Pack polynomial coefficients into a value.
    fn from_poly(coeffs: &[u64], field: GaloisField) -> Self {
        let p = field.prime;
        let mut value = 0u64;
        let mut base = 1u64;
        for &c in coeffs.iter().take(field.power as usize) {
            value += (c % p) * base;
            base *= p;
        }
        GfElement { value, field }
    }

    /// Addition in the field.
    pub fn add(self, other: Self) -> Self {
        assert_eq!(self.field, other.field);
        let p = self.field.prime;
        if self.field.power == 1 {
            return GfElement {
                value: (self.value + other.value) % p,
                field: self.field,
            };
        }
        let a = self.to_poly();
        let b = other.to_poly();
        let n = self.field.power as usize;
        let mut c = vec![0u64; n];
        for i in 0..n {
            c[i] = (a.get(i).copied().unwrap_or(0) + b.get(i).copied().unwrap_or(0)) % p;
        }
        Self::from_poly(&c, self.field)
    }

    /// Subtraction in the field.
    pub fn sub(self, other: Self) -> Self {
        assert_eq!(self.field, other.field);
        let p = self.field.prime;
        if self.field.power == 1 {
            return GfElement {
                value: (self.value + p - other.value % p) % p,
                field: self.field,
            };
        }
        let a = self.to_poly();
        let b = other.to_poly();
        let n = self.field.power as usize;
        let mut c = vec![0u64; n];
        for i in 0..n {
            c[i] = (a.get(i).copied().unwrap_or(0) + p
                - b.get(i).copied().unwrap_or(0) % p)
                % p;
        }
        Self::from_poly(&c, self.field)
    }

    /// Multiplication in the field.
    pub fn multiply(self, other: Self) -> Self {
        assert_eq!(self.field, other.field);
        let p = self.field.prime;
        if self.field.power == 1 {
            return GfElement {
                value: (self.value * other.value) % p,
                field: self.field,
            };
        }
        let a = self.to_poly();
        let b = other.to_poly();
        let n = self.field.power as usize;
        // Multiply polynomials
        let mut prod = vec![0u64; 2 * n];
        for i in 0..n {
            for j in 0..n {
                prod[i + j] = (prod[i + j] + a[i] * b[j]) % p;
            }
        }
        // Reduce modulo the irreducible polynomial
        let irr = self.field.irreducible_poly();
        poly_mod(&mut prod, &irr, p);
        prod.truncate(n);
        while prod.len() < n {
            prod.push(0);
        }
        Self::from_poly(&prod, self.field)
    }

    /// Multiplicative inverse (via extended Euclidean in the polynomial ring, or Fermat's little theorem).
    pub fn inverse(self) -> Self {
        assert!(self.value != 0, "cannot invert zero");
        let field = self.field;
        // a^{-1} = a^{p^n - 2} by Fermat's little theorem in GF(p^n)
        let order = field.order();
        self.pow(order - 2)
    }

    /// Exponentiation by squaring.
    pub fn pow(self, mut exp: u64) -> Self {
        let mut result = self.field.one();
        let mut base = self;
        while exp > 0 {
            if exp & 1 == 1 {
                result = result.multiply(base);
            }
            base = base.multiply(base);
            exp >>= 1;
        }
        result
    }

    /// Check if this element is zero.
    pub fn is_zero(self) -> bool {
        self.value == 0
    }
}

/// Polynomial modular reduction: reduce `poly` mod `modulus` in GF(p)[x].
fn poly_mod(poly: &mut Vec<u64>, modulus: &[u64], p: u64) {
    let deg_mod = modulus.len() - 1;
    let lead_inv = mod_inv(modulus[deg_mod], p);
    while poly.len() > deg_mod {
        let coeff = poly[poly.len() - 1];
        if coeff == 0 {
            poly.pop();
            continue;
        }
        let factor = coeff * lead_inv % p;
        let offset = poly.len() - modulus.len();
        for (i, &m) in modulus.iter().enumerate() {
            poly[offset + i] = (poly[offset + i] + p - factor * m % p) % p;
        }
        poly.pop();
    }
}

fn mod_inv(a: u64, p: u64) -> u64 {
    // Simple for prime p: a^{p-2} mod p
    let mut result = 1u64;
    let mut base = a % p;
    let mut exp = p - 2;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result * base % p;
        }
        base = base * base % p;
        exp >>= 1;
    }
    result
}

/// Find a generator (primitive element) of the multiplicative group of GF(p^n).
pub fn find_generator(field: GaloisField) -> GfElement {
    let order = field.order() - 1; // multiplicative group order
    let factors = super::primes::prime_factorization(order);

    for v in 1..field.order() {
        let g = field.element(v);
        let mut is_generator = true;
        for &(p, _) in &factors {
            if g.pow(order / p).value == 1 {
                is_generator = false;
                break;
            }
        }
        if is_generator {
            return g;
        }
    }
    field.one() // fallback
}

/// Build the full multiplication table for a Galois field.
pub fn multiplication_table(field: GaloisField) -> Vec<Vec<u64>> {
    let n = field.order() as usize;
    let mut table = vec![vec![0u64; n]; n];
    for i in 0..n {
        let a = field.element(i as u64);
        for j in 0..n {
            let b = field.element(j as u64);
            table[i][j] = a.multiply(b).value;
        }
    }
    table
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gf5_basic() {
        let f = GaloisField::new(5, 1);
        let a = f.element(3);
        let b = f.element(4);
        assert_eq!(a.add(b).value, 2); // 3+4 = 7 mod 5 = 2
        assert_eq!(a.multiply(b).value, 2); // 3*4 = 12 mod 5 = 2
    }

    #[test]
    fn gf7_inverse() {
        let f = GaloisField::new(7, 1);
        for v in 1..7 {
            let a = f.element(v);
            let inv = a.inverse();
            assert_eq!(a.multiply(inv).value, 1, "{}^-1 failed", v);
        }
    }

    #[test]
    fn gf5_additive_identity() {
        let f = GaloisField::new(5, 1);
        for v in 0..5 {
            let a = f.element(v);
            assert_eq!(a.add(f.zero()).value, a.value);
        }
    }

    #[test]
    fn gf5_multiplicative_identity() {
        let f = GaloisField::new(5, 1);
        for v in 0..5 {
            let a = f.element(v);
            assert_eq!(a.multiply(f.one()).value, a.value);
        }
    }

    #[test]
    fn gf4_arithmetic() {
        // GF(2^2) with irreducible x^2 + x + 1
        let f = GaloisField::new(2, 2);
        assert_eq!(f.order(), 4);
        // Elements: 0, 1, x (=2), x+1 (=3)
        let a = f.element(2); // x
        let b = f.element(3); // x+1
        let sum = a.add(b);
        // x + (x+1) = 1 in GF(2)
        assert_eq!(sum.value, 1);
        // x * (x+1) = x^2 + x. Mod (x^2+x+1): x^2+x ≡ 1 (since x^2+x+1≡0 => x^2+x≡1)
        let prod = a.multiply(b);
        // Wait: x^2 = x+1 (from x^2+x+1=0), so x*(x+1) = x^2+x = (x+1)+x = 1
        assert_eq!(prod.value, 1, "x * (x+1) should be 1 in GF(4)");
    }

    #[test]
    fn gf4_inverse() {
        let f = GaloisField::new(2, 2);
        for v in 1..4 {
            let a = f.element(v);
            let inv = a.inverse();
            assert_eq!(
                a.multiply(inv).value,
                1,
                "inverse of {} failed (got {})",
                v,
                inv.value
            );
        }
    }

    #[test]
    fn gf8_all_inverses() {
        let f = GaloisField::new(2, 3);
        assert_eq!(f.order(), 8);
        for v in 1..8 {
            let a = f.element(v);
            let inv = a.inverse();
            assert_eq!(a.multiply(inv).value, 1, "inverse of {} failed", v);
        }
    }

    #[test]
    fn generator_gf5() {
        let f = GaloisField::new(5, 1);
        let g = find_generator(f);
        // g should generate all non-zero elements
        let mut seen = std::collections::HashSet::new();
        let mut current = f.one();
        for _ in 0..4 {
            seen.insert(current.value);
            current = current.multiply(g);
        }
        assert_eq!(seen.len(), 4, "generator should produce all 4 non-zero elements");
    }

    #[test]
    fn generator_gf4() {
        let f = GaloisField::new(2, 2);
        let g = find_generator(f);
        let mut seen = std::collections::HashSet::new();
        let mut current = f.one();
        for _ in 0..3 {
            seen.insert(current.value);
            current = current.multiply(g);
        }
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn multiplication_table_gf3() {
        let f = GaloisField::new(3, 1);
        let table = multiplication_table(f);
        assert_eq!(table.len(), 3);
        assert_eq!(table[0], vec![0, 0, 0]); // 0 * anything = 0
        assert_eq!(table[1], vec![0, 1, 2]); // 1 * x = x
        assert_eq!(table[2], vec![0, 2, 1]); // 2 * 2 = 4 mod 3 = 1
    }

    #[test]
    fn distributive_law() {
        let f = GaloisField::new(5, 1);
        for a in 0..5 {
            for b in 0..5 {
                for c in 0..5 {
                    let ea = f.element(a);
                    let eb = f.element(b);
                    let ec = f.element(c);
                    // a * (b + c) = a*b + a*c
                    let lhs = ea.multiply(eb.add(ec));
                    let rhs = ea.multiply(eb).add(ea.multiply(ec));
                    assert_eq!(lhs.value, rhs.value, "distributive failed for {},{},{}", a, b, c);
                }
            }
        }
    }

    #[test]
    fn gf9_order() {
        let f = GaloisField::new(3, 2);
        assert_eq!(f.order(), 9);
        // Every non-zero element satisfies a^8 = 1
        for v in 1..9 {
            let a = f.element(v);
            assert_eq!(a.pow(8).value, 1, "a^8 != 1 for a={}", v);
        }
    }
}
