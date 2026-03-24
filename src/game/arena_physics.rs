//! Arena physics objects and environmental interactions for Chaos RPG.
//!
//! Provides a self-contained 2D physics world with broad-phase (sweep-and-prune),
//! narrow-phase collision detection (circle-circle, circle-AABB, AABB-AABB),
//! impulse-based collision response, raycasting, overlap tests, and a rich set
//! of arena room types with interactive traps, treasure, and chaos rift mechanics.

use glam::{Vec2, Vec3};
use std::collections::{HashMap, BTreeMap};

// ── Constants ────────────────────────────────────────────────────────────────

const DEFAULT_GRAVITY: Vec2 = Vec2::new(0.0, -9.81);
const DEFAULT_DAMPING: f32 = 0.99;
const ANGULAR_DAMPING: f32 = 0.98;
const POSITION_SLOP: f32 = 0.005;
const POSITION_CORRECTION: f32 = 0.4;
const SOLVER_ITERATIONS: usize = 8;

// ── ObjectId ─────────────────────────────────────────────────────────────────

/// Unique identifier for a physics object in the arena world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub u32);

// ── CollisionShape ───────────────────────────────────────────────────────────

/// Collision shape for arena physics objects.
#[derive(Debug, Clone)]
pub enum CollisionShape {
    /// Circle with radius.
    Circle { radius: f32 },
    /// Axis-aligned bounding box with half-extents.
    AABB { half_extents: Vec2 },
    /// Capsule: cylinder capped with hemispheres.
    Capsule { radius: f32, height: f32 },
}

impl CollisionShape {
    /// Compute the local-space axis-aligned bounding box.
    pub fn local_aabb(&self) -> AABB {
        match self {
            CollisionShape::Circle { radius } => AABB {
                min: Vec2::new(-*radius, -*radius),
                max: Vec2::new(*radius, *radius),
            },
            CollisionShape::AABB { half_extents } => AABB {
                min: -*half_extents,
                max: *half_extents,
            },
            CollisionShape::Capsule { radius, height } => {
                let half_h = height * 0.5;
                AABB {
                    min: Vec2::new(-*radius, -half_h - *radius),
                    max: Vec2::new(*radius, half_h + *radius),
                }
            }
        }
    }

    /// Compute area for mass calculations.
    pub fn area(&self) -> f32 {
        match self {
            CollisionShape::Circle { radius } => std::f32::consts::PI * radius * radius,
            CollisionShape::AABB { half_extents } => 4.0 * half_extents.x * half_extents.y,
            CollisionShape::Capsule { radius, height } => {
                std::f32::consts::PI * radius * radius + 2.0 * radius * height
            }
        }
    }
}

// ── AABB helper ──────────────────────────────────────────────────────────────

/// Axis-aligned bounding box for broad-phase.
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec2,
    pub max: Vec2,
}

impl AABB {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn from_center_half(center: Vec2, half: Vec2) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn overlaps(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec2 {
        (self.max - self.min) * 0.5
    }

    /// Merge two AABBs.
    pub fn union(&self, other: &AABB) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Ray intersection. Returns t value or None.
    pub fn ray_intersect(&self, origin: Vec2, dir: Vec2) -> Option<f32> {
        let inv_d = Vec2::new(
            if dir.x.abs() > 1e-12 { 1.0 / dir.x } else { f32::MAX },
            if dir.y.abs() > 1e-12 { 1.0 / dir.y } else { f32::MAX },
        );
        let t1 = (self.min.x - origin.x) * inv_d.x;
        let t2 = (self.max.x - origin.x) * inv_d.x;
        let t3 = (self.min.y - origin.y) * inv_d.y;
        let t4 = (self.max.y - origin.y) * inv_d.y;
        let tmin = t1.min(t2).max(t3.min(t4));
        let tmax = t1.max(t2).min(t3.max(t4));
        if tmax < 0.0 || tmin > tmax {
            None
        } else {
            Some(if tmin >= 0.0 { tmin } else { tmax })
        }
    }

    /// Expand by a margin.
    pub fn expand(&self, margin: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(margin),
            max: self.max + Vec2::splat(margin),
        }
    }
}

// ── PhysicsObject ────────────────────────────────────────────────────────────

/// A physics object within the arena world.
#[derive(Debug, Clone)]
pub struct PhysicsObject {
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: f32,
    pub mass: f32,
    pub restitution: f32,
    pub friction: f32,
    pub shape: CollisionShape,
    pub is_static: bool,
    pub is_trigger: bool,
    pub collision_layer: u32,
    // Internal fields
    inv_mass: f32,
    force_accum: Vec2,
    torque_accum: f32,
    angle: f32,
}

impl PhysicsObject {
    /// Create a new dynamic physics object.
    pub fn new(position: Vec3, mass: f32, shape: CollisionShape) -> Self {
        let inv = if mass > 0.0 && !mass.is_infinite() {
            1.0 / mass
        } else {
            0.0
        };
        Self {
            position,
            velocity: Vec3::ZERO,
            angular_velocity: 0.0,
            mass,
            restitution: 0.3,
            friction: 0.4,
            shape,
            is_static: false,
            is_trigger: false,
            collision_layer: 1,
            inv_mass: inv,
            force_accum: Vec2::ZERO,
            torque_accum: 0.0,
            angle: 0.0,
        }
    }

    /// Create a static physics object (infinite mass, no movement).
    pub fn new_static(position: Vec3, shape: CollisionShape) -> Self {
        let mut obj = Self::new(position, 0.0, shape);
        obj.is_static = true;
        obj.inv_mass = 0.0;
        obj
    }

    /// Create a trigger volume (no collision response, fires events).
    pub fn new_trigger(position: Vec3, shape: CollisionShape) -> Self {
        let mut obj = Self::new_static(position, shape);
        obj.is_trigger = true;
        obj
    }

    /// Position projected to 2D (XY plane).
    pub fn pos2d(&self) -> Vec2 {
        Vec2::new(self.position.x, self.position.y)
    }

    /// Velocity projected to 2D.
    pub fn vel2d(&self) -> Vec2 {
        Vec2::new(self.velocity.x, self.velocity.y)
    }

    /// Inverse mass (0 for static).
    pub fn inv_mass(&self) -> f32 {
        self.inv_mass
    }

    /// Compute world-space AABB.
    pub fn world_aabb(&self) -> AABB {
        let local = self.shape.local_aabb();
        let pos = self.pos2d();
        AABB {
            min: local.min + pos,
            max: local.max + pos,
        }
    }

    /// Apply a force (accumulated, applied during integration).
    pub fn apply_force(&mut self, force: Vec2) {
        self.force_accum += force;
    }

    /// Apply a torque.
    pub fn apply_torque(&mut self, torque: f32) {
        self.torque_accum += torque;
    }

    /// Apply an impulse at the center of mass.
    pub fn apply_impulse(&mut self, impulse: Vec2) {
        if self.is_static {
            return;
        }
        self.velocity.x += impulse.x * self.inv_mass;
        self.velocity.y += impulse.y * self.inv_mass;
    }

    /// Apply an impulse at a contact point.
    pub fn apply_impulse_at(&mut self, impulse: Vec2, contact_offset: Vec2) {
        if self.is_static {
            return;
        }
        self.velocity.x += impulse.x * self.inv_mass;
        self.velocity.y += impulse.y * self.inv_mass;
        self.angular_velocity += cross2d(contact_offset, impulse) * self.inv_mass;
    }

    /// Integrate forces and velocity (semi-implicit Euler).
    fn integrate(&mut self, dt: f32, gravity: Vec2, damping: f32) {
        if self.is_static {
            return;
        }
        // Apply gravity
        self.velocity.x += (gravity.x + self.force_accum.x * self.inv_mass) * dt;
        self.velocity.y += (gravity.y + self.force_accum.y * self.inv_mass) * dt;
        // Apply angular torque
        self.angular_velocity += self.torque_accum * self.inv_mass * dt;
        // Damping
        self.velocity.x *= damping;
        self.velocity.y *= damping;
        self.angular_velocity *= ANGULAR_DAMPING;
        // Integrate position
        self.position.x += self.velocity.x * dt;
        self.position.y += self.velocity.y * dt;
        self.angle += self.angular_velocity * dt;
        // Clear accumulators
        self.force_accum = Vec2::ZERO;
        self.torque_accum = 0.0;
    }
}

// ── 2D math helpers ──────────────────────────────────────────────────────────

/// 2D cross product (scalar).
#[inline]
fn cross2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Clamp a value between min and max.
#[inline]
fn clampf(val: f32, lo: f32, hi: f32) -> f32 {
    val.max(lo).min(hi)
}

/// Closest point on line segment AB to point P.
fn closest_point_on_segment(a: Vec2, b: Vec2, p: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-12 {
        return a;
    }
    let t = clampf((p - a).dot(ab) / len_sq, 0.0, 1.0);
    a + ab * t
}

// ── Collision contact ────────────────────────────────────────────────────────

/// Contact information from narrow-phase collision.
#[derive(Debug, Clone)]
pub struct Contact {
    /// Contact point in world space.
    pub point: Vec2,
    /// Contact normal (from A to B).
    pub normal: Vec2,
    /// Penetration depth (positive means overlapping).
    pub penetration: f32,
}

// ── RayHit ───────────────────────────────────────────────────────────────────

/// Result of a ray cast against the physics world.
#[derive(Debug, Clone)]
pub struct RayHit {
    pub object_id: ObjectId,
    pub point: Vec2,
    pub normal: Vec2,
    pub t: f32,
}

// ── Collision detection (narrow phase) ───────────────────────────────────────

/// Test circle vs circle collision.
fn collide_circle_circle(
    pos_a: Vec2,
    radius_a: f32,
    pos_b: Vec2,
    radius_b: f32,
) -> Option<Contact> {
    let delta = pos_b - pos_a;
    let dist_sq = delta.length_squared();
    let sum_r = radius_a + radius_b;
    if dist_sq >= sum_r * sum_r {
        return None;
    }
    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-6 {
        delta / dist
    } else {
        Vec2::new(1.0, 0.0)
    };
    let penetration = sum_r - dist;
    let point = pos_a + normal * (radius_a - penetration * 0.5);
    Some(Contact {
        point,
        normal,
        penetration,
    })
}

