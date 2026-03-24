//! Precipitation physics subsystem.
//!
//! Simulates rain, snow, hail, sleet, and drizzle — including individual droplet physics,
//! surface accumulation, puddle formation, snowpack, ice, and thunderstorm timing.

use std::collections::HashMap;
use super::{Vec3, lerp, smoothstep, fbm_2d, value_noise_2d};

// ── Types of precipitation ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrecipitationType {
    None,
    Drizzle,
    Rain,
    Snow,
    Sleet,
    Hail,
}

impl PrecipitationType {
    /// Terminal velocity (m/s) for a "typical" drop/particle.
    pub fn terminal_velocity(self) -> f32 {
        match self {
            Self::None    => 0.0,
            Self::Drizzle => 1.0,
            Self::Rain    => 6.5,
            Self::Snow    => 1.2,
            Self::Sleet   => 4.0,
            Self::Hail    => 20.0,
        }
    }

    /// Typical particle radius (metres).
    pub fn typical_radius_m(self) -> f32 {
        match self {
            Self::None    => 0.0,
            Self::Drizzle => 0.000_2,
            Self::Rain    => 0.001_5,
            Self::Snow    => 0.003,
            Self::Sleet   => 0.002,
            Self::Hail    => 0.015,
        }
    }

    /// Whether this type can form ice on surfaces.
    pub fn can_ice(self) -> bool {
        matches!(self, Self::Sleet | Self::Hail)
    }

    /// Whether this type accumulates as a soft layer.
    pub fn is_soft_accumulation(self) -> bool {
        matches!(self, Self::Snow | Self::Drizzle)
    }
}

// ── Configuration ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PrecipitationConfig {
    /// Maximum simultaneous droplets tracked explicitly.
    pub max_droplets: usize,
    /// Grid width for accumulation tracking.
    pub grid_width: usize,
    /// Grid depth for accumulation tracking.
    pub grid_depth: usize,
    /// World units covered by each accumulation cell.
    pub cell_size: f32,
    /// Threshold humidity for rain formation [0,1].
    pub rain_humidity_threshold: f32,
    /// Temperature (°C) below which precipitation falls as snow.
    pub snow_threshold_c: f32,
    /// Temperature (°C) below which sleet transitions to hail.
    pub hail_threshold_c: f32,
    /// Puddle evaporation rate per second.
    pub puddle_evap_rate: f32,
    /// Snowpack settling rate (fractional per second).
    pub snow_settle_rate: f32,
}

impl Default for PrecipitationConfig {
    fn default() -> Self {
        Self {
            max_droplets: 4096,
            grid_width: 64,
            grid_depth: 64,
            cell_size: 10.0,
            rain_humidity_threshold: 0.75,
            snow_threshold_c: 2.0,
            hail_threshold_c: -5.0,
            puddle_evap_rate: 0.001,
            snow_settle_rate: 0.0005,
        }
    }
}

// ── Droplet Physics ───────────────────────────────────────────────────────────

/// Physical parameters for a class of droplets/particles.
#[derive(Debug, Clone, Copy)]
pub struct DropletPhysics {
    pub radius_m: f32,
    pub density_kg_m3: f32,   // e.g. 1000 for water, 917 for ice
    pub drag_coeff: f32,
    pub air_density: f32,
}

impl DropletPhysics {
    pub fn water_drop(radius_m: f32) -> Self {
        Self { radius_m, density_kg_m3: 1000.0, drag_coeff: 0.47, air_density: 1.225 }
    }
    pub fn ice_particle(radius_m: f32) -> Self {
        Self { radius_m, density_kg_m3: 917.0, drag_coeff: 0.55, air_density: 1.225 }
    }
    pub fn snowflake(radius_m: f32) -> Self {
        Self { radius_m, density_kg_m3: 100.0, drag_coeff: 1.5, air_density: 1.225 }
    }

    /// Mass (kg) of the particle.
    pub fn mass(&self) -> f32 {
        let vol = (4.0 / 3.0) * std::f32::consts::PI * self.radius_m.powi(3);
        vol * self.density_kg_m3
    }

    /// Cross-sectional area (m²).
    pub fn area(&self) -> f32 {
        std::f32::consts::PI * self.radius_m * self.radius_m
    }

    /// Terminal velocity (m/s) under gravity, positive downward.
    pub fn terminal_velocity(&self) -> f32 {
        let m   = self.mass();
        let a   = self.area();
        let cd  = self.drag_coeff;
        let rho = self.air_density;
        let g   = 9.807_f32;
        (2.0 * m * g / (cd * rho * a)).sqrt()
    }

    /// Net acceleration given current velocity and ambient wind.
    pub fn acceleration(&self, vel: Vec3, wind: Vec3) -> Vec3 {
        let rel = vel.sub(wind); // velocity relative to air
        let speed = rel.length();
        let m = self.mass();
        let a = self.area();
        // Drag force opposing relative velocity
        let drag_mag = 0.5 * self.air_density * self.drag_coeff * a * speed * speed;
        let drag_dir = if speed > 1e-6 { rel.scale(-1.0 / speed) } else { Vec3::ZERO };
        let drag = drag_dir.scale(drag_mag / m);
        // Gravity
        let gravity = Vec3::new(0.0, -9.807, 0.0);
        gravity.add(drag)
    }
}

// ── Individual Droplet ────────────────────────────────────────────────────────

/// A single simulated precipitation particle.
#[derive(Debug, Clone)]
pub struct Droplet {
    pub position: Vec3,
    pub velocity: Vec3,
    pub kind: PrecipitationType,
    pub physics: DropletPhysics,
    /// Remaining lifetime (seconds).
    pub lifetime: f32,
    /// Has this droplet hit a surface?
    pub landed: bool,
    /// Accumulated coalescence (rain drops grow by merging).
    pub coalescence: f32,
}

impl Droplet {
    pub fn new_rain(pos: Vec3) -> Self {
        let r = 0.001 + value_noise_2d(pos.x * 0.1, pos.z * 0.1) * 0.002;
        let phys = DropletPhysics::water_drop(r);
        let vt = -phys.terminal_velocity();
        Self {
            position: pos,
            velocity: Vec3::new(0.0, vt * 0.5, 0.0),
            kind: PrecipitationType::Rain,
            physics: phys,
            lifetime: 10.0,
            landed: false,
            coalescence: 0.0,
        }
    }

