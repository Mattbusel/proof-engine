//! Force fields applied to particles: gravity well, vortex, turbulence, wind zone,
//! attractor/repulsor, drag, buoyancy. Supports force composition and tag masking.

use glam::{Vec3, Vec4};
use super::emitter::{Particle, ParticleTag, lcg_next};

// ─── Force field ID ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForceFieldId(pub u32);

// ─── Force field influence modes ──────────────────────────────────────────────

/// How a force field's strength falls off over distance.
#[derive(Debug, Clone, Copy)]
pub enum FalloffMode {
    /// Constant — no falloff, field applies uniformly within radius.
    Constant,
    /// Linear falloff from max at centre to zero at radius.
    Linear,
    /// Quadratic falloff (1 / r²-like).
    InverseSquare { min_dist: f32 },
    /// Smooth cubic: 1 - 3t² + 2t³.
    SmoothStep,
    /// Field only applies outside a minimum distance and inside max_radius.
    Annular { inner_radius: f32 },
}

impl FalloffMode {
    /// Returns a 0..1 multiplier given normalised t = dist / radius (0 = centre).
    pub fn factor(&self, t: f32, dist: f32, _radius: f32) -> f32 {
        match self {
            FalloffMode::Constant => 1.0,
            FalloffMode::Linear   => (1.0 - t).max(0.0),
            FalloffMode::InverseSquare { min_dist } => {
                let d = dist.max(*min_dist);
                1.0 / (d * d)
            }
            FalloffMode::SmoothStep => {
                let t = t.clamp(0.0, 1.0);
                1.0 - (3.0 * t * t - 2.0 * t * t * t)
            }
            FalloffMode::Annular { inner_radius } => {
                if dist < *inner_radius { 0.0 } else { (1.0 - t).max(0.0) }
            }
        }
    }
}

// ─── Tag mask ─────────────────────────────────────────────────────────────────

/// Determines which particles a force field affects.
#[derive(Debug, Clone, Copy)]
pub enum TagMask {
    /// Affect all particles regardless of tag.
    All,
    /// Only particles whose tag intersects this mask.
    Include(ParticleTag),
    /// All particles *except* those matching this mask.
    Exclude(ParticleTag),
}

impl TagMask {
    pub fn matches(&self, tag: ParticleTag) -> bool {
        match self {
            TagMask::All => true,
            TagMask::Include(m) => tag.contains(*m) || m.0 == 0,
            TagMask::Exclude(m) => !tag.contains(*m),
        }
    }
}

// ─── Individual force types ───────────────────────────────────────────────────

/// A gravitational well that pulls (or repels) particles toward a point.
#[derive(Debug, Clone)]
pub struct GravityWell {
    pub position:   Vec3,
    pub strength:   f32,
    pub radius:     f32,
    pub falloff:    FalloffMode,
    /// If true, field repels instead of attracts.
    pub repulsive:  bool,
    /// Kill particles that enter this exclusion sphere.
    pub absorb_radius: f32,
}

impl GravityWell {
    pub fn new(position: Vec3, strength: f32, radius: f32) -> Self {
        Self { position, strength, radius, falloff: FalloffMode::InverseSquare { min_dist: 0.1 }, repulsive: false, absorb_radius: 0.0 }
    }

    pub fn repulsor(position: Vec3, strength: f32, radius: f32) -> Self {
        Self { repulsive: true, ..Self::new(position, strength, radius) }
    }

    pub fn acceleration(&self, p_pos: Vec3) -> Vec3 {
        let delta = self.position - p_pos;
        let dist  = delta.length();
        if dist > self.radius || dist < 1e-6 { return Vec3::ZERO; }
        let dir    = delta / dist;
        let t      = dist / self.radius;
        let factor = self.falloff.factor(t, dist, self.radius);
        let sign   = if self.repulsive { -1.0 } else { 1.0 };
        dir * (self.strength * factor * sign)
    }

    pub fn should_absorb(&self, p_pos: Vec3) -> bool {
        self.absorb_radius > 0.0 && (self.position - p_pos).length() <= self.absorb_radius
    }
}

