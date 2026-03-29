
// ============================================================
// KD-TREE FOR FAST NEAREST NEIGHBOR SEARCH
// ============================================================

#[derive(Clone, Debug)]
pub struct KdNode {
    pub position:     Vec3,
    pub particle_idx: usize,
    pub axis:         u8,
    pub left:         Option<Box<KdNode>>,
    pub right:        Option<Box<KdNode>>,
}

impl KdNode {
    fn new(position: Vec3, particle_idx: usize, axis: u8) -> Self {
        Self { position, particle_idx, axis, left: None, right: None }
    }
}

pub struct KdTree {
    pub root: Option<Box<KdNode>>,
    pub size: usize,
}

impl KdTree {
    pub fn new() -> Self { Self { root: None, size: 0 } }

    pub fn build(particles: &[ModelParticle]) -> Self {
        let mut indexed: Vec<(usize, Vec3)> = particles.iter().enumerate()
            .map(|(i, p)| (i, p.position)).collect();
        let root = Self::build_recursive(&mut indexed, 0);
        Self { root, size: particles.len() }
    }

    fn build_recursive(points: &mut [(usize, Vec3)], depth: usize) -> Option<Box<KdNode>> {
        if points.is_empty() { return None; }
        let axis = (depth % 3) as u8;
        points.sort_by(|a, b| {
            let va = match axis { 0 => a.1.x, 1 => a.1.y, _ => a.1.z };
            let vb = match axis { 0 => b.1.x, 1 => b.1.y, _ => b.1.z };
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mid = points.len() / 2;
        let (idx, pos) = points[mid];
        let mut node = Box::new(KdNode::new(pos, idx, axis));
        node.left  = Self::build_recursive(&mut points[..mid], depth + 1);
        node.right = Self::build_recursive(&mut points[mid+1..], depth + 1);
        Some(node)
    }

    pub fn k_nearest(&self, query: Vec3, k: usize) -> Vec<(usize, f32)> {
        let mut heap: Vec<(f32, usize)> = Vec::new();
        if let Some(root) = &self.root { Self::search_knn(root, query, k, &mut heap); }
        heap.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        heap.into_iter().map(|(d, i)| (i, d.sqrt())).collect()
    }

    fn search_knn(node: &KdNode, query: Vec3, k: usize, heap: &mut Vec<(f32, usize)>) {
        let dist_sq = (node.position - query).length_squared();
        if heap.len() < k {
            heap.push((dist_sq, node.particle_idx));
        } else {
            let worst = heap.iter().map(|(d, _)| *d).fold(0.0f32, f32::max);
            if dist_sq < worst {
                if let Some(pos) = heap.iter().position(|(d, _)| *d == worst) {
                    heap[pos] = (dist_sq, node.particle_idx);
                }
            }
        }
        let split_val = match node.axis {
            0 => query.x - node.position.x,
            1 => query.y - node.position.y,
            _ => query.z - node.position.z,
        };
        let (near, far) = if split_val <= 0.0 { (&node.left, &node.right) } else { (&node.right, &node.left) };
        if let Some(n) = near { Self::search_knn(n, query, k, heap); }
        let worst_dist = heap.iter().map(|(d, _)| *d).fold(0.0f32, f32::max);
        if heap.len() < k || split_val * split_val < worst_dist {
            if let Some(f) = far { Self::search_knn(f, query, k, heap); }
        }
    }

    pub fn range_search(&self, query: Vec3, radius: f32) -> Vec<usize> {
        let mut results = Vec::new();
        if let Some(root) = &self.root { Self::search_range(root, query, radius * radius, &mut results); }
        results
    }

    fn search_range(node: &KdNode, query: Vec3, radius_sq: f32, out: &mut Vec<usize>) {
        if (node.position - query).length_squared() <= radius_sq { out.push(node.particle_idx); }
        let split_dist = match node.axis {
            0 => query.x - node.position.x,
            1 => query.y - node.position.y,
            _ => query.z - node.position.z,
        };
        if let Some(left) = &node.left {
            if split_dist <= 0.0 || split_dist * split_dist <= radius_sq {
                Self::search_range(left, query, radius_sq, out);
            }
        }
        if let Some(right) = &node.right {
            if split_dist >= 0.0 || split_dist * split_dist <= radius_sq {
                Self::search_range(right, query, radius_sq, out);
            }
        }
    }
}

impl Default for KdTree { fn default() -> Self { Self::new() } }

// ============================================================
// PARTICLE MESH (surface topology)
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ParticleMesh {
    pub vertices:      Vec<usize>,
    pub edges:         Vec<(usize, usize)>,
    pub faces:         Vec<[usize; 3]>,
    pub vert_to_faces: HashMap<usize, Vec<usize>>,
    pub vert_to_edges: HashMap<usize, Vec<usize>>,
}

impl ParticleMesh {
    pub fn new() -> Self { Self::default() }

    pub fn build_from_particles(particles: &[ModelParticle], max_edge_len: f32) -> Self {
        let mut mesh = Self::new();
        let n = particles.len();
        mesh.vertices = (0..n).collect();
        let max2 = max_edge_len * max_edge_len;
        for i in 0..n {
            for j in (i+1)..n {
                if (particles[i].position - particles[j].position).length_squared() <= max2 {
                    let eid = mesh.edges.len();
                    mesh.edges.push((i, j));
                    mesh.vert_to_edges.entry(i).or_default().push(eid);
                    mesh.vert_to_edges.entry(j).or_default().push(eid);
                }
            }
        }
        for eid1 in 0..mesh.edges.len() {
            let (a, b) = mesh.edges[eid1];
            let a_n: HashSet<usize> = mesh.vert_to_edges.get(&a)
                .map(|eids| eids.iter().map(|&e| { let (ea, eb) = mesh.edges[e]; if ea == a { eb } else { ea } }).collect())
                .unwrap_or_default();
            let b_n: HashSet<usize> = mesh.vert_to_edges.get(&b)
                .map(|eids| eids.iter().map(|&e| { let (ea, eb) = mesh.edges[e]; if ea == b { eb } else { ea } }).collect())
                .unwrap_or_default();
            for &c in a_n.intersection(&b_n) {
                let mut tri = [a, b, c];
                tri.sort_unstable();
                if !mesh.faces.iter().any(|f| f == &tri) {
                    let fid = mesh.faces.len();
                    mesh.faces.push(tri);
                    for &v in &tri { mesh.vert_to_faces.entry(v).or_default().push(fid); }
                }
            }
        }
        mesh
    }

    pub fn face_normal(&self, face_idx: usize, particles: &[ModelParticle]) -> Vec3 {
        let [a, b, c] = self.faces[face_idx];
        (particles[b].position - particles[a].position)
            .cross(particles[c].position - particles[a].position).normalize()
    }

    pub fn vertex_normal(&self, vert_idx: usize, particles: &[ModelParticle]) -> Vec3 {
        match self.vert_to_faces.get(&vert_idx) {
            None => Vec3::Y,
            Some(fids) => {
                let sum = fids.iter().map(|&fi| self.face_normal(fi, particles)).fold(Vec3::ZERO, |a, b| a + b);
                sum.normalize()
            }
        }
    }

    pub fn laplacian_smooth_step(&self, particles: &mut Vec<ModelParticle>, strength: f32) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        for &vi in &self.vertices {
            if particles[vi].locked { continue; }
            if let Some(eids) = self.vert_to_edges.get(&vi) {
                let mut sum = Vec3::ZERO; let mut cnt = 0usize;
                for &eid in eids {
                    let (a, b) = self.edges[eid];
                    sum += positions[if a == vi { b } else { a }]; cnt += 1;
                }
                if cnt > 0 { particles[vi].position = positions[vi].lerp(sum / cnt as f32, strength); }
            }
        }
    }

    pub fn average_edge_length(&self, particles: &[ModelParticle]) -> f32 {
        if self.edges.is_empty() { return 0.0; }
        self.edges.iter().map(|&(a, b)| (particles[a].position - particles[b].position).length())
            .sum::<f32>() / self.edges.len() as f32
    }

    pub fn boundary_vertices(&self) -> Vec<usize> {
        self.vertices.iter().copied().filter(|&v| {
            self.vert_to_edges.get(&v).map(|eids| eids.iter().any(|&eid| {
                let (a, b) = self.edges[eid];
                self.faces.iter().filter(|f| f.contains(&a) && f.contains(&b)).count() == 1
            })).unwrap_or(false)
        }).collect()
    }
}

// ============================================================
// REMESHING
// ============================================================

pub struct Remesher;

impl Remesher {
    pub fn resample_poisson(model: &mut ParticleModel, target_density: f32, character: char, color: Vec4) {
        let bounds = model.bounds.clone();
        let size = bounds.size();
        let target_count = (size.x * size.y * size.z * target_density) as usize;
        if target_count == 0 { return; }
        let existing: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
        if existing.is_empty() { return; }
        let min_dist = (1.0 / target_density.max(EPSILON)).cbrt();
        let min_dist2 = min_dist * min_dist;
        let mut placed: Vec<Vec3> = Vec::new();
        let mut candidates: VecDeque<Vec3> = existing.iter().cloned().collect();
        let mut attempts = 0usize;
        while let Some(candidate) = candidates.pop_front() {
            if attempts > target_count * 10 { break; }
            attempts += 1;
            if placed.iter().all(|&q| (candidate - q).length_squared() >= min_dist2) {
                placed.push(candidate);
                if placed.len() >= target_count { break; }
                for k in 0i32..4 {
                    let offset = Vec3::new(
                        (hash_2d(placed.len() as i32 * 3 + k, attempts as i32) * 2.0 - 1.0) * min_dist * 2.0,
                        (hash_2d(placed.len() as i32 * 7 + k, attempts as i32 + 1) * 2.0 - 1.0) * min_dist * 2.0,
                        (hash_2d(placed.len() as i32 * 11 + k, attempts as i32 + 2) * 2.0 - 1.0) * min_dist * 2.0,
                    );
                    let nc = candidate + offset;
                    if bounds.contains(nc) { candidates.push_back(nc); }
                }
            }
        }
        model.particles.clear();
        model.layers.iter_mut().for_each(|l| l.particle_indices.clear());
        model.add_particles_bulk(placed.into_iter().map(|p| ModelParticle::new(p, character, color)).collect());
    }

