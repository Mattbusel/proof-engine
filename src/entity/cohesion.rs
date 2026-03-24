//! Cohesion dynamics — spring-based physics keeping glyphs bound to their formation.
//!
//! Cohesion drives how tightly glyphs cling to their target positions. At cohesion=1.0
//! they snap instantly; at 0.0 they drift freely under chaos forces. Temperature adds
//! thermal jitter that makes formations feel alive and organic. When cohesion reaches
//! zero, the entity dissolves in an outward burst.

use glam::Vec3;
use crate::math::springs::SpringDamper3;

// ── Cohesion state per glyph ──────────────────────────────────────────────────

/// Per-glyph cohesion spring connecting the glyph to its formation slot.
pub struct GlyphCohesion {
    /// The spring driving this glyph toward its target slot.
    pub spring: SpringDamper3,
    /// Current temperature (0 = cold/calm, 1 = hot/jittery).
    pub temperature: f32,
    /// Thermal velocity — random drift added to spring velocity each frame.
    pub thermal_vel: Vec3,
    /// How much this glyph has drifted from its slot (0 = at rest, 1 = maximum drift).
    pub drift: f32,
    /// Phase offset for independent oscillation (prevents lockstep movement).
    pub phase: f32,
}

impl GlyphCohesion {
    /// Create a new cohesion spring at a formation slot position.
    pub fn new(slot_position: Vec3, cohesion_strength: f32, phase: f32) -> Self {
        let (stiffness, damping) = cohesion_to_spring(cohesion_strength);
        Self {
            spring: SpringDamper3::from_vec3(slot_position, stiffness, damping),
            temperature: 0.0,
            thermal_vel: Vec3::ZERO,
            drift: 0.0,
            phase,
        }
    }

    /// Step the cohesion spring by dt seconds.
    ///
    /// Returns the new glyph position including thermal jitter.
    pub fn tick(&mut self, dt: f32, cohesion: f32) -> Vec3 {
        // Recompute spring constants from current cohesion
        let (stiffness, damping) = cohesion_to_spring(cohesion);
        self.spring.x.stiffness = stiffness;
        self.spring.x.damping = damping;
        self.spring.y.stiffness = stiffness;
        self.spring.y.damping = damping;
        self.spring.z.stiffness = stiffness;
        self.spring.z.damping = damping;

        // Decay thermal jitter
        self.thermal_vel *= (1.0 - 4.0 * dt).max(0.0);

        // Step spring
        let base_pos = self.spring.tick(dt);

        // Compute drift from target
        let target = Vec3::new(
            self.spring.x.target,
            self.spring.y.target,
            self.spring.z.target,
        );
        self.drift = (base_pos - target).length();

        // Add thermal jitter to position
        base_pos + self.thermal_vel * self.temperature
    }

    /// Apply thermal energy to this glyph (increases jitter).
    pub fn heat(&mut self, temperature: f32, seed: f32) {
        self.temperature = temperature.clamp(0.0, 1.0);
        // Random impulse in a unit sphere direction
        let jitter_vel = thermal_direction(seed + self.phase) * temperature * 0.8;
        self.thermal_vel += jitter_vel;
    }

    /// Cool this glyph down (reduces jitter).
    pub fn cool(&mut self, rate: f32, dt: f32) {
        self.temperature = (self.temperature - rate * dt).max(0.0);
    }

    /// Move the target formation slot (animated spring follow).
    pub fn set_target(&mut self, new_slot: Vec3) {
        self.spring.set_target(new_slot);
    }

    /// Teleport position (no spring animation, instant).
    pub fn teleport(&mut self, pos: Vec3) {
        self.spring.x.position = pos.x;
        self.spring.y.position = pos.y;
        self.spring.z.position = pos.z;
        self.spring.x.velocity = 0.0;
        self.spring.y.velocity = 0.0;
        self.spring.z.velocity = 0.0;
    }

    /// Current position (without ticking).
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.spring.x.position, self.spring.y.position, self.spring.z.position)
    }

    /// Current velocity.
    pub fn velocity(&self) -> Vec3 {
        Vec3::new(self.spring.x.velocity, self.spring.y.velocity, self.spring.z.velocity)
    }

    /// Apply an impulse force (adds to spring velocity).
    pub fn apply_impulse(&mut self, impulse: Vec3) {
        self.spring.x.velocity += impulse.x;
        self.spring.y.velocity += impulse.y;
        self.spring.z.velocity += impulse.z;
    }
}

// ── Entity-level cohesion manager ─────────────────────────────────────────────

