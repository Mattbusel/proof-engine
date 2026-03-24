//! Weapon physics and combat trail system for Chaos RPG.
//!
//! Provides swing arc simulation, weapon trail rendering (ribbon mesh generation),
//! impact effects (camera shake, debris, distortion rings), damage number popups
//! with physics-based arcs, combo-trail integration, block/parry feedback, and
//! per-element visual effect descriptors.

use glam::{Vec2, Vec3, Quat};
use crate::combat::Element;

// ============================================================================
// Constants
// ============================================================================

const MAX_TRAIL_SEGMENTS: usize = 32;
const MAX_DAMAGE_NUMBERS: usize = 50;
const GRAVITY: f32 = 9.81;
const IMPACT_COMPRESS_DURATION: f32 = 0.2;
const PARRY_TIME_SCALE: f32 = 0.3;
const PARRY_SLOW_DURATION: f32 = 0.5;
const DEFAULT_DEBRIS_COUNT_MIN: usize = 5;
const DEFAULT_DEBRIS_COUNT_MAX: usize = 10;

// ============================================================================
// WeaponType
// ============================================================================

/// The ten weapon archetypes available in Chaos RPG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponType {
    Sword,
    Axe,
    Mace,
    Staff,
    Dagger,
    Spear,
    Bow,
    Fist,
    Scythe,
    Whip,
}

impl WeaponType {
    /// All weapon types as a slice, useful for iteration.
    pub fn all() -> &'static [WeaponType] {
        &[
            WeaponType::Sword,
            WeaponType::Axe,
            WeaponType::Mace,
            WeaponType::Staff,
            WeaponType::Dagger,
            WeaponType::Spear,
            WeaponType::Bow,
            WeaponType::Fist,
            WeaponType::Scythe,
            WeaponType::Whip,
        ]
    }

    /// Human-readable name for display.
    pub fn name(self) -> &'static str {
        match self {
            WeaponType::Sword  => "Sword",
            WeaponType::Axe    => "Axe",
            WeaponType::Mace   => "Mace",
            WeaponType::Staff  => "Staff",
            WeaponType::Dagger => "Dagger",
            WeaponType::Spear  => "Spear",
            WeaponType::Bow    => "Bow",
            WeaponType::Fist   => "Fist",
            WeaponType::Scythe => "Scythe",
            WeaponType::Whip   => "Whip",
        }
    }
}

// ============================================================================
// WeaponProfile
// ============================================================================

/// Physical and visual properties that govern how a weapon behaves during
/// swings, impacts, and trail rendering.
#[derive(Debug, Clone)]
pub struct WeaponProfile {
    /// Mass in kilograms. Affects impact camera shake and knockback.
    pub mass: f32,
    /// Length in metres from grip to tip.
    pub length: f32,
    /// Swing speed multiplier (1.0 = baseline sword speed).
    pub swing_speed: f32,
    /// Force applied on contact (Newtons-ish, game units).
    pub impact_force: f32,
    /// Half-width of the rendered trail ribbon.
    pub trail_width: f32,
    /// Number of trail segments to spawn per swing.
    pub trail_segments: usize,
    /// Optional elemental affinity baked into the weapon.
    pub element: Option<Element>,
}

impl WeaponProfile {
    /// Construct with explicit values.
    pub fn new(
        mass: f32,
        length: f32,
        swing_speed: f32,
        impact_force: f32,
        trail_width: f32,
        trail_segments: usize,
        element: Option<Element>,
    ) -> Self {
        Self { mass, length, swing_speed, impact_force, trail_width, trail_segments, element }
    }
}

// ============================================================================
// WeaponProfiles — factory presets
// ============================================================================

/// Factory methods returning canonical `WeaponProfile` for each `WeaponType`.
pub struct WeaponProfiles;

impl WeaponProfiles {
    /// Return the preset profile for the given weapon type.
    pub fn get(weapon: WeaponType) -> WeaponProfile {
        match weapon {
            WeaponType::Sword => WeaponProfile {
                mass: 1.5,
                length: 1.0,
                swing_speed: 1.0,
                impact_force: 80.0,
                trail_width: 0.12,
                trail_segments: 24,
                element: None,
            },
            WeaponType::Axe => WeaponProfile {
                mass: 3.5,
                length: 0.9,
                swing_speed: 0.65,
                impact_force: 160.0,
                trail_width: 0.18,
                trail_segments: 18,
                element: None,
            },
            WeaponType::Mace => WeaponProfile {
                mass: 4.0,
                length: 0.8,
                swing_speed: 0.55,
                impact_force: 200.0,
                trail_width: 0.15,
                trail_segments: 16,
                element: None,
            },
            WeaponType::Staff => WeaponProfile {
                mass: 1.2,
                length: 1.6,
                swing_speed: 0.85,
                impact_force: 40.0,
                trail_width: 0.20,
                trail_segments: 28,
                element: Some(Element::Entropy),
            },
            WeaponType::Dagger => WeaponProfile {
                mass: 0.5,
                length: 0.35,
                swing_speed: 1.6,
                impact_force: 35.0,
                trail_width: 0.06,
                trail_segments: 20,
                element: None,
            },
            WeaponType::Spear => WeaponProfile {
                mass: 2.0,
                length: 2.0,
                swing_speed: 0.75,
                impact_force: 120.0,
                trail_width: 0.08,
                trail_segments: 22,
                element: None,
            },
            WeaponType::Bow => WeaponProfile {
                mass: 0.8,
                length: 1.3,
                swing_speed: 0.40,
                impact_force: 90.0,
                trail_width: 0.04,
                trail_segments: 30,
                element: None,
            },
            WeaponType::Fist => WeaponProfile {
                mass: 0.3,
                length: 0.25,
                swing_speed: 2.0,
                impact_force: 50.0,
                trail_width: 0.10,
                trail_segments: 14,
                element: None,
            },
            WeaponType::Scythe => WeaponProfile {
                mass: 3.0,
                length: 1.8,
                swing_speed: 0.70,
                impact_force: 140.0,
                trail_width: 0.22,
                trail_segments: 26,
                element: Some(Element::Shadow),
            },
            WeaponType::Whip => WeaponProfile {
                mass: 0.6,
                length: 3.0,
                swing_speed: 1.2,
                impact_force: 30.0,
                trail_width: 0.05,
                trail_segments: 32,
                element: None,
            },
        }
    }

    /// Convenience: return all presets as a vec of `(WeaponType, WeaponProfile)`.
    pub fn all() -> Vec<(WeaponType, WeaponProfile)> {
        WeaponType::all().iter().map(|&w| (w, Self::get(w))).collect()
    }
}

// ============================================================================
// SwingArc
// ============================================================================

/// Describes a circular swing arc in 3D space (in the XZ plane centred at
/// `origin`). The weapon tip traces from `start_angle` to `end_angle` over
/// `duration` seconds at the given `radius` (== weapon length).
#[derive(Debug, Clone)]
pub struct SwingArc {
    /// Starting angle in radians.
    pub start_angle: f32,
    /// Ending angle in radians.
    pub end_angle: f32,
    /// Total swing duration in seconds.
    pub duration: f32,
    /// Elapsed time since swing began.
    pub elapsed: f32,
    /// World-space origin of the swing (character pivot).
    pub origin: Vec3,
    /// Radius of the arc (weapon length).
    pub radius: f32,
}

impl SwingArc {
    /// Create a new swing arc.
    pub fn new(
        start_angle: f32,
        end_angle: f32,
        duration: f32,
        origin: Vec3,
        radius: f32,
    ) -> Self {
        Self {
            start_angle,
            end_angle,
            duration,
            elapsed: 0.0,
            origin,
            radius,
        }
    }

    /// Normalised progress of the swing `[0, 1]`.
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 { return 1.0; }
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    /// Whether the swing has completed.
    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// Angle at normalised time `t` (`[0,1]`).
    fn angle_at(&self, t: f32) -> f32 {
        self.start_angle + (self.end_angle - self.start_angle) * t
    }

    /// Position along the arc at normalised time `t` (`[0,1]`).
    /// The arc lies in the XZ plane relative to `origin`.
    pub fn sample(&self, t: f32) -> Vec3 {
        let t_clamped = t.clamp(0.0, 1.0);
        let angle = self.angle_at(t_clamped);
        let x = angle.cos() * self.radius;
        let z = angle.sin() * self.radius;
        self.origin + Vec3::new(x, 0.0, z)
    }

    /// Tangential velocity at normalised time `t`.
    /// The magnitude equals `radius * angular_velocity`.
    pub fn velocity_at(&self, t: f32) -> Vec3 {
        let t_clamped = t.clamp(0.0, 1.0);
        let angle = self.angle_at(t_clamped);
        let angular_vel = if self.duration > 0.0 {
            (self.end_angle - self.start_angle) / self.duration
        } else {
            0.0
        };
        let speed = self.radius * angular_vel;
        // Tangent to circle at angle: (-sin, 0, cos)
        Vec3::new(-angle.sin() * speed, 0.0, angle.cos() * speed)
    }