/// Test circle vs AABB collision.
fn collide_circle_aabb(
    circle_pos: Vec2,
    radius: f32,
    aabb_pos: Vec2,
    half_ext: Vec2,
) -> Option<Contact> {
    let local = circle_pos - aabb_pos;
    let clamped = Vec2::new(
        clampf(local.x, -half_ext.x, half_ext.x),
        clampf(local.y, -half_ext.y, half_ext.y),
    );
    let closest = aabb_pos + clamped;
    let delta = circle_pos - closest;
    let dist_sq = delta.length_squared();
    if dist_sq >= radius * radius {
        return None;
    }
    let dist = dist_sq.sqrt();
    let normal = if dist > 1e-6 {
        delta / dist
    } else {
        // Circle center inside box — push out on shortest axis
        let dx = half_ext.x - local.x.abs();
        let dy = half_ext.y - local.y.abs();
        if dx < dy {
            Vec2::new(if local.x >= 0.0 { 1.0 } else { -1.0 }, 0.0)
        } else {
            Vec2::new(0.0, if local.y >= 0.0 { 1.0 } else { -1.0 })
        }
    };
    let penetration = radius - dist;
    let point = closest;
    Some(Contact {
        point,
        normal,
        penetration,
    })
}

/// Test AABB vs AABB collision.
fn collide_aabb_aabb(
    pos_a: Vec2,
    half_a: Vec2,
    pos_b: Vec2,
    half_b: Vec2,
) -> Option<Contact> {
    let delta = pos_b - pos_a;
    let overlap_x = half_a.x + half_b.x - delta.x.abs();
    if overlap_x <= 0.0 {
        return None;
    }
    let overlap_y = half_a.y + half_b.y - delta.y.abs();
    if overlap_y <= 0.0 {
        return None;
    }
    let (normal, penetration) = if overlap_x < overlap_y {
        (
            Vec2::new(if delta.x >= 0.0 { 1.0 } else { -1.0 }, 0.0),
            overlap_x,
        )
    } else {
        (
            Vec2::new(0.0, if delta.y >= 0.0 { 1.0 } else { -1.0 }),
            overlap_y,
        )
    };
    let point = pos_a + delta * 0.5;
    Some(Contact {
        point,
        normal,
        penetration,
    })
}

/// General narrow-phase dispatch between two shapes.
fn collide_shapes(
    pos_a: Vec2,
    shape_a: &CollisionShape,
    pos_b: Vec2,
    shape_b: &CollisionShape,
) -> Option<Contact> {
    match (shape_a, shape_b) {
        (CollisionShape::Circle { radius: ra }, CollisionShape::Circle { radius: rb }) => {
            collide_circle_circle(pos_a, *ra, pos_b, *rb)
        }
        (CollisionShape::Circle { radius }, CollisionShape::AABB { half_extents }) => {
            collide_circle_aabb(pos_a, *radius, pos_b, *half_extents)
        }
        (CollisionShape::AABB { half_extents }, CollisionShape::Circle { radius }) => {
            collide_circle_aabb(pos_b, *radius, pos_a, *half_extents).map(|mut c| {
                c.normal = -c.normal;
                c
            })
        }
        (CollisionShape::AABB { half_extents: ha }, CollisionShape::AABB { half_extents: hb }) => {
            collide_aabb_aabb(pos_a, *ha, pos_b, *hb)
        }
        // Capsule collisions — approximate as circle sweep (top and bottom circles + rect)
        (CollisionShape::Capsule { radius, height }, other) => {
            let half_h = height * 0.5;
            let top = pos_a + Vec2::new(0.0, half_h);
            let bot = pos_a - Vec2::new(0.0, half_h);
            let circ = CollisionShape::Circle { radius: *radius };
            let c1 = collide_shapes(top, &circ, pos_b, other);
            let c2 = collide_shapes(bot, &circ, pos_b, other);
            // Also test the middle rectangle
            let rect = CollisionShape::AABB {
                half_extents: Vec2::new(*radius, half_h),
            };
            let c3 = collide_shapes(pos_a, &rect, pos_b, other);
            // Return deepest
            [c1, c2, c3]
                .into_iter()
                .flatten()
                .max_by(|a, b| a.penetration.partial_cmp(&b.penetration).unwrap())
        }
        (other, CollisionShape::Capsule { radius, height }) => {
            let half_h = height * 0.5;
            let top = pos_b + Vec2::new(0.0, half_h);
            let bot = pos_b - Vec2::new(0.0, half_h);
            let circ = CollisionShape::Circle { radius: *radius };
            let c1 = collide_shapes(pos_a, other, top, &circ);
            let c2 = collide_shapes(pos_a, other, bot, &circ);
            let rect = CollisionShape::AABB {
                half_extents: Vec2::new(*radius, half_h),
            };
            let c3 = collide_shapes(pos_a, other, pos_b, &rect);
            [c1, c2, c3]
                .into_iter()
                .flatten()
                .max_by(|a, b| a.penetration.partial_cmp(&b.penetration).unwrap())
        }
    }
}

/// Raycast against a single shape. Returns (t, normal).
fn raycast_shape(
    origin: Vec2,
    dir: Vec2,
    max_dist: f32,
    pos: Vec2,
    shape: &CollisionShape,
) -> Option<(f32, Vec2)> {
    match shape {
        CollisionShape::Circle { radius } => {
            let oc = origin - pos;
            let a = dir.dot(dir);
            let b = 2.0 * oc.dot(dir);
            let c = oc.dot(oc) - radius * radius;
            let disc = b * b - 4.0 * a * c;
            if disc < 0.0 {
                return None;
            }
            let sqrt_disc = disc.sqrt();
            let t = (-b - sqrt_disc) / (2.0 * a);
            if t < 0.0 || t > max_dist {
                return None;
            }
            let hit = origin + dir * t;
            let normal = (hit - pos).normalize_or_zero();
            Some((t, normal))
        }
        CollisionShape::AABB { half_extents } => {
            let aabb = AABB::from_center_half(pos, *half_extents);
            aabb.ray_intersect(origin, dir).and_then(|t| {
                if t > max_dist {
                    return None;
                }
                let hit = origin + dir * t;
                let local = hit - pos;
                // Determine face normal
                let nx = if (local.x.abs() - half_extents.x).abs() < 0.01 {
                    if local.x > 0.0 { 1.0 } else { -1.0 }
                } else {
                    0.0
                };
                let ny = if (nx as f32).abs() < 0.5 {
                    if local.y > 0.0 { 1.0 } else { -1.0 }
                } else {
                    0.0
                };
                Some((t, Vec2::new(nx, ny)))
            })
        }
        CollisionShape::Capsule { radius, height } => {
            let half_h = height * 0.5;
            // Test top circle, bottom circle, and center rect
            let top = pos + Vec2::new(0.0, half_h);
            let bot = pos - Vec2::new(0.0, half_h);
            let circ = CollisionShape::Circle { radius: *radius };
            let rect = CollisionShape::AABB {
                half_extents: Vec2::new(*radius, half_h),
            };
            let r1 = raycast_shape(origin, dir, max_dist, top, &circ);
            let r2 = raycast_shape(origin, dir, max_dist, bot, &circ);
            let r3 = raycast_shape(origin, dir, max_dist, pos, &rect);
            [r1, r2, r3]
                .into_iter()
                .flatten()
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        }
    }
}

// ── Collision pair ───────────────────────────────────────────────────────────

/// A pair of objects that are colliding.
#[derive(Debug, Clone)]
pub struct CollisionPair {
    pub id_a: ObjectId,
    pub id_b: ObjectId,
    pub contact: Contact,
}

// ── Trigger event ────────────────────────────────────────────────────────────

/// Event fired when an object enters/exits a trigger volume.
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub trigger_id: ObjectId,
    pub other_id: ObjectId,
    pub is_enter: bool,
}

// ── Damage event ─────────────────────────────────────────────────────────────

/// Damage event produced by traps hitting entities.
#[derive(Debug, Clone)]
pub struct DamageEvent {
    pub source_description: String,
    pub target_object_id: ObjectId,
    pub damage: f32,
    pub knockback: Vec2,
}

// ── PhysicsWorld ─────────────────────────────────────────────────────────────

