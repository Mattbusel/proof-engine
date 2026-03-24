//! Open-world management: zone streaming, portals, day/night cycle,
//! weather simulation, and world-level event coordination.

use glam::{Vec2, Vec3, Vec4};
use std::collections::{HashMap, HashSet, VecDeque};

// ─── Zone ─────────────────────────────────────────────────────────────────────

/// Unique identifier for a world zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZoneId(pub u32);

/// Streaming state of a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneStreamState {
    Unloaded,
    Loading,
    Active,
    Dormant,   // loaded but not ticked (background)
    Unloading,
}

/// World axis-aligned bounding rectangle (XZ plane).
#[derive(Debug, Clone, Copy)]
pub struct WorldRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl WorldRect {
    pub fn new(min: Vec2, max: Vec2) -> Self { Self { min, max } }

    pub fn from_center(center: Vec2, half_size: Vec2) -> Self {
        Self { min: center - half_size, max: center + half_size }
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn overlaps(&self, other: &WorldRect) -> bool {
        self.min.x < other.max.x && self.max.x > other.min.x &&
        self.min.y < other.max.y && self.max.y > other.min.y
    }

    pub fn center(&self) -> Vec2 { (self.min + self.max) * 0.5 }

    pub fn size(&self) -> Vec2 { self.max - self.min }

    pub fn expand(&self, margin: f32) -> Self {
        Self {
            min: self.min - Vec2::splat(margin),
            max: self.max + Vec2::splat(margin),
        }
    }

    pub fn distance_to(&self, p: Vec2) -> f32 {
        let dx = (self.min.x - p.x).max(0.0).max(p.x - self.max.x);
        let dy = (self.min.y - p.y).max(0.0).max(p.y - self.max.y);
        Vec2::new(dx, dy).length()
    }
}

/// A world zone definition.
#[derive(Debug, Clone)]
pub struct Zone {
    pub id:         ZoneId,
    pub name:       String,
    pub bounds:     WorldRect,
    pub state:      ZoneStreamState,
    pub biome:      BiomeType,
    pub neighbors:  Vec<ZoneId>,
    pub portals:    Vec<PortalId>,
    pub load_priority: f32,
    pub is_indoor:  bool,
    pub ambient_color: Vec4,
    pub fog_color:     Vec4,
    pub fog_density:   f32,
    pub tick_count:    u64,
}

impl Zone {
    pub fn new(id: ZoneId, name: impl Into<String>, bounds: WorldRect) -> Self {
        Self {
            id, name: name.into(), bounds,
            state: ZoneStreamState::Unloaded,
            biome: BiomeType::Temperate,
            neighbors: Vec::new(),
            portals: Vec::new(),
            load_priority: 0.0,
            is_indoor: false,
            ambient_color: Vec4::new(0.2, 0.2, 0.3, 1.0),
            fog_color:     Vec4::new(0.7, 0.8, 0.9, 1.0),
            fog_density:   0.002,
            tick_count:    0,
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self.state, ZoneStreamState::Active | ZoneStreamState::Dormant)
    }
}

// ─── Biome ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeType {
    Temperate,
    Desert,
    Arctic,
    Tropical,
    Swamp,
    Volcanic,
    Underground,
    Ocean,
    Sky,
    Corrupted,
}

impl BiomeType {
    pub fn ambient_temperature(&self) -> f32 {
        match self {
            BiomeType::Desert      => 45.0,
            BiomeType::Arctic      => -20.0,
            BiomeType::Tropical    => 30.0,
            BiomeType::Volcanic    => 80.0,
            BiomeType::Underground => 12.0,
            BiomeType::Ocean       => 18.0,
            BiomeType::Sky         => -5.0,
            _                      => 20.0,
        }
    }

    pub fn base_weather(&self) -> WeatherType {
        match self {
            BiomeType::Desert   => WeatherType::Clear,
            BiomeType::Arctic   => WeatherType::Blizzard,
            BiomeType::Tropical => WeatherType::HeavyRain,
            BiomeType::Volcanic => WeatherType::AshStorm,
            _                   => WeatherType::Cloudy,
        }
    }
}

// ─── Portal ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortalId(pub u32);

