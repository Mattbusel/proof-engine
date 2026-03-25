//! 40+ easing functions following Robert Penner's equations.
//!
//! Each variant describes an acceleration curve over t ∈ [0, 1] → [0, 1].
//! Use `Easing::apply(t)` to convert normalized time to a curved value.

use std::f32::consts::{PI, TAU};

/// An easing function — transforms normalized time t ∈ [0, 1] → output ∈ [0, 1].
///
/// For typical usage, In = slow start, Out = slow end, InOut = slow at both ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    // ── Polynomial ───────────────────────────────────────────────────────────
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,

    // ── Trigonometric ─────────────────────────────────────────────────────────
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,

    // ── Exponential ───────────────────────────────────────────────────────────
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,

    // ── Circular ─────────────────────────────────────────────────────────────
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,

    // ── Back (overshoot) ──────────────────────────────────────────────────────
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,

    // ── Elastic ───────────────────────────────────────────────────────────────
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,

    // ── Bounce ────────────────────────────────────────────────────────────────
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,

    // ── Special ───────────────────────────────────────────────────────────────
    /// Smooth step: 3t² - 2t³ (zero derivative at endpoints).
    SmoothStep,
    /// Smoother step: 6t⁵ - 15t⁴ + 10t³ (Perlin's improved smoothstep).
    SmootherStep,
    /// Step at threshold 0.5 (instantaneous jump).
    Step,
    /// Instant-in, linear-out.
    EaseOutLinear,
    /// Hermite cubic through two tangents.
    Hermite { p0: f32, m0: f32, p1: f32, m1: f32 },
    /// Custom power: t^n.
    Power(f32),
    /// Sigmoid (logistic) curve.
    Sigmoid { k: f32 },
    /// Spring easing (underdamped oscillation to rest).
    Spring { stiffness: f32, damping: f32 },
    /// Parabolic arc (projectile).
    Parabola,
    /// Flash: instant full, then linear decay.
    Flash,
}

impl Easing {
    /// Apply the easing function to normalized time `t` ∈ [0, 1].
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            // ── Linear ───────────────────────────────────────────────────────
            Easing::Linear => t,

