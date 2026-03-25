//! showcase -- Proof Engine cinematic demo (75 seconds).
//!
//! Run: cargo run --release --example showcase

use proof_engine::prelude::*;
use proof_engine::math::attractors::AttractorType;
use std::f32::consts::{PI, TAU};

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Engine -- Showcase".to_string(),
        window_width: 1920, window_height: 1080,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true, bloom_intensity: 1.0,
            chromatic_aberration: 0.001, film_grain: 0.01,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut time = 0.0f32;
    let mut phase = 0u8;
    let mut pt = 0.0f32;
    let mut done = [false; 30]; // one-shot spawn flags
    let mut rain_count = 0usize;
    let mut spiral_count = 0usize;

    // Background stars
    for i in 0..500 {
        engine.spawn_glyph(Glyph {
            character: if hf(i, 4) > 0.7 { '+' } else { '.' },
            position: Vec3::new(hf(i,0)*50.0-25.0, hf(i,1)*30.0-15.0, -4.0-hf(i,2)*5.0),
            color: Vec4::new(0.35, 0.4, 0.55, 0.15),
            emission: 0.03 + hf(i,3) * 0.05,
            layer: RenderLayer::Background,
            ..Default::default()
        });
    }

    engine.run(move |engine, dt| {
        time += dt;
        pt += dt;

        match phase {
            // ═══════════════════════════════════════════════════════════
            // VOID (0-5s): Single pulsing dot. Darkness. Anticipation.
            // ═══════════════════════════════════════════════════════════
            0 => {
                if !done[0] { done[0] = true;
                    engine.spawn_glyph(Glyph {
                        character: '.', position: Vec3::ZERO,
                        color: Vec4::new(1.0, 0.9, 0.5, 1.0), emission: 2.0,
                        glow_color: Vec3::new(1.0, 0.8, 0.3), glow_radius: 3.0,
                        layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.5, depth: 0.4 }),
                        ..Default::default()
                    });
                }
                engine.config.render.bloom_intensity = 0.5 + (pt / 5.0).min(1.0) * 1.0;
                if pt > 5.0 { phase = 1; pt = 0.0; }
            }

            // ═══════════════════════════════════════════════════════════
            // GENESIS (5-15s): Math rain + golden spiral blooming.
            // ═══════════════════════════════════════════════════════════
            1 => {
                // Spawn math rain gradually (1 per frame for first 5s)
                if pt < 5.0 && rain_count < 200 {
                    let i = rain_count;
                    let chars = ['0','1','+','-','*','/','=','<','>','x','#','@'];
                    engine.spawn_glyph(Glyph {
                        character: chars[i % chars.len()],
                        position: Vec3::new(hf(i+500,0)*28.0-14.0, 13.0, -1.0),
                        velocity: Vec3::new((hf(i+500,1)-0.5)*0.5, -2.5-hf(i+500,2)*2.0, 0.0),
                        color: Vec4::new(0.1, 0.5+hf(i+500,3)*0.5, 0.3, 0.5),
                        emission: 0.4, lifetime: 5.0,
                        layer: RenderLayer::World, blend_mode: BlendMode::Additive,
                        ..Default::default()
                    });
                    rain_count += 1;
                }

                // Spawn golden spiral gradually (1 per frame after 2s)
                if pt > 2.0 && spiral_count < 180 {
                    let i = spiral_count;
                    let phi = 1.618033988_f32;
                    let angle = i as f32 * TAU / phi;
                    let r = (i as f32).sqrt() * 0.4;
                    let t = i as f32 / 180.0;
                    let chars = ['*','+','o','.','x','#','@','O',':','~'];
                    engine.spawn_glyph(Glyph {
                        character: chars[i % chars.len()],
                        position: Vec3::new(r * angle.cos(), r * angle.sin(), 0.0),
                        color: Vec4::new(1.0-t*0.2, 0.7-t*0.3, 0.2+t*0.6, 0.85),
                        emission: 1.8-t*0.8,
                        glow_color: Vec3::new(1.0, 0.6, 0.2), glow_radius: 1.5-t,
                        mass: 0.05, layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.2+t*0.4, depth: 0.08 }),
                        ..Default::default()
                    });
                    spiral_count += 1;
                }

                if !done[1] { done[1] = true;
                    engine.add_field(ForceField::Gravity { center: Vec3::ZERO, strength: 0.4, falloff: Falloff::InverseSquare });
                }

                engine.config.render.bloom_intensity = 1.5 + (pt * 0.5).sin() * 0.3;
                if pt > 10.0 { phase = 2; pt = 0.0; }
            }

            // ═══════════════════════════════════════════════════════════
            // THE EQUATIONS (15-27s): Three attractors, 1500 particles.
            // ═══════════════════════════════════════════════════════════
            2 => {
                if !done[2] { done[2] = true;
                    engine.config.render.bloom_intensity = 2.2;
                    engine.config.render.chromatic_aberration = 0.003;

                    // Lorenz (left, blue)
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Lorenz,
                        scale: 0.12, strength: 0.4, center: Vec3::new(-6.0, 0.0, 0.0),
                    });
                    for i in 0..500 {
                        let t = i as f32 / 500.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(-6.0+hf(i+2000,0)*4.0-2.0, hf(i+2000,1)*4.0-2.0, 0.0),
                            color: Vec4::new(0.2+t*0.3, 0.5+t*0.3, 1.0, 0.7), emission: 0.7+t*0.3,
                            glow_color: Vec3::new(0.3, 0.6, 1.0), glow_radius: 0.4,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Rossler (center-top, pink)
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Rossler,
                        scale: 0.12, strength: 0.4, center: Vec3::new(0.0, 3.0, 0.0),
                    });
                    for i in 0..500 {
                        let t = i as f32 / 500.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(hf(i+3000,0)*4.0-2.0, 3.0+hf(i+3000,1)*4.0-2.0, 0.0),
                            color: Vec4::new(1.0, 0.2+t*0.3, 0.5+t*0.3, 0.7), emission: 0.7+t*0.3,
                            glow_color: Vec3::new(1.0, 0.3, 0.6), glow_radius: 0.4,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Aizawa (right, green)
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Aizawa,
                        scale: 0.15, strength: 0.35, center: Vec3::new(6.0, -1.0, 0.0),
                    });
                    for i in 0..500 {
                        let t = i as f32 / 500.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(6.0+hf(i+4000,0)*4.0-2.0, -1.0+hf(i+4000,1)*4.0-2.0, 0.0),
                            color: Vec4::new(0.5+t*0.5, 0.8, 0.2+t*0.3, 0.7), emission: 0.7+t*0.3,
                            glow_color: Vec3::new(0.8, 1.0, 0.3), glow_radius: 0.4,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                engine.add_trauma(0.003 * dt);
                if pt > 12.0 { phase = 3; pt = 0.0; }
            }

            // ═══════════════════════════════════════════════════════════
            // LIFE (27-37s): Four entities orbit each other.
            // ═══════════════════════════════════════════════════════════
            3 => {
                if !done[3] { done[3] = true;
                    engine.config.render.bloom_intensity = 1.8;
                    engine.add_field(ForceField::Vortex { center: Vec3::ZERO, axis: Vec3::Z, strength: 0.15, radius: 8.0 });

                    let configs: [(Vec3, Vec4, &[char]); 4] = [
                        (Vec3::new(-3.0,3.0,0.0), Vec4::new(1.0,0.3,0.4,0.9), &['@','#','*','+','X','o']),
                        (Vec3::new(3.0,3.0,0.0), Vec4::new(0.3,0.6,1.0,0.9), &['O','o','.',':','+','=']),
                        (Vec3::new(-3.0,-3.0,0.0), Vec4::new(0.4,1.0,0.3,0.9), &['x','X','*','+','#','@']),
                        (Vec3::new(3.0,-3.0,0.0), Vec4::new(0.9,0.6,1.0,0.9), &['*','.','o','O','+','#']),
                    ];
                    for (ci, (pos, color, chars)) in configs.iter().enumerate() {
                        let mut ent = AmorphousEntity::new(&format!("Entity_{}", ci), *pos);
                        ent.entity_mass = 3.5; ent.cohesion = 0.75;
                        ent.pulse_rate = 0.4; ent.pulse_depth = 0.18;
                        ent.hp = 100.0; ent.max_hp = 100.0;
                        for i in 0..18 {
                            let a = (i as f32 / 18.0) * TAU;
                            let ring = if i < 6 { 0.4 } else if i < 12 { 0.8 } else { 1.2 };
                            ent.formation.push(Vec3::new(a.cos()*ring, a.sin()*ring, 0.0));
                            ent.formation_chars.push(chars[i % chars.len()]);
                            ent.formation_colors.push(*color);
                        }
                        engine.spawn_entity(ent);
                    }
                    // Orbiting wisps
                    for i in 0..60 {
                        let a = hf(i+6000,0)*TAU; let r = 4.0+hf(i+6000,1)*4.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(r*a.cos(), r*a.sin(), -0.2),
                            color: Vec4::new(0.5+hf(i+6000,2)*0.5, 0.3, 1.0-hf(i+6000,2)*0.5, 0.3),
                            emission: 0.4, mass: 0.01,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: r, speed: 0.03, eccentricity: 0.2 }),
                            ..Default::default()
                        });
                    }
                }
                if pt > 10.0 { phase = 4; pt = 0.0; }
            }

            // ═══════════════════════════════════════════════════════════
            // WAR (37-47s): Shockwaves. Debris. Dissolution.
            // ═══════════════════════════════════════════════════════════
            4 => {
                // Shockwave 1 at 2s
                if pt > 2.0 && !done[5] { done[5] = true;
                    engine.add_field(ForceField::Shockwave { center: Vec3::ZERO, speed: 7.0, strength: 5.0, thickness: 2.5, born_at: time });
                    engine.add_trauma(0.9);
                    engine.config.render.bloom_intensity = 3.5;
                    for i in 0..400 {
                        let a = (i as f32/400.0)*TAU*2.0;
                        let spd = 1.5+hf(i+7000,0)*5.0; let t = i as f32/400.0;
                        let (r,g,b) = if t<0.25{(1.0,0.9,0.3)}else if t<0.5{(1.0,0.4,0.2)}else if t<0.75{(0.8,0.2,0.6)}else{(0.3,0.3,1.0)};
                        engine.spawn_glyph(Glyph {
                            character: ['*','+','.','x','o','#'][i%6],
                            position: Vec3::new(hf(i+7000,1)*0.5-0.25, hf(i+7000,2)*0.5-0.25, 0.0),
                            velocity: Vec3::new(a.cos()*spd, a.sin()*spd, 0.0),
                            color: Vec4::new(r,g,b,0.9), emission: 2.0,
                            glow_color: Vec3::new(r,g,b), glow_radius: 1.2,
                            mass: 0.04, lifetime: 3.0+hf(i+7000,3)*3.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                // Shockwave 2 at 5s
                if pt > 5.0 && !done[6] { done[6] = true;
                    engine.add_field(ForceField::Shockwave { center: Vec3::new(2.0,-1.0,0.0), speed: 5.0, strength: 3.0, thickness: 2.0, born_at: time });
                    engine.add_trauma(0.6);
                    for i in 0..200 {
                        let a = hf(i+8000,0)*TAU; let spd = 2.0+hf(i+8000,1)*3.0;
                        engine.spawn_glyph(Glyph {
                            character: ['.','+','*'][i%3],
                            position: Vec3::new(2.0+hf(i+8000,2)*0.3-0.15, -1.0+hf(i+8000,3)*0.3-0.15, 0.0),
                            velocity: Vec3::new(a.cos()*spd, a.sin()*spd, 0.0),
                            color: Vec4::new(0.6,0.3,1.0,0.8), emission: 1.5,
                            mass: 0.03, lifetime: 2.0+hf(i+8000,4)*2.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                if pt > 3.0 { engine.config.render.bloom_intensity = (3.5-(pt-3.0)*0.3).max(1.5); }
                if pt > 10.0 { phase = 5; pt = 0.0; }
            }

            // ═══════════════════════════════════════════════════════════
            // SUPERNOVA (47-57s): Collapse. Explosion. Nebula.
            // ═══════════════════════════════════════════════════════════
            5 => {
                if !done[8] { done[8] = true;
                    engine.add_field(ForceField::Gravity { center: Vec3::ZERO, strength: 6.0, falloff: Falloff::InverseSquare });
                    engine.config.render.chromatic_aberration = 0.006;
                }
                // Collapse tension (0-4s)
                if pt < 4.0 {
                    engine.add_trauma(0.015*dt*(pt+1.0));
                }
                // EXPLOSION at 4s
                if pt > 4.0 && !done[9] { done[9] = true;
                    engine.add_field(ForceField::Shockwave { center: Vec3::ZERO, speed: 12.0, strength: 10.0, thickness: 5.0, born_at: time });
                    engine.add_trauma(1.0);
                    engine.config.render.bloom_intensity = 5.0;
                    for i in 0..1000 {
                        let a = (i as f32/1000.0)*TAU*5.0;
                        let spd = 0.5+hf(i+9000,0)*10.0; let t = i as f32/1000.0;
                        let (r,g,b) = if t<0.15{(1.0,1.0,0.9)}else if t<0.3{(1.0,0.9,0.3)}else if t<0.5{(1.0,0.4,0.1)}else if t<0.7{(0.9,0.15,0.5)}else if t<0.85{(0.5,0.1,0.8)}else{(0.15,0.1,0.5)};
                        engine.spawn_glyph(Glyph {
                            character: ['#','*','@','+','x','o','X','.'][i%8],
                            position: Vec3::new(hf(i+9000,1)*0.3-0.15, hf(i+9000,2)*0.3-0.15, 0.0),
                            velocity: Vec3::new(a.cos()*spd, a.sin()*spd, (hf(i+9000,3)-0.5)*2.0),
                            color: Vec4::new(r,g,b,0.95), emission: 3.5-t*2.0,
                            glow_color: Vec3::new(r,g,b), glow_radius: 2.5-t*1.5,
                            mass: 0.02, lifetime: 2.0+hf(i+9000,4)*6.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                // Nebula at 7s
                if pt > 7.0 && !done[10] { done[10] = true;
                    engine.add_field(ForceField::StrangeAttractor { attractor_type: AttractorType::Rossler, scale: 0.18, strength: 0.25, center: Vec3::ZERO });
                    for i in 0..400 {
                        let t = i as f32/400.0;
                        let c = if t<0.25{Vec4::new(0.5,0.1,0.8,0.35)}else if t<0.5{Vec4::new(0.1,0.3,0.9,0.3)}else if t<0.75{Vec4::new(0.8,0.15,0.3,0.25)}else{Vec4::new(0.2,0.7,0.4,0.2)};
                        engine.spawn_glyph(Glyph {
                            character: '.', position: Vec3::new(hf(i+11000,0)*6.0-3.0, hf(i+11000,1)*6.0-3.0, 0.0),
                            color: c, emission: 0.4, glow_color: Vec3::new(c.x,c.y,c.z), glow_radius: 1.5,
                            mass: 0.015, layer: RenderLayer::World, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                if pt > 5.0 { engine.config.render.bloom_intensity = (5.0-(pt-5.0)*0.6).max(1.5); }
                if pt > 10.0 { phase = 6; pt = 0.0; }
            }

            // ═══════════════════════════════════════════════════════════
            // THE GALAXY (57-75s): 2000 stars. Spiral arms. Kepler orbits.
            // ═══════════════════════════════════════════════════════════
            6 => {
                if !done[11] { done[11] = true;
                    engine.config.render.bloom_intensity = 2.0;
                    engine.config.render.chromatic_aberration = 0.002;
                    engine.config.render.film_grain = 0.008;
                    engine.add_field(ForceField::Gravity { center: Vec3::ZERO, strength: 1.8, falloff: Falloff::InverseSquare });
                    engine.add_field(ForceField::Vortex { center: Vec3::ZERO, axis: Vec3::Z, strength: 0.15, radius: 25.0 });

                    for i in 0..2000 {
                        let arm = i % 4;
                        let t = (i as f32/2000.0).sqrt();
                        let r = t * 18.0;
                        let spiral = (arm as f32)*TAU/4.0 + t*3.5*PI + (hf(i+12000,0)-0.5)*0.5;
                        let x = r * spiral.cos();
                        let y = r * spiral.sin();
                        let b = 0.1 + (1.0-t)*0.7;
                        let (cr,cg,cb) = if r<2.5{(0.85,0.85,1.0)}else if r<7.0{(1.0,0.85,0.45)}else if r<12.0{(1.0,0.6,0.25)}else{(0.9,0.35,0.15)};
                        engine.spawn_glyph(Glyph {
                            character: if r<3.0{'*'}else if hf(i+12000,3)>0.8{'+'}else{'.'},
                            position: Vec3::new(x, y, (hf(i+12000,1)-0.5)*0.2),
                            color: Vec4::new(cr*b, cg*b, cb*b, 0.85), emission: b*1.3,
                            glow_color: Vec3::new(cr,cg,cb), glow_radius: if r<3.0{1.5}else{0.3},
                            mass: 0.03, layer: RenderLayer::World, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: r, speed: 0.015+(1.0-t)*0.04, eccentricity: 0.08+hf(i+12000,2)*0.15 }),
                            ..Default::default()
                        });
                    }
                    // Accretion disk
                    for i in 0..30 {
                        let a = (i as f32/30.0)*TAU;
                        engine.spawn_glyph(Glyph {
                            character: '#', position: Vec3::new(a.cos()*1.0, a.sin()*1.0, 0.05),
                            color: Vec4::new(1.0,0.8,0.35,0.8), emission: 4.0,
                            glow_color: Vec3::new(1.0,0.6,0.2), glow_radius: 5.0,
                            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: 1.0, speed: 0.25, eccentricity: 0.0 }),
                            ..Default::default()
                        });
                    }
                }
                engine.config.render.bloom_intensity = 2.0 + (pt*0.4).sin()*0.25;
                // hold forever
            }
            _ => {}
        }
    });
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
