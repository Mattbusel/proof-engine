//! Force field lifecycle manager — creation, TTL expiry, spatial queries, composition.
//!
//! The FieldManager owns all active force fields in a scene. Fields can be permanent
//! or time-limited (TTL). Every frame, expired fields are pruned and the manager
//! provides efficient queries for field force at a given world position.

use glam::Vec3;
use crate::math::ForceField;
use crate::scene::FieldId;

// ── Managed field ─────────────────────────────────────────────────────────────

/// A force field with lifecycle metadata.
pub struct ManagedField {
    pub id: FieldId,
    pub field: ForceField,
    /// None = permanent, Some(seconds) = expires after this duration.
    pub ttl: Option<f32>,
    /// Seconds this field has been alive.
    pub age: f32,
    /// Tags for grouping/query filtering.
    pub tags: Vec<String>,
    /// Strength multiplier — can be faded over time.
    pub strength_scale: f32,
    /// Fade-in duration (ramps strength from 0 to 1 over this time).
    pub fade_in: f32,
    /// Fade-out duration (ramps strength from 1 to 0 before expiry).
    pub fade_out: f32,
}

impl ManagedField {
    /// Whether this field has exceeded its TTL.
    pub fn is_expired(&self) -> bool {
        self.ttl.map(|ttl| self.age >= ttl).unwrap_or(false)
    }

    /// Effective strength multiplier accounting for fade-in and fade-out.
    pub fn effective_scale(&self) -> f32 {
        let base = self.strength_scale;
        // Fade in
        let fade_in_factor = if self.fade_in > 0.0 {
            (self.age / self.fade_in).min(1.0)
        } else {
            1.0
        };
        // Fade out
        let fade_out_factor = if let Some(ttl) = self.ttl {
            if self.fade_out > 0.0 {
                let remaining = ttl - self.age;
                (remaining / self.fade_out).clamp(0.0, 1.0)
            } else {
                1.0
            }
        } else {
            1.0
        };
        base * fade_in_factor * fade_out_factor
    }

    /// Returns whether this field has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

// ── Field query result ────────────────────────────────────────────────────────

/// The aggregated effect of all fields at a position.
#[derive(Debug, Clone)]
pub struct FieldSample {
    /// Total force vector to apply.
    pub force: Vec3,
    /// Total temperature contribution (from HeatSources).
    pub temperature: f32,
    /// Total entropy contribution.
    pub entropy: f32,
    /// Number of fields that contributed.
    pub field_count: usize,
}

impl Default for FieldSample {
    fn default() -> Self {
        Self { force: Vec3::ZERO, temperature: 0.0, entropy: 0.0, field_count: 0 }
    }
}

// ── Field composition ─────────────────────────────────────────────────────────

/// How multiple fields combine their forces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldBlend {
    /// Sum all field forces (default).
    Additive,
    /// Average field forces.
    Average,
    /// Only the strongest field contributes.
    Dominant,
    /// Max force per component.
    ComponentMax,
}

// ── Spatial cell for broad-phase culling ─────────────────────────────────────

const CELL_SIZE: f32 = 10.0;

fn world_to_cell(pos: Vec3) -> (i32, i32, i32) {
    (
        (pos.x / CELL_SIZE).floor() as i32,
        (pos.y / CELL_SIZE).floor() as i32,
        (pos.z / CELL_SIZE).floor() as i32,
    )
}

// ── Field manager ─────────────────────────────────────────────────────────────

/// Manages all active force fields in a scene.
pub struct FieldManager {
    fields: Vec<ManagedField>,
    next_id: u32,
    pub blend_mode: FieldBlend,
    /// Global force multiplier applied to all fields.
    pub global_scale: f32,
}

