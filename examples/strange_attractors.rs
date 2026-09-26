//! strange_attractors: seven chaotic systems side by side.
//!
//! Lorenz, Rossler, Chen, Halvorsen, Aizawa, Thomas and Dadras, each with
//! 1,500 points advanced every frame by the engine's own RK4 integrator,
//! `proof_engine::math::attractors::rk4_step`. The points start spread
//! along one long trajectory, so the shape is there from the first frame.
//! Each panel turns slowly so the shape reads in 3D; colour is speed along
//! the flow, from the panel's own hue at rest to white where it is fastest.
//!
//! Every panel is fitted to the attractor's real extent, measured at start
//! up by integrating a long trajectory, so no system flies off screen.
//!
//! Controls: Space pauses, Esc quits.
//!
//! Run: `cargo run --release --example strange_attractors`

use proof_engine::math::attractors::{derivatives, initial_state, rk4_step};
use proof_engine::prelude::*;
use proof_engine::render::ui_layer::UiParticle;

const ATTRACTORS: &[(AttractorType, &str, (f32, f32, f32))] = &[
    (AttractorType::Lorenz, "Lorenz", (0.10, 0.85, 0.80)),
    (AttractorType::Rossler, "Rossler", (0.70, 0.35, 1.00)),
    (AttractorType::Chen, "Chen", (1.00, 0.50, 0.12)),
    (AttractorType::Halvorsen, "Halvorsen", (0.25, 0.85, 0.40)),
    (AttractorType::Aizawa, "Aizawa", (0.95, 0.75, 0.25)),
    (AttractorType::Thomas, "Thomas", (0.30, 0.75, 1.00)),
    (AttractorType::Dadras, "Dadras", (1.00, 0.30, 0.55)),
];

const POINTS: usize = 1500;
/// Integration steps per frame, each of the attractor's recommended size.
const STEPS_PER_FRAME: usize = 2;
/// Radians per second each panel turns.
const SPIN: f32 = 0.18;
/// How far the view looks down onto each attractor.
const TILT: f32 = 0.45;

struct Panel {
    kind: AttractorType,
    name: &'static str,
    hue: Vec3,
    points: Vec<Vec3>,
    center: Vec3,
    radius: f32,
    /// Speed that maps to full white.
    fast: f32,
    h: f32,
}

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine: strange attractors".to_string(),
        window_width: 1400,
        window_height: 800,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 0.9,
            bloom_threshold: 0.4,
            persistence: 0.5,
            vignette: 0.3,
            global_illumination: false,
            volumetric_fog: false,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut panels: Vec<Panel> = ATTRACTORS
        .iter()
        .map(|&(kind, name, (r, g, b))| {
            let h = kind.recommended_dt();
            // Settle onto the attractor, then measure it.
            let mut s = initial_state(kind);
            for _ in 0..6000 {
                s = rk4_step(kind, s, h);
            }
            let mut samples = Vec::with_capacity(20_000);
            let mut t = s;
            for _ in 0..20_000 {
                t = rk4_step(kind, t, h);
                samples.push(t);
            }
            let center = samples.iter().fold(Vec3::ZERO, |a, p| a + *p) / samples.len() as f32;
            let radius = samples.iter().map(|p| (*p - center).length()).fold(0.0, f32::max);
            let mut speeds: Vec<f32> = samples.iter().map(|p| derivatives(kind, *p).length()).collect();
            speeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let fast = speeds[speeds.len() * 95 / 100].max(1e-3);
            // Spread the points along the measured trajectory, a hair apart,
            // so the whole attractor is visible from the first frame.
            let stride = samples.len() / POINTS;
            let points = (0..POINTS)
                .map(|i| samples[i * stride] + Vec3::splat(hash(i as f32) * radius * 1e-4))
                .collect();
            Panel { kind, name, hue: Vec3::new(r, g, b), points, center, radius, fast, h }
        })
        .collect();

    let mut paused = false;
    let mut yaw = 0.0_f32;
    let mut sim_frames: u64 = 0;

    engine.run_ui(move |engine, dt| {
        if engine.input.just_pressed(Key::Escape) {
            engine.input.quit_requested = true;
        }
        if engine.input.just_pressed(Key::Space) {
            paused = !paused;
        }
        if !paused {
            for p in panels.iter_mut() {
                for s in p.points.iter_mut() {
                    for _ in 0..STEPS_PER_FRAME {
                        *s = rk4_step(p.kind, *s, p.h);
                    }
                }
            }
            yaw += dt * SPIN;
            sim_frames += 1;
        }

        let (w, h) = engine.render_size();
        let (w, h) = (w as f32, h as f32);
        let top = h * 0.09;
        let row_h = (h - top) / 2.0;
        let (sy, cy) = yaw.sin_cos();
        let (st, ct) = TILT.sin_cos();
        let size = (h / 300.0).max(1.5);
        let label = Vec4::new(0.86, 0.88, 0.92, 0.8);

        for (i, p) in panels.iter().enumerate() {
            // Four panels on the top row, three centred below.
            let (col, row, per_row) = if i < 4 { (i, 0, 4) } else { (i - 4, 1, 3) };
            let cell_w = w / 4.0;
            let x0 = (w - cell_w * per_row as f32) * 0.5 + cell_w * col as f32;
            let pcx = x0 + cell_w * 0.5;
            let pcy = top + row_h * (row as f32 + 0.5);
            let scale = (cell_w.min(row_h) * 0.44) / p.radius;

            let particles: Vec<UiParticle> = p
                .points
                .iter()
                .map(|s| {
                    let d = *s - p.center;
                    // Turn about the attractor's z axis, then tilt toward the viewer.
                    let x = d.x * cy - d.y * sy;
                    let depth = d.x * sy + d.y * cy;
                    let v = d.z * ct + depth * st;
                    let speed = (derivatives(p.kind, *s).length() / p.fast).clamp(0.0, 1.0);
                    let c = p.hue.lerp(Vec3::new(1.2, 1.15, 1.05), speed * speed);
                    let mut q = UiParticle::new(pcx + x * scale, pcy - v * scale, size, size, '●', c.extend(0.8));
                    q.emission = 0.5 + 0.8 * speed;
                    q
                })
                .collect();
            engine.ui.draw_particles(particles);
            engine.ui.draw_text(x0 + 16.0, pcy - row_h * 0.5 + 8.0, p.name, 1.0, label);
        }

        engine.ui.draw_text(20.0, 16.0, "Seven strange attractors   RK4, 1,500 points each   colour = speed along the flow", 1.0, label);
        let status = format!("step {}{}   [Space] pause  [Esc] quit", sim_frames * STEPS_PER_FRAME as u64, if paused { " (paused)" } else { "" });
        engine.ui.draw_text(20.0, 40.0, &status, 1.0, Vec4::new(0.86, 0.88, 0.92, 0.5));
    });
}

/// Cheap deterministic hash in [-0.5, 0.5).
fn hash(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43_758.547).fract() - 0.5
}
