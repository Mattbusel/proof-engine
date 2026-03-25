//! p-adic numbers: representations, norms, arithmetic, and fractal rendering.

use glam::{Vec2, Vec3, Vec4};

/// A p-adic number represented by its prime, digit expansion, and valuation.
/// The value is: sum_{i=0}^{n-1} digits[i] * p^(valuation + i).
#[derive(Debug, Clone, PartialEq)]
pub struct PAdic {
    pub prime: u64,
    pub digits: Vec<u64>,
    pub valuation: i64,
}

impl PAdic {
    /// Create a p-adic representation of an integer with given precision (number of digits).
    pub fn from_integer(n: i64, prime: u64, precision: usize) -> Self {
        assert!(prime >= 2, "prime must be >= 2");
        if n == 0 {
            return PAdic {
                prime,
                digits: vec![0; precision],
                valuation: 0,
            };
        }

        let p = prime as i64;
        // For negative numbers, we use p-adic representation (complement)
        // -1 in p-adic = (p-1)(p-1)(p-1)...
        let mut digits = Vec::with_capacity(precision);
        let mut val = n;

        if val > 0 {
            // Find valuation
            let mut v = 0i64;
            while val % p == 0 && val != 0 {
                val /= p;
                v += 1;
            }
            for _ in 0..precision {
                digits.push((val.rem_euclid(p)) as u64);
                val = val.div_euclid(p);
            }
            PAdic { prime, digits, valuation: v }
        } else {
            // Negative: compute as p^precision - |n| in base p
            // This gives the p-adic complement
            let mut v = 0i64;
            let mut abs_val = -val;
            while abs_val % p == 0 && abs_val != 0 {
                abs_val /= p;
                v += 1;
            }
            // Represent -abs_val in p-adic: use the fact that
            // -x = (p^N - x) in p-adic for sufficiently large N
            let mut complement = Vec::with_capacity(precision);
            let mut carry = 0i64;
            let mut first = true;
            let mut remaining = abs_val;
            for _ in 0..precision {
                let d = remaining.rem_euclid(p);
                remaining = remaining.div_euclid(p);
                let comp_digit = if first && d == 0 {
                    first = false;
                    0
                } else if first {
                    first = false;
                    (p - d) as u64
                } else {
                    (p - 1 - d - carry) as u64
                };
                // Actually, simpler: treat as unsigned and negate
                complement.push(0);
            }
            // Simpler approach: use modular arithmetic
            // -n mod p^k for large k
            digits.clear();
            let mut rem = n;
            for _ in 0..precision {
                let d = rem.rem_euclid(p) as u64;
                digits.push(d);
                rem = rem.div_euclid(p);
            }
            PAdic { prime, digits, valuation: v }
        }
    }

    /// Display the p-adic expansion as a string: ...d_2 d_1 d_0 . d_{-1} ...
    pub fn to_string_expansion(&self) -> String {
        let mut s = String::from("...");
        for d in self.digits.iter().rev() {
            s.push_str(&d.to_string());
        }
        s
    }

    /// Evaluate to f64 (truncated, only meaningful for analysis).
    pub fn to_f64_approx(&self) -> f64 {
        let p = self.prime as f64;
        let mut result = 0.0f64;
        for (i, &d) in self.digits.iter().enumerate() {
            result += d as f64 * p.powi((self.valuation + i as i64) as i32);
        }
        result
    }

    /// Number of digits in the expansion.
    pub fn precision(&self) -> usize {
        self.digits.len()
    }
}

/// p-adic valuation of n with respect to prime p.
pub fn padic_valuation(mut n: i64, p: u64) -> i64 {
    if n == 0 {
        return i64::MAX; // conventionally infinite
    }
    let p = p as i64;
    n = n.abs();
    let mut v = 0i64;
    while n % p == 0 {
        n /= p;
        v += 1;
    }
    v
}

/// p-adic norm |n|_p = p^{-v_p(n)}.
pub fn padic_norm(n: i64, p: u64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let v = padic_valuation(n, p);
    (p as f64).powi(-v as i32)
}

