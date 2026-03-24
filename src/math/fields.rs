//! Force fields — continuous spatial functions that apply forces to glyphs.
//!
//! ## Field taxonomy
//! - **Simple fields**: gravity, flow, vortex, repulsion, heat, damping
//! - **Math fields**: apply a `MathFunction` to a glyph property
//! - **Strange attractors**: drive motion along attractor trajectories
//! - **Composed fields**: combine multiple fields with blend operators
//! - **Animated fields**: fields whose parameters change over time
//!
//! All fields implement `force_at(pos, mass, charge, t) → Vec3`, making
//! them interchangeable in the scene graph field manager.

use glam::{Vec2, Vec3};
use super::attractors::AttractorType;
use super::MathFunction;

// ── ForceField ────────────────────────────────────────────────────────────────

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
    /// Oscillating push-pull field.
    Pulsing { center: Vec3, frequency: f32, amplitude: f32, radius: f32 },
    /// Shockwave expanding outward from an origin.
    Shockwave { center: Vec3, speed: f32, thickness: f32, strength: f32, born_at: f32 },
    /// Wind with per-layer turbulence (Perlin-driven).
    Wind { direction: Vec3, base_strength: f32, gust_frequency: f32, gust_amplitude: f32 },
    /// Portal-style warp field: pulls toward center, ejects on the other side.
    Warp { center: Vec3, exit: Vec3, radius: f32, strength: f32 },
    /// Tidal force (stretches along one axis, squishes along others).
    Tidal { center: Vec3, axis: Vec3, strength: f32, radius: f32 },
    /// Magnetic dipole (north + south poles).
    MagneticDipole { center: Vec3, axis: Vec3, moment: f32 },
    /// Saddle point field (hyperbolic potential).
    Saddle { center: Vec3, strength_x: f32, strength_y: f32 },
}

/// How field strength decreases with distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Falloff {
    None,
    Linear,
    InverseSquare,
    Exponential(f32),
    Gaussian(f32),
    /// Smooth step: linear to 1.0, then smoothstep to 0.
    SmoothStep(f32),
}

/// Which property of a glyph a MathField modifies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldTarget {
    PositionX, PositionY, PositionZ,
    ColorR, ColorG, ColorB, ColorA,
    Scale, Rotation, Emission, Temperature, Entropy,
}

impl ForceField {
    /// Compute the force vector applied to a point at `pos` with `mass`,
    /// `charge`, and world time `t`.
    pub fn force_at(&self, pos: Vec3, mass: f32, charge: f32, t: f32) -> Vec3 {
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
                let sign = if charge * field_charge > 0.0 { 1.0 } else { -1.0 };
                dir * sign * *strength / (dist * dist)
            }

            ForceField::HeatSource { .. } | ForceField::EntropyField { .. } => Vec3::ZERO,

            ForceField::Damping { center, radius, strength: _ } => {
                let dist = (pos - *center).length();
                if dist > *radius { return Vec3::ZERO; }
                Vec3::ZERO // Damping applied as velocity scale in scene tick
            }

            ForceField::MathField { .. } => Vec3::ZERO,

            ForceField::StrangeAttractor { attractor_type, scale, strength, center } => {
                let local = (pos - *center) / scale.max(0.001);
                let (_next, delta) = super::attractors::step(*attractor_type, local, 0.016);
                delta * *strength
            }

            ForceField::Pulsing { center, frequency, amplitude, radius } => {
                let dist = (pos - *center).length();
                if dist > *radius || dist < 0.001 { return Vec3::ZERO; }
                let dir = (pos - *center).normalize_or_zero();
                let wave = (t * frequency * std::f32::consts::TAU).sin();
                dir * *amplitude * wave * (1.0 - dist / radius)
            }

            ForceField::Shockwave { center, speed, thickness, strength, born_at } => {
                let dist = (pos - *center).length();
                let wave_r = (t - born_at) * speed;
                let diff = (dist - wave_r).abs();
                if diff > *thickness { return Vec3::ZERO; }
                let dir = (pos - *center).normalize_or_zero();
                let falloff = 1.0 - diff / thickness;
                dir * *strength * falloff / (wave_r + 1.0)
            }

            ForceField::Wind { direction, base_strength, gust_frequency, gust_amplitude } => {
                let gust = (t * gust_frequency * std::f32::consts::TAU
                           + pos.x * 0.3 + pos.z * 0.2).sin() * gust_amplitude;
                direction.normalize_or_zero() * (base_strength + gust)
            }

