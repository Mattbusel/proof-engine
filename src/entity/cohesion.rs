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
#[derive(Clone)]
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
    /// Multipliers on the spring constants cohesion would otherwise give.
    ///
    /// Cohesion says how strongly matter *wants* to hold its shape, which is a
    /// property of the thing. How fast it gets there, and whether it overshoots
    /// on the way, is a property of the shot: a body settling into an idle and
    /// a title card snapping into place want very different springs at the same
    /// cohesion. `1.0, 1.0` is the default cohesion curve.
    pub bind_stiffness: f32,
    pub bind_damping: f32,
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
            bind_stiffness: 1.0,
            bind_damping: 1.0,
        }
    }

    /// Step the cohesion spring by dt seconds.
    ///
    /// Returns the new glyph position including thermal jitter.
    pub fn tick(&mut self, dt: f32, cohesion: f32) -> Vec3 {
        // Recompute spring constants from current cohesion
        let (k, d) = cohesion_to_spring(cohesion);
        let stiffness = k * self.bind_stiffness.max(0.0);
        let damping = d * self.bind_damping.max(0.0);
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

    /// Apply an impulse (adds to spring velocity).
    ///
    /// An impulse is a change in momentum, so the same blow moves a light
    /// glyph further than a heavy one: dv = J / m.
    pub fn apply_impulse(&mut self, impulse: Vec3) {
        let dv = impulse / self.mass().max(0.01);
        self.spring.x.velocity += dv.x;
        self.spring.y.velocity += dv.y;
        self.spring.z.velocity += dv.z;
    }

    /// This glyph's inertia.
    pub fn mass(&self) -> f32 {
        self.spring.mass()
    }

    /// Set this glyph's inertia. Heavy glyphs form the core of a figure and
    /// swing slowly; light ones fly off first when the binding fails.
    pub fn set_mass(&mut self, mass: f32) {
        self.spring.set_mass(mass);
    }
}

// ── Entity-level cohesion manager ─────────────────────────────────────────────

