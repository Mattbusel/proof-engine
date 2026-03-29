
// ============================================================
// SECTION 23: PARTICLE ATLAS AND UV ANIMATION
// ============================================================

#[derive(Clone, Debug)]
pub struct AtlasRegion {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
    pub frame_index: u32,
}

pub struct ParticleAtlas {
    pub rows: u32,
    pub cols: u32,
    pub total_frames: u32,
    pub playback_fps: f32,
}

impl ParticleAtlas {
    pub fn new(rows: u32, cols: u32, fps: f32) -> Self {
        Self { rows, cols, total_frames: rows * cols, playback_fps: fps }
    }

    pub fn region_for_frame(&self, frame: u32) -> AtlasRegion {
        let f = frame % self.total_frames;
        let row = f / self.cols;
        let col = f % self.cols;
        let dv = 1.0 / self.rows as f32;
        let du = 1.0 / self.cols as f32;
        AtlasRegion {
            u_min: col as f32 * du,
            v_min: row as f32 * dv,
            u_max: (col + 1) as f32 * du,
            v_max: (row + 1) as f32 * dv,
            frame_index: f,
        }
    }

    pub fn frame_at_time(&self, t: f32) -> u32 {
        let frame = (t * self.playback_fps) as u32;
        frame % self.total_frames
    }

    pub fn region_at_time(&self, t: f32) -> AtlasRegion {
        self.region_for_frame(self.frame_at_time(t))
    }

    pub fn lerp_region(&self, t: f32) -> AtlasRegion {
        let exact = t * self.playback_fps;
        let frame_a = exact as u32 % self.total_frames;
        let frame_b = (frame_a + 1) % self.total_frames;
        let blend = exact.fract();
        let ra = self.region_for_frame(frame_a);
        let rb = self.region_for_frame(frame_b);
        AtlasRegion {
            u_min: ra.u_min + (rb.u_min - ra.u_min) * blend,
            v_min: ra.v_min + (rb.v_min - ra.v_min) * blend,
            u_max: ra.u_max + (rb.u_max - ra.u_max) * blend,
            v_max: ra.v_max + (rb.v_max - ra.v_max) * blend,
            frame_index: frame_a,
        }
    }
}

// ============================================================
// SECTION 24: EFFECT GRAPH / NODE GRAPH
// ============================================================

#[derive(Clone, Debug)]
pub struct EffectNode {
    pub id: u32,
    pub node_type: String,
    pub inputs: Vec<u32>,
    pub outputs: Vec<u32>,
    pub params: Vec<f32>,
}

pub struct EffectGraph {
    pub nodes: std::collections::HashMap<u32, EffectNode>,
    pub edges: Vec<(u32, u32)>,
    pub next_id: u32,
}

impl EffectGraph {
    pub fn new() -> Self { Self { nodes: std::collections::HashMap::new(), edges: Vec::new(), next_id: 1 } }

    pub fn add_node(&mut self, node_type: impl Into<String>, params: Vec<f32>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.insert(id, EffectNode { id, node_type: node_type.into(), inputs: Vec::new(), outputs: Vec::new(), params });
        id
    }

    pub fn connect(&mut self, from: u32, to: u32) {
        if let Some(n) = self.nodes.get_mut(&from) { n.outputs.push(to); }
        if let Some(n) = self.nodes.get_mut(&to) { n.inputs.push(from); }
        self.edges.push((from, to));
    }

    pub fn topological_sort(&self) -> Vec<u32> {
        let mut in_degree: std::collections::HashMap<u32, usize> = self.nodes.keys().map(|&k| (k, 0)).collect();
        for &(_, to) in &self.edges {
            *in_degree.entry(to).or_insert(0) += 1;
        }
        let mut queue: std::collections::VecDeque<u32> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(&k, _)| k).collect();
        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
            if let Some(node) = self.nodes.get(&id) {
                for &out in &node.outputs {
                    let d = in_degree.entry(out).or_insert(0);
                    *d = d.saturating_sub(1);
                    if *d == 0 { queue.push_back(out); }
                }
            }
        }
        order
    }

    pub fn evaluate(&self, inputs: &std::collections::HashMap<u32, f32>) -> std::collections::HashMap<u32, f32> {
        let mut values: std::collections::HashMap<u32, f32> = inputs.clone();
        for id in self.topological_sort() {
            if let Some(node) = self.nodes.get(&id) {
                let input_vals: Vec<f32> = node.inputs.iter().filter_map(|i| values.get(i)).copied().collect();
                let result = match node.node_type.as_str() {
                    "add" => input_vals.iter().sum(),
                    "mul" => input_vals.iter().product(),
                    "max" => input_vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
                    "min" => input_vals.iter().cloned().fold(f32::INFINITY, f32::min),
                    "const" => node.params.first().copied().unwrap_or(0.0),
                    _ => input_vals.first().copied().unwrap_or(0.0),
                };
                values.insert(id, result);
            }
        }
        values
    }
}

