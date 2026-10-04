//! The CPU work of the `lorenz` demo: 40,000 points stepped with RK4 each
//! frame, plus one RK4 step for each built-in attractor. No window.
//!
//! The same file builds against 0.2.3, which is how the release-to-release
//! numbers in the changelog were taken.

use criterion::{criterion_group, criterion_main, Criterion};
use proof_engine::math::attractors::{rk4_step, AttractorType};
use proof_engine::prelude::Vec3;
use std::hint::black_box;

fn bench(c: &mut Criterion) {
    let mut points: Vec<Vec3> = (0..40_000)
        .map(|i| Vec3::new(1.0 + i as f32 * 4e-4, 1.0, 1.0))
        .collect();
    c.bench_function("lorenz_40k_points_one_frame", |b| {
        b.iter(|| {
            for p in points.iter_mut() {
                *p = rk4_step(AttractorType::Lorenz, *p, 0.005);
            }
            black_box(points[0])
        })
    });

    // The same work through rk4_step_all (multi-core with --features parallel).
    let mut points2: Vec<Vec3> = (0..40_000)
        .map(|i| Vec3::new(1.0 + i as f32 * 4e-4, 1.0, 1.0))
        .collect();
    c.bench_function("lorenz_40k_points_rk4_step_all", |b| {
        b.iter(|| {
            proof_engine::math::attractors::rk4_step_all(
                AttractorType::Lorenz,
                &mut points2,
                0.005,
            );
            black_box(points2[0])
        })
    });

    let kinds = [
        AttractorType::Lorenz,
        AttractorType::Rossler,
        AttractorType::Chen,
        AttractorType::Halvorsen,
        AttractorType::Aizawa,
        AttractorType::Thomas,
        AttractorType::Dadras,
    ];
    let mut g = c.benchmark_group("rk4_step");
    for kind in kinds {
        let mut s = Vec3::new(0.1, 0.0, 0.0);
        g.bench_function(format!("{kind:?}"), |b| {
            b.iter(|| {
                s = rk4_step(black_box(kind), s, 0.001);
                black_box(s)
            })
        });
    }
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
