//! Full boid/flocking simulation with obstacle avoidance, leader following, and predator flee.
//!
//! This module implements Craig Reynolds' Boids rules extended with:
//!  - Weighted steering behaviors (alignment, cohesion, separation)
//!  - Obstacle avoidance (sphere obstacles)
//!  - Leader following (follow a target point with arrival behavior)
//!  - Predator avoidance (flee from predator positions)
//!  - Speed limits and steering force clamping
//!  - Wall containment (keep boids within a bounding box)
//!  - Wander behavior (noise-driven random steering when idle)

use glam::Vec3;
use std::f32::consts::TAU;

// ── Neighbor ──────────────────────────────────────────────────────────────────

/// A neighboring boid for computing steering forces.
#[derive(Clone, Debug)]
pub struct FlockNeighbor {
    pub position: Vec3,
    pub velocity: Vec3,
}

// ── Obstacle ──────────────────────────────────────────────────────────────────

/// A spherical obstacle to avoid.
#[derive(Clone, Debug)]
pub struct Obstacle {
    pub center: Vec3,
    pub radius: f32,
}

impl Obstacle {
    pub fn new(center: Vec3, radius: f32) -> Self { Self { center, radius } }

    /// Returns how much this obstacle overlaps with a point (0 = no overlap).
    pub fn overlap(&self, pos: Vec3) -> f32 {
        let dist = (pos - self.center).length();
        (self.radius - dist).max(0.0)
    }
}

// ── Flock config ──────────────────────────────────────────────────────────────

/// Weights and limits for the boid steering behaviors.
#[derive(Clone, Debug)]
pub struct FlockConfig {
    // ── Weights ───────────────────────────────────────────────────────────────
    /// Align velocity with neighbors.
    pub alignment_weight:   f32,
    /// Move toward neighbor center of mass.
    pub cohesion_weight:    f32,
    /// Steer away from nearby boids.
    pub separation_weight:  f32,
    /// Avoid obstacles.
    pub avoidance_weight:   f32,
    /// Follow a leader/target.
    pub leader_weight:      f32,
    /// Flee from predators.
    pub flee_weight:        f32,
    /// Random wander steering.
    pub wander_weight:      f32,
    /// Stay inside bounding box.
    pub containment_weight: f32,

    // ── Radii ─────────────────────────────────────────────────────────────────
    /// Radius within which neighbors are perceived.
    pub perception_radius:   f32,
    /// Radius within which separation is active.
    pub separation_radius:   f32,
    /// Radius within which the predator is feared.
    pub flee_radius:         f32,
    /// Arrival slow-down begins within this radius of the leader.
    pub arrival_radius:      f32,

    // ── Speed limits ──────────────────────────────────────────────────────────
    pub max_speed:     f32,
    pub max_force:     f32,
    pub min_speed:     f32,

    // ── Containment box ───────────────────────────────────────────────────────
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl Default for FlockConfig {
    fn default() -> Self {
        Self {
            alignment_weight:   1.0,
            cohesion_weight:    0.8,
            separation_weight:  1.5,
            avoidance_weight:   3.0,
            leader_weight:      1.2,
            flee_weight:        2.5,
            wander_weight:      0.3,
            containment_weight: 2.0,
            perception_radius:  4.0,
            separation_radius:  1.5,
            flee_radius:        8.0,
            arrival_radius:     2.0,
            max_speed:          5.0,
            max_force:          10.0,
            min_speed:          0.5,
            bounds_min:         Vec3::splat(-50.0),
            bounds_max:         Vec3::splat(50.0),
        }
    }
}

// ── Boid state ────────────────────────────────────────────────────────────────

/// State for a single boid in the flock.
#[derive(Clone, Debug)]
pub struct Boid {
    pub position:    Vec3,
    pub velocity:    Vec3,
    /// Wander circle angle (radians) — drifts each frame for smooth randomness.
    wander_angle:    f32,
    /// Accumulated age (used for wander noise phase).
    age:             f32,
    /// Index into the flock (for seeding per-boid random values).
    pub index:       usize,
}

impl Boid {
    pub fn new(position: Vec3, velocity: Vec3, index: usize) -> Self {
        Self {
            position,
            velocity,
            wander_angle: index as f32 * 2.399,  // irrational offset
            age: 0.0,
            index,
        }
    }

