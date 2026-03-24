//! Physics simulation — rigid bodies, constraints, collision, fluid dynamics.
//!
//! This module provides a simplified 2D/3D physics simulation layer for
//! Proof Engine. All physical objects are mathematical entities — their
//! motion is derived from differential equations integrated via RK4.
//!
//! ## Subsystems
//! - `RigidBody`   — point masses with velocity, acceleration, forces
//! - `Constraint`  — distance, angle, hinge, weld constraints
//! - `Collider`    — circle/box/capsule shapes for broad/narrow phase
//! - `PhysicsWorld`— simulation driver, constraint solver, broadphase
//! - `FluidSolver` — grid-based Eulerian fluid (velocity + pressure)
//! - `SoftBody`    — mass-spring soft body meshes
//!
//! All positions are in world-space float coordinates.

use glam::{Vec2, Vec3};

pub mod fluid;
pub mod soft_body;
pub mod rigid_body;
pub mod constraints;
pub mod joints;
pub mod fluids;

pub use joints::{Joint, JointType, Ragdoll, RagdollBone, CharacterController, JointSolver};

pub use fluid::FluidGrid;
pub use soft_body::SoftBody;

// ── RigidBody ─────────────────────────────────────────────────────────────────

/// A simulated rigid body (treated as a point mass for simplicity).
#[derive(Debug, Clone)]
pub struct RigidBody {
    pub id:              BodyId,
    pub position:        Vec2,
    pub velocity:        Vec2,
    pub acceleration:    Vec2,
    pub mass:            f32,
    pub inv_mass:        f32,   // 1/mass, 0 = static
    pub restitution:     f32,   // bounciness [0, 1]
    pub friction:        f32,   // surface friction [0, 1]
    pub damping:         f32,   // linear damping [0, 1]
    pub angle:           f32,   // rotation in radians
    pub angular_vel:     f32,
    pub angular_damp:    f32,
    pub inertia:         f32,   // moment of inertia
    pub inv_inertia:     f32,
    pub force_accum:     Vec2,  // accumulated forces for this step
    pub torque_accum:    f32,
    pub is_sleeping:     bool,
    pub collider:        Collider,
    /// User-provided tag for identification.
    pub tag:             u32,
    pub layer:           u8,
    pub mask:            u8,    // which layers to collide with
}

/// Unique body identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(pub u64);

impl RigidBody {
    /// Create a new dynamic body.
    pub fn dynamic(position: Vec2, mass: f32, collider: Collider) -> Self {
        let inertia = collider.compute_inertia(mass);
        Self {
            id:           BodyId(0),
            position,
            velocity:     Vec2::ZERO,
            acceleration: Vec2::ZERO,
            mass,
            inv_mass:     if mass > 0.0 { 1.0 / mass } else { 0.0 },
            restitution:  0.3,
            friction:     0.5,
            damping:      0.01,
            angle:        0.0,
            angular_vel:  0.0,
            angular_damp: 0.01,
            inertia,
            inv_inertia:  if inertia > 0.0 { 1.0 / inertia } else { 0.0 },
            force_accum:  Vec2::ZERO,
            torque_accum: 0.0,
            is_sleeping:  false,
            collider,
            tag:          0,
            layer:        1,
            mask:         0xFF,
        }
    }

    /// Create a static body (unmovable).
    pub fn static_body(position: Vec2, collider: Collider) -> Self {
        Self {
            inv_mass:     0.0,
            inv_inertia:  0.0,
            mass:         f32::INFINITY,
            inertia:      f32::INFINITY,
            ..Self::dynamic(position, 1.0, collider)
        }
    }

    /// Apply a force at the body's center of mass.
    pub fn apply_force(&mut self, force: Vec2) {
        if self.inv_mass > 0.0 {
            self.force_accum += force;
        }
    }

    /// Apply a force at a world-space point (generates torque if off-center).
    pub fn apply_force_at_point(&mut self, force: Vec2, point: Vec2) {
        self.apply_force(force);
        let r = point - self.position;
        self.torque_accum += r.x * force.y - r.y * force.x;
    }

    /// Apply an impulse (instantaneous velocity change).
    pub fn apply_impulse(&mut self, impulse: Vec2) {
        self.velocity += impulse * self.inv_mass;
    }