    /// Advance the arc by `dt` seconds. Returns `true` while still active.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        !self.finished()
    }
}

// ============================================================================
// WeaponTrailSegment
// ============================================================================

/// A single segment of the weapon trail ribbon.
#[derive(Debug, Clone)]
pub struct WeaponTrailSegment {
    /// World-space position of the segment centre.
    pub position: Vec3,
    /// Velocity (used for physics-based compression on impact).
    pub velocity: Vec3,
    /// Half-width of the ribbon at this segment.
    pub width: f32,
    /// RGBA colour.
    pub color: [f32; 4],
    /// Emission intensity (bloom).
    pub emission: f32,
    /// Age in seconds since this segment was spawned.
    pub age: f32,
}

impl WeaponTrailSegment {
    pub fn new(position: Vec3, velocity: Vec3, width: f32, color: [f32; 4], emission: f32) -> Self {
        Self { position, velocity, width, color, emission, age: 0.0 }
    }

    /// Update the segment by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.age += dt;
        self.position += self.velocity * dt;
        // Dampen velocity over time
        self.velocity *= (1.0 - 3.0 * dt).max(0.0);
    }

    /// Fade alpha based on age relative to a maximum lifetime.
    pub fn alpha(&self, max_age: f32) -> f32 {
        if max_age <= 0.0 { return 0.0; }
        (1.0 - (self.age / max_age)).clamp(0.0, 1.0)
    }
}

// ============================================================================
// TrailVertex — output for GPU ribbon rendering
// ============================================================================

/// A vertex emitted by the trail ribbon builder, ready for GPU upload.
#[derive(Debug, Clone, Copy)]
pub struct TrailVertex {
    pub position: Vec3,
    pub color: [f32; 4],
    pub emission: f32,
    pub uv: Vec2,
}

// ============================================================================
// WeaponTrail — ring-buffer trail manager
// ============================================================================

/// Manages a ring buffer of trail segments, spawning them along a swing arc
/// and applying impact compression.
#[derive(Debug, Clone)]
pub struct WeaponTrail {
    /// Ring buffer of trail segments.
    segments: Vec<WeaponTrailSegment>,
    /// Write index into the ring buffer.
    head: usize,
    /// Number of live segments in the buffer.
    count: usize,
    /// Timer controlling segment spawn rate.
    spawn_timer: f32,
    /// Interval between segment spawns (seconds).
    spawn_interval: f32,
    /// The weapon profile driving trail appearance.
    pub profile: WeaponProfile,
    /// Active swing arc (if any).
    active_arc: Option<SwingArc>,
    /// Impact compression state.
    impact_state: Option<ImpactCompressState>,
    /// Maximum segment age before culling (seconds).
    max_segment_age: f32,
    /// Combo intensity multiplier (1.0 = normal).
    combo_intensity: f32,
}

/// Internal state for the impact-compression animation.
#[derive(Debug, Clone)]
struct ImpactCompressState {
    contact_point: Vec3,
    elapsed: f32,
    duration: f32,
    phase: ImpactPhase,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ImpactPhase {
    /// Segments are being pulled toward the contact point.
    Compress,
    /// Segments spring back outward.
    SpringBack,
}

impl WeaponTrail {
    /// Create a new trail for the given weapon profile.
    pub fn new(profile: WeaponProfile) -> Self {
        let seg_count = profile.trail_segments.min(MAX_TRAIL_SEGMENTS);
        let spawn_interval = if seg_count > 0 { 1.0 / (seg_count as f32 * 2.0) } else { 0.05 };
        let mut segments = Vec::with_capacity(MAX_TRAIL_SEGMENTS);
        for _ in 0..MAX_TRAIL_SEGMENTS {
            segments.push(WeaponTrailSegment::new(
                Vec3::ZERO, Vec3::ZERO, 0.0, [0.0; 4], 0.0,
            ));
        }
        Self {
            segments,
            head: 0,
            count: 0,
            spawn_timer: 0.0,
            spawn_interval,
            profile,
            active_arc: None,
            impact_state: None,
            max_segment_age: 0.6,
            combo_intensity: 1.0,
        }
    }

    /// Set the combo intensity multiplier. Higher values make the trail wider,
    /// brighter, and more emissive.
    pub fn set_combo_intensity(&mut self, intensity: f32) {
        self.combo_intensity = intensity.max(1.0);
    }

    /// Begin a new swing, replacing any active arc.
    pub fn begin_swing(&mut self, arc: SwingArc) {
        self.active_arc = Some(arc);
        self.spawn_timer = 0.0;
    }

    /// Update the trail by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        // Pre-compute values that need &self before mutable borrow
        let trail_color = self.element_trail_color();
        let trail_width = self.profile.trail_width * self.combo_intensity;
        let base_emission = 0.5 * self.combo_intensity;

        // Tick the active arc and collect new segments (avoids borrow conflict)
        let mut new_segments: Vec<WeaponTrailSegment> = Vec::new();
        let mut arc_finished = false;
        if let Some(ref mut arc) = self.active_arc {
            arc.tick(dt);
            self.spawn_timer += dt;
            while self.spawn_timer >= self.spawn_interval {
                self.spawn_timer -= self.spawn_interval;
                let t = arc.progress();
                let pos = arc.sample(t);
                let vel = arc.velocity_at(t);
                new_segments.push(WeaponTrailSegment::new(
                    pos, vel * 0.1, trail_width, trail_color, base_emission,
                ));
            }
            arc_finished = arc.finished();
        }
        for seg in new_segments {
            self.push_segment(seg);
        }
        if arc_finished {
            self.active_arc = None;
        }

        // Age and update existing segments
        for i in 0..MAX_TRAIL_SEGMENTS {
            if self.segment_alive(i) {
                self.segments[i].update(dt);
            }
        }

        // Impact compression animation
        if let Some(ref mut state) = self.impact_state.clone() {
            state.elapsed += dt;
            match state.phase {
                ImpactPhase::Compress => {
                    // Pull segments toward impact point
                    let compress_strength = 8.0 * dt;
                    for i in 0..MAX_TRAIL_SEGMENTS {
                        if self.segment_alive(i) {
                            let diff = state.contact_point - self.segments[i].position;
                            let dist = diff.length();
                            if dist > 0.01 && dist < 2.0 {
                                let pull = diff.normalize() * compress_strength * (1.0 / (dist + 0.5));
                                self.segments[i].velocity += pull;
                            }
                        }
                    }
                    if state.elapsed >= IMPACT_COMPRESS_DURATION {
                        state.phase = ImpactPhase::SpringBack;
                        state.elapsed = 0.0;
                    }
                }
                ImpactPhase::SpringBack => {
                    // Push segments away from impact point
                    let spring_strength = 12.0 * dt;
                    for i in 0..MAX_TRAIL_SEGMENTS {
                        if self.segment_alive(i) {
                            let diff = self.segments[i].position - state.contact_point;
                            let dist = diff.length();
                            if dist > 0.01 && dist < 3.0 {
                                let push = diff.normalize() * spring_strength * (1.0 / (dist + 0.5));
                                self.segments[i].velocity += push;
                            }
                        }
                    }
                    if state.elapsed >= IMPACT_COMPRESS_DURATION {
                        self.impact_state = None;
                        return;
                    }
                }
            }
            self.impact_state = Some(state.clone());
        }
    }

    /// Trigger the impact compression effect at the given contact point.
    /// Segments near the contact will compress toward it, then spring back
    /// after `IMPACT_COMPRESS_DURATION` seconds.
    pub fn on_impact(&mut self, contact_point: Vec3) {
        // Boost emission near the impact
        for i in 0..MAX_TRAIL_SEGMENTS {
            if self.segment_alive(i) {
                let dist = (self.segments[i].position - contact_point).length();
                if dist < 1.5 {
                    self.segments[i].emission += 2.0 * (1.0 - dist / 1.5);
                }
            }
        }
        self.impact_state = Some(ImpactCompressState {
            contact_point,
            elapsed: 0.0,
            duration: IMPACT_COMPRESS_DURATION * 2.0,
            phase: ImpactPhase::Compress,
        });
    }

    /// Build triangle-strip render data from the current trail segments.
    pub fn get_render_data(&self) -> Vec<TrailVertex> {
        TrailRibbon::build(self)
    }

    // ── internal helpers ─────────────────────────────────────────────────

    fn push_segment(&mut self, seg: WeaponTrailSegment) {
        self.segments[self.head] = seg;
        self.head = (self.head + 1) % MAX_TRAIL_SEGMENTS;
        if self.count < MAX_TRAIL_SEGMENTS {
            self.count += 1;
        }
    }

    fn segment_alive(&self, index: usize) -> bool {
        if index >= MAX_TRAIL_SEGMENTS { return false; }
        // A segment is alive if its age is below the max and it has been written
        self.segments[index].age < self.max_segment_age && self.count > 0
    }

    fn ordered_segments(&self) -> Vec<&WeaponTrailSegment> {
        if self.count == 0 { return Vec::new(); }
        let mut out = Vec::with_capacity(self.count);
        let start = if self.count < MAX_TRAIL_SEGMENTS {
            0
        } else {
            self.head
        };
        for i in 0..self.count {
            let idx = (start + i) % MAX_TRAIL_SEGMENTS;
            if self.segments[idx].age < self.max_segment_age {
                out.push(&self.segments[idx]);
            }
        }
        out
    }