    pub fn new_snow(pos: Vec3) -> Self {
        let r = 0.002 + value_noise_2d(pos.x * 0.05, pos.z * 0.05) * 0.003;
        let phys = DropletPhysics::snowflake(r);
        Self {
            position: pos,
            velocity: Vec3::new(0.0, -0.8, 0.0),
            kind: PrecipitationType::Snow,
            physics: phys,
            lifetime: 20.0,
            landed: false,
            coalescence: 0.0,
        }
    }

    pub fn new_hail(pos: Vec3) -> Self {
        let r = 0.008 + value_noise_2d(pos.x * 0.02, pos.z * 0.02) * 0.015;
        let phys = DropletPhysics::ice_particle(r);
        let vt = -phys.terminal_velocity();
        Self {
            position: pos,
            velocity: Vec3::new(0.0, vt * 0.3, 0.0),
            kind: PrecipitationType::Hail,
            physics: phys,
            lifetime: 15.0,
            landed: false,
            coalescence: 0.0,
        }
    }

    pub fn new_sleet(pos: Vec3) -> Self {
        let r = 0.0015;
        let phys = DropletPhysics::ice_particle(r);
        let vt = -phys.terminal_velocity();
        Self {
            position: pos,
            velocity: Vec3::new(0.0, vt * 0.6, 0.0),
            kind: PrecipitationType::Sleet,
            physics: phys,
            lifetime: 8.0,
            landed: false,
            coalescence: 0.0,
        }
    }

    /// Integrate motion by `dt` seconds with the given ambient wind.
    pub fn tick(&mut self, dt: f32, wind: Vec3, surface_y: f32) {
        if self.landed { return; }
        let acc = self.physics.acceleration(self.velocity, wind);
        self.velocity = self.velocity.add(acc.scale(dt));
        self.position = self.position.add(self.velocity.scale(dt));
        self.lifetime -= dt;
        // Check surface contact
        if self.position.y <= surface_y {
            self.position.y = surface_y;
            self.landed = true;
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.landed && self.lifetime > 0.0
    }
}

// ── Rain Band ─────────────────────────────────────────────────────────────────

/// A mesoscale band of rain — used for efficient bulk simulation.
#[derive(Debug, Clone)]
pub struct RainBand {
    /// Centre of the band in world (x, z).
    pub centre: [f32; 2],
    /// Radius of the band (m).
    pub radius: f32,
    /// Intensity (mm/hr equivalent, 0–100).
    pub intensity: f32,
    /// Drift velocity (m/s).
    pub drift: [f32; 2],
    /// Precipitation type in this band.
    pub kind: PrecipitationType,
    /// Lifetime remaining (seconds).
    pub lifetime: f32,
}

impl RainBand {
    pub fn new(cx: f32, cz: f32, radius: f32, intensity: f32, kind: PrecipitationType) -> Self {
        Self {
            centre: [cx, cz],
            radius,
            intensity,
            drift: [2.0, 1.0],
            kind,
            lifetime: 3600.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.centre[0] += self.drift[0] * dt;
        self.centre[1] += self.drift[1] * dt;
        self.lifetime -= dt;
        // Intensity decays near end of life
        if self.lifetime < 300.0 {
            self.intensity *= 1.0 - dt / 300.0;
        }
    }

    /// Intensity (0–1) at world position (x, z).
    pub fn intensity_at(&self, x: f32, z: f32) -> f32 {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        let dist = (dx * dx + dz * dz).sqrt();
        if dist >= self.radius { return 0.0; }
        let t = smoothstep(self.radius, 0.0, dist);
        t * (self.intensity / 100.0)
    }

    pub fn is_alive(&self) -> bool {
        self.lifetime > 0.0 && self.intensity > 0.01
    }
}

// ── Surface Accumulation ─────────────────────────────────────────────────────

/// Grid of surface water/snow/ice depths.
#[derive(Debug, Clone)]
pub struct SurfaceAccumulation {
    pub width: usize,
    pub depth: usize,
    pub cell_size: f32,
    /// Water depth (m) per cell.
    pub water_depth: Vec<f32>,
    /// Snow depth (m) per cell.
    pub snow_depth: Vec<f32>,
    /// Ice thickness (m) per cell.
    pub ice_depth: Vec<f32>,
}

impl SurfaceAccumulation {
    pub fn new(width: usize, depth: usize, cell_size: f32) -> Self {
        let n = width * depth;
        Self {
            width,
            depth,
            cell_size,
            water_depth: vec![0.0; n],
            snow_depth:  vec![0.0; n],
            ice_depth:   vec![0.0; n],
        }
    }

    fn idx(&self, gx: usize, gz: usize) -> usize {
        gz * self.width + gx
    }

    fn world_to_grid(&self, wx: f32, wz: f32) -> (usize, usize) {
        let gx = (wx / self.cell_size).floor() as i32;
        let gz = (wz / self.cell_size).floor() as i32;
        (
            gx.rem_euclid(self.width as i32) as usize,
            gz.rem_euclid(self.depth as i32) as usize,
        )
    }

    /// Add `depth_m` of water at world position (wx, wz).
    pub fn add_water(&mut self, wx: f32, wz: f32, depth_m: f32) {
        let (gx, gz) = self.world_to_grid(wx, wz);
        let i = self.idx(gx, gz);
        self.water_depth[i] = (self.water_depth[i] + depth_m).min(1.0);
    }

    /// Add `depth_m` of snow at world position (wx, wz).
    pub fn add_snow(&mut self, wx: f32, wz: f32, depth_m: f32) {
        let (gx, gz) = self.world_to_grid(wx, wz);
        let i = self.idx(gx, gz);
        self.snow_depth[i] = (self.snow_depth[i] + depth_m).min(2.0);
    }

    /// Add `thickness_m` of ice at world position (wx, wz).
    pub fn add_ice(&mut self, wx: f32, wz: f32, thickness_m: f32) {
        let (gx, gz) = self.world_to_grid(wx, wz);
        let i = self.idx(gx, gz);
        self.ice_depth[i] = (self.ice_depth[i] + thickness_m).min(0.1);
    }