// ============================================================
// SECTION 25: BURST SCHEDULER
// ============================================================

#[derive(Clone, Debug)]
pub struct BurstEvent {
    pub time: f32,
    pub count: u32,
    pub probability: f32,
    pub fired: bool,
}

pub struct BurstScheduler {
    pub events: Vec<BurstEvent>,
    pub current_time: f32,
    pub rng_seed: u64,
}

impl BurstScheduler {
    pub fn new(seed: u64) -> Self { Self { events: Vec::new(), current_time: 0.0, rng_seed: seed } }

    pub fn add_burst(&mut self, time: f32, count: u32, probability: f32) {
        self.events.push(BurstEvent { time, count, probability, fired: false });
    }

    pub fn advance(&mut self, dt: f32) -> u32 {
        self.current_time += dt;
        let mut total = 0u32;
        let seed = self.rng_seed;
        for event in &mut self.events {
            if !event.fired && self.current_time >= event.time {
                // Simple LCG probability check
                let r = ((seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33) as f32) / (u32::MAX as f32);
                if r < event.probability {
                    total += event.count;
                }
                event.fired = true;
            }
        }
        total
    }

    pub fn reset(&mut self) {
        self.current_time = 0.0;
        for e in &mut self.events { e.fired = false; }
    }

    pub fn remaining_bursts(&self) -> usize {
        self.events.iter().filter(|e| !e.fired).count()
    }
}

// ============================================================
// SECTION 26: COLLIDER SHAPES FOR PARTICLE COLLISION
// ============================================================

use glam::{Vec2 as GlamVec2, Vec3 as GlamVec3};

#[derive(Clone, Debug)]
pub enum ColliderShape3D {
    Plane3 { normal: [f32; 3], offset: f32 },
    Sphere3D { center: [f32; 3], radius: f32 },
    Box3D { min: [f32; 3], max: [f32; 3] },
    Capsule3D { base: [f32; 3], tip: [f32; 3], radius: f32 },
}

impl ColliderShape3D {
    pub fn sdf(&self, p: [f32; 3]) -> f32 {
        match self {
            ColliderShape3D::Plane3 { normal, offset } => {
                normal[0]*p[0] + normal[1]*p[1] + normal[2]*p[2] - offset
            }
            ColliderShape3D::Sphere3D { center, radius } => {
                let dx = p[0]-center[0]; let dy = p[1]-center[1]; let dz = p[2]-center[2];
                (dx*dx + dy*dy + dz*dz).sqrt() - radius
            }
            ColliderShape3D::Box3D { min, max } => {
                let qx = (min[0]-p[0]).max(p[0]-max[0]);
                let qy = (min[1]-p[1]).max(p[1]-max[1]);
                let qz = (min[2]-p[2]).max(p[2]-max[2]);
                let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2) + qz.max(0.0).powi(2)).sqrt();
                let inside = qx.min(qy).min(qz).min(0.0);
                outside + inside
            }
            ColliderShape3D::Capsule3D { base, tip, radius } => {
                let ba = [tip[0]-base[0], tip[1]-base[1], tip[2]-base[2]];
                let pa = [p[0]-base[0], p[1]-base[1], p[2]-base[2]];
                let h = ((pa[0]*ba[0]+pa[1]*ba[1]+pa[2]*ba[2]) / (ba[0]*ba[0]+ba[1]*ba[1]+ba[2]*ba[2])).clamp(0.0, 1.0);
                let dx = pa[0]-ba[0]*h; let dy = pa[1]-ba[1]*h; let dz = pa[2]-ba[2]*h;
                (dx*dx + dy*dy + dz*dz).sqrt() - radius
            }
        }
    }

    pub fn contains(&self, p: [f32; 3]) -> bool { self.sdf(p) < 0.0 }
}

pub struct ParticleCollisionSystem {
    pub colliders: Vec<ColliderShape3D>,
    pub restitution: f32,
    pub friction: f32,
}

impl ParticleCollisionSystem {
    pub fn new(restitution: f32, friction: f32) -> Self {
        Self { colliders: Vec::new(), restitution, friction }
    }