impl FieldManager {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            next_id: 1,
            blend_mode: FieldBlend::Additive,
            global_scale: 1.0,
        }
    }

    // ── Field CRUD ────────────────────────────────────────────────────────────

    /// Add a permanent force field. Returns its ID.
    pub fn add(&mut self, field: ForceField) -> FieldId {
        self.add_full(field, None, vec![], 1.0, 0.0, 0.0)
    }

    /// Add a field with a time-to-live in seconds. Returns its ID.
    pub fn add_timed(&mut self, field: ForceField, ttl: f32) -> FieldId {
        self.add_full(field, Some(ttl), vec![], 1.0, 0.0, 0.0)
    }

    /// Add a field with a TTL and optional fade in/out durations.
    pub fn add_faded(&mut self, field: ForceField, ttl: f32, fade_in: f32, fade_out: f32) -> FieldId {
        self.add_full(field, Some(ttl), vec![], 1.0, fade_in, fade_out)
    }

    /// Add a tagged field.
    pub fn add_tagged(&mut self, field: ForceField, ttl: Option<f32>, tags: Vec<String>) -> FieldId {
        self.add_full(field, ttl, tags, 1.0, 0.0, 0.0)
    }

    fn add_full(
        &mut self,
        field: ForceField,
        ttl: Option<f32>,
        tags: Vec<String>,
        strength_scale: f32,
        fade_in: f32,
        fade_out: f32,
    ) -> FieldId {
        let id = FieldId(self.next_id);
        self.next_id += 1;
        self.fields.push(ManagedField {
            id,
            field,
            ttl,
            age: 0.0,
            tags,
            strength_scale,
            fade_in,
            fade_out,
        });
        id
    }

    /// Remove a field by ID. Returns true if removed.
    pub fn remove(&mut self, id: FieldId) -> bool {
        let before = self.fields.len();
        self.fields.retain(|f| f.id != id);
        self.fields.len() < before
    }

    /// Remove all fields with a specific tag.
    pub fn remove_tagged(&mut self, tag: &str) {
        self.fields.retain(|f| !f.has_tag(tag));
    }

    /// Remove all fields.
    pub fn clear(&mut self) {
        self.fields.clear();
    }

    /// Get a reference to a field by ID.
    pub fn get(&self, id: FieldId) -> Option<&ManagedField> {
        self.fields.iter().find(|f| f.id == id)
    }

    /// Get a mutable reference to a field by ID.
    pub fn get_mut(&mut self, id: FieldId) -> Option<&mut ManagedField> {
        self.fields.iter_mut().find(|f| f.id == id)
    }

    // ── Tick & expiry ─────────────────────────────────────────────────────────

    /// Advance all fields by dt seconds and prune expired ones.
    pub fn tick(&mut self, dt: f32) {
        for f in &mut self.fields {
            f.age += dt;
        }
        self.fields.retain(|f| !f.is_expired());
    }

    // ── Spatial queries ───────────────────────────────────────────────────────

    /// Sample all field effects at a world position.
    pub fn sample(&self, pos: Vec3, mass: f32, charge: f32, t: f32) -> FieldSample {
        let mut sample = FieldSample::default();
        let mut forces: Vec<Vec3> = Vec::new();

        for mf in &self.fields {
            let scale = mf.effective_scale() * self.global_scale;
            if scale == 0.0 { continue; }

            let force = mf.field.force_at(pos, mass, charge, t) * scale;
            let temp  = mf.field.temperature_at(pos) * scale;
            let entr  = mf.field.entropy_at(pos) * scale;

            sample.temperature += temp;
            sample.entropy += entr;
            sample.field_count += 1;
            forces.push(force);
        }

        sample.force = combine_forces(&forces, self.blend_mode);
        sample
    }

    /// Sample only fields with a specific tag.
    pub fn sample_tagged(&self, pos: Vec3, tag: &str, mass: f32, charge: f32, t: f32) -> FieldSample {
        let mut sample = FieldSample::default();
        let mut forces: Vec<Vec3> = Vec::new();

        for mf in self.fields.iter().filter(|f| f.has_tag(tag)) {
            let scale = mf.effective_scale() * self.global_scale;
            if scale == 0.0 { continue; }

            let force = mf.field.force_at(pos, mass, charge, t) * scale;
            sample.temperature += mf.field.temperature_at(pos) * scale;
            sample.entropy += mf.field.entropy_at(pos) * scale;
            sample.field_count += 1;
            forces.push(force);
        }

        sample.force = combine_forces(&forces, self.blend_mode);
        sample
    }

    /// Returns the pure force vector at a position (no temperature/entropy).
    pub fn force_at(&self, pos: Vec3, mass: f32, charge: f32, t: f32) -> Vec3 {
        self.sample(pos, mass, charge, t).force
    }

    /// Returns all field IDs whose bounding region overlaps `pos` within `radius`.
    pub fn fields_near(&self, pos: Vec3, radius: f32) -> Vec<FieldId> {
        self.fields.iter()
            .filter(|mf| field_center(&mf.field)
                .map(|c| (c - pos).length() < radius)
                .unwrap_or(true))
            .map(|mf| mf.id)
            .collect()
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Number of active fields.
    pub fn len(&self) -> usize { self.fields.len() }

    /// True if no fields are active.
    pub fn is_empty(&self) -> bool { self.fields.is_empty() }

    /// Iterator over all active managed fields.
    pub fn iter(&self) -> impl Iterator<Item = &ManagedField> {
        self.fields.iter()
    }

    // ── Field interference ────────────────────────────────────────────────────

    /// Compute field interference pattern at pos — how fields amplify/cancel each other.
    /// Returns a scalar in [-1, 1] representing constructive (+) or destructive (-) interference.
    pub fn interference_at(&self, pos: Vec3, t: f32) -> f32 {
        if self.fields.len() < 2 { return 0.0; }
        let forces: Vec<Vec3> = self.fields.iter()
            .map(|mf| mf.field.force_at(pos, 1.0, 0.0, t) * mf.effective_scale())
            .collect();
        if forces.is_empty() { return 0.0; }
        let sum: Vec3 = forces.iter().copied().sum();
        let mag_sum: f32 = forces.iter().map(|f| f.length()).sum();
        if mag_sum < 0.001 { return 0.0; }
        // Constructive if aligned (magnitude of sum ≈ sum of magnitudes)
        // Destructive if opposing (magnitude of sum ≈ 0)
        (sum.length() / mag_sum) * 2.0 - 1.0
    }

    /// Resonance score at pos — how much fields oscillate in phase.
    /// Uses the variance of field force magnitudes.
    pub fn resonance_at(&self, pos: Vec3, t: f32) -> f32 {
        let mags: Vec<f32> = self.fields.iter()
            .map(|mf| mf.field.force_at(pos, 1.0, 0.0, t).length() * mf.effective_scale())
            .collect();
        if mags.is_empty() { return 0.0; }
        let mean = mags.iter().sum::<f32>() / mags.len() as f32;
        let var  = mags.iter().map(|m| (m - mean).powi(2)).sum::<f32>() / mags.len() as f32;
        1.0 / (1.0 + var)  // high resonance = low variance
    }
}