/// Manages cohesion for all glyphs in an entity.
pub struct CohesionManager {
    pub glyphs: Vec<GlyphCohesion>,
    /// Entity-level cohesion [0, 1]. 0 = chaotic, 1 = perfectly bound.
    pub cohesion: f32,
    /// Dissolution state: None = intact, Some(t) = dissolving (t = time since start).
    pub dissolution: Option<f32>,
    /// Velocity vectors set during dissolution burst.
    pub burst_velocities: Vec<Vec3>,
}

impl CohesionManager {
    /// Create with N glyphs at given positions.
    pub fn new(positions: &[Vec3], cohesion: f32) -> Self {
        let glyphs = positions
            .iter()
            .enumerate()
            .map(|(i, &pos)| {
                let phase = i as f32 * 1.618033988; // golden ratio spacing
                GlyphCohesion::new(pos, cohesion, phase)
            })
            .collect();
        Self {
            glyphs,
            cohesion,
            dissolution: None,
            burst_velocities: Vec::new(),
        }
    }

    /// Tick all glyph springs by dt. Returns Vec of positions.
    pub fn tick(&mut self, dt: f32) -> Vec<Vec3> {
        if let Some(ref mut t) = self.dissolution {
            *t += dt;
            // Drift outward using stored burst velocities
            return self.glyphs
                .iter_mut()
                .zip(self.burst_velocities.iter())
                .map(|(g, &bv)| {
                    let pos = g.position() + bv * dt;
                    // Update spring position to match drift
                    g.spring.x.position = pos.x;
                    g.spring.y.position = pos.y;
                    g.spring.z.position = pos.z;
                    pos
                })
                .collect();
        }

        self.glyphs
            .iter_mut()
            .map(|g| g.tick(dt, self.cohesion))
            .collect()
    }

    /// Apply damage to cohesion (reduce it by amount).
    pub fn damage_cohesion(&mut self, amount: f32) {
        self.cohesion = (self.cohesion - amount).max(0.0);
        if self.cohesion == 0.0 && self.dissolution.is_none() {
            self.begin_dissolution();
        }
    }

    /// Restore cohesion (healing effect).
    pub fn restore_cohesion(&mut self, amount: f32) {
        self.cohesion = (self.cohesion + amount).min(1.0);
    }

    /// Apply thermal energy to all glyphs (makes them jitter).
    pub fn heat_all(&mut self, temperature: f32, time: f32) {
        for (i, g) in self.glyphs.iter_mut().enumerate() {
            g.heat(temperature, time + i as f32 * 0.37);
        }
    }

    /// Cool all glyphs down.
    pub fn cool_all(&mut self, rate: f32, dt: f32) {
        for g in &mut self.glyphs {
            g.cool(rate, dt);
        }
    }

    /// Apply an outward impulse from center (e.g. shockwave impact).
    pub fn apply_shockwave(&mut self, center: Vec3, strength: f32) {
        for g in &mut self.glyphs {
            let dir = (g.position() - center).normalize_or_zero();
            g.apply_impulse(dir * strength);
        }
    }

    /// Apply a directional force to all glyphs.
    pub fn apply_force(&mut self, force: Vec3) {
        for g in &mut self.glyphs {
            g.apply_impulse(force);
        }
    }

    /// Update formation targets (e.g. entity moved, or formation changed).
    pub fn update_targets(&mut self, new_positions: &[Vec3]) {
        for (g, &pos) in self.glyphs.iter_mut().zip(new_positions.iter()) {
            g.set_target(pos);
        }
    }

    /// Teleport all glyphs instantly to formation positions (no animation).
    pub fn teleport_all(&mut self, positions: &[Vec3]) {
        for (g, &pos) in self.glyphs.iter_mut().zip(positions.iter()) {
            g.teleport(pos);
        }
    }

    /// Whether the entity is currently dissolving.
    pub fn is_dissolving(&self) -> bool { self.dissolution.is_some() }

    /// Whether dissolution is complete (> 2 seconds have elapsed).
    pub fn is_dissolved(&self) -> bool {
        self.dissolution.map(|t| t > 2.0).unwrap_or(false)
    }

    /// Begin the dissolution burst.
    fn begin_dissolution(&mut self) {
        self.dissolution = Some(0.0);
        let center = self.centroid();
        self.burst_velocities = dissolution_burst(
            &self.glyphs.iter().map(|g| g.position()).collect::<Vec<_>>(),
            center,
        );
    }

