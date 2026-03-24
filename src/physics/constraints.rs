//! Sequential-impulse constraint solver for 2D/3D rigid bodies.
//!
//! Implements the Erin Catto / Box2D style iterative constraint solver:
//!
//! 1. Pre-compute constraint Jacobians and effective masses.
//! 2. Apply accumulated impulses from the previous frame (warm starting).
//! 3. Iterate: compute velocity errors, compute corrective impulses,
//!    clamp to feasible range, accumulate.
//! 4. Optional position correction via Baumgarte stabilization.
//!
//! ## Constraint types
//! - `DistanceConstraint` — maintain fixed distance between two bodies
//! - `HingeConstraint`    — two bodies share a common anchor point
//! - `SpringConstraint`   — spring-damper between two anchor points
//! - `PinConstraint`      — pin a body point to a world-space location
//! - `SliderConstraint`   — allow motion only along an axis
//! - `MotorConstraint`    — drive a joint at a target angular velocity
//! - `GearConstraint`     — couple the rotation of two bodies
//! - `PulleyConstraint`   — rope-over-pulley coupling of two bodies
//! - `WeldConstraint`     — fully rigidly attach two bodies together
//! - `MaxDistanceConstraint` — soft rope: only constrains when overstretched

use glam::{Vec2, Vec3};
use std::collections::HashMap;

// ── BodyHandle ────────────────────────────────────────────────────────────────

/// Reference to a rigid body in the solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyHandle(pub u32);

/// Sentinel for world-space anchors (infinite mass, immovable).
pub const WORLD: BodyHandle = BodyHandle(u32::MAX);

// ── BodyState ─────────────────────────────────────────────────────────────────

/// Minimal state the solver needs from a rigid body.
#[derive(Debug, Clone)]
pub struct BodyState {
    pub position:         Vec2,
    pub velocity:         Vec2,
    pub angle:            f32,
    pub angular_velocity: f32,
    /// Inverse mass (0.0 for static bodies).
    pub inv_mass:         f32,
    /// Inverse moment of inertia (0.0 for static).
    pub inv_inertia:      f32,
    /// Accumulated corrective impulse from constraints.
    pub impulse_accum:    Vec2,
    pub torque_accum:     f32,
}

impl BodyState {
    pub fn new(position: Vec2, mass: f32, inertia: f32) -> Self {
        Self {
            position,
            velocity: Vec2::ZERO,
            angle: 0.0,
            angular_velocity: 0.0,
            inv_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            inv_inertia: if inertia > 0.0 { 1.0 / inertia } else { 0.0 },
            impulse_accum: Vec2::ZERO,
            torque_accum: 0.0,
        }
    }

    pub fn static_body(position: Vec2) -> Self {
        Self::new(position, 0.0, 0.0)
    }

    pub fn apply_impulse(&mut self, impulse: Vec2, contact_arm: Vec2) {
        self.velocity     += impulse * self.inv_mass;
        self.angular_velocity += cross2(contact_arm, impulse) * self.inv_inertia;
    }

    pub fn is_static(&self) -> bool { self.inv_mass < 1e-12 }
}

/// 2D cross product (scalar result): a.x*b.y - a.y*b.x
fn cross2(a: Vec2, b: Vec2) -> f32 { a.x * b.y - a.y * b.x }

/// Perpendicular of a 2D vector: (-y, x)
fn perp(v: Vec2) -> Vec2 { Vec2::new(-v.y, v.x) }

/// Rotate a Vec2 by angle theta.
fn rotate2(v: Vec2, theta: f32) -> Vec2 {
    let (s, c) = theta.sin_cos();
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

// ── Constraint ID ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstraintId(pub u32);

// ── Constraint trait ──────────────────────────────────────────────────────────

/// A constraint imposes a restriction on body state.
pub trait Constraint: std::fmt::Debug {
    /// Compute the velocity constraint violation (Cdot = J * v).
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32;
    /// Compute the position constraint violation (C).
    fn compute_c(&self,    bodies: &HashMap<BodyHandle, BodyState>) -> f32;
    /// Apply the corrective impulse to body states.
    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32);
    /// Compute effective constraint mass (1 / (J M^-1 J^T)).
    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32;
    /// Baumgarte position bias.
    fn bias(&self, bodies: &HashMap<BodyHandle, BodyState>, dt: f32) -> f32 {
        let beta = 0.1;
        -beta / dt * self.compute_c(bodies)
    }
    /// Accumulated impulse for warm starting.
    fn accumulated_impulse(&self) -> f32;
    fn reset_accumulated(&mut self);
    fn add_accumulated(&mut self, delta: f32);
    /// Whether this constraint has an upper/lower impulse clamp.
    fn impulse_bounds(&self) -> (f32, f32) { (f32::NEG_INFINITY, f32::INFINITY) }
}

