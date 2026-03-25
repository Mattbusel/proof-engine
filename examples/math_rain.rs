//! math_rain — mathematical symbols cascading like digital rain.
//!
//! Thousands of equations, symbols, and numbers fall in columns,
//! each character driven by its own math function. Some columns
//! accelerate (exponential), some oscillate (sine), some are chaotic
//! (logistic map). Color shifts from green to gold to white at the
//! leading edge. Heavy bloom makes it glow.
//!
//! Think Matrix, but the math is real.
//!
//! Run: `cargo run --example math_rain`

use proof_engine::prelude::*;
use std::f32::consts::TAU;

const COLUMNS: usize = 100;
const CHARS_PER_COL: usize = 25;
const COL_SPACING: f32 = 0.8;
const ROW_SPACING: f32 = 0.7;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Mathematical Rain".to_string(),
        window_width: 1400,
        window_height: 900,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 2.5,
            film_grain: 0.03,
            ..Default::default()
        },
        ..Default::default()
    });

    let symbols: Vec<char> = "∂∇∫∑∏√∞≈≠≤≥±∓∈∉⊂⊃∪∩∧∨¬∀∃αβγδεζηθλμπσφψω0123456789+-×÷=<>()[]{}".chars().collect();

    // Each column is a stream of falling characters
    for col in 0..COLUMNS {
        let x = (col as f32 - COLUMNS as f32 / 2.0) * COL_SPACING;
        let col_speed = 0.3 + (col as f32 * 0.13).sin().abs() * 0.7; // varying speeds
        let col_phase = (col as f32 * 0.37).fract() * 20.0; // staggered start

        for row in 0..CHARS_PER_COL {
            let y = (CHARS_PER_COL as f32 / 2.0 - row as f32) * ROW_SPACING + col_phase;
            let char_idx = (col * 31 + row * 7) % symbols.len();

            // Leading characters (bottom of column) are brightest
            let depth = row as f32 / CHARS_PER_COL as f32;
            let is_leader = row < 3;

            let (color, emission) = if is_leader {
                // White-green bright leader
                (Vec4::new(0.8, 1.0, 0.9, 1.0), 3.0)
            } else if depth < 0.3 {
                // Bright green
                (Vec4::new(0.1, 0.9, 0.3, 0.9), 1.5)
            } else if depth < 0.7 {
                // Medium green
                (Vec4::new(0.05, 0.5, 0.15, 0.6), 0.8)
            } else {
                // Dim, fading tail
                (Vec4::new(0.02, 0.2, 0.08, 0.3), 0.3)
            };

            // Each character has its own mathematical behavior
            let behavior = match (col + row) % 8 {
                0 => MathFunction::Linear { slope: -col_speed, offset: y },
                1 => MathFunction::Sine {
                    amplitude: 0.3,
                    frequency: 0.2 + depth * 0.3,
                    phase: col as f32 * 0.5,
                },
                2 => MathFunction::Exponential {
                    start: y,
                    rate: -col_speed * 0.3,
                    target: -15.0,
                },
                3 => MathFunction::LogisticMap {
                    r: 3.5 + depth * 0.4,
                    x0: (col as f32 * 0.17).fract(),
                },
                4 => MathFunction::Collatz {
                    seed: (col * CHARS_PER_COL + row) as u64 + 1,
                    scale: 0.05,
                },
                5 => MathFunction::Perlin {
                    frequency: 0.3,
                    octaves: 2,
                    amplitude: 0.4,
                },
                6 => MathFunction::Breathing {
                    rate: 0.3 + col_speed * 0.2,
                    depth: 0.1,
                },
                _ => MathFunction::Linear { slope: -col_speed * 1.5, offset: y },
            };

            engine.spawn_glyph(Glyph {
                character: symbols[char_idx],
                position: Vec3::new(x, y, -depth * 0.5),
                color,
                emission,
                glow_color: Vec3::new(0.1, 0.8, 0.3),
                glow_radius: if is_leader { 2.0 } else { 0.5 },
                mass: 0.01,
                layer: if is_leader { RenderLayer::Entity } else { RenderLayer::World },
                blend_mode: BlendMode::Additive,
                life_function: Some(behavior),
                ..Default::default()
            });
        }
    }

    // Occasional bright flashes — "equation solved" moments
    for i in 0..20 {
        let x = ((i as f32 * 3.7).sin()) * (COLUMNS as f32 * COL_SPACING * 0.4);
        let y = ((i as f32 * 2.3).cos()) * 8.0;
        engine.spawn_glyph(Glyph {
            character: '=',
            position: Vec3::new(x, y, 0.5),
            color: Vec4::new(1.0, 1.0, 1.0, 0.0), // starts invisible
            emission: 5.0,
            glow_color: Vec3::new(0.5, 1.0, 0.7),
            glow_radius: 4.0,
            mass: 0.0,
            layer: RenderLayer::Overlay,
            blend_mode: BlendMode::Additive,
            life_function: Some(MathFunction::Breathing {
                rate: 0.05 + (i as f32 * 0.02),
                depth: 0.8,
            }),
            ..Default::default()
        });
    }

    // Downward flow field
    engine.add_field(ForceField::Flow {
        direction: Vec3::new(0.0, -1.0, 0.0),
        strength: 0.3,
        turbulence: 0.05,
    });

    engine.run(|_engine, _dt| {});
}