    fn element_trail_color(&self) -> [f32; 4] {
        match self.profile.element {
            Some(Element::Fire)      => [1.0, 0.5, 0.1, 1.0],
            Some(Element::Ice)       => [0.5, 0.85, 1.0, 1.0],
            Some(Element::Lightning) => [1.0, 0.95, 0.3, 1.0],
            Some(Element::Void)      => [0.3, 0.0, 0.5, 1.0],
            Some(Element::Entropy)   => [0.7, 0.2, 0.9, 1.0],
            Some(Element::Gravity)   => [0.3, 0.3, 0.7, 1.0],
            Some(Element::Radiant)   => [1.0, 1.0, 0.8, 1.0],
            Some(Element::Shadow)    => [0.15, 0.05, 0.25, 1.0],
            Some(Element::Temporal)  => [0.4, 0.9, 0.7, 1.0],
            Some(Element::Physical) | None => [0.9, 0.9, 0.95, 1.0],
        }
    }
}

// ============================================================================
// TrailRibbon — convert trail segments to a triangle-strip mesh
// ============================================================================

/// Converts a set of trail segments into a triangle-strip mesh suitable for
/// GPU rendering. Each segment produces two vertices (centre +/- width *
/// perpendicular direction). Colour fades with age; emission increases near
/// impact points.
pub struct TrailRibbon;

impl TrailRibbon {
    /// Build trail vertices from the weapon trail state.
    pub fn build(trail: &WeaponTrail) -> Vec<TrailVertex> {
        let segments = trail.ordered_segments();
        let seg_count = segments.len();
        if seg_count < 2 {
            return Vec::new();
        }

        let mut vertices = Vec::with_capacity(seg_count * 2);

        for i in 0..seg_count {
            let seg = &segments[i];
            let alpha = seg.alpha(trail.max_segment_age);
            let mut color = seg.color;
            color[3] *= alpha;

            // Compute perpendicular direction
            let forward = if i + 1 < seg_count {
                (segments[i + 1].position - seg.position).normalize_or_zero()
            } else if i > 0 {
                (seg.position - segments[i - 1].position).normalize_or_zero()
            } else {
                Vec3::X
            };

            let up = Vec3::Y;
            let perp = forward.cross(up).normalize_or_zero();
            let half_w = seg.width * 0.5;

            let uv_v = if seg_count > 1 { i as f32 / (seg_count - 1) as f32 } else { 0.0 };

            vertices.push(TrailVertex {
                position: seg.position + perp * half_w,
                color,
                emission: seg.emission,
                uv: Vec2::new(0.0, uv_v),
            });
            vertices.push(TrailVertex {
                position: seg.position - perp * half_w,
                color,
                emission: seg.emission,
                uv: Vec2::new(1.0, uv_v),
            });
        }

        vertices
    }

    /// Build index buffer for the triangle strip (pairs of triangles for each
    /// quad between consecutive segment pairs).
    pub fn build_indices(vertex_count: usize) -> Vec<u32> {
        if vertex_count < 4 { return Vec::new(); }
        let quad_count = vertex_count / 2 - 1;
        let mut indices = Vec::with_capacity(quad_count * 6);
        for q in 0..quad_count {
            let base = (q * 2) as u32;
            // Triangle 1
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            // Triangle 2
            indices.push(base + 1);
            indices.push(base + 3);
            indices.push(base + 2);
        }
        indices
    }
}

// ============================================================================
// ElementEffect — per-element impact visual descriptions
// ============================================================================

/// Describes the visual effect played when a weapon with a given element
/// strikes an entity.
#[derive(Debug, Clone)]
pub struct ElementEffect {
    /// Human-readable label for the effect.
    pub name: &'static str,
    /// Number of particles to spawn.
    pub particle_count: usize,
    /// Base colour of the effect.
    pub color: [f32; 4],
    /// Emission multiplier.
    pub emission: f32,
    /// Radius of the effect.
    pub radius: f32,
    /// Duration of the effect in seconds.
    pub duration: f32,
    /// Whether the effect chains / spreads to nearby targets.
    pub chains: bool,
    /// Number of chain targets.
    pub chain_count: usize,
    /// Maximum chain range.
    pub chain_range: f32,
}

