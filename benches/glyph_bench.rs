//! Glyph pool throughput benchmark.
//!
//! Measures spawn, tick, and force field application throughput.
//! Target: 5000+ glyphs at 60fps (< 2ms for 5000 glyph tick).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use proof_engine::glyph::{Glyph, GlyphPool, RenderLayer};
use glam::{Vec3, Vec4};

fn make_glyph(i: usize) -> Glyph {
    Glyph {
        character: '@',
        position: Vec3::new((i % 80) as f32, (i / 80) as f32, 0.0),
        color: Vec4::new(0.5, 0.8, 1.0, 1.0),
        emission: 0.1,
        mass: 1.0,
        layer: RenderLayer::Background,
        ..Default::default()
    }
}

fn bench_glyph_tick(c: &mut Criterion) {
    let mut pool = GlyphPool::new(8192);
    for i in 0..5000 {
        pool.spawn(make_glyph(i));
    }

    c.bench_function("glyph_pool_tick_5000", |b| {
        b.iter(|| {
            pool.tick(black_box(0.016));
        })
    });
}

fn bench_glyph_spawn(c: &mut Criterion) {
    c.bench_function("glyph_pool_spawn_8192", |b| {
        b.iter(|| {
            let mut pool = GlyphPool::new(8192);
            for i in 0..8192 {
                pool.spawn(make_glyph(black_box(i)));
            }
            pool
        })
    });
}

criterion_group!(benches, bench_glyph_tick, bench_glyph_spawn);
criterion_main!(benches);