    /// Advance the boid by dt with computed steering force.
    pub fn integrate(&mut self, steering: Vec3, dt: f32, config: &FlockConfig) {
        let clamped = limit(steering, config.max_force);
        self.velocity += clamped * dt;
        self.velocity  = limit(self.velocity, config.max_speed);

        // Enforce min speed (boids always move a little)
        let speed = self.velocity.length();
        if speed < config.min_speed && speed > 0.001 {
            self.velocity = self.velocity / speed * config.min_speed;
        }

        self.position += self.velocity * dt;
        self.age += dt;
    }
}

// ── Steering computations ─────────────────────────────────────────────────────

/// Compute all steering forces for a single boid and return the total.
pub fn compute_steering(
    boid:       &mut Boid,
    neighbors:  &[FlockNeighbor],
    obstacles:  &[Obstacle],
    leader:     Option<Vec3>,
    predators:  &[Vec3],
    config:     &FlockConfig,
) -> Vec3 {
    let mut total = Vec3::ZERO;

    // ── Classic boid rules ────────────────────────────────────────────────────
    let (align, cohere, separate) = boid_forces(boid, neighbors, config);
    total += align    * config.alignment_weight;
    total += cohere   * config.cohesion_weight;
    total += separate * config.separation_weight;

    // ── Obstacle avoidance ────────────────────────────────────────────────────
    if config.avoidance_weight > 0.0 {
        total += avoid_obstacles(boid, obstacles) * config.avoidance_weight;
    }

    // ── Leader following ──────────────────────────────────────────────────────
    if config.leader_weight > 0.0 {
        if let Some(target) = leader {
            total += seek_arrive(boid, target, config) * config.leader_weight;
        }
    }

    // ── Predator flee ─────────────────────────────────────────────────────────
    if config.flee_weight > 0.0 && !predators.is_empty() {
        total += flee_predators(boid, predators, config) * config.flee_weight;
    }

    // ── Wander ────────────────────────────────────────────────────────────────
    if config.wander_weight > 0.0 {
        total += wander(boid, config) * config.wander_weight;
    }

    // ── Containment ───────────────────────────────────────────────────────────
    if config.containment_weight > 0.0 {
        total += contain(boid, config) * config.containment_weight;
    }

    total
}

/// Classic three boid rules: alignment, cohesion, separation.
fn boid_forces(
    boid:      &Boid,
    neighbors: &[FlockNeighbor],
    config:    &FlockConfig,
) -> (Vec3, Vec3, Vec3) {
    if neighbors.is_empty() {
        return (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
    }

    let mut align_sum = Vec3::ZERO;
    let mut cohere_sum = Vec3::ZERO;
    let mut sep_sum = Vec3::ZERO;
    let mut align_count = 0usize;
    let mut cohere_count = 0usize;

    for n in neighbors {
        let delta = boid.position - n.position;
        let dist  = delta.length();

        if dist < config.perception_radius && dist > 0.001 {
            align_sum  += n.velocity;
            cohere_sum += n.position;
            align_count  += 1;
            cohere_count += 1;

            if dist < config.separation_radius {
                // Separation force: inverse distance weighted
                sep_sum += delta / (dist * dist);
            }
        }
    }

    let align = if align_count > 0 {
        let avg = align_sum / align_count as f32;
        steer_toward(boid, avg.normalize_or_zero() * config.max_speed)
    } else {
        Vec3::ZERO
    };

    let cohere = if cohere_count > 0 {
        let center = cohere_sum / cohere_count as f32;
        steer_toward(boid, (center - boid.position).normalize_or_zero() * config.max_speed)
    } else {
        Vec3::ZERO
    };

    let separate = if sep_sum.length() > 0.001 {
        steer_toward(boid, sep_sum.normalize_or_zero() * config.max_speed)
    } else {
        Vec3::ZERO
    };

    (align, cohere, separate)
}

/// Obstacle avoidance: steer away from sphere obstacles.
fn avoid_obstacles(boid: &Boid, obstacles: &[Obstacle]) -> Vec3 {
    let mut force = Vec3::ZERO;
    let look_ahead = boid.velocity.normalize_or_zero() * 2.5;
    let future_pos = boid.position + look_ahead;

    for obs in obstacles {
        let delta = future_pos - obs.center;
        let dist  = delta.length();
        let threat_radius = obs.radius + 1.0;  // buffer zone

        if dist < threat_radius && dist > 0.001 {
            let push = delta / dist * (threat_radius - dist) / threat_radius;
            force += push;
        }
    }

    // Also consider current position penetration
    for obs in obstacles {
        let overlap = obs.overlap(boid.position);
        if overlap > 0.0 {
            let dir = (boid.position - obs.center).normalize_or_zero();
            force += dir * overlap * 5.0;
        }
    }

    force
}

/// Seek and arrive at a leader/target with smooth slowdown near arrival radius.
fn seek_arrive(boid: &Boid, target: Vec3, config: &FlockConfig) -> Vec3 {
    let delta = target - boid.position;
    let dist  = delta.length();
    if dist < 0.01 { return Vec3::ZERO; }

    let desired_speed = if dist < config.arrival_radius {
        config.max_speed * (dist / config.arrival_radius)
    } else {
        config.max_speed
    };

    let desired = delta / dist * desired_speed;
    steer_toward(boid, desired)
}

/// Flee from all predator positions within flee_radius.
fn flee_predators(boid: &Boid, predators: &[Vec3], config: &FlockConfig) -> Vec3 {
    let mut force = Vec3::ZERO;
    for &predator in predators {
        let delta = boid.position - predator;
        let dist  = delta.length();
        if dist < config.flee_radius && dist > 0.001 {
            let urgency = 1.0 - dist / config.flee_radius;
            force += delta.normalize_or_zero() * urgency;
        }
    }
    if force.length() > 0.001 {
        steer_toward(boid, force.normalize_or_zero() * config.max_speed)
    } else {
        Vec3::ZERO
    }
}

/// Wander: smooth random steering using a wander circle on the velocity vector.
fn wander(boid: &mut Boid, config: &FlockConfig) -> Vec3 {
    let wander_radius   = 1.5f32;
    let wander_distance = 3.0f32;
    let wander_jitter   = 0.8f32;

    // Drift the wander angle with noise
    let noise = lcg_f32(boid.index as u64, (boid.age * 10.0) as u64) * 2.0 - 1.0;
    boid.wander_angle += noise * wander_jitter;

    let circle_center = boid.velocity.normalize_or_zero() * wander_distance;
    let wander_point = Vec3::new(
        circle_center.x + boid.wander_angle.cos() * wander_radius,
        circle_center.y + boid.wander_angle.sin() * wander_radius,
        circle_center.z,
    );

    let _ = config; // suppress unused warning
    wander_point.normalize_or_zero()
}

/// Containment: steer back inward when approaching bounding box edges.
fn contain(boid: &Boid, config: &FlockConfig) -> Vec3 {
    let margin = 5.0f32;
    let mut force = Vec3::ZERO;

    let min = config.bounds_min;
    let max = config.bounds_max;

    if boid.position.x < min.x + margin { force.x += 1.0; }
    if boid.position.x > max.x - margin { force.x -= 1.0; }
    if boid.position.y < min.y + margin { force.y += 1.0; }
    if boid.position.y > max.y - margin { force.y -= 1.0; }
    if boid.position.z < min.z + margin { force.z += 1.0; }
    if boid.position.z > max.z - margin { force.z -= 1.0; }

    force
}

// ── Full flock simulation ─────────────────────────────────────────────────────

/// The complete flock — owns and ticks all boids each frame.
pub struct Flock {
    pub boids:     Vec<Boid>,
    pub config:    FlockConfig,
    pub obstacles: Vec<Obstacle>,
    pub leader:    Option<Vec3>,
    pub predators: Vec<Vec3>,
}

impl Flock {
    pub fn new(config: FlockConfig) -> Self {
        Self {
            boids: Vec::new(),
            config,
            obstacles: Vec::new(),
            leader: None,
            predators: Vec::new(),
        }
    }

    /// Spawn N boids in a circle at the given center with random velocities.
    pub fn spawn_circle(&mut self, n: usize, center: Vec3, radius: f32) {
        let start = self.boids.len();
        for i in 0..n {
            let angle = i as f32 / n as f32 * TAU;
            let pos = center + Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0);
            let vel_angle = angle + std::f32::consts::FRAC_PI_2;
            let vel = Vec3::new(vel_angle.cos(), vel_angle.sin(), 0.0) * self.config.min_speed;
            self.boids.push(Boid::new(pos, vel, start + i));
        }
    }

    /// Spawn N boids in a random scatter within radius.
    pub fn spawn_scatter(&mut self, n: usize, center: Vec3, radius: f32, seed: u64) {
        let start = self.boids.len();
        let mut rng = seed;
        for i in 0..n {
            rng = rng.wrapping_mul(0x6c62272e07bb0142).wrapping_add(0x62b821756295c58d);
            let angle = (rng >> 32) as f32 / u32::MAX as f32 * TAU;
            rng = rng.wrapping_mul(0x6c62272e07bb0142).wrapping_add(0x62b821756295c58d);
            let r = ((rng >> 32) as f32 / u32::MAX as f32).sqrt() * radius;
            let pos = center + Vec3::new(angle.cos() * r, angle.sin() * r, 0.0);
            let vel_angle = angle + 1.57;
            let vel = Vec3::new(vel_angle.cos(), vel_angle.sin(), 0.0) * self.config.min_speed;
            self.boids.push(Boid::new(pos, vel, start + i));
        }
    }

    /// Tick all boids by dt seconds. Computes neighbor lists and integrates.
    pub fn tick(&mut self, dt: f32) {
        let n = self.boids.len();
        if n == 0 { return; }

        // Snapshot current positions/velocities for neighbor computation
        let snapshot: Vec<FlockNeighbor> = self.boids.iter()
            .map(|b| FlockNeighbor { position: b.position, velocity: b.velocity })
            .collect();

        // Compute steering forces (needs snapshot so we can mutably borrow boids)
        let forces: Vec<Vec3> = (0..n).map(|i| {
            let boid = &self.boids[i];
            let neighbors: Vec<FlockNeighbor> = snapshot.iter().enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, n)| FlockNeighbor { position: n.position, velocity: n.velocity })
                .collect();

            // We need mutable access to boid for wander_angle; clone and integrate later
            let mut tmp_boid = boid.clone();
            compute_steering(
                &mut tmp_boid,
                &neighbors,
                &self.obstacles,
                self.leader,
                &self.predators,
                &self.config,
            )
        }).collect();

