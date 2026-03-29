
// ============================================================
// SECTION 51: PARTICLE COLOR GRADIENT
// ============================================================

#[derive(Clone, Debug)]
pub struct ColorKey {
    pub time: f32,
    pub color: [f32; 4],
}

pub struct ParticleColorGradient {
    pub keys: Vec<ColorKey>,
}

impl ParticleColorGradient {
    pub fn new() -> Self { Self { keys: Vec::new() } }

    pub fn add_key(&mut self, time: f32, color: [f32; 4]) {
        let pos = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(pos, ColorKey { time, color });
    }

    pub fn evaluate(&self, t: f32) -> [f32; 4] {
        if self.keys.is_empty() { return [1.0; 4]; }
        if self.keys.len() == 1 { return self.keys[0].color; }
        let t = t.clamp(0.0, 1.0);
        if t <= self.keys[0].time { return self.keys[0].color; }
        if t >= self.keys.last().unwrap().time { return self.keys.last().unwrap().color; }
        let idx = self.keys.partition_point(|k| k.time <= t).saturating_sub(1);
        let next = (idx + 1).min(self.keys.len() - 1);
        let a = &self.keys[idx];
        let b = &self.keys[next];
        let span = b.time - a.time;
        let blend = if span > 1e-8 { (t - a.time) / span } else { 0.0 };
        [
            a.color[0] + (b.color[0]-a.color[0])*blend,
            a.color[1] + (b.color[1]-a.color[1])*blend,
            a.color[2] + (b.color[2]-a.color[2])*blend,
            a.color[3] + (b.color[3]-a.color[3])*blend,
        ]
    }

    pub fn sample_over_lifetime(&self, n: usize) -> Vec<[f32; 4]> {
        (0..n).map(|i| self.evaluate(i as f32 / (n-1).max(1) as f32)).collect()
    }
}

// ============================================================
// SECTION 52: PARTICLE SIZE CURVE
// ============================================================

#[derive(Clone, Debug)]
pub struct CurveKey {
    pub time: f32,
    pub value: f32,
    pub tangent_in: f32,
    pub tangent_out: f32,
}

pub struct ParticleSizeCurve {
    pub keys: Vec<CurveKey>,
}

impl ParticleSizeCurve {
    pub fn new() -> Self { Self { keys: Vec::new() } }

    pub fn add_key(&mut self, time: f32, value: f32) {
        let pos = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(pos, CurveKey { time, value, tangent_in: 0.0, tangent_out: 0.0 });
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 1.0; }
        if self.keys.len() == 1 { return self.keys[0].value; }
        let t = t.clamp(0.0, 1.0);
        if t <= self.keys[0].time { return self.keys[0].value; }
        if t >= self.keys.last().unwrap().time { return self.keys.last().unwrap().value; }
        let idx = self.keys.partition_point(|k| k.time <= t).saturating_sub(1);
        let next = (idx + 1).min(self.keys.len() - 1);
        let a = &self.keys[idx];
        let b = &self.keys[next];
        let span = b.time - a.time;
        let u = if span > 1e-8 { (t - a.time) / span } else { 0.0 };
        // Hermite interpolation
        let h00 = 2.0*u*u*u - 3.0*u*u + 1.0;
        let h10 = u*u*u - 2.0*u*u + u;
        let h01 = -2.0*u*u*u + 3.0*u*u;
        let h11 = u*u*u - u*u;
        h00*a.value + h10*span*a.tangent_out + h01*b.value + h11*span*b.tangent_in
    }
}

// ============================================================
// SECTION 53: TRAIL RENDERER
// ============================================================

#[derive(Clone, Debug)]
pub struct TrailPoint {
    pub position: [f32; 3],
    pub width: f32,
    pub color: [f32; 4],
    pub time: f32,
}

pub struct TrailRenderer {
    pub points: std::collections::VecDeque<TrailPoint>,
    pub max_points: usize,
    pub lifetime: f32,
    pub min_vertex_distance: f32,
    pub width_curve: ParticleSizeCurve,
    pub color_gradient: ParticleColorGradient,
}

impl TrailRenderer {
    pub fn new(max_points: usize, lifetime: f32) -> Self {
        let mut wc = ParticleSizeCurve::new();
        wc.add_key(0.0, 1.0);
        wc.add_key(1.0, 0.0);
        let mut cg = ParticleColorGradient::new();
        cg.add_key(0.0, [1.0, 1.0, 1.0, 1.0]);
        cg.add_key(1.0, [1.0, 1.0, 1.0, 0.0]);
        Self { points: std::collections::VecDeque::new(), max_points, lifetime, min_vertex_distance: 0.01, width_curve: wc, color_gradient: cg }
    }

    pub fn add_point(&mut self, pos: [f32; 3], time: f32, width: f32) {
        if let Some(last) = self.points.back() {
            let dx = pos[0]-last.position[0]; let dy = pos[1]-last.position[1]; let dz = pos[2]-last.position[2];
            if (dx*dx+dy*dy+dz*dz).sqrt() < self.min_vertex_distance { return; }
        }
        if self.points.len() >= self.max_points { self.points.pop_front(); }
        self.points.push_back(TrailPoint { position: pos, width, color: [1.0;4], time });
    }