    /// Sample water depth at world position.
    pub fn water_at(&self, wx: f32, wz: f32) -> f32 {
        let (gx, gz) = self.world_to_grid(wx, wz);
        self.water_depth[self.idx(gx, gz)]
    }

    /// Sample snow depth at world position.
    pub fn snow_at(&self, wx: f32, wz: f32) -> f32 {
        let (gx, gz) = self.world_to_grid(wx, wz);
        self.snow_depth[self.idx(gx, gz)]
    }

    /// Sample ice thickness at world position.
    pub fn ice_at(&self, wx: f32, wz: f32) -> f32 {
        let (gx, gz) = self.world_to_grid(wx, wz);
        self.ice_depth[self.idx(gx, gz)]
    }

    /// Evaporate water uniformly.
    pub fn evaporate(&mut self, rate: f32) {
        for v in &mut self.water_depth {
            *v = (*v - rate).max(0.0);
        }
    }

    /// Melt snow into water at given temperature above 0°C.
    pub fn melt_snow(&mut self, temp_above_zero_c: f32, dt: f32) {
        // Approx melt rate: ~3mm per degree per hour
        let melt = (temp_above_zero_c * 3e-3 / 3600.0) * dt;
        for i in 0..self.snow_depth.len() {
            let melted = self.snow_depth[i].min(melt);
            self.snow_depth[i]  -= melted;
            self.water_depth[i]  = (self.water_depth[i] + melted * 0.3).min(1.0);
        }
    }

    /// Freeze surface water into ice at given temperature below 0°C.
    pub fn freeze_water(&mut self, temp_below_zero_c: f32, dt: f32) {
        let freeze_rate = temp_below_zero_c * 1e-4 * dt;
        for i in 0..self.water_depth.len() {
            let frozen = self.water_depth[i].min(freeze_rate);
            self.water_depth[i] -= frozen;
            self.ice_depth[i]    = (self.ice_depth[i] + frozen * 0.9).min(0.1);
        }
    }

    /// Flow water downhill by simple gradient diffusion (treats each cell equally as flat land).
    pub fn flow_water(&mut self, dt: f32) {
        // Simplified: water spreads to adjacent cells with less water
        let flow_rate = 0.05 * dt;
        let old = self.water_depth.clone();
        for gz in 0..self.depth {
            for gx in 0..self.width {
                let i = self.idx(gx, gz);
                let h = old[i];
                if h < 1e-5 { continue; }
                // Four neighbours
                let neighbours = [
                    if gx > 0 { Some(self.idx(gx - 1, gz)) } else { None },
                    if gx + 1 < self.width  { Some(self.idx(gx + 1, gz)) } else { None },
                    if gz > 0 { Some(self.idx(gx, gz - 1)) } else { None },
                    if gz + 1 < self.depth  { Some(self.idx(gx, gz + 1)) } else { None },
                ];
                for nb in neighbours.iter().flatten() {
                    if old[*nb] < h {
                        let transfer = (h - old[*nb]) * flow_rate * 0.25;
                        self.water_depth[i]   -= transfer;
                        self.water_depth[*nb] += transfer;
                    }
                }
            }
        }
        for v in &mut self.water_depth {
            *v = v.clamp(0.0, 1.0);
        }
    }
}

// ── Puddle ────────────────────────────────────────────────────────────────────

/// A surface puddle — formed when water accumulates above a threshold.
#[derive(Debug, Clone)]
pub struct Puddle {
    pub centre: [f32; 2],
    pub radius: f32,
    /// Water volume (m³).
    pub volume: f32,
    /// Surface area (m²).
    pub area: f32,
    /// Evaporation rate (m³/s) under current conditions.
    pub evap_rate: f32,
    /// Age (seconds).
    pub age: f32,
    /// Is frozen?
    pub frozen: bool,
}

impl Puddle {
    pub fn new(cx: f32, cz: f32, initial_volume: f32) -> Self {
        let area = (initial_volume / 0.01).sqrt() * std::f32::consts::PI;
        let radius = (area / std::f32::consts::PI).sqrt();
        Self {
            centre: [cx, cz],
            radius,
            volume: initial_volume,
            area,
            evap_rate: 1e-6,
            age: 0.0,
            frozen: false,
        }
    }

    pub fn tick(&mut self, dt: f32, temp_c: f32, wind_speed: f32) {
        self.age += dt;
        if self.frozen { return; }

        if temp_c < 0.0 {
            self.frozen = true;
            return;
        }

        // Evaporation: driven by temperature and wind
        let evap = self.evap_rate * (1.0 + wind_speed * 0.1) * (1.0 + temp_c * 0.02) * dt;
        self.volume = (self.volume - evap).max(0.0);
        // Update geometry
        self.area   = (self.volume / 0.01).sqrt() * std::f32::consts::PI;
        self.radius = (self.area / std::f32::consts::PI).sqrt();
    }

    pub fn add_water(&mut self, volume: f32) {
        if self.frozen { return; }
        self.volume += volume;
        self.area    = (self.volume / 0.01).sqrt() * std::f32::consts::PI;
        self.radius  = (self.area / std::f32::consts::PI).sqrt();
    }

    pub fn is_alive(&self) -> bool {
        self.volume > 1e-8
    }

    pub fn contains(&self, x: f32, z: f32) -> bool {
        let dx = x - self.centre[0];
        let dz = z - self.centre[1];
        (dx * dx + dz * dz) <= self.radius * self.radius
    }

    /// Return water depth at the centre of the puddle.
    pub fn centre_depth_m(&self) -> f32 {
        if self.area < 1e-8 { return 0.0; }
        self.volume / self.area
    }
}

// ── Snowpack ──────────────────────────────────────────────────────────────────

/// One layer in a snow column.
#[derive(Debug, Clone)]
pub struct SnowpackLayer {
    /// Age since deposition (seconds).
    pub age: f32,
    /// Depth of this layer (m).
    pub depth_m: f32,
    /// Density (kg/m³) — freshly fallen ~50, settled ~300, firn ~600.
    pub density: f32,
    /// Temperature of the layer (K).
    pub temp_k: f32,
    /// Liquid water content [0,1] from melt.
    pub liquid_water: f32,
    /// Whether this layer has refrozen into ice.
    pub is_ice_layer: bool,
}

impl SnowpackLayer {
    pub fn fresh(depth_m: f32, temp_k: f32) -> Self {
        Self {
            age: 0.0,
            depth_m,
            density: 50.0,
            temp_k,
            liquid_water: 0.0,
            is_ice_layer: false,
        }
    }

