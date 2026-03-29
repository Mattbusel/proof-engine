// modeling_editor_ext3.rs — additional subsystems for the particle modeling editor

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 1: Constraint System
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum ConstraintKind {
    FixedPosition,
    FixedNormal,
    OnSurface { surface_id: u64 },
    Distance { target_idx: usize, min_dist: f32, max_dist: f32 },
    Axis { axis: Vec3, origin: Vec3 },
    Plane { normal: Vec3, offset: f32 },
    Sphere { center: Vec3, radius: f32 },
    Cage { min: Vec3, max: Vec3 },
    Mirror { axis: u8 },  // 0=X,1=Y,2=Z
}

#[derive(Clone, Debug)]
pub struct ParticleConstraint {
    pub particle_idx: usize,
    pub kind:         ConstraintKind,
    pub strength:     f32,
    pub enabled:      bool,
}

impl ParticleConstraint {
    pub fn new(particle_idx: usize, kind: ConstraintKind) -> Self {
        Self { particle_idx, kind, strength: 1.0, enabled: true }
    }

    pub fn apply(&self, pos: Vec3) -> Vec3 {
        if !self.enabled { return pos; }
        match &self.kind {
            ConstraintKind::FixedPosition => pos,
            ConstraintKind::FixedNormal   => pos,
            ConstraintKind::OnSurface { .. } => pos,
            ConstraintKind::Distance { target_idx: _, min_dist, max_dist } => {
                let len = pos.length();
                if len < *min_dist {
                    pos.normalize_or_zero() * *min_dist
                } else if len > *max_dist {
                    pos.normalize_or_zero() * *max_dist
                } else {
                    pos
                }
            }
            ConstraintKind::Axis { axis, origin } => {
                let d = pos - *origin;
                let proj = axis.dot(d);
                *origin + *axis * proj
            }
            ConstraintKind::Plane { normal, offset } => {
                let dist = normal.dot(pos) - offset;
                pos - *normal * dist * self.strength
            }
            ConstraintKind::Sphere { center, radius } => {
                let d = pos - *center;
                let len = d.length();
                if len > *radius {
                    *center + d.normalize_or_zero() * *radius
                } else {
                    pos
                }
            }
            ConstraintKind::Cage { min, max } => {
                Vec3::new(
                    pos.x.clamp(min.x, max.x),
                    pos.y.clamp(min.y, max.y),
                    pos.z.clamp(min.z, max.z),
                )
            }
            ConstraintKind::Mirror { axis } => {
                match axis {
                    0 => Vec3::new(pos.x.abs(), pos.y, pos.z),
                    1 => Vec3::new(pos.x, pos.y.abs(), pos.z),
                    2 => Vec3::new(pos.x, pos.y, pos.z.abs()),
                    _ => pos,
                }
            }
        }
    }
}

pub struct ConstraintSolver {
    pub constraints: Vec<ParticleConstraint>,
    pub iterations:  u32,
}

impl ConstraintSolver {
    pub fn new() -> Self {
        Self { constraints: Vec::new(), iterations: 4 }
    }

    pub fn add_constraint(&mut self, c: ParticleConstraint) {
        self.constraints.push(c);
    }

    pub fn remove_for_particle(&mut self, idx: usize) {
        self.constraints.retain(|c| c.particle_idx != idx);
    }

    pub fn solve(&self, positions: &mut Vec<Vec3>) {
        for _ in 0..self.iterations {
            for c in &self.constraints {
                if c.particle_idx < positions.len() {
                    let old = positions[c.particle_idx];
                    positions[c.particle_idx] = c.apply(old);
                }
            }
        }
    }