            // ── Quadratic ────────────────────────────────────────────────────
            Easing::EaseInQuad      => t * t,
            Easing::EaseOutQuad     => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOutQuad   => {
                if t < 0.5 { 2.0 * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            }

            // ── Cubic ────────────────────────────────────────────────────────
            Easing::EaseInCubic     => t * t * t,
            Easing::EaseOutCubic    => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOutCubic  => {
                if t < 0.5 { 4.0 * t * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
            }

            // ── Quartic ──────────────────────────────────────────────────────
            Easing::EaseInQuart     => t * t * t * t,
            Easing::EaseOutQuart    => 1.0 - (1.0 - t).powi(4),
            Easing::EaseInOutQuart  => {
                if t < 0.5 { 8.0 * t * t * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(4) / 2.0 }
            }

            // ── Quintic ──────────────────────────────────────────────────────
            Easing::EaseInQuint     => t * t * t * t * t,
            Easing::EaseOutQuint    => 1.0 - (1.0 - t).powi(5),
            Easing::EaseInOutQuint  => {
                if t < 0.5 { 16.0 * t * t * t * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(5) / 2.0 }
            }

            // ── Sine ─────────────────────────────────────────────────────────
            Easing::EaseInSine      => 1.0 - (t * PI / 2.0).cos(),
            Easing::EaseOutSine     => (t * PI / 2.0).sin(),
            Easing::EaseInOutSine   => -((PI * t).cos() - 1.0) / 2.0,

            // ── Exponential ──────────────────────────────────────────────────
            Easing::EaseInExpo  => {
                if t == 0.0 { 0.0 } else { 2.0_f32.powf(10.0 * t - 10.0) }
            }
            Easing::EaseOutExpo => {
                if t == 1.0 { 1.0 } else { 1.0 - 2.0_f32.powf(-10.0 * t) }
            }
            Easing::EaseInOutExpo => {
                if t == 0.0 { return 0.0; }
                if t == 1.0 { return 1.0; }
                if t < 0.5 { 2.0_f32.powf(20.0 * t - 10.0) / 2.0 }
                else { (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0 }
            }

            // ── Circular ─────────────────────────────────────────────────────
            Easing::EaseInCirc  => 1.0 - (1.0 - t * t).sqrt(),
            Easing::EaseOutCirc => (1.0 - (t - 1.0) * (t - 1.0)).sqrt(),
            Easing::EaseInOutCirc => {
                if t < 0.5 { (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0 }
                else { ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0 }
            }

            // ── Back (overshoot) ─────────────────────────────────────────────
            Easing::EaseInBack => {
                let c1 = 1.70158_f32;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Easing::EaseOutBack => {
                let c1 = 1.70158_f32;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Easing::EaseInOutBack => {
                let c1 = 1.70158_f32;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (2.0 * t - 2.0) + c2) + 2.0) / 2.0
                }
            }

            // ── Elastic ──────────────────────────────────────────────────────
            Easing::EaseInElastic => {
                if t == 0.0 { return 0.0; }
                if t == 1.0 { return 1.0; }
                let c4 = TAU / 3.0;
                -2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c4).sin()
            }
            Easing::EaseOutElastic => {
                if t == 0.0 { return 0.0; }
                if t == 1.0 { return 1.0; }
                let c4 = TAU / 3.0;
                2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
            }
            Easing::EaseInOutElastic => {
                if t == 0.0 { return 0.0; }
                if t == 1.0 { return 1.0; }
                let c5 = TAU / 4.5;
                if t < 0.5 {
                    -(2.0_f32.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
                } else {
                    (2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0 + 1.0
                }
            }

            // ── Bounce ───────────────────────────────────────────────────────
            Easing::EaseOutBounce => bounce_out(t),
            Easing::EaseInBounce  => 1.0 - bounce_out(1.0 - t),
            Easing::EaseInOutBounce => {
                if t < 0.5 { (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0 }
                else { (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0 }
            }

            // ── Special ──────────────────────────────────────────────────────
            Easing::SmoothStep    => t * t * (3.0 - 2.0 * t),
            Easing::SmootherStep  => t * t * t * (t * (t * 6.0 - 15.0) + 10.0),
            Easing::Step          => if t < 0.5 { 0.0 } else { 1.0 },
            Easing::EaseOutLinear => t,
            Easing::Parabola      => 4.0 * t * (1.0 - t),
            Easing::Flash         => if t < f32::EPSILON { 1.0 } else { 1.0 - t },

            Easing::Power(n)      => t.powf(n),

            Easing::Sigmoid { k } => {
                let s = |x: f32| 1.0 / (1.0 + (-k * x).exp());
                (s(t - 0.5) - s(-0.5)) / (s(0.5) - s(-0.5))
            }

            Easing::Spring { stiffness, damping } => {
                // Underdamped spring analytical solution
                let omega = stiffness.sqrt();
                let zeta  = damping / (2.0 * omega).max(f32::EPSILON);
                if zeta >= 1.0 {
                    1.0 - (1.0 + omega * t) * (-omega * t).exp()
                } else {
                    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
                    let decay   = (-zeta * omega * t).exp();
                    1.0 - decay * ((omega_d * t).cos() +
                          (zeta / (1.0 - zeta * zeta).sqrt()) * (omega_d * t).sin())
                }
            }

            Easing::Hermite { p0, m0, p1, m1 } => {
                let t2 = t * t;
                let t3 = t2 * t;
                let h00 =  2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 =        t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 =        t3 -       t2;
                h00 * p0 + h10 * m0 + h01 * p1 + h11 * m1
            }
        }
    }

    /// Return the approximate derivative dE/dt at `t` using central differences.
    pub fn derivative(&self, t: f32) -> f32 {
        let eps = 1e-4_f32;
        let hi = self.apply((t + eps).min(1.0));
        let lo = self.apply((t - eps).max(0.0));
        (hi - lo) / (2.0 * eps)
    }

    /// Name for debug display.
    pub fn name(&self) -> &'static str {
        match self {
            Easing::Linear          => "Linear",
            Easing::EaseInQuad      => "EaseInQuad",
            Easing::EaseOutQuad     => "EaseOutQuad",
            Easing::EaseInOutQuad   => "EaseInOutQuad",
            Easing::EaseInCubic     => "EaseInCubic",
            Easing::EaseOutCubic    => "EaseOutCubic",
            Easing::EaseInOutCubic  => "EaseInOutCubic",
            Easing::EaseInQuart     => "EaseInQuart",
            Easing::EaseOutQuart    => "EaseOutQuart",
            Easing::EaseInOutQuart  => "EaseInOutQuart",
            Easing::EaseInQuint     => "EaseInQuint",
            Easing::EaseOutQuint    => "EaseOutQuint",
            Easing::EaseInOutQuint  => "EaseInOutQuint",
            Easing::EaseInSine      => "EaseInSine",
            Easing::EaseOutSine     => "EaseOutSine",
            Easing::EaseInOutSine   => "EaseInOutSine",
            Easing::EaseInExpo      => "EaseInExpo",
            Easing::EaseOutExpo     => "EaseOutExpo",
            Easing::EaseInOutExpo   => "EaseInOutExpo",
            Easing::EaseInCirc      => "EaseInCirc",
            Easing::EaseOutCirc     => "EaseOutCirc",
            Easing::EaseInOutCirc   => "EaseInOutCirc",
            Easing::EaseInBack      => "EaseInBack",
            Easing::EaseOutBack     => "EaseOutBack",
            Easing::EaseInOutBack   => "EaseInOutBack",
            Easing::EaseInElastic   => "EaseInElastic",
            Easing::EaseOutElastic  => "EaseOutElastic",
            Easing::EaseInOutElastic => "EaseInOutElastic",
            Easing::EaseInBounce    => "EaseInBounce",
            Easing::EaseOutBounce   => "EaseOutBounce",
            Easing::EaseInOutBounce => "EaseInOutBounce",
            Easing::SmoothStep      => "SmoothStep",
            Easing::SmootherStep    => "SmootherStep",
            Easing::Step            => "Step",
            Easing::EaseOutLinear   => "EaseOutLinear",
            Easing::Parabola        => "Parabola",
            Easing::Flash           => "Flash",
            Easing::Power(_)        => "Power",
            Easing::Sigmoid { .. }  => "Sigmoid",
            Easing::Spring { .. }   => "Spring",
            Easing::Hermite { .. }  => "Hermite",
        }
    }

    /// All named easings (excludes parameterized variants).
    pub fn all_named() -> &'static [Easing] {
        &[
            Easing::Linear,
            Easing::EaseInQuad, Easing::EaseOutQuad, Easing::EaseInOutQuad,
            Easing::EaseInCubic, Easing::EaseOutCubic, Easing::EaseInOutCubic,
            Easing::EaseInQuart, Easing::EaseOutQuart, Easing::EaseInOutQuart,
            Easing::EaseInQuint, Easing::EaseOutQuint, Easing::EaseInOutQuint,
            Easing::EaseInSine, Easing::EaseOutSine, Easing::EaseInOutSine,
            Easing::EaseInExpo, Easing::EaseOutExpo, Easing::EaseInOutExpo,
            Easing::EaseInCirc, Easing::EaseOutCirc, Easing::EaseInOutCirc,
            Easing::EaseInBack, Easing::EaseOutBack, Easing::EaseInOutBack,
            Easing::EaseInElastic, Easing::EaseOutElastic, Easing::EaseInOutElastic,
            Easing::EaseInBounce, Easing::EaseOutBounce, Easing::EaseInOutBounce,
            Easing::SmoothStep, Easing::SmootherStep, Easing::Step,
            // Parabola and Flash are special-purpose easings that don't satisfy 0→1
        ]
    }
}

/// Shared bounce-out helper (used by In/InOut variants too).
fn bounce_out(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;
    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t2 = t - 1.5 / D1;
        N1 * t2 * t2 + 0.75
    } else if t < 2.5 / D1 {
        let t2 = t - 2.25 / D1;
        N1 * t2 * t2 + 0.9375
    } else {
        let t2 = t - 2.625 / D1;
        N1 * t2 * t2 + 0.984375
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_endpoints() {
        assert!((Easing::Linear.apply(0.0)).abs() < 1e-6);
        assert!((Easing::Linear.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn all_easings_start_at_zero_end_at_one() {
        let parameterized = [
            Easing::Power(2.5),
            Easing::Sigmoid { k: 5.0 },
            Easing::Spring { stiffness: 100.0, damping: 10.0 },
            Easing::SmoothStep,
            Easing::SmootherStep,
        ];
        for e in Easing::all_named().iter().chain(parameterized.iter()) {
            let start = e.apply(0.0);
            let end   = e.apply(1.0);
            assert!((start).abs() < 1e-4, "{} start={}", e.name(), start);
            assert!((end - 1.0).abs() < 1e-4, "{} end={}", e.name(), end);
        }
    }

    #[test]
    fn bounce_is_monotonic_at_end() {
        let prev = Easing::EaseOutBounce.apply(0.99);
        let curr = Easing::EaseOutBounce.apply(1.00);
        assert!(curr >= prev - 1e-4);
    }

    #[test]
    fn spring_overshoots() {
        let e = Easing::Spring { stiffness: 200.0, damping: 5.0 };
        let max = (0..200).map(|i| (e.apply(i as f32 / 100.0) * 1000.0) as i32).max().unwrap();
        assert!(max > 1000, "spring should overshoot past 1.0 (max={})", max);
    }

    #[test]
    fn hermite_through_endpoints() {
        let e = Easing::Hermite { p0: 0.0, m0: 1.0, p1: 1.0, m1: 1.0 };
        assert!((e.apply(0.0) - 0.0).abs() < 1e-5);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn parabola_peaks_at_half() {
        let e = Easing::Parabola;
        assert!((e.apply(0.5) - 1.0).abs() < 1e-5);
        assert!(e.apply(0.0).abs() < 1e-5);
        assert!(e.apply(1.0).abs() < 1e-5);
    }
}
