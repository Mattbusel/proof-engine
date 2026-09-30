//! The attractor scenes.
//!
//! Every scene is one of the engine's own strange attractors from
//! `proof_engine::math::attractors`, integrated with the engine's RK4 step.
//! Tens of thousands of points start spread along one long trajectory that
//! has already settled onto the attractor, a hair apart, and from then on
//! each point is advanced by the equations every frame. Colour is speed
//! along the flow. Nothing is precomputed or keyframed.

use proof_engine::math::attractors::{derivatives, initial_state, rk4_step};
use proof_engine::prelude::*;
use proof_engine::render::ui_layer::UiParticle;

/// One attractor as the saver shows it.
pub struct SceneDef {
    pub kind: AttractorType,
    /// Lower-case key used in the settings file and on the command line.
    pub key: &'static str,
    /// Name as drawn on screen (the engine font has no umlaut).
    pub name: &'static str,
    /// The equations, exactly as `derivatives` computes them.
    pub equations: [&'static str; 3],
    /// Axis drawn vertical before the view is tilted.
    pub up: Vec3,
    /// How far the view looks down onto the attractor, radians.
    pub tilt: f32,
    /// Simulated seconds per real second at normal speed.
    pub rate: f32,
    /// Colour at rest, in between, and at the fastest speeds.
    pub palette: [Vec3; 3],
}

const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The Thomas and Halvorsen systems are symmetric under cycling x, y, z, so
/// their natural axis is the diagonal.
const DIAG: Vec3 = v(0.577_350_3, 0.577_350_3, 0.577_350_3);

pub const SCENES: &[SceneDef] = &[
    SceneDef {
        kind: AttractorType::Lorenz,
        key: "lorenz",
        name: "Lorenz",
        equations: ["dx/dt = 10(y - x)", "dy/dt = x(28 - z) - y", "dz/dt = xy - 8z/3"],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.05,
        rate: 0.22,
        palette: [v(0.10, 0.55, 0.70), v(0.85, 0.80, 0.70), v(1.40, 0.55, 0.16)],
    },
    SceneDef {
        kind: AttractorType::Aizawa,
        key: "aizawa",
        name: "Aizawa",
        equations: [
            "dx/dt = (z - 0.7)x - 3.5y",
            "dy/dt = 3.5x + (z - 0.7)y",
            "dz/dt = 0.6 + 0.95z - z^3/3 - (x^2 + y^2)(1 + 0.25z) + 0.1zx^3",
        ],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.35,
        rate: 0.45,
        palette: [v(0.95, 0.45, 0.10), v(1.00, 0.80, 0.45), v(1.30, 1.20, 1.00)],
    },
    SceneDef {
        kind: AttractorType::Thomas,
        key: "thomas",
        name: "Thomas",
        equations: ["dx/dt = sin y - 0.208186x", "dy/dt = sin z - 0.208186y", "dz/dt = sin x - 0.208186z"],
        up: DIAG,
        tilt: 0.55,
        rate: 2.6,
        palette: [v(0.20, 0.45, 1.00), v(0.55, 0.85, 1.10), v(1.20, 1.20, 1.25)],
    },
    SceneDef {
        kind: AttractorType::Halvorsen,
        key: "halvorsen",
        name: "Halvorsen",
        equations: [
            "dx/dt = -1.4x - 4y - 4z - y^2",
            "dy/dt = -1.4y - 4z - 4x - z^2",
            "dz/dt = -1.4z - 4x - 4y - x^2",
        ],
        up: DIAG,
        tilt: 1.15,
        rate: 0.40,
        palette: [v(0.15, 0.65, 0.35), v(0.60, 0.95, 0.55), v(1.25, 1.20, 0.80)],
    },
    SceneDef {
        kind: AttractorType::Rossler,
        key: "rossler",
        name: "Rossler",
        equations: ["dx/dt = -y - z", "dy/dt = x + 0.2y", "dz/dt = 0.2 + z(x - 5.7)"],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.60,
        rate: 1.10,
        palette: [v(0.50, 0.25, 0.95), v(0.95, 0.45, 0.85), v(1.30, 1.10, 1.25)],
    },
    SceneDef {
        kind: AttractorType::Dadras,
        key: "dadras",
        name: "Dadras",
        equations: ["dx/dt = y - 3x + 2.7yz", "dy/dt = 1.7y - xz + z", "dz/dt = 2xy - 9z"],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.30,
        rate: 0.30,
        palette: [v(0.85, 0.15, 0.35), v(1.05, 0.55, 0.55), v(1.30, 1.10, 0.95)],
    },
    SceneDef {
        kind: AttractorType::Chen,
        key: "chen",
        name: "Chen",
        equations: ["dx/dt = 35(y - x)", "dy/dt = -7x - xz + 28y", "dz/dt = xy - 3z"],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.05,
        rate: 0.14,
        palette: [v(0.95, 0.40, 0.08), v(1.00, 0.75, 0.40), v(1.25, 1.15, 0.95)],
    },
    SceneDef {
        kind: AttractorType::Rabinovich,
        key: "rabinovich",
        name: "Rabinovich-Fabrikant",
        equations: [
            "dx/dt = y(z - 1 + x^2) + 0.87x",
            "dy/dt = x(3z + 1 - x^2) + 0.87y",
            "dz/dt = -2z(1.1 + xy)",
        ],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.35,
        rate: 0.35,
        palette: [v(0.10, 0.70, 0.75), v(0.55, 0.95, 0.85), v(1.15, 1.20, 1.10)],
    },
    SceneDef {
        kind: AttractorType::Sprott,
        key: "sprott",
        name: "Sprott B",
        equations: ["dx/dt = yz", "dy/dt = x - y", "dz/dt = 1 - xy"],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.40,
        rate: 0.80,
        palette: [v(0.95, 0.70, 0.15), v(1.05, 0.90, 0.55), v(1.25, 1.20, 1.05)],
    },
    SceneDef {
        kind: AttractorType::Burke,
        key: "burke",
        name: "Burke-Shaw",
        equations: ["dx/dt = -10(x + y)", "dy/dt = -y - 10xz", "dz/dt = 10xy + 4.272"],
        up: v(0.0, 0.0, 1.0),
        tilt: 0.25,
        rate: 0.15,
        palette: [v(0.30, 0.40, 1.00), v(0.75, 0.60, 1.05), v(1.20, 1.10, 1.25)],
    },
];

/// Look a scene up by its key, case-insensitively.
pub fn find(key: &str) -> Option<usize> {
    let k = key.trim().to_ascii_lowercase();
    SCENES.iter().position(|s| s.key == k)
}

/// A running attractor: its points, and what was measured about it.
pub struct Attractor {
    pub def: &'static SceneDef,
    points: Vec<Vec3>,
    /// Points on the settled trajectory, used to re-seed any point that
    /// escapes (the Rabinovich-Fabrikant system can throw one out).
    samples: Vec<Vec3>,
    center: Vec3,
    radius: f32,
    /// Speed that maps to the hot end of the palette.
    fast: f32,
    e1: Vec3,
    e2: Vec3,
    yaw: f32,
    h: f32,
    reseed: usize,
}

/// Radians per second the view turns.
const SPIN: f32 = 0.06;

impl Attractor {
    pub fn new(def: &'static SceneDef, points: usize, start_yaw: f32) -> Self {
        let kind = def.kind;
        let h = kind.recommended_dt();
        let mut s = initial_state(kind);
        for _ in 0..6000 {
            s = rk4_step(kind, s, h);
        }
        // One long settled trajectory. The points are spread along it so the
        // whole shape is there on the first frame.
        let per = 3usize;
        let n = points.max(1);
        let mut samples = Vec::with_capacity(n * per);
        for _ in 0..n * per {
            s = rk4_step(kind, s, h);
            if !s.is_finite() {
                s = initial_state(kind);
            }
            samples.push(s);
        }
        let center = samples.iter().fold(Vec3::ZERO, |a, p| a + *p) / samples.len() as f32;
        let mut dist: Vec<f32> = samples.iter().map(|p| (*p - center).length()).collect();
        dist.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let radius = dist[dist.len() * 995 / 1000].max(1e-3);
        let mut speeds: Vec<f32> = samples.iter().map(|p| derivatives(kind, *p).length()).collect();
        speeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let fast = speeds[speeds.len() * 95 / 100].max(1e-3);

        let pts = (0..n)
            .map(|i| samples[i * per] + Vec3::splat(hash(i as f32) * radius * 1e-4))
            .collect();
        // Keep a thinned copy of the trajectory for re-seeding.
        let keep: Vec<Vec3> = samples.iter().step_by(per * 4).copied().collect();

        let up = def.up.normalize();
        let seed = if up.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
        let e1 = seed.cross(up).normalize();
        let e2 = up.cross(e1).normalize();

        Self { def, points: pts, samples: keep, center, radius, fast, e1, e2, yaw: start_yaw, h, reseed: 0 }
    }