/// A portal connecting two zones.
#[derive(Debug, Clone)]
pub struct Portal {
    pub id:        PortalId,
    pub from_zone: ZoneId,
    pub to_zone:   ZoneId,
    pub from_pos:  Vec3,
    pub to_pos:    Vec3,
    pub from_rot:  f32,  // yaw in radians
    pub to_rot:    f32,
    pub radius:    f32,  // trigger radius
    pub bidirectional: bool,
    pub locked:    bool,
    pub unlock_condition: String,
}

impl Portal {
    pub fn new(id: PortalId, from: ZoneId, to: ZoneId, from_pos: Vec3, to_pos: Vec3) -> Self {
        Self {
            id, from_zone: from, to_zone: to,
            from_pos, to_pos,
            from_rot: 0.0, to_rot: std::f32::consts::PI,
            radius: 1.5, bidirectional: true, locked: false,
            unlock_condition: String::new(),
        }
    }

    pub fn player_in_trigger(&self, player_pos: Vec3) -> bool {
        (player_pos - self.from_pos).length() <= self.radius
    }
}

// ─── Day/night cycle ─────────────────────────────────────────────────────────

/// Full day/night cycle state.
#[derive(Debug, Clone)]
pub struct DayNightCycle {
    /// Time of day in range [0, 1) — 0=midnight, 0.25=6am, 0.5=noon, 0.75=6pm.
    pub time_of_day: f32,
    /// Duration of a full day in real seconds.
    pub day_duration: f32,
    /// Current day number.
    pub day_count: u32,
    /// Speed multiplier (1.0 = real-time).
    pub speed: f32,
    pub paused: bool,
}

impl DayNightCycle {
    pub fn new(day_duration: f32) -> Self {
        Self { time_of_day: 0.25, day_duration, day_count: 0, speed: 1.0, paused: false }
    }

    pub fn tick(&mut self, dt: f32) {
        if self.paused { return; }
        self.time_of_day += (dt * self.speed) / self.day_duration;
        if self.time_of_day >= 1.0 {
            self.time_of_day -= 1.0;
            self.day_count += 1;
        }
    }

    /// Hours since midnight (0–24).
    pub fn hour(&self) -> f32 { self.time_of_day * 24.0 }

    pub fn is_daytime(&self) -> bool {
        let h = self.hour();
        h >= 6.0 && h < 20.0
    }

    pub fn is_dawn(&self) -> bool { let h = self.hour(); h >= 5.5 && h < 7.5 }
    pub fn is_dusk(&self) -> bool { let h = self.hour(); h >= 18.5 && h < 20.5 }
    pub fn is_night(&self) -> bool { !self.is_daytime() }

    /// Sun direction (Vec3, normalized).
    pub fn sun_direction(&self) -> Vec3 {
        let angle = (self.time_of_day - 0.25) * std::f32::consts::TAU;
        Vec3::new(angle.cos(), angle.sin(), -0.3).normalize_or_zero()
    }

    /// Moon direction (opposite of sun, slightly offset).
    pub fn moon_direction(&self) -> Vec3 {
        let sun = self.sun_direction();
        Vec3::new(-sun.x, -sun.y * 0.8, -sun.z + 0.2).normalize_or_zero()
    }

    /// Sky color based on time of day.
    pub fn sky_color(&self) -> Vec4 {
        let h = self.hour();
        // Dawn: orange, Day: blue, Dusk: red-orange, Night: dark blue
        let (r, g, b) = if h < 5.0 || h >= 21.0 {
            (0.02, 0.02, 0.08)  // Night
        } else if h < 7.0 {
            let t = (h - 5.0) / 2.0;
            let r = 0.02 + t * (1.0 - 0.02);
            let g = 0.02 + t * (0.5 - 0.02);
            let b = 0.08 + t * (0.3 - 0.08);
            (r, g, b)
        } else if h < 18.0 {
            (0.3, 0.55, 0.9)  // Day
        } else if h < 20.0 {
            let t = (h - 18.0) / 2.0;
            let r = 0.3 + t * (0.8 - 0.3);
            let g = 0.55 + t * (0.3 - 0.55);
            let b = 0.9 + t * (0.1 - 0.9);
            (r, g, b)
        } else {
            let t = (h - 20.0) / 1.0;
            (0.8 + t * (0.02 - 0.8), 0.3 + t * (0.02 - 0.3), 0.1 + t * (0.08 - 0.1))
        };
        Vec4::new(r.max(0.0).min(1.0), g.max(0.0).min(1.0), b.max(0.0).min(1.0), 1.0)
    }

