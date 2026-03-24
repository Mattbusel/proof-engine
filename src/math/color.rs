//! Color science utilities: color spaces, palettes, gradients, LUT generation.
//!
//! Provides conversion between linear RGB, sRGB, HSV, HSL, Oklab, CIE Lab,
//! CIE LCH, and XYZ color spaces. Also includes gradient building, palette
//! generation, color harmonies, and LUT support.

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

// ── Core color types ──────────────────────────────────────────────────────────

/// Linear RGB color with alpha, all in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const WHITE:   Rgba = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK:   Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const RED:     Rgba = Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN:   Rgba = Rgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE:    Rgba = Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const YELLOW:  Rgba = Rgba { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const CYAN:    Rgba = Rgba { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const MAGENTA: Rgba = Rgba { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    pub fn rgb(r: f32, g: f32, b: f32) -> Self { Self { r, g, b, a: 1.0 } }

    pub fn from_vec4(v: Vec4) -> Self { Self { r: v.x, g: v.y, b: v.z, a: v.w } }
    pub fn to_vec4(self) -> Vec4 { Vec4::new(self.r, self.g, self.b, self.a) }
    pub fn to_vec3(self) -> Vec3 { Vec3::new(self.r, self.g, self.b) }

    /// Construct from an `0xRRGGBB` hex literal (alpha = 1).
    pub fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8)  & 0xFF) as f32 / 255.0;
        let b = ( hex        & 0xFF) as f32 / 255.0;
        Self::rgb(r, g, b)
    }

    /// Construct from an `0xRRGGBBAA` hex literal.
    pub fn from_hex_alpha(hex: u32) -> Self {
        let r = ((hex >> 24) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let b = ((hex >> 8)  & 0xFF) as f32 / 255.0;
        let a = ( hex        & 0xFF) as f32 / 255.0;
        Self { r, g, b, a }
    }

    pub fn with_alpha(self, a: f32) -> Self { Self { a, ..self } }
    pub fn lerp(self, other: Rgba, t: f32) -> Self {
        Rgba {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Premultiplied alpha blend: self over other.
    pub fn over(self, other: Rgba) -> Rgba {
        let ia = 1.0 - self.a;
        Rgba {
            r: self.r * self.a + other.r * ia,
            g: self.g * self.a + other.g * ia,
            b: self.b * self.a + other.b * ia,
            a: self.a + other.a * ia,
        }
    }

    /// Linear luminance (ITU-R BT.709).
    pub fn luminance(self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    /// Convert to 8-bit RGBA tuple.
    pub fn to_u8(self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0) as u8,
            (self.a.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }
}

impl From<Vec4> for Rgba {
    fn from(v: Vec4) -> Self { Self::from_vec4(v) }
}

impl From<Rgba> for Vec4 {
    fn from(c: Rgba) -> Self { c.to_vec4() }
}

// ── sRGB gamma ────────────────────────────────────────────────────────────────

/// Apply sRGB gamma (linear → display).
#[inline]
pub fn linear_to_srgb_channel(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Remove sRGB gamma (display → linear).
#[inline]
pub fn srgb_to_linear_channel(x: f32) -> f32 {
    if x <= 0.040_45 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: Rgba) -> Rgba {
    Rgba::new(
        linear_to_srgb_channel(c.r),
        linear_to_srgb_channel(c.g),
        linear_to_srgb_channel(c.b),
        c.a,
    )
}

pub fn srgb_to_linear(c: Rgba) -> Rgba {
    Rgba::new(
        srgb_to_linear_channel(c.r),
        srgb_to_linear_channel(c.g),
        srgb_to_linear_channel(c.b),
        c.a,
    )
}

// ── HSV ───────────────────────────────────────────────────────────────────────

/// HSV color: hue in `[0, 360)`, saturation and value in `[0, 1]`.
#[derive(Debug, Clone, Copy)]
pub struct Hsv { pub h: f32, pub s: f32, pub v: f32 }

impl Hsv {
    pub fn new(h: f32, s: f32, v: f32) -> Self { Self { h, s, v } }

    pub fn to_rgb(self) -> Rgba {
        let (r, g, b) = hsv_to_rgb(self.h, self.s, self.v);
        Rgba::rgb(r, g, b)
    }

    pub fn from_rgb(c: Rgba) -> Self {
        let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
        Self { h, s, v }
    }
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 { return (v, v, v); }
    let h = ((h % 360.0) + 360.0) % 360.0;
    let i = (h / 60.0) as u32;
    let f = h / 60.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max < 1e-8 { 0.0 } else { delta / max };
    let h = if delta < 1e-8 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (((h % 360.0) + 360.0) % 360.0, s, v)
}

// ── HSL ───────────────────────────────────────────────────────────────────────

/// HSL color: hue in `[0, 360)`, saturation and lightness in `[0, 1]`.
#[derive(Debug, Clone, Copy)]
pub struct Hsl { pub h: f32, pub s: f32, pub l: f32 }

impl Hsl {
    pub fn new(h: f32, s: f32, l: f32) -> Self { Self { h, s, l } }

    pub fn to_rgb(self) -> Rgba {
        let (r, g, b) = hsl_to_rgb(self.h, self.s, self.l);
        Rgba::rgb(r, g, b)
    }
}

fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    let t = ((t % 1.0) + 1.0) % 1.0;
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0 / 2.0 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    p
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 { return (l, l, l); }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let h = h / 360.0;
    (
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    )
}

pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l   = (max + min) * 0.5;
    let delta = max - min;

    if delta < 1e-8 { return (0.0, 0.0, l); }

    let s = if l < 0.5 { delta / (max + min) } else { delta / (2.0 - max - min) };
    let h = if max == r {
        60.0 * ((g - b) / delta + if g < b { 6.0 } else { 0.0 })
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (h, s, l)
}

// ── Oklab ────────────────────────────────────────────────────────────────────

/// Oklab color: a perceptually uniform color space by Björn Ottosson.
/// `L` = lightness [0,1], `a` and `b` are chroma axes (approx −0.5..0.5).
#[derive(Debug, Clone, Copy)]
pub struct Oklab { pub l: f32, pub a: f32, pub b: f32 }

impl Oklab {
    pub fn from_linear_rgb(c: Rgba) -> Self {
        let l = 0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b;
        let m = 0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b;
        let s = 0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b;

        let l_ = l.cbrt();
        let m_ = m.cbrt();
        let s_ = s.cbrt();

        Self {
            l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
            a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
            b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
        }
    }

    pub fn to_linear_rgb(self) -> Rgba {
        let l_ = self.l + 0.3963377774 * self.a + 0.2158037573 * self.b;
        let m_ = self.l - 0.1055613458 * self.a - 0.0638541728 * self.b;
        let s_ = self.l - 0.0894841775 * self.a - 1.2914855480 * self.b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        Rgba::rgb(
             4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
            -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
            -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
        )
    }

    /// Perceptually-uniform lerp in Oklab space.
    pub fn lerp(self, other: Oklab, t: f32) -> Oklab {
        Oklab {
            l: self.l + (other.l - self.l) * t,
            a: self.a + (other.a - self.a) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }
}

// ── CIE XYZ ───────────────────────────────────────────────────────────────────

/// CIE XYZ (D65 white point).
#[derive(Debug, Clone, Copy)]
pub struct Xyz { pub x: f32, pub y: f32, pub z: f32 }

impl Xyz {
    pub fn from_linear_rgb(c: Rgba) -> Self {
        Self {
            x: c.r * 0.4124 + c.g * 0.3576 + c.b * 0.1805,
            y: c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722,
            z: c.r * 0.0193 + c.g * 0.1192 + c.b * 0.9505,
        }
    }

    pub fn to_linear_rgb(self) -> Rgba {
        Rgba::rgb(
             self.x *  3.2406 + self.y * -1.5372 + self.z * -0.4986,
             self.x * -0.9689 + self.y *  1.8758 + self.z *  0.0415,
             self.x *  0.0557 + self.y * -0.2040 + self.z *  1.0570,
        )
    }
}

// ── CIE Lab ───────────────────────────────────────────────────────────────────

/// CIE L*a*b* color space (D65 white point).
#[derive(Debug, Clone, Copy)]
pub struct Lab { pub l: f32, pub a: f32, pub b: f32 }

const D65_X: f32 = 0.95047;
const D65_Y: f32 = 1.00000;
const D65_Z: f32 = 1.08883;

fn xyz_to_lab_f(t: f32) -> f32 {
    if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 }
}

impl Lab {
    pub fn from_xyz(xyz: Xyz) -> Self {
        let fx = xyz_to_lab_f(xyz.x / D65_X);
        let fy = xyz_to_lab_f(xyz.y / D65_Y);
        let fz = xyz_to_lab_f(xyz.z / D65_Z);
        Self {
            l: 116.0 * fy - 16.0,
            a: 500.0 * (fx - fy),
            b: 200.0 * (fy - fz),
        }
    }

    pub fn to_xyz(self) -> Xyz {
        let fy = (self.l + 16.0) / 116.0;
        let fx = self.a / 500.0 + fy;
        let fz = fy - self.b / 200.0;
        let cube = |v: f32| if v > 0.2069 { v * v * v } else { (v - 16.0 / 116.0) / 7.787 };
        Xyz { x: cube(fx) * D65_X, y: cube(fy) * D65_Y, z: cube(fz) * D65_Z }
    }

    pub fn from_rgb(c: Rgba) -> Self {
        Self::from_xyz(Xyz::from_linear_rgb(c))
    }

    pub fn to_rgb(self) -> Rgba {
        self.to_xyz().to_linear_rgb()
    }

    /// Delta E 1976 (perceptual distance).
    pub fn delta_e(&self, other: &Lab) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        (dl * dl + da * da + db * db).sqrt()
    }
}

/// CIE LCH (Lightness, Chroma, Hue in degrees).
#[derive(Debug, Clone, Copy)]
pub struct Lch { pub l: f32, pub c: f32, pub h: f32 }

impl Lch {
    pub fn from_lab(lab: Lab) -> Self {
        let c = (lab.a * lab.a + lab.b * lab.b).sqrt();
        let h = lab.b.atan2(lab.a).to_degrees();
        let h = ((h % 360.0) + 360.0) % 360.0;
        Self { l: lab.l, c, h }
    }

    pub fn to_lab(self) -> Lab {
        let h_rad = self.h.to_radians();
        Lab { l: self.l, a: self.c * h_rad.cos(), b: self.c * h_rad.sin() }
    }

    pub fn from_rgb(c: Rgba) -> Self { Self::from_lab(Lab::from_rgb(c)) }
    pub fn to_rgb(self) -> Rgba { self.to_lab().to_rgb() }

    pub fn lerp_hue(self, other: Lch, t: f32) -> Lch {
        // Shortest path around the hue circle
        let mut dh = other.h - self.h;
        if dh >  180.0 { dh -= 360.0; }
        if dh < -180.0 { dh += 360.0; }
        Lch {
            l: self.l + (other.l - self.l) * t,
            c: self.c + (other.c - self.c) * t,
            h: self.h + dh * t,
        }
    }
}

// ── Gradient ──────────────────────────────────────────────────────────────────

/// Interpolation mode for gradient stops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientMode {
    /// Linear RGB interpolation.
    LinearRgb,
    /// Oklab interpolation (perceptually uniform, no "dark middle" artifacts).
    Oklab,
    /// LCH interpolation (preserves hue).
    Lch,
    /// HSV interpolation.
    Hsv,
}

/// A color stop in a gradient.
#[derive(Debug, Clone, Copy)]
pub struct ColorStop {
    pub t:     f32,   // [0, 1]
    pub color: Rgba,
}

/// A multi-stop color gradient.
#[derive(Debug, Clone)]
pub struct Gradient {
    pub stops: Vec<ColorStop>,
    pub mode:  GradientMode,
}

impl Gradient {
    pub fn new(mode: GradientMode) -> Self {
        Self { stops: Vec::new(), mode }
    }

    pub fn add_stop(mut self, t: f32, color: Rgba) -> Self {
        self.stops.push(ColorStop { t: t.clamp(0.0, 1.0), color });
        self.stops.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
        self
    }

    /// Sample the gradient at `t ∈ [0, 1]`.
    pub fn sample(&self, t: f32) -> Rgba {
        if self.stops.is_empty() { return Rgba::BLACK; }
        if self.stops.len() == 1 { return self.stops[0].color; }

        let t = t.clamp(0.0, 1.0);

        // Find surrounding stops
        let i = self.stops.partition_point(|s| s.t <= t);
        if i == 0               { return self.stops[0].color; }
        if i >= self.stops.len() { return self.stops.last().unwrap().color; }

        let lo = &self.stops[i - 1];
        let hi = &self.stops[i];
        let f  = (t - lo.t) / (hi.t - lo.t).max(1e-8);

        match self.mode {
            GradientMode::LinearRgb => lo.color.lerp(hi.color, f),
            GradientMode::Oklab => {
                let a = Oklab::from_linear_rgb(lo.color);
                let b = Oklab::from_linear_rgb(hi.color);
                a.lerp(b, f).to_linear_rgb()
            }
            GradientMode::Lch => {
                let a = Lch::from_rgb(lo.color);
                let b = Lch::from_rgb(hi.color);
                a.lerp_hue(b, f).to_rgb()
            }
            GradientMode::Hsv => {
                let (ha, sa, va) = rgb_to_hsv(lo.color.r, lo.color.g, lo.color.b);
                let (hb, sb, vb) = rgb_to_hsv(hi.color.r, hi.color.g, hi.color.b);
                let mut dh = hb - ha;
                if dh >  180.0 { dh -= 360.0; }
                if dh < -180.0 { dh += 360.0; }
                let h = ha + dh * f;
                let s = sa + (sb - sa) * f;
                let v = va + (vb - va) * f;
                let (r, g, b) = hsv_to_rgb(h, s, v);
                Rgba::rgb(r, g, b)
            }
        }
    }

    /// Produce a `Vec<Rgba>` LUT with `n` entries.
    pub fn bake_lut(&self, n: usize) -> Vec<Rgba> {
        (0..n).map(|i| self.sample(i as f32 / (n - 1) as f32)).collect()
    }
}

// ── Named gradients ───────────────────────────────────────────────────────────

pub fn gradient_plasma() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::from_hex(0x0d0887))
        .add_stop(0.2, Rgba::from_hex(0x6a00a8))
        .add_stop(0.4, Rgba::from_hex(0xb12a90))
        .add_stop(0.6, Rgba::from_hex(0xe16462))
        .add_stop(0.8, Rgba::from_hex(0xfca636))
        .add_stop(1.0, Rgba::from_hex(0xf0f921))
}

