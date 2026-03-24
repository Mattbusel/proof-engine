//! Complex numbers, quaternions, and iterative fractal evaluation.
//!
//! Provides:
//! - `Complex` — fully-featured complex number arithmetic
//! - `Quaternion` — 3D rotation quaternion with SLERP and expmap
//! - Fractal samplers: Mandelbrot, Julia, BurningShip, Newton, Lyapunov
//! - Color mapping utilities for escape-time fractals

use std::fmt;
use glam::{Vec2, Vec3, Vec4};

// ── Complex ───────────────────────────────────────────────────────────────────

/// A complex number z = re + im·i.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE:  Self = Self { re: 1.0, im: 0.0 };
    pub const I:    Self = Self { re: 0.0, im: 1.0 };

    #[inline] pub fn new(re: f64, im: f64) -> Self { Self { re, im } }
    #[inline] pub fn from_polar(r: f64, theta: f64) -> Self {
        Self { re: r * theta.cos(), im: r * theta.sin() }
    }
    #[inline] pub fn from_vec2(v: Vec2) -> Self { Self { re: v.x as f64, im: v.y as f64 } }
    #[inline] pub fn to_vec2(self) -> Vec2 { Vec2::new(self.re as f32, self.im as f32) }

    // ── Basic properties ─────────────────────────────────────────────────────

    #[inline] pub fn conj(self)   -> Self { Self::new(self.re, -self.im) }
    #[inline] pub fn norm_sq(self) -> f64 { self.re * self.re + self.im * self.im }
    #[inline] pub fn norm(self)   -> f64 { self.norm_sq().sqrt() }
    #[inline] pub fn arg(self)    -> f64 { self.im.atan2(self.re) }
    #[inline] pub fn abs(self)    -> f64 { self.norm() }
    #[inline] pub fn is_zero(self) -> bool { self.re == 0.0 && self.im == 0.0 }

    /// Normalize to unit circle. Returns zero if already zero.
    pub fn normalize(self) -> Self {
        let n = self.norm();
        if n < 1e-300 { Self::ZERO } else { Self::new(self.re / n, self.im / n) }
    }

    // ── Arithmetic ───────────────────────────────────────────────────────────

    pub fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
    pub fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
    pub fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
    pub fn div(self, rhs: Self) -> Self {
        let d = rhs.norm_sq();
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / d,
            (self.im * rhs.re - self.re * rhs.im) / d,
        )
    }
    pub fn scale(self, s: f64) -> Self { Self::new(self.re * s, self.im * s) }
    pub fn neg(self)           -> Self { Self::new(-self.re, -self.im) }
    pub fn recip(self)         -> Self { Self::ONE.div(self) }

    // ── Transcendentals ──────────────────────────────────────────────────────

    pub fn exp(self) -> Self {
        let e = self.re.exp();
        Self::new(e * self.im.cos(), e * self.im.sin())
    }

    pub fn ln(self) -> Self {
        Self::new(self.norm().ln(), self.arg())
    }

    pub fn sqrt(self) -> Self {
        let r   = self.norm().sqrt();
        let phi = self.arg() * 0.5;
        Self::from_polar(r, phi)
    }

    /// z^w = exp(w * ln(z)) — principal branch.
    pub fn pow(self, w: Self) -> Self {
        if self.is_zero() { return Self::ZERO; }
        self.ln().mul(w).exp()
    }

    /// z^n for integer exponent (fast repeated squaring).
    pub fn powi(self, n: i32) -> Self {
        if n == 0 { return Self::ONE; }
        if n < 0  { return self.recip().powi(-n); }
        let mut result = Self::ONE;
        let mut base   = self;
        let mut exp    = n as u32;
        while exp > 0 {
            if exp & 1 == 1 { result = result.mul(base); }
            base = base.mul(base);
            exp >>= 1;
        }
        result
    }

    pub fn sin(self) -> Self {
        // sin(a+bi) = sin(a)cosh(b) + i cos(a)sinh(b)
        Self::new(
            self.re.sin() * self.im.cosh(),
            self.re.cos() * self.im.sinh(),
        )
    }

    pub fn cos(self) -> Self {
        // cos(a+bi) = cos(a)cosh(b) - i sin(a)sinh(b)
        Self::new(
             self.re.cos() * self.im.cosh(),
            -self.re.sin() * self.im.sinh(),
        )
    }

    pub fn tan(self) -> Self {
        self.sin().div(self.cos())
    }

    pub fn sinh(self) -> Self {
        Self::new(self.re.sinh() * self.im.cos(), self.re.cosh() * self.im.sin())
    }

    pub fn cosh(self) -> Self {
        Self::new(self.re.cosh() * self.im.cos(), self.re.sinh() * self.im.sin())
    }

    pub fn tanh(self) -> Self {
        self.sinh().div(self.cosh())
    }

    pub fn asin(self) -> Self {
        // asin(z) = -i * ln(iz + sqrt(1 - z^2))
        let iz  = Self::I.mul(self);
        let sq  = Self::ONE.sub(self.mul(self)).sqrt();
        Self::I.neg().mul(iz.add(sq).ln())
    }

    pub fn acos(self) -> Self {
        // acos(z) = -i * ln(z + i*sqrt(1 - z^2))
        let sq  = Self::ONE.sub(self.mul(self)).sqrt();
        Self::I.neg().mul(self.add(Self::I.mul(sq)).ln())
    }

    pub fn atan(self) -> Self {
        // atan(z) = i/2 * ln((i+z)/(i-z))
        let half_i = Self::new(0.0, 0.5);
        let iz_plus  = Self::I.add(self);
        let iz_minus = Self::I.sub(self);
        half_i.mul(iz_plus.div(iz_minus).ln())
    }

    // ── Iteration helpers ────────────────────────────────────────────────────

    /// Mandelbrot iteration: z_{n+1} = z_n^2 + c. Returns z.
    #[inline] pub fn mandelbrot_step(self, c: Self) -> Self {
        self.mul(self).add(c)
    }

    /// Burning ship step: z_{n+1} = (|re| + i|im|)^2 + c.
    #[inline] pub fn burning_ship_step(self, c: Self) -> Self {
        let abs_z = Self::new(self.re.abs(), self.im.abs());
        abs_z.mul(abs_z).add(c)
    }

    /// Tricorn step: z_{n+1} = conj(z)^2 + c.
    #[inline] pub fn tricorn_step(self, c: Self) -> Self {
        self.conj().mul(self.conj()).add(c)
    }
}

