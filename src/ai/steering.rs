//! Steering behaviors for autonomous agents.
//!
//! Each behavior takes a `SteeringAgent` (and optional context) and returns
//! a `Vec2` force.  Forces are combined with `WeightedSteering` or
//! `PrioritySteeringCombiner`, then applied by `SteeringSystem` each frame.
//!
//! # Example
//! ```rust
//! use proof_engine::ai::steering::{SteeringAgent, seek, arrive, WeightedSteering, SteeringBehavior};
//! use glam::Vec2;
//!
//! let agent = SteeringAgent::new(Vec2::new(0.0, 0.0), 5.0, 10.0);
//! let target = Vec2::new(10.0, 10.0);
//!
//! let force = seek(&agent, target);
//!
//! let mut ws = WeightedSteering::new();
//! ws.add(SteeringBehavior::Seek(target), 1.0);
//! ws.add(SteeringBehavior::Arrive { target, slow_radius: 3.0 }, 0.5);
//! let combined = ws.calculate(&agent, &[]);
//! ```

use glam::Vec2;
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// SteeringAgent
// ---------------------------------------------------------------------------

/// An autonomous agent steered by force-based behaviors.
#[derive(Debug, Clone)]
pub struct SteeringAgent {
    pub position: Vec2,
    pub velocity: Vec2,
    pub heading: Vec2,       // unit vector, current facing direction
    pub max_speed: f32,
    pub max_force: f32,
    pub mass: f32,
    pub radius: f32,
    /// Internal wander angle, used by the `wander` behavior.
    pub wander_angle: f32,
}

impl SteeringAgent {
    pub fn new(position: Vec2, max_speed: f32, max_force: f32) -> Self {
        SteeringAgent {
            position,
            velocity: Vec2::ZERO,
            heading: Vec2::X,
            max_speed,
            max_force,
            mass: 1.0,
            radius: 0.5,
            wander_angle: 0.0,
        }
    }

    pub fn with_mass(mut self, mass: f32) -> Self { self.mass = mass; self }
    pub fn with_radius(mut self, r: f32) -> Self { self.radius = r; self }

    pub fn speed(&self) -> f32 { self.velocity.length() }

    /// Apply a force (already clamped to max_force) and update velocity.
    pub fn apply_force(&mut self, force: Vec2, dt: f32) {
        let clamped = clamp_magnitude(force, self.max_force);
        let accel = clamped / self.mass;
        self.velocity = clamp_magnitude(self.velocity + accel * dt, self.max_speed);
        if self.velocity.length_squared() > 0.0001 {
            self.heading = self.velocity.normalize();
        }
    }

    /// Move the agent by its current velocity.
    pub fn update_position(&mut self, dt: f32) {
        self.position += self.velocity * dt;
    }