pub fn gradient_inferno() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::from_hex(0x000004))
        .add_stop(0.25, Rgba::from_hex(0x420a68))
        .add_stop(0.5,  Rgba::from_hex(0x932667))
        .add_stop(0.75, Rgba::from_hex(0xdd513a))
        .add_stop(0.9,  Rgba::from_hex(0xfca50a))
        .add_stop(1.0,  Rgba::from_hex(0xfcffa4))
}

pub fn gradient_viridis() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0,  Rgba::from_hex(0x440154))
        .add_stop(0.25, Rgba::from_hex(0x31688e))
        .add_stop(0.5,  Rgba::from_hex(0x35b779))
        .add_stop(0.75, Rgba::from_hex(0x90d743))
        .add_stop(1.0,  Rgba::from_hex(0xfde725))
}

pub fn gradient_fire() -> Gradient {
    Gradient::new(GradientMode::LinearRgb)
        .add_stop(0.0, Rgba::BLACK)
        .add_stop(0.3, Rgba::rgb(0.5, 0.0, 0.0))
        .add_stop(0.6, Rgba::rgb(1.0, 0.3, 0.0))
        .add_stop(0.8, Rgba::rgb(1.0, 0.8, 0.0))
        .add_stop(1.0, Rgba::WHITE)
}

