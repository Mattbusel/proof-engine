//! Boid/flocking behavior for particles.

use glam::Vec3;

/// A neighboring particle for flocking calculation.
pub struct FlockNeighbor {
    pub position: Vec3,
    pub velocity: Vec3,
}

/// Compute a steering force for one boid given its neighbors.
pub fn flock_force(
    pos: Vec3,
    vel: Vec3,
    neighbors: &[FlockNeighbor],
    alignment: f32,
    cohesion: f32,
    separation: f32,
    radius: f32,
) -> Vec3 {
    if neighbors.is_empty() {
        return Vec3::ZERO;
    }

    let mut avg_vel = Vec3::ZERO;
    let mut avg_pos = Vec3::ZERO;
    let mut sep = Vec3::ZERO;

    for n in neighbors {
        avg_vel += n.velocity;
        avg_pos += n.position;
        let delta = pos - n.position;
        let dist = delta.length();
        if dist > 0.001 && dist < radius * 0.5 {
            sep += delta / (dist * dist);
        }
    }

    let n = neighbors.len() as f32;
    avg_vel /= n;
    avg_pos /= n;

    let align  = (avg_vel - vel) * alignment;
    let cohese = (avg_pos - pos) * cohesion;
    let repel  = sep * separation;

    align + cohese + repel
}