    pub fn tick(&mut self, dt: f32, surface_temp_k: f32) {
        self.age += dt;
        // Heat diffusion (simplified)
        self.temp_k = lerp(self.temp_k, surface_temp_k, 0.0001 * dt);
        // Settling: density increases over time
        let settle_rate = 5e-6 * dt * (self.density / 50.0).sqrt();
        let new_density = (self.density + settle_rate * 300.0).min(917.0);
        // Conserve mass (depth decreases as density increases)
        if new_density > self.density + 1e-6 {
            self.depth_m *= self.density / new_density;
            self.density  = new_density;
        }
        // Melting above 273.15 K
        if self.temp_k > 273.15 {
            let melt = (self.temp_k - 273.15) * 0.001 * dt;
            let melted = self.depth_m.min(melt);
            self.depth_m    -= melted;
            self.liquid_water = (self.liquid_water + melted * 0.5 / self.depth_m.max(0.01)).min(1.0);
        }
        // Refreeze if cold with liquid water
        if self.temp_k < 271.0 && self.liquid_water > 0.0 {
            self.is_ice_layer = true;
        }
    }

    /// Snow water equivalent (m of water).
    pub fn swe(&self) -> f32 {
        self.depth_m * self.density / 1000.0
    }
}

/// Full snowpack at a grid cell, composed of layers.
#[derive(Debug, Clone)]
pub struct Snowpack {
    pub layers: Vec<SnowpackLayer>,
    pub max_layers: usize,
}

impl Snowpack {
    pub fn new() -> Self {
        Self { layers: Vec::new(), max_layers: 16 }
    }

    /// Total depth (m).
    pub fn total_depth(&self) -> f32 {
        self.layers.iter().map(|l| l.depth_m).sum()
    }

    /// Total SWE (m).
    pub fn total_swe(&self) -> f32 {
        self.layers.iter().map(|l| l.swe()).sum()
    }

    /// Add a fresh snow layer of given depth.
    pub fn deposit(&mut self, depth_m: f32, temp_k: f32) {
        if depth_m < 1e-6 { return; }
        self.layers.push(SnowpackLayer::fresh(depth_m, temp_k));
        // Merge thin layers if we exceed max_layers
        if self.layers.len() > self.max_layers {
            let a = self.layers.remove(0);
            let b = &mut self.layers[0];
            let total_mass = a.depth_m * a.density + b.depth_m * b.density;
            let total_depth = a.depth_m + b.depth_m;
            b.depth_m = total_depth;
            b.density = if total_depth > 0.0 { total_mass / total_depth } else { 100.0 };
            b.age = b.age.min(a.age);
        }
    }

    pub fn tick(&mut self, dt: f32, surface_temp_k: f32) {
        for layer in &mut self.layers {
            layer.tick(dt, surface_temp_k);
        }
        self.layers.retain(|l| l.depth_m > 1e-6);
    }
}

impl Default for Snowpack {
    fn default() -> Self { Self::new() }
}

// ── Ice Sheet ─────────────────────────────────────────────────────────────────

/// Ice formation on a surface.
#[derive(Debug, Clone)]
pub struct IceSheet {
    pub thickness_m: f32,
    pub surface_temp_k: f32,
    pub age: f32,
    /// Black ice: forms from thin water films, transparent and slippery.
    pub is_black_ice: bool,
    /// Rime ice: forms from freezing fog droplets.
    pub is_rime: bool,
}

impl IceSheet {
    pub fn new(thickness_m: f32, temp_k: f32, is_black_ice: bool) -> Self {
        Self {
            thickness_m,
            surface_temp_k: temp_k,
            age: 0.0,
            is_black_ice,
            is_rime: false,
        }
    }

    pub fn rime(thickness_m: f32) -> Self {
        Self {
            thickness_m,
            surface_temp_k: 268.0,
            age: 0.0,
            is_black_ice: false,
            is_rime: true,
        }
    }

    pub fn tick(&mut self, dt: f32, surface_temp_k: f32) {
        self.age += dt;
        self.surface_temp_k = surface_temp_k;
        if surface_temp_k > 273.15 {
            let melt = (surface_temp_k - 273.15) * 5e-5 * dt;
            self.thickness_m = (self.thickness_m - melt).max(0.0);
        } else {
            // Growth from residual moisture
            self.thickness_m = (self.thickness_m + 1e-7 * dt).min(0.05);
        }
    }

    pub fn friction_coefficient(&self) -> f32 {
        if self.is_black_ice {
            0.05 + self.thickness_m * 2.0 // very slippery
        } else if self.is_rime {
            0.15 + self.thickness_m * 5.0
        } else {
            0.1 + self.thickness_m * 3.0
        }
    }

    pub fn is_alive(&self) -> bool { self.thickness_m > 1e-7 }
}

// ── Snow Crystal ──────────────────────────────────────────────────────────────

/// A snow crystal with a simplified dendritic shape parameterisation.
#[derive(Debug, Clone, Copy)]
pub struct SnowCrystal {
    /// Crystal habit encoded as integer (0=plate, 1=column, 2=dendrite, 3=needle, 4=spatial dendrite).
    pub habit: u8,
    /// Maximum dimension (m).
    pub size_m: f32,
    /// Mass (kg).
    pub mass_kg: f32,
    /// Growth rate factor based on supersaturation.
    pub growth_rate: f32,
    /// Temperature at which it formed (K).
    pub formation_temp_k: f32,
}

impl SnowCrystal {
    pub fn form(temp_k: f32, supersaturation: f32) -> Self {
        let tc = temp_k - 273.15;
        let habit = if tc > -2.0        { 0 } // thin plates
            else if tc > -5.0           { 3 } // needles
            else if tc > -10.0          { 2 } // dendrites
            else if tc > -22.0          { 1 } // columns
            else                        { 4 }; // spatial dendrites
        let base_size = 0.001 + supersaturation * 0.003;
        let mass = match habit {
            0 => base_size.powi(2) * 1e-6 * 900.0,
            1 => base_size.powi(3) * 1e-9 * 900.0,
            _ => base_size.powi(2) * 3e-7 * 200.0,
        };
        Self {
            habit,
            size_m: base_size,
            mass_kg: mass,
            growth_rate: supersaturation * 1e-5,
            formation_temp_k: temp_k,
        }
    }