    /// Average position of all glyphs.
    pub fn centroid(&self) -> Vec3 {
        if self.glyphs.is_empty() { return Vec3::ZERO; }
        let sum: Vec3 = self.glyphs.iter().map(|g| g.position()).sum();
        sum / self.glyphs.len() as f32
    }

    /// Max drift across all glyphs (measures how chaotic the formation is).
    pub fn max_drift(&self) -> f32 {
        self.glyphs.iter().map(|g| g.drift).fold(0.0f32, f32::max)
    }

    /// Average temperature.
    pub fn avg_temperature(&self) -> f32 {
        if self.glyphs.is_empty() { return 0.0; }
        self.glyphs.iter().map(|g| g.temperature).sum::<f32>() / self.glyphs.len() as f32
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Convert cohesion [0, 1] to spring stiffness and damping.
///
/// At cohesion=0: very loose (stiffness=0.5, damping=0.3)
/// At cohesion=1: snappy (stiffness=40, damping=8)
pub fn cohesion_to_spring(cohesion: f32) -> (f32, f32) {
    let c = cohesion.clamp(0.0, 1.0);
    let stiffness = 0.5 + c * c * 39.5;  // quadratic for more natural feel
    let damping   = 0.3 + c * 7.7;
    (stiffness, damping)
}

/// Calculate how far a glyph at `actual` should move toward `target`
/// given cohesion strength [0, 1] and elapsed time dt.
pub fn cohesion_pull(actual: Vec3, target: Vec3, cohesion: f32, dt: f32) -> Vec3 {
    let delta = target - actual;
    let stiffness = cohesion_to_spring(cohesion).0;
    delta * stiffness * dt
}

/// Emit a formation dissolution burst.
/// Returns outward velocity vectors for each glyph.
pub fn dissolution_burst(positions: &[Vec3], center: Vec3) -> Vec<Vec3> {
    positions.iter().enumerate().map(|(i, pos)| {
        let dir = (*pos - center).normalize_or_zero();
        let speed = 2.0 + rand_f32_seeded(i as u64) * 3.0;
        // Add upward component for visual interest
        let up_bias = Vec3::new(0.0, rand_f32_seeded(i as u64 + 1000) * 2.0, 0.0);
        dir * speed + up_bias
    }).collect()
}

/// Evaluate a thermal random direction from a seed.
fn thermal_direction(seed: f32) -> Vec3 {
    let h1 = (seed * 127.1 + 311.7) as u64;
    let h1 = h1.wrapping_mul(0x9e3779b97f4a7c15);
    let h2 = h1.wrapping_mul(0x6c62272e07bb0142);
    let h3 = h2.wrapping_mul(0x9e3779b97f4a7c15);
    let x = (h1 >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0;
    let y = (h2 >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0;
    let z = (h3 >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0;
    Vec3::new(x, y, z).normalize_or_zero()
}

fn rand_f32_seeded(seed: u64) -> f32 {
    let x = seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(0x6c62272e07bb0142);
    (x >> 32) as f32 / u32::MAX as f32
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cohesion_spring_bounds() {
        let (s0, d0) = cohesion_to_spring(0.0);
        let (s1, d1) = cohesion_to_spring(1.0);
        assert!(s1 > s0);
        assert!(d1 > d0);
    }

    #[test]
    fn manager_ticks_without_panic() {
        let positions = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
        let mut mgr = CohesionManager::new(&positions, 0.8);
        let result = mgr.tick(0.016);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn dissolution_triggers_at_zero_cohesion() {
        let positions = vec![Vec3::X, Vec3::Y, Vec3::Z];
        let mut mgr = CohesionManager::new(&positions, 0.1);
        mgr.damage_cohesion(0.1);
        assert!(mgr.is_dissolving());
    }

    #[test]
    fn shockwave_imparts_velocity() {
        let positions = vec![Vec3::new(1.0, 0.0, 0.0)];
        let mut mgr = CohesionManager::new(&positions, 0.9);
        let before = mgr.glyphs[0].velocity();
        mgr.apply_shockwave(Vec3::ZERO, 5.0);
        let after = mgr.glyphs[0].velocity();
        assert!(after.length() > before.length());
    }

    #[test]
    fn cohesion_pull_scales_with_cohesion() {
        let a = Vec3::ZERO;
        let b = Vec3::new(1.0, 0.0, 0.0);
        let low  = cohesion_pull(a, b, 0.1, 0.016).length();
        let high = cohesion_pull(a, b, 0.9, 0.016).length();
        assert!(high > low);
    }
}