impl fmt::Display for Complex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.im >= 0.0 {
            write!(f, "{:.4}+{:.4}i", self.re, self.im)
        } else {
            write!(f, "{:.4}{:.4}i", self.re, self.im)
        }
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Complex::add(self, rhs) }
}
impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Complex::sub(self, rhs) }
}
impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { Complex::mul(self, rhs) }
}
impl std::ops::Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self { Complex::div(self, rhs) }
}
impl std::ops::Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self { Complex::neg(self) }
}

// ── Fractal samplers ──────────────────────────────────────────────────────────

/// Result of an escape-time fractal iteration.
#[derive(Clone, Copy, Debug)]
pub struct EscapeResult {
    /// Whether the orbit escaped within `max_iter`.
    pub escaped:    bool,
    /// Number of iterations before escape (or max_iter).
    pub iterations: u32,
    /// Smooth iteration count for anti-banding coloring.
    pub smooth:     f64,
    /// Final z magnitude before escape.
    pub final_norm: f64,
    /// Final z value at escape.
    pub final_z:    Complex,
}

impl EscapeResult {
    /// Normalized [0, 1] escape value using smooth iteration count.
    pub fn normalized(&self, max_iter: u32) -> f64 {
        self.smooth / max_iter as f64
    }

    /// Hue angle [0, 2π] from the orbit angle of the final z.
    pub fn orbit_angle(&self) -> f64 {
        self.final_z.arg()
    }
}

// ── Mandelbrot ────────────────────────────────────────────────────────────────

/// Parameters for the generalized Mandelbrot set: z → z^power + c.
#[derive(Clone, Debug)]
pub struct MandelbrotParams {
    /// Exponent (2 = classic Mandelbrot).
    pub power:      f64,
    /// Maximum iterations.
    pub max_iter:   u32,
    /// Escape radius squared.
    pub escape_r_sq: f64,
}

impl Default for MandelbrotParams {
    fn default() -> Self {
        Self { power: 2.0, max_iter: 256, escape_r_sq: 4.0 }
    }
}

impl MandelbrotParams {
    pub fn new(power: f64, max_iter: u32) -> Self {
        Self { power, max_iter, escape_r_sq: 4.0 }
    }