    pub fn add_collider(&mut self, shape: ColliderShape3D) { self.colliders.push(shape); }

    pub fn resolve_particle(&self, pos: &mut [f32; 3], vel: &mut [f32; 3]) {
        for collider in &self.colliders {
            let d = collider.sdf(*pos);
            if d < 0.0 {
                // Push out by gradient approximation
                let eps = 0.001f32;
                let gx = collider.sdf([pos[0]+eps, pos[1], pos[2]]) - collider.sdf([pos[0]-eps, pos[1], pos[2]]);
                let gy = collider.sdf([pos[0], pos[1]+eps, pos[2]]) - collider.sdf([pos[0], pos[1]-eps, pos[2]]);
                let gz = collider.sdf([pos[0], pos[1], pos[2]+eps]) - collider.sdf([pos[0], pos[1], pos[2]-eps]);
                let len = (gx*gx + gy*gy + gz*gz).sqrt().max(1e-8);
                let nx = gx/len; let ny = gy/len; let nz = gz/len;
                pos[0] -= nx * d;
                pos[1] -= ny * d;
                pos[2] -= nz * d;
                let vdotn = vel[0]*nx + vel[1]*ny + vel[2]*nz;
                if vdotn < 0.0 {
                    vel[0] -= (1.0 + self.restitution) * vdotn * nx;
                    vel[1] -= (1.0 + self.restitution) * vdotn * ny;
                    vel[2] -= (1.0 + self.restitution) * vdotn * nz;
                    // Apply friction to tangential
                    let tx = vel[0] - vdotn*nx; let ty = vel[1] - vdotn*ny; let tz = vel[2] - vdotn*nz;
                    vel[0] -= tx * self.friction;
                    vel[1] -= ty * self.friction;
                    vel[2] -= tz * self.friction;
                }
            }
        }
    }
}

// ============================================================
// SECTION 27: RIBBON MESH BUILDER
// ============================================================

#[derive(Clone, Debug)]
pub struct RibbonVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub normal: [f32; 3],
}

pub struct RibbonMeshBuilder {
    pub vertices: Vec<RibbonVertex>,
    pub indices: Vec<u32>,
    pub width: f32,
    pub uv_tile: f32,
}

impl RibbonMeshBuilder {
    pub fn new(width: f32, uv_tile: f32) -> Self {
        Self { vertices: Vec::new(), indices: Vec::new(), width, uv_tile }
    }

    pub fn build_from_positions(&mut self, positions: &[[f32; 3]], colors: &[[f32; 4]]) {
        self.vertices.clear();
        self.indices.clear();
        if positions.len() < 2 { return; }
        let n = positions.len();
        for i in 0..n {
            let p = positions[i];
            let tangent = if i + 1 < n {
                let d = [positions[i+1][0]-p[0], positions[i+1][1]-p[1], positions[i+1][2]-p[2]];
                let len = (d[0]*d[0]+d[1]*d[1]+d[2]*d[2]).sqrt().max(1e-8);
                [d[0]/len, d[1]/len, d[2]/len]
            } else if i > 0 {
                let d = [p[0]-positions[i-1][0], p[1]-positions[i-1][1], p[2]-positions[i-1][2]];
                let len = (d[0]*d[0]+d[1]*d[1]+d[2]*d[2]).sqrt().max(1e-8);
                [d[0]/len, d[1]/len, d[2]/len]
            } else { [1.0, 0.0, 0.0] };

            let up = [0.0f32, 1.0, 0.0];
            let right = [
                tangent[1]*up[2] - tangent[2]*up[1],
                tangent[2]*up[0] - tangent[0]*up[2],
                tangent[0]*up[1] - tangent[1]*up[0],
            ];
            let rlen = (right[0]*right[0]+right[1]*right[1]+right[2]*right[2]).sqrt().max(1e-8);
            let right = [right[0]/rlen, right[1]/rlen, right[2]/rlen];

            let t = i as f32 / (n - 1) as f32;
            let color = if i < colors.len() { colors[i] } else { [1.0; 4] };
            let hw = self.width * 0.5;
            let normal = [tangent[1]*right[2]-tangent[2]*right[1], tangent[2]*right[0]-tangent[0]*right[2], tangent[0]*right[1]-tangent[1]*right[0]];

            self.vertices.push(RibbonVertex {
                position: [p[0]-right[0]*hw, p[1]-right[1]*hw, p[2]-right[2]*hw],
                uv: [0.0, t * self.uv_tile],
                color, normal,
            });
            self.vertices.push(RibbonVertex {
                position: [p[0]+right[0]*hw, p[1]+right[1]*hw, p[2]+right[2]*hw],
                uv: [1.0, t * self.uv_tile],
                color, normal,
            });
        }
        for i in 0..(n as u32 - 1) {
            let base = i * 2;
            self.indices.extend_from_slice(&[base, base+1, base+2, base+1, base+3, base+2]);
        }
    }

    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn index_count(&self) -> usize { self.indices.len() }
}