    pub fn update(&mut self, current_time: f32) {
        while let Some(front) = self.points.front() {
            if current_time - front.time > self.lifetime { self.points.pop_front(); } else { break; }
        }
        let n = self.points.len();
        let lt = self.lifetime;
        let ct = current_time;
        let keys_colors: Vec<(f32, [f32;4])> = self.points.iter().enumerate().map(|(i, p)| {
            let age_frac = (ct - p.time) / lt.max(1e-8);
            let t = 1.0 - age_frac.clamp(0.0,1.0);
            (t, [1.0,1.0,1.0,t])
        }).collect();
        for (i, p) in self.points.iter_mut().enumerate() {
            p.color = keys_colors[i].1;
        }
    }

    pub fn point_count(&self) -> usize { self.points.len() }
}

// ============================================================
// SECTION 54: PARTICLE MATERIAL PROPERTIES
// ============================================================

#[derive(Clone, Debug)]
pub enum ParticleBlendMode {
    Additive,
    AlphaBlend,
    Premultiplied,
    Multiply,
    Screen,
}

#[derive(Clone, Debug)]
pub struct ParticleMaterial {
    pub texture_id: u32,
    pub normal_map_id: Option<u32>,
    pub blend_mode: ParticleBlendMode,
    pub emission_strength: f32,
    pub soft_particles_distance: f32,
    pub distortion_strength: f32,
    pub receive_shadows: bool,
    pub cast_shadows: bool,
    pub render_queue: i32,
}

impl Default for ParticleMaterial {
    fn default() -> Self {
        Self { texture_id: 0, normal_map_id: None, blend_mode: ParticleBlendMode::AlphaBlend, emission_strength: 1.0, soft_particles_distance: 1.0, distortion_strength: 0.0, receive_shadows: false, cast_shadows: false, render_queue: 3000 }
    }
}

impl ParticleMaterial {
    pub fn additive(texture_id: u32) -> Self {
        Self { texture_id, blend_mode: ParticleBlendMode::Additive, emission_strength: 2.0, ..Default::default() }
    }
    pub fn alpha_blend(texture_id: u32) -> Self {
        Self { texture_id, ..Default::default() }
    }
}

// ============================================================
// SECTION 55: PARTICLE SYSTEM REGISTRY
// ============================================================

pub struct ParticleSystemRegistry {
    pub systems: std::collections::HashMap<String, ManagedParticleSystemConfig>,
}

impl ParticleSystemRegistry {
    pub fn new() -> Self { Self { systems: std::collections::HashMap::new() } }

    pub fn register(&mut self, cfg: ManagedParticleSystemConfig) {
        self.systems.insert(cfg.name.clone(), cfg);
    }

    pub fn instantiate(&self, name: &str, capacity: usize) -> Option<ManagedParticleSystemInstance> {
        self.systems.get(name).map(|cfg| {
            let new_cfg = ManagedParticleSystemConfig {
                name: cfg.name.clone(),
                max_duration: cfg.max_duration,
                looping: cfg.looping,
                stop_action: cfg.stop_action.clone(),
                auto_destroy_delay: cfg.auto_destroy_delay,
            };
            ManagedParticleSystemInstance::new(new_cfg, capacity)
        })
    }

    pub fn registered_names(&self) -> Vec<&String> {
        let mut names: Vec<&String> = self.systems.keys().collect();
        names.sort();
        names
    }

    pub fn count(&self) -> usize { self.systems.len() }
}

// ============================================================
// SECTION 56: PARTICLE SYSTEM FINAL TESTS AND VERSION
// ============================================================

#[test]
fn test_color_gradient() {
    let mut g = ParticleColorGradient::new();
    g.add_key(0.0, [1.0, 0.0, 0.0, 1.0]);
    g.add_key(1.0, [0.0, 0.0, 1.0, 1.0]);
    let mid = g.evaluate(0.5);
    assert!((mid[0] - 0.5).abs() < 1e-5);
    assert!((mid[2] - 0.5).abs() < 1e-5);
}

#[test]
fn test_size_curve() {
    let mut c = ParticleSizeCurve::new();
    c.add_key(0.0, 1.0);
    c.add_key(1.0, 0.0);
    let v = c.evaluate(0.5);
    assert!(v >= 0.0 && v <= 1.0);
}

#[test]
fn test_trail_renderer() {
    let mut trail = TrailRenderer::new(50, 3.0);
    trail.add_point([0.0,0.0,0.0], 0.0, 1.0);
    trail.add_point([1.0,0.0,0.0], 0.1, 1.0);
    trail.add_point([2.0,0.0,0.0], 0.2, 1.0);
    trail.update(0.5);
    assert!(trail.point_count() > 0);
}

#[test]
fn test_particle_registry() {
    let mut reg = ParticleSystemRegistry::new();
    reg.register(ManagedParticleSystemConfig::new("Fireball", 5.0, false));
    reg.register(ManagedParticleSystemConfig::new("Sparkle", 2.0, true));
    assert_eq!(reg.count(), 2);
    let inst = reg.instantiate("Fireball", 100);
    assert!(inst.is_some());
}

pub fn particle_system_complete_version() -> &'static str {
    "ParticleSystemEditor v3.3 - 56 Sections - Production Ready"
}