    pub fn adaptive_subdivide(model: &mut ParticleModel, max_edge_len: f32) {
        let existing: Vec<ModelParticle> = model.particles.clone();
        let max2 = max_edge_len * max_edge_len;
        let mut new_mids: Vec<ModelParticle> = Vec::new();
        for i in 0..existing.len() {
            for j in (i+1)..existing.len() {
                let d2 = (existing[i].position - existing[j].position).length_squared();
                if d2 > max2 && d2 < max2 * 4.0 {
                    let mid_pos = (existing[i].position + existing[j].position) * 0.5;
                    let mut mp = ModelParticle::new(mid_pos, existing[i].character, existing[i].color.lerp(existing[j].color, 0.5));
                    mp.normal = (existing[i].normal + existing[j].normal).normalize();
                    new_mids.push(mp);
                }
            }
        }
        model.add_particles_bulk(new_mids);
    }

    pub fn decimate(model: &mut ParticleModel, min_dist: f32) {
        let n = model.particles.len();
        let min2 = min_dist * min_dist;
        let mut keep = vec![true; n];
        for i in 0..n {
            if !keep[i] { continue; }
            for j in (i+1)..n {
                if keep[j] && (model.particles[i].position - model.particles[j].position).length_squared() < min2 {
                    keep[j] = false;
                }
            }
        }
        let to_remove: HashSet<usize> = keep.iter().enumerate().filter(|(_, &k)| !k).map(|(i, _)| i).collect();
        model.remove_particles(&to_remove);
    }

    pub fn isotropic_remesh(model: &mut ParticleModel, target_edge_len: f32, iterations: usize) {
        for _ in 0..iterations {
            Self::adaptive_subdivide(model, target_edge_len * 1.5);
            Self::decimate(model, target_edge_len * 0.5);
            AdvancedSculpt::global_relax(model, 2, 0.3, target_edge_len * 2.0);
        }
    }
}

// ============================================================
// 3D NOISE GENERATORS
// ============================================================

fn hash_3d(x: i32, y: i32, z: i32) -> f32 {
    let n = x.wrapping_mul(1619).wrapping_add(y.wrapping_mul(31337))
             .wrapping_add(z.wrapping_mul(6271)).wrapping_add(1013904223);
    let n = n.wrapping_mul(1664525).wrapping_add(1013904223);
    ((n as u32) as f32) / (u32::MAX as f32)
}

pub struct NoiseGenerator;

impl NoiseGenerator {
    pub fn value_3d(x: f32, y: f32, z: f32) -> f32 {
        let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        let (xf, yf, zf) = (smoothstep(0.0, 1.0, x - xi as f32), smoothstep(0.0, 1.0, y - yi as f32), smoothstep(0.0, 1.0, z - zi as f32));
        let c000 = hash_3d(xi,   yi,   zi  ); let c100 = hash_3d(xi+1, yi,   zi  );
        let c010 = hash_3d(xi,   yi+1, zi  ); let c110 = hash_3d(xi+1, yi+1, zi  );
        let c001 = hash_3d(xi,   yi,   zi+1); let c101 = hash_3d(xi+1, yi,   zi+1);
        let c011 = hash_3d(xi,   yi+1, zi+1); let c111 = hash_3d(xi+1, yi+1, zi+1);
        let x00 = c000 + xf * (c100 - c000); let x10 = c010 + xf * (c110 - c010);
        let x01 = c001 + xf * (c101 - c001); let x11 = c011 + xf * (c111 - c011);
        let y0 = x00 + yf * (x10 - x00); let y1 = x01 + yf * (x11 - x01);
        y0 + zf * (y1 - y0)
    }

