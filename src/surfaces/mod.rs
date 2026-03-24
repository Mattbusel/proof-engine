//! # Surface Rendering Subsystem
//!
//! Mathematical surface rendering for the Proof Engine.
//!
//! ## Modules
//!
//! - [`parametric`] — Parametric surface definitions and mesh tessellation
//! - [`heightfield`] — Height-map surfaces with noise, LOD, chunking, and collision
//! - [`deformation`] — Time-varying surface deformation (breathe, wave, twist, melt, etc.)
//! - [`uvanimation`] — UV coordinate animation, flow maps, triplanar projection

pub mod parametric;
pub mod heightfield;
pub mod deformation;
pub mod uvanimation;

// Re-export primary types for convenience.
pub use parametric::{
    Surface, SurfaceMesh, Sphere, Torus, MobiusStrip, KleinBottle,
    BoySurface, RomanSurface, CrossCap, TrefoilKnot, FigureEight,
    Catenoid, Helicoid, EnneperSurface, DiniSurface, FunctionSurface,
};
pub use heightfield::{
    HeightFieldSurface, NoiseSource, HeightFieldChunk, ChunkManager,
    HeightFieldCollider, LodLevel,
};
pub use deformation::{
    DeformationMode, DeformationStack, Deformation, MorphTarget, WaveSimulation,
    KeyframeAnimator,
};
pub use uvanimation::{
    UVAnimator, UVMode, FlowMap, ParallaxLayer, SpriteSheetAnimator,
    TriplanarProjector, UVUnwrap,
};