impl ElementEffect {
    /// Get the canonical impact effect for the given element.
    pub fn for_element(element: Element) -> Self {
        match element {
            Element::Fire => Self {
                name: "Ember Burst",
                particle_count: 30,
                color: [1.0, 0.4, 0.1, 1.0],
                emission: 3.0,
                radius: 1.5,
                duration: 0.8,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Ice => Self {
                name: "Crystal Shatter",
                particle_count: 20,
                color: [0.5, 0.85, 1.0, 1.0],
                emission: 2.0,
                radius: 1.2,
                duration: 1.0,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Lightning => Self {
                name: "Arc Chain",
                particle_count: 15,
                color: [1.0, 0.95, 0.2, 1.0],
                emission: 5.0,
                radius: 0.5,
                duration: 0.3,
                chains: true,
                chain_count: 3,
                chain_range: 5.0,
            },
            Element::Void => Self {
                name: "Void Collapse",
                particle_count: 25,
                color: [0.2, 0.0, 0.4, 1.0],
                emission: 2.5,
                radius: 2.0,
                duration: 1.2,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Entropy => Self {
                name: "Chaos Splatter",
                particle_count: 40,
                color: [0.6, 0.1, 0.8, 1.0],
                emission: 4.0,
                radius: 2.5,
                duration: 1.5,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Gravity => Self {
                name: "Gravity Pulse",
                particle_count: 18,
                color: [0.3, 0.3, 0.6, 1.0],
                emission: 2.0,
                radius: 3.0,
                duration: 0.6,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Radiant => Self {
                name: "Radiant Burst",
                particle_count: 35,
                color: [1.0, 1.0, 0.7, 1.0],
                emission: 6.0,
                radius: 2.0,
                duration: 0.5,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Shadow => Self {
                name: "Shadow Tendrils",
                particle_count: 22,
                color: [0.1, 0.05, 0.2, 1.0],
                emission: 1.5,
                radius: 2.5,
                duration: 1.8,
                chains: true,
                chain_count: 2,
                chain_range: 3.0,
            },
            Element::Temporal => Self {
                name: "Time Fracture",
                particle_count: 16,
                color: [0.4, 0.9, 0.7, 1.0],
                emission: 3.5,
                radius: 1.8,
                duration: 2.0,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
            Element::Physical => Self {
                name: "Impact Spark",
                particle_count: 12,
                color: [0.85, 0.8, 0.75, 1.0],
                emission: 1.0,
                radius: 0.8,
                duration: 0.4,
                chains: false,
                chain_count: 0,
                chain_range: 0.0,
            },
        }
    }
}

// ============================================================================
// CameraShake
// ============================================================================

/// Screen camera shake triggered by weapon impacts.
#[derive(Debug, Clone)]
pub struct CameraShake {
    /// Remaining duration in seconds.
    pub duration: f32,
    /// Current intensity (decays over time).
    pub intensity: f32,
    /// Maximum intensity at spawn.
    pub max_intensity: f32,
    /// Current shake offset to apply to the camera.
    pub offset: Vec3,
    /// Frequency of the shake oscillation.
    pub frequency: f32,
    /// Elapsed time since shake began.
    pub elapsed: f32,
}

impl CameraShake {
    /// Create a shake proportional to weapon mass and velocity magnitude.
    pub fn from_impact(mass: f32, velocity_magnitude: f32) -> Self {
        let intensity = (mass * velocity_magnitude * 0.01).clamp(0.01, 2.0);
        let duration = (intensity * 0.3).clamp(0.1, 0.8);
        Self {
            duration,
            intensity,
            max_intensity: intensity,
            offset: Vec3::ZERO,
            frequency: 25.0,
            elapsed: 0.0,
        }
    }

    /// Advance the shake and return the current offset.
    pub fn update(&mut self, dt: f32) -> Vec3 {
        if self.duration <= 0.0 {
            self.offset = Vec3::ZERO;
            return Vec3::ZERO;
        }
        self.elapsed += dt;
        self.duration -= dt;
        let decay = (self.duration / (self.max_intensity * 0.3).max(0.01)).clamp(0.0, 1.0);
        let phase = self.elapsed * self.frequency;
        self.offset = Vec3::new(
            phase.sin() * self.intensity * decay,
            (phase * 1.3).cos() * self.intensity * decay * 0.7,
            (phase * 0.7).sin() * self.intensity * decay * 0.3,
        );
        self.offset
    }

    /// Whether the shake has finished.
    pub fn finished(&self) -> bool {
        self.duration <= 0.0
    }
}

// ============================================================================
// DebrisGlyph — flying glyph debris on hit
// ============================================================================

/// A small glyph fragment that flies off when an entity is struck.
#[derive(Debug, Clone)]
pub struct DebrisGlyph {
    /// The character rendered for this debris piece.
    pub glyph: char,
    /// World-space position.
    pub position: Vec3,
    /// World-space velocity.
    pub velocity: Vec3,
    /// Spin rate in radians/s.
    pub spin: f32,
    /// Current rotation angle.
    pub rotation: f32,
    /// Scale factor (shrinks over lifetime).
    pub scale: f32,
    /// RGBA colour.
    pub color: [f32; 4],
    /// Remaining lifetime in seconds.
    pub lifetime: f32,
    /// Maximum lifetime.
    pub max_lifetime: f32,
}

impl DebrisGlyph {
    /// Create debris flying outward from a contact point.
    pub fn spawn(contact: Vec3, direction: Vec3, glyph: char, color: [f32; 4]) -> Self {
        let speed = 3.0 + pseudo_random_from_pos(contact) * 4.0;
        let spread = Vec3::new(
            pseudo_random_component(contact.x),
            pseudo_random_component(contact.y).abs() * 0.5 + 0.5,
            pseudo_random_component(contact.z),
        );
        let vel = (direction.normalize_or_zero() + spread).normalize_or_zero() * speed;
        let lifetime = 0.5 + pseudo_random_from_pos(contact) * 0.8;
        Self {
            glyph,
            position: contact,
            velocity: vel,
            spin: (pseudo_random_component(contact.x + contact.z) * 10.0),
            rotation: 0.0,
            scale: 0.8 + pseudo_random_from_pos(contact) * 0.4,
            color,
            lifetime,
            max_lifetime: lifetime,
        }
    }

    /// Advance the debris physics.
    pub fn update(&mut self, dt: f32) {
        self.lifetime -= dt;
        self.position += self.velocity * dt;
        self.velocity.y -= GRAVITY * dt;
        self.velocity *= (1.0 - 1.5 * dt).max(0.0);
        self.rotation += self.spin * dt;
        let age_ratio = 1.0 - (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
        self.scale *= (1.0 - age_ratio * 0.3).max(0.1);
        self.color[3] = (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
    }

    /// Whether this debris has expired.
    pub fn dead(&self) -> bool {
        self.lifetime <= 0.0
    }
}

/// Spawn a batch of debris glyphs from a hit.
pub fn spawn_debris(
    contact: Vec3,
    direction: Vec3,
    color: [f32; 4],
    count: usize,
) -> Vec<DebrisGlyph> {
    let glyphs = ['*', '+', '#', '~', '^', '%', '!', '?', '@', '&'];
    let actual_count = count.clamp(DEFAULT_DEBRIS_COUNT_MIN, DEFAULT_DEBRIS_COUNT_MAX);
    (0..actual_count)
        .map(|i| {
            let offset = Vec3::new(i as f32 * 0.1, i as f32 * 0.05, -(i as f32) * 0.08);
            let g = glyphs[i % glyphs.len()];
            DebrisGlyph::spawn(contact + offset * 0.1, direction, g, color)
        })
        .collect()
}

// ============================================================================
// ShockwaveRing — screen-space distortion at impact
// ============================================================================

/// A screen-space distortion ring (shockwave) expanding outward from the
/// impact point.
#[derive(Debug, Clone)]
pub struct ShockwaveRing {
    /// Screen-space centre (normalised [0,1]).
    pub center: Vec2,
    /// Current ring radius (normalised screen units).
    pub radius: f32,
    /// Expansion speed (units/s).
    pub speed: f32,
    /// Ring thickness.
    pub thickness: f32,
    /// Distortion magnitude.
    pub distortion: f32,
    /// Remaining lifetime.
    pub lifetime: f32,
    /// Max lifetime.
    pub max_lifetime: f32,
}

impl ShockwaveRing {
    /// Create a new shockwave at screen-space position.
    pub fn new(center: Vec2, intensity: f32) -> Self {
        Self {
            center,
            radius: 0.0,
            speed: 0.8 + intensity * 0.4,
            thickness: 0.02 + intensity * 0.01,
            distortion: 0.03 * intensity,
            lifetime: 0.4 + intensity * 0.2,
            max_lifetime: 0.4 + intensity * 0.2,
        }
    }

    /// Advance the ring.
    pub fn update(&mut self, dt: f32) {
        self.lifetime -= dt;
        self.radius += self.speed * dt;
        let decay = (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
        self.distortion *= decay;
        self.thickness *= decay;
    }

    /// Whether the ring has expired.
    pub fn finished(&self) -> bool {
        self.lifetime <= 0.0
    }
}

// ============================================================================
// DamageNumber — physics-based popup
// ============================================================================

/// A floating damage number with physics-based arc movement.
#[derive(Debug, Clone)]
pub struct DamageNumber {
    /// The damage value to display.
    pub value: i32,
    /// World-space position.
    pub position: Vec3,
    /// Current velocity (upward + slight horizontal drift).
    pub velocity: Vec3,
    /// Display scale (crits are larger).
    pub scale: f32,
    /// RGBA colour.
    pub color: [f32; 4],
    /// Remaining lifetime in seconds.
    pub lifetime: f32,
    /// Maximum lifetime.
    pub max_lifetime: f32,
    /// Whether this was a critical hit.
    pub crit: bool,
    /// Element (for colour).
    pub element: Option<Element>,
}

impl DamageNumber {
    /// Spawn a new damage number at the given position.
    pub fn new(value: i32, position: Vec3, crit: bool, element: Option<Element>) -> Self {
        let base_scale = if crit { 1.8 } else { 1.0 };
        let lifetime = if crit { 1.5 } else { 1.0 };
        let rand_x = pseudo_random_component(position.x) * 1.5;
        let rand_z = pseudo_random_component(position.z) * 1.5;
        let upward_speed = if crit { 5.0 } else { 3.0 };

        let color = match element {
            Some(el) => {
                let c = el.color();
                [c.x, c.y, c.z, 1.0]
            }
            None => {
                if crit {
                    [1.0, 0.9, 0.1, 1.0] // Gold for crits
                } else {
                    [1.0, 1.0, 1.0, 1.0] // White
                }
            }
        };

        Self {
            value,
            position,
            velocity: Vec3::new(rand_x, upward_speed, rand_z),
            scale: base_scale,
            color,
            lifetime,
            max_lifetime: lifetime,
            crit,
            element,
        }
    }

    /// Advance the damage number physics.
    pub fn update(&mut self, dt: f32) {
        self.lifetime -= dt;
        self.position += self.velocity * dt;
        // Gravity pulls it down slightly for an arc
        self.velocity.y -= GRAVITY * 0.4 * dt;
        // Dampen horizontal drift
        self.velocity.x *= (1.0 - 2.0 * dt).max(0.0);
        self.velocity.z *= (1.0 - 2.0 * dt).max(0.0);
        // Fade out
        let age_ratio = (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
        self.color[3] = age_ratio;
        // Crit numbers have a dramatic scale pulse
        if self.crit {
            let life_pct = 1.0 - age_ratio;
            if life_pct < 0.15 {
                // Quick scale-up at the start
                self.scale = 1.8 + (life_pct / 0.15) * 0.5;
            } else {
                self.scale = 2.3 * age_ratio;
            }
        } else {
            self.scale = 1.0 * age_ratio.max(0.3);
        }
    }

    /// Whether this number has expired.
    pub fn dead(&self) -> bool {
        self.lifetime <= 0.0
    }
}

// ============================================================================
// DamageNumberManager — pooled damage number system
// ============================================================================

/// Object pool of `DamageNumber` instances. Re-uses slots to avoid allocation.
#[derive(Debug, Clone)]
pub struct DamageNumberManager {
    /// Pool of damage numbers.
    numbers: Vec<Option<DamageNumber>>,
    /// Maximum pool size.
    capacity: usize,
}

impl DamageNumberManager {
    /// Create a pool with the given capacity (default 50).
    pub fn new(capacity: usize) -> Self {
        Self {
            numbers: vec![None; capacity],
            capacity,
        }
    }

    /// Spawn a damage number. If the pool is full, the oldest number is
    /// replaced.
    pub fn spawn(&mut self, value: i32, position: Vec3, crit: bool, element: Option<Element>) {
        let dmg = DamageNumber::new(value, position, crit, element);
        // Try to find an empty slot
        for slot in self.numbers.iter_mut() {
            if slot.is_none() {
                *slot = Some(dmg);
                return;
            }
        }
        // If all slots are occupied, replace the one with the least lifetime remaining
        let mut min_life = f32::MAX;
        let mut min_idx = 0;
        for (i, slot) in self.numbers.iter().enumerate() {
            if let Some(ref n) = slot {
                if n.lifetime < min_life {
                    min_life = n.lifetime;
                    min_idx = i;
                }
            }
        }
        self.numbers[min_idx] = Some(dmg);
    }

    /// Update all active damage numbers.
    pub fn update(&mut self, dt: f32) {
        for slot in self.numbers.iter_mut() {
            if let Some(ref mut n) = slot {
                n.update(dt);
                if n.dead() {
                    *slot = None;
                }
            }
        }
    }

    /// Return references to all active damage numbers.
    pub fn active(&self) -> Vec<&DamageNumber> {
        self.numbers.iter().filter_map(|s| s.as_ref()).collect()
    }

    /// Current count of active numbers.
    pub fn active_count(&self) -> usize {
        self.numbers.iter().filter(|s| s.is_some()).count()
    }
}

impl Default for DamageNumberManager {
    fn default() -> Self {
        Self::new(MAX_DAMAGE_NUMBERS)
    }
}

// ============================================================================
// ImpactEffect — orchestrates all impact visuals
// ============================================================================

/// Full description of an impact event, combining camera shake, debris,
/// trail compression, damage number, shockwave, and element-specific effects.
#[derive(Debug, Clone)]
pub struct ImpactEffect {
    /// Camera shake to apply.
    pub camera_shake: CameraShake,
    /// Debris glyphs spawned.
    pub debris: Vec<DebrisGlyph>,
    /// Screen-space shockwave ring.
    pub shockwave: ShockwaveRing,
    /// Damage number to display.
    pub damage_number: DamageNumber,
    /// Element-specific visual effect (if weapon has an element).
    pub element_effect: Option<ElementEffect>,
    /// Contact point in world space.
    pub contact_point: Vec3,
    /// Whether this impact has been fully consumed by the renderer.
    pub consumed: bool,
}

impl ImpactEffect {
    /// Generate a complete impact from the weapon striking at a contact point.
    pub fn generate(
        weapon: &WeaponProfile,
        contact_point: Vec3,
        velocity_magnitude: f32,
        damage: i32,
        crit: bool,
        screen_pos: Vec2,
        hit_direction: Vec3,
    ) -> Self {
        let mass = weapon.mass;

        // Camera shake
        let camera_shake = CameraShake::from_impact(mass, velocity_magnitude);

        // Debris (5-10 glyphs)
        let debris_count = (5.0 + mass * 1.5).min(10.0) as usize;
        let debris_color = match weapon.element {
            Some(el) => {
                let c = el.color();
                [c.x, c.y, c.z, 1.0]
            }
            None => [0.85, 0.8, 0.75, 1.0],
        };
        let debris = spawn_debris(contact_point, hit_direction, debris_color, debris_count);

        // Shockwave ring
        let shock_intensity = (mass * velocity_magnitude * 0.005).clamp(0.5, 2.0);
        let shockwave = ShockwaveRing::new(screen_pos, shock_intensity);

        // Damage number
        let damage_number = DamageNumber::new(damage, contact_point + Vec3::Y * 0.5, crit, weapon.element);

        // Element effect
        let element_effect = weapon.element.map(ElementEffect::for_element);

        Self {
            camera_shake,
            debris,
            shockwave,
            damage_number,
            element_effect,
            contact_point,
            consumed: false,
        }
    }

    /// Advance all sub-effects by `dt`.
    pub fn update(&mut self, dt: f32) {
        self.camera_shake.update(dt);
        for d in self.debris.iter_mut() {
            d.update(dt);
        }
        self.debris.retain(|d| !d.dead());
        self.shockwave.update(dt);
        self.damage_number.update(dt);

        // Mark consumed when everything is done
        if self.camera_shake.finished()
            && self.debris.is_empty()
            && self.shockwave.finished()
            && self.damage_number.dead()
        {
            self.consumed = true;
        }
    }
}

// ============================================================================
// ComboTrailIntegration — combo counter affects trail visuals
// ============================================================================

/// Computes trail visual modifiers based on the current combo count.
#[derive(Debug, Clone)]
pub struct ComboTrailIntegration {
    /// Current combo count.
    pub combo_count: u32,
    /// Trail width multiplier.
    pub width_multiplier: f32,
    /// Trail emission multiplier.
    pub emission_multiplier: f32,
    /// Trail intensity multiplier.
    pub intensity_multiplier: f32,
    /// Whether a milestone effect should fire.
    pub milestone_pending: bool,
    /// The milestone tier that was just reached (10, 25, 50, 100).
    pub milestone_tier: u32,
}

impl ComboTrailIntegration {
    pub fn new() -> Self {
        Self {
            combo_count: 0,
            width_multiplier: 1.0,
            emission_multiplier: 1.0,
            intensity_multiplier: 1.0,
            milestone_pending: false,
            milestone_tier: 0,
        }
    }

    /// Update the combo count and recompute modifiers.
    pub fn set_combo(&mut self, count: u32) {
        let prev = self.combo_count;
        self.combo_count = count;

        // Scale modifiers with combo count (log scale for diminishing returns)
        let factor = 1.0 + (count as f32).ln().max(0.0) * 0.3;
        self.width_multiplier = factor.min(3.0);
        self.emission_multiplier = factor.min(4.0);
        self.intensity_multiplier = factor.min(5.0);

        // Check milestones
        self.milestone_pending = false;
        for &milestone in &[10u32, 25, 50, 100] {
            if prev < milestone && count >= milestone {
                self.milestone_pending = true;
                self.milestone_tier = milestone;
            }
        }
    }

    /// Consume and return the pending milestone tier, if any.
    pub fn take_milestone(&mut self) -> Option<u32> {
        if self.milestone_pending {
            self.milestone_pending = false;
            Some(self.milestone_tier)
        } else {
            None
        }
    }

    /// Apply modifiers to a weapon trail.
    pub fn apply_to_trail(&self, trail: &mut WeaponTrail) {
        trail.set_combo_intensity(self.intensity_multiplier);
    }
}

impl Default for ComboTrailIntegration {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ComboMilestoneEffect
// ============================================================================

/// Describes the special visual effect triggered at a combo milestone.
#[derive(Debug, Clone)]
pub struct ComboMilestoneEffect {
    /// The milestone tier.
    pub tier: u32,
    /// Particle burst count (scales with tier).
    pub particle_count: usize,
    /// Colour of the burst.
    pub color: [f32; 4],
    /// Emission strength.
    pub emission: f32,
    /// Shockwave radius.
    pub shockwave_radius: f32,
    /// Screen flash intensity.
    pub flash_intensity: f32,
    /// Duration of the effect.
    pub duration: f32,
    /// Remaining time.
    pub remaining: f32,
}

impl ComboMilestoneEffect {
    /// Create a milestone effect for the given tier.
    pub fn for_tier(tier: u32) -> Self {
        let scale = match tier {
            10  => 1.0,
            25  => 1.8,
            50  => 3.0,
            100 => 5.0,
            _   => 1.0,
        };
        let duration = 0.5 + scale * 0.2;
        Self {
            tier,
            particle_count: (20.0 * scale) as usize,
            color: match tier {
                10  => [1.0, 0.8, 0.2, 1.0],  // Gold
                25  => [0.2, 0.8, 1.0, 1.0],  // Cyan
                50  => [1.0, 0.3, 0.8, 1.0],  // Magenta
                100 => [1.0, 1.0, 1.0, 1.0],  // White (all elements)
                _   => [1.0, 1.0, 1.0, 1.0],
            },
            emission: 3.0 * scale,
            shockwave_radius: 0.5 * scale,
            flash_intensity: 0.3 * scale,
            duration,
            remaining: duration,
        }
    }

    /// Advance the effect.
    pub fn update(&mut self, dt: f32) {
        self.remaining -= dt;
    }

    /// Whether the effect has finished.
    pub fn finished(&self) -> bool {
        self.remaining <= 0.0
    }
}

// ============================================================================
// BlockEffect
// ============================================================================

/// Visual and physical feedback when an attack is blocked.
#[derive(Debug, Clone)]
pub struct BlockEffect {
    /// Contact point where weapons met.
    pub contact_point: Vec3,
    /// Spark particles.
    pub sparks: Vec<SparkParticle>,
    /// Direction the defender is pushed back.
    pub pushback_direction: Vec3,
    /// Pushback force magnitude.
    pub pushback_force: f32,
    /// Whether the attacker's trail should reverse.
    pub trail_bounce: bool,
    /// Remaining duration of the effect.
    pub duration: f32,
    /// Max duration.
    pub max_duration: f32,
}

/// A single spark particle emitted on block.
#[derive(Debug, Clone)]
pub struct SparkParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: [f32; 4],
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
}

impl SparkParticle {
    pub fn spawn(origin: Vec3, index: usize) -> Self {
        let angle = (index as f32) * 0.7;
        let speed = 4.0 + (index as f32) * 0.5;
        Self {
            position: origin,
            velocity: Vec3::new(
                angle.cos() * speed,
                2.0 + (index as f32 % 3.0) * 1.5,
                angle.sin() * speed,
            ),
            color: [1.0, 0.9, 0.3, 1.0],
            lifetime: 0.3 + (index as f32) * 0.02,
            max_lifetime: 0.3 + (index as f32) * 0.02,
            size: 0.05 + (index as f32) * 0.005,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.lifetime -= dt;
        self.position += self.velocity * dt;
        self.velocity.y -= GRAVITY * dt;
        self.velocity *= (1.0 - 4.0 * dt).max(0.0);
        self.color[3] = (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
        self.size *= (1.0 - 2.0 * dt).max(0.01);
    }

    pub fn dead(&self) -> bool {
        self.lifetime <= 0.0
    }
}

impl BlockEffect {
    /// Create a block effect from an attack hitting a defender's guard.
    pub fn generate(
        contact_point: Vec3,
        attacker_direction: Vec3,
        weapon_mass: f32,
        velocity_magnitude: f32,
    ) -> Self {
        let spark_count = (8.0 + weapon_mass * 2.0).min(20.0) as usize;
        let sparks: Vec<SparkParticle> = (0..spark_count)
            .map(|i| SparkParticle::spawn(contact_point, i))
            .collect();
        let pushback = -attacker_direction.normalize_or_zero();
        let force = (weapon_mass * velocity_magnitude * 0.05).clamp(0.5, 5.0);
        let dur = 0.3 + force * 0.05;
        Self {
            contact_point,
            sparks,
            pushback_direction: pushback,
            pushback_force: force,
            trail_bounce: true,
            duration: dur,
            max_duration: dur,
        }
    }

    /// Advance the block effect.
    pub fn update(&mut self, dt: f32) {
        self.duration -= dt;
        for s in self.sparks.iter_mut() {
            s.update(dt);
        }
        self.sparks.retain(|s| !s.dead());
        // Decay pushback over time
        let decay = (self.duration / self.max_duration).clamp(0.0, 1.0);
        self.pushback_force *= decay;
    }

    /// Whether the block effect has fully expired.
    pub fn finished(&self) -> bool {
        self.duration <= 0.0 && self.sparks.is_empty()
    }
}

// ============================================================================
// ParryEffect
// ============================================================================

/// Visual and gameplay feedback for a perfect-timing parry.
#[derive(Debug, Clone)]
pub struct ParryEffect {
    /// Contact point in world space.
    pub contact_point: Vec3,
    /// Current time-scale (starts at `PARRY_TIME_SCALE`, returns to 1.0).
    pub time_scale: f32,
    /// Duration the slow-motion effect persists.
    pub slow_duration: f32,
    /// Elapsed time since the parry.
    pub elapsed: f32,
    /// Flash intensity (starts high, decays quickly).
    pub flash_intensity: f32,
    /// Whether the attacker should be stunned.
    pub attacker_stunned: bool,
    /// Duration of the attacker stun.
    pub stun_duration: f32,
    /// Particle burst spawned at parry.
    pub burst_particles: Vec<ParryBurstParticle>,
    /// Whether the effect has been fully applied.
    pub consumed: bool,
}

/// A single particle from the parry burst.
#[derive(Debug, Clone)]
pub struct ParryBurstParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: [f32; 4],
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub emission: f32,
}

impl ParryBurstParticle {
    pub fn spawn(origin: Vec3, index: usize) -> Self {
        let angle = (index as f32) * 0.5;
        let elevation = ((index as f32) * 0.37).sin() * 0.8;
        let speed = 6.0 + (index as f32) * 0.3;
        let lifetime = 0.5 + (index as f32) * 0.03;
        Self {
            position: origin,
            velocity: Vec3::new(
                angle.cos() * speed,
                elevation * speed,
                angle.sin() * speed,
            ),
            color: [1.0, 1.0, 0.9, 1.0],
            lifetime,
            max_lifetime: lifetime,
            size: 0.08,
            emission: 5.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.lifetime -= dt;
        self.position += self.velocity * dt;
        self.velocity *= (1.0 - 3.0 * dt).max(0.0);
        let age_ratio = (self.lifetime / self.max_lifetime).clamp(0.0, 1.0);
        self.color[3] = age_ratio;
        self.emission *= age_ratio;
        self.size *= (1.0 - 1.5 * dt).max(0.01);
    }

    pub fn dead(&self) -> bool {
        self.lifetime <= 0.0
    }
}

impl ParryEffect {
    /// Create a parry effect at the given contact point.
    pub fn generate(contact_point: Vec3) -> Self {
        let particle_count = 30;
        let burst: Vec<ParryBurstParticle> = (0..particle_count)
            .map(|i| ParryBurstParticle::spawn(contact_point, i))
            .collect();
        Self {
            contact_point,
            time_scale: PARRY_TIME_SCALE,
            slow_duration: PARRY_SLOW_DURATION,
            elapsed: 0.0,
            flash_intensity: 3.0,
            attacker_stunned: true,
            stun_duration: 1.0,
            burst_particles: burst,
            consumed: false,
        }
    }

    /// Advance the parry effect. Note: `dt` here is real-time delta, not
    /// affected by the slow-mo.
    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt;

        // Flash decays quickly
        self.flash_intensity = (self.flash_intensity - dt * 10.0).max(0.0);

        // Time scale ramps back to 1.0 after slow_duration
        if self.elapsed >= self.slow_duration {
            let ramp = ((self.elapsed - self.slow_duration) / 0.3).clamp(0.0, 1.0);
            self.time_scale = PARRY_TIME_SCALE + (1.0 - PARRY_TIME_SCALE) * ramp;
        }

        // Update burst particles (in real-time, not slowed)
        for p in self.burst_particles.iter_mut() {
            p.update(dt);
        }
        self.burst_particles.retain(|p| !p.dead());

        // Stun countdown
        if self.attacker_stunned {
            self.stun_duration -= dt;
            if self.stun_duration <= 0.0 {
                self.attacker_stunned = false;
            }
        }

        // Consumed when all effects are done
        if self.time_scale >= 0.99
            && self.flash_intensity <= 0.0
            && self.burst_particles.is_empty()
            && !self.attacker_stunned
        {
            self.consumed = true;
        }
    }

    /// The current time scale to apply to the game simulation.
    pub fn current_time_scale(&self) -> f32 {
        self.time_scale
    }

    /// Whether the parry effect has completed.
    pub fn finished(&self) -> bool {
        self.consumed
    }
}

// ============================================================================
// WeaponPhysicsSystem — top-level orchestrator
// ============================================================================

/// Top-level system that ties together weapon trails, impact effects, damage
/// numbers, and combo integration.
#[derive(Debug, Clone)]
pub struct WeaponPhysicsSystem {
    /// The active weapon trail.
    pub trail: WeaponTrail,
    /// Active impact effects.
    pub impacts: Vec<ImpactEffect>,
    /// Damage number manager (pooled).
    pub damage_numbers: DamageNumberManager,
    /// Combo trail integration.
    pub combo_integration: ComboTrailIntegration,
    /// Active combo milestone effects.
    pub milestone_effects: Vec<ComboMilestoneEffect>,
    /// Active block effects.
    pub block_effects: Vec<BlockEffect>,
    /// Active parry effect (at most one at a time).
    pub parry_effect: Option<ParryEffect>,
    /// Global time scale (affected by parry slow-mo).
    pub time_scale: f32,
}

impl WeaponPhysicsSystem {
    /// Create a new system for the given weapon type.
    pub fn new(weapon_type: WeaponType) -> Self {
        let profile = WeaponProfiles::get(weapon_type);
        Self {
            trail: WeaponTrail::new(profile),
            impacts: Vec::new(),
            damage_numbers: DamageNumberManager::default(),
            combo_integration: ComboTrailIntegration::new(),
            milestone_effects: Vec::new(),
            block_effects: Vec::new(),
            parry_effect: None,
            time_scale: 1.0,
        }
    }

    /// Switch to a different weapon type, resetting the trail.
    pub fn switch_weapon(&mut self, weapon_type: WeaponType) {
        let profile = WeaponProfiles::get(weapon_type);
        self.trail = WeaponTrail::new(profile);
    }

    /// Begin a swing with the given arc parameters.
    pub fn begin_swing(&mut self, arc: SwingArc) {
        self.trail.begin_swing(arc);
    }

    /// Handle a weapon hitting an entity.
    pub fn on_hit(
        &mut self,
        contact_point: Vec3,
        velocity_magnitude: f32,
        damage: i32,
        crit: bool,
        screen_pos: Vec2,
        hit_direction: Vec3,
    ) {
        // Trail compression
        self.trail.on_impact(contact_point);

        // Generate full impact effect
        let impact = ImpactEffect::generate(
            &self.trail.profile,
            contact_point,
            velocity_magnitude,
            damage,
            crit,
            screen_pos,
            hit_direction,
        );
        self.impacts.push(impact);

        // Damage number
        self.damage_numbers.spawn(damage, contact_point + Vec3::Y * 0.5, crit, self.trail.profile.element);
    }

    /// Handle an attack being blocked.
    pub fn on_block(
        &mut self,
        contact_point: Vec3,
        attacker_direction: Vec3,
        velocity_magnitude: f32,
    ) {
        let block = BlockEffect::generate(
            contact_point,
            attacker_direction,
            self.trail.profile.mass,
            velocity_magnitude,
        );
        self.block_effects.push(block);
    }

    /// Handle a perfect parry.
    pub fn on_parry(&mut self, contact_point: Vec3) {
        let parry = ParryEffect::generate(contact_point);
        self.parry_effect = Some(parry);
    }

    /// Update the combo count (from the combo tracker).
    pub fn update_combo(&mut self, combo_count: u32) {
        self.combo_integration.set_combo(combo_count);
        self.combo_integration.apply_to_trail(&mut self.trail);
        if let Some(tier) = self.combo_integration.take_milestone() {
            self.milestone_effects.push(ComboMilestoneEffect::for_tier(tier));
        }
    }

    /// Tick all systems by `dt` (real-time).
    pub fn update(&mut self, dt: f32) {
        // Apply parry time scale
        self.time_scale = if let Some(ref parry) = self.parry_effect {
            parry.current_time_scale()
        } else {
            1.0
        };
        let game_dt = dt * self.time_scale;

        // Trail
        self.trail.update(game_dt);

        // Impact effects
        for impact in self.impacts.iter_mut() {
            impact.update(game_dt);
        }
        self.impacts.retain(|i| !i.consumed);

        // Damage numbers
        self.damage_numbers.update(game_dt);

        // Milestone effects
        for m in self.milestone_effects.iter_mut() {
            m.update(game_dt);
        }
        self.milestone_effects.retain(|m| !m.finished());

        // Block effects
        for b in self.block_effects.iter_mut() {
            b.update(game_dt);
        }
        self.block_effects.retain(|b| !b.finished());

        // Parry effect (uses real dt, not game dt)
        if let Some(ref mut parry) = self.parry_effect {
            parry.update(dt);
            if parry.finished() {
                self.parry_effect = None;
            }
        }
    }

    /// Get trail render data for the GPU.
    pub fn trail_vertices(&self) -> Vec<TrailVertex> {
        self.trail.get_render_data()
    }
}

// ============================================================================
// Pseudo-random helpers (deterministic, no external crate needed)
// ============================================================================

/// Deterministic pseudo-random float in [0, 1] from a Vec3 position.
fn pseudo_random_from_pos(p: Vec3) -> f32 {
    let seed = (p.x * 12.9898 + p.y * 78.233 + p.z * 45.164).sin() * 43758.5453;
    seed.fract().abs()
}

/// Deterministic pseudo-random float in [-1, 1] from a single f32.
fn pseudo_random_component(v: f32) -> f32 {
    let seed = (v * 127.1 + 311.7).sin() * 43758.5453;
    seed.fract() * 2.0 - 1.0
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // ── SwingArc tests ───────────────────────────────────────────────────

    #[test]
    fn swing_arc_sample_start_and_end() {
        let arc = SwingArc::new(0.0, PI, 1.0, Vec3::ZERO, 2.0);
        let start = arc.sample(0.0);
        let end = arc.sample(1.0);
        // At t=0, angle=0 => position = (radius, 0, 0)
        assert!((start.x - 2.0).abs() < 0.001);
        assert!(start.z.abs() < 0.001);
        // At t=1, angle=PI => position = (-radius, 0, ~0)
        assert!((end.x + 2.0).abs() < 0.001);
        assert!(end.z.abs() < 0.01);
    }

    #[test]
    fn swing_arc_sample_with_origin() {
        let origin = Vec3::new(5.0, 0.0, 3.0);
        let arc = SwingArc::new(0.0, PI, 1.0, origin, 1.0);
        let mid = arc.sample(0.5);
        // At t=0.5, angle=PI/2 => x = cos(PI/2)=0, z = sin(PI/2)=1
        assert!((mid.x - 5.0).abs() < 0.01);
        assert!((mid.z - 4.0).abs() < 0.01);
    }

    #[test]
    fn swing_arc_velocity_direction() {
        let arc = SwingArc::new(0.0, PI, 1.0, Vec3::ZERO, 2.0);
        let vel = arc.velocity_at(0.0);
        // At angle=0 tangent is (−sin(0), 0, cos(0)) * speed = (0, 0, 1)*speed
        // angular_vel = PI/1 = PI, speed = 2*PI
        assert!(vel.x.abs() < 0.01);
        assert!((vel.z - 2.0 * PI).abs() < 0.1);
    }

    #[test]
    fn swing_arc_progress_clamp() {
        let mut arc = SwingArc::new(0.0, PI, 1.0, Vec3::ZERO, 1.0);
        assert!((arc.progress() - 0.0).abs() < 0.001);
        arc.elapsed = 0.5;
        assert!((arc.progress() - 0.5).abs() < 0.001);
        arc.elapsed = 2.0;
        assert!((arc.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn swing_arc_tick_finishes() {
        let mut arc = SwingArc::new(0.0, PI, 0.5, Vec3::ZERO, 1.0);
        assert!(arc.tick(0.3));
        assert!(!arc.finished());
        assert!(!arc.tick(0.3));
        assert!(arc.finished());
    }

    // ── WeaponTrailSegment tests ─────────────────────────────────────────

    #[test]
    fn trail_segment_ages() {
        let mut seg = WeaponTrailSegment::new(
            Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 0.1, [1.0; 4], 1.0,
        );
        assert!((seg.age - 0.0).abs() < 0.001);
        seg.update(0.1);
        assert!((seg.age - 0.1).abs() < 0.001);
        // Position should have moved
        assert!(seg.position.x > 0.0);
    }

    #[test]
    fn trail_segment_alpha_fades() {
        let mut seg = WeaponTrailSegment::new(
            Vec3::ZERO, Vec3::ZERO, 0.1, [1.0; 4], 1.0,
        );
        assert!((seg.alpha(1.0) - 1.0).abs() < 0.001);
        seg.age = 0.5;
        assert!((seg.alpha(1.0) - 0.5).abs() < 0.001);
        seg.age = 1.0;
        assert!((seg.alpha(1.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn trail_segment_velocity_dampens() {
        let mut seg = WeaponTrailSegment::new(
            Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 0.1, [1.0; 4], 1.0,
        );
        let initial_speed = seg.velocity.length();
        seg.update(0.1);
        assert!(seg.velocity.length() < initial_speed);
    }

    // ── DamageNumber tests ───────────────────────────────────────────────

    #[test]
    fn damage_number_moves_upward_initially() {
        let mut dmg = DamageNumber::new(100, Vec3::ZERO, false, None);
        let initial_y = dmg.position.y;
        dmg.update(0.05);
        assert!(dmg.position.y > initial_y);
    }

    #[test]
    fn damage_number_arcs_back_down() {
        let mut dmg = DamageNumber::new(100, Vec3::ZERO, false, None);
        // Let it rise
        for _ in 0..10 {
            dmg.update(0.05);
        }
        let peak_y = dmg.position.y;
        // Continue and it should come back down (gravity)
        for _ in 0..30 {
            dmg.update(0.05);
        }
        assert!(dmg.position.y < peak_y);
    }

    #[test]
    fn damage_number_crit_is_larger() {
        let normal = DamageNumber::new(100, Vec3::ZERO, false, None);
        let crit = DamageNumber::new(100, Vec3::ZERO, true, None);
        assert!(crit.scale > normal.scale);
    }

    #[test]
    fn damage_number_fades_alpha() {
        let mut dmg = DamageNumber::new(50, Vec3::ZERO, false, None);
        assert!((dmg.color[3] - 1.0).abs() < 0.01);
        for _ in 0..20 {
            dmg.update(0.05);
        }
        assert!(dmg.color[3] < 1.0);
    }

    #[test]
    fn damage_number_dies_after_lifetime() {
        let mut dmg = DamageNumber::new(50, Vec3::ZERO, false, None);
        assert!(!dmg.dead());
        for _ in 0..100 {
            dmg.update(0.05);
        }
        assert!(dmg.dead());
    }

    // ── DamageNumberManager tests ────────────────────────────────────────

    #[test]
    fn damage_manager_spawns_and_updates() {
        let mut mgr = DamageNumberManager::new(5);
        mgr.spawn(100, Vec3::ZERO, false, None);
        mgr.spawn(200, Vec3::ONE, true, Some(Element::Fire));
        assert_eq!(mgr.active_count(), 2);
        // Update until they expire
        for _ in 0..100 {
            mgr.update(0.05);
        }
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn damage_manager_replaces_oldest_when_full() {
        let mut mgr = DamageNumberManager::new(3);
        mgr.spawn(1, Vec3::ZERO, false, None);
        mgr.spawn(2, Vec3::ZERO, false, None);
        mgr.spawn(3, Vec3::ZERO, false, None);
        assert_eq!(mgr.active_count(), 3);
        // Pool is full, next spawn replaces oldest
        mgr.update(0.5); // age them so there's a "least lifetime"
        mgr.spawn(4, Vec3::ZERO, false, None);
        assert_eq!(mgr.active_count(), 3);
    }

    // ── WeaponTrail tests ────────────────────────────────────────────────

    #[test]
    fn trail_spawns_segments_during_swing() {
        let profile = WeaponProfiles::get(WeaponType::Sword);
        let mut trail = WeaponTrail::new(profile);
        let arc = SwingArc::new(0.0, PI, 0.5, Vec3::ZERO, 1.0);
        trail.begin_swing(arc);
        // Run several update ticks
        for _ in 0..20 {
            trail.update(0.025);
        }
        let verts = trail.get_render_data();
        // Should have generated some vertices
        assert!(!verts.is_empty());
    }

    #[test]
    fn trail_impact_modifies_segments() {
        let profile = WeaponProfiles::get(WeaponType::Axe);
        let mut trail = WeaponTrail::new(profile);
        let arc = SwingArc::new(0.0, PI, 0.5, Vec3::ZERO, 1.0);
        trail.begin_swing(arc);
        for _ in 0..10 {
            trail.update(0.025);
        }
        trail.on_impact(Vec3::new(0.5, 0.0, 0.5));
        // After impact, further updates should not panic
        for _ in 0..10 {
            trail.update(0.025);
        }
    }

    // ── TrailRibbon tests ────────────────────────────────────────────────

    #[test]
    fn trail_ribbon_vertex_pairs() {
        let profile = WeaponProfiles::get(WeaponType::Sword);
        let mut trail = WeaponTrail::new(profile);
        let arc = SwingArc::new(0.0, PI, 0.3, Vec3::ZERO, 1.0);
        trail.begin_swing(arc);
        for _ in 0..15 {
            trail.update(0.02);
        }
        let verts = trail.get_render_data();
        // Vertex count should be even (pairs)
        assert_eq!(verts.len() % 2, 0);
    }

    #[test]
    fn trail_ribbon_indices_valid() {
        let indices = TrailRibbon::build_indices(8);
        assert!(!indices.is_empty());
        // 4 quads * 6 = should be 3 quads * 6 = 18
        assert_eq!(indices.len(), 18);
        for &idx in &indices {
            assert!(idx < 8);
        }
    }

    // ── CameraShake tests ────────────────────────────────────────────────

    #[test]
    fn camera_shake_decays() {
        let mut shake = CameraShake::from_impact(3.0, 10.0);
        assert!(!shake.finished());
        let initial_intensity = shake.intensity;
        for _ in 0..50 {
            shake.update(0.02);
        }
        assert!(shake.finished() || shake.offset.length() < initial_intensity);
    }

    #[test]
    fn camera_shake_zero_mass() {
        let shake = CameraShake::from_impact(0.0, 0.0);
        assert!(shake.intensity <= 0.01);
    }

    // ── ComboTrailIntegration tests ──────────────────────────────────────

    #[test]
    fn combo_integration_milestones() {
        let mut combo = ComboTrailIntegration::new();
        combo.set_combo(9);
        assert!(!combo.milestone_pending);
        combo.set_combo(10);
        assert!(combo.milestone_pending);
        assert_eq!(combo.milestone_tier, 10);
        let tier = combo.take_milestone();
        assert_eq!(tier, Some(10));
        assert!(!combo.milestone_pending);
    }

    #[test]
    fn combo_integration_scaling() {
        let mut combo = ComboTrailIntegration::new();
        combo.set_combo(1);
        let w1 = combo.width_multiplier;
        combo.set_combo(50);
        let w50 = combo.width_multiplier;
        assert!(w50 > w1);
    }

    // ── ImpactEffect tests ───────────────────────────────────────────────

    #[test]
    fn impact_effect_generates_all_components() {
        let profile = WeaponProfiles::get(WeaponType::Mace);
        let impact = ImpactEffect::generate(
            &profile,
            Vec3::new(1.0, 0.0, 1.0),
            15.0,
            250,
            true,
            Vec2::new(0.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
        );
        assert!(!impact.consumed);
        assert!(!impact.debris.is_empty());
        assert!(impact.damage_number.crit);
        assert_eq!(impact.damage_number.value, 250);
    }

    #[test]
    fn impact_effect_eventually_consumed() {
        let profile = WeaponProfiles::get(WeaponType::Dagger);
        let mut impact = ImpactEffect::generate(
            &profile,
            Vec3::ZERO,
            5.0,
            30,
            false,
            Vec2::new(0.5, 0.5),
            Vec3::X,
        );
        for _ in 0..200 {
            impact.update(0.05);
        }
        assert!(impact.consumed);
    }

    // ── BlockEffect tests ────────────────────────────────────────────────

    #[test]
    fn block_effect_pushback_direction() {
        let block = BlockEffect::generate(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            2.0,
            10.0,
        );
        // Pushback should be opposite to attacker direction
        assert!(block.pushback_direction.x < 0.0);
    }

    #[test]
    fn block_effect_finishes() {
        let mut block = BlockEffect::generate(
            Vec3::ZERO,
            Vec3::X,
            1.0,
            5.0,
        );
        for _ in 0..100 {
            block.update(0.05);
        }
        assert!(block.finished());
    }

    // ── ParryEffect tests ────────────────────────────────────────────────

    #[test]
    fn parry_effect_slows_time() {
        let parry = ParryEffect::generate(Vec3::ZERO);
        assert!((parry.time_scale - PARRY_TIME_SCALE).abs() < 0.01);
        assert!(parry.attacker_stunned);
    }

    #[test]
    fn parry_effect_time_returns_to_normal() {
        let mut parry = ParryEffect::generate(Vec3::ZERO);
        for _ in 0..200 {
            parry.update(0.02);
        }
        assert!(parry.time_scale > 0.95);
    }

    #[test]
    fn parry_effect_finishes() {
        let mut parry = ParryEffect::generate(Vec3::ZERO);
        for _ in 0..300 {
            parry.update(0.02);
        }
        assert!(parry.finished());
    }

    // ── WeaponPhysicsSystem integration tests ────────────────────────────

    #[test]
    fn system_swing_and_hit() {
        let mut sys = WeaponPhysicsSystem::new(WeaponType::Sword);
        let arc = SwingArc::new(0.0, PI, 0.3, Vec3::ZERO, 1.0);
        sys.begin_swing(arc);
        for _ in 0..10 {
            sys.update(0.02);
        }
        sys.on_hit(
            Vec3::new(1.0, 0.0, 0.0),
            8.0, 100, false,
            Vec2::new(0.5, 0.5),
            Vec3::X,
        );
        assert_eq!(sys.impacts.len(), 1);
        assert_eq!(sys.damage_numbers.active_count(), 1);
    }

    #[test]
    fn system_combo_milestones() {
        let mut sys = WeaponPhysicsSystem::new(WeaponType::Fist);
        sys.update_combo(9);
        assert!(sys.milestone_effects.is_empty());
        sys.update_combo(10);
        assert_eq!(sys.milestone_effects.len(), 1);
        assert_eq!(sys.milestone_effects[0].tier, 10);
    }

    #[test]
    fn system_parry_slows_game() {
        let mut sys = WeaponPhysicsSystem::new(WeaponType::Sword);
        sys.on_parry(Vec3::ZERO);
        sys.update(0.01);
        assert!(sys.time_scale < 1.0);
    }

    // ── WeaponProfiles tests ─────────────────────────────────────────────

    #[test]
    fn all_weapon_profiles_valid() {
        for &wt in WeaponType::all() {
            let p = WeaponProfiles::get(wt);
            assert!(p.mass > 0.0, "{:?} mass must be positive", wt);
            assert!(p.length > 0.0, "{:?} length must be positive", wt);
            assert!(p.swing_speed > 0.0, "{:?} swing_speed must be positive", wt);
            assert!(p.impact_force > 0.0, "{:?} impact_force must be positive", wt);
            assert!(p.trail_width > 0.0, "{:?} trail_width must be positive", wt);
            assert!(p.trail_segments > 0, "{:?} trail_segments must be > 0", wt);
        }
    }

    #[test]
    fn sword_is_faster_than_axe() {
        let sword = WeaponProfiles::get(WeaponType::Sword);
        let axe = WeaponProfiles::get(WeaponType::Axe);
        assert!(sword.swing_speed > axe.swing_speed);
        assert!(sword.mass < axe.mass);
    }

    #[test]
    fn dagger_is_fastest() {
        let dagger = WeaponProfiles::get(WeaponType::Dagger);
        for &wt in WeaponType::all() {
            if wt == WeaponType::Dagger || wt == WeaponType::Fist {
                continue;
            }
            let p = WeaponProfiles::get(wt);
            assert!(
                dagger.swing_speed >= p.swing_speed,
                "Dagger should be faster than {:?}", wt
            );
        }
    }

    #[test]
    fn element_effects_for_all_elements() {
        let elements = [
            Element::Physical, Element::Fire, Element::Ice, Element::Lightning,
            Element::Void, Element::Entropy, Element::Gravity, Element::Radiant,
            Element::Shadow, Element::Temporal,
        ];
        for el in &elements {
            let eff = ElementEffect::for_element(*el);
            assert!(eff.particle_count > 0);
            assert!(eff.duration > 0.0);
        }
    }

    #[test]
    fn shockwave_ring_expands() {
        let mut ring = ShockwaveRing::new(Vec2::new(0.5, 0.5), 1.0);
        let r0 = ring.radius;
        ring.update(0.1);
        assert!(ring.radius > r0);
    }

    #[test]
    fn shockwave_ring_finishes() {
        let mut ring = ShockwaveRing::new(Vec2::new(0.5, 0.5), 1.0);
        for _ in 0..100 {
            ring.update(0.05);
        }
        assert!(ring.finished());
    }
}