    /// Advance every point by `dt` real seconds times `speed`.
    pub fn step(&mut self, dt: f32, speed: f32) {
        let sim = (dt * self.def.rate * speed).max(0.0);
        if sim <= 0.0 {
            return;
        }
        let sub = ((sim / self.h).ceil() as usize).clamp(1, 12);
        let h = sim / sub as f32;
        let kind = self.def.kind;
        let limit = self.radius * 6.0;
        for p in self.points.iter_mut() {
            for _ in 0..sub {
                *p = rk4_step(kind, *p, h);
            }
            if !p.is_finite() || (*p - self.center).length() > limit {
                self.reseed = (self.reseed + 7919) % self.samples.len();
                *p = self.samples[self.reseed];
            }
        }
        self.yaw += dt * SPIN;
    }

    /// Project into the rectangle `(x, y, w, h)` in framebuffer pixels.
    /// `alpha` scales the brightness, for fading between scenes.
    pub fn draw(&self, out: &mut Vec<UiParticle>, rect: [f32; 4], alpha: f32) {
        let [rx, ry, rw, rh] = rect;
        if alpha <= 0.001 {
            return;
        }
        let (cx, cy) = (rx + rw * 0.5, ry + rh * 0.5);
        let scale = rw.min(rh) * 0.46 / self.radius;
        let (sy, cyaw) = self.yaw.sin_cos();
        let (st, ct) = self.def.tilt.sin_cos();
        let up = self.def.up.normalize();
        let size = (rh / 380.0).max(1.2);
        let [cool, mid, hot] = self.def.palette;
        let kind = self.def.kind;
        out.reserve(self.points.len());
        for p in &self.points {
            let d = *p - self.center;
            let (a, b, c) = (d.dot(self.e1), d.dot(self.e2), d.dot(up));
            let x = a * cyaw - b * sy;
            let depth = a * sy + b * cyaw;
            let vy = c * ct + depth * st;
            let speed = (derivatives(kind, *p).length() / self.fast).clamp(0.0, 1.0);
            let col = if speed < 0.5 { cool.lerp(mid, speed * 2.0) } else { mid.lerp(hot, (speed - 0.5) * 2.0) };
            // Nearer points a little brighter, so the shape reads in depth.
            let near = 0.75 + 0.25 * (depth / self.radius).clamp(-1.0, 1.0);
            let mut q = UiParticle::new(cx + x * scale, cy - vy * scale, size, size, '●', (col * near * alpha).extend(0.85));
            q.emission = (0.55 + 0.9 * speed) * alpha;
            out.push(q);
        }
    }
}

/// Cheap deterministic hash in [-0.5, 0.5).
fn hash(x: f32) -> f32 {
    ((x * 12.9898).sin() * 43_758.547).fract() - 0.5
}

/// How many points to give a region of this many pixels.
pub fn points_for_area(w: f32, h: f32, preview: bool) -> usize {
    if preview {
        return 2500;
    }
    let full_hd = 1920.0 * 1080.0;
    ((w * h / full_hd) * 30_000.0).clamp(8_000.0, 48_000.0) as usize
}

/// Seconds to fade in and out at each scene change.
pub const FADE: f32 = 2.5;

/// One region of the screen (one monitor) cycling through scenes.
pub struct Slot {
    pub rect: [f32; 4],
    order: Vec<usize>,
    pos: usize,
    pub current: Attractor,
    t: f32,
    points: usize,
    pub caption: bool,
    pub blank: bool,
}

impl Slot {
    /// `order` is the list of scene indices to cycle through, starting at `pos`.
    pub fn new(rect: [f32; 4], order: Vec<usize>, pos: usize, points: usize, caption: bool) -> Self {
        let pos = pos % order.len();
        let current = Attractor::new(&SCENES[order[pos]], points, -1.05);
        Self { rect, order, pos, current, t: 0.0, points, caption, blank: false }
    }

