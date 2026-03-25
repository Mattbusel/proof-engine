//! Field evaluation and sampling — evaluate the scalar field at any point, compute
//! gradient for normals, color and emission via weighted average of source contributions.

use glam::{Vec3, Vec4};
use super::entity_field::{MetaballEntity, FieldSource};

/// Complete sample of the entity's field at a point.
#[derive(Debug, Clone)]
pub struct FieldSample {
    /// Total field strength at this point.
    pub strength: f32,
    /// Weighted-average color from contributing sources.
    pub color: Vec4,
    /// Weighted-average emission intensity.
    pub emission: f32,
    /// Field gradient (unnormalized — proportional to rate of change).
    pub gradient: Vec3,
    /// Whether this point is on or above the isosurface threshold.
    pub above_threshold: bool,
    /// How far above/below the threshold (signed distance estimate).
    pub signed_distance_estimate: f32,
}

impl Default for FieldSample {
    fn default() -> Self {
        Self {
            strength: 0.0,
            color: Vec4::new(0.5, 0.5, 0.5, 1.0),
            emission: 0.0,
            gradient: Vec3::ZERO,
            above_threshold: false,
            signed_distance_estimate: 0.0,
        }
    }
}

/// Batch field evaluator — evaluates the entity's field with optimizations.
pub struct FieldEvaluator {
    /// Epsilon for gradient computation (central differences).
    pub gradient_eps: f32,
    /// Whether to compute gradient (can skip if only need strength).
    pub compute_gradient: bool,
    /// Whether to compute color/emission (can skip for pure MC classification).
    pub compute_material: bool,
    /// Minimum source contribution to consider (skip negligible sources).
    pub min_contribution: f32,
}

impl Default for FieldEvaluator {
    fn default() -> Self {
        Self {
            gradient_eps: 0.01,
            compute_gradient: true,
            compute_material: true,
            min_contribution: 1e-6,
        }
    }
}

impl FieldEvaluator {
    pub fn strength_only() -> Self {
        Self { compute_gradient: false, compute_material: false, ..Default::default() }
    }

    pub fn with_gradient() -> Self {
        Self { compute_gradient: true, compute_material: false, ..Default::default() }
    }

    pub fn full() -> Self { Self::default() }

    /// Evaluate the field at a single point.
    pub fn evaluate(&self, entity: &MetaballEntity, point: Vec3) -> FieldSample {
        let mut total_strength = 0.0f32;
        let mut weighted_color = Vec4::ZERO;
        let mut weighted_emission = 0.0f32;

        // Sum contributions from all active sources
        for source in &entity.sources {
            if !source.is_active() { continue; }

            // Quick rejection: check if point is within source's influence sphere
            let to_source = source.position - point;
            let dist_sq = to_source.length_squared();
            let max_dist = source.radius * 1.1; // small margin
            if dist_sq > max_dist * max_dist { continue; }

            let contrib = source.evaluate(point, entity.hp_ratio);
            if contrib < self.min_contribution { continue; }

            total_strength += contrib;

            if self.compute_material {
                weighted_color += source.color * contrib;
                weighted_emission += source.emission * contrib;
            }
        }

        // Normalize weighted averages
        let color = if total_strength > 1e-8 && self.compute_material {
            weighted_color / total_strength
        } else {
            Vec4::new(0.5, 0.5, 0.5, 1.0)
        };

        let emission = if total_strength > 1e-8 && self.compute_material {
            weighted_emission / total_strength
        } else {
            0.0
        };

        // Gradient via central differences
        let gradient = if self.compute_gradient {
            let eps = self.gradient_eps;
            Vec3::new(
                evaluate_strength(entity, point + Vec3::X * eps)
                    - evaluate_strength(entity, point - Vec3::X * eps),
                evaluate_strength(entity, point + Vec3::Y * eps)
                    - evaluate_strength(entity, point - Vec3::Y * eps),
                evaluate_strength(entity, point + Vec3::Z * eps)
                    - evaluate_strength(entity, point - Vec3::Z * eps),
            ) / (2.0 * eps)
        } else {
            Vec3::ZERO
        };

        let above = total_strength >= entity.threshold;
        let sde = (total_strength - entity.threshold) / gradient.length().max(1e-6);

        FieldSample {
            strength: total_strength,
            color,
            emission,
            gradient,
            above_threshold: above,
            signed_distance_estimate: sde,
        }
    }

    /// Evaluate field strength only (fast path for marching cubes classification).
    pub fn evaluate_strength_at(&self, entity: &MetaballEntity, point: Vec3) -> f32 {
        evaluate_strength(entity, point)
    }

    /// Sample the field on a regular 3D grid. Returns a flat array [z][y][x].
    pub fn sample_grid(
        &self,
        entity: &MetaballEntity,
        bounds_min: Vec3,
        bounds_max: Vec3,
        resolution: u32,
    ) -> FieldGrid {
        let res = resolution as usize;
        let total = res * res * res;
        let step = (bounds_max - bounds_min) / (resolution - 1) as f32;

        let mut strengths = Vec::with_capacity(total);
        let mut colors = if self.compute_material { Vec::with_capacity(total) } else { Vec::new() };
        let mut emissions = if self.compute_material { Vec::with_capacity(total) } else { Vec::new() };

        for z in 0..res {
            for y in 0..res {
                for x in 0..res {
                    let point = bounds_min + Vec3::new(x as f32, y as f32, z as f32) * step;

                    if self.compute_material {
                        let (s, c, e) = entity.evaluate_full(point);
                        strengths.push(s);
                        colors.push(c);
                        emissions.push(e);
                    } else {
                        strengths.push(evaluate_strength(entity, point));
                    }
                }
            }
        }

        FieldGrid {
            strengths,
            colors,
            emissions,
            resolution,
            bounds_min,
            bounds_max,
            step,
        }
    }
}