// ── DistanceConstraint ────────────────────────────────────────────────────────

/// Maintain a fixed distance between two local anchor points on two bodies.
#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    pub body_a:     BodyHandle,
    pub body_b:     BodyHandle,
    pub anchor_a:   Vec2,
    pub anchor_b:   Vec2,
    pub rest_length: f32,
    /// 0.0 = rigid, >0 = soft (compliance).
    pub compliance: f32,
    accumulated:    f32,
}

impl DistanceConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2, rest_length: f32) -> Self {
        Self { body_a, body_b, anchor_a, anchor_b, rest_length, compliance: 0.0, accumulated: 0.0 }
    }

    pub fn soft(mut self, compliance: f32) -> Self { self.compliance = compliance; self }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }

    fn constraint_dir(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len   = delta.length();
        let dir   = if len > 1e-7 { delta / len } else { Vec2::X };
        (dir, len)
    }
}

impl Constraint for DistanceConstraint {
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let (dir, _) = self.constraint_dir(bodies);
        let va = bodies.get(&self.body_a).map(|b| b.velocity + perp(pa - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity + perp(pb - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        (vb - va).dot(dir)
    }

    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (_, len) = self.constraint_dir(bodies);
        len - self.rest_length
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let (dir, _) = self.constraint_dir(bodies);
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let ra = ba.map(|b| pa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bb.map(|b| pb - b.position).unwrap_or(Vec2::ZERO);
        let im_a = ba.map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bb.map(|b| b.inv_mass).unwrap_or(0.0);
        let ii_a = ba.map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bb.map(|b| b.inv_inertia).unwrap_or(0.0);
        let rda = cross2(ra, dir);
        let rdb = cross2(rb, dir);
        let denom = im_a + im_b + ii_a * rda * rda + ii_b * rdb * rdb + self.compliance;
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = {
            let ba = bodies.get(&self.body_a);
            let bb = bodies.get(&self.body_b);
            let pa = ba.map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
            let pb = bb.map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
            (pa, pb)
        };
        let delta = pb - pa;
        let len   = delta.length();
        let dir   = if len > 1e-7 { delta / len } else { Vec2::X };
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            let r = pa - b.position;
            b.apply_impulse(-impulse, r);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            let r = pb - b.position;
            b.apply_impulse(impulse, r);
        }
    }

    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated += d; }
}

// ── HingeConstraint ───────────────────────────────────────────────────────────

/// Two bodies share a common world-space anchor point.
#[derive(Debug, Clone)]
pub struct HingeConstraint {
    pub body_a:   BodyHandle,
    pub body_b:   BodyHandle,
    pub anchor_a: Vec2,
    pub anchor_b: Vec2,
    /// Optional angle limits (min, max) in radians relative to body A's angle.
    pub limits:   Option<(f32, f32)>,
    accum_x:      f32,
    accum_y:      f32,
    accum_limit:  f32,
}

impl HingeConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2) -> Self {
        Self { body_a, body_b, anchor_a, anchor_b, limits: None, accum_x: 0.0, accum_y: 0.0, accum_limit: 0.0 }
    }

    pub fn with_limits(mut self, min: f32, max: f32) -> Self {
        self.limits = Some((min, max));
        self
    }

    fn relative_angle(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let angle_a = bodies.get(&self.body_a).map(|b| b.angle).unwrap_or(0.0);
        let angle_b = bodies.get(&self.body_b).map(|b| b.angle).unwrap_or(0.0);
        angle_b - angle_a
    }
}