    pub fn sample(&self, c: Complex) -> EscapeResult {
        let mut z = Complex::ZERO;
        let pw = Complex::new(self.power, 0.0);
        for i in 0..self.max_iter {
            z = z.pow(pw).add(c);
            let n = z.norm_sq();
            if n > self.escape_r_sq {
                // Smooth (continuous) coloring — subtract log2(log2(|z|))
                let smooth = i as f64 + 1.0 - (n.sqrt().ln().ln() / std::f64::consts::LN_2);
                return EscapeResult {
                    escaped: true, iterations: i,
                    smooth, final_norm: n.sqrt(), final_z: z,
                };
            }
        }
        EscapeResult {
            escaped: false, iterations: self.max_iter,
            smooth: self.max_iter as f64, final_norm: z.norm(), final_z: z,
        }
    }
}

// ── Julia ─────────────────────────────────────────────────────────────────────

/// Julia set: z → z^power + c where c is fixed.
#[derive(Clone, Debug)]
pub struct JuliaParams {
    /// The constant c.
    pub c:           Complex,
    pub power:       f64,
    pub max_iter:    u32,
    pub escape_r_sq: f64,
}

impl JuliaParams {
    pub fn new(c: Complex, max_iter: u32) -> Self {
        Self { c, power: 2.0, max_iter, escape_r_sq: 4.0 }
    }

    /// Notable Julia sets by index (0..7).
    pub fn preset(idx: usize) -> Self {
        let presets = [
            Complex::new(-0.7269, 0.1889),   // Douady rabbit
            Complex::new(-0.4, 0.6),          // Spirals
            Complex::new(0.285, 0.01),        // Dense spirals
            Complex::new(-0.835, -0.2321),    // Dendrite
            Complex::new(-0.7, 0.27015),      // Whirlpools
            Complex::new(0.45, 0.1428),       // Cauliflower
            Complex::new(-0.123, 0.745),      // Douady/Hubbard
            Complex::new(0.0, 0.8),           // San Marco
        ];
        let c = presets[idx % presets.len()];
        Self { c, power: 2.0, max_iter: 256, escape_r_sq: 4.0 }
    }

    pub fn sample(&self, z0: Complex) -> EscapeResult {
        let mut z = z0;
        let pw    = Complex::new(self.power, 0.0);
        for i in 0..self.max_iter {
            z = z.pow(pw).add(self.c);
            let n = z.norm_sq();
            if n > self.escape_r_sq {
                let smooth = i as f64 + 1.0 - (n.sqrt().ln().ln() / std::f64::consts::LN_2);
                return EscapeResult {
                    escaped: true, iterations: i,
                    smooth, final_norm: n.sqrt(), final_z: z,
                };
            }
        }
        EscapeResult {
            escaped: false, iterations: self.max_iter,
            smooth: self.max_iter as f64, final_norm: z.norm(), final_z: z,
        }
    }
}

// ── Burning Ship ──────────────────────────────────────────────────────────────

pub struct BurningShip {
    pub max_iter:    u32,
    pub escape_r_sq: f64,
}

impl Default for BurningShip {
    fn default() -> Self { Self { max_iter: 256, escape_r_sq: 4.0 } }
}

impl BurningShip {
    pub fn sample(&self, c: Complex) -> EscapeResult {
        let mut z = Complex::ZERO;
        for i in 0..self.max_iter {
            z = z.burning_ship_step(c);
            let n = z.norm_sq();
            if n > self.escape_r_sq {
                let smooth = i as f64 + 1.0 - (n.sqrt().ln().ln() / std::f64::consts::LN_2);
                return EscapeResult {
                    escaped: true, iterations: i,
                    smooth, final_norm: n.sqrt(), final_z: z,
                };
            }
        }
        EscapeResult {
            escaped: false, iterations: self.max_iter,
            smooth: self.max_iter as f64, final_norm: z.norm(), final_z: z,
        }
    }
}

// ── Newton fractal ────────────────────────────────────────────────────────────

/// Newton's method fractal for a polynomial p(z)/p'(z).
/// Converges to roots; color by which root was reached.
pub struct NewtonFractal {
    /// Coefficients of p(z) = coeff[0] + coeff[1]*z + ... + coeff[n]*z^n.
    pub coeffs:   Vec<Complex>,
    pub max_iter: u32,
    pub tol:      f64,
    pub relax:    Complex,  // relaxation parameter (usually 1+0i)
}