// ─── Vortex ───────────────────────────────────────────────────────────────────

/// A spinning vortex that imparts tangential force around an axis.
#[derive(Debug, Clone)]
pub struct VortexField {
    pub position:    Vec3,
    /// Normalised spin axis.
    pub axis:        Vec3,
    pub strength:    f32,
    pub radius:      f32,
    pub falloff:     FalloffMode,
    /// Additional inward/outward radial component (negative = inward).
    pub radial_pull: f32,
    /// Upward/downward component along axis.
    pub axial_pull:  f32,
}

impl VortexField {
    pub fn new(position: Vec3, axis: Vec3, strength: f32, radius: f32) -> Self {
        Self {
            position, axis: axis.normalize_or_zero(),
            strength, radius, falloff: FalloffMode::Linear,
            radial_pull: 0.0, axial_pull: 0.0,
        }
    }

    pub fn tornado(position: Vec3, strength: f32, radius: f32) -> Self {
        Self { radial_pull: -strength * 0.3, axial_pull: strength * 0.5, ..Self::new(position, Vec3::Y, strength, radius) }
    }

    pub fn acceleration(&self, p_pos: Vec3) -> Vec3 {
        let to_axis_origin = p_pos - self.position;
        // Project onto plane perpendicular to axis
        let along_axis  = self.axis * to_axis_origin.dot(self.axis);
        let radial_vec  = to_axis_origin - along_axis;
        let dist        = radial_vec.length();

        if dist > self.radius || dist < 1e-6 { return Vec3::ZERO; }

        let radial_dir  = radial_vec / dist;
        let tangent_dir = self.axis.cross(radial_dir).normalize_or_zero();
        let t           = dist / self.radius;
        let factor      = self.falloff.factor(t, dist, self.radius);

        let tangent  = tangent_dir * (self.strength * factor);
        let radial   = radial_dir  * (self.radial_pull * factor);
        let axial    = self.axis   * (self.axial_pull * factor);

        tangent + radial + axial
    }
}

// ─── Turbulence (Perlin-like) ─────────────────────────────────────────────────

/// Pseudo-random turbulence using a value-noise approach on a 3D grid.
#[derive(Debug, Clone)]
pub struct TurbulenceField {
    pub strength:      f32,
    pub frequency:     f32,    // spatial frequency of noise
    pub time_speed:    f32,    // how fast the noise evolves
    pub octaves:       u32,    // noise octave count
    pub lacunarity:    f32,    // frequency multiplier per octave
    pub persistence:   f32,    // amplitude multiplier per octave
    pub radius:        f32,    // 0 = infinite
    pub position:      Vec3,
    time:              f32,
}

