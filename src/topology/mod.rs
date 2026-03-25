// topology/mod.rs — Module hub for topological spaces and non-Euclidean geometry

pub mod hyperbolic;
pub mod spherical;
pub mod toroidal;
pub mod klein;
pub mod mobius;
pub mod projective;
pub mod genus;
pub mod tiling;
pub mod portals;
pub mod geodesic;
pub mod curvature;

pub use hyperbolic::{PoincareDisk, KleinDisk, HyperbolicTiling, HyperbolicPolygon, HyperbolicGrid};
pub use spherical::{SphericalCoord, SphericalGrid};
pub use toroidal::ToroidalSpace;
pub use klein::{KleinBottle, KleinNavigation, KleinRenderer};
pub use mobius::{MobiusStrip, MobiusNavigation};
pub use projective::ProjectivePlane;
pub use genus::{SurfaceType, GenusRenderer};
pub use tiling::{WallpaperGroup, TileInstance, FundamentalDomain, PenroseTile, PenroseType};
pub use portals::{Portal, PortalFrame, TopologyType, PortalManager};
pub use geodesic::GeodesicSurface;
pub use curvature::{CurvatureField, GaussianCurvature};