    /// Ambient light intensity (0–1).
    pub fn ambient_intensity(&self) -> f32 {
        let h = self.hour();
        if h < 6.0 || h >= 20.0 { return 0.05; }
        if h < 8.0 { return 0.05 + (h - 6.0) / 2.0 * 0.95; }
        if h > 18.0 { return 0.05 + (20.0 - h) / 2.0 * 0.95; }
        1.0
    }

    /// Sun light intensity.
    pub fn sun_intensity(&self) -> f32 {
        let h = self.hour();
        if h < 6.0 || h >= 20.0 { return 0.0; }
        let t = if h < 13.0 { (h - 6.0) / 7.0 } else { (20.0 - h) / 7.0 };
        t.max(0.0).min(1.0)
    }

    pub fn set_hour(&mut self, h: f32) {
        self.time_of_day = (h / 24.0).fract();
    }
}

// ─── Weather ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherType {
    Clear,
    Cloudy,
    Overcast,
    LightRain,
    HeavyRain,
    Thunderstorm,
    LightSnow,
    HeavySnow,
    Blizzard,
    Fog,
    Heatwave,
    AshStorm,
    MagicStorm,
}

/// Weather system with transition and effects.
#[derive(Debug, Clone)]
pub struct WeatherSystem {
    pub current:    WeatherType,
    pub target:     WeatherType,
    pub blend:      f32,   // 0=current, 1=target
    pub transition_speed: f32,
    pub cloud_coverage: f32,  // 0..1
    pub precipitation: f32,   // 0..1 (rain/snow intensity)
    pub wind_speed:    f32,   // m/s
    pub wind_dir:      Vec2,  // normalized
    pub visibility:    f32,   // km
    pub temperature:   f32,   // celsius
    pub humidity:      f32,   // 0..1
    pub lightning_chance: f32, // per-second probability during storm
    pub thunder_delay:    f32, // seconds after lightning
    time_in_state: f32,
    forecast:      Vec<(WeatherType, f32)>, // upcoming weather + duration
}

impl WeatherSystem {
    pub fn new() -> Self {
        Self {
            current: WeatherType::Clear,
            target:  WeatherType::Clear,
            blend:   1.0,
            transition_speed: 0.1,
            cloud_coverage: 0.1,
            precipitation: 0.0,
            wind_speed: 2.0,
            wind_dir: Vec2::new(1.0, 0.0),
            visibility: 20.0,
            temperature: 20.0,
            humidity: 0.4,
            lightning_chance: 0.0,
            thunder_delay: 3.0,
            time_in_state: 0.0,
            forecast: Vec::new(),
        }
    }

    pub fn transition_to(&mut self, weather: WeatherType, speed: f32) {
        if self.current == weather { return; }
        self.target = weather;
        self.blend = 0.0;
        self.transition_speed = speed;
    }

    pub fn tick(&mut self, dt: f32) {
        self.time_in_state += dt;

        if self.blend < 1.0 {
            self.blend = (self.blend + self.transition_speed * dt).min(1.0);
            if self.blend >= 1.0 {
                self.current = self.target;
                self.time_in_state = 0.0;
            }
        }

        // Update derived values from current + blend toward target
        let (cc, pr, ws, vis, temp, hum, light) = Self::weather_params(self.current);
        let (tcc, tpr, tws, tvis, ttemp, thum, tlight) = Self::weather_params(self.target);
        let t = self.blend;
        self.cloud_coverage   = cc + t * (tcc - cc);
        self.precipitation    = pr + t * (tpr - pr);
        self.wind_speed       = ws + t * (tws - ws);
        self.visibility       = vis + t * (tvis - vis);
        self.temperature      = temp + t * (ttemp - temp);
        self.humidity         = hum + t * (thum - hum);
        self.lightning_chance = light + t * (tlight - light);
    }

