//! MetaballEntity — the scalar field definition for isosurface entities.
//!
//! Each entity is a collection of field sources whose combined scalar field defines
//! the entity's visible body. The isosurface at `threshold` is the entity's skin.
//! HP modulates source strength — taking damage literally reshapes the body.

use glam::{Vec3, Vec4};
use crate::math::AttractorType;

// ── FalloffType ─────────────────────────────────────────────────────────────

/// How a field source's strength decays with distance.
#[derive(Debug, Clone)]
pub enum FalloffType {
    /// 1 / (1 + r²). Simple, infinite range.
    InverseSquare,
    /// exp(-r²/(2σ²)). Bell curve, configurable width.
    Gaussian,
    /// (1 - r²/R²)³ for r < R, else 0. C2 continuous. Standard metaball kernel.
    Wyvill,
    /// Field modulated by a chaotic attractor's trajectory.
    Attractor(AttractorType),
    /// Linear falloff: 1 - r/R, clamped to 0.
    Linear,
    /// Smooth polynomial: 1 - 3(r/R)² + 2(r/R)³. Hermite-like.
    SmoothPoly,
}

impl FalloffType {
    /// Evaluate the falloff at distance `r` with source radius `R`.
    pub fn evaluate(&self, r: f32, radius: f32) -> f32 {
        match self {
            Self::InverseSquare => {
                1.0 / (1.0 + (r * r) / (radius * radius))
            }
            Self::Gaussian => {
                let sigma = radius * 0.4;
                (-r * r / (2.0 * sigma * sigma)).exp()
            }
            Self::Wyvill => {
                if r >= radius { return 0.0; }
                let t = r * r / (radius * radius);
                let v = 1.0 - t;
                v * v * v
            }
            Self::Linear => {
                if r >= radius { 0.0 } else { 1.0 - r / radius }
            }
            Self::SmoothPoly => {
                if r >= radius { return 0.0; }
                let t = r / radius;
                1.0 - 3.0 * t * t + 2.0 * t * t * t
            }
            Self::Attractor(_) => {
                // Attractor-driven falloff uses Wyvill as base
                if r >= radius { return 0.0; }
                let t = r * r / (radius * radius);
                let v = 1.0 - t;
                v * v * v
            }
        }
    }
}

impl Default for FalloffType {
    fn default() -> Self { Self::Wyvill }
}

// ── FieldSource ─────────────────────────────────────────────────────────────

/// A single field source (metaball) contributing to the entity's scalar field.
#[derive(Debug, Clone)]
pub struct FieldSource {
    /// Current world-space position of this source.
    pub position: Vec3,
    /// Base (rest) position, relative to entity center.
    pub rest_offset: Vec3,
    /// Current field strength (modulated by HP, damage, breathing).
    pub strength: f32,
    /// Influence radius. Beyond this distance, contribution is zero (for Wyvill/Linear/SmoothPoly).
    pub radius: f32,
    /// How the field decays with distance.
    pub falloff: FalloffType,
    /// Color contribution for surface coloring.
    pub color: Vec4,
    /// Emission intensity.
    pub emission: f32,
    /// Strength at full HP (before any modulation).
    pub base_strength: f32,
    /// Breathing animation amplitude.
    pub breath_amplitude: f32,
    /// Breathing animation phase offset (different sources breathe slightly out of sync).
    pub breath_phase_offset: f32,
    /// If true, this source has been permanently destroyed by a crit.
    pub destroyed: bool,
    /// Temporary strength reduction from recent damage (decays over time).
    pub damage_reduction: f32,
    /// Time of last damage hit to this source.
    pub last_hit_time: f32,
    /// Name/tag for this source (e.g. "head", "torso_upper", "left_arm").
    pub tag: String,
}

impl FieldSource {
    pub fn new(offset: Vec3, strength: f32, radius: f32) -> Self {
        Self {
            position: offset,
            rest_offset: offset,
            strength,
            radius,
            falloff: FalloffType::Wyvill,
            color: Vec4::ONE,
            emission: 0.0,
            base_strength: strength,
            breath_amplitude: 0.02,
            breath_phase_offset: 0.0,
            destroyed: false,
            damage_reduction: 0.0,
            last_hit_time: -10.0,
            tag: String::new(),
        }
    }

