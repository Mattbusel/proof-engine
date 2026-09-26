//! math_rain: columns of mathematical symbols falling like digital rain.
//!
//! A hundred columns fall at speeds set by `|sin(0.13 c)|`, so neighbouring
//! columns drift in and out of step. Each column's symbols are re-chosen by
//! its own logistic map, `x -> r x (1 - x)` with `r` near 3.9, so which
//! symbols flicker is chaotic rather than random. The leading glyphs are
//! white-green and bright enough to bloom; the tail fades to dark green.
//!
//! Positions come from the column equations every frame, so the rain runs
//! steadily for as long as the window is open.
//!
//! Esc quits.
//!
//! Run: `cargo run --release --example math_rain`

use proof_engine::glyph::GlyphId;
use proof_engine::prelude::*;

const COLUMNS: usize = 100;
const CHARS_PER_COL: usize = 25;
const COL_SPACING: f32 = 0.8;
const ROW_SPACING: f32 = 0.7;
/// Height of the band a column falls through before it wraps to the top.
const SPAN: f32 = 72.0;
const TOP: f32 = 30.0;

struct Column {
    x: f32,
    speed: f32,
    phase: f32,
    /// Logistic map state and parameter for this column.
    lx: f32,
    r: f32,
    glyphs: Vec<GlyphId>,
}

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine: mathematical rain".to_string(),
        window_width: 1400,
        window_height: 900,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.2,
            ..Default::default()
        },
        ..Default::default()
    });

    let symbols: Vec<char> = "0123456789ABCDEFabcdef+-*/=<>()[]{}|&^~%#@!?:;xXoO".chars().collect();

    let mut columns: Vec<Column> = Vec::with_capacity(COLUMNS);
    for col in 0..COLUMNS {
        let x = (col as f32 - COLUMNS as f32 / 2.0 + 0.5) * COL_SPACING;
        let speed = 3.0 + (col as f32 * 0.13).sin().abs() * 6.0;
        let phase = (col as f32 * 0.618_034).fract() * SPAN;
        let mut glyphs = Vec::with_capacity(CHARS_PER_COL);

        for row in 0..CHARS_PER_COL {
            // Row 0 is the leading edge, at the bottom of the column.
            let depth = row as f32 / CHARS_PER_COL as f32;
            let (color, emission) = if row == 0 {
                (Vec4::new(0.85, 1.0, 0.9, 1.0), 1.8)
            } else if row < 3 {
                (Vec4::new(0.4, 1.0, 0.6, 1.0), 1.1)
            } else {
                // Exponential fade along the tail.
                let f = (-depth * 2.4).exp();
                (Vec4::new(0.05 * f, 0.85 * f, 0.3 * f, 0.35 + 0.6 * f), 0.2 + 0.6 * f)
            };
            let id = engine.spawn_glyph(Glyph {
                character: symbols[(col * 31 + row * 7) % symbols.len()],
                position: Vec3::new(x, 0.0, -depth * 0.5),
                color,
                emission,
                glow_color: Vec3::new(0.1, 0.8, 0.3),
                glow_radius: if row == 0 { 1.2 } else { 0.3 },
                layer: if row == 0 { RenderLayer::Entity } else { RenderLayer::World },
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
            glyphs.push(id);
        }

        columns.push(Column {
            x,
            speed,
            phase,
            lx: 0.1 + (col as f32 * 0.17).fract() * 0.8,
            r: 3.82 + (col as f32 * 0.29).fract() * 0.17,
            glyphs,
        });
    }

    // Fit all hundred columns across the window.
    let half_w = COLUMNS as f32 * COL_SPACING * 0.5;
    let half_h = half_w * 900.0 / 1400.0;
    let dist = half_h / (30.0_f32).to_radians().tan();
    engine.camera.set_position_instant(Vec3::new(0.0, 0.0, dist));

    let mut time = 0.0_f32;
    let mut flicker = 0.0_f32;

    engine.run(move |engine, dt| {
        if engine.input.just_pressed(Key::Escape) {
            engine.input.quit_requested = true;
        }
        time += dt;
        flicker += dt;
        // Step every column's logistic map about twenty times a second.
        let step_maps = flicker > 0.05;
        if step_maps {
            flicker = 0.0;
        }

        for c in columns.iter_mut() {
            let head = TOP - (c.speed * time + c.phase).rem_euclid(SPAN);
            if step_maps {
                c.lx = c.r * c.lx * (1.0 - c.lx);
            }
            // The map's value picks one row to change and what it becomes.
            let row_to_change = (c.lx * CHARS_PER_COL as f32) as usize % CHARS_PER_COL;
            for (row, id) in c.glyphs.iter().enumerate() {
                if let Some(g) = engine.scene.glyphs.get_mut(*id) {
                    g.position.x = c.x;
                    g.position.y = head + row as f32 * ROW_SPACING;
                    if step_maps && (row == row_to_change || row == 0) {
                        let k = ((c.lx * 9973.0) as usize + row * 7) % symbols.len();
                        g.character = symbols[k];
                    }
                }
            }
        }
    });
}