    pub fn grow(&mut self, dt: f32, supersaturation: f32) {
        self.size_m  += self.growth_rate * supersaturation * dt;
        self.mass_kg += self.growth_rate * supersaturation * dt * 1e-4;
    }

    pub fn habit_name(&self) -> &'static str {
        match self.habit {
            0 => "Thin plate",
            1 => "Column",
            2 => "Stellar dendrite",
            3 => "Needle",
            4 => "Spatial dendrite",
            _ => "Unknown",
        }
    }
}

// ── Hail Stone ────────────────────────────────────────────────────────────────

/// A hail stone that grows by cycling through a thunderstorm updraft.
#[derive(Debug, Clone)]
pub struct HailStone {
    pub radius_m: f32,
    pub mass_kg: f32,
    pub position: Vec3,
    pub velocity: Vec3,
    /// Number of updraft cycles.
    pub cycles: u32,
    /// Total ice accumulated (kg).
    pub ice_mass: f32,
    /// Liquid water coat thickness (m) — from warm air excursions.
    pub liquid_coat: f32,
}

impl HailStone {
    pub fn new(pos: Vec3) -> Self {
        Self {
            radius_m: 0.002,
            mass_kg:  1e-5,
            position: pos,
            velocity: Vec3::new(0.0, -5.0, 0.0),
            cycles:   0,
            ice_mass: 0.0,
            liquid_coat: 0.0,
        }
    }

    /// Grow by accreting supercooled liquid water.
    pub fn accrete(&mut self, liquid_water_content: f32, dt: f32) {
        let cross_section = std::f32::consts::PI * self.radius_m * self.radius_m;
        let collection_efficiency = 0.8_f32;
        let rel_speed = self.velocity.length();
        let added_mass = liquid_water_content * cross_section * collection_efficiency * rel_speed * dt;
        self.ice_mass  += added_mass;
        self.mass_kg   += added_mass;
        // Update radius
        let vol = self.mass_kg / 917.0;
        self.radius_m = (3.0 * vol / (4.0 * std::f32::consts::PI)).cbrt();
    }

    pub fn tick(&mut self, dt: f32, updraft: f32, wind: Vec3) {
        let phys = DropletPhysics::ice_particle(self.radius_m);
        let acc  = phys.acceleration(self.velocity, wind.add(Vec3::new(0.0, updraft, 0.0)));
        self.velocity = self.velocity.add(acc.scale(dt));
        self.position = self.position.add(self.velocity.scale(dt));
        // Count updraft cycle crossings
        if self.velocity.y > 0.0 { self.cycles += 0; } // already counting by altitude in system
    }

    pub fn diameter_mm(&self) -> f32 {
        self.radius_m * 2000.0
    }

    pub fn severity_class(&self) -> &'static str {
        let d = self.diameter_mm();
        if d < 5.0       { "Pea" }
        else if d < 19.0 { "Marble" }
        else if d < 38.0 { "Golf ball" }
        else if d < 64.0 { "Tennis ball" }
        else              { "Softball" }
    }
}

// ── Sleet Particle ────────────────────────────────────────────────────────────

/// A sleet particle (partially frozen raindrop).
#[derive(Debug, Clone, Copy)]
pub struct SleetParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub radius_m: f32,
    /// Ice fraction [0,1].
    pub ice_fraction: f32,
    pub lifetime: f32,
}

impl SleetParticle {
    pub fn new(pos: Vec3) -> Self {
        Self {
            position: pos,
            velocity: Vec3::new(0.0, -4.0, 0.0),
            radius_m: 0.002,
            ice_fraction: 0.5,
            lifetime: 10.0,
        }
    }

    pub fn tick(&mut self, dt: f32, temp_k: f32, wind: Vec3) {
        let phys = DropletPhysics::ice_particle(self.radius_m);
        let acc  = phys.acceleration(self.velocity, wind);
        self.velocity = self.velocity.add(acc.scale(dt));
        self.position = self.position.add(self.velocity.scale(dt));
        self.lifetime -= dt;
        // Freeze if cold enough
        if temp_k < 270.0 {
            self.ice_fraction = (self.ice_fraction + 0.1 * dt).min(1.0);
        }
    }
}

// ── Thunder & Lightning ───────────────────────────────────────────────────────

/// A cumulonimbus thunderstorm cell.
#[derive(Debug, Clone)]
pub struct ThunderCell {
    /// World position (x, z).
    pub position: [f32; 2],
    /// Altitude of the charge centre (m).
    pub charge_altitude: f32,
    /// Cloud base altitude (m).
    pub cloud_base_m: f32,
    /// Cloud top altitude (m).
    pub cloud_top_m: f32,
    /// Updraft speed (m/s).
    pub updraft: f32,
    /// Electric charge separation (Coulombs, simplified).
    pub charge: f32,
    /// Discharge threshold (Coulombs).
    pub discharge_threshold: f32,
    /// Active lightning bolts.
    pub bolts: Vec<LightningBolt>,
    /// Time since last discharge (s).
    pub time_since_discharge: f32,
    /// Average discharge interval (s).
    pub mean_discharge_interval: f32,
    /// Cell lifetime remaining (s).
    pub lifetime: f32,
    /// Cell intensity [0,1].
    pub intensity: f32,
    /// Random seed for this cell's internal variation.
    cell_seed: f32,
}

impl ThunderCell {
    pub fn new(cx: f32, cz: f32) -> Self {
        Self {
            position: [cx, cz],
            charge_altitude: 6_000.0,
            cloud_base_m: 1_500.0,
            cloud_top_m: 12_000.0,
            updraft: 25.0,
            charge: 0.0,
            discharge_threshold: 1.0,
            bolts: Vec::new(),
            time_since_discharge: 0.0,
            mean_discharge_interval: 30.0,
            lifetime: 7200.0,
            intensity: 0.0,
            cell_seed: cx.abs().fract() + cz.abs().fract(),
        }
    }

