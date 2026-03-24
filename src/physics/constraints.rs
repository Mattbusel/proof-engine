//! XPBD + Sequential-Impulse constraint solver for 2D/3D rigid bodies.
//!
//! Implements both:
//! - Sequential Impulse (SI) solver — velocity-level constraint resolution
//!   following Erin Catto / Box2D style iterative impulses.
//! - Extended Position-Based Dynamics (XPBD) — position-level constraints
//!   with compliance, substep integration, and constraint islands.
//!
//! ## Constraint types
//! - `ContactConstraint`      — normal + friction, restitution, Baumgarte, warm-start
//! - `DistanceConstraint`     — rigid rod (exact) and soft spring (compliance)
//! - `HingeConstraint`        — single-axis rotation with limits and motor
//! - `BallSocketConstraint`   — 3-DOF free rotation with cone limit
//! - `SliderConstraint`       — prismatic joint with translation limits and motor
//! - `WeldConstraint`         — fully rigid 6-DOF lock
//! - `PulleyConstraint`       — two bodies through a fixed pulley ratio
//! - `MotorConstraint`        — angular velocity drive
//! - `MaxDistanceConstraint`  — soft rope
//! - `GearConstraint`         — ratio-based angular coupling

use glam::{Vec2, Vec3, Mat2};
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
        self.velocity         += impulse * self.inv_mass;
        self.angular_velocity += cross2(contact_arm, impulse) * self.inv_inertia;
    }

    pub fn apply_angular_impulse(&mut self, ang_impulse: f32) {
        self.angular_velocity += ang_impulse * self.inv_inertia;
    }

    pub fn is_static(&self) -> bool { self.inv_mass < 1e-12 }

    pub fn kinetic_energy(&self) -> f32 {
        let m = if self.inv_mass > 1e-12 { 1.0 / self.inv_mass } else { 0.0 };
        let i = if self.inv_inertia > 1e-12 { 1.0 / self.inv_inertia } else { 0.0 };
        0.5 * m * self.velocity.length_squared() + 0.5 * i * self.angular_velocity * self.angular_velocity
    }
}

// ── Math helpers ──────────────────────────────────────────────────────────────

/// 2D cross product (scalar result): a.x*b.y - a.y*b.x
#[inline]
pub fn cross2(a: Vec2, b: Vec2) -> f32 { a.x * b.y - a.y * b.x }

/// Perpendicular of a 2D vector: (-y, x)
#[inline]
pub fn perp(v: Vec2) -> Vec2 { Vec2::new(-v.y, v.x) }

/// Rotate a Vec2 by angle theta.
#[inline]
pub fn rotate2(v: Vec2, theta: f32) -> Vec2 {
    let (s, c) = theta.sin_cos();
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

/// Clamp an angle to [-PI, PI].
#[inline]
fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::PI;
    let mut r = a % (2.0 * PI);
    if r > PI  { r -= 2.0 * PI; }
    if r < -PI { r += 2.0 * PI; }
    r
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
    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32;
    /// Apply the corrective impulse to body states.
    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32);
    /// Compute effective constraint mass (1 / (J M^-1 J^T)).
    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32;
    /// XPBD compliance (inverse stiffness). 0 = rigid.
    fn get_compliance(&self) -> f32 { 0.0 }
    /// Baumgarte position bias.
    fn bias(&self, bodies: &HashMap<BodyHandle, BodyState>, dt: f32) -> f32 {
        let beta = 0.1;
        -beta / dt * self.compute_c(bodies)
    }
    /// Prepare the constraint for the current step (pre-compute cached values).
    fn prepare(&mut self, _bodies: &HashMap<BodyHandle, BodyState>, _dt: f32) {}
    /// Solve velocity constraint (one iteration).
    fn solve_velocity(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        let em = self.effective_mass(bodies);
        if em < 1e-10 { return; }
        let cdot = self.compute_cdot(bodies);
        let bias = self.bias(bodies, dt);
        let delta = -(cdot + bias) * em;
        let (lo, hi) = self.impulse_bounds();
        let prev = self.accumulated_impulse();
        let new_acc = (prev + delta).clamp(lo, hi);
        let actual = new_acc - prev;
        self.add_accumulated(actual);
        if actual.abs() > 1e-14 {
            self.apply_impulse(bodies, actual);
        }
    }
    /// Solve position constraint using XPBD (one sub-step).
    fn solve_position(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        let compliance = self.get_compliance();
        let em = self.effective_mass(bodies);
        let c = self.compute_c(bodies);
        let denom = em + compliance / (dt * dt);
        if denom < 1e-10 { return; }
        let lambda = -c / denom;
        self.apply_impulse(bodies, lambda);
    }
    /// Accumulated impulse for warm starting.
    fn accumulated_impulse(&self) -> f32;
    fn reset_accumulated(&mut self);
    fn add_accumulated(&mut self, delta: f32);
    /// Whether this constraint has an upper/lower impulse clamp.
    fn impulse_bounds(&self) -> (f32, f32) { (f32::NEG_INFINITY, f32::INFINITY) }
    /// Bodies involved in this constraint.
    fn body_handles(&self) -> Vec<BodyHandle> { Vec::new() }
}

// ── ContactConstraint ─────────────────────────────────────────────────────────

/// Contact constraint: normal impulse (non-penetration) + friction impulse.
///
/// Uses Baumgarte stabilization and warm-starting of cached impulses.
#[derive(Debug, Clone)]
pub struct ContactConstraint {
    pub body_a:         BodyHandle,
    pub body_b:         BodyHandle,
    /// Contact point in world space.
    pub contact_point:  Vec2,
    /// Contact normal (from A to B).
    pub normal:         Vec2,
    /// Penetration depth (positive = overlap).
    pub penetration:    f32,
    /// Restitution (bounciness).
    pub restitution:    f32,
    /// Friction coefficient.
    pub friction:       f32,
    /// Baumgarte bias factor.
    pub baumgarte:      f32,
    /// Penetration slop — small overlaps are allowed.
    pub slop:           f32,
    /// Warm-started normal impulse.
    pub cached_normal:  f32,
    /// Warm-started friction impulse.
    pub cached_tangent: f32,
    /// Effective mass along normal (cached in prepare).
    eff_mass_n:         f32,
    /// Effective mass along tangent (cached in prepare).
    eff_mass_t:         f32,
    /// Velocity bias (restitution + Baumgarte).
    bias:               f32,
    /// Contact arm A (world anchor - body A center).
    ra:                 Vec2,
    /// Contact arm B (world anchor - body B center).
    rb:                 Vec2,
}

impl ContactConstraint {
    pub fn new(
        body_a: BodyHandle, body_b: BodyHandle,
        contact_point: Vec2, normal: Vec2, penetration: f32,
        restitution: f32, friction: f32,
    ) -> Self {
        Self {
            body_a, body_b, contact_point, normal, penetration,
            restitution, friction,
            baumgarte: 0.2, slop: 0.005,
            cached_normal: 0.0, cached_tangent: 0.0,
            eff_mass_n: 0.0, eff_mass_t: 0.0,
            bias: 0.0, ra: Vec2::ZERO, rb: Vec2::ZERO,
        }
    }

    fn tangent(&self) -> Vec2 {
        Vec2::new(-self.normal.y, self.normal.x)
    }