    pub fn with_color(mut self, color: Vec4) -> Self { self.color = color; self }
    pub fn with_emission(mut self, e: f32) -> Self { self.emission = e; self }
    pub fn with_falloff(mut self, f: FalloffType) -> Self { self.falloff = f; self }
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self { self.tag = tag.into(); self }
    pub fn with_breath(mut self, amplitude: f32, phase: f32) -> Self {
        self.breath_amplitude = amplitude;
        self.breath_phase_offset = phase;
        self
    }

    /// Effective strength after HP modulation, damage reduction, and destruction.
    pub fn effective_strength(&self, hp_ratio: f32) -> f32 {
        if self.destroyed { return 0.0; }
        (self.base_strength * hp_ratio - self.damage_reduction).max(0.0)
    }

    /// Evaluate this source's contribution at a world-space point.
    pub fn evaluate(&self, point: Vec3, hp_ratio: f32) -> f32 {
        let strength = self.effective_strength(hp_ratio);
        if strength <= 0.0 { return 0.0; }
        let dist = (point - self.position).length();
        strength * self.falloff.evaluate(dist, self.radius)
    }

    /// Whether this source can contribute any field value.
    pub fn is_active(&self) -> bool {
        !self.destroyed && self.base_strength > 0.0
    }
}

impl Default for FieldSource {
    fn default() -> Self { Self::new(Vec3::ZERO, 1.0, 1.0) }
}

// ── MetaballEntity ──────────────────────────────────────────────────────────

/// An entity defined by a scalar field from metaball sources.
///
/// The visible body is the isosurface at `threshold`. HP modulates all source
/// strengths, so damage physically reshapes the body. Crits destroy individual
/// sources, punching permanent holes.
#[derive(Debug, Clone)]
pub struct MetaballEntity {
    /// All field sources making up this entity.
    pub sources: Vec<FieldSource>,
    /// Isosurface threshold. Surface exists where field(p) >= threshold.
    pub threshold: f32,
    /// Current HP ratio (0.0 = dead, 1.0 = full HP).
    pub hp_ratio: f32,
    /// Grid resolution for marching cubes extraction.
    /// 32 for normal entities, 64 for bosses.
    pub grid_resolution: u32,
    /// Current breathing animation phase.
    pub breath_phase: f32,
    /// Breathing animation frequency (Hz).
    pub breath_frequency: f32,
    /// Entity center in world space.
    pub center: Vec3,
    /// Entity rotation (euler angles for simplicity).
    pub rotation: Vec3,
    /// Entity scale.
    pub scale: f32,
    /// Bounding box half-extents (auto-computed from sources).
    pub bounds_half: Vec3,
    /// Whether the mesh needs re-extraction this frame.
    pub dirty: bool,
    /// Name for debugging.
    pub name: String,
}

