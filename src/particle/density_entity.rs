//! Particle density entities — thousands of particles forming shapes through concentration.
//!
//! The entity's visible form emerges from particle density: dense regions look solid,
//! edges are wispy. Damage thins the cloud. Death scatters it.

use glam::{Vec2, Vec3, Vec4};
use std::f32::consts::TAU;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Shape field — defines the target formation shape via bones
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A bone segment that defines part of the entity's shape.
/// Particles are attracted to positions near bones proportional to bone density.
#[derive(Debug, Clone)]
pub struct ShapeBone {
    /// Start position of the bone (relative to entity center).
    pub start: Vec2,
    /// End position.
    pub end: Vec2,
    /// Radius of influence around the bone.
    pub radius: f32,
    /// Density weight (higher = more particles attracted here).
    pub density: f32,
    /// Bone name (for debugging/editor).
    pub name: String,
}

impl ShapeBone {
    pub fn new(start: Vec2, end: Vec2, radius: f32, density: f32) -> Self {
        Self { start, end, radius, density, name: String::new() }
    }

    pub fn named(mut self, name: &str) -> Self { self.name = name.to_string(); self }

    /// Length of this bone segment.
    pub fn length(&self) -> f32 { (self.end - self.start).length() }

    /// Midpoint of the bone.
    pub fn midpoint(&self) -> Vec2 { (self.start + self.end) * 0.5 }

    /// Closest point on the bone segment to a given position.
    pub fn closest_point(&self, pos: Vec2) -> Vec2 {
        let ab = self.end - self.start;
        let len_sq = ab.length_squared();
        if len_sq < 1e-6 { return self.start; }
        let t = ((pos - self.start).dot(ab) / len_sq).clamp(0.0, 1.0);
        self.start + ab * t
    }

    /// Distance from a point to the bone segment.
    pub fn distance_to(&self, pos: Vec2) -> f32 {
        (pos - self.closest_point(pos)).length()
    }

    /// Density contribution at a point (gaussian falloff from bone).
    pub fn density_at(&self, pos: Vec2, falloff: f32) -> f32 {
        let dist = self.distance_to(pos);
        if dist > self.radius * 3.0 { return 0.0; }
        self.density * (-dist * dist * falloff / (self.radius * self.radius)).exp()
    }
}

/// The shape field: collection of bones defining the entity's target form.
#[derive(Debug, Clone)]
pub struct ShapeField {
    pub bones: Vec<ShapeBone>,
    /// How quickly density falls off with distance from bones.
    pub density_falloff: f32,
    /// Total density integral (cached for normalization).
    total_density: f32,
}

impl ShapeField {
    pub fn new(bones: Vec<ShapeBone>) -> Self {
        let mut sf = Self { bones, density_falloff: 2.0, total_density: 0.0 };
        sf.recompute_total();
        sf
    }

    fn recompute_total(&mut self) {
        self.total_density = self.bones.iter().map(|b| b.density * b.length().max(0.1) * b.radius).sum();
    }

    /// Sample the combined density field at a world position.
    pub fn density_at(&self, pos: Vec2) -> f32 {
        self.bones.iter().map(|b| b.density_at(pos, self.density_falloff)).sum()
    }

    /// Generate a target position for a particle based on density-weighted random sampling.
    /// Uses the bone system: pick a bone proportional to its density*length*radius,
    /// then pick a point along the bone with gaussian offset.
    pub fn sample_target(&self, rng_seed: u32) -> Vec2 {
        if self.bones.is_empty() { return Vec2::ZERO; }

        // Pick a bone weighted by density * volume
        let mut target_weight = hash_f32(rng_seed, 0) * self.total_density;
        let mut bone_idx = 0;
        for (i, bone) in self.bones.iter().enumerate() {
            let w = bone.density * bone.length().max(0.1) * bone.radius;
            target_weight -= w;
            if target_weight <= 0.0 { bone_idx = i; break; }
        }
        let bone = &self.bones[bone_idx];

        // Pick a point along the bone
        let t = hash_f32(rng_seed, 1);
        let on_bone = bone.start + (bone.end - bone.start) * t;

        // Gaussian offset perpendicular to bone
        let bone_dir = (bone.end - bone.start).normalize_or_zero();
        let perp = Vec2::new(-bone_dir.y, bone_dir.x);
        let offset_along = (hash_f32(rng_seed, 2) - 0.5) * bone.radius * 0.5;
        let offset_perp = (hash_f32(rng_seed, 3) + hash_f32(rng_seed, 4) - 1.0) * bone.radius;

        on_bone + bone_dir * offset_along + perp * offset_perp
    }

