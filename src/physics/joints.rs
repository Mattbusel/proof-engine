//! 3D Physics joints, rigid bodies, collision detection, broadphase, ray casting,
//! impulse resolution, and sleep system.
//!
//! ## Components
//! - `RigidBody`          — full 3D rigid body with Quat orientation, Mat3 inertia tensor
//! - `RigidBodyHandle`    — newtype u32 handle
//! - `PhysicsWorld3D`     — arena of RigidBody, gravity, timestep, broadphase, `step()`
//! - `CollisionShape`     — Sphere, Box, Capsule, ConvexHull
//! - `CollisionDetector`  — sphere-sphere, sphere-box, box-box (SAT), capsule-capsule
//! - `Broadphase`         — sweep-and-prune on X-axis
//! - `RayCast`            — ray vs shape intersection
//! - `ImpulseResolver`    — sequential impulse with friction cone, restitution, warm-start
//! - `SleepSystem`        — kinetic energy threshold, sleep counter, island wake propagation

use glam::{Vec2, Vec3, Quat, Mat3};
use std::collections::HashMap;

// ── RigidBodyHandle ────────────────────────────────────────────────────────────

/// Opaque handle into a PhysicsWorld3D body arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RigidBodyHandle(pub u32);

// ── SleepState ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SleepState {
    Awake,
    Drowsy { timer: f32 },
    Asleep,
}

// ── RigidBody ─────────────────────────────────────────────────────────────────

/// Full 3D rigid body.
#[derive(Debug, Clone)]
pub struct RigidBody {
    /// World-space position.
    pub position:       Vec3,
    /// Linear velocity.
    pub velocity:       Vec3,
    /// Angular velocity (world space, in rad/s around each axis).
    pub angular_vel:    Vec3,
    /// Orientation quaternion.
    pub orientation:    Quat,
    /// Total mass (kg). 0 = static.
    pub mass:           f32,
    /// Inverse mass (0 = static).
    pub inv_mass:       f32,
    /// Inverse inertia tensor in local body space.
    pub inv_inertia_local: Mat3,
    /// Accumulated forces this step (world space).
    pub force_accum:    Vec3,
    /// Accumulated torques this step (world space).
    pub torque_accum:   Vec3,
    /// Coefficient of restitution.
    pub restitution:    f32,
    /// Friction coefficient.
    pub friction:       f32,
    /// Linear damping (0–1).
    pub linear_damping: f32,
    /// Angular damping (0–1).
    pub angular_damping: f32,
    /// Sleep / drowsy / awake state.
    pub sleep_state:    SleepState,
    /// Collision group mask.
    pub collision_group: u32,
    /// Which groups this body collides with.
    pub collision_mask:  u32,
    /// User tag.
    pub tag:            u32,
    /// Shape.
    pub shape:          CollisionShape,
}

impl RigidBody {
    /// Create a dynamic rigid body.
    pub fn dynamic(position: Vec3, mass: f32, shape: CollisionShape) -> Self {
        let inv_inertia_local = shape.compute_inverse_inertia_tensor(mass);
        Self {
            position,
            velocity:         Vec3::ZERO,
            angular_vel:      Vec3::ZERO,
            orientation:      Quat::IDENTITY,
            mass,
            inv_mass:         if mass > 0.0 { 1.0 / mass } else { 0.0 },
            inv_inertia_local,
            force_accum:      Vec3::ZERO,
            torque_accum:     Vec3::ZERO,
            restitution:      0.3,
            friction:         0.5,
            linear_damping:   0.01,
            angular_damping:  0.05,
            sleep_state:      SleepState::Awake,
            collision_group:  1,
            collision_mask:   u32::MAX,
            tag:              0,
            shape,
        }
    }

    /// Create a static rigid body (inv_mass = 0, won't move).
    pub fn static_body(position: Vec3, shape: CollisionShape) -> Self {
        Self {
            inv_mass:           0.0,
            mass:               f32::INFINITY,
            inv_inertia_local:  Mat3::ZERO,
            ..Self::dynamic(position, 1.0, shape)
        }
    }

    pub fn is_static(&self) -> bool { self.inv_mass < 1e-12 }
    pub fn is_sleeping(&self) -> bool { matches!(self.sleep_state, SleepState::Asleep) }

    /// Wake the body.
    pub fn wake(&mut self) {
        self.sleep_state = SleepState::Awake;
    }

    /// Apply a force at center of mass.
    pub fn apply_force(&mut self, force: Vec3) {
        if !self.is_static() { self.force_accum += force; }
    }

    /// Apply a torque.
    pub fn apply_torque(&mut self, torque: Vec3) {
        if !self.is_static() { self.torque_accum += torque; }
    }

    /// Apply force at a world-space point (generates torque).
    pub fn apply_force_at_point(&mut self, force: Vec3, world_point: Vec3) {
        self.apply_force(force);
        let r = world_point - self.position;
        self.apply_torque(r.cross(force));
    }

    /// Apply an instantaneous linear impulse.
    pub fn apply_linear_impulse(&mut self, impulse: Vec3) {
        self.velocity += impulse * self.inv_mass;
    }

    /// Apply an instantaneous angular impulse.
    pub fn apply_angular_impulse(&mut self, impulse: Vec3) {
        let iw = self.inv_inertia_world();
        self.angular_vel += iw * impulse;
    }

    /// Apply an impulse at a world-space point.
    pub fn apply_impulse_at_point(&mut self, impulse: Vec3, world_point: Vec3) {
        self.apply_linear_impulse(impulse);
        let r = world_point - self.position;
        self.apply_angular_impulse(r.cross(impulse));
    }

    /// Clear accumulated forces and torques.
    pub fn clear_forces(&mut self) {
        self.force_accum  = Vec3::ZERO;
        self.torque_accum = Vec3::ZERO;
    }

    /// Inverse inertia tensor in world space.
    pub fn inv_inertia_world(&self) -> Mat3 {
        let rot = Mat3::from_quat(self.orientation);
        let rot_t = rot.transpose();
        rot * self.inv_inertia_local * rot_t
    }

    /// Velocity at a world-space point on the body.
    pub fn velocity_at_point(&self, world_point: Vec3) -> Vec3 {
        let r = world_point - self.position;
        self.velocity + self.angular_vel.cross(r)
    }

    /// Semi-implicit Euler integration.
    pub fn integrate(&mut self, dt: f32, gravity: Vec3) {
        if self.is_static() || self.is_sleeping() { return; }

        // Linear integration
        let accel = self.force_accum * self.inv_mass + gravity;
        self.velocity += accel * dt;
        self.velocity *= (1.0 - self.linear_damping * dt).max(0.0);
        self.position += self.velocity * dt;

        // Angular integration
        let iw = self.inv_inertia_world();
        let alpha = iw * self.torque_accum;
        self.angular_vel += alpha * dt;
        self.angular_vel *= (1.0 - self.angular_damping * dt).max(0.0);

        // Update orientation
        let half_omega = self.angular_vel * 0.5;
        let dq = Quat::from_xyzw(half_omega.x, half_omega.y, half_omega.z, 0.0) * self.orientation;
        self.orientation = (self.orientation + dq * dt).normalize();

        self.clear_forces();
    }

    /// Kinetic energy (linear + rotational).
    pub fn kinetic_energy(&self) -> f32 {
        let linear = 0.5 * self.mass * self.velocity.length_squared();
        let rot_mat = Mat3::from_quat(self.orientation);
        let inertia_local = pseudo_inverse_mat3(self.inv_inertia_local);
        let ang_local = rot_mat.transpose() * self.angular_vel;
        let rotational = 0.5 * ang_local.dot(inertia_local * ang_local);
        linear + rotational
    }
}

/// Approximate inverse of a diagonal-ish inertia tensor.
fn pseudo_inverse_mat3(m: Mat3) -> Mat3 {
    // For diagonal matrices: invert each diagonal element
    let d = Vec3::new(m.x_axis.x, m.y_axis.y, m.z_axis.z);
    Mat3::from_diagonal(Vec3::new(
        if d.x.abs() > 1e-10 { 1.0 / d.x } else { 0.0 },
        if d.y.abs() > 1e-10 { 1.0 / d.y } else { 0.0 },
        if d.z.abs() > 1e-10 { 1.0 / d.z } else { 0.0 },
    ))
}

// ── CollisionShape ────────────────────────────────────────────────────────────

/// Collision shapes supported by the physics engine.
#[derive(Debug, Clone)]
pub enum CollisionShape {
    Sphere  { radius: f32 },
    Box     { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
    ConvexHull(Vec<Vec3>),
}

impl CollisionShape {
    pub fn sphere(radius: f32) -> Self { Self::Sphere { radius } }
    pub fn box_shape(half: Vec3) -> Self { Self::Box { half_extents: half } }
    pub fn capsule(radius: f32, half_height: f32) -> Self { Self::Capsule { radius, half_height } }