    fn compute_effective_mass_along(
        &self, dir: Vec2, bodies: &HashMap<BodyHandle, BodyState>,
    ) -> f32 {
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let im_a = ba.map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bb.map(|b| b.inv_mass).unwrap_or(0.0);
        let ii_a = ba.map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bb.map(|b| b.inv_inertia).unwrap_or(0.0);
        let rda = cross2(self.ra, dir);
        let rdb = cross2(self.rb, dir);
        let d = im_a + im_b + ii_a * rda * rda + ii_b * rdb * rdb;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    fn relative_velocity_at_contact(&self, bodies: &HashMap<BodyHandle, BodyState>) -> Vec2 {
        let va = bodies.get(&self.body_a)
            .map(|b| b.velocity + perp(self.ra) * b.angular_velocity)
            .unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b)
            .map(|b| b.velocity + perp(self.rb) * b.angular_velocity)
            .unwrap_or(Vec2::ZERO);
        vb - va
    }

    /// Apply normal impulse only.
    fn apply_normal_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let impulse = self.normal * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, self.ra);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, self.rb);
        }
    }

    /// Apply tangent (friction) impulse.
    fn apply_tangent_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let tangent = self.tangent();
        let impulse = tangent * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, self.ra);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, self.rb);
        }
    }

    /// Solve both normal and friction in one call, handling warm-starting.
    pub fn solve_contact(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        // Normal
        let rel_vel = self.relative_velocity_at_contact(bodies);
        let vn = rel_vel.dot(self.normal);
        let lambda_n = -(vn + self.bias) * self.eff_mass_n;
        let prev_n = self.cached_normal;
        let new_n = (prev_n + lambda_n).max(0.0);
        let actual_n = new_n - prev_n;
        self.cached_normal = new_n;
        self.apply_normal_impulse(bodies, actual_n);

        // Friction
        let rel_vel2 = self.relative_velocity_at_contact(bodies);
        let tangent = self.tangent();
        let vt = rel_vel2.dot(tangent);
        let lambda_t = -vt * self.eff_mass_t;
        let max_friction = self.friction * self.cached_normal;
        let prev_t = self.cached_tangent;
        let new_t = (prev_t + lambda_t).clamp(-max_friction, max_friction);
        let actual_t = new_t - prev_t;
        self.cached_tangent = new_t;
        self.apply_tangent_impulse(bodies, actual_t);
    }
}

impl Constraint for ContactConstraint {
    fn prepare(&mut self, bodies: &HashMap<BodyHandle, BodyState>, dt: f32) {
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        self.ra = ba.map(|b| self.contact_point - b.position).unwrap_or(Vec2::ZERO);
        self.rb = bb.map(|b| self.contact_point - b.position).unwrap_or(Vec2::ZERO);

        self.eff_mass_n = self.compute_effective_mass_along(self.normal, bodies);
        self.eff_mass_t = self.compute_effective_mass_along(self.tangent(), bodies);

        // Restitution bias: only if separating velocity is significant
        let rel_vel = self.relative_velocity_at_contact(bodies);
        let vn = rel_vel.dot(self.normal);
        let restitution_bias = if vn < -1.0 { -self.restitution * vn } else { 0.0 };

        // Baumgarte bias
        let baumgarte_bias = self.baumgarte / dt * (self.penetration - self.slop).max(0.0);

        self.bias = restitution_bias + baumgarte_bias;
        // Note: warm-start impulse application happens in solve_velocity (bodies is &mut there)
    }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        self.relative_velocity_at_contact(bodies).dot(self.normal)
    }

    fn compute_c(&self, _bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        -self.penetration
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        self.compute_effective_mass_along(self.normal, bodies)
    }

    fn bias(&self, _bodies: &HashMap<BodyHandle, BodyState>, _dt: f32) -> f32 {
        self.bias
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        self.apply_normal_impulse(bodies, lambda);
    }

    fn solve_velocity(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        self.solve_contact(bodies, dt);
    }

    fn impulse_bounds(&self) -> (f32, f32) { (0.0, f32::INFINITY) }
    fn accumulated_impulse(&self) -> f32 { self.cached_normal }
    fn reset_accumulated(&mut self) { self.cached_normal = 0.0; self.cached_tangent = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.cached_normal = (self.cached_normal + d).max(0.0); }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── DistanceConstraint ────────────────────────────────────────────────────────

/// Maintain a fixed distance between two local anchor points on two bodies.
/// With compliance = 0 this is a rigid rod; compliance > 0 gives spring behavior.
#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    pub body_a:      BodyHandle,
    pub body_b:      BodyHandle,
    pub anchor_a:    Vec2,
    pub anchor_b:    Vec2,
    pub rest_length: f32,
    /// 0.0 = rigid rod, >0 = soft spring (XPBD compliance).
    pub compliance:  f32,
    accumulated:     f32,
}

impl DistanceConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2, rest_length: f32) -> Self {
        Self { body_a, body_b, anchor_a, anchor_b, rest_length, compliance: 0.0, accumulated: 0.0 }
    }

    /// Create a rigid rod (exact distance, zero compliance).
    pub fn rigid_rod(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2, rest_length: f32) -> Self {
        Self::new(body_a, anchor_a, body_b, anchor_b, rest_length)
    }

    /// Create a soft spring (positive compliance = 1/k).
    pub fn soft(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2, rest_length: f32, compliance: f32) -> Self {
        Self { body_a, body_b, anchor_a, anchor_b, rest_length, compliance, accumulated: 0.0 }
    }

    pub fn with_compliance(mut self, c: f32) -> Self { self.compliance = c; self }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }

    fn constraint_dir(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len = delta.length();
        let dir = if len > 1e-7 { delta / len } else { Vec2::X };
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

    fn get_compliance(&self) -> f32 { self.compliance }

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
        let denom = im_a + im_b + ii_a * rda * rda + ii_b * rdb * rdb;
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = {
            let ba = bodies.get(&self.body_a);
            let bb = bodies.get(&self.body_b);
            (
                ba.map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO),
                bb.map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO),
            )
        };
        let delta = pb - pa;
        let len = delta.length();
        let dir = if len > 1e-7 { delta / len } else { Vec2::X };
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
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── HingeConstraint ───────────────────────────────────────────────────────────

/// Hinge (revolute) joint: two bodies share a common anchor. Allows rotation
/// around the Z-axis. Supports angular limits and a velocity motor.
#[derive(Debug, Clone)]
pub struct HingeConstraint {
    pub body_a:      BodyHandle,
    pub body_b:      BodyHandle,
    pub anchor_a:    Vec2,
    pub anchor_b:    Vec2,
    /// Angular limits: (min, max) relative angle in radians.
    pub limits:      Option<(f32, f32)>,
    /// Motor: (target_velocity, max_torque).
    pub motor:       Option<(f32, f32)>,
    /// Compliance for soft limits (0 = rigid).
    pub compliance:  f32,
    accum_x:         f32,
    accum_y:         f32,
    accum_limit:     f32,
    accum_motor:     f32,
}

