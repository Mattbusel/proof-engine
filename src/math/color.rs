//! Color science utilities: color spaces, palettes, gradients, LUT generation.
//!
//! Provides conversion between linear RGB, sRGB, HSV, HSL, Oklab, CIE Lab,
//! CIE LCH, and XYZ color spaces. Also includes gradient building, palette
//! generation, color harmonies, and LUT support.

#![warn(missing_docs)]

use glam::{Vec3, Vec4};
use std::f32::consts::PI;

// ── Core color types ──────────────────────────────────────────────────────────

/// Linear RGB color with alpha, all in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red, 0.0 to 1.0.
    pub r: f32,
    /// Green, 0.0 to 1.0.
    pub g: f32,
    /// Blue, 0.0 to 1.0.
    pub b: f32,
    /// Alpha (opacity), 0.0 to 1.0.
    pub a: f32,
}

impl Rgba {
    /// Opaque white.
    pub const WHITE:   Rgba = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    /// Opaque black.
    pub const BLACK:   Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    /// Opaque red.
    pub const RED:     Rgba = Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    /// Opaque green.
    pub const GREEN:   Rgba = Rgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    /// Opaque blue.
    pub const BLUE:    Rgba = Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    /// Opaque yellow.
    pub const YELLOW:  Rgba = Rgba { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    /// Opaque cyan.
    pub const CYAN:    Rgba = Rgba { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    /// Opaque magenta.
    pub const MAGENTA: Rgba = Rgba { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    /// Fully transparent black.
    pub const TRANSPARENT: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    /// A colour from red, green, blue and alpha.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    /// An opaque colour from red, green and blue.
    pub fn rgb(r: f32, g: f32, b: f32) -> Self { Self { r, g, b, a: 1.0 } }

    /// From a `Vec4` laid out as (r, g, b, a).
    pub fn from_vec4(v: Vec4) -> Self { Self { r: v.x, g: v.y, b: v.z, a: v.w } }
    /// As a `Vec4` (r, g, b, a).
    pub fn to_vec4(self) -> Vec4 { Vec4::new(self.r, self.g, self.b, self.a) }
    /// As a `Vec3` (r, g, b), dropping alpha.
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

    /// The same colour with alpha replaced by `a`.
    pub fn with_alpha(self, a: f32) -> Self { Self { a, ..self } }
    /// Per-channel linear interpolation: `t = 0` gives `self`, `t = 1` gives `other`.
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

/// Encode linear RGB to sRGB gamma (alpha unchanged).
pub fn linear_to_srgb(c: Rgba) -> Rgba {
    Rgba::new(
        linear_to_srgb_channel(c.r),
        linear_to_srgb_channel(c.g),
        linear_to_srgb_channel(c.b),
        c.a,
    )
}

/// Decode sRGB gamma to linear RGB (alpha unchanged).
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
pub struct Hsv {
    /// Hue in degrees, 0 to 360.
    pub h: f32,
    /// Saturation, 0.0 to 1.0.
    pub s: f32,
    /// Value (brightness), 0.0 to 1.0.
    pub v: f32,
}

impl Hsv {
    /// From hue (degrees), saturation and value.
    pub fn new(h: f32, s: f32, v: f32) -> Self { Self { h, s, v } }

    /// Convert to an opaque RGB colour.
    pub fn to_rgb(self) -> Rgba {
        let (r, g, b) = hsv_to_rgb(self.h, self.s, self.v);
        Rgba::rgb(r, g, b)
    }

    /// Convert from RGB (alpha ignored).
    pub fn from_rgb(c: Rgba) -> Self {
        let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
        Self { h, s, v }
    }
}

/// HSV (hue in degrees, wrapped to 0..360) to `(r, g, b)`.
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

/// `(r, g, b)` to HSV with hue in degrees, 0 to 360.
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
pub struct Hsl {
    /// Hue in degrees, 0 to 360.
    pub h: f32,
    /// Saturation, 0.0 to 1.0.
    pub s: f32,
    /// Lightness, 0.0 to 1.0.
    pub l: f32,
}

impl Hsl {
    /// From hue (degrees), saturation and lightness.
    pub fn new(h: f32, s: f32, l: f32) -> Self { Self { h, s, l } }

    /// Convert to an opaque RGB colour.
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

/// HSL (hue in degrees) to `(r, g, b)`.
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

/// `(r, g, b)` to HSL with hue in degrees.
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
/// `L` = lightness \[0,1\], `a` and `b` are chroma axes (approx −0.5..0.5).
#[derive(Debug, Clone, Copy)]
pub struct Oklab {
    /// Perceived lightness, 0.0 to 1.0.
    pub l: f32,
    /// Green (negative) to red (positive) axis.
    pub a: f32,
    /// Blue (negative) to yellow (positive) axis.
    pub b: f32,
}

impl Oklab {
    /// Convert from linear RGB (alpha ignored).
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

    /// Convert to opaque linear RGB (may fall outside 0..1 for out-of-gamut colours).
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
pub struct Xyz {
    /// X tristimulus value.
    pub x: f32,
    /// Y tristimulus value (luminance).
    pub y: f32,
    /// Z tristimulus value.
    pub z: f32,
}

impl Xyz {
    /// Convert from linear sRGB primaries (alpha ignored).
    pub fn from_linear_rgb(c: Rgba) -> Self {
        Self {
            x: c.r * 0.4124 + c.g * 0.3576 + c.b * 0.1805,
            y: c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722,
            z: c.r * 0.0193 + c.g * 0.1192 + c.b * 0.9505,
        }
    }

    /// Convert to opaque linear sRGB.
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
pub struct Lab {
    /// Lightness, 0 to 100.
    pub l: f32,
    /// Green (negative) to red (positive) axis.
    pub a: f32,
    /// Blue (negative) to yellow (positive) axis.
    pub b: f32,
}

const D65_X: f32 = 0.95047;
const D65_Y: f32 = 1.00000;
const D65_Z: f32 = 1.08883;

fn xyz_to_lab_f(t: f32) -> f32 {
    if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 }
}

impl Lab {
    /// Convert from XYZ relative to the D65 white point.
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

    /// Convert to XYZ relative to the D65 white point.
    pub fn to_xyz(self) -> Xyz {
        let fy = (self.l + 16.0) / 116.0;
        let fx = self.a / 500.0 + fy;
        let fz = fy - self.b / 200.0;
        let cube = |v: f32| if v > 0.2069 { v * v * v } else { (v - 16.0 / 116.0) / 7.787 };
        Xyz { x: cube(fx) * D65_X, y: cube(fy) * D65_Y, z: cube(fz) * D65_Z }
    }

    /// Convert from linear RGB (alpha ignored).
    pub fn from_rgb(c: Rgba) -> Self {
        Self::from_xyz(Xyz::from_linear_rgb(c))
    }

    /// Convert to opaque linear RGB.
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
pub struct Lch {
    /// Lightness, 0 to 100.
    pub l: f32,
    /// Chroma (colourfulness), 0 and up.
    pub c: f32,
    /// Hue in degrees, 0 to 360.
    pub h: f32,
}

impl Lch {
    /// Convert from Lab (polar form of a and b).
    pub fn from_lab(lab: Lab) -> Self {
        let c = (lab.a * lab.a + lab.b * lab.b).sqrt();
        let h = lab.b.atan2(lab.a).to_degrees();
        let h = ((h % 360.0) + 360.0) % 360.0;
        Self { l: lab.l, c, h }
    }

    /// Convert to Lab.
    pub fn to_lab(self) -> Lab {
        let h_rad = self.h.to_radians();
        Lab { l: self.l, a: self.c * h_rad.cos(), b: self.c * h_rad.sin() }
    }

    /// Convert from linear RGB (alpha ignored).
    pub fn from_rgb(c: Rgba) -> Self { Self::from_lab(Lab::from_rgb(c)) }
    /// Convert to opaque linear RGB.
    pub fn to_rgb(self) -> Rgba { self.to_lab().to_rgb() }

    /// Interpolate lightness and chroma linearly and hue along the shorter
    /// way round the colour wheel.
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
    /// Position along the gradient, 0.0 to 1.0.
    pub t:     f32,
    /// Colour at that position.
    pub color: Rgba,
}

/// A multi-stop color gradient.
#[derive(Debug, Clone)]
pub struct Gradient {
    /// Stops, kept sorted by `t`.
    pub stops: Vec<ColorStop>,
    /// Colour space used to blend between stops.
    pub mode:  GradientMode,
}

impl Gradient {
    /// An empty gradient that blends in `mode`.
    pub fn new(mode: GradientMode) -> Self {
        Self { stops: Vec::new(), mode }
    }

    /// Add a stop at `t` (clamped to 0..1) and keep the stops sorted.
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

// The scientific colour maps come from the `colorgrad` crate, which carries
// the published control points (matplotlib's viridis family, ColorBrewer,
// Google's turbo, cubehelix and more) and fits a smooth basis spline through
// them. The hand-picked five or six stops these used to have drifted
// visibly from the real maps between the stops.

/// The `colorgrad` crate, re-exported so its gradients can be passed to
/// [`Gradient::from_colorgrad`] without a version mismatch.
pub use colorgrad;

/// Every name [`preset_gradient`] accepts.
pub const PRESET_GRADIENTS: &[&str] = &[
    // perceptually uniform, sequential
    "viridis", "inferno", "magma", "plasma", "cividis", "turbo",
    // cyclic and rainbow
    "sinebow", "rainbow", "cubehelix", "warm", "cool",
    // ColorBrewer diverging
    "spectral", "rd_bu", "rd_yl_bu", "rd_yl_gn", "br_bg", "pr_gn", "pi_yg", "pu_or", "rd_gy",
    // ColorBrewer sequential
    "blues", "greens", "greys", "oranges", "purples", "reds",
    "bu_gn", "bu_pu", "gn_bu", "or_rd", "pu_bu_gn", "pu_bu", "pu_rd", "rd_pu",
    "yl_gn_bu", "yl_gn", "yl_or_br", "yl_or_rd",
];

/// Stops baked from a preset. Enough that piecewise-linear sampling stays
/// within one 8-bit step of the spline.
const PRESET_STOPS: usize = 64;

/// A named colour map, baked into a [`Gradient`] (see [`PRESET_GRADIENTS`]).
///
/// Names are matched case-insensitively, and `-` or spaces may stand in for
/// `_`, so `"RdYlBu"`-style names work as `"rd-yl-bu"`.
///
/// ```rust
/// use proof_engine::math::color::preset_gradient;
/// let turbo = preset_gradient("turbo").unwrap();
/// let lut = turbo.bake_lut(256); // ready to upload as a 1D texture
/// assert_eq!(lut.len(), 256);
/// assert!(preset_gradient("no such map").is_none());
/// ```
pub fn preset_gradient(name: &str) -> Option<Gradient> {
    use colorgrad::preset as p;
    let key = name.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let n = PRESET_STOPS;
    Some(match key.as_str() {
        "viridis" => Gradient::from_colorgrad(&p::viridis(), n),
        "inferno" => Gradient::from_colorgrad(&p::inferno(), n),
        "magma" => Gradient::from_colorgrad(&p::magma(), n),
        "plasma" => Gradient::from_colorgrad(&p::plasma(), n),
        "cividis" => Gradient::from_colorgrad(&p::cividis(), n),
        "turbo" => Gradient::from_colorgrad(&p::turbo(), n),
        "sinebow" => Gradient::from_colorgrad(&p::sinebow(), n),
        "rainbow" => Gradient::from_colorgrad(&p::rainbow(), n),
        "cubehelix" => Gradient::from_colorgrad(&p::cubehelix_default(), n),
        "warm" => Gradient::from_colorgrad(&p::warm(), n),
        "cool" => Gradient::from_colorgrad(&p::cool(), n),
        "spectral" => Gradient::from_colorgrad(&p::spectral(), n),
        "rd_bu" => Gradient::from_colorgrad(&p::rd_bu(), n),
        "rd_yl_bu" => Gradient::from_colorgrad(&p::rd_yl_bu(), n),
        "rd_yl_gn" => Gradient::from_colorgrad(&p::rd_yl_gn(), n),
        "br_bg" => Gradient::from_colorgrad(&p::br_bg(), n),
        "pr_gn" => Gradient::from_colorgrad(&p::pr_gn(), n),
        "pi_yg" => Gradient::from_colorgrad(&p::pi_yg(), n),
        "pu_or" => Gradient::from_colorgrad(&p::pu_or(), n),
        "rd_gy" => Gradient::from_colorgrad(&p::rd_gy(), n),
        "blues" => Gradient::from_colorgrad(&p::blues(), n),
        "greens" => Gradient::from_colorgrad(&p::greens(), n),
        "greys" | "grays" => Gradient::from_colorgrad(&p::greys(), n),
        "oranges" => Gradient::from_colorgrad(&p::oranges(), n),
        "purples" => Gradient::from_colorgrad(&p::purples(), n),
        "reds" => Gradient::from_colorgrad(&p::reds(), n),
        "bu_gn" => Gradient::from_colorgrad(&p::bu_gn(), n),
        "bu_pu" => Gradient::from_colorgrad(&p::bu_pu(), n),
        "gn_bu" => Gradient::from_colorgrad(&p::gn_bu(), n),
        "or_rd" => Gradient::from_colorgrad(&p::or_rd(), n),
        "pu_bu_gn" => Gradient::from_colorgrad(&p::pu_bu_gn(), n),
        "pu_bu" => Gradient::from_colorgrad(&p::pu_bu(), n),
        "pu_rd" => Gradient::from_colorgrad(&p::pu_rd(), n),
        "rd_pu" => Gradient::from_colorgrad(&p::rd_pu(), n),
        "yl_gn_bu" => Gradient::from_colorgrad(&p::yl_gn_bu(), n),
        "yl_gn" => Gradient::from_colorgrad(&p::yl_gn(), n),
        "yl_or_br" => Gradient::from_colorgrad(&p::yl_or_br(), n),
        "yl_or_rd" => Gradient::from_colorgrad(&p::yl_or_rd(), n),
        _ => return None,
    })
}

impl Gradient {
    /// Bake any `colorgrad` gradient into `samples` evenly spaced stops,
    /// interpolated in [`GradientMode::LinearRgb`] between them.
    ///
    /// Colours keep the values `colorgrad` gives, the same 0 to 1 encoding
    /// as [`Rgba::from_hex`].
    pub fn from_colorgrad<G: colorgrad::Gradient + ?Sized>(g: &G, samples: usize) -> Gradient {
        let (lo, hi) = g.domain();
        let n = samples.max(2);
        let mut out = Gradient::new(GradientMode::LinearRgb);
        out.stops = (0..n)
            .map(|i| {
                let t = i as f32 / (n - 1) as f32;
                let c = g.at(lo + (hi - lo) * t);
                // Basis splines can overshoot by a rounding error.
                let c = c.clamp();
                ColorStop { t, color: Rgba::new(c.r, c.g, c.b, c.a) }
            })
            .collect();
        out
    }

    /// Parse a CSS-style gradient, as written inside `linear-gradient()`:
    /// colour names, hex, `rgb()`, `hsl()` and so on, with optional
    /// percentage positions.
    ///
    /// ```rust
    /// use proof_engine::math::color::Gradient;
    /// let g = Gradient::from_css("#000, deeppink 40%, gold").unwrap();
    /// let mid = g.sample(0.4);
    /// assert!(mid.r > 0.99 && mid.g < 0.1);
    /// assert!(Gradient::from_css("not a colour, at all").is_err());
    /// ```
    pub fn from_css(css: &str) -> Result<Gradient, String> {
        let g = colorgrad::GradientBuilder::new()
            .css(css)
            .build::<colorgrad::LinearGradient>()
            .map_err(|e| format!("bad gradient {css:?}: {e}"))?;
        // Linear in RGB between its stops, so 256 samples reproduce it to
        // within a quarter of a percent of the width, hard stops included.
        Ok(Gradient::from_colorgrad(&g, 256))
    }
}

/// Matplotlib's plasma.
pub fn gradient_plasma() -> Gradient {
    preset_gradient("plasma").expect("built-in preset")
}

/// Matplotlib's inferno.
pub fn gradient_inferno() -> Gradient {
    preset_gradient("inferno").expect("built-in preset")
}

/// Matplotlib's viridis.
pub fn gradient_viridis() -> Gradient {
    preset_gradient("viridis").expect("built-in preset")
}

/// Black through dark red, orange and yellow to white.
pub fn gradient_fire() -> Gradient {
    Gradient::new(GradientMode::LinearRgb)
        .add_stop(0.0, Rgba::BLACK)
        .add_stop(0.3, Rgba::rgb(0.5, 0.0, 0.0))
        .add_stop(0.6, Rgba::rgb(1.0, 0.3, 0.0))
        .add_stop(0.8, Rgba::rgb(1.0, 0.8, 0.0))
        .add_stop(1.0, Rgba::WHITE)
}

/// Black through deep blue and sky blue to white, blended in Oklab.
pub fn gradient_ice() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::BLACK)
        .add_stop(0.4, Rgba::rgb(0.0, 0.2, 0.5))
        .add_stop(0.7, Rgba::rgb(0.2, 0.6, 1.0))
        .add_stop(1.0, Rgba::WHITE)
}

/// Magenta to cyan and back to magenta, blended in Oklab.
pub fn gradient_neon() -> Gradient {
    Gradient::new(GradientMode::Oklab)
        .add_stop(0.0, Rgba::from_hex(0xff00ff))
        .add_stop(0.5, Rgba::from_hex(0x00ffff))
        .add_stop(1.0, Rgba::from_hex(0xff00ff))
}

/// Health bar colours: red at 0, amber at 0.5, green at 1.
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
    /// Display name.
    pub name:   String,
    /// The colours, in order.
    pub colors: Vec<Rgba>,
}

impl Palette {
    /// A palette called `name` with these colours.
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

    fn close(a: Rgba, hex: u32) -> bool {
        let b = Rgba::from_hex(hex);
        (a.r - b.r).abs() < 0.02 && (a.g - b.g).abs() < 0.02 && (a.b - b.b).abs() < 0.02
    }

    #[test]
    fn presets_hit_the_published_end_points() {
        // viridis runs from #440154 to #fde725, plasma from #0d0887 to
        // #f0f921, inferno from #000004 to #fcffa4.
        let v = gradient_viridis();
        assert!(close(v.sample(0.0), 0x440154) && close(v.sample(1.0), 0xfde725));
        let p = gradient_plasma();
        assert!(close(p.sample(0.0), 0x0d0887) && close(p.sample(1.0), 0xf0f921));
        let i = gradient_inferno();
        assert!(close(i.sample(0.0), 0x000004) && close(i.sample(1.0), 0xfcffa4));
        // viridis's published midpoint is the teal #21918c. The old
        // five-stop version gave #35b779 there, 0.15 off in green; the
        // colorgrad fit is within 0.06 on every channel.
        let (m, want) = (v.sample(0.5), Rgba::from_hex(0x21918c));
        let err = (m.r - want.r).abs().max((m.g - want.g).abs()).max((m.b - want.b).abs());
        assert!(err < 0.06, "{m:?} is {err} from #21918c");
    }

    #[test]
    fn every_listed_preset_resolves_and_is_well_formed() {
        for name in PRESET_GRADIENTS {
            let g = preset_gradient(name).unwrap_or_else(|| panic!("{name} missing"));
            assert_eq!(g.stops.len(), PRESET_STOPS);
            for c in g.bake_lut(17) {
                for ch in [c.r, c.g, c.b, c.a] {
                    assert!((0.0..=1.0).contains(&ch), "{name}: {ch}");
                }
            }
        }
        assert!(preset_gradient("Rd-Yl-Bu").is_some());
        assert!(preset_gradient("nope").is_none());
    }

    #[test]
    fn css_gradients_parse_with_positions_and_hard_stops() {
        let g = Gradient::from_css("red, red 50%, blue 50%, blue").unwrap();
        assert!(close(g.sample(0.25), 0xff0000));
        assert!(close(g.sample(0.75), 0x0000ff));
        let g = Gradient::from_css("#000, #fff").unwrap();
        assert!((g.sample(0.5).r - 0.5).abs() < 0.01);
        assert!(Gradient::from_css("").is_err() || Gradient::from_css("").unwrap().stops.len() >= 2);
        assert!(Gradient::from_css("bogus, nonsense").is_err());
    }

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