    /// Compute the inverse inertia tensor for this shape and mass.
    pub fn compute_inverse_inertia_tensor(&self, mass: f32) -> Mat3 {
        if mass < 1e-12 { return Mat3::ZERO; }
        let inv_m = 1.0 / mass;
        match self {
            Self::Sphere { radius } => {
                let i = 2.0 / 5.0 * mass * radius * radius;
                Mat3::from_diagonal(Vec3::splat(1.0 / i))
            }
            Self::Box { half_extents: h } => {
                let ix = mass * (h.y * h.y + h.z * h.z) / 3.0;
                let iy = mass * (h.x * h.x + h.z * h.z) / 3.0;
                let iz = mass * (h.x * h.x + h.y * h.y) / 3.0;
                Mat3::from_diagonal(Vec3::new(
                    if ix > 1e-10 { 1.0 / ix } else { 0.0 },
                    if iy > 1e-10 { 1.0 / iy } else { 0.0 },
                    if iz > 1e-10 { 1.0 / iz } else { 0.0 },
                ))
            }
            Self::Capsule { radius, half_height } => {
                let r2 = radius * radius;
                let h2 = half_height * half_height;
                let iy = mass * (r2 * 2.0 / 5.0 + h2 / 3.0 + r2 / 2.0);
                let ixz = mass * (r2 * 2.0 / 5.0 + h2 / 3.0);
                Mat3::from_diagonal(Vec3::new(
                    if ixz > 1e-10 { 1.0 / ixz } else { 0.0 },
                    if iy  > 1e-10 { 1.0 / iy  } else { 0.0 },
                    if ixz > 1e-10 { 1.0 / ixz } else { 0.0 },
                ))
            }
            Self::ConvexHull(pts) => {
                if pts.is_empty() { return Mat3::ZERO; }
                // Approximate with bounding box
                let mut mn = Vec3::splat(f32::INFINITY);
                let mut mx = Vec3::splat(f32::NEG_INFINITY);
                for p in pts {
                    mn = mn.min(*p);
                    mx = mx.max(*p);
                }
                let h = (mx - mn) * 0.5;
                let ix = mass * (h.y * h.y + h.z * h.z) / 3.0;
                let iy = mass * (h.x * h.x + h.z * h.z) / 3.0;
                let iz = mass * (h.x * h.x + h.y * h.y) / 3.0;
                Mat3::from_diagonal(Vec3::new(
                    if ix > 1e-10 { 1.0 / ix } else { 0.0 },
                    if iy > 1e-10 { 1.0 / iy } else { 0.0 },
                    if iz > 1e-10 { 1.0 / iz } else { 0.0 },
                ))
            }
        }
    }

    /// Bounding radius.
    pub fn bounding_radius(&self) -> f32 {
        match self {
            Self::Sphere { radius }        => *radius,
            Self::Box { half_extents }     => half_extents.length(),
            Self::Capsule { radius, half_height } => radius + half_height,
            Self::ConvexHull(pts) => pts.iter().map(|p| p.length()).fold(0.0_f32, f32::max),
        }
    }

    /// AABB half-extents.
    pub fn aabb_half_extents(&self) -> Vec3 {
        match self {
            Self::Sphere { radius }        => Vec3::splat(*radius),
            Self::Box { half_extents }     => *half_extents,
            Self::Capsule { radius, half_height } => Vec3::new(*radius, half_height + radius, *radius),
            Self::ConvexHull(pts) => {
                let mut mx = Vec3::ZERO;
                for p in pts { mx = mx.max(p.abs()); }
                mx
            }
        }
    }
}

// ── ContactPoint ──────────────────────────────────────────────────────────────

/// A single contact point in a collision manifold.
#[derive(Debug, Clone, Copy)]
pub struct ContactPoint {
    /// Contact point in world space.
    pub point:          Vec3,
    /// Contact normal (from A toward B).
    pub normal:         Vec3,
    /// Penetration depth (positive = overlap).
    pub depth:          f32,
    /// Cached normal impulse (warm starting).
    pub cached_normal:  f32,
    /// Cached tangent impulses (friction, warm starting).
    pub cached_tangent: [f32; 2],
}

impl ContactPoint {
    pub fn new(point: Vec3, normal: Vec3, depth: f32) -> Self {
        Self { point, normal, depth, cached_normal: 0.0, cached_tangent: [0.0; 2] }
    }
}

/// Result of collision detection between two bodies.
#[derive(Debug, Clone)]
pub struct ContactManifold3D {
    pub handle_a:  RigidBodyHandle,
    pub handle_b:  RigidBodyHandle,
    /// Up to 4 contact points.
    pub contacts:  Vec<ContactPoint>,
}

impl ContactManifold3D {
    pub fn new(a: RigidBodyHandle, b: RigidBodyHandle) -> Self {
        Self { handle_a: a, handle_b: b, contacts: Vec::with_capacity(4) }
    }

    pub fn add_contact(&mut self, pt: ContactPoint) {
        if self.contacts.len() < 4 {
            self.contacts.push(pt);
        }
    }
}

// ── CollisionDetector ─────────────────────────────────────────────────────────

/// Narrow-phase collision detection.
pub struct CollisionDetector;

impl CollisionDetector {
    /// Sphere vs sphere.
    pub fn sphere_sphere(
        pos_a: Vec3, ra: f32, ha: RigidBodyHandle,
        pos_b: Vec3, rb: f32, hb: RigidBodyHandle,
    ) -> Option<ContactManifold3D> {
        let delta = pos_b - pos_a;
        let dist  = delta.length();
        let sum_r = ra + rb;
        if dist >= sum_r || dist < 1e-8 { return None; }
        let normal = if dist > 1e-7 { delta / dist } else { Vec3::Y };
        let mut m = ContactManifold3D::new(ha, hb);
        m.add_contact(ContactPoint::new(
            pos_a + normal * ra,
            normal,
            sum_r - dist,
        ));
        Some(m)
    }

    /// Sphere vs axis-aligned box (fallback uses sphere-point clamping).
    pub fn sphere_box(
        sphere_pos: Vec3, sphere_r: f32, hs: RigidBodyHandle,
        box_pos: Vec3, box_rot: Quat, box_half: Vec3, hb: RigidBodyHandle,
    ) -> Option<ContactManifold3D> {
        // Transform sphere center into box local space
        let box_inv_rot = box_rot.inverse();
        let local_sphere = box_inv_rot * (sphere_pos - box_pos);

        // Clamp to box extents
        let clamped = local_sphere.clamp(-box_half, box_half);
        let local_delta = local_sphere - clamped;
        let dist = local_delta.length();

        if dist >= sphere_r { return None; }

        // If sphere center is inside box
        if dist < 1e-8 {
            // Find the closest face normal
            let overlap = box_half - local_sphere.abs();
            let min_overlap_axis = if overlap.x < overlap.y && overlap.x < overlap.z {
                Vec3::X * local_sphere.x.signum()
            } else if overlap.y < overlap.z {
                Vec3::Y * local_sphere.y.signum()
            } else {
                Vec3::Z * local_sphere.z.signum()
            };
            let world_normal = box_rot * min_overlap_axis;
            let penetration = overlap.min_element() + sphere_r;
            let mut m = ContactManifold3D::new(hs, hb);
            m.add_contact(ContactPoint::new(sphere_pos - world_normal * sphere_r, world_normal, penetration));
            return Some(m);
        }

        let local_normal = local_delta / dist;
        let world_normal = box_rot * local_normal;
        let depth = sphere_r - dist;
        let contact_point = sphere_pos - world_normal * sphere_r;
        let mut m = ContactManifold3D::new(hs, hb);
        m.add_contact(ContactPoint::new(contact_point, world_normal, depth));
        Some(m)
    }

    /// Box vs box using SAT (Separating Axis Theorem).
    pub fn box_box(
        pos_a: Vec3, rot_a: Quat, half_a: Vec3, ha: RigidBodyHandle,
        pos_b: Vec3, rot_b: Quat, half_b: Vec3, hb: RigidBodyHandle,
    ) -> Option<ContactManifold3D> {
        let axes_a = [
            rot_a * Vec3::X, rot_a * Vec3::Y, rot_a * Vec3::Z,
        ];
        let axes_b = [
            rot_b * Vec3::X, rot_b * Vec3::Y, rot_b * Vec3::Z,
        ];

        let t = pos_b - pos_a;

        let mut min_depth = f32::MAX;
        let mut best_normal = Vec3::X;

        // Test 15 SAT axes: 3 face-A, 3 face-B, 9 edge-edge
        let test_axes: Vec<Vec3> = {
            let mut v: Vec<Vec3> = Vec::with_capacity(15);
            v.extend_from_slice(&axes_a);
            v.extend_from_slice(&axes_b);
            for &aa in &axes_a {
                for &ab in &axes_b {
                    let cross = aa.cross(ab);
                    if cross.length_squared() > 1e-10 {
                        v.push(cross.normalize());
                    }
                }
            }
            v
        };

        for axis in test_axes {
            let proj_a = project_box_onto_axis(half_a, axes_a, axis);
            let proj_b = project_box_onto_axis(half_b, axes_b, axis);
            let t_proj = t.dot(axis).abs();
            let depth = proj_a + proj_b - t_proj;
            if depth < 0.0 { return None; }  // Separating axis found
            if depth < min_depth {
                min_depth = depth;
                // Normal should point from A to B
                best_normal = if t.dot(axis) < 0.0 { -axis } else { axis };
            }
        }

        let mut m = ContactManifold3D::new(ha, hb);
        // Approximate contact point at midpoint between centers projected onto normal
        let contact = pos_a + best_normal * (min_depth * 0.5);
        m.add_contact(ContactPoint::new(contact, best_normal, min_depth));
        Some(m)
    }