pub fn gradient_ice() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::BLACK)
        .add_stop(0.4, Rgba::rgb(0.0, 0.2, 0.5))
        .add_stop(0.7, Rgba::rgb(0.2, 0.6, 1.0))
        .add_stop(1.0, Rgba::WHITE)
}

pub fn gradient_neon() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::from_hex(0xff00ff))
        .add_stop(0.5, Rgba::from_hex(0x00ffff))
        .add_stop(1.0, Rgba::from_hex(0xff00ff))
}

pub fn gradient_health() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::rgb(1.0, 0.0, 0.0))
        .add_stop(0.5, Rgba::rgb(1.0, 0.8, 0.0))
        .add_stop(1.0, Rgba::rgb(0.0, 1.0, 0.2))
}

// ── Color harmonies ───────────────────────────────────────────────────────────

/// Generate a complementary color (180° hue rotation).
pub fn complementary(c: Rgba) -> Rgba {
    let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
    let (r, g, b) = hsv_to_rgb((h + 180.0) % 360.0, s, v);
    Rgba::rgb(r, g, b)
}

/// Generate split-complementary colors (150° and 210°).
pub fn split_complementary(c: Rgba) -> (Rgba, Rgba) {
    let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
    let mk = |dh: f32| {
        let (r, g, b) = hsv_to_rgb((h + dh) % 360.0, s, v);
        Rgba::rgb(r, g, b)
    };
    (mk(150.0), mk(210.0))
}