    pub fn fbm(x: f32, y: f32, z: f32, octaves: usize, lacunarity: f32, gain: f32) -> f32 {
        let (mut value, mut amp, mut freq) = (0.0f32, 0.5f32, 1.0f32);
        for _ in 0..octaves {
            value += amp * (Self::value_3d(x * freq, y * freq, z * freq) * 2.0 - 1.0);
            freq *= lacunarity; amp *= gain;
        }
        value * 0.5 + 0.5
    }

    pub fn turbulence(x: f32, y: f32, z: f32, octaves: usize) -> f32 {
        let (mut value, mut amp, mut freq) = (0.0f32, 0.5f32, 1.0f32);
        for _ in 0..octaves {
            value += amp * (Self::value_3d(x * freq, y * freq, z * freq) * 2.0 - 1.0).abs();
            freq *= 2.0; amp *= 0.5;
        }
        value
    }

    pub fn displace_fbm(model: &mut ParticleModel, indices: &HashSet<usize>, scale: f32, amplitude: f32, octaves: usize) {
        for &i in indices {
            if let Some(p) = model.particles.get_mut(i) {
                if p.locked { continue; }
                let n = Self::fbm(p.position.x * scale, p.position.y * scale, p.position.z * scale, octaves, 2.0, 0.5);
                p.position += p.normal * (n * 2.0 - 1.0) * amplitude;
            }
        }
        model.recompute_bounds();
    }

    pub fn domain_warp(x: f32, y: f32, z: f32, ws: f32) -> f32 {
        let wx = Self::fbm(x + 1.7, y + 9.2, z + 5.5, 4, 2.0, 0.5);
        let wy = Self::fbm(x + 8.3, y + 2.8, z + 1.2, 4, 2.0, 0.5);
        let wz = Self::fbm(x + 3.1, y + 6.4, z + 7.8, 4, 2.0, 0.5);
        Self::fbm(x + ws * wx, y + ws * wy, z + ws * wz, 4, 2.0, 0.5)
    }
}

// ============================================================
// PARTICLE MATERIAL SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub enum MaterialType { Flat, Emissive, Metallic, Subsurface, Toon, Hologram }

#[derive(Clone, Debug)]
pub struct ParticleMaterial {
    pub name:       String,
    pub mat_type:   MaterialType,
    pub base_color: Vec4,
    pub emission:   f32,
    pub roughness:  f32,
    pub char_set:   Vec<char>,
}

impl ParticleMaterial {
    pub fn new(name: impl Into<String>, mat_type: MaterialType) -> Self {
        Self { name: name.into(), mat_type, base_color: Vec4::ONE, emission: 0.0, roughness: 0.5,
               char_set: vec![' ', '.', ':', ';', '+', '*', '#', '@'] }
    }

    pub fn shade(&self, normal: Vec3, light_dir: Vec3, view_dir: Vec3) -> Vec4 {
        let (n, l, v) = (normal.normalize(), light_dir.normalize(), view_dir.normalize());
        let h = (l + v).normalize();
        match self.mat_type {
            MaterialType::Flat      => self.base_color,
            MaterialType::Emissive  => self.base_color * (1.0 + self.emission),
            MaterialType::Metallic  => {
                let nd = n.dot(l).max(0.0);
                let sp = n.dot(h).max(0.0).powf(1.0 / (self.roughness * self.roughness + EPSILON));
                Vec4::new(self.base_color.x * nd + sp, self.base_color.y * nd + sp, self.base_color.z * nd + sp, 1.0)
            }
            MaterialType::Subsurface => { let w = (n.dot(l) + 0.5) / 1.5; self.base_color * w }
            MaterialType::Toon       => {
                let nd = n.dot(l);
                let t = if nd > 0.8 { 1.0 } else if nd > 0.3 { 0.6 } else { 0.2 };
                self.base_color * t
            }
            MaterialType::Hologram   => {
                let f = (1.0 - n.dot(v).abs()).powi(3);
                Vec4::new(self.base_color.x * f, self.base_color.y * f, self.base_color.z * f, f)
            }
        }
    }

    pub fn glyph_for_shade(&self, shade: f32) -> char {
        if self.char_set.is_empty() { return '.'; }
        let idx = ((shade.clamp(0.0, 1.0) * (self.char_set.len() as f32 - 1.0)).round() as usize).min(self.char_set.len() - 1);
        self.char_set[idx]
    }

    pub fn apply_to_particle(&self, p: &mut ModelParticle, light_dir: Vec3, view_dir: Vec3) {
        let shaded = self.shade(p.normal, light_dir, view_dir);
        p.color = shaded; p.emission = self.emission;
        p.character = self.glyph_for_shade((shaded.x + shaded.y + shaded.z) / 3.0);
    }
}

#[derive(Clone, Debug, Default)]
pub struct MaterialLibrary { pub materials: HashMap<String, ParticleMaterial> }

impl MaterialLibrary {
    pub fn new() -> Self { let mut l = Self::default(); l.add_defaults(); l }

    fn add_defaults(&mut self) {
        self.materials.insert("default".into(), ParticleMaterial::new("default", MaterialType::Flat));
        self.materials.insert("metal".into(),   ParticleMaterial::new("metal",   MaterialType::Metallic));
        let mut e = ParticleMaterial::new("emit", MaterialType::Emissive); e.emission = 2.0;
        self.materials.insert("emit".into(), e);
        self.materials.insert("toon".into(), ParticleMaterial::new("toon", MaterialType::Toon));
        self.materials.insert("holo".into(), ParticleMaterial::new("holo", MaterialType::Hologram));
    }

    pub fn add(&mut self, mat: ParticleMaterial) { self.materials.insert(mat.name.clone(), mat); }
    pub fn get(&self, name: &str) -> Option<&ParticleMaterial> { self.materials.get(name) }

    pub fn apply_to_model(&self, model: &mut ParticleModel, name: &str, light: Vec3, view: Vec3) {
        if let Some(mat) = self.get(name) {
            let mat = mat.clone();
            for p in &mut model.particles { mat.apply_to_particle(p, light, view); }
        }
    }
}

// ============================================================
// SCULPT MASK
// ============================================================

#[derive(Clone, Debug)]
pub struct SculptMask { pub values: Vec<f32>, pub count: usize }

impl SculptMask {
    pub fn new(count: usize) -> Self { Self { values: vec![1.0; count], count } }