    /// Capsule vs capsule.
    pub fn capsule_capsule(
        pos_a: Vec3, axis_a: Vec3, r_a: f32, hh_a: f32, ha: RigidBodyHandle,
        pos_b: Vec3, axis_b: Vec3, r_b: f32, hh_b: f32, hb: RigidBodyHandle,
    ) -> Option<ContactManifold3D> {
        // Find closest points on the two line segments
        let p1 = pos_a - axis_a * hh_a;
        let p2 = pos_a + axis_a * hh_a;
        let p3 = pos_b - axis_b * hh_b;
        let p4 = pos_b + axis_b * hh_b;

        let (cp_a, cp_b) = closest_points_on_segments(p1, p2, p3, p4);
        let delta = cp_b - cp_a;
        let dist = delta.length();
        let sum_r = r_a + r_b;
        if dist >= sum_r { return None; }

        let normal = if dist > 1e-7 { delta / dist } else { Vec3::Y };
        let depth = sum_r - dist;
        let contact_point = cp_a + normal * r_a;

        let mut m = ContactManifold3D::new(ha, hb);
        m.add_contact(ContactPoint::new(contact_point, normal, depth));
        Some(m)
    }

    /// Dispatch collision detection based on shape types.
    pub fn detect(
        body_a: &RigidBody, ha: RigidBodyHandle,
        body_b: &RigidBody, hb: RigidBodyHandle,
    ) -> Option<ContactManifold3D> {
        match (&body_a.shape, &body_b.shape) {
            (CollisionShape::Sphere { radius: ra }, CollisionShape::Sphere { radius: rb }) => {
                Self::sphere_sphere(body_a.position, *ra, ha, body_b.position, *rb, hb)
            }
            (CollisionShape::Sphere { radius: rs }, CollisionShape::Box { half_extents }) => {
                Self::sphere_box(body_a.position, *rs, ha, body_b.position, body_b.orientation, *half_extents, hb)
            }
            (CollisionShape::Box { half_extents }, CollisionShape::Sphere { radius: rs }) => {
                let m = Self::sphere_box(body_b.position, *rs, hb, body_a.position, body_a.orientation, *half_extents, ha)?;
                // Flip normals
                let mut flipped = ContactManifold3D::new(ha, hb);
                for c in m.contacts {
                    flipped.add_contact(ContactPoint::new(c.point, -c.normal, c.depth));
                }
                Some(flipped)
            }
            (CollisionShape::Box { half_extents: ha_ext }, CollisionShape::Box { half_extents: hb_ext }) => {
                Self::box_box(body_a.position, body_a.orientation, *ha_ext, ha, body_b.position, body_b.orientation, *hb_ext, hb)
            }
            (CollisionShape::Capsule { radius: ra, half_height: hha }, CollisionShape::Capsule { radius: rb, half_height: hhb }) => {
                let axis_a = body_a.orientation * Vec3::Y;
                let axis_b = body_b.orientation * Vec3::Y;
                Self::capsule_capsule(body_a.position, axis_a, *ra, *hha, ha, body_b.position, axis_b, *rb, *hhb, hb)
            }
            _ => {
                // GJK fallback: use bounding sphere approximation
                let ra = body_a.shape.bounding_radius();
                let rb = body_b.shape.bounding_radius();
                Self::sphere_sphere(body_a.position, ra, ha, body_b.position, rb, hb)
            }
        }
    }
}

/// Project OBB half-extents onto an axis.
fn project_box_onto_axis(half: Vec3, axes: [Vec3; 3], axis: Vec3) -> f32 {
    half.x * axes[0].dot(axis).abs()
        + half.y * axes[1].dot(axis).abs()
        + half.z * axes[2].dot(axis).abs()
}

/// Find the closest points on two line segments.
fn closest_points_on_segments(p1: Vec3, p2: Vec3, p3: Vec3, p4: Vec3) -> (Vec3, Vec3) {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let r  = p1 - p3;
    let a  = d1.dot(d1);
    let e  = d2.dot(d2);
    let f  = d2.dot(r);

    let (s, t) = if a < 1e-10 && e < 1e-10 {
        (0.0, 0.0)
    } else if a < 1e-10 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e < 1e-10 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let s = if denom.abs() > 1e-10 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let t = (b * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };

    (p1 + d1 * s, p3 + d2 * t)
}

// ── Broadphase ────────────────────────────────────────────────────────────────

/// AABB entry for broadphase sweep-and-prune.
#[derive(Debug, Clone)]
pub struct BroadphaseEntry {
    pub handle:  RigidBodyHandle,
    pub min_x:   f32,
    pub max_x:   f32,
    pub min_y:   f32,
    pub max_y:   f32,
    pub min_z:   f32,
    pub max_z:   f32,
}

/// Sweep-and-prune broadphase on the X axis with insertion sort.
pub struct Broadphase {
    entries: Vec<BroadphaseEntry>,
}

impl Broadphase {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    /// Rebuild from all bodies.
    pub fn rebuild(&mut self, bodies: &HashMap<RigidBodyHandle, RigidBody>) {
        self.entries.clear();
        for (h, b) in bodies {
            let half = b.shape.aabb_half_extents();
            self.entries.push(BroadphaseEntry {
                handle: *h,
                min_x: b.position.x - half.x,
                max_x: b.position.x + half.x,
                min_y: b.position.y - half.y,
                max_y: b.position.y + half.y,
                min_z: b.position.z - half.z,
                max_z: b.position.z + half.z,
            });
        }
        // Insertion sort by min_x (nearly sorted after rebuild)
        let n = self.entries.len();
        for i in 1..n {
            let mut j = i;
            while j > 0 && self.entries[j - 1].min_x > self.entries[j].min_x {
                self.entries.swap(j - 1, j);
                j -= 1;
            }
        }
    }

    /// Returns all overlapping pairs (handle_a, handle_b).
    pub fn overlapping_pairs(&self) -> Vec<(RigidBodyHandle, RigidBodyHandle)> {
        let mut pairs = Vec::new();
        let n = self.entries.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let a = &self.entries[i];
                let b = &self.entries[j];
                // Early-out on X axis
                if b.min_x > a.max_x { break; }
                // Check Y and Z overlap
                if a.min_y > b.max_y || b.min_y > a.max_y { continue; }
                if a.min_z > b.max_z || b.min_z > a.max_z { continue; }
                pairs.push((a.handle, b.handle));
            }
        }
        pairs
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }
}

impl Default for Broadphase {
    fn default() -> Self { Self::new() }
}

// ── RayCast ───────────────────────────────────────────────────────────────────

/// Ray for intersection tests.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin:    Vec3,
    pub direction: Vec3,  // should be normalized
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction: direction.normalize_or_zero() }
    }

    pub fn at(&self, t: f32) -> Vec3 { self.origin + self.direction * t }
}

/// Result of a ray-cast hit.
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub handle:  RigidBodyHandle,
    pub t:       f32,       // distance along ray
    pub point:   Vec3,
    pub normal:  Vec3,
}

/// Ray intersection routines.
pub struct RayCast;

impl RayCast {
    /// Ray vs sphere.
    pub fn ray_sphere(ray: &Ray, center: Vec3, radius: f32) -> Option<f32> {
        let oc = ray.origin - center;
        let b = oc.dot(ray.direction);
        let c = oc.dot(oc) - radius * radius;
        let disc = b * b - c;
        if disc < 0.0 { return None; }
        let sqrt_disc = disc.sqrt();
        let t1 = -b - sqrt_disc;
        let t2 = -b + sqrt_disc;
        if t1 > 1e-4 { Some(t1) }
        else if t2 > 1e-4 { Some(t2) }
        else { None }
    }

    /// Ray vs AABB (slab method), transformed into box local space.
    pub fn ray_box(ray: &Ray, box_pos: Vec3, box_rot: Quat, box_half: Vec3) -> Option<f32> {
        let inv_rot = box_rot.inverse();
        let local_origin = inv_rot * (ray.origin - box_pos);
        let local_dir    = inv_rot * ray.direction;

        let t_min_arr = (-box_half - local_origin) / local_dir;
        let t_max_arr = ( box_half - local_origin) / local_dir;

        let t_min_v = t_min_arr.min(t_max_arr);
        let t_max_v = t_min_arr.max(t_max_arr);

        let t_enter = t_min_v.x.max(t_min_v.y).max(t_min_v.z);
        let t_exit  = t_max_v.x.min(t_max_v.y).min(t_max_v.z);

        if t_enter > t_exit || t_exit < 1e-4 { return None; }
        let t = if t_enter > 1e-4 { t_enter } else { t_exit };
        Some(t)
    }