// ============================================================
// SECTION 28: LOD SELECTOR
// ============================================================

#[derive(Clone, Debug)]
pub struct LodLevel {
    pub level: u8,
    pub max_particles: usize,
    pub update_rate: f32,
    pub render_complexity: f32,
}

pub struct LodSelector {
    pub levels: Vec<LodLevel>,
    pub current_level: u8,
    pub hysteresis: f32,
}

impl LodSelector {
    pub fn new() -> Self {
        Self {
            levels: vec![
                LodLevel { level: 0, max_particles: 10000, update_rate: 1.0, render_complexity: 1.0 },
                LodLevel { level: 1, max_particles: 5000, update_rate: 0.5, render_complexity: 0.7 },
                LodLevel { level: 2, max_particles: 1000, update_rate: 0.25, render_complexity: 0.4 },
                LodLevel { level: 3, max_particles: 100, update_rate: 0.1, render_complexity: 0.1 },
            ],
            current_level: 0,
            hysteresis: 0.1,
        }
    }

    pub fn select(&mut self, distance: f32, screen_area: f32) -> &LodLevel {
        let score = screen_area / (1.0 + distance * 0.01);
        let new_level = if score > 0.8 { 0 } else if score > 0.4 { 1 } else if score > 0.1 { 2 } else { 3 };
        self.current_level = new_level;
        &self.levels[self.current_level as usize]
    }

    pub fn max_particles_for_distance(&self, dist: f32) -> usize {
        let idx = if dist < 10.0 { 0 } else if dist < 50.0 { 1 } else if dist < 200.0 { 2 } else { 3 };
        self.levels[idx.min(self.levels.len()-1)].max_particles
    }
}

// ============================================================
// SECTION 29: WIND SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct WindZone {
    pub center: [f32; 3],
    pub radius: f32,
    pub direction: [f32; 3],
    pub strength: f32,
    pub turbulence: f32,
    pub enabled: bool,
}

impl WindZone {
    pub fn new(center: [f32; 3], radius: f32, dir: [f32; 3], strength: f32) -> Self {
        Self { center, radius, direction: dir, strength, turbulence: 0.1, enabled: true }
    }

    pub fn wind_at(&self, pos: [f32; 3], time: f32) -> [f32; 3] {
        if !self.enabled { return [0.0; 3]; }
        let dx = pos[0]-self.center[0]; let dy = pos[1]-self.center[1]; let dz = pos[2]-self.center[2];
        let dist = (dx*dx+dy*dy+dz*dz).sqrt();
        if dist > self.radius { return [0.0; 3]; }
        let falloff = 1.0 - (dist / self.radius).clamp(0.0, 1.0);
        let noise = (pos[0]*0.1 + time*0.3).sin() * self.turbulence;
        [
            self.direction[0] * self.strength * falloff + noise,
            self.direction[1] * self.strength * falloff,
            self.direction[2] * self.strength * falloff + noise * 0.5,
        ]
    }
}

pub struct WindSystem {
    pub zones: Vec<WindZone>,
    pub global_wind: [f32; 3],
}

impl WindSystem {
    pub fn new() -> Self { Self { zones: Vec::new(), global_wind: [0.0; 3] } }

    pub fn add_zone(&mut self, zone: WindZone) { self.zones.push(zone); }

    pub fn set_global_wind(&mut self, dir: [f32; 3], strength: f32) {
        let len = (dir[0]*dir[0]+dir[1]*dir[1]+dir[2]*dir[2]).sqrt().max(1e-8);
        self.global_wind = [dir[0]/len*strength, dir[1]/len*strength, dir[2]/len*strength];
    }

    pub fn sample_wind(&self, pos: [f32; 3], time: f32) -> [f32; 3] {
        let mut w = self.global_wind;
        for zone in &self.zones {
            let zw = zone.wind_at(pos, time);
            w[0] += zw[0]; w[1] += zw[1]; w[2] += zw[2];
        }
        w
    }
}