impl TurbulenceField {
    pub fn new(strength: f32, frequency: f32) -> Self {
        Self {
            strength, frequency, time_speed: 0.5,
            octaves: 3, lacunarity: 2.0, persistence: 0.5,
            radius: 0.0, position: Vec3::ZERO, time: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) { self.time += dt * self.time_speed; }

    pub fn acceleration(&self, p_pos: Vec3) -> Vec3 {
        if self.radius > 0.0 && (p_pos - self.position).length() > self.radius {
            return Vec3::ZERO;
        }

        // Sample noise at 3 offset positions to get a 3D vector
        let t = self.time;
        let nx = self.fbm(p_pos + Vec3::new(0.0,   100.5, 300.2), t);
        let ny = self.fbm(p_pos + Vec3::new(100.1, 0.0,   200.3), t);
        let nz = self.fbm(p_pos + Vec3::new(200.8, 300.1, 0.0),   t);

        Vec3::new(nx, ny, nz) * self.strength
    }

    fn fbm(&self, pos: Vec3, time: f32) -> f32 {
        let mut value = 0.0_f32;
        let mut amplitude = 1.0_f32;
        let mut frequency = self.frequency;
        let mut max_value = 0.0_f32;

        for _ in 0..self.octaves {
            value     += self.value_noise_3d(pos * frequency, time) * amplitude;
            max_value += amplitude;
            amplitude *= self.persistence;
            frequency *= self.lacunarity;
        }

        if max_value > 0.0 { value / max_value } else { 0.0 }
    }

    fn value_noise_3d(&self, pos: Vec3, time: f32) -> f32 {
        // Integer cell
        let ix = pos.x.floor() as i32;
        let iy = pos.y.floor() as i32;
        let iz = pos.z.floor() as i32;
        let it = time.floor() as i32;

        // Fractional
        let fx = pos.x - ix as f32;
        let fy = pos.y - iy as f32;
        let fz = pos.z - iz as f32;

        // Smooth interpolation
        let ux = fx * fx * (3.0 - 2.0 * fx);
        let uy = fy * fy * (3.0 - 2.0 * fy);
        let uz = fz * fz * (3.0 - 2.0 * fz);

        // 8-corner hash
        let h = |x: i32, y: i32, z: i32, t: i32| -> f32 {
            let mut s = (x as u64).wrapping_mul(1619)
                .wrapping_add((y as u64).wrapping_mul(31337))
                .wrapping_add((z as u64).wrapping_mul(6971))
                .wrapping_add((t as u64).wrapping_mul(1013904223))
                ^ 0x5851F42D4C957F2D;
            s ^= s >> 33;
            s = s.wrapping_mul(0xFF51AFD7ED558CCD);
            s ^= s >> 33;
            (s as f32 / u64::MAX as f32) * 2.0 - 1.0
        };

        let v000 = h(ix,   iy,   iz,   it);
        let v100 = h(ix+1, iy,   iz,   it);
        let v010 = h(ix,   iy+1, iz,   it);
        let v110 = h(ix+1, iy+1, iz,   it);
        let v001 = h(ix,   iy,   iz+1, it);
        let v101 = h(ix+1, iy,   iz+1, it);
        let v011 = h(ix,   iy+1, iz+1, it);
        let v111 = h(ix+1, iy+1, iz+1, it);

        let lerp = |a: f32, b: f32, t: f32| a + t * (b - a);

        lerp(
            lerp(lerp(v000, v100, ux), lerp(v010, v110, ux), uy),
            lerp(lerp(v001, v101, ux), lerp(v011, v111, ux), uy),
            uz,
        )
    }
}

// ─── Wind zone ────────────────────────────────────────────────────────────────

/// A directional wind zone, optionally bounded to a box region.
#[derive(Debug, Clone)]
pub struct WindZone {
    pub direction:    Vec3,   // normalised
    pub speed:        f32,
    pub gust_strength: f32,  // additional random gust amplitude
    pub gust_frequency: f32, // Hz
    pub bounds_min:   Option<Vec3>,
    pub bounds_max:   Option<Vec3>,
    time:             f32,
    gust_phase:       f32,
}

impl WindZone {
    pub fn new(direction: Vec3, speed: f32) -> Self {
        Self {
            direction: direction.normalize_or_zero(), speed,
            gust_strength: speed * 0.3, gust_frequency: 0.5,
            bounds_min: None, bounds_max: None,
            time: 0.0, gust_phase: 0.0,
        }
    }

    pub fn global(direction: Vec3, speed: f32) -> Self { Self::new(direction, speed) }

    pub fn bounded(mut self, min: Vec3, max: Vec3) -> Self {
        self.bounds_min = Some(min);
        self.bounds_max = Some(max);
        self
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.gust_phase = self.time * self.gust_frequency * std::f32::consts::TAU;
    }