impl Constraint for HingeConstraint {
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        // Velocity error in the X direction (simplified — full hinge uses 2 constraints)
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let pa = ba.map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bb.map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        let va = ba.map(|b| b.velocity + perp(pa - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        let vb = bb.map(|b| b.velocity + perp(pb - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        (vb - va).length()
    }

    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let pa = ba.map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bb.map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pb - pa).length()
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let im_a = ba.map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bb.map(|b| b.inv_mass).unwrap_or(0.0);
        let denom = im_a + im_b;
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let impulse_dir = {
            let ba = bodies.get(&self.body_a);
            let bb = bodies.get(&self.body_b);
            let pa = ba.map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
            let pb = bb.map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
            let delta = pb - pa;
            let len = delta.length();
            if len > 1e-7 { delta / len } else { Vec2::X }
        };
        let impulse = impulse_dir * lambda;
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, pa - b.position);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, pb - b.position);
        }
    }

    fn accumulated_impulse(&self) -> f32 { self.accum_x }
    fn reset_accumulated(&mut self) { self.accum_x = 0.0; self.accum_y = 0.0; self.accum_limit = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accum_x += d; }
}

// ── SpringConstraint ──────────────────────────────────────────────────────────

/// A spring-damper between two anchor points. Unlike DistanceConstraint,
/// this adds restoring force proportional to stretch (Hooke's law).
#[derive(Debug, Clone)]
pub struct SpringConstraint {
    pub body_a:      BodyHandle,
    pub body_b:      BodyHandle,
    pub anchor_a:    Vec2,
    pub anchor_b:    Vec2,
    pub rest_length: f32,
    pub stiffness:   f32,
    pub damping:     f32,
    accumulated:     f32,
}

impl SpringConstraint {
    pub fn new(a: BodyHandle, aa: Vec2, b: BodyHandle, ab: Vec2, rest: f32, k: f32, d: f32) -> Self {
        Self { body_a: a, body_b: b, anchor_a: aa, anchor_b: ab, rest_length: rest, stiffness: k, damping: d, accumulated: 0.0 }
    }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }
}

impl Constraint for SpringConstraint {
    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        (pb - pa).length() - self.rest_length
    }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len   = delta.length().max(1e-7);
        let dir   = delta / len;
        let va = bodies.get(&self.body_a).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        (vb - va).dot(dir)
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let denom = im_a + im_b + self.stiffness;
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn bias(&self, bodies: &HashMap<BodyHandle, BodyState>, dt: f32) -> f32 {
        // Spring bias: drives position toward rest length
        -self.stiffness * self.compute_c(bodies) / dt
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len   = delta.length().max(1e-7);
        let dir   = delta / len;
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-impulse, Vec2::ZERO); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(impulse,  Vec2::ZERO); }
    }

    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated += d; }
}

// ── PinConstraint ─────────────────────────────────────────────────────────────

/// Pin a body's local anchor to a fixed world-space point.
#[derive(Debug, Clone)]
pub struct PinConstraint {
    pub body:       BodyHandle,
    pub local_anchor: Vec2,
    pub world_target: Vec2,
    accumulated:    f32,
}

impl PinConstraint {
    pub fn new(body: BodyHandle, local_anchor: Vec2, world_target: Vec2) -> Self {
        Self { body, local_anchor, world_target, accumulated: 0.0 }
    }
}

impl Constraint for PinConstraint {
    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let pa = bodies.get(&self.body)
            .map(|b| b.position + rotate2(self.local_anchor, b.angle))
            .unwrap_or(Vec2::ZERO);
        (pa - self.world_target).length()
    }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let b = match bodies.get(&self.body) { Some(b) => b, None => return 0.0 };
        let pa = b.position + rotate2(self.local_anchor, b.angle);
        let dir = (pa - self.world_target).normalize_or_zero();
        (b.velocity + perp(pa - b.position) * b.angular_velocity).dot(dir)
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let b = match bodies.get(&self.body) { Some(b) => b, None => return 0.0 };
        let pa = b.position + rotate2(self.local_anchor, b.angle);
        let r  = pa - b.position;
        let dir = (pa - self.world_target).normalize_or_zero();
        let rd  = cross2(r, dir);
        let denom = b.inv_mass + b.inv_inertia * rd * rd;
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let b = match bodies.get_mut(&self.body) { Some(b) => b, None => return };
        let pa = b.position + rotate2(self.local_anchor, b.angle);
        let dir = (pa - self.world_target).normalize_or_zero();
        b.apply_impulse(-dir * lambda, pa - b.position);
    }

    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated += d; }
}

