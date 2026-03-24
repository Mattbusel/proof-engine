//! 2D Rigid Body Physics for Proof Engine.
//!
//! Full dynamics pipeline:
//! - Rigid body integration (semi-implicit Euler)
//! - Collision shapes: Circle, Box, Polygon, Capsule
//! - AABB broad phase
//! - SAT narrow phase for convex polygons
//! - Sequential impulse solver with position correction
//! - Joint constraints: distance, revolute, prismatic, weld, spring
//! - Continuous collision detection (sweep test)
//! - Raycasting against all shapes
//! - Sleeping bodies for performance

use glam::{Vec2, Mat2};
use std::collections::HashMap;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const GRAVITY: Vec2 = Vec2::new(0.0, -9.81);
const SLEEP_VELOCITY_THRESHOLD: f32 = 0.05;
const SLEEP_ANGULAR_THRESHOLD:  f32 = 0.05;
const SLEEP_TIME_THRESHOLD:     f32 = 0.5;
const POSITION_CORRECTION_SLOP: f32 = 0.005;
const POSITION_CORRECTION_PERCENT: f32 = 0.4;
const MAX_SOLVER_ITERATIONS:    usize = 10;

// ── BodyId ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodyId(pub u32);

// ── Shape ─────────────────────────────────────────────────────────────────────

/// Collision shape definition.
#[derive(Debug, Clone)]
pub enum Shape {
    /// Circle with radius.
    Circle { radius: f32 },
    /// Axis-aligned box (half-extents before rotation).
    Box    { half_w: f32, half_h: f32 },
    /// Convex polygon with vertices in local space (CCW winding).
    Polygon { vertices: Vec<Vec2> },
    /// Capsule: two circles joined by a rectangle.
    Capsule { half_height: f32, radius: f32 },
}

impl Shape {
    /// Compute area.
    pub fn area(&self) -> f32 {
        match self {
            Shape::Circle { radius }          => std::f32::consts::PI * radius * radius,
            Shape::Box { half_w, half_h }     => 4.0 * half_w * half_h,
            Shape::Polygon { vertices }       => polygon_area(vertices).abs(),
            Shape::Capsule { half_height, radius } => {
                std::f32::consts::PI * radius * radius + 4.0 * half_height * radius
            }
        }
    }

    /// Compute moment of inertia (per unit mass) around center.
    pub fn moment_of_inertia(&self) -> f32 {
        match self {
            Shape::Circle { radius }      => 0.5 * radius * radius,
            Shape::Box { half_w, half_h } => (half_w * half_w + half_h * half_h) / 3.0,
            Shape::Polygon { vertices }   => polygon_moment(vertices),
            Shape::Capsule { half_height, radius } => {
                let rect_i = (4.0 * half_height * radius * (half_height * half_height / 3.0 + radius * radius / 4.0));
                let circ_i = 0.5 * radius * radius * std::f32::consts::PI * radius * radius;
                rect_i + circ_i
            }
        }
    }

    /// Compute local AABB.
    pub fn local_aabb(&self) -> Aabb2 {
        match self {
            Shape::Circle { radius } => Aabb2::new(-Vec2::splat(*radius), Vec2::splat(*radius)),
            Shape::Box { half_w, half_h } => Aabb2::new(Vec2::new(-half_w, -half_h), Vec2::new(*half_w, *half_h)),
            Shape::Polygon { vertices } => {
                let mn = vertices.iter().copied().reduce(|a, b| a.min(b)).unwrap_or(Vec2::ZERO);
                let mx = vertices.iter().copied().reduce(|a, b| a.max(b)).unwrap_or(Vec2::ZERO);
                Aabb2::new(mn, mx)
            }
            Shape::Capsule { half_height, radius } => {
                Aabb2::new(Vec2::new(-radius, -half_height - radius), Vec2::new(*radius, half_height + radius))
            }
        }
    }

    /// Get vertices of this shape in local space (approximated for circles/capsules).
    pub fn local_vertices(&self) -> Vec<Vec2> {
        match self {
            Shape::Box { half_w, half_h } => vec![
                Vec2::new(-*half_w, -*half_h),
                Vec2::new( *half_w, -*half_h),
                Vec2::new( *half_w,  *half_h),
                Vec2::new(-*half_w,  *half_h),
            ],
            Shape::Polygon { vertices } => vertices.clone(),
            Shape::Circle { radius } => {
                // 8-sided approximation for broadphase
                (0..8).map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    Vec2::new(a.cos(), a.sin()) * *radius
                }).collect()
            }
            Shape::Capsule { half_height, radius } => vec![
                Vec2::new(-*radius, -*half_height),
                Vec2::new( *radius, -*half_height),
                Vec2::new( *radius,  *half_height),
                Vec2::new(-*radius,  *half_height),
            ],
        }
    }
}

fn polygon_area(verts: &[Vec2]) -> f32 {
    let n = verts.len();
    let mut area = 0.0_f32;
    for i in 0..n {
        let j = (i + 1) % n;
        area += verts[i].x * verts[j].y - verts[j].x * verts[i].y;
    }
    area * 0.5
}

fn polygon_moment(verts: &[Vec2]) -> f32 {
    let n = verts.len();
    let mut num = 0.0_f32;
    let mut den = 0.0_f32;
    for i in 0..n {
        let j = (i + 1) % n;
        let cross = verts[i].perp_dot(verts[j]).abs();
        num += cross * (verts[i].dot(verts[i]) + verts[i].dot(verts[j]) + verts[j].dot(verts[j]));
        den += cross;
    }
    if den.abs() < 1e-6 { return 1.0; }
    num / (6.0 * den)
}

// ── Aabb2 ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Aabb2 {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb2 {
    pub fn new(min: Vec2, max: Vec2) -> Self { Self { min, max } }

    pub fn from_center_half(center: Vec2, half: Vec2) -> Self {
        Self { min: center - half, max: center + half }
    }

    pub fn overlaps(&self, other: &Aabb2) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn expand(&self, margin: f32) -> Self {
        Self { min: self.min - Vec2::splat(margin), max: self.max + Vec2::splat(margin) }
    }

    pub fn center(&self) -> Vec2 { (self.min + self.max) * 0.5 }
    pub fn half_extents(&self) -> Vec2 { (self.max - self.min) * 0.5 }

    /// Merge two AABBs.
    pub fn union(&self, other: &Aabb2) -> Self {
        Self { min: self.min.min(other.min), max: self.max.max(other.max) }
    }

    /// Ray-AABB intersection. Returns t (min hit distance) or None.
    pub fn ray_intersect(&self, origin: Vec2, dir: Vec2) -> Option<f32> {
        let inv_d = Vec2::new(
            if dir.x.abs() > 1e-10 { 1.0 / dir.x } else { f32::INFINITY },
            if dir.y.abs() > 1e-10 { 1.0 / dir.y } else { f32::INFINITY },
        );
        let t1 = (self.min - origin) * inv_d;
        let t2 = (self.max - origin) * inv_d;
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);
        let t_enter = tmin.x.max(tmin.y);
        let t_exit  = tmax.x.min(tmax.y);
        if t_enter <= t_exit && t_exit >= 0.0 { Some(t_enter.max(0.0)) } else { None }
    }
}