/// Core 2D physics simulation for the arena.
pub struct PhysicsWorld {
    objects: HashMap<ObjectId, PhysicsObject>,
    next_id: u32,
    pub gravity: Vec2,
    pub damping: f32,
    // Broad-phase sorted axis endpoints (object_id -> min_x for sweep-and-prune)
    broad_cache: Vec<(ObjectId, f32, f32)>, // id, min_x, max_x
    // Previous frame triggers for enter/exit detection
    active_triggers: HashMap<(ObjectId, ObjectId), bool>,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_id: 0,
            gravity: DEFAULT_GRAVITY,
            damping: DEFAULT_DAMPING,
            broad_cache: Vec::new(),
            active_triggers: HashMap::new(),
        }
    }

    /// Add a physics object, returning its unique ID.
    pub fn add_object(&mut self, obj: PhysicsObject) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        self.objects.insert(id, obj);
        id
    }

    /// Remove a physics object.
    pub fn remove_object(&mut self, id: ObjectId) -> Option<PhysicsObject> {
        // Clean up trigger state
        self.active_triggers.retain(|k, _| k.0 != id && k.1 != id);
        self.objects.remove(&id)
    }

    /// Get a reference to an object.
    pub fn get_object(&self, id: ObjectId) -> Option<&PhysicsObject> {
        self.objects.get(&id)
    }

    /// Get a mutable reference to an object.
    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut PhysicsObject> {
        self.objects.get_mut(&id)
    }

    /// Number of active objects.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Iterate over all object IDs.
    pub fn object_ids(&self) -> Vec<ObjectId> {
        self.objects.keys().copied().collect()
    }

    /// Step the physics world forward by dt seconds.
    ///
    /// Returns (collision_pairs, trigger_events) from this step.
    pub fn step(&mut self, dt: f32) -> (Vec<CollisionPair>, Vec<TriggerEvent>) {
        // 1. Integrate velocities and positions
        let gravity = self.gravity;
        let damping = self.damping;
        for obj in self.objects.values_mut() {
            obj.integrate(dt, gravity, damping);
        }

        // 2. Broad phase: sweep-and-prune on X axis
        self.broad_cache.clear();
        for (&id, obj) in &self.objects {
            let aabb = obj.world_aabb();
            self.broad_cache.push((id, aabb.min.x, aabb.max.x));
        }
        self.broad_cache.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let mut broad_pairs: Vec<(ObjectId, ObjectId)> = Vec::new();
        for i in 0..self.broad_cache.len() {
            let (id_a, _min_a, max_a) = self.broad_cache[i];
            for j in (i + 1)..self.broad_cache.len() {
                let (id_b, min_b, _max_b) = self.broad_cache[j];
                if min_b > max_a {
                    break; // No more overlaps on X for this object
                }
                // Also check Y overlap
                let aabb_a = self.objects[&id_a].world_aabb();
                let aabb_b = self.objects[&id_b].world_aabb();
                if aabb_a.overlaps(&aabb_b) {
                    broad_pairs.push((id_a, id_b));
                }
            }
        }

        // 3. Narrow phase + response
        let mut collision_pairs = Vec::new();
        let mut trigger_events = Vec::new();
        let mut new_triggers: HashMap<(ObjectId, ObjectId), bool> = HashMap::new();

        for (id_a, id_b) in &broad_pairs {
            let obj_a = &self.objects[id_a];
            let obj_b = &self.objects[id_b];

            // Layer check
            if obj_a.collision_layer & obj_b.collision_layer == 0 {
                continue;
            }

            let pos_a = obj_a.pos2d();
            let pos_b = obj_b.pos2d();

            if let Some(contact) = collide_shapes(pos_a, &obj_a.shape, pos_b, &obj_b.shape) {
                // Trigger handling
                if obj_a.is_trigger || obj_b.is_trigger {
                    let key = if *id_a < *id_b {
                        (*id_a, *id_b)
                    } else {
                        (*id_b, *id_a)
                    };
                    new_triggers.insert(key, true);
                    if !self.active_triggers.contains_key(&key) {
                        trigger_events.push(TriggerEvent {
                            trigger_id: if obj_a.is_trigger { *id_a } else { *id_b },
                            other_id: if obj_a.is_trigger { *id_b } else { *id_a },
                            is_enter: true,
                        });
                    }
                    continue;
                }

                collision_pairs.push(CollisionPair {
                    id_a: *id_a,
                    id_b: *id_b,
                    contact: contact.clone(),
                });
            }
        }

        // Trigger exit events
        for (key, _) in &self.active_triggers {
            if !new_triggers.contains_key(key) {
                let obj_a = self.objects.get(&key.0);
                let obj_b = self.objects.get(&key.1);
                if let (Some(a), Some(b)) = (obj_a, obj_b) {
                    trigger_events.push(TriggerEvent {
                        trigger_id: if a.is_trigger { key.0 } else { key.1 },
                        other_id: if a.is_trigger { key.1 } else { key.0 },
                        is_enter: false,
                    });
                }
            }
        }
        self.active_triggers = new_triggers;

        // 4. Impulse resolution (sequential impulse solver)
        for _iter in 0..SOLVER_ITERATIONS {
            for pair in &collision_pairs {
                self.resolve_collision(pair);
            }
        }

        // 5. Position correction (Baumgarte stabilization)
        for pair in &collision_pairs {
            self.correct_positions(pair);
        }

        (collision_pairs, trigger_events)
    }

    /// Resolve collision impulse for a pair.
    fn resolve_collision(&mut self, pair: &CollisionPair) {
        let inv_a;
        let inv_b;
        let vel_a;
        let vel_b;
        let rest;
        let fric;
        {
            let a = &self.objects[&pair.id_a];
            let b = &self.objects[&pair.id_b];
            inv_a = a.inv_mass;
            inv_b = b.inv_mass;
            vel_a = a.vel2d();
            vel_b = b.vel2d();
            rest = (a.restitution + b.restitution) * 0.5;
            fric = (a.friction + b.friction) * 0.5;
        }

        if inv_a + inv_b < 1e-12 {
            return; // Both static
        }

        let n = pair.contact.normal;
        let rel_vel = vel_b - vel_a;
        let vel_along_normal = rel_vel.dot(n);

        // Don't resolve if separating
        if vel_along_normal > 0.0 {
            return;
        }

        // Impulse magnitude
        let j = -(1.0 + rest) * vel_along_normal / (inv_a + inv_b);
        let impulse = n * j;

        // Apply impulses
        if let Some(a) = self.objects.get_mut(&pair.id_a) {
            a.apply_impulse(-impulse);
        }
        if let Some(b) = self.objects.get_mut(&pair.id_b) {
            b.apply_impulse(impulse);
        }

        // Friction impulse
        let vel_a2;
        let vel_b2;
        {
            let a = &self.objects[&pair.id_a];
            let b = &self.objects[&pair.id_b];
            vel_a2 = a.vel2d();
            vel_b2 = b.vel2d();
        }
        let rel_vel2 = vel_b2 - vel_a2;
        let tangent = rel_vel2 - n * rel_vel2.dot(n);
        let tangent_len = tangent.length();
        if tangent_len > 1e-6 {
            let tangent_norm = tangent / tangent_len;
            let jt = -rel_vel2.dot(tangent_norm) / (inv_a + inv_b);
            let friction_impulse = if jt.abs() < j * fric {
                tangent_norm * jt
            } else {
                tangent_norm * (-j * fric)
            };
            if let Some(a) = self.objects.get_mut(&pair.id_a) {
                a.apply_impulse(-friction_impulse);
            }
            if let Some(b) = self.objects.get_mut(&pair.id_b) {
                b.apply_impulse(friction_impulse);
            }
        }
    }

    /// Apply position correction to prevent sinking.
    fn correct_positions(&mut self, pair: &CollisionPair) {
        let inv_a;
        let inv_b;
        {
            let a = &self.objects[&pair.id_a];
            let b = &self.objects[&pair.id_b];
            inv_a = a.inv_mass;
            inv_b = b.inv_mass;
        }
        let total_inv = inv_a + inv_b;
        if total_inv < 1e-12 {
            return;
        }
        let pen = pair.contact.penetration;
        if pen <= POSITION_SLOP {
            return;
        }
        let correction = pair.contact.normal * (POSITION_CORRECTION * (pen - POSITION_SLOP) / total_inv);
        if let Some(a) = self.objects.get_mut(&pair.id_a) {
            a.position.x -= correction.x * inv_a;
            a.position.y -= correction.y * inv_a;
        }
        if let Some(b) = self.objects.get_mut(&pair.id_b) {
            b.position.x += correction.x * inv_b;
            b.position.y += correction.y * inv_b;
        }
    }

    /// Cast a ray into the world.
    pub fn raycast(&self, origin: Vec2, dir: Vec2, max_dist: f32) -> Option<RayHit> {
        let dir_norm = if dir.length_squared() > 1e-12 {
            dir.normalize()
        } else {
            return None;
        };

        let mut best: Option<RayHit> = None;
        for (&id, obj) in &self.objects {
            if obj.is_trigger {
                continue;
            }
            if let Some((t, normal)) = raycast_shape(origin, dir_norm, max_dist, obj.pos2d(), &obj.shape) {
                if best.as_ref().map_or(true, |b| t < b.t) {
                    best = Some(RayHit {
                        object_id: id,
                        point: origin + dir_norm * t,
                        normal,
                        t,
                    });
                }
            }
        }
        best
    }

    /// Test for all objects overlapping a given shape at a position.
    pub fn overlap_test(&self, shape: &CollisionShape, pos: Vec2) -> Vec<ObjectId> {
        let mut results = Vec::new();
        for (&id, obj) in &self.objects {
            let obj_pos = obj.pos2d();
            if collide_shapes(pos, shape, obj_pos, &obj.shape).is_some() {
                results.push(id);
            }
        }
        results
    }

    /// Apply a radial force from a point (explosion, vortex).
    pub fn apply_radial_force(&mut self, center: Vec2, radius: f32, strength: f32) {
        for obj in self.objects.values_mut() {
            if obj.is_static {
                continue;
            }
            let delta = obj.pos2d() - center;
            let dist = delta.length();
            if dist < radius && dist > 1e-6 {
                let falloff = 1.0 - dist / radius;
                let force = delta.normalize() * strength * falloff;
                obj.apply_force(force);
            }
        }
    }

    /// Apply a vortex (pull toward center with tangential spin).
    pub fn apply_vortex_force(&mut self, center: Vec2, radius: f32, pull_strength: f32, spin_strength: f32) {
        for obj in self.objects.values_mut() {
            if obj.is_static {
                continue;
            }
            let delta = obj.pos2d() - center;
            let dist = delta.length();
            if dist < radius && dist > 1e-6 {
                let falloff = 1.0 - dist / radius;
                let dir = delta.normalize();
                let tangent = Vec2::new(-dir.y, dir.x);
                let force = -dir * pull_strength * falloff + tangent * spin_strength * falloff;
                obj.apply_force(force);
            }
        }
    }
}

// ── Room types ───────────────────────────────────────────────────────────────

/// Type of arena room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    Normal,
    Trap,
    Treasure,
    Boss,
    ChaosRift,
    Shop,
}

/// Exit descriptor.
#[derive(Debug, Clone)]
pub struct RoomExit {
    pub position: Vec2,
    pub direction: Vec2,
    pub target_room: Option<u32>,
}

/// A room within the arena dungeon.
#[derive(Debug, Clone)]
pub struct ArenaRoom {
    pub room_type: RoomType,
    pub bounds: AABB,
    pub physics_objects: Vec<ObjectId>,
    pub spawn_points: Vec<Vec2>,
    pub exits: Vec<RoomExit>,
    pub room_id: u32,
}

impl ArenaRoom {
    pub fn new(room_id: u32, room_type: RoomType, bounds: AABB) -> Self {
        Self {
            room_type,
            bounds,
            physics_objects: Vec::new(),
            spawn_points: Vec::new(),
            exits: Vec::new(),
            room_id,
        }
    }

    /// Add a spawn point.
    pub fn add_spawn_point(&mut self, point: Vec2) {
        self.spawn_points.push(point);
    }

    /// Add an exit.
    pub fn add_exit(&mut self, exit: RoomExit) {
        self.exits.push(exit);
    }

    /// Register a physics object as belonging to this room.
    pub fn register_object(&mut self, id: ObjectId) {
        self.physics_objects.push(id);
    }

    /// Check if a point is within room bounds.
    pub fn contains_point(&self, p: Vec2) -> bool {
        self.bounds.contains_point(p)
    }

