//! lorenz: forty thousand points riding the Lorenz equations.
//!
//! Every point is a state `(x, y, z)` advanced each frame by the engine's own
//! RK4 integrator, `proof_engine::math::attractors::rk4_step`, on
//!
//! ```text
//! dx/dt = 10 (y - x)
//! dy/dt = x (28 - z) - y
//! dz/dt = x y - 8/3 z
//! ```
//!
//! Nothing is keyframed and nothing is a precomputed curve. The two wings
//! appear because that is where the equations send the points. Colour is the
//! point's speed along the flow, cool where it drifts and hot where it is
//! flung across from one wing to the other. Points are drawn into the HDR
//! world pass, so they add up to real light before bloom and the tonemap.
//!
//! Controls: Space pauses, Left/Right turn the view, Esc quits.
//!
//! Run: `cargo run --release --example lorenz`

use proof_engine::math::attractors::{derivatives, rk4_step};
use proof_engine::prelude::*;
use proof_engine::render::ui_layer::UiParticle;

const POINTS: usize = 40_000;
/// Simulated seconds per real second.
const TIME_SCALE: f32 = 0.22;
/// Integration substeps per frame, so a slow frame never takes a big step.
const SUBSTEPS: usize = 2;
/// Radians per second the view turns on its own.
const SPIN: f32 = 0.05;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine: Lorenz attractor".to_string(),
        window_width: 1280,
        window_height: 720,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.1,
            bloom_threshold: 0.35,
            tonemap: 1.0,
            exposure: 1.1,
            persistence: 0.55,
            vignette: 0.35,
            // Everything here is drawn in the UI layer; the 3D lighting
            // solves would have nothing to light.
            global_illumination: false,
            volumetric_fog: false,
            ..Default::default()
        },
        ..Default::default()
    });

    // Spread the points along one long trajectory that has already settled
    // onto the attractor, then separate them by a hair. Chaos does the rest.
    let lorenz = AttractorType::Lorenz;
    let mut s = Vec3::new(0.1, 0.0, 0.0);
    for _ in 0..3000 {
        s = rk4_step(lorenz, s, 0.005);
    }
    let mut points: Vec<Vec3> = (0..POINTS)
        .map(|i| {
            for _ in 0..3 {
                s = rk4_step(lorenz, s, 0.005);
            }
            s + Vec3::splat(hash(i as f32) * 1e-3)
        })
        .collect();

    // Start looking across both wings; they line up behind each other at +45 degrees.
    let mut yaw = -1.05_f32;
    let mut paused = false;
    let mut sim_t = 0.0_f32;

    engine.run_ui(move |engine, dt| {
        if engine.input.just_pressed(Key::Escape) {
            engine.input.quit_requested = true;
        }
        if engine.input.just_pressed(Key::Space) {
            paused = !paused;
        }
        if engine.input.is_pressed(Key::Left) {
            yaw -= dt * 1.2;
        }
        if engine.input.is_pressed(Key::Right) {
            yaw += dt * 1.2;
        }

        if !paused {
            let h = dt * TIME_SCALE / SUBSTEPS as f32;
            for p in points.iter_mut() {
                for _ in 0..SUBSTEPS {
                    *p = rk4_step(lorenz, *p, h);
                }
            }
            sim_t += dt * TIME_SCALE;
            yaw += dt * SPIN;
        }

        let (w, h) = engine.render_size();
        let (w, h) = (w as f32, h as f32);
        // The attractor spans about 50 units in z and 45 across; fit it.
        let scale = h / 58.0;
        let (cx, cy) = (w * 0.5, h * 0.5);
        let (sy, cyaw) = yaw.sin_cos();
        let size = (h / 360.0).max(1.5);

        let particles: Vec<UiParticle> = points
            .iter()
            .map(|p| {
                // Turn about the vertical (z) axis, then look from the side.
                let x = p.x * cyaw - p.y * sy;
                let depth = p.x * sy + p.y * cyaw;
                let px = cx + x * scale;
                let py = cy - (p.z - 25.0) * scale;
                let speed = derivatives(lorenz, *p).length();
                let c = palette((speed / 190.0).clamp(0.0, 1.0));
                // Nearer points a little brighter, so the wings read as depth.
                let near = 0.75 + 0.25 * (depth / 25.0).clamp(-1.0, 1.0);
                let mut q = UiParticle::new(px, py, size, size, '●', (c * near).extend(0.85));
                q.emission = 0.6 + 0.9 * (speed / 190.0).min(1.0);
                q
            })
            .collect();
        engine.ui.draw_particles(particles);

        let dim = Vec4::new(0.85, 0.88, 0.92, 0.75);
        engine.ui.draw_text(20.0, 18.0, "Lorenz attractor   dx/dt = 10(y - x)   dy/dt = x(28 - z) - y   dz/dt = xy - 8z/3", 1.0, dim);
        let status = format!(
            "{POINTS} points   RK4   t = {sim_t:.2}{}   [Space] pause  [Left/Right] turn",
            if paused { " (paused)" } else { "" }
        );
        engine.ui.draw_text(20.0, 42.0, &status, 1.0, Vec4::new(0.85, 0.88, 0.92, 0.55));
    });
}

/// Slow is a cool teal, fast is a hot amber: the same two colours the Nishita
/// sky produces at dusk.
fn palette(t: f32) -> Vec3 {
    let cool = Vec3::new(0.10, 0.55, 0.70);
    let mid = Vec3::new(0.85, 0.80, 0.70);
    let hot = Vec3::new(1.40, 0.55, 0.16);
    if t < 0.5 {
        cool.lerp(mid, t * 2.0)
    } else {
        mid.lerp(hot, (t - 0.5) * 2.0)
    }
}

/// Cheap deterministic hash in [0, 1).
fn hash(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43_758.547).fract().abs()
}