    /// Ray vs capsule.
    pub fn ray_capsule(ray: &Ray, cap_pos: Vec3, cap_axis: Vec3, radius: f32, half_height: f32) -> Option<f32> {
        // Test against the cylinder body, then the two end-caps
        let p = ray.origin - cap_pos;
        let axis = cap_axis.normalize_or_zero();
        let d = ray.direction - ray.direction.dot(axis) * axis;
        let e = p - p.dot(axis) * axis;

        let a = d.dot(d);
        let b = 2.0 * d.dot(e);
        let c = e.dot(e) - radius * radius;

        let mut best_t = f32::MAX;

        if a > 1e-10 {
            let disc = b * b - 4.0 * a * c;
            if disc >= 0.0 {
                let sqrt_d = disc.sqrt();
                for t in [(-b - sqrt_d) / (2.0 * a), (-b + sqrt_d) / (2.0 * a)] {
                    if t > 1e-4 {
                        let pt = ray.at(t) - cap_pos;
                        let h = pt.dot(axis);
                        if h.abs() <= half_height && t < best_t {
                            best_t = t;
                        }
                    }
                }
            }
        }

        // End caps
        for sign in [-1.0_f32, 1.0] {
            let cap_center = cap_pos + axis * (sign * half_height);
            if let Some(t) = Self::ray_sphere(ray, cap_center, radius) {
                if t < best_t { best_t = t; }
            }
        }

        if best_t < f32::MAX { Some(best_t) } else { None }
    }

    /// Test a ray against a body's shape. Returns t if hit.
    pub fn ray_vs_body(ray: &Ray, body: &RigidBody) -> Option<f32> {
        match &body.shape {
            CollisionShape::Sphere { radius } =>
                Self::ray_sphere(ray, body.position, *radius),
            CollisionShape::Box { half_extents } =>
                Self::ray_box(ray, body.position, body.orientation, *half_extents),
            CollisionShape::Capsule { radius, half_height } => {
                let axis = body.orientation * Vec3::Y;
                Self::ray_capsule(ray, body.position, axis, *radius, *half_height)
            }
            CollisionShape::ConvexHull(_) => {
                // Approximate with bounding sphere
                let r = body.shape.bounding_radius();
                Self::ray_sphere(ray, body.position, r)
            }
        }
    }
}

// ── ImpulseResolver ───────────────────────────────────────────────────────────

/// Sequential impulse resolver with friction cone projection, restitution,
/// and warm-starting.
pub struct ImpulseResolver {
    /// Number of resolution iterations per step.
    pub iterations:    u32,
    /// Baumgarte stabilization factor.
    pub baumgarte:     f32,
    /// Penetration slop.
    pub slop:          f32,
    /// Enable warm starting.
    pub warm_start:    bool,
}

impl Default for ImpulseResolver {
    fn default() -> Self {
        Self { iterations: 10, baumgarte: 0.2, slop: 0.005, warm_start: true }
    }
}

impl ImpulseResolver {
    pub fn new() -> Self { Self::default() }

    pub fn with_iterations(mut self, n: u32) -> Self { self.iterations = n; self }

    /// Resolve a single contact point between two bodies.
    pub fn resolve_contact(
        &self,
        a: &mut RigidBody, b: &mut RigidBody,
        contact: &mut ContactPoint,
        dt: f32,
    ) {
        let ra = contact.point - a.position;
        let rb = contact.point - b.position;
        let n  = contact.normal;

        let va = a.velocity_at_point(contact.point);
        let vb = b.velocity_at_point(contact.point);
        let rel_vel = vb - va;
        let vn = rel_vel.dot(n);

        // Don't resolve if separating
        if vn > 0.0 { return; }

        let e = a.restitution.min(b.restitution);
        let restitution_term = if vn.abs() > 1.0 { -e * vn } else { 0.0 };
        let baumgarte_bias = self.baumgarte / dt * (contact.depth - self.slop).max(0.0);

        // Effective mass along normal
        let iwa = a.inv_inertia_world();
        let iwb = b.inv_inertia_world();
        let k_n = a.inv_mass + b.inv_mass
            + (iwa * ra.cross(n)).cross(ra).dot(n)
            + (iwb * rb.cross(n)).cross(rb).dot(n);

        if k_n < 1e-10 { return; }

        let lambda_n = -(vn + restitution_term + baumgarte_bias) / k_n;
        let prev_n = contact.cached_normal;
        let new_n  = (prev_n + lambda_n).max(0.0);
        let actual_n = new_n - prev_n;
        contact.cached_normal = new_n;

        let impulse_n = n * actual_n;
        a.apply_impulse_at_point(-impulse_n, contact.point);
        b.apply_impulse_at_point( impulse_n, contact.point);

        // Friction
        let rel_vel2 = b.velocity_at_point(contact.point) - a.velocity_at_point(contact.point);
        let tangential = rel_vel2 - n * rel_vel2.dot(n);
        let tangential_len = tangential.length();
        if tangential_len < 1e-7 { return; }
        let t1 = tangential / tangential_len;
        let t2 = n.cross(t1);

        let mu = (a.friction + b.friction) * 0.5;
        let max_friction = mu * contact.cached_normal;

        for (ti, tangent) in [t1, t2].iter().enumerate() {
            let k_t = a.inv_mass + b.inv_mass
                + (iwa * ra.cross(*tangent)).cross(ra).dot(*tangent)
                + (iwb * rb.cross(*tangent)).cross(rb).dot(*tangent);
            if k_t < 1e-10 { continue; }
            let vt = rel_vel2.dot(*tangent);
            let lambda_t = -vt / k_t;
            let prev_t = contact.cached_tangent[ti];
            let new_t = (prev_t + lambda_t).clamp(-max_friction, max_friction);
            let actual_t = new_t - prev_t;
            contact.cached_tangent[ti] = new_t;
            let imp_t = *tangent * actual_t;
            a.apply_impulse_at_point(-imp_t, contact.point);
            b.apply_impulse_at_point( imp_t, contact.point);
        }
    }

    /// Resolve all contacts in a manifold.
    pub fn resolve_manifold(
        &self,
        a: &mut RigidBody, b: &mut RigidBody,
        manifold: &mut ContactManifold3D,
        dt: f32,
    ) {
        // Warm-start: apply cached impulses
        if self.warm_start {
            for contact in &manifold.contacts {
                let impulse_n = contact.normal * contact.cached_normal * 0.8;
                a.apply_impulse_at_point(-impulse_n, contact.point);
                b.apply_impulse_at_point( impulse_n, contact.point);
            }
        }

        for _ in 0..self.iterations {
            for contact in &mut manifold.contacts {
                self.resolve_contact(a, b, contact, dt);
            }
        }
    }
}

// ── SleepSystem ───────────────────────────────────────────────────────────────

/// Manages sleeping bodies to skip expensive simulation when at rest.
pub struct SleepSystem {
    /// Kinetic energy threshold below which bodies become drowsy.
    pub energy_threshold: f32,
    /// Time a body must remain below threshold before sleeping (seconds).
    pub sleep_delay:      f32,
    /// Drowsy timers: handle -> seconds_below_threshold.
    drowsy_timers:        HashMap<RigidBodyHandle, f32>,
}

impl SleepSystem {
    pub fn new(energy_threshold: f32, sleep_delay: f32) -> Self {
        Self { energy_threshold, sleep_delay, drowsy_timers: HashMap::new() }
    }

    /// Update sleep states for all bodies. Returns handles that just went to sleep.
    pub fn update(
        &mut self,
        bodies: &mut HashMap<RigidBodyHandle, RigidBody>,
        dt: f32,
    ) -> Vec<RigidBodyHandle> {
        let mut newly_slept = Vec::new();

        for (h, body) in bodies.iter_mut() {
            if body.is_static() { continue; }

            let ke = body.kinetic_energy();
            if ke < self.energy_threshold {
                let timer = self.drowsy_timers.entry(*h).or_insert(0.0);
                *timer += dt;
                if *timer >= self.sleep_delay {
                    body.sleep_state = SleepState::Asleep;
                    body.velocity    = Vec3::ZERO;
                    body.angular_vel = Vec3::ZERO;
                    newly_slept.push(*h);
                } else {
                    body.sleep_state = SleepState::Drowsy { timer: *timer };
                }
            } else {
                self.drowsy_timers.remove(h);
                body.sleep_state = SleepState::Awake;
            }
        }

        newly_slept
    }

    /// Wake a body and propagate to all bodies connected by contacts.
    pub fn wake_body(
        &mut self,
        handle: RigidBodyHandle,
        bodies: &mut HashMap<RigidBodyHandle, RigidBody>,
        manifolds: &[ContactManifold3D],
    ) {
        let mut to_wake = vec![handle];
        let mut visited = std::collections::HashSet::new();

        while let Some(h) = to_wake.pop() {
            if visited.contains(&h) { continue; }
            visited.insert(h);

            if let Some(body) = bodies.get_mut(&h) {
                body.wake();
                self.drowsy_timers.remove(&h);
            }

            // Propagate to connected bodies
            for manifold in manifolds {
                if manifold.handle_a == h && !visited.contains(&manifold.handle_b) {
                    to_wake.push(manifold.handle_b);
                }
                if manifold.handle_b == h && !visited.contains(&manifold.handle_a) {
                    to_wake.push(manifold.handle_a);
                }
            }
        }
    }

    /// Wake all bodies touching the given AABB.
    pub fn wake_in_region(
        &mut self,
        region_min: Vec3, region_max: Vec3,
        bodies: &mut HashMap<RigidBodyHandle, RigidBody>,
        manifolds: &[ContactManifold3D],
    ) {
        let to_wake: Vec<RigidBodyHandle> = bodies.iter()
            .filter(|(_, b)| {
                let h = b.shape.aabb_half_extents();
                b.position.x + h.x >= region_min.x && b.position.x - h.x <= region_max.x &&
                b.position.y + h.y >= region_min.y && b.position.y - h.y <= region_max.y &&
                b.position.z + h.z >= region_min.z && b.position.z - h.z <= region_max.z
            })
            .map(|(h, _)| *h)
            .collect();

        for h in to_wake {
            self.wake_body(h, bodies, manifolds);
        }
    }

