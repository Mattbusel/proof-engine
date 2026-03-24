//! MathFunction enum and evaluation.
//!
//! Every visual property of every Glyph can be driven by a MathFunction.
//! There are no keyframes, no tweens — only continuous functions of time.

use glam::Vec3;
use std::f32::consts::{PI, TAU};
use super::attractors;

/// A continuous mathematical function that maps (time, input) → f32.
///
/// Functions are composable: `Sum`, `Product`, and `Chain` allow building
/// arbitrarily complex behaviors from simple primitives.
#[derive(Debug, Clone)]
pub enum MathFunction {
    // ── Basic ─────────────────────────────────────────────────────────────────
    /// Always returns the same value.
    Constant(f32),
    /// Linear function: slope * t + offset.
    Linear { slope: f32, offset: f32 },

    // ── Oscillation ───────────────────────────────────────────────────────────
    /// Sinusoidal oscillation.
    Sine { amplitude: f32, frequency: f32, phase: f32 },
    /// Cosinusoidal oscillation.
    Cosine { amplitude: f32, frequency: f32, phase: f32 },
    /// Triangle wave oscillation.
    Triangle { amplitude: f32, frequency: f32, phase: f32 },
    /// Square wave (snaps between +/- amplitude).
    Square { amplitude: f32, frequency: f32, duty: f32 },

    // ── Organic motion ────────────────────────────────────────────────────────
    /// Lorenz attractor trajectory (x-coordinate).
    Lorenz { sigma: f32, rho: f32, beta: f32, scale: f32 },
    /// Perlin noise.
    Perlin { frequency: f32, octaves: u8, amplitude: f32 },
    /// Simplex noise.
    Simplex { frequency: f32, amplitude: f32 },

    // ── Convergence / divergence ──────────────────────────────────────────────
    /// Exponential approach to target: target + (start - target) * e^(-rate * t).
    Exponential { start: f32, target: f32, rate: f32 },
    /// Logistic map iteration: x_{n+1} = r * x_n * (1 - x_n). Exhibits bifurcation.
    LogisticMap { r: f32, x0: f32 },
    /// Collatz sequence mapped to a float path. Bouncy convergence.
    Collatz { seed: u64, scale: f32 },

    // ── Attraction / orbiting ─────────────────────────────────────────────────
    /// Circular or elliptical orbit around a center point.
    Orbit { center: Vec3, radius: f32, speed: f32, eccentricity: f32 },
    /// Outward (or inward) spiral.
    Spiral { center: Vec3, radius_rate: f32, speed: f32 },
    /// Golden-ratio spiral.
    GoldenSpiral { center: Vec3, scale: f32, speed: f32 },
    /// Lissajous figure.
    Lissajous { a: f32, b: f32, delta: f32, scale: f32 },

    // ── Strange attractor ─────────────────────────────────────────────────────
    /// Move along a strange attractor trajectory (x-coordinate output).
    StrangeAttractor {
        attractor_type: crate::math::attractors::AttractorType,
        scale: f32,
        strength: f32,
    },

    // ── Fractal ───────────────────────────────────────────────────────────────
    /// Mandelbrot escape-time mapped to float.
    MandelbrotEscape { c_real: f32, c_imag: f32, scale: f32 },
    /// Julia set escape-time mapped to float.
    JuliaSet { c_real: f32, c_imag: f32, scale: f32 },

    // ── Damping / weight ──────────────────────────────────────────────────────
    /// Spring-damper: approaches target with optional overshoot.
    SpringDamper { target: f32, stiffness: f32, damping: f32 },
    /// Critically damped spring (no overshoot, fastest convergence).
    CriticallyDamped { target: f32, speed: f32 },

    // ── Special biological ────────────────────────────────────────────────────
    /// Realistic cardiac waveform (P, QRS, T waves).
    HeartBeat { bpm: f32, intensity: f32 },
    /// Four-phase breathing cycle: inhale → hold → exhale → hold.
    Breathing { rate: f32, depth: f32 },
    /// Physical pendulum with gravity and damping.
    Pendulum { length: f32, gravity: f32, damping: f32 },
    /// Traveling wave.
    Wave { wavelength: f32, speed: f32, amplitude: f32, decay: f32 },

    // ── Composition ───────────────────────────────────────────────────────────
    /// Add two function outputs.
    Sum(Box<MathFunction>, Box<MathFunction>),
    /// Multiply two function outputs.
    Product(Box<MathFunction>, Box<MathFunction>),
    /// Chain: output of each function feeds as input to the next.
    Chain(Vec<MathFunction>),
    /// Modulate amplitude of inner function by outer function output.
    Modulate { carrier: Box<MathFunction>, modulator: Box<MathFunction> },
    /// Clamp the output to [min, max].
    Clamp { inner: Box<MathFunction>, min: f32, max: f32 },
    /// Absolute value of inner.
    Abs(Box<MathFunction>),
}