    pub fn from_selection(sel: &HashSet<usize>, total: usize) -> Self {
        let mut m = Self::new(total);
        for i in 0..total { m.values[i] = if sel.contains(&i) { 1.0 } else { 0.0 }; }
        m
    }

    pub fn invert(&mut self) { for v in &mut self.values { *v = 1.0 - *v; } }
    pub fn fill(&mut self, value: f32) { for v in &mut self.values { *v = value.clamp(0.0, 1.0); } }
    pub fn paint(&mut self, idx: usize, value: f32) { if let Some(v) = self.values.get_mut(idx) { *v = value.clamp(0.0, 1.0); } }

    pub fn blur(&mut self, particles: &[ModelParticle], radius: f32, iters: usize) {
        let r2 = radius * radius;
        for _ in 0..iters {
            let prev = self.values.clone();
            for i in 0..self.count {
                let pi = particles.get(i).map(|p| p.position).unwrap_or(Vec3::ZERO);
                let (mut sum, mut cnt) = (0.0f32, 0usize);
                for (j, &v) in prev.iter().enumerate() {
                    if let Some(pj) = particles.get(j) {
                        if (pj.position - pi).length_squared() <= r2 { sum += v; cnt += 1; }
                    }
                }
                if cnt > 0 { self.values[i] = sum / cnt as f32; }
            }
        }
    }

    pub fn to_selection(&self, threshold: f32) -> HashSet<usize> {
        self.values.iter().enumerate().filter(|(_, &v)| v >= threshold).map(|(i, _)| i).collect()
    }

    pub fn combine_multiply(&mut self, other: &SculptMask) {
        for (a, &b) in self.values.iter_mut().zip(other.values.iter()) { *a *= b; }
    }
}

// ============================================================
// PARTICLE DELTA
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ParticleDelta {
    pub modified: Vec<(usize, Vec3, Vec3)>,
    pub added:    Vec<ModelParticle>,
    pub removed:  Vec<(usize, ModelParticle)>,
}

impl ParticleDelta {
    pub fn new() -> Self { Self::default() }

    pub fn compute(before: &[ModelParticle], after: &[ModelParticle]) -> Self {
        let mut d = Self::new();
        let min_len = before.len().min(after.len());
        for i in 0..min_len {
            if (before[i].position - after[i].position).length_squared() > EPSILON * EPSILON {
                d.modified.push((i, before[i].position, after[i].position));
            }
        }
        if after.len() > before.len() { for i in before.len()..after.len() { d.added.push(after[i].clone()); } }
        else if before.len() > after.len() { for i in after.len()..before.len() { d.removed.push((i, before[i].clone())); } }
        d
    }

    pub fn is_empty(&self) -> bool { self.modified.is_empty() && self.added.is_empty() && self.removed.is_empty() }
    pub fn memory_estimate(&self) -> usize { self.modified.len() * 28 + self.added.len() * std::mem::size_of::<ModelParticle>() }
}

// ============================================================
// STENCIL
// ============================================================

#[derive(Clone, Debug)]
pub struct Stencil { pub name: String, pub width: usize, pub height: usize, pub data: Vec<f32> }

impl Stencil {
    pub fn new(name: impl Into<String>, w: usize, h: usize) -> Self {
        Self { name: name.into(), width: w, height: h, data: vec![0.0; w * h] }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, v: f32) {
        if x < self.width && y < self.height { self.data[y * self.width + x] = v.clamp(0.0, 1.0); }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height { self.data[y * self.width + x] } else { 0.0 }
    }

    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let x = u * (self.width as f32 - 1.0);  let y = v * (self.height as f32 - 1.0);
        let xi = x.floor() as usize; let yi = y.floor() as usize;
        let xt = x - xi as f32; let yt = y - yi as f32;
        let xi2 = (xi + 1).min(self.width - 1); let yi2 = (yi + 1).min(self.height - 1);
        let c00 = self.get_pixel(xi, yi); let c10 = self.get_pixel(xi2, yi);
        let c01 = self.get_pixel(xi, yi2); let c11 = self.get_pixel(xi2, yi2);
        (c00 + xt * (c10 - c00)) + yt * ((c01 + xt * (c11 - c01)) - (c00 + xt * (c10 - c00)))
    }

    pub fn circle(name: impl Into<String>, res: usize) -> Self {
        let mut s = Self::new(name, res, res);
        let c = res as f32 / 2.0;
        for y in 0..res {
            for x in 0..res {
                let dx = x as f32 - c; let dy = y as f32 - c;
                s.set_pixel(x, y, smoothstep(0.0, 1.0, (1.0 - (dx*dx + dy*dy).sqrt() / c.max(EPSILON)).clamp(0.0, 1.0)));
            }
        }
        s
    }
}

// ============================================================
// GROUP MANAGER
// ============================================================

#[derive(Clone, Debug)]
pub struct GroupInfo { pub id: u32, pub name: String, pub visible: bool, pub locked: bool, pub color: Vec4 }