/// Sampled 3D field grid.
#[derive(Debug, Clone)]
pub struct FieldGrid {
    pub strengths: Vec<f32>,
    pub colors: Vec<Vec4>,
    pub emissions: Vec<f32>,
    pub resolution: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub step: Vec3,
}

impl FieldGrid {
    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        let r = self.resolution as usize;
        z * r * r + y * r + x
    }

    pub fn strength_at(&self, x: usize, y: usize, z: usize) -> f32 {
        self.strengths[self.index(x, y, z)]
    }

    pub fn position_at(&self, x: usize, y: usize, z: usize) -> Vec3 {
        self.bounds_min + Vec3::new(x as f32, y as f32, z as f32) * self.step
    }

    pub fn color_at(&self, x: usize, y: usize, z: usize) -> Vec4 {
        if self.colors.is_empty() { Vec4::ONE } else { self.colors[self.index(x, y, z)] }
    }

    pub fn emission_at(&self, x: usize, y: usize, z: usize) -> f32 {
        if self.emissions.is_empty() { 0.0 } else { self.emissions[self.index(x, y, z)] }
    }
}

/// Fast strength-only evaluation (no color/gradient).
fn evaluate_strength(entity: &MetaballEntity, point: Vec3) -> f32 {
    let mut total = 0.0f32;
    for source in &entity.sources {
        if !source.is_active() { continue; }
        total += source.evaluate(point, entity.hp_ratio);
    }
    total
}

// ── Damage zone modulation ──────────────────────────────────────────────────

/// Temporary damage zone that reduces field strength in an area.
#[derive(Debug, Clone)]
pub struct DamageZone {
    pub center: Vec3,
    pub radius: f32,
    pub intensity: f32,
    pub time_remaining: f32,
    pub total_duration: f32,
}

impl DamageZone {
    pub fn new(center: Vec3, radius: f32, intensity: f32, duration: f32) -> Self {
        Self { center, radius, intensity, time_remaining: duration, total_duration: duration }
    }

    /// Evaluate the strength reduction at a point (0.0 = no effect, 1.0 = full suppression).
    pub fn evaluate(&self, point: Vec3) -> f32 {
        let dist = (point - self.center).length();
        if dist >= self.radius { return 0.0; }
        let spatial = 1.0 - dist / self.radius;
        let temporal = self.time_remaining / self.total_duration;
        spatial * temporal * self.intensity
    }

    /// Update the zone (tick down timer). Returns false when expired.
    pub fn update(&mut self, dt: f32) -> bool {
        self.time_remaining -= dt;
        self.time_remaining > 0.0
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::entity_field::FieldSource;

    fn test_entity() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.5, 8);
        e.add_source(FieldSource::new(Vec3::ZERO, 1.0, 1.0));
        e
    }

    #[test]
    fn evaluator_strength_only() {
        let e = test_entity();
        let ev = FieldEvaluator::strength_only();
        let sample = ev.evaluate(&e, Vec3::ZERO);
        assert!(sample.strength > 0.0);
        assert_eq!(sample.gradient, Vec3::ZERO); // not computed
    }

    #[test]
    fn evaluator_with_gradient() {
        let e = test_entity();
        let ev = FieldEvaluator::with_gradient();
        let sample = ev.evaluate(&e, Vec3::new(0.5, 0.0, 0.0));
        assert!(sample.gradient.length() > 0.0, "Gradient should be non-zero off-center");
    }

    #[test]
    fn evaluator_full() {
        let mut e = MetaballEntity::new(0.5, 8);
        e.add_source(FieldSource::new(Vec3::ZERO, 1.0, 1.0).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)));
        let ev = FieldEvaluator::full();
        let sample = ev.evaluate(&e, Vec3::ZERO);
        assert!(sample.color.x > 0.5); // red
    }

    #[test]
    fn sample_grid_correct_size() {
        let e = test_entity();
        let ev = FieldEvaluator::strength_only();
        let grid = ev.sample_grid(&e, Vec3::splat(-2.0), Vec3::splat(2.0), 4);
        assert_eq!(grid.strengths.len(), 4 * 4 * 4);
    }

    #[test]
    fn damage_zone_decays() {
        let mut zone = DamageZone::new(Vec3::ZERO, 1.0, 1.0, 1.0);
        assert!(zone.evaluate(Vec3::ZERO) > 0.0);
        zone.update(0.5);
        let after = zone.evaluate(Vec3::ZERO);
        assert!(after < 1.0);
        zone.update(0.6);
        assert!(!zone.update(0.0)); // expired
    }

    #[test]
    fn grid_indexing() {
        let e = test_entity();
        let ev = FieldEvaluator::strength_only();
        let grid = ev.sample_grid(&e, Vec3::splat(-1.0), Vec3::splat(1.0), 4);
        let center_val = grid.strength_at(2, 2, 2);
        let corner_val = grid.strength_at(0, 0, 0);
        assert!(center_val > corner_val, "Center should be stronger than corner");
    }
}