// ============================================================
// SECTION 30: PARTICLE POOL WITH GENERATION COUNTER
// ============================================================

#[derive(Clone)]
pub struct PooledParticle {
    pub generation: u32,
    pub alive: bool,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub life: f32,
    pub max_life: f32,
    pub color: [f32; 4],
    pub size: f32,
}

impl Default for PooledParticle {
    fn default() -> Self {
        Self { generation: 0, alive: false, position: [0.0; 3], velocity: [0.0; 3], life: 0.0, max_life: 1.0, color: [1.0; 4], size: 1.0 }
    }
}

pub struct ParticlePoolGen {
    pub particles: Vec<PooledParticle>,
    pub free_list: Vec<usize>,
    pub active_count: usize,
    pub peak_count: usize,
}

impl ParticlePoolGen {
    pub fn new(capacity: usize) -> Self {
        let particles = vec![PooledParticle::default(); capacity];
        let free_list = (0..capacity).rev().collect();
        Self { particles, free_list, active_count: 0, peak_count: 0 }
    }

    pub fn spawn(&mut self, pos: [f32; 3], vel: [f32; 3], life: f32, color: [f32; 4], size: f32) -> Option<usize> {
        let idx = self.free_list.pop()?;
        let gen = self.particles[idx].generation + 1;
        self.particles[idx] = PooledParticle { generation: gen, alive: true, position: pos, velocity: vel, life, max_life: life, color, size };
        self.active_count += 1;
        if self.active_count > self.peak_count { self.peak_count = self.active_count; }
        Some(idx)
    }

    pub fn kill(&mut self, idx: usize) {
        if self.particles[idx].alive {
            self.particles[idx].alive = false;
            self.free_list.push(idx);
            self.active_count = self.active_count.saturating_sub(1);
        }
    }

    pub fn update(&mut self, dt: f32, gravity: [f32; 3]) {
        let mut to_kill = Vec::new();
        for (i, p) in self.particles.iter_mut().enumerate() {
            if !p.alive { continue; }
            p.life -= dt;
            if p.life <= 0.0 { to_kill.push(i); continue; }
            p.velocity[0] += gravity[0]*dt; p.velocity[1] += gravity[1]*dt; p.velocity[2] += gravity[2]*dt;
            p.position[0] += p.velocity[0]*dt; p.position[1] += p.velocity[1]*dt; p.position[2] += p.velocity[2]*dt;
        }
        for i in to_kill { self.kill(i); }
    }

    pub fn capacity(&self) -> usize { self.particles.len() }
    pub fn available(&self) -> usize { self.free_list.len() }
}

// ============================================================
// SECTION 31: VOLUMETRIC FOG
// ============================================================

pub struct VolumetricFogVolume {
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub density: f32,
    pub scatter_coeff: f32,
    pub absorb_coeff: f32,
    pub phase_g: f32,
    pub color: [f32; 3],
    pub height_falloff: f32,
    pub enabled: bool,
}

impl VolumetricFogVolume {
    pub fn new(min: [f32; 3], max: [f32; 3], density: f32) -> Self {
        Self { bounds_min: min, bounds_max: max, density, scatter_coeff: 0.1, absorb_coeff: 0.01, phase_g: 0.3, color: [0.8, 0.85, 0.9], height_falloff: 0.1, enabled: true }
    }

    pub fn density_at(&self, pos: [f32; 3]) -> f32 {
        if !self.enabled { return 0.0; }
        for i in 0..3 {
            if pos[i] < self.bounds_min[i] || pos[i] > self.bounds_max[i] { return 0.0; }
        }
        let height_above_base = (pos[1] - self.bounds_min[1]).max(0.0);
        self.density * (-self.height_falloff * height_above_base).exp()
    }

    pub fn henyey_greenstein(&self, cos_theta: f32) -> f32 {
        let g = self.phase_g;
        let denom = 1.0 + g*g - 2.0*g*cos_theta;
        (1.0 - g*g) / (4.0 * std::f32::consts::PI * denom.powf(1.5))
    }

