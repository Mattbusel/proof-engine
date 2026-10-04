//! Cross-check: `math::attractors::rk4_step` against the `ode_solvers`
//! crate's RK4, on equations written out again here in f64 from their
//! published definitions (so a wrong constant in the engine would show up
//! too, not only a wrong integrator).

use ode_solvers::{Rk4, System, Vector3};
use proof_engine::math::attractors::{rk4_step, AttractorType};
use proof_engine::prelude::Vec3;

type State = Vector3<f64>;

#[derive(Clone, Copy)]
struct Reference(AttractorType);

impl System<f64, State> for Reference {
    fn system(&self, _t: f64, s: &State, d: &mut State) {
        let (x, y, z) = (s[0], s[1], s[2]);
        let (dx, dy, dz) = match self.0 {
            // Lorenz 1963: sigma 10, rho 28, beta 8/3.
            AttractorType::Lorenz => (10.0 * (y - x), x * (28.0 - z) - y, x * y - 8.0 / 3.0 * z),
            // Rossler 1976: a 0.2, b 0.2, c 5.7.
            AttractorType::Rossler => (-y - z, x + 0.2 * y, 0.2 + z * (x - 5.7)),
            // Thomas 1999: b 0.208186.
            AttractorType::Thomas => {
                let b = 0.208186;
                (y.sin() - b * x, z.sin() - b * y, x.sin() - b * z)
            }
            // Aizawa: a .95, b .7, c .6, d 3.5, e .25, f .1.
            AttractorType::Aizawa => (
                (z - 0.7) * x - 3.5 * y,
                3.5 * x + (z - 0.7) * y,
                0.6 + 0.95 * z - z.powi(3) / 3.0 - (x * x + y * y) * (1.0 + 0.25 * z)
                    + 0.1 * z * x.powi(3),
            ),
            // Chen: a 35, b 3, c 28.
            AttractorType::Chen => (35.0 * (y - x), (28.0 - 35.0) * x - x * z + 28.0 * y, x * y - 3.0 * z),
            other => unimplemented!("no reference for {other:?}"),
        };
        d[0] = dx;
        d[1] = dy;
        d[2] = dz;
    }
}

fn compare(kind: AttractorType, start: [f64; 3], dt: f64, steps: usize, tol: f64) {
    let mut ours = Vec3::new(start[0] as f32, start[1] as f32, start[2] as f32);
    for _ in 0..steps {
        ours = rk4_step(kind, ours, dt as f32);
    }
    let t_end = dt * steps as f64;
    let mut solver = Rk4::new(Reference(kind), 0.0, State::new(start[0], start[1], start[2]), t_end, dt);
    solver.integrate().expect("reference integration");
    let r = solver.y_out().last().copied().expect("output");
    let err = ((ours.x as f64 - r[0]).powi(2) + (ours.y as f64 - r[1]).powi(2) + (ours.z as f64 - r[2]).powi(2)).sqrt();
    let scale = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt().max(1.0);
    assert!(
        err / scale < tol,
        "{kind:?}: engine {ours:?} vs ode_solvers ({:.6}, {:.6}, {:.6}), relative error {:.2e}",
        r[0], r[1], r[2], err / scale
    );
}

#[test]
fn lorenz_matches_ode_solvers() {
    compare(AttractorType::Lorenz, [1.0, 1.0, 1.0], 0.005, 200, 1e-4);
}

#[test]
fn rossler_matches_ode_solvers() {
    compare(AttractorType::Rossler, [0.1, 0.0, 0.0], 0.01, 500, 1e-4);
}

#[test]
fn thomas_matches_ode_solvers() {
    compare(AttractorType::Thomas, [0.1, 0.0, 0.0], 0.05, 200, 1e-4);
}

#[test]
fn aizawa_matches_ode_solvers() {
    compare(AttractorType::Aizawa, [0.1, 0.0, 0.0], 0.01, 300, 1e-4);
}

#[test]
fn chen_matches_ode_solvers() {
    compare(AttractorType::Chen, [-10.0, 0.0, 37.0], 0.002, 200, 1e-4);
}