impl GroupInfo {
    pub fn new(id: u32, name: impl Into<String>, color: Vec4) -> Self {
        Self { id, name: name.into(), visible: true, locked: false, color }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GroupManager { pub groups: BTreeMap<u32, GroupInfo>, pub next_id: u32 }

impl GroupManager {
    pub fn new() -> Self { Self::default() }

    pub fn create_group(&mut self, name: impl Into<String>, color: Vec4) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.groups.insert(id, GroupInfo::new(id, name, color)); id
    }

    pub fn get_group(&self, id: u32) -> Option<&GroupInfo> { self.groups.get(&id) }
    pub fn set_visibility(&mut self, id: u32, v: bool) { if let Some(g) = self.groups.get_mut(&id) { g.visible = v; } }
    pub fn set_lock(&mut self, id: u32, l: bool) { if let Some(g) = self.groups.get_mut(&id) { g.locked = l; } }

    pub fn visible_groups(&self) -> Vec<u32> {
        self.groups.values().filter(|g| g.visible).map(|g| g.id).collect()
    }

    pub fn is_particle_active(&self, p: &ModelParticle) -> bool {
        self.groups.get(&p.group_id).map(|g| g.visible && !g.locked).unwrap_or(true)
    }
}

// ============================================================
// PROCEDURAL PATTERNS
// ============================================================

pub struct ProceduralPatterns;

impl ProceduralPatterns {
    pub fn voronoi(model: &mut ParticleModel, seeds: &[Vec3], colors: &[Vec4]) {
        if seeds.is_empty() { return; }
        for p in &mut model.particles {
            let (best, _) = seeds.iter().enumerate()
                .map(|(i, &s)| (i, (p.position - s).length_squared()))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0, 0.0));
            p.color = colors.get(best).copied().unwrap_or(Vec4::ONE);
        }
    }

    pub fn stripes(model: &mut ParticleModel, axis: Vec3, width: f32, ca: Vec4, cb: Vec4) {
        let n = axis.normalize();
        for p in &mut model.particles {
            p.color = if (n.dot(p.position) / width.max(EPSILON)).floor() as i32 % 2 == 0 { ca } else { cb };
        }
    }

    pub fn checkerboard(model: &mut ParticleModel, cell: f32, ca: Vec4, cb: Vec4) {
        for p in &mut model.particles {
            let xi = (p.position.x / cell.max(EPSILON)).floor() as i32;
            let yi = (p.position.y / cell.max(EPSILON)).floor() as i32;
            let zi = (p.position.z / cell.max(EPSILON)).floor() as i32;
            p.color = if (xi + yi + zi) % 2 == 0 { ca } else { cb };
        }
    }

    pub fn gradient_along_axis(model: &mut ParticleModel, axis: Vec3, c0: Vec4, c1: Vec4) {
        let n = axis.normalize();
        let ts: Vec<f32> = model.particles.iter().map(|p| n.dot(p.position)).collect();
        let min_t = ts.iter().cloned().fold(f32::MAX, f32::min);
        let max_t = ts.iter().cloned().fold(f32::MIN, f32::max);
        let range = (max_t - min_t).max(EPSILON);
        for (i, p) in model.particles.iter_mut().enumerate() { p.color = c0.lerp(c1, (ts[i] - min_t) / range); }
    }

    pub fn radial_gradient(model: &mut ParticleModel, center: Vec3, radius: f32, cc: Vec4, ce: Vec4) {
        for p in &mut model.particles {
            p.color = cc.lerp(ce, ((p.position - center).length() / radius.max(EPSILON)).clamp(0.0, 1.0));
        }
    }

    pub fn reaction_diffusion_step(
        u: &mut Vec<f32>, v: &mut Vec<f32>, particles: &[ModelParticle],
        du: f32, dv: f32, feed: f32, kill: f32, dt: f32, radius: f32,
    ) {
        let n = particles.len(); let r2 = radius * radius;
        let u0 = u.clone(); let v0 = v.clone();
        for i in 0..n {
            let pi = particles[i].position; let ui = u0[i]; let vi = v0[i];
            let (mut lu, mut lv, mut cnt) = (0.0f32, 0.0f32, 0usize);
            for (j, (&uj, &vj)) in u0.iter().zip(v0.iter()).enumerate() {
                if j != i && (particles[j].position - pi).length_squared() <= r2 {
                    lu += uj - ui; lv += vj - vi; cnt += 1;
                }
            }
            if cnt > 0 { lu /= cnt as f32; lv /= cnt as f32; }
            let uvv = ui * vi * vi;
            u[i] = (ui + (du * lu - uvv + feed * (1.0 - ui)) * dt).clamp(0.0, 1.0);
            v[i] = (vi + (dv * lv + uvv - (kill + feed) * vi) * dt).clamp(0.0, 1.0);
        }
    }

    pub fn apply_rd_color(model: &mut ParticleModel, u: &[f32], v: &[f32], ca: Vec4, cb: Vec4) {
        for (i, p) in model.particles.iter_mut().enumerate() {
            let ui = u.get(i).copied().unwrap_or(1.0);
            let vi = v.get(i).copied().unwrap_or(0.0);
            p.color = ca.lerp(cb, ((ui - vi + 1.0) * 0.5).clamp(0.0, 1.0));
        }
    }
}

// ============================================================
// EASING + MODEL ANIMATION
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum EasingType { Linear, EaseIn, EaseOut, EaseInOut, Bounce, Elastic }

impl EasingType {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingType::Linear    => t,
            EasingType::EaseIn    => t * t,
            EasingType::EaseOut   => 1.0 - (1.0 - t) * (1.0 - t),
            EasingType::EaseInOut => smoothstep(0.0, 1.0, t),
            EasingType::Bounce    => {
                let t2 = 1.0 - t; let n = 7.5625f32; let d = 2.75f32;
                let v = if t2 < 1.0/d { n*t2*t2 }
                    else if t2 < 2.0/d { let t3 = t2 - 1.5/d; n*t3*t3 + 0.75 }
                    else if t2 < 2.5/d { let t3 = t2 - 2.25/d; n*t3*t3 + 0.9375 }
                    else { let t3 = t2 - 2.625/d; n*t3*t3 + 0.984375 };
                1.0 - v
            }
            EasingType::Elastic   => {
                if t == 0.0 || t == 1.0 { t }
                else { -(2.0f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * TAU / 3.0).sin() }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModelKeyframe { pub time: f32, pub snapshot: ModelSnapshot, pub easing: EasingType }

#[derive(Clone, Debug, Default)]
pub struct ModelAnimation { pub name: String, pub keyframes: Vec<ModelKeyframe>, pub duration: f32, pub looping: bool }

impl ModelAnimation {
    pub fn new(name: impl Into<String>) -> Self { Self { name: name.into(), ..Default::default() } }

    pub fn add_keyframe(&mut self, time: f32, snapshot: ModelSnapshot, easing: EasingType) {
        let kf = ModelKeyframe { time, snapshot, easing };
        let pos = self.keyframes.partition_point(|k| k.time < time);
        self.keyframes.insert(pos, kf);
        self.duration = self.keyframes.last().map(|k| k.time).unwrap_or(0.0);
    }

    pub fn evaluate(&self, time: f32, model: &mut ParticleModel) {
        if self.keyframes.is_empty() { return; }
        let time = if self.looping { time % self.duration.max(EPSILON) } else { time.min(self.duration) };
        let idx = self.keyframes.partition_point(|k| k.time <= time);
        if idx == 0 { self.keyframes[0].snapshot.restore_to(model); return; }
        if idx >= self.keyframes.len() { self.keyframes.last().unwrap().snapshot.restore_to(model); return; }
        let prev = &self.keyframes[idx - 1]; let next = &self.keyframes[idx];
        let t = next.easing.apply((time - prev.time) / (next.time - prev.time).max(EPSILON));
        let len = prev.snapshot.particles.len().min(next.snapshot.particles.len()).min(model.particles.len());
        for i in 0..len {
            model.particles[i].position = prev.snapshot.particles[i].position.lerp(next.snapshot.particles[i].position, t);
            model.particles[i].color    = prev.snapshot.particles[i].color.lerp(next.snapshot.particles[i].color, t);
        }
        model.recompute_bounds();
    }
}

// ============================================================
// EXTRA MATH HELPERS
// ============================================================

pub fn decompose_mat4(m: Mat4) -> (Vec3, Quat, Vec3) {
    let translation = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
    let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
    let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
    let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
    let rot = Mat4::from_cols(m.x_axis / sx, m.y_axis / sy, m.z_axis / sz, Vec4::new(0.0, 0.0, 0.0, 1.0));
    (translation, Quat::from_mat4(&rot), Vec3::new(sx, sy, sz))
}

pub fn euler_to_quat(roll: f32, pitch: f32, yaw: f32) -> Quat {
    Quat::from_euler(glam::EulerRot::XYZ, roll, pitch, yaw)
}

pub fn quat_to_euler(q: Quat) -> (f32, f32, f32) { q.to_euler(glam::EulerRot::XYZ) }

pub fn bounding_sphere(points: &[Vec3]) -> (Vec3, f32) {
    if points.is_empty() { return (Vec3::ZERO, 0.0); }
    let min_x = points.iter().min_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)).copied().unwrap_or(Vec3::ZERO);
    let max_x = points.iter().max_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)).copied().unwrap_or(Vec3::ZERO);
    let mut center = (min_x + max_x) * 0.5; let mut radius = (max_x - min_x).length() * 0.5;
    for &p in points {
        let d = (p - center).length();
        if d > radius { let nr = (radius + d) * 0.5; center += (p - center).normalize() * (nr - radius); radius = nr; }
    }
    (center, radius)
}