/// Generate triadic colors (120° apart).
pub fn triadic(c: Rgba) -> (Rgba, Rgba) {
    let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
    let mk = |dh: f32| {
        let (r, g, b) = hsv_to_rgb((h + dh) % 360.0, s, v);
        Rgba::rgb(r, g, b)
    };
    (mk(120.0), mk(240.0))
}

/// Generate analogous colors (±30°).
pub fn analogous(c: Rgba) -> (Rgba, Rgba) {
    let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
    let mk = |dh: f32| {
        let (r, g, b) = hsv_to_rgb((h + dh + 360.0) % 360.0, s, v);
        Rgba::rgb(r, g, b)
    };
    (mk(-30.0), mk(30.0))
}

/// Generate a tetradic (square) color scheme.
pub fn tetradic(c: Rgba) -> [Rgba; 4] {
    let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
    std::array::from_fn(|i| {
        let (r, g, b) = hsv_to_rgb((h + i as f32 * 90.0) % 360.0, s, v);
        Rgba::rgb(r, g, b)
    })
}

// ── Palette types ─────────────────────────────────────────────────────────────

/// A named palette of colors.
#[derive(Debug, Clone)]
pub struct Palette {
    pub name:   String,
    pub colors: Vec<Rgba>,
}

impl Palette {
    pub fn new(name: impl Into<String>, colors: Vec<Rgba>) -> Self {
        Self { name: name.into(), colors }
    }