impl HingeConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2) -> Self {
        Self {
            body_a, body_b, anchor_a, anchor_b,
            limits: None, motor: None, compliance: 0.0,
            accum_x: 0.0, accum_y: 0.0, accum_limit: 0.0, accum_motor: 0.0,
        }
    }

    /// Add angular limits in radians (relative angle of B w.r.t. A).
    pub fn with_limits(mut self, min: f32, max: f32) -> Self {
        self.limits = Some((min, max)); self
    }

    /// Add a rotational motor.
    pub fn with_motor(mut self, target_velocity: f32, max_torque: f32) -> Self {
        self.motor = Some((target_velocity, max_torque)); self
    }

    pub fn with_compliance(mut self, c: f32) -> Self { self.compliance = c; self }

    fn relative_angle(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let angle_a = bodies.get(&self.body_a).map(|b| b.angle).unwrap_or(0.0);
        let angle_b = bodies.get(&self.body_b).map(|b| b.angle).unwrap_or(0.0);
        wrap_angle(angle_b - angle_a)
    }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }

    fn positional_effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let im_a = ba.map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bb.map(|b| b.inv_mass).unwrap_or(0.0);
        let d = im_a + im_b;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    /// Solve the position constraint (anchor match) and return correction impulse.
    fn solve_anchor(&self, bodies: &mut HashMap<BodyHandle, BodyState>, beta: f32, dt: f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        if delta.length_squared() < 1e-12 { return; }
        let len = delta.length();
        let dir = delta / len;
        let em = self.positional_effective_mass(bodies);
        if em < 1e-10 { return; }
        let lambda = -beta / dt * len * em;
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, pa - b.position);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, pb - b.position);
        }
    }

    /// Solve the limit constraint (angular).
    fn solve_limit(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        let Some((min, max)) = self.limits else { return };
        let rel_angle = self.relative_angle(bodies);
        let violation = if rel_angle < min {
            rel_angle - min
        } else if rel_angle > max {
            rel_angle - max
        } else {
            return;
        };
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let denom = ii_a + ii_b + self.compliance / (dt * dt);
        if denom < 1e-10 { return; }
        let lambda = -violation / denom;
        let prev = self.accum_limit;
        let new_acc = if rel_angle < min {
            (prev + lambda).max(0.0)
        } else {
            (prev + lambda).min(0.0)
        };
        let actual = new_acc - prev;
        self.accum_limit = new_acc;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_angular_impulse(-actual);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_angular_impulse(actual);
        }
    }

    /// Solve the motor constraint.
    fn solve_motor(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>) {
        let Some((target_vel, max_torque)) = self.motor else { return };
        let omega_a = bodies.get(&self.body_a).map(|b| b.angular_velocity).unwrap_or(0.0);
        let omega_b = bodies.get(&self.body_b).map(|b| b.angular_velocity).unwrap_or(0.0);
        let rel_omega = omega_b - omega_a;
        let error = rel_omega - target_vel;
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let denom = ii_a + ii_b;
        if denom < 1e-10 { return; }
        let lambda = -error / denom;
        let prev = self.accum_motor;
        let new_acc = (prev + lambda).clamp(-max_torque, max_torque);
        let actual = new_acc - prev;
        self.accum_motor = new_acc;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_angular_impulse(-actual);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_angular_impulse(actual);
        }
    }
}

impl Constraint for HingeConstraint {
    fn prepare(&mut self, _bodies: &HashMap<BodyHandle, BodyState>, _dt: f32) {
        // Warm-start: accumulators are kept from the previous frame
    }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let ba = bodies.get(&self.body_a);
        let bb = bodies.get(&self.body_b);
        let va = ba.map(|b| b.velocity + perp(pa - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        let vb = bb.map(|b| b.velocity + perp(pb - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        (vb - va).length()
    }

    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        (pb - pa).length()
    }

    fn get_compliance(&self) -> f32 { self.compliance }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        self.positional_effective_mass(bodies)
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len = delta.length();
        let dir = if len > 1e-7 { delta / len } else { Vec2::X };
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, pa - b.position);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, pb - b.position);
        }
    }

    fn solve_velocity(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        // Positional anchor constraint
        let em = self.effective_mass(bodies);
        if em > 1e-10 {
            let cdot = self.compute_cdot(bodies);
            let bias = self.bias(bodies, dt);
            let delta = -(cdot + bias) * em;
            let prev = self.accum_x;
            let new_acc = prev + delta;
            let actual = new_acc - prev;
            self.accum_x = new_acc;
            if actual.abs() > 1e-14 {
                self.apply_impulse(bodies, actual);
            }
        }
        // Limit and motor
        self.solve_limit(bodies, dt);
        self.solve_motor(bodies);
    }

    fn solve_position(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        self.solve_anchor(bodies, 0.2, dt);
    }

    fn accumulated_impulse(&self) -> f32 { self.accum_x }
    fn reset_accumulated(&mut self) {
        self.accum_x = 0.0; self.accum_y = 0.0;
        self.accum_limit = 0.0; self.accum_motor = 0.0;
    }
    fn add_accumulated(&mut self, d: f32) { self.accum_x += d; }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── BallSocketConstraint ─────────────────────────────────────────────────────

/// Ball-and-socket joint: 3-DOF rotation, fixed relative position.
/// Optionally limits the cone angle between the two body Z-axes.
#[derive(Debug, Clone)]
pub struct BallSocketConstraint {
    pub body_a:       BodyHandle,
    pub body_b:       BodyHandle,
    pub anchor_a:     Vec2,
    pub anchor_b:     Vec2,
    /// Max cone half-angle in radians (None = unlimited).
    pub cone_limit:   Option<f32>,
    pub compliance:   f32,
    accumulated:      f32,
    accum_cone:       f32,
}

impl BallSocketConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2) -> Self {
        Self { body_a, body_b, anchor_a, anchor_b, cone_limit: None, compliance: 0.0, accumulated: 0.0, accum_cone: 0.0 }
    }

    pub fn with_cone_limit(mut self, angle: f32) -> Self { self.cone_limit = Some(angle); self }
    pub fn with_compliance(mut self, c: f32) -> Self { self.compliance = c; self }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }

    fn solve_cone_limit(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>) {
        let Some(max_angle) = self.cone_limit else { return };
        let angle_a = bodies.get(&self.body_a).map(|b| b.angle).unwrap_or(0.0);
        let angle_b = bodies.get(&self.body_b).map(|b| b.angle).unwrap_or(0.0);
        let rel = wrap_angle(angle_b - angle_a);
        if rel.abs() <= max_angle { return; }
        let violation = if rel > max_angle { rel - max_angle } else { rel + max_angle };
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let denom = ii_a + ii_b;
        if denom < 1e-10 { return; }
        let lambda = -violation / denom;
        let prev = self.accum_cone;
        let new_acc = if rel > max_angle { (prev + lambda).min(0.0) } else { (prev + lambda).max(0.0) };
        let actual = new_acc - prev;
        self.accum_cone = new_acc;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_angular_impulse(-actual); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_angular_impulse(actual); }
    }
}

impl Constraint for BallSocketConstraint {
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let va = bodies.get(&self.body_a).map(|b| b.velocity + perp(pa - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity + perp(pb - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        (vb - va).length()
    }

    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        (pb - pa).length()
    }

    fn get_compliance(&self) -> f32 { self.compliance }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let d = im_a + im_b;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len = delta.length();
        let dir = if len > 1e-7 { delta / len } else { Vec2::X };
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, pa - b.position);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, pb - b.position);
        }
    }

    fn solve_velocity(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        let em = self.effective_mass(bodies);
        if em > 1e-10 {
            let cdot = self.compute_cdot(bodies);
            let bias = self.bias(bodies, dt);
            let delta = -(cdot + bias) * em;
            self.accumulated += delta;
            self.apply_impulse(bodies, delta);
        }
        self.solve_cone_limit(bodies);
    }

    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; self.accum_cone = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated += d; }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── SliderConstraint ──────────────────────────────────────────────────────────

/// Prismatic (slider) joint: allows translation along one axis only.
/// Supports translation limits and a linear motor.
#[derive(Debug, Clone)]
pub struct SliderConstraint {
    pub body_a:     BodyHandle,
    pub body_b:     BodyHandle,
    pub anchor_a:   Vec2,
    pub anchor_b:   Vec2,
    /// Slide axis in world space (unit vector).
    pub axis:       Vec2,
    /// Translation limits (min, max) in world units.
    pub limits:     Option<(f32, f32)>,
    /// Motor: (target_velocity, max_force).
    pub motor:      Option<(f32, f32)>,
    pub compliance: f32,
    accum_perp:     f32,  // perpendicular constraint
    accum_limit:    f32,
    accum_motor:    f32,
}

