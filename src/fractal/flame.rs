//! Flame fractals (Scott Draves algorithm) — nonlinear IFS with color mapping.

use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlameVariation {
    Linear, Sinusoidal, Spherical, Swirl, Horseshoe, Polar, Handkerchief,
    Heart, Disc, Spiral, Hyperbolic, Diamond, Ex, Julia, Bent, Fisheye,
}

impl FlameVariation {
    pub fn apply(&self, p: Vec2) -> Vec2 {
        let (x, y) = (p.x, p.y);
        let r = p.length().max(1e-10);
        let theta = y.atan2(x);
        match self {
            Self::Linear => p,
            Self::Sinusoidal => Vec2::new(x.sin(), y.sin()),
            Self::Spherical => p / (r * r),
            Self::Swirl => { let r2 = r * r; Vec2::new(x * r2.sin() - y * r2.cos(), x * r2.cos() + y * r2.sin()) }
            Self::Horseshoe => Vec2::new((x - y) * (x + y) / r, 2.0 * x * y / r),
            Self::Polar => Vec2::new(theta / std::f32::consts::PI, r - 1.0),
            Self::Handkerchief => Vec2::new(r * (theta + r).sin(), r * (theta - r).cos()),
            Self::Heart => Vec2::new(r * (theta * r).sin(), -r * (theta * r).cos()),
            Self::Disc => { let tr = theta / std::f32::consts::PI; Vec2::new(tr * (std::f32::consts::PI * r).sin(), tr * (std::f32::consts::PI * r).cos()) }
            Self::Spiral => Vec2::new((theta.cos() + r.sin()) / r, (theta.sin() - r.cos()) / r),
            Self::Hyperbolic => Vec2::new(theta.sin() / r, r * theta.cos()),
            Self::Diamond => Vec2::new(theta.sin() * r.cos(), theta.cos() * r.sin()),
            Self::Ex => { let p0 = (theta + r).sin().powi(3); let p1 = (theta - r).cos().powi(3); Vec2::new(r * (p0 + p1), r * (p0 - p1)) }
            Self::Julia => { let sr = r.sqrt(); let omega = if (theta * 1000.0) as i32 % 2 == 0 { 0.0 } else { std::f32::consts::PI }; Vec2::new(sr * (theta / 2.0 + omega).cos(), sr * (theta / 2.0 + omega).sin()) }
            Self::Bent => Vec2::new(if x >= 0.0 { x } else { 2.0 * x }, if y >= 0.0 { y } else { y / 2.0 }),
            Self::Fisheye => { let rr = 2.0 / (r + 1.0); Vec2::new(rr * y, rr * x) }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlameParams {
    pub variations: Vec<(FlameVariation, f32)>,
    pub color_speed: f32,
    pub iterations: u32,
    pub supersample: u32,
}

impl Default for FlameParams {
    fn default() -> Self {
        Self { variations: vec![(FlameVariation::Linear, 0.5), (FlameVariation::Sinusoidal, 0.5)], color_speed: 0.5, iterations: 100000, supersample: 1 }
    }
}

/// Render a flame fractal. Returns (x, y, color_index) tuples.
pub fn render_flame(params: &FlameParams, seed: u64) -> Vec<(Vec2, f32)> {
    let mut points = Vec::with_capacity(params.iterations as usize);
    let mut p = Vec2::new(0.1, 0.1);
    let mut color = 0.5f32;
    let mut rng = seed;

    for i in 0..params.iterations + 20 {
        // Pick a variation weighted randomly
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let total_w: f32 = params.variations.iter().map(|(_, w)| w).sum();
        let r = (rng >> 33) as f32 / (u32::MAX >> 1) as f32 * total_w;
        let mut cum = 0.0;
        let mut selected = &params.variations[0];
        for v in &params.variations {
            cum += v.1;
            if r <= cum { selected = v; break; }
        }

        p = selected.0.apply(p);
        color = color * (1.0 - params.color_speed) + params.color_speed * (selected.0 as u8 as f32 / 16.0);

        if i >= 20 { points.push((p, color)); }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flame_renders() {
        let params = FlameParams { iterations: 1000, ..Default::default() };
        let result = render_flame(&params, 42);
        assert_eq!(result.len(), 1000);
    }
    #[test]
    fn variations_dont_panic() {
        let p = Vec2::new(0.5, 0.3);
        for v in &[FlameVariation::Linear, FlameVariation::Spherical, FlameVariation::Swirl, FlameVariation::Heart, FlameVariation::Julia] {
            let _r = v.apply(p);
        }
    }
}
