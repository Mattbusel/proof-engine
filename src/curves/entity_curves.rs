//! Core data structures for curve-based entities.
//!
//! A CurveEntity is a collection of mathematical curves that form a visible
//! entity in the scene. Each curve responds to force fields, breathing,
//! damage, and death the same way glyph clusters do.

use glam::{Vec2, Vec3, Vec4};
use crate::math::MathFunction;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Curve types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The mathematical definition of a curve.
#[derive(Debug, Clone)]
pub enum CurveType {
    /// Bezier curve of arbitrary degree (control points define shape).
    Bezier { degree: u32 },
    /// Lissajous figure: x = A*sin(a*t + delta), y = B*sin(b*t).
    Lissajous { a: f32, b: f32, delta: f32 },
    /// Parametric curve defined by two MathFunctions: x(t), y(t).
    Parametric { x_fn: MathFunction, y_fn: MathFunction },
    /// Circle with optional distortion function applied to radius.
    Circle { radius: f32, distortion: Option<MathFunction> },
    /// Spiral: r = rate * theta, with optional decay.
    Spiral { rate: f32, decay: f32 },
    /// Rose curve: r = amplitude * cos(k * theta).
    Rose { k: f32, amplitude: f32 },
    /// Hypotrochoid: the curve traced by a point on a circle rolling inside another.
    Hypotrochoid { big_r: f32, small_r: f32, d: f32 },
    /// Superellipse (Lame curve): |x/a|^n + |y/b|^n = 1.
    Superellipse { a: f32, b: f32, n: f32 },
    /// Catenary: y = a * cosh(x/a).
    Catenary { a: f32, span: f32 },
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Single curve
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single mathematical curve within an entity.
#[derive(Debug, Clone)]
pub struct EntityCurve {
    /// The mathematical definition.
    pub curve_type: CurveType,
    /// Control points (interpretation depends on curve_type).
    /// For Bezier: these ARE the control points.
    /// For others: these define offset/scale/anchor positions.
    pub control_points: Vec<Vec2>,
    /// Base (unmodified) control points for restoring after deformation.
    pub base_points: Vec<Vec2>,
    /// RGBA color of this curve.
    pub color: Vec4,
    /// Emission intensity (for glow/bloom).
    pub emission: f32,
    /// Line thickness in world units.
    pub thickness: f32,
    /// Stiffness: how much this curve resists external forces (higher = more rigid).
    pub stiffness: f32,
    /// Number of line segments to tessellate into.
    pub segment_count: u32,
    /// Whether this curve is still intact (false = broken by crit/death).
    pub alive: bool,
    /// Per-point velocity (for physics response and dissolution).
    pub point_velocities: Vec<Vec2>,
    /// Closed curve (connect last point back to first).
    pub closed: bool,
    /// Dash pattern: None = solid, Some(on, off) = dashed.
    pub dash_pattern: Option<(f32, f32)>,
    /// Layer priority for rendering order.
    pub layer: u32,
}

impl EntityCurve {
    /// Create a new curve with the given type and initial control points.
    pub fn new(curve_type: CurveType, control_points: Vec<Vec2>) -> Self {
        let n = control_points.len();
        Self {
            base_points: control_points.clone(),
            control_points,
            curve_type,
            color: Vec4::new(0.5, 0.7, 1.0, 0.9),
            emission: 1.0,
            thickness: 0.03,
            stiffness: 1.0,
            segment_count: 64,
            alive: true,
            point_velocities: vec![Vec2::ZERO; n],
            closed: false,
            dash_pattern: None,
            layer: 0,
        }
    }

    pub fn with_color(mut self, color: Vec4) -> Self { self.color = color; self }
    pub fn with_emission(mut self, e: f32) -> Self { self.emission = e; self }
    pub fn with_thickness(mut self, t: f32) -> Self { self.thickness = t; self }
    pub fn with_stiffness(mut self, s: f32) -> Self { self.stiffness = s; self }
    pub fn with_segments(mut self, n: u32) -> Self { self.segment_count = n; self }
    pub fn with_closed(mut self, c: bool) -> Self { self.closed = c; self }
    pub fn with_dash(mut self, on: f32, off: f32) -> Self { self.dash_pattern = Some((on, off)); self }
    pub fn with_layer(mut self, l: u32) -> Self { self.layer = l; self }

    /// Apply an external force impulse to all control points.
    /// Points with lower stiffness move more.
    pub fn apply_force(&mut self, force: Vec2) {
        let inv_stiff = 1.0 / self.stiffness.max(0.01);
        for vel in &mut self.point_velocities {
            *vel += force * inv_stiff;
        }
    }

    /// Apply a directional impulse from a hit (recoil).
    /// Points closer to the impact side are affected more.
    pub fn apply_hit_recoil(&mut self, direction: Vec2, magnitude: f32) {
        let dir = direction.normalize_or_zero();
        let inv_stiff = 1.0 / self.stiffness.max(0.01);
        for (i, pt) in self.control_points.iter().enumerate() {
            let facing = (pt.normalize_or_zero()).dot(dir);
            let response = if facing > 0.0 {
                dir * magnitude * (0.5 + facing * 0.5) * inv_stiff
            } else {
                dir * magnitude * 0.2 * inv_stiff
            };
            self.point_velocities[i] += response;
        }
    }