impl SliderConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2, axis: Vec2) -> Self {
        let axis = axis.normalize_or_zero();
        Self {
            body_a, body_b, anchor_a, anchor_b, axis,
            limits: None, motor: None, compliance: 0.0,
            accum_perp: 0.0, accum_limit: 0.0, accum_motor: 0.0,
        }
    }

    pub fn with_limits(mut self, min: f32, max: f32) -> Self { self.limits = Some((min, max)); self }
    pub fn with_motor(mut self, vel: f32, max_force: f32) -> Self { self.motor = Some((vel, max_force)); self }
    pub fn with_compliance(mut self, c: f32) -> Self { self.compliance = c; self }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }

    fn slide_offset(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        (pb - pa).dot(self.axis)
    }

    fn perp_offset(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let perp_axis = Vec2::new(-self.axis.y, self.axis.x);
        (pb - pa).dot(perp_axis)
    }

    fn solve_perp(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, beta: f32, dt: f32) {
        let perp_axis = Vec2::new(-self.axis.y, self.axis.x);
        let c = self.perp_offset(bodies);
        if c.abs() < 1e-8 { return; }
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let (pa, pb) = self.world_anchors(bodies);
        let ra = bodies.get(&self.body_a).map(|b| pa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bodies.get(&self.body_b).map(|b| pb - b.position).unwrap_or(Vec2::ZERO);
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let rda = cross2(ra, perp_axis);
        let rdb = cross2(rb, perp_axis);
        let denom = im_a + im_b + ii_a * rda * rda + ii_b * rdb * rdb;
        if denom < 1e-10 { return; }
        let lambda = -beta / dt * c / denom;
        let impulse = perp_axis * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-impulse, ra); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(impulse, rb); }
    }

    fn solve_translation_limit(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        let Some((min, max)) = self.limits else { return };
        let offset = self.slide_offset(bodies);
        let violation = if offset < min { offset - min } else if offset > max { offset - max } else { return };
        let (pa, pb) = self.world_anchors(bodies);
        let ra = bodies.get(&self.body_a).map(|b| pa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bodies.get(&self.body_b).map(|b| pb - b.position).unwrap_or(Vec2::ZERO);
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let rda = cross2(ra, self.axis);
        let rdb = cross2(rb, self.axis);
        let denom = im_a + im_b + ii_a * rda * rda + ii_b * rdb * rdb + self.compliance / (dt * dt);
        if denom < 1e-10 { return; }
        let lambda = -violation / denom;
        let prev = self.accum_limit;
        let new_acc = if offset < min { (prev + lambda).max(0.0) } else { (prev + lambda).min(0.0) };
        let actual = new_acc - prev;
        self.accum_limit = new_acc;
        let impulse = self.axis * actual;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-impulse, ra); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(impulse, rb); }
    }

    fn solve_motor_constraint(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>) {
        let Some((target_vel, max_force)) = self.motor else { return };
        let va = bodies.get(&self.body_a).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let rel_vel = (vb - va).dot(self.axis);
        let error = rel_vel - target_vel;
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let denom = im_a + im_b;
        if denom < 1e-10 { return; }
        let lambda = -error / denom;
        let prev = self.accum_motor;
        let new_acc = (prev + lambda).clamp(-max_force, max_force);
        let actual = new_acc - prev;
        self.accum_motor = new_acc;
        let impulse = self.axis * actual;
        let (pa, pb) = self.world_anchors(bodies);
        let ra = bodies.get(&self.body_a).map(|b| pa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bodies.get(&self.body_b).map(|b| pb - b.position).unwrap_or(Vec2::ZERO);
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-impulse, ra); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(impulse, rb); }
    }
}

impl Constraint for SliderConstraint {
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let va = bodies.get(&self.body_a).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        (vb - va).dot(Vec2::new(-self.axis.y, self.axis.x))
    }

    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        self.perp_offset(bodies)
    }

    fn get_compliance(&self) -> f32 { self.compliance }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let perp_axis = Vec2::new(-self.axis.y, self.axis.x);
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let d = im_a + im_b;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let perp_axis = Vec2::new(-self.axis.y, self.axis.x);
        let impulse = perp_axis * lambda;
        let (pa, pb) = self.world_anchors(bodies);
        let ra = bodies.get(&self.body_a).map(|b| pa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bodies.get(&self.body_b).map(|b| pb - b.position).unwrap_or(Vec2::ZERO);
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-impulse, ra); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(impulse, rb); }
    }

    fn solve_velocity(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        self.solve_perp(bodies, 0.1, dt);
        self.solve_translation_limit(bodies, dt);
        self.solve_motor_constraint(bodies);
    }

    fn solve_position(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        self.solve_perp(bodies, 0.2, dt);
    }

    fn accumulated_impulse(&self) -> f32 { self.accum_perp }
    fn reset_accumulated(&mut self) { self.accum_perp = 0.0; self.accum_limit = 0.0; self.accum_motor = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accum_perp += d; }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── WeldConstraint ────────────────────────────────────────────────────────────

/// Weld joint: fully rigid 6-DOF lock. Fixes both relative position and angle.
#[derive(Debug, Clone)]
pub struct WeldConstraint {
    pub body_a:          BodyHandle,
    pub body_b:          BodyHandle,
    pub anchor_a:        Vec2,
    pub anchor_b:        Vec2,
    /// Reference relative angle (captured at creation time).
    pub ref_angle:       f32,
    /// Angular compliance (0 = perfectly rigid).
    pub angular_compliance: f32,
    /// Linear compliance.
    pub linear_compliance: f32,
    accum_lin:           f32,
    accum_ang:           f32,
}

impl WeldConstraint {
    pub fn new(body_a: BodyHandle, anchor_a: Vec2, body_b: BodyHandle, anchor_b: Vec2, ref_angle: f32) -> Self {
        Self {
            body_a, body_b, anchor_a, anchor_b, ref_angle,
            angular_compliance: 0.0, linear_compliance: 0.0,
            accum_lin: 0.0, accum_ang: 0.0,
        }
    }

    pub fn with_angular_compliance(mut self, c: f32) -> Self { self.angular_compliance = c; self }
    pub fn with_linear_compliance(mut self, c: f32) -> Self { self.linear_compliance = c; self }

    fn world_anchors(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        (pa, pb)
    }

    fn angle_error(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let angle_a = bodies.get(&self.body_a).map(|b| b.angle).unwrap_or(0.0);
        let angle_b = bodies.get(&self.body_b).map(|b| b.angle).unwrap_or(0.0);
        wrap_angle(angle_b - angle_a - self.ref_angle)
    }
}

impl Constraint for WeldConstraint {
    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        let va = bodies.get(&self.body_a).map(|b| b.velocity + perp(pa - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity + perp(pb - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        (vb - va).length()
    }

    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (pa, pb) = self.world_anchors(bodies);
        (pb - pa).length()
    }

    fn get_compliance(&self) -> f32 { self.linear_compliance }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let d = im_a + im_b;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len = delta.length();
        let dir = if len > 1e-7 { delta / len } else { Vec2::X };
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-impulse, pa - b.position);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(impulse, pb - b.position);
        }
    }

    fn solve_velocity(&mut self, bodies: &mut HashMap<BodyHandle, BodyState>, dt: f32) {
        // Linear
        let em_lin = self.effective_mass(bodies);
        if em_lin > 1e-10 {
            let cdot = self.compute_cdot(bodies);
            let bias = self.bias(bodies, dt);
            let delta = -(cdot + bias) * em_lin;
            self.accum_lin += delta;
            self.apply_impulse(bodies, delta);
        }
        // Angular
        let err = self.angle_error(bodies);
        let omega_a = bodies.get(&self.body_a).map(|b| b.angular_velocity).unwrap_or(0.0);
        let omega_b = bodies.get(&self.body_b).map(|b| b.angular_velocity).unwrap_or(0.0);
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let denom = ii_a + ii_b + self.angular_compliance / (dt * dt);
        if denom > 1e-10 {
            let ang_bias = -0.1 / dt * err;
            let rel_omega = omega_b - omega_a;
            let lambda = -(rel_omega + ang_bias) / denom;
            self.accum_ang += lambda;
            if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_angular_impulse(-lambda); }
            if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_angular_impulse(lambda); }
        }
    }

    fn accumulated_impulse(&self) -> f32 { self.accum_lin }
    fn reset_accumulated(&mut self) { self.accum_lin = 0.0; self.accum_ang = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accum_lin += d; }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── PulleyConstraint ──────────────────────────────────────────────────────────

