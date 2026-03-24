//! Mathematical primitives: functions, force fields, attractors.
//!
//! Every visual property in Proof Engine is driven by a `MathFunction`.
//! Every spatial interaction is modeled as a `ForceField`.
//! No keyframes. No tweens. Only functions.

pub mod eval;
pub mod attractors;
pub mod noise;
pub mod springs;
pub mod fields;
pub mod color;

pub use eval::MathFunction;
pub use fields::{ForceField, Falloff, FieldTarget};
pub use attractors::AttractorType;
pub use springs::{SpringDamper, Spring3D, SpringDamper3};


/// Evaluate a scalar function of time.
/// `t` = elapsed seconds since engine start.
/// `input` = chained input value (for composed functions).
pub fn evaluate(f: &MathFunction, t: f32, input: f32) -> f32 {
    f.evaluate(t, input)
}

/// Map a value from [in_min, in_max] to [out_min, out_max].
#[inline]
pub fn remap(v: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (v - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

/// Smooth step (3t² - 2t³).
#[inline]
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smoother step (6t⁵ - 15t⁴ + 10t³).
#[inline]
pub fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Lerp two colors.
#[inline]
pub fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}

/// Hsv to RGB.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 { return (v, v, v); }
    let h = h % 360.0;
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