    pub fn acceleration(&self, p_pos: Vec3) -> Vec3 {
        if let (Some(bmin), Some(bmax)) = (self.bounds_min, self.bounds_max) {
            if p_pos.x < bmin.x || p_pos.x > bmax.x ||
               p_pos.y < bmin.y || p_pos.y > bmax.y ||
               p_pos.z < bmin.z || p_pos.z > bmax.z {
                return Vec3::ZERO;
            }
        }
        let gust = self.gust_phase.sin() * self.gust_strength;
        self.direction * (self.speed + gust)
    }
}

// ─── Attractor / Repulsor ─────────────────────────────────────────────────────

/// Simple point attractor or repulsor (alias over GravityWell with cleaner API).
#[derive(Debug, Clone)]
pub struct AttractorRepulsor {
    pub position:  Vec3,
    pub strength:  f32,
    pub radius:    f32,
    pub mode:      AttractorMode,
    pub falloff:   FalloffMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttractorMode {
    Attract,
    Repel,
    Orbit,
}

impl AttractorRepulsor {
    pub fn attractor(position: Vec3, strength: f32, radius: f32) -> Self {
        Self { position, strength, radius, mode: AttractorMode::Attract, falloff: FalloffMode::Linear }
    }

    pub fn repulsor(position: Vec3, strength: f32, radius: f32) -> Self {
        Self { position, strength, radius, mode: AttractorMode::Repel, falloff: FalloffMode::Linear }
    }

    pub fn orbit(position: Vec3, strength: f32, radius: f32) -> Self {
        Self { position, strength, radius, mode: AttractorMode::Orbit, falloff: FalloffMode::SmoothStep }
    }

    pub fn acceleration(&self, p_pos: Vec3) -> Vec3 {
        let delta = self.position - p_pos;
        let dist  = delta.length();
        if dist > self.radius || dist < 1e-6 { return Vec3::ZERO; }
        let dir    = delta / dist;
        let t      = dist / self.radius;
        let factor = self.falloff.factor(t, dist, self.radius);

        match self.mode {
            AttractorMode::Attract => dir * (self.strength * factor),
            AttractorMode::Repel   => -dir * (self.strength * factor),
            AttractorMode::Orbit   => {
                let up = Vec3::Y;
                let tangent = dir.cross(up).normalize_or_zero();
                tangent * (self.strength * factor)
            }
        }
    }
}

// ─── Drag ─────────────────────────────────────────────────────────────────────

/// Linear drag: decelerates particles proportional to their speed.
#[derive(Debug, Clone)]
pub struct DragField {
    pub coefficient:    f32,   // drag force = -velocity * coefficient
    pub quadratic:      bool,  // if true: force = -velocity * |velocity| * coefficient
    pub bounds_min:     Option<Vec3>,
    pub bounds_max:     Option<Vec3>,
}

impl DragField {
    pub fn new(coefficient: f32) -> Self {
        Self { coefficient, quadratic: false, bounds_min: None, bounds_max: None }
    }

    pub fn quadratic(coefficient: f32) -> Self {
        Self { quadratic: true, ..Self::new(coefficient) }
    }

    pub fn acceleration(&self, p_pos: Vec3, velocity: Vec3) -> Vec3 {
        if let (Some(bmin), Some(bmax)) = (self.bounds_min, self.bounds_max) {
            if p_pos.x < bmin.x || p_pos.x > bmax.x ||
               p_pos.y < bmin.y || p_pos.y > bmax.y ||
               p_pos.z < bmin.z || p_pos.z > bmax.z {
                return Vec3::ZERO;
            }
        }
        if self.quadratic {
            -velocity * velocity.length() * self.coefficient
        } else {
            -velocity * self.coefficient
        }
    }
}

// ─── Buoyancy ─────────────────────────────────────────────────────────────────

/// Buoyancy force: applies upward lift inversely proportional to particle density.
#[derive(Debug, Clone)]
pub struct BuoyancyField {
    /// Upward direction (usually +Y).
    pub up:             Vec3,
    /// Fluid density (e.g. air ~1.2 kg/m³, water ~1000).
    pub fluid_density:  f32,
    /// Gravity magnitude used for buoyancy calculation.
    pub gravity:        f32,
    /// Only apply below this height.
    pub surface_height: Option<f32>,
}

impl BuoyancyField {
    pub fn air(gravity: f32) -> Self {
        Self { up: Vec3::Y, fluid_density: 1.2, gravity, surface_height: None }
    }

    pub fn water(gravity: f32, surface_y: f32) -> Self {
        Self { up: Vec3::Y, fluid_density: 1000.0, gravity, surface_height: Some(surface_y) }
    }