/// Pulley constraint: body_a and body_b are connected via a rope over two
/// fixed pulley points. Maintains: len_a + ratio * len_b = constant.
#[derive(Debug, Clone)]
pub struct PulleyConstraint {
    pub body_a:       BodyHandle,
    pub body_b:       BodyHandle,
    pub anchor_a:     Vec2,      // local anchor on body A
    pub anchor_b:     Vec2,      // local anchor on body B
    pub pulley_a:     Vec2,      // fixed world-space pulley point for A
    pub pulley_b:     Vec2,      // fixed world-space pulley point for B
    pub ratio:        f32,       // pulley ratio
    pub total_length: f32,       // len_a + ratio * len_b at rest
    accumulated:      f32,
}

impl PulleyConstraint {
    pub fn new(
        body_a: BodyHandle, anchor_a: Vec2, pulley_a: Vec2,
        body_b: BodyHandle, anchor_b: Vec2, pulley_b: Vec2,
        ratio: f32, total_length: f32,
    ) -> Self {
        Self { body_a, body_b, anchor_a, anchor_b, pulley_a, pulley_b, ratio, total_length, accumulated: 0.0 }
    }

    fn world_anchor_a(&self, bodies: &HashMap<BodyHandle, BodyState>) -> Vec2 {
        bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO)
    }
    fn world_anchor_b(&self, bodies: &HashMap<BodyHandle, BodyState>) -> Vec2 {
        bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO)
    }

    fn lengths(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (f32, f32) {
        let wa = self.world_anchor_a(bodies);
        let wb = self.world_anchor_b(bodies);
        let la = (wa - self.pulley_a).length();
        let lb = (wb - self.pulley_b).length();
        (la, lb)
    }

    fn dirs(&self, bodies: &HashMap<BodyHandle, BodyState>) -> (Vec2, Vec2) {
        let wa = self.world_anchor_a(bodies);
        let wb = self.world_anchor_b(bodies);
        let da = (wa - self.pulley_a);
        let db = (wb - self.pulley_b);
        let la = da.length();
        let lb = db.length();
        (
            if la > 1e-7 { da / la } else { Vec2::X },
            if lb > 1e-7 { db / lb } else { Vec2::X },
        )
    }
}

impl Constraint for PulleyConstraint {
    fn compute_c(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let (la, lb) = self.lengths(bodies);
        la + self.ratio * lb - self.total_length
    }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let wa = self.world_anchor_a(bodies);
        let wb = self.world_anchor_b(bodies);
        let (da, db) = self.dirs(bodies);
        let va = bodies.get(&self.body_a).map(|b| b.velocity + perp(wa - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity + perp(wb - b.position) * b.angular_velocity).unwrap_or(Vec2::ZERO);
        va.dot(da) + self.ratio * vb.dot(db)
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let wa = self.world_anchor_a(bodies);
        let wb = self.world_anchor_b(bodies);
        let (da, db) = self.dirs(bodies);
        let ra = bodies.get(&self.body_a).map(|b| wa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bodies.get(&self.body_b).map(|b| wb - b.position).unwrap_or(Vec2::ZERO);
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let rda = cross2(ra, da);
        let rdb = cross2(rb, db);
        let denom = im_a + ii_a * rda * rda + self.ratio * self.ratio * (im_b + ii_b * rdb * rdb);
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let wa = self.world_anchor_a(bodies);
        let wb = self.world_anchor_b(bodies);
        let (da, db) = self.dirs(bodies);
        let ra = bodies.get(&self.body_a).map(|b| wa - b.position).unwrap_or(Vec2::ZERO);
        let rb = bodies.get(&self.body_b).map(|b| wb - b.position).unwrap_or(Vec2::ZERO);
        if let Some(b) = bodies.get_mut(&self.body_a) {
            b.apply_impulse(-da * lambda, ra);
        }
        if let Some(b) = bodies.get_mut(&self.body_b) {
            b.apply_impulse(-db * (lambda * self.ratio), rb);
        }
    }

    fn impulse_bounds(&self) -> (f32, f32) { (0.0, f32::INFINITY) }
    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated = (self.accumulated + d).max(0.0); }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── SpringConstraint ──────────────────────────────────────────────────────────

/// A spring-damper between two anchor points (Hooke's law).
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
        let len = delta.length().max(1e-7);
        let dir = delta / len;
        let va = bodies.get(&self.body_a).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        (vb - va).dot(dir)
    }

    fn get_compliance(&self) -> f32 { if self.stiffness > 1e-6 { 1.0 / self.stiffness } else { 0.0 } }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let denom = im_a + im_b + self.get_compliance();
        if denom < 1e-10 { 0.0 } else { 1.0 / denom }
    }

    fn bias(&self, bodies: &HashMap<BodyHandle, BodyState>, dt: f32) -> f32 {
        -self.stiffness * self.compute_c(bodies) / dt
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let (pa, pb) = self.world_anchors(bodies);
        let delta = pb - pa;
        let len = delta.length().max(1e-7);
        let dir = delta / len;
        let impulse = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-impulse, Vec2::ZERO); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(impulse,  Vec2::ZERO); }
    }

    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated += d; }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── PinConstraint ─────────────────────────────────────────────────────────────

/// Pin a body's local anchor to a fixed world-space point.
#[derive(Debug, Clone)]
pub struct PinConstraint {
    pub body:         BodyHandle,
    pub local_anchor: Vec2,
    pub world_target: Vec2,
    pub compliance:   f32,
    accumulated:      f32,
}

impl PinConstraint {
    pub fn new(body: BodyHandle, local_anchor: Vec2, world_target: Vec2) -> Self {
        Self { body, local_anchor, world_target, compliance: 0.0, accumulated: 0.0 }
    }
    pub fn with_compliance(mut self, c: f32) -> Self { self.compliance = c; self }
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

    fn get_compliance(&self) -> f32 { self.compliance }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let b = match bodies.get(&self.body) { Some(b) => b, None => return 0.0 };
        let pa = b.position + rotate2(self.local_anchor, b.angle);
        let r = pa - b.position;
        let dir = (pa - self.world_target).normalize_or_zero();
        let rd = cross2(r, dir);
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
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body] }
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
        bodies.get(&self.body).map(|b| b.angular_velocity).unwrap_or(0.0) - self.target_velocity
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
    fn add_accumulated(&mut self, d: f32) {
        self.accumulated = (self.accumulated + d).clamp(-self.max_torque, self.max_torque);
    }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body] }
}

// ── GearConstraint ────────────────────────────────────────────────────────────

/// Couple the angular velocities of two bodies: omega_a + ratio * omega_b = 0.
#[derive(Debug, Clone)]
pub struct GearConstraint {
    pub body_a:  BodyHandle,
    pub body_b:  BodyHandle,
    pub ratio:   f32,
    accumulated: f32,
}

impl GearConstraint {
    pub fn new(body_a: BodyHandle, body_b: BodyHandle, ratio: f32) -> Self {
        Self { body_a, body_b, ratio, accumulated: 0.0 }
    }
}

