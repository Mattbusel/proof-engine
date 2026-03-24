//! # Shader Graph System
//!
//! A node-based shader graph system for the Proof Engine. Provides a visual-programming
//! style interface for constructing GPU shaders from interconnected nodes.
//!
//! ## Architecture
//!
//! - **Nodes**: 40+ node types covering sources, transforms, math, effects, color, noise, and outputs
//! - **Compiler**: Topological sort, dead-node elimination, constant folding, CSE, GLSL codegen
//! - **Optimizer**: Type inference, algebraic simplification, node merging, variant caching
//! - **Presets**: 15+ ready-made shader graphs for common game effects
//! - **Serialize**: Custom text format for saving/loading graphs with round-trip fidelity

pub mod nodes;
pub mod compiler;
pub mod optimizer;
pub mod presets;
pub mod serialize;

pub use nodes::{
    NodeType, ShaderNode, Socket, SocketDirection, DataType, SocketId,
    NodeId, Connection, ShaderGraph,
};
pub use compiler::{CompiledShader, CompileError, CompileOptions, ShaderCompiler};
pub use optimizer::{OptimizationPass, OptimizerConfig, ShaderOptimizer};
pub use presets::ShaderPresets;
pub use serialize::{GraphSerializer, SerializeError};