    pub fn sleeping_count(&self, bodies: &HashMap<RigidBodyHandle, RigidBody>) -> usize {
        bodies.values().filter(|b| b.is_sleeping()).count()
    }
}

// ── PhysicsWorld3D ────────────────────────────────────────────────────────────

/// The 3D physics simulation world.
pub struct PhysicsWorld3D {
    pub bodies:        HashMap<RigidBodyHandle, RigidBody>,
    pub gravity:       Vec3,
    pub fixed_dt:      f32,
    pub substeps:      u32,
    accumulator:       f32,
    next_id:           u32,
    broadphase:        Broadphase,
    resolver:          ImpulseResolver,
    pub sleep_system:  SleepSystem,
    manifolds:         Vec<ContactManifold3D>,
}

impl PhysicsWorld3D {
    pub fn new() -> Self {
        Self {
            bodies:      HashMap::new(),
            gravity:     Vec3::new(0.0, -9.81, 0.0),
            fixed_dt:    1.0 / 60.0,
            substeps:    2,
            accumulator: 0.0,
            next_id:     1,
            broadphase:  Broadphase::new(),
            resolver:    ImpulseResolver::default(),
            sleep_system: SleepSystem::new(0.1, 0.5),
            manifolds:   Vec::new(),
        }
    }

    /// Add a body, returning its handle.
    pub fn add_body(&mut self, body: RigidBody) -> RigidBodyHandle {
        let h = RigidBodyHandle(self.next_id);
        self.next_id += 1;
        self.bodies.insert(h, body);
        h
    }

    /// Remove a body.
    pub fn remove_body(&mut self, h: RigidBodyHandle) {
        self.bodies.remove(&h);
    }

    pub fn body(&self, h: RigidBodyHandle) -> Option<&RigidBody> { self.bodies.get(&h) }
    pub fn body_mut(&mut self, h: RigidBodyHandle) -> Option<&mut RigidBody> { self.bodies.get_mut(&h) }

    pub fn body_count(&self) -> usize { self.bodies.len() }

    /// Cast a ray, returning the closest hit.
    pub fn raycast(&self, ray: &Ray) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        for (h, body) in &self.bodies {
            if let Some(t) = RayCast::ray_vs_body(ray, body) {
                let is_better = best.as_ref().map(|b| t < b.t).unwrap_or(true);
                if is_better {
                    let point  = ray.at(t);
                    let normal = (point - body.position).normalize_or_zero();
                    best = Some(RayHit { handle: *h, t, point, normal });
                }
            }
        }
        best
    }

    /// Step the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.accumulator += dt;
        let fixed = self.fixed_dt;
        while self.accumulator >= fixed {
            let sub_dt = fixed / self.substeps as f32;
            for _ in 0..self.substeps {
                self.fixed_step(sub_dt);
            }
            self.accumulator -= fixed;
        }
    }

    fn fixed_step(&mut self, dt: f32) {
        let gravity = self.gravity;

        // Integrate all bodies
        for body in self.bodies.values_mut() {
            body.integrate(dt, gravity);
        }

        // Broadphase
        self.broadphase.rebuild(&self.bodies);
        let pairs = self.broadphase.overlapping_pairs();

        // Narrow phase + resolve
        self.manifolds.clear();
        for (ha, hb) in pairs {
            // Skip both static
            let (a_static, b_static) = {
                let a = self.bodies.get(&ha);
                let b = self.bodies.get(&hb);
                (a.map(|b| b.is_static()).unwrap_or(true), b.map(|b| b.is_static()).unwrap_or(true))
            };
            if a_static && b_static { continue; }

            // Collision group check
            let (a_group, a_mask, b_group, b_mask) = {
                let a = self.bodies.get(&ha);
                let b = self.bodies.get(&hb);
                (
                    a.map(|b| b.collision_group).unwrap_or(0),
                    a.map(|b| b.collision_mask).unwrap_or(0),
                    b.map(|b| b.collision_group).unwrap_or(0),
                    b.map(|b| b.collision_mask).unwrap_or(0),
                )
            };
            if a_group & b_mask == 0 && b_group & a_mask == 0 { continue; }

            let body_a_copy = self.bodies.get(&ha).cloned();
            let body_b_copy = self.bodies.get(&hb).cloned();

            if let (Some(ba), Some(bb)) = (body_a_copy, body_b_copy) {
                if let Some(mut manifold) = CollisionDetector::detect(&ba, ha, &bb, hb) {
                    // Wake sleeping bodies
                    if ba.is_sleeping() || bb.is_sleeping() {
                        if let Some(b) = self.bodies.get_mut(&ha) { b.wake(); }
                        if let Some(b) = self.bodies.get_mut(&hb) { b.wake(); }
                    }

                    // Resolve impulses
                    let body_a = self.bodies.get_mut(&ha).unwrap() as *mut RigidBody;
                    let body_b = self.bodies.get_mut(&hb).unwrap() as *mut RigidBody;
                    // Safety: ha != hb (different handles in same HashMap), so these are distinct.
                    unsafe {
                        self.resolver.resolve_manifold(&mut *body_a, &mut *body_b, &mut manifold, dt);
                    }

                    self.manifolds.push(manifold);
                }
            }
        }

        // Update sleep system
        self.sleep_system.update(&mut self.bodies, dt);
    }

    pub fn manifolds(&self) -> &[ContactManifold3D] { &self.manifolds }

    pub fn sleeping_count(&self) -> usize {
        self.sleep_system.sleeping_count(&self.bodies)
    }

    /// Apply a global force field (e.g. wind) to all non-sleeping bodies.
    pub fn apply_global_force(&mut self, force: Vec3) {
        for body in self.bodies.values_mut() {
            if !body.is_static() && !body.is_sleeping() {
                body.apply_force(force);
            }
        }
    }
}

impl Default for PhysicsWorld3D {
    fn default() -> Self { Self::new() }
}

// ── Legacy 2D types (kept for compatibility) ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointType {
    Distance, Revolute, Prismatic, Fixed, Spring, BallSocket, Weld, Pulley, Gear,
}

#[derive(Debug, Clone, Copy)]
pub struct JointAnchor {
    pub local_a: Vec2,
    pub local_b: Vec2,
}

impl JointAnchor {
    pub fn at_origins() -> Self { Self { local_a: Vec2::ZERO, local_b: Vec2::ZERO } }
    pub fn new(local_a: Vec2, local_b: Vec2) -> Self { Self { local_a, local_b } }
}

#[derive(Debug, Clone, Copy)]
pub struct JointLimits {
    pub min: f32,
    pub max: f32,
    pub motor_speed: f32,
    pub max_motor_force: f32,
    pub motor_enabled: bool,
}

impl JointLimits {
    pub fn none() -> Self {
        Self { min: f32::NEG_INFINITY, max: f32::INFINITY, motor_speed: 0.0, max_motor_force: 0.0, motor_enabled: false }
    }

    pub fn range(min: f32, max: f32) -> Self {
        Self { min, max, motor_speed: 0.0, max_motor_force: 0.0, motor_enabled: false }
    }

    pub fn with_motor(mut self, speed: f32, max_force: f32) -> Self {
        self.motor_speed = speed; self.max_motor_force = max_force; self.motor_enabled = true; self
    }

    pub fn clamp(&self, value: f32) -> f32 { value.clamp(self.min, self.max) }
}

#[derive(Debug, Clone)]
pub struct Joint {
    pub id:            u32,
    pub kind:          JointType,
    pub body_a:        u32,
    pub body_b:        Option<u32>,
    pub anchor:        JointAnchor,
    pub limits:        JointLimits,
    pub stiffness:     f32,
    pub damping:       f32,
    pub rest_length:   f32,
    pub frequency:     f32,
    pub collide_connected: bool,
    pub break_force:   f32,
    pub broken:        bool,
    pub gear_ratio:    f32,
    pub pulley_ratio:  f32,
    pub pulley_anchor_a: Vec2,
    pub pulley_anchor_b: Vec2,
}

