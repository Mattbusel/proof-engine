//! Manages the lifecycle of force fields: creation, expiry, and queries.

use crate::math::ForceField;
use crate::scene::FieldId;
use glam::Vec3;

/// A field with an optional time-to-live.
pub struct ManagedField {
    pub id: FieldId,
    pub field: ForceField,
    pub ttl: Option<f32>,    // None = permanent, Some(t) = expires after t seconds
    pub age: f32,
}

impl ManagedField {
    pub fn is_expired(&self) -> bool {
        self.ttl.map(|ttl| self.age >= ttl).unwrap_or(false)
    }
}