    pub fn tick(&mut self, dt: f32, humidity: f32, updraft_forcing: f32) {
        self.lifetime -= dt;
        self.time_since_discharge += dt;

        // Charge builds proportional to updraft and humidity
        let charge_rate = self.updraft * humidity * 0.002;
        self.charge += charge_rate * dt;
        self.intensity = (self.charge / self.discharge_threshold).clamp(0.0, 1.0);

        // Discharge when threshold is reached
        if self.charge >= self.discharge_threshold {
            self.discharge();
        }

        // Vary updraft
        let noise = value_noise_2d(self.cell_seed + self.time_since_discharge * 0.001, 0.0);
        self.updraft = (10.0 + updraft_forcing + noise * 20.0).max(0.0);

        // Decay old bolts
        self.bolts.retain(|b| b.is_alive());
        for bolt in &mut self.bolts {
            bolt.tick(dt);
        }

        // Decay intensity as cell ages
        let age_fraction = 1.0 - self.lifetime / 7200.0;
        if age_fraction > 0.7 {
            self.updraft *= 0.999;
        }
    }

    fn discharge(&mut self) {
        self.charge = 0.0;
        self.time_since_discharge = 0.0;
        // Create a lightning bolt
        let bolt = LightningBolt::new(
            Vec3::new(self.position[0], self.charge_altitude, self.position[1]),
            self.cloud_base_m,
            self.intensity,
        );
        self.bolts.push(bolt);
    }

    /// Distance to nearest active bolt strike point.
    pub fn nearest_strike_dist(&self, x: f32, z: f32) -> Option<f32> {
        self.bolts.iter()
            .filter(|b| b.is_alive() && b.struck)
            .map(|b| {
                let dx = b.strike_point.x - x;
                let dz = b.strike_point.z - z;
                (dx * dx + dz * dz).sqrt()
            })
            .reduce(f32::min)
    }

    /// Thunder delay (seconds) from strike at position (px, pz) to observer at (ox, oz).
    pub fn thunder_delay_s(strike_x: f32, strike_z: f32, obs_x: f32, obs_z: f32) -> f32 {
        let dx = strike_x - obs_x;
        let dz = strike_z - obs_z;
        let dist = (dx * dx + dz * dz).sqrt();
        dist / 343.0 // speed of sound
    }

    pub fn is_alive(&self) -> bool { self.lifetime > 0.0 }
}

// ── Lightning Bolt ────────────────────────────────────────────────────────────

/// A single lightning bolt with fractal channel geometry.
#[derive(Debug, Clone)]
pub struct LightningBolt {
    /// Starting point (usually in the cloud).
    pub origin: Vec3,
    /// Endpoint of the main channel.
    pub strike_point: Vec3,
    /// Intermediate vertices of the channel.
    pub channel: Vec<Vec3>,
    /// Whether the bolt has actually struck the ground.
    pub struck: bool,
    /// Remaining visible lifetime (s).
    pub visible_time: f32,
    /// Peak current (kA).
    pub peak_current_ka: f32,
    /// Number of return strokes.
    pub return_strokes: u32,
    /// Thunder intensity [0,1].
    pub thunder_intensity: f32,
}

impl LightningBolt {
    pub fn new(origin: Vec3, cloud_base: f32, intensity: f32) -> Self {
        let mut channel = Vec::new();
        // Build a simple fractal stepped leader
        let steps = 12usize;
        let mut pt = origin;
        let strike = Vec3::new(
            origin.x + (value_noise_2d(origin.x * 0.001, 0.5) * 2.0 - 1.0) * 2_000.0,
            cloud_base - 20.0,
            origin.z + (value_noise_2d(0.5, origin.z * 0.001) * 2.0 - 1.0) * 2_000.0,
        );
        channel.push(pt);
        for i in 1..steps {
            let t = i as f32 / steps as f32;
            let noise_x = (value_noise_2d(pt.x * 0.0001 + t, pt.z * 0.0001) * 2.0 - 1.0) * 300.0;
            let noise_z = (value_noise_2d(pt.z * 0.0001 + t + 7.0, pt.x * 0.0001) * 2.0 - 1.0) * 300.0;
            let next = Vec3::new(
                lerp(origin.x, strike.x, t) + noise_x,
                lerp(origin.y, strike.y, t),
                lerp(origin.z, strike.z, t) + noise_z,
            );
            channel.push(next);
            pt = next;
        }
        channel.push(strike);
        Self {
            origin,
            strike_point: strike,
            channel,
            struck: true,
            visible_time: 0.3,
            peak_current_ka: 20.0 + intensity * 80.0,
            return_strokes: 1 + (intensity * 3.0) as u32,
            thunder_intensity: intensity,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.visible_time -= dt;
    }

    pub fn is_alive(&self) -> bool { self.visible_time > 0.0 }

    /// Estimate danger radius (m) for lethal current.
    pub fn danger_radius_m(&self) -> f32 {
        self.peak_current_ka * 0.5
    }
}

// ── Precipitation Event ───────────────────────────────────────────────────────

/// A discrete precipitation event (for weather logging/callbacks).
#[derive(Debug, Clone)]
pub struct PrecipitationEvent {
    pub kind: PrecipitationType,
    pub intensity: f32,
    pub duration_s: f32,
    pub elapsed_s: f32,
    pub centre: [f32; 2],
    pub radius: f32,
}

impl PrecipitationEvent {
    pub fn progress(&self) -> f32 {
        (self.elapsed_s / self.duration_s).clamp(0.0, 1.0)
    }
    pub fn is_finished(&self) -> bool { self.elapsed_s >= self.duration_s }
}

// ── Precipitation System ──────────────────────────────────────────────────────

/// The main precipitation simulation state.
#[derive(Debug, Clone)]
pub struct PrecipitationSystem {
    pub config: PrecipitationConfig,
    pub droplets: Vec<Droplet>,
    pub rain_bands: Vec<RainBand>,
    pub surface: SurfaceAccumulation,
    pub puddles: Vec<Puddle>,
    pub snowpacks: HashMap<(i32, i32), Snowpack>,
    pub ice_sheets: HashMap<(i32, i32), IceSheet>,
    pub hailstones: Vec<HailStone>,
    pub sleet_particles: Vec<SleetParticle>,
    pub thunder_cells: Vec<ThunderCell>,
    pub events: Vec<PrecipitationEvent>,
    pub dominant_kind: PrecipitationType,
    pub global_intensity: f32,  // 0–1
    /// Spawn accumulator for droplets.
    spawn_accum: f32,
    /// Total precipitation fallen (m water equivalent).
    pub total_precip_m: f32,
    /// Noise offset drifting over time.
    noise_t: f32,
}

impl PrecipitationSystem {
    pub fn new() -> Self {
        Self::with_config(PrecipitationConfig::default())
    }