            ForceField::Warp { center, exit: _, radius, strength } => {
                let delta = pos - *center;
                let dist = delta.length();
                if dist > *radius || dist < 0.001 { return Vec3::ZERO; }
                let dir = -delta.normalize_or_zero(); // pull toward center
                dir * *strength * (1.0 - dist / radius).powi(2)
            }

            ForceField::Tidal { center, axis, strength, radius } => {
                let delta = pos - *center;
                let dist = delta.length();
                if dist > *radius { return Vec3::ZERO; }
                let ax = axis.normalize();
                let along = ax * ax.dot(delta);
                let perp  = delta - along;
                // Stretch along axis, compress perpendicular
                (along * 2.0 - perp) * *strength * (1.0 - dist / radius)
            }

            ForceField::MagneticDipole { center, axis, moment } => {
                let r = pos - *center;
                let dist = r.length().max(0.01);
                let r_hat = r / dist;
                let m = axis.normalize() * *moment;
                let factor = 1.0 / (dist * dist * dist);
                (3.0 * r_hat * r_hat.dot(m) - m) * factor
            }

            ForceField::Saddle { center, strength_x, strength_y } => {
                let d = pos - *center;
                Vec3::new(d.x * strength_x, -d.y * strength_y, 0.0)
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

    /// Returns the entropy contribution at `pos`.
    pub fn entropy_at(&self, pos: Vec3) -> f32 {
        if let ForceField::EntropyField { center, radius, strength } = self {
            let dist = (pos - *center).length();
            if dist < *radius {
                return strength * (1.0 - dist / radius);
            }
        }
        0.0
    }

    /// Returns the damping multiplier at `pos` (1.0 = no damping, 0.0 = full stop).
    pub fn damping_at(&self, pos: Vec3) -> f32 {
        if let ForceField::Damping { center, radius, strength } = self {
            let dist = (pos - *center).length();
            if dist < *radius {
                return 1.0 - strength * (1.0 - dist / radius);
            }
        }
        1.0
    }

    /// True if this field type is purely visual (no position force).
    pub fn is_non_positional(&self) -> bool {
        matches!(self,
            ForceField::HeatSource { .. }
          | ForceField::EntropyField { .. }
          | ForceField::MathField { .. }
          | ForceField::Damping { .. }
        )
    }

    /// Returns a friendly label for debug UI.
    pub fn label(&self) -> &'static str {
        match self {
            ForceField::Gravity { .. }          => "Gravity",
            ForceField::Flow { .. }             => "Flow",
            ForceField::Vortex { .. }           => "Vortex",
            ForceField::Repulsion { .. }        => "Repulsion",
            ForceField::Electromagnetic { .. }  => "EM",
            ForceField::HeatSource { .. }       => "Heat",
            ForceField::MathField { .. }        => "Math",
            ForceField::StrangeAttractor { .. } => "Attractor",
            ForceField::EntropyField { .. }     => "Entropy",
            ForceField::Damping { .. }          => "Damping",
            ForceField::Pulsing { .. }          => "Pulsing",
            ForceField::Shockwave { .. }        => "Shockwave",
            ForceField::Wind { .. }             => "Wind",
            ForceField::Warp { .. }             => "Warp",
            ForceField::Tidal { .. }            => "Tidal",
            ForceField::MagneticDipole { .. }   => "Dipole",
            ForceField::Saddle { .. }           => "Saddle",
        }
    }

    /// Returns the center position if this field has one, else None.
    pub fn center(&self) -> Option<Vec3> {
        match self {
            ForceField::Gravity { center, .. }          => Some(*center),
            ForceField::Vortex { center, .. }           => Some(*center),
            ForceField::Repulsion { center, .. }        => Some(*center),
            ForceField::Electromagnetic { center, .. }  => Some(*center),
            ForceField::HeatSource { center, .. }       => Some(*center),
            ForceField::MathField { center, .. }        => Some(*center),
            ForceField::StrangeAttractor { center, .. } => Some(*center),
            ForceField::EntropyField { center, .. }     => Some(*center),
            ForceField::Damping { center, .. }          => Some(*center),
            ForceField::Pulsing { center, .. }          => Some(*center),
            ForceField::Shockwave { center, .. }        => Some(*center),
            ForceField::Warp { center, .. }             => Some(*center),
            ForceField::Tidal { center, .. }            => Some(*center),
            ForceField::MagneticDipole { center, .. }   => Some(*center),
            ForceField::Saddle { center, .. }           => Some(*center),
            _ => None,
        }
    }
}

