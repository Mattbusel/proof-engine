//! Force fields — continuous spatial functions that apply forces to glyphs.

use glam::Vec3;
use super::attractors::AttractorType;
use super::MathFunction;

/// A spatial force that acts on glyphs within its region of influence.
#[derive(Clone, Debug)]
pub enum ForceField {
    /// Pulls glyphs toward a point, proportional to mass.
    Gravity { center: Vec3, strength: f32, falloff: Falloff },
    /// Pushes glyphs in a direction.
    Flow { direction: Vec3, strength: f32, turbulence: f32 },
    /// Spins glyphs around an axis.
    Vortex { center: Vec3, axis: Vec3, strength: f32, radius: f32 },
    /// Pushes glyphs away from a point.
    Repulsion { center: Vec3, strength: f32, radius: f32 },
    /// Attracts opposite charges, repels same charges.
    Electromagnetic { center: Vec3, charge: f32, strength: f32 },
    /// Increases temperature (and thus motion) of nearby glyphs.
    HeatSource { center: Vec3, temperature: f32, radius: f32 },
    /// Applies a math function to a glyph property in a region.
    MathField { center: Vec3, radius: f32, function: MathFunction, target: FieldTarget },
    /// Strange attractor dynamics pulling glyphs along attractor paths.
    StrangeAttractor { attractor_type: AttractorType, scale: f32, strength: f32, center: Vec3 },
    /// Increases entropy (chaos) of glyphs in a region.
    EntropyField { center: Vec3, radius: f32, strength: f32 },
    /// Reduces velocity of glyphs (viscosity).
    Damping { center: Vec3, radius: f32, strength: f32 },
}

/// How field strength decreases with distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Falloff {
    None,
    Linear,
    InverseSquare,
    Exponential(f32),
    Gaussian(f32),
}

/// Which property of a glyph a MathField modifies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldTarget {
    PositionX,
    PositionY,
    PositionZ,
    ColorR,
    ColorG,
    ColorB,
    ColorA,
    Scale,
    Rotation,
    Emission,
    Temperature,
    Entropy,
}

impl ForceField {
    /// Compute the force vector applied to a point at `pos` with `mass` and `charge`.
    pub fn force_at(&self, pos: Vec3, mass: f32, charge: f32, _t: f32) -> Vec3 {
        match self {
            ForceField::Gravity { center, strength, falloff } => {
                let delta = *center - pos;
                let dist = delta.length().max(0.01);
                let dir = delta / dist;
                let s = falloff_factor(*falloff, dist, 1.0) * strength * mass;
                dir * s
            }

            ForceField::Flow { direction, strength, turbulence: _ } => {
                direction.normalize_or_zero() * *strength
            }

            ForceField::Vortex { center, axis, strength, radius } => {
                let delta = pos - *center;
                let dist = delta.length();
                if dist > *radius || dist < 0.001 { return Vec3::ZERO; }
                let tangent = axis.normalize().cross(delta).normalize_or_zero();
                tangent * *strength * (1.0 - dist / radius)
            }

            ForceField::Repulsion { center, strength, radius } => {
                let delta = pos - *center;
                let dist = delta.length();
                if dist > *radius || dist < 0.001 { return Vec3::ZERO; }
                let dir = delta / dist;
                dir * *strength * (1.0 - dist / radius)
            }

            ForceField::Electromagnetic { center, charge: field_charge, strength } => {
                let delta = pos - *center;
                let dist = delta.length().max(0.01);
                let dir = delta / dist;
                // Same sign = repel, opposite = attract
                let sign = if charge * field_charge > 0.0 { 1.0 } else { -1.0 };
                dir * sign * *strength / (dist * dist)
            }

            ForceField::HeatSource { .. } | ForceField::EntropyField { .. } => {
                Vec3::ZERO // These fields modify glyph properties, not position
            }

            ForceField::Damping { center, radius, strength: _ } => {
                let dist = (pos - *center).length();
                if dist > *radius { return Vec3::ZERO; }
                Vec3::ZERO // Damping is applied as velocity reduction in scene tick
            }

            ForceField::MathField { .. } => Vec3::ZERO,

            ForceField::StrangeAttractor { attractor_type, scale, strength, center } => {
                let local = (pos - *center) / scale.max(0.001);
                let (_next, delta) = super::attractors::step(*attractor_type, local, 0.016);
                delta * *strength
            }
        }
    }

    /// Returns the temperature contribution at `pos` (for heat fields).
    pub fn temperature_at(&self, pos: Vec3) -> f32 {
        if let ForceField::HeatSource { center, temperature, radius } = self {
            let dist = (pos - *center).length();
            if dist < *radius {
                return temperature * (1.0 - dist / radius);
            }
        }
        0.0
    }

    /// Returns the entropy contribution at `pos` (for entropy fields).
    pub fn entropy_at(&self, pos: Vec3) -> f32 {
        if let ForceField::EntropyField { center, radius, strength } = self {
            let dist = (pos - *center).length();
            if dist < *radius {
                return strength * (1.0 - dist / radius);
            }
        }
        0.0
    }
}

fn falloff_factor(falloff: Falloff, distance: f32, max_distance: f32) -> f32 {
    match falloff {
        Falloff::None => 1.0,
        Falloff::Linear => (1.0 - distance / max_distance).max(0.0),
        Falloff::InverseSquare => 1.0 / (distance * distance).max(0.0001),
        Falloff::Exponential(rate) => (-distance * rate).exp(),
        Falloff::Gaussian(sigma) => {
            let x = distance / sigma;
            (-0.5 * x * x).exp()
        }
    }
}