    pub fn smoke_in_air() -> Self {
        // Smoke particles are lighter than air — they rise
        Self { up: Vec3::Y, fluid_density: 1.8, gravity: 9.81, surface_height: None }
    }

    /// Buoyant acceleration for a particle with given mass and volume (estimated from size).
    pub fn acceleration(&self, p_pos: Vec3, mass: f32, size: f32) -> Vec3 {
        if let Some(sy) = self.surface_height {
            if p_pos.y > sy { return Vec3::ZERO; }
        }
        // Volume ≈ sphere of radius size/2
        let radius = size * 0.5;
        let volume = (4.0 / 3.0) * std::f32::consts::PI * radius * radius * radius;
        let buoyant_force = self.fluid_density * volume * self.gravity;
        let weight        = mass * self.gravity;
        let net_force     = buoyant_force - weight;
        self.up * (net_force / mass.max(1e-6))
    }
}

// ─── Composed force field ──────────────────────────────────────────────────────

/// A single force field entry in the world, combining a field type with metadata.
#[derive(Debug, Clone)]
pub struct ForceField {
    pub id:       ForceFieldId,
    pub enabled:  bool,
    pub strength_scale: f32,
    pub tag_mask: TagMask,
    pub kind:     ForceFieldKind,
    pub priority: i32,
}

/// The concrete force implementation.
#[derive(Debug, Clone)]
pub enum ForceFieldKind {
    GravityWell(GravityWell),
    Vortex(VortexField),
    Turbulence(TurbulenceField),
    Wind(WindZone),
    Attractor(AttractorRepulsor),
    Drag(DragField),
    Buoyancy(BuoyancyField),
    /// Constant global gravity.
    Gravity { acceleration: Vec3 },
    /// A custom force defined by a coefficient table sampled over distance.
    Spline {
        position: Vec3,
        axis:     Vec3,
        radius:   f32,
        /// (normalised_distance, force_magnitude) keyframes.
        curve:    Vec<(f32, f32)>,
    },
}

impl ForceField {
    pub fn new(id: ForceFieldId, kind: ForceFieldKind) -> Self {
        Self { id, enabled: true, strength_scale: 1.0, tag_mask: TagMask::All, kind, priority: 0 }
    }

    pub fn with_tag_mask(mut self, mask: TagMask)   -> Self { self.tag_mask = mask; self }
    pub fn with_scale(mut self, s: f32)              -> Self { self.strength_scale = s; self }
    pub fn with_priority(mut self, p: i32)           -> Self { self.priority = p; self }
    pub fn disabled(mut self) -> Self { self.enabled = false; self }

    /// Compute the acceleration this field imparts on a particle (per unit mass, i.e. force/mass).
    pub fn apply(&self, particle: &Particle) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        if !self.tag_mask.matches(particle.tag) { return Vec3::ZERO; }

        let raw = match &self.kind {
            ForceFieldKind::GravityWell(gw)  => gw.acceleration(particle.position),
            ForceFieldKind::Vortex(vx)       => vx.acceleration(particle.position),
            ForceFieldKind::Turbulence(tb)   => tb.acceleration(particle.position),
            ForceFieldKind::Wind(wz)         => wz.acceleration(particle.position),
            ForceFieldKind::Attractor(at)    => at.acceleration(particle.position),
            ForceFieldKind::Drag(dr)         => dr.acceleration(particle.position, particle.velocity),
            ForceFieldKind::Buoyancy(by)     => by.acceleration(particle.position, particle.mass, particle.size),
            ForceFieldKind::Gravity { acceleration } => *acceleration,
            ForceFieldKind::Spline { position, axis, radius, curve } => {
                let delta = particle.position - *position;
                let dist  = delta.length();
                if dist > *radius || dist < 1e-6 || curve.is_empty() {
                    Vec3::ZERO
                } else {
                    let t = dist / radius;
                    let mag = sample_curve(curve, t);
                    let dir = (*axis).normalize_or_zero();
                    dir * mag
                }
            }
        };