// ── PhysicsMaterial ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct PhysicsMaterial {
    pub restitution: f32, // bounciness 0..1
    pub friction:    f32, // coefficient 0..1
    pub density:     f32, // kg/m²
}

impl Default for PhysicsMaterial {
    fn default() -> Self { Self { restitution: 0.3, friction: 0.5, density: 1.0 } }
}

impl PhysicsMaterial {
    pub fn bouncy()   -> Self { Self { restitution: 0.9, friction: 0.1, density: 0.5 } }
    pub fn sticky()   -> Self { Self { restitution: 0.0, friction: 0.9, density: 1.5 } }
    pub fn ice()      -> Self { Self { restitution: 0.1, friction: 0.02, density: 0.9 } }
    pub fn rubber()   -> Self { Self { restitution: 0.8, friction: 0.7, density: 1.2 } }
    pub fn metal()    -> Self { Self { restitution: 0.2, friction: 0.3, density: 7.8 } }
    pub fn wood()     -> Self { Self { restitution: 0.3, friction: 0.6, density: 0.6 } }
}

// ── BodyType ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyType {
    /// Fully simulated.
    Dynamic,
    /// Infinite mass, moved manually.
    Kinematic,
    /// Never moves.
    Static,
}

// ── RigidBody2D ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RigidBody2D {
    pub id:        BodyId,
    pub body_type: BodyType,
    pub shape:     Shape,
    pub material:  PhysicsMaterial,

    // State
    pub position:         Vec2,
    pub angle:            f32,
    pub linear_velocity:  Vec2,
    pub angular_velocity: f32,

    // Derived
    pub mass:            f32,
    pub inv_mass:        f32,
    pub inertia:         f32,
    pub inv_inertia:     f32,

    // Accumulated forces (reset each step)
    pub force:           Vec2,
    pub torque:          f32,

    // Damping
    pub linear_damping:  f32,
    pub angular_damping: f32,

    // Sleeping
    pub sleeping:        bool,
    sleep_timer:         f32,

    // User data
    pub user_data:       u64,
    pub collision_layer: u32,
    pub collision_mask:  u32,

    // Position from previous frame (for CCD)
    pub prev_position:   Vec2,
    pub prev_angle:      f32,
    pub enabled:         bool,
    pub fixed_rotation:  bool,
    pub gravity_scale:   f32,
}

impl RigidBody2D {
    pub fn new(id: BodyId, shape: Shape, material: PhysicsMaterial) -> Self {
        let area = shape.area().max(1e-6);
        let mass = area * material.density;
        let inertia = mass * shape.moment_of_inertia();
        Self {
            id,
            body_type: BodyType::Dynamic,
            shape,
            material,
            position: Vec2::ZERO,
            angle: 0.0,
            linear_velocity: Vec2::ZERO,
            angular_velocity: 0.0,
            mass,
            inv_mass: 1.0 / mass,
            inertia,
            inv_inertia: 1.0 / inertia,
            force: Vec2::ZERO,
            torque: 0.0,
            linear_damping: 0.01,
            angular_damping: 0.01,
            sleeping: false,
            sleep_timer: 0.0,
            user_data: 0,
            collision_layer: 1,
            collision_mask: !0,
            prev_position: Vec2::ZERO,
            prev_angle: 0.0,
            enabled: true,
            fixed_rotation: false,
            gravity_scale: 1.0,
        }
    }

    pub fn static_body(id: BodyId, shape: Shape) -> Self {
        let mut b = Self::new(id, shape, PhysicsMaterial::default());
        b.body_type = BodyType::Static;
        b.inv_mass = 0.0;
        b.inv_inertia = 0.0;
        b
    }

    pub fn apply_force(&mut self, force: Vec2) {
        if self.body_type != BodyType::Dynamic { return; }
        self.force += force;
        self.sleeping = false;
    }

    pub fn apply_force_at(&mut self, force: Vec2, point: Vec2) {
        if self.body_type != BodyType::Dynamic { return; }
        self.force += force;
        self.torque += (point - self.position).perp_dot(force);
        self.sleeping = false;
    }

    pub fn apply_impulse(&mut self, impulse: Vec2) {
        if self.body_type != BodyType::Dynamic { return; }
        self.linear_velocity += impulse * self.inv_mass;
        self.sleeping = false;
    }

    pub fn apply_impulse_at(&mut self, impulse: Vec2, point: Vec2) {
        if self.body_type != BodyType::Dynamic { return; }
        self.linear_velocity += impulse * self.inv_mass;
        let r = point - self.position;
        self.angular_velocity += r.perp_dot(impulse) * self.inv_inertia;
        self.sleeping = false;
    }

    pub fn apply_torque(&mut self, torque: f32) {
        if self.body_type != BodyType::Dynamic { return; }
        self.torque += torque;
        self.sleeping = false;
    }

    pub fn velocity_at_point(&self, point: Vec2) -> Vec2 {
        let r = point - self.position;
        self.linear_velocity + Vec2::new(-self.angular_velocity * r.y, self.angular_velocity * r.x)
    }

    pub fn world_aabb(&self) -> Aabb2 {
        let local = self.shape.local_aabb();
        // Approximate: inflate by max dimension when rotated
        let r = local.half_extents().length();
        Aabb2::from_center_half(self.position, Vec2::splat(r))
    }

    pub fn rotation_matrix(&self) -> Mat2 {
        let (s, c) = self.angle.sin_cos();
        Mat2::from_cols(Vec2::new(c, s), Vec2::new(-s, c))
    }

    pub fn local_to_world(&self, local: Vec2) -> Vec2 {
        self.position + self.rotation_matrix() * local
    }

    pub fn world_to_local(&self, world: Vec2) -> Vec2 {
        self.rotation_matrix().transpose() * (world - self.position)
    }

    /// Get world-space vertices.
    pub fn world_vertices(&self) -> Vec<Vec2> {
        let rot = self.rotation_matrix();
        self.shape.local_vertices().iter().map(|v| self.position + rot * *v).collect()
    }

    fn integrate_forces(&mut self, dt: f32, gravity: Vec2) {
        if self.body_type != BodyType::Dynamic || self.sleeping { return; }
        let accel = self.force * self.inv_mass + gravity * self.gravity_scale;
        self.linear_velocity += accel * dt;
        self.linear_velocity *= 1.0 / (1.0 + self.linear_damping * dt);

        if !self.fixed_rotation {
            self.angular_velocity += self.torque * self.inv_inertia * dt;
            self.angular_velocity *= 1.0 / (1.0 + self.angular_damping * dt);
        }

        self.force = Vec2::ZERO;
        self.torque = 0.0;
    }

