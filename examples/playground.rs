//! playground — Interactive visual sandbox for the Proof Engine.
//!
//! A full staging environment where you can:
//! - Place glyphs with the mouse
//! - Switch between tools (spawn, force field, entity, particle)
//! - Adjust colors, emission, bloom in real time
//! - Add/remove force fields interactively
//! - See a live HUD with FPS, glyph count, active fields, controls
//! - Cycle through attractor types, characters, colors
//! - Camera pan with WASD, zoom with scroll
//!
//! CONTROLS:
//!   Mouse click     — Place current tool at cursor
//!   1-5             — Switch tool (Glyph, Field, Entity, Particle, Eraser)
//!   Q/E             — Cycle character set
//!   R/F             — Cycle color palette
//!   T/G             — Increase/decrease emission
//!   Y/H             — Increase/decrease glow radius
//!   Z/X             — Cycle force field type
//!   B               — Toggle bloom on/off
//!   N/M             — Bloom intensity up/down
//!   Space           — Screen shake
//!   Tab             — Toggle HUD
//!   Backspace       — Clear all
//!   WASD            — Pan camera
//!   Scroll          — Zoom
//!
//! Run: `cargo run --example playground`

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;
use proof_engine::glyph;
use proof_engine::particle;
use std::f32::consts::TAU;

// ── Tool modes ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Glyph,
    ForceField,
    Entity,
    Particle,
    Eraser,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Glyph => "GLYPH",
            Tool::ForceField => "FIELD",
            Tool::Entity => "ENTITY",
            Tool::Particle => "PARTICLE",
            Tool::Eraser => "ERASER",
        }
    }
    fn color(self) -> Vec4 {
        match self {
            Tool::Glyph => Vec4::new(0.2, 1.0, 0.5, 1.0),
            Tool::ForceField => Vec4::new(1.0, 0.5, 0.2, 1.0),
            Tool::Entity => Vec4::new(0.5, 0.3, 1.0, 1.0),
            Tool::Particle => Vec4::new(1.0, 0.2, 0.6, 1.0),
            Tool::Eraser => Vec4::new(1.0, 0.2, 0.2, 1.0),
        }
    }
}

// ── Character palettes ──────────────────────────────────────────────────────

const CHAR_SETS: &[&[char]] = &[
    &['@', '#', '*', '+', 'x', 'o', '.', '~'],                    // ASCII
    &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],        // Digits
    &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'],                  // Letters
    &['░', '▒', '▓', '█', '▄', '▀', '▌', '▐'],                  // Blocks
    &['─', '│', '┌', '┐', '└', '┘', '├', '┤'],                  // Box
    &['+', '-', '*', '/', '=', '<', '>', '%'],                    // Math
];

const CHAR_SET_NAMES: &[&str] = &["ASCII", "Digits", "Letters", "Blocks", "Box", "Math"];

// ── Color palettes ──────────────────────────────────────────────────────────

const COLOR_PALETTES: &[&[(f32, f32, f32)]] = &[
    &[(0.0, 1.0, 0.5), (0.0, 0.8, 0.4), (0.0, 0.6, 0.3)],         // Matrix green
    &[(1.0, 0.3, 0.1), (1.0, 0.6, 0.1), (1.0, 0.9, 0.3)],         // Fire
    &[(0.3, 0.5, 1.0), (0.5, 0.7, 1.0), (0.8, 0.9, 1.0)],         // Ice
    &[(0.7, 0.2, 1.0), (0.5, 0.1, 0.8), (0.9, 0.4, 1.0)],         // Void purple
    &[(1.0, 0.8, 0.2), (0.9, 0.7, 0.1), (1.0, 1.0, 0.5)],         // Gold
    &[(1.0, 1.0, 1.0), (0.8, 0.8, 0.8), (0.5, 0.5, 0.5)],         // Mono
    &[(1.0, 0.2, 0.5), (1.0, 0.4, 0.7), (0.8, 0.1, 0.4)],         // Neon pink
    &[(0.2, 0.8, 0.8), (0.1, 0.6, 0.7), (0.3, 1.0, 0.9)],         // Cyan
];

const PALETTE_NAMES: &[&str] = &[
    "Matrix", "Fire", "Ice", "Void", "Gold", "Mono", "Neon", "Cyan",
];

// ── Force field presets ─────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum FieldPreset {
    GravityWell,
    Repulsor,
    Vortex,
    LorenzAttractor,
    RosslerAttractor,
    Flow,
}