    pub fn march_ray(&self, origin: [f32; 3], direction: [f32; 3], max_dist: f32, steps: u32) -> [f32; 4] {
        let step_size = max_dist / steps as f32;
        let mut transmittance = 1.0f32;
        let mut in_scatter = [0.0f32; 3];
        for i in 0..steps {
            let t = (i as f32 + 0.5) * step_size;
            let pos = [origin[0]+direction[0]*t, origin[1]+direction[1]*t, origin[2]+direction[2]*t];
            let d = self.density_at(pos);
            if d > 0.0 {
                let extinction = (self.scatter_coeff + self.absorb_coeff) * d * step_size;
                let step_transmittance = (-extinction).exp();
                let inscatter_amount = transmittance * (1.0 - step_transmittance) * self.scatter_coeff / (self.scatter_coeff + self.absorb_coeff);
                in_scatter[0] += inscatter_amount * self.color[0];
                in_scatter[1] += inscatter_amount * self.color[1];
                in_scatter[2] += inscatter_amount * self.color[2];
                transmittance *= step_transmittance;
            }
        }
        [in_scatter[0], in_scatter[1], in_scatter[2], transmittance]
    }
}

// ============================================================
// SECTION 32: PARTICLE NOISE FUNCTIONS
// ============================================================

fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
fn lerp_f(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }
fn grad3(hash: u32, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 { y } else if h == 12 || h == 14 { x } else { z };
    let u = if h & 1 != 0 { -u } else { u };
    let v = if h & 2 != 0 { -v } else { v };
    u + v
}
fn perm(x: u32) -> u32 { (x.wrapping_mul(16807).wrapping_add(0x12345678)) & 255 }

pub fn perlin_noise_3d(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32; let yi = y.floor() as i32; let zi = z.floor() as i32;
    let xf = x - x.floor(); let yf = y - y.floor(); let zf = z - z.floor();
    let u = fade(xf); let v = fade(yf); let w = fade(zf);
    let aaa = perm(perm(perm(xi as u32) + yi as u32) + zi as u32);
    let aba = perm(perm(perm(xi as u32) + (yi+1) as u32) + zi as u32);
    let aab = perm(perm(perm(xi as u32) + yi as u32) + (zi+1) as u32);
    let abb = perm(perm(perm(xi as u32) + (yi+1) as u32) + (zi+1) as u32);
    let baa = perm(perm(perm((xi+1) as u32) + yi as u32) + zi as u32);
    let bba = perm(perm(perm((xi+1) as u32) + (yi+1) as u32) + zi as u32);
    let bab = perm(perm(perm((xi+1) as u32) + yi as u32) + (zi+1) as u32);
    let bbb = perm(perm(perm((xi+1) as u32) + (yi+1) as u32) + (zi+1) as u32);
    let x1 = lerp_f(grad3(aaa,xf,yf,zf), grad3(baa,xf-1.0,yf,zf), u);
    let x2 = lerp_f(grad3(aba,xf,yf-1.0,zf), grad3(bba,xf-1.0,yf-1.0,zf), u);
    let y1 = lerp_f(x1, x2, v);
    let x3 = lerp_f(grad3(aab,xf,yf,zf-1.0), grad3(bab,xf-1.0,yf,zf-1.0), u);
    let x4 = lerp_f(grad3(abb,xf,yf-1.0,zf-1.0), grad3(bbb,xf-1.0,yf-1.0,zf-1.0), u);
    let y2 = lerp_f(x3, x4, v);
    lerp_f(y1, y2, w)
}

