//! Spawn system — wave-based enemy spawning with zones, patterns, and blueprints.
//!
//! `WaveManager` drives timed waves of entity spawns defined by `SpawnWave`.
//! Each wave contains one or more `SpawnGroup`s that describe how many entities
//! to spawn, in what pattern, from what zone, and at what rate.

use glam::Vec3;
use std::collections::HashMap;

// ── SpawnZone ─────────────────────────────────────────────────────────────────

/// Defines where entities can spawn.
#[derive(Clone, Debug)]
pub enum SpawnZone {
    /// A single fixed point.
    Point(Vec3),
    /// Uniform random within an AABB.
    Box { min: Vec3, max: Vec3 },
    /// Uniform random within a sphere.
    Sphere { center: Vec3, radius: f32 },
    /// Random on the surface of a sphere.
    SphereSurface { center: Vec3, radius: f32 },
    /// Uniform random within a disc (flat in XZ).
    Disc { center: Vec3, inner_radius: f32, outer_radius: f32 },
    /// Along a line segment.
    Line { start: Vec3, end: Vec3 },
    /// On a ring of equally-spaced points (deterministic).
    Ring { center: Vec3, radius: f32, count: usize, phase: f32 },
    /// Relative to the player position.
    AroundPlayer { offset_min: f32, offset_max: f32 },
}

impl SpawnZone {
    /// Generate a spawn position using a pseudo-random seed.
    pub fn sample(&self, rng: &mut u64, player_pos: Vec3) -> Vec3 {
        match self {
            SpawnZone::Point(p) => *p,

            SpawnZone::Box { min, max } => {
                Vec3::new(
                    min.x + rng_f32(rng) * (max.x - min.x),
                    min.y + rng_f32(rng) * (max.y - min.y),
                    min.z + rng_f32(rng) * (max.z - min.z),
                )
            }

            SpawnZone::Sphere { center, radius } => {
                let (center, radius) = (*center, *radius);
                
                // Rejection sampling for uniform interior
                loop {
                    let x = rng_f32_signed(rng);
                    let y = rng_f32_signed(rng);
                    let z = rng_f32_signed(rng);
                    if x*x + y*y + z*z <= 1.0 {
                        return center + Vec3::new(x, y, z) * radius;
                    }
                }
            }

            SpawnZone::SphereSurface { center, radius } => {
                let (center, radius) = (*center, *radius);
                
                let theta = rng_f32(rng) * std::f32::consts::TAU;
                let phi   = (rng_f32_signed(rng)).acos();
                center + Vec3::new(
                    phi.sin() * theta.cos(),
                    phi.sin() * theta.sin(),
                    phi.cos(),
                ) * radius
            }

            SpawnZone::Disc { center, inner_radius, outer_radius } => {
                let (center, inner_radius, outer_radius) = (*center, *inner_radius, *outer_radius);
                let angle  = rng_f32(rng) * std::f32::consts::TAU;
                let r      = (inner_radius + rng_f32(rng) * (outer_radius - inner_radius)).sqrt();
                center + Vec3::new(r * angle.cos(), 0.0, r * angle.sin())
            }

            SpawnZone::Line { start, end } => {
                let (start, end) = (*start, *end);
                start.lerp(end, rng_f32(rng))
            }

            SpawnZone::Ring { center, radius, count, phase } => {
                let (center, radius, count, phase) = (*center, *radius, *count, *phase);
                let idx   = (rng_f32(rng) * count as f32) as usize % count;
                let angle = phase + std::f32::consts::TAU * idx as f32 / count as f32;
                center + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
            }

            SpawnZone::AroundPlayer { offset_min, offset_max } => {
                let angle  = rng_f32(rng) * std::f32::consts::TAU;
                let radius = offset_min + rng_f32(rng) * (offset_max - offset_min);
                player_pos + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
            }
        }
    }
}

fn rng_f32(rng: &mut u64) -> f32 {
    *rng ^= *rng << 13; *rng ^= *rng >> 7; *rng ^= *rng << 17;
    (*rng & 0xFFFF) as f32 / 65535.0
}