// ── Falloff ───────────────────────────────────────────────────────────────────

pub fn falloff_factor(falloff: Falloff, distance: f32, max_distance: f32) -> f32 {
    match falloff {
        Falloff::None           => 1.0,
        Falloff::Linear         => (1.0 - distance / max_distance).max(0.0),
        Falloff::InverseSquare  => 1.0 / (distance * distance).max(0.0001),
        Falloff::Exponential(r) => (-distance * r).exp(),
        Falloff::Gaussian(sig)  => {
            let x = distance / sig;
            (-0.5 * x * x).exp()
        }
        Falloff::SmoothStep(r) => {
            let t = (1.0 - distance / r).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
    }
}

// ── FieldComposer ─────────────────────────────────────────────────────────────

/// Combines multiple fields with a blend operator.
#[derive(Clone, Debug)]
pub struct FieldComposer {
    pub layers: Vec<FieldLayer>,
}

#[derive(Clone, Debug)]
pub struct FieldLayer {
    pub field:  ForceField,
    pub blend:  FieldBlend,
    pub weight: f32,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldBlend {
    /// Add force vectors.
    Add,
    /// Multiply force vectors (modulation).
    Multiply,
    /// Take the maximum of each component.
    Max,
    /// Override: this layer replaces all previous layers.
    Override,
    /// Subtract this layer's force from accumulated.
    Subtract,
}

impl FieldComposer {
    pub fn new() -> Self { Self { layers: Vec::new() } }

    pub fn add(mut self, field: ForceField) -> Self {
        self.layers.push(FieldLayer { field, blend: FieldBlend::Add, weight: 1.0, enabled: true });
        self
    }

    pub fn add_weighted(mut self, field: ForceField, weight: f32) -> Self {
        self.layers.push(FieldLayer { field, blend: FieldBlend::Add, weight, enabled: true });
        self
    }

    pub fn add_blended(mut self, field: ForceField, blend: FieldBlend, weight: f32) -> Self {
        self.layers.push(FieldLayer { field, blend, weight, enabled: true });
        self
    }

    /// Evaluate the composed force at a point.
    pub fn force_at(&self, pos: Vec3, mass: f32, charge: f32, t: f32) -> Vec3 {
        let mut acc = Vec3::ZERO;
        for layer in &self.layers {
            if !layer.enabled { continue; }
            let f = layer.field.force_at(pos, mass, charge, t) * layer.weight;
            acc = match layer.blend {
                FieldBlend::Add      => acc + f,
                FieldBlend::Subtract => acc - f,
                FieldBlend::Multiply => acc * f,
                FieldBlend::Max      => acc.max(f),
                FieldBlend::Override => f,
            };
        }
        acc
    }

    pub fn enable_layer(&mut self, idx: usize, enabled: bool) {
        if let Some(l) = self.layers.get_mut(idx) { l.enabled = enabled; }
    }

    pub fn set_weight(&mut self, idx: usize, weight: f32) {
        if let Some(l) = self.layers.get_mut(idx) { l.weight = weight; }
    }
}

// ── FieldSampler ──────────────────────────────────────────────────────────────

/// Samples a field onto a 2D grid for visualization.
pub struct FieldSampler {
    pub width:  usize,
    pub height: usize,
    pub x_min:  f32,
    pub x_max:  f32,
    pub y_min:  f32,
    pub y_max:  f32,
    pub z:      f32,
    /// Sampled force vectors (flat array, row-major).
    pub forces: Vec<Vec3>,
    pub magnitudes: Vec<f32>,
}

impl FieldSampler {
    pub fn new(width: usize, height: usize, bounds: (f32, f32, f32, f32)) -> Self {
        let n = width * height;
        Self {
            width, height,
            x_min: bounds.0, x_max: bounds.2,
            y_min: bounds.1, y_max: bounds.3,
            z: 0.0,
            forces:     vec![Vec3::ZERO; n],
            magnitudes: vec![0.0; n],
        }
    }

    /// Sample the field at all grid points.
    pub fn sample(&mut self, field: &ForceField) {
        let dx = (self.x_max - self.x_min) / self.width as f32;
        let dy = (self.y_max - self.y_min) / self.height as f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let wx = self.x_min + (x as f32 + 0.5) * dx;
                let wy = self.y_min + (y as f32 + 0.5) * dy;
                let f = field.force_at(Vec3::new(wx, wy, self.z), 1.0, 0.0, 0.0);
                let i = y * self.width + x;
                self.forces[i] = f;
                self.magnitudes[i] = f.length();
            }
        }
    }

    /// Sample a composer at all grid points.
    pub fn sample_composer(&mut self, composer: &FieldComposer) {
        let dx = (self.x_max - self.x_min) / self.width as f32;
        let dy = (self.y_max - self.y_min) / self.height as f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let wx = self.x_min + (x as f32 + 0.5) * dx;
                let wy = self.y_min + (y as f32 + 0.5) * dy;
                let f = composer.force_at(Vec3::new(wx, wy, self.z), 1.0, 0.0, 0.0);
                let i = y * self.width + x;
                self.forces[i] = f;
                self.magnitudes[i] = f.length();
            }
        }
    }

    /// Maximum magnitude seen in the last sample.
    pub fn max_magnitude(&self) -> f32 {
        self.magnitudes.iter().cloned().fold(0.0_f32, f32::max)
    }

    /// Get the force at grid cell (x, y).
    pub fn force_at_cell(&self, x: usize, y: usize) -> Vec3 {
        self.forces.get(y * self.width + x).copied().unwrap_or(Vec3::ZERO)
    }

    /// Compute normalized flow lines for visualization (streamline points).
    pub fn streamline(&self, start: Vec2, steps: usize, dt: f32) -> Vec<Vec2> {
        let mut pts = Vec::with_capacity(steps);
        let mut pos = start;
        for _ in 0..steps {
            pts.push(pos);
            let f = self.sample_at_world(pos);
            if f.length_squared() < 1e-6 { break; }
            pos += f * dt;
        }
        pts
    }

    fn sample_at_world(&self, pos: Vec2) -> Vec2 {
        let tx = (pos.x - self.x_min) / (self.x_max - self.x_min);
        let ty = (pos.y - self.y_min) / (self.y_max - self.y_min);
        let cx = (tx * self.width as f32).clamp(0.0, self.width as f32 - 1.001);
        let cy = (ty * self.height as f32).clamp(0.0, self.height as f32 - 1.001);
        let x0 = cx.floor() as usize;
        let y0 = cy.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx = cx.fract();
        let fy = cy.fract();
        let f00 = self.forces[y0 * self.width + x0].truncate();
        let f10 = self.forces[y0 * self.width + x1].truncate();
        let f01 = self.forces[y1 * self.width + x0].truncate();
        let f11 = self.forces[y1 * self.width + x1].truncate();
        let f0 = Vec2::lerp(f00, f10, fx);
        let f1 = Vec2::lerp(f01, f11, fx);
        Vec2::lerp(f0, f1, fy)
    }

    /// Render force vectors as a flat RGBA buffer (for debug textures).
    pub fn to_rgba(&self) -> Vec<u8> {
        let n = self.width * self.height;
        let max_mag = self.max_magnitude().max(0.001);
        let mut out = vec![0u8; n * 4];
        for i in 0..n {
            let f = self.forces[i];
            let r = (f.x / max_mag * 0.5 + 0.5).clamp(0.0, 1.0);
            let g = (f.y / max_mag * 0.5 + 0.5).clamp(0.0, 1.0);
            let b = (self.magnitudes[i] / max_mag).clamp(0.0, 1.0);
            out[i * 4    ] = (r * 255.0) as u8;
            out[i * 4 + 1] = (g * 255.0) as u8;
            out[i * 4 + 2] = (b * 255.0) as u8;
            out[i * 4 + 3] = 255;
        }
        out
    }
}

