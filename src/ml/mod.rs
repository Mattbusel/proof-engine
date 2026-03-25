//! Machine learning primitives for Proof Engine.
//!
//! Provides tensor operations, neural network models, inference,
//! style transfer, upscaling, AI opponents, procedural generation,
//! embedding visualization, and training visualization.

pub mod tensor;
pub mod model;
pub mod inference;
pub mod style_transfer;
pub mod upscale;
pub mod ai_opponent;
pub mod procgen;
pub mod embeddings;
pub mod training_viz;

pub use tensor::Tensor;
pub use model::{Model, Layer, Sequential};
pub use inference::{InferenceEngine, Device};
pub use style_transfer::StyleTransfer;
pub use upscale::Upscaler;
pub use ai_opponent::{AIBrain, GameState, Action};
pub use procgen::{FormationGenerator, RoomLayoutGenerator, NameGenerator};
pub use embeddings::EmbeddingSpace;
pub use training_viz::{TrainingLog, TrainingDashboard};