    fn weather_params(w: WeatherType) -> (f32, f32, f32, f32, f32, f32, f32) {
        // (cloud_coverage, precipitation, wind_speed, visibility, temperature, humidity, lightning_chance)
        match w {
            WeatherType::Clear       => (0.1, 0.0, 2.0,  20.0, 22.0, 0.3, 0.0),
            WeatherType::Cloudy      => (0.5, 0.0, 4.0,  15.0, 18.0, 0.5, 0.0),
            WeatherType::Overcast    => (0.9, 0.0, 5.0,  10.0, 14.0, 0.7, 0.0),
            WeatherType::LightRain   => (0.7, 0.3, 6.0,  8.0,  12.0, 0.8, 0.0),
            WeatherType::HeavyRain   => (0.9, 0.8, 10.0, 3.0,  10.0, 0.95,0.0),
            WeatherType::Thunderstorm=> (1.0, 0.9, 15.0, 2.0,  9.0,  1.0, 0.05),
            WeatherType::LightSnow   => (0.7, 0.3, 5.0,  6.0, -2.0,  0.7, 0.0),
            WeatherType::HeavySnow   => (0.9, 0.8, 8.0,  2.0, -8.0,  0.8, 0.0),
            WeatherType::Blizzard    => (1.0, 1.0, 20.0, 0.5,-15.0,  0.9, 0.0),
            WeatherType::Fog         => (0.3, 0.0, 1.0,  0.2, 10.0,  0.95,0.0),
            WeatherType::Heatwave    => (0.0, 0.0, 3.0,  18.0, 42.0, 0.1, 0.0),
            WeatherType::AshStorm    => (1.0, 0.0, 18.0, 0.5, 35.0,  0.1, 0.0),
            WeatherType::MagicStorm  => (1.0, 0.5, 12.0, 1.0, 10.0,  0.7, 0.1),
        }
    }

    pub fn is_raining(&self) -> bool {
        matches!(self.current, WeatherType::LightRain | WeatherType::HeavyRain | WeatherType::Thunderstorm)
    }

    pub fn is_snowing(&self) -> bool {
        matches!(self.current, WeatherType::LightSnow | WeatherType::HeavySnow | WeatherType::Blizzard)
    }

    pub fn push_forecast(&mut self, weather: WeatherType, duration_secs: f32) {
        self.forecast.push((weather, duration_secs));
    }

    pub fn advance_forecast(&mut self) {
        if !self.forecast.is_empty() {
            let (next, _) = self.forecast.remove(0);
            self.transition_to(next, 0.05);
        }
    }
}

// ─── World clock ─────────────────────────────────────────────────────────────

/// In-game calendar tracking.
#[derive(Debug, Clone)]
pub struct WorldClock {
    pub year:   u32,
    pub month:  u32,  // 1-12
    pub day:    u32,  // 1-30
    pub hour:   f32,  // 0-24
    pub days_per_month: u32,
    pub months_per_year: u32,
    pub epoch_name: String,
}

impl WorldClock {
    pub fn new() -> Self {
        Self { year: 1, month: 1, day: 1, hour: 6.0, days_per_month: 30, months_per_year: 12, epoch_name: "Age of Stars".into() }
    }

    pub fn advance_hours(&mut self, h: f32) {
        self.hour += h;
        while self.hour >= 24.0 {
            self.hour -= 24.0;
            self.advance_days(1);
        }
    }

    pub fn advance_days(&mut self, d: u32) {
        self.day += d;
        while self.day > self.days_per_month {
            self.day -= self.days_per_month;
            self.month += 1;
            if self.month > self.months_per_year {
                self.month = 1;
                self.year += 1;
            }
        }
    }

    pub fn total_days(&self) -> u64 {
        let y = self.year as u64;
        let m = self.month as u64;
        let d = self.day as u64;
        y * self.months_per_year as u64 * self.days_per_month as u64
            + (m - 1) * self.days_per_month as u64 + (d - 1)
    }

    pub fn display(&self) -> String {
        format!("Day {}/{}/{} {}  ({})",
            self.day, self.month, self.year,
            format!("{:02}:{:02}", self.hour as u32, ((self.hour.fract()) * 60.0) as u32),
            self.epoch_name)
    }