    /// Get center of the room.
    pub fn center(&self) -> Vec2 {
        self.bounds.center()
    }

    /// Get room dimensions.
    pub fn dimensions(&self) -> Vec2 {
        self.bounds.max - self.bounds.min
    }
}

// ── Trap System ──────────────────────────────────────────────────────────────

/// State of a swinging pendulum trap.
#[derive(Debug, Clone)]
pub struct SwingingPendulum {
    pub pivot: Vec2,
    pub rope_length: f32,
    pub bob_mass: f32,
    pub angular_position: f32,
    pub angular_velocity: f32,
    pub damage: f32,
    pub bob_radius: f32,
    pub physics_object_id: Option<ObjectId>,
}

impl SwingingPendulum {
    pub fn new(pivot: Vec2, rope_length: f32, bob_mass: f32, initial_angle: f32, damage: f32) -> Self {
        Self {
            pivot,
            rope_length,
            bob_mass,
            angular_position: initial_angle,
            angular_velocity: 0.0,
            damage,
            bob_radius: 0.5,
            physics_object_id: None,
        }
    }

    /// Bob position in world space.
    pub fn bob_position(&self) -> Vec2 {
        self.pivot
            + Vec2::new(
                self.angular_position.sin() * self.rope_length,
                -self.angular_position.cos() * self.rope_length,
            )
    }

    /// Update using gravity-driven oscillation.
    pub fn update(&mut self, dt: f32, gravity: f32) {
        // Angular acceleration: alpha = -(g / L) * sin(theta)
        let alpha = -(gravity / self.rope_length) * self.angular_position.sin();
        self.angular_velocity += alpha * dt;
        // Light damping to simulate air resistance
        self.angular_velocity *= 0.999;
        self.angular_position += self.angular_velocity * dt;
    }

    /// Sync the physics object position in the world.
    pub fn sync_physics(&self, world: &mut PhysicsWorld) {
        if let Some(id) = self.physics_object_id {
            if let Some(obj) = world.get_object_mut(id) {
                let pos = self.bob_position();
                obj.position.x = pos.x;
                obj.position.y = pos.y;
            }
        }
    }

    /// Spawn the physics object into the world.
    pub fn spawn(&mut self, world: &mut PhysicsWorld) -> ObjectId {
        let pos = self.bob_position();
        let mut obj = PhysicsObject::new(
            Vec3::new(pos.x, pos.y, 0.0),
            self.bob_mass,
            CollisionShape::Circle {
                radius: self.bob_radius,
            },
        );
        obj.is_static = true; // Kinematically driven
        let id = world.add_object(obj);
        self.physics_object_id = Some(id);
        id
    }
}

/// State of a falling rock.
#[derive(Debug, Clone)]
pub struct FallingRock {
    pub object_id: ObjectId,
    pub has_shattered: bool,
    pub debris_ids: Vec<ObjectId>,
}

/// Falling rocks trap system.
#[derive(Debug, Clone)]
pub struct FallingRocks {
    pub spawn_positions: Vec<Vec2>,
    pub trigger_zone: AABB,
    pub rock_mass: f32,
    pub rock_radius: f32,
    pub damage: f32,
    pub is_triggered: bool,
    pub active_rocks: Vec<FallingRock>,
    pub debris_lifetime: f32,
    pub debris_timer: f32,
    pub shatter_threshold_velocity: f32,
}

impl FallingRocks {
    pub fn new(spawn_positions: Vec<Vec2>, trigger_zone: AABB, damage: f32) -> Self {
        Self {
            spawn_positions,
            trigger_zone,
            rock_mass: 5.0,
            rock_radius: 0.4,
            damage,
            is_triggered: false,
            active_rocks: Vec::new(),
            debris_lifetime: 3.0,
            debris_timer: 0.0,
            shatter_threshold_velocity: 5.0,
        }
    }

    /// Trigger the rock fall.
    pub fn trigger(&mut self, world: &mut PhysicsWorld) {
        if self.is_triggered {
            return;
        }
        self.is_triggered = true;
        for &pos in &self.spawn_positions {
            let obj = PhysicsObject::new(
                Vec3::new(pos.x, pos.y, 0.0),
                self.rock_mass,
                CollisionShape::Circle {
                    radius: self.rock_radius,
                },
            );
            let id = world.add_object(obj);
            self.active_rocks.push(FallingRock {
                object_id: id,
                has_shattered: false,
                debris_ids: Vec::new(),
            });
        }
    }

    /// Update: check for floor impacts and spawn debris.
    pub fn update(&mut self, dt: f32, world: &mut PhysicsWorld, floor_y: f32) {
        if !self.is_triggered {
            return;
        }

        // Check for shattering
        for rock in &mut self.active_rocks {
            if rock.has_shattered {
                continue;
            }
            if let Some(obj) = world.get_object(rock.object_id) {
                let vel = obj.vel2d().length();
                let at_floor = obj.position.y <= floor_y + self.rock_radius * 2.0;
                if at_floor && vel < self.shatter_threshold_velocity {
                    rock.has_shattered = true;
                    // Spawn debris
                    let pos = obj.pos2d();
                    let debris_count = 4;
                    for i in 0..debris_count {
                        let angle =
                            (i as f32 / debris_count as f32) * std::f32::consts::TAU;
                        let dir = Vec2::new(angle.cos(), angle.sin());
                        let mut debris = PhysicsObject::new(
                            Vec3::new(pos.x, pos.y, 0.0),
                            self.rock_mass * 0.15,
                            CollisionShape::Circle {
                                radius: self.rock_radius * 0.3,
                            },
                        );
                        debris.velocity.x = dir.x * 3.0;
                        debris.velocity.y = dir.y * 3.0 + 2.0;
                        debris.restitution = 0.5;
                        let did = world.add_object(debris);
                        rock.debris_ids.push(did);
                    }
                }
            }
        }

        // Debris lifetime
        if self.active_rocks.iter().any(|r| !r.debris_ids.is_empty()) {
            self.debris_timer += dt;
            if self.debris_timer >= self.debris_lifetime {
                for rock in &mut self.active_rocks {
                    for &did in &rock.debris_ids {
                        world.remove_object(did);
                    }
                    rock.debris_ids.clear();
                }
                self.debris_timer = 0.0;
            }
        }
    }

    /// Check if a point is in the trigger zone.
    pub fn check_trigger(&self, point: Vec2) -> bool {
        !self.is_triggered && self.trigger_zone.contains_point(point)
    }
}

/// Spike pit trap.
#[derive(Debug, Clone)]
pub struct SpikePit {
    pub trigger_zone: AABB,
    pub spike_positions: Vec<Vec2>,
    pub spike_heights: Vec<f32>,
    pub spike_velocities: Vec<f32>,
    pub target_height: f32,
    pub spring_constant: f32,
    pub spring_damping: f32,
    pub damage_per_second: f32,
    pub is_active: bool,
    pub object_ids: Vec<ObjectId>,
}

impl SpikePit {
    pub fn new(trigger_zone: AABB, spike_count: usize, target_height: f32, damage: f32) -> Self {
        let width = trigger_zone.max.x - trigger_zone.min.x;
        let spacing = width / (spike_count as f32 + 1.0);
        let base_y = trigger_zone.min.y;
        let positions: Vec<Vec2> = (0..spike_count)
            .map(|i| {
                Vec2::new(
                    trigger_zone.min.x + spacing * (i as f32 + 1.0),
                    base_y,
                )
            })
            .collect();
        Self {
            trigger_zone,
            spike_positions: positions,
            spike_heights: vec![0.0; spike_count],
            spike_velocities: vec![0.0; spike_count],
            target_height,
            spring_constant: 200.0,
            spring_damping: 8.0,
            damage_per_second: damage,
            is_active: false,
            object_ids: Vec::new(),
        }
    }

    /// Activate the spikes.
    pub fn activate(&mut self) {
        self.is_active = true;
    }

    /// Update spike physics (spring-damper system with overshoot).
    pub fn update(&mut self, dt: f32, world: &mut PhysicsWorld) {
        if !self.is_active {
            return;
        }
        for i in 0..self.spike_heights.len() {
            let displacement = self.target_height - self.spike_heights[i];
            let spring_force = self.spring_constant * displacement;
            let damping_force = -self.spring_damping * self.spike_velocities[i];
            let accel = spring_force + damping_force;
            self.spike_velocities[i] += accel * dt;
            self.spike_heights[i] += self.spike_velocities[i] * dt;
            // Clamp to not go below ground
            if self.spike_heights[i] < 0.0 {
                self.spike_heights[i] = 0.0;
                self.spike_velocities[i] = 0.0;
            }

            // Sync physics objects
            if i < self.object_ids.len() {
                if let Some(obj) = world.get_object_mut(self.object_ids[i]) {
                    obj.position.y = self.spike_positions[i].y + self.spike_heights[i] * 0.5;
                }
            }
        }
    }

    /// Spawn spike physics objects.
    pub fn spawn(&mut self, world: &mut PhysicsWorld) {
        self.object_ids.clear();
        for pos in &self.spike_positions {
            let obj = PhysicsObject::new_static(
                Vec3::new(pos.x, pos.y, 0.0),
                CollisionShape::AABB {
                    half_extents: Vec2::new(0.1, 0.01),
                },
            );
            let id = world.add_object(obj);
            self.object_ids.push(id);
        }
    }

    /// Check if a point triggers the spikes.
    pub fn check_trigger(&self, point: Vec2) -> bool {
        !self.is_active && self.trigger_zone.contains_point(point)
    }

    /// Check if a point is touching any risen spike.
    pub fn is_touching_spike(&self, point: Vec2, spike_radius: f32) -> bool {
        for (i, pos) in self.spike_positions.iter().enumerate() {
            if i >= self.spike_heights.len() {
                continue;
            }
            let h = self.spike_heights[i];
            if h < 0.1 {
                continue;
            }
            let spike_top = pos.y + h;
            let spike_aabb = AABB {
                min: Vec2::new(pos.x - spike_radius, pos.y),
                max: Vec2::new(pos.x + spike_radius, spike_top),
            };
            if spike_aabb.contains_point(point) {
                return true;
            }
        }
        false
    }
}