    pub fn with_config(config: PrecipitationConfig) -> Self {
        let surface = SurfaceAccumulation::new(config.grid_width, config.grid_depth, config.cell_size);
        Self {
            config,
            droplets: Vec::new(),
            rain_bands: Vec::new(),
            surface,
            puddles: Vec::new(),
            snowpacks: HashMap::new(),
            ice_sheets: HashMap::new(),
            hailstones: Vec::new(),
            sleet_particles: Vec::new(),
            thunder_cells: Vec::new(),
            events: Vec::new(),
            dominant_kind: PrecipitationType::None,
            global_intensity: 0.0,
            spawn_accum: 0.0,
            total_precip_m: 0.0,
            noise_t: 0.0,
        }
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    pub fn tick(&mut self, dt: f32, temp_c: f32, humidity: f32, wind: Vec3, pressure_pa: f32) {
        self.noise_t += dt * 0.01;

        // Determine precipitation type from temperature
        let kind = self.classify_precip(temp_c, humidity, pressure_pa);
        self.dominant_kind = kind;

        // Global intensity from humidity excess
        let excess = (humidity - self.config.rain_humidity_threshold).max(0.0);
        let target_intensity = smoothstep(0.0, 0.3, excess);
        self.global_intensity = lerp(self.global_intensity, target_intensity, dt * 0.1);

        // Spawn / manage rain bands
        self.update_rain_bands(dt, kind);

        // Spawn individual droplets (for VFX-level simulation)
        if self.global_intensity > 0.05 {
            self.spawn_droplets(dt, kind, wind, temp_c);
        }

        // Integrate droplets
        let surface_y = 0.0_f32;
        let mut land_events: Vec<(Vec3, PrecipitationType)> = Vec::new();
        for drop in &mut self.droplets {
            drop.tick(dt, wind, surface_y);
            if drop.landed {
                land_events.push((drop.position, drop.kind));
            }
        }
        self.droplets.retain(|d| d.is_alive());

        // Process landing events
        for (pos, kind) in land_events {
            self.handle_landing(pos, kind, temp_c);
        }

        // Update hailstones
        for h in &mut self.hailstones {
            h.tick(dt, 15.0, wind);
        }
        self.hailstones.retain(|h| h.position.y > 0.0);

        // Update sleet
        for s in &mut self.sleet_particles {
            let temp_k = temp_c + 273.15;
            s.tick(dt, temp_k, wind);
        }
        self.sleet_particles.retain(|s| s.position.y > 0.0 && s.lifetime > 0.0);

        // Update thunder cells
        let updraft = self.global_intensity * 20.0;
        for cell in &mut self.thunder_cells {
            cell.tick(dt, humidity, updraft);
        }
        self.thunder_cells.retain(|c| c.is_alive());

        // Surface water flow and evaporation
        self.surface.flow_water(dt);
        let evap = self.config.puddle_evap_rate * (1.0 + (temp_c * 0.05).max(0.0)) * dt;
        self.surface.evaporate(evap);

        // Melt or freeze snow
        if temp_c > 0.0 {
            self.surface.melt_snow(temp_c, dt);
        } else {
            self.surface.freeze_water(-temp_c, dt);
        }

        // Update snowpacks
        let temp_k = temp_c + 273.15;
        for sp in self.snowpacks.values_mut() {
            sp.tick(dt, temp_k);
        }
        self.snowpacks.retain(|_, sp| sp.total_depth() > 1e-6);

        // Update ice sheets
        for ice in self.ice_sheets.values_mut() {
            ice.tick(dt, temp_k);
        }
        self.ice_sheets.retain(|_, ice| ice.is_alive());

        // Update puddles
        let wind_spd = wind.length();
        for puddle in &mut self.puddles {
            puddle.tick(dt, temp_c, wind_spd);
        }
        self.puddles.retain(|p| p.is_alive());

        // Update events
        for ev in &mut self.events {
            ev.elapsed_s += dt;
        }
        self.events.retain(|e| !e.is_finished());

        // Total precipitation accumulation
        self.total_precip_m += self.global_intensity * 5e-6 * dt;
    }

    fn classify_precip(&self, temp_c: f32, humidity: f32, pressure_pa: f32) -> PrecipitationType {
        if humidity < self.config.rain_humidity_threshold { return PrecipitationType::None; }
        // High pressure suppresses precipitation
        if pressure_pa > 102_500.0 { return PrecipitationType::None; }
        if temp_c < self.config.hail_threshold_c { return PrecipitationType::Hail; }
        if temp_c < 0.0 { return PrecipitationType::Sleet; }
        if temp_c < self.config.snow_threshold_c { return PrecipitationType::Snow; }
        if humidity < 0.85 { return PrecipitationType::Drizzle; }
        PrecipitationType::Rain
    }

    fn update_rain_bands(&mut self, dt: f32, kind: PrecipitationType) {
        for band in &mut self.rain_bands {
            band.tick(dt);
        }
        self.rain_bands.retain(|b| b.is_alive());

        // Spawn a new band if intensity is high and we have few bands
        if self.global_intensity > 0.2 && self.rain_bands.len() < 4 {
            let cx = (value_noise_2d(self.noise_t, 0.3) * 2.0 - 1.0) * 5000.0;
            let cz = (value_noise_2d(0.7, self.noise_t + 1.0) * 2.0 - 1.0) * 5000.0;
            let band = RainBand::new(cx, cz, 3_000.0, self.global_intensity * 80.0, kind);
            self.rain_bands.push(band);
        }

        // Possibly spawn thunder cells
        if kind == PrecipitationType::Rain && self.global_intensity > 0.6 && self.thunder_cells.is_empty() {
            let cx = (value_noise_2d(self.noise_t * 1.3, 0.0) * 2.0 - 1.0) * 8000.0;
            let cz = (value_noise_2d(0.0, self.noise_t * 1.3 + 2.0) * 2.0 - 1.0) * 8000.0;
            self.thunder_cells.push(ThunderCell::new(cx, cz));
        }
    }

    fn spawn_droplets(&mut self, dt: f32, kind: PrecipitationType, wind: Vec3, temp_c: f32) {
        if self.droplets.len() >= self.config.max_droplets { return; }
        let spawn_rate = self.global_intensity * 50.0;
        self.spawn_accum += spawn_rate * dt;
        let to_spawn = self.spawn_accum as usize;
        self.spawn_accum -= to_spawn as f32;

        for i in 0..to_spawn {
            if self.droplets.len() >= self.config.max_droplets { break; }
            let offset_x = (value_noise_2d(self.noise_t + i as f32 * 0.1, 0.0) * 2.0 - 1.0) * 200.0;
            let offset_z = (value_noise_2d(0.0, self.noise_t + i as f32 * 0.1 + 50.0) * 2.0 - 1.0) * 200.0;
            let spawn_y  = 300.0 + value_noise_2d(self.noise_t, i as f32) * 200.0;
            let pos = Vec3::new(offset_x + wind.x * 2.0, spawn_y, offset_z + wind.z * 2.0);

            let drop = match kind {
                PrecipitationType::Rain | PrecipitationType::Drizzle => Droplet::new_rain(pos),
                PrecipitationType::Snow  => Droplet::new_snow(pos),
                PrecipitationType::Hail  => {
                    self.hailstones.push(HailStone::new(pos));
                    continue;
                }
                PrecipitationType::Sleet => {
                    self.sleet_particles.push(SleetParticle::new(pos));
                    continue;
                }
                PrecipitationType::None  => continue,
            };
            self.droplets.push(drop);
        }
    }

    fn handle_landing(&mut self, pos: Vec3, kind: PrecipitationType, temp_c: f32) {
        match kind {
            PrecipitationType::Rain | PrecipitationType::Drizzle => {
                let depth = 1e-5;
                self.surface.add_water(pos.x, pos.z, depth);
                self.total_precip_m += depth;
                // Form or grow puddles
                let cell_water = self.surface.water_at(pos.x, pos.z);
                if cell_water > 0.005 {
                    // Check if there's a nearby puddle
                    let mut found = false;
                    for puddle in &mut self.puddles {
                        if puddle.contains(pos.x, pos.z) {
                            puddle.add_water(1e-6);
                            found = true;
                            break;
                        }
                    }
                    if !found && self.puddles.len() < 256 {
                        self.puddles.push(Puddle::new(pos.x, pos.z, 1e-4));
                    }
                }
            }
            PrecipitationType::Snow => {
                let key = (
                    (pos.x / self.config.cell_size).floor() as i32,
                    (pos.z / self.config.cell_size).floor() as i32,
                );
                let sp = self.snowpacks.entry(key).or_insert_with(Snowpack::new);
                sp.deposit(5e-6, temp_c + 273.15);
                self.surface.add_snow(pos.x, pos.z, 5e-6);
                self.total_precip_m += 5e-6 * 0.1;
            }
            PrecipitationType::Sleet => {
                self.surface.add_ice(pos.x, pos.z, 1e-6);
                let key = (
                    (pos.x / self.config.cell_size).floor() as i32,
                    (pos.z / self.config.cell_size).floor() as i32,
                );
                self.ice_sheets.entry(key)
                    .or_insert_with(|| IceSheet::new(0.0, temp_c + 273.15, temp_c < -1.0))
                    .thickness_m += 1e-6;
            }
            _ => {}
        }
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Total precipitation intensity (0–1) at world position.
    pub fn intensity_at(&self, x: f32, z: f32) -> f32 {
        let base = self.global_intensity;
        let band_contrib: f32 = self.rain_bands.iter()
            .map(|b| b.intensity_at(x, z))
            .fold(0.0_f32, f32::max);
        let noise = fbm_2d(x * 0.0001 + self.noise_t, z * 0.0001, 3) * 0.2;
        (base * 0.5 + band_contrib * 0.5 + noise).clamp(0.0, 1.0)
    }

    pub fn dominant_type(&self) -> PrecipitationType {
        self.dominant_kind
    }

    /// Return the nearest thunder cell if within range.
    pub fn nearest_thunder_cell(&self, x: f32, z: f32, max_dist: f32) -> Option<&ThunderCell> {
        self.thunder_cells.iter().filter(|c| {
            let dx = c.position[0] - x;
            let dz = c.position[1] - z;
            (dx * dx + dz * dz).sqrt() < max_dist
        }).min_by(|a, b| {
            let da = { let dx = a.position[0]-x; let dz = a.position[1]-z; dx*dx+dz*dz };
            let db = { let dx = b.position[0]-x; let dz = b.position[1]-z; dx*dx+dz*dz };
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Manually trigger a precipitation event.
    pub fn trigger_event(&mut self, kind: PrecipitationType, intensity: f32, duration_s: f32, cx: f32, cz: f32, radius: f32) {
        self.events.push(PrecipitationEvent {
            kind,
            intensity,
            duration_s,
            elapsed_s: 0.0,
            centre: [cx, cz],
            radius,
        });
        self.rain_bands.push(RainBand::new(cx, cz, radius, intensity * 80.0, kind));
    }

    /// How much snow (m depth) is at world position.
    pub fn snow_depth_at(&self, x: f32, z: f32) -> f32 {
        self.surface.snow_at(x, z)
    }

    /// How much ice (m thickness) is at world position.
    pub fn ice_thickness_at(&self, x: f32, z: f32) -> f32 {
        self.surface.ice_at(x, z)
    }

    /// How much water (m depth) is at world position.
    pub fn water_depth_at(&self, x: f32, z: f32) -> f32 {
        self.surface.water_at(x, z)
    }
}

impl Default for PrecipitationSystem {
    fn default() -> Self { Self::new() }
}