    /// Step physics: apply velocity, spring-back toward base positions, damping.
    pub fn step_physics(&mut self, dt: f32, damping: f32) {
        for i in 0..self.control_points.len() {
            // Spring toward base position
            let to_base = self.base_points[i] - self.control_points[i];
            let spring_force = to_base * self.stiffness * 5.0;
            self.point_velocities[i] += spring_force * dt;

            // Damping
            self.point_velocities[i] *= damping;

            // Integrate
            self.control_points[i] += self.point_velocities[i] * dt;
        }
    }

    /// Break this curve (from crit hit or death).
    pub fn break_curve(&mut self) {
        self.alive = false;
        self.stiffness = 0.0;
    }

    /// Total energy (kinetic) in this curve's control points.
    pub fn kinetic_energy(&self) -> f32 {
        self.point_velocities.iter().map(|v| v.length_squared()).sum::<f32>() * 0.5
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Curve entity (collection of curves forming one entity)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A complete entity composed of mathematical curves.
#[derive(Debug, Clone)]
pub struct CurveEntity {
    /// All curves composing this entity.
    pub curves: Vec<EntityCurve>,
    /// World-space position of this entity.
    pub position: Vec3,
    /// Center of mass (computed from control points).
    pub center_of_mass: Vec2,
    /// HP ratio (1.0 = full, 0.0 = dead).
    pub hp_ratio: f32,
    /// Breathing oscillation phase.
    pub breath_phase: f32,
    /// Breathing rate (Hz).
    pub breath_rate: f32,
    /// Breathing amplitude (how much curves expand/contract).
    pub breath_amplitude: f32,
    /// Accumulated force response (for hit recoil).
    pub force_response: Vec2,
    /// Entity name/type.
    pub name: String,
    /// Global emission multiplier.
    pub emission_mult: f32,
    /// Global damping for all curve physics.
    pub damping: f32,
    /// Whether this entity is alive.
    pub alive: bool,
    /// Time since death (for dissolution).
    pub death_time: f32,
    /// Total accumulated time.
    pub time: f32,
    /// Unique ID.
    pub id: u32,
}

impl CurveEntity {
    pub fn new(name: &str, position: Vec3) -> Self {
        Self {
            curves: Vec::new(),
            position,
            center_of_mass: Vec2::ZERO,
            hp_ratio: 1.0,
            breath_phase: 0.0,
            breath_rate: 0.5,
            breath_amplitude: 0.03,
            force_response: Vec2::ZERO,
            name: name.to_string(),
            emission_mult: 1.0,
            damping: 0.92,
            alive: true,
            death_time: 0.0,
            time: 0.0,
            id: 0,
        }
    }

    /// Add a curve to this entity.
    pub fn add_curve(&mut self, curve: EntityCurve) {
        self.curves.push(curve);
    }

    /// Update the entity for one frame.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.breath_phase += dt * self.breath_rate * std::f32::consts::TAU;

        // Apply breathing to all curves
        let breath_scale = 1.0 + self.breath_phase.sin() * self.breath_amplitude;
        for curve in &mut self.curves {
            for (i, pt) in curve.control_points.iter_mut().enumerate() {
                let base = curve.base_points[i];
                let breathed = base * breath_scale;
                // Blend toward breathed position (don't override physics)
                *pt += (breathed - *pt) * 0.1;
            }
        }

        // HP degradation: add noise to control points
        if self.hp_ratio < 1.0 {
            let noise_amp = (1.0 - self.hp_ratio) * 0.15;
            for curve in &mut self.curves {
                for (i, pt) in curve.control_points.iter_mut().enumerate() {
                    let noise_x = simple_noise(self.time * 3.0 + i as f32 * 1.618) * noise_amp;
                    let noise_y = simple_noise(self.time * 2.7 + i as f32 * 2.718) * noise_amp;
                    pt.x += noise_x;
                    pt.y += noise_y;
                }
            }
        }

        // Step physics for all curves
        for curve in &mut self.curves {
            curve.step_physics(dt, self.damping);
        }

        // Update center of mass
        self.update_center_of_mass();

        // Decay force response
        self.force_response *= 0.9;

        // Death timer
        if !self.alive {
            self.death_time += dt;
        }
    }

    /// Compute center of mass from all control points.
    pub fn update_center_of_mass(&mut self) {
        let mut sum = Vec2::ZERO;
        let mut count = 0u32;
        for curve in &self.curves {
            for pt in &curve.control_points {
                sum += *pt;
                count += 1;
            }
        }
        if count > 0 {
            self.center_of_mass = sum / count as f32;
        }
    }

    /// Apply a hit from a direction with given damage.
    pub fn apply_hit(&mut self, direction: Vec2, damage: f32) {
        self.hp_ratio = (self.hp_ratio - damage / 100.0).max(0.0);
        self.force_response += direction * damage * 0.01;
        for curve in &mut self.curves {
            curve.apply_hit_recoil(direction, damage * 0.5);
        }
        if self.hp_ratio <= 0.0 {
            self.alive = false;
        }
    }

    /// Set HP ratio and update emission accordingly.
    pub fn set_hp(&mut self, hp_ratio: f32) {
        self.hp_ratio = hp_ratio.clamp(0.0, 1.0);
        self.emission_mult = 0.3 + hp_ratio * 0.7;
        if hp_ratio <= 0.0 { self.alive = false; }
    }

    /// Increase stiffness temporarily (defend).
    pub fn brace(&mut self, multiplier: f32) {
        for curve in &mut self.curves {
            curve.stiffness *= multiplier;
        }
    }

    /// Restore original stiffness (end defend).
    pub fn unbrace(&mut self, multiplier: f32) {
        for curve in &mut self.curves {
            curve.stiffness /= multiplier;
        }
    }

    /// Break a random curve (crit hit).
    pub fn break_random_curve(&mut self, rng_seed: u32) {
        let alive_curves: Vec<usize> = self.curves.iter().enumerate()
            .filter(|(_, c)| c.alive)
            .map(|(i, _)| i)
            .collect();
        if let Some(&idx) = alive_curves.get(rng_seed as usize % alive_curves.len().max(1)) {
            self.curves[idx].break_curve();
        }
    }

    /// Trigger death dissolution.
    pub fn die(&mut self) {
        self.alive = false;
        self.death_time = 0.0;
        for curve in &mut self.curves {
            curve.stiffness = 0.0;
            // Add random outward velocity to each control point
            for (i, vel) in curve.point_velocities.iter_mut().enumerate() {
                let angle = (i as f32 * 2.399) + self.time; // golden angle spread
                *vel += Vec2::new(angle.cos(), angle.sin()) * 2.0;
            }
        }
    }

    /// Whether the entity has fully dissolved.
    pub fn is_dissolved(&self) -> bool {
        !self.alive && self.death_time > 3.0
    }

    /// Bounding box of all control points.
    pub fn bounding_box(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for curve in &self.curves {
            for pt in &curve.control_points {
                min = min.min(*pt);
                max = max.max(*pt);
            }
        }
        (min, max)
    }

    /// Number of alive curves.
    pub fn alive_curve_count(&self) -> usize {
        self.curves.iter().filter(|c| c.alive).count()
    }

    /// Total number of control points across all curves.
    pub fn total_control_points(&self) -> usize {
        self.curves.iter().map(|c| c.control_points.len()).sum()
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Simple noise (deterministic, no dependency)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn simple_noise(x: f32) -> f32 {
    let xi = x.floor() as i32;
    let xf = x - x.floor();
    let t = xf * xf * (3.0 - 2.0 * xf);
    let a = hash_f(xi);
    let b = hash_f(xi + 1);
    a + (b - a) * t
}

fn hash_f(n: i32) -> f32 {
    let n = (n as u32).wrapping_mul(0x9E3779B9);
    let n = n ^ (n >> 16);
    let n = n.wrapping_mul(0x85EBCA6B);
    (n & 0x00FF_FFFF) as f32 / 0x0080_0000 as f32 - 1.0
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curve_entity_creation() {
        let mut ent = CurveEntity::new("test", Vec3::ZERO);
        let curve = EntityCurve::new(
            CurveType::Circle { radius: 1.0, distortion: None },
            vec![Vec2::ZERO],
        );
        ent.add_curve(curve);
        assert_eq!(ent.curves.len(), 1);
        assert!(ent.alive);
    }

    #[test]
    fn test_hit_recoil() {
        let mut ent = CurveEntity::new("test", Vec3::ZERO);
        let curve = EntityCurve::new(
            CurveType::Bezier { degree: 3 },
            vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)],
        );
        ent.add_curve(curve);
        ent.apply_hit(Vec2::new(1.0, 0.0), 30.0);
        assert!(ent.hp_ratio < 1.0);
        // Some velocity should be applied
        assert!(ent.curves[0].point_velocities.iter().any(|v| v.length() > 0.0));
    }

    #[test]
    fn test_death_dissolution() {
        let mut ent = CurveEntity::new("test", Vec3::ZERO);
        ent.add_curve(EntityCurve::new(CurveType::Circle { radius: 1.0, distortion: None }, vec![Vec2::ZERO]));
        ent.die();
        assert!(!ent.alive);
        for _ in 0..200 { ent.tick(1.0 / 60.0); }
        assert!(ent.is_dissolved());
    }

    #[test]
    fn test_bounding_box() {
        let mut ent = CurveEntity::new("test", Vec3::ZERO);
        ent.add_curve(EntityCurve::new(
            CurveType::Bezier { degree: 2 },
            vec![Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0)],
        ));
        let (min, max) = ent.bounding_box();
        assert!(min.x <= -1.0 && min.y <= -1.0);
        assert!(max.x >= 1.0 && max.y >= 1.0);
    }
}