const FIELD_PRESETS: &[FieldPreset] = &[
    FieldPreset::GravityWell,
    FieldPreset::Repulsor,
    FieldPreset::Vortex,
    FieldPreset::LorenzAttractor,
    FieldPreset::RosslerAttractor,
    FieldPreset::Flow,
];

const FIELD_NAMES: &[&str] = &[
    "Gravity", "Repulsor", "Vortex", "Lorenz", "Rossler", "Flow",
];

fn make_field(preset: FieldPreset, pos: Vec3) -> ForceField {
    match preset {
        FieldPreset::GravityWell => ForceField::Gravity {
            center: pos, strength: 2.0, falloff: Falloff::InverseSquare,
        },
        FieldPreset::Repulsor => ForceField::Gravity {
            center: pos, strength: -3.0, falloff: Falloff::InverseSquare,
        },
        FieldPreset::Vortex => ForceField::Vortex {
            center: pos, axis: Vec3::Z, strength: 0.5, radius: 8.0,
        },
        FieldPreset::LorenzAttractor => ForceField::StrangeAttractor {
            attractor_type: AttractorType::Lorenz,
            scale: 0.2, strength: 0.4, center: pos,
        },
        FieldPreset::RosslerAttractor => ForceField::StrangeAttractor {
            attractor_type: AttractorType::Rossler,
            scale: 0.2, strength: 0.4, center: pos,
        },
        FieldPreset::Flow => ForceField::Flow {
            direction: Vec3::new(0.0, -1.0, 0.0),
            strength: 0.3, turbulence: 0.2,
        },
    }
}