/// Flame jet trap.
#[derive(Debug, Clone)]
pub struct FlameJet {
    pub position: Vec2,
    pub direction: Vec2,
    pub warmup_time: f32,
    pub active_time: f32,
    pub cooldown_time: f32,
    pub cycle_timer: f32,
    pub damage_per_second: f32,
    pub jet_length: f32,
    pub jet_width: f32,
    pub particle_spawn_rate: f32,
    pub particles: Vec<FlameParticle>,
    state: FlameJetState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FlameJetState {
    Warmup,
    Active,
    Cooldown,
}

/// A single flame particle.
#[derive(Debug, Clone)]
pub struct FlameParticle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
}

impl FlameJet {
    pub fn new(position: Vec2, direction: Vec2, damage: f32) -> Self {
        Self {
            position,
            direction: direction.normalize_or_zero(),
            warmup_time: 0.5,
            active_time: 2.0,
            cooldown_time: 1.5,
            cycle_timer: 0.0,
            damage_per_second: damage,
            jet_length: 4.0,
            jet_width: 1.0,
            particle_spawn_rate: 30.0,
            particles: Vec::new(),
            state: FlameJetState::Cooldown,
        }
    }

    /// Current total cycle duration.
    fn cycle_duration(&self) -> f32 {
        self.warmup_time + self.active_time + self.cooldown_time
    }

    /// Is the jet currently firing?
    pub fn is_firing(&self) -> bool {
        self.state == FlameJetState::Active
    }

    /// Is the jet warming up?
    pub fn is_warming_up(&self) -> bool {
        self.state == FlameJetState::Warmup
    }

    /// Update the flame jet cycle and particles.
    pub fn update(&mut self, dt: f32) {
        self.cycle_timer += dt;
        let cycle = self.cycle_duration();
        if self.cycle_timer >= cycle {
            self.cycle_timer -= cycle;
        }

        self.state = if self.cycle_timer < self.warmup_time {
            FlameJetState::Warmup
        } else if self.cycle_timer < self.warmup_time + self.active_time {
            FlameJetState::Active
        } else {
            FlameJetState::Cooldown
        };

        // Spawn particles when active
        if self.is_firing() {
            let count = (self.particle_spawn_rate * dt).ceil() as usize;
            for _ in 0..count {
                let spread = (pseudo_random(self.cycle_timer) - 0.5) * self.jet_width;
                let perp = Vec2::new(-self.direction.y, self.direction.x);
                let vel = self.direction * 8.0 + perp * spread * 2.0;
                self.particles.push(FlameParticle {
                    position: self.position,
                    velocity: vel,
                    lifetime: 0.0,
                    max_lifetime: 0.5,
                    size: 0.2,
                });
            }
        }

        // Update particles
        for p in &mut self.particles {
            p.position += p.velocity * dt;
            p.velocity.y += 1.5 * dt; // Slight upward drift for fire
            p.lifetime += dt;
            p.size *= 0.98;
        }

        // Remove dead particles
        self.particles.retain(|p| p.lifetime < p.max_lifetime);
    }

    /// Check if a point is within the flame jet's damage zone.
    pub fn is_in_flame_zone(&self, point: Vec2) -> bool {
        if !self.is_firing() {
            return false;
        }
        let to_point = point - self.position;
        let along = to_point.dot(self.direction);
        if along < 0.0 || along > self.jet_length {
            return false;
        }
        let perp = Vec2::new(-self.direction.y, self.direction.x);
        let lateral = to_point.dot(perp).abs();
        // Flame cone widens with distance
        let width_at_dist = self.jet_width * 0.5 * (1.0 + along / self.jet_length);
        lateral < width_at_dist
    }
}

/// Simple pseudo-random from a seed (deterministic, fast).
fn pseudo_random(seed: f32) -> f32 {
    let x = (seed * 12.9898).sin() * 43758.5453;
    x - x.floor()
}

/// Crushing walls trap.
#[derive(Debug, Clone)]
pub struct CrushingWalls {
    pub left_wall_pos: f32,
    pub right_wall_pos: f32,
    pub left_start: f32,
    pub right_start: f32,
    pub close_speed: f32,
    pub wall_height: f32,
    pub wall_y: f32,
    pub min_gap: f32,
    pub damage_per_second: f32,
    pub is_active: bool,
    pub is_stopped: bool,
    pub push_force: f32,
    pub left_wall_id: Option<ObjectId>,
    pub right_wall_id: Option<ObjectId>,
    pub power_source_id: Option<ObjectId>,
    pub power_source_health: f32,
}

impl CrushingWalls {
    pub fn new(left_x: f32, right_x: f32, wall_y: f32, wall_height: f32, damage: f32) -> Self {
        Self {
            left_wall_pos: left_x,
            right_wall_pos: right_x,
            left_start: left_x,
            right_start: right_x,
            close_speed: 0.8,
            wall_height,
            wall_y,
            min_gap: 0.5,
            damage_per_second: damage,
            is_active: false,
            is_stopped: false,
            push_force: 50.0,
            left_wall_id: None,
            right_wall_id: None,
            power_source_id: None,
            power_source_health: 100.0,
        }
    }

    /// Activate the crushing walls.
    pub fn activate(&mut self) {
        self.is_active = true;
    }

    /// Damage the power source. Returns true if destroyed.
    pub fn damage_power_source(&mut self, amount: f32) -> bool {
        self.power_source_health -= amount;
        if self.power_source_health <= 0.0 {
            self.is_stopped = true;
            self.power_source_health = 0.0;
            true
        } else {
            false
        }
    }

    /// Spawn wall physics objects.
    pub fn spawn(&mut self, world: &mut PhysicsWorld) {
        let wall_half = Vec2::new(0.5, self.wall_height * 0.5);
        let left_obj = PhysicsObject::new_static(
            Vec3::new(self.left_wall_pos, self.wall_y, 0.0),
            CollisionShape::AABB { half_extents: wall_half },
        );
        self.left_wall_id = Some(world.add_object(left_obj));

        let right_obj = PhysicsObject::new_static(
            Vec3::new(self.right_wall_pos, self.wall_y, 0.0),
            CollisionShape::AABB { half_extents: wall_half },
        );
        self.right_wall_id = Some(world.add_object(right_obj));

        // Power source (small destructible box between walls)
        let mid_x = (self.left_wall_pos + self.right_wall_pos) * 0.5;
        let ps_obj = PhysicsObject::new_static(
            Vec3::new(mid_x, self.wall_y + self.wall_height * 0.5 + 1.0, 0.0),
            CollisionShape::AABB {
                half_extents: Vec2::new(0.3, 0.3),
            },
        );
        self.power_source_id = Some(world.add_object(ps_obj));
    }

    /// Update the crushing walls.
    pub fn update(&mut self, dt: f32, world: &mut PhysicsWorld) {
        if !self.is_active || self.is_stopped {
            return;
        }
        let gap = self.right_wall_pos - self.left_wall_pos;
        if gap > self.min_gap {
            self.left_wall_pos += self.close_speed * dt;
            self.right_wall_pos -= self.close_speed * dt;
        }
        // Sync physics objects
        if let Some(id) = self.left_wall_id {
            if let Some(obj) = world.get_object_mut(id) {
                obj.position.x = self.left_wall_pos;
            }
        }
        if let Some(id) = self.right_wall_id {
            if let Some(obj) = world.get_object_mut(id) {
                obj.position.x = self.right_wall_pos;
            }
        }
    }

    /// Check if a point is being crushed between the walls.
    pub fn is_being_crushed(&self, point: Vec2) -> bool {
        if !self.is_active || self.is_stopped {
            return false;
        }
        let in_y = point.y >= self.wall_y - self.wall_height * 0.5
            && point.y <= self.wall_y + self.wall_height * 0.5;
        let in_x = point.x >= self.left_wall_pos && point.x <= self.right_wall_pos;
        let gap = self.right_wall_pos - self.left_wall_pos;
        in_x && in_y && gap < 2.0 // Only crush when walls are close
    }

    /// Get push direction for an entity between the walls.
    pub fn push_direction(&self, point: Vec2) -> Vec2 {
        let mid = (self.left_wall_pos + self.right_wall_pos) * 0.5;
        if point.x < mid {
            Vec2::new(self.push_force, 0.0)
        } else {
            Vec2::new(-self.push_force, 0.0)
        }
    }

    /// Reset walls to starting positions.
    pub fn reset(&mut self) {
        self.left_wall_pos = self.left_start;
        self.right_wall_pos = self.right_start;
        self.is_active = false;
        self.is_stopped = false;
        self.power_source_health = 100.0;
    }
}

/// Arrow trap: fires projectiles at regular intervals.
#[derive(Debug, Clone)]
pub struct ArrowTrap {
    pub position: Vec2,
    pub direction: Vec2,
    pub fire_interval: f32,
    pub arrow_speed: f32,
    pub arrow_mass: f32,
    pub damage: f32,
    pub timer: f32,
    pub active_arrows: Vec<ObjectId>,
    pub max_arrows: usize,
    pub arrow_lifetime: f32,
    pub arrow_timers: Vec<f32>,
}

impl ArrowTrap {
    pub fn new(position: Vec2, direction: Vec2, fire_interval: f32, damage: f32) -> Self {
        Self {
            position,
            direction: direction.normalize_or_zero(),
            fire_interval,
            arrow_speed: 12.0,
            arrow_mass: 0.2,
            damage,
            timer: 0.0,
            active_arrows: Vec::new(),
            max_arrows: 10,
            arrow_lifetime: 5.0,
            arrow_timers: Vec::new(),
        }
    }

    /// Update the arrow trap, firing if interval elapsed.
    pub fn update(&mut self, dt: f32, world: &mut PhysicsWorld) {
        self.timer += dt;

        // Fire arrow if interval elapsed
        if self.timer >= self.fire_interval {
            self.timer -= self.fire_interval;
            if self.active_arrows.len() < self.max_arrows {
                let mut arrow = PhysicsObject::new(
                    Vec3::new(self.position.x, self.position.y, 0.0),
                    self.arrow_mass,
                    CollisionShape::Capsule {
                        radius: 0.05,
                        height: 0.4,
                    },
                );
                arrow.velocity.x = self.direction.x * self.arrow_speed;
                arrow.velocity.y = self.direction.y * self.arrow_speed;
                arrow.restitution = 0.1;
                arrow.collision_layer = 0xFFFF_FFFF;
                let id = world.add_object(arrow);
                self.active_arrows.push(id);
                self.arrow_timers.push(0.0);
            }
        }

        // Update arrow lifetimes
        let mut to_remove = Vec::new();
        for i in 0..self.arrow_timers.len() {
            self.arrow_timers[i] += dt;
            if self.arrow_timers[i] >= self.arrow_lifetime {
                to_remove.push(i);
            }
        }

        // Remove expired arrows (reverse order to maintain indices)
        for &i in to_remove.iter().rev() {
            if i < self.active_arrows.len() {
                world.remove_object(self.active_arrows[i]);
                self.active_arrows.remove(i);
                self.arrow_timers.remove(i);
            }
        }
    }
}