// ── AnimatedField ─────────────────────────────────────────────────────────────

/// A field whose strength or center animates over time.
#[derive(Clone, Debug)]
pub struct AnimatedField {
    pub field:    ForceField,
    pub timeline: Vec<AnimationKey>,
}

#[derive(Clone, Debug)]
pub struct AnimationKey {
    pub time:     f32,
    pub strength: f32,
    pub offset:   Vec3,
}

impl AnimatedField {
    pub fn new(field: ForceField) -> Self {
        Self { field, timeline: Vec::new() }
    }

    pub fn key(mut self, time: f32, strength: f32, offset: Vec3) -> Self {
        self.timeline.push(AnimationKey { time, strength, offset });
        self.timeline.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        self
    }

    pub fn force_at(&self, pos: Vec3, mass: f32, charge: f32, t: f32) -> Vec3 {
        let (strength, offset) = self.eval_at(t);
        let shifted_pos = pos - offset;
        self.field.force_at(shifted_pos, mass, charge, t) * strength
    }

    fn eval_at(&self, t: f32) -> (f32, Vec3) {
        if self.timeline.is_empty() { return (1.0, Vec3::ZERO); }
        if self.timeline.len() == 1 {
            let k = &self.timeline[0];
            return (k.strength, k.offset);
        }
        if t <= self.timeline[0].time {
            let k = &self.timeline[0];
            return (k.strength, k.offset);
        }
        let last = self.timeline.last().unwrap();
        if t >= last.time { return (last.strength, last.offset); }

        let i = self.timeline.partition_point(|k| k.time <= t) - 1;
        let k0 = &self.timeline[i];
        let k1 = &self.timeline[i + 1];
        let span = k1.time - k0.time;
        let ft = if span < 1e-6 { 0.0 } else { (t - k0.time) / span };
        let strength = k0.strength + (k1.strength - k0.strength) * ft;
        let offset   = Vec3::lerp(k0.offset, k1.offset, ft);
        (strength, offset)
    }
}