        raw * self.strength_scale
    }

    pub fn tick(&mut self, dt: f32) {
        match &mut self.kind {
            ForceFieldKind::Turbulence(tb) => tb.tick(dt),
            ForceFieldKind::Wind(wz)       => wz.tick(dt),
            _ => {}
        }
    }

    pub fn marks_for_death(&self, particle: &Particle) -> bool {
        if let ForceFieldKind::GravityWell(gw) = &self.kind {
            return gw.should_absorb(particle.position);
        }
        false
    }
}

fn sample_curve(curve: &[(f32, f32)], t: f32) -> f32 {
    if curve.len() == 1 { return curve[0].1; }
    let i = curve.partition_point(|(ct, _)| *ct <= t);
    if i == 0 { return curve[0].1; }
    if i >= curve.len() { return curve.last().unwrap().1; }
    let (t0, v0) = curve[i - 1];
    let (t1, v1) = curve[i];
    let f = (t - t0) / (t1 - t0).max(1e-6);
    v0 + f * (v1 - v0)
}

// ─── Force composition ────────────────────────────────────────────────────────

/// Blend mode when combining multiple force fields at the same location.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForceBlendMode {
    /// Sum all forces (default physical behaviour).
    Additive,
    /// Use the strongest single force.
    Override,
    /// Multiply forces together (chaining effect).
    Multiply,
    /// Average all forces.
    Average,
}

/// A group of force fields evaluated together with a blend mode.
#[derive(Debug, Clone)]
pub struct ForceComposite {
    pub fields:     Vec<ForceField>,
    pub blend_mode: ForceBlendMode,
    pub global_scale: f32,
}

impl ForceComposite {
    pub fn new() -> Self {
        Self { fields: Vec::new(), blend_mode: ForceBlendMode::Additive, global_scale: 1.0 }
    }

    pub fn with_blend(mut self, mode: ForceBlendMode) -> Self { self.blend_mode = mode; self }

    pub fn add(&mut self, field: ForceField) { self.fields.push(field); }
    pub fn remove(&mut self, id: ForceFieldId) { self.fields.retain(|f| f.id != id); }
    pub fn get_mut(&mut self, id: ForceFieldId) -> Option<&mut ForceField> {
        self.fields.iter_mut().find(|f| f.id == id)
    }

    pub fn tick(&mut self, dt: f32) {
        for f in &mut self.fields { f.tick(dt); }
    }

    /// Compute the net acceleration for a particle from all fields.
    pub fn net_acceleration(&self, particle: &Particle) -> Vec3 {
        let enabled: Vec<&ForceField> = self.fields.iter().filter(|f| f.enabled).collect();
        if enabled.is_empty() { return Vec3::ZERO; }

        let result = match self.blend_mode {
            ForceBlendMode::Additive => {
                enabled.iter().map(|f| f.apply(particle)).fold(Vec3::ZERO, |a, b| a + b)
            }
            ForceBlendMode::Override => {
                enabled.iter()
                    .map(|f| f.apply(particle))
                    .max_by(|a, b| a.length_squared().partial_cmp(&b.length_squared()).unwrap())
                    .unwrap_or(Vec3::ZERO)
            }
            ForceBlendMode::Multiply => {
                enabled.iter().map(|f| f.apply(particle))
                    .fold(Vec3::ONE, |a, b| Vec3::new(a.x * b.x, a.y * b.y, a.z * b.z))
            }
            ForceBlendMode::Average => {
                let sum = enabled.iter().map(|f| f.apply(particle)).fold(Vec3::ZERO, |a, b| a + b);
                sum / enabled.len() as f32
            }
        };

        result * self.global_scale
    }

    /// Apply all forces to a particle, accumulating into its acceleration.
    pub fn apply_to_particle(&self, particle: &mut Particle) {
        particle.acceleration += self.net_acceleration(particle);
    }

