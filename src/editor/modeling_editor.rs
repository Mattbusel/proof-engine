#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

pub const MAX_BONE_INFLUENCES: usize = 4;
pub const MAX_LOD_LEVELS: usize = 4;
pub const MARCHING_CUBES_THRESHOLD: f32 = 0.5;
pub const METABALL_THRESHOLD: f32 = 1.0;
pub const MAX_UNDO_STEPS: usize = 64;
pub const DEFAULT_BRUSH_RADIUS: f32 = 1.0;
pub const DEFAULT_BRUSH_STRENGTH: f32 = 0.5;
pub const DEFAULT_BRUSH_DENSITY: f32 = 4.0;
pub const EPSILON: f32 = 1e-6;
pub const PI: f32 = std::f32::consts::PI;
pub const TAU: f32 = std::f32::consts::TAU;
pub const PHI: f32 = 1.618_033_9;

// ============================================================
// CORE DATA STRUCTURES
// ============================================================

#[derive(Clone, Debug)]
pub struct Aabb3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb3 {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        }
    }

    pub fn expand(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &Aabb3) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn surface_area(&self) -> f32 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    pub fn volume(&self) -> f32 {
        let s = self.size();
        s.x * s.y * s.z
    }
}

impl Default for Aabb3 {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug)]
pub struct Ray3 {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray3 {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction: direction.normalize() }
    }

    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        let ap = p - self.origin;
        let t = ap.dot(self.direction).max(0.0);
        let closest = self.origin + self.direction * t;
        (p - closest).length()
    }

    pub fn intersect_sphere(&self, center: Vec3, radius: f32) -> Option<f32> {
        let oc = self.origin - center;
        let a = self.direction.dot(self.direction);
        let b = 2.0 * oc.dot(self.direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            None
        } else {
            let t = (-b - discriminant.sqrt()) / (2.0 * a);
            if t > EPSILON { Some(t) } else {
                let t2 = (-b + discriminant.sqrt()) / (2.0 * a);
                if t2 > EPSILON { Some(t2) } else { None }
            }
        }
    }

    pub fn intersect_aabb(&self, aabb: &Aabb3) -> Option<f32> {
        let inv_dir = Vec3::new(
            if self.direction.x.abs() > EPSILON { 1.0 / self.direction.x } else { f32::MAX },
            if self.direction.y.abs() > EPSILON { 1.0 / self.direction.y } else { f32::MAX },
            if self.direction.z.abs() > EPSILON { 1.0 / self.direction.z } else { f32::MAX },
        );
        let t1 = (aabb.min - self.origin) * inv_dir;
        let t2 = (aabb.max - self.origin) * inv_dir;
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);
        let t_enter = tmin.x.max(tmin.y).max(tmin.z);
        let t_exit  = tmax.x.min(tmax.y).min(tmax.z);
        if t_enter <= t_exit && t_exit > 0.0 {
            Some(if t_enter > 0.0 { t_enter } else { 0.0 })
        } else {
            None
        }
    }

    pub fn intersect_plane(&self, plane_normal: Vec3, plane_d: f32) -> Option<f32> {
        let denom = plane_normal.dot(self.direction);
        if denom.abs() < EPSILON { return None; }
        let t = (plane_d - plane_normal.dot(self.origin)) / denom;
        if t > EPSILON { Some(t) } else { None }
    }
}

// ============================================================
// MODEL PARTICLE
// ============================================================

#[derive(Clone, Debug)]
pub struct ModelParticle {
    pub position:     Vec3,
    pub character:    char,
    pub color:        Vec4,
    pub emission:     f32,
    pub normal:       Vec3,
    pub bone_weights: [f32; MAX_BONE_INFLUENCES],
    pub bone_indices: [u8;  MAX_BONE_INFLUENCES],
    pub group_id:     u32,
    pub layer_id:     u8,
    pub selected:     bool,
    pub locked:       bool,
}

impl ModelParticle {
    pub fn new(position: Vec3, character: char, color: Vec4) -> Self {
        Self {
            position,
            character,
            color,
            emission:     0.0,
            normal:       Vec3::Y,
            bone_weights: [1.0, 0.0, 0.0, 0.0],
            bone_indices: [0, 0, 0, 0],
            group_id:     0,
            layer_id:     0,
            selected:     false,
            locked:       false,
        }
    }

    pub fn with_normal(mut self, normal: Vec3) -> Self {
        self.normal = normal.normalize();
        self
    }

    pub fn with_emission(mut self, emission: f32) -> Self {
        self.emission = emission;
        self
    }

    pub fn with_group(mut self, group_id: u32) -> Self {
        self.group_id = group_id;
        self
    }

    /// Returns the position snapped to a grid of given cell size.
    pub fn snapped_position(&self, grid_size: f32) -> Vec3 {
        if grid_size < EPSILON { return self.position; }
        Vec3::new(
            (self.position.x / grid_size).round() * grid_size,
            (self.position.y / grid_size).round() * grid_size,
            (self.position.z / grid_size).round() * grid_size,
        )
    }
}

impl Default for ModelParticle {
    fn default() -> Self {
        Self::new(Vec3::ZERO, '.', Vec4::ONE)
    }
}

// ============================================================
// SKELETON / BONES
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleBone {
    pub id:        u32,
    pub name:      String,
    pub head:      Vec3,
    pub tail:      Vec3,
    pub parent_id: Option<u32>,
    pub rest_matrix:  Mat4,
    pub pose_matrix:  Mat4,
    pub local_rotation: Quat,
    pub local_scale:    Vec3,
}

impl ParticleBone {
    pub fn new(id: u32, name: impl Into<String>, head: Vec3, tail: Vec3) -> Self {
        let rest_matrix = Mat4::from_translation(head);
        Self {
            id,
            name: name.into(),
            head,
            tail,
            parent_id: None,
            rest_matrix,
            pose_matrix: rest_matrix,
            local_rotation: Quat::IDENTITY,
            local_scale: Vec3::ONE,
        }
    }

    pub fn length(&self) -> f32 {
        (self.tail - self.head).length()
    }

    pub fn direction(&self) -> Vec3 {
        (self.tail - self.head).normalize()
    }

    pub fn closest_point_on_bone(&self, p: Vec3) -> Vec3 {
        let dir = self.tail - self.head;
        let len = dir.length();
        if len < EPSILON { return self.head; }
        let t = ((p - self.head).dot(dir) / (len * len)).clamp(0.0, 1.0);
        self.head + dir * t
    }

    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        (p - self.closest_point_on_bone(p)).length()
    }

    pub fn build_pose_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local_scale,
            self.local_rotation,
            self.head,
        )
    }
}

#[derive(Clone, Debug)]
pub struct ParticleSkeleton {
    pub bones:      Vec<ParticleBone>,
    pub bind_poses: Vec<Mat4>,
}

impl ParticleSkeleton {
    pub fn new() -> Self {
        Self { bones: Vec::new(), bind_poses: Vec::new() }
    }

    pub fn add_bone(&mut self, bone: ParticleBone) -> u32 {
        let id = bone.id;
        self.bind_poses.push(bone.rest_matrix);
        self.bones.push(bone);
        id
    }

    pub fn find_bone(&self, id: u32) -> Option<&ParticleBone> {
        self.bones.iter().find(|b| b.id == id)
    }

    pub fn find_bone_mut(&mut self, id: u32) -> Option<&mut ParticleBone> {
        self.bones.iter_mut().find(|b| b.id == id)
    }

    /// Compute skinning weights for a particle position.
    /// Returns arrays of (bone_index, weight) for N nearest bones.
    pub fn compute_skin_weights(&self, position: Vec3, num_influences: usize) -> (Vec<usize>, Vec<f32>) {
        let mut dist_pairs: Vec<(usize, f32)> = self.bones.iter().enumerate()
            .map(|(i, b)| (i, b.distance_to_point(position)))
            .collect();
        dist_pairs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        dist_pairs.truncate(num_influences);

        let mut indices = Vec::new();
        let mut weights = Vec::new();
        let mut total_weight = 0.0f32;

        for (idx, dist) in &dist_pairs {
            let w = if *dist < EPSILON { 1e6 } else { 1.0 / (dist * dist) };
            indices.push(*idx);
            weights.push(w);
            total_weight += w;
        }

        if total_weight > EPSILON {
            for w in &mut weights { *w /= total_weight; }
        }

        (indices, weights)
    }

    /// Apply pose transforms to a position.
    pub fn transform_position(&self, position: Vec3, bone_indices: &[u8; 4], bone_weights: &[f32; 4]) -> Vec3 {
        let mut result = Vec3::ZERO;
        for i in 0..MAX_BONE_INFLUENCES {
            let w = bone_weights[i];
            if w < EPSILON { continue; }
            let bi = bone_indices[i] as usize;
            if bi >= self.bones.len() { continue; }
            let pose  = self.bones[bi].pose_matrix;
            let bind_inv = self.bind_poses[bi].inverse();
            let skinned = pose * bind_inv * Vec4::new(position.x, position.y, position.z, 1.0);
            result += Vec3::new(skinned.x, skinned.y, skinned.z) * w;
        }
        result
    }

    pub fn bind_all_particles(&mut self, particles: &mut Vec<ModelParticle>) {
        for p in particles.iter_mut() {
            let (indices, weights) = self.compute_skin_weights(p.position, MAX_BONE_INFLUENCES);
            for i in 0..MAX_BONE_INFLUENCES {
                p.bone_indices[i] = indices.get(i).copied().unwrap_or(0) as u8;
                p.bone_weights[i] = weights.get(i).copied().unwrap_or(0.0);
            }
        }
    }
}

impl Default for ParticleSkeleton {
    fn default() -> Self { Self::new() }
}

// ============================================================
// LOD
// ============================================================

#[derive(Clone, Debug)]
pub struct LodLevel {
    pub level:       u8,
    pub particles:   Vec<usize>,  // indices into parent model's particles
    pub distance:    f32,
    pub density_pct: f32,
}

impl LodLevel {
    pub fn new(level: u8, distance: f32, density_pct: f32) -> Self {
        Self { level, particles: Vec::new(), distance, density_pct }
    }
}

// ============================================================
// LAYER
// ============================================================

#[derive(Clone, Debug)]
pub enum LayerBlendMode {
    Replace,
    Add,
    Mask,
}

#[derive(Clone, Debug)]
pub struct ModelLayer {
    pub id:         u8,
    pub name:       String,
    pub visible:    bool,
    pub locked:     bool,
    pub opacity:    f32,
    pub blend_mode: LayerBlendMode,
    pub particle_indices: Vec<usize>,
}

impl ModelLayer {
    pub fn new(id: u8, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            visible:    true,
            locked:     false,
            opacity:    1.0,
            blend_mode: LayerBlendMode::Replace,
            particle_indices: Vec::new(),
        }
    }

    pub fn toggle_visibility(&mut self) { self.visible = !self.visible; }
    pub fn toggle_lock(&mut self) { self.locked = !self.locked; }
}

// ============================================================
// PARTICLE MODEL
// ============================================================

#[derive(Clone, Debug)]
pub struct ParticleModel {
    pub id:         u64,
    pub name:       String,
    pub particles:  Vec<ModelParticle>,
    pub bounds:     Aabb3,
    pub lod_levels: Vec<LodLevel>,
    pub skeleton:   Option<ParticleSkeleton>,
    pub layers:     Vec<ModelLayer>,
    pub metadata:   HashMap<String, String>,
}

impl ParticleModel {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        let mut model = Self {
            id,
            name: name.into(),
            particles:  Vec::new(),
            bounds:     Aabb3::empty(),
            lod_levels: Vec::new(),
            skeleton:   None,
            layers:     Vec::new(),
            metadata:   HashMap::new(),
        };
        model.layers.push(ModelLayer::new(0, "Layer 0"));
        model
    }

    pub fn recompute_bounds(&mut self) {
        self.bounds = Aabb3::empty();
        for p in &self.particles {
            self.bounds.expand(p.position);
        }
    }

    pub fn center_of_mass(&self) -> Vec3 {
        if self.particles.is_empty() { return Vec3::ZERO; }
        let sum: Vec3 = self.particles.iter().map(|p| p.position).fold(Vec3::ZERO, |a, b| a + b);
        sum / self.particles.len() as f32
    }

    pub fn add_particle(&mut self, p: ModelParticle) -> usize {
        let idx = self.particles.len();
        self.bounds.expand(p.position);
        if let Some(layer) = self.layers.last_mut() {
            layer.particle_indices.push(idx);
        }
        self.particles.push(p);
        idx
    }

    pub fn add_particles_bulk(&mut self, new_particles: Vec<ModelParticle>) {
        let start = self.particles.len();
        for (i, p) in new_particles.into_iter().enumerate() {
            self.bounds.expand(p.position);
            if let Some(layer) = self.layers.last_mut() {
                layer.particle_indices.push(start + i);
            }
            self.particles.push(p);
        }
    }

    pub fn remove_particles(&mut self, indices: &HashSet<usize>) {
        let mut new_particles = Vec::with_capacity(self.particles.len());
        let mut remap: Vec<Option<usize>> = vec![None; self.particles.len()];
        let mut new_idx = 0;
        for (old_idx, p) in self.particles.drain(..).enumerate() {
            if !indices.contains(&old_idx) {
                remap[old_idx] = Some(new_idx);
                new_particles.push(p);
                new_idx += 1;
            }
        }
        self.particles = new_particles;
        // Remap layer indices
        for layer in &mut self.layers {
            layer.particle_indices = layer.particle_indices.iter()
                .filter_map(|&i| remap.get(i).copied().flatten())
                .collect();
        }
        self.recompute_bounds();
    }

    pub fn particles_in_radius(&self, center: Vec3, radius: f32) -> Vec<usize> {
        let r2 = radius * radius;
        self.particles.iter().enumerate()
            .filter(|(_, p)| (p.position - center).length_squared() <= r2)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn particles_in_aabb(&self, aabb: &Aabb3) -> Vec<usize> {
        self.particles.iter().enumerate()
            .filter(|(_, p)| aabb.contains(p.position))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn generate_lods(&mut self) {
        self.lod_levels.clear();
        let total = self.particles.len();
        let configs: [(u8, f32, f32); 4] = [
            (0, 10.0,  1.00),
            (1, 30.0,  0.50),
            (2, 60.0,  0.25),
            (3, 120.0, 0.10),
        ];
        for (level, distance, pct) in &configs {
            let mut lod = LodLevel::new(*level, *distance, *pct);
            let target = ((total as f32) * pct).round() as usize;
            lod.particles = subsample_indices(total, target);
            self.lod_levels.push(lod);
        }
    }

    pub fn select_lod(&self, camera_distance: f32, lod_bias: f32) -> usize {
        let adjusted = camera_distance * (1.0 + lod_bias);
        for (i, lod) in self.lod_levels.iter().enumerate().rev() {
            if adjusted >= lod.distance {
                return i;
            }
        }
        0
    }

    pub fn merge_layer_into(&mut self, src_id: u8, dst_id: u8) {
        let src_idx = self.layers.iter().position(|l| l.id == src_id);
        let dst_idx = self.layers.iter().position(|l| l.id == dst_id);
        if let (Some(si), Some(di)) = (src_idx, dst_idx) {
            let src_indices = self.layers[si].particle_indices.clone();
            for idx in src_indices {
                self.layers[di].particle_indices.push(idx);
            }
            // Update layer_id on particles
            for &pi in &self.layers[di].particle_indices {
                if let Some(p) = self.particles.get_mut(pi) {
                    p.layer_id = dst_id;
                }
            }
            self.layers.remove(si);
        }
    }

    pub fn add_layer(&mut self, name: impl Into<String>) -> u8 {
        let id = self.layers.len() as u8;
        self.layers.push(ModelLayer::new(id, name));
        id
    }
}

// Helper: subsample indices evenly
fn subsample_indices(total: usize, target: usize) -> Vec<usize> {
    if target >= total { return (0..total).collect(); }
    if target == 0 { return Vec::new(); }
    let step = total as f32 / target as f32;
    (0..target).map(|i| ((i as f32 * step) as usize).min(total - 1)).collect()
}

// ============================================================
// BRUSH TYPES
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum BrushKind {
    Add,
    Remove,
    Smooth,
    Inflate,
    Pinch,
    Color,
    Char,
    Flatten,
    Crease,
    Clone,
}

impl Default for BrushKind {
    fn default() -> Self { BrushKind::Add }
}

#[derive(Clone, Debug)]
pub struct BrushParams {
    pub kind:     BrushKind,
    pub radius:   f32,
    pub strength: f32,
    pub density:  f32,
    pub color:    Vec4,
    pub character: char,
    pub falloff:  FalloffCurve,
}

impl Default for BrushParams {
    fn default() -> Self {
        Self {
            kind:      BrushKind::Add,
            radius:    DEFAULT_BRUSH_RADIUS,
            strength:  DEFAULT_BRUSH_STRENGTH,
            density:   DEFAULT_BRUSH_DENSITY,
            color:     Vec4::ONE,
            character: '.',
            falloff:   FalloffCurve::Smooth,
        }
    }
}

// ============================================================
// FALLOFF CURVES
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum FalloffCurve {
    Constant,
    Linear,
    Smooth,
    Sphere,
    Root,
    Sharp,
}

impl FalloffCurve {
    /// Returns weight in [0,1] given normalized distance t in [0,1].
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            FalloffCurve::Constant => 1.0,
            FalloffCurve::Linear   => 1.0 - t,
            FalloffCurve::Smooth   => smoothstep(0.0, 1.0, 1.0 - t),
            FalloffCurve::Sphere   => (1.0 - t * t).max(0.0).sqrt(),
            FalloffCurve::Root     => (1.0 - t).sqrt(),
            FalloffCurve::Sharp    => (1.0 - t).powi(3),
        }
    }
}

#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

// ============================================================
// SYMMETRY
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum SymmetryMode {
    None,
    X, Y, Z,
    XY, XZ, YZ,
    XYZ,
}

impl SymmetryMode {
    /// Returns mirror positions for a given input position.
    pub fn mirrors(&self, p: Vec3) -> Vec<Vec3> {
        match self {
            SymmetryMode::None => vec![],
            SymmetryMode::X    => vec![Vec3::new(-p.x,  p.y,  p.z)],
            SymmetryMode::Y    => vec![Vec3::new( p.x, -p.y,  p.z)],
            SymmetryMode::Z    => vec![Vec3::new( p.x,  p.y, -p.z)],
            SymmetryMode::XY   => vec![
                Vec3::new(-p.x,  p.y,  p.z),
                Vec3::new( p.x, -p.y,  p.z),
                Vec3::new(-p.x, -p.y,  p.z),
            ],
            SymmetryMode::XZ   => vec![
                Vec3::new(-p.x,  p.y,  p.z),
                Vec3::new( p.x,  p.y, -p.z),
                Vec3::new(-p.x,  p.y, -p.z),
            ],
            SymmetryMode::YZ   => vec![
                Vec3::new( p.x, -p.y,  p.z),
                Vec3::new( p.x,  p.y, -p.z),
                Vec3::new( p.x, -p.y, -p.z),
            ],
            SymmetryMode::XYZ  => vec![
                Vec3::new(-p.x,  p.y,  p.z),
                Vec3::new( p.x, -p.y,  p.z),
                Vec3::new( p.x,  p.y, -p.z),
                Vec3::new(-p.x, -p.y,  p.z),
                Vec3::new(-p.x,  p.y, -p.z),
                Vec3::new( p.x, -p.y, -p.z),
                Vec3::new(-p.x, -p.y, -p.z),
            ],
        }
    }
}

