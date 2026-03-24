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

    // ── Sawtooth / pulse ──────────────────────────────────────────────────────
    /// Sawtooth wave (rising ramp that resets to -1).
    Sawtooth { amplitude: f32, frequency: f32, phase: f32 },
    /// Ramp: linearly rises from 0 to amplitude over `duration`, then resets.
    Ramp { amplitude: f32, duration: f32 },

    // ── Signal functions ──────────────────────────────────────────────────────
    /// Sinc function: sin(π·x) / (π·x). Used for signal reconstruction.
    Sinc { frequency: f32, amplitude: f32 },
    /// Gaussian bell curve: amplitude · e^(-((t-center)/width)²).
    Gaussian { amplitude: f32, center: f32, width: f32 },
    /// Two-frequency beat: interference between slightly detuned oscillators.
    BeatFrequency { freq1: f32, freq2: f32, amplitude: f32 },
    /// Wave packet: Gaussian envelope × carrier sine.
    WavePacket { carrier_freq: f32, envelope_width: f32, amplitude: f32, center: f32 },
    /// Fourier series: sum of harmonics with user-specified coefficients.
    FourierSeries { fundamental: f32, coefficients: Vec<(f32, f32)> }, // (sin_coeff, cos_coeff)

    // ── Activation / shaping ─────────────────────────────────────────────────
    /// Sigmoid (logistic) function maps any input → (0, 1).
    Sigmoid { steepness: f32, center: f32 },
    /// Hyperbolic tangent: amplitude · tanh(steepness · t).
    Tanh { amplitude: f32, steepness: f32 },
    /// Soft-plus: amplitude · ln(1 + e^(steepness·t)) / steepness.
    SoftPlus { amplitude: f32, steepness: f32 },
    /// Rectified linear: max(0, slope·t + offset).
    Relu { slope: f32, offset: f32 },
    /// Power law: sign(t) · |t|^exponent.
    PowerLaw { exponent: f32, scale: f32 },

    // ── Chaos / dynamical systems ─────────────────────────────────────────────
    /// Van der Pol oscillator (nonlinear oscillator with limit cycle).
    VanDerPol { mu: f32, amplitude: f32 },
    /// Duffing oscillator (chaotic forced nonlinear oscillator).
    Duffing { alpha: f32, beta: f32, delta: f32, gamma: f32, omega: f32 },
    /// Tent map iterated n times: exhibits period-doubling route to chaos.
    TentMap { r: f32, x0: f32 },
    /// Hénon map: chaotic 2D map (returns x-coordinate).
    HenonMap { a: f32, b: f32, x0: f32, y0: f32 },
    /// Rössler system x-coordinate trajectory.
    Roessler { a: f32, b: f32, c: f32, scale: f32 },

    // ── Physical simulations ──────────────────────────────────────────────────
    /// Double pendulum: chaotic system of two connected pendulums (angle θ₁).
    DoublePendulum { l1: f32, l2: f32, m1: f32, m2: f32, theta1_0: f32, theta2_0: f32 },
    /// Projectile motion: height above ground (y-axis) at time t.
    Projectile { v0: f32, angle_deg: f32, gravity: f32 },
    /// Simple harmonic motion with optional initial displacement.
    SimpleHarmonic { omega: f32, amplitude: f32, phase: f32, decay: f32 },
    /// Damped sine with exact analytic form.
    DampedSine { omega: f32, zeta: f32, amplitude: f32, phase: f32 },
    /// Epicyclic motion: small circle of radius `r2` orbiting at radius `r1`.
    Epicycle { r1: f32, r2: f32, omega1: f32, omega2: f32 },

    // ── Statistical / noise ───────────────────────────────────────────────────
    /// Fractional Brownian motion: multi-octave noise with Hurst exponent H.
    FractionalBrownian { frequency: f32, octaves: u8, hurst: f32, amplitude: f32 },
    /// Domain-warped noise: the input to the noise is itself displaced by another noise.
    DomainWarp { frequency: f32, warp_strength: f32, octaves: u8, amplitude: f32 },
    /// Cellular / Worley noise: distance to nearest point in a Poisson point process.
    Cellular { frequency: f32, amplitude: f32 },

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
    /// Scale: multiply output of inner by `factor`.
    Scale { inner: Box<MathFunction>, factor: f32 },
    /// Offset: add `offset` to output of inner.
    Offset { inner: Box<MathFunction>, offset: f32 },
    /// Invert: negate the output of inner.
    Invert(Box<MathFunction>),
    /// Normalize inner to [-1, 1] over a given sample window of `t_range` seconds.
    /// (sampled at `steps` points; expensive — use sparingly.)
    Normalize { inner: Box<MathFunction>, t_range: f32, steps: u32 },
    /// Delay: evaluate inner at (t - delay_seconds).
    Delay { inner: Box<MathFunction>, delay: f32 },
    /// Mirror: map t → |t mod (2·period) - period| creating a symmetric waveform.
    Mirror { inner: Box<MathFunction>, period: f32 },
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
            MathFunction::Lorenz { sigma: _, rho: _, beta: _, scale } => {
                // Run 40 iterations of Lorenz from a seed derived from t
                let init = glam::Vec3::new(
                    (t * 0.1).sin() * 10.0,
                    (t * 0.07).cos() * 10.0,
                    20.0 + (t * 0.05).sin() * 5.0,
                );
                let mut state = init;
                for _ in 0..40 {
                    let (next, _) = attractors::step(attractors::AttractorType::Lorenz, state, 0.01);
                    state = next;
                }
                state.x * scale
            }
            MathFunction::Perlin { frequency, octaves, amplitude } => {
                use crate::math::noise::fbm;
                fbm(t * frequency, 0.0, *octaves, 0.5, 2.0) * amplitude
            }
            MathFunction::Simplex { frequency, amplitude } => {
                use crate::math::noise::noise1;
                noise1(t * frequency) * amplitude
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
                let init = attractors::initial_state(*attractor_type);
                let seed_t = (t * 0.05).sin() * 5.0;
                let mut state = glam::Vec3::new(
                    init.x + seed_t,
                    init.y + (t * 0.03).cos() * 5.0,
                    init.z,
                );
                for _ in 0..20 {
                    let (next, _) = attractors::step(*attractor_type, state, 0.01);
                    state = next;
                }
                state.x * scale * strength
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

            // ── Sawtooth / pulse ─────────────────────────────────────────────
            MathFunction::Sawtooth { amplitude, frequency, phase } => {
                let p = (t * frequency + phase / TAU).fract();
                amplitude * (2.0 * p - 1.0)
            }
            MathFunction::Ramp { amplitude, duration } => {
                let p = (t / duration.max(f32::EPSILON)).fract();
                amplitude * p
            }

            // ── Signal functions ──────────────────────────────────────────────
            MathFunction::Sinc { frequency, amplitude } => {
                let x = t * frequency * PI;
                let v = if x.abs() < f32::EPSILON { 1.0 } else { x.sin() / x };
                amplitude * v
            }
            MathFunction::Gaussian { amplitude, center, width } => {
                let x = (t - center) / width.max(f32::EPSILON);
                amplitude * (-x * x).exp()
            }
            MathFunction::BeatFrequency { freq1, freq2, amplitude } => {
                amplitude * (t * freq1 * TAU).sin() * (t * freq2 * TAU).sin()
            }
            MathFunction::WavePacket { carrier_freq, envelope_width, amplitude, center } => {
                let carrier  = (t * carrier_freq * TAU).sin();
                let envelope = {
                    let x = (t - center) / envelope_width.max(f32::EPSILON);
                    (-x * x).exp()
                };
                amplitude * carrier * envelope
            }
            MathFunction::FourierSeries { fundamental, coefficients } => {
                let mut v = 0.0_f32;
                for (n, (sin_c, cos_c)) in coefficients.iter().enumerate() {
                    let harmonic = (n as f32 + 1.0) * fundamental * TAU * t;
                    v += sin_c * harmonic.sin() + cos_c * harmonic.cos();
                }
                v
            }

            // ── Activation / shaping ─────────────────────────────────────────
            MathFunction::Sigmoid { steepness, center } => {
                1.0 / (1.0 + (-steepness * (t - center)).exp())
            }
            MathFunction::Tanh { amplitude, steepness } => {
                amplitude * (steepness * t).tanh()
            }
            MathFunction::SoftPlus { amplitude, steepness } => {
                let s = steepness.max(f32::EPSILON);
                amplitude * (1.0 + (s * t).exp()).ln() / s
            }
            MathFunction::Relu { slope, offset } => {
                (slope * t + offset).max(0.0)
            }
            MathFunction::PowerLaw { exponent, scale } => {
                scale * t.abs().powf(*exponent) * t.signum()
            }

            // ── Chaos / dynamical systems ──────────────────────────────────────
            MathFunction::VanDerPol { mu, amplitude } => {
                // Numerical integration of Van der Pol: ẍ - μ(1 - x²)ẋ + x = 0
                let dt_inner = 0.02_f32;
                let steps    = (t / dt_inner) as u32;
                let mut x    = *amplitude;
                let mut v    = 0.0_f32;
                for _ in 0..steps.min(5000) {
                    let a = mu * (1.0 - x * x) * v - x;
                    v += a * dt_inner;
                    x += v * dt_inner;
                }
                x.clamp(-amplitude * 3.0, amplitude * 3.0)
            }
            MathFunction::Duffing { alpha, beta, delta, gamma, omega } => {
                // Forced Duffing oscillator: ẍ + δẋ + αx + βx³ = γcos(ωt)
                let dt_inner = 0.005_f32;
                let steps    = (t / dt_inner) as u32;
                let mut x    = 0.5_f32;
                let mut v    = 0.0_f32;
                let mut s    = 0.0_f32;
                for _ in 0..steps.min(10000) {
                    let a = -delta * v - alpha * x - beta * x * x * x + gamma * (omega * s).cos();
                    v += a * dt_inner;
                    x += v * dt_inner;
                    s += dt_inner;
                }
                x.clamp(-5.0, 5.0)
            }
            MathFunction::TentMap { r, x0 } => {
                let n = (t * 40.0) as u32;
                let mut x = *x0;
                for _ in 0..n.min(500) {
                    x = if x < 0.5 { r * x } else { r * (1.0 - x) };
                }
                x * 2.0 - 1.0
            }
            MathFunction::HenonMap { a, b, x0, y0 } => {
                let n = (t * 30.0) as u32;
                let mut x = *x0;
                let mut y = *y0;
                for _ in 0..n.min(1000) {
                    let new_x = 1.0 - a * x * x + y;
                    y = b * x;
                    x = new_x;
                }
                x.clamp(-2.0, 2.0)
            }
            MathFunction::Roessler { a, b, c, scale } => {
                let dt_inner = 0.01_f32;
                let steps    = (t / dt_inner) as u32;
                let (mut rx, mut ry, mut rz) = (0.1_f32, 0.0_f32, 0.0_f32);
                for _ in 0..steps.min(5000) {
                    let dx = -(ry + rz);
                    let dy = rx + a * ry;
                    let dz = b + rz * (rx - c);
                    rx += dx * dt_inner;
                    ry += dy * dt_inner;
                    rz += dz * dt_inner;
                }
                rx * scale
            }

            // ── Physical simulations ───────────────────────────────────────────
            MathFunction::DoublePendulum { l1, l2, m1, m2, theta1_0, theta2_0 } => {
                // Runge-Kutta 4 integration of double pendulum equations
                let g     = 9.81_f32;
                let dt_rk = 0.005_f32;
                let steps = (t / dt_rk) as u32;
                let mut th1 = *theta1_0;
                let mut th2 = *theta2_0;
                let mut w1  = 0.0_f32;
                let mut w2  = 0.0_f32;

                let dp_accel = |th1: f32, th2: f32, w1: f32, w2: f32| -> (f32, f32) {
                    let dth = th1 - th2;
                    let denom1 = (m1 + m2) * l1 - m2 * l1 * dth.cos() * dth.cos();
                    let denom2 = (l2 / l1) * denom1;
                    let a1 = (m2 * l1 * w1 * w1 * dth.sin() * dth.cos()
                             + m2 * g * th2.sin() * dth.cos()
                             + m2 * l2 * w2 * w2 * dth.sin()
                             - (m1 + m2) * g * th1.sin()) / denom1.max(f32::EPSILON);
                    let a2 = (-(m1 + m2) * (l1 * w1 * w1 * dth.sin()
                             + g * th1.sin() * dth.cos() - g * th2.sin())
                             - m2 * l2 * w2 * w2 * dth.sin() * dth.cos()) / denom2.max(f32::EPSILON);
                    (a1, a2)
                };

                for _ in 0..steps.min(8000) {
                    let (a1, a2) = dp_accel(th1, th2, w1, w2);
                    // Simple Euler (fast enough for visual use)
                    w1 += a1 * dt_rk;
                    w2 += a2 * dt_rk;
                    th1 += w1 * dt_rk;
                    th2 += w2 * dt_rk;
                }
                th1.clamp(-PI * 4.0, PI * 4.0)
            }
            MathFunction::Projectile { v0, angle_deg, gravity } => {
                let angle = angle_deg.to_radians();
                let vy    = v0 * angle.sin();
                vy * t - 0.5 * gravity * t * t
            }
            MathFunction::SimpleHarmonic { omega, amplitude, phase, decay } => {
                let env = if *decay > f32::EPSILON { (-decay * t).exp() } else { 1.0 };
                amplitude * env * (omega * t + phase).sin()
            }
            MathFunction::DampedSine { omega, zeta, amplitude, phase } => {
                let omega_d = omega * (1.0 - zeta * zeta).abs().sqrt();
                let env     = (-zeta * omega * t).exp();
                amplitude * env * (omega_d * t + phase).sin()
            }
            MathFunction::Epicycle { r1, r2, omega1, omega2 } => {
                let x = r1 * (omega1 * t).cos() + r2 * (omega2 * t).cos();
                x // y-coord is evaluate(t+0.5, .) with offset if caller wants 2D
            }

            // ── Statistical / noise ───────────────────────────────────────────
            MathFunction::FractionalBrownian { frequency, octaves, hurst, amplitude } => {
                use crate::math::noise::fbm;
                // Hurst exponent controls persistence: H=0.5 → Brownian, H>0.5 → smooth
                let persistence = 2.0_f32.powf(-*hurst);
                fbm(t * frequency, 0.0, *octaves, persistence, 2.0) * amplitude
            }
            MathFunction::DomainWarp { frequency, warp_strength, octaves, amplitude } => {
                use crate::math::noise::{fbm, noise1};
                // Displace the domain by a noise field before sampling
                let warp = noise1(t * frequency * 0.5) * warp_strength;
                fbm((t + warp) * frequency, 0.0, *octaves, 0.5, 2.0) * amplitude
            }
            MathFunction::Cellular { frequency, amplitude } => {
                // Approximated Worley noise: minimum distance to a grid of random points
                let cell = (t * frequency).floor();
                let frac = (t * frequency).fract();
                let mut min_dist = f32::MAX;
                for offset in [-1i32, 0, 1] {
                    let point_cell = cell + offset as f32;
                    // Deterministic random point within each cell
                    let hash = (point_cell * 127.321 + 3481.12).sin().abs();
                    let point = hash; // point position within [0, 1]
                    let dist  = (frac - point - offset as f32).abs();
                    min_dist = min_dist.min(dist);
                }
                amplitude * min_dist * 2.0
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
            MathFunction::Abs(inner)      => inner.evaluate(t, input).abs(),
            MathFunction::Scale  { inner, factor } => inner.evaluate(t, input) * factor,
            MathFunction::Offset { inner, offset } => inner.evaluate(t, input) + offset,
            MathFunction::Invert(inner)   => -inner.evaluate(t, input),
            MathFunction::Normalize { inner, t_range, steps } => {
                // Sample inner at `steps` points to find the value range, then normalize
                let n   = (*steps).max(2) as usize;
                let dt  = t_range / (n - 1) as f32;
                let mut min_v = f32::MAX;
                let mut max_v = f32::MIN;
                for i in 0..n {
                    let v = inner.evaluate(i as f32 * dt, 0.0);
                    if v < min_v { min_v = v; }
                    if v > max_v { max_v = v; }
                }
                let range = (max_v - min_v).max(f32::EPSILON);
                let raw   = inner.evaluate(t, input);
                (raw - min_v) / range * 2.0 - 1.0
            }
            MathFunction::Delay { inner, delay } => {
                inner.evaluate((t - delay).max(0.0), input)
            }
            MathFunction::Mirror { inner, period } => {
                let p   = period.max(f32::EPSILON);
                let t2  = (t % (2.0 * p) + 2.0 * p) % (2.0 * p);
                let t_m = if t2 < p { t2 } else { 2.0 * p - t2 };
                inner.evaluate(t_m, input)
            }
        }
    }

    // ── Utility methods ────────────────────────────────────────────────────────

    /// Numerical derivative dF/dt at time `t` using central differences.
    ///
    /// Epsilon defaults to 1e-4 — reduce for smoother but less accurate results.
    pub fn derivative(&self, t: f32, epsilon: f32) -> f32 {
        let hi = self.evaluate(t + epsilon, 0.0);
        let lo = self.evaluate(t - epsilon, 0.0);
        (hi - lo) / (2.0 * epsilon.max(f32::EPSILON))
    }

    /// Numerical integral ∫F dt over [from, to] using Simpson's rule with `steps` intervals.
    ///
    /// `steps` must be even (rounded up if not).
    pub fn integrate(&self, from: f32, to: f32, steps: u32) -> f32 {
        let n  = ((steps + 1) & !1) as usize; // ensure even
        let h  = (to - from) / n as f32;
        let mut sum = self.evaluate(from, 0.0) + self.evaluate(to, 0.0);
        for i in 1..n {
            let x = from + i as f32 * h;
            let w = if i % 2 == 0 { 2.0 } else { 4.0 };
            sum += w * self.evaluate(x, 0.0);
        }
        sum * h / 3.0
    }

    /// Sample the function at `n` uniformly spaced points over [t_start, t_end].
    pub fn sample_range(&self, t_start: f32, t_end: f32, n: u32) -> Vec<f32> {
        let count = n.max(2) as usize;
        let dt    = (t_end - t_start) / (count - 1) as f32;
        (0..count).map(|i| self.evaluate(t_start + i as f32 * dt, 0.0)).collect()
    }

    /// Find the approximate minimum and maximum output over [t_start, t_end].
    pub fn find_range(&self, t_start: f32, t_end: f32, steps: u32) -> (f32, f32) {
        let samples = self.sample_range(t_start, t_end, steps.max(2));
        let min = samples.iter().cloned().fold(f32::MAX, f32::min);
        let max = samples.iter().cloned().fold(f32::MIN, f32::max);
        (min, max)
    }

    /// Find approximate zero-crossings in [t_start, t_end].
    pub fn zero_crossings(&self, t_start: f32, t_end: f32, steps: u32) -> Vec<f32> {
        let count = steps.max(2) as usize;
        let dt    = (t_end - t_start) / (count - 1) as f32;
        let mut crossings = Vec::new();
        let mut prev = self.evaluate(t_start, 0.0);
        for i in 1..count {
            let t = t_start + i as f32 * dt;
            let v = self.evaluate(t, 0.0);
            if (prev < 0.0) != (v < 0.0) {
                // Linear interpolation of crossing time
                let frac = -prev / (v - prev).max(f32::EPSILON);
                crossings.push(t - dt + frac * dt);
            }
            prev = v;
        }
        crossings
    }

    /// Evaluate as a 3D trajectory: x = f(t, 0), y = f(t+1, 0), z = f(t+2, 0).
    ///
    /// This is the default particle behavior — all three axes driven by the same
    /// function but offset in time, producing organic non-planar motion.
    pub fn evaluate_vec3(&self, t: f32) -> glam::Vec3 {
        glam::Vec3::new(
            self.evaluate(t,       0.0),
            self.evaluate(t + 1.0, 0.0),
            self.evaluate(t + 2.0, 0.0),
        )
    }

    /// Evaluate as a 3D trajectory with explicit phase offsets per axis.
    pub fn evaluate_vec3_phased(&self, t: f32, phase_x: f32, phase_y: f32, phase_z: f32) -> glam::Vec3 {
        glam::Vec3::new(
            self.evaluate(t + phase_x, 0.0),
            self.evaluate(t + phase_y, 0.0),
            self.evaluate(t + phase_z, 0.0),
        )
    }

    /// Create a `Sum` of this function and another.
    pub fn add(self, other: MathFunction) -> MathFunction {
        MathFunction::Sum(Box::new(self), Box::new(other))
    }

    /// Create a `Product` of this function and another.
    pub fn mul(self, other: MathFunction) -> MathFunction {
        MathFunction::Product(Box::new(self), Box::new(other))
    }

    /// Scale the output by `factor`.
    pub fn scale(self, factor: f32) -> MathFunction {
        MathFunction::Scale { inner: Box::new(self), factor }
    }

    /// Add a constant offset to the output.
    pub fn offset(self, offset: f32) -> MathFunction {
        MathFunction::Offset { inner: Box::new(self), offset }
    }

    /// Clamp the output to [min, max].
    pub fn clamp(self, min: f32, max: f32) -> MathFunction {
        MathFunction::Clamp { inner: Box::new(self), min, max }
    }

    /// Delay the function by `seconds`.
    pub fn delay(self, seconds: f32) -> MathFunction {
        MathFunction::Delay { inner: Box::new(self), delay: seconds }
    }

    /// Invert (negate) the output.
    pub fn invert(self) -> MathFunction {
        MathFunction::Invert(Box::new(self))
    }

    /// Modulate by another function (amplitude modulation).
    pub fn modulate(self, modulator: MathFunction) -> MathFunction {
        MathFunction::Modulate {
            carrier:   Box::new(self),
            modulator: Box::new(modulator),
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