impl Joint {
    pub fn distance(id: u32, body_a: u32, body_b: u32, local_a: Vec2, local_b: Vec2, length: f32) -> Self {
        Self {
            id, kind: JointType::Distance, body_a, body_b: Some(body_b),
            anchor: JointAnchor::new(local_a, local_b),
            limits: JointLimits::range(length * 0.9, length * 1.1),
            stiffness: 1.0, damping: 0.1, rest_length: length,
            frequency: 10.0, collide_connected: false,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn spring(id: u32, body_a: u32, body_b: u32, local_a: Vec2, local_b: Vec2, rest_len: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            id, kind: JointType::Spring, body_a, body_b: Some(body_b),
            anchor: JointAnchor::new(local_a, local_b),
            limits: JointLimits::none(),
            stiffness, damping, rest_length: rest_len,
            frequency: 5.0, collide_connected: true,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn revolute(id: u32, body_a: u32, body_b: u32, anchor: Vec2) -> Self {
        Self {
            id, kind: JointType::Revolute, body_a, body_b: Some(body_b),
            anchor: JointAnchor { local_a: anchor, local_b: anchor },
            limits: JointLimits::none(),
            stiffness: 1.0, damping: 0.05, rest_length: 0.0,
            frequency: 10.0, collide_connected: false,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn fixed(id: u32, body_a: u32, body_b: u32) -> Self {
        Self {
            id, kind: JointType::Fixed, body_a, body_b: Some(body_b),
            anchor: JointAnchor::at_origins(),
            limits: JointLimits::none(),
            stiffness: 1.0, damping: 1.0, rest_length: 0.0,
            frequency: 60.0, collide_connected: false,
            break_force: 0.0, broken: false, gear_ratio: 1.0,
            pulley_ratio: 1.0, pulley_anchor_a: Vec2::ZERO, pulley_anchor_b: Vec2::ZERO,
        }
    }

    pub fn with_limits(mut self, min: f32, max: f32) -> Self { self.limits = JointLimits::range(min, max); self }
    pub fn with_motor(mut self, speed: f32, max_force: f32) -> Self { self.limits = self.limits.with_motor(speed, max_force); self }
    pub fn with_break_force(mut self, force: f32) -> Self { self.break_force = force; self }
    pub fn collide_connected(mut self) -> Self { self.collide_connected = true; self }
    pub fn is_active(&self) -> bool { !self.broken }
}

#[derive(Debug, Clone, Default)]
pub struct JointImpulse {
    pub impulse:         Vec2,
    pub angular_impulse: f32,
    pub lambda:          f32,
}

/// Positional constraint solver using Sequential Impulses.
#[derive(Debug, Clone)]
pub struct JointSolver {
    pub iterations:    u32,
    pub position_slop: f32,
    pub baumgarte:     f32,
}

impl JointSolver {
    pub fn new() -> Self { Self { iterations: 10, position_slop: 0.005, baumgarte: 0.2 } }

    pub fn solve_distance(
        &self,
        pos_a: &mut Vec2, vel_a: &mut Vec2, inv_mass_a: f32,
        pos_b: &mut Vec2, vel_b: &mut Vec2, inv_mass_b: f32,
        rest_len: f32, stiffness: f32, damping: f32, dt: f32,
    ) {
        let delta = *pos_b - *pos_a;
        let dist = delta.length();
        if dist < 1e-6 { return; }
        let dir = delta / dist;
        let stretch = dist - rest_len;
        let relative_vel = (*vel_b - *vel_a).dot(dir);
        let force_mag = -stiffness * stretch - damping * relative_vel;
        let total_inv_mass = inv_mass_a + inv_mass_b;
        if total_inv_mass < 1e-6 { return; }
        let impulse = dir * (-force_mag * dt / total_inv_mass);
        *vel_a -= impulse * inv_mass_a;
        *vel_b += impulse * inv_mass_b;
    }

    pub fn solve_rigid_distance(
        &self,
        pos_a: &mut Vec2, vel_a: &mut Vec2, inv_mass_a: f32,
        pos_b: &mut Vec2, vel_b: &mut Vec2, inv_mass_b: f32,
        target_dist: f32, dt: f32,
    ) {
        for _ in 0..self.iterations {
            let delta = *pos_b - *pos_a;
            let dist = delta.length();
            if dist < 1e-6 { continue; }
            let dir = delta / dist;
            let error = dist - target_dist;
            let total_inv_mass = inv_mass_a + inv_mass_b;
            if total_inv_mass < 1e-6 { break; }
            let correction = dir * (error * self.baumgarte / (total_inv_mass * dt));
            *pos_a += correction * inv_mass_a;
            *pos_b -= correction * inv_mass_b;
            let vel_along = (*vel_b - *vel_a).dot(dir);
            let lambda = -vel_along / total_inv_mass;
            *vel_a -= dir * (lambda * inv_mass_a);
            *vel_b += dir * (lambda * inv_mass_b);
        }
    }
}

impl Default for JointSolver {
    fn default() -> Self { Self::new() }
}

// ── RagdollBone ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RagdollBone {
    pub name:        String,
    pub body_id:     u32,
    pub position:    Vec2,
    pub velocity:    Vec2,
    pub angle:       f32,
    pub angular_vel: f32,
    pub mass:        f32,
    pub inv_mass:    f32,
    pub half_extents: Vec2,
    pub damping:     f32,
    pub restitution: f32,
    pub friction:    f32,
}

impl RagdollBone {
    pub fn new(name: impl Into<String>, id: u32, pos: Vec2, mass: f32, half: Vec2) -> Self {
        Self {
            name: name.into(), body_id: id, position: pos,
            velocity: Vec2::ZERO, angle: 0.0, angular_vel: 0.0,
            mass, inv_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            half_extents: half, damping: 0.1, restitution: 0.3, friction: 0.5,
        }
    }

    pub fn apply_impulse(&mut self, impulse: Vec2, point: Vec2) {
        self.velocity += impulse * self.inv_mass;
        let r = point - self.position;
        let inertia_inv = 1.0 / (self.mass * (self.half_extents.x.powi(2) + self.half_extents.y.powi(2)) / 3.0).max(1e-6);
        self.angular_vel += (r.x * impulse.y - r.y * impulse.x) * inertia_inv;
    }

    pub fn integrate(&mut self, dt: f32, gravity: Vec2) {
        if self.inv_mass <= 0.0 { return; }
        self.velocity += gravity * dt;
        self.velocity *= (1.0 - self.damping * dt).max(0.0);
        self.position += self.velocity * dt;
        self.angle += self.angular_vel * dt;
        self.angular_vel *= (1.0 - self.damping * dt).max(0.0);
    }

    pub fn world_point(&self, local: Vec2) -> Vec2 {
        let (sin, cos) = self.angle.sin_cos();
        self.position + Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
    }
}

// ── Ragdoll ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Ragdoll {
    pub bones:   Vec<RagdollBone>,
    pub joints:  Vec<Joint>,
    pub active:  bool,
    pub gravity: Vec2,
    solver:      JointSolver,
}

impl Ragdoll {
    pub fn new() -> Self {
        Self { bones: Vec::new(), joints: Vec::new(), active: false, gravity: Vec2::new(0.0, -9.81), solver: JointSolver::new() }
    }

    pub fn add_bone(&mut self, bone: RagdollBone) -> u32 {
        let id = self.bones.len() as u32;
        self.bones.push(bone);
        id
    }

    pub fn add_joint(&mut self, joint: Joint) { self.joints.push(joint); }

    pub fn activate(&mut self, impulse: Vec2, contact_bone: usize) {
        self.active = true;
        if let Some(bone) = self.bones.get_mut(contact_bone) {
            bone.apply_impulse(impulse, bone.position);
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        for bone in &mut self.bones {
            bone.velocity    = Vec2::ZERO;
            bone.angular_vel = 0.0;
        }
    }

    pub fn update(&mut self, dt: f32) {
        if !self.active { return; }
        for bone in &mut self.bones { bone.integrate(dt, self.gravity); }

        let joint_snapshot: Vec<Joint> = self.joints.iter().filter(|j| j.is_active()).cloned().collect();
        for joint in &joint_snapshot {
            let Some(body_b_id) = joint.body_b else { continue };
            let a_idx = self.bones.iter().position(|b| b.body_id == joint.body_a);
            let b_idx = self.bones.iter().position(|b| b.body_id == body_b_id);
            if a_idx.is_none() || b_idx.is_none() { continue; }
            let ai = a_idx.unwrap();
            let bi = b_idx.unwrap();
            match joint.kind {
                JointType::Distance | JointType::Spring => {
                    let (left, right) = self.bones.split_at_mut(ai.max(bi));
                    let (bone_a, bone_b) = if ai < bi { (&mut left[ai], &mut right[0]) } else { (&mut right[0], &mut left[bi]) };
                    self.solver.solve_distance(
                        &mut bone_a.position, &mut bone_a.velocity, bone_a.inv_mass,
                        &mut bone_b.position, &mut bone_b.velocity, bone_b.inv_mass,
                        joint.rest_length, joint.stiffness, joint.damping, dt,
                    );
                }
                JointType::Fixed | JointType::Weld => {
                    let (left, right) = self.bones.split_at_mut(ai.max(bi));
                    let (bone_a, bone_b) = if ai < bi { (&mut left[ai], &mut right[0]) } else { (&mut right[0], &mut left[bi]) };
                    self.solver.solve_rigid_distance(
                        &mut bone_a.position, &mut bone_a.velocity, bone_a.inv_mass,
                        &mut bone_b.position, &mut bone_b.velocity, bone_b.inv_mass,
                        joint.rest_length, dt,
                    );
                }
                _ => {}
            }
        }

        for joint in &mut self.joints {
            if joint.broken || joint.break_force <= 0.0 { continue; }
            let Some(body_b_id) = joint.body_b else { continue };
            let a_pos = self.bones.iter().find(|b| b.body_id == joint.body_a).map(|b| b.position);
            let b_pos = self.bones.iter().find(|b| b.body_id == body_b_id).map(|b| b.position);
            if let (Some(pa), Some(pb)) = (a_pos, b_pos) {
                let stretch = ((pb - pa).length() - joint.rest_length).abs();
                if stretch * joint.stiffness > joint.break_force {
                    joint.broken = true;
                }
            }
        }
    }

    pub fn bone_by_name(&self, name: &str) -> Option<&RagdollBone> {
        self.bones.iter().find(|b| b.name == name)
    }

    pub fn bone_by_name_mut(&mut self, name: &str) -> Option<&mut RagdollBone> {
        self.bones.iter_mut().find(|b| b.name == name)
    }

    pub fn humanoid(position: Vec2) -> Self {
        let mut r = Ragdoll::new();
        let (tw, th) = (Vec2::new(0.2, 0.3), Vec2::new(0.15, 0.15));
        let (uh, lh) = (Vec2::new(0.08, 0.25), Vec2::new(0.07, 0.22));
        let (fh, ua, la) = (Vec2::new(0.10, 0.06), Vec2::new(0.07, 0.22), Vec2::new(0.06, 0.18));
        r.add_bone(RagdollBone::new("torso",      0,  position + Vec2::new( 0.0,  0.0),  15.0, tw));
        r.add_bone(RagdollBone::new("head",       1,  position + Vec2::new( 0.0,  0.55),  5.0, th));
        r.add_bone(RagdollBone::new("upper_arm_l",2,  position + Vec2::new(-0.35, 0.2),   3.0, ua));
        r.add_bone(RagdollBone::new("lower_arm_l",3,  position + Vec2::new(-0.35,-0.1),   2.0, la));
        r.add_bone(RagdollBone::new("upper_arm_r",4,  position + Vec2::new( 0.35, 0.2),   3.0, ua));
        r.add_bone(RagdollBone::new("lower_arm_r",5,  position + Vec2::new( 0.35,-0.1),   2.0, la));
        r.add_bone(RagdollBone::new("upper_leg_l",6,  position + Vec2::new(-0.12,-0.5),   5.0, uh));
        r.add_bone(RagdollBone::new("lower_leg_l",7,  position + Vec2::new(-0.12,-0.9),   4.0, lh));
        r.add_bone(RagdollBone::new("foot_l",     8,  position + Vec2::new(-0.12,-1.2),   1.5, fh));
        r.add_bone(RagdollBone::new("upper_leg_r",9,  position + Vec2::new( 0.12,-0.5),   5.0, uh));
        r.add_bone(RagdollBone::new("lower_leg_r",10, position + Vec2::new( 0.12,-0.9),   4.0, lh));
        r.add_bone(RagdollBone::new("foot_r",     11, position + Vec2::new( 0.12,-1.2),   1.5, fh));
        r.add_joint(Joint::revolute(0, 0, 1, Vec2::new(0.0, 0.45)).with_limits(-0.5, 0.5));
        r.add_joint(Joint::revolute(1, 0, 2, Vec2::new(-0.25, 0.25)).with_limits(-2.0, 0.5));
        r.add_joint(Joint::revolute(2, 2, 3, Vec2::new(-0.35,-0.05)).with_limits(-2.2, 0.0));
        r.add_joint(Joint::revolute(3, 0, 4, Vec2::new( 0.25, 0.25)).with_limits(-0.5, 2.0));
        r.add_joint(Joint::revolute(4, 4, 5, Vec2::new( 0.35,-0.05)).with_limits(0.0, 2.2));
        r.add_joint(Joint::revolute(5, 0, 6, Vec2::new(-0.12,-0.35)).with_limits(-0.5, 1.5));
        r.add_joint(Joint::revolute(6, 6, 7, Vec2::new(-0.12,-0.75)).with_limits(0.0, 2.5));
        r.add_joint(Joint::revolute(7, 7, 8, Vec2::new(-0.12,-1.12)).with_limits(-0.8, 0.8));
        r.add_joint(Joint::revolute(8, 0, 9, Vec2::new( 0.12,-0.35)).with_limits(-0.5, 1.5));
        r.add_joint(Joint::revolute(9, 9, 10,Vec2::new( 0.12,-0.75)).with_limits(0.0, 2.5));
        r.add_joint(Joint::revolute(10,10,11,Vec2::new( 0.12,-1.12)).with_limits(-0.8, 0.8));
        r
    }

    pub fn center_of_mass(&self) -> Vec2 {
        let (sm, sp) = self.bones.iter().fold((0.0f32, Vec2::ZERO), |(tm, tp), b| (tm + b.mass, tp + b.position * b.mass));
        if sm > 0.0 { sp / sm } else { Vec2::ZERO }
    }

    pub fn is_at_rest(&self) -> bool {
        const T: f32 = 0.05;
        self.bones.iter().all(|b| b.velocity.length() < T && b.angular_vel.abs() < T)
    }
}

impl Default for Ragdoll {
    fn default() -> Self { Self::new() }
}

// ── CharacterController ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CharacterController {
    pub position:          Vec2,
    pub velocity:          Vec2,
    pub size:              Vec2,
    pub speed:             f32,
    pub jump_force:        f32,
    pub gravity:           f32,
    pub on_ground:         bool,
    pub on_slope:          bool,
    pub max_slope_angle:   f32,
    pub step_height:       f32,
    pub coyote_time:       f32,
    pub coyote_timer:      f32,
    pub jump_buffer:       f32,
    pub jump_buffer_timer: f32,
    pub wall_stick_time:   f32,
    pub wall_stick_timer:  f32,
    pub is_wall_sliding:   bool,
    pub wall_normal:       Vec2,
    pub wall_jump_force:   Vec2,
    pub move_input:        f32,
    pub jump_requested:    bool,
    pub crouch:            bool,
    pub dash_velocity:     Vec2,
    pub dash_timer:        f32,
    pub dash_cooldown:     f32,
    pub dash_speed:        f32,
    pub dashes_remaining:  u32,
    pub max_dashes:        u32,
}

impl CharacterController {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self {
            position, velocity: Vec2::ZERO, size,
            speed: 6.0, jump_force: 12.0, gravity: -20.0,
            on_ground: false, on_slope: false, max_slope_angle: 0.7,
            step_height: 0.25,
            coyote_time: 0.15, coyote_timer: 0.0,
            jump_buffer: 0.12, jump_buffer_timer: 0.0,
            wall_stick_time: 0.3, wall_stick_timer: 0.0,
            is_wall_sliding: false, wall_normal: Vec2::ZERO, wall_jump_force: Vec2::new(6.0, 10.0),
            move_input: 0.0, jump_requested: false, crouch: false,
            dash_velocity: Vec2::ZERO, dash_timer: 0.0, dash_cooldown: 0.8,
            dash_speed: 15.0, dashes_remaining: 2, max_dashes: 2,
        }
    }

    pub fn move_input(&mut self, x: f32) { self.move_input = x.clamp(-1.0, 1.0); }
    pub fn request_jump(&mut self) { self.jump_requested = true; self.jump_buffer_timer = self.jump_buffer; }

    pub fn request_dash(&mut self) {
        if self.dashes_remaining > 0 && self.dash_timer <= 0.0 {
            self.dash_velocity = Vec2::new(self.move_input * self.dash_speed, 0.0);
            if self.move_input == 0.0 { self.dash_velocity.x = self.dash_speed; }
            self.dash_timer = 0.2; self.dash_cooldown = 0.8;
            self.dashes_remaining -= 1;
        }
    }

    pub fn update(&mut self, dt: f32, is_grounded: bool, slope_normal: Option<Vec2>) {
        if self.on_ground && !is_grounded { self.coyote_timer = self.coyote_time; }
        self.on_ground = is_grounded;
        if self.coyote_timer > 0.0 { self.coyote_timer -= dt; }
        if self.jump_buffer_timer > 0.0 { self.jump_buffer_timer -= dt; }
        if self.dash_cooldown > 0.0 { self.dash_cooldown -= dt; }
        if self.dash_timer > 0.0 { self.dash_timer -= dt; self.velocity.x = self.dash_velocity.x; }
        if is_grounded { self.dashes_remaining = self.max_dashes; }

        if !is_grounded {
            let grav = if self.is_wall_sliding { self.gravity * 0.3 } else { self.gravity };
            self.velocity.y += grav * dt;
        } else if self.velocity.y < 0.0 {
            self.velocity.y = 0.0;
        }

        if self.dash_timer <= 0.0 {
            let target = self.move_input * self.speed * if self.crouch { 0.5 } else { 1.0 };
            let accel = if is_grounded { 20.0 } else { 8.0 };
            self.velocity.x += (target - self.velocity.x) * (accel * dt).min(1.0);
        }

        let can_jump  = is_grounded || self.coyote_timer > 0.0;
        let wants_jump = self.jump_requested || self.jump_buffer_timer > 0.0;
        if can_jump && wants_jump {
            self.velocity.y = self.jump_force;
            self.coyote_timer = 0.0; self.jump_buffer_timer = 0.0; self.jump_requested = false;
        } else if !is_grounded && self.is_wall_sliding && wants_jump {
            self.velocity = self.wall_normal * self.wall_jump_force.x + Vec2::Y * self.wall_jump_force.y;
            self.is_wall_sliding = false; self.jump_buffer_timer = 0.0; self.jump_requested = false;
        }
        self.jump_requested = false;

        if let Some(normal) = slope_normal {
            let slope_angle = normal.dot(Vec2::Y).acos();
            self.on_slope = slope_angle > 0.1;
            if self.on_slope && is_grounded && self.dash_timer <= 0.0 {
                let right = Vec2::new(normal.y, -normal.x);
                self.velocity = right * self.velocity.dot(right);
            }
        }
        self.position += self.velocity * dt;
    }

    pub fn is_moving(&self) -> bool { self.velocity.length() > 0.1 }
    pub fn is_falling(&self) -> bool { self.velocity.y < -0.5 && !self.on_ground }
    pub fn is_jumping(&self) -> bool { self.velocity.y > 0.5 }
    pub fn facing_right(&self) -> bool { self.velocity.x >= 0.0 }
    pub fn aabb_min(&self) -> Vec2 { self.position - self.size }
    pub fn aabb_max(&self) -> Vec2 { self.position + self.size }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigid_body_falls_under_gravity() {
        let mut world = PhysicsWorld3D::new();
        let h = world.add_body(RigidBody::dynamic(Vec3::new(0.0, 10.0, 0.0), 1.0, CollisionShape::sphere(0.5)));
        world.step(0.5);
        let body = world.body(h).unwrap();
        assert!(body.position.y < 10.0, "body should fall: y={}", body.position.y);
    }

    #[test]
    fn static_body_does_not_move() {
        let mut world = PhysicsWorld3D::new();
        let h = world.add_body(RigidBody::static_body(Vec3::ZERO, CollisionShape::sphere(1.0)));
        world.step(1.0);
        assert_eq!(world.body(h).unwrap().position, Vec3::ZERO);
    }

    #[test]
    fn sphere_sphere_collision_detected() {
        let m = CollisionDetector::sphere_sphere(
            Vec3::ZERO, 1.0, RigidBodyHandle(1),
            Vec3::new(1.5, 0.0, 0.0), 1.0, RigidBodyHandle(2),
        );
        assert!(m.is_some(), "spheres should overlap");
        let m = CollisionDetector::sphere_sphere(
            Vec3::ZERO, 1.0, RigidBodyHandle(1),
            Vec3::new(3.0, 0.0, 0.0), 1.0, RigidBodyHandle(2),
        );
        assert!(m.is_none(), "spheres should not overlap");
    }

    #[test]
    fn sphere_box_collision() {
        let m = CollisionDetector::sphere_box(
            Vec3::new(0.0, 1.4, 0.0), 0.6, RigidBodyHandle(1),
            Vec3::ZERO, Quat::IDENTITY, Vec3::splat(1.0), RigidBodyHandle(2),
        );
        assert!(m.is_some(), "sphere should touch box");
    }

    #[test]
    fn box_box_collision_sat() {
        let m = CollisionDetector::box_box(
            Vec3::ZERO, Quat::IDENTITY, Vec3::splat(1.0), RigidBodyHandle(1),
            Vec3::new(1.5, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(1.0), RigidBodyHandle(2),
        );
        assert!(m.is_some(), "boxes should overlap");
    }

    #[test]
    fn capsule_capsule_collision() {
        let m = CollisionDetector::capsule_capsule(
            Vec3::ZERO, Vec3::Y, 0.5, 1.0, RigidBodyHandle(1),
            Vec3::new(0.8, 0.0, 0.0), Vec3::Y, 0.5, 1.0, RigidBodyHandle(2),
        );
        assert!(m.is_some(), "capsules should overlap");
    }

    #[test]
    fn broadphase_finds_overlapping_pairs() {
        let mut bp = Broadphase::new();
        let mut bodies = HashMap::new();
        bodies.insert(RigidBodyHandle(1), RigidBody::dynamic(Vec3::ZERO, 1.0, CollisionShape::sphere(1.0)));
        bodies.insert(RigidBodyHandle(2), RigidBody::dynamic(Vec3::new(1.5, 0.0, 0.0), 1.0, CollisionShape::sphere(1.0)));
        bodies.insert(RigidBodyHandle(3), RigidBody::dynamic(Vec3::new(100.0, 0.0, 0.0), 1.0, CollisionShape::sphere(1.0)));
        bp.rebuild(&bodies);
        let pairs = bp.overlapping_pairs();
        assert!(!pairs.is_empty(), "should find at least one overlapping pair");
        let has_far = pairs.iter().any(|(a, b)| *a == RigidBodyHandle(3) || *b == RigidBodyHandle(3));
        assert!(!has_far, "far body should not be in any pair");
    }

    #[test]
    fn raycast_hits_sphere() {
        let mut world = PhysicsWorld3D::new();
        let h = world.add_body(RigidBody::dynamic(Vec3::new(0.0, 0.0, 5.0), 1.0, CollisionShape::sphere(1.0)));
        let ray = Ray::new(Vec3::ZERO, Vec3::Z);
        let hit = world.raycast(&ray);
        assert!(hit.is_some(), "ray should hit sphere");
        assert_eq!(hit.unwrap().handle, h);
    }

    #[test]
    fn raycast_misses_sphere() {
        let mut world = PhysicsWorld3D::new();
        world.add_body(RigidBody::dynamic(Vec3::new(0.0, 5.0, 0.0), 1.0, CollisionShape::sphere(0.5)));
        let ray = Ray::new(Vec3::ZERO, Vec3::Z);
        let hit = world.raycast(&ray);
        assert!(hit.is_none(), "ray going Z should miss sphere at Y=5");
    }

    #[test]
    fn sleep_system_sleeps_body() {
        let mut bodies = HashMap::new();
        let h = RigidBodyHandle(1);
        bodies.insert(h, RigidBody::dynamic(Vec3::ZERO, 1.0, CollisionShape::sphere(0.5)));
        let mut sys = SleepSystem::new(10.0, 0.1); // high threshold so zero-velocity body sleeps quickly
        for _ in 0..10 {
            sys.update(&mut bodies, 0.05);
        }
        assert!(bodies[&h].is_sleeping(), "body should have slept");
    }

    #[test]
    fn sleep_wake_propagation() {
        let mut bodies = HashMap::new();
        let h1 = RigidBodyHandle(1);
        let h2 = RigidBodyHandle(2);
        let mut b1 = RigidBody::dynamic(Vec3::ZERO, 1.0, CollisionShape::sphere(0.5));
        b1.sleep_state = SleepState::Asleep;
        let mut b2 = RigidBody::dynamic(Vec3::new(0.5, 0.0, 0.0), 1.0, CollisionShape::sphere(0.5));
        b2.sleep_state = SleepState::Asleep;
        bodies.insert(h1, b1);
        bodies.insert(h2, b2);
        let manifolds = vec![ContactManifold3D { handle_a: h1, handle_b: h2, contacts: vec![] }];
        let mut sys = SleepSystem::new(0.01, 0.5);
        sys.wake_body(h1, &mut bodies, &manifolds);
        assert!(!bodies[&h1].is_sleeping(), "h1 should be awake");
        assert!(!bodies[&h2].is_sleeping(), "h2 should have been woken by propagation");
    }

    #[test]
    fn joint_spring_velocity_change() {
        let solver = JointSolver::new();
        let mut pa = Vec2::ZERO;
        let mut va = Vec2::ZERO;
        let mut pb = Vec2::new(5.0, 0.0);
        let mut vb = Vec2::ZERO;
        solver.solve_distance(&mut pa, &mut va, 1.0, &mut pb, &mut vb, 1.0, 2.0, 100.0, 0.5, 0.016);
        assert!(va.length() > 0.0 || vb.length() > 0.0, "velocities should change");
    }

    #[test]
    fn ragdoll_humanoid_bone_count() {
        let r = Ragdoll::humanoid(Vec2::new(0.0, 5.0));
        assert_eq!(r.bones.len(), 12);
        assert!(!r.joints.is_empty());
        assert!(r.bone_by_name("torso").is_some());
    }

    #[test]
    fn ragdoll_integrates() {
        let mut r = Ragdoll::humanoid(Vec2::ZERO);
        r.activate(Vec2::new(5.0, 8.0), 0);
        r.update(0.016);
        assert!(r.active);
    }

    #[test]
    fn character_controller_moves() {
        let mut cc = CharacterController::new(Vec2::ZERO, Vec2::new(0.4, 0.8));
        cc.move_input(1.0);
        cc.update(0.016, true, None);
        assert!(cc.position.x > 0.0, "controller should move right");
    }

    #[test]
    fn character_controller_jumps() {
        let mut cc = CharacterController::new(Vec2::ZERO, Vec2::new(0.4, 0.8));
        cc.request_jump();
        cc.update(0.016, true, None);
        assert!(cc.velocity.y > 0.0, "should be jumping");
    }

    #[test]
    fn collision_shape_bounding_radius() {
        let s = CollisionShape::sphere(2.0);
        assert!((s.bounding_radius() - 2.0).abs() < 1e-5);
        let b = CollisionShape::box_shape(Vec3::splat(1.0));
        assert!(b.bounding_radius() > 1.0);
    }

    #[test]
    fn rigid_body_kinetic_energy() {
        let mut b = RigidBody::dynamic(Vec3::ZERO, 2.0, CollisionShape::sphere(1.0));
        b.velocity = Vec3::new(3.0, 0.0, 0.0);
        let ke = b.kinetic_energy();
        // KE = 0.5 * 2 * 9 = 9
        assert!((ke - 9.0).abs() < 0.1, "KE should be ~9, got {ke}");
    }
}