fn rng_f32_signed(rng: &mut u64) -> f32 {
    rng_f32(rng) * 2.0 - 1.0
}

// ── SpawnPattern ─────────────────────────────────────────────────────────────

/// How multiple entities in a group are arranged.
#[derive(Clone, Debug)]
pub enum SpawnPattern {
    /// All at random positions within the zone.
    Random,
    /// Equally spaced on a ring.
    Ring { radius: f32, phase_offset: f32 },
    /// Grid pattern in XZ.
    Grid { cols: u32, spacing: Vec3 },
    /// V-formation (like birds).
    VFormation { spread: f32, depth: f32 },
    /// Single-file line.
    Line { direction: Vec3, spacing: f32 },
    /// Random within a burst radius from zone center.
    Burst { radius: f32 },
    /// Each entity in a formation around a leader.
    Escort { leader_offset: Vec3, follower_offsets: Vec<Vec3> },
}

impl SpawnPattern {
    /// Generate positions for `count` entities using this pattern.
    pub fn positions(&self, count: usize, zone_center: Vec3, rng: &mut u64) -> Vec<Vec3> {
        match self {
            SpawnPattern::Random => {
                (0..count).map(|_| {
                    zone_center + Vec3::new(
                        rng_f32_signed(rng),
                        0.0,
                        rng_f32_signed(rng),
                    )
                }).collect()
            }

            SpawnPattern::Ring { radius, phase_offset } => {
                (0..count).map(|i| {
                    let angle = phase_offset + std::f32::consts::TAU * i as f32 / count as f32;
                    zone_center + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
                }).collect()
            }

            SpawnPattern::Grid { cols, spacing } => {
                let cols = (*cols).max(1) as usize;
                (0..count).map(|i| {
                    let col = i % cols;
                    let row = i / cols;
                    zone_center + Vec3::new(col as f32, 0.0, row as f32) * *spacing
                }).collect()
            }

            SpawnPattern::VFormation { spread, depth } => {
                (0..count).map(|i| {
                    let offset_x = (i as f32 - count as f32 * 0.5) * spread;
                    let offset_z = (i as f32 * 0.5).abs() * depth;
                    zone_center + Vec3::new(offset_x, 0.0, offset_z)
                }).collect()
            }

            SpawnPattern::Line { direction, spacing } => {
                let dir = direction.normalize_or_zero();
                (0..count).map(|i| {
                    zone_center + dir * (i as f32 * spacing)
                }).collect()
            }

            SpawnPattern::Burst { radius } => {
                (0..count).map(|_| {
                    let angle  = rng_f32(rng) * std::f32::consts::TAU;
                    let r      = rng_f32(rng).sqrt() * radius;
                    zone_center + Vec3::new(angle.cos() * r, 0.0, angle.sin() * r)
                }).collect()
            }

            SpawnPattern::Escort { leader_offset, follower_offsets } => {
                let mut positions = vec![zone_center + *leader_offset];
                for (i, off) in follower_offsets.iter().enumerate() {
                    if i + 1 >= count { break; }
                    positions.push(zone_center + *off);
                }
                while positions.len() < count {
                    positions.push(zone_center);
                }
                positions
            }
        }
    }
}

// ── EntityBlueprint ───────────────────────────────────────────────────────────

/// Defines a type of entity to spawn.
#[derive(Clone, Debug)]
pub struct EntityBlueprint {
    pub name:     String,
    pub tags:     Vec<String>,
    pub hp:       f32,
    pub speed:    f32,
    pub damage:   f32,
    pub scale:    Vec3,
    pub color:    [f32; 4],
    /// AI behavior tree name.
    pub ai:       Option<String>,
    /// Custom attributes.
    pub attrs:    HashMap<String, f32>,
    /// Character glyphs that make up this entity.
    pub glyphs:   Vec<char>,
}

impl EntityBlueprint {
    pub fn new(name: &str) -> Self {
        Self {
            name:   name.into(),
            tags:   Vec::new(),
            hp:     100.0,
            speed:  3.0,
            damage: 10.0,
            scale:  Vec3::ONE,
            color:  [1.0, 1.0, 1.0, 1.0],
            ai:     None,
            attrs:  HashMap::new(),
            glyphs: vec!['@'],
        }
    }