/// p-adic distance |a - b|_p.
pub fn padic_distance(a: i64, b: i64, p: u64) -> f64 {
    padic_norm(a - b, p)
}

/// Add two p-adic numbers (must have same prime and precision is min of both).
pub fn add(a: &PAdic, b: &PAdic) -> PAdic {
    assert_eq!(a.prime, b.prime, "cannot add p-adics with different primes");
    let p = a.prime;
    let prec = a.digits.len().min(b.digits.len());

    // Align valuations
    let min_val = a.valuation.min(b.valuation);
    let a_shift = (a.valuation - min_val) as usize;
    let b_shift = (b.valuation - min_val) as usize;

    let total_len = prec + a_shift.max(b_shift);
    let mut digits = vec![0u64; total_len];
    let mut carry = 0u64;

    for i in 0..total_len {
        let da = if i >= a_shift && i - a_shift < a.digits.len() {
            a.digits[i - a_shift]
        } else {
            0
        };
        let db = if i >= b_shift && i - b_shift < b.digits.len() {
            b.digits[i - b_shift]
        } else {
            0
        };
        let sum = da + db + carry;
        digits[i] = sum % p;
        carry = sum / p;
    }

    // Trim to precision
    digits.truncate(prec);

    // Find valuation (leading zeros)
    let mut val = min_val;
    while !digits.is_empty() && digits[0] == 0 {
        digits.remove(0);
        val += 1;
    }
    if digits.is_empty() {
        digits.push(0);
        val = 0;
    }

    PAdic { prime: p, digits, valuation: val }
}