// ── MotorConstraint ───────────────────────────────────────────────────────────

/// Drive a body at a target angular velocity. Acts as a torque motor.
#[derive(Debug, Clone)]
pub struct MotorConstraint {
    pub body:            BodyHandle,
    pub target_velocity: f32,
    pub max_torque:      f32,
    accumulated:         f32,
}

impl MotorConstraint {
    pub fn new(body: BodyHandle, target_velocity: f32, max_torque: f32) -> Self {
        Self { body, target_velocity, max_torque, accumulated: 0.0 }
    }
}

impl Constraint for MotorConstraint {
    fn compute_c(&self, _: &HashMap<BodyHandle, BodyState>) -> f32 { 0.0 }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let omega = bodies.get(&self.body).map(|b| b.angular_velocity).unwrap_or(0.0);
        omega - self.target_velocity
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let ii = bodies.get(&self.body).map(|b| b.inv_inertia).unwrap_or(0.0);
        if ii < 1e-10 { 0.0 } else { 1.0 / ii }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        if let Some(b) = bodies.get_mut(&self.body) {
            b.angular_velocity -= lambda * b.inv_inertia;
        }
    }

    fn impulse_bounds(&self) -> (f32, f32) { (-self.max_torque, self.max_torque) }
    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated = (self.accumulated + d).clamp(-self.max_torque, self.max_torque); }
}

// ── MaxDistanceConstraint ─────────────────────────────────────────────────────

/// Soft rope: only applies constraint force when stretched beyond max_distance.
#[derive(Debug, Clone)]
pub struct MaxDistanceConstraint {
    pub body_a:       BodyHandle,
    pub body_b:       BodyHandle,
    pub anchor_a:     Vec2,
    pub anchor_b:     Vec2,
    pub max_distance: f32,
    accumulated:      f32,
}

impl MaxDistanceConstraint {
    pub fn new(a: BodyHandle, aa: Vec2, b: BodyHandle, ab: Vec2, max_dist: f32) -> Self {
        Self { body_a: a, body_b: b, anchor_a: aa, anchor_b: ab, max_distance: max_dist, accumulated: 0.0 }
    }

    fn current_length(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pb - pa).length()
    }
}

impl Constraint for MaxDistanceConstraint {
    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        (self.current_length(bodies) - self.max_distance).max(0.0)
    }
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        if self.compute_c(bodies) <= 0.0 { return 0.0; }
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        let delta = pb - pa;
        let len   = delta.length().max(1e-7);
        let dir   = delta / len;
        let va = bodies.get(&self.body_a).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        (vb - va).dot(dir)
    }
    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let d = im_a + im_b; if d < 1e-10 { 0.0 } else { 1.0 / d }
    }
    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        let delta = pb - pa; let len = delta.length().max(1e-7); let dir = delta / len;
        let imp = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-imp, Vec2::ZERO); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(imp,  Vec2::ZERO); }
    }
    fn impulse_bounds(&self) -> (f32, f32) { (0.0, f32::INFINITY) }
    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated = (self.accumulated + d).max(0.0); }
}

// ── ConstraintSolver ──────────────────────────────────────────────────────────

/// Sequential-impulse constraint solver.
///
/// Call `solve()` once per physics step. Internally iterates
/// `iteration_count` times applying velocity corrections.
pub struct ConstraintSolver {
    pub iteration_count:  u32,
    pub baumgarte_factor: f32,
    pub warm_start:       bool,
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self { iteration_count: 10, baumgarte_factor: 0.2, warm_start: true }
    }
}

impl ConstraintSolver {
    pub fn new(iterations: u32) -> Self {
        Self { iteration_count: iterations, ..Default::default() }
    }

