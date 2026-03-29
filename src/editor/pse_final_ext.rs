
// ============================================================
// SECTION 38: GPU PARTICLE PARAMS (COMPUTE SHADER DATA)
// ============================================================

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuParticleData {
    pub position: [f32; 4],
    pub velocity: [f32; 4],
    pub color: [f32; 4],
    pub life_size_rot: [f32; 4],  // x=life, y=max_life, z=size, w=rotation
    pub custom0: [f32; 4],
    pub custom1: [f32; 4],
}

impl Default for GpuParticleData {
    fn default() -> Self {
        Self { position: [0.0;4], velocity: [0.0;4], color: [1.0;4], life_size_rot: [1.0,1.0,1.0,0.0], custom0: [0.0;4], custom1: [0.0;4] }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuEmitterParams {
    pub spawn_rate: f32,
    pub max_particles: u32,
    pub delta_time: f32,
    pub simulation_time: f32,
    pub gravity: [f32; 4],
    pub wind: [f32; 4],
    pub turbulence_strength: f32,
    pub turbulence_scale: f32,
    pub drag_coefficient: f32,
    pub flags: u32,
}

impl Default for GpuEmitterParams {
    fn default() -> Self {
        Self { spawn_rate: 100.0, max_particles: 10000, delta_time: 0.016, simulation_time: 0.0, gravity: [0.0,-9.81,0.0,0.0], wind: [0.0;4], turbulence_strength: 0.1, turbulence_scale: 1.0, drag_coefficient: 0.01, flags: 0 }
    }
}

pub struct GpuParticleBuffer {
    pub data: Vec<GpuParticleData>,
    pub params: GpuEmitterParams,
    pub alive_count: u32,
    pub free_list: Vec<u32>,
}

impl GpuParticleBuffer {
    pub fn new(capacity: usize) -> Self {
        let data = vec![GpuParticleData::default(); capacity];
        let free_list = (0..capacity as u32).rev().collect();
        Self { data, params: GpuEmitterParams::default(), alive_count: 0, free_list }
    }

    pub fn spawn_particle(&mut self, pos: [f32; 4], vel: [f32; 4], color: [f32; 4], life: f32, size: f32) -> Option<u32> {
        let idx = self.free_list.pop()?;
        self.data[idx as usize] = GpuParticleData {
            position: pos, velocity: vel, color,
            life_size_rot: [life, life, size, 0.0],
            ..Default::default()
        };
        self.alive_count += 1;
        Some(idx)
    }

    pub fn as_bytes(&self) -> &[u8] {
        let ptr = self.data.as_ptr() as *const u8;
        let len = self.data.len() * std::mem::size_of::<GpuParticleData>();
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    pub fn capacity(&self) -> usize { self.data.len() }
    pub fn utilization(&self) -> f32 { self.alive_count as f32 / self.capacity() as f32 }
}

// ============================================================
// SECTION 39: EASING FUNCTIONS
// ============================================================

pub mod easing {
    pub fn linear(t: f32) -> f32 { t }
    pub fn ease_in_quad(t: f32) -> f32 { t * t }
    pub fn ease_out_quad(t: f32) -> f32 { t * (2.0 - t) }
    pub fn ease_in_out_quad(t: f32) -> f32 { if t < 0.5 { 2.0*t*t } else { -1.0 + (4.0-2.0*t)*t } }
    pub fn ease_in_cubic(t: f32) -> f32 { t * t * t }
    pub fn ease_out_cubic(t: f32) -> f32 { let t1 = t - 1.0; t1*t1*t1 + 1.0 }
    pub fn ease_in_out_cubic(t: f32) -> f32 { if t < 0.5 { 4.0*t*t*t } else { (t-1.0)*(2.0*t-2.0)*(2.0*t-2.0) + 1.0 } }
    pub fn ease_in_quart(t: f32) -> f32 { t * t * t * t }
    pub fn ease_out_quart(t: f32) -> f32 { 1.0 - (t-1.0).powi(4) }
    pub fn ease_in_sine(t: f32) -> f32 { 1.0 - (t * std::f32::consts::FRAC_PI_2).cos() }
    pub fn ease_out_sine(t: f32) -> f32 { (t * std::f32::consts::FRAC_PI_2).sin() }
    pub fn ease_in_out_sine(t: f32) -> f32 { -(((std::f32::consts::PI * t).cos() - 1.0) / 2.0) }
    pub fn ease_in_expo(t: f32) -> f32 { if t == 0.0 { 0.0 } else { 2.0f32.powf(10.0 * t - 10.0) } }
    pub fn ease_out_expo(t: f32) -> f32 { if t == 1.0 { 1.0 } else { 1.0 - 2.0f32.powf(-10.0 * t) } }
    pub fn ease_in_circ(t: f32) -> f32 { 1.0 - (1.0 - t*t).sqrt() }
    pub fn ease_out_circ(t: f32) -> f32 { ((1.0-(t-1.0)*(t-1.0))).sqrt() }
    pub fn bounce_out(t: f32) -> f32 {
        if t < 1.0/2.75 { 7.5625*t*t }
        else if t < 2.0/2.75 { let t = t - 1.5/2.75; 7.5625*t*t + 0.75 }
        else if t < 2.5/2.75 { let t = t - 2.25/2.75; 7.5625*t*t + 0.9375 }
        else { let t = t - 2.625/2.75; 7.5625*t*t + 0.984375 }
    }
    pub fn elastic_out(t: f32) -> f32 {
        if t == 0.0 || t == 1.0 { return t; }
        let c4 = (2.0 * std::f32::consts::PI) / 3.0;
        2.0f32.powf(-10.0*t) * ((t*10.0-0.75)*c4).sin() + 1.0
    }
    pub fn evaluate(name: &str, t: f32) -> f32 {
        match name {
            "linear" => linear(t), "ease_in_quad" => ease_in_quad(t),
            "ease_out_quad" => ease_out_quad(t), "ease_in_cubic" => ease_in_cubic(t),
            "ease_out_cubic" => ease_out_cubic(t), "ease_in_sine" => ease_in_sine(t),
            "ease_out_sine" => ease_out_sine(t), "bounce_out" => bounce_out(t),
            "elastic_out" => elastic_out(t), _ => linear(t),
        }
    }
}

// ============================================================
// SECTION 40: PARTICLE DECAL SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleDecal {
    pub id: u32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub atlas_frame: u32,
}

impl ParticleDecal {
    pub fn new(id: u32, pos: [f32; 3], normal: [f32; 3], size: f32, life: f32) -> Self {
        Self { id, position: pos, normal, size, rotation: 0.0, color: [1.0;4], lifetime: life, max_lifetime: life, atlas_frame: 0 }
    }

    pub fn age_fraction(&self) -> f32 { 1.0 - (self.lifetime / self.max_lifetime).clamp(0.0, 1.0) }
    pub fn is_alive(&self) -> bool { self.lifetime > 0.0 }

    pub fn update(&mut self, dt: f32) {
        self.lifetime -= dt;
        let age = self.age_fraction();
        self.color[3] = (1.0 - age * age).max(0.0);
    }
}

pub struct DecalManager {
    pub decals: Vec<ParticleDecal>,
    pub max_decals: usize,
    pub next_id: u32,
}

impl DecalManager {
    pub fn new(max_decals: usize) -> Self { Self { decals: Vec::new(), max_decals, next_id: 1 } }

    pub fn spawn_decal(&mut self, pos: [f32; 3], normal: [f32; 3], size: f32, life: f32) -> u32 {
        if self.decals.len() >= self.max_decals {
            // Remove oldest
            self.decals.remove(0);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.decals.push(ParticleDecal::new(id, pos, normal, size, life));
        id
    }

    pub fn update(&mut self, dt: f32) {
        for d in &mut self.decals { d.update(dt); }
        self.decals.retain(|d| d.is_alive());
    }

    pub fn active_count(&self) -> usize { self.decals.len() }
}

// ============================================================
// SECTION 41: SPH FLUID SIMULATION (SECONDARY)
// ============================================================

pub struct SphFluid2 {
    pub positions: Vec<[f32; 3]>,
    pub velocities: Vec<[f32; 3]>,
    pub densities: Vec<f32>,
    pub pressures: Vec<f32>,
    pub mass: f32,
    pub smoothing_radius: f32,
    pub rest_density: f32,
    pub pressure_constant: f32,
    pub viscosity: f32,
}

impl SphFluid2 {
    pub fn new(mass: f32, h: f32, rest_density: f32, k: f32, visc: f32) -> Self {
        Self { positions: Vec::new(), velocities: Vec::new(), densities: Vec::new(), pressures: Vec::new(), mass, smoothing_radius: h, rest_density, pressure_constant: k, viscosity: visc }
    }

    pub fn add_particle(&mut self, pos: [f32; 3]) {
        self.positions.push(pos);
        self.velocities.push([0.0; 3]);
        self.densities.push(0.0);
        self.pressures.push(0.0);
    }

    fn poly6_kernel(&self, r_sq: f32) -> f32 {
        let h2 = self.smoothing_radius * self.smoothing_radius;
        if r_sq > h2 { return 0.0; }
        let diff = h2 - r_sq;
        (315.0 / (64.0 * std::f32::consts::PI * self.smoothing_radius.powi(9))) * diff * diff * diff
    }

    pub fn compute_densities(&mut self) {
        let n = self.positions.len();
        for i in 0..n {
            let mut rho = 0.0f32;
            for j in 0..n {
                let dx = self.positions[i][0]-self.positions[j][0];
                let dy = self.positions[i][1]-self.positions[j][1];
                let dz = self.positions[i][2]-self.positions[j][2];
                let r_sq = dx*dx + dy*dy + dz*dz;
                rho += self.mass * self.poly6_kernel(r_sq);
            }
            self.densities[i] = rho;
            self.pressures[i] = self.pressure_constant * (rho - self.rest_density).max(0.0);
        }
    }

    pub fn step(&mut self, dt: f32, gravity: [f32; 3]) {
        if self.positions.is_empty() { return; }
        self.compute_densities();
        let n = self.positions.len();
        let mut forces: Vec<[f32; 3]> = vec![[0.0; 3]; n];
        for i in 0..n {
            forces[i][0] += gravity[0] * self.densities[i];
            forces[i][1] += gravity[1] * self.densities[i];
            forces[i][2] += gravity[2] * self.densities[i];
        }
        for i in 0..n {
            if self.densities[i] > 1e-8 {
                self.velocities[i][0] += forces[i][0] / self.densities[i] * dt;
                self.velocities[i][1] += forces[i][1] / self.densities[i] * dt;
                self.velocities[i][2] += forces[i][2] / self.densities[i] * dt;
            }
            self.positions[i][0] += self.velocities[i][0] * dt;
            self.positions[i][1] += self.velocities[i][1] * dt;
            self.positions[i][2] += self.velocities[i][2] * dt;
        }
    }

    pub fn particle_count(&self) -> usize { self.positions.len() }
}

// ============================================================
// SECTION 42: PARTICLE BENCHMARK
// ============================================================

pub struct BenchmarkResult {
    pub update_us: u64,
    pub render_prep_us: u64,
    pub particle_count: usize,
    pub throughput_mpp: f32,
}

pub fn benchmark_particle_pool(capacity: usize, spawn_count: usize) -> BenchmarkResult {
    use std::time::Instant;
    let mut pool = ParticlePoolGen::new(capacity);
    let t0 = Instant::now();
    for i in 0..spawn_count.min(capacity) {
        let fi = i as f32;
        pool.spawn([fi, 0.0, 0.0], [0.0, 1.0, 0.0], 10.0, [1.0;4], 1.0);
    }
    pool.update(0.016, [0.0, -9.81, 0.0]);
    let update_us = t0.elapsed().as_micros() as u64;
    let throughput_mpp = pool.active_count as f32 / update_us.max(1) as f32;
    BenchmarkResult { update_us, render_prep_us: 0, particle_count: pool.active_count, throughput_mpp }
}

// ============================================================
// SECTION 43: PARTICLE EVENT QUEUE
// ============================================================

#[derive(Clone, Debug)]
pub enum ParticleEventKind {
    ParticleDied { index: usize },
    CollisionDetected { particle_idx: usize, collider_idx: usize },
    EmitterStarted,
    EmitterStopped,
    BurstFired { count: u32 },
}

#[derive(Clone, Debug)]
pub struct ParticleEvent {
    pub kind: ParticleEventKind,
    pub time: f64,
}

pub struct ParticleEventQueue {
    pub events: std::collections::VecDeque<ParticleEvent>,
    pub capacity: usize,
}

impl ParticleEventQueue {
    pub fn new(capacity: usize) -> Self { Self { events: std::collections::VecDeque::new(), capacity } }

    pub fn push(&mut self, kind: ParticleEventKind, time: f64) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(ParticleEvent { kind, time });
    }

    pub fn drain_up_to(&mut self, max_time: f64) -> Vec<ParticleEvent> {
        let mut result = Vec::new();
        while let Some(e) = self.events.front() {
            if e.time <= max_time {
                result.push(self.events.pop_front().unwrap());
            } else { break; }
        }
        result
    }

    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
}

// ============================================================
// SECTION 44: FRUSTUM CULLING FOR PARTICLES
// ============================================================

pub struct FrustumPlane {
    pub normal: [f32; 3],
    pub distance: f32,
}

impl FrustumPlane {
    pub fn signed_distance(&self, point: [f32; 3]) -> f32 {
        self.normal[0]*point[0] + self.normal[1]*point[1] + self.normal[2]*point[2] + self.distance
    }
}

pub struct ParticleFrustum {
    pub planes: [FrustumPlane; 6],
}

impl ParticleFrustum {
    pub fn from_view_proj(m: [[f32; 4]; 4]) -> Self {
        let row = |i: usize| [m[0][i], m[1][i], m[2][i], m[3][i]];
        let r0 = row(0); let r1 = row(1); let r2 = row(2); let r3 = row(3);
        let plane = |a: [f32;4], b: [f32;4], sign: f32| {
            let nx = a[0] + sign*b[0];
            let ny = a[1] + sign*b[1];
            let nz = a[2] + sign*b[2];
            let nd = a[3] + sign*b[3];
            let len = (nx*nx+ny*ny+nz*nz).sqrt().max(1e-8);
            FrustumPlane { normal: [nx/len, ny/len, nz/len], distance: nd/len }
        };
        Self {
            planes: [
                plane(r3, r0, 1.0), plane(r3, r0, -1.0),
                plane(r3, r1, 1.0), plane(r3, r1, -1.0),
                plane(r3, r2, 1.0), plane(r3, r2, -1.0),
            ]
        }
    }

    pub fn test_sphere(&self, center: [f32; 3], radius: f32) -> bool {
        for plane in &self.planes {
            if plane.signed_distance(center) < -radius { return false; }
        }
        true
    }

    pub fn cull_particles(&self, positions: &[[f32; 3]], radius: f32) -> Vec<usize> {
        positions.iter().enumerate()
            .filter(|(_, &pos)| self.test_sphere(pos, radius))
            .map(|(i, _)| i)
            .collect()
    }
}

// ============================================================
// SECTION 45: FINAL PARTICLE SYSTEM TESTS
// ============================================================

#[test]
fn test_gpu_particle_buffer() {
    let mut buf = GpuParticleBuffer::new(1000);
    let idx = buf.spawn_particle([0.0;4], [0.0,1.0,0.0,0.0], [1.0;4], 5.0, 1.0);
    assert!(idx.is_some());
    assert_eq!(buf.alive_count, 1);
    assert!(buf.utilization() > 0.0);
}

#[test]
fn test_decal_manager() {
    let mut dm = DecalManager::new(50);
    let id = dm.spawn_decal([0.0;3], [0.0,1.0,0.0], 1.0, 5.0);
    assert!(id > 0);
    dm.update(1.0);
    assert_eq!(dm.active_count(), 1);
}

#[test]
fn test_easing_functions() {
    assert!((easing::linear(0.5) - 0.5).abs() < 1e-6);
    assert!((easing::ease_in_quad(1.0) - 1.0).abs() < 1e-6);
    assert!((easing::ease_out_quad(0.0) - 0.0).abs() < 1e-6);
    assert!(easing::bounce_out(0.5) >= 0.0);
}

#[test]
fn test_sph_fluid() {
    let mut fluid = SphFluid2::new(1.0, 0.1, 1000.0, 100.0, 0.1);
    for i in 0..5 {
        fluid.add_particle([i as f32 * 0.05, 0.0, 0.0]);
    }
    fluid.step(0.01, [0.0, -9.81, 0.0]);
    assert_eq!(fluid.particle_count(), 5);
}

#[test]
fn test_particle_event_queue() {
    let mut q = ParticleEventQueue::new(100);
    q.push(ParticleEventKind::EmitterStarted, 0.0);
    q.push(ParticleEventKind::BurstFired { count: 10 }, 0.5);
    let events = q.drain_up_to(0.3);
    assert_eq!(events.len(), 1);
}

pub fn particle_system_editor_extended_version() -> &'static str {
    "ParticleSystemEditor v3.1 - Extended - 45 Sections"
}
