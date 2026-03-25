//! Riemann zeta function, critical strip rendering, and zero detection.

use glam::{Vec2, Vec3, Vec4};
use std::f64::consts::PI;

// ─── Complex number type ────────────────────────────────────────────────────

/// Minimal complex number for zeta computations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };
    pub const ONE: Complex = Complex { re: 1.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    pub fn conj(self) -> Self {
        Self { re: self.re, im: -self.im }
    }

    pub fn exp(self) -> Self {
        let r = self.re.exp();
        Self {
            re: r * self.im.cos(),
            im: r * self.im.sin(),
        }
    }

    pub fn ln(self) -> Self {
        Self {
            re: self.norm().ln(),
            im: self.arg(),
        }
    }

    /// self^p via exp(p * ln(self))
    pub fn powc(self, p: Complex) -> Self {
        if self.norm_sq() == 0.0 {
            return Complex::ZERO;
        }
        (p * self.ln()).exp()
    }

    /// self^r for real exponent
    pub fn powr(self, r: f64) -> Self {
        self.powc(Complex::new(r, 0.0))
    }

    pub fn sin(self) -> Self {
        Self {
            re: self.re.sin() * self.im.cosh(),
            im: self.re.cos() * self.im.sinh(),
        }
    }

    pub fn cos(self) -> Self {
        Self {
            re: self.re.cos() * self.im.cosh(),
            im: -self.re.sin() * self.im.sinh(),
        }
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { re: self.re + rhs.re, im: self.im + rhs.im }
    }
}

impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { re: self.re - rhs.re, im: self.im - rhs.im }
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let d = rhs.norm_sq();
        Self {
            re: (self.re * rhs.re + self.im * rhs.im) / d,
            im: (self.im * rhs.re - self.re * rhs.im) / d,
        }
    }
}

impl std::ops::Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self { re: self.re * rhs, im: self.im * rhs }
    }
}

impl std::ops::Mul<Complex> for f64 {
    type Output = Complex;
    fn mul(self, rhs: Complex) -> Complex {
        Complex { re: self * rhs.re, im: self * rhs.im }
    }
}

impl std::ops::AddAssign for Complex {
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl std::ops::Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self {
        Self { re: -self.re, im: -self.im }
    }
}

// ─── Gamma function (Lanczos approximation) ─────────────────────────────────