    pub fn with_hp(mut self, hp: f32) -> Self { self.hp = hp; self }
    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }
    pub fn with_damage(mut self, d: f32) -> Self { self.damage = d; self }
    pub fn with_color(mut self, c: [f32; 4]) -> Self { self.color = c; self }
    pub fn with_ai(mut self, ai: &str) -> Self { self.ai = Some(ai.into()); self }
    pub fn with_glyph(mut self, g: char) -> Self { self.glyphs = vec![g]; self }
    pub fn with_glyphs(mut self, g: Vec<char>) -> Self { self.glyphs = g; self }
    pub fn tagged(mut self, tag: &str) -> Self { self.tags.push(tag.into()); self }
    pub fn with_attr(mut self, k: &str, v: f32) -> Self { self.attrs.insert(k.into(), v); self }
}

// ── SpawnGroup ────────────────────────────────────────────────────────────────

/// One group within a wave: blueprint × count at a zone.
#[derive(Clone, Debug)]
pub struct SpawnGroup {
    pub blueprint:      String,         // name in BlueprintLibrary
    pub count:          u32,
    pub zone:           SpawnZone,
    pub pattern:        SpawnPattern,
    /// Spawns per second (0 = all at once).
    pub rate:           f32,
    /// Delay from wave start before this group activates.
    pub delay:          f32,
    /// Tag applied to all spawned entities.
    pub tag:            Option<String>,
    /// If true, the wave doesn't complete until all these entities die.
    pub blocking:       bool,
}

impl SpawnGroup {
    pub fn new(blueprint: &str, count: u32, zone: SpawnZone) -> Self {
        Self {
            blueprint: blueprint.into(),
            count,
            zone,
            pattern:   SpawnPattern::Random,
            rate:       0.0,
            delay:      0.0,
            tag:        None,
            blocking:   true,
        }
    }

    pub fn with_pattern(mut self, p: SpawnPattern) -> Self { self.pattern = p; self }
    pub fn with_rate(mut self, r: f32) -> Self { self.rate = r; self }
    pub fn with_delay(mut self, d: f32) -> Self { self.delay = d; self }
    pub fn tagged(mut self, t: &str) -> Self { self.tag = Some(t.into()); self }
    pub fn non_blocking(mut self) -> Self { self.blocking = false; self }
}

// ── SpawnWave ─────────────────────────────────────────────────────────────────

/// A complete wave: one or more groups, activation conditions, completion rewards.
#[derive(Clone, Debug)]
pub struct SpawnWave {
    pub name:       String,
    pub groups:     Vec<SpawnGroup>,
    /// Delay after the previous wave before this one starts.
    pub pre_delay:  f32,
    /// Delay after this wave completes before the next.
    pub post_delay: f32,
    /// Music vibe to set when this wave starts.
    pub music_vibe: Option<String>,
    /// Flag to set when the wave is cleared.
    pub on_clear:   Option<String>,
    /// Repeat this wave indefinitely.
    pub repeat:     bool,
}

impl SpawnWave {
    pub fn new(name: &str, groups: Vec<SpawnGroup>) -> Self {
        Self {
            name:       name.into(),
            groups,
            pre_delay:  0.0,
            post_delay: 2.0,
            music_vibe: None,
            on_clear:   None,
            repeat:     false,
        }
    }

    pub fn with_pre_delay(mut self, d: f32) -> Self { self.pre_delay = d; self }
    pub fn with_post_delay(mut self, d: f32) -> Self { self.post_delay = d; self }
    pub fn with_music(mut self, v: &str) -> Self { self.music_vibe = Some(v.into()); self }
    pub fn on_clear(mut self, flag: &str) -> Self { self.on_clear = Some(flag.into()); self }
    pub fn repeating(mut self) -> Self { self.repeat = true; self }
}

// ── Group runtime state ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct GroupState {
    pub spawned:    u32,
    pub killed:     u32,
    pub timer:      f32,    // rate timer
    pub delay_done: bool,
    pub delay_timer: f32,
    pub complete:   bool,
}

// ── SpawnEvent ────────────────────────────────────────────────────────────────