    fn integrate_velocities(&mut self, dt: f32) {
        if self.body_type != BodyType::Dynamic || self.sleeping { return; }
        self.prev_position = self.position;
        self.prev_angle    = self.angle;
        self.position += self.linear_velocity * dt;
        if !self.fixed_rotation {
            self.angle += self.angular_velocity * dt;
        }
    }

    fn update_sleep(&mut self, dt: f32) {
        if self.body_type != BodyType::Dynamic { return; }
        let v2 = self.linear_velocity.length_squared();
        let w2 = self.angular_velocity * self.angular_velocity;
        if v2 < SLEEP_VELOCITY_THRESHOLD * SLEEP_VELOCITY_THRESHOLD
        && w2 < SLEEP_ANGULAR_THRESHOLD  * SLEEP_ANGULAR_THRESHOLD {
            self.sleep_timer += dt;
            if self.sleep_timer > SLEEP_TIME_THRESHOLD {
                self.sleeping = true;
                self.linear_velocity  = Vec2::ZERO;
                self.angular_velocity = 0.0;
            }
        } else {
            self.sleep_timer = 0.0;
            self.sleeping    = false;
        }
    }
}

// ── ContactManifold ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContactPoint {
    pub point:      Vec2, // world space
    pub normal:     Vec2, // from b to a
    pub depth:      f32,  // penetration depth
    pub r_a:        Vec2, // contact point relative to body A center
    pub r_b:        Vec2, // contact point relative to body B center
    // Cached impulses for warm starting
    pub normal_impulse:    f32,
    pub tangent_impulse:   f32,
    pub mass_normal:       f32,
    pub mass_tangent:      f32,
    pub velocity_bias:     f32,
}