pub fn fbm_3d_noise(x: f32, y: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 0.5f32;
    let mut frequency = 1.0f32;
    for _ in 0..octaves {
        value += amplitude * perlin_noise_3d(x * frequency, y * frequency, z * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    value
}

pub fn worley_f1_3d(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32; let yi = y.floor() as i32; let zi = z.floor() as i32;
    let mut min_dist = f32::INFINITY;
    for dx in -1..=1i32 {
        for dy in -1..=1i32 {
            for dz in -1..=1i32 {
                let cx = xi + dx; let cy = yi + dy; let cz = zi + dz;
                let h = perm(perm(perm(cx as u32) + cy as u32) + cz as u32);
                let px = cx as f32 + (h as f32 / 255.0);
                let h2 = perm(h + 1); let h3 = perm(h + 2);
                let py = cy as f32 + (h2 as f32 / 255.0);
                let pz = cz as f32 + (h3 as f32 / 255.0);
                let dist = ((x-px)*(x-px)+(y-py)*(y-py)+(z-pz)*(z-pz)).sqrt();
                if dist < min_dist { min_dist = dist; }
            }
        }
    }
    min_dist
}

// ============================================================
// SECTION 33: PARTICLE PRESETS (EXTENDED)
// ============================================================

pub struct ParticlePresetLibrary {
    pub names: Vec<&'static str>,
    pub descriptions: Vec<&'static str>,
}

impl ParticlePresetLibrary {
    pub fn new() -> Self {
        Self {
            names: vec![
                "FireFlame", "WaterSplash", "MagicSparks", "ExplosionSmoke", "LeavesSwirl",
                "SnowFall", "RainDrop", "StarBurst", "DustCloud", "BloodSplatter",
                "IceShards", "LightningArc", "ManaOrb", "PoisonCloud", "HealingAura",
                "TeleportVortex", "MeteoricImpact", "SoulEscape", "HolyLight", "ShadowTendrils",
            ],
            descriptions: vec![
                "Upward flame with heat distortion", "Water droplets with splash rings",
                "Colorful arcane sparks", "Billowing smoke from explosion",
                "Autumn leaves in wind spiral", "Gentle falling snowflakes",
                "Rainfall with surface impact", "Radial starburst burst",
                "Settling dust cloud", "Gore splatter effect",
                "Sharp crystalline ice shards", "Branching lightning bolt",
                "Pulsing mana sphere", "Toxic gas cloud", "Green healing particles",
                "Swirling teleport vortex", "Impact shockwave with debris",
                "Spirit wisps rising", "Divine light beam", "Dark tendril spread",
            ],
        }
    }

    pub fn count(&self) -> usize { self.names.len() }

    pub fn name_at(&self, idx: usize) -> &'static str {
        self.names.get(idx).copied().unwrap_or("Unknown")
    }
}

// ============================================================
// SECTION 34: MANAGED PARTICLE SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum StopActionType { None, Destroy, Disable, Callback }

pub struct ManagedParticleSystemConfig {
    pub name: String,
    pub max_duration: f32,
    pub looping: bool,
    pub stop_action: StopActionType,
    pub auto_destroy_delay: f32,
}

impl ManagedParticleSystemConfig {
    pub fn new(name: impl Into<String>, duration: f32, looping: bool) -> Self {
        Self { name: name.into(), max_duration: duration, looping, stop_action: StopActionType::Destroy, auto_destroy_delay: 2.0 }
    }
}

pub struct ManagedParticleSystemInstance {
    pub config: ManagedParticleSystemConfig,
    pub pool: ParticlePoolGen,
    pub elapsed_time: f32,
    pub playing: bool,
    pub stopped: bool,
    pub wind_system: WindSystem,
}

impl ManagedParticleSystemInstance {
    pub fn new(config: ManagedParticleSystemConfig, capacity: usize) -> Self {
        Self { config, pool: ParticlePoolGen::new(capacity), elapsed_time: 0.0, playing: false, stopped: false, wind_system: WindSystem::new() }
    }

    pub fn play(&mut self) { self.playing = true; self.stopped = false; self.elapsed_time = 0.0; }
    pub fn stop(&mut self) { self.playing = false; self.stopped = true; }
    pub fn pause(&mut self) { self.playing = false; }

    pub fn update(&mut self, dt: f32) {
        if !self.playing { return; }
        self.elapsed_time += dt;
        let wind = self.wind_system.sample_wind([0.0; 3], self.elapsed_time);
        self.pool.update(dt, [wind[0], -9.81 + wind[1], wind[2]]);
        if !self.config.looping && self.elapsed_time >= self.config.max_duration {
            self.stop();
        }
    }

    pub fn active_particles(&self) -> usize { self.pool.active_count }
}

// ============================================================
// SECTION 35: PARTICLE SNAPSHOT / SERIALIZATION
// ============================================================

pub struct ParticleSnapshot {
    pub timestamp: f64,
    pub active_count: usize,
    pub peak_count: usize,
    pub system_name: String,
    pub positions: Vec<[f32; 3]>,
    pub velocities: Vec<[f32; 3]>,
}

impl ParticleSnapshot {
    pub fn capture(system: &ManagedParticleSystemInstance, timestamp: f64) -> Self {
        let alive: Vec<&PooledParticle> = system.pool.particles.iter().filter(|p| p.alive).collect();
        Self {
            timestamp,
            active_count: system.pool.active_count,
            peak_count: system.pool.peak_count,
            system_name: system.config.name.clone(),
            positions: alive.iter().map(|p| p.position).collect(),
            velocities: alive.iter().map(|p| p.velocity).collect(),
        }
    }

    pub fn to_json_string(&self) -> String {
        let pos_str: String = self.positions.iter()
            .map(|p| format!("[{:.3},{:.3},{:.3}]", p[0], p[1], p[2]))
            .collect::<Vec<_>>().join(",");
        format!(
            "{{\"timestamp\":{:.3},\"system\":\"{}\",\"active\":{},\"peak\":{},\"positions\":[{}]}}",
            self.timestamp, self.system_name, self.active_count, self.peak_count, pos_str
        )
    }
}

// ============================================================
// SECTION 36: EXTENDED PARTICLE EDITOR STATE
// ============================================================

pub struct ExtendedParticleEditorState {
    pub systems: Vec<ManagedParticleSystemInstance>,
    pub atlas: ParticleAtlas,
    pub lod_selector: LodSelector,
    pub preset_library: ParticlePresetLibrary,
    pub collision_system: ParticleCollisionSystem,
    pub snapshots: Vec<ParticleSnapshot>,
    pub global_time: f64,
}

impl ExtendedParticleEditorState {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            atlas: ParticleAtlas::new(4, 4, 24.0),
            lod_selector: LodSelector::new(),
            preset_library: ParticlePresetLibrary::new(),
            collision_system: ParticleCollisionSystem::new(0.3, 0.1),
            snapshots: Vec::new(),
            global_time: 0.0,
        }
    }

    pub fn add_system(&mut self, name: impl Into<String>, capacity: usize, duration: f32) -> usize {
        let cfg = ManagedParticleSystemConfig::new(name, duration, true);
        self.systems.push(ManagedParticleSystemInstance::new(cfg, capacity));
        self.systems.len() - 1
    }

    pub fn update_all(&mut self, dt: f32) {
        self.global_time += dt as f64;
        for sys in &mut self.systems {
            sys.update(dt);
        }
    }

    pub fn snapshot_all(&mut self) {
        let ts = self.global_time;
        let snaps: Vec<ParticleSnapshot> = self.systems.iter()
            .map(|s| ParticleSnapshot::capture(s, ts))
            .collect();
        self.snapshots.extend(snaps);
        if self.snapshots.len() > 100 {
            self.snapshots.drain(0..50);
        }
    }

    pub fn total_active_particles(&self) -> usize {
        self.systems.iter().map(|s| s.pool.active_count).sum()
    }
}