    /// Clear accumulated forces.
    pub fn clear_forces(&mut self) {
        self.force_accum  = Vec2::ZERO;
        self.torque_accum = 0.0;
    }

    /// Returns true if the body is static (infinite mass).
    pub fn is_static(&self) -> bool { self.inv_mass == 0.0 }

    /// Kinetic energy.
    pub fn kinetic_energy(&self) -> f32 {
        0.5 * self.mass * self.velocity.length_squared()
        + 0.5 * self.inertia * self.angular_vel * self.angular_vel
    }

    /// Advance the body by one semi-implicit Euler step.
    fn integrate(&mut self, dt: f32, gravity: Vec2) {
        if self.is_static() || self.is_sleeping { return; }

        // Linear
        let total_force = self.force_accum + gravity * self.mass;
        self.acceleration  = total_force * self.inv_mass;
        self.velocity     += self.acceleration * dt;
        self.velocity     *= 1.0 - self.damping * dt;
        self.position     += self.velocity * dt;

        // Angular
        let alpha         = self.torque_accum * self.inv_inertia;
        self.angular_vel += alpha * dt;
        self.angular_vel *= 1.0 - self.angular_damp * dt;
        self.angle       += self.angular_vel * dt;

        self.clear_forces();
    }

    /// Set body to sleep if velocity is very small.
    fn try_sleep(&mut self, sleep_threshold: f32) {
        if self.velocity.length_squared() < sleep_threshold * sleep_threshold
            && self.angular_vel.abs() < sleep_threshold
        {
            self.is_sleeping = true;
            self.velocity    = Vec2::ZERO;
            self.angular_vel = 0.0;
        }
    }

    /// Wake the body from sleep.
    pub fn wake(&mut self) { self.is_sleeping = false; }
}

// ── Collider ──────────────────────────────────────────────────────────────────

/// Collision shape for a body.
#[derive(Debug, Clone, Copy)]
pub enum Collider {
    Circle  { radius: f32 },
    Box     { half_w: f32, half_h: f32 },
    Capsule { radius: f32, half_height: f32 },
    /// Point collider (treated as infinitesimally small circle).
    Point,
}

impl Collider {
    pub fn circle(radius: f32) -> Self { Self::Circle { radius } }
    pub fn box_shape(w: f32, h: f32) -> Self { Self::Box { half_w: w * 0.5, half_h: h * 0.5 } }
    pub fn capsule(radius: f32, height: f32) -> Self { Self::Capsule { radius, half_height: height * 0.5 } }

    /// Compute the moment of inertia for this shape and mass.
    pub fn compute_inertia(&self, mass: f32) -> f32 {
        match self {
            Self::Circle { radius }      => 0.5 * mass * radius * radius,
            Self::Box { half_w, half_h } => mass * (half_w * half_w + half_h * half_h) / 3.0,
            Self::Capsule { radius, half_height } => {
                let r2 = radius * radius;
                let h2 = half_height * half_height;
                mass * (r2 + h2) / 3.0
            }
            Self::Point                  => 0.0,
        }
    }

    /// Approximate AABB (axis-aligned bounding box).
    pub fn aabb(&self) -> (Vec2, Vec2) {
        match self {
            Self::Circle { radius }      => (Vec2::splat(-radius), Vec2::splat(*radius)),
            Self::Box { half_w, half_h } => (Vec2::new(-half_w, -half_h), Vec2::new(*half_w, *half_h)),
            Self::Capsule { radius, half_height } => {
                (Vec2::new(-radius, -half_height - radius),
                 Vec2::new(*radius,  *half_height + radius))
            }
            Self::Point                  => (Vec2::ZERO, Vec2::ZERO),
        }
    }

    /// Bounding radius.
    pub fn bounding_radius(&self) -> f32 {
        match self {
            Self::Circle { radius }        => *radius,
            Self::Box { half_w, half_h }   => half_w.hypot(*half_h),
            Self::Capsule { radius, half_height } => radius + half_height,
            Self::Point                    => 0.0,
        }
    }
}

// ── Collision detection ───────────────────────────────────────────────────────

/// Result of a collision test between two bodies.
#[derive(Debug, Clone)]
pub struct ContactManifold {
    pub body_a:  BodyId,
    pub body_b:  BodyId,
    /// Penetration depth (positive = overlap).
    pub depth:   f32,
    /// Normal pointing from A to B.
    pub normal:  Vec2,
    /// Contact point in world space.
    pub point:   Vec2,
}