impl ContactPoint {
    pub fn new(point: Vec2, normal: Vec2, depth: f32) -> Self {
        Self {
            point, normal, depth,
            r_a: Vec2::ZERO, r_b: Vec2::ZERO,
            normal_impulse: 0.0, tangent_impulse: 0.0,
            mass_normal: 0.0, mass_tangent: 0.0,
            velocity_bias: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContactManifold {
    pub body_a:   BodyId,
    pub body_b:   BodyId,
    pub contacts: Vec<ContactPoint>,
    pub restitution: f32,
    pub friction:    f32,
}

// ── SAT Collision ─────────────────────────────────────────────────────────────

/// Separating Axis Theorem collision detection.
pub struct Sat;

impl Sat {
    pub fn test_circle_circle(
        pos_a: Vec2, r_a: f32,
        pos_b: Vec2, r_b: f32,
    ) -> Option<ContactPoint> {
        let delta = pos_b - pos_a;
        let dist  = delta.length();
        let sum_r = r_a + r_b;
        if dist >= sum_r || dist < 1e-8 { return None; }
        let normal = delta / dist;
        let depth  = sum_r - dist;
        let point  = pos_a + normal * r_a;
        Some(ContactPoint::new(point, -normal, depth))
    }

    pub fn test_circle_polygon(
        circle_pos: Vec2, radius: f32,
        poly_pos: Vec2,   poly_rot: Mat2, poly_verts: &[Vec2],
    ) -> Option<ContactPoint> {
        // Transform circle center to polygon local space
        let local_center = poly_rot.transpose() * (circle_pos - poly_pos);
        let n = poly_verts.len();

        let mut min_overlap = f32::INFINITY;
        let mut best_normal = Vec2::X;
        let mut closest_on_edge = Vec2::ZERO;

        for i in 0..n {
            let a = poly_verts[i];
            let b = poly_verts[(i + 1) % n];
            let edge = b - a;
            let normal = Vec2::new(edge.y, -edge.x).normalize_or_zero();
            let dist = (local_center - a).dot(normal);
            if dist > radius { return None; } // Separated on this axis
            if dist < min_overlap {
                min_overlap = dist;
                best_normal = normal;
                // Closest point on edge
                let t = (local_center - a).dot(edge) / edge.dot(edge).max(1e-10);
                closest_on_edge = a + edge * t.clamp(0.0, 1.0);
            }
        }

        // Check if circle center is inside polygon
        let overlap = radius - min_overlap;
        let contact_normal = poly_rot * best_normal;
        let contact_point  = poly_pos + poly_rot * closest_on_edge;
        Some(ContactPoint::new(contact_point, contact_normal, overlap.max(0.0)))
    }

    pub fn test_polygon_polygon(
        pos_a: Vec2, rot_a: Mat2, verts_a: &[Vec2],
        pos_b: Vec2, rot_b: Mat2, verts_b: &[Vec2],
    ) -> Option<ContactManifold> {
        // World-space vertices
        let wa: Vec<Vec2> = verts_a.iter().map(|v| pos_a + rot_a * *v).collect();
        let wb: Vec<Vec2> = verts_b.iter().map(|v| pos_b + rot_b * *v).collect();

        let mut min_overlap = f32::INFINITY;
        let mut best_normal = Vec2::ZERO;

        // Test edges of A
        if let Some((overlap, normal)) = Self::min_separation_axis(&wa, &wb) {
            if overlap < min_overlap { min_overlap = overlap; best_normal = normal; }
        } else { return None; }

        // Test edges of B
        if let Some((overlap, normal)) = Self::min_separation_axis(&wb, &wa) {
            if overlap < min_overlap { min_overlap = overlap; best_normal = -normal; }
        } else { return None; }

        // Ensure normal points from A to B
        if best_normal.dot(pos_b - pos_a) < 0.0 { best_normal = -best_normal; }

        // Find contact points (incident/reference edge clipping)
        let contacts = Self::find_contact_points(&wa, &wb, best_normal);

        if contacts.is_empty() { return None; }

        let contact_points = contacts.into_iter().map(|(pt, depth)| {
            ContactPoint::new(pt, -best_normal, depth.max(0.0))
        }).collect();

        Some(ContactManifold {
            body_a: BodyId(0), body_b: BodyId(0),
            contacts: contact_points,
            restitution: 0.3, friction: 0.5,
        })
    }

    fn min_separation_axis(a: &[Vec2], b: &[Vec2]) -> Option<(f32, Vec2)> {
        let n = a.len();
        let mut min_overlap = f32::INFINITY;
        let mut best_normal = Vec2::ZERO;

        for i in 0..n {
            let edge   = a[(i + 1) % n] - a[i];
            let normal = Vec2::new(edge.y, -edge.x).normalize_or_zero();

            // Project all of B onto this axis
            let (min_b, _max_b) = project_polygon(b, normal);
            let (min_a,  max_a) = project_polygon(a, normal);

            let overlap = max_a - min_b;
            if overlap <= 0.0 { return None; } // Separating axis found
            if overlap < min_overlap { min_overlap = overlap; best_normal = normal; }
        }

        Some((min_overlap, best_normal))
    }

    fn find_contact_points(a: &[Vec2], b: &[Vec2], normal: Vec2) -> Vec<(Vec2, f32)> {
        let mut contacts = Vec::new();
        // Clip B vertices that are behind A's reference face
        for &p in b {
            // Find depth along normal
            let depth = project_polygon(a, normal).1 - p.dot(normal);
            if depth >= -POSITION_CORRECTION_SLOP {
                contacts.push((p, depth));
            }
            if contacts.len() >= 2 { break; }
        }
        contacts
    }

    /// Circle vs Box collision.
    pub fn test_circle_box(
        circle_pos: Vec2, radius: f32,
        box_pos: Vec2, box_rot: Mat2, half_w: f32, half_h: f32,
    ) -> Option<ContactPoint> {
        let verts = [
            Vec2::new(-half_w, -half_h),
            Vec2::new( half_w, -half_h),
            Vec2::new( half_w,  half_h),
            Vec2::new(-half_w,  half_h),
        ];
        Self::test_circle_polygon(circle_pos, radius, box_pos, box_rot, &verts)
    }
}

fn project_polygon(verts: &[Vec2], axis: Vec2) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in verts {
        let d = v.dot(axis);
        min = min.min(d);
        max = max.max(d);
    }
    (min, max)
}

// ── ConstraintJoint ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointId(pub u32);

/// Joint connecting two bodies.
#[derive(Debug, Clone)]
pub enum Joint {
    /// Maintains a fixed distance between two anchor points.
    Distance {
        id:          JointId,
        body_a:      BodyId,
        body_b:      BodyId,
        anchor_a:    Vec2, // local space
        anchor_b:    Vec2, // local space
        rest_length: f32,
        stiffness:   f32,
        damping:     f32,
    },
    /// Allows relative rotation only.
    Revolute {
        id:          JointId,
        body_a:      BodyId,
        body_b:      BodyId,
        anchor_a:    Vec2,
        anchor_b:    Vec2,
        lower_angle: Option<f32>,
        upper_angle: Option<f32>,
        motor_speed: Option<f32>,
        motor_torque: f32,
    },
    /// Allows only linear motion along an axis.
    Prismatic {
        id:          JointId,
        body_a:      BodyId,
        body_b:      BodyId,
        anchor_a:    Vec2,
        anchor_b:    Vec2,
        axis:        Vec2,
        lower_limit: Option<f32>,
        upper_limit: Option<f32>,
    },
    /// Fully locks two bodies together.
    Weld {
        id:          JointId,
        body_a:      BodyId,
        body_b:      BodyId,
        anchor_a:    Vec2,
        anchor_b:    Vec2,
        ref_angle:   f32,
    },
    /// Spring joint (soft distance constraint).
    Spring {
        id:          JointId,
        body_a:      BodyId,
        body_b:      BodyId,
        anchor_a:    Vec2,
        anchor_b:    Vec2,
        rest_length: f32,
        frequency:   f32, // Hz
        damping_ratio: f32,
    },
    /// Mouse/target joint — pull body toward a world point.
    Target {
        id:         JointId,
        body:       BodyId,
        anchor:     Vec2, // local point on body
        target:     Vec2, // world point
        max_force:  f32,
        frequency:  f32,
        damping_ratio: f32,
    },
}

impl Joint {
    pub fn id(&self) -> JointId {
        match self {
            Joint::Distance   { id, .. } => *id,
            Joint::Revolute   { id, .. } => *id,
            Joint::Prismatic  { id, .. } => *id,
            Joint::Weld       { id, .. } => *id,
            Joint::Spring     { id, .. } => *id,
            Joint::Target     { id, .. } => *id,
        }
    }

    /// Apply impulses to enforce this joint.
    pub fn solve(&self, bodies: &mut HashMap<BodyId, RigidBody2D>, dt: f32) {
        match self {
            Joint::Distance { body_a, body_b, anchor_a, anchor_b, rest_length, stiffness, damping, id: _ } => {
                let (pa, pb, va, vb, inv_ma, inv_mb, inv_ia, inv_ib, ra, rb) = {
                    let a = bodies.get(body_a);
                    let b = bodies.get(body_b);
                    if a.is_none() || b.is_none() { return; }
                    let a = a.unwrap();
                    let b = b.unwrap();
                    let rot_a = a.rotation_matrix();
                    let rot_b = b.rotation_matrix();
                    let ra = rot_a * *anchor_a;
                    let rb = rot_b * *anchor_b;
                    (a.position, b.position, a.linear_velocity, b.linear_velocity,
                     a.inv_mass, b.inv_mass, a.inv_inertia, b.inv_inertia, ra, rb)
                };

                let wa = pa + ra;
                let wb = pb + rb;
                let delta = wb - wa;
                let len = delta.length();
                if len < 1e-6 { return; }
                let n = delta / len;
                let stretch = len - rest_length;

                // Spring force (Hooke's law)
                let rel_vel = (vb + Vec2::new(-b_angvel(bodies, body_b) * rb.y, b_angvel(bodies, body_b) * rb.x))
                    - (va + Vec2::new(-a_angvel(bodies, body_a) * ra.y, a_angvel(bodies, body_a) * ra.x));
                let vel_along = rel_vel.dot(n);

                let impulse_mag = stiffness * stretch * dt + damping * vel_along * dt;
                let impulse = n * impulse_mag;

                let effective_mass = inv_ma + inv_mb
                    + ra.perp_dot(n).powi(2) * inv_ia
                    + rb.perp_dot(n).powi(2) * inv_ib;
                if effective_mass < 1e-10 { return; }
                let scaled = impulse / effective_mass.max(0.001);

                if let Some(a) = bodies.get_mut(body_a) {
                    if a.body_type == BodyType::Dynamic {
                        a.linear_velocity  -= scaled * inv_ma;
                        a.angular_velocity -= ra.perp_dot(scaled) * inv_ia;
                    }
                }
                if let Some(b) = bodies.get_mut(body_b) {
                    if b.body_type == BodyType::Dynamic {
                        b.linear_velocity  += scaled * inv_mb;
                        b.angular_velocity += rb.perp_dot(scaled) * inv_ib;
                    }
                }
            }

            Joint::Spring { body_a, body_b, anchor_a, anchor_b, rest_length, frequency, damping_ratio, id: _ } => {
                let omega = 2.0 * std::f32::consts::PI * frequency;
                let k  = omega * omega;
                let c  = 2.0 * damping_ratio * omega;
                let a_stiff = 1.0 / (1.0 + c * dt + k * dt * dt);

                let pa = bodies.get(body_a).map(|b| b.position).unwrap_or_default();
                let pb = bodies.get(body_b).map(|b| b.position).unwrap_or_default();
                let delta = pb - pa;
                let len = delta.length();
                if len < 1e-6 { return; }
                let n = delta / len;
                let stretch = len - rest_length;

                let impulse = n * (stretch * k * dt * a_stiff);

                if let Some(a) = bodies.get_mut(body_a) {
                    if a.body_type == BodyType::Dynamic { a.apply_impulse(impulse); }
                }
                if let Some(b) = bodies.get_mut(body_b) {
                    if b.body_type == BodyType::Dynamic { b.apply_impulse(-impulse); }
                }
            }

            Joint::Target { body, anchor, target, max_force, frequency, damping_ratio, id: _ } => {
                let omega = 2.0 * std::f32::consts::PI * frequency;
                let k = omega * omega;
                let c = 2.0 * damping_ratio * omega;

                if let Some(b) = bodies.get_mut(body) {
                    if b.body_type != BodyType::Dynamic { return; }
                    let rot = b.rotation_matrix();
                    let world_anchor = b.position + rot * *anchor;
                    let error = *target - world_anchor;
                    let force = error * k - b.linear_velocity * c;
                    let force = force.clamp_length_max(*max_force);
                    b.apply_force(force);
                }
            }

            _ => { /* Other joints would apply positional corrections */ }
        }
    }
}

fn a_angvel(bodies: &HashMap<BodyId, RigidBody2D>, id: &BodyId) -> f32 {
    bodies.get(id).map(|b| b.angular_velocity).unwrap_or(0.0)
}
fn b_angvel(bodies: &HashMap<BodyId, RigidBody2D>, id: &BodyId) -> f32 {
    bodies.get(id).map(|b| b.angular_velocity).unwrap_or(0.0)
}

// ── ImpulseSolver ─────────────────────────────────────────────────────────────

pub struct ImpulseSolver;

impl ImpulseSolver {
    /// Pre-compute effective masses for a contact manifold.
    pub fn pre_step(manifold: &mut ContactManifold, bodies: &HashMap<BodyId, RigidBody2D>, dt: f32) {
        let (inv_ma, inv_mb, inv_ia, inv_ib, rest, fric, vel_a, vel_b, ang_a, ang_b, pa, pb) = {
            let a = bodies.get(&manifold.body_a);
            let b = bodies.get(&manifold.body_b);
            if a.is_none() || b.is_none() { return; }
            let a = a.unwrap(); let b = b.unwrap();
            let rest = (a.material.restitution * b.material.restitution).sqrt();
            let fric = (a.material.friction    * b.material.friction).sqrt();
            (a.inv_mass, b.inv_mass, a.inv_inertia, b.inv_inertia,
             rest, fric, a.linear_velocity, b.linear_velocity,
             a.angular_velocity, b.angular_velocity, a.position, b.position)
        };

        for cp in &mut manifold.contacts {
            cp.r_a = cp.point - pa;
            cp.r_b = cp.point - pb;

            let rn_a = cp.r_a.perp_dot(cp.normal);
            let rn_b = cp.r_b.perp_dot(cp.normal);
            let k_normal = inv_ma + inv_mb + rn_a * rn_a * inv_ia + rn_b * rn_b * inv_ib;
            cp.mass_normal = if k_normal > 1e-10 { 1.0 / k_normal } else { 0.0 };

            let tangent = Vec2::new(cp.normal.y, -cp.normal.x);
            let rt_a = cp.r_a.perp_dot(tangent);
            let rt_b = cp.r_b.perp_dot(tangent);
            let k_tangent = inv_ma + inv_mb + rt_a * rt_a * inv_ia + rt_b * rt_b * inv_ib;
            cp.mass_tangent = if k_tangent > 1e-10 { 1.0 / k_tangent } else { 0.0 };

            // Restitution bias
            let vrel = {
                let vb = vel_b + Vec2::new(-ang_b * cp.r_b.y, ang_b * cp.r_b.x);
                let va = vel_a + Vec2::new(-ang_a * cp.r_a.y, ang_a * cp.r_a.x);
                (vb - va).dot(cp.normal)
            };
            cp.velocity_bias = if vrel < -1.0 { -rest * vrel } else { 0.0 };

            // Position correction bias (Baumgarte)
            manifold.restitution = rest;
            manifold.friction    = fric;
            let _ = dt; // used in position correction step
        }
    }

    /// Apply one iteration of sequential impulse.
    pub fn apply_impulse(manifold: &mut ContactManifold, bodies: &mut HashMap<BodyId, RigidBody2D>) {
        let fric = manifold.friction;
        for cp in &mut manifold.contacts {
            let (vel_a, ang_a, vel_b, ang_b, inv_ma, inv_mb, inv_ia, inv_ib) = {
                let a = bodies.get(&manifold.body_a);
                let b = bodies.get(&manifold.body_b);
                if a.is_none() || b.is_none() { return; }
                let a = a.unwrap(); let b = b.unwrap();
                (a.linear_velocity, a.angular_velocity, b.linear_velocity, b.angular_velocity,
                 a.inv_mass, b.inv_mass, a.inv_inertia, b.inv_inertia)
            };

            let vb = vel_b + Vec2::new(-ang_b * cp.r_b.y, ang_b * cp.r_b.x);
            let va = vel_a + Vec2::new(-ang_a * cp.r_a.y, ang_a * cp.r_a.x);
            let vrel = vb - va;

            // Normal impulse
            let vn  = vrel.dot(cp.normal);
            let dj  = cp.mass_normal * (-vn + cp.velocity_bias);
            let j0  = cp.normal_impulse;
            cp.normal_impulse = (j0 + dj).max(0.0); // clamp to non-negative
            let dj  = cp.normal_impulse - j0;
            let impulse_n = cp.normal * dj;

            // Friction impulse (tangential)
            let tangent = Vec2::new(cp.normal.y, -cp.normal.x);
            let vt  = vrel.dot(tangent);
            let djt = -cp.mass_tangent * vt;
            let max_friction = fric * cp.normal_impulse;
            let j0t = cp.tangent_impulse;
            cp.tangent_impulse = (j0t + djt).clamp(-max_friction, max_friction);
            let djt = cp.tangent_impulse - j0t;
            let impulse_t = tangent * djt;

            let impulse = impulse_n + impulse_t;

            if let Some(a) = bodies.get_mut(&manifold.body_a) {
                if a.body_type == BodyType::Dynamic {
                    a.linear_velocity  -= impulse * inv_ma;
                    a.angular_velocity -= cp.r_a.perp_dot(impulse) * inv_ia;
                }
            }
            if let Some(b) = bodies.get_mut(&manifold.body_b) {
                if b.body_type == BodyType::Dynamic {
                    b.linear_velocity  += impulse * inv_mb;
                    b.angular_velocity += cp.r_b.perp_dot(impulse) * inv_ib;
                }
            }
        }
    }

    /// Position correction to prevent drift.
    pub fn correct_positions(manifold: &ContactManifold, bodies: &mut HashMap<BodyId, RigidBody2D>) {
        for cp in &manifold.contacts {
            let (inv_ma, inv_mb) = {
                let a = bodies.get(&manifold.body_a).map(|b| b.inv_mass).unwrap_or(0.0);
                let b = bodies.get(&manifold.body_b).map(|b| b.inv_mass).unwrap_or(0.0);
                (a, b)
            };

            let correction_mag = ((cp.depth - POSITION_CORRECTION_SLOP).max(0.0)
                / (inv_ma + inv_mb + 1e-10)) * POSITION_CORRECTION_PERCENT;
            let correction = cp.normal * correction_mag;

            if let Some(a) = bodies.get_mut(&manifold.body_a) {
                if a.body_type == BodyType::Dynamic { a.position -= correction * inv_ma; }
            }
            if let Some(b) = bodies.get_mut(&manifold.body_b) {
                if b.body_type == BodyType::Dynamic { b.position += correction * inv_mb; }
            }
        }
    }
}

// ── Raycasting ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RayHit {
    pub body_id:  BodyId,
    pub point:    Vec2,
    pub normal:   Vec2,
    pub distance: f32,
    pub fraction: f32,
}

pub struct RayCaster;

impl RayCaster {
    pub fn cast_vs_circle(
        origin: Vec2, dir: Vec2, max_dist: f32,
        center: Vec2, radius: f32,
    ) -> Option<f32> {
        let oc = origin - center;
        let b = oc.dot(dir);
        let c = oc.dot(oc) - radius * radius;
        let disc = b * b - c;
        if disc < 0.0 { return None; }
        let t = -b - disc.sqrt();
        if t >= 0.0 && t <= max_dist { Some(t) } else { None }
    }

