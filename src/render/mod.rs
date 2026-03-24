//! Rendering pipeline — window, OpenGL context, glyph batching, post-processing.

pub mod camera;
pub mod pipeline;
pub mod postfx;
pub mod shaders;

pub use pipeline::Pipeline;