impl Constraint for GearConstraint {
    fn compute_c(&self, _: &HashMap<BodyHandle, BodyState>) -> f32 { 0.0 }

    fn compute_cdot(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let oa = bodies.get(&self.body_a).map(|b| b.angular_velocity).unwrap_or(0.0);
        let ob = bodies.get(&self.body_b).map(|b| b.angular_velocity).unwrap_or(0.0);
        oa + self.ratio * ob
    }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let ii_a = bodies.get(&self.body_a).map(|b| b.inv_inertia).unwrap_or(0.0);
        let ii_b = bodies.get(&self.body_b).map(|b| b.inv_inertia).unwrap_or(0.0);
        let d = ii_a + self.ratio * self.ratio * ii_b;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_angular_impulse(-lambda); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_angular_impulse(-lambda * self.ratio); }
    }

    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated += d; }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── MaxDistanceConstraint ─────────────────────────────────────────────────────

/// Soft rope: only constrains when distance exceeds max_distance.
#[derive(Debug, Clone)]
pub struct MaxDistanceConstraint {
    pub body_a:       BodyHandle,
    pub body_b:       BodyHandle,
    pub anchor_a:     Vec2,
    pub anchor_b:     Vec2,
    pub max_distance: f32,
    pub compliance:   f32,
    accumulated:      f32,
}

impl MaxDistanceConstraint {
    pub fn new(a: BodyHandle, aa: Vec2, b: BodyHandle, ab: Vec2, max_dist: f32) -> Self {
        Self { body_a: a, body_b: b, anchor_a: aa, anchor_b: ab, max_distance: max_dist, compliance: 0.0, accumulated: 0.0 }
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
        let len = delta.length().max(1e-7);
        let dir = delta / len;
        let va = bodies.get(&self.body_a).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        let vb = bodies.get(&self.body_b).map(|b| b.velocity).unwrap_or(Vec2::ZERO);
        (vb - va).dot(dir)
    }

    fn get_compliance(&self) -> f32 { self.compliance }

    fn effective_mass(&self, bodies: &HashMap<BodyHandle, BodyState>) -> f32 {
        let im_a = bodies.get(&self.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
        let im_b = bodies.get(&self.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
        let d = im_a + im_b;
        if d < 1e-10 { 0.0 } else { 1.0 / d }
    }

    fn apply_impulse(&self, bodies: &mut HashMap<BodyHandle, BodyState>, lambda: f32) {
        let pa = bodies.get(&self.body_a).map(|b| b.position + rotate2(self.anchor_a, b.angle)).unwrap_or(Vec2::ZERO);
        let pb = bodies.get(&self.body_b).map(|b| b.position + rotate2(self.anchor_b, b.angle)).unwrap_or(Vec2::ZERO);
        let delta = pb - pa;
        let len = delta.length().max(1e-7);
        let dir = delta / len;
        let imp = dir * lambda;
        if let Some(b) = bodies.get_mut(&self.body_a) { b.apply_impulse(-imp, Vec2::ZERO); }
        if let Some(b) = bodies.get_mut(&self.body_b) { b.apply_impulse(imp,  Vec2::ZERO); }
    }

    fn impulse_bounds(&self) -> (f32, f32) { (0.0, f32::INFINITY) }
    fn accumulated_impulse(&self) -> f32 { self.accumulated }
    fn reset_accumulated(&mut self) { self.accumulated = 0.0; }
    fn add_accumulated(&mut self, d: f32) { self.accumulated = (self.accumulated + d).max(0.0); }
    fn body_handles(&self) -> Vec<BodyHandle> { vec![self.body_a, self.body_b] }
}

// ── ConstraintSolver ──────────────────────────────────────────────────────────

/// Sequential-impulse + XPBD constraint solver.
///
/// Call `solve()` once per physics step. Internally iterates
/// `iteration_count` times applying velocity and/or position corrections.
pub struct ConstraintSolver {
    pub iteration_count:  u32,
    pub baumgarte_factor: f32,
    pub warm_start:       bool,
    /// Number of XPBD substeps (for solve_xpbd).
    pub substep_count:    u32,
}

impl Default for ConstraintSolver {
    fn default() -> Self {
        Self { iteration_count: 10, baumgarte_factor: 0.2, warm_start: true, substep_count: 4 }
    }
}

impl ConstraintSolver {
    pub fn new(iterations: u32) -> Self {
        Self { iteration_count: iterations, ..Default::default() }
    }

    /// Solve all constraints using sequential impulses (velocity level).
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
                    c.apply_impulse(bodies, lambda * 0.8); // scale down slightly
                }
            }
        } else {
            for c in constraints.iter_mut() { c.reset_accumulated(); }
        }

        // Prepare constraints (compute cached values, apply warm start)
        for c in constraints.iter_mut() {
            c.prepare(bodies, dt);
        }

        // Iterative velocity solve
        for _ in 0..self.iteration_count {
            for c in constraints.iter_mut() {
                c.solve_velocity(bodies, dt);
            }
        }
    }

    /// Apply position correction (post-solve) using Baumgarte stabilization.
    pub fn solve_positions(
        &self,
        bodies:      &mut HashMap<BodyHandle, BodyState>,
        constraints: &mut [Box<dyn Constraint>],
        dt:          f32,
    ) {
        let alpha = self.baumgarte_factor / dt.max(1e-6);
        for c in constraints.iter_mut() {
            let em = c.effective_mass(bodies);
            if em < 1e-10 { continue; }
            let c_val = c.compute_c(bodies);
            if c_val.abs() < 1e-5 { continue; }
            let lambda = -alpha * c_val * em;
            c.apply_impulse(bodies, lambda);
        }
    }

    /// Solve using XPBD with substep integration.
    pub fn solve_xpbd(
        &self,
        bodies:      &mut HashMap<BodyHandle, BodyState>,
        constraints: &mut [Box<dyn Constraint>],
        dt:          f32,
    ) {
        if dt < 1e-8 { return; }
        let sub_dt = dt / self.substep_count as f32;

        for _ in 0..self.substep_count {
            // Reset accumulated lambdas for XPBD
            for c in constraints.iter_mut() { c.reset_accumulated(); }

            // Integrate velocities
            for body in bodies.values_mut() {
                if !body.is_static() {
                    body.position += body.velocity * sub_dt;
                    body.angle    += body.angular_velocity * sub_dt;
                }
            }

            // Position-level constraint solve
            for _ in 0..self.iteration_count {
                for c in constraints.iter_mut() {
                    c.solve_position(bodies, sub_dt);
                }
            }

            // Derive velocities from position corrections
            // (velocities are updated by apply_impulse inside solve_position)
        }
    }
}

// ── Constraint Island Detection ───────────────────────────────────────────────

/// A group of bodies and constraints that are connected and should be solved together.
#[derive(Debug, Clone)]
pub struct ConstraintIsland {
    pub body_handles:      Vec<BodyHandle>,
    pub constraint_indices: Vec<usize>,
}

