//! Electromagnetic simulation module — FDTD solvers, electric/magnetic fields,
//! EM wave propagation, charged particle dynamics, plasma PIC simulation,
//! lightning generation, Faraday shielding, and antenna radiation patterns.
//!
//! All simulations use real physics equations with glam math types.

pub mod fdtd;
pub mod electric;
pub mod magnetic;
pub mod waves;
pub mod charged_particles;
pub mod plasma;
pub mod lightning;
pub mod faraday;
pub mod antenna;

pub use fdtd::{FdtdGrid, FdtdGrid2D, MaterialGrid, FdtdRenderer};
pub use electric::{PointCharge, ElectricFieldLine, Dipole, Capacitor, LineCharge, ElectricFieldRenderer};
pub use magnetic::{CurrentSegment, InfiniteWire, CircularLoop, Solenoid, MagneticFieldRenderer};
pub use waves::{PlaneWave, SphericalWave, GaussianBeam, WavePacket, WaveRenderer};
pub use charged_particles::{ChargedParticle, ExBDrift, GradBDrift, CurvatureDrift, MagneticMirror, ParticleTracer, ChargedParticleSystem};
pub use plasma::{PicSimulation, PicParticle, PicGrid, PlasmaRenderer};
pub use lightning::{DielectricBreakdown, LightningBolt, LightningRenderer};
pub use faraday::{FaradayCage, CageRenderer, ConductingSphere};
pub use antenna::{HertzianDipole, HalfWaveDipole, AntennaArray, AntennaRenderer};