/// Test circle vs circle.
pub fn circle_circle(
    pos_a: Vec2, ra: f32,
    pos_b: Vec2, rb: f32,
    id_a: BodyId, id_b: BodyId,
) -> Option<ContactManifold> {
    let delta  = pos_b - pos_a;
    let dist   = delta.length();
    let sum_r  = ra + rb;
    if dist >= sum_r || dist < 1e-8 { return None; }
    let normal = if dist > 0.0 { delta / dist } else { Vec2::X };
    Some(ContactManifold {
        body_a: id_a, body_b: id_b,
        depth:  sum_r - dist,
        normal,
        point:  pos_a + normal * ra,
    })
}

/// Test AABB vs AABB (using bounding boxes).
pub fn aabb_aabb(
    pos_a: Vec2, (min_a, max_a): (Vec2, Vec2),
    pos_b: Vec2, (min_b, max_b): (Vec2, Vec2),
    id_a: BodyId, id_b: BodyId,
) -> Option<ContactManifold> {
    let a_min = pos_a + min_a;
    let a_max = pos_a + max_a;
    let b_min = pos_b + min_b;
    let b_max = pos_b + max_b;

    if a_max.x < b_min.x || b_max.x < a_min.x { return None; }
    if a_max.y < b_min.y || b_max.y < a_min.y { return None; }

    // Find minimum penetration axis
    let dx = (a_max.x.min(b_max.x) - a_min.x.max(b_min.x)).abs();
    let dy = (a_max.y.min(b_max.y) - a_min.y.max(b_min.y)).abs();
    let (depth, normal) = if dx < dy {
        let sign = if pos_a.x < pos_b.x { -1.0 } else { 1.0 };
        (dx, Vec2::new(sign, 0.0))
    } else {
        let sign = if pos_a.y < pos_b.y { -1.0 } else { 1.0 };
        (dy, Vec2::new(0.0, sign))
    };

    Some(ContactManifold {
        body_a: id_a, body_b: id_b, depth, normal,
        point: (pos_a + pos_b) * 0.5,
    })
}

/// Resolve a contact manifold by applying impulses to both bodies.
pub fn resolve_contact(a: &mut RigidBody, b: &mut RigidBody, manifold: &ContactManifold) {
    // Relative velocity at contact point
    let rel_vel = b.velocity - a.velocity;
    let vel_along_normal = rel_vel.dot(manifold.normal);

    // Don't resolve if separating
    if vel_along_normal > 0.0 { return; }

    let e         = a.restitution.min(b.restitution);
    let j_scalar  = -(1.0 + e) * vel_along_normal / (a.inv_mass + b.inv_mass);
    let impulse   = manifold.normal * j_scalar;

    a.velocity -= impulse * a.inv_mass;
    b.velocity += impulse * b.inv_mass;

    // Positional correction (Baumgarte)
    const SLOP:  f32 = 0.01;
    const BIAS:  f32 = 0.2;
    let correction = (manifold.depth - SLOP).max(0.0) * BIAS
                   / (a.inv_mass + b.inv_mass);
    let correction_v = manifold.normal * correction;
    if !a.is_static() { a.position -= correction_v * a.inv_mass; }
    if !b.is_static() { b.position += correction_v * b.inv_mass; }
}

// ── Constraints ───────────────────────────────────────────────────────────────

/// A physical constraint between two bodies.
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Maintain a fixed distance between two bodies.
    Distance {
        body_a: BodyId,
        body_b: BodyId,
        rest_length: f32,
        stiffness:   f32,
        damping:     f32,
    },
    /// Spring constraint (distance with Hooke's law).
    Spring {
        body_a:    BodyId,
        body_b:    BodyId,
        rest_len:  f32,
        k:         f32,    // spring constant
        c:         f32,    // damping
    },
    /// Pin one body to a world-space position.
    Pin {
        body:     BodyId,
        target:   Vec2,
        stiffness: f32,
    },
    /// Keep body within an axis-aligned box.
    Boundary {
        body: BodyId,
        min:  Vec2,
        max:  Vec2,
    },
    /// Angular constraint (limit rotation range).
    AngleLimit {
        body:       BodyId,
        min_angle:  f32,
        max_angle:  f32,
        stiffness:  f32,
    },
}