    /// Sample the palette by index (wraps around).
    pub fn get(&self, i: usize) -> Rgba {
        if self.colors.is_empty() { return Rgba::WHITE; }
        self.colors[i % self.colors.len()]
    }

    /// Sample interpolated between palette colors.
    pub fn sample(&self, t: f32) -> Rgba {
        if self.colors.is_empty() { return Rgba::WHITE; }
        if self.colors.len() == 1 { return self.colors[0]; }
        let t = t.fract().abs();
        let f = t * (self.colors.len() - 1) as f32;
        let i = f as usize;
        let j = (i + 1).min(self.colors.len() - 1);
        self.colors[i].lerp(self.colors[j], f.fract())
    }
}

/// CRT terminal / retrowave palette.
pub fn palette_crt() -> Palette {
    Palette::new("CRT", vec![
        Rgba::from_hex(0x00ff00), // phosphor green
        Rgba::from_hex(0x00ffff), // cyan
        Rgba::from_hex(0xff6600), // amber
        Rgba::from_hex(0xffffff), // white
    ])
}

/// ANSI 16-color terminal palette.
pub fn palette_ansi16() -> Palette {
    Palette::new("ANSI16", vec![
        Rgba::from_hex(0x000000), Rgba::from_hex(0xaa0000),
        Rgba::from_hex(0x00aa00), Rgba::from_hex(0xaa5500),
        Rgba::from_hex(0x0000aa), Rgba::from_hex(0xaa00aa),
        Rgba::from_hex(0x00aaaa), Rgba::from_hex(0xaaaaaa),
        Rgba::from_hex(0x555555), Rgba::from_hex(0xff5555),
        Rgba::from_hex(0x55ff55), Rgba::from_hex(0xffff55),
        Rgba::from_hex(0x5555ff), Rgba::from_hex(0xff55ff),
        Rgba::from_hex(0x55ffff), Rgba::from_hex(0xffffff),
    ])
}