    /// Advance the slot. Cycles to the next scene when `scene_len` runs out
    /// and there is more than one scene in the order.
    pub fn tick(&mut self, dt: f32, speed: f32, scene_len: f32) {
        if self.blank {
            return;
        }
        self.t += dt;
        if self.order.len() > 1 && self.t >= scene_len {
            self.pos = (self.pos + 1) % self.order.len();
            self.current = Attractor::new(&SCENES[self.order[self.pos]], self.points, -1.05);
            self.t = 0.0;
        }
        self.current.step(dt, speed);
    }

    /// Jump to the fade-out of the current scene, so the next one follows.
    pub fn skip(&mut self, scene_len: f32) {
        if self.order.len() > 1 {
            self.t = self.t.max(scene_len - FADE * 0.5);
        }
    }

    /// Brightness envelope for the current scene.
    pub fn fade(&self, scene_len: f32) -> f32 {
        let fade_in = (self.t / FADE).min(1.0);
        let fade_out = if self.order.len() > 1 { ((scene_len - self.t) / FADE).clamp(0.0, 1.0) } else { 1.0 };
        let f = fade_in.min(fade_out);
        f * f * (3.0 - 2.0 * f)
    }

    pub fn draw(&self, ui: &mut proof_engine::render::ui_layer::UiLayer, scene_len: f32, text_scale: f32) {
        if self.blank {
            return;
        }
        let alpha = self.fade(scene_len);
        let mut out = Vec::new();
        self.current.draw(&mut out, self.rect, alpha);
        ui.draw_particles(out);

        if self.caption {
            // The name and equations show for a few seconds after each fade in.
            let t = self.t - 1.5;
            let show = (t / 1.5).clamp(0.0, 1.0) * ((10.0 - t) / 1.5).clamp(0.0, 1.0);
            if show > 0.0 {
                let [rx, ry, _rw, rh] = self.rect;
                let line = 24.0 * text_scale;
                let x = rx + 36.0 * text_scale;
                let mut y = ry + rh - 36.0 * text_scale - line * 4.0;
                let c = Vec4::new(0.86, 0.88, 0.92, 0.85 * show * alpha);
                ui.draw_text(x, y, &format!("{} attractor", self.current.def.name), text_scale, c);
                let dim = Vec4::new(0.86, 0.88, 0.92, 0.6 * show * alpha);
                for eq in self.current.def.equations {
                    y += line;
                    ui.draw_text(x, y, eq, text_scale * 0.85, dim);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_findable() {
        for (i, s) in SCENES.iter().enumerate() {
            assert_eq!(find(s.key), Some(i));
            assert_eq!(find(&s.key.to_uppercase()), Some(i));
        }
        assert_eq!(find("cycle"), None);
    }

    #[test]
    fn every_scene_stays_bounded_and_finite() {
        for def in SCENES {
            let mut a = Attractor::new(def, 500, 0.0);
            for _ in 0..300 {
                a.step(1.0 / 30.0, 1.0);
            }
            for p in &a.points {
                assert!(p.is_finite(), "{} produced a non-finite point", def.name);
                assert!((*p - a.center).length() <= a.radius * 6.0 + 1e-3, "{} escaped", def.name);
            }
        }
    }

    #[test]
    fn projected_points_land_inside_their_rectangle() {
        for def in SCENES {
            let a = Attractor::new(def, 2000, 0.3);
            let mut out = Vec::new();
            a.draw(&mut out, [100.0, 50.0, 800.0, 600.0], 1.0);
            let inside = out
                .iter()
                .filter(|q| q.x >= 100.0 && q.x <= 900.0 && q.y >= 50.0 && q.y <= 650.0)
                .count();
            assert!(inside as f32 >= out.len() as f32 * 0.98, "{}: {inside}/{} inside", def.name, out.len());
        }
    }
}