/// Emitted by the spawn system for the game to process.
#[derive(Clone, Debug)]
pub struct SpawnEvent {
    pub blueprint: String,
    pub position:  Vec3,
    pub tag:       Option<String>,
    pub wave_name: String,
}

// ── WaveManager ───────────────────────────────────────────────────────────────

/// Manages a sequence of spawn waves.
pub struct WaveManager {
    waves:        Vec<SpawnWave>,
    current_wave: usize,
    group_states: Vec<GroupState>,
    /// Seconds until wave starts (pre-delay).
    wave_timer:   f32,
    /// Whether the current wave is active.
    active:       bool,
    /// Post-delay timer after wave clear.
    post_timer:   f32,
    post_pending: bool,
    rng:          u64,
    pub flags:    HashMap<String, bool>,
    pub player_pos: Vec3,
    /// Library of blueprints.
    pub blueprints: BlueprintLibrary,
    pub finished:   bool,
    pub wave_count:  u32,
}

impl WaveManager {
    pub fn new(waves: Vec<SpawnWave>, blueprints: BlueprintLibrary) -> Self {
        let n = waves.first().map(|w| w.groups.len()).unwrap_or(0);
        let pre = waves.first().map(|w| w.pre_delay).unwrap_or(0.0);
        let group_states = vec![GroupState {
            spawned: 0, killed: 0, timer: 0.0,
            delay_done: false, delay_timer: 0.0, complete: false,
        }; n];

        Self {
            waves,
            current_wave: 0,
            group_states,
            wave_timer:   pre,
            active:       false,
            post_timer:   0.0,
            post_pending: false,
            rng:          0xDEADBEEF_CAFEBABE,
            flags:        HashMap::new(),
            player_pos:   Vec3::ZERO,
            blueprints,
            finished:     false,
            wave_count:   0,
        }
    }

    pub fn start(&mut self) {
        if self.waves.is_empty() {
            self.finished = true;
            return;
        }
        self.active = false;
        self.wave_timer = self.waves[0].pre_delay;
    }

    /// Notify the manager that an entity with `tag` was killed.
    pub fn on_entity_killed(&mut self, tag: &str) {
        let wave = match self.waves.get(self.current_wave) {
            Some(w) => w,
            None    => return,
        };
        for (i, group) in wave.groups.iter().enumerate() {
            if group.tag.as_deref() == Some(tag) || group.blocking {
                if let Some(s) = self.group_states.get_mut(i) {
                    s.killed += 1;
                }
            }
        }
    }

    /// Advance the spawn system by dt. Returns spawned entities this tick.
    pub fn tick(&mut self, dt: f32) -> Vec<SpawnEvent> {
        if self.finished { return Vec::new(); }
        let mut events = Vec::new();

        // Pre-delay
        if !self.active && !self.post_pending {
            self.wave_timer -= dt;
            if self.wave_timer <= 0.0 {
                self.activate_current_wave();
            }
            return events;
        }

        // Post-delay
        if self.post_pending {
            self.post_timer -= dt;
            if self.post_timer <= 0.0 {
                self.post_pending = false;
                self.advance_wave();
            }
            return events;
        }

        // Active wave
        let wave = match self.waves.get(self.current_wave).cloned() {
            Some(w) => w,
            None    => return events,
        };

        let mut all_done = true;

        for (gi, group) in wave.groups.iter().enumerate() {
            let state = &mut self.group_states[gi];
            if state.complete { continue; }

            // Per-group delay
            if !state.delay_done {
                state.delay_timer += dt;
                if state.delay_timer < group.delay { all_done = false; continue; }
                state.delay_done = true;
            }

            // Spawn rate
            let remaining = group.count - state.spawned;
            if remaining > 0 {
                all_done = false;
                if group.rate <= 0.0 {
                    // Spawn all at once
                    let positions = group.pattern.positions(
                        remaining as usize,
                        group.zone.sample(&mut self.rng, self.player_pos),
                        &mut self.rng,
                    );
                    for pos in positions {
                        events.push(SpawnEvent {
                            blueprint: group.blueprint.clone(),
                            position:  pos,
                            tag:       group.tag.clone(),
                            wave_name: wave.name.clone(),
                        });
                        state.spawned += 1;
                    }
                } else {
                    state.timer += dt;
                    while state.timer >= 1.0 / group.rate && state.spawned < group.count {
                        state.timer -= 1.0 / group.rate;
                        let pos = group.zone.sample(&mut self.rng, self.player_pos);
                        events.push(SpawnEvent {
                            blueprint: group.blueprint.clone(),
                            position:  pos,
                            tag:       group.tag.clone(),
                            wave_name: wave.name.clone(),
                        });
                        state.spawned += 1;
                    }
                }
            } else if group.blocking {
                // Wait for kills
                let needed = group.count;
                if state.killed < needed {
                    all_done = false;
                } else {
                    state.complete = true;
                }
            } else {
                state.complete = true;
            }
        }

        if all_done && self.active {
            self.on_wave_cleared(&wave.clone());
        }

        events
    }