/// Chaos RPG element colors.
pub fn palette_chaos_elements() -> Palette {
    Palette::new("ChaosElements", vec![
        Rgba::from_hex(0xff4400), // fire
        Rgba::from_hex(0x00aaff), // water/ice
        Rgba::from_hex(0x44ff44), // life
        Rgba::from_hex(0xaa00ff), // shadow/void
        Rgba::from_hex(0xffcc00), // lightning
        Rgba::from_hex(0x22ffcc), // arcane
        Rgba::from_hex(0xff00aa), // chaos
    ])
}

// ── Tone mapping ──────────────────────────────────────────────────────────────

/// Reinhard tone mapping operator.
pub fn tonemap_reinhard(c: Rgba) -> Rgba {
    let map = |x: f32| x / (x + 1.0);
    Rgba::new(map(c.r), map(c.g), map(c.b), c.a)
}

/// ACES filmic tone mapping approximation (Narkowicz 2015).
pub fn tonemap_aces(c: Rgba) -> Rgba {
    let aces = |x: f32| -> f32 {
        const A: f32 = 2.51;
        const B: f32 = 0.03;
        const C: f32 = 2.43;
        const D: f32 = 0.59;
        const E: f32 = 0.14;
        ((x * (A * x + B)) / (x * (C * x + D) + E)).clamp(0.0, 1.0)
    };
    Rgba::new(aces(c.r), aces(c.g), aces(c.b), c.a)
}

/// Uncharted 2 "Hable" filmic tone mapping.
pub fn tonemap_uncharted2(c: Rgba) -> Rgba {
    fn partial(x: f32) -> f32 {
        const A: f32 = 0.15; const B: f32 = 0.50;
        const C: f32 = 0.10; const D: f32 = 0.20;
        const E: f32 = 0.02; const F: f32 = 0.30;
        ((x*(A*x+C*B)+D*E) / (x*(A*x+B)+D*F)) - E/F
    }
    let exposure_bias = 2.0_f32;
    let curr = |x: f32| partial(x * exposure_bias);
    let white = partial(11.2);
    let scale = 1.0 / white;
    Rgba::new(curr(c.r)*scale, curr(c.g)*scale, curr(c.b)*scale, c.a)
}

// ── Color distance ────────────────────────────────────────────────────────────

/// Euclidean distance in linear RGB space.
pub fn distance_rgb(a: Rgba, b: Rgba) -> f32 {
    let dr = a.r - b.r; let dg = a.g - b.g; let db = a.b - b.b;
    (dr*dr + dg*dg + db*db).sqrt()
}

/// Perceptual distance using CIE Lab Delta-E 1976.
pub fn distance_lab_e76(a: Rgba, b: Rgba) -> f32 {
    Lab::from_rgb(a).delta_e(&Lab::from_rgb(b))
}