    /// Apply forces to a whole slice of particles and remove absorbed ones.
    pub fn apply_and_cull(&self, particles: &mut Vec<Particle>) {
        for p in particles.iter_mut() {
            p.acceleration += self.net_acceleration(p);
        }
        // Remove particles absorbed by a gravity well
        particles.retain(|p| {
            !self.fields.iter().any(|f| f.marks_for_death(p))
        });
    }
}

impl Default for ForceComposite {
    fn default() -> Self { Self::new() }
}

// ─── Force field world manager ────────────────────────────────────────────────

/// Global registry for all force fields active in the world.
pub struct ForceFieldWorld {
    pub composite: ForceComposite,
    next_id:       u32,
    /// Gravity constant applied to all particles unless overridden.
    pub gravity:   Vec3,
    pub gravity_enabled: bool,
}

impl ForceFieldWorld {
    pub fn new() -> Self {
        Self {
            composite: ForceComposite::new(),
            next_id:   1,
            gravity:   Vec3::new(0.0, -9.81, 0.0),
            gravity_enabled: true,
        }
    }

    fn alloc_id(&mut self) -> ForceFieldId {
        let id = ForceFieldId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_gravity_well(&mut self, pos: Vec3, strength: f32, radius: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::GravityWell(GravityWell::new(pos, strength, radius))));
        id
    }

    pub fn add_vortex(&mut self, pos: Vec3, axis: Vec3, strength: f32, radius: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Vortex(VortexField::new(pos, axis, strength, radius))));
        id
    }

    pub fn add_turbulence(&mut self, strength: f32, frequency: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Turbulence(TurbulenceField::new(strength, frequency))));
        id
    }

    pub fn add_wind(&mut self, direction: Vec3, speed: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Wind(WindZone::new(direction, speed))));
        id
    }

    pub fn add_attractor(&mut self, pos: Vec3, strength: f32, radius: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Attractor(AttractorRepulsor::attractor(pos, strength, radius))));
        id
    }

    pub fn add_repulsor(&mut self, pos: Vec3, strength: f32, radius: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Attractor(AttractorRepulsor::repulsor(pos, strength, radius))));
        id
    }

    pub fn add_drag(&mut self, coefficient: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Drag(DragField::new(coefficient))));
        id
    }

    pub fn add_buoyancy(&mut self, gravity: f32) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, ForceFieldKind::Buoyancy(BuoyancyField::air(gravity))));
        id
    }

    pub fn add_field(&mut self, kind: ForceFieldKind) -> ForceFieldId {
        let id = self.alloc_id();
        self.composite.add(ForceField::new(id, kind));
        id
    }

    pub fn remove_field(&mut self, id: ForceFieldId) {
        self.composite.remove(id);
    }

    pub fn get_mut(&mut self, id: ForceFieldId) -> Option<&mut ForceField> {
        self.composite.get_mut(id)
    }

    pub fn tick(&mut self, dt: f32) {
        self.composite.tick(dt);
    }

    pub fn apply_to_particles(&self, particles: &mut Vec<Particle>) {
        // Apply global gravity first
        if self.gravity_enabled {
            for p in particles.iter_mut() {
                p.acceleration += self.gravity;
            }
        }
        self.composite.apply_and_cull(particles);
    }

    pub fn field_count(&self) -> usize { self.composite.fields.len() }
}

impl Default for ForceFieldWorld {
    fn default() -> Self { Self::new() }
}

// ─── Preset helpers ───────────────────────────────────────────────────────────

/// Convenience constructors for common force-field scenarios.
pub struct ForcePresets;

impl ForcePresets {
    /// Standard Earth gravity downward.
    pub fn earth_gravity() -> ForceFieldKind {
        ForceFieldKind::Gravity { acceleration: Vec3::new(0.0, -9.81, 0.0) }
    }

    /// Low gravity (moon-like).
    pub fn moon_gravity() -> ForceFieldKind {
        ForceFieldKind::Gravity { acceleration: Vec3::new(0.0, -1.62, 0.0) }
    }

    /// Anti-gravity / upward lift field.
    pub fn anti_gravity(strength: f32) -> ForceFieldKind {
        ForceFieldKind::Gravity { acceleration: Vec3::new(0.0, strength, 0.0) }
    }