/// Aggregated trap system for a room.
#[derive(Debug, Clone)]
pub struct TrapSystem {
    pub pendulums: Vec<SwingingPendulum>,
    pub falling_rocks: Vec<FallingRocks>,
    pub spike_pits: Vec<SpikePit>,
    pub flame_jets: Vec<FlameJet>,
    pub crushing_walls: Vec<CrushingWalls>,
    pub arrow_traps: Vec<ArrowTrap>,
}

impl TrapSystem {
    pub fn new() -> Self {
        Self {
            pendulums: Vec::new(),
            falling_rocks: Vec::new(),
            spike_pits: Vec::new(),
            flame_jets: Vec::new(),
            crushing_walls: Vec::new(),
            arrow_traps: Vec::new(),
        }
    }

    /// Update all traps in the system.
    pub fn update(&mut self, dt: f32, world: &mut PhysicsWorld, floor_y: f32) {
        let gravity = world.gravity.y.abs();

        for pendulum in &mut self.pendulums {
            pendulum.update(dt, gravity);
            pendulum.sync_physics(world);
        }

        for rocks in &mut self.falling_rocks {
            rocks.update(dt, world, floor_y);
        }

        for spikes in &mut self.spike_pits {
            spikes.update(dt, world);
        }

        for jet in &mut self.flame_jets {
            jet.update(dt);
        }

        for walls in &mut self.crushing_walls {
            walls.update(dt, world);
        }

        for arrows in &mut self.arrow_traps {
            arrows.update(dt, world);
        }
    }

    /// Collect damage events for an entity at a given position.
    pub fn check_damage(&self, entity_pos: Vec2, entity_id: ObjectId) -> Vec<DamageEvent> {
        let mut events = Vec::new();

        for pendulum in &self.pendulums {
            let bob_pos = pendulum.bob_position();
            let dist = (entity_pos - bob_pos).length();
            if dist < pendulum.bob_radius + 0.5 {
                let knockback = (entity_pos - bob_pos).normalize_or_zero() * 5.0;
                events.push(DamageEvent {
                    source_description: "Swinging Pendulum".to_string(),
                    target_object_id: entity_id,
                    damage: pendulum.damage,
                    knockback,
                });
            }
        }

        for jet in &self.flame_jets {
            if jet.is_in_flame_zone(entity_pos) {
                events.push(DamageEvent {
                    source_description: "Flame Jet".to_string(),
                    target_object_id: entity_id,
                    damage: jet.damage_per_second,
                    knockback: jet.direction * 2.0,
                });
            }
        }

        for spikes in &self.spike_pits {
            if spikes.is_touching_spike(entity_pos, 0.15) {
                events.push(DamageEvent {
                    source_description: "Spike Pit".to_string(),
                    target_object_id: entity_id,
                    damage: spikes.damage_per_second,
                    knockback: Vec2::new(0.0, 3.0),
                });
            }
        }

        for walls in &self.crushing_walls {
            if walls.is_being_crushed(entity_pos) {
                let push = walls.push_direction(entity_pos);
                events.push(DamageEvent {
                    source_description: "Crushing Walls".to_string(),
                    target_object_id: entity_id,
                    damage: walls.damage_per_second,
                    knockback: push,
                });
            }
        }

        events
    }

    /// Check trigger zones for an entity position (activates traps).
    pub fn check_triggers(&mut self, entity_pos: Vec2, world: &mut PhysicsWorld) {
        for rocks in &mut self.falling_rocks {
            if rocks.check_trigger(entity_pos) {
                rocks.trigger(world);
            }
        }

        for spikes in &mut self.spike_pits {
            if spikes.check_trigger(entity_pos) {
                spikes.activate();
            }
        }
    }
}

// ── Treasure Room ────────────────────────────────────────────────────────────

/// Treasure chest with hinge-lid physics.
#[derive(Debug, Clone)]
pub struct TreasureChest {
    pub position: Vec2,
    pub lid_angle: f32,
    pub lid_angular_velocity: f32,
    pub lid_spring_constant: f32,
    pub lid_damping: f32,
    pub is_open: bool,
    pub target_angle: f32,
    pub loot_items: Vec<ObjectId>,
    pub body_object_id: Option<ObjectId>,
    pub loot_scatter_speed: f32,
}

impl TreasureChest {
    pub fn new(position: Vec2) -> Self {
        Self {
            position,
            lid_angle: 0.0,
            lid_angular_velocity: 0.0,
            lid_spring_constant: 80.0,
            lid_damping: 6.0,
            is_open: false,
            target_angle: std::f32::consts::FRAC_PI_2 * 1.5, // ~135 degrees open
            loot_items: Vec::new(),
            body_object_id: None,
            loot_scatter_speed: 4.0,
        }
    }

    /// Spawn the chest body physics object.
    pub fn spawn(&mut self, world: &mut PhysicsWorld) -> ObjectId {
        let obj = PhysicsObject::new_static(
            Vec3::new(self.position.x, self.position.y, 0.0),
            CollisionShape::AABB {
                half_extents: Vec2::new(0.5, 0.3),
            },
        );
        let id = world.add_object(obj);
        self.body_object_id = Some(id);
        id
    }

    /// Open the chest and spawn loot items.
    pub fn open(&mut self, world: &mut PhysicsWorld, loot_count: usize) {
        if self.is_open {
            return;
        }
        self.is_open = true;

        // Spawn loot items
        for i in 0..loot_count {
            let angle = (i as f32 / loot_count as f32) * std::f32::consts::PI + 0.1;
            let dir = Vec2::new(angle.cos(), angle.sin());
            let mut item = PhysicsObject::new(
                Vec3::new(self.position.x, self.position.y + 0.5, 0.0),
                0.3,
                CollisionShape::Circle { radius: 0.15 },
            );
            item.velocity.x = dir.x * self.loot_scatter_speed;
            item.velocity.y = dir.y * self.loot_scatter_speed + 2.0;
            item.friction = 0.8; // High friction so items settle
            item.restitution = 0.3;
            let id = world.add_object(item);
            self.loot_items.push(id);
        }
    }

    /// Update lid angular spring physics.
    pub fn update(&mut self, dt: f32) {
        if !self.is_open {
            return;
        }
        let displacement = self.target_angle - self.lid_angle;
        let spring_torque = self.lid_spring_constant * displacement;
        let damping_torque = -self.lid_damping * self.lid_angular_velocity;
        let accel = spring_torque + damping_torque;
        self.lid_angular_velocity += accel * dt;
        self.lid_angle += self.lid_angular_velocity * dt;
        // Clamp
        self.lid_angle = clampf(self.lid_angle, 0.0, self.target_angle + 0.1);
    }
}

/// Pedestal item that bobs up and down.
#[derive(Debug, Clone)]
pub struct PedestalItem {
    pub base_position: Vec2,
    pub bob_amplitude: f32,
    pub bob_frequency: f32,
    pub bob_timer: f32,
    pub halo_radius: f32,
    pub halo_particle_count: usize,
    pub collision_object_id: Option<ObjectId>,
    pub item_name: String,
}

impl PedestalItem {
    pub fn new(position: Vec2, item_name: String) -> Self {
        Self {
            base_position: position,
            bob_amplitude: 0.3,
            bob_frequency: 2.0,
            bob_timer: 0.0,
            halo_radius: 0.6,
            halo_particle_count: 8,
            collision_object_id: None,
            item_name,
        }
    }

    /// Spawn the collision object.
    pub fn spawn(&mut self, world: &mut PhysicsWorld) -> ObjectId {
        let obj = PhysicsObject::new_static(
            Vec3::new(self.base_position.x, self.base_position.y, 0.0),
            CollisionShape::Circle { radius: 0.3 },
        );
        let id = world.add_object(obj);
        self.collision_object_id = Some(id);
        id
    }

    /// Current display position (with bob).
    pub fn display_position(&self) -> Vec2 {
        let offset = (self.bob_timer * self.bob_frequency * std::f32::consts::TAU).sin()
            * self.bob_amplitude;
        Vec2::new(self.base_position.x, self.base_position.y + offset)
    }

    /// Get halo particle positions (golden ring around item).
    pub fn halo_positions(&self) -> Vec<Vec2> {
        let center = self.display_position();
        (0..self.halo_particle_count)
            .map(|i| {
                let angle = (i as f32 / self.halo_particle_count as f32) * std::f32::consts::TAU
                    + self.bob_timer * 1.5;
                center + Vec2::new(angle.cos(), angle.sin()) * self.halo_radius
            })
            .collect()
    }

    /// Update bob timer.
    pub fn update(&mut self, dt: f32) {
        self.bob_timer += dt;
    }
}

/// Treasure room aggregator.
#[derive(Debug, Clone)]
pub struct TreasureRoom {
    pub chests: Vec<TreasureChest>,
    pub pedestals: Vec<PedestalItem>,
}

impl TreasureRoom {
    pub fn new() -> Self {
        Self {
            chests: Vec::new(),
            pedestals: Vec::new(),
        }
    }

    pub fn update(&mut self, dt: f32) {
        for chest in &mut self.chests {
            chest.update(dt);
        }
        for pedestal in &mut self.pedestals {
            pedestal.update(dt);
        }
    }
}

// ── Chaos Rift Room ──────────────────────────────────────────────────────────