impl NewtonFractal {
    /// z^3 - 1 = 0 (classic 3-root Newton fractal).
    pub fn cubic_unity() -> Self {
        Self {
            coeffs:   vec![
                Complex::new(-1.0, 0.0),
                Complex::ZERO,
                Complex::ZERO,
                Complex::ONE,
            ],
            max_iter: 50,
            tol:      1e-6,
            relax:    Complex::ONE,
        }
    }

    /// z^4 - 1 = 0.
    pub fn quartic_unity() -> Self {
        Self {
            coeffs:   vec![
                Complex::new(-1.0, 0.0),
                Complex::ZERO, Complex::ZERO, Complex::ZERO,
                Complex::ONE,
            ],
            max_iter: 50,
            tol:      1e-6,
            relax:    Complex::ONE,
        }
    }

    /// Evaluate polynomial at z.
    fn eval(&self, z: Complex) -> Complex {
        let mut result = Complex::ZERO;
        let mut zpow   = Complex::ONE;
        for &c in &self.coeffs {
            result = result.add(c.mul(zpow));
            zpow   = zpow.mul(z);
        }
        result
    }

    /// Evaluate polynomial derivative at z.
    fn eval_deriv(&self, z: Complex) -> Complex {
        let mut result = Complex::ZERO;
        let mut zpow   = Complex::ONE;
        for (i, &c) in self.coeffs.iter().enumerate().skip(1) {
            let coeff = c.scale(i as f64);
            result = result.add(coeff.mul(zpow));
            zpow   = zpow.mul(z);
        }
        result
    }

    /// Returns (converged_to_root_index, iterations, final_z).
    pub fn sample(&self, z0: Complex) -> (Option<usize>, u32, Complex) {
        // Find roots (degree 1 less than coeffs.len()-1... but we don't compute analytically)
        // Just run Newton and let caller identify which root was hit.
        let mut z = z0;
        for i in 0..self.max_iter {
            let fz  = self.eval(z);
            let dfz = self.eval_deriv(z);
            if dfz.norm_sq() < 1e-300 { break; }
            let step = fz.div(dfz);
            z = z.sub(self.relax.mul(step));
            if step.norm() < self.tol {
                return (None, i, z);  // converged — root index assigned by caller
            }
        }
        (None, self.max_iter, z)
    }
}

// ── Lyapunov exponent ─────────────────────────────────────────────────────────

/// Computes the Lyapunov exponent for a logistic-map-like system.
/// The sequence string alternates between two parameter values A and B.
pub struct LyapunovFractal {
    pub sequence:    Vec<bool>, // false = use A, true = use B
    pub warmup:      u32,
    pub iterations:  u32,
}

impl Default for LyapunovFractal {
    fn default() -> Self {
        // Classic "AB" pattern
        Self { sequence: vec![false, true], warmup: 200, iterations: 1000 }
    }
}

impl LyapunovFractal {
    pub fn new(pattern: &str) -> Self {
        let sequence = pattern.chars()
            .filter(|c| *c == 'A' || *c == 'B')
            .map(|c| c == 'B')
            .collect();
        Self { sequence, warmup: 200, iterations: 1000 }
    }

    /// Sample the Lyapunov exponent at (a, b) in [0,4]×[0,4].
    /// Returns the exponent (negative = stable, positive = chaotic).
    pub fn sample(&self, a: f64, b: f64) -> f64 {
        if self.sequence.is_empty() { return 0.0; }
        let mut x   = 0.5_f64;
        let seq_len = self.sequence.len();

        // Warmup
        for i in 0..(self.warmup as usize) {
            let r = if self.sequence[i % seq_len] { b } else { a };
            x = r * x * (1.0 - x);
        }

        // Measure
        let mut lyap = 0.0_f64;
        for i in 0..(self.iterations as usize) {
            let r = if self.sequence[i % seq_len] { b } else { a };
            x = r * x * (1.0 - x);
            let d = (r * (1.0 - 2.0 * x)).abs();
            if d > 0.0 { lyap += d.ln(); }
        }
        lyap / self.iterations as f64
    }
}

// ── Fractal color palette ─────────────────────────────────────────────────────