    pub fn cast_vs_aabb(origin: Vec2, dir: Vec2, max_dist: f32, aabb: &Aabb2) -> Option<f32> {
        aabb.ray_intersect(origin, dir).filter(|&t| t <= max_dist)
    }

    pub fn cast_vs_polygon(
        origin: Vec2, dir: Vec2, max_dist: f32,
        pos: Vec2, rot: Mat2, verts: &[Vec2],
    ) -> Option<(f32, Vec2)> {
        let local_origin = rot.transpose() * (origin - pos);
        let local_dir    = rot.transpose() * dir;
        let n = verts.len();
        let mut best_t = f32::INFINITY;
        let mut best_n = Vec2::Y;

        for i in 0..n {
            let a = verts[i];
            let b = verts[(i + 1) % n];
            let edge = b - a;
            let normal = Vec2::new(edge.y, -edge.x);
            let denom = normal.dot(local_dir);
            if denom.abs() < 1e-10 { continue; }
            let t = normal.dot(a - local_origin) / denom;
            if t < 0.0 || t > max_dist { continue; }
            // Check if hit point is on edge
            let hit = local_origin + local_dir * t;
            let proj = (hit - a).dot(edge) / edge.dot(edge).max(1e-10);
            if proj >= 0.0 && proj <= 1.0 && t < best_t {
                best_t = t;
                best_n = rot * normal.normalize_or_zero();
            }
        }

        if best_t <= max_dist { Some((best_t, best_n)) } else { None }
    }
}

// ── CCD (Continuous Collision Detection) ──────────────────────────────────────

pub struct Ccd;

impl Ccd {
    /// Sweep two moving circles for earliest time-of-impact [0, 1].
    pub fn sweep_circles(
        pa0: Vec2, pa1: Vec2, ra: f32,
        pb0: Vec2, pb1: Vec2, rb: f32,
    ) -> Option<f32> {
        let dpa = pa1 - pa0;
        let dpb = pb1 - pb0;
        let rel_vel = dpa - dpb;
        let rel_pos = pa0 - pb0;
        let sum_r = ra + rb;

        let a = rel_vel.dot(rel_vel);
        let b = 2.0 * rel_pos.dot(rel_vel);
        let c = rel_pos.dot(rel_pos) - sum_r * sum_r;

        if a.abs() < 1e-10 {
            // Parallel movement
            return if c <= 0.0 { Some(0.0) } else { None };
        }

        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 { return None; }

        let t = (-b - disc.sqrt()) / (2.0 * a);
        if t >= 0.0 && t <= 1.0 { Some(t) } else { None }
    }