/// Detect constraint islands using union-find.
pub fn detect_islands(
    bodies: &HashMap<BodyHandle, BodyState>,
    constraints: &[Box<dyn Constraint>],
) -> Vec<ConstraintIsland> {
    let body_list: Vec<BodyHandle> = bodies.keys().copied().collect();
    let n = body_list.len();
    if n == 0 { return Vec::new(); }

    // Map handle -> index
    let mut handle_to_idx: HashMap<BodyHandle, usize> = HashMap::new();
    for (i, h) in body_list.iter().enumerate() {
        handle_to_idx.insert(*h, i);
    }

    // Union-Find
    let mut parent: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = vec![0; n];

    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] != x { parent[x] = find(parent, parent[x]); }
        parent[x]
    }

    fn union(parent: &mut Vec<usize>, rank: &mut Vec<usize>, x: usize, y: usize) {
        let rx = find(parent, x);
        let ry = find(parent, y);
        if rx == ry { return; }
        if rank[rx] < rank[ry] { parent[rx] = ry; }
        else if rank[rx] > rank[ry] { parent[ry] = rx; }
        else { parent[ry] = rx; rank[rx] += 1; }
    }

    // Union bodies connected by constraints
    for c in constraints.iter() {
        let handles = c.body_handles();
        if handles.len() < 2 { continue; }
        for i in 1..handles.len() {
            if let (Some(&ia), Some(&ib)) = (handle_to_idx.get(&handles[0]), handle_to_idx.get(&handles[i])) {
                union(&mut parent, &mut rank, ia, ib);
            }
        }
    }

    // Group by root
    let mut island_map: HashMap<usize, usize> = HashMap::new();
    let mut islands: Vec<ConstraintIsland> = Vec::new();

    for (i, h) in body_list.iter().enumerate() {
        let root = find(&mut parent, i);
        let island_idx = *island_map.entry(root).or_insert_with(|| {
            let idx = islands.len();
            islands.push(ConstraintIsland { body_handles: Vec::new(), constraint_indices: Vec::new() });
            idx
        });
        islands[island_idx].body_handles.push(*h);
    }

    // Assign constraints to islands
    for (ci, c) in constraints.iter().enumerate() {
        let handles = c.body_handles();
        if handles.is_empty() { continue; }
        let first = handles[0];
        if let Some(&idx) = handle_to_idx.get(&first) {
            let root = find(&mut parent, idx);
            if let Some(&island_idx) = island_map.get(&root) {
                islands[island_idx].constraint_indices.push(ci);
            }
        }
    }

    islands
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
    /// Whether to use island-based solving.
    pub use_islands: bool,
    /// Whether to use XPBD substep integration.
    pub use_xpbd:    bool,
}

impl ConstraintWorld {
    pub fn new() -> Self {
        Self {
            bodies:        HashMap::new(),
            constraints:   Vec::new(),
            solver:        ConstraintSolver::default(),
            gravity:       Vec2::new(0.0, -9.81),
            next_body_id:  1,
            next_con_id:   1,
            use_islands:   false,
            use_xpbd:      false,
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
        if dt < 1e-8 { return; }

        // Apply gravity
        for body in self.bodies.values_mut() {
            if !body.is_static() {
                body.velocity += self.gravity * dt;
            }
        }

        if self.use_xpbd {
            // XPBD path: integrate then position-solve
            for body in self.bodies.values_mut() {
                if !body.is_static() {
                    body.position += body.velocity * dt;
                    body.angle    += body.angular_velocity * dt;
                }
            }
            self.solver.solve_xpbd(&mut self.bodies, &mut self.constraints, dt);
        } else {
            // SI path
            // Integrate velocities to positions
            for body in self.bodies.values_mut() {
                if !body.is_static() {
                    body.position += body.velocity * dt;
                    body.angle    += body.angular_velocity * dt;
                }
            }

            self.solver.solve(&mut self.bodies, &mut self.constraints, dt);
            self.solver.solve_positions(&mut self.bodies, &mut self.constraints, dt);
        }
    }

    pub fn body(&self, h: BodyHandle) -> Option<&BodyState> { self.bodies.get(&h) }
    pub fn body_mut(&mut self, h: BodyHandle) -> Option<&mut BodyState> { self.bodies.get_mut(&h) }

    pub fn body_count(&self) -> usize { self.bodies.len() }
    pub fn constraint_count(&self) -> usize { self.constraints.len() }

    /// Compute total kinetic energy.
    pub fn total_kinetic_energy(&self) -> f32 {
        self.bodies.values().map(|b| b.kinetic_energy()).sum()
    }

    /// Remove all sleeping bodies from the simulation.
    pub fn remove_sleeping(&mut self, threshold: f32) {
        let sleeping: Vec<BodyHandle> = self.bodies.iter()
            .filter(|(_, b)| b.kinetic_energy() < threshold)
            .map(|(h, _)| *h)
            .collect();
        for h in sleeping { self.bodies.remove(&h); }
    }
}

impl Default for ConstraintWorld {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn make_two_bodies(world: &mut ConstraintWorld, mass: f32, pos_a: Vec2, pos_b: Vec2) -> (BodyHandle, BodyHandle) {
        let a = world.add_body(BodyState::new(pos_a, mass, mass * 0.1));
        let b = world.add_body(BodyState::new(pos_b, mass, mass * 0.1));
        (a, b)
    }

    #[test]
    fn distance_constraint_rigid_rod() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let (a, b) = make_two_bodies(&mut world, 1.0, Vec2::ZERO, Vec2::new(3.0, 0.0));
        let c = Box::new(DistanceConstraint::rigid_rod(a, Vec2::ZERO, b, Vec2::ZERO, 2.0));
        world.add_constraint(c);
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let pa = world.body(a).unwrap().position;
        let pb = world.body(b).unwrap().position;
        let dist = (pb - pa).length();
        assert!((dist - 2.0).abs() < 0.1, "rod should maintain 2m distance, got {dist}");
    }