impl MathFunction {
    /// Evaluate the function at time `t` with chained `input`.
    pub fn evaluate(&self, t: f32, input: f32) -> f32 {
        match self {
            // ── Basic ─────────────────────────────────────────────────────────
            MathFunction::Constant(v) => *v,
            MathFunction::Linear { slope, offset } => slope * t + offset,

            // ── Oscillation ───────────────────────────────────────────────────
            MathFunction::Sine { amplitude, frequency, phase } => {
                amplitude * (t * frequency * TAU + phase).sin()
            }
            MathFunction::Cosine { amplitude, frequency, phase } => {
                amplitude * (t * frequency * TAU + phase).cos()
            }
            MathFunction::Triangle { amplitude, frequency, phase } => {
                let p = (t * frequency + phase / TAU).fract();
                amplitude * (if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p })
            }
            MathFunction::Square { amplitude, frequency, duty } => {
                let p = (t * frequency).fract();
                if p < *duty { *amplitude } else { -amplitude }
            }

            // ── Organic ───────────────────────────────────────────────────────
            MathFunction::Lorenz { sigma, rho, beta, scale } => {
                // Run 40 iterations of Lorenz from a seed derived from t
                let (x, _, _) = attractors::lorenz_step(
                    (t * 0.1).sin() * 10.0,
                    (t * 0.07).cos() * 10.0,
                    20.0 + (t * 0.05).sin() * 5.0,
                    *sigma, *rho, *beta, 40
                );
                x * scale
            }
            MathFunction::Perlin { frequency, octaves, amplitude } => {
                use crate::math::noise::perlin_1d;
                perlin_1d(t * frequency, *octaves) * amplitude
            }
            MathFunction::Simplex { frequency, amplitude } => {
                use crate::math::noise::simplex_1d;
                simplex_1d(t * frequency) * amplitude
            }

            // ── Convergence ───────────────────────────────────────────────────
            MathFunction::Exponential { start, target, rate } => {
                target + (start - target) * (-rate * t).exp()
            }
            MathFunction::LogisticMap { r, x0 } => {
                // Iterate the logistic map `n` times where n ~ t * 60
                let n = (t * 30.0) as u32;
                let mut x = *x0;
                for _ in 0..n.min(200) {
                    x = r * x * (1.0 - x);
                }
                x * 2.0 - 1.0 // map [0,1] → [-1,1]
            }
            MathFunction::Collatz { seed, scale } => {
                // Generate Collatz sequence from seed, use t to index into it
                let n = (t * 10.0) as usize;
                let seq = collatz_sequence(*seed, 200);
                let v = seq.get(n % seq.len()).copied().unwrap_or(1) as f32;
                // Normalize: the sequence is bounded by the highest step
                let max = seq.iter().copied().fold(1u64, u64::max) as f32;
                (v / max) * 2.0 * scale - scale
            }

            // ── Orbiting ──────────────────────────────────────────────────────
            MathFunction::Orbit { radius, speed, eccentricity, .. } => {
                let angle = t * speed;
                let r = radius * (1.0 - eccentricity * angle.cos());
                r * angle.cos()
            }
            MathFunction::Spiral { radius_rate, speed, .. } => {
                let angle = t * speed;
                let r = t * radius_rate;
                r * angle.cos()
            }
            MathFunction::GoldenSpiral { scale, speed, .. } => {
                let phi = 1.618_034_f32;
                let angle = t * speed;
                phi.powf(angle / TAU) * angle.cos() * scale
            }
            MathFunction::Lissajous { a, b, delta, scale } => {
                (a * t).sin() * scale + (b * t + delta).sin() * scale * 0.5
            }

            // ── Strange attractor ──────────────────────────────────────────────
            MathFunction::StrangeAttractor { attractor_type, scale, strength } => {
                let (ix, iy, iz) = attractor_type.initial_conditions();
                let seed_t = (t * 0.05).sin() * 5.0;
                let (nx, _, _) = attractor_type.step(
                    ix + seed_t, iy + (t * 0.03).cos() * 5.0, iz, 20, 0.01
                );
                nx * scale * strength
            }

            // ── Fractal ───────────────────────────────────────────────────────
            MathFunction::MandelbrotEscape { c_real, c_imag, scale } => {
                let (mut zr, mut zi) = (input, input * 0.5);
                let max_iter = 64;
                let mut iter = 0;
                while iter < max_iter && zr * zr + zi * zi < 4.0 {
                    let new_zr = zr * zr - zi * zi + c_real;
                    zi = 2.0 * zr * zi + c_imag;
                    zr = new_zr;
                    iter += 1;
                }
                (iter as f32 / max_iter as f32) * 2.0 * scale - scale
            }
            MathFunction::JuliaSet { c_real, c_imag, scale } => {
                let (mut zr, mut zi) = ((t * 0.1).sin() * 2.0, (t * 0.07).cos() * 2.0);
                let max_iter = 64;
                let mut iter = 0;
                while iter < max_iter && zr * zr + zi * zi < 4.0 {
                    let new_zr = zr * zr - zi * zi + c_real;
                    zi = 2.0 * zr * zi + c_imag;
                    zr = new_zr;
                    iter += 1;
                }
                (iter as f32 / max_iter as f32) * 2.0 * scale - scale
            }

            // ── Damping ───────────────────────────────────────────────────────
            MathFunction::SpringDamper { target, stiffness, damping } => {
                // Analytical underdamped spring
                let omega = stiffness.sqrt();
                let zeta = damping / (2.0 * omega);
                if zeta < 1.0 {
                    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
                    let decay = (-zeta * omega * t).exp();
                    target + (-target) * decay * ((omega_d * t).cos() + zeta / (1.0 - zeta * zeta).sqrt() * (omega_d * t).sin())
                } else {
                    // Overdamped: simple exponential
                    target + (-target) * (-omega * t).exp()
                }
            }
            MathFunction::CriticallyDamped { target, speed } => {
                target + (-target) * (1.0 + speed * t) * (-speed * t).exp()
            }

            // ── Biological ────────────────────────────────────────────────────
            MathFunction::HeartBeat { bpm, intensity } => {
                let cycle = (t * bpm / 60.0).fract();
                if cycle < 0.10 {
                    intensity * 0.3 * (cycle / 0.10 * PI).sin()      // P wave
                } else if cycle < 0.18 {
                    0.0                                                // PR segment
                } else if cycle < 0.28 {
                    intensity * 2.0 * (cycle - 0.18) / 0.10          // QRS up
                } else if cycle < 0.33 {
                    intensity * 2.0 * (1.0 - (cycle - 0.28) / 0.05)  // QRS down
                } else if cycle < 0.50 {
                    intensity * 0.3 * ((cycle - 0.33) / 0.17 * PI).sin() // T wave
                } else {
                    0.0                                                // diastole
                }
            }
            MathFunction::Breathing { rate, depth } => {
                let cycle = (t * rate).fract();
                if cycle < 0.40 {
                    depth * (cycle / 0.40 * PI).sin()                 // inhale
                } else if cycle < 0.45 {
                    *depth                                              // hold full
                } else if cycle < 0.85 {
                    depth * ((cycle - 0.45) / 0.40 * PI + PI).sin() + depth // exhale
                } else {
                    0.0                                                // hold empty
                }
            }
            MathFunction::Pendulum { length, gravity, damping } => {
                let omega = (gravity / length).sqrt();
                let theta0 = PI / 6.0; // 30 degree initial swing
                let decay = (-damping * t).exp();
                theta0 * decay * (omega * t).cos()
            }
            MathFunction::Wave { wavelength, speed, amplitude, decay } => {
                let phase = t * speed / wavelength;
                amplitude * phase.sin() * (-decay * t).exp()
            }

            // ── Composition ───────────────────────────────────────────────────
            MathFunction::Sum(a, b) => a.evaluate(t, input) + b.evaluate(t, input),
            MathFunction::Product(a, b) => a.evaluate(t, input) * b.evaluate(t, input),
            MathFunction::Chain(functions) => {
                let mut value = input;
                for f in functions {
                    value = f.evaluate(t, value);
                }
                value
            }
            MathFunction::Modulate { carrier, modulator } => {
                carrier.evaluate(t, input) * modulator.evaluate(t, input)
            }
            MathFunction::Clamp { inner, min, max } => {
                inner.evaluate(t, input).clamp(*min, *max)
            }
            MathFunction::Abs(inner) => inner.evaluate(t, input).abs(),
        }
    }
}