/// A rift object spawned from the chaos portal.
#[derive(Debug, Clone)]
pub struct RiftObject {
    pub object_id: ObjectId,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Chaos rift room: portal that spawns random physics objects.
#[derive(Debug, Clone)]
pub struct ChaosRiftRoom {
    pub rift_center: Vec2,
    pub rift_radius: f32,
    pub vortex_pull_strength: f32,
    pub vortex_spin_strength: f32,
    pub spawn_rate: f32,
    pub spawn_rate_escalation: f32,
    pub max_spawn_rate: f32,
    pub spawn_timer: f32,
    pub elapsed: f32,
    pub active_objects: Vec<RiftObject>,
    pub max_objects: usize,
    pub despawn_burst_count: usize,
    pub seed: f32,
}

impl ChaosRiftRoom {
    pub fn new(center: Vec2, radius: f32) -> Self {
        Self {
            rift_center: center,
            rift_radius: radius,
            vortex_pull_strength: 15.0,
            vortex_spin_strength: 8.0,
            spawn_rate: 1.0,
            spawn_rate_escalation: 0.1,
            max_spawn_rate: 10.0,
            spawn_timer: 0.0,
            elapsed: 0.0,
            active_objects: Vec::new(),
            max_objects: 50,
            despawn_burst_count: 6,
            seed: 42.0,
        }
    }

    /// Update the chaos rift, spawning objects and applying vortex.
    pub fn update(&mut self, dt: f32, world: &mut PhysicsWorld) {
        self.elapsed += dt;

        // Escalate spawn rate
        let current_rate = (self.spawn_rate + self.spawn_rate_escalation * self.elapsed)
            .min(self.max_spawn_rate);
        let spawn_interval = 1.0 / current_rate;

        self.spawn_timer += dt;

        // Spawn objects
        while self.spawn_timer >= spawn_interval && self.active_objects.len() < self.max_objects {
            self.spawn_timer -= spawn_interval;
            self.seed += 1.0;

            let angle = pseudo_random(self.seed * 0.7) * std::f32::consts::TAU;
            let speed = 2.0 + pseudo_random(self.seed * 1.3) * 5.0;
            let dir = Vec2::new(angle.cos(), angle.sin());

            // Random shape
            let shape_rand = pseudo_random(self.seed * 2.1);
            let shape = if shape_rand < 0.33 {
                let r = 0.1 + pseudo_random(self.seed * 3.7) * 0.5;
                CollisionShape::Circle { radius: r }
            } else if shape_rand < 0.66 {
                let w = 0.1 + pseudo_random(self.seed * 4.3) * 0.4;
                let h = 0.1 + pseudo_random(self.seed * 5.1) * 0.4;
                CollisionShape::AABB {
                    half_extents: Vec2::new(w, h),
                }
            } else {
                let r = 0.08 + pseudo_random(self.seed * 6.2) * 0.2;
                let h = 0.2 + pseudo_random(self.seed * 7.4) * 0.5;
                CollisionShape::Capsule { radius: r, height: h }
            };

            let mass = 0.5 + pseudo_random(self.seed * 8.9) * 3.0;
            let mut obj = PhysicsObject::new(
                Vec3::new(self.rift_center.x, self.rift_center.y, 0.0),
                mass,
                shape,
            );
            obj.velocity.x = dir.x * speed;
            obj.velocity.y = dir.y * speed;
            obj.restitution = 0.6;

            let id = world.add_object(obj);
            self.active_objects.push(RiftObject {
                object_id: id,
                lifetime: 0.0,
                max_lifetime: 8.0 + pseudo_random(self.seed * 9.1) * 4.0,
            });
        }

        // Apply vortex force
        world.apply_vortex_force(
            self.rift_center,
            self.rift_radius * 3.0,
            self.vortex_pull_strength,
            self.vortex_spin_strength,
        );

        // Check for objects touching the rift center (despawn with burst)
        let mut to_despawn = Vec::new();
        for (i, rift_obj) in self.active_objects.iter_mut().enumerate() {
            rift_obj.lifetime += dt;
            if let Some(obj) = world.get_object(rift_obj.object_id) {
                let dist = (obj.pos2d() - self.rift_center).length();
                if dist < self.rift_radius * 0.3 || rift_obj.lifetime >= rift_obj.max_lifetime {
                    to_despawn.push(i);
                }
            } else {
                to_despawn.push(i);
            }
        }

        // Despawn (reverse order)
        for &i in to_despawn.iter().rev() {
            if i < self.active_objects.len() {
                let rift_obj = self.active_objects.remove(i);
                world.remove_object(rift_obj.object_id);
            }
        }
    }

    /// Current effective spawn rate.
    pub fn current_spawn_rate(&self) -> f32 {
        (self.spawn_rate + self.spawn_rate_escalation * self.elapsed).min(self.max_spawn_rate)
    }

    /// Number of active rift objects.
    pub fn active_count(&self) -> usize {
        self.active_objects.len()
    }
}

// ── ArenaPhysicsManager ──────────────────────────────────────────────────────

/// Callback type for collision events.
pub type CollisionCallback = fn(ObjectId, ObjectId, &Contact);

/// Top-level manager that owns the PhysicsWorld, trap systems, and room state.
pub struct ArenaPhysicsManager {
    pub world: PhysicsWorld,
    pub rooms: Vec<ArenaRoom>,
    pub trap_systems: HashMap<u32, TrapSystem>,
    pub treasure_rooms: HashMap<u32, TreasureRoom>,
    pub chaos_rifts: HashMap<u32, ChaosRiftRoom>,
    pub damage_events: Vec<DamageEvent>,
    pub collision_callbacks: Vec<CollisionCallback>,
    pub floor_y: f32,
}

impl ArenaPhysicsManager {
    pub fn new() -> Self {
        Self {
            world: PhysicsWorld::new(),
            rooms: Vec::new(),
            trap_systems: HashMap::new(),
            treasure_rooms: HashMap::new(),
            chaos_rifts: HashMap::new(),
            damage_events: Vec::new(),
            collision_callbacks: Vec::new(),
            floor_y: 0.0,
        }
    }

    /// Set the global floor Y coordinate.
    pub fn set_floor(&mut self, y: f32) {
        self.floor_y = y;
    }

    /// Add a room to the arena.
    pub fn add_room(&mut self, room: ArenaRoom) -> u32 {
        let id = room.room_id;
        self.rooms.push(room);
        id
    }

    /// Register a trap system for a room.
    pub fn register_trap_system(&mut self, room_id: u32, system: TrapSystem) {
        self.trap_systems.insert(room_id, system);
    }

    /// Register a treasure room.
    pub fn register_treasure_room(&mut self, room_id: u32, treasure: TreasureRoom) {
        self.treasure_rooms.insert(room_id, treasure);
    }

    /// Register a chaos rift room.
    pub fn register_chaos_rift(&mut self, room_id: u32, rift: ChaosRiftRoom) {
        self.chaos_rifts.insert(room_id, rift);
    }

    /// Register a collision callback.
    pub fn on_collision(&mut self, callback: CollisionCallback) {
        self.collision_callbacks.push(callback);
    }

    /// Step the entire arena physics forward.
    pub fn step(&mut self, dt: f32) -> Vec<DamageEvent> {
        self.damage_events.clear();

        // Step physics world
        let (collisions, _triggers) = self.world.step(dt);

        // Fire collision callbacks
        for pair in &collisions {
            for cb in &self.collision_callbacks {
                cb(pair.id_a, pair.id_b, &pair.contact);
            }
        }

        // Update trap systems
        let floor_y = self.floor_y;
        for (_, system) in &mut self.trap_systems {
            system.update(dt, &mut self.world, floor_y);
        }

        // Update treasure rooms
        for (_, treasure) in &mut self.treasure_rooms {
            treasure.update(dt);
        }

        // Update chaos rifts
        for (_, rift) in &mut self.chaos_rifts {
            rift.update(dt, &mut self.world);
        }

        self.damage_events.clone()
    }

    /// Check trap damage for an entity.
    pub fn check_entity_damage(&self, entity_pos: Vec2, entity_id: ObjectId) -> Vec<DamageEvent> {
        let mut all_damage = Vec::new();
        for (_, system) in &self.trap_systems {
            let events = system.check_damage(entity_pos, entity_id);
            all_damage.extend(events);
        }
        all_damage
    }

    /// Notify traps that an entity is at a position (for trigger zones).
    pub fn notify_entity_position(&mut self, entity_pos: Vec2) {
        for (_, system) in &mut self.trap_systems {
            system.check_triggers(entity_pos, &mut self.world);
        }
    }

    /// Raycast through the arena.
    pub fn raycast(&self, origin: Vec2, dir: Vec2, max_dist: f32) -> Option<RayHit> {
        self.world.raycast(origin, dir, max_dist)
    }

    /// Overlap test in the arena.
    pub fn overlap_test(&self, shape: &CollisionShape, pos: Vec2) -> Vec<ObjectId> {
        self.world.overlap_test(shape, pos)
    }

    /// Find which room contains a point.
    pub fn room_at(&self, pos: Vec2) -> Option<&ArenaRoom> {
        self.rooms.iter().find(|r| r.contains_point(pos))
    }

    /// Get total active physics objects.
    pub fn total_objects(&self) -> usize {
        self.world.object_count()
    }
}

// ── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- Collision detection tests --

    #[test]
    fn test_circle_circle_collision() {
        let c = collide_circle_circle(Vec2::ZERO, 1.0, Vec2::new(1.5, 0.0), 1.0);
        assert!(c.is_some());
        let c = c.unwrap();
        assert!((c.penetration - 0.5).abs() < 0.01);
        assert!(c.normal.x > 0.9);
    }

    #[test]
    fn test_circle_circle_no_collision() {
        let c = collide_circle_circle(Vec2::ZERO, 1.0, Vec2::new(3.0, 0.0), 1.0);
        assert!(c.is_none());
    }

    #[test]
    fn test_aabb_aabb_collision() {
        let c = collide_aabb_aabb(
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            Vec2::new(1.5, 0.0),
            Vec2::new(1.0, 1.0),
        );
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(c.penetration > 0.0);
    }

    #[test]
    fn test_aabb_aabb_no_collision() {
        let c = collide_aabb_aabb(
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(1.0, 1.0),
        );
        assert!(c.is_none());
    }

    #[test]
    fn test_circle_aabb_collision() {
        let c = collide_circle_aabb(Vec2::new(1.8, 0.0), 0.5, Vec2::ZERO, Vec2::new(1.0, 1.0));
        assert!(c.is_some());
    }

    #[test]
    fn test_circle_aabb_no_collision() {
        let c = collide_circle_aabb(Vec2::new(3.0, 0.0), 0.5, Vec2::ZERO, Vec2::new(1.0, 1.0));
        assert!(c.is_none());
    }

    // -- Physics world tests --