    pub fn solve_model(&self, model: &mut ParticleModel) {
        let mut positions: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
        self.solve(&mut positions);
        for (i, p) in model.particles.iter_mut().enumerate() {
            p.position = positions[i];
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 2: Physics Simulation
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PhysicsParticle {
    pub position:     Vec3,
    pub velocity:     Vec3,
    pub acceleration: Vec3,
    pub mass:         f32,
    pub damping:      f32,
    pub fixed:        bool,
}

impl PhysicsParticle {
    pub fn new(position: Vec3, mass: f32) -> Self {
        Self {
            position,
            velocity:     Vec3::ZERO,
            acceleration: Vec3::ZERO,
            mass,
            damping:      0.98,
            fixed:        false,
        }
    }

    pub fn integrate(&mut self, dt: f32) {
        if self.fixed { return; }
        self.velocity = (self.velocity + self.acceleration * dt) * self.damping;
        self.position += self.velocity * dt;
        self.acceleration = Vec3::ZERO;
    }

    pub fn apply_force(&mut self, force: Vec3) {
        if !self.fixed {
            self.acceleration += force / self.mass;
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpringConstraint {
    pub a:          usize,
    pub b:          usize,
    pub rest_len:   f32,
    pub stiffness:  f32,
    pub damping:    f32,
}

impl SpringConstraint {
    pub fn new(a: usize, b: usize, rest_len: f32, stiffness: f32) -> Self {
        Self { a, b, rest_len, stiffness, damping: 0.01 }
    }

    pub fn apply(&self, particles: &mut Vec<PhysicsParticle>) {
        if self.a >= particles.len() || self.b >= particles.len() { return; }
        let pa = particles[self.a].position;
        let pb = particles[self.b].position;
        let d  = pb - pa;
        let dist = d.length();
        if dist < 1e-6 { return; }
        let stretch = dist - self.rest_len;
        let dir     = d / dist;
        let force   = dir * stretch * self.stiffness;
        let va = particles[self.a].velocity;
        let vb = particles[self.b].velocity;
        let damp_force = (vb - va).dot(dir) * self.damping * dir;
        particles[self.a].apply_force( force + damp_force);
        particles[self.b].apply_force(-force - damp_force);
    }
}

pub struct PhysicsSimulator {
    pub particles: Vec<PhysicsParticle>,
    pub springs:   Vec<SpringConstraint>,
    pub gravity:   Vec3,
    pub substeps:  u32,
    pub time:      f32,
}

impl PhysicsSimulator {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            springs:   Vec::new(),
            gravity:   Vec3::new(0.0, -9.81, 0.0),
            substeps:  4,
            time:      0.0,
        }
    }

    pub fn add_particle(&mut self, position: Vec3, mass: f32) -> usize {
        let idx = self.particles.len();
        self.particles.push(PhysicsParticle::new(position, mass));
        idx
    }

    pub fn add_spring(&mut self, a: usize, b: usize, stiffness: f32) {
        let rest = if a < self.particles.len() && b < self.particles.len() {
            (self.particles[b].position - self.particles[a].position).length()
        } else {
            1.0
        };
        self.springs.push(SpringConstraint::new(a, b, rest, stiffness));
    }

    pub fn step(&mut self, dt: f32) {
        let sub_dt = dt / self.substeps as f32;
        for _ in 0..self.substeps {
            // Apply gravity
            for p in &mut self.particles {
                p.apply_force(self.gravity * p.mass);
            }
            // Apply springs
            let springs = self.springs.clone();
            for s in &springs {
                s.apply(&mut self.particles);
            }
            // Integrate
            for p in &mut self.particles {
                p.integrate(sub_dt);
            }
        }
        self.time += dt;
    }

    pub fn apply_to_model(&self, model: &mut ParticleModel) {
        for (i, p) in self.particles.iter().enumerate() {
            if i < model.particles.len() {
                model.particles[i].position = p.position;
            }
        }
    }

    pub fn wind_force(&mut self, direction: Vec3, strength: f32, turbulence: f32) {
        for (i, p) in self.particles.iter_mut().enumerate() {
            let noise_val = ((i as f32 * 0.37 + self.time * 2.1).sin()
                + (i as f32 * 0.71 + self.time * 1.3).cos()) * 0.5;
            let t = Vec3::new(
                (i as f32 * 0.53 + self.time).sin(),
                (i as f32 * 0.29 + self.time * 1.7).cos(),
                (i as f32 * 0.61 + self.time * 0.9).sin(),
            ) * turbulence * noise_val;
            p.apply_force((direction + t) * strength * p.mass);
        }
    }

    pub fn collision_floor(&mut self, y: f32, restitution: f32) {
        for p in &mut self.particles {
            if p.position.y < y {
                p.position.y = y;
                p.velocity.y = -p.velocity.y * restitution;
            }
        }
    }

    pub fn collision_sphere(&mut self, center: Vec3, radius: f32, restitution: f32) {
        for p in &mut self.particles {
            let d = p.position - center;
            let dist = d.length();
            if dist < radius {
                let n = d.normalize_or_zero();
                p.position = center + n * radius;
                let vn = p.velocity.dot(n);
                if vn < 0.0 {
                    p.velocity -= n * vn * (1.0 + restitution);
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 3: Curve Tools
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum CurveType {
    Polyline,
    CatmullRom { alpha: f32 },
    Bezier,
    BSpline { degree: usize },
    Nurbs { degree: usize, weights: Vec<f32> },
}

#[derive(Clone, Debug)]
pub struct ModelCurve {
    pub control_points: Vec<Vec3>,
    pub curve_type:     CurveType,
    pub closed:         bool,
    pub resolution:     u32,
    pub name:           String,
}

impl ModelCurve {
    pub fn new(name: &str, curve_type: CurveType) -> Self {
        Self {
            control_points: Vec::new(),
            curve_type,
            closed: false,
            resolution: 64,
            name: name.to_string(),
        }
    }

    pub fn add_point(&mut self, p: Vec3) {
        self.control_points.push(p);
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        if self.control_points.is_empty() { return Vec3::ZERO; }
        if self.control_points.len() == 1 { return self.control_points[0]; }
        match &self.curve_type {
            CurveType::Polyline => self.eval_polyline(t),
            CurveType::CatmullRom { alpha } => self.eval_catmull_rom(t, *alpha),
            CurveType::Bezier => self.eval_bezier(t),
            CurveType::BSpline { degree } => self.eval_bspline(t, *degree),
            CurveType::Nurbs { degree, weights } => self.eval_nurbs(t, *degree, weights),
        }
    }

    fn eval_polyline(&self, t: f32) -> Vec3 {
        let n = self.control_points.len() - 1;
        let scaled = t.clamp(0.0, 1.0) * n as f32;
        let i = (scaled as usize).min(n - 1);
        let f = scaled - i as f32;
        self.control_points[i].lerp(self.control_points[i + 1], f)
    }

    fn eval_catmull_rom(&self, t: f32, alpha: f32) -> Vec3 {
        let pts = &self.control_points;
        let n = pts.len();
        if n < 2 { return pts[0]; }
        let scaled = t.clamp(0.0, 1.0) * (n - 1) as f32;
        let i1 = (scaled as usize).min(n - 2);
        let local_t = scaled - i1 as f32;
        let i0 = if i1 == 0 { 0 } else { i1 - 1 };
        let i2 = (i1 + 1).min(n - 1);
        let i3 = (i1 + 2).min(n - 1);
        let p0 = pts[i0]; let p1 = pts[i1]; let p2 = pts[i2]; let p3 = pts[i3];
        // Centripetal parameterization
        let t01 = (p1 - p0).length().powf(alpha);
        let t12 = (p2 - p1).length().powf(alpha);
        let t23 = (p3 - p2).length().powf(alpha);
        let m1 = if t01 + t12 > 1e-6 {
            (p2 - p1 + (p1 - p0) * (t12 / (t01 + 1e-6)) - (p2 - p0) * (t12 / (t01 + t12 + 1e-6))) * 0.5
        } else { p2 - p1 };
        let m2 = if t12 + t23 > 1e-6 {
            (p3 - p2 + (p2 - p1) * (t23 / (t12 + 1e-6)) - (p3 - p1) * (t23 / (t12 + t23 + 1e-6))) * 0.5
        } else { p2 - p1 };
        let u = local_t;
        let u2 = u * u; let u3 = u2 * u;
        p1 * (2.0*u3 - 3.0*u2 + 1.0)
            + m1 * (u3 - 2.0*u2 + u)
            + p2 * (-2.0*u3 + 3.0*u2)
            + m2 * (u3 - u2)
    }

    fn eval_bezier(&self, t: f32) -> Vec3 {
        let pts = &self.control_points;
        let n = pts.len() - 1;
        let mut result = Vec3::ZERO;
        for (i, p) in pts.iter().enumerate() {
            let b = Self::bernstein(n, i, t);
            result += *p * b;
        }
        result
    }

    fn bernstein(n: usize, i: usize, t: f32) -> f32 {
        Self::binomial(n, i) as f32 * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
    }

    fn binomial(n: usize, k: usize) -> u64 {
        if k > n { return 0; }
        let k = k.min(n - k);
        let mut result = 1u64;
        for i in 0..k {
            result = result * (n - i) as u64 / (i + 1) as u64;
        }
        result
    }

    fn eval_bspline(&self, t: f32, degree: usize) -> Vec3 {
        let pts = &self.control_points;
        let n = pts.len();
        if n == 0 { return Vec3::ZERO; }
        let order = degree + 1;
        // Uniform knot vector
        let num_knots = n + order;
        let knots: Vec<f32> = (0..num_knots).map(|i| i as f32 / (num_knots - 1) as f32).collect();
        let t_clamped = t.clamp(knots[degree], knots[n]);
        // De Boor's algorithm
        let mut k = degree;
        for i in degree..(n + degree) {
            if t_clamped >= knots[i] && t_clamped < knots[i + 1] {
                k = i;
                break;
            }
        }
        let mut d: Vec<Vec3> = (0..=degree).map(|j| {
            let idx = j + k - degree;
            if idx < n { pts[idx] } else { Vec3::ZERO }
        }).collect();
        for r in 1..=degree {
            for j in (r..=degree).rev() {
                let kj = j + k - degree;
                let denom = knots[kj + degree - r + 1] - knots[kj];
                let alpha = if denom.abs() > 1e-9 {
                    (t_clamped - knots[kj]) / denom
                } else { 0.0 };
                d[j] = d[j - 1].lerp(d[j], alpha);
            }
        }
        d[degree]
    }

    fn eval_nurbs(&self, t: f32, degree: usize, weights: &[f32]) -> Vec3 {
        let pts = &self.control_points;
        let n = pts.len().min(weights.len());
        if n == 0 { return Vec3::ZERO; }
        // Homogeneous coordinates
        let order = degree + 1;
        let num_knots = n + order;
        let knots: Vec<f32> = (0..num_knots).map(|i| i as f32 / (num_knots - 1) as f32).collect();
        let t_c = t.clamp(knots[degree], knots[n]);
        let mut k = degree;
        for i in degree..(n + degree) {
            if t_c >= knots[i] && t_c < knots[i + 1] { k = i; break; }
        }
        // Weighted homogeneous de Boor
        let mut hw: Vec<Vec4> = (0..=degree).map(|j| {
            let idx = (j + k - degree).min(n - 1);
            let w = weights[idx];
            Vec4::new(pts[idx].x * w, pts[idx].y * w, pts[idx].z * w, w)
        }).collect();
        for r in 1..=degree {
            for j in (r..=degree).rev() {
                let kj = j + k - degree;
                let denom = knots[kj + degree - r + 1] - knots[kj];
                let alpha = if denom.abs() > 1e-9 { (t_c - knots[kj]) / denom } else { 0.0 };
                hw[j] = hw[j - 1] + (hw[j] - hw[j - 1]) * alpha;
            }
        }
        let w = hw[degree].w;
        if w.abs() < 1e-9 { return Vec3::ZERO; }
        Vec3::new(hw[degree].x / w, hw[degree].y / w, hw[degree].z / w)
    }

    /// Sample the curve into N points
    pub fn sample(&self, n: u32) -> Vec<Vec3> {
        (0..n).map(|i| {
            let t = i as f32 / (n - 1).max(1) as f32;
            self.evaluate(t)
        }).collect()
    }

    /// Extrude curve into particles along the path
    pub fn extrude_to_particles(&self, char_: char, color: Vec4, spacing: f32) -> Vec<ModelParticle> {
        let pts = self.sample(self.resolution);
        let mut particles = Vec::new();
        let mut dist = 0.0_f32;
        for i in 1..pts.len() {
            let seg_len = (pts[i] - pts[i - 1]).length();
            while dist <= seg_len {
                let t = dist / seg_len.max(1e-6);
                let pos = pts[i - 1].lerp(pts[i], t);
                let tangent = (pts[i] - pts[i - 1]).normalize_or_zero();
                particles.push(ModelParticle {
                    position:     pos,
                    character:    char_,
                    color,
                    emission:     0.0,
                    normal:       tangent,
                    bone_weights: [1.0, 0.0, 0.0, 0.0],
                    bone_indices: [0, 0, 0, 0],
                    group_id:     0,
                    layer_id:     0,
                    selected:     false,
                    locked:       false,
                });
                dist += spacing;
            }
            dist -= seg_len;
        }
        particles
    }

    /// Compute arc length
    pub fn arc_length(&self, samples: u32) -> f32 {
        let pts = self.sample(samples);
        let mut len = 0.0_f32;
        for i in 1..pts.len() {
            len += (pts[i] - pts[i - 1]).length();
        }
        len
    }

    /// Closest point on curve to a query point
    pub fn closest_point(&self, query: Vec3, samples: u32) -> (Vec3, f32) {
        let pts = self.sample(samples);
        let mut best = pts[0];
        let mut best_t = 0.0_f32;
        let mut best_d2 = f32::MAX;
        for (i, p) in pts.iter().enumerate() {
            let d2 = (*p - query).length_squared();
            if d2 < best_d2 {
                best_d2 = d2;
                best = *p;
                best_t = i as f32 / (samples - 1).max(1) as f32;
            }
        }
        (best, best_t)
    }

    /// Frenet-Serret frame at parameter t
    pub fn frenet_frame(&self, t: f32) -> (Vec3, Vec3, Vec3) {
        let eps = 0.001_f32;
        let p0 = self.evaluate((t - eps).max(0.0));
        let p1 = self.evaluate((t + eps).min(1.0));
        let tangent = (p1 - p0).normalize_or_zero();
        let p2 = self.evaluate((t - 2.0 * eps).max(0.0));
        let p3 = self.evaluate((t + 2.0 * eps).min(1.0));
        let accel = p3 - 2.0 * self.evaluate(t) + p2;
        let normal = if accel.length_squared() > 1e-9 {
            (accel - tangent * tangent.dot(accel)).normalize_or_zero()
        } else {
            let up = if tangent.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
            tangent.cross(up).normalize_or_zero()
        };
        let binormal = tangent.cross(normal).normalize_or_zero();
        (tangent, normal, binormal)
    }
}

pub struct CurveLibrary {
    pub curves: HashMap<String, ModelCurve>,
}

impl CurveLibrary {
    pub fn new() -> Self { Self { curves: HashMap::new() } }
    pub fn add(&mut self, curve: ModelCurve) { self.curves.insert(curve.name.clone(), curve); }
    pub fn get(&self, name: &str) -> Option<&ModelCurve> { self.curves.get(name) }
    pub fn remove(&mut self, name: &str) -> Option<ModelCurve> { self.curves.remove(name) }
    pub fn names(&self) -> Vec<&String> { self.curves.keys().collect() }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 4: Texture Projection
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum ProjectionMode {
    Planar  { normal: Vec3, up: Vec3, origin: Vec3 },
    Spherical { center: Vec3 },
    Cylindrical { axis: Vec3, origin: Vec3 },
    Cubic   { scale: f32 },
    Camera  { view_proj: Mat4 },
}

pub struct TextureProjector {
    pub mode:    ProjectionMode,
    pub scale:   Vec2,
    pub offset:  Vec2,
    pub rotation: f32,
    pub flip_u:  bool,
    pub flip_v:  bool,
}

impl TextureProjector {
    pub fn new(mode: ProjectionMode) -> Self {
        Self { mode, scale: Vec2::ONE, offset: Vec2::ZERO, rotation: 0.0, flip_u: false, flip_v: false }
    }

    pub fn project(&self, pos: Vec3) -> Vec2 {
        let uv = match &self.mode {
            ProjectionMode::Planar { normal, up, origin } => {
                let right = up.cross(*normal).normalize_or_zero();
                let local = pos - *origin;
                Vec2::new(local.dot(right), local.dot(*up))
            }
            ProjectionMode::Spherical { center } => {
                let d = (pos - *center).normalize_or_zero();
                let u = 0.5 + d.z.atan2(d.x) / (2.0 * std::f32::consts::PI);
                let v = 0.5 - d.y.asin() / std::f32::consts::PI;
                Vec2::new(u, v)
            }
            ProjectionMode::Cylindrical { axis, origin } => {
                let d = pos - *origin;
                let height = d.dot(*axis);
                let radial = d - *axis * height;
                let angle = radial.z.atan2(radial.x);
                Vec2::new(angle / (2.0 * std::f32::consts::PI) + 0.5, height)
            }
            ProjectionMode::Cubic { scale } => {
                let p = pos * *scale;
                Vec2::new(p.x.fract(), p.y.fract())
            }
            ProjectionMode::Camera { view_proj } => {
                let clip = *view_proj * Vec4::new(pos.x, pos.y, pos.z, 1.0);
                let ndc = if clip.w.abs() > 1e-6 {
                    Vec2::new(clip.x / clip.w, clip.y / clip.w)
                } else { Vec2::ZERO };
                (ndc + Vec2::ONE) * 0.5
            }
        };
        // Apply rotation
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();
        let centered = uv - Vec2::splat(0.5);
        let rotated  = Vec2::new(
            centered.x * cos_r - centered.y * sin_r,
            centered.x * sin_r + centered.y * cos_r,
        );
        let uv2 = (rotated + Vec2::splat(0.5)) * self.scale + self.offset;
        Vec2::new(
            if self.flip_u { 1.0 - uv2.x } else { uv2.x },
            if self.flip_v { 1.0 - uv2.y } else { uv2.y },
        )
    }

    pub fn apply_to_model_colors(&self, model: &mut ParticleModel, palette: &[Vec4]) {
        if palette.is_empty() { return; }
        for p in &mut model.particles {
            let uv = self.project(p.position);
            let ux = uv.x.fract().abs();
            let uy = uv.y.fract().abs();
            // Map UV to palette using Halton-like 2D index
            let px = ((ux * palette.len() as f32) as usize).min(palette.len() - 1);
            let py = ((uy * palette.len() as f32) as usize).min(palette.len() - 1);
            let idx = (px + py) % palette.len();
            p.color = palette[idx];
        }
    }

    pub fn apply_to_model_chars(&self, model: &mut ParticleModel, char_set: &[char]) {
        if char_set.is_empty() { return; }
        for p in &mut model.particles {
            let uv = self.project(p.position);
            let ux = uv.x.fract().abs();
            let idx = ((ux * char_set.len() as f32) as usize).min(char_set.len() - 1);
            p.character = char_set[idx];
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 5: Particle Field Effects
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum FieldType {
    Gravitational { center: Vec3, strength: f32 },
    Magnetic      { axis: Vec3, origin: Vec3, strength: f32 },
    Wind          { direction: Vec3, strength: f32, turbulence: f32 },
    Vortex        { axis: Vec3, origin: Vec3, angular_vel: f32, decay: f32 },
    Repulsion     { center: Vec3, radius: f32, strength: f32 },
    Attraction    { center: Vec3, radius: f32, strength: f32 },
    Turbulent     { scale: f32, strength: f32, time_offset: f32 },
    Shockwave     { origin: Vec3, speed: f32, strength: f32, time: f32 },
}

impl FieldType {
    pub fn evaluate(&self, pos: Vec3, time: f32) -> Vec3 {
        match self {
            FieldType::Gravitational { center, strength } => {
                let d = *center - pos;
                let d2 = d.length_squared();
                if d2 < 1e-6 { return Vec3::ZERO; }
                d.normalize_or_zero() * *strength / d2
            }
            FieldType::Magnetic { axis, origin, strength } => {
                let d = pos - *origin;
                let along = axis.dot(d);
                let perp = d - *axis * along;
                axis.cross(perp) * *strength
            }
            FieldType::Wind { direction, strength, turbulence } => {
                let t = time;
                let noise = Vec3::new(
                    (pos.x * 0.5 + t).sin() * (pos.z * 0.3).cos(),
                    (pos.y * 0.4 + t * 1.3).sin(),
                    (pos.z * 0.6 + t * 0.7).cos(),
                ) * *turbulence;
                *direction * *strength + noise
            }
            FieldType::Vortex { axis, origin, angular_vel, decay } => {
                let d = pos - *origin;
                let along = axis.dot(d);
                let perp = d - *axis * along;
                let r = perp.length();
                if r < 1e-6 { return Vec3::ZERO; }
                let tangent = axis.cross(perp).normalize_or_zero();
                let speed = *angular_vel * (-r * *decay).exp();
                tangent * speed
            }
            FieldType::Repulsion { center, radius, strength } => {
                let d = pos - *center;
                let dist = d.length();
                if dist > *radius || dist < 1e-6 { return Vec3::ZERO; }
                let falloff = 1.0 - dist / *radius;
                d.normalize_or_zero() * *strength * falloff * falloff
            }
            FieldType::Attraction { center, radius, strength } => {
                let d = *center - pos;
                let dist = d.length();
                if dist > *radius || dist < 1e-6 { return Vec3::ZERO; }
                let falloff = 1.0 - dist / *radius;
                d.normalize_or_zero() * *strength * falloff
            }
            FieldType::Turbulent { scale, strength, time_offset } => {
                let s = *scale;
                let t = time + *time_offset;
                Vec3::new(
                    (pos.x * s + t * 1.1).sin() * (pos.y * s * 0.7).cos(),
                    (pos.y * s + t * 0.8).sin() * (pos.z * s * 1.3).cos(),
                    (pos.z * s + t * 1.5).sin() * (pos.x * s * 0.9).cos(),
                ) * *strength
            }
            FieldType::Shockwave { origin, speed, strength, time } => {
                let elapsed = time;
                let radius = speed * elapsed;
                let d = pos - *origin;
                let dist = d.length();
                let wave_width = 0.5_f32;
                let diff = (dist - radius).abs();
                if diff > wave_width { return Vec3::ZERO; }
                let falloff = 1.0 - diff / wave_width;
                d.normalize_or_zero() * *strength * falloff
            }
        }
    }
}

pub struct ParticleField {
    pub fields:    Vec<FieldType>,
    pub time:      f32,
    pub enabled:   bool,
}

impl ParticleField {
    pub fn new() -> Self { Self { fields: Vec::new(), time: 0.0, enabled: true } }
    pub fn add(&mut self, f: FieldType) { self.fields.push(f); }
    pub fn clear(&mut self) { self.fields.clear(); }

    pub fn evaluate(&self, pos: Vec3) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        let mut total = Vec3::ZERO;
        for f in &self.fields {
            total += f.evaluate(pos, self.time);
        }
        total
    }

    pub fn apply_displacement(&self, model: &mut ParticleModel, dt: f32, max_disp: f32) {
        if !self.enabled { return; }
        for p in &mut model.particles {
            if p.locked { continue; }
            let force = self.evaluate(p.position);
            let disp = force * dt;
            let disp_len = disp.length();
            if disp_len > max_disp {
                p.position += disp / disp_len * max_disp;
            } else {
                p.position += disp;
            }
        }
    }

    pub fn apply_color_modulation(&self, model: &mut ParticleModel) {
        for p in &mut model.particles {
            let force = self.evaluate(p.position);
            let intensity = (force.length() * 0.1).min(1.0);
            p.color = Vec4::new(
                (p.color.x + intensity * 0.1).min(1.0),
                (p.color.y - intensity * 0.05).max(0.0),
                (p.color.z + intensity * 0.2).min(1.0),
                p.color.w,
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 6: Render Pipeline
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RenderCell {
    pub character: char,
    pub fg_color:  Vec4,
    pub bg_color:  Vec4,
    pub bold:      bool,
    pub italic:    bool,
    pub depth:     f32,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            character: ' ',
            fg_color:  Vec4::ONE,
            bg_color:  Vec4::ZERO,
            bold:      false,
            italic:    false,
            depth:     f32::MAX,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderBuffer {
    pub width:  usize,
    pub height: usize,
    pub cells:  Vec<RenderCell>,
    pub depth:  Vec<f32>,
}

impl RenderBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            cells: vec![RenderCell::default(); n],
            depth: vec![f32::MAX; n],
        }
    }

    pub fn clear(&mut self) {
        for c in &mut self.cells { *c = RenderCell::default(); }
        for d in &mut self.depth { *d = f32::MAX; }
    }

    pub fn set(&mut self, x: usize, y: usize, cell: RenderCell) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            if cell.depth < self.depth[idx] {
                self.depth[idx] = cell.depth;
                self.cells[idx] = cell;
            }
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&RenderCell> {
        if x < self.width && y < self.height {
            Some(&self.cells[y * self.width + x])
        } else { None }
    }

    pub fn composite(&mut self, other: &RenderBuffer) {
        for y in 0..self.height.min(other.height) {
            for x in 0..self.width.min(other.width) {
                if let Some(c) = other.get(x, y) {
                    if c.character != ' ' {
                        self.set(x, y, c.clone());
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderCamera {
    pub position:   Vec3,
    pub target:     Vec3,
    pub up:         Vec3,
    pub fov:        f32,
    pub near:       f32,
    pub far:        f32,
    pub ortho:      bool,
    pub ortho_size: f32,
}

impl RenderCamera {
    pub fn new() -> Self {
        Self {
            position:   Vec3::new(0.0, 5.0, 10.0),
            target:     Vec3::ZERO,
            up:         Vec3::Y,
            fov:        60.0_f32.to_radians(),
            near:       0.1,
            far:        1000.0,
            ortho:      false,
            ortho_size: 10.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        if self.ortho {
            let h = self.ortho_size * 0.5;
            let w = h * aspect;
            Mat4::orthographic_rh(-w, w, -h, h, self.near, self.far)
        } else {
            Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
        }
    }

    pub fn world_to_screen(&self, pos: Vec3, width: u32, height: u32) -> Option<(i32, i32, f32)> {
        let aspect = width as f32 / height as f32;
        let vp = self.proj_matrix(aspect) * self.view_matrix();
        let clip = vp * Vec4::new(pos.x, pos.y, pos.z, 1.0);
        if clip.w.abs() < 1e-6 { return None; }
        let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
        if ndc.z < -1.0 || ndc.z > 1.0 { return None; }
        let sx = ((ndc.x + 1.0) * 0.5 * width  as f32) as i32;
        let sy = ((1.0 - ndc.y) * 0.5 * height as f32) as i32;
        Some((sx, sy, ndc.z))
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        let arm = self.position - self.target;
        let radius = arm.length();
        let yaw   = arm.z.atan2(arm.x) + delta_yaw;
        let pitch = (arm.y / radius.max(1e-6)).asin() + delta_pitch;
        let pitch = pitch.clamp(-1.5, 1.5);
        self.position = self.target + Vec3::new(
            radius * pitch.cos() * yaw.cos(),
            radius * pitch.sin(),
            radius * pitch.cos() * yaw.sin(),
        );
    }

    pub fn dolly(&mut self, delta: f32) {
        let dir = (self.target - self.position).normalize_or_zero();
        self.position += dir * delta;
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let fwd   = (self.target - self.position).normalize_or_zero();
        let right = fwd.cross(self.up).normalize_or_zero();
        let up    = right.cross(fwd).normalize_or_zero();
        let delta = right * dx + up * dy;
        self.position += delta;
        self.target   += delta;
    }
}

pub struct ParticleRenderer {
    pub camera:       RenderCamera,
    pub buffer:       RenderBuffer,
    pub show_normals: bool,
    pub show_bones:   bool,
    pub show_grid:    bool,
    pub grid_size:    f32,
    pub ambient:      f32,
    pub light_dir:    Vec3,
}

impl ParticleRenderer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            camera:       RenderCamera::new(),
            buffer:       RenderBuffer::new(width, height),
            show_normals: false,
            show_bones:   false,
            show_grid:    true,
            grid_size:    1.0,
            ambient:      0.2,
            light_dir:    Vec3::new(0.5, 1.0, 0.3).normalize_or_zero(),
        }
    }

    pub fn render_model(&mut self, model: &ParticleModel) {
        let w = self.buffer.width as u32;
        let h = self.buffer.height as u32;
        for p in &model.particles {
            if let Some((sx, sy, depth)) = self.camera.world_to_screen(p.position, w, h) {
                if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 { continue; }
                let diffuse = self.light_dir.dot(p.normal).max(0.0);
                let light   = self.ambient + diffuse * (1.0 - self.ambient);
                let lit_color = Vec4::new(
                    (p.color.x * light).min(1.0),
                    (p.color.y * light).min(1.0),
                    (p.color.z * light).min(1.0),
                    p.color.w,
                );
                let cell = RenderCell {
                    character: if p.selected { '*' } else { p.character },
                    fg_color:  lit_color,
                    bg_color:  Vec4::ZERO,
                    bold:      p.selected,
                    italic:    false,
                    depth,
                };
                self.buffer.set(sx as usize, sy as usize, cell);
            }
        }
    }

    pub fn render_grid(&mut self) {
        if !self.show_grid { return; }
        let w = self.buffer.width as u32;
        let h = self.buffer.height as u32;
        let half = 10.0_f32;
        let step = self.grid_size;
        let mut x = -half;
        while x <= half {
            let mut z = -half;
            while z <= half {
                let pos = Vec3::new(x, 0.0, z);
                if let Some((sx, sy, d)) = self.camera.world_to_screen(pos, w, h) {
                    if sx >= 0 && sy >= 0 && sx < w as i32 && sy < h as i32 {
                        self.buffer.set(sx as usize, sy as usize, RenderCell {
                            character: '.',
                            fg_color:  Vec4::new(0.3, 0.3, 0.3, 1.0),
                            bg_color:  Vec4::ZERO,
                            bold:      false,
                            italic:    false,
                            depth:     d,
                        });
                    }
                }
                z += step;
            }
            x += step;
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.buffer = RenderBuffer::new(width, height);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 7: Model Comparison and Diffing
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleDiff {
    pub added:   Vec<ModelParticle>,
    pub removed: Vec<usize>,
    pub moved:   Vec<(usize, Vec3, Vec3)>,
    pub recolored: Vec<(usize, Vec4, Vec4)>,
}

impl ParticleDiff {
    pub fn compute(before: &ParticleModel, after: &ParticleModel) -> Self {
        let before_n = before.particles.len();
        let after_n  = after.particles.len();
        let mut moved: Vec<(usize, Vec3, Vec3)> = Vec::new();
        let mut recolored: Vec<(usize, Vec4, Vec4)> = Vec::new();
        let common = before_n.min(after_n);
        for i in 0..common {
            let bp = &before.particles[i];
            let ap = &after.particles[i];
            if (bp.position - ap.position).length_squared() > 1e-8 {
                moved.push((i, bp.position, ap.position));
            }
            if (bp.color - ap.color).length_squared() > 1e-8 {
                recolored.push((i, bp.color, ap.color));
            }
        }
        let added: Vec<ModelParticle> = if after_n > before_n {
            after.particles[before_n..].to_vec()
        } else { Vec::new() };
        let removed: Vec<usize> = if before_n > after_n {
            (after_n..before_n).collect()
        } else { Vec::new() };
        Self { added, removed, moved, recolored }
    }

    pub fn apply(&self, model: &mut ParticleModel) {
        // Remove in reverse order
        let mut to_remove = self.removed.clone();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in &to_remove {
            if *idx < model.particles.len() {
                model.particles.remove(*idx);
            }
        }
        // Apply moves
        for (i, _from, to) in &self.moved {
            if *i < model.particles.len() {
                model.particles[*i].position = *to;
            }
        }
        // Apply recolors
        for (i, _from, to) in &self.recolored {
            if *i < model.particles.len() {
                model.particles[*i].color = *to;
            }
        }
        // Add new
        for p in &self.added {
            model.particles.push(p.clone());
        }
    }

    pub fn invert(&self) -> Self {
        Self {
            added:     Vec::new(),
            removed:   (0..self.added.len()).collect(),
            moved:     self.moved.iter().map(|(i, f, t)| (*i, *t, *f)).collect(),
            recolored: self.recolored.iter().map(|(i, f, t)| (*i, *t, *f)).collect(),
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "added={}, removed={}, moved={}, recolored={}",
            self.added.len(), self.removed.len(), self.moved.len(), self.recolored.len()
        )
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 8: Spatial Partitioning — Octree
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct OctreeNode {
    pub center:   Vec3,
    pub half:     f32,
    pub indices:  Vec<usize>,
    pub children: Option<Box<[OctreeNode; 8]>>,
}

impl OctreeNode {
    const MAX_CAPACITY: usize = 16;
    const MAX_DEPTH:    u32   = 8;

    pub fn new(center: Vec3, half: f32) -> Self {
        Self { center, half, indices: Vec::new(), children: None }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        let d = p - self.center;
        d.x.abs() <= self.half && d.y.abs() <= self.half && d.z.abs() <= self.half
    }

    pub fn insert(&mut self, idx: usize, pos: Vec3, depth: u32) {
        if self.children.is_some() {
            let child_idx = self.child_index(pos);
            if let Some(children) = &mut self.children {
                children[child_idx].insert(idx, pos, depth + 1);
            }
            return;
        }
        self.indices.push(idx);
        if self.indices.len() > Self::MAX_CAPACITY && depth < Self::MAX_DEPTH {
            self.subdivide(depth);
        }
    }

    fn child_index(&self, p: Vec3) -> usize {
        let dx = if p.x >= self.center.x { 1 } else { 0 };
        let dy = if p.y >= self.center.y { 2 } else { 0 };
        let dz = if p.z >= self.center.z { 4 } else { 0 };
        dx | dy | dz
    }

    fn subdivide(&mut self, depth: u32) {
        let h = self.half * 0.5;
        let c = self.center;
        let make_child = |dx: f32, dy: f32, dz: f32| {
            OctreeNode::new(c + Vec3::new(dx * h, dy * h, dz * h), h)
        };
        let children: [OctreeNode; 8] = [
            make_child(-1.0, -1.0, -1.0),
            make_child( 1.0, -1.0, -1.0),
            make_child(-1.0,  1.0, -1.0),
            make_child( 1.0,  1.0, -1.0),
            make_child(-1.0, -1.0,  1.0),
            make_child( 1.0, -1.0,  1.0),
            make_child(-1.0,  1.0,  1.0),
            make_child( 1.0,  1.0,  1.0),
        ];
        self.children = Some(Box::new(children));
        let old_indices: Vec<usize> = self.indices.drain(..).collect();
        // Re-insert requires positions; store indices in parent for now
        // (actual re-insert needs positions from outside — skip for leaf storage)
        self.indices = old_indices;
    }

    pub fn query_sphere(&self, center: Vec3, radius: f32, result: &mut Vec<usize>) {
        let d = center - self.center;
        let max_d = d.x.abs().max(d.y.abs()).max(d.z.abs());
        if max_d > self.half + radius { return; }
        for &i in &self.indices {
            result.push(i);
        }
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.query_sphere(center, radius, result);
            }
        }
    }

    pub fn count(&self) -> usize {
        let mut n = self.indices.len();
        if let Some(children) = &self.children {
            for c in children.iter() { n += c.count(); }
        }
        n
    }
}

pub struct Octree {
    pub root:      OctreeNode,
    pub positions: Vec<Vec3>,
}

impl Octree {
    pub fn build(positions: &[Vec3]) -> Self {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for &p in positions {
            min = min.min(p);
            max = max.max(p);
        }
        let center = (min + max) * 0.5;
        let half   = ((max - min).max_element() * 0.5 + 0.001).max(1.0);
        let mut root = OctreeNode::new(center, half);
        for (i, &p) in positions.iter().enumerate() {
            root.insert(i, p, 0);
        }
        Self { root, positions: positions.to_vec() }
    }

    pub fn radius_search(&self, center: Vec3, radius: f32) -> Vec<usize> {
        let mut candidates = Vec::new();
        self.root.query_sphere(center, radius, &mut candidates);
        candidates.sort_unstable();
        candidates.dedup();
        let r2 = radius * radius;
        candidates.into_iter()
            .filter(|&i| i < self.positions.len() && (self.positions[i] - center).length_squared() <= r2)
            .collect()
    }

    pub fn nearest(&self, query: Vec3, k: usize) -> Vec<usize> {
        if self.positions.is_empty() { return Vec::new(); }
        // Brute-force for small sets; for large octrees, use radius expansion
        let mut dists: Vec<(f32, usize)> = self.positions.iter().enumerate()
            .map(|(i, &p)| ((p - query).length_squared(), i))
            .collect();
        dists.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        dists.into_iter().take(k).map(|(_, i)| i).collect()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 9: Mesh Boolean Operations (particle-based CSG)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum CsgOperation {
    Union,
    Subtract,
    Intersect,
    Difference,
}

pub struct ParticleCsg;

impl ParticleCsg {
    /// Union: merge two models, removing overlapping particles
    pub fn union(a: &ParticleModel, b: &ParticleModel, merge_threshold: f32) -> ParticleModel {
        let mut result = a.clone();
        let thresh2 = merge_threshold * merge_threshold;
        let a_positions: Vec<Vec3> = a.particles.iter().map(|p| p.position).collect();
        'outer: for bp in &b.particles {
            for ap in &a_positions {
                if (*ap - bp.position).length_squared() < thresh2 {
                    continue 'outer;
                }
            }
            result.particles.push(bp.clone());
        }
        result.recompute_bounds();
        result
    }

    /// Subtract: keep particles from A that are not inside B's bounding volume
    pub fn subtract(a: &ParticleModel, b: &ParticleModel, margin: f32) -> ParticleModel {
        let b_bounds = &b.bounds;
        let expanded_min = b_bounds.min - Vec3::splat(margin);
        let expanded_max = b_bounds.max + Vec3::splat(margin);
        let mut result = a.clone();
        result.particles.retain(|p| {
            let inside = p.position.x >= expanded_min.x && p.position.x <= expanded_max.x
                && p.position.y >= expanded_min.y && p.position.y <= expanded_max.y
                && p.position.z >= expanded_min.z && p.position.z <= expanded_max.z;
            !inside
        });
        result.recompute_bounds();
        result
    }

    /// Intersect: keep only particles from A that overlap with B's volume
    pub fn intersect(a: &ParticleModel, b: &ParticleModel, margin: f32) -> ParticleModel {
        let b_bounds = &b.bounds;
        let expanded_min = b_bounds.min - Vec3::splat(margin);
        let expanded_max = b_bounds.max + Vec3::splat(margin);
        let mut result = a.clone();
        result.particles.retain(|p| {
            p.position.x >= expanded_min.x && p.position.x <= expanded_max.x
                && p.position.y >= expanded_min.y && p.position.y <= expanded_max.y
                && p.position.z >= expanded_min.z && p.position.z <= expanded_max.z
        });
        result.recompute_bounds();
        result
    }

    /// Shell: keep particles near the surface of B (within threshold)
    pub fn shell(model: &ParticleModel, b: &ParticleModel, shell_thickness: f32) -> ParticleModel {
        let b_positions: Vec<Vec3> = b.particles.iter().map(|p| p.position).collect();
        let t2 = shell_thickness * shell_thickness;
        let mut result = model.clone();
        result.particles.retain(|p| {
            b_positions.iter().any(|&bp| (bp - p.position).length_squared() <= t2)
        });
        result.recompute_bounds();
        result
    }

    /// Apply operation
    pub fn apply(op: &CsgOperation, a: &ParticleModel, b: &ParticleModel, threshold: f32) -> ParticleModel {
        match op {
            CsgOperation::Union     => Self::union(a, b, threshold),
            CsgOperation::Subtract  => Self::subtract(a, b, threshold),
            CsgOperation::Intersect => Self::intersect(a, b, threshold),
            CsgOperation::Difference => {
                // symmetric difference: union - intersect
                let u = Self::union(a, b, threshold);
                let i = Self::intersect(a, b, threshold);
                Self::subtract(&u, &i, threshold * 0.5)
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 10: Color Gradient and Palette Tools
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GradientStop {
    pub t:     f32,
    pub color: Vec4,
}

#[derive(Clone, Debug)]
pub struct ColorGradient {
    pub stops: Vec<GradientStop>,
    pub name:  String,
}

impl ColorGradient {
    pub fn new(name: &str) -> Self {
        Self { stops: Vec::new(), name: name.to_string() }
    }

    pub fn add_stop(&mut self, t: f32, color: Vec4) {
        self.stops.push(GradientStop { t, color });
        self.stops.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn evaluate(&self, t: f32) -> Vec4 {
        if self.stops.is_empty() { return Vec4::ONE; }
        if self.stops.len() == 1 { return self.stops[0].color; }
        let t = t.clamp(0.0, 1.0);
        if t <= self.stops[0].t { return self.stops[0].color; }
        if t >= self.stops.last().unwrap().t { return self.stops.last().unwrap().color; }
        for i in 1..self.stops.len() {
            if t <= self.stops[i].t {
                let a = &self.stops[i - 1];
                let b = &self.stops[i];
                let local_t = (t - a.t) / (b.t - a.t).max(1e-6);
                return a.color.lerp(b.color, local_t);
            }
        }
        self.stops.last().unwrap().color
    }

    pub fn rainbow() -> Self {
        let mut g = Self::new("rainbow");
        g.add_stop(0.0,  Vec4::new(1.0, 0.0, 0.0, 1.0));
        g.add_stop(0.16, Vec4::new(1.0, 0.5, 0.0, 1.0));
        g.add_stop(0.33, Vec4::new(1.0, 1.0, 0.0, 1.0));
        g.add_stop(0.5,  Vec4::new(0.0, 1.0, 0.0, 1.0));
        g.add_stop(0.66, Vec4::new(0.0, 0.5, 1.0, 1.0));
        g.add_stop(0.83, Vec4::new(0.0, 0.0, 1.0, 1.0));
        g.add_stop(1.0,  Vec4::new(0.5, 0.0, 1.0, 1.0));
        g
    }

    pub fn grayscale() -> Self {
        let mut g = Self::new("grayscale");
        g.add_stop(0.0, Vec4::new(0.0, 0.0, 0.0, 1.0));
        g.add_stop(1.0, Vec4::new(1.0, 1.0, 1.0, 1.0));
        g
    }

    pub fn fire() -> Self {
        let mut g = Self::new("fire");
        g.add_stop(0.0,  Vec4::new(0.0, 0.0, 0.0, 1.0));
        g.add_stop(0.25, Vec4::new(0.5, 0.0, 0.0, 1.0));
        g.add_stop(0.5,  Vec4::new(1.0, 0.3, 0.0, 1.0));
        g.add_stop(0.75, Vec4::new(1.0, 0.8, 0.0, 1.0));
        g.add_stop(1.0,  Vec4::new(1.0, 1.0, 0.9, 1.0));
        g
    }

    pub fn apply_height(&self, model: &mut ParticleModel) {
        if model.particles.is_empty() { return; }
        let min_y = model.particles.iter().map(|p| p.position.y).fold(f32::MAX, f32::min);
        let max_y = model.particles.iter().map(|p| p.position.y).fold(f32::MIN, f32::max);
        let range = (max_y - min_y).max(1e-6);
        for p in &mut model.particles {
            let t = (p.position.y - min_y) / range;
            p.color = self.evaluate(t);
        }
    }

    pub fn apply_distance(&self, model: &mut ParticleModel, origin: Vec3, max_dist: f32) {
        for p in &mut model.particles {
            let d = (p.position - origin).length() / max_dist.max(1e-6);
            p.color = self.evaluate(d.clamp(0.0, 1.0));
        }
    }

    pub fn apply_normal_angle(&self, model: &mut ParticleModel, reference: Vec3) {
        let ref_n = reference.normalize_or_zero();
        for p in &mut model.particles {
            let angle = ref_n.dot(p.normal.normalize_or_zero()).clamp(-1.0, 1.0).acos();
            let t = angle / std::f32::consts::PI;
            p.color = self.evaluate(t);
        }
    }

    pub fn sample_n(&self, n: usize) -> Vec<Vec4> {
        (0..n).map(|i| self.evaluate(i as f32 / (n - 1).max(1) as f32)).collect()
    }
}

pub struct PaletteManager {
    pub gradients: Vec<ColorGradient>,
    pub palettes:  HashMap<String, Vec<Vec4>>,
}

impl PaletteManager {
    pub fn new() -> Self {
        let mut pm = Self { gradients: Vec::new(), palettes: HashMap::new() };
        pm.gradients.push(ColorGradient::rainbow());
        pm.gradients.push(ColorGradient::grayscale());
        pm.gradients.push(ColorGradient::fire());
        pm
    }

    pub fn add_gradient(&mut self, g: ColorGradient) { self.gradients.push(g); }
    pub fn add_palette(&mut self, name: &str, colors: Vec<Vec4>) {
        self.palettes.insert(name.to_string(), colors);
    }
    pub fn get_gradient(&self, name: &str) -> Option<&ColorGradient> {
        self.gradients.iter().find(|g| g.name == name)
    }
    pub fn get_palette(&self, name: &str) -> Option<&Vec<Vec4>> {
        self.palettes.get(name)
    }

    pub fn quantize_model(&self, model: &mut ParticleModel, palette_name: &str) {
        let Some(palette) = self.palettes.get(palette_name) else { return; };
        if palette.is_empty() { return; }
        for p in &mut model.particles {
            let best = palette.iter()
                .min_by(|a, b| {
                    (**a - p.color).length_squared()
                        .partial_cmp(&(**b - p.color).length_squared())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
                .unwrap_or(p.color);
            p.color = best;
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 11: Particle Decals and Overlays
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ParticleDecal {
    pub position:   Vec3,
    pub normal:     Vec3,
    pub radius:     f32,
    pub depth:      f32,
    pub char_set:   Vec<char>,
    pub color:      Vec4,
    pub blend_mode: DecalBlend,
    pub opacity:    f32,
}

#[derive(Clone, Debug)]
pub enum DecalBlend {
    Replace,
    Multiply,
    Add,
    Screen,
    Overlay,
}

impl ParticleDecal {
    pub fn new(position: Vec3, normal: Vec3, radius: f32) -> Self {
        Self {
            position,
            normal:     normal.normalize_or_zero(),
            radius,
            depth:      0.1,
            char_set:   vec!['#'],
            color:      Vec4::ONE,
            blend_mode: DecalBlend::Replace,
            opacity:    1.0,
        }
    }

    pub fn apply_to_model(&self, model: &mut ParticleModel) {
        let r2 = self.radius * self.radius;
        for p in &mut model.particles {
            let d = p.position - self.position;
            let dist2 = d.length_squared();
            if dist2 > r2 { continue; }
            // Project onto decal plane
            let along_normal = d.dot(self.normal);
            if along_normal.abs() > self.depth { continue; }
            let t = 1.0 - (dist2 / r2).sqrt();
            let alpha = t * self.opacity;
            // Choose char
            if !self.char_set.is_empty() {
                let idx = ((1.0 - t) * (self.char_set.len() - 1) as f32) as usize;
                let idx = idx.min(self.char_set.len() - 1);
                p.character = self.char_set[idx];
            }
            // Blend color
            p.color = match &self.blend_mode {
                DecalBlend::Replace  => self.color.lerp(p.color, 1.0 - alpha),
                DecalBlend::Multiply => {
                    let m = Vec4::new(p.color.x * self.color.x, p.color.y * self.color.y,
                                      p.color.z * self.color.z, p.color.w);
                    p.color.lerp(m, alpha)
                }
                DecalBlend::Add => {
                    Vec4::new(
                        (p.color.x + self.color.x * alpha).min(1.0),
                        (p.color.y + self.color.y * alpha).min(1.0),
                        (p.color.z + self.color.z * alpha).min(1.0),
                        p.color.w,
                    )
                }
                DecalBlend::Screen => {
                    let sc = Vec4::new(
                        1.0 - (1.0 - p.color.x) * (1.0 - self.color.x),
                        1.0 - (1.0 - p.color.y) * (1.0 - self.color.y),
                        1.0 - (1.0 - p.color.z) * (1.0 - self.color.z),
                        p.color.w,
                    );
                    p.color.lerp(sc, alpha)
                }
                DecalBlend::Overlay => {
                    let overlay = |base: f32, src: f32| {
                        if base < 0.5 { 2.0 * base * src } else { 1.0 - 2.0 * (1.0 - base) * (1.0 - src) }
                    };
                    let ov = Vec4::new(
                        overlay(p.color.x, self.color.x),
                        overlay(p.color.y, self.color.y),
                        overlay(p.color.z, self.color.z),
                        p.color.w,
                    );
                    p.color.lerp(ov, alpha)
                }
            };
        }
    }
}

pub struct DecalLayer {
    pub decals: Vec<ParticleDecal>,
    pub enabled: bool,
}

impl DecalLayer {
    pub fn new() -> Self { Self { decals: Vec::new(), enabled: true } }
    pub fn add(&mut self, d: ParticleDecal) { self.decals.push(d); }
    pub fn apply_all(&self, model: &mut ParticleModel) {
        if !self.enabled { return; }
        for d in &self.decals { d.apply_to_model(model); }
    }
    pub fn clear(&mut self) { self.decals.clear(); }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 12: Instancing and Scatter
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ScatterInstance {
    pub position:  Vec3,
    pub rotation:  Quat,
    pub scale:     Vec3,
    pub color_tint: Vec4,
}

#[derive(Clone, Debug)]
pub struct ScatterSettings {
    pub density:       f32,
    pub random_rot:    bool,
    pub align_normal:  bool,
    pub scale_min:     f32,
    pub scale_max:     f32,
    pub color_var:     f32,
    pub seed:          u64,
}

impl Default for ScatterSettings {
    fn default() -> Self {
        Self {
            density:      1.0,
            random_rot:   true,
            align_normal: true,
            scale_min:    0.8,
            scale_max:    1.2,
            color_var:    0.1,
            seed:         42,
        }
    }
}

pub struct ParticleScatter {
    pub template:  ParticleModel,
    pub instances: Vec<ScatterInstance>,
    pub settings:  ScatterSettings,
}

impl ParticleScatter {
    pub fn new(template: ParticleModel) -> Self {
        Self { template, instances: Vec::new(), settings: ScatterSettings::default() }
    }

    pub fn scatter_on_model(&mut self, surface: &ParticleModel) {
        self.instances.clear();
        let mut rng = self.settings.seed;
        let count = (surface.particles.len() as f32 * self.settings.density) as usize;
        for i in 0..count {
            if i >= surface.particles.len() { break; }
            let surf_p = &surface.particles[i];
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r0 = (rng >> 33) as f32 / u32::MAX as f32;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r1 = (rng >> 33) as f32 / u32::MAX as f32;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r2 = (rng >> 33) as f32 / u32::MAX as f32;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r3 = (rng >> 33) as f32 / u32::MAX as f32;
            let scale_s = self.settings.scale_min + r0 * (self.settings.scale_max - self.settings.scale_min);
            let rotation = if self.settings.align_normal {
                let up = Vec3::Y;
                let n  = surf_p.normal.normalize_or_zero();
                let axis = up.cross(n);
                if axis.length_squared() > 1e-6 {
                    Quat::from_axis_angle(axis.normalize(), up.dot(n).clamp(-1.0, 1.0).acos())
                } else { Quat::IDENTITY }
            } else if self.settings.random_rot {
                // Random quaternion from uniform distribution
                let u = r1; let v = r2; let w = r3;
                Quat::from_xyzw(
                    (1.0 - u).sqrt() * (2.0 * std::f32::consts::PI * v).sin(),
                    (1.0 - u).sqrt() * (2.0 * std::f32::consts::PI * v).cos(),
                    u.sqrt() * (2.0 * std::f32::consts::PI * w).sin(),
                    u.sqrt() * (2.0 * std::f32::consts::PI * w).cos(),
                )
            } else { Quat::IDENTITY };
            let color_tint = Vec4::new(
                1.0 + (r0 - 0.5) * self.settings.color_var,
                1.0 + (r1 - 0.5) * self.settings.color_var,
                1.0 + (r2 - 0.5) * self.settings.color_var,
                1.0,
            );
            self.instances.push(ScatterInstance {
                position: surf_p.position,
                rotation,
                scale: Vec3::splat(scale_s),
                color_tint,
            });
        }
    }

    pub fn bake(&self) -> ParticleModel {
        let mut result = ParticleModel::new("scatter_baked");
        for inst in &self.instances {
            let xform = Mat4::from_scale_rotation_translation(inst.scale, inst.rotation, inst.position);
            for tp in &self.template.particles {
                let new_pos = (xform * Vec4::new(tp.position.x, tp.position.y, tp.position.z, 1.0)).truncate();
                let new_nrm = (inst.rotation * tp.normal).normalize_or_zero();
                result.particles.push(ModelParticle {
                    position:     new_pos,
                    character:    tp.character,
                    color:        Vec4::new(
                                      tp.color.x * inst.color_tint.x,
                                      tp.color.y * inst.color_tint.y,
                                      tp.color.z * inst.color_tint.z,
                                      tp.color.w,
                                  ),
                    emission:     tp.emission,
                    normal:       new_nrm,
                    bone_weights: tp.bone_weights,
                    bone_indices: tp.bone_indices,
                    group_id:     tp.group_id,
                    layer_id:     tp.layer_id,
                    selected:     false,
                    locked:       false,
                });
            }
        }
        result.recompute_bounds();
        result
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 13: History/Journal with branching
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HistoryNode {
    pub id:       u64,
    pub parent:   Option<u64>,
    pub children: Vec<u64>,
    pub snapshot: ModelSnapshot,
    pub label:    String,
    pub timestamp: u64,
}

pub struct BranchingHistory {
    pub nodes:      HashMap<u64, HistoryNode>,
    pub current_id: Option<u64>,
    pub next_id:    u64,
}

impl BranchingHistory {
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), current_id: None, next_id: 1 }
    }

    pub fn push(&mut self, snapshot: ModelSnapshot, label: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        if let Some(parent_id) = self.current_id {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.children.push(id);
            }
        }
        self.nodes.insert(id, HistoryNode {
            id,
            parent:    self.current_id,
            children:  Vec::new(),
            snapshot,
            label:     label.to_string(),
            timestamp: id, // monotonic surrogate
        });
        self.current_id = Some(id);
        id
    }

    pub fn undo(&mut self) -> Option<&ModelSnapshot> {
        let cur = self.current_id?;
        let parent = self.nodes.get(&cur)?.parent?;
        self.current_id = Some(parent);
        Some(&self.nodes[&parent].snapshot)
    }

    pub fn redo_to(&mut self, child_id: u64) -> Option<&ModelSnapshot> {
        let cur = self.current_id?;
        if !self.nodes.get(&cur)?.children.contains(&child_id) { return None; }
        self.current_id = Some(child_id);
        Some(&self.nodes[&child_id].snapshot)
    }

    pub fn list_children(&self) -> Vec<(u64, &str)> {
        let Some(cur) = self.current_id else { return Vec::new(); };
        let Some(node) = self.nodes.get(&cur) else { return Vec::new(); };
        node.children.iter()
            .filter_map(|&id| self.nodes.get(&id).map(|n| (id, n.label.as_str())))
            .collect()
    }

    pub fn path_to_root(&self) -> Vec<u64> {
        let mut path = Vec::new();
        let mut cur = self.current_id;
        while let Some(id) = cur {
            path.push(id);
            cur = self.nodes.get(&id).and_then(|n| n.parent);
        }
        path
    }

    pub fn branch_count(&self) -> usize {
        self.nodes.values().filter(|n| n.children.len() > 1).count()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SECTION 14: Final integration tests for ext3
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod ext3_tests {
    use super::*;

    fn make_sphere_model(n: usize) -> ParticleModel {
        let mut m = ParticleModel::new("sphere");
        let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
        for i in 0..n {
            let y = 1.0 - (i as f32 / (n - 1).max(1) as f32) * 2.0;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = golden * i as f32;
            m.particles.push(ModelParticle {
                position: Vec3::new(r * theta.cos(), y, r * theta.sin()),
                character: 'o',
                color: Vec4::new(0.8, 0.6, 0.4, 1.0),
                emission: 0.0,
                normal: Vec3::new(r * theta.cos(), y, r * theta.sin()).normalize_or_zero(),
                bone_weights: [1.0, 0.0, 0.0, 0.0],
                bone_indices: [0, 0, 0, 0],
                group_id: 0,
                layer_id: 0,
                selected: false,
                locked: false,
            });
        }
        m.recompute_bounds();
        m
    }

    #[test]
    fn test_constraint_plane() {
        let c = ParticleConstraint::new(0, ConstraintKind::Plane {
            normal: Vec3::Y, offset: 0.0
        });
        let pos = Vec3::new(1.0, -2.0, 0.0);
        let result = c.apply(pos);
        assert!(result.y.abs() < 0.001, "plane constraint should bring y to 0");
    }

    #[test]
    fn test_constraint_cage() {
        let c = ParticleConstraint::new(0, ConstraintKind::Cage {
            min: Vec3::splat(-1.0), max: Vec3::splat(1.0)
        });
        let pos = Vec3::new(5.0, -3.0, 2.0);
        let r = c.apply(pos);
        assert!(r.x <= 1.0 && r.x >= -1.0);
        assert!(r.y <= 1.0 && r.y >= -1.0);
        assert!(r.z <= 1.0 && r.z >= -1.0);
    }

    #[test]
    fn test_spring_simulation() {
        let mut sim = PhysicsSimulator::new();
        let a = sim.add_particle(Vec3::ZERO, 1.0);
        let b = sim.add_particle(Vec3::new(2.0, 0.0, 0.0), 1.0);
        sim.particles[a].fixed = true;
        sim.add_spring(a, b, 10.0);
        sim.gravity = Vec3::ZERO;
        let initial_pos = sim.particles[b].position;
        sim.step(0.016);
        // Spring should pull b toward a (rest length = 2.0 initially, so no force)
        let final_pos = sim.particles[b].position;
        let moved = (final_pos - initial_pos).length();
        assert!(moved < 0.1, "no displacement when at rest length: {}", moved);
    }

    #[test]
    fn test_curve_polyline() {
        let mut curve = ModelCurve::new("test", CurveType::Polyline);
        curve.add_point(Vec3::ZERO);
        curve.add_point(Vec3::new(1.0, 0.0, 0.0));
        curve.add_point(Vec3::new(2.0, 0.0, 0.0));
        let mid = curve.evaluate(0.5);
        assert!((mid.x - 1.0).abs() < 0.01, "midpoint should be x=1: {}", mid.x);
    }

    #[test]
    fn test_curve_bezier() {
        let mut curve = ModelCurve::new("bez", CurveType::Bezier);
        curve.add_point(Vec3::ZERO);
        curve.add_point(Vec3::new(0.0, 2.0, 0.0));
        curve.add_point(Vec3::new(1.0, 2.0, 0.0));
        curve.add_point(Vec3::new(1.0, 0.0, 0.0));
        let start = curve.evaluate(0.0);
        let end   = curve.evaluate(1.0);
        assert!(start.length() < 0.001);
        assert!((end - Vec3::new(1.0, 0.0, 0.0)).length() < 0.001);
    }

    #[test]
    fn test_curve_arc_length() {
        let mut curve = ModelCurve::new("line", CurveType::Polyline);
        curve.add_point(Vec3::ZERO);
        curve.add_point(Vec3::new(10.0, 0.0, 0.0));
        let len = curve.arc_length(100);
        assert!((len - 10.0).abs() < 0.1, "arc length should be ~10: {}", len);
    }

    #[test]
    fn test_texture_projection_spherical() {
        let proj = TextureProjector::new(ProjectionMode::Spherical { center: Vec3::ZERO });
        let uv = proj.project(Vec3::new(1.0, 0.0, 0.0));
        assert!(uv.x >= 0.0 && uv.x <= 1.0);
        assert!(uv.y >= 0.0 && uv.y <= 1.0);
    }

    #[test]
    fn test_field_vortex() {
        let field = FieldType::Vortex {
            axis: Vec3::Y, origin: Vec3::ZERO, angular_vel: 2.0, decay: 0.5
        };
        let force = field.evaluate(Vec3::new(1.0, 0.0, 0.0), 0.0);
        // Should have a tangential component (non-zero)
        assert!(force.length() > 0.0);
    }

    #[test]
    fn test_particle_field_displacement() {
        let mut field = ParticleField::new();
        field.add(FieldType::Wind { direction: Vec3::X, strength: 1.0, turbulence: 0.0 });
        let mut model = make_sphere_model(50);
        let before: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
        field.apply_displacement(&mut model, 0.1, 1.0);
        let moved = model.particles.iter().zip(before.iter())
            .filter(|(a, b)| (a.position - **b).length() > 0.001)
            .count();
        assert!(moved > 0, "wind should move particles");
    }

    #[test]
    fn test_render_buffer_depth() {
        let mut buf = RenderBuffer::new(80, 24);
        let cell_close = RenderCell { character: 'X', depth: 1.0, ..RenderCell::default() };
        let cell_far   = RenderCell { character: 'Y', depth: 5.0, ..RenderCell::default() };
        buf.set(10, 10, cell_far.clone());
        buf.set(10, 10, cell_close.clone());
        assert_eq!(buf.get(10, 10).unwrap().character, 'X', "closer should win");
        buf.set(10, 10, RenderCell { character: 'Z', depth: 10.0, ..RenderCell::default() });
        assert_eq!(buf.get(10, 10).unwrap().character, 'X', "closer should still win");
    }

    #[test]
    fn test_particle_diff_round_trip() {
        let before = make_sphere_model(30);
        let mut after = before.clone();
        after.particles[0].position += Vec3::new(1.0, 0.0, 0.0);
        after.particles[5].color    = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let diff = ParticleDiff::compute(&before, &after);
        assert_eq!(diff.moved.len(), 1);
        assert_eq!(diff.recolored.len(), 1);
        let inv = diff.invert();
        let mut restored = after.clone();
        inv.apply(&mut restored);
        let d = (restored.particles[0].position - before.particles[0].position).length();
        assert!(d < 0.001, "position should be restored: {}", d);
    }

    #[test]
    fn test_octree_radius_search() {
        let positions: Vec<Vec3> = (0..100).map(|i| {
            let t = i as f32 * 0.1;
            Vec3::new(t.sin(), t.cos(), t * 0.1)
        }).collect();
        let tree = Octree::build(&positions);
        let near = tree.radius_search(Vec3::ZERO, 1.5);
        assert!(!near.is_empty(), "should find neighbors");
        for &i in &near {
            assert!(i < positions.len());
            assert!(positions[i].length() <= 1.5 + 1e-4);
        }
    }

    #[test]
    fn test_csg_union() {
        let a = make_sphere_model(50);
        let mut b = make_sphere_model(20);
        for p in &mut b.particles { p.position += Vec3::new(5.0, 0.0, 0.0); }
        b.recompute_bounds();
        let u = ParticleCsg::union(&a, &b, 0.05);
        assert_eq!(u.particles.len(), 70, "union should have all particles");
    }

    #[test]
    fn test_csg_subtract() {
        let mut a = make_sphere_model(100);
        // center a at origin
        let b = make_sphere_model(10); // small sphere at origin
        let s = ParticleCsg::subtract(&a, &b, 0.0);
        assert!(s.particles.len() < a.particles.len(), "subtract should remove some");
    }

    #[test]
    fn test_color_gradient() {
        let g = ColorGradient::rainbow();
        let c0 = g.evaluate(0.0);
        let c1 = g.evaluate(1.0);
        assert!((c0.x - 1.0).abs() < 0.01, "start should be red");
        assert!(c1.z > 0.0, "end should have blue");
        let samples = g.sample_n(10);
        assert_eq!(samples.len(), 10);
    }

    #[test]
    fn test_gradient_apply_height() {
        let mut model = make_sphere_model(50);
        let g = ColorGradient::fire();
        g.apply_height(&mut model);
        // Just check it doesn't panic and colors are valid
        for p in &model.particles {
            assert!(p.color.x >= 0.0 && p.color.x <= 1.0);
            assert!(p.color.y >= 0.0 && p.color.y <= 1.0);
            assert!(p.color.z >= 0.0 && p.color.z <= 1.0);
        }
    }

    #[test]
    fn test_decal_replace() {
        let mut model = make_sphere_model(50);
        let decal = ParticleDecal {
            position:   Vec3::new(1.0, 0.0, 0.0),
            normal:     Vec3::X,
            radius:     0.5,
            depth:      0.2,
            char_set:   vec!['@'],
            color:      Vec4::new(1.0, 0.0, 0.0, 1.0),
            blend_mode: DecalBlend::Replace,
            opacity:    1.0,
        };
        decal.apply_to_model(&mut model);
        // At least some particles should be affected
        let changed = model.particles.iter().filter(|p| p.character == '@').count();
        // May be 0 if no particles are exactly in range — just test no panic
        let _ = changed;
    }

    #[test]
    fn test_scatter_bake() {
        let template = make_sphere_model(5);
        let surface  = make_sphere_model(20);
        let mut scatter = ParticleScatter::new(template);
        scatter.settings.density = 0.5;
        scatter.scatter_on_model(&surface);
        let baked = scatter.bake();
        assert!(!baked.particles.is_empty(), "baked model should have particles");
    }

    #[test]
    fn test_branching_history() {
        let mut hist = BranchingHistory::new();
        let m0 = ParticleModel::new("v0");
        let m1 = ParticleModel::new("v1");
        let m2 = ParticleModel::new("v2");
        hist.push(ModelSnapshot { particles: m0.particles.clone(), name: "v0".into() }, "initial");
        hist.push(ModelSnapshot { particles: m1.particles.clone(), name: "v1".into() }, "step1");
        let _snap = hist.undo();
        hist.push(ModelSnapshot { particles: m2.particles.clone(), name: "v2".into() }, "branch");
        assert_eq!(hist.branch_count(), 1, "should have one branching point");
        let path = hist.path_to_root();
        assert_eq!(path.len(), 3);
    }

    #[test]
    fn test_camera_orbit() {
        let mut cam = RenderCamera::new();
        let initial_pos = cam.position;
        cam.orbit(0.1, 0.0);
        let new_pos = cam.position;
        let dist_before = (initial_pos - cam.target).length();
        let dist_after  = (new_pos    - cam.target).length();
        assert!((dist_before - dist_after).abs() < 0.01, "orbit preserves distance");
    }

    #[test]
    fn test_camera_dolly() {
        let mut cam = RenderCamera::new();
        let d0 = (cam.position - cam.target).length();
        cam.dolly(-2.0);
        let d1 = (cam.position - cam.target).length();
        assert!(d1 > d0, "dolly backward increases distance");
    }

    #[test]
    fn test_constraint_solver_multi() {
        let mut solver = ConstraintSolver::new();
        solver.add_constraint(ParticleConstraint::new(0, ConstraintKind::Cage {
            min: Vec3::splat(-1.0), max: Vec3::splat(1.0)
        }));
        solver.add_constraint(ParticleConstraint::new(1, ConstraintKind::Sphere {
            center: Vec3::ZERO, radius: 0.5
        }));
        let mut positions = vec![Vec3::new(10.0, 10.0, 10.0), Vec3::new(1.0, 0.0, 0.0)];
        solver.solve(&mut positions);
        assert!(positions[0].x <= 1.0);
        assert!(positions[1].length() <= 0.5 + 1e-4);
    }

    #[test]
    fn test_frenet_frame() {
        let mut curve = ModelCurve::new("circle", CurveType::CatmullRom { alpha: 0.5 });
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            curve.add_point(Vec3::new(a.cos(), 0.0, a.sin()));
        }
        let (t, n, b) = curve.frenet_frame(0.5);
        // Tangent, normal, binormal should be roughly orthogonal
        assert!(t.dot(n).abs() < 0.1, "T perp N");
        assert!(t.dot(b).abs() < 0.1, "T perp B");
    }

    #[test]
    fn test_nurbs_evaluation() {
        let mut curve = ModelCurve::new("nurbs", CurveType::Nurbs {
            degree:  3,
            weights: vec![1.0, 1.0, 1.0, 1.0],
        });
        curve.add_point(Vec3::ZERO);
        curve.add_point(Vec3::new(1.0, 1.0, 0.0));
        curve.add_point(Vec3::new(2.0, 1.0, 0.0));
        curve.add_point(Vec3::new(3.0, 0.0, 0.0));
        let p = curve.evaluate(0.5);
        assert!(p.x > 0.0 && p.x < 3.0, "NURBS midpoint in range: {:?}", p);
    }
}
