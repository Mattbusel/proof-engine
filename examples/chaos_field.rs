//! chaos_field — the living mathematical background.
//!
//! Spawns 2000+ glyphs, each driven by one of the 10 chaos engine math functions.
//! Demonstrates: force fields, math function diversity, entropy evolution.
//!
//! Run: `cargo run --example chaos_field`

use proof_engine::prelude::*;

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Chaos Field".to_string(),
        window_width: 1280,
        window_height: 800,
        ..Default::default()
    });

    // Mathematical symbol set for chaos field glyphs
    const SYMBOLS: &[char] = &[
        '∞', '∑', '∫', '∂', '∇', '∆', '∏', 'λ', 'α', 'β', 'γ', 'δ', 'ε', 'π', 'σ', 'φ', 'ψ', 'ω',
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
        '+', '-', '×', '÷', '=', '<', '>',
        '░', '▒', '▓', '█',
    ];

    // Spawn 2000 glyphs distributed across the screen in 3 depth layers
    for i in 0..2000usize {
        let x = (i % 80) as f32 * 1.5 - 60.0;
        let y = (i / 80) as f32 * 1.2 - 15.0;
        let z = -((i % 3) as f32) * 2.0;  // 3 parallax layers

        let ch = SYMBOLS[i % SYMBOLS.len()];

        // Assign one of 10 math functions based on glyph index
        let behavior = match i % 10 {
            0 => MathFunction::Lorenz { sigma: 10.0, rho: 28.0, beta: 2.67, scale: 0.008 },
            1 => MathFunction::Sine { amplitude: 0.3, frequency: 0.3 + (i % 7) as f32 * 0.05, phase: i as f32 * 0.7 },
            2 => MathFunction::LogisticMap { r: 3.7 + (i % 5) as f32 * 0.05, x0: (i as f32 * 0.137).fract() },
            3 => MathFunction::Collatz { seed: i as u64 + 1, scale: 0.2 },
            4 => MathFunction::GoldenSpiral { center: Vec3::ZERO, scale: 0.1, speed: 0.1 + (i % 4) as f32 * 0.03 },
            5 => MathFunction::Breathing { rate: 0.2 + (i % 6) as f32 * 0.04, depth: 0.15 },
            6 => MathFunction::Orbit { center: Vec3::ZERO, radius: 0.5 + (i % 8) as f32 * 0.1, speed: 0.2 + (i % 5) as f32 * 0.04, eccentricity: 0.3 },
            7 => MathFunction::MandelbrotEscape { c_real: -0.7 + (i % 10) as f32 * 0.03, c_imag: 0.27, scale: 0.1 },
            8 => MathFunction::Perlin { frequency: 0.5 + (i % 4) as f32 * 0.1, octaves: 3, amplitude: 0.2 },
            _ => MathFunction::Exponential { start: 0.0, rate: 0.3 + (i % 7) as f32 * 0.05, target: (i as f32 * 0.23).sin() },
        };

        // Brightness tiers: background = dim, mid = medium, near = brighter
        let brightness = 0.06 + (z.abs() / 6.0) * 0.1;
        let hue = (i as f32 * 0.13).fract();
        let color = Vec4::new(
            brightness * (1.0 + hue),
            brightness * (0.8 + (hue * 2.0).fract()),
            brightness * (1.5 - hue),
            1.0,
        );

        engine.spawn_glyph(Glyph {
            character: ch,
            position: Vec3::new(x, y, z),
            color,
            emission: brightness * 2.0,
            life_function: Some(behavior),
            layer: RenderLayer::Background,
            mass: 0.1,
            entropy: (i as f32 * 0.07).fract(),
            ..Default::default()
        });
    }

    // Downward flow field (like scrolling text)
    engine.add_field(ForceField::Flow {
        direction: Vec3::new(0.0, -1.0, 0.0),
        strength: 0.05,
        turbulence: 0.3,
    });

    // Gentle central gravity
    engine.add_field(ForceField::Gravity {
        center: Vec3::ZERO,
        strength: 0.02,
        falloff: Falloff::Linear,
    });

    let mut _time = 0.0f32;
    engine.run(move |_engine, dt| {
        _time += dt;
        // As time progresses, entropy increases (simulating corruption)
        // In the full integration, this tracks misery_index from game state
    });
}