fn gamma(z: Complex) -> Complex {
    // Reflection for Re(z) < 0.5
    if z.re < 0.5 {
        let pi_z = Complex::new(PI, 0.0) * z;
        return Complex::new(PI, 0.0) / (pi_z.sin() * gamma(Complex::new(1.0, 0.0) - z));
    }

    let z = z - Complex::ONE;
    let g = 7.0;
    let coefs: [f64; 9] = [
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

    let mut x = Complex::new(coefs[0], 0.0);
    for (i, &c) in coefs.iter().enumerate().skip(1) {
        x += Complex::new(c, 0.0) / (z + Complex::new(i as f64, 0.0));
    }

    let t = z + Complex::new(g + 0.5, 0.0);
    let sqrt_2pi = Complex::new((2.0 * PI).sqrt(), 0.0);
    sqrt_2pi * t.powc(z + Complex::new(0.5, 0.0)) * (-t).exp() * x
}

// ─── Zeta function ──────────────────────────────────────────────────────────

/// Riemann zeta function via the Borwein method (Dirichlet eta + analytic continuation).
///
/// Uses the globally convergent series based on Chebyshev-like coefficients
/// (Borwein 1995). Accurate to ~15 digits for Re(s) > -10.
pub fn zeta(s: Complex) -> Complex {
    // Pole at s = 1
    if (s.re - 1.0).abs() < 1e-14 && s.im.abs() < 1e-14 {
        return Complex::new(f64::INFINITY, 0.0);
    }

    // Functional equation for Re(s) < 0: zeta(s) = 2^s * pi^(s-1) * sin(pi*s/2) * Gamma(1-s) * zeta(1-s)
    if s.re < 0.0 {
        let one_minus_s = Complex::new(1.0, 0.0) - s;
        let two_s = Complex::new(2.0, 0.0).powc(s);
        let pi_s_minus_1 = Complex::new(PI, 0.0).powc(s - Complex::ONE);
        let sin_term = (Complex::new(PI, 0.0) * s * 0.5).sin();
        let gam = gamma(one_minus_s);
        let zeta_1ms = zeta(one_minus_s);
        return two_s * pi_s_minus_1 * sin_term * gam * zeta_1ms;
    }

    // Borwein method: use eta function with n terms
    let n = 30usize;
    // Precompute d_k coefficients
    let mut d = vec![0.0f64; n + 1];
    d[0] = 1.0;
    // d_k = sum_{j=0}^{k} n! / (j! * (n-j)!) -- actually d_k = n * sum ...
    // Simpler: use the partial sums of binomial coefficients
    {
        let mut binom = 1.0f64;
        for k in 0..=n {
            d[k] = binom;
            if k < n {
                binom *= (n - k) as f64 / (k + 1) as f64;
            }
        }
        // d_k = partial sums: d[k] = sum_{j=0}^{k} C(n, j)
        // Actually we want d_k = n * sum_{j=0..k} (-1)^j C(n,j) ... let's use the
        // standard globally convergent approach instead.
    }

    // Standard approach: zeta(s) = (1 / (1 - 2^{1-s})) * eta(s)
    // eta(s) = sum_{k=1}^{inf} (-1)^{k-1} / k^s
    // Accelerated via Euler-Maclaurin / Cohen-Villegas-Zagier acceleration

    // Cohen-Villegas-Zagier: pick N terms
    let nn = 40usize;
    let mut sum = Complex::ZERO;
    let mut c = Complex::ZERO; // Kahan compensation
    // d_k weights
    let mut dk = 0.0f64;
    let dn = {
        let mut v = 0.0f64;
        let mut binom = 1.0f64;
        for j in 0..=nn {
            v += binom;
            if j < nn {
                binom *= (nn - j) as f64 / (j + 1) as f64;
            }
        }
        v
    };

    let mut binom = 1.0f64;
    dk = 0.0;
    for k in 0..=nn {
        dk += binom;
        let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
        let term_weight = sign * (dk - dn);
        let k1 = Complex::new((k + 1) as f64, 0.0);
        let term = Complex::new(term_weight, 0.0) / k1.powc(s);
        // Kahan summation
        let y = term - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
        if k < nn {
            binom *= (nn - k) as f64 / (k + 1) as f64;
        }
    }

    let eta = sum * Complex::new(-1.0 / dn, 0.0);

    // zeta(s) = eta(s) / (1 - 2^{1-s})
    let two_1ms = Complex::new(2.0, 0.0).powc(Complex::new(1.0, 0.0) - s);
    let denom = Complex::ONE - two_1ms;

    if denom.norm_sq() < 1e-30 {
        // s is near a zero of the denominator; fall back to direct Dirichlet series
        let mut direct = Complex::ZERO;
        for k in 1..=200 {
            direct += Complex::new(1.0, 0.0) / Complex::new(k as f64, 0.0).powc(s);
        }
        return direct;
    }

    eta / denom
}

/// Zeta on the critical line: s = 0.5 + it.
pub fn zeta_on_critical_line(t: f64) -> Complex {
    zeta(Complex::new(0.5, t))
}

/// Hardy's Z-function: real-valued function whose zeros on the real line
/// correspond to zeros of zeta on the critical line.
/// Z(t) = e^{i*theta(t)} * zeta(1/2 + it)
/// where theta(t) is the Riemann-Siegel theta function.
pub fn z_function(t: f64) -> f64 {
    let theta = riemann_siegel_theta(t);
    let z = zeta_on_critical_line(t);
    // Z(t) = exp(i*theta) * zeta(1/2+it)  — the result should be real
    let rot = Complex::new(theta.cos(), theta.sin());
    let result = rot * z;
    result.re
}

/// Riemann-Siegel theta function.
fn riemann_siegel_theta(t: f64) -> f64 {
    // theta(t) = arg(Gamma(1/4 + it/2)) - t/2 * ln(pi)
    // Stirling approximation:
    // theta(t) ≈ t/2 * ln(t/(2*pi*e)) - pi/8 + 1/(48*t) + ...
    let term1 = (t / 2.0) * ((t / (2.0 * PI)).ln()) - t / 2.0 - PI / 8.0;
    let term2 = 1.0 / (48.0 * t);
    let term3 = 7.0 / (5760.0 * t * t * t);
    term1 + term2 + term3
}

/// Find approximate locations of non-trivial zeros of zeta on the critical line
/// in the interval [t_min, t_max] by detecting sign changes of Z(t).
pub fn find_zeros(t_min: f64, t_max: f64, resolution: usize) -> Vec<f64> {
    let mut zeros = Vec::new();
    let step = (t_max - t_min) / resolution as f64;
    let mut prev = z_function(t_min);
    let mut t = t_min + step;
    for _ in 1..=resolution {
        let current = z_function(t);
        if prev * current < 0.0 {
            // Bisect to refine
            let zero = bisect_zero(t - step, t, 50);
            zeros.push(zero);
        }
        prev = current;
        t += step;
    }
    zeros
}

fn bisect_zero(mut a: f64, mut b: f64, iterations: usize) -> f64 {
    for _ in 0..iterations {
        let mid = (a + b) / 2.0;
        let fa = z_function(a);
        let fm = z_function(mid);
        if fa * fm <= 0.0 {
            b = mid;
        } else {
            a = mid;
        }
    }
    (a + b) / 2.0
}

// ─── Renderers ──────────────────────────────────────────────────────────────

/// Renders the critical strip 0 < Re(s) < 1 as a colored grid.
pub struct CriticalStripRenderer {
    pub re_range: (f64, f64),
    pub im_range: (f64, f64),
    pub resolution: (usize, usize),
}

/// A single colored cell in the critical strip visualization.
pub struct StripCell {
    pub position: Vec2,
    pub color: Vec4,
    pub zeta_value: Complex,
}

impl CriticalStripRenderer {
    pub fn new(
        re_range: (f64, f64),
        im_range: (f64, f64),
        resolution: (usize, usize),
    ) -> Self {
        Self { re_range, im_range, resolution }
    }

    /// Default critical strip: Re in [0, 1], Im in [-30, 30].
    pub fn default_strip() -> Self {
        Self::new((0.0, 1.0), (-30.0, 30.0), (50, 300))
    }

    /// Generate the grid of colored cells.
    pub fn render(&self) -> Vec<StripCell> {
        let (re_min, re_max) = self.re_range;
        let (im_min, im_max) = self.im_range;
        let (nx, ny) = self.resolution;
        let re_step = (re_max - re_min) / nx as f64;
        let im_step = (im_max - im_min) / ny as f64;

        let mut cells = Vec::with_capacity(nx * ny);
        for i in 0..nx {
            for j in 0..ny {
                let re = re_min + (i as f64 + 0.5) * re_step;
                let im = im_min + (j as f64 + 0.5) * im_step;
                let s = Complex::new(re, im);
                let z = zeta(s);
                let mag = z.norm().ln().max(-5.0).min(5.0);
                let phase = z.arg();

                // Map magnitude to brightness, phase to hue
                let brightness = ((mag + 5.0) / 10.0) as f32;
                let hue = ((phase + PI) / (2.0 * PI)) as f32;
                let color = hsv_to_rgba(hue, 0.8, brightness);

                cells.push(StripCell {
                    position: Vec2::new(i as f32, j as f32),
                    color,
                    zeta_value: z,
                });
            }
        }
        cells
    }
}

/// Marks non-trivial zeros with special glyphs.
pub struct ZeroMarker {
    pub origin: Vec3,
    pub scale: f32,
}

/// A glyph at a zero location.
pub struct ZeroGlyph {
    pub t: f64,
    pub position: Vec3,
    pub character: char,
    pub color: Vec4,
}

impl ZeroMarker {
    pub fn new(origin: Vec3, scale: f32) -> Self {
        Self { origin, scale }
    }

    /// Find zeros and produce glyphs.
    pub fn mark_zeros(&self, t_min: f64, t_max: f64, resolution: usize) -> Vec<ZeroGlyph> {
        let zeros = find_zeros(t_min, t_max, resolution);
        zeros
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let y = (t - t_min) / (t_max - t_min);
                ZeroGlyph {
                    t,
                    position: self.origin
                        + Vec3::new(0.5 * self.scale, y as f32 * self.scale, 0.0),
                    character: '\u{2742}', // ❂
                    color: Vec4::new(1.0, 0.2, 0.2, 1.0),
                }
            })
            .collect()
    }
}

