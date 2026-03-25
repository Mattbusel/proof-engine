//! Tessellate mathematical curves into polylines for rendering.
//!
//! Each curve type has its own evaluation function. The tessellator
//! samples the curve at N points and returns a Vec<Vec2> polyline.

use glam::Vec2;
use super::entity_curves::{EntityCurve, CurveType};
use crate::math::MathFunction;
use std::f32::consts::{PI, TAU};

/// Tessellate a curve into a polyline.
pub fn tessellate_curve(curve: &EntityCurve) -> Vec<Vec2> {
    if !curve.alive && curve.point_velocities.iter().all(|v| v.length() < 0.001) {
        return Vec::new(); // fully dissolved, nothing to draw
    }

    let n = curve.segment_count.max(4) as usize;

    match &curve.curve_type {
        CurveType::Bezier { degree } => tessellate_bezier(&curve.control_points, *degree, n),
        CurveType::Lissajous { a, b, delta } => tessellate_lissajous(*a, *b, *delta, &curve.control_points, n),
        CurveType::Parametric { x_fn, y_fn } => tessellate_parametric(x_fn, y_fn, &curve.control_points, n),
        CurveType::Circle { radius, distortion } => tessellate_circle(*radius, distortion.as_ref(), &curve.control_points, n),
        CurveType::Spiral { rate, decay } => tessellate_spiral(*rate, *decay, &curve.control_points, n),
        CurveType::Rose { k, amplitude } => tessellate_rose(*k, *amplitude, &curve.control_points, n),
        CurveType::Hypotrochoid { big_r, small_r, d } => tessellate_hypotrochoid(*big_r, *small_r, *d, &curve.control_points, n),
        CurveType::Superellipse { a, b, n: exp } => tessellate_superellipse(*a, *b, *exp, &curve.control_points, n),
        CurveType::Catenary { a, span } => tessellate_catenary(*a, *span, &curve.control_points, n),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Bezier (De Casteljau algorithm)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_bezier(points: &[Vec2], _degree: u32, num_samples: usize) -> Vec<Vec2> {
    if points.len() < 2 { return points.to_vec(); }

    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = i as f32 / num_samples as f32;
        result.push(de_casteljau(points, t));
    }
    result
}

/// De Casteljau's algorithm for evaluating a Bezier curve at parameter t.
fn de_casteljau(points: &[Vec2], t: f32) -> Vec2 {
    if points.len() == 1 { return points[0]; }
    let mut work = points.to_vec();
    let n = work.len();
    for level in 1..n {
        for i in 0..n - level {
            work[i] = work[i] * (1.0 - t) + work[i + 1] * t;
        }
    }
    work[0]
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Lissajous: x = A*sin(a*t + delta), y = B*sin(b*t)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_lissajous(a: f32, b: f32, delta: f32, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    let scale = anchors.get(1).copied().unwrap_or(Vec2::ONE);

    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = TAU * i as f32 / num_samples as f32;
        let x = center.x + scale.x * (a * t + delta).sin();
        let y = center.y + scale.y * (b * t).sin();
        result.push(Vec2::new(x, y));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Parametric: x(t), y(t) from MathFunction pair
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_parametric(x_fn: &MathFunction, y_fn: &MathFunction, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = i as f32 / num_samples as f32;
        let x = center.x + x_fn.evaluate(t, 0.0);
        let y = center.y + y_fn.evaluate(t, 0.0);
        result.push(Vec2::new(x, y));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Circle with optional distortion
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_circle(radius: f32, distortion: Option<&MathFunction>, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = TAU * i as f32 / num_samples as f32;
        let r = if let Some(dist_fn) = distortion {
            radius + dist_fn.evaluate(t, 0.0) * radius * 0.3
        } else {
            radius
        };
        result.push(Vec2::new(center.x + r * t.cos(), center.y + r * t.sin()));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Spiral: r = rate * theta * exp(-decay * theta)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_spiral(rate: f32, decay: f32, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    let max_theta = TAU * 3.0; // 3 full turns
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let theta = max_theta * i as f32 / num_samples as f32;
        let r = rate * theta * (-decay * theta).exp();
        result.push(Vec2::new(center.x + r * theta.cos(), center.y + r * theta.sin()));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Rose curve: r = amplitude * cos(k * theta)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_rose(k: f32, amplitude: f32, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    // Rose with integer k closes in PI, with rational k closes in 2*PI*denominator
    let max_theta = if (k - k.round()).abs() < 0.01 { PI } else { TAU * 2.0 };
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let theta = max_theta * i as f32 / num_samples as f32;
        let r = amplitude * (k * theta).cos();
        result.push(Vec2::new(center.x + r * theta.cos(), center.y + r * theta.sin()));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Hypotrochoid: point on a small circle rolling inside a big one
// x = (R-r)*cos(t) + d*cos((R-r)/r * t)
// y = (R-r)*sin(t) - d*sin((R-r)/r * t)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_hypotrochoid(big_r: f32, small_r: f32, d: f32, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    let diff = big_r - small_r;
    let ratio = diff / small_r.max(0.001);
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = TAU * i as f32 / num_samples as f32;
        let x = center.x + diff * t.cos() + d * (ratio * t).cos();
        let y = center.y + diff * t.sin() - d * (ratio * t).sin();
        result.push(Vec2::new(x, y));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Superellipse: |x/a|^n + |y/b|^n = 1
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_superellipse(a: f32, b: f32, n: f32, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let center = anchors.first().copied().unwrap_or(Vec2::ZERO);
    let exp = 2.0 / n.max(0.01);
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = TAU * i as f32 / num_samples as f32;
        let cos_t = t.cos();
        let sin_t = t.sin();
        let x = a * cos_t.abs().powf(exp) * cos_t.signum();
        let y = b * sin_t.abs().powf(exp) * sin_t.signum();
        result.push(Vec2::new(center.x + x, center.y + y));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Catenary: y = a * cosh(x/a)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn tessellate_catenary(a: f32, span: f32, anchors: &[Vec2], num_samples: usize) -> Vec<Vec2> {
    let start = anchors.first().copied().unwrap_or(Vec2::new(-span * 0.5, 0.0));
    let mut result = Vec::with_capacity(num_samples + 1);
    for i in 0..=num_samples {
        let t = i as f32 / num_samples as f32;
        let x = start.x + t * span;
        let x_centered = (t - 0.5) * span;
        let y = start.y - a * (x_centered / a.max(0.01)).cosh() + a;
        result.push(Vec2::new(x, y));
    }
    result
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Utility: polyline arc length
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Total arc length of a polyline.
pub fn polyline_length(points: &[Vec2]) -> f32 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// Resample a polyline to have evenly-spaced points.
pub fn resample_polyline(points: &[Vec2], num_output: usize) -> Vec<Vec2> {
    if points.len() < 2 || num_output < 2 { return points.to_vec(); }
    let total_len = polyline_length(points);
    if total_len < 1e-6 { return vec![points[0]; num_output]; }
    let segment_len = total_len / (num_output - 1) as f32;

    let mut result = Vec::with_capacity(num_output);
    result.push(points[0]);
    let mut accumulated = 0.0f32;
    let mut target = segment_len;
    let mut src_idx = 0;

    for _ in 1..num_output - 1 {
        while src_idx < points.len() - 1 {
            let seg_len = (points[src_idx + 1] - points[src_idx]).length();
            if accumulated + seg_len >= target {
                let t = (target - accumulated) / seg_len.max(1e-6);
                result.push(points[src_idx] + (points[src_idx + 1] - points[src_idx]) * t);
                target += segment_len;
                break;
            }
            accumulated += seg_len;
            src_idx += 1;
        }
    }
    result.push(*points.last().unwrap());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_endpoints() {
        let pts = vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)];
        let curve = EntityCurve::new(CurveType::Bezier { degree: 2 }, pts.clone());
        let poly = tessellate_curve(&curve);
        assert!((poly[0] - pts[0]).length() < 0.01, "should start at first control point");
        assert!((poly.last().unwrap() - pts[2]).length() < 0.01, "should end at last");
    }

    #[test]
    fn test_circle_closed() {
        let curve = EntityCurve::new(CurveType::Circle { radius: 1.0, distortion: None }, vec![Vec2::ZERO]);
        let poly = tessellate_curve(&curve);
        assert!((poly[0] - poly.last().unwrap()).length() < 0.01, "circle should close");
    }

    #[test]
    fn test_lissajous_samples() {
        let curve = EntityCurve::new(
            CurveType::Lissajous { a: 3.0, b: 2.0, delta: std::f32::consts::FRAC_PI_2 },
            vec![Vec2::ZERO, Vec2::ONE],
        );
        let poly = tessellate_curve(&curve);
        assert!(poly.len() > 10);
    }

    #[test]
    fn test_rose_curve() {
        let curve = EntityCurve::new(CurveType::Rose { k: 5.0, amplitude: 1.0 }, vec![Vec2::ZERO]);
        let poly = tessellate_curve(&curve);
        assert!(poly.len() > 10);
        // All points should be within amplitude
        for p in &poly { assert!(p.length() <= 1.1); }
    }

    #[test]
    fn test_hypotrochoid() {
        let curve = EntityCurve::new(CurveType::Hypotrochoid { big_r: 5.0, small_r: 3.0, d: 3.0 }, vec![Vec2::ZERO]);
        let poly = tessellate_curve(&curve);
        assert!(poly.len() > 10);
    }

    #[test]
    fn test_polyline_resample() {
        let pts = vec![Vec2::ZERO, Vec2::new(10.0, 0.0)];
        let resampled = resample_polyline(&pts, 11);
        assert_eq!(resampled.len(), 11);
        assert!((resampled[5].x - 5.0).abs() < 0.1);
    }
}
