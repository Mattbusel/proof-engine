//! Mathematical curves as entity structure.
//!
//! Entities drawn with Bezier curves, Lissajous figures, parametric equations,
//! spirals, roses, and hypotrochoids instead of discrete glyph characters.
//! Every visual comes from an equation.

pub mod entity_curves;
pub mod tessellate;
pub mod curve_renderer;
pub mod curve_anim;
pub mod dissolve;
pub mod templates;

pub use entity_curves::{CurveEntity, EntityCurve, CurveType};
pub use tessellate::tessellate_curve;
pub use curve_anim::{CurveAnimState, CurveAnimator};
pub use dissolve::DissolveState;
pub use templates::CurveTemplates;
