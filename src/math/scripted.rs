//! Dynamical systems written in a script, reloaded while the program runs.
//!
//! The built-in [`AttractorType`](super::AttractorType)s are compiled in.
//! A [`ScriptedSystem`] is the same idea with the equations in a
//! [Rhai](https://rhai.rs) script instead, so you can try a new system, or
//! retune sigma and rho, without recompiling, and see it change as soon as
//! the file is saved.
//!
//! A script sees the state as `x`, `y`, `z` and the time as `t`, all
//! floats, and ends with the derivative as a three element array:
//!
//! ```text
//! // Lorenz
//! let sigma = 10.0;
//! let rho = 28.0;
//! let beta = 8.0 / 3.0;
//! [sigma * (y - x), x * (rho - z) - y, x * y - beta * z]
//! ```
//!
//! Rhai's standard maths is available (`sin`, `cos`, `exp`, `sqrt`, `abs`,
//! `PI()` and so on). Integer division is integer division: write `8.0 / 3.0`,
//! not `8 / 3`. Every evaluation is capped at a fixed number of operations,
//! so a script with an endless loop returns an error instead of freezing
//! the frame.
//!
//! ```rust
//! use proof_engine::math::scripted::ScriptedSystem;
//! use proof_engine::prelude::Vec3;
//!
//! let thomas = ScriptedSystem::compile("
//!     let b = 0.208186;
//!     [sin(y) - b * x, sin(z) - b * y, sin(x) - b * z]
//! ").unwrap();
//! let mut p = Vec3::new(0.1, 0.0, 0.0);
//! for i in 0..1000 {
//!     p = thomas.rk4_step(p, i as f32 * 0.05, 0.05).unwrap();
//! }
//! assert!(p.length() < 6.0, "Thomas' attractor is bounded");
//! ```

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use glam::Vec3;
use rhai::{Dynamic, Engine, Scope, AST};

/// Operations one evaluation may take before it is stopped.
const MAX_OPERATIONS: u64 = 50_000;

/// A script that failed to load, compile or run.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptedError {
    /// What went wrong, with the line and column when Rhai knows them.
    pub message: String,
}

impl fmt::Display for ScriptedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ScriptedError {}

fn err(message: impl Into<String>) -> ScriptedError {
    ScriptedError { message: message.into() }
}

/// A three-dimensional ODE `dp/dt = f(p, t)` whose `f` is a Rhai script.
pub struct ScriptedSystem {
    engine: Engine,
    ast: AST,
    source: String,
    file: Option<(PathBuf, Option<SystemTime>)>,
}

impl fmt::Debug for ScriptedSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptedSystem")
            .field("file", &self.file.as_ref().map(|(p, _)| p))
            .field("source", &self.source)
            .finish()
    }
}

fn make_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(4096);
    engine.set_max_array_size(1024);
    // print and debug go to the log instead of stdout.
    engine.on_print(|s| log::info!("script: {s}"));
    engine.on_debug(|s, _, pos| log::debug!("script {pos}: {s}"));
    engine
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn number(v: &Dynamic, i: usize) -> Result<f32, ScriptedError> {
    if let Ok(f) = v.as_float() {
        Ok(f as f32)
    } else if let Ok(n) = v.as_int() {
        Ok(n as f32)
    } else {
        Err(err(format!("element {i} of the result is a {}, not a number", v.type_name())))
    }
}

impl ScriptedSystem {
    /// Compile a script from source text.
    pub fn compile(source: &str) -> Result<Self, ScriptedError> {
        let engine = make_engine();
        let ast = engine.compile(source).map_err(|e| err(format!("compile error: {e}")))?;
        let sys = Self { engine, ast, source: source.to_string(), file: None };
        // Run it once at the origin so a script that compiles but cannot
        // produce a vector is caught here, not on the first frame.
        sys.derivative(Vec3::ZERO, 0.0)?;
        Ok(sys)
    }