    /// Explosion radial blast: repulsor at centre.
    pub fn explosion_blast(center: Vec3, strength: f32, radius: f32) -> ForceFieldKind {
        ForceFieldKind::Attractor(AttractorRepulsor::repulsor(center, strength, radius))
    }

    /// Black-hole pull.
    pub fn black_hole(center: Vec3, strength: f32, event_horizon: f32) -> ForceFieldKind {
        ForceFieldKind::GravityWell(GravityWell {
            position: center, strength, radius: strength * 5.0,
            falloff: FalloffMode::InverseSquare { min_dist: 0.01 },
            repulsive: false,
            absorb_radius: event_horizon,
        })
    }

    /// Gentle ambient turbulence (e.g. heat shimmer).
    pub fn heat_shimmer() -> ForceFieldKind {
        ForceFieldKind::Turbulence(TurbulenceField {
            strength: 0.8, frequency: 0.5, time_speed: 0.3,
            octaves: 2, lacunarity: 2.0, persistence: 0.4,
            radius: 0.0, position: Vec3::ZERO, time: 0.0,
        })
    }

    /// Outdoor wind with gusts.
    pub fn outdoor_wind(direction: Vec3, base_speed: f32) -> ForceFieldKind {
        ForceFieldKind::Wind(WindZone {
            direction: direction.normalize_or_zero(), speed: base_speed,
            gust_strength: base_speed * 0.4, gust_frequency: 0.3,
            bounds_min: None, bounds_max: None,
            time: 0.0, gust_phase: 0.0,
        })
    }

    /// Air resistance for fast-moving projectiles.
    pub fn air_resistance() -> ForceFieldKind {
        ForceFieldKind::Drag(DragField::quadratic(0.05))
    }

    /// Tornado centred at a point.
    pub fn tornado(center: Vec3, strength: f32, radius: f32) -> ForceFieldKind {
        ForceFieldKind::Vortex(VortexField::tornado(center, strength, radius))
    }

    /// Water current pushing in a direction within a volume.
    pub fn water_current(direction: Vec3, speed: f32, bounds_min: Vec3, bounds_max: Vec3) -> ForceFieldKind {
        ForceFieldKind::Wind(WindZone {
            direction: direction.normalize_or_zero(), speed,
            gust_strength: 0.0, gust_frequency: 0.0,
            bounds_min: Some(bounds_min), bounds_max: Some(bounds_max),
            time: 0.0, gust_phase: 0.0,
        })
    }

    /// Smoke buoyancy (smoke rises, lighter than air).
    pub fn smoke_buoyancy() -> ForceFieldKind {
        ForceFieldKind::Buoyancy(BuoyancyField::smoke_in_air())
    }
}

// ─── Force debug info ─────────────────────────────────────────────────────────

/// Debug information about forces acting on a particle at a given point.
#[derive(Debug, Clone)]
pub struct ForceDebugSample {
    pub position:     Vec3,
    pub total_force:  Vec3,
    pub per_field:    Vec<(ForceFieldId, Vec3)>,
}

impl ForceFieldWorld {
    /// Sample the force at a given position with a synthetic particle.
    pub fn debug_sample(&self, pos: Vec3) -> ForceDebugSample {
        let test = Particle {
            id: 0, position: pos, velocity: Vec3::ZERO, acceleration: Vec3::ZERO,
            color: Vec4::ONE, size: 0.1, rotation: 0.0, angular_vel: 0.0,
            age: 0.0, lifetime: 1.0, mass: 1.0,
            tag: ParticleTag::ALL,
            emitter_id: 0, custom: [0.0; 4],
        };

        let mut per_field = Vec::new();
        let mut total = Vec3::ZERO;

        if self.gravity_enabled {
            total += self.gravity;
        }

        for f in &self.composite.fields {
            let acc = f.apply(&test);
            per_field.push((f.id, acc));
            total += acc;
        }

        ForceDebugSample { position: pos, total_force: total, per_field }
    }
}