    #[test]
    fn test_add_remove_objects() {
        let mut world = PhysicsWorld::new();
        let id = world.add_object(PhysicsObject::new(
            Vec3::ZERO,
            1.0,
            CollisionShape::Circle { radius: 1.0 },
        ));
        assert_eq!(world.object_count(), 1);
        world.remove_object(id);
        assert_eq!(world.object_count(), 0);
    }

    #[test]
    fn test_gravity_integration() {
        let mut world = PhysicsWorld::new();
        let id = world.add_object(PhysicsObject::new(
            Vec3::new(0.0, 10.0, 0.0),
            1.0,
            CollisionShape::Circle { radius: 0.5 },
        ));
        // Step for 1 second in 10 steps
        for _ in 0..10 {
            world.step(0.1);
        }
        let obj = world.get_object(id).unwrap();
        // Object should have fallen
        assert!(obj.position.y < 10.0);
    }

    #[test]
    fn test_impulse_resolution() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO; // Disable gravity for this test

        // Two circles moving toward each other
        let id_a = world.add_object({
            let mut obj = PhysicsObject::new(
                Vec3::new(-1.0, 0.0, 0.0),
                1.0,
                CollisionShape::Circle { radius: 1.0 },
            );
            obj.velocity.x = 5.0;
            obj
        });
        let id_b = world.add_object({
            let mut obj = PhysicsObject::new(
                Vec3::new(1.0, 0.0, 0.0),
                1.0,
                CollisionShape::Circle { radius: 1.0 },
            );
            obj.velocity.x = -5.0;
            obj
        });

        world.step(0.016);

        let a = world.get_object(id_a).unwrap();
        let b = world.get_object(id_b).unwrap();
        // After collision, they should be separating
        assert!(a.velocity.x <= 0.0);
        assert!(b.velocity.x >= 0.0);
    }

    #[test]
    fn test_static_object_immovable() {
        let mut world = PhysicsWorld::new();
        let id = world.add_object(PhysicsObject::new_static(
            Vec3::new(0.0, 0.0, 0.0),
            CollisionShape::AABB {
                half_extents: Vec2::new(5.0, 0.5),
            },
        ));
        world.step(1.0);
        let obj = world.get_object(id).unwrap();
        assert!((obj.position.y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_raycast() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;
        world.add_object(PhysicsObject::new_static(
            Vec3::new(5.0, 0.0, 0.0),
            CollisionShape::Circle { radius: 1.0 },
        ));
        let hit = world.raycast(Vec2::ZERO, Vec2::new(1.0, 0.0), 100.0);
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert!((hit.point.x - 4.0).abs() < 0.1);
    }

    #[test]
    fn test_overlap_test() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;
        let id = world.add_object(PhysicsObject::new_static(
            Vec3::new(0.0, 0.0, 0.0),
            CollisionShape::Circle { radius: 2.0 },
        ));
        let results = world.overlap_test(&CollisionShape::Circle { radius: 1.0 }, Vec2::new(1.0, 0.0));
        assert!(results.contains(&id));
    }

    #[test]
    fn test_trigger_events() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;

        // Trigger volume
        world.add_object(PhysicsObject::new_trigger(
            Vec3::new(0.0, 0.0, 0.0),
            CollisionShape::Circle { radius: 2.0 },
        ));

        // Dynamic object entering trigger
        let mut entering = PhysicsObject::new(
            Vec3::new(0.5, 0.0, 0.0),
            1.0,
            CollisionShape::Circle { radius: 0.5 },
        );
        entering.collision_layer = 1;
        world.add_object(entering);

        let (_, triggers) = world.step(0.016);
        assert!(!triggers.is_empty());
        assert!(triggers[0].is_enter);
    }

    // -- Trap timing tests --

    #[test]
    fn test_pendulum_oscillation() {
        let mut pendulum = SwingingPendulum::new(Vec2::new(0.0, 5.0), 3.0, 2.0, 0.5, 10.0);
        let initial_angle = pendulum.angular_position;
        // Run for a bit
        for _ in 0..100 {
            pendulum.update(0.016, 9.81);
        }
        // Should have oscillated (angle changed)
        assert!((pendulum.angular_position - initial_angle).abs() > 0.01);
    }

    #[test]
    fn test_spike_pit_spring_physics() {
        let trigger_zone = AABB::new(Vec2::new(-2.0, -1.0), Vec2::new(2.0, 0.0));
        let mut spikes = SpikePit::new(trigger_zone, 4, 1.5, 20.0);
        let mut world = PhysicsWorld::new();
        spikes.spawn(&mut world);
        spikes.activate();

        // Run spring simulation
        for _ in 0..200 {
            spikes.update(0.016, &mut world);
        }

        // Spikes should have risen close to target
        for h in &spikes.spike_heights {
            assert!((*h - 1.5).abs() < 0.3, "Spike height {} not near target 1.5", h);
        }
    }

    #[test]
    fn test_flame_jet_cycle() {
        let mut jet = FlameJet::new(Vec2::ZERO, Vec2::new(1.0, 0.0), 15.0);
        // Run through a full cycle
        let mut was_firing = false;
        let mut was_warming = false;
        for _ in 0..300 {
            jet.update(0.016);
            if jet.is_firing() {
                was_firing = true;
            }
            if jet.is_warming_up() {
                was_warming = true;
            }
        }
        assert!(was_firing, "Flame jet should have fired");
        assert!(was_warming, "Flame jet should have warmed up");
    }

    #[test]
    fn test_crushing_walls_close() {
        let mut walls = CrushingWalls::new(-5.0, 5.0, 0.0, 4.0, 30.0);
        let mut world = PhysicsWorld::new();
        walls.spawn(&mut world);
        walls.activate();

        let initial_gap = walls.right_wall_pos - walls.left_wall_pos;
        for _ in 0..100 {
            walls.update(0.016, &mut world);
        }
        let final_gap = walls.right_wall_pos - walls.left_wall_pos;
        assert!(final_gap < initial_gap, "Walls should have closed in");
    }

    #[test]
    fn test_crushing_walls_stop_on_power_source_destroy() {
        let mut walls = CrushingWalls::new(-5.0, 5.0, 0.0, 4.0, 30.0);
        let mut world = PhysicsWorld::new();
        walls.spawn(&mut world);
        walls.activate();

        // Run a bit
        for _ in 0..50 {
            walls.update(0.016, &mut world);
        }
        let gap_before = walls.right_wall_pos - walls.left_wall_pos;

        // Destroy power source
        walls.damage_power_source(200.0);
        assert!(walls.is_stopped);

        // Run more — gap should not change
        for _ in 0..50 {
            walls.update(0.016, &mut world);
        }
        let gap_after = walls.right_wall_pos - walls.left_wall_pos;
        assert!((gap_before - gap_after).abs() < 0.001);
    }

    #[test]
    fn test_arrow_trap_fires() {
        let mut trap = ArrowTrap::new(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, 10.0);
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;

        // Run for 2 seconds
        for _ in 0..125 {
            trap.update(0.016, &mut world);
        }
        assert!(
            !trap.active_arrows.is_empty(),
            "Arrow trap should have fired at least one arrow"
        );
    }

    #[test]
    fn test_treasure_chest_open() {
        let mut chest = TreasureChest::new(Vec2::ZERO);
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;
        chest.spawn(&mut world);
        chest.open(&mut world, 5);
        assert!(chest.is_open);
        assert_eq!(chest.loot_items.len(), 5);
        // Run spring
        for _ in 0..100 {
            chest.update(0.016);
        }
        assert!(chest.lid_angle > 0.5, "Lid should have opened");
    }

    #[test]
    fn test_chaos_rift_spawns_objects() {
        let mut rift = ChaosRiftRoom::new(Vec2::ZERO, 3.0);
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;

        // Run for several seconds
        for _ in 0..200 {
            rift.update(0.016, &mut world);
        }
        assert!(
            rift.active_count() > 0,
            "Chaos rift should have spawned objects"
        );
    }

    #[test]
    fn test_chaos_rift_escalation() {
        let mut rift = ChaosRiftRoom::new(Vec2::ZERO, 3.0);
        let rate_initial = rift.current_spawn_rate();
        rift.elapsed = 30.0;
        let rate_later = rift.current_spawn_rate();
        assert!(rate_later > rate_initial, "Spawn rate should escalate");
    }

    #[test]
    fn test_arena_manager_step() {
        let mut mgr = ArenaPhysicsManager::new();
        mgr.world.gravity = Vec2::ZERO;

        let room = ArenaRoom::new(0, RoomType::Trap, AABB::new(Vec2::new(-10.0, -10.0), Vec2::new(10.0, 10.0)));
        mgr.add_room(room);

        let mut traps = TrapSystem::new();
        traps.pendulums.push(SwingingPendulum::new(
            Vec2::new(0.0, 5.0),
            3.0,
            2.0,
            0.5,
            10.0,
        ));
        mgr.register_trap_system(0, traps);

        // Step should not panic
        for _ in 0..60 {
            mgr.step(0.016);
        }
    }

    #[test]
    fn test_aabb_helpers() {
        let a = AABB::new(Vec2::ZERO, Vec2::new(2.0, 2.0));
        assert!(a.contains_point(Vec2::new(1.0, 1.0)));
        assert!(!a.contains_point(Vec2::new(3.0, 1.0)));
        assert_eq!(a.center(), Vec2::new(1.0, 1.0));
        assert_eq!(a.half_extents(), Vec2::new(1.0, 1.0));
    }

    #[test]
    fn test_pedestal_item_bob() {
        let mut pedestal = PedestalItem::new(Vec2::new(0.0, 3.0), "Magic Sword".to_string());
        pedestal.update(0.5);
        let pos = pedestal.display_position();
        // Should have bobbed from base
        assert!((pos.y - 3.0).abs() <= pedestal.bob_amplitude + 0.01);
    }

    #[test]
    fn test_radial_force() {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec2::ZERO;
        let id = world.add_object(PhysicsObject::new(
            Vec3::new(2.0, 0.0, 0.0),
            1.0,
            CollisionShape::Circle { radius: 0.5 },
        ));
        world.apply_radial_force(Vec2::ZERO, 5.0, 100.0);
        world.step(0.016);
        let obj = world.get_object(id).unwrap();
        // Should have been pushed away from origin
        assert!(obj.velocity.x > 0.0);
    }
}