// ============================================================
// SECTION 37: PARTICLE EDITOR UNIT TESTS
// ============================================================

#[test]
fn test_particle_atlas() {
    let atlas = ParticleAtlas::new(4, 4, 24.0);
    let r = atlas.region_for_frame(0);
    assert!((r.u_max - 0.25).abs() < 1e-5);
    let r2 = atlas.region_at_time(1.0);
    assert!(r2.frame_index < 16);
}

#[test]
fn test_burst_scheduler() {
    let mut sched = BurstScheduler::new(12345);
    sched.add_burst(0.5, 10, 1.0);
    sched.add_burst(1.0, 20, 1.0);
    let spawned = sched.advance(0.6);
    assert_eq!(spawned, 10);
}

#[test]
fn test_particle_pool_gen() {
    let mut pool = ParticlePoolGen::new(100);
    let idx = pool.spawn([0.0;3], [0.0,1.0,0.0], 5.0, [1.0;4], 1.0);
    assert!(idx.is_some());
    assert_eq!(pool.active_count, 1);
    pool.update(1.0, [0.0,-9.81,0.0]);
    assert_eq!(pool.active_count, 1);
}

#[test]
fn test_lod_selector() {
    let mut sel = LodSelector::new();
    let lod = sel.select(100.0, 0.2);
    assert!(lod.level > 0);
}

#[test]
fn test_perlin_noise() {
    let n = perlin_noise_3d(1.23, 4.56, 7.89);
    assert!(n >= -2.0 && n <= 2.0);
    let fbm = fbm_3d_noise(0.5, 0.5, 0.5, 4);
    assert!(fbm >= -2.0 && fbm <= 2.0);
}

#[test]
fn test_ribbon_mesh() {
    let mut builder = RibbonMeshBuilder::new(1.0, 1.0);
    let positions = vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[2.0,0.0,0.0]];
    let colors = vec![[1.0,1.0,1.0,1.0];3];
    builder.build_from_positions(&positions, &colors);
    assert_eq!(builder.vertex_count(), 6);
    assert_eq!(builder.index_count(), 12);
}

#[test]
fn test_volumetric_fog() {
    let fog = VolumetricFogVolume::new([-5.0;3],[5.0;3],0.1);
    assert!(fog.density_at([0.0,0.0,0.0]) > 0.0);
    assert_eq!(fog.density_at([10.0,0.0,0.0]), 0.0);
}

pub fn particle_system_editor_version() -> &'static str {
    "ParticleSystemEditor v3.0 - Full Feature Set - 37 Sections"
}