// ── FieldPresets ──────────────────────────────────────────────────────────────

/// Factory methods for common in-game force field configurations.
pub struct FieldPresets;

impl FieldPresets {
    /// Planet-like gravity well.
    pub fn planet(center: Vec3, mass: f32) -> ForceField {
        ForceField::Gravity {
            center,
            strength: mass * 6.674e-3,
            falloff:  Falloff::InverseSquare,
        }
    }

    /// Dust devil / tornado vortex.
    pub fn tornado(center: Vec3, strength: f32, radius: f32) -> ForceField {
        ForceField::Vortex {
            center,
            axis:     Vec3::Y,
            strength,
            radius,
        }
    }

    /// Omnidirectional explosion shockwave.
    pub fn explosion(center: Vec3, strength: f32, born_at: f32) -> ForceField {
        ForceField::Shockwave {
            center,
            speed:     15.0,
            thickness: 3.0,
            strength,
            born_at,
        }
    }

    /// River current flowing in a direction.
    pub fn river(direction: Vec3, speed: f32) -> ForceField {
        ForceField::Flow { direction, strength: speed, turbulence: 0.1 }
    }

    /// Bonfire heat column rising upward.
    pub fn bonfire(center: Vec3, heat: f32) -> ForceField {
        ForceField::HeatSource { center, temperature: heat, radius: 3.0 }
    }

    /// Spinning galaxy arm (Lorenz-driven).
    pub fn galaxy_arm(center: Vec3, scale: f32) -> ForceField {
        ForceField::StrangeAttractor {
            attractor_type: AttractorType::Lorenz,
            scale,
            strength: 0.5,
            center,
        }
    }

    /// Frost aura: cold damping field.
    pub fn frost_aura(center: Vec3, radius: f32) -> ForceField {
        ForceField::Damping { center, radius, strength: 0.7 }
    }

    /// Chaos zone: pure entropy field.
    pub fn chaos_zone(center: Vec3, radius: f32) -> ForceField {
        ForceField::EntropyField { center, radius, strength: 2.0 }
    }