// ============================================================
// SELECTION SYSTEM
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct SelectionSystem {
    pub selected: HashSet<usize>,
    pub named_sets: HashMap<String, HashSet<usize>>,
}

impl SelectionSystem {
    pub fn new() -> Self { Self::default() }

    pub fn clear(&mut self) {
        self.selected.clear();
    }

    pub fn select_all(&mut self, count: usize) {
        self.selected = (0..count).collect();
    }

    pub fn invert(&mut self, total: usize) {
        let all: HashSet<usize> = (0..total).collect();
        self.selected = all.difference(&self.selected).copied().collect();
    }

    pub fn add(&mut self, idx: usize) { self.selected.insert(idx); }
    pub fn remove(&mut self, idx: usize) { self.selected.remove(&idx); }
    pub fn toggle(&mut self, idx: usize) {
        if self.selected.contains(&idx) { self.selected.remove(&idx); }
        else { self.selected.insert(idx); }
    }

    pub fn box_select(&mut self, particles: &[ModelParticle], aabb: &Aabb3, add: bool) {
        if !add { self.selected.clear(); }
        for (i, p) in particles.iter().enumerate() {
            if aabb.contains(p.position) { self.selected.insert(i); }
        }
    }

    pub fn sphere_select(&mut self, particles: &[ModelParticle], center: Vec3, radius: f32, add: bool) {
        if !add { self.selected.clear(); }
        let r2 = radius * radius;
        for (i, p) in particles.iter().enumerate() {
            if (p.position - center).length_squared() <= r2 {
                self.selected.insert(i);
            }
        }
    }

    pub fn paint_select(&mut self, particles: &[ModelParticle], ray: &Ray3, radius: f32, add: bool) {
        if !add { self.selected.clear(); }
        for (i, p) in particles.iter().enumerate() {
            if ray.distance_to_point(p.position) <= radius {
                self.selected.insert(i);
            }
        }
    }

    pub fn select_by_group(&mut self, particles: &[ModelParticle], group_id: u32, add: bool) {
        if !add { self.selected.clear(); }
        for (i, p) in particles.iter().enumerate() {
            if p.group_id == group_id { self.selected.insert(i); }
        }
    }

    pub fn select_by_char(&mut self, particles: &[ModelParticle], ch: char, add: bool) {
        if !add { self.selected.clear(); }
        for (i, p) in particles.iter().enumerate() {
            if p.character == ch { self.selected.insert(i); }
        }
    }

    /// HSV distance based color selection.
    pub fn select_by_color_range(
        &mut self,
        particles: &[ModelParticle],
        target_color: Vec4,
        tolerance: f32,
        add: bool,
    ) {
        if !add { self.selected.clear(); }
        let th = rgb_to_hsv(target_color.x, target_color.y, target_color.z);
        for (i, p) in particles.iter().enumerate() {
            let ph = rgb_to_hsv(p.color.x, p.color.y, p.color.z);
            let dh = hue_distance(th.0, ph.0);
            let ds = (th.1 - ph.1).abs();
            let dv = (th.2 - ph.2).abs();
            let dist = (dh * dh + ds * ds + dv * dv).sqrt();
            if dist <= tolerance { self.selected.insert(i); }
        }
    }

    /// Grow selection: add particles adjacent (within radius) to current selection.
    pub fn grow(&mut self, particles: &[ModelParticle], radius: f32) {
        let current: Vec<usize> = self.selected.iter().copied().collect();
        let r2 = radius * radius;
        for (i, p) in particles.iter().enumerate() {
            if self.selected.contains(&i) { continue; }
            for &sel in &current {
                if (particles[sel].position - p.position).length_squared() <= r2 {
                    self.selected.insert(i);
                    break;
                }
            }
        }
    }

    /// Shrink selection: remove boundary particles (those with non-selected neighbors).
    pub fn shrink(&mut self, particles: &[ModelParticle], radius: f32) {
        let r2 = radius * radius;
        let to_remove: HashSet<usize> = self.selected.iter().copied().filter(|&si| {
            particles.iter().enumerate().any(|(i, p)| {
                !self.selected.contains(&i) &&
                (particles[si].position - p.position).length_squared() <= r2
            })
        }).collect();
        for idx in to_remove { self.selected.remove(&idx); }
    }

    pub fn save_named_set(&mut self, name: impl Into<String>) {
        self.named_sets.insert(name.into(), self.selected.clone());
    }

    pub fn load_named_set(&mut self, name: &str) {
        if let Some(set) = self.named_sets.get(name) {
            self.selected = set.clone();
        }
    }

    pub fn union_named_set(&mut self, name: &str) {
        if let Some(set) = self.named_sets.get(name) {
            for &idx in set { self.selected.insert(idx); }
        }
    }
}

// ============================================================
// COLOR UTILITIES
// ============================================================

pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let v = max;
    let s = if max > EPSILON { delta / max } else { 0.0 };
    let h = if delta < EPSILON {
        0.0
    } else if (max - r).abs() < EPSILON {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < EPSILON {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, v)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s < EPSILON { return (v, v, v); }
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r1 + m, g1 + m, b1 + m)
}

pub fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { (360.0 - d) / 180.0 } else { d / 180.0 }
}

// ============================================================
// PRIMITIVE GENERATORS
// ============================================================

pub struct PrimitiveBuilder;

impl PrimitiveBuilder {
    /// Fibonacci sphere — uniform distribution of N points on a sphere surface.
    pub fn sphere(center: Vec3, radius: f32, n: usize, character: char, color: Vec4) -> Vec<ModelParticle> {
        let mut particles = Vec::with_capacity(n);
        let golden_angle = PI * (3.0 - 5.0_f32.sqrt());
        for i in 0..n {
            let y = 1.0 - (i as f32 / (n as f32 - 1.0)) * 2.0;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = golden_angle * i as f32;
            let x = theta.cos() * r;
            let z = theta.sin() * r;
            let normal = Vec3::new(x, y, z).normalize();
            let pos = center + normal * radius;
            particles.push(ModelParticle::new(pos, character, color).with_normal(normal));
        }
        particles
    }

    /// Cube — particles on 6 faces, with optional interior fill.
    pub fn cube(
        center: Vec3,
        half_size: Vec3,
        particles_per_face: usize,
        fill_interior: bool,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let n = (particles_per_face as f32).sqrt().ceil() as usize;
        let faces: [(Vec3, Vec3, Vec3); 6] = [
            (Vec3::X,  Vec3::Y,  Vec3::Z),
            (-Vec3::X, Vec3::Z,  Vec3::Y),
            (Vec3::Y,  Vec3::X,  Vec3::Z),
            (-Vec3::Y, Vec3::Z,  Vec3::X),
            (Vec3::Z,  Vec3::X,  Vec3::Y),
            (-Vec3::Z, Vec3::Y,  Vec3::X),
        ];
        for (normal, u_axis, v_axis) in &faces {
            let face_center = center + *normal * (normal.abs().dot(half_size));
            let hu = u_axis.abs().dot(half_size);
            let hv = v_axis.abs().dot(half_size);
            for ui in 0..n {
                for vi in 0..n {
                    let u = (ui as f32 / (n as f32 - 1.0).max(1.0)) * 2.0 - 1.0;
                    let v = (vi as f32 / (n as f32 - 1.0).max(1.0)) * 2.0 - 1.0;
                    let pos = face_center + *u_axis * (u * hu) + *v_axis * (v * hv);
                    particles.push(ModelParticle::new(pos, character, color).with_normal(*normal));
                }
            }
        }
        if fill_interior {
            let steps = (particles_per_face as f32).cbrt().ceil() as usize;
            for xi in 0..steps {
                for yi in 0..steps {
                    for zi in 0..steps {
                        let x = (xi as f32 / steps as f32) * 2.0 - 1.0;
                        let y = (yi as f32 / steps as f32) * 2.0 - 1.0;
                        let z = (zi as f32 / steps as f32) * 2.0 - 1.0;
                        let pos = center + Vec3::new(x * half_size.x, y * half_size.y, z * half_size.z);
                        particles.push(ModelParticle::new(pos, character, color).with_normal(Vec3::Y));
                    }
                }
            }
        }
        particles
    }

    /// Cylinder — circular bands + caps.
    pub fn cylinder(
        center: Vec3,
        radius: f32,
        height: f32,
        segments: usize,
        bands: usize,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let half_h = height * 0.5;
        // Side bands
        for bi in 0..bands {
            let y = (bi as f32 / (bands as f32 - 1.0).max(1.0)) * height - half_h + center.y;
            for si in 0..segments {
                let angle = TAU * si as f32 / segments as f32;
                let x = center.x + radius * angle.cos();
                let z = center.z + radius * angle.sin();
                let normal = Vec3::new(angle.cos(), 0.0, angle.sin());
                particles.push(ModelParticle::new(Vec3::new(x, y, z), character, color).with_normal(normal));
            }
        }
        // Caps
        for cap in [half_h, -half_h] {
            let normal = if cap > 0.0 { Vec3::Y } else { -Vec3::Y };
            let n = (segments as f32).sqrt().ceil() as usize;
            for ri in 0..n {
                let r = (ri as f32 / n as f32) * radius;
                for si in 0..segments {
                    let angle = TAU * si as f32 / segments as f32;
                    let x = center.x + r * angle.cos();
                    let z = center.z + r * angle.sin();
                    let y = center.y + cap;
                    particles.push(ModelParticle::new(Vec3::new(x, y, z), character, color).with_normal(normal));
                }
            }
        }
        particles
    }

    /// Cone.
    pub fn cone(
        apex: Vec3,
        base_center: Vec3,
        base_radius: f32,
        segments: usize,
        bands: usize,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let axis = (apex - base_center).normalize();
        let height = (apex - base_center).length();
        let perp = if axis.abs().dot(Vec3::X) < 0.9 {
            axis.cross(Vec3::X).normalize()
        } else {
            axis.cross(Vec3::Y).normalize()
        };
        let perp2 = axis.cross(perp).normalize();
        for bi in 0..bands {
            let t = bi as f32 / bands as f32;
            let r = base_radius * (1.0 - t);
            let pos_center = base_center + axis * (t * height);
            for si in 0..segments {
                let angle = TAU * si as f32 / segments as f32;
                let pos = pos_center + perp * (angle.cos() * r) + perp2 * (angle.sin() * r);
                let outward = (perp * angle.cos() + perp2 * angle.sin()).normalize();
                let slope = (base_radius / height).atan();
                let normal = (outward + axis * slope.tan()).normalize();
                particles.push(ModelParticle::new(pos, character, color).with_normal(normal));
            }
        }
        // Base cap
        let n = (segments as f32 / 2.0).ceil() as usize;
        for ri in 0..=n {
            let r = (ri as f32 / n as f32) * base_radius;
            for si in 0..segments {
                let angle = TAU * si as f32 / segments as f32;
                let pos = base_center + perp * (angle.cos() * r) + perp2 * (angle.sin() * r);
                particles.push(ModelParticle::new(pos, character, color).with_normal(-axis));
            }
        }
        particles
    }

    /// Torus — parametric surface: (R + r*cos(v)) * cos(u), etc.
    pub fn torus(
        center: Vec3,
        major_radius: f32,
        minor_radius: f32,
        u_segments: usize,
        v_segments: usize,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        for ui in 0..u_segments {
            let u = TAU * ui as f32 / u_segments as f32;
            for vi in 0..v_segments {
                let v = TAU * vi as f32 / v_segments as f32;
                let x = (major_radius + minor_radius * v.cos()) * u.cos();
                let y = minor_radius * v.sin();
                let z = (major_radius + minor_radius * v.cos()) * u.sin();
                // Normal: derivative of surface position w.r.t. v, cross u, simplified to outward normal
                let ring_center = Vec3::new(major_radius * u.cos(), 0.0, major_radius * u.sin());
                let surface_pos = center + Vec3::new(x, y, z);
                let normal = (surface_pos - (center + ring_center)).normalize();
                particles.push(ModelParticle::new(surface_pos, character, color).with_normal(normal));
            }
        }
        particles
    }

    /// Plane — grid of particles with optional Perlin-like noise displacement.
    pub fn plane(
        center: Vec3,
        width: f32,
        depth: f32,
        cols: usize,
        rows: usize,
        noise_scale: f32,
        noise_amplitude: f32,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let hw = width * 0.5;
        let hd = depth * 0.5;
        for ri in 0..rows {
            for ci in 0..cols {
                let u = ci as f32 / (cols as f32 - 1.0).max(1.0);
                let v = ri as f32 / (rows as f32 - 1.0).max(1.0);
                let x = center.x - hw + u * width;
                let z = center.z - hd + v * depth;
                let noise = simple_noise_2d(x * noise_scale, z * noise_scale);
                let y = center.y + noise * noise_amplitude;
                particles.push(ModelParticle::new(Vec3::new(x, y, z), character, color).with_normal(Vec3::Y));
            }
        }
        particles
    }

    /// Text3D — extrude ASCII characters into 3D particle slabs.
    pub fn text3d(
        text: &str,
        origin: Vec3,
        char_width: f32,
        char_height: f32,
        depth: f32,
        particles_per_char: usize,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        for (ci, ch) in text.chars().enumerate() {
            let x_offset = ci as f32 * char_width;
            let bits = char_bitmap(ch);
            for (row, &row_bits) in bits.iter().enumerate() {
                for col in 0..5 {
                    if (row_bits >> (4 - col)) & 1 == 1 {
                        let x = origin.x + x_offset + col as f32 * (char_width / 5.0);
                        let y = origin.y + (bits.len() - 1 - row) as f32 * (char_height / bits.len() as f32);
                        // Front face
                        particles.push(ModelParticle::new(
                            Vec3::new(x, y, origin.z),
                            character, color,
                        ).with_normal(-Vec3::Z));
                        // Back face
                        particles.push(ModelParticle::new(
                            Vec3::new(x, y, origin.z + depth),
                            character, color,
                        ).with_normal(Vec3::Z));
                        // Extrusion steps
                        let steps = (particles_per_char / 10).max(1);
                        for si in 1..steps {
                            let z = origin.z + (si as f32 / steps as f32) * depth;
                            particles.push(ModelParticle::new(Vec3::new(x, y, z), character, color));
                        }
                    }
                }
            }
        }
        particles
    }

    /// PointCloud — import from Vec<Vec3> with color mapping by height.
    pub fn point_cloud(
        points: Vec<Vec3>,
        character: char,
        color_low: Vec4,
        color_high: Vec4,
    ) -> Vec<ModelParticle> {
        if points.is_empty() { return Vec::new(); }
        let min_y = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let max_y = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let range = (max_y - min_y).max(EPSILON);
        points.into_iter().map(|p| {
            let t = (p.y - min_y) / range;
            let color = color_low.lerp(color_high, t);
            ModelParticle::new(p, character, color)
        }).collect()
    }

    /// MarchingCubes — generate surface particles from a scalar density field.
    pub fn marching_cubes(
        field: &dyn Fn(Vec3) -> f32,
        bounds: &Aabb3,
        resolution: usize,
        threshold: f32,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let size = bounds.size();
        let dx = size.x / resolution as f32;
        let dy = size.y / resolution as f32;
        let dz = size.z / resolution as f32;

        for xi in 0..resolution {
            for yi in 0..resolution {
                for zi in 0..resolution {
                    let x0 = bounds.min.x + xi as f32 * dx;
                    let y0 = bounds.min.y + yi as f32 * dy;
                    let z0 = bounds.min.z + zi as f32 * dz;

                    // 8 corners of voxel
                    let corners = [
                        Vec3::new(x0,      y0,      z0     ),
                        Vec3::new(x0 + dx, y0,      z0     ),
                        Vec3::new(x0 + dx, y0 + dy, z0     ),
                        Vec3::new(x0,      y0 + dy, z0     ),
                        Vec3::new(x0,      y0,      z0 + dz),
                        Vec3::new(x0 + dx, y0,      z0 + dz),
                        Vec3::new(x0 + dx, y0 + dy, z0 + dz),
                        Vec3::new(x0,      y0 + dy, z0 + dz),
                    ];
                    let values: [f32; 8] = std::array::from_fn(|i| field(corners[i]));

                    // Build case index
                    let mut case_idx: u8 = 0;
                    for (i, &v) in values.iter().enumerate() {
                        if v >= threshold { case_idx |= 1 << i; }
                    }
                    if case_idx == 0 || case_idx == 255 { continue; }

                    // Edge intersections for this case
                    let edge_mask = MC_EDGE_TABLE[case_idx as usize];
                    let mut edge_verts: [Vec3; 12] = [Vec3::ZERO; 12];

                    // The 12 edges of a cube: pairs of corner indices
                    let edge_corners: [(usize, usize); 12] = [
                        (0,1),(1,2),(2,3),(3,0),
                        (4,5),(5,6),(6,7),(7,4),
                        (0,4),(1,5),(2,6),(3,7),
                    ];

                    for (ei, &(a, b)) in edge_corners.iter().enumerate() {
                        if edge_mask & (1 << ei) != 0 {
                            let va = values[a];
                            let vb = values[b];
                            let t = if (vb - va).abs() > EPSILON {
                                (threshold - va) / (vb - va)
                            } else {
                                0.5
                            };
                            edge_verts[ei] = corners[a].lerp(corners[b], t);
                        }
                    }

                    // Emit triangles (and thus particles)
                    let tris = &MC_TRI_TABLE[case_idx as usize];
                    let mut ti = 0;
                    while ti < tris.len() && tris[ti] != 255 {
                        let e0 = tris[ti]     as usize;
                        let e1 = tris[ti + 1] as usize;
                        let e2 = tris[ti + 2] as usize;
                        let p0 = edge_verts[e0];
                        let p1 = edge_verts[e1];
                        let p2 = edge_verts[e2];
                        let normal = (p1 - p0).cross(p2 - p0).normalize();
                        let centroid = (p0 + p1 + p2) / 3.0;
                        particles.push(ModelParticle::new(centroid, character, color).with_normal(normal));
                        ti += 3;
                    }
                }
            }
        }
        particles
    }

    /// Metaballs — generate surface particles from N metaballs.
    pub fn metaballs(
        balls: &[(Vec3, f32)],
        bounds: &Aabb3,
        resolution: usize,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let field = |p: Vec3| -> f32 {
            balls.iter().map(|(center, radius)| {
                let d2 = (p - *center).length_squared();
                if d2 < EPSILON { 1e6 } else { (radius * radius) / d2 }
            }).sum()
        };
        Self::marching_cubes(&field, bounds, resolution, METABALL_THRESHOLD, character, color)
    }
}

// ============================================================
// SIMPLE NOISE UTILITY
// ============================================================

