//! Rendering pipeline — window, OpenGL context, glyph batching, post-processing.

pub mod camera;
pub mod pipeline;
pub mod postfx;
pub mod shader_graph;
pub mod shaders;
pub mod text_renderer;
pub mod lighting;

pub use pipeline::Pipeline;