    /// Pendulum: oscillating gravity with period.
    pub fn pendulum(center: Vec3, amplitude: f32, frequency: f32, radius: f32) -> ForceField {
        ForceField::Pulsing { center, frequency, amplitude, radius }
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Vec3 { Vec3::ZERO }

    #[test]
    fn test_gravity_pulls_toward_center() {
        let field = ForceField::Gravity {
            center:   Vec3::new(5.0, 0.0, 0.0),
            strength: 1.0,
            falloff:  Falloff::None,
        };
        let f = field.force_at(Vec3::ZERO, 1.0, 0.0, 0.0);
        assert!(f.x > 0.0); // should pull toward +X
    }

    #[test]
    fn test_repulsion_pushes_away() {
        let field = ForceField::Repulsion { center: origin(), strength: 1.0, radius: 10.0 };
        let f = field.force_at(Vec3::new(1.0, 0.0, 0.0), 1.0, 0.0, 0.0);
        assert!(f.x > 0.0); // pushes away from origin
    }

    #[test]
    fn test_repulsion_zero_outside_radius() {
        let field = ForceField::Repulsion { center: origin(), strength: 1.0, radius: 1.0 };
        let f = field.force_at(Vec3::new(5.0, 0.0, 0.0), 1.0, 0.0, 0.0);
        assert_eq!(f, Vec3::ZERO);
    }

    #[test]
    fn test_flow_direction() {
        let field = ForceField::Flow {
            direction: Vec3::X,
            strength:  2.0,
            turbulence: 0.0,
        };
        let f = field.force_at(origin(), 1.0, 0.0, 0.0);
        assert!((f.x - 2.0).abs() < 0.01);
        assert!(f.y.abs() < 0.01);
    }

    #[test]
    fn test_vortex_tangential() {
        let field = ForceField::Vortex {
            center:   Vec3::ZERO,
            axis:     Vec3::Z,
            strength: 1.0,
            radius:   10.0,
        };
        let f = field.force_at(Vec3::new(1.0, 0.0, 0.0), 1.0, 0.0, 0.0);
        // Tangent to +X should be ±Y
        assert!(f.y.abs() > 0.1);
        assert!(f.x.abs() < 0.1);
    }

    #[test]
    fn test_pulsing_varies_over_time() {
        let field = ForceField::Pulsing {
            center:    origin(),
            frequency: 1.0,
            amplitude: 1.0,
            radius:    10.0,
        };
        let f0 = field.force_at(Vec3::X, 1.0, 0.0, 0.0);
        let f1 = field.force_at(Vec3::X, 1.0, 0.0, 0.25);
        assert!((f0.x - f1.x).abs() > 0.01);
    }

    #[test]
    fn test_composer_add() {
        let c = FieldComposer::new()
            .add(ForceField::Flow { direction: Vec3::X, strength: 1.0, turbulence: 0.0 })
            .add(ForceField::Flow { direction: Vec3::X, strength: 1.0, turbulence: 0.0 });
        let f = c.force_at(origin(), 1.0, 0.0, 0.0);
        assert!((f.x - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_field_sampler() {
        let mut sampler = FieldSampler::new(8, 8, (-4.0, -4.0, 4.0, 4.0));
        let field = ForceField::Flow { direction: Vec3::X, strength: 1.0, turbulence: 0.0 };
        sampler.sample(&field);
        let max = sampler.max_magnitude();
        assert!((max - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_animated_field() {
        let af = AnimatedField::new(
            ForceField::Flow { direction: Vec3::X, strength: 1.0, turbulence: 0.0 }
        )
        .key(0.0, 0.0, Vec3::ZERO)
        .key(1.0, 2.0, Vec3::ZERO);
        let f0 = af.force_at(Vec3::ZERO, 1.0, 0.0, 0.0);
        let f1 = af.force_at(Vec3::ZERO, 1.0, 0.0, 1.0);
        assert!(f0.x < f1.x); // stronger at t=1
    }

    #[test]
    fn test_falloff_factors() {
        assert!((falloff_factor(Falloff::None, 5.0, 10.0) - 1.0).abs() < 1e-6);
        assert!((falloff_factor(Falloff::Linear, 5.0, 10.0) - 0.5).abs() < 1e-6);
        assert!(falloff_factor(Falloff::InverseSquare, 2.0, 10.0) < 1.0);
        let g = falloff_factor(Falloff::Gaussian(2.0), 0.0, 10.0);
        assert!((g - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_preset_planet_gravity() {
        let field = FieldPresets::planet(Vec3::new(10.0, 0.0, 0.0), 100.0);
        let f = field.force_at(Vec3::ZERO, 1.0, 0.0, 0.0);
        assert!(f.x > 0.0);
    }

    #[test]
    fn test_shockwave_zero_before_wave_arrives() {
        let field = FieldPresets::explosion(origin(), 10.0, 0.0);
        let f = field.force_at(Vec3::new(100.0, 0.0, 0.0), 1.0, 0.0, 0.0);
        assert_eq!(f, Vec3::ZERO);
    }

    #[test]
    fn test_field_label() {
        let field = ForceField::Flow { direction: Vec3::X, strength: 1.0, turbulence: 0.0 };
        assert_eq!(field.label(), "Flow");
    }
}