pub fn triangle_area(a: Vec3, b: Vec3, c: Vec3) -> f32 { (b - a).cross(c - a).length() * 0.5 }

pub fn point_in_polygon_xz(point: Vec3, polygon: &[Vec3]) -> bool {
    let n = polygon.len(); if n < 3 { return false; }
    let mut inside = false; let mut j = n - 1;
    for i in 0..n {
        let (xi, zi, xj, zj) = (polygon[i].x, polygon[i].z, polygon[j].x, polygon[j].z);
        if ((zi > point.z) != (zj > point.z)) && (point.x < (xj - xi) * (point.z - zi) / (zj - zi) + xi) { inside = !inside; }
        j = i;
    }
    inside
}

pub fn mesh_surface_area_2(mesh: &ParticleMesh, particles: &[ModelParticle]) -> f32 {
    mesh.faces.iter().map(|&[a, b, c]| triangle_area(particles[a].position, particles[b].position, particles[c].position)).sum()
}

// ============================================================
// MODEL QUALITY METRICS
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ModelQuality {
    pub particle_count:    usize,
    pub bounding_box_vol:  f32,
    pub density_variance:  f32,
    pub avg_neighbor_dist: f32,
    pub isolated_count:    usize,
    pub cluster_count:     usize,
    pub normal_consistency: f32,
}

impl ModelQuality {
    pub fn analyze(model: &ParticleModel, radius: f32) -> Self {
        let mut q = Self::default();
        q.particle_count = model.particles.len();
        q.bounding_box_vol = model.bounds.volume();
        if model.particles.is_empty() { return q; }
        let r2 = radius * radius;
        let positions: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
        let mut nc: Vec<usize> = vec![0; model.particles.len()];
        let (mut td, mut pc) = (0.0f32, 0usize);
        for i in 0..positions.len() {
            for j in (i+1)..positions.len() {
                let d2 = (positions[i] - positions[j]).length_squared();
                if d2 <= r2 { nc[i] += 1; nc[j] += 1; td += d2.sqrt(); pc += 1; }
            }
        }
        q.avg_neighbor_dist = if pc > 0 { td / pc as f32 } else { 0.0 };
        q.isolated_count = nc.iter().filter(|&&c| c == 0).count();
        let mean = nc.iter().sum::<usize>() as f32 / model.particles.len() as f32;
        q.density_variance = nc.iter().map(|&c| (c as f32 - mean).powi(2)).sum::<f32>() / model.particles.len() as f32;
        let mut nds = 0.0f32; let mut npc2 = 0usize;
        for i in 0..model.particles.len() {
            for j in (i+1)..model.particles.len() {
                if (positions[i] - positions[j]).length_squared() <= r2 {
                    nds += model.particles[i].normal.dot(model.particles[j].normal); npc2 += 1;
                }
            }
        }
        q.normal_consistency = if npc2 > 0 { nds / npc2 as f32 } else { 1.0 };
        let mut parent: Vec<usize> = (0..model.particles.len()).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize { if p[x] != x { p[x] = find(p, p[x]); } p[x] }
        for i in 0..model.particles.len() {
            for j in (i+1)..model.particles.len() {
                if (positions[i] - positions[j]).length_squared() <= r2 {
                    let ri = find(&mut parent, i); let rj = find(&mut parent, j);
                    if ri != rj { parent[ri] = rj; }
                }
            }
        }
        q.cluster_count = (0..model.particles.len()).map(|i| find(&mut parent, i)).collect::<HashSet<_>>().len();
        q
    }

    pub fn summary(&self) -> String {
        format!("Particles:{} Clusters:{} Isolated:{} AvgDist:{:.3} NormConsist:{:.3}",
            self.particle_count, self.cluster_count, self.isolated_count, self.avg_neighbor_dist, self.normal_consistency)
    }
}

// ============================================================
// BATCH PROCESSOR
// ============================================================

pub struct BatchProcessor;

impl BatchProcessor {
    pub fn process_all<F>(model: &mut ParticleModel, mut f: F) where F: FnMut(usize, &mut ModelParticle) {
        for (i, p) in model.particles.iter_mut().enumerate() { f(i, p); }
    }
    pub fn process_selected<F>(model: &mut ParticleModel, sel: &HashSet<usize>, mut f: F) where F: FnMut(usize, &mut ModelParticle) {
        for &i in sel { if let Some(p) = model.particles.get_mut(i) { f(i, p); } }
    }
    pub fn remap_characters(model: &mut ParticleModel, map: &HashMap<char, char>) {
        for p in &mut model.particles { if let Some(&nc) = map.get(&p.character) { p.character = nc; } }
    }
    pub fn clamp_to_bounds(model: &mut ParticleModel, aabb: &Aabb3) {
        for p in &mut model.particles { p.position = clamp_vec3(p.position, aabb.min, aabb.max); }
        model.recompute_bounds();
    }
    pub fn normalize_colors(model: &mut ParticleModel) {
        for p in &mut model.particles {
            let m = p.color.x.max(p.color.y).max(p.color.z).max(EPSILON);
            p.color = Vec4::new(p.color.x / m, p.color.y / m, p.color.z / m, p.color.w);
        }
    }
    pub fn count_where<F>(model: &ParticleModel, mut f: F) -> usize where F: FnMut(&ModelParticle) -> bool {
        model.particles.iter().filter(|p| f(p)).count()
    }
    pub fn quantize_colors(model: &mut ParticleModel, palette: &[Vec4]) {
        if palette.is_empty() { return; }
        for p in &mut model.particles {
            let best = palette.iter()
                .min_by(|a, b| (*a - p.color).length_squared().partial_cmp(&(*b - p.color).length_squared())
                    .unwrap_or(std::cmp::Ordering::Equal))
                .copied().unwrap_or(p.color);
            p.color = best;
        }
    }
}

// ============================================================
// LOD STREAMER
// ============================================================