/// Multiply two p-adic numbers.
pub fn multiply(a: &PAdic, b: &PAdic) -> PAdic {
    assert_eq!(a.prime, b.prime);
    let p = a.prime;
    let prec = a.digits.len().min(b.digits.len());
    let val = a.valuation + b.valuation;

    let mut digits = vec![0u64; prec];
    for i in 0..a.digits.len().min(prec) {
        let mut carry = 0u64;
        for j in 0..b.digits.len().min(prec) {
            if i + j >= prec {
                break;
            }
            let prod = a.digits[i] * b.digits[j] + digits[i + j] + carry;
            digits[i + j] = prod % p;
            carry = prod / p;
        }
    }

    PAdic { prime: p, digits, valuation: val }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

/// Renders p-adic expansions as a tree / fractal-like pattern.
/// Each level of the tree corresponds to a digit position,
/// branching into p children (one per possible digit).
pub struct PadicRenderer {
    pub prime: u64,
    pub depth: usize,
    pub origin: Vec3,
    pub scale: f32,
}

pub struct PadicNode {
    pub digit: u64,
    pub level: usize,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl PadicRenderer {
    pub fn new(prime: u64, depth: usize, origin: Vec3, scale: f32) -> Self {
        Self { prime, depth, origin, scale }
    }

    /// Generate a tree of all p-adic digit sequences up to `depth` levels.
    pub fn render_tree(&self) -> Vec<PadicNode> {
        let mut nodes = Vec::new();
        self.build_tree(&mut nodes, 0, 0, 0.0, self.scale);
        nodes
    }

    fn build_tree(
        &self,
        nodes: &mut Vec<PadicNode>,
        level: usize,
        _path_value: u64,
        x_center: f32,
        width: f32,
    ) {
        if level >= self.depth {
            return;
        }
        let p = self.prime;
        let child_width = width / p as f32;
        for d in 0..p {
            let x = x_center + (d as f32 - (p - 1) as f32 / 2.0) * child_width;
            let y = -(level as f32) * self.scale * 0.5;
            let hue = d as f32 / p as f32;
            nodes.push(PadicNode {
                digit: d,
                level,
                position: self.origin + Vec3::new(x, y, 0.0),
                color: Vec4::new(hue, 1.0 - hue, 0.5, 1.0),
                character: std::char::from_digit(d as u32, 36).unwrap_or('?'),
            });
            self.build_tree(
                nodes,
                level + 1,
                _path_value * p + d,
                x,
                child_width,
            );
        }
    }

    /// Highlight the path corresponding to a specific integer in the tree.
    pub fn highlight_path(&self, n: i64) -> Vec<(usize, u64)> {
        let p = self.prime as i64;
        let mut path = Vec::new();
        let mut val = n;
        for level in 0..self.depth {
            let d = val.rem_euclid(p) as u64;
            path.push((level, d));
            val = val.div_euclid(p);
        }
        path
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_integer_basic() {
        let pa = PAdic::from_integer(25, 5, 5);
        assert_eq!(pa.prime, 5);
        assert_eq!(pa.valuation, 2);
        // 25 = 5^2, so digits should be [1, 0, 0, ...]
        assert_eq!(pa.digits[0], 1);
    }

    #[test]
    fn from_integer_zero() {
        let pa = PAdic::from_integer(0, 3, 5);
        assert_eq!(pa.digits, vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn padic_norm_values() {
        // |25|_5 = 5^{-2} = 0.04
        assert!((padic_norm(25, 5) - 0.04).abs() < 1e-12);
        // |12|_3 = 3^{-1} = 1/3
        assert!((padic_norm(12, 3) - 1.0 / 3.0).abs() < 1e-12);
        // |7|_5 = 1 (7 not divisible by 5)
        assert!((padic_norm(7, 5) - 1.0).abs() < 1e-12);
        // |0|_p = 0
        assert_eq!(padic_norm(0, 7), 0.0);
    }

    #[test]
    fn padic_distance_values() {
        // |1 - 26|_5 = |25|_5 = 1/25
        assert!((padic_distance(1, 26, 5) - 0.04).abs() < 1e-12);
    }

    #[test]
    fn padic_ultrametric() {
        // |a - c|_p <= max(|a-b|_p, |b-c|_p)  (strong triangle inequality)
        let a = 3;
        let b = 8;
        let c = 18;
        let p = 5u64;
        let d_ac = padic_distance(a, c, p);
        let d_ab = padic_distance(a, b, p);
        let d_bc = padic_distance(b, c, p);
        assert!(d_ac <= d_ab.max(d_bc) + 1e-12);
    }

    #[test]
    fn add_padics() {
        // In 5-adics: 3 + 7 = 10
        let a = PAdic::from_integer(3, 5, 5);
        let b = PAdic::from_integer(7, 5, 5);
        let c = add(&a, &b);
        let val = c.to_f64_approx();
        assert!((val - 10.0).abs() < 1e-6, "expected 10, got {}", val);
    }

    #[test]
    fn multiply_padics() {
        // In 5-adics: 3 * 4 = 12
        let a = PAdic::from_integer(3, 5, 5);
        let b = PAdic::from_integer(4, 5, 5);
        let c = multiply(&a, &b);
        let val = c.to_f64_approx();
        assert!((val - 12.0).abs() < 1e-6, "expected 12, got {}", val);
    }

    #[test]
    fn negative_integer() {
        // -1 in 2-adics should be ...1111
        let pa = PAdic::from_integer(-1, 2, 8);
        for &d in &pa.digits {
            assert_eq!(d, 1, "all digits of -1 in 2-adic should be 1");
        }
    }

    #[test]
    fn renderer_tree_size() {
        let r = PadicRenderer::new(3, 3, Vec3::ZERO, 10.0);
        let nodes = r.render_tree();
        // 3 + 9 + 27 = 39
        assert_eq!(nodes.len(), 3 + 9 + 27);
    }

    #[test]
    fn highlight_path() {
        let r = PadicRenderer::new(5, 4, Vec3::ZERO, 1.0);
        let path = r.highlight_path(37);
        // 37 in base 5: 37 = 1*25 + 2*5 + 2 => digits [2, 2, 1, 0]
        assert_eq!(path[0], (0, 2));
        assert_eq!(path[1], (1, 2));
        assert_eq!(path[2], (2, 1));
        assert_eq!(path[3], (3, 0));
    }
}
