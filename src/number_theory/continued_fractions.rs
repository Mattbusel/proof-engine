//! Continued fractions: representation, convergents, and visualization.

use glam::{Vec2, Vec3, Vec4};

/// A continued fraction [a0; a1, a2, ...] representing a0 + 1/(a1 + 1/(a2 + ...)).
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuedFraction {
    pub integer_part: i64,
    pub coefficients: Vec<u64>,
}

impl ContinuedFraction {
    /// Create from explicit parts.
    pub fn new(integer_part: i64, coefficients: Vec<u64>) -> Self {
        Self { integer_part, coefficients }
    }

    /// Build a continued fraction from a rational number p/q.
    pub fn from_rational(mut num: i64, mut den: i64) -> Self {
        if den == 0 {
            return Self { integer_part: 0, coefficients: vec![] };
        }
        if den < 0 {
            num = -num;
            den = -den;
        }

        // Handle negative: use floor division
        let integer_part = num.div_euclid(den);
        let mut remainder = num.rem_euclid(den);
        let mut coefficients = Vec::new();
        let mut d = den;

        while remainder != 0 {
            let new_d = remainder;
            let q = d / remainder;
            remainder = d % remainder;
            d = new_d;
            coefficients.push(q as u64);
        }

        Self { integer_part, coefficients }
    }

    /// Build a continued fraction from a floating point number, computing up to max_terms.
    pub fn from_f64(x: f64, max_terms: usize) -> Self {
        let integer_part = x.floor() as i64;
        let mut frac = x - integer_part as f64;
        let mut coefficients = Vec::new();

        for _ in 0..max_terms {
            if frac.abs() < 1e-12 {
                break;
            }
            let recip = 1.0 / frac;
            let a = recip.floor() as u64;
            if a == 0 {
                break;
            }
            coefficients.push(a);
            frac = recip - a as f64;
        }

        Self { integer_part, coefficients }
    }

    /// Compute convergents (rational approximations) as (numerator, denominator) pairs.
    /// Returns all convergents from [a0] to [a0; a1, ..., an].
    pub fn convergents(&self) -> Vec<(i64, i64)> {
        let mut result = Vec::new();
        let mut h_prev: i64 = 1;
        let mut k_prev: i64 = 0;
        let mut h_curr: i64 = self.integer_part;
        let mut k_curr: i64 = 1;
        result.push((h_curr, k_curr));

        for &a in &self.coefficients {
            let a = a as i64;
            let h_next = a * h_curr + h_prev;
            let k_next = a * k_curr + k_prev;
            h_prev = h_curr;
            k_prev = k_curr;
            h_curr = h_next;
            k_curr = k_next;
            result.push((h_curr, k_curr));
        }

        result
    }

    /// Evaluate the continued fraction as an f64.
    pub fn to_f64(&self) -> f64 {
        let mut value = 0.0f64;
        for &a in self.coefficients.iter().rev() {
            value = 1.0 / (a as f64 + value);
        }
        self.integer_part as f64 + value
    }

    /// Number of terms (not counting integer part).
    pub fn len(&self) -> usize {
        self.coefficients.len()
    }

    pub fn is_empty(&self) -> bool {
        self.coefficients.is_empty()
    }
}

/// Compute the periodic continued fraction for sqrt(n), where n is not a perfect square.
/// Returns a ContinuedFraction where coefficients form the repeating period.
pub fn sqrt_cf(n: u64) -> ContinuedFraction {
    let sqrt_n = (n as f64).sqrt();
    let a0 = sqrt_n.floor() as u64;
    if a0 * a0 == n {
        // Perfect square
        return ContinuedFraction {
            integer_part: a0 as i64,
            coefficients: vec![],
        };
    }

    let mut coefficients = Vec::new();
    let mut m: i64 = 0;
    let mut d: i64 = 1;
    let mut a = a0 as i64;

    loop {
        m = d * a - m;
        d = (n as i64 - m * m) / d;
        if d == 0 {
            break;
        }
        a = (a0 as i64 + m) / d;
        coefficients.push(a as u64);
        // Period ends when a == 2 * a0
        if a == 2 * a0 as i64 {
            break;
        }
    }

    ContinuedFraction {
        integer_part: a0 as i64,
        coefficients,
    }
}

/// Render convergents as points on a number line converging to a target value.
pub struct ConvergentVisualizer {
    pub origin: Vec3,
    pub scale: f32,
    pub target: f64,
}