impl Default for FieldManager {
    fn default() -> Self { Self::new() }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn combine_forces(forces: &[Vec3], blend: FieldBlend) -> Vec3 {
    if forces.is_empty() { return Vec3::ZERO; }
    match blend {
        FieldBlend::Additive => forces.iter().copied().sum(),
        FieldBlend::Average  => forces.iter().copied().sum::<Vec3>() / forces.len() as f32,
        FieldBlend::Dominant => forces.iter().copied()
            .max_by(|a, b| a.length_squared().partial_cmp(&b.length_squared()).unwrap())
            .unwrap_or(Vec3::ZERO),
        FieldBlend::ComponentMax => forces.iter().copied().fold(Vec3::ZERO, |acc, f| {
            Vec3::new(
                if f.x.abs() > acc.x.abs() { f.x } else { acc.x },
                if f.y.abs() > acc.y.abs() { f.y } else { acc.y },
                if f.z.abs() > acc.z.abs() { f.z } else { acc.z },
            )
        }),
    }
}

/// Extract the logical center of a field (if it has one).
fn field_center(field: &ForceField) -> Option<Vec3> {
    use crate::math::ForceField as FF;
    match field {
        FF::Gravity { center, .. }        => Some(*center),
        FF::Vortex  { center, .. }        => Some(*center),
        FF::Repulsion { center, .. }      => Some(*center),
        FF::Electromagnetic { center, .. }=> Some(*center),
        FF::HeatSource { center, .. }     => Some(*center),
        FF::MathField { center, .. }      => Some(*center),
        FF::StrangeAttractor { center, .. }=> Some(*center),
        FF::EntropyField { center, .. }   => Some(*center),
        FF::Damping { center, .. }        => Some(*center),
        FF::Flow { .. }          => None,
        FF::Pulsing { center, .. }       => Some(*center),
        FF::Shockwave { center, .. }     => Some(*center),
        FF::Warp { center, .. }          => Some(*center),
        FF::Tidal { center, .. }         => Some(*center),
        FF::MagneticDipole { center, .. }=> Some(*center),
        FF::Saddle { center, .. }        => Some(*center),
        FF::Wind { .. }                  => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::fields::Falloff;

    #[test]
    fn add_and_remove() {
        let mut mgr = FieldManager::new();
        let id = mgr.add(ForceField::Gravity {
            center: Vec3::ZERO, strength: 1.0, falloff: Falloff::InverseSquare,
        });
        assert_eq!(mgr.len(), 1);
        assert!(mgr.remove(id));
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn timed_field_expires() {
        let mut mgr = FieldManager::new();
        mgr.add_timed(ForceField::Flow {
            direction: Vec3::X, strength: 1.0, turbulence: 0.0,
        }, 0.5);
        mgr.tick(0.3);
        assert_eq!(mgr.len(), 1);
        mgr.tick(0.3);
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn sample_returns_force() {
        let mut mgr = FieldManager::new();
        mgr.add(ForceField::Flow { direction: Vec3::X, strength: 2.0, turbulence: 0.0 });
        let sample = mgr.sample(Vec3::ZERO, 1.0, 0.0, 0.0);
        assert!(sample.force.x > 0.0);
    }

    #[test]
    fn fade_in_scales_force() {
        let mut mgr = FieldManager::new();
        mgr.add_faded(
            ForceField::Flow { direction: Vec3::X, strength: 2.0, turbulence: 0.0 },
            2.0, 1.0, 0.0,
        );
        // At t=0 the field just spawned; effective scale should be near 0
        let sample_early = mgr.sample(Vec3::ZERO, 1.0, 0.0, 0.0);
        mgr.tick(1.0);
        let sample_late = mgr.sample(Vec3::ZERO, 1.0, 0.0, 1.0);
        assert!(sample_late.force.x > sample_early.force.x);
    }
}