    pub fn season(&self) -> Season {
        match self.month {
            3..=5  => Season::Spring,
            6..=8  => Season::Summer,
            9..=11 => Season::Autumn,
            _      => Season::Winter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season { Spring, Summer, Autumn, Winter }

// ─── Zone streaming manager ───────────────────────────────────────────────────

/// Manages which zones are loaded/active based on player position.
pub struct ZoneStreamer {
    pub zones:         HashMap<ZoneId, Zone>,
    pub portals:       HashMap<PortalId, Portal>,
    pub active_zones:  HashSet<ZoneId>,
    pub load_distance: f32,
    pub unload_distance: f32,
    next_zone_id:      u32,
    next_portal_id:    u32,
    pub load_queue:    VecDeque<ZoneId>,
    pub unload_queue:  VecDeque<ZoneId>,
}

impl ZoneStreamer {
    pub fn new(load_distance: f32) -> Self {
        Self {
            zones: HashMap::new(),
            portals: HashMap::new(),
            active_zones: HashSet::new(),
            load_distance,
            unload_distance: load_distance * 1.5,
            next_zone_id: 1,
            next_portal_id: 1,
            load_queue: VecDeque::new(),
            unload_queue: VecDeque::new(),
        }
    }

    pub fn register_zone(&mut self, name: impl Into<String>, bounds: WorldRect) -> ZoneId {
        let id = ZoneId(self.next_zone_id);
        self.next_zone_id += 1;
        self.zones.insert(id, Zone::new(id, name, bounds));
        id
    }

    pub fn add_portal(&mut self, from: ZoneId, to: ZoneId, from_pos: Vec3, to_pos: Vec3) -> PortalId {
        let id = PortalId(self.next_portal_id);
        self.next_portal_id += 1;
        let portal = Portal::new(id, from, to, from_pos, to_pos);
        // Link zones
        if let Some(z) = self.zones.get_mut(&from) { z.portals.push(id); }
        if let Some(z) = self.zones.get_mut(&to) {
            if portal.bidirectional { z.portals.push(id); }
        }
        self.portals.insert(id, portal);
        id
    }

    /// Update streaming based on player world position.
    pub fn update(&mut self, player_pos: Vec3, dt: f32) {
        let player_xz = Vec2::new(player_pos.x, player_pos.z);

        // Determine which zones should be loaded
        let should_load: HashSet<ZoneId> = self.zones.iter()
            .filter(|(_, z)| {
                let dist = z.bounds.distance_to(player_xz);
                dist <= self.load_distance
            })
            .map(|(id, _)| *id)
            .collect();

        let should_unload: HashSet<ZoneId> = self.active_zones.iter()
            .filter(|id| {
                if let Some(z) = self.zones.get(id) {
                    z.bounds.distance_to(player_xz) > self.unload_distance
                } else { true }
            })
            .cloned()
            .collect();

        // Queue loads
        for id in &should_load {
            if !self.active_zones.contains(id) {
                if !self.load_queue.contains(id) {
                    self.load_queue.push_back(*id);
                }
            }
        }

        // Queue unloads
        for id in &should_unload {
            if !self.unload_queue.contains(id) {
                self.unload_queue.push_back(*id);
            }
        }

        // Process load queue (one per frame to avoid spikes)
        if let Some(id) = self.load_queue.pop_front() {
            if let Some(z) = self.zones.get_mut(&id) {
                z.state = ZoneStreamState::Active;
            }
            self.active_zones.insert(id);
        }

        // Process unload queue
        if let Some(id) = self.unload_queue.pop_front() {
            if let Some(z) = self.zones.get_mut(&id) {
                z.state = ZoneStreamState::Unloaded;
            }
            self.active_zones.remove(&id);
        }

        // Tick active zones
        for id in &self.active_zones {
            if let Some(z) = self.zones.get_mut(id) {
                z.tick_count = z.tick_count.wrapping_add(1);
                let _ = dt;
            }
        }
    }

    pub fn zone_at(&self, world_pos: Vec3) -> Option<ZoneId> {
        let xz = Vec2::new(world_pos.x, world_pos.z);
        self.zones.iter()
            .find(|(_, z)| z.bounds.contains_point(xz))
            .map(|(id, _)| *id)
    }

    pub fn get_zone(&self, id: ZoneId) -> Option<&Zone> { self.zones.get(&id) }
    pub fn get_zone_mut(&mut self, id: ZoneId) -> Option<&mut Zone> { self.zones.get_mut(&id) }

    pub fn portals_near(&self, pos: Vec3, radius: f32) -> Vec<&Portal> {
        self.portals.values()
            .filter(|p| (p.from_pos - pos).length() <= radius)
            .collect()
    }

    pub fn check_portal_transitions(&self, player_pos: Vec3) -> Option<&Portal> {
        self.portals.values().find(|p| !p.locked && p.player_in_trigger(player_pos))
    }
}

// ─── World state ─────────────────────────────────────────────────────────────

/// Top-level world state coordinator.
pub struct WorldState {
    pub day_night:  DayNightCycle,
    pub weather:    WeatherSystem,
    pub clock:      WorldClock,
    pub streamer:   ZoneStreamer,
    pub ticks:      u64,
    pub world_time: f64,  // total elapsed seconds
    pub paused:     bool,
    events:         Vec<WorldEvent>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            day_night: DayNightCycle::new(1200.0),  // 20-minute days
            weather:   WeatherSystem::new(),
            clock:     WorldClock::new(),
            streamer:  ZoneStreamer::new(200.0),
            ticks:     0,
            world_time: 0.0,
            paused:    false,
            events:    Vec::new(),
        }
    }

    pub fn tick(&mut self, dt: f32, player_pos: Vec3) {
        if self.paused { return; }
        let dt = dt.min(0.1);  // clamp to avoid spiral

        self.world_time += dt as f64;
        self.ticks += 1;

        self.day_night.tick(dt);
        self.weather.tick(dt);
        self.clock.advance_hours(dt / 3600.0 * self.day_night.speed);
        self.streamer.update(player_pos, dt);

        // Weather forecast advance (every in-game hour)
        if self.ticks % 3600 == 0 {
            self.weather.advance_forecast();
        }

        // Lightning events during storms
        if self.weather.lightning_chance > 0.0 {
            // Simple deterministic check using time
            let should_lightning = (self.world_time * 1000.0) as u64 % 1000 < (self.weather.lightning_chance * 1000.0) as u64;
            if should_lightning {
                self.events.push(WorldEvent::Lightning {
                    position: Vec3::new(
                        ((self.ticks * 17) % 1000) as f32 - 500.0,
                        100.0,
                        ((self.ticks * 31) % 1000) as f32 - 500.0,
                    ),
                });
            }
        }
    }

    /// Drain and return world events since last call.
    pub fn drain_events(&mut self) -> Vec<WorldEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn current_zone(&self, player_pos: Vec3) -> Option<ZoneId> {
        self.streamer.zone_at(player_pos)
    }

    pub fn sky_color(&self) -> Vec4 {
        self.day_night.sky_color()
    }

    pub fn fog_color(&self) -> Vec4 {
        // Blend zone fog with weather fog
        Vec4::new(0.7, 0.8, 0.9, 1.0)
    }

    pub fn fog_density(&self) -> f32 {
        let base = 0.002;
        let weather_mult = 1.0 + (1.0 - self.weather.visibility / 20.0) * 5.0;
        base * weather_mult.max(1.0)
    }

    pub fn ambient_color(&self) -> Vec4 {
        let sky = self.day_night.sky_color();
        let intensity = self.day_night.ambient_intensity();
        Vec4::new(sky.x * intensity, sky.y * intensity, sky.z * intensity, 1.0)
    }
}

/// World-level events dispatched during tick.
#[derive(Debug, Clone)]
pub enum WorldEvent {
    ZoneLoaded(ZoneId),
    ZoneUnloaded(ZoneId),
    DayStart { day: u32 },
    NightStart { day: u32 },
    WeatherChanged { from: WeatherType, to: WeatherType },
    Lightning { position: Vec3 },
    SeasonChanged(Season),
    NewYear { year: u32 },
}

impl Default for WorldState {
    fn default() -> Self { Self::new() }
}

impl Default for WeatherSystem {
    fn default() -> Self { Self::new() }
}

impl Default for DayNightCycle {
    fn default() -> Self { Self::new(1200.0) }
}

impl Default for WorldClock {
    fn default() -> Self { Self::new() }
}