impl Constraint {
    /// Apply the constraint by modifying body forces/velocities.
    pub fn apply(&self, bodies: &mut [RigidBody]) {
        match self {
            Constraint::Spring { body_a, body_b, rest_len, k, c } => {
                let (idx_a, idx_b) = find_pair_indices(bodies, *body_a, *body_b);
                if let (Some(ia), Some(ib)) = (idx_a, idx_b) {
                    // Split borrow
                    let (left, right) = bodies.split_at_mut(ia.max(ib));
                    let (a, b) = if ia < ib {
                        (&mut left[ia], &mut right[0])
                    } else {
                        (&mut right[0], &mut left[ib])
                    };
                    let delta  = b.position - a.position;
                    let dist   = delta.length();
                    if dist < 1e-6 { return; }
                    let dir    = delta / dist;
                    let spring = *k * (dist - rest_len);
                    let damp   = *c * (b.velocity - a.velocity).dot(dir);
                    let force  = (spring + damp) * dir;
                    a.apply_force(force);
                    b.apply_force(-force);
                }
            }
            Constraint::Distance { body_a, body_b, rest_length, stiffness, damping } => {
                let (idx_a, idx_b) = find_pair_indices(bodies, *body_a, *body_b);
                if let (Some(ia), Some(ib)) = (idx_a, idx_b) {
                    let (left, right) = bodies.split_at_mut(ia.max(ib));
                    let (a, b) = if ia < ib {
                        (&mut left[ia], &mut right[0])
                    } else {
                        (&mut right[0], &mut left[ib])
                    };
                    let delta  = b.position - a.position;
                    let dist   = delta.length();
                    if dist < 1e-6 { return; }
                    let dir    = delta / dist;
                    let err    = dist - rest_length;
                    let damp   = *damping * (b.velocity - a.velocity).dot(dir);
                    let force  = (err * stiffness + damp) * dir;
                    a.apply_force( force);
                    b.apply_force(-force);
                }
            }
            Constraint::Pin { body, target, stiffness } => {
                if let Some(b) = bodies.iter_mut().find(|b| b.id == *body) {
                    let delta = *target - b.position;
                    b.apply_force(delta * *stiffness);
                }
            }
            Constraint::Boundary { body, min, max } => {
                if let Some(b) = bodies.iter_mut().find(|b| b.id == *body) {
                    if b.position.x < min.x { b.position.x = min.x; b.velocity.x = b.velocity.x.abs(); }
                    if b.position.x > max.x { b.position.x = max.x; b.velocity.x = -b.velocity.x.abs(); }
                    if b.position.y < min.y { b.position.y = min.y; b.velocity.y = b.velocity.y.abs(); }
                    if b.position.y > max.y { b.position.y = max.y; b.velocity.y = -b.velocity.y.abs(); }
                }
            }
            Constraint::AngleLimit { body, min_angle, max_angle, stiffness } => {
                if let Some(b) = bodies.iter_mut().find(|b| b.id == *body) {
                    if b.angle < *min_angle {
                        let err = min_angle - b.angle;
                        b.torque_accum += err * stiffness;
                    } else if b.angle > *max_angle {
                        let err = max_angle - b.angle;
                        b.torque_accum += err * stiffness;
                    }
                }
            }
        }
    }
}

fn find_pair_indices(bodies: &[RigidBody], a: BodyId, b: BodyId) -> (Option<usize>, Option<usize>) {
    let ia = bodies.iter().position(|bod| bod.id == a);
    let ib = bodies.iter().position(|bod| bod.id == b);
    (ia, ib)
}

// ── PhysicsWorld ──────────────────────────────────────────────────────────────

