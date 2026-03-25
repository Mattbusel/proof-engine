//! Fractal Rendering System — Mandelbrot, Julia, 3D fractals, IFS, L-systems,
//! flame fractals, Newton fractals, deep zoom, progressive rendering, fractal terrain.
//!
//! A boss arena that IS a Mandelbrot set. You fight inside the fractal.

pub mod mandelbrot;
pub mod julia;
pub mod fractal3d;
pub mod ifs;
pub mod lsystem;
pub mod flame;
pub mod newton;
pub mod terrain;
pub mod progressive;

pub use mandelbrot::{MandelbrotRenderer, MandelbrotParams};
pub use julia::{JuliaRenderer, JuliaParams};
pub use fractal3d::{Mandelbulb, Mandelbox, RayMarchParams};
pub use ifs::{IfsSystem, IfsFractal, AffineTx};
pub use lsystem::{LSystem, LSystemRule, TurtleState};
pub use flame::{FlameParams, FlameVariation};
pub use newton::{NewtonFractal, Polynomial};
pub use terrain::{FractalTerrain, TerrainParams};
pub use progressive::ProgressiveRenderer;