        // Apply forces and integrate
        for (boid, force) in self.boids.iter_mut().zip(forces.iter()) {
            boid.integrate(*force, dt, &self.config);
        }
    }

    /// Add a spherical obstacle to avoid.
    pub fn add_obstacle(&mut self, center: Vec3, radius: f32) {
        self.obstacles.push(Obstacle::new(center, radius));
    }

    /// Set the leader position (boids will follow).
    pub fn set_leader(&mut self, pos: Option<Vec3>) {
        self.leader = pos;
    }

    /// Set predator positions (boids will flee).
    pub fn set_predators(&mut self, predators: Vec<Vec3>) {
        self.predators = predators;
    }

    /// Number of boids.
    pub fn len(&self) -> usize { self.boids.len() }
    pub fn is_empty(&self) -> bool { self.boids.is_empty() }

    /// Average position (flock centroid).
    pub fn centroid(&self) -> Vec3 {
        if self.boids.is_empty() { return Vec3::ZERO; }
        let sum: Vec3 = self.boids.iter().map(|b| b.position).sum();
        sum / self.boids.len() as f32
    }

    /// Average speed.
    pub fn avg_speed(&self) -> f32 {
        if self.boids.is_empty() { return 0.0; }
        self.boids.iter().map(|b| b.velocity.length()).sum::<f32>() / self.boids.len() as f32
    }

    /// Cohesion metric: 1.0 = tight flock, 0.0 = completely scattered.
    /// Measured as 1 / (1 + average distance from centroid).
    pub fn cohesion_metric(&self) -> f32 {
        if self.boids.is_empty() { return 1.0; }
        let c = self.centroid();
        let avg_dist = self.boids.iter()
            .map(|b| (b.position - c).length())
            .sum::<f32>() / self.boids.len() as f32;
        1.0 / (1.0 + avg_dist)
    }

    /// Polarization: how aligned the flock velocity is (1.0 = all same direction).
    pub fn polarization(&self) -> f32 {
        if self.boids.is_empty() { return 0.0; }
        let sum: Vec3 = self.boids.iter()
            .map(|b| b.velocity.normalize_or_zero())
            .sum();
        sum.length() / self.boids.len() as f32
    }

    /// Kill and remove boids with a predicate.
    pub fn remove_if(&mut self, predicate: impl Fn(&Boid) -> bool) {
        self.boids.retain(|b| !predicate(b));
    }
}