/// The physics simulation world.
///
/// Maintains a list of bodies and constraints, steps them each frame,
/// performs collision detection and resolution.
pub struct PhysicsWorld {
    bodies:          Vec<RigidBody>,
    constraints:     Vec<Constraint>,
    pub gravity:     Vec2,
    pub substeps:    u32,
    pub sleep_threshold: f32,
    next_id:         u64,
    /// Collision events from last step.
    contacts:        Vec<ContactManifold>,
    /// Time accumulated for fixed-step integration.
    accumulator:     f32,
    pub fixed_dt:    f32,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            bodies:          Vec::new(),
            constraints:     Vec::new(),
            gravity:         Vec2::new(0.0, -9.81),
            substeps:        4,
            sleep_threshold: 0.01,
            next_id:         1,
            contacts:        Vec::new(),
            accumulator:     0.0,
            fixed_dt:        1.0 / 120.0,
        }
    }

    /// Spawn a body and return its ID.
    pub fn add_body(&mut self, mut body: RigidBody) -> BodyId {
        let id    = BodyId(self.next_id);
        self.next_id += 1;
        body.id   = id;
        self.bodies.push(body);
        id
    }

    /// Remove a body by ID.
    pub fn remove_body(&mut self, id: BodyId) {
        self.bodies.retain(|b| b.id != id);
    }

    /// Add a constraint.
    pub fn add_constraint(&mut self, c: Constraint) {
        self.constraints.push(c);
    }

    /// Get a body by ID (shared reference).
    pub fn body(&self, id: BodyId) -> Option<&RigidBody> {
        self.bodies.iter().find(|b| b.id == id)
    }

    /// Get a body by ID (mutable reference).
    pub fn body_mut(&mut self, id: BodyId) -> Option<&mut RigidBody> {
        self.bodies.iter_mut().find(|b| b.id == id)
    }

    /// Apply gravity from Vec3 (ignores Z).
    pub fn set_gravity_3d(&mut self, g: Vec3) {
        self.gravity = Vec2::new(g.x, g.y);
    }

    /// Advance the simulation by `dt` seconds, using fixed sub-steps.
    pub fn step(&mut self, dt: f32) {
        self.accumulator += dt;
        while self.accumulator >= self.fixed_dt {
            let step_dt = self.fixed_dt / self.substeps as f32;
            for _ in 0..self.substeps {
                self.fixed_step(step_dt);
            }
            self.accumulator -= self.fixed_dt;
        }
    }

    fn fixed_step(&mut self, dt: f32) {
        // Apply constraints (generates forces)
        let constraints = self.constraints.clone();
        for c in &constraints {
            c.apply(&mut self.bodies);
        }

        // Integrate bodies
        let gravity = self.gravity;
        for body in &mut self.bodies {
            body.integrate(dt, gravity);
        }

        // Broad phase + narrow phase collision
        self.contacts.clear();
        let n = self.bodies.len();
        for i in 0..n {
            for j in (i + 1)..n {
                // Layer mask check
                if self.bodies[i].layer & self.bodies[j].mask == 0
                && self.bodies[j].layer & self.bodies[i].mask == 0 {
                    continue;
                }
                // Skip both static
                if self.bodies[i].is_static() && self.bodies[j].is_static() { continue; }

                let pa = self.bodies[i].position;
                let pb = self.bodies[j].position;
                let ca = self.bodies[i].collider;
                let cb = self.bodies[j].collider;
                let id_a = self.bodies[i].id;
                let id_b = self.bodies[j].id;

                // Circle vs Circle fast path
                let contact = match (ca, cb) {
                    (Collider::Circle { radius: ra }, Collider::Circle { radius: rb }) => {
                        circle_circle(pa, ra, pb, rb, id_a, id_b)
                    }
                    _ => {
                        // Fall back to AABB
                        aabb_aabb(pa, ca.aabb(), pb, cb.aabb(), id_a, id_b)
                    }
                };

                if let Some(manifold) = contact {
                    self.contacts.push(manifold);
                }
            }
        }

        // Resolve contacts
        let contacts = self.contacts.clone();
        for contact in &contacts {
            let ia = self.bodies.iter().position(|b| b.id == contact.body_a);
            let ib = self.bodies.iter().position(|b| b.id == contact.body_b);
            if let (Some(ia), Some(ib)) = (ia, ib) {
                let (left, right) = self.bodies.split_at_mut(ia.max(ib));
                let (a, b) = if ia < ib {
                    (&mut left[ia], &mut right[0])
                } else {
                    (&mut right[0], &mut left[ib])
                };
                resolve_contact(a, b, contact);
            }
        }

        // Sleep pass
        let threshold = self.sleep_threshold;
        for body in &mut self.bodies {
            body.try_sleep(threshold);
        }
    }

    /// Returns all contact manifolds from the last step.
    pub fn contacts(&self) -> &[ContactManifold] {
        &self.contacts
    }

    /// Returns all body positions.
    pub fn positions(&self) -> Vec<(BodyId, Vec2)> {
        self.bodies.iter().map(|b| (b.id, b.position)).collect()
    }

    pub fn body_count(&self) -> usize { self.bodies.len() }
}