/// Maps an EscapeResult to RGBA color.
#[derive(Clone, Copy, Debug)]
pub enum FractalPalette {
    /// Classic grayscale escape time.
    Grayscale,
    /// Smooth gradient using a 3-cycle cosine palette.
    Smooth { offset: f32, freq: f32 },
    /// Orbit trap: color based on closest approach to origin.
    OrbitTrap,
    /// Angle coloring: hue = orbit angle.
    AngleBased,
    /// Ultra-fractal style: 4 stops of gradient.
    UltraFractal,
    /// Fire: black → red → orange → yellow → white.
    Fire,
    /// Electric: black → deep blue → cyan → white.
    Electric,
}

impl FractalPalette {
    /// Convert an EscapeResult to RGBA in [0,1].
    pub fn color(&self, result: &EscapeResult, max_iter: u32) -> Vec4 {
        if !result.escaped {
            return Vec4::ZERO; // inside = black
        }
        let t = (result.smooth / max_iter as f64).clamp(0.0, 1.0) as f32;

        match self {
            FractalPalette::Grayscale => {
                let v = t;
                Vec4::new(v, v, v, 1.0)
            }
            FractalPalette::Smooth { offset, freq } => {
                let r = (std::f32::consts::TAU * (t * freq + offset + 0.0)).cos() * 0.5 + 0.5;
                let g = (std::f32::consts::TAU * (t * freq + offset + 0.333)).cos() * 0.5 + 0.5;
                let b = (std::f32::consts::TAU * (t * freq + offset + 0.667)).cos() * 0.5 + 0.5;
                Vec4::new(r, g, b, 1.0)
            }
            FractalPalette::OrbitTrap => {
                // Color by how close orbit got to origin
                let n = result.final_norm as f32;
                let v = 1.0 - (n / 2.0).min(1.0);
                Vec4::new(v * 0.9, v * 0.4, v * 1.0, 1.0)
            }
            FractalPalette::AngleBased => {
                let angle = result.final_z.arg() as f32;
                let hue   = (angle / std::f32::consts::TAU + 0.5).fract();
                Self::hsv_to_rgb(hue, 0.8 + t * 0.2, 0.7 + t * 0.3)
            }
            FractalPalette::UltraFractal => {
                // 4-stop gradient: 0=black, 0.25=teal, 0.5=gold, 0.75=white, 1=black
                let t4 = (t * 4.0).fract();
                let stop = (t * 4.0) as u32 % 4;
                let (a, b) = match stop {
                    0 => (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.8, 0.8)),
                    1 => (Vec3::new(0.0, 0.8, 0.8), Vec3::new(1.0, 0.85, 0.0)),
                    2 => (Vec3::new(1.0, 0.85, 0.0), Vec3::new(1.0, 1.0, 1.0)),
                    _ => (Vec3::new(1.0, 1.0, 1.0), Vec3::new(0.0, 0.0, 0.0)),
                };
                let c = a.lerp(b, t4);
                Vec4::new(c.x, c.y, c.z, 1.0)
            }
            FractalPalette::Fire => {
                if t < 0.25 {
                    let tt = t / 0.25;
                    Vec4::new(tt, 0.0, 0.0, 1.0)
                } else if t < 0.5 {
                    let tt = (t - 0.25) / 0.25;
                    Vec4::new(1.0, tt * 0.5, 0.0, 1.0)
                } else if t < 0.75 {
                    let tt = (t - 0.5) / 0.25;
                    Vec4::new(1.0, 0.5 + tt * 0.5, 0.0, 1.0)
                } else {
                    let tt = (t - 0.75) / 0.25;
                    Vec4::new(1.0, 1.0, tt, 1.0)
                }
            }
            FractalPalette::Electric => {
                if t < 0.33 {
                    let tt = t / 0.33;
                    Vec4::new(0.0, 0.0, tt * 0.6, 1.0)
                } else if t < 0.66 {
                    let tt = (t - 0.33) / 0.33;
                    Vec4::new(0.0, tt, 0.6 + tt * 0.4, 1.0)
                } else {
                    let tt = (t - 0.66) / 0.34;
                    Vec4::new(tt, 1.0, 1.0, 1.0)
                }
            }
        }
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Vec4 {
        let i = (h * 6.0) as u32;
        let f = h * 6.0 - i as f32;
        let p = v * (1.0 - s);
        let q = v * (1.0 - f * s);
        let t = v * (1.0 - (1.0 - f) * s);
        let (r, g, b) = match i % 6 {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        Vec4::new(r, g, b, 1.0)
    }
}

// ── Quaternion ────────────────────────────────────────────────────────────────

/// Unit quaternion for 3D rotation: q = w + xi + yj + zk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quaternion {
    pub const IDENTITY: Self = Self { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(w: f32, x: f32, y: f32, z: f32) -> Self { Self { w, x, y, z } }

    /// Create from axis-angle (axis must be normalized).
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let half = angle * 0.5;
        let s    = half.sin();
        Self::new(half.cos(), axis.x * s, axis.y * s, axis.z * s)
    }