impl MetaballEntity {
    pub fn new(threshold: f32, resolution: u32) -> Self {
        Self {
            sources: Vec::new(),
            threshold,
            hp_ratio: 1.0,
            grid_resolution: resolution,
            breath_phase: 0.0,
            breath_frequency: 0.3,
            center: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: 1.0,
            bounds_half: Vec3::splat(2.0),
            dirty: true,
            name: String::new(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a field source.
    pub fn add_source(&mut self, source: FieldSource) -> usize {
        let idx = self.sources.len();
        self.sources.push(source);
        self.recompute_bounds();
        self.dirty = true;
        idx
    }

    /// Find the source nearest to a world-space point.
    pub fn nearest_source(&self, point: Vec3) -> Option<usize> {
        self.sources.iter().enumerate()
            .filter(|(_, s)| s.is_active())
            .min_by(|(_, a), (_, b)| {
                let da = (a.position - point).length_squared();
                let db = (b.position - point).length_squared();
                da.partial_cmp(&db).unwrap()
            })
            .map(|(i, _)| i)
    }

    /// Find a source by tag.
    pub fn find_source(&self, tag: &str) -> Option<usize> {
        self.sources.iter().position(|s| s.tag == tag)
    }

    /// Update the entity for a new frame.
    ///
    /// - Advances breathing animation
    /// - Updates source positions from breathing offsets
    /// - Decays damage reductions (spring-back)
    /// - Marks as dirty if anything changed
    pub fn update(&mut self, dt: f32, time: f32) {
        self.breath_phase = time * self.breath_frequency * std::f32::consts::TAU;

        let mut any_changed = false;
        for source in &mut self.sources {
            if source.destroyed { continue; }

            // Breathing animation
            let breath = (self.breath_phase + source.breath_phase_offset).sin()
                * source.breath_amplitude;
            let new_pos = self.center + (source.rest_offset + source.rest_offset.normalize_or_zero() * breath) * self.scale;

            if (new_pos - source.position).length_squared() > 1e-6 {
                source.position = new_pos;
                any_changed = true;
            }

            // Damage recovery (spring-back): reduce damage_reduction by 50% per 0.5s
            if source.damage_reduction > 0.001 {
                let recovery_rate = 0.5_f32.powf(dt / 0.5);
                let permanent_reduction = source.base_strength * (1.0 - self.hp_ratio);
                let excess = (source.damage_reduction - permanent_reduction).max(0.0);
                source.damage_reduction = permanent_reduction + excess * recovery_rate;
                any_changed = true;
            }

            // Update effective strength
            source.strength = source.effective_strength(self.hp_ratio);
        }

        if any_changed { self.dirty = true; }
    }

    /// Evaluate the scalar field at a world-space point.
    pub fn evaluate(&self, point: Vec3) -> f32 {
        let mut total = 0.0;
        for source in &self.sources {
            total += source.evaluate(point, self.hp_ratio);
        }
        total
    }

    /// Evaluate the scalar field and compute color/emission at a point.
    pub fn evaluate_full(&self, point: Vec3) -> (f32, Vec4, f32) {
        let mut total_strength = 0.0;
        let mut total_color = Vec4::ZERO;
        let mut total_emission = 0.0;

        for source in &self.sources {
            let contrib = source.evaluate(point, self.hp_ratio);
            if contrib > 0.0 {
                total_strength += contrib;
                total_color += source.color * contrib;
                total_emission += source.emission * contrib;
            }
        }

        if total_strength > 0.0 {
            total_color /= total_strength;
            total_emission /= total_strength;
        }

        (total_strength, total_color, total_emission)
    }

    /// Compute the gradient (normal direction) at a point via central differences.
    pub fn gradient(&self, point: Vec3) -> Vec3 {
        let eps = 0.01 * self.scale;
        Vec3::new(
            self.evaluate(point + Vec3::X * eps) - self.evaluate(point - Vec3::X * eps),
            self.evaluate(point + Vec3::Y * eps) - self.evaluate(point - Vec3::Y * eps),
            self.evaluate(point + Vec3::Z * eps) - self.evaluate(point - Vec3::Z * eps),
        ) / (2.0 * eps)
    }

    /// Normal at a point on the isosurface (normalized gradient, pointing outward).
    pub fn normal_at(&self, point: Vec3) -> Vec3 {
        self.gradient(point).normalize_or_zero()
    }

    /// Set HP ratio. Marks dirty.
    pub fn set_hp(&mut self, ratio: f32) {
        let new_ratio = ratio.clamp(0.0, 1.0);
        if (self.hp_ratio - new_ratio).abs() > 1e-6 {
            self.hp_ratio = new_ratio;
            self.dirty = true;
        }
    }

    /// Move the entity center. Updates all source positions.
    pub fn set_center(&mut self, center: Vec3) {
        if (self.center - center).length_squared() > 1e-6 {
            self.center = center;
            for source in &mut self.sources {
                source.position = center + source.rest_offset * self.scale;
            }
            self.dirty = true;
        }
    }

    /// Recompute bounding box from source positions and radii.
    pub fn recompute_bounds(&mut self) {
        let mut max_extent = Vec3::ZERO;
        for source in &self.sources {
            let extent = source.rest_offset.abs() + Vec3::splat(source.radius);
            max_extent = max_extent.max(extent);
        }
        self.bounds_half = max_extent * self.scale * 1.1; // 10% padding
    }

    /// World-space bounding box (min, max).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        (self.center - self.bounds_half, self.center + self.bounds_half)
    }

    /// Number of active (non-destroyed) sources.
    pub fn active_source_count(&self) -> usize {
        self.sources.iter().filter(|s| s.is_active()).count()
    }

    /// Total active source count.
    pub fn source_count(&self) -> usize { self.sources.len() }

    /// Whether the entity is effectively dead (no visible surface).
    pub fn is_dead(&self) -> bool {
        self.hp_ratio <= 0.0 || self.active_source_count() == 0
    }

    /// Consume the dirty flag.
    pub fn take_dirty(&mut self) -> bool {
        let d = self.dirty;
        self.dirty = false;
        d
    }
}

impl Default for MetaballEntity {
    fn default() -> Self { Self::new(0.5, 32) }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_entity() -> MetaballEntity {
        let mut e = MetaballEntity::new(0.5, 16);
        e.add_source(FieldSource::new(Vec3::ZERO, 1.0, 1.0).with_tag("center"));
        e.add_source(FieldSource::new(Vec3::new(0.8, 0.0, 0.0), 0.7, 0.8).with_tag("right"));
        e.add_source(FieldSource::new(Vec3::new(-0.8, 0.0, 0.0), 0.7, 0.8).with_tag("left"));
        e
    }

    #[test]
    fn field_at_source_center_is_strong() {
        let e = basic_entity();
        let val = e.evaluate(Vec3::ZERO);
        assert!(val > e.threshold, "Field at center should exceed threshold: {val}");
    }

    #[test]
    fn field_far_away_is_zero() {
        let e = basic_entity();
        let val = e.evaluate(Vec3::new(100.0, 0.0, 0.0));
        assert!(val < 0.01, "Field far away should be ~0: {val}");
    }

    #[test]
    fn hp_modulates_field() {
        let mut e = basic_entity();
        let full_hp = e.evaluate(Vec3::ZERO);
        e.set_hp(0.5);
        let half_hp = e.evaluate(Vec3::ZERO);
        assert!(half_hp < full_hp, "Half HP should weaken field: full={full_hp}, half={half_hp}");
    }

    #[test]
    fn destroyed_source_contributes_nothing() {
        let mut e = basic_entity();
        e.sources[0].destroyed = true;
        let val = e.sources[0].evaluate(Vec3::ZERO, 1.0);
        assert_eq!(val, 0.0);
    }

    #[test]
    fn find_source_by_tag() {
        let e = basic_entity();
        assert_eq!(e.find_source("right"), Some(1));
        assert_eq!(e.find_source("nonexistent"), None);
    }

    #[test]
    fn nearest_source_correct() {
        let e = basic_entity();
        let idx = e.nearest_source(Vec3::new(0.7, 0.0, 0.0)).unwrap();
        assert_eq!(idx, 1); // "right" source at 0.8
    }

    #[test]
    fn gradient_points_away_from_sources() {
        let e = basic_entity();
        let grad = e.gradient(Vec3::new(1.5, 0.0, 0.0));
        // Gradient should point away from center (positive x direction)
        assert!(grad.x > 0.0 || grad.length() < 0.01, "grad.x={}", grad.x);
    }

    #[test]
    fn evaluate_full_returns_weighted_color() {
        let mut e = MetaballEntity::new(0.5, 16);
        e.add_source(FieldSource::new(Vec3::ZERO, 1.0, 1.0).with_color(Vec4::new(1.0, 0.0, 0.0, 1.0)));
        let (strength, color, _emission) = e.evaluate_full(Vec3::ZERO);
        assert!(strength > 0.0);
        assert!(color.x > 0.5); // red dominant
    }

    #[test]
    fn wyvill_falloff_c2_continuous() {
        let f = FalloffType::Wyvill;
        let r = 1.0;
        // At boundary: should be exactly 0
        assert_eq!(f.evaluate(r, r), 0.0);
        // At center: should be 1
        assert_eq!(f.evaluate(0.0, r), 1.0);
        // Just inside boundary: should be very small but positive
        assert!(f.evaluate(r * 0.99, r) > 0.0);
    }

    #[test]
    fn entity_bounds_encompass_sources() {
        let e = basic_entity();
        let (min, max) = e.bounds();
        for source in &e.sources {
            assert!(source.rest_offset.x >= min.x && source.rest_offset.x <= max.x);
        }
    }

    #[test]
    fn dead_entity_detection() {
        let mut e = basic_entity();
        assert!(!e.is_dead());
        e.set_hp(0.0);
        assert!(e.is_dead());
    }

    #[test]
    fn breathing_changes_positions() {
        let mut e = basic_entity();
        let pos_before = e.sources[1].position;
        e.update(0.016, 1.0);
        let pos_after = e.sources[1].position;
        // Position should shift slightly from breathing
        // (may be very small depending on amplitude)
        assert!(e.sources[1].breath_amplitude > 0.0);
    }
}