impl Default for PhysicsWorld {
    fn default() -> Self { Self::new() }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Verlet integration step (position + velocity-Verlet).
/// More accurate than Euler for conservative forces.
pub fn verlet_step(pos: Vec2, vel: Vec2, acc: Vec2, dt: f32) -> (Vec2, Vec2) {
    let new_pos = pos + vel * dt + acc * (0.5 * dt * dt);
    // acc_new would be recalculated from forces at new_pos in a full verlet
    let new_vel = vel + acc * dt;
    (new_pos, new_vel)
}

/// Runge-Kutta 4 for a 2D position given a force function.
pub fn rk4_2d<F>(pos: Vec2, vel: Vec2, mass: f32, dt: f32, force_fn: F) -> (Vec2, Vec2)
where
    F: Fn(Vec2, Vec2) -> Vec2
{
    let inv_m = 1.0 / mass.max(1e-8);
    let k1_v  = force_fn(pos, vel) * inv_m;
    let k1_p  = vel;

    let k2_v  = force_fn(pos + k1_p * (dt * 0.5), vel + k1_v * (dt * 0.5)) * inv_m;
    let k2_p  = vel + k1_v * (dt * 0.5);

    let k3_v  = force_fn(pos + k2_p * (dt * 0.5), vel + k2_v * (dt * 0.5)) * inv_m;
    let k3_p  = vel + k2_v * (dt * 0.5);

    let k4_v  = force_fn(pos + k3_p * dt, vel + k3_v * dt) * inv_m;
    let k4_p  = vel + k3_v * dt;

    let new_vel = vel + (k1_v + k2_v * 2.0 + k3_v * 2.0 + k4_v) * (dt / 6.0);
    let new_pos = pos + (k1_p + k2_p * 2.0 + k3_p * 2.0 + k4_p) * (dt / 6.0);
    (new_pos, new_vel)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_falls_under_gravity() {
        let mut world = PhysicsWorld::new();
        let id = world.add_body(RigidBody::dynamic(Vec2::ZERO, 1.0, Collider::circle(0.5)));
        world.step(0.5);
        let body = world.body(id).unwrap();
        assert!(body.position.y < 0.0, "body should fall: y={}", body.position.y);
    }

    #[test]
    fn static_body_does_not_move() {
        let mut world = PhysicsWorld::new();
        let id = world.add_body(RigidBody::static_body(Vec2::new(0.0, -5.0), Collider::box_shape(10.0, 1.0)));
        world.step(1.0);
        let body = world.body(id).unwrap();
        assert_eq!(body.position, Vec2::new(0.0, -5.0));
    }

    #[test]
    fn spring_constraint_attracts() {
        let mut world = PhysicsWorld::new();
        let id_a = world.add_body(RigidBody::dynamic(Vec2::new(-2.0, 0.0), 1.0, Collider::circle(0.1)));
        let id_b = world.add_body(RigidBody::dynamic(Vec2::new( 2.0, 0.0), 1.0, Collider::circle(0.1)));
        world.gravity = Vec2::ZERO;
        world.add_constraint(Constraint::Spring { body_a: id_a, body_b: id_b, rest_len: 1.0, k: 10.0, c: 1.0 });
        let dist_before = 4.0;
        world.step(0.1);
        let pa = world.body(id_a).unwrap().position;
        let pb = world.body(id_b).unwrap().position;
        let dist_after = (pb - pa).length();
        assert!(dist_after < dist_before, "spring should pull bodies closer");
    }

    #[test]
    fn circle_collision_detected() {
        let a = circle_circle(Vec2::ZERO, 1.0, Vec2::new(1.5, 0.0), 1.0, BodyId(1), BodyId(2));
        assert!(a.is_some(), "circles should overlap");
        let b = circle_circle(Vec2::ZERO, 1.0, Vec2::new(3.0, 0.0), 1.0, BodyId(1), BodyId(2));
        assert!(b.is_none(), "circles should not overlap");
    }
}