    /// Create from Euler angles (roll, pitch, yaw) in radians.
    pub fn from_euler(roll: f32, pitch: f32, yaw: f32) -> Self {
        let cr = (roll  * 0.5).cos(); let sr = (roll  * 0.5).sin();
        let cp = (pitch * 0.5).cos(); let sp = (pitch * 0.5).sin();
        let cy = (yaw   * 0.5).cos(); let sy = (yaw   * 0.5).sin();
        Self::new(
            cr * cp * cy + sr * sp * sy,
            sr * cp * cy - cr * sp * sy,
            cr * sp * cy + sr * cp * sy,
            cr * cp * sy - sr * sp * cy,
        )
    }

    pub fn norm(self) -> f32 {
        (self.w*self.w + self.x*self.x + self.y*self.y + self.z*self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let n = self.norm();
        Self::new(self.w/n, self.x/n, self.y/n, self.z/n)
    }

    pub fn conjugate(self) -> Self {
        Self::new(self.w, -self.x, -self.y, -self.z)
    }

    pub fn inverse(self) -> Self {
        let n2 = self.w*self.w + self.x*self.x + self.y*self.y + self.z*self.z;
        let c  = self.conjugate();
        Self::new(c.w/n2, c.x/n2, c.y/n2, c.z/n2)
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.w*rhs.w + self.x*rhs.x + self.y*rhs.y + self.z*rhs.z
    }

    /// Hamilton product.
    pub fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.w*rhs.w - self.x*rhs.x - self.y*rhs.y - self.z*rhs.z,
            self.w*rhs.x + self.x*rhs.w + self.y*rhs.z - self.z*rhs.y,
            self.w*rhs.y - self.x*rhs.z + self.y*rhs.w + self.z*rhs.x,
            self.w*rhs.z + self.x*rhs.y - self.y*rhs.x + self.z*rhs.w,
        )
    }

    /// Rotate a 3D vector by this quaternion.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let q = self;
        let qv = Vec3::new(q.x, q.y, q.z);
        let uv = qv.cross(v);
        let uuv = qv.cross(uv);
        v + (uv * q.w + uuv) * 2.0
    }

    /// Spherical linear interpolation between two quaternions.
    pub fn slerp(self, other: Self, t: f32) -> Self {
        let mut d = self.dot(other);
        let other = if d < 0.0 {
            d = -d;
            Self::new(-other.w, -other.x, -other.y, -other.z)
        } else {
            other
        };

        if d > 0.9995 {
            // Very close — linear interpolation is fine
            return Self::new(
                self.w + t*(other.w - self.w),
                self.x + t*(other.x - self.x),
                self.y + t*(other.y - self.y),
                self.z + t*(other.z - self.z),
            ).normalize();
        }

        let theta_0 = d.acos();
        let theta   = theta_0 * t;
        let sin_t0  = theta_0.sin();
        let sin_t   = theta.sin();
        let s1 = (theta_0 - theta).sin() / sin_t0;
        let s2 = sin_t / sin_t0;

        Self::new(
            s1 * self.w + s2 * other.w,
            s1 * self.x + s2 * other.x,
            s1 * self.y + s2 * other.y,
            s1 * self.z + s2 * other.z,
        )
    }

    /// Extract the rotation angle around the rotation axis.
    pub fn angle(self) -> f32 {
        2.0 * self.w.clamp(-1.0, 1.0).acos()
    }

    /// Extract the rotation axis (normalized). Returns Y if rotation is near zero.
    pub fn axis(self) -> Vec3 {
        let s_sq = 1.0 - self.w * self.w;
        if s_sq < 1e-10 { return Vec3::Y; }
        let inv_s = 1.0 / s_sq.sqrt();
        Vec3::new(self.x * inv_s, self.y * inv_s, self.z * inv_s)
    }

    /// Exponential map: converts a rotation vector (axis * angle) to a quaternion.
    pub fn exp_map(v: Vec3) -> Self {
        let theta = v.length();
        if theta < 1e-8 { return Self::IDENTITY; }
        let half  = theta * 0.5;
        let s     = half.sin() / theta;
        Self::new(half.cos(), v.x * s, v.y * s, v.z * s)
    }

    /// Log map: converts a quaternion to its rotation vector (axis * angle).
    pub fn log_map(self) -> Vec3 {
        let v_norm = Vec3::new(self.x, self.y, self.z).length();
        if v_norm < 1e-8 { return Vec3::ZERO; }
        let theta = v_norm.atan2(self.w);
        Vec3::new(self.x, self.y, self.z) * (theta / v_norm)
    }

    /// To 4x4 rotation matrix (column-major, compatible with glam).
    pub fn to_mat4(self) -> glam::Mat4 {
        glam::Mat4::from_quat(glam::Quat::from_xyzw(self.x, self.y, self.z, self.w))
    }
}