    fn activate_current_wave(&mut self) {
        let wave = match self.waves.get(self.current_wave) {
            Some(w) => w,
            None    => { self.finished = true; return; }
        };
        let n = wave.groups.len();
        self.group_states = vec![GroupState {
            spawned: 0, killed: 0, timer: 0.0,
            delay_done: false, delay_timer: 0.0, complete: false,
        }; n];
        self.active = true;
    }

    fn on_wave_cleared(&mut self, wave: &SpawnWave) {
        self.active = false;
        self.wave_count += 1;

        if let Some(flag) = &wave.on_clear {
            self.flags.insert(flag.clone(), true);
        }

        if wave.repeat {
            self.wave_timer  = wave.pre_delay;
            self.post_pending = true;
            self.post_timer  = wave.post_delay;
        } else {
            self.post_pending = true;
            self.post_timer  = wave.post_delay;
        }
    }

    fn advance_wave(&mut self) {
        if self.waves.get(self.current_wave).map(|w| w.repeat).unwrap_or(false) {
            // Stay on same wave
            self.wave_timer = self.waves[self.current_wave].pre_delay;
        } else {
            self.current_wave += 1;
            if self.current_wave >= self.waves.len() {
                self.finished = true;
                return;
            }
            self.wave_timer = self.waves[self.current_wave].pre_delay;
        }
    }

    pub fn current_wave_name(&self) -> &str {
        self.waves.get(self.current_wave).map(|w| w.name.as_str()).unwrap_or("none")
    }

    pub fn total_waves(&self) -> usize { self.waves.len() }
    pub fn is_active(&self) -> bool { self.active }
    pub fn get_flag(&self, k: &str) -> bool { self.flags.get(k).copied().unwrap_or(false) }
}

// ── BlueprintLibrary ──────────────────────────────────────────────────────────

/// Registry of named entity blueprints.
#[derive(Default)]
pub struct BlueprintLibrary {
    pub blueprints: HashMap<String, EntityBlueprint>,
}