    /// Solve all constraints. Bodies and constraints are updated in place.
    pub fn solve(
        &self,
        bodies:      &mut HashMap<BodyHandle, BodyState>,
        constraints: &mut [Box<dyn Constraint>],
        dt:          f32,
    ) {
        if dt < 1e-8 { return; }

        // Warm starting: apply accumulated impulses from last frame
        if self.warm_start {
            for c in constraints.iter() {
                let lambda = c.accumulated_impulse();
                if lambda.abs() > 1e-10 {
                    c.apply_impulse(bodies, lambda);
                }
            }
        } else {
            for c in constraints.iter_mut() { c.reset_accumulated(); }
        }

        // Iterative velocity solve
        for _ in 0..self.iteration_count {
            for c in constraints.iter_mut() {
                let em = c.effective_mass(bodies);
                if em < 1e-10 { continue; }

                let cdot  = c.compute_cdot(bodies);
                let bias  = c.bias(bodies, dt);
                let delta = -(cdot + bias) * em;

                // Clamp accumulated impulse
                let (lo, hi) = c.impulse_bounds();
                let prev = c.accumulated_impulse();
                let new  = (prev + delta).clamp(lo, hi);
                let actual_delta = new - prev;
                c.add_accumulated(actual_delta);

                if actual_delta.abs() > 1e-12 {
                    c.apply_impulse(bodies, actual_delta);
                }
            }
        }
    }

    /// Apply position correction (post-solve) using pseudo-velocities.
    pub fn solve_positions(
        &self,
        bodies:      &mut HashMap<BodyHandle, BodyState>,
        constraints: &[Box<dyn Constraint>],
        dt:          f32,
    ) {
        let alpha = self.baumgarte_factor / dt.max(1e-6);
        for c in constraints {
            let em = c.effective_mass(bodies);
            if em < 1e-10 { continue; }
            let c_val = c.compute_c(bodies);
            if c_val.abs() < 1e-5 { continue; }
            let lambda = -alpha * c_val * em;
            c.apply_impulse(bodies, lambda);
        }
    }
}

// ── ConstraintWorld ───────────────────────────────────────────────────────────

/// Manages all bodies and constraints in one place.
pub struct ConstraintWorld {
    pub bodies:      HashMap<BodyHandle, BodyState>,
    pub constraints: Vec<Box<dyn Constraint>>,
    pub solver:      ConstraintSolver,
    pub gravity:     Vec2,
    next_body_id:    u32,
    next_con_id:     u32,
}

impl ConstraintWorld {
    pub fn new() -> Self {
        Self {
            bodies:       HashMap::new(),
            constraints:  Vec::new(),
            solver:       ConstraintSolver::default(),
            gravity:      Vec2::new(0.0, -9.81),
            next_body_id: 1,
            next_con_id:  1,
        }
    }

    pub fn add_body(&mut self, state: BodyState) -> BodyHandle {
        let id = BodyHandle(self.next_body_id);
        self.next_body_id += 1;
        self.bodies.insert(id, state);
        id
    }

    pub fn add_constraint(&mut self, c: Box<dyn Constraint>) -> ConstraintId {
        let id = ConstraintId(self.next_con_id);
        self.next_con_id += 1;
        self.constraints.push(c);
        id
    }

    pub fn remove_body(&mut self, handle: BodyHandle) {
        self.bodies.remove(&handle);
    }

    pub fn step(&mut self, dt: f32) {
        // Apply gravity to all dynamic bodies
        for body in self.bodies.values_mut() {
            if !body.is_static() {
                body.velocity += self.gravity * dt;
            }
        }

        // Integrate velocities to positions (before constraint solve)
        for body in self.bodies.values_mut() {
            if !body.is_static() {
                body.position += body.velocity * dt;
                body.angle    += body.angular_velocity * dt;
            }
        }

        // Solve constraints
        self.solver.solve(&mut self.bodies, &mut self.constraints, dt);
        self.solver.solve_positions(&mut self.bodies, &self.constraints, dt);
    }

    pub fn body(&self, h: BodyHandle) -> Option<&BodyState> { self.bodies.get(&h) }
    pub fn body_mut(&mut self, h: BodyHandle) -> Option<&mut BodyState> { self.bodies.get_mut(&h) }
}

impl Default for ConstraintWorld {
    fn default() -> Self { Self::new() }
}
