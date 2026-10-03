//! scripted_attractor: draw any 3D dynamical system from a Rhai script, no
//! window needed, and redraw it every time the script is saved.
//!
//! The equations live in a `.rhai` file (see `examples/scripts/` for Lorenz,
//! Thomas and Aizawa). The script sees `x`, `y`, `z` and `t` and ends with
//! `[dx, dy, dz]`. One trajectory is integrated with RK4, its points are
//! binned into a density image, the density is log-scaled and coloured with
//! a scientific colour map, and the picture is written as a PNG.
//!
//! With `--watch` it keeps running: edit the script, save, and the PNG is
//! redrawn. A script with a typo is reported and the last good picture
//! stays, so you can tune constants with an image viewer open beside your
//! editor.
//!
//! Run:
//!
//! ```text
//! cargo run --release --example scripted_attractor -- examples/scripts/thomas.rhai thomas.png --dt 0.05
//! cargo run --release --example scripted_attractor -- examples/scripts/lorenz.rhai lorenz.png --map turbo --watch
//! ```
//!
//! Options: `--steps N` (default 300000), `--dt S` (default 0.01),
//! `--map NAME` (any of `math::color::PRESET_GRADIENTS`, default inferno),
//! `--size WxH` (default 1200x900), `--watch`.

use proof_engine::export::save_rgba;
use proof_engine::math::color::{preset_gradient, Gradient, PRESET_GRADIENTS};
use proof_engine::math::scripted::{ScriptedError, ScriptedSystem};
use proof_engine::prelude::Vec3;

struct Options {
    script: String,
    out: String,
    steps: usize,
    dt: f32,
    map: String,
    size: (u32, u32),
    watch: bool,
}

fn parse() -> Result<Options, String> {
    let mut o = Options {
        script: concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts/lorenz.rhai").to_string(),
        out: "scripted_attractor.png".to_string(),
        steps: 300_000,
        dt: 0.01,
        map: "inferno".to_string(),
        size: (1200, 900),
        watch: false,
    };
    let mut args = std::env::args().skip(1);
    let mut positional = 0;
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match a.as_str() {
            "--steps" => o.steps = value("--steps")?.parse().map_err(|e| format!("--steps: {e}"))?,
            "--dt" => o.dt = value("--dt")?.parse().map_err(|e| format!("--dt: {e}"))?,
            "--map" => o.map = value("--map")?,
            "--size" => {
                let v = value("--size")?;
                let (w, h) = v.split_once('x').ok_or("--size is WIDTHxHEIGHT")?;
                o.size = (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?);
            }
            "--watch" => o.watch = true,
            _ if positional == 0 => { o.script = a; positional += 1; }
            _ if positional == 1 => { o.out = a; positional += 1; }
            _ => return Err(format!("unexpected argument {a}")),
        }
    }
    Ok(o)
}

/// Integrate one trajectory, skipping the transient.
fn trajectory(sys: &ScriptedSystem, steps: usize, dt: f32) -> Result<Vec<Vec3>, ScriptedError> {
    let mut p = Vec3::new(0.1, 0.05, 0.02);
    let mut t = 0.0;
    let warmup = (steps / 50).max(500);
    let mut points = Vec::with_capacity(steps);
    for i in 0..warmup + steps {
        p = sys.rk4_step(p, t, dt)?;
        t += dt;
        if i >= warmup {
            points.push(p);
        }
    }
    Ok(points)
}

/// Bin the points on the two axes with the widest spread, log-scale the
/// counts and colour them.
fn render(points: &[Vec3], (w, h): (u32, u32), map: &Gradient) -> Vec<u8> {
    let (lo, hi) = points.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let span = hi - lo;
    let mut axes = [(span.x, 0usize), (span.y, 1), (span.z, 2)];
    axes.sort_by(|a, b| b.0.total_cmp(&a.0));
    // The wider axis across, the other up: most attractors read best so.
    let (ax, ay) = (axes[0].1, axes[1].1);
    let margin = 0.06;
    let scale = ((w as f32 * (1.0 - 2.0 * margin)) / span[ax]).min((h as f32 * (1.0 - 2.0 * margin)) / span[ay]);
    let (cx, cy) = ((lo[ax] + hi[ax]) * 0.5, (lo[ay] + hi[ay]) * 0.5);

    let mut density = vec![0u32; (w * h) as usize];
    for p in points {
        let px = (w as f32 * 0.5 + (p[ax] - cx) * scale) as i64;
        let py = (h as f32 * 0.5 - (p[ay] - cy) * scale) as i64;
        if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
            density[(py as u32 * w + px as u32) as usize] += 1;
        }
    }
    let max = density.iter().copied().max().unwrap_or(1).max(1) as f32;
    let norm = (1.0 + max).ln();
    density
        .iter()
        .flat_map(|&n| {
            if n == 0 {
                return [0, 0, 0, 255];
            }
            // Never start at the map's darkest end, or single hits vanish.
            let v = 0.15 + 0.85 * (1.0 + n as f32).ln() / norm;
            let c = map.sample(v).to_u8();
            [c[0], c[1], c[2], 255]
        })
        .collect()
}

fn draw(sys: &ScriptedSystem, o: &Options, map: &Gradient) {
    let start = std::time::Instant::now();
    match trajectory(sys, o.steps, o.dt) {
        Ok(points) => {
            let px = render(&points, o.size, map);
            match save_rgba(&o.out, o.size.0, o.size.1, &px) {
                Ok(()) => println!("wrote {} ({} points, {:.1} s)", o.out, points.len(), start.elapsed().as_secs_f32()),
                Err(e) => eprintln!("could not write {}: {e}", o.out),
            }
        }
        Err(e) => eprintln!("the script failed while integrating: {e}"),
    }
}

fn main() {
    let o = match parse() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let Some(map) = preset_gradient(&o.map) else {
        eprintln!("unknown colour map {:?}; try one of {}", o.map, PRESET_GRADIENTS.join(", "));
        std::process::exit(2);
    };
    let mut sys = match ScriptedSystem::load(&o.script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    draw(&sys, &o, &map);
    if !o.watch {
        return;
    }
    println!("watching {} for changes; Ctrl+C to stop", o.script);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        match sys.reload_if_changed() {
            Ok(true) => draw(&sys, &o, &map),
            Ok(false) => {}
            Err(e) => eprintln!("{e} (keeping the previous version)"),
        }
    }
}