impl BlueprintLibrary {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, blueprint: EntityBlueprint) {
        self.blueprints.insert(blueprint.name.clone(), blueprint);
    }

    pub fn get(&self, name: &str) -> Option<&EntityBlueprint> {
        self.blueprints.get(name)
    }

    /// Register standard enemy blueprints.
    pub fn with_defaults(mut self) -> Self {
        self.register(EntityBlueprint::new("grunt")
            .with_hp(60.0).with_speed(2.5).with_damage(8.0)
            .with_color([0.8, 0.2, 0.2, 1.0]).with_glyph('g').tagged("enemy"));
        self.register(EntityBlueprint::new("archer")
            .with_hp(40.0).with_speed(2.0).with_damage(15.0)
            .with_color([0.8, 0.5, 0.2, 1.0]).with_glyph('a').tagged("enemy"));
        self.register(EntityBlueprint::new("tank")
            .with_hp(200.0).with_speed(1.5).with_damage(25.0)
            .with_color([0.5, 0.2, 0.8, 1.0]).with_glyph('T').tagged("enemy"));
        self.register(EntityBlueprint::new("healer")
            .with_hp(50.0).with_speed(2.0).with_damage(5.0)
            .with_color([0.2, 0.9, 0.4, 1.0]).with_glyph('h').tagged("enemy"));
        self.register(EntityBlueprint::new("boss")
            .with_hp(1000.0).with_speed(3.5).with_damage(50.0)
            .with_color([1.0, 0.1, 0.1, 1.0]).with_glyph('B').tagged("enemy").tagged("boss")
            .with_attr("enrage_threshold", 0.3));
        self
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_zone_point() {
        let z   = SpawnZone::Point(Vec3::new(1.0, 2.0, 3.0));
        let mut rng = 12345u64;
        let p   = z.sample(&mut rng, Vec3::ZERO);
        assert_eq!(p, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn spawn_zone_sphere_bounded() {
        let z   = SpawnZone::Sphere { center: Vec3::ZERO, radius: 5.0 };
        let mut rng = 42u64;
        for _ in 0..100 {
            let p = z.sample(&mut rng, Vec3::ZERO);
            assert!(p.length() <= 5.05, "Point outside sphere: {:?}", p);
        }
    }

    #[test]
    fn spawn_pattern_ring_count() {
        let p = SpawnPattern::Ring { radius: 3.0, phase_offset: 0.0 };
        let positions = p.positions(8, Vec3::ZERO, &mut 0u64);
        assert_eq!(positions.len(), 8);
    }

    #[test]
    fn spawn_pattern_grid() {
        let p = SpawnPattern::Grid { cols: 3, spacing: Vec3::ONE };
        let positions = p.positions(9, Vec3::ZERO, &mut 0u64);
        assert_eq!(positions.len(), 9);
    }

    #[test]
    fn blueprint_library_default() {
        let lib = BlueprintLibrary::new().with_defaults();
        assert!(lib.get("grunt").is_some());
        assert!(lib.get("boss").is_some());
        assert!(lib.get("nobody").is_none());
    }

    #[test]
    fn wave_manager_starts_and_spawns() {
        let lib = BlueprintLibrary::new().with_defaults();
        let wave = SpawnWave::new("w1", vec![
            SpawnGroup::new("grunt", 3, SpawnZone::Point(Vec3::ZERO))
                .with_rate(0.0)   // all at once
                .non_blocking(),
        ]).with_pre_delay(0.0).with_post_delay(0.0);

        let mut mgr = WaveManager::new(vec![wave], lib);
        mgr.start();

        // Tick past activation
        let events = mgr.tick(0.016);
        assert!(!events.is_empty(), "Expected spawn events");
    }

    #[test]
    fn wave_manager_rate_spawn() {
        let lib = BlueprintLibrary::new().with_defaults();
        let wave = SpawnWave::new("w1", vec![
            SpawnGroup::new("grunt", 10, SpawnZone::Point(Vec3::ZERO))
                .with_rate(5.0)  // 5 per second
                .non_blocking(),
        ]).with_pre_delay(0.0).with_post_delay(0.0);

        let mut mgr = WaveManager::new(vec![wave], lib);
        mgr.start();

        let mut total = 0;
        for _ in 0..60 {
            total += mgr.tick(1.0 / 60.0).len();
        }
        // In 1 second at 5/s, should spawn ~5
        assert!(total >= 4 && total <= 6, "Expected ~5 spawns, got {}", total);
    }

    #[test]
    fn wave_advances() {
        let lib = BlueprintLibrary::new().with_defaults();
        let w1 = SpawnWave::new("w1", vec![
            SpawnGroup::new("grunt", 1, SpawnZone::Point(Vec3::ZERO))
                .non_blocking(),
        ]).with_pre_delay(0.0).with_post_delay(0.0);
        let w2 = SpawnWave::new("w2", vec![
            SpawnGroup::new("tank", 1, SpawnZone::Point(Vec3::ONE))
                .non_blocking(),
        ]).with_pre_delay(0.0).with_post_delay(0.0);

        let mut mgr = WaveManager::new(vec![w1, w2], lib);
        mgr.start();

        // Drain wave 1
        for _ in 0..30 { mgr.tick(0.1); }
        assert_eq!(mgr.current_wave_name(), "w2");
    }
}
