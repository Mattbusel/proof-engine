//! Particle system throughput benchmark.
//!
//! Measures how many particles per second the pool can tick.
//! Target: 2000+ particles at 60fps (< 0.5ms per tick for 2000 particles).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use proof_engine::particle::{MathParticle, ParticlePool, ParticleInteraction};
use proof_engine::glyph::{Glyph, RenderLayer, BlendMode};
use proof_engine::math::MathFunction;
use glam::{Vec3, Vec4};

fn make_particle(i: usize) -> MathParticle {
    MathParticle {
        glyph: Glyph {
            character: '·',
            position: Vec3::new(i as f32 * 0.1, 0.0, 0.0),
            color: Vec4::ONE,
            layer: RenderLayer::Particle,
            blend_mode: BlendMode::Additive,
            ..Default::default()
        },
        behavior: MathFunction::Lorenz { sigma: 10.0, rho: 28.0, beta: 2.67, scale: 0.01 },
        trail: false,
        trail_length: 0,
        trail_decay: 0.8,
        interaction: ParticleInteraction::None,
        origin: Vec3::new(i as f32 * 0.1, 0.0, 0.0),
        age: 0.0,
        lifetime: 5.0,
    }
}

fn bench_particle_tick(c: &mut Criterion) {
    let mut pool = ParticlePool::new(4096);
    for i in 0..2000 {
        pool.spawn(make_particle(i));
    }

    c.bench_function("particle_pool_tick_2000", |b| {
        b.iter(|| {
            pool.tick(black_box(0.016));
        })
    });
}

fn bench_particle_spawn(c: &mut Criterion) {
    c.bench_function("particle_pool_spawn_4096", |b| {
        b.iter(|| {
            let mut pool = ParticlePool::new(4096);
            for i in 0..4096 {
                pool.spawn(make_particle(black_box(i)));
            }
            pool
        })
    });
}

criterion_group!(benches, bench_particle_tick, bench_particle_spawn);
criterion_main!(benches);
