//! Metaball / Isosurface Entity System
//!
//! Each entity is a 3D scalar field defined by a set of field sources (metaballs).
//! The visible surface is extracted via marching cubes at a configurable threshold.
//! HP affects field strength — damage literally reshapes the entity's body.
//!
//! # Architecture
//!
//! ```text
//! MetaballEntity
//!   ├─ sources: Vec<FieldSource>   (position, strength, radius, falloff, color)
//!   ├─ threshold: f32              (isosurface level)
//!   ├─ hp_ratio: f32               (0.0 = dead, 1.0 = full HP)
//!   └─ grid_resolution: u32        (32 normal, 64 boss)
//!
//! Per frame:
//!   1. Update sources (HP modulation, damage zones, breathing)
//!   2. Evaluate field on 3D grid (CPU or GPU compute)
//!   3. March cubes → ExtractedMesh
//!   4. Apply surface material (iridescence, translucency, fresnel)
//!   5. Render into G-buffer via deferred pipeline
//! ```

pub mod entity_field;
pub mod field_eval;
pub mod marching_cubes;
pub mod gpu_marching_cubes;
pub mod surface_material;
pub mod damage;
pub mod templates;

pub use entity_field::{MetaballEntity, FieldSource, FalloffType};
pub use field_eval::FieldSample;
pub use marching_cubes::{MCVertex, ExtractedMesh, MarchingCubesExtractor};
pub use gpu_marching_cubes::GpuMarchingCubes;
pub use surface_material::{SurfaceMaterial, MaterialSample};
pub use damage::{DamageEvent, DamageResponse, DamageSystem};
pub use templates::EntityTemplate;