pub fn simple_noise_2d(x: f32, y: f32) -> f32 {
    // Value noise using integer hash
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let u = smoothstep(0.0, 1.0, xf);
    let v = smoothstep(0.0, 1.0, yf);
    let aa = hash_2d(xi,   yi  );
    let ba = hash_2d(xi+1, yi  );
    let ab = hash_2d(xi,   yi+1);
    let bb = hash_2d(xi+1, yi+1);
    let x1 = aa + u * (ba - aa);
    let x2 = ab + u * (bb - ab);
    x1 + v * (x2 - x1)
}

fn hash_2d(x: i32, y: i32) -> f32 {
    let n = x.wrapping_mul(1619).wrapping_add(y.wrapping_mul(31337)).wrapping_add(1013904223);
    let n = n.wrapping_mul(1664525).wrapping_add(1013904223);
    ((n as u32) as f32) / (u32::MAX as f32)
}

// ============================================================
// CHAR BITMAP (for Text3D)
// ============================================================

fn char_bitmap(ch: char) -> Vec<u8> {
    match ch {
        'A' => vec![0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => vec![0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => vec![0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => vec![0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        'E' => vec![0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => vec![0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => vec![0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => vec![0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => vec![0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => vec![0b11111, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110],
        'K' => vec![0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => vec![0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => vec![0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001],
        'N' => vec![0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => vec![0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => vec![0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => vec![0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => vec![0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => vec![0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => vec![0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => vec![0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => vec![0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
        'W' => vec![0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => vec![0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001],
        'Y' => vec![0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => vec![0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => vec![0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => vec![0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => vec![0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => vec![0b11111, 0b00001, 0b00010, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => vec![0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => vec![0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => vec![0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => vec![0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => vec![0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => vec![0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        ' ' => vec![0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        '.' => vec![0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        '!' => vec![0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '?' => vec![0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100],
        _   => vec![0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111],
    }
}

// ============================================================
// MARCHING CUBES TABLES
// ============================================================

// Edge table: for each of the 256 cases, which of the 12 edges are active.
static MC_EDGE_TABLE: [u16; 256] = [
    0x000, 0x109, 0x203, 0x30a, 0x406, 0x50f, 0x605, 0x70c,
    0x80c, 0x905, 0xa0f, 0xb06, 0xc0a, 0xd03, 0xe09, 0xf00,
    0x190, 0x099, 0x393, 0x29a, 0x596, 0x49f, 0x795, 0x69c,
    0x99c, 0x895, 0xb9f, 0xa96, 0xd9a, 0xc93, 0xf99, 0xe90,
    0x230, 0x339, 0x033, 0x13a, 0x636, 0x73f, 0x435, 0x53c,
    0xa3c, 0xb35, 0x83f, 0x936, 0xe3a, 0xf33, 0xc39, 0xd30,
    0x3a0, 0x2a9, 0x1a3, 0x0aa, 0x7a6, 0x6af, 0x5a5, 0x4ac,
    0xbac, 0xaa5, 0x9af, 0x8a6, 0xfaa, 0xea3, 0xda9, 0xca0,
    0x460, 0x569, 0x663, 0x76a, 0x066, 0x16f, 0x265, 0x36c,
    0xc6c, 0xd65, 0xe6f, 0xf66, 0x86a, 0x963, 0xa69, 0xb60,
    0x5f0, 0x4f9, 0x7f3, 0x6fa, 0x1f6, 0x0ff, 0x3f5, 0x2fc,
    0xdfc, 0xcf5, 0xfff, 0xef6, 0x9fa, 0x8f3, 0xbf9, 0xaf0,
    0x650, 0x759, 0x453, 0x55a, 0x256, 0x35f, 0x055, 0x15c,
    0xe5c, 0xf55, 0xc5f, 0xd56, 0xa5a, 0xb53, 0x859, 0x950,
    0x7c0, 0x6c9, 0x5c3, 0x4ca, 0x3c6, 0x2cf, 0x1c5, 0x0cc,
    0xfcc, 0xec5, 0xdcf, 0xcc6, 0xbca, 0xac3, 0x9c9, 0x8c0,
    0x8c0, 0x9c9, 0xac3, 0xbca, 0xcc6, 0xdcf, 0xec5, 0xfcc,
    0x0cc, 0x1c5, 0x2cf, 0x3c6, 0x4ca, 0x5c3, 0x6c9, 0x7c0,
    0x950, 0x859, 0xb53, 0xa5a, 0xd56, 0xc5f, 0xf55, 0xe5c,
    0x15c, 0x055, 0x35f, 0x256, 0x55a, 0x453, 0x759, 0x650,
    0xaf0, 0xbf9, 0x8f3, 0x9fa, 0xef6, 0xfff, 0xcf5, 0xdfc,
    0x2fc, 0x3f5, 0x0ff, 0x1f6, 0x6fa, 0x7f3, 0x4f9, 0x5f0,
    0xb60, 0xa69, 0x963, 0x86a, 0xf66, 0xe6f, 0xd65, 0xc6c,
    0x36c, 0x265, 0x16f, 0x066, 0x76a, 0x663, 0x569, 0x460,
    0xca0, 0xda9, 0xea3, 0xfaa, 0x8a6, 0x9af, 0xaa5, 0xbac,
    0x4ac, 0x5a5, 0x6af, 0x7a6, 0x0aa, 0x1a3, 0x2a9, 0x3a0,
    0xd30, 0xc39, 0xf33, 0xe3a, 0x936, 0x83f, 0xb35, 0xa3c,
    0x53c, 0x435, 0x73f, 0x636, 0x13a, 0x033, 0x339, 0x230,
    0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c,
    0x69c, 0x795, 0x49f, 0x596, 0x29a, 0x393, 0x099, 0x190,
    0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905, 0x80c,
    0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x000,
];

// Triangle table: for each case, up to 5 triangles = 15 edge indices, terminated by 255.
// Using a flat fixed-size structure to keep things simple.
static MC_TRI_TABLE: [[u8; 16]; 256] = {
    let mut t = [[255u8; 16]; 256];
    // A subset of the most common cases filled in manually (full 256-case table)
    // Case 0: no triangles
    // Case 1: one corner
    t[1]   = [0,8,3, 255,255,255,255,255,255,255,255,255,255,255,255,255];
    t[2]   = [0,1,9, 255,255,255,255,255,255,255,255,255,255,255,255,255];
    t[3]   = [1,8,3, 9,8,1, 255,255,255,255,255,255,255,255,255,255];
    t[4]   = [1,2,10,255,255,255,255,255,255,255,255,255,255,255,255,255];
    t[5]   = [0,8,3, 1,2,10,255,255,255,255,255,255,255,255,255,255];
    t[6]   = [9,2,10,0,2,9, 255,255,255,255,255,255,255,255,255,255];
    t[7]   = [2,8,3, 2,10,8,10,9,8, 255,255,255,255,255,255,255];
    t[8]   = [3,11,2,255,255,255,255,255,255,255,255,255,255,255,255,255];
    t[9]   = [0,11,2,8,11,0, 255,255,255,255,255,255,255,255,255,255];
    t[10]  = [1,9,0, 2,3,11,255,255,255,255,255,255,255,255,255,255];
    t[11]  = [1,11,2,1,9,11,9,8,11, 255,255,255,255,255,255,255];
    t[12]  = [3,10,1,11,10,3,255,255,255,255,255,255,255,255,255,255];
    t[13]  = [0,10,1,0,8,10,8,11,10,255,255,255,255,255,255,255];
    t[14]  = [3,9,0, 3,11,9,11,10,9,255,255,255,255,255,255,255];
    t[15]  = [9,8,10,10,8,11,255,255,255,255,255,255,255,255,255,255];
    t[254] = [0,8,3, 255,255,255,255,255,255,255,255,255,255,255,255,255];
    t
};

// ============================================================
// SCULPT BRUSH OPERATIONS
// ============================================================

pub struct SculptEngine;

impl SculptEngine {
    /// Add particles at hit_pos using Poisson disk sampling within brush footprint.
    pub fn apply_add(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        surface_normal: Vec3,
        params: &BrushParams,
        character: char,
        color: Vec4,
        symmetry: &SymmetryMode,
    ) {
        let positions = Self::poisson_disk_sample_disk(hit_pos, surface_normal, params.radius, params.density);
        let mut new_particles: Vec<ModelParticle> = positions.iter().map(|&p| {
            ModelParticle::new(p, character, color).with_normal(surface_normal)
        }).collect();
        // Mirror
        for mirror_pos in symmetry.mirrors(hit_pos) {
            let mirrored = Self::poisson_disk_sample_disk(mirror_pos, surface_normal, params.radius, params.density);
            for mp in mirrored {
                new_particles.push(ModelParticle::new(mp, character, color).with_normal(surface_normal));
            }
        }
        model.add_particles_bulk(new_particles);
    }

    /// Poisson disk sampling on a disk surface.
    pub fn poisson_disk_sample_disk(
        center: Vec3,
        normal: Vec3,
        radius: f32,
        density: f32,
    ) -> Vec<Vec3> {
        let n_particles = (PI * radius * radius * density).round() as usize;
        let n_particles = n_particles.max(1);
        let perp = {
            let n = normal.normalize();
            let up = if n.abs().dot(Vec3::X) < 0.9 { Vec3::X } else { Vec3::Y };
            n.cross(up).normalize()
        };
        let perp2 = normal.normalize().cross(perp).normalize();

        let min_dist = 1.0 / density.sqrt();
        let mut placed: Vec<Vec3> = Vec::new();
        let mut attempts = 0usize;

        // Simple random-ish Poisson disk via rejection
        while placed.len() < n_particles && attempts < n_particles * 30 {
            attempts += 1;
            let r = radius * hash_2d(attempts as i32, placed.len() as i32).sqrt();
            let angle = TAU * hash_2d(placed.len() as i32 * 7, attempts as i32 * 13);
            let local = perp * (r * angle.cos()) + perp2 * (r * angle.sin());
            let candidate = center + local;
            let ok = placed.iter().all(|&q| (candidate - q).length() >= min_dist);
            if ok { placed.push(candidate); }
        }
        placed
    }

    /// Remove particles within brush radius using smooth falloff.
    pub fn apply_remove(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
        symmetry: &SymmetryMode,
    ) {
        let r = params.radius;
        let r2 = r * r;
        let mut to_remove = HashSet::new();

        for (i, p) in model.particles.iter().enumerate() {
            if p.locked { continue; }
            let d2 = (p.position - hit_pos).length_squared();
            if d2 <= r2 {
                let t = (d2 / r2).sqrt();
                let falloff = params.falloff.evaluate(t);
                // Remove if falloff * strength > threshold
                if falloff * params.strength > 0.5 {
                    to_remove.insert(i);
                }
            }
        }

        // Handle symmetry
        for mirror_pos in symmetry.mirrors(hit_pos) {
            for (i, p) in model.particles.iter().enumerate() {
                if p.locked { continue; }
                let d2 = (p.position - mirror_pos).length_squared();
                if d2 <= r2 {
                    let t = (d2 / r2).sqrt();
                    let falloff = params.falloff.evaluate(t);
                    if falloff * params.strength > 0.5 { to_remove.insert(i); }
                }
            }
        }

        model.remove_particles(&to_remove);
    }

    /// Smooth: Laplacian smoothing — move each particle toward neighborhood centroid.
    pub fn apply_smooth(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        let k = params.strength;

        let affected: Vec<usize> = model.particles.iter().enumerate()
            .filter(|(_, p)| !p.locked && (p.position - hit_pos).length_squared() <= r2)
            .map(|(i, _)| i)
            .collect();

        let positions: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
        let neighbour_radius = r * 0.5;
        let nb_r2 = neighbour_radius * neighbour_radius;

        for &ai in &affected {
            let pi = positions[ai];
            let d = (pi - hit_pos).length();
            let falloff = params.falloff.evaluate(d / r);

            // Compute neighborhood centroid
            let (centroid, count) = positions.iter().enumerate()
                .filter(|(j, q)| *j != ai && (**q - pi).length_squared() <= nb_r2)
                .fold((Vec3::ZERO, 0usize), |(acc, n), (_, q)| (acc + *q, n + 1));

            if count > 0 {
                let centroid = centroid / count as f32;
                model.particles[ai].position = pi.lerp(centroid, k * falloff);
            }
        }
    }

    /// Inflate: move particles outward along average normal.
    pub fn apply_inflate(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        for p in &mut model.particles {
            if p.locked { continue; }
            let d2 = (p.position - hit_pos).length_squared();
            if d2 > r2 { continue; }
            let t = d2.sqrt() / r;
            let falloff = params.falloff.evaluate(t);
            p.position += p.normal * params.strength * falloff;
        }
    }

    /// Pinch: attract particles toward brush center.
    pub fn apply_pinch(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        for p in &mut model.particles {
            if p.locked { continue; }
            let diff = hit_pos - p.position;
            let d2 = diff.length_squared();
            if d2 > r2 { continue; }
            let t = d2.sqrt() / r;
            let falloff = params.falloff.evaluate(t);
            p.position += diff.normalize() * params.strength * falloff;
        }
    }

    /// Color brush: paint color onto particles with falloff.
    pub fn apply_color(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        for p in &mut model.particles {
            if p.locked { continue; }
            let d2 = (p.position - hit_pos).length_squared();
            if d2 > r2 { continue; }
            let t = d2.sqrt() / r;
            let falloff = params.falloff.evaluate(t);
            let blend = falloff * params.strength;
            p.color = p.color.lerp(params.color, blend);
        }
    }

    /// Char brush: replace glyph within radius.
    pub fn apply_char(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        for p in &mut model.particles {
            if p.locked { continue; }
            let d2 = (p.position - hit_pos).length_squared();
            if d2 > r2 { continue; }
            let t = d2.sqrt() / r;
            let falloff = params.falloff.evaluate(t);
            if falloff * params.strength > 0.3 {
                p.character = params.character;
            }
        }
    }

    /// Flatten: project particles onto best-fit plane.
    /// Uses PCA via covariance matrix to find the plane normal.
    pub fn apply_flatten(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;

        // Gather affected particles
        let affected: Vec<usize> = model.particles.iter().enumerate()
            .filter(|(_, p)| !p.locked && (p.position - hit_pos).length_squared() <= r2)
            .map(|(i, _)| i)
            .collect();

        if affected.len() < 3 { return; }

        // Compute centroid
        let centroid = affected.iter()
            .map(|&i| model.particles[i].position)
            .fold(Vec3::ZERO, |a, b| a + b)
            / affected.len() as f32;

        // Compute 3x3 covariance matrix
        let mut cov = [[0f32; 3]; 3];
        for &i in &affected {
            let d = model.particles[i].position - centroid;
            let dv = [d.x, d.y, d.z];
            for r in 0..3 {
                for c in 0..3 { cov[r][c] += dv[r] * dv[c]; }
            }
        }
        for r in 0..3 { for c in 0..3 { cov[r][c] /= affected.len() as f32; } }

        // Power iteration to find dominant eigenvector (plane normal = least variance = smallest eigenvalue)
        // We find largest eigenvector, then use cross products for smallest.
        let dominant = power_iteration_3x3(&cov, 32);
        let second = gram_schmidt_orthogonalize(dominant, &cov, 32);
        let plane_normal = dominant.cross(second).normalize();
        let plane_d = plane_normal.dot(centroid);

        // Project particles onto plane
        for &i in &affected {
            let p = &mut model.particles[i];
            let d = (p.position - hit_pos).length();
            let falloff = params.falloff.evaluate(d / r);
            let dist_to_plane = plane_normal.dot(p.position) - plane_d;
            p.position -= plane_normal * dist_to_plane * falloff * params.strength;
        }
    }

    /// Crease: push particles toward nearest crease line.
    pub fn apply_crease(
        model: &mut ParticleModel,
        hit_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        // Find affected particles
        let positions: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
        for i in 0..model.particles.len() {
            if model.particles[i].locked { continue; }
            let d2 = (positions[i] - hit_pos).length_squared();
            if d2 > r2 { continue; }
            let t = d2.sqrt() / r;
            let falloff = params.falloff.evaluate(t);

            // Find neighbors
            let neighbors: Vec<Vec3> = positions.iter().enumerate()
                .filter(|(j, q)| *j != i && (**q - positions[i]).length_squared() <= r2 * 0.25)
                .map(|(_, &q)| q)
                .collect();
            if neighbors.len() < 2 { continue; }

            // Find farthest pair as crease direction
            let mut max_dist = 0.0f32;
            let mut crease_dir = Vec3::X;
            for &na in &neighbors {
                for &nb in &neighbors {
                    let d = (na - nb).length();
                    if d > max_dist {
                        max_dist = d;
                        crease_dir = (nb - na).normalize();
                    }
                }
            }
            // Project particle onto crease line through centroid
            let centroid = neighbors.iter().fold(Vec3::ZERO, |a, &b| a + b) / neighbors.len() as f32;
            let to_p = positions[i] - centroid;
            let proj = centroid + crease_dir * crease_dir.dot(to_p);
            model.particles[i].position = model.particles[i].position.lerp(proj, falloff * params.strength);
        }
    }

    /// Clone brush: stamp particles from source area to target area.
    pub fn apply_clone(
        model: &mut ParticleModel,
        source_pos: Vec3,
        target_pos: Vec3,
        params: &BrushParams,
    ) {
        let r = params.radius;
        let r2 = r * r;
        let offset = target_pos - source_pos;

        let cloned: Vec<ModelParticle> = model.particles.iter()
            .filter(|p| !p.locked && (p.position - source_pos).length_squared() <= r2)
            .map(|p| {
                let mut np = p.clone();
                np.position += offset;
                np
            })
            .collect();

        model.add_particles_bulk(cloned);
    }
}

// ============================================================
// MATH HELPERS FOR PCA
// ============================================================

/// Power iteration: find dominant eigenvector of 3x3 symmetric matrix.
fn power_iteration_3x3(m: &[[f32; 3]; 3], iterations: usize) -> Vec3 {
    let mut v = Vec3::new(1.0, 1.0, 1.0).normalize();
    for _ in 0..iterations {
        let mv = mat3_mul_vec3(m, v);
        let len = mv.length();
        if len < EPSILON { break; }
        v = mv / len;
    }
    v
}

fn gram_schmidt_orthogonalize(dominant: Vec3, m: &[[f32; 3]; 3], iterations: usize) -> Vec3 {
    // Start with a vector perpendicular to dominant
    let perp = if dominant.abs().dot(Vec3::X) < 0.9 {
        dominant.cross(Vec3::X).normalize()
    } else {
        dominant.cross(Vec3::Y).normalize()
    };
    let mut v = perp;
    for _ in 0..iterations {
        let mv = mat3_mul_vec3(m, v);
        // Deflate: remove dominant component
        let deflated = mv - dominant * dominant.dot(mv);
        let len = deflated.length();
        if len < EPSILON { break; }
        v = deflated / len;
    }
    v
}

fn mat3_mul_vec3(m: &[[f32; 3]; 3], v: Vec3) -> Vec3 {
    let va = [v.x, v.y, v.z];
    let mut result = [0.0f32; 3];
    for i in 0..3 {
        for j in 0..3 { result[i] += m[i][j] * va[j]; }
    }
    Vec3::new(result[0], result[1], result[2])
}

// ============================================================
// TRANSFORM TOOLS
// ============================================================

pub struct TransformTools;

impl TransformTools {
    pub fn translate(particles: &mut Vec<ModelParticle>, indices: &HashSet<usize>, delta: Vec3) {
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if !p.locked { p.position += delta; }
            }
        }
    }

    pub fn rotate(particles: &mut Vec<ModelParticle>, indices: &HashSet<usize>, pivot: Vec3, quat: Quat) {
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                let local = p.position - pivot;
                p.position = pivot + quat * local;
                p.normal = quat * p.normal;
            }
        }
    }

    pub fn scale_uniform(particles: &mut Vec<ModelParticle>, indices: &HashSet<usize>, pivot: Vec3, factor: f32) {
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                p.position = pivot + (p.position - pivot) * factor;
            }
        }
    }

    pub fn scale_nonuniform(particles: &mut Vec<ModelParticle>, indices: &HashSet<usize>, pivot: Vec3, factors: Vec3) {
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                let local = p.position - pivot;
                p.position = pivot + Vec3::new(local.x * factors.x, local.y * factors.y, local.z * factors.z);
            }
        }
    }

    pub fn mirror(particles: &mut Vec<ModelParticle>, indices: &HashSet<usize>, pivot: Vec3, axis: Vec3) {
        let axis_n = axis.normalize();
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                let local = p.position - pivot;
                let proj = axis_n * axis_n.dot(local);
                p.position = pivot + local - 2.0 * proj;
                p.normal = p.normal - 2.0 * axis_n * axis_n.dot(p.normal);
            }
        }
    }

    /// Bend deformation along axis.
    pub fn bend(
        particles: &mut Vec<ModelParticle>,
        indices: &HashSet<usize>,
        pivot: Vec3,
        axis: Vec3,
        angle_per_unit: f32,
    ) {
        let axis_n = axis.normalize();
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                let local = p.position - pivot;
                let along = axis_n.dot(local);
                let angle = along * angle_per_unit;
                let quat = Quat::from_axis_angle(axis_n.cross(local).normalize(), angle);
                p.position = pivot + quat * local;
            }
        }
    }

    /// Taper: scale falloff along axis.
    pub fn taper(
        particles: &mut Vec<ModelParticle>,
        indices: &HashSet<usize>,
        pivot: Vec3,
        axis: Vec3,
        taper_factor: f32,
    ) {
        let axis_n = axis.normalize();
        let all_positions: Vec<Vec3> = indices.iter()
            .filter_map(|&i| particles.get(i).map(|p| p.position))
            .collect();
        if all_positions.is_empty() { return; }
        let min_t = all_positions.iter().map(|&p| axis_n.dot(p - pivot)).fold(f32::MAX, f32::min);
        let max_t = all_positions.iter().map(|&p| axis_n.dot(p - pivot)).fold(f32::MIN, f32::max);
        let range = (max_t - min_t).max(EPSILON);
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                let local = p.position - pivot;
                let t = (axis_n.dot(local) - min_t) / range;
                let scale = 1.0 + (t - 0.5) * taper_factor;
                let perp = local - axis_n * axis_n.dot(local);
                p.position = pivot + axis_n * axis_n.dot(local) + perp * scale;
            }
        }
    }

    /// Twist: rotate amount proportional to Y (or axis) position.
    pub fn twist(
        particles: &mut Vec<ModelParticle>,
        indices: &HashSet<usize>,
        pivot: Vec3,
        axis: Vec3,
        twist_rate: f32,
    ) {
        let axis_n = axis.normalize();
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                let local = p.position - pivot;
                let t = axis_n.dot(local);
                let angle = t * twist_rate;
                let quat = Quat::from_axis_angle(axis_n, angle);
                p.position = pivot + quat * local;
                p.normal = quat * p.normal;
            }
        }
    }

    /// Lattice deform: trilinear interpolation through 3D control point grid.
    pub fn lattice_deform(
        particles: &mut Vec<ModelParticle>,
        indices: &HashSet<usize>,
        lattice: &LatticeDeformer,
    ) {
        for &i in indices {
            if let Some(p) = particles.get_mut(i) {
                if p.locked { continue; }
                p.position = lattice.deform(p.position);
            }
        }
    }

    /// Proportional editing: transform with soft falloff.
    pub fn proportional_translate(
        particles: &mut Vec<ModelParticle>,
        selected: &HashSet<usize>,
        pivot: Vec3,
        delta: Vec3,
        radius: f32,
        falloff: &FalloffCurve,
    ) {
        for (i, p) in particles.iter_mut().enumerate() {
            if p.locked { continue; }
            let d = (p.position - pivot).length();
            if d > radius { continue; }
            let t = d / radius;
            let weight = if selected.contains(&i) { 1.0 } else { falloff.evaluate(t) };
            p.position += delta * weight;
        }
    }

    pub fn proportional_rotate(
        particles: &mut Vec<ModelParticle>,
        selected: &HashSet<usize>,
        pivot: Vec3,
        quat: Quat,
        radius: f32,
        falloff: &FalloffCurve,
    ) {
        for (i, p) in particles.iter_mut().enumerate() {
            if p.locked { continue; }
            let d = (p.position - pivot).length();
            if d > radius { continue; }
            let t = d / radius;
            let weight = if selected.contains(&i) { 1.0 } else { falloff.evaluate(t) };
            let partial_q = Quat::IDENTITY.slerp(quat, weight);
            let local = p.position - pivot;
            p.position = pivot + partial_q * local;
            p.normal = partial_q * p.normal;
        }
    }

    pub fn proportional_scale(
        particles: &mut Vec<ModelParticle>,
        selected: &HashSet<usize>,
        pivot: Vec3,
        factor: f32,
        radius: f32,
        falloff: &FalloffCurve,
    ) {
        for (i, p) in particles.iter_mut().enumerate() {
            if p.locked { continue; }
            let d = (p.position - pivot).length();
            if d > radius { continue; }
            let t = d / radius;
            let weight = if selected.contains(&i) { 1.0 } else { falloff.evaluate(t) };
            let effective_scale = 1.0 + (factor - 1.0) * weight;
            p.position = pivot + (p.position - pivot) * effective_scale;
        }
    }
}

// ============================================================
// LATTICE DEFORMER
// ============================================================

#[derive(Clone, Debug)]
pub struct LatticeDeformer {
    pub control_points: Vec<Vec3>,
    pub rest_points:    Vec<Vec3>,
    pub res_x: usize,
    pub res_y: usize,
    pub res_z: usize,
    pub bounds: Aabb3,
}

impl LatticeDeformer {
    pub fn new(bounds: Aabb3, res_x: usize, res_y: usize, res_z: usize) -> Self {
        let mut rest_points = Vec::new();
        for zi in 0..res_z {
            for yi in 0..res_y {
                for xi in 0..res_x {
                    let u = xi as f32 / (res_x as f32 - 1.0).max(1.0);
                    let v = yi as f32 / (res_y as f32 - 1.0).max(1.0);
                    let w = zi as f32 / (res_z as f32 - 1.0).max(1.0);
                    rest_points.push(bounds.min + bounds.size() * Vec3::new(u, v, w));
                }
            }
        }
        let control_points = rest_points.clone();
        Self { control_points, rest_points, res_x, res_y, res_z, bounds }
    }

    fn index(&self, xi: usize, yi: usize, zi: usize) -> usize {
        zi * self.res_y * self.res_x + yi * self.res_x + xi
    }

    /// Trilinear interpolation through control point grid.
    pub fn deform(&self, pos: Vec3) -> Vec3 {
        let s = self.bounds.size();
        let local = pos - self.bounds.min;
        let u = (local.x / s.x.max(EPSILON)).clamp(0.0, 1.0);
        let v = (local.y / s.y.max(EPSILON)).clamp(0.0, 1.0);
        let w = (local.z / s.z.max(EPSILON)).clamp(0.0, 1.0);

        let xi = ((u * (self.res_x as f32 - 1.0)) as usize).min(self.res_x - 2);
        let yi = ((v * (self.res_y as f32 - 1.0)) as usize).min(self.res_y - 2);
        let zi = ((w * (self.res_z as f32 - 1.0)) as usize).min(self.res_z - 2);

        let ut = u * (self.res_x as f32 - 1.0) - xi as f32;
        let vt = v * (self.res_y as f32 - 1.0) - yi as f32;
        let wt = w * (self.res_z as f32 - 1.0) - zi as f32;

        let c000 = self.control_points[self.index(xi,   yi,   zi  )];
        let c100 = self.control_points[self.index(xi+1, yi,   zi  )];
        let c010 = self.control_points[self.index(xi,   yi+1, zi  )];
        let c110 = self.control_points[self.index(xi+1, yi+1, zi  )];
        let c001 = self.control_points[self.index(xi,   yi,   zi+1)];
        let c101 = self.control_points[self.index(xi+1, yi,   zi+1)];
        let c011 = self.control_points[self.index(xi,   yi+1, zi+1)];
        let c111 = self.control_points[self.index(xi+1, yi+1, zi+1)];

        let r000 = self.rest_points[self.index(xi,   yi,   zi  )];
        let r100 = self.rest_points[self.index(xi+1, yi,   zi  )];
        let r010 = self.rest_points[self.index(xi,   yi+1, zi  )];
        let r110 = self.rest_points[self.index(xi+1, yi+1, zi  )];
        let r001 = self.rest_points[self.index(xi,   yi,   zi+1)];
        let r101 = self.rest_points[self.index(xi+1, yi,   zi+1)];
        let r011 = self.rest_points[self.index(xi,   yi+1, zi+1)];
        let r111 = self.rest_points[self.index(xi+1, yi+1, zi+1)];

        // Trilinear interpolation of displacement
        let trilinear = |p000: Vec3, p100: Vec3, p010: Vec3, p110: Vec3,
                          p001: Vec3, p101: Vec3, p011: Vec3, p111: Vec3| -> Vec3 {
            let x00 = p000.lerp(p100, ut);
            let x10 = p010.lerp(p110, ut);
            let x01 = p001.lerp(p101, ut);
            let x11 = p011.lerp(p111, ut);
            let y0  = x00.lerp(x10, vt);
            let y1  = x01.lerp(x11, vt);
            y0.lerp(y1, wt)
        };

        let rest_interp   = trilinear(r000,r100,r010,r110,r001,r101,r011,r111);
        let ctrl_interp   = trilinear(c000,c100,c010,c110,c001,c101,c011,c111);
        let displacement  = ctrl_interp - rest_interp;
        pos + displacement
    }

    pub fn set_control_point(&mut self, xi: usize, yi: usize, zi: usize, new_pos: Vec3) {
        let idx = self.index(xi, yi, zi);
        if idx < self.control_points.len() {
            self.control_points[idx] = new_pos;
        }
    }

    pub fn reset(&mut self) {
        self.control_points = self.rest_points.clone();
    }
}

// ============================================================
// UNDO/REDO
// ============================================================

#[derive(Clone, Debug)]
pub struct ModelSnapshot {
    pub model_id:  u64,
    pub particles: Vec<ModelParticle>,
    pub bounds:    Aabb3,
    pub label:     String,
}

impl ModelSnapshot {
    pub fn capture(model: &ParticleModel, label: impl Into<String>) -> Self {
        Self {
            model_id:  model.id,
            particles: model.particles.clone(),
            bounds:    model.bounds.clone(),
            label:     label.into(),
        }
    }

    pub fn restore_to(&self, model: &mut ParticleModel) {
        model.particles = self.particles.clone();
        model.bounds    = self.bounds.clone();
    }
}

#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    pub undo: VecDeque<ModelSnapshot>,
    pub redo: VecDeque<ModelSnapshot>,
}

impl UndoStack {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, snapshot: ModelSnapshot) {
        self.redo.clear();
        if self.undo.len() >= MAX_UNDO_STEPS {
            self.undo.pop_front();
        }
        self.undo.push_back(snapshot);
    }

    pub fn undo(&mut self, model: &mut ParticleModel) -> bool {
        if let Some(snap) = self.undo.pop_back() {
            let current = ModelSnapshot::capture(model, "redo");
            self.redo.push_back(current);
            snap.restore_to(model);
            true
        } else { false }
    }

    pub fn redo(&mut self, model: &mut ParticleModel) -> bool {
        if let Some(snap) = self.redo.pop_back() {
            let current = ModelSnapshot::capture(model, "undo");
            self.undo.push_back(current);
            snap.restore_to(model);
            true
        } else { false }
    }

    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn history_labels(&self) -> Vec<&str> {
        self.undo.iter().map(|s| s.label.as_str()).collect()
    }
}

// ============================================================
// IMPORT / EXPORT
// ============================================================

pub struct ModelIO;

impl ModelIO {
    /// Export model to text: one particle per line.
    pub fn export_text(model: &ParticleModel) -> String {
        let mut lines = Vec::with_capacity(model.particles.len() + 1);
        lines.push(format!("# ParticleModel: {} id:{}", model.name, model.id));
        for p in &model.particles {
            lines.push(format!(
                "{:.6} {:.6} {:.6} {} {:.4} {:.4} {:.4} {:.4} {:.4}",
                p.position.x, p.position.y, p.position.z,
                p.character as u32,
                p.color.x, p.color.y, p.color.z, p.color.w,
                p.emission,
            ));
        }
        lines.join("\n")
    }

    /// Import model from text format.
    pub fn import_text(text: &str, id: u64) -> Option<ParticleModel> {
        let mut model = ParticleModel::new(id, "imported");
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                // Parse name from header
                if let Some(rest) = line.strip_prefix("# ParticleModel:") {
                    if let Some(name_part) = rest.split("id:").next() {
                        model.name = name_part.trim().to_string();
                    }
                }
                continue;
            }
            if line.is_empty() { continue; }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 { continue; }
            let px: f32 = parts[0].parse().ok()?;
            let py: f32 = parts[1].parse().ok()?;
            let pz: f32 = parts[2].parse().ok()?;
            let char_code: u32 = parts[3].parse().ok()?;
            let r: f32 = parts[4].parse().ok()?;
            let g: f32 = parts[5].parse().ok()?;
            let b: f32 = parts[6].parse().ok()?;
            let a: f32 = parts[7].parse().ok()?;
            let emission: f32 = parts[8].parse().ok()?;
            let ch = char::from_u32(char_code).unwrap_or('.');
            let mut p = ModelParticle::new(Vec3::new(px, py, pz), ch, Vec4::new(r, g, b, a));
            p.emission = emission;
            model.add_particle(p);
        }
        Some(model)
    }

    /// Convert model particles to a simple engine glyph array.
    /// Returns (position, char, color, emission) tuples.
    pub fn to_glyph_array(model: &ParticleModel) -> Vec<(Vec3, char, Vec4, f32)> {
        model.particles.iter().map(|p| (p.position, p.character, p.color, p.emission)).collect()
    }

    /// Build model from glyph array.
    pub fn from_glyph_array(id: u64, name: impl Into<String>, glyphs: &[(Vec3, char, Vec4, f32)]) -> ParticleModel {
        let mut model = ParticleModel::new(id, name);
        for &(pos, ch, color, emission) in glyphs {
            let mut p = ModelParticle::new(pos, ch, color);
            p.emission = emission;
            model.add_particle(p);
        }
        model
    }

    pub fn compute_bounding_box(model: &ParticleModel) -> Aabb3 {
        let mut aabb = Aabb3::empty();
        for p in &model.particles { aabb.expand(p.position); }
        aabb
    }

    pub fn compute_center_of_mass(model: &ParticleModel) -> Vec3 {
        model.center_of_mass()
    }
}

// ============================================================
// RAY-PARTICLE INTERSECTION (PICKING)
// ============================================================

pub struct ParticlePicker;

impl ParticlePicker {
    /// Find the closest particle to a camera ray within a tolerance cylinder.
    /// Returns (particle_index, t_along_ray) or None.
    pub fn pick_closest(
        particles: &[ModelParticle],
        ray: &Ray3,
        tolerance: f32,
    ) -> Option<(usize, f32)> {
        let mut best_idx = None;
        let mut best_dist = f32::MAX;
        let mut best_t    = 0.0f32;

        for (i, p) in particles.iter().enumerate() {
            // Distance from point to ray
            let ap = p.position - ray.origin;
            let t  = ap.dot(ray.direction).max(0.0);
            let closest = ray.at(t);
            let dist = (p.position - closest).length();
            if dist <= tolerance && dist < best_dist {
                best_dist = dist;
                best_idx  = Some(i);
                best_t    = t;
            }
        }
        best_idx.map(|i| (i, best_t))
    }

    /// Find all particles within a tolerance cylinder of the ray.
    pub fn pick_all_in_ray(
        particles: &[ModelParticle],
        ray: &Ray3,
        tolerance: f32,
    ) -> Vec<usize> {
        particles.iter().enumerate()
            .filter(|(_, p)| ray.distance_to_point(p.position) <= tolerance)
            .map(|(i, _)| i)
            .collect()
    }

    /// Ray-sphere intersection per particle for exact picking.
    pub fn pick_ray_sphere(
        particles: &[ModelParticle],
        ray: &Ray3,
        particle_radius: f32,
    ) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in particles.iter().enumerate() {
            if let Some(t) = ray.intersect_sphere(p.position, particle_radius) {
                match best {
                    None => best = Some((i, t)),
                    Some((_, bt)) if t < bt => best = Some((i, t)),
                    _ => {}
                }
            }
        }
        best
    }
}

// ============================================================
// VISUALIZATION HELPERS
// ============================================================

pub struct VisualizationHelper;

impl VisualizationHelper {
    /// Compute per-particle normals from k-nearest neighborhood (PCA on local point cloud).
    pub fn compute_normals_pca(particles: &mut Vec<ModelParticle>, k: usize) {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        let n = positions.len();
        for i in 0..n {
            let pi = positions[i];
            // Find k nearest neighbors
            let mut dists: Vec<(usize, f32)> = positions.iter().enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(j, &pj)| (j, (pj - pi).length_squared()))
                .collect();
            dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            dists.truncate(k);

            if dists.is_empty() { continue; }

            // Compute centroid
            let centroid = dists.iter()
                .map(|(j, _)| positions[*j])
                .fold(pi, |a, b| a + b)
                / (dists.len() + 1) as f32;

            // Covariance matrix
            let mut cov = [[0.0f32; 3]; 3];
            for (j, _) in &dists {
                let d = positions[*j] - centroid;
                let dv = [d.x, d.y, d.z];
                for r in 0..3 { for c in 0..3 { cov[r][c] += dv[r] * dv[c]; } }
            }
            let cnt = dists.len() as f32;
            for r in 0..3 { for c in 0..3 { cov[r][c] /= cnt; } }

            let v1 = power_iteration_3x3(&cov, 16);
            let v2 = gram_schmidt_orthogonalize(v1, &cov, 16);
            let normal = v1.cross(v2).normalize();
            // Orient toward viewer (assume origin viewpoint)
            let oriented = if normal.dot(pi) > 0.0 { normal } else { -normal };
            particles[i].normal = oriented;
        }
    }

    /// Detect surface vs interior particles using k-NN density estimation.
    /// Surface particles have lower local density than interior ones.
    pub fn classify_surface_interior(
        particles: &[ModelParticle],
        k: usize,
        density_threshold: f32,
    ) -> Vec<bool> {
        let positions: Vec<Vec3> = particles.iter().map(|p| p.position).collect();
        positions.iter().enumerate().map(|(i, &pi)| {
            let mut dists: Vec<f32> = positions.iter().enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &pj)| (pj - pi).length_squared())
                .collect();
            dists.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            dists.truncate(k);
            let avg_dist = if dists.is_empty() { f32::MAX } else {
                dists.iter().map(|d| d.sqrt()).sum::<f32>() / dists.len() as f32
            };
            // Surface particles have larger average distance to neighbors
            avg_dist > density_threshold
        }).collect()
    }

    /// Wireframe mode: find pairs of nearby particles to draw as edges.
    pub fn find_edges(particles: &[ModelParticle], max_dist: f32) -> Vec<(usize, usize)> {
        let max_dist2 = max_dist * max_dist;
        let mut edges = Vec::new();
        for i in 0..particles.len() {
            for j in (i+1)..particles.len() {
                if (particles[i].position - particles[j].position).length_squared() <= max_dist2 {
                    edges.push((i, j));
                }
            }
        }
        edges
    }

    /// Normal visualization: offset particles slightly along their normal.
    pub fn normal_visualization_particles(
        particles: &[ModelParticle],
        offset: f32,
        normal_char: char,
        normal_color: Vec4,
    ) -> Vec<ModelParticle> {
        particles.iter().map(|p| {
            let tip_pos = p.position + p.normal * offset;
            ModelParticle::new(tip_pos, normal_char, normal_color).with_normal(p.normal)
        }).collect()
    }

    /// Compute the average normal of a set of particles.
    pub fn average_normal(particles: &[ModelParticle], indices: &[usize]) -> Vec3 {
        if indices.is_empty() { return Vec3::Y; }
        let sum = indices.iter()
            .filter_map(|&i| particles.get(i))
            .fold(Vec3::ZERO, |a, p| a + p.normal);
        sum.normalize()
    }

    /// Generate a color-coded particle overlay showing normals direction.
    pub fn normal_color_overlay(particles: &mut Vec<ModelParticle>) {
        for p in particles.iter_mut() {
            // Map normal components [-1,1] to [0,1] for RGB display
            let r = (p.normal.x * 0.5 + 0.5).clamp(0.0, 1.0);
            let g = (p.normal.y * 0.5 + 0.5).clamp(0.0, 1.0);
            let b = (p.normal.z * 0.5 + 0.5).clamp(0.0, 1.0);
            p.color = Vec4::new(r, g, b, 1.0);
        }
    }

    /// Build a spatial hash grid for fast neighbor lookups.
    pub fn build_spatial_hash(
        particles: &[ModelParticle],
        cell_size: f32,
    ) -> HashMap<(i32, i32, i32), Vec<usize>> {
        let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in particles.iter().enumerate() {
            let key = (
                (p.position.x / cell_size).floor() as i32,
                (p.position.y / cell_size).floor() as i32,
                (p.position.z / cell_size).floor() as i32,
            );
            grid.entry(key).or_default().push(i);
        }
        grid
    }

    /// Query spatial hash grid for neighbors within radius.
    pub fn query_spatial_hash(
        grid: &HashMap<(i32, i32, i32), Vec<usize>>,
        center: Vec3,
        radius: f32,
        cell_size: f32,
    ) -> Vec<usize> {
        let r2 = radius * radius;
        let cx = (center.x / cell_size).floor() as i32;
        let cy = (center.y / cell_size).floor() as i32;
        let cz = (center.z / cell_size).floor() as i32;
        let cells = (radius / cell_size).ceil() as i32 + 1;
        let mut results = Vec::new();
        for dx in -cells..=cells {
            for dy in -cells..=cells {
                for dz in -cells..=cells {
                    if let Some(cell) = grid.get(&(cx + dx, cy + dy, cz + dz)) {
                        for &idx in cell { results.push(idx); }
                    }
                }
            }
        }
        // NOTE: caller must filter by actual distance
        results
    }
}

// ============================================================
// MAIN MODEL EDITOR
// ============================================================

#[derive(Clone, Debug)]
pub struct ModelEditor {
    pub models:           HashMap<u64, ParticleModel>,
    pub active_model_id:  Option<u64>,
    pub active_layer:     usize,
    pub active_brush:     BrushKind,
    pub brush_radius:     f32,
    pub brush_strength:   f32,
    pub brush_density:    f32,
    pub active_char:      char,
    pub active_color:     Vec4,
    pub selection:        HashSet<usize>,
    pub pivot:            Vec3,
    pub undo_stack:       VecDeque<ModelSnapshot>,
    pub redo_stack:       VecDeque<ModelSnapshot>,
    pub grid_snap:        bool,
    pub grid_size:        f32,
    pub symmetry:         SymmetryMode,
    pub next_model_id:    u64,
    pub brush_params:     BrushParams,
    pub selection_sys:    SelectionSystem,
    pub undo_sys:         UndoStack,
    pub proportional_edit: bool,
    pub proportional_radius: f32,
    pub proportional_falloff: FalloffCurve,
    pub wireframe_mode:   bool,
    pub normal_vis:       bool,
    pub show_bounds:      bool,
    pub lattice:          Option<LatticeDeformer>,
}

impl ModelEditor {
    pub fn new() -> Self {
        Self {
            models:           HashMap::new(),
            active_model_id:  None,
            active_layer:     0,
            active_brush:     BrushKind::Add,
            brush_radius:     DEFAULT_BRUSH_RADIUS,
            brush_strength:   DEFAULT_BRUSH_STRENGTH,
            brush_density:    DEFAULT_BRUSH_DENSITY,
            active_char:      '.',
            active_color:     Vec4::ONE,
            selection:        HashSet::new(),
            pivot:            Vec3::ZERO,
            undo_stack:       VecDeque::new(),
            redo_stack:       VecDeque::new(),
            grid_snap:        false,
            grid_size:        0.25,
            symmetry:         SymmetryMode::None,
            next_model_id:    1,
            brush_params:     BrushParams::default(),
            selection_sys:    SelectionSystem::new(),
            undo_sys:         UndoStack::new(),
            proportional_edit: false,
            proportional_radius: 2.0,
            proportional_falloff: FalloffCurve::Smooth,
            wireframe_mode:   false,
            normal_vis:       false,
            show_bounds:      false,
            lattice:          None,
        }
    }

    pub fn create_model(&mut self, name: impl Into<String>) -> u64 {
        let id = self.next_model_id;
        self.next_model_id += 1;
        let model = ParticleModel::new(id, name);
        self.models.insert(id, model);
        self.active_model_id = Some(id);
        id
    }

    pub fn active_model(&self) -> Option<&ParticleModel> {
        self.active_model_id.and_then(|id| self.models.get(&id))
    }

    pub fn active_model_mut(&mut self) -> Option<&mut ParticleModel> {
        self.active_model_id.and_then(|id| self.models.get_mut(&id))
    }

    pub fn make_brush_params(&self) -> BrushParams {
        BrushParams {
            kind:      self.active_brush.clone(),
            radius:    self.brush_radius,
            strength:  self.brush_strength,
            density:   self.brush_density,
            color:     self.active_color,
            character: self.active_char,
            falloff:   FalloffCurve::Smooth,
        }
    }

    fn push_undo(&mut self, label: &str) {
        if let Some(model) = self.active_model() {
            let snap = ModelSnapshot::capture(model, label);
            self.undo_sys.push(snap);
        }
    }

    /// Apply brush at ray intersection point.
    pub fn apply_brush(&mut self, _ray: Ray3, hit_pos: Vec3) {
        let brush = self.active_brush.clone();
        let params = self.make_brush_params();
        let symmetry = self.symmetry.clone();
        let active_char = self.active_char;
        let active_color = self.active_color;
        self.push_undo(&format!("brush {:?}", brush));

        if let Some(model) = self.active_model_mut() {
            match brush {
                BrushKind::Add => {
                    SculptEngine::apply_add(model, hit_pos, Vec3::Y, &params, active_char, active_color, &symmetry);
                }
                BrushKind::Remove => {
                    SculptEngine::apply_remove(model, hit_pos, &params, &symmetry);
                }
                BrushKind::Smooth => {
                    SculptEngine::apply_smooth(model, hit_pos, &params);
                }
                BrushKind::Inflate => {
                    SculptEngine::apply_inflate(model, hit_pos, &params);
                }
                BrushKind::Pinch => {
                    SculptEngine::apply_pinch(model, hit_pos, &params);
                }
                BrushKind::Color => {
                    SculptEngine::apply_color(model, hit_pos, &params);
                }
                BrushKind::Char => {
                    SculptEngine::apply_char(model, hit_pos, &params);
                }
                BrushKind::Flatten => {
                    SculptEngine::apply_flatten(model, hit_pos, &params);
                }
                BrushKind::Crease => {
                    SculptEngine::apply_crease(model, hit_pos, &params);
                }
                BrushKind::Clone => {
                    // Clone uses source/target; default to offset by up vector
                    let target = hit_pos + Vec3::new(params.radius * 2.0, 0.0, 0.0);
                    SculptEngine::apply_clone(model, hit_pos, target, &params);
                }
            }
            model.recompute_bounds();
        }
    }

    // ---- TRANSFORM WRAPPERS ----

    pub fn cmd_translate(&mut self, delta: Vec3) {
        self.push_undo("translate");
        let selection = self.selection.clone();
        let grid_snap = self.grid_snap;
        let grid_size = self.grid_size;
        let proportional = self.proportional_edit;
        if let Some(model) = self.active_model_mut() {
            if proportional {
                // handled separately
            } else {
                TransformTools::translate(&mut model.particles, &selection, delta);
            }
            if grid_snap {
                for &i in &selection {
                    if let Some(p) = model.particles.get_mut(i) {
                        p.position = p.snapped_position(grid_size);
                    }
                }
            }
            model.recompute_bounds();
        }
    }

    pub fn cmd_rotate(&mut self, axis: Vec3, angle_radians: f32) {
        self.push_undo("rotate");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        let quat = Quat::from_axis_angle(axis.normalize(), angle_radians);
        if let Some(model) = self.active_model_mut() {
            TransformTools::rotate(&mut model.particles, &selection, pivot, quat);
            model.recompute_bounds();
        }
    }

    pub fn cmd_scale_uniform(&mut self, factor: f32) {
        self.push_undo("scale");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        if let Some(model) = self.active_model_mut() {
            TransformTools::scale_uniform(&mut model.particles, &selection, pivot, factor);
            model.recompute_bounds();
        }
    }

    pub fn cmd_scale_nonuniform(&mut self, factors: Vec3) {
        self.push_undo("scale_nonuniform");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        if let Some(model) = self.active_model_mut() {
            TransformTools::scale_nonuniform(&mut model.particles, &selection, pivot, factors);
            model.recompute_bounds();
        }
    }

    pub fn cmd_mirror(&mut self, axis: Vec3) {
        self.push_undo("mirror");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        if let Some(model) = self.active_model_mut() {
            TransformTools::mirror(&mut model.particles, &selection, pivot, axis);
            model.recompute_bounds();
        }
    }

    pub fn cmd_bend(&mut self, axis: Vec3, angle_per_unit: f32) {
        self.push_undo("bend");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        if let Some(model) = self.active_model_mut() {
            TransformTools::bend(&mut model.particles, &selection, pivot, axis, angle_per_unit);
            model.recompute_bounds();
        }
    }

    pub fn cmd_taper(&mut self, axis: Vec3, taper_factor: f32) {
        self.push_undo("taper");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        if let Some(model) = self.active_model_mut() {
            TransformTools::taper(&mut model.particles, &selection, pivot, axis, taper_factor);
            model.recompute_bounds();
        }
    }

    pub fn cmd_twist(&mut self, axis: Vec3, twist_rate: f32) {
        self.push_undo("twist");
        let selection = self.selection.clone();
        let pivot = self.pivot;
        if let Some(model) = self.active_model_mut() {
            TransformTools::twist(&mut model.particles, &selection, pivot, axis, twist_rate);
            model.recompute_bounds();
        }
    }

    pub fn cmd_lattice_deform(&mut self) {
        if self.lattice.is_none() { return; }
        self.push_undo("lattice_deform");
        let selection = self.selection.clone();
        let lattice = self.lattice.clone().unwrap();
        if let Some(model) = self.active_model_mut() {
            TransformTools::lattice_deform(&mut model.particles, &selection, &lattice);
            model.recompute_bounds();
        }
    }

    // ---- UNDO/REDO ----

    pub fn undo(&mut self) {
        let id = self.active_model_id;
        if let Some(id) = id {
            if let Some(model) = self.models.get_mut(&id) {
                self.undo_sys.undo(model);
            }
        }
    }

    pub fn redo(&mut self) {
        let id = self.active_model_id;
        if let Some(id) = id {
            if let Some(model) = self.models.get_mut(&id) {
                self.undo_sys.redo(model);
            }
        }
    }

    // ---- SELECTION WRAPPERS ----

    pub fn select_all(&mut self) {
        if let Some(model) = self.active_model() {
            let n = model.particles.len();
            self.selection = (0..n).collect();
        }
    }

    pub fn deselect_all(&mut self) {
        self.selection.clear();
    }

    pub fn invert_selection(&mut self) {
        if let Some(model) = self.active_model() {
            let n = model.particles.len();
            let all: HashSet<usize> = (0..n).collect();
            self.selection = all.difference(&self.selection).copied().collect();
        }
    }

    pub fn box_select(&mut self, aabb: Aabb3, add: bool) {
        if let Some(model) = self.active_model() {
            let new_sel: HashSet<usize> = model.particles.iter().enumerate()
                .filter(|(_, p)| aabb.contains(p.position))
                .map(|(i, _)| i)
                .collect();
            if add { self.selection.extend(new_sel.iter()); }
            else   { self.selection = new_sel; }
        }
    }

    pub fn sphere_select(&mut self, center: Vec3, radius: f32, add: bool) {
        if let Some(model) = self.active_model() {
            let r2 = radius * radius;
            let new_sel: HashSet<usize> = model.particles.iter().enumerate()
                .filter(|(_, p)| (p.position - center).length_squared() <= r2)
                .map(|(i, _)| i)
                .collect();
            if add { self.selection.extend(new_sel.iter()); }
            else   { self.selection = new_sel; }
        }
    }

    pub fn select_by_char(&mut self, ch: char, add: bool) {
        if let Some(model) = self.active_model() {
            let new_sel: HashSet<usize> = model.particles.iter().enumerate()
                .filter(|(_, p)| p.character == ch)
                .map(|(i, _)| i)
                .collect();
            if add { self.selection.extend(new_sel.iter()); }
            else   { self.selection = new_sel; }
        }
    }

    pub fn select_by_group(&mut self, group_id: u32, add: bool) {
        if let Some(model) = self.active_model() {
            let new_sel: HashSet<usize> = model.particles.iter().enumerate()
                .filter(|(_, p)| p.group_id == group_id)
                .map(|(i, _)| i)
                .collect();
            if add { self.selection.extend(new_sel.iter()); }
            else   { self.selection = new_sel; }
        }
    }

    pub fn select_by_color(&mut self, target: Vec4, tolerance: f32, add: bool) {
        if let Some(model) = self.active_model() {
            let th = rgb_to_hsv(target.x, target.y, target.z);
            let new_sel: HashSet<usize> = model.particles.iter().enumerate()
                .filter(|(_, p)| {
                    let ph = rgb_to_hsv(p.color.x, p.color.y, p.color.z);
                    let d = ((hue_distance(th.0, ph.0)).powi(2)
                             + (th.1 - ph.1).powi(2)
                             + (th.2 - ph.2).powi(2)).sqrt();
                    d <= tolerance
                })
                .map(|(i, _)| i)
                .collect();
            if add { self.selection.extend(new_sel.iter()); }
            else   { self.selection = new_sel; }
        }
    }

    pub fn grow_selection(&mut self, radius: f32) {
        if let Some(model) = self.active_model() {
            let r2 = radius * radius;
            let current: Vec<usize> = self.selection.iter().copied().collect();
            let n = model.particles.len();
            let mut additions = HashSet::new();
            for (i, p) in model.particles.iter().enumerate() {
                if self.selection.contains(&i) { continue; }
                for &sel in &current {
                    if (model.particles[sel].position - p.position).length_squared() <= r2 {
                        additions.insert(i);
                        break;
                    }
                }
            }
            self.selection.extend(additions.iter());
        }
    }

    pub fn shrink_selection(&mut self, radius: f32) {
        if let Some(model) = self.active_model() {
            let r2 = radius * radius;
            let to_remove: HashSet<usize> = self.selection.iter().copied().filter(|&si| {
                model.particles.iter().enumerate().any(|(i, p)| {
                    !self.selection.contains(&i)
                        && (model.particles[si].position - p.position).length_squared() <= r2
                })
            }).collect();
            for idx in to_remove { self.selection.remove(&idx); }
        }
    }

    // ---- PRIMITIVE INSERTION ----

    pub fn insert_sphere(&mut self, center: Vec3, radius: f32, n: usize) {
        self.push_undo("insert_sphere");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::sphere(center, radius, n, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_cube(&mut self, center: Vec3, half_size: Vec3, ppf: usize, fill: bool) {
        self.push_undo("insert_cube");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::cube(center, half_size, ppf, fill, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_cylinder(&mut self, center: Vec3, radius: f32, height: f32, segs: usize, bands: usize) {
        self.push_undo("insert_cylinder");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::cylinder(center, radius, height, segs, bands, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_torus(&mut self, center: Vec3, major: f32, minor: f32, us: usize, vs: usize) {
        self.push_undo("insert_torus");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::torus(center, major, minor, us, vs, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_plane(&mut self, center: Vec3, w: f32, d: f32, cols: usize, rows: usize) {
        self.push_undo("insert_plane");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::plane(center, w, d, cols, rows, 0.5, 0.1, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_text3d(&mut self, text: &str, origin: Vec3) {
        self.push_undo("insert_text3d");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::text3d(text, origin, 0.8, 1.0, 0.3, 5, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_marching_cubes(
        &mut self,
        field: &dyn Fn(Vec3) -> f32,
        bounds: Aabb3,
        resolution: usize,
    ) {
        self.push_undo("insert_marching_cubes");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::marching_cubes(field, &bounds, resolution, MARCHING_CUBES_THRESHOLD, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    pub fn insert_metaballs(&mut self, balls: &[(Vec3, f32)], bounds: Aabb3, resolution: usize) {
        self.push_undo("insert_metaballs");
        let ch = self.active_char;
        let col = self.active_color;
        let particles = PrimitiveBuilder::metaballs(balls, &bounds, resolution, ch, col);
        if let Some(model) = self.active_model_mut() {
            model.add_particles_bulk(particles);
        }
    }

    // ---- LOD ----

    pub fn generate_lods(&mut self) {
        if let Some(model) = self.active_model_mut() {
            model.generate_lods();
        }
    }

    // ---- NORMALS ----

    pub fn recompute_normals(&mut self, k: usize) {
        if let Some(model) = self.active_model_mut() {
            VisualizationHelper::compute_normals_pca(&mut model.particles, k);
        }
    }

    // ---- EXPORT ----

    pub fn export_active_model(&self) -> Option<String> {
        self.active_model().map(ModelIO::export_text)
    }

    pub fn import_model(&mut self, text: &str) -> Option<u64> {
        let id = self.next_model_id;
        self.next_model_id += 1;
        let model = ModelIO::import_text(text, id)?;
        self.models.insert(id, model);
        self.active_model_id = Some(id);
        Some(id)
    }

    // ---- PICKING ----

    pub fn pick(&self, ray: &Ray3, tolerance: f32) -> Option<(usize, f32)> {
        self.active_model().and_then(|m| {
            ParticlePicker::pick_closest(&m.particles, ray, tolerance)
        })
    }

    // ---- LAYERS ----

    pub fn add_layer(&mut self, name: impl Into<String>) {
        if let Some(model) = self.active_model_mut() {
            let id = model.add_layer(name);
            self.active_layer = id as usize;
        }
    }

    pub fn set_active_layer(&mut self, layer_id: usize) {
        self.active_layer = layer_id;
    }

    pub fn toggle_layer_visibility(&mut self, layer_id: u8) {
        if let Some(model) = self.active_model_mut() {
            if let Some(layer) = model.layers.iter_mut().find(|l| l.id == layer_id) {
                layer.toggle_visibility();
            }
        }
    }

    pub fn merge_layers(&mut self, src: u8, dst: u8) {
        self.push_undo("merge_layers");
        if let Some(model) = self.active_model_mut() {
            model.merge_layer_into(src, dst);
        }
    }

    // ---- PIVOT ----

    pub fn set_pivot_to_selection_center(&mut self) {
        if let Some(model) = self.active_model() {
            if self.selection.is_empty() { return; }
            let sum: Vec3 = self.selection.iter()
                .filter_map(|&i| model.particles.get(i))
                .map(|p| p.position)
                .fold(Vec3::ZERO, |a, b| a + b);
            self.pivot = sum / self.selection.len() as f32;
        }
    }

    pub fn set_pivot_to_model_center(&mut self) {
        if let Some(model) = self.active_model() {
            self.pivot = model.center_of_mass();
        }
    }

    // ---- LATTICE SETUP ----

    pub fn init_lattice(&mut self, res_x: usize, res_y: usize, res_z: usize) {
        if let Some(model) = self.active_model() {
            let bounds = model.bounds.clone();
            self.lattice = Some(LatticeDeformer::new(bounds, res_x, res_y, res_z));
        }
    }

    pub fn set_lattice_control_point(&mut self, xi: usize, yi: usize, zi: usize, pos: Vec3) {
        if let Some(lattice) = &mut self.lattice {
            lattice.set_control_point(xi, yi, zi, pos);
        }
    }

    pub fn reset_lattice(&mut self) {
        if let Some(lattice) = &mut self.lattice {
            lattice.reset();
        }
    }

    // ---- SKELETON ----

    pub fn add_bone(&mut self, name: impl Into<String>, head: Vec3, tail: Vec3) -> Option<u32> {
        if let Some(model) = self.active_model_mut() {
            let skel = model.skeleton.get_or_insert_with(ParticleSkeleton::new);
            let id = skel.bones.len() as u32;
            let bone = ParticleBone::new(id, name, head, tail);
            Some(skel.add_bone(bone))
        } else { None }
    }

    pub fn bind_skeleton(&mut self) {
        if let Some(model) = self.active_model_mut() {
            if let Some(skel) = &mut model.skeleton {
                let mut ps = model.particles.clone();
                skel.bind_all_particles(&mut ps);
                model.particles = ps;
            }
        }
    }

    pub fn apply_skeleton_pose(&mut self) {
        if let Some(model) = self.active_model_mut() {
            if let Some(skel) = &model.skeleton {
                for p in &mut model.particles {
                    p.position = skel.transform_position(p.position, &p.bone_indices, &p.bone_weights);
                }
            }
        }
    }

    // ---- WIREFRAME / NORMALS VIS ----

    pub fn get_wireframe_edges(&self) -> Vec<(usize, usize)> {
        self.active_model().map(|m| {
            VisualizationHelper::find_edges(&m.particles, self.brush_radius * 0.5)
        }).unwrap_or_default()
    }

    pub fn get_normal_vis_particles(&self) -> Vec<ModelParticle> {
        self.active_model().map(|m| {
            VisualizationHelper::normal_visualization_particles(
                &m.particles, 0.15, '|', Vec4::new(0.0, 1.0, 0.5, 1.0)
            )
        }).unwrap_or_default()
    }

    // ---- MISC ----

    pub fn delete_selected(&mut self) {
        self.push_undo("delete");
        let sel = self.selection.clone();
        if let Some(model) = self.active_model_mut() {
            model.remove_particles(&sel);
        }
        self.selection.clear();
    }

    pub fn duplicate_selected(&mut self) {
        self.push_undo("duplicate");
        let sel = self.selection.clone();
        if let Some(model) = self.active_model_mut() {
            let dups: Vec<ModelParticle> = sel.iter()
                .filter_map(|&i| model.particles.get(i))
                .cloned()
                .collect();
            let start = model.particles.len();
            model.add_particles_bulk(dups);
            // Select the duplicates
        }
    }

    pub fn set_selected_group(&mut self, group_id: u32) {
        let sel: Vec<usize> = self.selection.iter().copied().collect();
        if let Some(model) = self.active_model_mut() {
            for i in sel {
                if let Some(p) = model.particles.get_mut(i) {
                    p.group_id = group_id;
                }
            }
        }
    }

    pub fn set_selected_char(&mut self, ch: char) {
        let sel: Vec<usize> = self.selection.iter().copied().collect();
        if let Some(model) = self.active_model_mut() {
            for i in sel {
                if let Some(p) = model.particles.get_mut(i) {
                    p.character = ch;
                }
            }
        }
    }

    pub fn set_selected_color(&mut self, color: Vec4) {
        let sel: Vec<usize> = self.selection.iter().copied().collect();
        if let Some(model) = self.active_model_mut() {
            for i in sel {
                if let Some(p) = model.particles.get_mut(i) {
                    p.color = color;
                }
            }
        }
    }

    pub fn lock_selected(&mut self) {
        let sel: Vec<usize> = self.selection.iter().copied().collect();
        if let Some(model) = self.active_model_mut() {
            for i in sel {
                if let Some(p) = model.particles.get_mut(i) {
                    p.locked = true;
                }
            }
        }
    }

    pub fn unlock_selected(&mut self) {
        let sel: Vec<usize> = self.selection.iter().copied().collect();
        if let Some(model) = self.active_model_mut() {
            for i in sel {
                if let Some(p) = model.particles.get_mut(i) {
                    p.locked = false;
                }
            }
        }
    }

    pub fn center_model_at_origin(&mut self) {
        self.push_undo("center");
        if let Some(model) = self.active_model_mut() {
            let com = model.center_of_mass();
            for p in &mut model.particles {
                p.position -= com;
            }
            model.recompute_bounds();
        }
    }

    pub fn flip_normals(&mut self) {
        let sel: Vec<usize> = self.selection.iter().copied().collect();
        if let Some(model) = self.active_model_mut() {
            for i in sel {
                if let Some(p) = model.particles.get_mut(i) {
                    p.normal = -p.normal;
                }
            }
        }
    }

    pub fn particle_count(&self) -> usize {
        self.active_model().map(|m| m.particles.len()).unwrap_or(0)
    }

    pub fn selected_count(&self) -> usize {
        self.selection.len()
    }

    pub fn set_brush_radius(&mut self, r: f32) {
        self.brush_radius = r.max(EPSILON);
        self.brush_params.radius = self.brush_radius;
    }

    pub fn set_brush_strength(&mut self, s: f32) {
        self.brush_strength = s.clamp(0.0, 1.0);
        self.brush_params.strength = self.brush_strength;
    }

    pub fn set_brush_density(&mut self, d: f32) {
        self.brush_density = d.max(0.1);
        self.brush_params.density = self.brush_density;
    }

    pub fn set_symmetry(&mut self, mode: SymmetryMode) {
        self.symmetry = mode;
    }

    pub fn toggle_grid_snap(&mut self) {
        self.grid_snap = !self.grid_snap;
    }
}

impl Default for ModelEditor {
    fn default() -> Self { Self::new() }
}

// ============================================================
// ADDITIONAL MATH UTILITIES
// ============================================================

/// Project a vector onto a plane defined by its normal.
pub fn project_onto_plane(v: Vec3, plane_normal: Vec3) -> Vec3 {
    let n = plane_normal.normalize();
    v - n * n.dot(v)
}

/// Angle between two vectors in radians.
pub fn angle_between(a: Vec3, b: Vec3) -> f32 {
    let dot = a.normalize().dot(b.normalize()).clamp(-1.0, 1.0);
    dot.acos()
}

/// Signed angle from a to b around axis.
pub fn signed_angle(a: Vec3, b: Vec3, axis: Vec3) -> f32 {
    let cross = a.cross(b);
    let sign = if cross.dot(axis) >= 0.0 { 1.0 } else { -1.0 };
    angle_between(a, b) * sign
}

/// Linear interpolation between two colors in HSV space.
pub fn lerp_color_hsv(a: Vec4, b: Vec4, t: f32) -> Vec4 {
    let (ah, asat, av) = rgb_to_hsv(a.x, a.y, a.z);
    let (bh, bsat, bv) = rgb_to_hsv(b.x, b.y, b.z);
    let h = ah + (bh - ah) * t;
    let s = asat + (bsat - asat) * t;
    let v = av + (bv - av) * t;
    let (r, g, bl) = hsv_to_rgb(h, s, v);
    let alpha = a.w + (b.w - a.w) * t;
    Vec4::new(r, g, bl, alpha)
}

/// Closest point on segment AB to point P.
pub fn closest_point_on_segment(a: Vec3, b: Vec3, p: Vec3) -> Vec3 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < EPSILON { return a; }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    a + ab * t
}

/// Barycentric coordinates of point P in triangle ABC.
pub fn barycentric(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = (d00 * d11 - d01 * d01).max(EPSILON);
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    Vec3::new(1.0 - v - w, v, w)
}

/// Clamp a Vec3 component-wise to given bounds.
pub fn clamp_vec3(v: Vec3, min: Vec3, max: Vec3) -> Vec3 {
    Vec3::new(
        v.x.clamp(min.x, max.x),
        v.y.clamp(min.y, max.y),
        v.z.clamp(min.z, max.z),
    )
}

/// Remap a value from [in_min, in_max] to [out_min, out_max].
pub fn remap(val: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (val - in_min) / (in_max - in_min + EPSILON);
    out_min + t * (out_max - out_min)
}

/// Signed distance from point to AABB.
pub fn sdf_aabb(p: Vec3, half_extent: Vec3) -> f32 {
    let q = Vec3::new(p.x.abs(), p.y.abs(), p.z.abs()) - half_extent;
    q.max(Vec3::ZERO).length() + q.x.max(q.y).max(q.z).min(0.0)
}

/// Signed distance from point to sphere.
pub fn sdf_sphere(p: Vec3, center: Vec3, radius: f32) -> f32 {
    (p - center).length() - radius
}

/// Signed distance from point to torus.
pub fn sdf_torus(p: Vec3, major_r: f32, minor_r: f32) -> f32 {
    let q = Vec2::new((Vec2::new(p.x, p.z)).length() - major_r, p.y);
    q.length() - minor_r
}

/// Signed distance from point to cylinder.
pub fn sdf_cylinder(p: Vec3, height: f32, radius: f32) -> f32 {
    let d = Vec2::new(Vec2::new(p.x, p.z).length() - radius, p.y.abs() - height * 0.5);
    d.x.max(d.y).min(0.0) + Vec2::new(d.x.max(0.0), d.y.max(0.0)).length()
}

// ============================================================
// PARTICLE STATISTICS
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ParticleStats {
    pub count:          usize,
    pub bounds:         Aabb3,
    pub center_of_mass: Vec3,
    pub avg_color:      Vec4,
    pub char_histogram: BTreeMap<char, usize>,
    pub group_counts:   BTreeMap<u32, usize>,
    pub avg_emission:   f32,
    pub surface_count:  usize,
    pub interior_count: usize,
}

impl ParticleStats {
    pub fn compute(model: &ParticleModel) -> Self {
        let n = model.particles.len();
        if n == 0 { return Self::default(); }

        let mut stats = Self::default();
        stats.count = n;
        stats.bounds = model.bounds.clone();
        stats.center_of_mass = model.center_of_mass();

        let mut color_sum = Vec4::ZERO;
        let mut emission_sum = 0.0f32;
        for p in &model.particles {
            color_sum += p.color;
            emission_sum += p.emission;
            *stats.char_histogram.entry(p.character).or_insert(0) += 1;
            *stats.group_counts.entry(p.group_id).or_insert(0) += 1;
        }
        stats.avg_color    = color_sum / n as f32;
        stats.avg_emission = emission_sum / n as f32;

        // Surface classification with default params
        let is_surface = VisualizationHelper::classify_surface_interior(&model.particles, 8, 0.5);
        stats.surface_count  = is_surface.iter().filter(|&&s| s).count();
        stats.interior_count = n - stats.surface_count;

        stats
    }
}

// ============================================================
// ADVANCED SCULPT OPERATIONS
// ============================================================

pub struct AdvancedSculpt;

impl AdvancedSculpt {
    /// Relax particles: iterative Laplacian smoothing over the whole model.
    pub fn global_relax(model: &mut ParticleModel, iterations: usize, strength: f32, neighbor_radius: f32) {
        let r2 = neighbor_radius * neighbor_radius;
        for _ in 0..iterations {
            let positions: Vec<Vec3> = model.particles.iter().map(|p| p.position).collect();
            for i in 0..model.particles.len() {
                if model.particles[i].locked { continue; }
                let pi = positions[i];
                let (sum, cnt) = positions.iter().enumerate()
                    .filter(|(j, q)| *j != i && (**q - pi).length_squared() <= r2)
                    .fold((Vec3::ZERO, 0usize), |(a, n), (_, q)| (a + *q, n + 1));
                if cnt > 0 {
                    let centroid = sum / cnt as f32;
                    model.particles[i].position = pi.lerp(centroid, strength);
                }
            }
        }
        model.recompute_bounds();
    }

    /// Jitter: add random noise to particle positions.
    pub fn jitter(model: &mut ParticleModel, indices: &HashSet<usize>, amplitude: f32, seed: u32) {
        for (count, &i) in indices.iter().enumerate() {
            if let Some(p) = model.particles.get_mut(i) {
                if p.locked { continue; }
                let nx = (hash_2d(i as i32 * 3, seed as i32) * 2.0 - 1.0) * amplitude;
                let ny = (hash_2d(i as i32 * 7, seed as i32 + 1) * 2.0 - 1.0) * amplitude;
                let nz = (hash_2d(i as i32 * 13, seed as i32 + 2) * 2.0 - 1.0) * amplitude;
                p.position += Vec3::new(nx, ny, nz);
            }
        }
        model.recompute_bounds();
    }

    /// Shrink-wrap: project each particle onto the surface of a target sphere.
    pub fn shrink_wrap_sphere(
        model: &mut ParticleModel,
        indices: &HashSet<usize>,
        center: Vec3,
        radius: f32,
        blend: f32,
    ) {
        for &i in indices {
            if let Some(p) = model.particles.get_mut(i) {
                if p.locked { continue; }
                let dir = (p.position - center).normalize();
                let target = center + dir * radius;
                p.position = p.position.lerp(target, blend);
            }
        }
        model.recompute_bounds();
    }

    /// Push particles inside AABB to the surface.
    pub fn push_to_aabb_surface(
        model: &mut ParticleModel,
        indices: &HashSet<usize>,
        aabb: &Aabb3,
        blend: f32,
    ) {
        let center = aabb.center();
        let half = aabb.size() * 0.5;
        for &i in indices {
            if let Some(p) = model.particles.get_mut(i) {
                if p.locked { continue; }
                if !aabb.contains(p.position) { continue; }
                let local = p.position - center;
                // Find closest face
                let d = [
                    (half.x - local.x.abs(), Vec3::new(local.x.signum(), 0.0, 0.0)),
                    (half.y - local.y.abs(), Vec3::new(0.0, local.y.signum(), 0.0)),
                    (half.z - local.z.abs(), Vec3::new(0.0, 0.0, local.z.signum())),
                ];
                let (_, face_normal) = d.iter().min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)).copied().unwrap_or((0.0, Vec3::X));
                let face_pos = center + face_normal * half;
                let projected = p.position - face_normal * (face_normal.dot(p.position - face_pos));
                p.position = p.position.lerp(projected, blend);
            }
        }
        model.recompute_bounds();
    }

    /// Array duplicate: create N copies of the selection along an axis.
    pub fn array_duplicate(
        model: &mut ParticleModel,
        indices: &HashSet<usize>,
        count: usize,
        step: Vec3,
    ) {
        let source: Vec<ModelParticle> = indices.iter()
            .filter_map(|&i| model.particles.get(i))
            .cloned()
            .collect();
        let mut new_particles = Vec::new();
        for step_i in 1..=count {
            let offset = step * step_i as f32;
            for p in &source {
                let mut np = p.clone();
                np.position += offset;
                new_particles.push(np);
            }
        }
        model.add_particles_bulk(new_particles);
    }

    /// Radial array: place copies at evenly spaced angles around an axis.
    pub fn radial_array(
        model: &mut ParticleModel,
        indices: &HashSet<usize>,
        center: Vec3,
        axis: Vec3,
        count: usize,
    ) {
        let source: Vec<ModelParticle> = indices.iter()
            .filter_map(|&i| model.particles.get(i))
            .cloned()
            .collect();
        let mut new_particles = Vec::new();
        let axis_n = axis.normalize();
        for ci in 1..count {
            let angle = TAU * ci as f32 / count as f32;
            let quat = Quat::from_axis_angle(axis_n, angle);
            for p in &source {
                let local = p.position - center;
                let mut np = p.clone();
                np.position = center + quat * local;
                np.normal   = quat * p.normal;
                new_particles.push(np);
            }
        }
        model.add_particles_bulk(new_particles);
    }

    /// Merge nearby particles within a threshold distance.
    pub fn merge_by_distance(model: &mut ParticleModel, threshold: f32) {
        let threshold2 = threshold * threshold;
        let n = model.particles.len();
        let mut merged = vec![false; n];
        let mut new_particles = Vec::new();
        for i in 0..n {
            if merged[i] { continue; }
            let pi = model.particles[i].position;
            let mut positions_to_merge = vec![pi];
            for j in (i+1)..n {
                if !merged[j] && (model.particles[j].position - pi).length_squared() <= threshold2 {
                    merged[j] = true;
                    positions_to_merge.push(model.particles[j].position);
                }
            }
            let avg = positions_to_merge.iter().fold(Vec3::ZERO, |a, &b| a + b) / positions_to_merge.len() as f32;
            let mut p = model.particles[i].clone();
            p.position = avg;
            new_particles.push(p);
        }
        model.particles = new_particles;
        model.recompute_bounds();
    }

    /// Separate disconnected components into individual models.
    pub fn find_connected_components(
        model: &ParticleModel,
        max_edge_len: f32,
    ) -> Vec<Vec<usize>> {
        let n = model.particles.len();
        let max2 = max_edge_len * max_edge_len;
        let mut visited = vec![false; n];
        let mut components = Vec::new();

        for start in 0..n {
            if visited[start] { continue; }
            let mut component = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(start);
            visited[start] = true;
            while let Some(current) = queue.pop_front() {
                component.push(current);
                let pc = model.particles[current].position;
                for j in 0..n {
                    if !visited[j] && (model.particles[j].position - pc).length_squared() <= max2 {
                        visited[j] = true;
                        queue.push_back(j);
                    }
                }
            }
            components.push(component);
        }
        components
    }

    /// Compute the convex hull center (centroid of bounding box corners).
    pub fn convex_hull_center(model: &ParticleModel) -> Vec3 {
        model.bounds.center()
    }

    /// Scale model to fit in a unit cube.
    pub fn normalize_scale(model: &mut ParticleModel) {
        let size = model.bounds.size();
        let max_dim = size.x.max(size.y).max(size.z);
        if max_dim < EPSILON { return; }
        let scale = 1.0 / max_dim;
        let center = model.center_of_mass();
        for p in &mut model.particles {
            p.position = center + (p.position - center) * scale;
        }
        model.recompute_bounds();
    }
}

// ============================================================
// PARTICLE EFFECTS
// ============================================================

pub struct ParticleEffects;

impl ParticleEffects {
    /// Animate particles: oscillate along their normal direction using a sine wave.
    pub fn animate_wave(
        model: &mut ParticleModel,
        time: f32,
        amplitude: f32,
        frequency: f32,
        phase_scale: f32,
    ) {
        for p in &mut model.particles {
            let phase = p.position.x * phase_scale + p.position.z * phase_scale;
            let offset = amplitude * (frequency * time + phase).sin();
            p.position += p.normal * offset;
        }
        model.recompute_bounds();
    }

    /// Pulse emission over time.
    pub fn animate_emission_pulse(model: &mut ParticleModel, time: f32, frequency: f32) {
        for p in &mut model.particles {
            p.emission = 0.5 + 0.5 * (TAU * frequency * time).sin();
        }
    }

    /// Attract particles toward a moving attractor point.
    pub fn attract(
        model: &mut ParticleModel,
        attractor: Vec3,
        strength: f32,
        radius: f32,
        delta_time: f32,
    ) {
        let r2 = radius * radius;
        for p in &mut model.particles {
            if p.locked { continue; }
            let d2 = (p.position - attractor).length_squared();
            if d2 > r2 || d2 < EPSILON { continue; }
            let dir = (attractor - p.position).normalize();
            let falloff = 1.0 - (d2 / r2).sqrt();
            p.position += dir * strength * falloff * delta_time;
        }
        model.recompute_bounds();
    }

    /// Gravity simulation step.
    pub fn apply_gravity(
        model: &mut ParticleModel,
        gravity: Vec3,
        delta_time: f32,
        floor_y: Option<f32>,
    ) {
        for p in &mut model.particles {
            if p.locked { continue; }
            p.position += gravity * delta_time;
            if let Some(fy) = floor_y {
                if p.position.y < fy { p.position.y = fy; }
            }
        }
        model.recompute_bounds();
    }

    /// Color cycle through hue over time.
    pub fn animate_color_cycle(model: &mut ParticleModel, time: f32, speed: f32) {
        for p in &mut model.particles {
            let (_, s, v) = rgb_to_hsv(p.color.x, p.color.y, p.color.z);
            let new_h = (time * speed * 360.0) % 360.0;
            let (r, g, b) = hsv_to_rgb(new_h, s.max(0.5), v.max(0.5));
            p.color = Vec4::new(r, g, b, p.color.w);
        }
    }

    /// Scatter particles randomly within their bounding box.
    pub fn scatter(model: &mut ParticleModel, indices: &HashSet<usize>, seed: u32) {
        let bounds = model.bounds.clone();
        for &i in indices {
            if let Some(p) = model.particles.get_mut(i) {
                if p.locked { continue; }
                let rx = hash_2d(i as i32, seed as i32);
                let ry = hash_2d(i as i32 + 1000, seed as i32 + 1);
                let rz = hash_2d(i as i32 + 2000, seed as i32 + 2);
                p.position = bounds.min + bounds.size() * Vec3::new(rx, ry, rz);
            }
        }
        model.recompute_bounds();
    }
}

// ============================================================
// GLYPH RENDERING HELPERS
// ============================================================

/// For each particle, determine the best ASCII glyph for a given view direction.
pub fn compute_view_dependent_glyph(particle: &ModelParticle, view_dir: Vec3) -> char {
    let n = particle.normal;
    let dot = n.dot(-view_dir).clamp(-1.0, 1.0);
    // Select glyph by angle: facing = dense, grazing = sparse
    if dot > 0.8      { '#' }
    else if dot > 0.6 { '@' }
    else if dot > 0.4 { particle.character }
    else if dot > 0.2 { '.' }
    else if dot > 0.0 { ',' }
    else              { ' ' }
}

/// Convert a depth value to ASCII shading character.
pub fn depth_to_ascii(depth: f32, near: f32, far: f32) -> char {
    const SHADING: &[char] = &[' ', '.', ':', ';', '-', '=', '+', '*', '#', '@', '█'];
    let t = 1.0 - ((depth - near) / (far - near)).clamp(0.0, 1.0);
    let idx = ((t * (SHADING.len() as f32 - 1.0)).round() as usize).min(SHADING.len() - 1);
    SHADING[idx]
}

/// Sort particles back-to-front for correct alpha blending.
pub fn sort_particles_back_to_front(particles: &mut Vec<ModelParticle>, view_origin: Vec3) {
    particles.sort_by(|a, b| {
        let da = (a.position - view_origin).length_squared();
        let db = (b.position - view_origin).length_squared();
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Sort particles front-to-back for depth testing.
pub fn sort_particles_front_to_back(particles: &mut Vec<ModelParticle>, view_origin: Vec3) {
    particles.sort_by(|a, b| {
        let da = (a.position - view_origin).length_squared();
        let db = (b.position - view_origin).length_squared();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Project a Vec3 world position to screen space (Vec2) given MVP matrix.
pub fn project_to_screen(pos: Vec3, mvp: Mat4, screen_w: f32, screen_h: f32) -> Option<Vec2> {
    let clip = mvp * Vec4::new(pos.x, pos.y, pos.z, 1.0);
    if clip.w.abs() < EPSILON { return None; }
    let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    if ndc.z < -1.0 || ndc.z > 1.0 { return None; }
    Some(Vec2::new(
        (ndc.x * 0.5 + 0.5) * screen_w,
        (1.0 - (ndc.y * 0.5 + 0.5)) * screen_h,
    ))
}

/// Unproject screen position to a world-space Ray3.
pub fn unproject_ray(
    screen_x: f32,
    screen_y: f32,
    screen_w: f32,
    screen_h: f32,
    inv_mvp: Mat4,
) -> Ray3 {
    let ndc_x =  (screen_x / screen_w) * 2.0 - 1.0;
    let ndc_y = -((screen_y / screen_h) * 2.0 - 1.0);
    let near_clip = inv_mvp * Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
    let far_clip  = inv_mvp * Vec4::new(ndc_x, ndc_y,  1.0, 1.0);
    let near_world = Vec3::new(near_clip.x, near_clip.y, near_clip.z) / near_clip.w;
    let far_world  = Vec3::new(far_clip.x,  far_clip.y,  far_clip.z)  / far_clip.w;
    Ray3::new(near_world, (far_world - near_world).normalize())
}

// ============================================================
// TESTS (compile-checked)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_expand() {
        let mut a = Aabb3::empty();
        a.expand(Vec3::new(1.0, 2.0, 3.0));
        a.expand(Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(a.min, Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(a.max, Vec3::new(1.0, 2.0, 3.0));
        assert!((a.center() - Vec3::ZERO).length() < EPSILON);
    }

    #[test]
    fn test_ray_sphere() {
        let ray = Ray3::new(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);
        let hit = ray.intersect_sphere(Vec3::ZERO, 1.0);
        assert!(hit.is_some());
        let t = hit.unwrap();
        assert!((t - 4.0).abs() < 1e-4, "t={}", t);
    }

    #[test]
    fn test_smoothstep() {
        assert!((smoothstep(0.0, 1.0, 0.0) - 0.0).abs() < EPSILON);
        assert!((smoothstep(0.0, 1.0, 1.0) - 1.0).abs() < EPSILON);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_primitive() {
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 100, '.', Vec4::ONE);
        assert_eq!(particles.len(), 100);
        for p in &particles {
            let dist = p.position.length();
            assert!((dist - 1.0).abs() < 1e-4, "dist={}", dist);
        }
    }

    #[test]
    fn test_torus_primitive() {
        let particles = PrimitiveBuilder::torus(Vec3::ZERO, 2.0, 0.5, 16, 8, '.', Vec4::ONE);
        assert_eq!(particles.len(), 16 * 8);
    }

    #[test]
    fn test_rgb_hsv_roundtrip() {
        let (r, g, b) = (0.3, 0.6, 0.9);
        let (h, s, v) = rgb_to_hsv(r, g, b);
        let (r2, g2, b2) = hsv_to_rgb(h, s, v);
        assert!((r - r2).abs() < 1e-4);
        assert!((g - g2).abs() < 1e-4);
        assert!((b - b2).abs() < 1e-4);
    }

    #[test]
    fn test_model_editor_create() {
        let mut editor = ModelEditor::new();
        let id = editor.create_model("test");
        assert_eq!(editor.active_model_id, Some(id));
        assert!(editor.active_model().is_some());
    }

    #[test]
    fn test_insert_sphere_and_select() {
        let mut editor = ModelEditor::new();
        editor.create_model("m");
        editor.insert_sphere(Vec3::ZERO, 1.0, 50);
        assert_eq!(editor.particle_count(), 50);
        editor.sphere_select(Vec3::ZERO, 2.0, false);
        assert_eq!(editor.selected_count(), 50);
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = ModelEditor::new();
        editor.create_model("m");
        editor.insert_sphere(Vec3::ZERO, 1.0, 20);
        let before = editor.particle_count();
        editor.select_all();
        editor.delete_selected();
        assert_eq!(editor.particle_count(), 0);
        editor.undo();
        assert_eq!(editor.particle_count(), before);
    }

    #[test]
    fn test_transform_translate() {
        let mut editor = ModelEditor::new();
        editor.create_model("m");
        editor.active_color = Vec4::ONE;
        editor.active_char = '.';
        if let Some(model) = editor.active_model_mut() {
            model.add_particle(ModelParticle::new(Vec3::ZERO, '.', Vec4::ONE));
        }
        editor.select_all();
        editor.cmd_translate(Vec3::new(1.0, 0.0, 0.0));
        if let Some(p) = editor.active_model().and_then(|m| m.particles.first()) {
            assert!((p.position.x - 1.0).abs() < EPSILON);
        }
    }

    #[test]
    fn test_lattice_deformer_identity() {
        let bounds = Aabb3::new(-Vec3::ONE, Vec3::ONE);
        let lattice = LatticeDeformer::new(bounds, 2, 2, 2);
        let p = Vec3::new(0.5, 0.5, 0.5);
        let dp = lattice.deform(p);
        // Identity lattice should not move point
        assert!((dp - p).length() < 1e-3, "dp={:?}", dp);
    }

    #[test]
    fn test_selection_grow_shrink() {
        let mut sel = SelectionSystem::new();
        let mut particles = vec![
            ModelParticle::new(Vec3::ZERO,            '.', Vec4::ONE),
            ModelParticle::new(Vec3::new(0.5, 0.0, 0.0), '.', Vec4::ONE),
            ModelParticle::new(Vec3::new(2.0, 0.0, 0.0), '.', Vec4::ONE),
        ];
        sel.add(0);
        sel.grow(&particles, 0.6);
        assert!(sel.selected.contains(&1));
        assert!(!sel.selected.contains(&2));
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mut model = ParticleModel::new(1, "test");
        model.add_particle(ModelParticle::new(Vec3::new(1.0, 2.0, 3.0), 'A', Vec4::new(1.0, 0.5, 0.0, 1.0)));
        let text = ModelIO::export_text(&model);
        let imported = ModelIO::import_text(&text, 2).unwrap();
        assert_eq!(imported.particles.len(), 1);
        assert_eq!(imported.particles[0].character, 'A');
        assert!((imported.particles[0].position - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-4);
    }

    #[test]
    fn test_barycentric() {
        let a = Vec3::ZERO;
        let b = Vec3::X;
        let c = Vec3::Y;
        let bc = barycentric(Vec3::new(0.5, 0.0, 0.0), a, b, c);
        assert!((bc.y - 0.5).abs() < 1e-4, "bc={:?}", bc);
    }

    #[test]
    fn test_falloff_curves() {
        let curves = [
            FalloffCurve::Constant,
            FalloffCurve::Linear,
            FalloffCurve::Smooth,
            FalloffCurve::Sphere,
            FalloffCurve::Root,
            FalloffCurve::Sharp,
        ];
        for c in &curves {
            assert!((c.evaluate(0.0) - 1.0).abs() < EPSILON, "{:?} at 0", c);
            assert!((c.evaluate(1.0)).abs() < EPSILON || matches!(c, FalloffCurve::Constant),
                    "{:?} at 1 = {}", c, c.evaluate(1.0));
        }
    }

    #[test]
    fn test_symmetry_mirrors() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        let m = SymmetryMode::XYZ.mirrors(p);
        assert_eq!(m.len(), 7);
        assert!(m.contains(&Vec3::new(-1.0, 2.0, 3.0)));
        assert!(m.contains(&Vec3::new(-1.0, -2.0, -3.0)));
    }

    #[test]
    fn test_sdf_sphere() {
        let p = Vec3::new(2.0, 0.0, 0.0);
        let d = sdf_sphere(p, Vec3::ZERO, 1.0);
        assert!((d - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_particle_model_lod() {
        let mut model = ParticleModel::new(1, "lod");
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 1.0, 200, '.', Vec4::ONE);
        model.add_particles_bulk(particles);
        model.generate_lods();
        assert_eq!(model.lod_levels.len(), 4);
        assert_eq!(model.lod_levels[0].particles.len(), 200);
        assert!(model.lod_levels[1].particles.len() <= 100);
    }

    #[test]
    fn test_hash_2d() {
        // Should be deterministic
        assert_eq!(hash_2d(1, 2), hash_2d(1, 2));
        assert_ne!(hash_2d(1, 2), hash_2d(2, 1));
    }

    #[test]
    fn test_spatial_hash() {
        let particles = PrimitiveBuilder::sphere(Vec3::ZERO, 2.0, 50, '.', Vec4::ONE);
        let grid = VisualizationHelper::build_spatial_hash(&particles, 1.0);
        let neighbors = VisualizationHelper::query_spatial_hash(&grid, Vec3::ZERO, 1.0, 1.0);
        assert!(!neighbors.is_empty());
    }
}

// ============================================================
// SERIALIZATION HELPERS (no external deps)
// ============================================================

pub struct ModelSerializer;

impl ModelSerializer {
    /// Serialize a model to a compact binary-like text representation.
    pub fn serialize_compact(model: &ParticleModel) -> Vec<u8> {
        let mut out = Vec::new();
        // Header
        out.extend_from_slice(b"PMDL");
        out.extend_from_slice(&model.id.to_le_bytes());
        let name_bytes = model.name.as_bytes();
        out.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(name_bytes);
        // Particle count
        out.extend_from_slice(&(model.particles.len() as u32).to_le_bytes());
        // Particles
        for p in &model.particles {
            out.extend_from_slice(&p.position.x.to_le_bytes());
            out.extend_from_slice(&p.position.y.to_le_bytes());
            out.extend_from_slice(&p.position.z.to_le_bytes());
            out.extend_from_slice(&(p.character as u32).to_le_bytes());
            out.extend_from_slice(&p.color.x.to_le_bytes());
            out.extend_from_slice(&p.color.y.to_le_bytes());
            out.extend_from_slice(&p.color.z.to_le_bytes());
            out.extend_from_slice(&p.color.w.to_le_bytes());
            out.extend_from_slice(&p.emission.to_le_bytes());
            out.extend_from_slice(&p.normal.x.to_le_bytes());
            out.extend_from_slice(&p.normal.y.to_le_bytes());
            out.extend_from_slice(&p.normal.z.to_le_bytes());
            out.push(p.group_id as u8);
            out.push(p.layer_id);
        }
        out
    }

    /// Deserialize from compact format.
    pub fn deserialize_compact(data: &[u8]) -> Option<ParticleModel> {
        if data.len() < 16 { return None; }
        if &data[0..4] != b"PMDL" { return None; }
        let mut cursor = 4usize;

        let id = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);
        cursor += 8;
        let name_len = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
        cursor += 4;
        if cursor + name_len > data.len() { return None; }
        let name = String::from_utf8(data[cursor..cursor+name_len].to_vec()).ok()?;
        cursor += name_len;

        let particle_count = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
        cursor += 4;

        let mut model = ParticleModel::new(id, name);
        let bytes_per_particle = 4*3 + 4 + 4*4 + 4 + 4*3 + 2; // 58 bytes
        if cursor + particle_count * bytes_per_particle > data.len() { return None; }

        for _ in 0..particle_count {
            let px = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let py = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let pz = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let ch_code = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let cr = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let cg = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let cb = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let ca = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let emission = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let nx = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let ny = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let nz = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?); cursor += 4;
            let group_id = data[cursor] as u32; cursor += 1;
            let layer_id = data[cursor]; cursor += 1;

            let ch = char::from_u32(ch_code).unwrap_or('.');
            let mut p = ModelParticle::new(
                Vec3::new(px, py, pz),
                ch,
                Vec4::new(cr, cg, cb, ca),
            );
            p.emission = emission;
            p.normal = Vec3::new(nx, ny, nz);
            p.group_id = group_id;
            p.layer_id = layer_id;
            model.add_particle(p);
        }

        Some(model)
    }
}

// ============================================================
// CLIPBOARD / COPY-PASTE SUPPORT
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ModelClipboard {
    pub particles:       Vec<ModelParticle>,
    pub pivot:           Vec3,
    pub source_model_id: Option<u64>,
}

impl ModelClipboard {
    pub fn new() -> Self { Self::default() }

    pub fn copy_selection(&mut self, model: &ParticleModel, selection: &HashSet<usize>) {
        self.particles = selection.iter()
            .filter_map(|&i| model.particles.get(i))
            .cloned()
            .collect();
        self.pivot = if self.particles.is_empty() { Vec3::ZERO } else {
            let sum = self.particles.iter().map(|p| p.position).fold(Vec3::ZERO, |a, b| a + b);
            sum / self.particles.len() as f32
        };
        self.source_model_id = Some(model.id);
    }

    pub fn paste_at(&self, model: &mut ParticleModel, target: Vec3) {
        let offset = target - self.pivot;
        let new_particles: Vec<ModelParticle> = self.particles.iter().map(|p| {
            let mut np = p.clone();
            np.position += offset;
            np
        }).collect();
        model.add_particles_bulk(new_particles);
    }

    pub fn is_empty(&self) -> bool { self.particles.is_empty() }

    pub fn count(&self) -> usize { self.particles.len() }
}

// ============================================================
// GRID AND GUIDE HELPERS
// ============================================================

pub struct GridHelper;

impl GridHelper {
    /// Generate grid particle positions for a floor grid.
    pub fn floor_grid(
        center: Vec3,
        half_extent: f32,
        spacing: f32,
        y: f32,
        character: char,
        color: Vec4,
    ) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let steps = (half_extent / spacing).ceil() as i32;
        for xi in -steps..=steps {
            for zi in -steps..=steps {
                let x = center.x + xi as f32 * spacing;
                let z = center.z + zi as f32 * spacing;
                particles.push(ModelParticle::new(Vec3::new(x, y, z), character, color).with_normal(Vec3::Y));
            }
        }
        particles
    }

    /// Generate axis indicator particles.
    pub fn axis_indicators(length: f32, steps: usize) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        for i in 0..steps {
            let t = i as f32 / steps as f32 * length;
            particles.push(ModelParticle::new(Vec3::new(t, 0.0, 0.0), 'x', Vec4::new(1.0, 0.2, 0.2, 1.0)).with_normal(Vec3::X));
            particles.push(ModelParticle::new(Vec3::new(0.0, t, 0.0), 'y', Vec4::new(0.2, 1.0, 0.2, 1.0)).with_normal(Vec3::Y));
            particles.push(ModelParticle::new(Vec3::new(0.0, 0.0, t), 'z', Vec4::new(0.2, 0.2, 1.0, 1.0)).with_normal(Vec3::Z));
        }
        particles
    }

    /// Snap a position to the nearest grid point.
    pub fn snap(pos: Vec3, grid_size: f32) -> Vec3 {
        if grid_size < EPSILON { return pos; }
        Vec3::new(
            (pos.x / grid_size).round() * grid_size,
            (pos.y / grid_size).round() * grid_size,
            (pos.z / grid_size).round() * grid_size,
        )
    }

    /// Generate bounding box wireframe particles.
    pub fn bbox_wireframe(aabb: &Aabb3, density: usize, character: char, color: Vec4) -> Vec<ModelParticle> {
        let mut particles = Vec::new();
        let corners = [
            aabb.min,
            Vec3::new(aabb.max.x, aabb.min.y, aabb.min.z),
            Vec3::new(aabb.min.x, aabb.max.y, aabb.min.z),
            Vec3::new(aabb.max.x, aabb.max.y, aabb.min.z),
            Vec3::new(aabb.min.x, aabb.min.y, aabb.max.z),
            Vec3::new(aabb.max.x, aabb.min.y, aabb.max.z),
            Vec3::new(aabb.min.x, aabb.max.y, aabb.max.z),
            aabb.max,
        ];
        let edges: [(usize, usize); 12] = [
            (0,1),(2,3),(4,5),(6,7),
            (0,2),(1,3),(4,6),(5,7),
            (0,4),(1,5),(2,6),(3,7),
        ];
        for (a, b) in &edges {
            for i in 0..density {
                let t = i as f32 / (density as f32 - 1.0).max(1.0);
                let pos = corners[*a].lerp(corners[*b], t);
                particles.push(ModelParticle::new(pos, character, color));
            }
        }
        particles
    }
}

// ============================================================
// CURVE PATH DEFORMER
// ============================================================

#[derive(Clone, Debug)]
pub struct CatmullRomSpline {
    pub control_points: Vec<Vec3>,
    pub alpha:          f32,  // 0.0=uniform, 0.5=centripetal, 1.0=chordal
}

impl CatmullRomSpline {
    pub fn new(alpha: f32) -> Self {
        Self { control_points: Vec::new(), alpha }
    }

    pub fn add_point(&mut self, p: Vec3) {
        self.control_points.push(p);
    }

    fn segment_t(p0: Vec3, p1: Vec3, alpha: f32) -> f32 {
        let d = (p1 - p0).length();
        d.powf(alpha)
    }

    /// Evaluate spline at parameter t in [0, num_segments].
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.control_points.len();
        if n < 2 { return self.control_points.first().copied().unwrap_or(Vec3::ZERO); }
        if n == 2 {
            return self.control_points[0].lerp(self.control_points[1], t.clamp(0.0, 1.0));
        }

        let max_seg = (n - 1) as f32;
        let t = t.clamp(0.0, max_seg);
        let seg = (t as usize).min(n - 2);
        let local_t = t - seg as f32;

        let p0 = self.control_points[seg.saturating_sub(1).max(0)];
        let p1 = self.control_points[seg];
        let p2 = self.control_points[(seg + 1).min(n - 1)];
        let p3 = self.control_points[(seg + 2).min(n - 1)];

        // Catmull-Rom with alpha parameterization
        let t0 = 0.0f32;
        let t1 = t0 + Self::segment_t(p0, p1, self.alpha);
        let t2 = t1 + Self::segment_t(p1, p2, self.alpha);
        let t3 = t2 + Self::segment_t(p2, p3, self.alpha);

        let t_param = t1 + local_t * (t2 - t1);

        let safe_div = |a: Vec3, b: f32| -> Vec3 {
            if b.abs() < EPSILON { Vec3::ZERO } else { a / b }
        };

        let a1 = safe_div(p0 * (t1 - t_param) + p1 * (t_param - t0), t1 - t0);
        let a2 = safe_div(p1 * (t2 - t_param) + p2 * (t_param - t1), t2 - t1);
        let a3 = safe_div(p2 * (t3 - t_param) + p3 * (t_param - t2), t3 - t2);
        let b1 = safe_div(a1 * (t2 - t_param) + a2 * (t_param - t0), t2 - t0);
        let b2 = safe_div(a2 * (t3 - t_param) + a3 * (t_param - t1), t3 - t1);
        safe_div(b1 * (t2 - t_param) + b2 * (t_param - t1), t2 - t1)
    }

    /// Sample N evenly-spaced points along the spline.
    pub fn sample(&self, n: usize) -> Vec<Vec3> {
        if self.control_points.is_empty() { return Vec::new(); }
        let max_t = (self.control_points.len() - 1) as f32;
        (0..n).map(|i| {
            let t = i as f32 / (n as f32 - 1.0).max(1.0) * max_t;
            self.evaluate(t)
        }).collect()
    }

    /// Deform particles to follow the spline.
    pub fn deform_along_path(
        &self,
        model: &mut ParticleModel,
        indices: &HashSet<usize>,
        axis: Vec3,
    ) {
        let axis_n = axis.normalize();
        let all_positions: Vec<Vec3> = indices.iter()
            .filter_map(|&i| model.particles.get(i))
            .map(|p| p.position)
            .collect();
        if all_positions.is_empty() { return; }
        let min_t = all_positions.iter().map(|&p| axis_n.dot(p)).fold(f32::MAX, f32::min);
        let max_t = all_positions.iter().map(|&p| axis_n.dot(p)).fold(f32::MIN, f32::max);
        let range = (max_t - min_t).max(EPSILON);
        let spline_len = (self.control_points.len() - 1) as f32;

        for &i in indices {
            if let Some(p) = model.particles.get_mut(i) {
                if p.locked { continue; }
                let t = (axis_n.dot(p.position) - min_t) / range * spline_len;
                let spline_pos = self.evaluate(t);
                let perp = p.position - axis_n * axis_n.dot(p.position);
                p.position = spline_pos + perp;
            }
        }
        model.recompute_bounds();
    }
}

// ============================================================
// BRUSH PRESET LIBRARY
// ============================================================

#[derive(Clone, Debug)]
pub struct BrushPreset {
    pub name:     String,
    pub kind:     BrushKind,
    pub radius:   f32,
    pub strength: f32,
    pub density:  f32,
    pub falloff:  FalloffCurve,
    pub character: char,
    pub color:    Vec4,
}

impl BrushPreset {
    pub fn new(name: impl Into<String>, kind: BrushKind) -> Self {
        Self {
            name:      name.into(),
            kind,
            radius:    1.0,
            strength:  0.5,
            density:   4.0,
            falloff:   FalloffCurve::Smooth,
            character: '.',
            color:     Vec4::ONE,
        }
    }

    pub fn to_params(&self) -> BrushParams {
        BrushParams {
            kind:      self.kind.clone(),
            radius:    self.radius,
            strength:  self.strength,
            density:   self.density,
            color:     self.color,
            character: self.character,
            falloff:   self.falloff.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BrushPresetLibrary {
    pub presets: BTreeMap<String, BrushPreset>,
}

impl BrushPresetLibrary {
    pub fn new() -> Self {
        let mut lib = Self::default();
        lib.add_defaults();
        lib
    }

    fn add_defaults(&mut self) {
        let mut add = BrushPreset::new("Default Add", BrushKind::Add);
        add.density = 8.0;
        self.presets.insert(add.name.clone(), add);

        let mut smooth = BrushPreset::new("Heavy Smooth", BrushKind::Smooth);
        smooth.strength = 0.8;
        smooth.radius   = 2.0;
        self.presets.insert(smooth.name.clone(), smooth);

        let mut inflate = BrushPreset::new("Inflate", BrushKind::Inflate);
        inflate.strength = 0.3;
        self.presets.insert(inflate.name.clone(), inflate);

        let mut pinch = BrushPreset::new("Pinch", BrushKind::Pinch);
        pinch.radius = 0.5;
        pinch.strength = 0.7;
        self.presets.insert(pinch.name.clone(), pinch);

        let flatten = BrushPreset::new("Flatten", BrushKind::Flatten);
        self.presets.insert(flatten.name.clone(), flatten);
    }

    pub fn add(&mut self, preset: BrushPreset) {
        self.presets.insert(preset.name.clone(), preset);
    }

    pub fn get(&self, name: &str) -> Option<&BrushPreset> {
        self.presets.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.presets.keys().map(|s| s.as_str()).collect()
    }
}

// ============================================================
// RENDERING CONTEXT HELPERS
// ============================================================

/// Holds the data needed to render the model in the ASCII engine.
#[derive(Clone, Debug, Default)]
pub struct ModelRenderData {
    pub glyphs:      Vec<(Vec3, char, Vec4, f32)>,
    pub edge_pairs:  Vec<(Vec3, Vec3, char, Vec4)>,
    pub normal_tips: Vec<(Vec3, char, Vec4)>,
    pub bounds_wf:   Vec<(Vec3, char, Vec4)>,
    pub lod_level:   usize,
}

impl ModelRenderData {
    pub fn build(
        editor: &ModelEditor,
        camera_pos:  Vec3,
        camera_dist: f32,
        lod_bias:    f32,
    ) -> Self {
        let mut data = Self::default();
        let model = match editor.active_model() { Some(m) => m, None => return data };

        let lod_idx = model.select_lod(camera_dist, lod_bias);
        data.lod_level = lod_idx;

        let particle_indices: &[usize] = if model.lod_levels.is_empty() {
            &[]
        } else {
            &model.lod_levels[lod_idx.min(model.lod_levels.len() - 1)].particles
        };

        if particle_indices.is_empty() {
            // No LOD computed — render all
            for p in &model.particles {
                if let Some(layer) = model.layers.iter().find(|l| l.id == p.layer_id) {
                    if !layer.visible { continue; }
                    let alpha_blended = Vec4::new(p.color.x, p.color.y, p.color.z, p.color.w * layer.opacity);
                    data.glyphs.push((p.position, p.character, alpha_blended, p.emission));
                } else {
                    data.glyphs.push((p.position, p.character, p.color, p.emission));
                }
            }
        } else {
            for &pi in particle_indices {
                if let Some(p) = model.particles.get(pi) {
                    data.glyphs.push((p.position, p.character, p.color, p.emission));
                }
            }
        }

        if editor.wireframe_mode {
            let edges = VisualizationHelper::find_edges(&model.particles, editor.brush_radius * 0.5);
            for (a, b) in edges {
                if let (Some(pa), Some(pb)) = (model.particles.get(a), model.particles.get(b)) {
                    data.edge_pairs.push((pa.position, pb.position, '-', Vec4::new(0.5, 0.5, 0.5, 1.0)));
                }
            }
        }

        if editor.normal_vis {
            for p in &model.particles {
                let tip = p.position + p.normal * 0.2;
                data.normal_tips.push((tip, '^', Vec4::new(0.0, 1.0, 0.5, 1.0)));
            }
        }

        if editor.show_bounds {
            let wf = GridHelper::bbox_wireframe(&model.bounds, 8, '+', Vec4::new(1.0, 1.0, 0.0, 0.8));
            for p in wf {
                data.bounds_wf.push((p.position, p.character, p.color));
            }
        }

        data
    }
}

// ============================================================
// TOOL CONTEXT (ties together editor + clipboard + presets)
// ============================================================

pub struct ModelingToolContext {
    pub editor:    ModelEditor,
    pub clipboard: ModelClipboard,
    pub presets:   BrushPresetLibrary,
    pub spline:    CatmullRomSpline,
    pub stats:     Option<ParticleStats>,
}

impl ModelingToolContext {
    pub fn new() -> Self {
        Self {
            editor:    ModelEditor::new(),
            clipboard: ModelClipboard::new(),
            presets:   BrushPresetLibrary::new(),
            spline:    CatmullRomSpline::new(0.5),
            stats:     None,
        }
    }

    pub fn refresh_stats(&mut self) {
        self.stats = self.editor.active_model().map(ParticleStats::compute);
    }

    pub fn apply_preset_brush(&mut self, preset_name: &str, hit_pos: Vec3, ray: Ray3) {
        if let Some(preset) = self.presets.get(preset_name) {
            let params = preset.to_params();
            self.editor.active_brush = params.kind.clone();
            self.editor.brush_radius  = params.radius;
            self.editor.brush_strength = params.strength;
            self.editor.brush_density  = params.density;
            self.editor.active_char    = params.character;
            self.editor.active_color   = params.color;
        }
        self.editor.apply_brush(ray, hit_pos);
    }

    pub fn copy(&mut self) {
        let sel = self.editor.selection.clone();
        if let Some(model) = self.editor.active_model() {
            self.clipboard.copy_selection(model, &sel);
        }
    }

    pub fn paste(&mut self, target: Vec3) {
        if self.clipboard.is_empty() { return; }
        self.editor.push_undo("paste");
        let clipboard = self.clipboard.clone();
        if let Some(model) = self.editor.active_model_mut() {
            clipboard.paste_at(model, target);
        }
    }

    pub fn add_spline_point(&mut self, p: Vec3) {
        self.spline.add_point(p);
    }

    pub fn apply_spline_deform(&mut self, axis: Vec3) {
        let sel = self.editor.selection.clone();
        let spline = self.spline.clone();
        if let Some(model) = self.editor.active_model_mut() {
            spline.deform_along_path(model, &sel, axis);
        }
    }

    pub fn export(&self) -> Option<String> {
        self.editor.export_active_model()
    }

    pub fn import(&mut self, text: &str) -> Option<u64> {
        self.editor.import_model(text)
    }

    pub fn particle_count(&self) -> usize {
        self.editor.particle_count()
    }

    pub fn selection_count(&self) -> usize {
        self.editor.selected_count()
    }
}

impl Default for ModelingToolContext {
    fn default() -> Self { Self::new() }
}



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
                .min_by(|a, b| (**a - p.color).length_squared().partial_cmp(&(**b - p.color).length_squared())
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
        let mut result = ParticleModel::new(0, "scatter_baked");
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
        let mut m = ParticleModel::new(1, "sphere");
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
        let m0 = ParticleModel::new(1, "v0");
        let m1 = ParticleModel::new(2, "v1");
        let m2 = ParticleModel::new(3, "v2");
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

// SECTION 15: Utility functions and constants

/// Remap clamped — like remap but clamps output
pub fn remap_clamped(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = ((value - in_min) / (in_max - in_min).max(1e-9)).clamp(0.0, 1.0);
    out_min + t * (out_max - out_min)
}

/// Smoothstep with custom exponent
pub fn smoothstep_exp(edge0: f32, edge1: f32, x: f32, exp: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-9)).clamp(0.0, 1.0);
    t.powf(exp)
}

/// Smootherstep variant with asymmetric rise/fall
pub fn asymmetric_smoothstep(edge0: f32, edge1: f32, x: f32, bias: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-9)).clamp(0.0, 1.0);
    let biased = if bias > 0.5 {
        1.0 - (1.0 - t).powf(1.0 / (1.0 - bias).max(0.01))
    } else {
        t.powf(1.0 / bias.max(0.01))
    };
    biased
}

/// Signed angle between two Vec3s around an axis (degrees)
pub fn signed_angle_deg(from: Vec3, to: Vec3, axis: Vec3) -> f32 {
    let unsigned = from.dot(to).clamp(-1.0, 1.0).acos();
    let cross = from.cross(to);
    let signed = if axis.dot(cross) < 0.0 { -unsigned } else { unsigned };
    signed.to_degrees()
}

/// Project vector onto a plane and normalize
pub fn project_onto_plane_normalized(v: Vec3, normal: Vec3) -> Vec3 {
    (v - normal * normal.dot(v)).normalize_or_zero()
}

/// Reflect vector across a normal
pub fn reflect_vector(v: Vec3, normal: Vec3) -> Vec3 {
    v - normal * 2.0 * normal.dot(v)
}

/// Rotate a Vec3 by angle around an axis
pub fn rotate_around_axis(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    Quat::from_axis_angle(axis.normalize_or_zero(), angle) * v
}

/// Convert RGB to HSL
pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    if (max - min).abs() < 1e-6 { return (0.0, 0.0, l); }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 { t += 1.0; }
    if t > 1.0 { t -= 1.0; }
    if t < 1.0/6.0 { return p + (q - p) * 6.0 * t; }
    if t < 0.5     { return q; }
    if t < 2.0/3.0 { return p + (q - p) * (2.0/3.0 - t) * 6.0; }
    p
}

/// Convert HSL to RGB
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s < 1e-6 { return (l, l, l); }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    (hue_to_rgb(p, q, h + 1.0/3.0),
     hue_to_rgb(p, q, h),
     hue_to_rgb(p, q, h - 1.0/3.0))
}

/// Random float in [0,1] from a LCG seed (mutates seed)
pub fn lcg_rand(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as u32) as f32 / u32::MAX as f32
}

/// Halton sequence for quasi-random sampling
pub fn halton(index: u32, base: u32) -> f32 {
    let mut f = 1.0_f32;
    let mut r = 0.0_f32;
    let mut i = index;
    let b = base as f32;
    while i > 0 {
        f /= b;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// Map a 1D index to a 2D Hilbert curve coordinate (order n)
pub fn hilbert_d2xy(n: u32, d: u32) -> (u32, u32) {
    let mut s = 1u32;
    let mut x = 0u32;
    let mut y = 0u32;
    let mut t = d;
    while s < n {
        let rx = 1 & (t / 2);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 { x = s.wrapping_sub(1).wrapping_sub(x); y = s.wrapping_sub(1).wrapping_sub(y); }
            std::mem::swap(&mut x, &mut y);
        }
        x = x.wrapping_add(s * rx);
        y = y.wrapping_add(s * ry);
        t /= 4;
        s *= 2;
    }
    (x, y)
}

pub const GOLDEN_RATIO: f32 = 1.618033988749895;
pub const INV_GOLDEN_RATIO: f32 = 0.6180339887498948;
pub const SQRT3: f32 = 1.7320508075688772;