/// Find the nearest color in a palette (by Lab Delta-E).
pub fn nearest_in_palette(color: Rgba, palette: &Palette) -> usize {
    let lab = Lab::from_rgb(color);
    palette.colors.iter()
        .enumerate()
        .min_by(|(_, &a), (_, &b)| {
            let da = lab.delta_e(&Lab::from_rgb(a));
            let db = lab.delta_e(&Lab::from_rgb(b));
            da.partial_cmp(&db).unwrap()
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

// ── Color adjustment ──────────────────────────────────────────────────────────

/// Adjust hue by `delta` degrees.
pub fn adjust_hue(c: Rgba, delta: f32) -> Rgba {
    let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
    let (r, g, b) = hsv_to_rgb((h + delta + 360.0) % 360.0, s, v);
    Rgba::new(r, g, b, c.a)
}

/// Saturate or desaturate (1 = no change, 0 = greyscale, >1 = boost).
pub fn adjust_saturation(c: Rgba, factor: f32) -> Rgba {
    let lum = c.luminance();
    Rgba::new(
        lum + (c.r - lum) * factor,
        lum + (c.g - lum) * factor,
        lum + (c.b - lum) * factor,
        c.a,
    )
}

/// Adjust brightness (additive offset).
pub fn adjust_brightness(c: Rgba, delta: f32) -> Rgba {
    Rgba::new((c.r + delta).clamp(0.0, 1.0),
              (c.g + delta).clamp(0.0, 1.0),
              (c.b + delta).clamp(0.0, 1.0),
              c.a)
}

/// Adjust contrast around 0.5 midpoint (factor >1 = more contrast).
pub fn adjust_contrast(c: Rgba, factor: f32) -> Rgba {
    let adj = |x: f32| ((x - 0.5) * factor + 0.5).clamp(0.0, 1.0);
    Rgba::new(adj(c.r), adj(c.g), adj(c.b), c.a)
}

/// Mix color `c` with white by `factor ∈ [0, 1]` (0 = original, 1 = white).
pub fn tint_white(c: Rgba, factor: f32) -> Rgba {
    c.lerp(Rgba::WHITE, factor)
}

/// Mix color `c` with black by `factor ∈ [0, 1]` (0 = original, 1 = black).
pub fn shade_black(c: Rgba, factor: f32) -> Rgba {
    c.lerp(Rgba::BLACK, factor)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool { (a - b).abs() < 0.005 }

    #[test]
    fn hsv_roundtrip() {
        let (h0, s0, v0) = (200.0f32, 0.7, 0.8);
        let (r, g, b) = hsv_to_rgb(h0, s0, v0);
        let (h1, s1, v1) = rgb_to_hsv(r, g, b);
        assert!(approx_eq(h0, h1), "hue mismatch: {h0} vs {h1}");
        assert!(approx_eq(s0, s1));
        assert!(approx_eq(v0, v1));
    }

    #[test]
    fn hsl_roundtrip() {
        let (r, g, b) = hsl_to_rgb(120.0, 0.5, 0.5);
        let (h, s, l) = rgb_to_hsl(r, g, b);
        assert!(approx_eq(h, 120.0), "hue mismatch: {h}");
        assert!(approx_eq(s, 0.5));
        assert!(approx_eq(l, 0.5));
    }

    #[test]
    fn oklab_roundtrip() {
        let c = Rgba::from_hex(0x3a7bd5);
        let oklab = Oklab::from_linear_rgb(c);
        let back  = oklab.to_linear_rgb();
        assert!(approx_eq(c.r, back.r), "r mismatch: {} vs {}", c.r, back.r);
        assert!(approx_eq(c.g, back.g));
        assert!(approx_eq(c.b, back.b));
    }

    #[test]
    fn gradient_endpoints() {
        let g = gradient_fire();
        let lo = g.sample(0.0);
        let hi = g.sample(1.0);
        assert!(lo.luminance() < 0.01);
        assert!(hi.luminance() > 0.9);
    }

    #[test]
    fn complementary_is_180_degrees() {
        let c = Rgba::from_hex(0xff0000); // red
        let comp = complementary(c);
        let (h, _, _) = rgb_to_hsv(comp.r, comp.g, comp.b);
        // Complementary of red (0°) should be cyan (180°)
        assert!((h - 180.0).abs() < 2.0, "hue={h}");
    }

    #[test]
    fn tonemap_aces_bounds() {
        let bright = Rgba::rgb(10.0, 5.0, 2.0); // HDR value
        let tm = tonemap_aces(bright);
        assert!(tm.r <= 1.0);
        assert!(tm.g <= 1.0);
        assert!(tm.b <= 1.0);
    }
}