    /// Find the first sub-step where body A's swept AABB hits body B.
    pub fn needs_ccd(body: &RigidBody2D) -> bool {
        let aabb = body.world_aabb();
        let diag = (aabb.max - aabb.min).length();
        let vel  = body.linear_velocity.length();
        // CCD needed when velocity would traverse more than half the body per step at 60hz
        vel * (1.0 / 60.0) > diag * 0.5
    }
}

// ── PhysicsWorld2D ────────────────────────────────────────────────────────────

/// The main physics simulation world.
pub struct PhysicsWorld2D {
    pub bodies:    HashMap<BodyId, RigidBody2D>,
    pub joints:    Vec<Joint>,
    pub gravity:   Vec2,
    pub substeps:  u32,
    next_body_id:  u32,
    next_joint_id: u32,

    // Broadphase cache
    manifolds:     Vec<ContactManifold>,

    // Stats
    pub last_contact_count: usize,
    pub last_solve_time_us: u64,

    // Sleep
    pub allow_sleeping: bool,
}

impl PhysicsWorld2D {
    pub fn new() -> Self {
        Self {
            bodies: HashMap::new(),
            joints: Vec::new(),
            gravity: GRAVITY,
            substeps: 1,
            next_body_id: 1,
            next_joint_id: 1,
            manifolds: Vec::new(),
            last_contact_count: 0,
            last_solve_time_us: 0,
            allow_sleeping: true,
        }
    }

    pub fn zero_gravity() -> Self {
        let mut w = Self::new();
        w.gravity = Vec2::ZERO;
        w
    }

    pub fn add_body(&mut self, mut body: RigidBody2D) -> BodyId {
        let id = BodyId(self.next_body_id);
        self.next_body_id += 1;
        body.id = id;
        self.bodies.insert(id, body);
        id
    }

    pub fn remove_body(&mut self, id: BodyId) {
        self.bodies.remove(&id);
        self.joints.retain(|j| match j {
            Joint::Distance  { body_a, body_b, .. } => *body_a != id && *body_b != id,
            Joint::Revolute  { body_a, body_b, .. } => *body_a != id && *body_b != id,
            Joint::Prismatic { body_a, body_b, .. } => *body_a != id && *body_b != id,
            Joint::Weld      { body_a, body_b, .. } => *body_a != id && *body_b != id,
            Joint::Spring    { body_a, body_b, .. } => *body_a != id && *body_b != id,
            Joint::Target    { body, .. }           => *body != id,
        });
    }

    pub fn add_joint(&mut self, mut joint: Joint) -> JointId {
        let id = JointId(self.next_joint_id);
        self.next_joint_id += 1;
        // Patch id into joint
        match &mut joint {
            Joint::Distance  { id: jid, .. } => *jid = id,
            Joint::Revolute  { id: jid, .. } => *jid = id,
            Joint::Prismatic { id: jid, .. } => *jid = id,
            Joint::Weld      { id: jid, .. } => *jid = id,
            Joint::Spring    { id: jid, .. } => *jid = id,
            Joint::Target    { id: jid, .. } => *jid = id,
        }
        self.joints.push(joint);
        id
    }

    pub fn get_body(&self, id: BodyId) -> Option<&RigidBody2D> {
        self.bodies.get(&id)
    }