fn hsv_to_rgba(h: f32, s: f32, v: f32) -> Vec4 {
    let h = h * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Vec4::new(r + m, g + m, b + m, 1.0)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn complex_arithmetic() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, -1.0);
        let sum = a + b;
        assert!(approx(sum.re, 4.0, 1e-12));
        assert!(approx(sum.im, 1.0, 1e-12));
        let prod = a * b;
        // (1+2i)(3-i) = 3 -i +6i -2i^2 = 5 + 5i
        assert!(approx(prod.re, 5.0, 1e-12));
        assert!(approx(prod.im, 5.0, 1e-12));
    }

    #[test]
    fn complex_exp_ln() {
        let z = Complex::new(1.0, PI);
        let e = z.exp();
        // e^(1+i*pi) = e * (cos(pi) + i*sin(pi)) = -e + 0i
        assert!(approx(e.re, -std::f64::consts::E, 1e-10));
        assert!(approx(e.im, 0.0, 1e-10));
    }

    #[test]
    fn zeta_at_2() {
        // zeta(2) = pi^2/6 ≈ 1.6449340668
        let z = zeta(Complex::new(2.0, 0.0));
        assert!(approx(z.re, PI * PI / 6.0, 1e-6));
        assert!(approx(z.im, 0.0, 1e-6));
    }

    #[test]
    fn zeta_at_minus_1() {
        // zeta(-1) = -1/12
        let z = zeta(Complex::new(-1.0, 0.0));
        assert!(approx(z.re, -1.0 / 12.0, 1e-4));
        assert!(approx(z.im, 0.0, 1e-4));
    }

    #[test]
    fn zeta_at_4() {
        // zeta(4) = pi^4/90
        let z = zeta(Complex::new(4.0, 0.0));
        assert!(approx(z.re, PI.powi(4) / 90.0, 1e-6));
        assert!(z.im.abs() < 1e-6);
    }

    #[test]
    fn first_zero_approx() {
        // First non-trivial zero at t ≈ 14.134725
        let zeros = find_zeros(13.0, 16.0, 2000);
        assert!(!zeros.is_empty(), "should find at least one zero");
        assert!(
            approx(zeros[0], 14.134725, 0.01),
            "first zero should be near 14.1347, got {}",
            zeros[0]
        );
    }

    #[test]
    fn z_function_sign_change() {
        // Z(t) should change sign near t ≈ 14.134
        let a = z_function(14.0);
        let b = z_function(14.5);
        assert!(
            a * b < 0.0,
            "Z should change sign between 14 and 14.5: Z(14)={}, Z(14.5)={}",
            a,
            b
        );
    }

    #[test]
    fn critical_strip_renderer() {
        let r = CriticalStripRenderer::new((0.0, 1.0), (-5.0, 5.0), (5, 10));
        let cells = r.render();
        assert_eq!(cells.len(), 50);
    }

    #[test]
    fn zero_marker_produces_glyphs() {
        let marker = ZeroMarker::new(Vec3::ZERO, 10.0);
        let glyphs = marker.mark_zeros(13.0, 16.0, 2000);
        assert!(!glyphs.is_empty());
    }
}