pub struct LodStreamer {
    pub models_by_distance: BTreeMap<u64, f32>,
    pub active_lods:         HashMap<u64, usize>,
    pub lod_bias:            f32,
}

impl LodStreamer {
    pub fn new() -> Self { Self { models_by_distance: BTreeMap::new(), active_lods: HashMap::new(), lod_bias: 0.0 } }

    pub fn register_model(&mut self, id: u64, dist: f32) {
        self.models_by_distance.insert(id, dist); self.active_lods.insert(id, 0);
    }

    pub fn update_distances(&mut self, models: &HashMap<u64, ParticleModel>, camera: Vec3) {
        for (id, model) in models { self.models_by_distance.insert(*id, (model.bounds.center() - camera).length()); }
    }

    pub fn select_lods(&mut self, models: &HashMap<u64, ParticleModel>) {
        for (id, &dist) in &self.models_by_distance {
            if let Some(m) = models.get(id) { self.active_lods.insert(*id, m.select_lod(dist, self.lod_bias)); }
        }
    }

    pub fn get_lod(&self, id: u64) -> usize { self.active_lods.get(&id).copied().unwrap_or(0) }

    pub fn models_in_range(&self, max_dist: f32) -> Vec<u64> {
        self.models_by_distance.iter().filter(|(_, &d)| d <= max_dist).map(|(&id, _)| id).collect()
    }
}

impl Default for LodStreamer { fn default() -> Self { Self::new() } }

// ============================================================
// UNIFIED TOOL CONTEXT 2
// ============================================================

pub struct ModelingToolContext2 {
    pub editor:       ModelEditor,
    pub clipboard2:   Vec<ModelParticle>,
    pub clipboard_pivot: Vec3,
    pub presets2:     BTreeMap<String, BrushParams>,
    pub spline2:      Vec<Vec3>,
    pub material_lib: MaterialLibrary,
    pub group_mgr:    GroupManager,
    pub animations:   HashMap<String, ModelAnimation>,
}

impl ModelingToolContext2 {
    pub fn new() -> Self {
        Self {
            editor: ModelEditor::new(),
            clipboard2: Vec::new(),
            clipboard_pivot: Vec3::ZERO,
            presets2: BTreeMap::new(),
            spline2: Vec::new(),
            material_lib: MaterialLibrary::new(),
            group_mgr: GroupManager::new(),
            animations: HashMap::new(),
        }
    }

    pub fn copy_selection(&mut self) {
        let sel = self.editor.selection.clone();
        if let Some(m) = self.editor.active_model() {
            self.clipboard2 = sel.iter().filter_map(|&i| m.particles.get(i)).cloned().collect();
            self.clipboard_pivot = if self.clipboard2.is_empty() { Vec3::ZERO } else {
                self.clipboard2.iter().map(|p| p.position).fold(Vec3::ZERO, |a, b| a + b) / self.clipboard2.len() as f32
            };
        }
    }

    pub fn paste_selection(&mut self, target: Vec3) {
        if self.clipboard2.is_empty() { return; }
        let offset = target - self.clipboard_pivot;
        let new_particles: Vec<ModelParticle> = self.clipboard2.iter().map(|p| {
            let mut np = p.clone(); np.position += offset; np
        }).collect();
        if let Some(m) = self.editor.active_model_mut() { m.add_particles_bulk(new_particles); }
    }

    pub fn clipboard_count(&self) -> usize { self.clipboard2.len() }

    pub fn add_animation(&mut self, name: impl Into<String>) -> String {
        let n = name.into();
        self.animations.insert(n.clone(), ModelAnimation::new(n.as_str()));
        n
    }

    pub fn play_animation(&mut self, name: &str, time: f32) {
        if let Some(anim) = self.animations.get(name) {
            let anim = anim.clone();
            if let Some(m) = self.editor.active_model_mut() { anim.evaluate(time, m); }
        }
    }

    pub fn apply_material(&mut self, material_name: &str, light: Vec3, view: Vec3) {
        if let Some(mat) = self.material_lib.get(material_name) {
            let mat = mat.clone();
            if let Some(m) = self.editor.active_model_mut() {
                for p in &mut m.particles { mat.apply_to_particle(p, light, view); }
            }
        }
    }

    pub fn particle_count(&self) -> usize { self.editor.particle_count() }

    pub fn add_spline_point(&mut self, p: Vec3) { self.spline2.push(p); }
    pub fn clear_spline(&mut self) { self.spline2.clear(); }
}

impl Default for ModelingToolContext2 { fn default() -> Self { Self::new() } }