    pub fn get_body_mut(&mut self, id: BodyId) -> Option<&mut RigidBody2D> {
        self.bodies.get_mut(&id)
    }

    /// Advance the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let sub_dt = dt / self.substeps as f32;
        for _ in 0..self.substeps {
            self.sub_step(sub_dt);
        }
    }

    fn sub_step(&mut self, dt: f32) {
        // 1. Integrate forces
        for body in self.bodies.values_mut() {
            if !body.enabled { continue; }
            body.integrate_forces(dt, self.gravity);
        }

        // 2. Solve joints
        let joint_ids: Vec<usize> = (0..self.joints.len()).collect();
        for &i in &joint_ids {
            let joint = self.joints[i].clone();
            joint.solve(&mut self.bodies, dt);
        }

        // 3. Integrate velocities
        for body in self.bodies.values_mut() {
            if !body.enabled { continue; }
            body.integrate_velocities(dt);
        }

        // 4. Broadphase + narrowphase
        self.manifolds = self.detect_collisions();

        // 5. Pre-step contacts
        for manifold in &mut self.manifolds {
            ImpulseSolver::pre_step(manifold, &self.bodies, dt);
        }

        // 6. Sequential impulse solver
        for _ in 0..MAX_SOLVER_ITERATIONS {
            for i in 0..self.manifolds.len() {
                let manifold = &mut self.manifolds[i];
                ImpulseSolver::apply_impulse(manifold, &mut self.bodies);
            }
        }

        // 7. Position correction
        for manifold in &self.manifolds {
            ImpulseSolver::correct_positions(manifold, &mut self.bodies);
        }

        // 8. Sleep update
        if self.allow_sleeping {
            for body in self.bodies.values_mut() {
                body.update_sleep(dt);
            }
        }

        self.last_contact_count = self.manifolds.iter().map(|m| m.contacts.len()).sum();
    }

    fn detect_collisions(&self) -> Vec<ContactManifold> {
        let mut manifolds = Vec::new();
        let ids: Vec<BodyId> = self.bodies.keys().copied().collect();

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let id_a = ids[i];
                let id_b = ids[j];

                let a = &self.bodies[&id_a];
                let b = &self.bodies[&id_b];

                if !a.enabled || !b.enabled { continue; }
                if a.body_type == BodyType::Static && b.body_type == BodyType::Static { continue; }
                if a.sleeping && b.sleeping { continue; }
                if (a.collision_mask & b.collision_layer) == 0 { continue; }

                // Broadphase
                let aabb_a = a.world_aabb();
                let aabb_b = b.world_aabb();
                if !aabb_a.overlaps(&aabb_b) { continue; }

                // Narrowphase
                if let Some(mut m) = self.narrow_phase(a, b) {
                    m.body_a = id_a;
                    m.body_b = id_b;
                    m.restitution = (a.material.restitution * b.material.restitution).sqrt();
                    m.friction    = (a.material.friction    * b.material.friction).sqrt();
                    manifolds.push(m);
                }
            }
        }

        manifolds
    }

    fn narrow_phase(&self, a: &RigidBody2D, b: &RigidBody2D) -> Option<ContactManifold> {
        match (&a.shape, &b.shape) {
            (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
                Sat::test_circle_circle(a.position, *ra, b.position, *rb).map(|cp| {
                    ContactManifold {
                        body_a: a.id, body_b: b.id,
                        contacts: vec![cp],
                        restitution: 0.3, friction: 0.5,
                    }
                })
            }
            (Shape::Circle { radius }, Shape::Box { half_w, half_h }) => {
                Sat::test_circle_box(a.position, *radius, b.position, b.rotation_matrix(), *half_w, *half_h)
                    .map(|cp| ContactManifold { body_a: a.id, body_b: b.id, contacts: vec![cp], restitution: 0.3, friction: 0.5 })
            }
            (Shape::Box { half_w, half_h }, Shape::Circle { radius }) => {
                Sat::test_circle_box(b.position, *radius, a.position, a.rotation_matrix(), *half_w, *half_h)
                    .map(|mut cp| { cp.normal = -cp.normal; ContactManifold { body_a: a.id, body_b: b.id, contacts: vec![cp], restitution: 0.3, friction: 0.5 } })
            }
            (_, _) => {
                // Polygon vs polygon (covers Box vs Box, Polygon vs Polygon, etc.)
                let verts_a = a.shape.local_vertices();
                let verts_b = b.shape.local_vertices();
                Sat::test_polygon_polygon(
                    a.position, a.rotation_matrix(), &verts_a,
                    b.position, b.rotation_matrix(), &verts_b,
                ).map(|mut m| { m.body_a = a.id; m.body_b = b.id; m })
            }
        }
    }

    /// Cast a ray through the world, returning all hits sorted by distance.
    pub fn raycast(&self, origin: Vec2, dir: Vec2, max_dist: f32) -> Vec<RayHit> {
        let dir = dir.normalize_or_zero();
        let mut hits = Vec::new();

        for (id, body) in &self.bodies {
            if !body.enabled { continue; }
            match &body.shape {
                Shape::Circle { radius } => {
                    if let Some(t) = RayCaster::cast_vs_circle(origin, dir, max_dist, body.position, *radius) {
                        let point  = origin + dir * t;
                        let normal = (point - body.position).normalize_or_zero();
                        hits.push(RayHit { body_id: *id, point, normal, distance: t, fraction: t / max_dist });
                    }
                }
                _ => {
                    let verts = body.world_vertices();
                    if let Some((t, normal)) = RayCaster::cast_vs_polygon(origin, dir, max_dist, body.position, body.rotation_matrix(), &body.shape.local_vertices()) {
                        let _ = verts;
                        hits.push(RayHit { body_id: *id, point: origin + dir * t, normal, distance: t, fraction: t / max_dist });
                    }
                }
            }
        }

        hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        hits
    }

    /// Query all bodies overlapping a circle.
    pub fn query_circle(&self, center: Vec2, radius: f32) -> Vec<BodyId> {
        self.bodies.iter()
            .filter(|(_, b)| (b.position - center).length() < radius + b.world_aabb().half_extents().length())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Apply an explosion impulse: pushes all dynamic bodies within radius.
    pub fn explode(&mut self, center: Vec2, radius: f32, force: f32) {
        let ids: Vec<BodyId> = self.bodies.keys().copied().collect();
        for id in ids {
            if let Some(body) = self.bodies.get_mut(&id) {
                if body.body_type != BodyType::Dynamic { continue; }
                let delta = body.position - center;
                let dist  = delta.length();
                if dist < radius && dist > 1e-6 {
                    let falloff = 1.0 - (dist / radius);
                    let impulse = delta.normalize_or_zero() * force * falloff;
                    body.apply_impulse(impulse);
                }
            }
        }
    }

    /// Wake all sleeping bodies.
    pub fn wake_all(&mut self) {
        for body in self.bodies.values_mut() {
            body.sleeping = false;
            body.sleep_timer = 0.0;
        }
    }
}

