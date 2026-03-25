//! Relativistic physics module for Proof Engine.
//!
//! Provides implementations of special and general relativistic effects
//! for game rendering: Lorentz contraction, time dilation, Doppler shifts,
//! gravitational lensing, black holes, wormholes, and spacetime diagrams.

pub mod lorentz;
pub mod time_dilation;
pub mod doppler;
pub mod searchlight;
pub mod terrell;
pub mod lensing;
pub mod grav_time;
pub mod black_hole;
pub mod wormhole;
pub mod spacetime;

pub use lorentz::{
    lorentz_factor, FourVector, LorentzBoost, LorentzContractor, LorentzRenderer,
    contract_length, proper_time, velocity_addition, rapidity,
    four_momentum, relativistic_energy, relativistic_momentum, invariant_mass,
};
pub use time_dilation::{
    time_dilation_factor, DilatedClock, TimeDilationRenderer, ClockComparison,
    twin_paradox, muon_lifetime, gravitational_time_dilation,
};
pub use doppler::{
    relativistic_doppler, transverse_doppler, wavelength_shift, redshift_z,
    color_from_wavelength, doppler_color_shift, DopplerRenderer, cosmic_redshift,
};
pub use searchlight::{
    relativistic_beaming, aberration_angle, headlight_factor, SearchlightRenderer,
    apparent_brightness, solid_angle_transform,
};
pub use terrell::{
    terrell_rotation_angle, apparent_position, retarded_time, TerellRenderer,
    render_moving_cube, finite_light_speed_positions,
};
pub use lensing::{
    deflection_angle, einstein_radius, lens_equation, image_positions,
    magnification, LensingField, apply_lensing, LensingRenderer,
    microlensing_lightcurve,
};
pub use grav_time::{
    schwarzschild_time_dilation, gravitational_redshift, proper_time_rate,
    gps_correction, GravTimeDilationField, GravTimeRenderer,
};
pub use black_hole::{
    schwarzschild_radius, photon_sphere_radius, isco_radius, BlackHole,
    ray_trace_schwarzschild, ray_deflection, AccretionDisk, disk_emission,
    render_black_hole, shadow_boundary, tidal_force, hawking_temperature,
    BlackHoleRenderer,
};
pub use wormhole::{
    EllisWormhole, proper_distance, embedding_diagram, ray_trace_wormhole,
    render_wormhole, WormholePortal, transform_through_wormhole,
    wormhole_stability, WormholeRenderer,
};
pub use spacetime::{
    MinkowskiDiagram, SpacetimeEvent, Worldline, light_cone,
    is_timelike, is_spacelike, is_lightlike, proper_time_along_worldline,
    boost_diagram, PenroseDiagram, penrose_transform, SpacetimeRenderer,
    causal_structure,
};