    /// Bounding box of the shape field.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for bone in &self.bones {
            min = min.min(bone.start - Vec2::splat(bone.radius));
            min = min.min(bone.end - Vec2::splat(bone.radius));
            max = max.max(bone.start + Vec2::splat(bone.radius));
            max = max.max(bone.end + Vec2::splat(bone.radius));
        }
        (min, max)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Density particle
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single particle in the density cloud.
#[derive(Debug, Clone, Copy)]
pub struct DensityParticle {
    /// Current position (relative to entity center).
    pub position: Vec2,
    /// Velocity.
    pub velocity: Vec2,
    /// Target position from shape field.
    pub target_position: Vec2,
    /// Base target (before breathing/animation offsets).
    pub base_target: Vec2,
    /// RGBA color.
    pub color: Vec4,
    /// Emission intensity.
    pub emission: f32,
    /// Visual size (world units).
    pub size: f32,
    /// Mass (for force response).
    pub mass: f32,
    /// Whether this particle is alive (crits permanently kill particles).
    pub alive: bool,
    /// Per-particle binding strength multiplier (temporarily reduced on hit).
    pub binding_mult: f32,
    /// Timer for temporary binding reduction.
    pub binding_restore_timer: f32,
}

impl DensityParticle {
    pub fn new(target: Vec2, color: Vec4) -> Self {
        Self {
            position: target + Vec2::new(hash_f32_simple(target.x as u32) * 0.1, hash_f32_simple(target.y as u32) * 0.1),
            velocity: Vec2::ZERO,
            target_position: target,
            base_target: target,
            color,
            emission: 0.5,
            size: 0.015,
            mass: 1.0,
            alive: true,
            binding_mult: 1.0,
            binding_restore_timer: 0.0,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Density entity
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// An entity composed of thousands of particles forming a shape through density.
#[derive(Debug, Clone)]
pub struct DensityEntity {
    /// All particles in the cloud.
    pub particles: Vec<DensityParticle>,
    /// Shape definition (bones).
    pub shape_field: ShapeField,
    /// World position.
    pub position: Vec3,
    /// HP ratio (1.0 = full, 0.0 = dead).
    pub hp_ratio: f32,
    /// Global binding strength (spring constant toward target).
    pub binding_strength: f32,
    /// Base binding strength (full HP value).
    pub base_binding: f32,
    /// Maximum particle count (decreases permanently on crits).
    pub max_particles: u32,
    /// Breathing phase.
    pub breath_phase: f32,
    /// Breathing rate.
    pub breath_rate: f32,
    /// Breathing amplitude.
    pub breath_amplitude: f32,
    /// Global damping.
    pub damping: f32,
    /// Jitter strength (shimmer).
    pub jitter: f32,
    /// Base color for the entity.
    pub base_color: Vec4,
    /// Whether the entity is alive.
    pub alive: bool,
    /// Time since death.
    pub death_time: f32,
    /// Accumulated time.
    pub time: f32,
    /// Entity name.
    pub name: String,
    /// Unique ID.
    pub id: u32,
    /// Center of mass (computed).
    pub center_of_mass: Vec2,
}

impl DensityEntity {
    /// Create a new density entity from a shape field and particle count.
    pub fn new(name: &str, position: Vec3, shape: ShapeField, particle_count: u32, color: Vec4) -> Self {
        let mut particles = Vec::with_capacity(particle_count as usize);
        for i in 0..particle_count {
            let target = shape.sample_target(i * 7 + 13);
            let mut p = DensityParticle::new(target, color);
            // Vary color slightly per particle
            let hue_shift = hash_f32(i, 10) * 0.1 - 0.05;
            p.color.x = (p.color.x + hue_shift).clamp(0.0, 1.0);
            p.color.y = (p.color.y + hue_shift * 0.5).clamp(0.0, 1.0);
            // Size varies with density at target
            let density = shape.density_at(target);
            p.size = 0.01 + density * 0.005;
            p.emission = 0.3 + density * 0.4;
            particles.push(p);
        }

        Self {
            particles,
            shape_field: shape,
            position,
            hp_ratio: 1.0,
            binding_strength: 20.0,
            base_binding: 20.0,
            max_particles: particle_count,
            breath_phase: 0.0,
            breath_rate: 0.5,
            breath_amplitude: 0.02,
            damping: 0.92,
            jitter: 0.1,
            base_color: color,
            alive: true,
            death_time: 0.0,
            time: 0.0,
            name: name.to_string(),
            id: 0,
            center_of_mass: Vec2::ZERO,
        }
    }

    /// Tick the entity (physics, breathing, HP effects).
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.breath_phase += dt * self.breath_rate * TAU;

        if !self.alive { self.death_time += dt; }

        // Update binding from HP
        let hp_binding = if self.hp_ratio > 0.3 {
            self.base_binding * self.hp_ratio
        } else {
            self.base_binding * self.hp_ratio * 0.5 // extra-loose below 30%
        };
        self.binding_strength = hp_binding;

        // Jitter scales with missing HP
        let effective_jitter = self.jitter * (1.0 + (1.0 - self.hp_ratio) * 3.0);

        // Update breathing targets
        let breath_scale = 1.0 + self.breath_phase.sin() * self.breath_amplitude;

        for p in &mut self.particles {
            if !p.alive { continue; }

            // Restore binding multiplier
            if p.binding_restore_timer > 0.0 {
                p.binding_restore_timer -= dt;
                if p.binding_restore_timer <= 0.0 { p.binding_mult = 1.0; }
            }

            // Update target with breathing
            p.target_position = p.base_target * breath_scale;

            // Spring force toward target
            let to_target = p.target_position - p.position;
            let spring = to_target * self.binding_strength * p.binding_mult;

            // Damping
            let damp = -p.velocity * (1.0 - self.damping) * 60.0; // frame-rate independent

            // Jitter
            let jx = hash_f32_simple((self.time * 100.0 + p.position.x * 50.0) as u32) * effective_jitter;
            let jy = hash_f32_simple((self.time * 100.0 + p.position.y * 50.0 + 100.0) as u32) * effective_jitter;
            let jitter_force = Vec2::new(jx, jy);

            // Total acceleration
            let accel = (spring + damp + jitter_force) / p.mass;

            // Symplectic Euler
            p.velocity += accel * dt;
            p.position += p.velocity * dt;
        }

        // Update center of mass
        self.compute_center_of_mass();
    }

    /// Compute center of mass of alive particles.
    pub fn compute_center_of_mass(&mut self) {
        let mut sum = Vec2::ZERO;
        let mut count = 0u32;
        for p in &self.particles {
            if p.alive {
                sum += p.position;
                count += 1;
            }
        }
        if count > 0 { self.center_of_mass = sum / count as f32; }
    }

    /// Count alive particles.
    pub fn alive_count(&self) -> u32 {
        self.particles.iter().filter(|p| p.alive).count() as u32
    }

    /// Apply a hit: particles near impact receive impulse, binding temporarily reduced.
    pub fn apply_hit(&mut self, impact_pos: Vec2, damage: f32, radius: f32) {
        self.hp_ratio = (self.hp_ratio - damage / 100.0).max(0.0);
        if self.hp_ratio <= 0.0 { self.alive = false; }

        let impulse_strength = damage / 100.0;
        for p in &mut self.particles {
            if !p.alive { continue; }
            let to_particle = p.position - impact_pos;
            let dist = to_particle.length();
            if dist > radius { continue; }

            let falloff = 1.0 - (dist / radius);
            let impulse = to_particle.normalize_or_zero() * impulse_strength * falloff * 3.0;
            p.velocity += impulse;
            p.binding_mult = 0.3; // temporarily reduce binding
            p.binding_restore_timer = 0.5;
        }
    }

    /// Crit hit: permanently kill some particles.
    pub fn apply_crit(&mut self, impact_pos: Vec2, damage: f32) {
        self.apply_hit(impact_pos, damage, 1.5);

        // Permanently kill particles (closest to impact)
        let kill_count = (damage / 10.0) as usize;
        let mut distances: Vec<(usize, f32)> = self.particles.iter().enumerate()
            .filter(|(_, p)| p.alive)
            .map(|(i, p)| (i, (p.position - impact_pos).length()))
            .collect();
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        for i in 0..kill_count.min(distances.len()) {
            self.particles[distances[i].0].alive = false;
        }
        self.max_particles = self.alive_count();
    }

    /// Trigger death: all particles scatter.
    pub fn die(&mut self) {
        self.alive = false;
        self.death_time = 0.0;
        self.binding_strength = 0.0;

        for p in &mut self.particles {
            if !p.alive { continue; }
            let outward = (p.position - self.center_of_mass).normalize_or_zero();
            let angle = hash_f32_simple((p.position.x * 100.0) as u32) * TAU;
            let random_dir = Vec2::new(angle.cos(), angle.sin());
            p.velocity += (outward * 0.7 + random_dir * 0.3) * 3.0;
            p.binding_mult = 0.0;
        }
    }

    /// Whether the entity has fully dissolved (all emission faded).
    pub fn is_dissolved(&self) -> bool {
        !self.alive && self.death_time > 2.5
    }

    /// Bounding box of alive particles.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for p in &self.particles {
            if p.alive { min = min.min(p.position); max = max.max(p.position); }
        }
        (min, max)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Hash helpers
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn hash_f32(seed: u32, variant: usize) -> f32 {
    let n = seed.wrapping_mul(374761393).wrapping_add(variant as u32 * 668265263);
    let n = n ^ (n >> 13);
    let n = n.wrapping_mul(0x5851F42D);
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

fn hash_f32_simple(seed: u32) -> f32 {
    hash_f32(seed, 0) * 2.0 - 1.0 // [-1, 1]
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shape() -> ShapeField {
        ShapeField::new(vec![
            ShapeBone::new(Vec2::new(0.0, -0.5), Vec2::new(0.0, 0.5), 0.3, 1.0).named("torso"),
            ShapeBone::new(Vec2::new(0.0, 0.5), Vec2::new(0.0, 0.8), 0.2, 1.5).named("head"),
        ])
    }

    #[test]
    fn test_density_entity_creation() {
        let shape = test_shape();
        let ent = DensityEntity::new("test", Vec3::ZERO, shape, 500, Vec4::new(0.3, 0.5, 1.0, 0.9));
        assert_eq!(ent.particles.len(), 500);
        assert_eq!(ent.alive_count(), 500);
    }

    #[test]
    fn test_hit_reduces_hp() {
        let shape = test_shape();
        let mut ent = DensityEntity::new("test", Vec3::ZERO, shape, 100, Vec4::ONE);
        ent.apply_hit(Vec2::ZERO, 30.0, 1.0);
        assert!(ent.hp_ratio < 1.0);
    }

    #[test]
    fn test_crit_kills_particles() {
        let shape = test_shape();
        let mut ent = DensityEntity::new("test", Vec3::ZERO, shape, 100, Vec4::ONE);
        let before = ent.alive_count();
        ent.apply_crit(Vec2::ZERO, 50.0);
        assert!(ent.alive_count() < before);
    }

    #[test]
    fn test_death_and_dissolve() {
        let shape = test_shape();
        let mut ent = DensityEntity::new("test", Vec3::ZERO, shape, 50, Vec4::ONE);
        ent.die();
        assert!(!ent.alive);
        for _ in 0..200 { ent.tick(1.0 / 60.0); }
        assert!(ent.is_dissolved());
    }

    #[test]
    fn test_bone_closest_point() {
        let bone = ShapeBone::new(Vec2::ZERO, Vec2::new(2.0, 0.0), 0.5, 1.0);
        let closest = bone.closest_point(Vec2::new(1.0, 1.0));
        assert!((closest.x - 1.0).abs() < 0.01);
        assert!(closest.y.abs() < 0.01);
    }

    #[test]
    fn test_shape_sample() {
        let shape = test_shape();
        for i in 0..100 {
            let pt = shape.sample_target(i);
            assert!(pt.length() < 3.0, "sample should be near bones");
        }
    }
}