// ── Main ────────────────────────────────────────────────────────────────────

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine — Playground".to_string(),
        window_width: 1400,
        window_height: 900,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.5,
            chromatic_aberration: 0.002,
            film_grain: 0.01,
            ..Default::default()
        },
        ..Default::default()
    });

    // State
    let mut tool = Tool::Glyph;
    let mut char_set_idx = 0usize;
    let mut palette_idx = 0usize;
    let mut field_idx = 0usize;
    let mut emission = 1.5f32;
    let mut glow_radius = 1.0f32;
    let mut show_hud = true;
    let mut glyph_count = 0u32;
    let mut field_count = 0u32;
    let mut spawn_counter = 0u32;
    let mut time = 0.0f32;
    let mut cam_x = 0.0f32;
    let mut cam_y = 0.0f32;

    // HUD glyphs (pre-spawned, updated each frame)
    let mut hud_ids: Vec<glyph::GlyphId> = Vec::new();

    // Spawn a grid of dots as reference background
    for y in -20..=20 {
        for x in -30..=30 {
            if (x + y) % 4 == 0 {
                engine.spawn_glyph(Glyph {
                    character: '.',
                    position: Vec3::new(x as f32 * 1.0, y as f32 * 1.0, -2.0),
                    color: Vec4::new(0.1, 0.1, 0.15, 0.2),
                    emission: 0.0,
                    layer: RenderLayer::Background,
                    ..Default::default()
                });
            }
        }
    }

    engine.run(move |engine, dt| {
        time += dt;
        let input = engine.input.clone();

        // ── Tool switching (1-5) ────────────────────────────────────────
        if input.just_pressed(Key::Num1) { tool = Tool::Glyph; }
        if input.just_pressed(Key::Num2) { tool = Tool::ForceField; }
        if input.just_pressed(Key::Num3) { tool = Tool::Entity; }
        if input.just_pressed(Key::Num4) { tool = Tool::Particle; }
        if input.just_pressed(Key::Num5) { tool = Tool::Eraser; }

        // ── Character set cycling (Q/E) ─────────────────────────────────
        if input.just_pressed(Key::Q) {
            char_set_idx = (char_set_idx + CHAR_SETS.len() - 1) % CHAR_SETS.len();
        }
        if input.just_pressed(Key::E) {
            char_set_idx = (char_set_idx + 1) % CHAR_SETS.len();
        }

        // ── Color palette cycling (R/F) ─────────────────────────────────
        if input.just_pressed(Key::R) {
            palette_idx = (palette_idx + 1) % COLOR_PALETTES.len();
        }
        if input.just_pressed(Key::F) {
            palette_idx = (palette_idx + COLOR_PALETTES.len() - 1) % COLOR_PALETTES.len();
        }

        // ── Emission (T/G) ──────────────────────────────────────────────
        if input.just_pressed(Key::T) { emission = (emission + 0.3).min(5.0); }
        if input.just_pressed(Key::G) { emission = (emission - 0.3).max(0.0); }

        // ── Glow radius (Y/H) ──────────────────────────────────────────
        if input.just_pressed(Key::Y) { glow_radius = (glow_radius + 0.3).min(5.0); }
        if input.just_pressed(Key::H) { glow_radius = (glow_radius - 0.3).max(0.0); }

        // ── Field preset cycling (Z/X) ─────────────────────────────────
        if input.just_pressed(Key::Z) {
            field_idx = (field_idx + FIELD_PRESETS.len() - 1) % FIELD_PRESETS.len();
        }
        if input.just_pressed(Key::X) {
            field_idx = (field_idx + 1) % FIELD_PRESETS.len();
        }

        // ── Bloom toggle (B) / intensity (N/M) ─────────────────────────
        if input.just_pressed(Key::B) {
            engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled;
        }
        if input.just_pressed(Key::N) {
            engine.config.render.bloom_intensity = (engine.config.render.bloom_intensity + 0.3).min(5.0);
        }
        if input.just_pressed(Key::M) {
            engine.config.render.bloom_intensity = (engine.config.render.bloom_intensity - 0.3).max(0.0);
        }

        // ── Screen shake (Space) ────────────────────────────────────────
        if input.just_pressed(Key::Space) {
            engine.add_trauma(0.5);
        }

        // ── HUD toggle (Tab) ───────────────────────────────────────────
        if input.just_pressed(Key::Tab) {
            show_hud = !show_hud;
        }

        // ── Camera pan (WASD) ───────────────────────────────────────────
        let cam_speed = 10.0 * dt;
        if input.is_pressed(Key::W) || input.is_pressed(Key::Up)    { cam_y += cam_speed; }
        if input.is_pressed(Key::S) || input.is_pressed(Key::Down)  { cam_y -= cam_speed; }
        if input.is_pressed(Key::A) || input.is_pressed(Key::Left)  { cam_x -= cam_speed; }
        if input.is_pressed(Key::D) || input.is_pressed(Key::Right) { cam_x += cam_speed; }
        engine.camera.position.x.target = cam_x;
        engine.camera.position.y.target = cam_y;

        // ── Mouse placement ─────────────────────────────────────────────
        if input.mouse_left_just_pressed {
            // Convert mouse position to world space (approximate)
            let mx = (input.mouse_x / engine.config.window_width as f32 - 0.5) * 30.0 + cam_x;
            let my = -(input.mouse_y / engine.config.window_height as f32 - 0.5) * 20.0 + cam_y;
            let world_pos = Vec3::new(mx, my, 0.0);

            let chars = CHAR_SETS[char_set_idx];
            let colors = COLOR_PALETTES[palette_idx];

            match tool {
                Tool::Glyph => {
                    // Spawn a cluster of 5-8 glyphs around the click
                    let count = 5 + (spawn_counter % 4) as usize;
                    for i in 0..count {
                        let angle = (i as f32 / count as f32) * TAU;
                        let r = 0.3 + (i as f32 * 0.17).sin().abs() * 0.5;
                        let ch = chars[(spawn_counter as usize + i) % chars.len()];
                        let (cr, cg, cb) = colors[i % colors.len()];
                        let brightness = 0.6 + (i as f32 * 0.2).sin().abs() * 0.4;

                        engine.spawn_glyph(Glyph {
                            character: ch,
                            position: world_pos + Vec3::new(angle.cos() * r, angle.sin() * r, 0.0),
                            color: Vec4::new(cr * brightness, cg * brightness, cb * brightness, 0.9),
                            emission,
                            glow_color: Vec3::new(cr, cg, cb),
                            glow_radius,
                            mass: 0.1,
                            layer: RenderLayer::Entity,
                            blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Breathing {
                                rate: 0.3 + (i as f32 * 0.1),
                                depth: 0.1,
                            }),
                            ..Default::default()
                        });
                        glyph_count += 1;
                    }
                    spawn_counter += 1;
                }
                Tool::ForceField => {
                    let field = make_field(FIELD_PRESETS[field_idx], world_pos);
                    engine.add_field(field);
                    field_count += 1;
                }
                Tool::Entity => {
                    // Spawn an amorphous entity
                    let mut entity = AmorphousEntity::new("Creature", world_pos);
                    entity.entity_mass = 3.0;
                    entity.cohesion = 0.7;
                    entity.pulse_rate = 0.5;
                    entity.pulse_depth = 0.15;
                    entity.hp = 100.0;
                    entity.max_hp = 100.0;

                    // Build formation from current char/color set
                    let n = 12;
                    for i in 0..n {
                        let angle = (i as f32 / n as f32) * TAU;
                        let r = 0.8 + (i as f32 * 0.3).sin().abs() * 0.5;
                        entity.formation.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
                        entity.formation_chars.push(chars[i % chars.len()]);
                        let (cr, cg, cb) = colors[i % colors.len()];
                        entity.formation_colors.push(Vec4::new(cr, cg, cb, 0.9));
                    }

                    engine.spawn_entity(entity);
                    glyph_count += n as u32;
                }
                Tool::Particle => {
                    // Burst of particles at click
                    let (cr, cg, cb) = colors[0];
                    engine.emit_particles(
                        particle::EmitterPreset::DeathExplosion {
                            color: Vec4::new(cr, cg, cb, 1.0),
                        },
                        world_pos,
                    );
                    glyph_count += 30;
                }
                Tool::Eraser => {
                    // Add a brief strong repulsor to push things away
                    engine.add_field(ForceField::Gravity {
                        center: world_pos,
                        strength: -10.0,
                        falloff: Falloff::InverseSquare,
                    });
                    engine.add_trauma(0.2);
                }
            }
        }

        // ── Clear all (Backspace) ───────────────────────────────────────
        if input.just_pressed(Key::Backspace) {
            engine.scene = SceneGraph::new();
            glyph_count = 0;
            field_count = 0;
            // Re-spawn background grid
            for y in -20..=20 {
                for x in -30..=30 {
                    if (x + y) % 4 == 0 {
                        engine.spawn_glyph(Glyph {
                            character: '.',
                            position: Vec3::new(x as f32, y as f32, -2.0),
                            color: Vec4::new(0.1, 0.1, 0.15, 0.2),
                            layer: RenderLayer::Background,
                            ..Default::default()
                        });
                    }
                }
            }
        }

        // ── HUD rendering ───────────────────────────────────────────────
        if show_hud {
            let hud_x = cam_x - 14.0;
            let hud_y = cam_y + 9.0;
            let tc = tool.color();

            // Spawn HUD text as glyphs (they'll be recreated each frame
            // in a real implementation — for now, this is the concept)
            let lines = [
                format!("PROOF ENGINE PLAYGROUND"),
                format!(""),
                format!("Tool: {} [1-5]", tool.name()),
                format!("Chars: {} [Q/E]", CHAR_SET_NAMES[char_set_idx]),
                format!("Color: {} [R/F]", PALETTE_NAMES[palette_idx]),
                format!("Field: {} [Z/X]", FIELD_NAMES[field_idx]),
                format!("Emission: {:.1} [T/G]", emission),
                format!("Glow: {:.1} [Y/H]", glow_radius),
                format!("Bloom: {} {:.1} [B/N/M]",
                    if engine.config.render.bloom_enabled { "ON" } else { "OFF" },
                    engine.config.render.bloom_intensity),
                format!(""),
                format!("Glyphs: {}  Fields: {}", glyph_count, field_count),
                format!("Click to place | Space=shake"),
                format!("WASD=pan | Tab=HUD | Bksp=clear"),
            ];

            // We render HUD by spawning short-lived glyphs for each character
            // This is hacky but demonstrates the engine's capability
            for (row, line) in lines.iter().enumerate() {
                for (col, ch) in line.chars().enumerate() {
                    if ch == ' ' { continue; }
                    engine.spawn_glyph(Glyph {
                        character: ch,
                        position: Vec3::new(
                            hud_x + col as f32 * 0.45,
                            hud_y - row as f32 * 0.6,
                            1.0,
                        ),
                        color: if row == 0 {
                            Vec4::new(1.0, 0.9, 0.3, 0.9)
                        } else if row == 2 {
                            tc
                        } else {
                            Vec4::new(0.6, 0.6, 0.7, 0.7)
                        },
                        emission: if row == 0 { 1.0 } else { 0.3 },
                        layer: RenderLayer::UI,
                        lifetime: dt * 1.5, // disappear next frame
                        ..Default::default()
                    });
                }
            }
        }
    });
}