// ── Free function versions (for use without full Flock struct) ────────────────

/// Compute a steering force for one boid given its neighbors.
/// Simplified version using just the three classic forces.
pub fn flock_force(
    pos:        Vec3,
    vel:        Vec3,
    neighbors:  &[FlockNeighbor],
    alignment:  f32,
    cohesion:   f32,
    separation: f32,
    radius:     f32,
) -> Vec3 {
    if neighbors.is_empty() { return Vec3::ZERO; }

    let mut avg_vel  = Vec3::ZERO;
    let mut avg_pos  = Vec3::ZERO;
    let mut sep      = Vec3::ZERO;
    let mut count    = 0usize;

    for n in neighbors {
        let delta = pos - n.position;
        let dist  = delta.length();
        if dist < radius && dist > 0.001 {
            avg_vel += n.velocity;
            avg_pos += n.position;
            count   += 1;
            if dist < radius * 0.5 {
                sep += delta / (dist * dist);
            }
        }
    }

    if count == 0 { return Vec3::ZERO; }

    let n = count as f32;
    avg_vel /= n;
    avg_pos /= n;

    let align   = (avg_vel - vel) * alignment;
    let cohese  = (avg_pos - pos) * cohesion;
    let repel   = sep * separation;

    align + cohese + repel
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Compute a steering force (desired - current velocity), clamped to max_force.
fn steer_toward(boid: &Boid, desired: Vec3) -> Vec3 {
    desired - boid.velocity
}

/// Clamp a vector to maximum length.
fn limit(v: Vec3, max: f32) -> Vec3 {
    let len = v.length();
    if len > max && len > 0.001 { v / len * max } else { v }
}

/// Simple LCG float in [0, 1) from two u64 seeds.
fn lcg_f32(seed1: u64, seed2: u64) -> f32 {
    let x = seed1.wrapping_mul(0x9e3779b97f4a7c15)
                 .wrapping_add(seed2)
                 .wrapping_mul(0x6c62272e07bb0142);
    (x >> 32) as f32 / u32::MAX as f32
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flock_ticks_without_panic() {
        let mut flock = Flock::new(FlockConfig::default());
        flock.spawn_circle(10, Vec3::ZERO, 3.0);
        assert_eq!(flock.len(), 10);
        flock.tick(0.016);
        flock.tick(0.016);
    }

    #[test]
    fn separation_pushes_apart() {
        let pos = Vec3::ZERO;
        let vel = Vec3::X;
        let neighbors = vec![
            FlockNeighbor { position: Vec3::new(0.5, 0.0, 0.0), velocity: Vec3::X },
        ];
        let force = flock_force(pos, vel, &neighbors, 0.0, 0.0, 1.0, 4.0);
        // Separation should push away (negative x direction from 0.5 offset)
        assert!(force.x < 0.0);
    }

    #[test]
    fn cohesion_metric_tight_flock() {
        let mut flock = Flock::new(FlockConfig::default());
        flock.spawn_circle(6, Vec3::ZERO, 0.1);
        let metric = flock.cohesion_metric();
        assert!(metric > 0.8);
    }

    #[test]
    fn flock_flee_changes_direction() {
        let config = FlockConfig { flee_weight: 5.0, ..Default::default() };
        let mut flock = Flock::new(config);
        flock.spawn_circle(5, Vec3::new(3.0, 0.0, 0.0), 0.5);
        flock.set_predators(vec![Vec3::ZERO]);

        let init_centroid = flock.centroid();
        for _ in 0..10 {
            flock.tick(0.016);
        }
        let after_centroid = flock.centroid();
        // Flock should have moved away from origin
        assert!(after_centroid.x > init_centroid.x);
    }
}
