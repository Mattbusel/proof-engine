//! Rendering pipeline — window, OpenGL context, glyph batching, post-processing.

pub mod camera;
pub mod pipeline;
pub mod postfx;
pub mod shader_graph;
pub mod shaders;
pub mod text_renderer;
pub mod lighting;
#[path = "compute/mod.rs"] pub mod compute;
pub mod render_graph;
pub mod pbr;
pub mod ui_layer;
pub mod ui_layer_renderer;
pub mod ui_primitives;

pub mod hdr;
pub mod glyph_3d_renderer;
pub mod glyph_depth_effects;

pub use pipeline::Pipeline;