    /// Load and compile a script file, remembering the path for
    /// [`reload_if_changed`](Self::reload_if_changed).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ScriptedError> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path)
            .map_err(|e| err(format!("could not read {}: {e}", path.display())))?;
        let mut sys = Self::compile(&source).map_err(|e| err(format!("{}: {e}", path.display())))?;
        sys.file = Some((path.to_path_buf(), mtime(path)));
        Ok(sys)
    }

    /// The source the system is running.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The file it was loaded from, if any.
    pub fn path(&self) -> Option<&Path> {
        self.file.as_ref().map(|(p, _)| p.as_path())
    }

    /// Recompile from the file if it changed on disk since the last load.
    ///
    /// Returns `Ok(true)` when the new script is now running and `Ok(false)`
    /// when nothing changed. If the new version does not compile, or does
    /// not return a vector, the error comes back and the previous script
    /// keeps running, so a half-typed edit never stops the animation.
    pub fn reload_if_changed(&mut self) -> Result<bool, ScriptedError> {
        let Some((path, seen)) = self.file.clone() else { return Ok(false) };
        let now = mtime(&path);
        if now.is_none() || now == seen {
            return Ok(false);
        }
        // Remember this version even if it fails, so a broken save is
        // reported once rather than every frame.
        if let Some((_, t)) = self.file.as_mut() {
            *t = now;
        }
        let fresh = Self::load(&path)?;
        self.ast = fresh.ast;
        self.source = fresh.source;
        Ok(true)
    }

    /// `dp/dt` at state `p` and time `t`.
    pub fn derivative(&self, p: Vec3, t: f32) -> Result<Vec3, ScriptedError> {
        let mut scope = Scope::new();
        scope.push("x", p.x as f64);
        scope.push("y", p.y as f64);
        scope.push("z", p.z as f64);
        scope.push("t", t as f64);
        let out: Dynamic = self
            .engine
            .eval_ast_with_scope(&mut scope, &self.ast)
            .map_err(|e| err(format!("runtime error: {e}")))?;
        let arr = out
            .into_typed_array::<Dynamic>()
            .map_err(|ty| err(format!("the script must end with [dx, dy, dz], got a {ty}")))?;
        if arr.len() != 3 {
            return Err(err(format!("the script must end with [dx, dy, dz], got {} elements", arr.len())));
        }
        let v = Vec3::new(number(&arr[0], 0)?, number(&arr[1], 1)?, number(&arr[2], 2)?);
        if !v.is_finite() {
            return Err(err(format!("the derivative at {p} is not finite: {v}")));
        }
        Ok(v)
    }

    /// One classic fourth-order Runge-Kutta step of size `dt` from time `t`,
    /// the same integrator as [`attractors::rk4_step`](super::attractors::rk4_step).
    pub fn rk4_step(&self, p: Vec3, t: f32, dt: f32) -> Result<Vec3, ScriptedError> {
        let h = dt * 0.5;
        let k1 = self.derivative(p, t)?;
        let k2 = self.derivative(p + k1 * h, t + h)?;
        let k3 = self.derivative(p + k2 * h, t + h)?;
        let k4 = self.derivative(p + k3 * dt, t + dt)?;
        Ok(p + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (dt / 6.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::attractors::{rk4_step, AttractorType};

    const LORENZ: &str = "
        let sigma = 10.0;
        let rho = 28.0;
        let beta = 8.0 / 3.0;
        [sigma * (y - x), x * (rho - z) - y, x * y - beta * z]
    ";

    #[test]
    fn a_scripted_lorenz_tracks_the_built_in_one() {
        let sys = ScriptedSystem::compile(LORENZ).unwrap();
        let (mut a, mut b) = (Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 1.0));
        for i in 0..200 {
            a = sys.rk4_step(a, i as f32 * 0.005, 0.005).unwrap();
            b = rk4_step(AttractorType::Lorenz, b, 0.005);
        }
        // Same equations, same integrator: the trajectories agree until
        // f32 rounding (scripts compute in f64) is amplified by chaos.
        assert!((a - b).length() < 1e-2, "{a} vs {b}");
    }

    #[test]
    fn ints_floats_maths_and_time_all_work() {
        let sys = ScriptedSystem::compile("[1, 2.5 * x, sin(t) + PI() * 0.0]").unwrap();
        let d = sys.derivative(Vec3::new(2.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2).unwrap();
        assert_eq!(d, Vec3::new(1.0, 5.0, 1.0));
    }

    #[test]
    fn bad_scripts_are_errors_not_panics() {
        for (src, needle) in [
            ("[x, y", "compile error"),
            ("x + y", "must end with"),
            ("[x, y]", "got 2 elements"),
            ("[x, \"y\", z]", "not a number"),
            ("[undefined_var, y, z]", "runtime error"),
            ("loop { }", "runtime error"),
            ("[1.0 / 0.0, 0.0, 0.0]", "not finite"),
        ] {
            let e = ScriptedSystem::compile(src).unwrap_err();
            assert!(e.message.contains(needle), "{src:?}: {}", e.message);
        }
    }

    #[test]
    fn hot_reload_swaps_in_new_equations_and_survives_broken_edits() {
        let dir = std::env::temp_dir().join("proof_engine_scripted_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("field.rhai");
        std::fs::write(&path, "[1.0, 0.0, 0.0]").unwrap();
        let mut sys = ScriptedSystem::load(&path).unwrap();
        assert_eq!(sys.path(), Some(path.as_path()));
        assert_eq!(sys.reload_if_changed(), Ok(false));

        // File systems with coarse timestamps need the clock to move on.
        let bump = |p: &Path| {
            let f = std::fs::File::options().write(true).open(p).unwrap();
            let later = mtime(p).unwrap() + std::time::Duration::from_secs(2);
            f.set_modified(later).unwrap();
        };

        std::fs::write(&path, "[0.0, 2.0, 0.0]").unwrap();
        bump(&path);
        assert_eq!(sys.reload_if_changed(), Ok(true));
        assert_eq!(sys.derivative(Vec3::ZERO, 0.0).unwrap(), Vec3::new(0.0, 2.0, 0.0));

        // A half-typed edit is reported once and the old script keeps going.
        std::fs::write(&path, "[0.0, 3.0,").unwrap();
        bump(&path);
        assert!(sys.reload_if_changed().is_err());
        assert_eq!(sys.reload_if_changed(), Ok(false), "the broken version is not retried every frame");
        assert_eq!(sys.derivative(Vec3::ZERO, 0.0).unwrap(), Vec3::new(0.0, 2.0, 0.0));
    }
}