/// Manages cohesion for all glyphs in an entity.
#[derive(Clone)]
pub struct CohesionManager {
    /// Where the ground is, if this matter is standing on any.
    ///
    /// In formation space, y downward: nothing may go below it. See
    /// [`CohesionManager::set_floor`].
    floor: Option<f32>,
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
            floor: None,
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
            let mut out: Vec<Vec3> = self
                .glyphs
                .iter_mut()
                .zip(self.burst_velocities.iter())
                .map(|(g, &bv)| {
                    let pos = g.position() + bv * dt;
                    // Update spring position to match drift
                    g.spring.x.position = pos.x;
                    g.spring.y.position = pos.y;
                    g.spring.z.position = pos.z;
                    // Dissolving matter is the furthest thing there is from its
                    // slot, so `drift` has to keep tracking it. Leaving it at
                    // its last bound value tells every reader downstream that a
                    // scattering cloud is still held together.
                    let target = Vec3::new(
                        g.spring.x.target,
                        g.spring.y.target,
                        g.spring.z.target,
                    );
                    g.drift = (pos - target).length();
                    pos
                })
                .collect();
            self.apply_floor(&mut out);
            return out;
        }

        let mut out: Vec<Vec3> = self
            .glyphs
            .iter_mut()
            .map(|g| g.tick(dt, self.cohesion))
            .collect();
        self.apply_floor(&mut out);
        out
    }

    /// Put a floor under the matter.
    ///
    /// `y` is where the ground is, in the same space the formation is in, and
    /// y grows downward — so nothing may end up with a larger y than this.
    /// `None` removes it.
    ///
    /// This is what stops a figure being a cloud that happens to hover at the
    /// right height. Matter driven into the ground stops at it, debris from a
    /// death piles up on it instead of falling through it, and a body that
    /// crouches has something to crouch *onto*.
    ///
    /// The bounce is deliberately small and the friction large: this is a stone
    /// floor and a body, not a ball.
    pub fn set_floor(&mut self, y: Option<f32>) {
        self.floor = y;
    }

    /// Where the floor is, if there is one.
    pub fn floor(&self) -> Option<f32> {
        self.floor
    }

    /// Stop anything that has gone through the floor, and take its energy.
    fn apply_floor(&mut self, out: &mut [Vec3]) {
        let Some(y) = self.floor else { return };
        for (g, p) in self.glyphs.iter_mut().zip(out.iter_mut()) {
            if p.y <= y {
                continue;
            }
            p.y = y;
            g.spring.y.position = y;
            // Most of the downward speed is absorbed by the ground and a little
            // is given back. Sideways speed is scrubbed off by friction, which
            // is what makes debris settle rather than skate.
            if g.spring.y.velocity > 0.0 {
                g.spring.y.velocity *= -0.18;
            }
            g.spring.x.velocity *= 0.72;
            g.spring.z.velocity *= 0.72;
            g.thermal_vel.y = g.thermal_vel.y.min(0.0);
        }
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

    /// Stop dissolving and let the springs take hold again.
    ///
    /// Dissolution is a one-way branch in `tick`: once it starts, glyphs coast
    /// on their burst velocities and the springs are never consulted again.
    /// That is right for a death, and wrong for everything else matter can do
    /// — scattering and re-forming into a different shape, a teleport, a
    /// transformation. Ending it puts the glyphs back under the springs from
    /// wherever they happen to have drifted to, so they fly home rather than
    /// snapping.
    pub fn end_dissolution(&mut self, cohesion: f32) {
        self.dissolution = None;
        self.burst_velocities.clear();
        self.cohesion = cohesion.clamp(0.0, 1.0);
        // The burst left the springs with no velocity of their own, because
        // dissolution moves positions directly. Give them a nudge outward so
        // the return reads as matter being pulled back rather than teleporting.
        for g in &mut self.glyphs {
            g.thermal_vel = Vec3::ZERO;
        }
    }

    /// Whether the matter is currently coming apart.
    pub fn dissolving(&self) -> bool {
        self.dissolution.is_some()
    }

    /// Apply thermal energy to all glyphs (makes them jitter).
    pub fn heat_all(&mut self, temperature: f32, time: f32) {
        for (i, g) in self.glyphs.iter_mut().enumerate() {
            g.heat(temperature, time + i as f32 * 0.37);
        }
    }

    /// Set how hard the springs pull, on top of what cohesion asks for.
    ///
    /// `1.0, 1.0` is the plain cohesion curve, which is underdamped on purpose:
    /// a body that has just been hit should wobble. Raising both makes matter
    /// arrive faster and stop when it gets there, which is what a title card or
    /// any deliberate transformation wants.
    pub fn set_bind(&mut self, stiffness_scale: f32, damping_scale: f32) {
        for g in &mut self.glyphs {
            g.bind_stiffness = stiffness_scale.max(0.0);
            g.bind_damping = damping_scale.max(0.0);
        }
    }

    /// Give each glyph its own spring, by slot.
    ///
    /// This is what makes a material a material. Cohesion says how strongly the
    /// *entity* holds together; these multipliers say how strongly each piece
    /// of matter in it resists being moved, and they are the difference between
    /// a steel blade and a wool cloak hanging off the same shoulders. Steel is
    /// four hundred thousand times stiffer than flesh in reality; nothing here
    /// needs that range, but the ordering has to be real or a sword bends like
    /// an arm.
    ///
    /// Damping is the other half and is just as material: steel rings because
    /// it barely damps, cloth does not because it damps almost completely.
    ///
    /// Shorter than the glyph list is fine: the rest keep what they have.
    pub fn set_bind_each(&mut self, springs: &[(f32, f32)]) {
        for (g, (k, c)) in self.glyphs.iter_mut().zip(springs.iter()) {
            g.bind_stiffness = k.max(0.0);
            g.bind_damping = c.max(0.0);
        }
    }

    /// Give each glyph its own inertia, by slot.
    ///
    /// Shorter than the glyph list is fine: the rest keep the mass they have.
    pub fn set_masses(&mut self, masses: &[f32]) {
        for (g, m) in self.glyphs.iter_mut().zip(masses.iter()) {
            g.set_mass(*m);
        }
    }

    /// Total mass of the matter in this entity.
    pub fn total_mass(&self) -> f32 {
        self.glyphs.iter().map(|g| g.mass()).sum()
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
        let mut burst = dissolution_burst(
            &self.glyphs.iter().map(|g| g.position()).collect::<Vec<_>>(),
            center,
        );
        // The blast delivers momentum, not speed: light matter sprays outward
        // while the heavy core barely shifts.
        for (v, g) in burst.iter_mut().zip(self.glyphs.iter()) {
            *v /= g.mass().max(0.01);
        }
        self.burst_velocities = burst;
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