pub struct ConvergentGlyph {
    pub index: usize,
    pub value: f64,
    pub numerator: i64,
    pub denominator: i64,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl ConvergentVisualizer {
    pub fn new(origin: Vec3, scale: f32, target: f64) -> Self {
        Self { origin, scale, target }
    }

    /// Render the convergents of a continued fraction on a number line.
    pub fn render(&self, cf: &ContinuedFraction) -> Vec<ConvergentGlyph> {
        let convergents = cf.convergents();
        convergents
            .iter()
            .enumerate()
            .map(|(i, &(h, k))| {
                let value = h as f64 / k as f64;
                let error = (value - self.target).abs();
                let x_offset = (value - self.target) as f32 * self.scale;
                let y_offset = i as f32 * 0.5; // stack vertically
                let err_color = (1.0 - (error as f32 * 10.0).min(1.0)).max(0.0);
                ConvergentGlyph {
                    index: i,
                    value,
                    numerator: h,
                    denominator: k,
                    position: self.origin + Vec3::new(x_offset, y_offset, 0.0),
                    color: Vec4::new(1.0 - err_color, err_color, 0.3, 1.0),
                    character: if i % 2 == 0 { '+' } else { '-' },
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
    fn rational_cf() {
        // 355/113 = [3; 7, 16, 11] -- wait, 355/113 = [3; 7, 16]
        // Actually: 355 = 3*113 + 16, 113 = 7*16 + 1, 16 = 16*1
        // So [3; 7, 16]
        let cf = ContinuedFraction::from_rational(355, 113);
        assert_eq!(cf.integer_part, 3);
        assert_eq!(cf.coefficients, vec![7, 16]);
    }

    #[test]
    fn rational_roundtrip() {
        let cf = ContinuedFraction::from_rational(22, 7);
        assert_eq!(cf.integer_part, 3);
        assert_eq!(cf.coefficients, vec![7]);
        let val = cf.to_f64();
        assert!((val - 22.0 / 7.0).abs() < 1e-12);
    }

    #[test]
    fn pi_cf() {
        let cf = ContinuedFraction::from_f64(std::f64::consts::PI, 10);
        assert_eq!(cf.integer_part, 3);
        // pi = [3; 7, 15, 1, 292, ...]
        assert_eq!(cf.coefficients[0], 7);
        assert_eq!(cf.coefficients[1], 15);
        assert_eq!(cf.coefficients[2], 1);
        assert_eq!(cf.coefficients[3], 292);
    }

    #[test]
    fn convergents_of_pi() {
        let cf = ContinuedFraction::from_f64(std::f64::consts::PI, 5);
        let conv = cf.convergents();
        // First convergent: 3/1
        assert_eq!(conv[0], (3, 1));
        // Second: 22/7
        assert_eq!(conv[1], (22, 7));
        // Third: 333/106
        assert_eq!(conv[2], (333, 106));
        // Fourth: 355/113
        assert_eq!(conv[3], (355, 113));
    }

    #[test]
    fn to_f64_accuracy() {
        let cf = ContinuedFraction::from_f64(std::f64::consts::E, 15);
        let val = cf.to_f64();
        assert!((val - std::f64::consts::E).abs() < 1e-10);
    }

    #[test]
    fn sqrt_2_cf() {
        let cf = sqrt_cf(2);
        assert_eq!(cf.integer_part, 1);
        // sqrt(2) = [1; 2, 2, 2, ...] period = [2]
        assert_eq!(cf.coefficients[0], 2);
        // Last element should be 2*a0 = 2
        assert_eq!(*cf.coefficients.last().unwrap(), 2);
    }

    #[test]
    fn sqrt_3_cf() {
        let cf = sqrt_cf(3);
        assert_eq!(cf.integer_part, 1);
        // sqrt(3) = [1; 1, 2] period = [1, 2]
        assert_eq!(cf.coefficients, vec![1, 2]);
    }

    #[test]
    fn sqrt_7_cf() {
        let cf = sqrt_cf(7);
        assert_eq!(cf.integer_part, 2);
        // sqrt(7) = [2; 1, 1, 1, 4]
        assert_eq!(cf.coefficients, vec![1, 1, 1, 4]);
    }

    #[test]
    fn perfect_square_cf() {
        let cf = sqrt_cf(9);
        assert_eq!(cf.integer_part, 3);
        assert!(cf.coefficients.is_empty());
    }

    #[test]
    fn convergent_visualizer() {
        let cf = ContinuedFraction::from_f64(std::f64::consts::PI, 5);
        let vis = ConvergentVisualizer::new(Vec3::ZERO, 100.0, std::f64::consts::PI);
        let glyphs = vis.render(&cf);
        assert!(!glyphs.is_empty());
        // Later convergents should be closer to the target
        let last = glyphs.last().unwrap();
        assert!((last.value - std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn negative_rational() {
        let cf = ContinuedFraction::from_rational(-7, 3);
        // -7/3 = -3 + 2/3, so [floor(-7/3)=-3; ...] = [-3; 3] since 1/(3/2) needs...
        // -7 = -3*3 + 2, so integer_part = -3, remainder = 2
        // 3 = 1*2 + 1, 2 = 2*1 => coefficients = [1, 2]
        assert_eq!(cf.integer_part, -3);
        let val = cf.to_f64();
        assert!((val - (-7.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn from_rational_zero() {
        let cf = ContinuedFraction::from_rational(0, 1);
        assert_eq!(cf.integer_part, 0);
        assert!(cf.coefficients.is_empty());
    }
}