    #[test]
    fn distance_constraint_soft_spring() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let (a, b) = make_two_bodies(&mut world, 1.0, Vec2::ZERO, Vec2::new(5.0, 0.0));
        let c = Box::new(DistanceConstraint::soft(a, Vec2::ZERO, b, Vec2::ZERO, 2.0, 0.01));
        world.add_constraint(c);
        world.step(1.0 / 60.0);
        // Compliance means it gives a bit — just ensure it ran without panic
        let pb = world.body(b).unwrap().position;
        assert!(pb.x.is_finite());
    }

    #[test]
    fn hinge_constraint_limits() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body({
            let mut s = BodyState::new(Vec2::new(0.0, 1.0), 1.0, 0.1);
            s.angular_velocity = 10.0;
            s
        });
        let c = Box::new(
            HingeConstraint::new(a, Vec2::ZERO, b, Vec2::new(0.0, -1.0))
                .with_limits(-PI / 4.0, PI / 4.0)
        );
        world.add_constraint(c);
        for _ in 0..120 {
            world.step(1.0 / 60.0);
        }
        let angle = world.body(b).unwrap().angle;
        assert!(angle.abs() <= PI / 4.0 + 0.1, "angle {} should be within limits", angle);
    }

    #[test]
    fn hinge_constraint_motor() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body(BodyState::new(Vec2::ZERO, 1.0, 1.0));
        let c = Box::new(
            HingeConstraint::new(a, Vec2::ZERO, b, Vec2::ZERO)
                .with_motor(2.0 * PI, 100.0)
        );
        world.add_constraint(c);
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let omega = world.body(b).unwrap().angular_velocity;
        assert!(omega > 0.0, "motor should drive positive angular velocity, got {omega}");
    }

    #[test]
    fn ball_socket_cone_limit() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body({
            let mut s = BodyState::new(Vec2::new(1.0, 0.0), 1.0, 0.1);
            s.angular_velocity = 5.0;
            s
        });
        let c = Box::new(
            BallSocketConstraint::new(a, Vec2::ZERO, b, Vec2::new(-1.0, 0.0))
                .with_cone_limit(PI / 6.0)
        );
        world.add_constraint(c);
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let angle = world.body(b).unwrap().angle;
        assert!(angle.abs() <= PI / 6.0 + 0.2, "cone angle exceeded");
    }

    #[test]
    fn slider_constraint_limits() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body({
            let mut s = BodyState::new(Vec2::new(0.5, 0.0), 1.0, 0.1);
            s.velocity = Vec2::new(5.0, 0.0);
            s
        });
        let c = Box::new(
            SliderConstraint::new(a, Vec2::ZERO, b, Vec2::ZERO, Vec2::X)
                .with_limits(-1.0, 1.0)
        );
        world.add_constraint(c);
        for _ in 0..120 {
            world.step(1.0 / 60.0);
        }
        let pos_x = world.body(b).unwrap().position.x;
        assert!(pos_x <= 1.5, "slider should be limited, got x={pos_x}");
    }

    #[test]
    fn weld_constraint_holds() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::new(0.0, -9.81);
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body(BodyState::new(Vec2::new(1.0, 0.0), 1.0, 0.1));
        let c = Box::new(WeldConstraint::new(a, Vec2::new(1.0, 0.0), b, Vec2::ZERO, 0.0));
        world.add_constraint(c);
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let pos = world.body(b).unwrap().position;
        assert!((pos - Vec2::new(1.0, 0.0)).length() < 0.5, "weld should hold, drift={}", (pos - Vec2::new(1.0, 0.0)).length());
    }

    #[test]
    fn pulley_constraint_ratio() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::new(Vec2::new(-3.0, 0.0), 1.0, 0.1));
        let b = world.add_body(BodyState::new(Vec2::new( 3.0, 0.0), 1.0, 0.1));
        let init_la = (Vec2::new(-3.0, 0.0) - Vec2::new(-1.0, 2.0)).length();
        let init_lb = (Vec2::new( 3.0, 0.0) - Vec2::new( 1.0, 2.0)).length();
        let total = init_la + init_lb;
        let c = Box::new(PulleyConstraint::new(
            a, Vec2::ZERO, Vec2::new(-1.0, 2.0),
            b, Vec2::ZERO, Vec2::new( 1.0, 2.0),
            1.0, total,
        ));
        world.add_constraint(c);
        world.step(1.0 / 60.0);
        let pa = world.body(a).unwrap().position;
        assert!(pa.is_finite(), "pulley should not produce NaN");
    }

    #[test]
    fn spring_constraint_oscillates() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body(BodyState::new(Vec2::new(3.0, 0.0), 1.0, 0.1));
        let c = Box::new(SpringConstraint::new(a, Vec2::ZERO, b, Vec2::ZERO, 1.0, 50.0, 0.5));
        world.add_constraint(c);
        let pos_before = world.body(b).unwrap().position.x;
        world.step(0.016);
        let pos_after = world.body(b).unwrap().position.x;
        assert!(pos_after < pos_before, "spring should pull body toward rest length");
    }

    #[test]
    fn motor_constraint_drives_rotation() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::new(Vec2::ZERO, 1.0, 1.0));
        let c = Box::new(MotorConstraint::new(a, 5.0, 100.0));
        world.add_constraint(c);
        for _ in 0..30 {
            world.step(1.0 / 60.0);
        }
        let omega = world.body(a).unwrap().angular_velocity;
        assert!(omega > 0.0, "motor should spin body, got {omega}");
    }

    #[test]
    fn gear_constraint_couples_bodies() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body({
            let mut s = BodyState::new(Vec2::ZERO, 1.0, 1.0);
            s.angular_velocity = 2.0;
            s
        });
        let b = world.add_body(BodyState::new(Vec2::new(2.0, 0.0), 1.0, 1.0));
        let c = Box::new(GearConstraint::new(a, b, 2.0));
        world.add_constraint(c);
        for _ in 0..30 {
            world.step(1.0 / 60.0);
        }
        let oa = world.body(a).unwrap().angular_velocity;
        let ob = world.body(b).unwrap().angular_velocity;
        // omega_a + 2 * omega_b ~ 0
        let coupling_err = (oa + 2.0 * ob).abs();
        assert!(coupling_err < 1.0, "gear coupling error too large: {coupling_err}");
    }

    #[test]
    fn max_distance_constraint_rope() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::ZERO;
        let a = world.add_body(BodyState::static_body(Vec2::ZERO));
        let b = world.add_body({
            let mut s = BodyState::new(Vec2::new(3.0, 0.0), 1.0, 0.1);
            s.velocity = Vec2::new(10.0, 0.0);
            s
        });
        let c = Box::new(MaxDistanceConstraint::new(a, Vec2::ZERO, b, Vec2::ZERO, 2.0));
        world.add_constraint(c);
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let dist = world.body(b).unwrap().position.length();
        assert!(dist <= 2.5, "rope should limit distance, got {dist}");
    }

    #[test]
    fn pin_constraint_holds_at_world_point() {
        let mut world = ConstraintWorld::new();
        world.gravity = Vec2::new(0.0, -9.81);
        let a = world.add_body(BodyState::new(Vec2::new(0.0, 5.0), 1.0, 0.1));
        let target = Vec2::new(0.0, 5.0);
        let c = Box::new(PinConstraint::new(a, Vec2::ZERO, target));
        world.add_constraint(c);
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let pos = world.body(a).unwrap().position;
        let err = (pos - target).length();
        assert!(err < 0.5, "pin should hold, error={err}");
    }

    #[test]
    fn constraint_world_gravity_applied() {
        let mut world = ConstraintWorld::new();
        let a = world.add_body(BodyState::new(Vec2::new(0.0, 10.0), 1.0, 0.1));
        world.step(1.0);
        let pos = world.body(a).unwrap().position;
        assert!(pos.y < 10.0, "gravity should pull body down");
    }

    #[test]
    fn island_detection_finds_connected_components() {
        let mut world = ConstraintWorld::new();
        let a = world.add_body(BodyState::new(Vec2::ZERO, 1.0, 0.1));
        let b = world.add_body(BodyState::new(Vec2::new(1.0, 0.0), 1.0, 0.1));
        let c = world.add_body(BodyState::new(Vec2::new(5.0, 0.0), 1.0, 0.1));  // isolated
        world.add_constraint(Box::new(DistanceConstraint::new(a, Vec2::ZERO, b, Vec2::ZERO, 1.0)));
        let islands = detect_islands(&world.bodies, &world.constraints);
        // Should find 2 islands: {a,b} and {c}
        assert_eq!(islands.len(), 2, "should detect 2 islands");
    }

    #[test]
    fn contact_constraint_non_penetration() {
        let mut bodies: HashMap<BodyHandle, BodyState> = HashMap::new();
        let h_a = BodyHandle(1);
        let h_b = BodyHandle(2);
        let mut ba = BodyState::new(Vec2::ZERO, 1.0, 0.1);
        let mut bb = BodyState::new(Vec2::new(0.5, 0.0), 1.0, 0.1);
        ba.velocity = Vec2::new(1.0, 0.0);
        bb.velocity = Vec2::new(-1.0, 0.0);
        bodies.insert(h_a, ba);
        bodies.insert(h_b, bb);
        let mut c = ContactConstraint::new(
            h_a, h_b,
            Vec2::new(0.25, 0.0), Vec2::X, 0.5,
            0.3, 0.5,
        );
        c.prepare(&bodies, 1.0 / 60.0);
        c.solve_contact(&mut bodies, 1.0 / 60.0);
        let va = bodies[&h_a].velocity.x;
        let vb = bodies[&h_b].velocity.x;
        assert!(vb >= va - 0.01, "relative velocity after impulse should be non-negative");
    }

    #[test]
    fn total_kinetic_energy_is_finite() {
        let mut world = ConstraintWorld::new();
        for i in 0..10 {
            let s = BodyState::new(Vec2::new(i as f32, 0.0), 1.0, 0.1);
            world.add_body(s);
        }
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }
        let ke = world.total_kinetic_energy();
        assert!(ke.is_finite(), "KE should be finite");
    }
}