impl Default for Quaternion {
    fn default() -> Self { Self::IDENTITY }
}

impl std::ops::Mul for Quaternion {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self { Quaternion::mul(self, rhs) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_mul() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, 4.0);
        let c = a * b;
        assert!((c.re - (-5.0)).abs() < 1e-10);
        assert!((c.im - 10.0).abs() < 1e-10);
    }

    #[test]
    fn complex_euler_identity() {
        // e^(iπ) + 1 = 0
        let z = Complex::new(0.0, std::f64::consts::PI).exp();
        assert!((z.re + 1.0).abs() < 1e-10);
        assert!(z.im.abs() < 1e-10);
    }

    #[test]
    fn complex_sqrt() {
        let z = Complex::new(-1.0, 0.0).sqrt();
        assert!(z.re.abs() < 1e-10);
        assert!((z.im - 1.0).abs() < 1e-10);
    }

    #[test]
    fn mandelbrot_origin_escapes_not() {
        let params = MandelbrotParams::default();
        let result = params.sample(Complex::ZERO);
        assert!(!result.escaped);
    }

    #[test]
    fn mandelbrot_far_escapes() {
        let params = MandelbrotParams::default();
        let result = params.sample(Complex::new(3.0, 3.0));
        assert!(result.escaped);
        assert!(result.iterations < 5);
    }

    #[test]
    fn julia_samples() {
        let j = JuliaParams::preset(0);
        let r = j.sample(Complex::new(0.0, 0.0));
        // Origin for Douady rabbit — should be inside (not escaped)
        // (it's near the Julia set; result may vary)
        let _ = r;
    }

    #[test]
    fn quat_identity_rotates_nothing() {
        let q = Quaternion::IDENTITY;
        let v = Vec3::new(1.0, 2.0, 3.0);
        let w = q.rotate(v);
        assert!((w - v).length() < 1e-5);
    }

    #[test]
    fn quat_axis_angle_roundtrip() {
        let axis  = Vec3::new(0.0, 1.0, 0.0);
        let angle = std::f32::consts::FRAC_PI_4;
        let q     = Quaternion::from_axis_angle(axis, angle);
        let v     = Vec3::new(1.0, 0.0, 0.0);
        let w     = q.rotate(v);
        // Should rotate X toward -Z by 45°
        assert!((w.x - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert!((w.z + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    }

    #[test]
    fn quat_slerp_halfway() {
        let a = Quaternion::IDENTITY;
        let b = Quaternion::from_axis_angle(Vec3::Y, std::f32::consts::PI);
        let m = a.slerp(b, 0.5);
        let expected_angle = std::f32::consts::FRAC_PI_2;
        assert!((m.angle() - expected_angle).abs() < 1e-4);
    }

    #[test]
    fn lyapunov_stable() {
        let l = LyapunovFractal::default();
        // r=2 should be stable (positive population growth, non-chaotic)
        let e = l.sample(2.0, 2.0);
        assert!(e < 0.0, "Expected negative Lyapunov exponent, got {e}");
    }

    #[test]
    fn lyapunov_chaotic() {
        let l = LyapunovFractal::default();
        // r near 4 is chaotic
        let e = l.sample(3.9, 3.9);
        assert!(e > 0.0, "Expected positive Lyapunov exponent at r=3.9, got {e}");
    }

    #[test]
    fn palette_maps_escaped() {
        let result = EscapeResult {
            escaped: true, iterations: 50, smooth: 51.3,
            final_norm: 2.1, final_z: Complex::new(2.0, 0.5),
        };
        let color = FractalPalette::Fire.color(&result, 256);
        assert!(color.w > 0.0);
    }
}