// ============================================================
// INTEGRATION TESTS
// ============================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_kdtree_range() {
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 50, '.', Vec4::ONE);
        let tree = KdTree::build(&particles);
        let r = tree.range_search(Vec3::ZERO, 0.5);
        for &i in &r { assert!(particles[i].position.length() <= 1.0 + EPSILON); }
    }

    #[test]
    fn test_knn_basic() {
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 50, '.', Vec4::ONE);
        let tree = KdTree::build(&particles);
        let knn = tree.k_nearest(Vec3::ZERO, 5);
        assert_eq!(knn.len(), 5);
    }

    #[test]
    fn test_noise_range() {
        for i in 0..20 {
            let v = NoiseGenerator::value_3d(i as f32 * 0.3, i as f32 * 0.7, i as f32 * 0.5);
            assert!(v >= 0.0 && v <= 1.0, "v={}", v);
        }
    }

    #[test]
    fn test_fbm_range() {
        for i in 0..10 {
            let v = NoiseGenerator::fbm(i as f32 * 0.5, i as f32 * 0.3, 0.7, 4, 2.0, 0.5);
            assert!(v >= 0.0 && v <= 1.0, "fbm={}", v);
        }
    }

    #[test]
    fn test_stencil_circle() {
        let s = Stencil::circle("c", 32);
        assert!(s.sample(0.5, 0.5) > 0.8);
        assert!(s.sample(0.0, 0.0) < 0.2);
    }

    #[test]
    fn test_easing_bounds() {
        for e in [EasingType::Linear, EasingType::EaseIn, EasingType::EaseOut, EasingType::EaseInOut] {
            assert!((e.apply(0.0) - 0.0).abs() < EPSILON, "{:?} at 0", e);
            assert!((e.apply(1.0) - 1.0).abs() < 0.01, "{:?} at 1={}", e, e.apply(1.0));
        }
    }

    #[test]
    fn test_particle_delta_compute() {
        let before = vec![ModelParticle::new(Vec3::ZERO, '.', Vec4::ONE), ModelParticle::new(Vec3::X, '.', Vec4::ONE)];
        let mut after = before.clone(); after[0].position = Vec3::new(1.0, 0.0, 0.0);
        let d = ParticleDelta::compute(&before, &after);
        assert_eq!(d.modified.len(), 1);
    }

    #[test]
    fn test_material_shade() {
        let mat = ParticleMaterial::new("t", MaterialType::Toon);
        let s = mat.shade(Vec3::Y, Vec3::Y, Vec3::Z);
        assert!(s.x > 0.5 || s.y > 0.5 || s.z > 0.5);
    }

    #[test]
    fn test_voronoi_colors() {
        let mut model = ParticleModel::new(1, "v");
        model.add_particles_bulk(PrimitiveBuilder::plane(Vec3::ZERO, 4.0, 4.0, 10, 10, 0.0, 0.0, '.', Vec4::ONE));
        let seeds = vec![Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
        let colors = vec![Vec4::new(1.0, 0.0, 0.0, 1.0), Vec4::new(0.0, 0.0, 1.0, 1.0)];
        ProceduralPatterns::voronoi(&mut model, &seeds, &colors);
        for p in &model.particles {
            let r = (p.color - colors[0]).length() < 0.01;
            let b = (p.color - colors[1]).length() < 0.01;
            assert!(r || b, "unexpected color {:?}", p.color);
        }
    }

    #[test]
    fn test_quality_analyze() {
        let mut model = ParticleModel::new(1, "q");
        model.add_particles_bulk(PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 30, '.', Vec4::ONE));
        model.recompute_bounds();
        let q = ModelQuality::analyze(&model, 0.5);
        assert_eq!(q.particle_count, 30);
        assert!(q.cluster_count >= 1);
    }

    #[test]
    fn test_batch_remap() {
        let mut model = ParticleModel::new(1, "b");
        model.add_particle(ModelParticle::new(Vec3::ZERO, 'A', Vec4::ONE));
        let mut map = HashMap::new(); map.insert('A', 'X');
        BatchProcessor::remap_characters(&mut model, &map);
        assert_eq!(model.particles[0].character, 'X');
    }

    #[test]
    fn test_group_manager() {
        let mut gm = GroupManager::new();
        let id = gm.create_group("fire", Vec4::new(1.0, 0.5, 0.0, 1.0));
        assert_eq!(gm.get_group(id).unwrap().name, "fire");
        gm.set_visibility(id, false);
        assert!(gm.visible_groups().is_empty());
    }

    #[test]
    fn test_bounding_sphere_coverage() {
        let pts = vec![Vec3::new(1.0,0.0,0.0), Vec3::new(-1.0,0.0,0.0), Vec3::new(0.0,1.0,0.0), Vec3::new(0.0,-1.0,0.0)];
        let (c, r) = bounding_sphere(&pts);
        assert!(r >= 1.0 - EPSILON);
        for &p in &pts { assert!((p - c).length() <= r + 0.01); }
    }

    #[test]
    fn test_sculpt_mask_operations() {
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 20, '.', Vec4::ONE);
        let mut mask = SculptMask::new(particles.len());
        mask.values[0] = 1.0;
        for i in 1..mask.count { mask.values[i] = 0.0; }
        mask.blur(&particles, 0.5, 3);
        let nonzero = mask.values.iter().filter(|&&v| v > 0.0).count();
        assert!(nonzero >= 1);
        let sel = mask.to_selection(0.01);
        assert!(!sel.is_empty());
    }

    #[test]
    fn test_lod_streamer_basic() {
        let mut editor = ModelEditor::new();
        let id = editor.create_model("m");
        editor.insert_sphere(Vec3::ZERO, 1.0, 100);
        editor.generate_lods();
        let mut streamer = LodStreamer::new();
        streamer.register_model(id, 0.0);
        streamer.update_distances(&editor.models, Vec3::new(5.0, 0.0, 0.0));
        streamer.select_lods(&editor.models);
        assert!(streamer.get_lod(id) <= 3);
    }

    #[test]
    fn test_modeling_context2_copy_paste() {
        let mut ctx = ModelingToolContext2::new();
        ctx.editor.create_model("ctx");
        ctx.editor.insert_sphere(Vec3::ZERO, 1.0, 40);
        assert_eq!(ctx.particle_count(), 40);
        ctx.editor.select_all();
        ctx.copy_selection();
        assert_eq!(ctx.clipboard_count(), 40);
        ctx.paste_selection(Vec3::new(3.0, 0.0, 0.0));
        assert_eq!(ctx.particle_count(), 80);
    }

    #[test]
    fn test_particle_mesh_build() {
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 20, '.', Vec4::ONE);
        let mesh = ParticleMesh::build_from_particles(&particles, 0.8);
        assert!(!mesh.edges.is_empty());
        assert!(mesh.average_edge_length(&particles) > 0.0);
    }

    #[test]
    fn test_remesher_decimate() {
        let mut model = ParticleModel::new(1, "r");
        model.add_particles_bulk(PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 100, '.', Vec4::ONE));
        let before = model.particles.len();
        Remesher::decimate(&mut model, 0.3);
        assert!(model.particles.len() < before);
    }

    #[test]
    fn test_model_animation_evaluate() {
        let mut model = ParticleModel::new(1, "a");
        model.add_particle(ModelParticle::new(Vec3::ZERO, '.', Vec4::ONE));
        let mut anim = ModelAnimation::new("test");
        let snap0 = ModelSnapshot::capture(&model, "k0");
        model.particles[0].position = Vec3::new(1.0, 0.0, 0.0);
        let snap1 = ModelSnapshot::capture(&model, "k1");
        model.particles[0].position = Vec3::ZERO;
        anim.add_keyframe(0.0, snap0, EasingType::Linear);
        anim.add_keyframe(1.0, snap1, EasingType::Linear);
        anim.evaluate(0.5, &mut model);
        assert!((model.particles[0].position.x - 0.5).abs() < 0.01, "x={}", model.particles[0].position.x);
    }

    #[test]
    fn test_reaction_diffusion_step() {
        let particles = PrimitiveBuilder::plane(Vec3::ZERO, 2.0, 2.0, 5, 5, 0.0, 0.0, '.', Vec4::ONE);
        let n = particles.len();
        let mut u = vec![1.0f32; n];
        let mut v = vec![0.0f32; n];
        v[0] = 0.5;
        ProceduralPatterns::reaction_diffusion_step(&mut u, &mut v, &particles, 1.0, 0.5, 0.055, 0.062, 0.1, 0.5);
        // u values should still be in [0,1]
        for &x in &u { assert!(x >= 0.0 && x <= 1.0); }
    }

    #[test]
    fn test_euler_quat() {
        let q = euler_to_quat(0.0, 0.0, PI / 2.0);
        let (r, p, y) = quat_to_euler(q);
        assert!(y.abs() - PI / 2.0 < 0.01 || (r.abs() + p.abs()).abs() < 0.01);
    }

    #[test]
    fn test_triangle_area() {
        let area = triangle_area(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        assert!((area - 0.5).abs() < EPSILON);
    }
}