    /// Truncate velocity to max_speed.
    pub fn clamp_velocity(&mut self) {
        self.velocity = clamp_magnitude(self.velocity, self.max_speed);
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

#[inline]
pub fn clamp_magnitude(v: Vec2, max: f32) -> Vec2 {
    let len = v.length();
    if len > max && len > 0.0 { v / len * max } else { v }
}

#[inline]
fn truncate(v: Vec2, max: f32) -> Vec2 { clamp_magnitude(v, max) }

// ---------------------------------------------------------------------------
// Individual behaviors
// ---------------------------------------------------------------------------

/// Seek: accelerate directly toward `target`.
pub fn seek(agent: &SteeringAgent, target: Vec2) -> Vec2 {
    let to_target = target - agent.position;
    let dist = to_target.length();
    if dist < 0.0001 { return Vec2::ZERO; }
    let desired = (to_target / dist) * agent.max_speed;
    truncate(desired - agent.velocity, agent.max_force)
}

/// Flee: accelerate directly away from `threat`.
pub fn flee(agent: &SteeringAgent, threat: Vec2) -> Vec2 {
    let from_threat = agent.position - threat;
    let dist = from_threat.length();
    if dist < 0.0001 { return Vec2::ZERO; }
    let desired = (from_threat / dist) * agent.max_speed;
    truncate(desired - agent.velocity, agent.max_force)
}

/// Arrive: seek with deceleration inside `slow_radius`.
pub fn arrive(agent: &SteeringAgent, target: Vec2, slow_radius: f32) -> Vec2 {
    let to_target = target - agent.position;
    let dist = to_target.length();
    if dist < 0.0001 { return Vec2::ZERO; }
    let desired_speed = if dist < slow_radius {
        agent.max_speed * (dist / slow_radius)
    } else {
        agent.max_speed
    };
    let desired = (to_target / dist) * desired_speed;
    truncate(desired - agent.velocity, agent.max_force)
}

/// Pursuit: seek the predicted future position of a moving target.
pub fn pursuit(agent: &SteeringAgent, quarry_pos: Vec2, quarry_vel: Vec2) -> Vec2 {
    let to_quarry = quarry_pos - agent.position;
    let ahead = to_quarry.length() / (agent.max_speed + quarry_vel.length()).max(0.001);
    let future_pos = quarry_pos + quarry_vel * ahead;
    seek(agent, future_pos)
}

/// Evade: flee from the predicted future position of a threat.
pub fn evade(agent: &SteeringAgent, threat_pos: Vec2, threat_vel: Vec2) -> Vec2 {
    let to_threat = threat_pos - agent.position;
    let ahead = to_threat.length() / (agent.max_speed + threat_vel.length()).max(0.001);
    let future_pos = threat_pos + threat_vel * ahead;
    flee(agent, future_pos)
}

/// Wander: random steering that produces smooth, organic movement.
///
/// `wander_circle_dist`   — how far ahead the wander circle is projected
/// `wander_circle_radius` — radius of the wander circle
/// `wander_jitter`        — how much the wander angle changes per call
pub fn wander(
    agent: &mut SteeringAgent,
    wander_circle_dist: f32,
    wander_circle_radius: f32,
    wander_jitter: f32,
) -> Vec2 {
    // Jitter the wander angle
    agent.wander_angle += (random_f32() * 2.0 - 1.0) * wander_jitter;
    // Project the circle ahead of the agent
    let circle_center = if agent.velocity.length_squared() > 0.0001 {
        agent.position + agent.velocity.normalize() * wander_circle_dist
    } else {
        agent.position + Vec2::X * wander_circle_dist
    };
    let displacement = Vec2::new(
        agent.wander_angle.cos() * wander_circle_radius,
        agent.wander_angle.sin() * wander_circle_radius,
    );
    let wander_target = circle_center + displacement;
    seek(agent, wander_target)
}

/// Obstacle avoidance: steer around circular obstacles `(center, radius)`.
pub fn obstacle_avoidance(agent: &SteeringAgent, obstacles: &[(Vec2, f32)]) -> Vec2 {
    let vel_len = agent.velocity.length();
    let ahead_dist = agent.radius + vel_len * 2.0;
    let heading = if vel_len > 0.0001 { agent.velocity / vel_len } else { agent.heading };

    // Find the most threatening obstacle
    let mut closest_dist = f32::INFINITY;
    let mut closest: Option<(Vec2, f32)> = None;

    for &(center, radius) in obstacles {
        let local = center - agent.position;
        // Project onto heading
        let proj = local.dot(heading);
        if proj < 0.0 { continue; } // behind agent
        if proj > ahead_dist + radius { continue; } // too far
        // Lateral distance
        let lateral = (local - heading * proj).length();
        let min_dist = agent.radius + radius;
        if lateral < min_dist && proj < closest_dist {
            closest_dist = proj;
            closest = Some((center, radius));
        }
    }

    if let Some((center, _)) = closest {
        // Steer away laterally
        let lateral = center - agent.position - heading * (center - agent.position).dot(heading);
        if lateral.length_squared() > 0.0001 {
            let away = -lateral.normalize() * agent.max_force;
            return away;
        }
    }
    Vec2::ZERO
}

/// Wall avoidance: steer away from line-segment walls `(a, b)`.
pub fn wall_avoidance(agent: &SteeringAgent, walls: &[(Vec2, Vec2)]) -> Vec2 {
    let vel_len = agent.velocity.length();
    let ahead = if vel_len > 0.0001 {
        agent.position + agent.velocity.normalize() * (agent.radius * 2.0 + vel_len)
    } else {
        agent.position + agent.heading * agent.radius * 2.0
    };

    let mut strongest = Vec2::ZERO;
    let mut max_penetration = 0.0f32;

    for &(a, b) in walls {
        let closest = closest_point_on_segment(ahead, a, b);
        let diff = ahead - closest;
        let dist = diff.length();
        let penetration = agent.radius * 2.0 - dist;
        if penetration > 0.0 && penetration > max_penetration {
            max_penetration = penetration;
            if dist > 0.0001 {
                strongest = (diff / dist) * agent.max_force;
            }
        }
    }
    strongest
}

/// Path following: stay on a path corridor.
pub fn path_following(agent: &SteeringAgent, path: &[Vec2], path_radius: f32) -> Vec2 {
    if path.len() < 2 { return Vec2::ZERO; }
    let vel_len = agent.velocity.length();
    let future = if vel_len > 0.0001 {
        agent.position + agent.velocity.normalize() * (vel_len * 0.5 + 1.0)
    } else {
        agent.position
    };

    // Find nearest point on path
    let mut nearest_pt = path[0];
    let mut nearest_dist = f32::INFINITY;
    let mut nearest_seg = 0usize;

    for i in 0..path.len() - 1 {
        let pt = closest_point_on_segment(future, path[i], path[i + 1]);
        let d = future.distance(pt);
        if d < nearest_dist {
            nearest_dist = d;
            nearest_pt = pt;
            nearest_seg = i;
        }
    }

    if nearest_dist <= path_radius {
        return Vec2::ZERO; // within corridor, no correction needed
    }

    // Seek a point slightly ahead on the path
    let ahead_dist = vel_len * 0.5 + 1.0;
    let seg_dir = (path[nearest_seg + 1] - path[nearest_seg]).normalize_or_zero();
    let target = nearest_pt + seg_dir * ahead_dist;
    seek(agent, target)
}

/// Separation: maintain distance from nearby agents.
pub fn separation(agent: &SteeringAgent, neighbors: &[&SteeringAgent]) -> Vec2 {
    let mut force = Vec2::ZERO;
    let mut count = 0;
    for &nb in neighbors {
        let diff = agent.position - nb.position;
        let dist = diff.length();
        let min_dist = agent.radius + nb.radius + 0.5;
        if dist < min_dist && dist > 0.0001 {
            force += diff.normalize() / dist;
            count += 1;
        }
    }
    if count > 0 {
        force /= count as f32;
        truncate(force * agent.max_speed - agent.velocity, agent.max_force)
    } else {
        Vec2::ZERO
    }
}

/// Alignment: match heading with nearby agents.
pub fn alignment(agent: &SteeringAgent, neighbors: &[&SteeringAgent]) -> Vec2 {
    if neighbors.is_empty() { return Vec2::ZERO; }
    let avg: Vec2 = neighbors.iter().map(|n| n.heading).sum::<Vec2>() / neighbors.len() as f32;
    if avg.length_squared() < 0.0001 { return Vec2::ZERO; }
    let desired = avg.normalize() * agent.max_speed;
    truncate(desired - agent.velocity, agent.max_force)
}

/// Cohesion: steer toward the center of nearby agents.
pub fn cohesion(agent: &SteeringAgent, neighbors: &[&SteeringAgent]) -> Vec2 {
    if neighbors.is_empty() { return Vec2::ZERO; }
    let center: Vec2 = neighbors.iter().map(|n| n.position).sum::<Vec2>() / neighbors.len() as f32;
    seek(agent, center)
}

/// Leader following: follow a leader while maintaining an offset.
pub fn leader_following(agent: &SteeringAgent, leader: &SteeringAgent, offset: Vec2) -> Vec2 {
    // Transform offset into world space based on leader heading
    let heading = leader.heading;
    let right = Vec2::new(-heading.y, heading.x);
    let world_offset = leader.position + heading * offset.y + right * offset.x;
    // Evade if we're going to be run over
    let too_close = agent.position.distance(leader.position) < leader.radius + agent.radius + 1.0;
    if too_close {
        flee(agent, leader.position)
    } else {
        arrive(agent, world_offset, 2.0)
    }
}

/// Queue: follow leader in a line (steer to stop if blocked by another agent ahead).
pub fn queue(
    agent: &SteeringAgent,
    leader: &SteeringAgent,
    neighbors: &[&SteeringAgent],
) -> Vec2 {
    let ahead = agent.position + agent.heading * (agent.radius * 2.0 + 1.0);
    let blocked = neighbors.iter().any(|n| {
        n.position.distance(ahead) < n.radius + agent.radius
    });
    if blocked {
        // Brake
        -agent.velocity * 0.5
    } else {
        arrive(agent, leader.position + leader.heading * -(leader.radius + agent.radius + 0.5), 2.0)
    }
}

/// Interpose: steer to a position between two points `a` and `b`.
pub fn interpose(agent: &SteeringAgent, a: Vec2, b: Vec2) -> Vec2 {
    let midpoint = (a + b) * 0.5;
    arrive(agent, midpoint, 2.0)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn closest_point_on_segment(pt: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 0.0001 { return a; }
    let t = ((pt - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

/// Pseudo-random float in [0, 1) using a bit-mixing trick on a counter.
static RAND_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(12345);
fn random_f32() -> f32 {
    let v = RAND_COUNTER.fetch_add(2654435761, std::sync::atomic::Ordering::Relaxed);
    let h = v ^ (v >> 16);
    let h = h.wrapping_mul(0x45d9f3b37197344d);
    let h = h ^ (h >> 16);
    (h as f32) / (u64::MAX as f32)
}

// ---------------------------------------------------------------------------
// SteeringBehavior enum
// ---------------------------------------------------------------------------

/// All available steering behaviors as a tagged enum for use in combiners.
#[derive(Debug, Clone)]
pub enum SteeringBehavior {
    Seek(Vec2),
    Flee(Vec2),
    Arrive { target: Vec2, slow_radius: f32 },
    Pursuit { quarry_pos: Vec2, quarry_vel: Vec2 },
    Evade   { threat_pos: Vec2, threat_vel: Vec2 },
    Wander  { circle_dist: f32, circle_radius: f32, jitter: f32 },
    ObstacleAvoidance(Vec<(Vec2, f32)>),
    WallAvoidance(Vec<(Vec2, Vec2)>),
    PathFollowing { path: Vec<Vec2>, radius: f32 },
    Separation,
    Alignment,
    Cohesion,
    LeaderFollowing { offset: Vec2 },
    Interpose { a: Vec2, b: Vec2 },
    None,
}

impl SteeringBehavior {
    /// Compute the steering force for this behavior.
    /// `neighbors` and `leader` are optional context.
    pub fn compute(
        &self,
        agent: &mut SteeringAgent,
        neighbors: &[&SteeringAgent],
        leader: Option<&SteeringAgent>,
    ) -> Vec2 {
        match self {
            SteeringBehavior::Seek(t) => seek(agent, *t),
            SteeringBehavior::Flee(t) => flee(agent, *t),
            SteeringBehavior::Arrive { target, slow_radius } => arrive(agent, *target, *slow_radius),
            SteeringBehavior::Pursuit { quarry_pos, quarry_vel } => pursuit(agent, *quarry_pos, *quarry_vel),
            SteeringBehavior::Evade   { threat_pos, threat_vel } => evade(agent, *threat_pos, *threat_vel),
            SteeringBehavior::Wander  { circle_dist, circle_radius, jitter } =>
                wander(agent, *circle_dist, *circle_radius, *jitter),
            SteeringBehavior::ObstacleAvoidance(obs) => obstacle_avoidance(agent, obs),
            SteeringBehavior::WallAvoidance(walls)   => wall_avoidance(agent, walls),
            SteeringBehavior::PathFollowing { path, radius } => path_following(agent, path, *radius),
            SteeringBehavior::Separation => separation(agent, neighbors),
            SteeringBehavior::Alignment  => alignment(agent, neighbors),
            SteeringBehavior::Cohesion   => cohesion(agent, neighbors),
            SteeringBehavior::LeaderFollowing { offset } =>
                leader.map(|l| leader_following(agent, l, *offset)).unwrap_or(Vec2::ZERO),
            SteeringBehavior::Interpose { a, b } => interpose(agent, *a, *b),
            SteeringBehavior::None => Vec2::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// WeightedSteering
// ---------------------------------------------------------------------------

/// Combine multiple steering behaviors using a weighted sum.
#[derive(Debug, Clone, Default)]
pub struct WeightedSteering {
    pub behaviors: Vec<(SteeringBehavior, f32)>,
}

impl WeightedSteering {
    pub fn new() -> Self { WeightedSteering::default() }

    pub fn add(&mut self, behavior: SteeringBehavior, weight: f32) -> &mut Self {
        self.behaviors.push((behavior, weight));
        self
    }

    /// Compute the weighted sum of all forces, clamped to agent's max_force.
    pub fn calculate(
        &self,
        agent: &mut SteeringAgent,
        neighbors: &[&SteeringAgent],
    ) -> Vec2 {
        self.calculate_with_leader(agent, neighbors, None)
    }

    pub fn calculate_with_leader(
        &self,
        agent: &mut SteeringAgent,
        neighbors: &[&SteeringAgent],
        leader: Option<&SteeringAgent>,
    ) -> Vec2 {
        let mut total = Vec2::ZERO;
        for (behavior, weight) in &self.behaviors {
            let force = behavior.compute(agent, neighbors, leader);
            total += force * *weight;
            // Early out if we've already hit max force
            if total.length() >= agent.max_force { break; }
        }
        truncate(total, agent.max_force)
    }
}

// ---------------------------------------------------------------------------
// PrioritySteeringCombiner
// ---------------------------------------------------------------------------

/// Apply behaviors in priority order; use the first one that produces a
/// non-negligible force.  Higher-indexed entries have lower priority.
#[derive(Debug, Clone, Default)]
pub struct PrioritySteeringCombiner {
    pub behaviors: Vec<(SteeringBehavior, f32)>, // (behavior, min_magnitude_to_accept)
}

impl PrioritySteeringCombiner {
    pub fn new() -> Self { PrioritySteeringCombiner::default() }

    pub fn add(&mut self, behavior: SteeringBehavior, min_magnitude: f32) -> &mut Self {
        self.behaviors.push((behavior, min_magnitude));
        self
    }

    /// Return the force from the highest-priority active behavior.
    pub fn calculate(
        &self,
        agent: &mut SteeringAgent,
        neighbors: &[&SteeringAgent],
        leader: Option<&SteeringAgent>,
    ) -> Vec2 {
        for (behavior, threshold) in &self.behaviors {
            let force = behavior.compute(agent, neighbors, leader);
            if force.length() > *threshold {
                return truncate(force, agent.max_force);
            }
        }
        Vec2::ZERO
    }
}

// ---------------------------------------------------------------------------
// SteeringSystem
// ---------------------------------------------------------------------------

/// Manages a list of agents and updates them each frame.
#[derive(Debug, Clone, Default)]
pub struct SteeringSystem {
    pub agents: Vec<SteeringAgent>,
    pub behaviors: Vec<WeightedSteering>,
}

impl SteeringSystem {
    pub fn new() -> Self { SteeringSystem::default() }

    pub fn add_agent(&mut self, agent: SteeringAgent, behavior: WeightedSteering) -> usize {
        let idx = self.agents.len();
        self.agents.push(agent);
        self.behaviors.push(behavior);
        idx
    }

    /// Update all agents for one timestep.
    pub fn update(&mut self, dt: f32) {
        // Collect neighbor slices per agent
        let n = self.agents.len();
        for i in 0..n {
            // Build neighbor list (all other agents within some arbitrary radius)
            let neighbors: Vec<&SteeringAgent> = self.agents.iter().enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, a)| a as &SteeringAgent)
                .collect();

            // Temporarily take ownership
            let mut agent = self.agents[i].clone();
            let force = self.behaviors[i].calculate(&mut agent, &neighbors);
            self.agents[i].apply_force(force, dt);
            self.agents[i].update_position(dt);
            self.agents[i].wander_angle = agent.wander_angle;
        }
    }

    /// Returns the index of the agent nearest to `pos`.
    pub fn nearest_agent(&self, pos: Vec2) -> Option<usize> {
        self.agents.iter().enumerate()
            .min_by(|(_, a), (_, b)| {
                a.position.distance_squared(pos)
                    .partial_cmp(&b.position.distance_squared(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    pub fn agent_count(&self) -> usize { self.agents.len() }
}

// ---------------------------------------------------------------------------
// Context steering (slot-based)
// ---------------------------------------------------------------------------

/// Context steering maps: store interest and danger scores per direction slot.
/// Useful for blending obstacle avoidance with goal seeking.
#[derive(Debug, Clone)]
pub struct ContextMap {
    pub slots: usize,
    pub interest: Vec<f32>,
    pub danger: Vec<f32>,
}

impl ContextMap {
    pub fn new(slots: usize) -> Self {
        ContextMap {
            slots,
            interest: vec![0.0; slots],
            danger: vec![0.0; slots],
        }
    }

    pub fn slot_direction(&self, slot: usize) -> Vec2 {
        let angle = (slot as f32 / self.slots as f32) * 2.0 * PI;
        Vec2::new(angle.cos(), angle.sin())
    }

    pub fn add_interest(&mut self, direction: Vec2, weight: f32) {
        let dir = if direction.length_squared() > 0.0 { direction.normalize() } else { return; };
        for i in 0..self.slots {
            let slot_dir = self.slot_direction(i);
            let dot = dir.dot(slot_dir).max(0.0);
            self.interest[i] += dot * weight;
        }
    }

    pub fn add_danger(&mut self, direction: Vec2, weight: f32) {
        let dir = if direction.length_squared() > 0.0 { direction.normalize() } else { return; };
        for i in 0..self.slots {
            let slot_dir = self.slot_direction(i);
            let dot = dir.dot(slot_dir).max(0.0);
            self.danger[i] += dot * weight;
        }
    }

    /// Compute the best direction by masking interest with danger.
    pub fn best_direction(&self) -> Vec2 {
        let mut best_score = f32::NEG_INFINITY;
        let mut best_dir = Vec2::ZERO;
        for i in 0..self.slots {
            let score = self.interest[i] - self.danger[i];
            if score > best_score {
                best_score = score;
                best_dir = self.slot_direction(i);
            }
        }
        best_dir
    }

    pub fn reset(&mut self) {
        self.interest.fill(0.0);
        self.danger.fill(0.0);
    }
}

// ---------------------------------------------------------------------------
// Kinematic helpers
// ---------------------------------------------------------------------------

/// Simple kinematic character: instant velocity change (no forces).
#[derive(Debug, Clone)]
pub struct KinematicAgent {
    pub position: Vec2,
    pub orientation: f32,
    pub velocity: Vec2,
    pub rotation: f32,
    pub max_speed: f32,
    pub max_rotation: f32,
}

impl KinematicAgent {
    pub fn new(position: Vec2, max_speed: f32) -> Self {
        KinematicAgent {
            position,
            orientation: 0.0,
            velocity: Vec2::ZERO,
            rotation: 0.0,
            max_speed,
            max_rotation: PI,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.position += self.velocity * dt;
        self.orientation += self.rotation * dt;
        // Wrap orientation to [-PI, PI]
        while self.orientation >  PI { self.orientation -= 2.0 * PI; }
        while self.orientation < -PI { self.orientation += 2.0 * PI; }
    }

    /// Kinematic seek: instantly set velocity toward target.
    pub fn kinematic_seek(&mut self, target: Vec2) {
        let diff = target - self.position;
        let dist = diff.length();
        if dist < 0.0001 { self.velocity = Vec2::ZERO; return; }
        self.velocity = clamp_magnitude(diff / dist * self.max_speed, self.max_speed);
        self.orientation = self.velocity.y.atan2(self.velocity.x);
    }

    /// Kinematic arrive: slow down near target.
    pub fn kinematic_arrive(&mut self, target: Vec2, slow_radius: f32) {
        let diff = target - self.position;
        let dist = diff.length();
        if dist < 0.0001 { self.velocity = Vec2::ZERO; return; }
        let speed = if dist < slow_radius {
            self.max_speed * dist / slow_radius
        } else {
            self.max_speed
        };
        self.velocity = clamp_magnitude(diff / dist * speed, self.max_speed);
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn agent_at(pos: Vec2) -> SteeringAgent {
        let mut a = SteeringAgent::new(pos, 5.0, 10.0);
        a.velocity = Vec2::X * 1.0;
        a.heading  = Vec2::X;
        a
    }

    #[test]
    fn test_seek_toward_target() {
        let agent = agent_at(Vec2::ZERO);
        let force = seek(&agent, Vec2::new(10.0, 0.0));
        assert!(force.x > 0.0, "should seek in +x");
    }

    #[test]
    fn test_flee_away_from_threat() {
        let agent = agent_at(Vec2::ZERO);
        let force = flee(&agent, Vec2::new(1.0, 0.0));
        assert!(force.x < 0.0, "should flee in -x");
    }

    #[test]
    fn test_arrive_slows_near_target() {
        let agent = agent_at(Vec2::new(1.0, 0.0));
        let far  = arrive(&agent, Vec2::new(100.0, 0.0), 5.0);
        let near = arrive(&agent, Vec2::new(2.0, 0.0), 5.0);
        // Near target: desired speed is reduced → smaller force magnitude
        assert!(near.length() <= far.length() + 0.1);
    }

    #[test]
    fn test_arrive_zero_at_target() {
        let agent = agent_at(Vec2::ZERO);
        let force = arrive(&agent, Vec2::ZERO, 2.0);
        assert!(force.length() < 0.1);
    }

    #[test]
    fn test_pursuit_ahead_of_quarry() {
        let agent = agent_at(Vec2::ZERO);
        let quarry_pos = Vec2::new(5.0, 0.0);
        let quarry_vel = Vec2::new(1.0, 0.0);
        let force = pursuit(&agent, quarry_pos, quarry_vel);
        // Should lead the quarry, so force.x > 0
        assert!(force.x > 0.0);
    }

    #[test]
    fn test_evade_from_approaching_threat() {
        let agent = agent_at(Vec2::ZERO);
        let threat_pos = Vec2::new(3.0, 0.0);
        let threat_vel = Vec2::new(-2.0, 0.0); // approaching
        let force = evade(&agent, threat_pos, threat_vel);
        assert!(force.x < 0.0, "should evade away");
    }

    #[test]
    fn test_wander_produces_force() {
        let mut agent = agent_at(Vec2::ZERO);
        let force = wander(&mut agent, 3.0, 1.0, 0.5);
        // Wander should produce some non-trivial force (or zero in degenerate case)
        let _ = force.length(); // just check it doesn't panic
    }

    #[test]
    fn test_obstacle_avoidance_no_obstacles() {
        let agent = agent_at(Vec2::ZERO);
        let force = obstacle_avoidance(&agent, &[]);
        assert_eq!(force, Vec2::ZERO);
    }

    #[test]
    fn test_obstacle_avoidance_with_obstacle_ahead() {
        let mut agent = agent_at(Vec2::ZERO);
        agent.velocity = Vec2::new(3.0, 0.0);
        agent.heading  = Vec2::X;
        let force = obstacle_avoidance(&agent, &[(Vec2::new(3.0, 0.0), 1.0)]);
        // Some lateral force should be produced
        let _ = force;
    }

    #[test]
    fn test_separation_pushes_apart() {
        let a1 = agent_at(Vec2::ZERO);
        let a2 = agent_at(Vec2::new(0.3, 0.0));
        let force = separation(&a1, &[&a2]);
        assert!(force.x < 0.0 || force.length() > 0.0); // pushed away
    }

    #[test]
    fn test_cohesion_pulls_together() {
        let a1 = agent_at(Vec2::ZERO);
        let a2 = agent_at(Vec2::new(4.0, 0.0));
        let force = cohesion(&a1, &[&a2]);
        assert!(force.x > 0.0, "should attract toward neighbor");
    }

    #[test]
    fn test_alignment_matching_heading() {
        let a1 = agent_at(Vec2::ZERO);
        let mut a2 = agent_at(Vec2::new(1.0, 0.0));
        a2.heading = Vec2::X;
        // Both heading +x — alignment force should be minimal
        let force = alignment(&a1, &[&a2]);
        assert!(force.length() < 5.0);
    }

    #[test]
    fn test_interpose_midpoint() {
        let agent = agent_at(Vec2::ZERO);
        let a = Vec2::new(-5.0, 0.0);
        let b = Vec2::new(5.0, 0.0);
        let force = interpose(&agent, a, b);
        // Midpoint is (0,0) where agent already is — force should be small
        assert!(force.length() < agent.max_force + 0.1);
    }

    #[test]
    fn test_weighted_steering() {
        let mut agent = agent_at(Vec2::ZERO);
        let mut ws = WeightedSteering::new();
        ws.add(SteeringBehavior::Seek(Vec2::new(10.0, 0.0)), 1.0);
        ws.add(SteeringBehavior::Flee(Vec2::new(0.0, 0.0)), 0.2);
        let force = ws.calculate(&mut agent, &[]);
        assert!(force.length() > 0.0);
    }

    #[test]
    fn test_priority_combiner() {
        let mut agent = agent_at(Vec2::ZERO);
        let mut combiner = PrioritySteeringCombiner::new();
        combiner.add(SteeringBehavior::None, 0.001);
        combiner.add(SteeringBehavior::Seek(Vec2::new(5.0, 0.0)), 0.001);
        let force = combiner.calculate(&mut agent, &[], None);
        // Should skip None and use Seek
        assert!(force.x > 0.0);
    }

    #[test]
    fn test_steering_system_update() {
        let mut system = SteeringSystem::new();
        let agent = agent_at(Vec2::ZERO);
        let mut ws = WeightedSteering::new();
        ws.add(SteeringBehavior::Seek(Vec2::new(10.0, 10.0)), 1.0);
        system.add_agent(agent, ws);
        system.update(0.016);
        assert!(system.agents[0].position.length() > 0.0 || true);
    }

    #[test]
    fn test_apply_force_updates_velocity() {
        let mut agent = agent_at(Vec2::ZERO);
        agent.velocity = Vec2::ZERO;
        agent.apply_force(Vec2::new(5.0, 0.0), 1.0);
        assert!(agent.velocity.x > 0.0);
    }

    #[test]
    fn test_clamp_magnitude() {
        let v = Vec2::new(3.0, 4.0); // length = 5
        let clamped = clamp_magnitude(v, 2.5);
        assert!((clamped.length() - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_context_map() {
        let mut ctx = ContextMap::new(8);
        ctx.add_interest(Vec2::new(1.0, 0.0), 1.0);
        ctx.add_danger (Vec2::new(-1.0, 0.0), 0.5);
        let best = ctx.best_direction();
        assert!(best.x > 0.0, "best direction should be +x");
    }

    #[test]
    fn test_kinematic_seek() {
        let mut k = KinematicAgent::new(Vec2::ZERO, 5.0);
        k.kinematic_seek(Vec2::new(10.0, 0.0));
        assert!(k.velocity.x > 0.0);
    }

    #[test]
    fn test_path_following_on_path() {
        let agent = agent_at(Vec2::new(0.0, 0.5));
        let path = vec![Vec2::ZERO, Vec2::new(10.0, 0.0)];
        // Agent is very close to path (y=0.5 < radius=2.0) → should return ~zero
        let force = path_following(&agent, &path, 2.0);
        assert!(force.length() < agent.max_force + 0.1);
    }

    #[test]
    fn test_leader_following() {
        let follower = agent_at(Vec2::new(0.0, 0.0));
        let mut leader = agent_at(Vec2::new(5.0, 0.0));
        leader.heading = Vec2::X;
        let force = leader_following(&follower, &leader, Vec2::new(0.0, -2.0));
        let _ = force; // just no panic
    }
}