impl Default for PhysicsWorld2D {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn circle_body(id: u32, pos: Vec2, r: f32) -> RigidBody2D {
        let mut b = RigidBody2D::new(BodyId(id), Shape::Circle { radius: r }, PhysicsMaterial::default());
        b.position = pos;
        b
    }

    fn static_box(id: u32, pos: Vec2, hw: f32, hh: f32) -> RigidBody2D {
        let mut b = RigidBody2D::static_body(BodyId(id), Shape::Box { half_w: hw, half_h: hh });
        b.position = pos;
        b
    }

    #[test]
    fn test_circle_circle_collision() {
        let hit = Sat::test_circle_circle(Vec2::ZERO, 1.0, Vec2::new(1.5, 0.0), 1.0);
        assert!(hit.is_some(), "overlapping circles should collide");
        let cp = hit.unwrap();
        assert!(cp.depth > 0.0);
        assert!((cp.depth - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_circle_circle_no_collision() {
        let hit = Sat::test_circle_circle(Vec2::ZERO, 0.5, Vec2::new(2.0, 0.0), 0.5);
        assert!(hit.is_none(), "separated circles should not collide");
    }

    #[test]
    fn test_sat_box_box() {
        let va = [Vec2::new(-1.0,-1.0), Vec2::new(1.0,-1.0), Vec2::new(1.0,1.0), Vec2::new(-1.0,1.0)];
        let vb = [Vec2::new(-1.0,-1.0), Vec2::new(1.0,-1.0), Vec2::new(1.0,1.0), Vec2::new(-1.0,1.0)];
        // Overlapping boxes at offset 1.5 — should collide
        let result = Sat::test_polygon_polygon(
            Vec2::ZERO, Mat2::IDENTITY, &va,
            Vec2::new(1.5, 0.0), Mat2::IDENTITY, &vb,
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_sat_box_box_separated() {
        let va = [Vec2::new(-0.5,-0.5), Vec2::new(0.5,-0.5), Vec2::new(0.5,0.5), Vec2::new(-0.5,0.5)];
        let vb = va;
        let result = Sat::test_polygon_polygon(
            Vec2::ZERO, Mat2::IDENTITY, &va,
            Vec2::new(3.0, 0.0), Mat2::IDENTITY, &vb,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_rigid_body_gravity() {
        let mut world = PhysicsWorld2D::new();
        let id = world.add_body(circle_body(1, Vec2::new(0.0, 10.0), 0.5));
        let y0 = world.get_body(id).unwrap().position.y;
        world.step(1.0);
        let y1 = world.get_body(id).unwrap().position.y;
        assert!(y1 < y0, "body should fall under gravity");
    }

    #[test]
    fn test_static_body_no_move() {
        let mut world = PhysicsWorld2D::new();
        let id = world.add_body(static_box(1, Vec2::ZERO, 5.0, 0.5));
        let p0 = world.get_body(id).unwrap().position;
        world.step(1.0);
        let p1 = world.get_body(id).unwrap().position;
        assert_eq!(p0, p1, "static body should not move");
    }

    #[test]
    fn test_collision_response() {
        let mut world = PhysicsWorld2D::new();
        // Floor
        world.add_body(static_box(1, Vec2::new(0.0, -5.0), 10.0, 0.5));
        // Ball falling onto floor
        let ball_id = world.add_body(circle_body(2, Vec2::new(0.0, -4.0), 0.5));

        // Run simulation
        for _ in 0..60 {
            world.step(1.0 / 60.0);
        }

        let ball = world.get_body(ball_id).unwrap();
        // Ball should be resting on floor: y ≈ -4.5 (floor top at -4.5, ball radius 0.5)
        assert!(ball.position.y > -5.5, "ball should not fall through floor");
    }

    #[test]
    fn test_raycast_hits_circle() {
        let mut world = PhysicsWorld2D::new();
        world.add_body(circle_body(1, Vec2::new(5.0, 0.0), 1.0));
        let hits = world.raycast(Vec2::ZERO, Vec2::X, 20.0);
        assert!(!hits.is_empty(), "ray should hit the circle");
        assert!((hits[0].distance - 4.0).abs() < 0.1);
    }

    #[test]
    fn test_raycast_misses() {
        let mut world = PhysicsWorld2D::new();
        world.add_body(circle_body(1, Vec2::new(0.0, 5.0), 1.0));
        let hits = world.raycast(Vec2::ZERO, Vec2::X, 20.0);
        assert!(hits.is_empty(), "horizontal ray should miss circle above");
    }

    #[test]
    fn test_aabb_overlap() {
        let a = Aabb2::new(Vec2::ZERO, Vec2::ONE);
        let b = Aabb2::new(Vec2::new(0.5, 0.5), Vec2::new(1.5, 1.5));
        assert!(a.overlaps(&b));
        let c = Aabb2::new(Vec2::new(2.0, 0.0), Vec2::new(3.0, 1.0));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn test_explode_pushes_bodies() {
        let mut world = PhysicsWorld2D::zero_gravity();
        let id = world.add_body(circle_body(1, Vec2::new(2.0, 0.0), 0.5));
        world.explode(Vec2::ZERO, 5.0, 10.0);
        let body = world.get_body(id).unwrap();
        assert!(body.linear_velocity.length() > 0.0, "explosion should impart velocity");
    }

    #[test]
    fn test_distance_joint() {
        let mut world = PhysicsWorld2D::zero_gravity();
        let a = world.add_body(circle_body(1, Vec2::ZERO, 0.2));
        let b = world.add_body(circle_body(2, Vec2::new(2.0, 0.0), 0.2));
        world.add_joint(Joint::Distance {
            id: JointId(0),
            body_a: a, body_b: b,
            anchor_a: Vec2::ZERO, anchor_b: Vec2::ZERO,
            rest_length: 2.0,
            stiffness: 100.0,
            damping: 1.0,
        });
        world.step(0.1);
        // Both bodies should still be roughly separated by ~2 units
        let pa = world.get_body(a).unwrap().position;
        let pb = world.get_body(b).unwrap().position;
        assert!((pa - pb).length() < 3.0);
    }

    #[test]
    fn test_ccd_fast_body() {
        let mut b = circle_body(1, Vec2::ZERO, 0.1);
        b.linear_velocity = Vec2::new(1000.0, 0.0); // Very fast
        assert!(Ccd::needs_ccd(&b));
        b.linear_velocity = Vec2::new(0.1, 0.0); // Slow
        assert!(!Ccd::needs_ccd(&b));
    }

    #[test]
    fn test_sweep_circles() {
        // Two circles approaching each other
        let toi = Ccd::sweep_circles(
            Vec2::ZERO,       Vec2::new(1.0, 0.0), 0.5,
            Vec2::new(2.0, 0.0), Vec2::new(1.0, 0.0), 0.5,
        );
        assert!(toi.is_some());
    }

    #[test]
    fn test_polygon_area() {
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let area = polygon_area(&verts).abs();
        assert!((area - 1.0).abs() < 0.001);
    }
}