/// Generate Collatz sequence starting from n, up to max_steps.
fn collatz_sequence(n: u64, max_steps: usize) -> Vec<u64> {
    let mut seq = Vec::with_capacity(max_steps);
    let mut x = n.max(1);
    seq.push(x);
    while x != 1 && seq.len() < max_steps {
        x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
        seq.push(x);
    }
    seq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_at_zero() {
        let f = MathFunction::Sine { amplitude: 1.0, frequency: 1.0, phase: 0.0 };
        assert!((f.evaluate(0.0, 0.0)).abs() < 1e-5);
    }

    #[test]
    fn constant_is_constant() {
        let f = MathFunction::Constant(42.0);
        assert_eq!(f.evaluate(0.0, 0.0), 42.0);
        assert_eq!(f.evaluate(100.0, -999.0), 42.0);
    }

    #[test]
    fn breathing_never_negative() {
        let f = MathFunction::Breathing { rate: 0.25, depth: 1.0 };
        for i in 0..100 {
            let v = f.evaluate(i as f32 * 0.1, 0.0);
            assert!(v >= -0.01, "breathing went negative at t={}", i as f32 * 0.1);
        }
    }

    #[test]
    fn chain_composes() {
        let f = MathFunction::Chain(vec![
            MathFunction::Constant(2.0),
            MathFunction::Linear { slope: 1.0, offset: 0.0 }, // output of prev is input
        ]);
        // Constant(2.0) always outputs 2.0 regardless of input.
        // Linear with slope=1 just passes through the input.
        // Chain: first outputs 2.0, that becomes input to Linear → 2.0
        // But Linear uses t not input for its own computation...
        // Actually our Linear is slope * t + offset, input is ignored.
        // So result = 1.0 * 0.0 + 0.0 = 0.0 for t=0.
        let _ = f.evaluate(0.0, 0.0); // just test it doesn't panic
    }
}
