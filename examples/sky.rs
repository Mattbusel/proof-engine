//! sky: a day passing over a mountain range, lit by the Nishita sky model.
//!
//! Every cell of the sky is `nishita_sky::compute_sky_color` evaluated for
//! that cell's view direction, every frame: single scattering through a
//! Rayleigh and Mie atmosphere, integrated along the view ray and along a
//! second ray toward the sun. Nothing is a gradient or a texture. The blue at
//! noon, the orange band at sunset and the dark violet after it all come out
//! of the same integral as the sun moves.
//!
//! The mountains are a sum of sines and take their light from the sky just
//! above the horizon, so they warm up and go dark with it.
//!
//! Controls: Space pauses, Up/Down change the speed of the day, Esc quits.
//!
//! Run: `cargo run --release --example sky`

use proof_engine::nishita_sky::{compute_sky_color, SkyConfig};
use proof_engine::prelude::*;
use proof_engine::render::ui_layer::UiParticle;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Sky cells across and down. Each is one full scattering integral per frame.
const COLS: usize = 120;
const ROWS: usize = 56;
/// Horizontal field of view, radians.
const FOV_X: f32 = 1.9;
/// Elevation at the top and bottom of the sky area, radians.
const EL_TOP: f32 = 1.05;
const EL_BOTTOM: f32 = -0.01;
/// Brings the model's radiance into the range the tonemapper expects.
const EXPOSURE: f32 = 1.6;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine: Nishita sky".to_string(),
        window_width: 1280,
        window_height: 720,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.2,
            film_grain: 0.015,
            ..Default::default()
        },
        ..Default::default()
    });

    // Start mid-morning; one full day takes about 50 seconds at speed 1.
    let mut day = 0.18_f32;
    let mut speed = 1.0_f32;
    let mut paused = false;
    let mut clock_t = 0.0_f32;

    let stars: Vec<(f32, f32, f32)> = (0..220)
        .map(|i| {
            let f = i as f32;
            (hash(f * 1.7), hash(f * 3.1 + 7.0) * 0.8, 0.4 + 0.6 * hash(f * 5.3 + 1.0))
        })
        .collect();

    engine.run_ui(move |engine, dt| {
        if engine.input.just_pressed(Key::Escape) {
            engine.input.quit_requested = true;
        }
        if engine.input.just_pressed(Key::Space) {
            paused = !paused;
        }
        if engine.input.just_pressed(Key::Up) {
            speed = (speed * 2.0).min(16.0);
        }
        if engine.input.just_pressed(Key::Down) {
            speed = (speed * 0.5).max(0.125);
        }
        clock_t += dt;
        if !paused {
            day = (day + dt * speed / 50.0).fract();
        }

        let (w, h) = engine.render_size();
        let (w, h) = (w as f32, h as f32);
        let horizon_y = h * 0.78;

        // The sun rises in the east (left), peaks at noon and sets in the west.
        // `day` 0.0 is sunrise, 0.5 is sunset; the second half is night.
        let angle = day * TAU;
        let sun_dir = Vec3::new(-angle.cos() * 0.85, angle.sin() * 0.8, -0.55).normalize();
        let sky = SkyConfig {
            sun_direction: sun_dir,
            sun_intensity: 22.0,
            num_samples: 10,
            num_light_samples: 6,
            ..Default::default()
        };

        // Sky: one filled rect per cell, drawn into the HDR world pass so the
        // bloom and tonemap see real radiance.
        let cell_w = w / COLS as f32;
        let cell_h = horizon_y / ROWS as f32;
        // Light the mountains with the sky just above the horizon, averaged
        // across the view.
        let horizon_light = (0..8)
            .map(|i| compute_sky_color(view_dir((i as f32 / 7.0 - 0.5) * FOV_X, 0.04), &sky) * EXPOSURE)
            .fold(Vec3::ZERO, |a, c| a + c / 8.0);
        let mut brightness = 0.0;
        for row in 0..ROWS {
            let v = (row as f32 + 0.5) / ROWS as f32;
            let el = EL_TOP + (EL_BOTTOM - EL_TOP) * v;
            for col in 0..COLS {
                let u = (col as f32 + 0.5) / COLS as f32;
                let dir = view_dir((u - 0.5) * FOV_X, el);
                let c = compute_sky_color(dir, &sky) * EXPOSURE;
                brightness += c.length() / (COLS * ROWS) as f32;
                engine.ui.draw_rect(
                    col as f32 * cell_w,
                    row as f32 * cell_h,
                    cell_w + 1.0,
                    cell_h + 1.0,
                    Vec4::new(c.x, c.y, c.z, 1.0),
                    true,
                );
            }
        }

        // Stars fade in as the sky goes dark.
        let star_alpha = (1.0 - brightness * 4.0).clamp(0.0, 1.0);
        if star_alpha > 0.01 {
            let twinkle = clock_t;
            let pts: Vec<UiParticle> = stars
                .iter()
                .map(|&(sx, sy, b)| {
                    let a = star_alpha * b * (0.75 + 0.25 * (twinkle * 3.0 + sx * 40.0).sin());
                    let mut p = UiParticle::new(sx * w, sy * horizon_y, 6.0, 10.0, '.', Vec4::new(0.9, 0.92, 1.0, a));
                    p.emission = a;
                    p
                })
                .collect();
            engine.ui.draw_particles(pts);
        }

        // The sun, when it is above the horizon and inside the view.
        let sun_az = sun_dir.x.atan2(-sun_dir.z);
        let sun_el = sun_dir.y.asin();
        let su = sun_az / FOV_X + 0.5;
        let sv = (sun_el - EL_TOP) / (EL_BOTTOM - EL_TOP);
        if sun_el > EL_BOTTOM && (0.0..=1.0).contains(&su) {
            let tint = compute_sky_color(sun_dir, &sky).normalize_or_zero();
            let warm = Vec3::ONE.lerp(Vec3::new(1.0, 0.55, 0.25), (1.0 - sun_el / 0.35).clamp(0.0, 1.0));
            let c = (warm * 0.8 + tint * 0.2) * 3.0;
            let mut p = UiParticle::new(su * w - 22.0, sv * horizon_y - 22.0, 44.0, 44.0, '●', Vec4::new(c.x, c.y, c.z, 1.0));
            p.emission = 4.0;
            p.glow = 3.0;
            engine.ui.draw_particles(vec![p]);
        }

        // Mountains: three ridgelines, each a sum of sines, lit by the sky.
        for layer in 0..3 {
            let lf = layer as f32;
            let base = horizon_y + lf * h * 0.05;
            let shade = horizon_light * (0.30 - lf * 0.09) + Vec3::splat(0.004);
            let step = 4.0;
            let mut x = 0.0;
            while x < w {
                let t = x / w * 9.0 + lf * 3.3;
                let ridge = (t * 0.9).sin() * 0.5 + (t * 2.3 + 1.0).sin() * 0.25 + (t * 5.1 + 2.0).sin() * 0.1;
                let top = base - (0.08 - lf * 0.02) * h * (ridge + 0.9);
                engine.ui.draw_rect(x, top, step + 1.0, h - top, Vec4::new(shade.x, shade.y, shade.z, 1.0), true);
                x += step;
            }
        }

        // HUD.
        let hours = (6.0 + day * 24.0) % 24.0;
        let clock = format!("{:02}:{:02}", hours as u32, ((hours.fract()) * 60.0) as u32);
        engine.ui.draw_text(16.0, 16.0, "Nishita sky: Rayleigh + Mie single scattering, per cell, per frame", 1.0, Vec4::new(1.0, 1.0, 1.0, 0.85));
        let status = format!(
            "{clock}   sun elevation {:+.1} deg   speed x{speed}{}   [Space] pause  [Up/Down] speed",
            sun_el.to_degrees(),
            if paused { " (paused)" } else { "" }
        );
        engine.ui.draw_text(16.0, 40.0, &status, 1.0, Vec4::new(1.0, 1.0, 1.0, 0.7));
    });
}

/// View direction for an azimuth (0 = straight ahead, looking down -Z) and an
/// elevation above the horizon, both in radians.
fn view_dir(az: f32, el: f32) -> Vec3 {
    let el = el.clamp(-FRAC_PI_2, FRAC_PI_2);
    Vec3::new(az.sin() * el.cos(), el.sin(), -az.cos() * el.cos())
}

/// Cheap deterministic hash in [0, 1).
fn hash(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43_758.547).fract().abs()
}
