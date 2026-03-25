//! showcase -- Proof Engine cinematic demo (75 seconds).
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
    let mut phase: i32 = -1;
    let mut pt = 0.0f32;
    let mut spiral_i = 0usize;
    let mut rain_i = 0usize;

    // Scale for small UI-sized glyphs vs normal scene glyphs
    let sm = Vec2::splat(0.3); // small (rain, dots)
    let md = Vec2::splat(0.5); // medium (particles)
    let lg = Vec2::splat(0.8); // large (entities, spiral)

    engine.run(move |engine, dt| {
        time += dt;

        let new_phase = if time < 5.0 { 0 }
            else if time < 15.0 { 1 }
            else if time < 27.0 { 2 }
            else if time < 37.0 { 3 }
            else if time < 47.0 { 4 }
            else if time < 57.0 { 5 }
            else { 6 };

        if new_phase != phase {
            phase = new_phase;
            pt = 0.0;
            spiral_i = 0;
            rain_i = 0;
            engine.scene = SceneGraph::new();
            spawn_stars(engine);

            match phase {
                0 => { // VOID -- single pulsing dot
                    engine.config.render.bloom_intensity = 0.8;
                    engine.config.render.film_grain = 0.02;
                    engine.spawn_glyph(Glyph {
                        character: '*', position: Vec3::ZERO, scale: lg,
                        color: Vec4::new(1.0, 0.9, 0.5, 1.0), emission: 3.0,
                        glow_color: Vec3::new(1.0, 0.8, 0.3), glow_radius: 4.0,
                        layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.5, depth: 0.4 }),
                        ..Default::default()
                    });
                }
                1 => { // GENESIS -- rain + spiral
                    engine.config.render.bloom_intensity = 1.5;
                    engine.config.render.film_grain = 0.01;
                    engine.add_field(ForceField::Gravity { center: Vec3::ZERO, strength: 0.3, falloff: Falloff::InverseSquare });
                    // The central glow persists
                    engine.spawn_glyph(Glyph {
                        character: '*', position: Vec3::ZERO, scale: lg,
                        color: Vec4::new(1.0, 0.85, 0.3, 1.0), emission: 2.5,
                        glow_color: Vec3::new(1.0, 0.7, 0.2), glow_radius: 3.0,
                        layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.6, depth: 0.2 }),
                        ..Default::default()
                    });
                }
                2 => { // EQUATIONS -- three attractors
                    engine.config.render.bloom_intensity = 2.2;
                    engine.config.render.chromatic_aberration = 0.003;

                    // Lorenz (left, blue) -- particles close to center
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Lorenz, scale: 0.12, strength: 0.4,
                        center: Vec3::new(-5.0, 0.0, 0.0),
                    });
                    for i in 0..400 {
                        let t = i as f32/400.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: sm,
                            position: Vec3::new(-5.0+hf(i+2000,0)*2.0-1.0, hf(i+2000,1)*2.0-1.0, 0.0),
                            color: Vec4::new(0.2+t*0.3, 0.5+t*0.3, 1.0, 0.8), emission: 1.0,
                            glow_color: Vec3::new(0.3, 0.6, 1.0), glow_radius: 0.5,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Rossler (center-top, pink)
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Rossler, scale: 0.12, strength: 0.4,
                        center: Vec3::new(0.0, 2.5, 0.0),
                    });
                    for i in 0..400 {
                        let t = i as f32/400.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: sm,
                            position: Vec3::new(hf(i+3000,0)*2.0-1.0, 2.5+hf(i+3000,1)*2.0-1.0, 0.0),
                            color: Vec4::new(1.0, 0.2+t*0.3, 0.5+t*0.3, 0.8), emission: 1.0,
                            glow_color: Vec3::new(1.0, 0.3, 0.6), glow_radius: 0.5,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }

                    // Aizawa (right, green)
                    engine.add_field(ForceField::StrangeAttractor {
                        attractor_type: AttractorType::Aizawa, scale: 0.15, strength: 0.35,
                        center: Vec3::new(5.0, -1.0, 0.0),
                    });
                    for i in 0..400 {
                        let t = i as f32/400.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: sm,
                            position: Vec3::new(5.0+hf(i+4000,0)*2.0-1.0, -1.0+hf(i+4000,1)*2.0-1.0, 0.0),
                            color: Vec4::new(0.5+t*0.5, 0.8, 0.2+t*0.3, 0.8), emission: 1.0,
                            glow_color: Vec3::new(0.8, 1.0, 0.3), glow_radius: 0.5,
                            mass: 0.06, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                3 => { // LIFE -- four entities
                    engine.config.render.bloom_intensity = 1.8;
                    engine.config.render.chromatic_aberration = 0.002;
                    engine.add_field(ForceField::Vortex { center: Vec3::ZERO, axis: Vec3::Z, strength: 0.12, radius: 8.0 });

                    let cfgs: [(Vec3, Vec4, [char;6]); 4] = [
                        (Vec3::new(-3.0,2.5,0.0), Vec4::new(1.0,0.3,0.4,0.9), ['@','#','*','+','X','o']),
                        (Vec3::new(3.0,2.5,0.0), Vec4::new(0.3,0.6,1.0,0.9), ['O','o','.',':','+','=']),
                        (Vec3::new(-3.0,-2.5,0.0), Vec4::new(0.4,1.0,0.3,0.9), ['x','X','*','+','#','@']),
                        (Vec3::new(3.0,-2.5,0.0), Vec4::new(0.9,0.6,1.0,0.9), ['*','.','o','O','+','#']),
                    ];
                    for (ci,(pos,color,chars)) in cfgs.iter().enumerate() {
                        let mut ent = AmorphousEntity::new(&format!("E{}",ci), *pos);
                        ent.entity_mass = 3.5; ent.cohesion = 0.75; ent.pulse_rate = 0.4; ent.pulse_depth = 0.18;
                        ent.hp = 100.0; ent.max_hp = 100.0;
                        for i in 0..18 {
                            let a = (i as f32/18.0)*TAU;
                            let r = if i<6{0.4}else if i<12{0.7}else{1.0};
                            ent.formation.push(Vec3::new(a.cos()*r, a.sin()*r, 0.0));
                            ent.formation_chars.push(chars[i%chars.len()]);
                            ent.formation_colors.push(*color);
                        }
                        engine.spawn_entity(ent);
                    }
                    // Ambient wisps
                    for i in 0..50 {
                        let a = hf(i+6000,0)*TAU; let r = 3.5+hf(i+6000,1)*3.0;
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: sm,
                            position: Vec3::new(r*a.cos(), r*a.sin(), -0.2),
                            color: Vec4::new(0.5+hf(i+6000,2)*0.5, 0.3, 1.0-hf(i+6000,2)*0.5, 0.3),
                            emission: 0.5, mass: 0.01,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: r, speed: 0.03, eccentricity: 0.2 }),
                            ..Default::default()
                        });
                    }
                }
                4 => { // WAR -- re-spawn entities + vortex
                    engine.config.render.bloom_intensity = 2.0;
                    engine.add_field(ForceField::Vortex { center: Vec3::ZERO, axis: Vec3::Z, strength: 0.2, radius: 10.0 });
                    let cfgs: [(Vec3, Vec4, [char;6]); 4] = [
                        (Vec3::new(-2.5,2.0,0.0), Vec4::new(1.0,0.3,0.4,0.9), ['@','#','*','+','X','o']),
                        (Vec3::new(2.5,2.0,0.0), Vec4::new(0.3,0.6,1.0,0.9), ['O','o','.',':','+','=']),
                        (Vec3::new(-2.5,-2.0,0.0), Vec4::new(0.4,1.0,0.3,0.9), ['x','X','*','+','#','@']),
                        (Vec3::new(2.5,-2.0,0.0), Vec4::new(0.9,0.6,1.0,0.9), ['*','.','o','O','+','#']),
                    ];
                    for (ci,(pos,color,chars)) in cfgs.iter().enumerate() {
                        let mut ent = AmorphousEntity::new(&format!("E{}",ci), *pos);
                        ent.entity_mass = 3.5; ent.cohesion = 0.75; ent.pulse_rate = 0.4; ent.pulse_depth = 0.18;
                        ent.hp = 100.0; ent.max_hp = 100.0;
                        for i in 0..18 {
                            let a = (i as f32/18.0)*TAU;
                            let r = if i<6{0.4}else if i<12{0.7}else{1.0};
                            ent.formation.push(Vec3::new(a.cos()*r, a.sin()*r, 0.0));
                            ent.formation_chars.push(chars[i%chars.len()]);
                            ent.formation_colors.push(*color);
                        }
                        engine.spawn_entity(ent);
                    }
                }
                5 => { // SUPERNOVA -- collapse seed particles
                    engine.config.render.bloom_intensity = 1.5;
                    engine.config.render.chromatic_aberration = 0.005;
                    engine.add_field(ForceField::Gravity { center: Vec3::ZERO, strength: 5.0, falloff: Falloff::InverseSquare });
                    for i in 0..200 {
                        let a = hf(i+5500,0)*TAU; let r = 1.5+hf(i+5500,1)*4.0; let t = i as f32/200.0;
                        engine.spawn_glyph(Glyph {
                            character: ['*','.','+','o'][i%4], scale: md,
                            position: Vec3::new(r*a.cos(), r*a.sin(), 0.0),
                            color: Vec4::new(0.8-t*0.3, 0.3+t*0.5, 0.5+t*0.5, 0.8), emission: 1.0,
                            mass: 0.05, layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                6 => { // GALAXY
                    engine.config.render.bloom_intensity = 2.0;
                    engine.config.render.chromatic_aberration = 0.002;
                    engine.config.render.film_grain = 0.008;
                    engine.add_field(ForceField::Gravity { center: Vec3::ZERO, strength: 1.5, falloff: Falloff::InverseSquare });
                    engine.add_field(ForceField::Vortex { center: Vec3::ZERO, axis: Vec3::Z, strength: 0.12, radius: 20.0 });
                    for i in 0..2000 {
                        let arm = i % 4;
                        let t = (i as f32/2000.0).sqrt();
                        let r = t * 15.0;
                        let spiral = (arm as f32)*TAU/4.0 + t*3.5*PI + (hf(i+12000,0)-0.5)*0.5;
                        let b = 0.1 + (1.0-t)*0.7;
                        let (cr,cg,cb) = if r<2.0{(0.85,0.85,1.0)}else if r<6.0{(1.0,0.85,0.45)}else if r<10.0{(1.0,0.6,0.25)}else{(0.9,0.35,0.15)};
                        engine.spawn_glyph(Glyph {
                            character: if r<2.0{'*'}else if hf(i+12000,3)>0.8{'+'}else{'.'},
                            scale: if r<2.0{md}else{sm},
                            position: Vec3::new(r*spiral.cos(), r*spiral.sin(), (hf(i+12000,1)-0.5)*0.2),
                            color: Vec4::new(cr*b, cg*b, cb*b, 0.85), emission: b*1.5,
                            glow_color: Vec3::new(cr,cg,cb), glow_radius: if r<2.0{2.0}else{0.4},
                            mass: 0.03, layer: RenderLayer::World, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: r, speed: 0.012+(1.0-t)*0.035, eccentricity: 0.08+hf(i+12000,2)*0.15 }),
                            ..Default::default()
                        });
                    }
                    // Accretion disk
                    for i in 0..25 {
                        let a = (i as f32/25.0)*TAU;
                        engine.spawn_glyph(Glyph {
                            character: '#', scale: lg,
                            position: Vec3::new(a.cos()*0.8, a.sin()*0.8, 0.05),
                            color: Vec4::new(1.0,0.8,0.35,0.8), emission: 4.0,
                            glow_color: Vec3::new(1.0,0.6,0.2), glow_radius: 5.0,
                            layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                            life_function: Some(MathFunction::Orbit { center: Vec3::ZERO, radius: 0.8, speed: 0.2, eccentricity: 0.0 }),
                            ..Default::default()
                        });
                    }
                }
                _ => {}
            }
        }

        // Per-frame effects
        pt += dt;
        match phase {
            0 => { engine.config.render.bloom_intensity = 0.8 + (pt/5.0).min(1.0)*0.7; }
            1 => {
                // Rain: 1 per frame, small scale, start above view
                if rain_i < 200 {
                    let i = rain_i;
                    let chars = ['0','1','+','-','*','/','=','x','#','@'];
                    engine.spawn_glyph(Glyph {
                        character: chars[i%chars.len()], scale: sm,
                        position: Vec3::new(hf(i+500,0)*16.0-8.0, 7.0+hf(i+500,1)*2.0, -1.5),
                        velocity: Vec3::new((hf(i+500,2)-0.5)*0.3, -1.5-hf(i+500,3)*1.5, 0.0),
                        color: Vec4::new(0.1, 0.5+hf(i+500,4)*0.5, 0.3, 0.4),
                        emission: 0.3, lifetime: 6.0,
                        layer: RenderLayer::World, blend_mode: BlendMode::Additive,
                        ..Default::default()
                    });
                    rain_i += 1;
                }
                // Spiral: 1 per frame after 2s
                if pt > 2.0 && spiral_i < 150 {
                    let i = spiral_i;
                    let phi = 1.618033988_f32;
                    let angle = i as f32 * TAU / phi;
                    let r = (i as f32).sqrt() * 0.35;
                    let t = i as f32/150.0;
                    let chars = ['*','+','o','.','x','#','@','O'];
                    engine.spawn_glyph(Glyph {
                        character: chars[i%chars.len()], scale: md,
                        position: Vec3::new(r*angle.cos(), r*angle.sin(), 0.0),
                        color: Vec4::new(1.0-t*0.2, 0.7-t*0.3, 0.2+t*0.6, 0.85),
                        emission: 1.8-t*0.8,
                        glow_color: Vec3::new(1.0, 0.6, 0.2), glow_radius: 1.0,
                        mass: 0.05, layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                        life_function: Some(MathFunction::Breathing { rate: 0.2+t*0.4, depth: 0.08 }),
                        ..Default::default()
                    });
                    spiral_i += 1;
                }
                engine.config.render.bloom_intensity = 1.5 + (pt*0.5).sin()*0.3;
            }
            2 => { engine.add_trauma(0.003*dt); }
            4 => {
                // Shockwave at 2s
                if pt > 2.0 && pt < 2.0 + dt*2.0 {
                    engine.add_field(ForceField::Shockwave { center: Vec3::ZERO, speed: 6.0, strength: 4.0, thickness: 2.0, born_at: time });
                    engine.add_trauma(0.8);
                    engine.config.render.bloom_intensity = 3.5;
                    for i in 0..300 {
                        let a = (i as f32/300.0)*TAU*2.0; let spd = 1.0+hf(i+7000,0)*4.0; let t = i as f32/300.0;
                        let (r,g,b) = if t<0.25{(1.0,0.9,0.3)}else if t<0.5{(1.0,0.4,0.2)}else if t<0.75{(0.8,0.2,0.6)}else{(0.3,0.3,1.0)};
                        engine.spawn_glyph(Glyph {
                            character: ['*','+','.','x','o','#'][i%6], scale: sm,
                            position: Vec3::new(hf(i+7000,1)*0.4-0.2, hf(i+7000,2)*0.4-0.2, 0.0),
                            velocity: Vec3::new(a.cos()*spd, a.sin()*spd, 0.0),
                            color: Vec4::new(r,g,b,0.9), emission: 2.0,
                            glow_color: Vec3::new(r,g,b), glow_radius: 1.0,
                            mass: 0.04, lifetime: 4.0+hf(i+7000,3)*3.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                if pt > 3.0 { engine.config.render.bloom_intensity = (3.5-(pt-3.0)*0.3).max(1.5); }
            }
            5 => {
                if pt < 4.0 { engine.add_trauma(0.01*dt*(pt+1.0)); }
                // Explosion at 4s
                if pt > 4.0 && pt < 4.0+dt*2.0 {
                    engine.add_field(ForceField::Shockwave { center: Vec3::ZERO, speed: 10.0, strength: 8.0, thickness: 4.0, born_at: time });
                    engine.add_trauma(1.0);
                    engine.config.render.bloom_intensity = 5.0;
                    for i in 0..800 {
                        let a = (i as f32/800.0)*TAU*4.0; let spd = 0.5+hf(i+9000,0)*8.0; let t = i as f32/800.0;
                        let (r,g,b) = if t<0.15{(1.0,1.0,0.9)}else if t<0.3{(1.0,0.9,0.3)}else if t<0.5{(1.0,0.4,0.1)}else if t<0.7{(0.9,0.15,0.5)}else if t<0.85{(0.5,0.1,0.8)}else{(0.15,0.1,0.5)};
                        engine.spawn_glyph(Glyph {
                            character: ['#','*','@','+','x','o','X','.'][i%8], scale: sm,
                            position: Vec3::new(hf(i+9000,1)*0.2-0.1, hf(i+9000,2)*0.2-0.1, 0.0),
                            velocity: Vec3::new(a.cos()*spd, a.sin()*spd, (hf(i+9000,3)-0.5)*1.5),
                            color: Vec4::new(r,g,b,0.95), emission: 3.0-t*1.5,
                            glow_color: Vec3::new(r,g,b), glow_radius: 2.0-t,
                            mass: 0.02, lifetime: 2.0+hf(i+9000,4)*5.0,
                            layer: RenderLayer::Particle, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                // Nebula at 7s
                if pt > 7.0 && pt < 7.0+dt*2.0 {
                    engine.add_field(ForceField::StrangeAttractor { attractor_type: AttractorType::Rossler, scale: 0.15, strength: 0.2, center: Vec3::ZERO });
                    for i in 0..300 {
                        let t = i as f32/300.0;
                        let c = if t<0.25{Vec4::new(0.5,0.1,0.8,0.4)}else if t<0.5{Vec4::new(0.1,0.3,0.9,0.35)}else if t<0.75{Vec4::new(0.8,0.15,0.3,0.3)}else{Vec4::new(0.2,0.7,0.4,0.25)};
                        engine.spawn_glyph(Glyph {
                            character: '.', scale: sm,
                            position: Vec3::new(hf(i+11000,0)*5.0-2.5, hf(i+11000,1)*5.0-2.5, 0.0),
                            color: c, emission: 0.5, glow_radius: 1.5, mass: 0.015,
                            layer: RenderLayer::World, blend_mode: BlendMode::Additive,
                            ..Default::default()
                        });
                    }
                }
                if pt > 5.0 { engine.config.render.bloom_intensity = (5.0-(pt-5.0)*0.6).max(1.5); }
            }
            6 => { engine.config.render.bloom_intensity = 2.0+(pt*0.4).sin()*0.25; }
            _ => {}
        }
    });
}

fn spawn_stars(engine: &mut ProofEngine) {
    for i in 0..400 {
        engine.spawn_glyph(Glyph {
            character: if hf(i,4)>0.7{'+'} else {'.'},
            scale: Vec2::splat(0.2),
            position: Vec3::new(hf(i,0)*40.0-20.0, hf(i,1)*24.0-12.0, -4.0-hf(i,2)*4.0),
            color: Vec4::new(0.35, 0.4, 0.55, 0.2),
            emission: 0.04+hf(i,3)*0.06,
            layer: RenderLayer::Background, ..Default::default()
        });
    }
}

fn hf(seed: usize, variant: usize) -> f32 {
    let n = (seed.wrapping_mul(374761393)+variant.wrapping_mul(668265263)) as u32;
    let n = n^(n>>13); let n = n.wrapping_mul(0x5851F42D); let n = n^(n>>16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
