// Quantum mechanics simulation modules for Proof Engine
//
// Provides Schrodinger equation solvers, wave function visualization,
// quantum tunneling, harmonic oscillators, hydrogen atom orbitals,
// double-slit experiments, entanglement, spin dynamics, measurement,
// and quantum computing primitives.

pub mod schrodinger;
pub mod wavefunction;
pub mod tunneling;
pub mod harmonic;
pub mod hydrogen;
pub mod double_slit;
pub mod entanglement;
pub mod spin;
pub mod measurement;
pub mod computing;

pub use schrodinger::{Complex, WaveFunction1D, SchrodingerSolver1D, SchrodingerSolver2D};
pub use wavefunction::{
    gaussian_wavepacket, plane_wave, momentum_space, wigner_function,
    WaveFunctionRenderer1D, WaveFunctionRenderer2D, DensityMatrix, PhaseColorMap,
};
pub use tunneling::{RectangularBarrier, transmission_coefficient, TunnelingSimulation, TunnelingResult};
pub use harmonic::{qho_energy, qho_wavefunction, hermite_polynomial};
pub use hydrogen::{hydrogen_energy, hydrogen_orbital, spherical_harmonic};
pub use double_slit::{DoubleSlitSetup, intensity_pattern};
pub use entanglement::{QubitState, TwoQubitState, bell_state};
pub use spin::{SpinState, bloch_angles, from_bloch};
pub use measurement::{MeasurementBasis, measure, BornRule};
pub use computing::{QuantumRegister, QuantumGate, QuantumCircuit};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_imports() {
        let c = Complex::new(1.0, 0.0);
        assert!((c.norm() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_hydrogen_ground_state_energy() {
        let e = hydrogen_energy(1);
        assert!((e - (-13.6)).abs() < 0.01);
    }

    #[test]
    fn test_qho_ground_state() {
        let e = qho_energy(0, 1.0, 1.0);
        assert!((e - 0.5).abs() < 1e-10);
    }
}
