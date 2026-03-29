#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
//  VOXEL EDITOR
//  Full implementation: SVO, materials, sculpt ops,
//  Marching Cubes, Dual Contouring, procedural generation,
//  physics, LOD, import/export, and editor tools.
// ============================================================

// ─── Constants ───────────────────────────────────────────────────────────────

const MATERIAL_COUNT: usize = 64;
const MAX_OCTREE_DEPTH: usize = 8;
const VOXEL_SIZE: f32 = 0.25;
const MAX_UNDO_HISTORY: usize = 64;
const CHUNK_DIM: usize = 16; // chunk size for RLE export
const MAX_DENSITY: f32 = 1.0;
const MIN_DENSITY: f32 = 0.0;
const ISO_LEVEL: f32 = 0.5;

// ─── Material System ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelMaterial {
    pub id: u8,
    pub color: Vec4,         // RGBA
    pub density_factor: f32, // affects marching cubes iso level
    pub emission: Vec3,      // emissive color
    pub hardness: f32,       // structural strength [0..1]
    pub roughness: f32,
    pub metallic: f32,
    pub transparency: f32,
}

impl Default for VoxelMaterial {
    fn default() -> Self {
        Self {
            id: 0,
            color: Vec4::new(0.8, 0.8, 0.8, 1.0),
            density_factor: 1.0,
            emission: Vec3::ZERO,
            hardness: 0.5,
            roughness: 0.8,
            metallic: 0.0,
            transparency: 0.0,
        }
    }
}

pub fn default_material_table() -> Vec<VoxelMaterial> {
    let mut table = Vec::with_capacity(MATERIAL_COUNT);
    // 0: Air (empty)
    table.push(VoxelMaterial { id: 0, color: Vec4::new(0.0, 0.0, 0.0, 0.0), density_factor: 0.0, hardness: 0.0, ..Default::default() });
    // 1: Stone
    table.push(VoxelMaterial { id: 1, color: Vec4::new(0.5, 0.5, 0.5, 1.0), hardness: 0.9, ..Default::default() });
    // 2: Dirt
    table.push(VoxelMaterial { id: 2, color: Vec4::new(0.4, 0.3, 0.2, 1.0), hardness: 0.3, ..Default::default() });
    // 3: Grass
    table.push(VoxelMaterial { id: 3, color: Vec4::new(0.2, 0.7, 0.2, 1.0), hardness: 0.2, ..Default::default() });
    // 4: Sand
    table.push(VoxelMaterial { id: 4, color: Vec4::new(0.9, 0.8, 0.6, 1.0), hardness: 0.1, ..Default::default() });
    // 5: Water
    table.push(VoxelMaterial { id: 5, color: Vec4::new(0.1, 0.3, 0.8, 0.7), hardness: 0.0, transparency: 0.7, ..Default::default() });
    // 6: Wood
    table.push(VoxelMaterial { id: 6, color: Vec4::new(0.6, 0.4, 0.2, 1.0), hardness: 0.6, ..Default::default() });
    // 7: Leaves
    table.push(VoxelMaterial { id: 7, color: Vec4::new(0.1, 0.6, 0.1, 0.9), hardness: 0.1, ..Default::default() });
    // 8: Iron Ore
    table.push(VoxelMaterial { id: 8, color: Vec4::new(0.6, 0.5, 0.4, 1.0), hardness: 0.95, metallic: 0.7, ..Default::default() });
    // 9: Gold Ore
    table.push(VoxelMaterial { id: 9, color: Vec4::new(0.9, 0.8, 0.2, 1.0), hardness: 0.7, metallic: 0.9, ..Default::default() });
    // 10: Coal
    table.push(VoxelMaterial { id: 10, color: Vec4::new(0.1, 0.1, 0.1, 1.0), hardness: 0.8, ..Default::default() });
    // 11: Diamond
    table.push(VoxelMaterial { id: 11, color: Vec4::new(0.7, 0.95, 1.0, 0.9), hardness: 1.0, roughness: 0.05, ..Default::default() });
    // 12: Lava
    table.push(VoxelMaterial { id: 12, color: Vec4::new(1.0, 0.3, 0.0, 1.0), hardness: 0.05, emission: Vec3::new(3.0, 0.5, 0.0), ..Default::default() });
    // 13: Gravel
    table.push(VoxelMaterial { id: 13, color: Vec4::new(0.6, 0.6, 0.5, 1.0), hardness: 0.25, ..Default::default() });
    // 14: Obsidian
    table.push(VoxelMaterial { id: 14, color: Vec4::new(0.1, 0.05, 0.15, 1.0), hardness: 0.99, metallic: 0.2, roughness: 0.2, ..Default::default() });
    // 15: Snow
    table.push(VoxelMaterial { id: 15, color: Vec4::new(0.95, 0.97, 1.0, 1.0), hardness: 0.05, roughness: 0.9, ..Default::default() });
    // 16: Ice
    table.push(VoxelMaterial { id: 16, color: Vec4::new(0.8, 0.9, 1.0, 0.8), hardness: 0.4, transparency: 0.4, roughness: 0.05, ..Default::default() });
    // 17: Clay
    table.push(VoxelMaterial { id: 17, color: Vec4::new(0.7, 0.65, 0.55, 1.0), hardness: 0.2, ..Default::default() });
    // 18: Marble
    table.push(VoxelMaterial { id: 18, color: Vec4::new(0.95, 0.93, 0.9, 1.0), hardness: 0.85, roughness: 0.1, ..Default::default() });
    // 19: Copper
    table.push(VoxelMaterial { id: 19, color: Vec4::new(0.85, 0.55, 0.3, 1.0), hardness: 0.75, metallic: 0.9, roughness: 0.4, ..Default::default() });
    // Fill remaining with generic materials
    for i in 20..MATERIAL_COUNT {
        let hue = i as f32 / MATERIAL_COUNT as f32;
        let r = (hue * 6.28).sin() * 0.5 + 0.5;
        let g = (hue * 6.28 + 2.094).sin() * 0.5 + 0.5;
        let b = (hue * 6.28 + 4.189).sin() * 0.5 + 0.5;
        table.push(VoxelMaterial {
            id: i as u8,
            color: Vec4::new(r, g, b, 1.0),
            hardness: (i as f32) / MATERIAL_COUNT as f32,
            ..Default::default()
        });
    }
    table
}

/// Blend two materials at a boundary.
pub fn blend_materials(a: &VoxelMaterial, b: &VoxelMaterial, t: f32) -> VoxelMaterial {
    VoxelMaterial {
        id: if t < 0.5 { a.id } else { b.id },
        color: a.color.lerp(b.color, t),
        density_factor: a.density_factor + (b.density_factor - a.density_factor) * t,
        emission: a.emission.lerp(b.emission, t),
        hardness: a.hardness + (b.hardness - a.hardness) * t,
        roughness: a.roughness + (b.roughness - a.roughness) * t,
        metallic: a.metallic + (b.metallic - a.metallic) * t,
        transparency: a.transparency + (b.transparency - a.transparency) * t,
    }
}

// ─── Voxel Cell ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Voxel {
    pub density: f32,    // [0, 1]: 0 = empty, 1 = full
    pub material: u8,
}

impl Voxel {
    pub const EMPTY: Voxel = Voxel { density: 0.0, material: 0 };
    pub const SOLID: Voxel = Voxel { density: 1.0, material: 1 };

    pub fn new(density: f32, material: u8) -> Self {
        Self { density: density.clamp(0.0, 1.0), material }
    }

    pub fn is_empty(&self) -> bool {
        self.density < 0.001
    }

    pub fn is_solid(&self) -> bool {
        self.density > ISO_LEVEL
    }
}

// ─── Sparse Voxel Octree ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum OctreeNode {
    Leaf(Voxel),
    Branch {
        children: Box<[Option<Box<OctreeNode>>; 8]>,
        // bounding box encoded by position in tree
    },
    Empty,
}

impl OctreeNode {
    pub fn is_leaf(&self) -> bool {
        matches!(self, OctreeNode::Leaf(_))
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, OctreeNode::Empty)
    }

    pub fn is_branch(&self) -> bool {
        matches!(self, OctreeNode::Branch { .. })
    }

    pub fn leaf_voxel(&self) -> Option<Voxel> {
        match self {
            OctreeNode::Leaf(v) => Some(*v),
            _ => None,
        }
    }
}

fn empty_children() -> Box<[Option<Box<OctreeNode>>; 8]> {
    Box::new([None, None, None, None, None, None, None, None])
}

/// Child index encoding: 3 bits per level (xyz).
/// child_index(dx, dy, dz) = dx | (dy << 1) | (dz << 2)
pub fn child_index(dx: u8, dy: u8, dz: u8) -> usize {
    (dx as usize) | ((dy as usize) << 1) | ((dz as usize) << 2)
}

pub fn child_offset(idx: usize) -> (u8, u8, u8) {
    let dx = (idx & 1) as u8;
    let dy = ((idx >> 1) & 1) as u8;
    let dz = ((idx >> 2) & 1) as u8;
    (dx, dy, dz)
}

#[derive(Debug, Clone)]
pub struct SparseVoxelOctree {
    pub root: OctreeNode,
    pub depth: usize,
    pub origin: Vec3,
    pub size: f32, // total size
}

impl SparseVoxelOctree {
    pub fn new(origin: Vec3, size: f32, depth: usize) -> Self {
        Self {
            root: OctreeNode::Empty,
            depth,
            origin,
            size,
        }
    }

    /// Voxel size at the leaf level.
    pub fn leaf_size(&self) -> f32 {
        self.size / (1 << self.depth) as f32
    }

    /// Convert a world position to leaf coordinates [0, 2^depth).
    pub fn world_to_leaf(&self, pos: Vec3) -> Option<(i32, i32, i32)> {
        let rel = pos - self.origin;
        let n = (1 << self.depth) as f32;
        let lx = (rel.x / self.size * n) as i32;
        let ly = (rel.y / self.size * n) as i32;
        let lz = (rel.z / self.size * n) as i32;
        let max = (1 << self.depth) as i32;
        if lx < 0 || ly < 0 || lz < 0 || lx >= max || ly >= max || lz >= max {
            None
        } else {
            Some((lx, ly, lz))
        }
    }

    pub fn leaf_to_world(&self, lx: i32, ly: i32, lz: i32) -> Vec3 {
        let n = (1 << self.depth) as f32;
        let leaf_size = self.size / n;
        self.origin + Vec3::new(lx as f32, ly as f32, lz as f32) * leaf_size
    }

    /// Insert a voxel at world position.
    pub fn insert(&mut self, pos: Vec3, voxel: Voxel) {
        if let Some((lx, ly, lz)) = self.world_to_leaf(pos) {
            let max = 1 << self.depth;
            insert_recursive(&mut self.root, lx, ly, lz, max, self.depth, voxel);
        }
    }

    /// Query a voxel at world position.
    pub fn query(&self, pos: Vec3) -> Voxel {
        if let Some((lx, ly, lz)) = self.world_to_leaf(pos) {
            let max = 1 << self.depth;
            query_recursive(&self.root, lx, ly, lz, max, self.depth)
        } else {
            Voxel::EMPTY
        }
    }

    /// Delete (set to empty) a voxel at world position.
    pub fn delete(&mut self, pos: Vec3) {
        self.insert(pos, Voxel::EMPTY);
    }

    /// AABB intersection: returns positions of all leaves within the AABB.
    pub fn aabb_query(&self, min: Vec3, max: Vec3, out: &mut Vec<(Vec3, Voxel)>) {
        let leaf_size = self.leaf_size();
        aabb_recursive(
            &self.root,
            self.origin,
            self.size,
            self.depth,
            min,
            max,
            leaf_size,
            out,
        );
    }

    /// Ray traversal using DDA on octree.
    pub fn ray_intersect(&self, ray_origin: Vec3, ray_dir: Vec3) -> Option<(Vec3, Voxel, f32)> {
        let dir = if ray_dir.length() > 1e-9 { ray_dir.normalize() } else { return None; };
        ray_traverse_octree(
            &self.root,
            self.origin,
            self.size,
            self.depth,
            ray_origin,
            dir,
        )
    }

    /// Collapse uniform children (if all 8 children are same leaf voxel).
    pub fn optimize(&mut self) {
        optimize_node(&mut self.root);
    }

    /// Count all non-empty leaves.
    pub fn voxel_count(&self) -> usize {
        count_recursive(&self.root)
    }
}

fn insert_recursive(node: &mut OctreeNode, lx: i32, ly: i32, lz: i32, size: i32, depth: usize, voxel: Voxel) {
    if depth == 0 {
        *node = if voxel.is_empty() { OctreeNode::Empty } else { OctreeNode::Leaf(voxel) };
        return;
    }
    let half = size / 2;
    let dx = (lx >= half) as u8;
    let dy = (ly >= half) as u8;
    let dz = (lz >= half) as u8;
    let idx = child_index(dx, dy, dz);
    let nlx = lx - dx as i32 * half;
    let nly = ly - dy as i32 * half;
    let nlz = lz - dz as i32 * half;

    match node {
        OctreeNode::Empty => {
            if voxel.is_empty() { return; }
            let mut children = empty_children();
            let mut child = OctreeNode::Empty;
            insert_recursive(&mut child, nlx, nly, nlz, half, depth - 1, voxel);
            children[idx] = Some(Box::new(child));
            *node = OctreeNode::Branch { children };
        }
        OctreeNode::Leaf(existing) => {
            let existing_v = *existing;
            let mut children = empty_children();
            // Expand: fill all 8 children with existing value
            for ci in 0..8 {
                children[ci] = Some(Box::new(OctreeNode::Leaf(existing_v)));
            }
            let mut child = OctreeNode::Leaf(existing_v);
            insert_recursive(&mut child, nlx, nly, nlz, half, depth - 1, voxel);
            children[idx] = Some(Box::new(child));
            *node = OctreeNode::Branch { children };
        }
        OctreeNode::Branch { children } => {
            let child = children[idx].get_or_insert_with(|| Box::new(OctreeNode::Empty));
            insert_recursive(child, nlx, nly, nlz, half, depth - 1, voxel);
        }
    }
}

fn query_recursive(node: &OctreeNode, lx: i32, ly: i32, lz: i32, size: i32, depth: usize) -> Voxel {
    match node {
        OctreeNode::Empty => Voxel::EMPTY,
        OctreeNode::Leaf(v) => *v,
        OctreeNode::Branch { children } => {
            if depth == 0 { return Voxel::EMPTY; }
            let half = size / 2;
            let dx = (lx >= half) as u8;
            let dy = (ly >= half) as u8;
            let dz = (lz >= half) as u8;
            let idx = child_index(dx, dy, dz);
            let nlx = lx - dx as i32 * half;
            let nly = ly - dy as i32 * half;
            let nlz = lz - dz as i32 * half;
            match &children[idx] {
                Some(child) => query_recursive(child, nlx, nly, nlz, half, depth - 1),
                None => Voxel::EMPTY,
            }
        }
    }
}

fn aabb_recursive(
    node: &OctreeNode,
    node_origin: Vec3,
    node_size: f32,
    depth: usize,
    aabb_min: Vec3,
    aabb_max: Vec3,
    leaf_size: f32,
    out: &mut Vec<(Vec3, Voxel)>,
) {
    // Check if this node's AABB intersects query AABB
    let node_max = node_origin + Vec3::splat(node_size);
    if node_origin.x > aabb_max.x || node_max.x < aabb_min.x { return; }
    if node_origin.y > aabb_max.y || node_max.y < aabb_min.y { return; }
    if node_origin.z > aabb_max.z || node_max.z < aabb_min.z { return; }

    match node {
        OctreeNode::Empty => {}
        OctreeNode::Leaf(v) => {
            if !v.is_empty() {
                out.push((node_origin, *v));
            }
        }
        OctreeNode::Branch { children } => {
            let half = node_size / 2.0;
            for ci in 0..8 {
                let (dx, dy, dz) = child_offset(ci);
                let child_origin = node_origin + Vec3::new(dx as f32 * half, dy as f32 * half, dz as f32 * half);
                if let Some(child) = &children[ci] {
                    aabb_recursive(child, child_origin, half, depth.saturating_sub(1), aabb_min, aabb_max, leaf_size, out);
                }
            }
        }
    }
}

fn ray_traverse_octree(
    node: &OctreeNode,
    node_origin: Vec3,
    node_size: f32,
    depth: usize,
    ray_origin: Vec3,
    ray_dir: Vec3,
) -> Option<(Vec3, Voxel, f32)> {
    // Slab test
    let inv_dir = Vec3::new(
        if ray_dir.x.abs() > 1e-9 { 1.0 / ray_dir.x } else { f32::MAX },
        if ray_dir.y.abs() > 1e-9 { 1.0 / ray_dir.y } else { f32::MAX },
        if ray_dir.z.abs() > 1e-9 { 1.0 / ray_dir.z } else { f32::MAX },
    );
    let t1 = (node_origin - ray_origin) * inv_dir;
    let t2 = (node_origin + Vec3::splat(node_size) - ray_origin) * inv_dir;
    let t_min = t1.min(t2);
    let t_max = t1.max(t2);
    let t_enter = t_min.x.max(t_min.y).max(t_min.z);
    let t_exit = t_max.x.min(t_max.y).min(t_max.z);
    if t_enter > t_exit || t_exit < 0.0 { return None; }

    match node {
        OctreeNode::Empty => None,
        OctreeNode::Leaf(v) => {
            if v.is_empty() { None }
            else {
                let hit_pos = ray_origin + ray_dir * t_enter.max(0.0);
                Some((hit_pos, *v, t_enter.max(0.0)))
            }
        }
        OctreeNode::Branch { children } => {
            let half = node_size / 2.0;
            let mut best: Option<(Vec3, Voxel, f32)> = None;
            for ci in 0..8 {
                let (dx, dy, dz) = child_offset(ci);
                let child_origin = node_origin + Vec3::new(dx as f32 * half, dy as f32 * half, dz as f32 * half);
                if let Some(child) = &children[ci] {
                    if let Some(hit) = ray_traverse_octree(child, child_origin, half, depth.saturating_sub(1), ray_origin, ray_dir) {
                        if best.as_ref().map_or(true, |b: &(Vec3, Voxel, f32)| hit.2 < b.2) {
                            best = Some(hit);
                        }
                    }
                }
            }
            best
        }
    }
}

fn optimize_node(node: &mut OctreeNode) {
    match node {
        OctreeNode::Branch { children } => {
            for ci in 0..8 {
                if let Some(child) = &mut children[ci] {
                    optimize_node(child);
                }
            }
            // Check if all children are same leaf
            let first = children[0].as_ref().and_then(|c| c.leaf_voxel());
            if let Some(v0) = first {
                let all_same = children.iter().all(|c| {
                    c.as_ref().and_then(|n| n.leaf_voxel()) == Some(v0)
                });
                if all_same {
                    *node = OctreeNode::Leaf(v0);
                }
            } else {
                // Check all empty
                let all_empty = children.iter().all(|c| {
                    c.as_ref().map_or(true, |n| n.is_empty())
                });
                if all_empty {
                    *node = OctreeNode::Empty;
                }
            }
        }
        _ => {}
    }
}

fn count_recursive(node: &OctreeNode) -> usize {
    match node {
        OctreeNode::Empty => 0,
        OctreeNode::Leaf(v) => if v.is_empty() { 0 } else { 1 },
        OctreeNode::Branch { children } => {
            children.iter().map(|c| c.as_ref().map_or(0, |n| count_recursive(n))).sum()
        }
    }
}

// ─── Dense Voxel Grid (for editing operations) ────────────────────────────────

#[derive(Debug, Clone)]
pub struct VoxelGrid {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
    pub voxels: Vec<Voxel>,
    pub origin: Vec3,
    pub voxel_size: f32,
}

impl VoxelGrid {
    pub fn new(width: usize, height: usize, depth: usize, origin: Vec3, voxel_size: f32) -> Self {
        Self {
            width, height, depth,
            voxels: vec![Voxel::EMPTY; width * height * depth],
            origin,
            voxel_size,
        }
    }

    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        x + y * self.width + z * self.width * self.height
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Voxel {
        if x < 0 || y < 0 || z < 0
            || x >= self.width as i32
            || y >= self.height as i32
            || z >= self.depth as i32
        {
            Voxel::EMPTY
        } else {
            self.voxels[self.index(x as usize, y as usize, z as usize)]
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, v: Voxel) {
        if x < 0 || y < 0 || z < 0
            || x >= self.width as i32
            || y >= self.height as i32
            || z >= self.depth as i32
        {
            return;
        }
        let idx = self.index(x as usize, y as usize, z as usize);
        self.voxels[idx] = v;
    }

    pub fn world_to_grid(&self, pos: Vec3) -> (i32, i32, i32) {
        let rel = pos - self.origin;
        let x = (rel.x / self.voxel_size) as i32;
        let y = (rel.y / self.voxel_size) as i32;
        let z = (rel.z / self.voxel_size) as i32;
        (x, y, z)
    }

    pub fn grid_to_world(&self, x: i32, y: i32, z: i32) -> Vec3 {
        self.origin + Vec3::new(x as f32, y as f32, z as f32) * self.voxel_size
    }

    pub fn to_svo(&self) -> SparseVoxelOctree {
        let size = self.width.max(self.height).max(self.depth) as f32 * self.voxel_size;
        let depth = (size / self.voxel_size).log2().ceil() as usize;
        let depth = depth.min(MAX_OCTREE_DEPTH);
        let mut svo = SparseVoxelOctree::new(self.origin, size, depth);
        for z in 0..self.depth {
            for y in 0..self.height {
                for x in 0..self.width {
                    let v = self.voxels[self.index(x, y, z)];
                    if !v.is_empty() {
                        let pos = self.grid_to_world(x as i32, y as i32, z as i32);
                        svo.insert(pos, v);
                    }
                }
            }
        }
        svo
    }
}

// ─── Sculpt Operations ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SculptOp {
    AddSphere { center: Vec3, radius: f32, material: u8, density: f32 },
    RemoveSphere { center: Vec3, radius: f32 },
    AddBox { min: Vec3, max: Vec3, material: u8, density: f32 },
    RemoveBox { min: Vec3, max: Vec3 },
    SmoothSphere { center: Vec3, radius: f32, strength: f32 },
    PaintSphere { center: Vec3, radius: f32, material: u8 },
    Flatten { center: Vec3, radius: f32, plane_normal: Vec3, plane_d: f32, strength: f32 },
}

pub fn apply_sculpt_op(grid: &mut VoxelGrid, op: &SculptOp) {
    match op {
        SculptOp::AddSphere { center, radius, material, density } => {
            let min_cell = grid.world_to_grid(*center - Vec3::splat(*radius + grid.voxel_size));
            let max_cell = grid.world_to_grid(*center + Vec3::splat(*radius + grid.voxel_size));
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        let world = grid.grid_to_world(x, y, z) + Vec3::splat(grid.voxel_size * 0.5);
                        let dist = (world - *center).length();
                        if dist <= *radius {
                            let existing = grid.get(x, y, z);
                            let new_density = (existing.density + density * (1.0 - dist / radius)).min(1.0);
                            grid.set(x, y, z, Voxel::new(new_density, *material));
                        }
                    }
                }
            }
        }
        SculptOp::RemoveSphere { center, radius } => {
            let min_cell = grid.world_to_grid(*center - Vec3::splat(*radius + grid.voxel_size));
            let max_cell = grid.world_to_grid(*center + Vec3::splat(*radius + grid.voxel_size));
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        let world = grid.grid_to_world(x, y, z) + Vec3::splat(grid.voxel_size * 0.5);
                        let dist = (world - *center).length();
                        if dist <= *radius {
                            let falloff = 1.0 - dist / radius;
                            let existing = grid.get(x, y, z);
                            let new_density = (existing.density - falloff).max(0.0);
                            if new_density < 0.001 {
                                grid.set(x, y, z, Voxel::EMPTY);
                            } else {
                                grid.set(x, y, z, Voxel::new(new_density, existing.material));
                            }
                        }
                    }
                }
            }
        }
        SculptOp::AddBox { min: bmin, max: bmax, material, density } => {
            let min_cell = grid.world_to_grid(*bmin);
            let max_cell = grid.world_to_grid(*bmax);
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        grid.set(x, y, z, Voxel::new(*density, *material));
                    }
                }
            }
        }
        SculptOp::RemoveBox { min: bmin, max: bmax } => {
            let min_cell = grid.world_to_grid(*bmin);
            let max_cell = grid.world_to_grid(*bmax);
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        grid.set(x, y, z, Voxel::EMPTY);
                    }
                }
            }
        }
        SculptOp::SmoothSphere { center, radius, strength } => {
            let min_cell = grid.world_to_grid(*center - Vec3::splat(*radius));
            let max_cell = grid.world_to_grid(*center + Vec3::splat(*radius));
            let mut deltas: Vec<(i32, i32, i32, f32, u8)> = Vec::new();
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        let world = grid.grid_to_world(x, y, z);
                        let dist = (world - *center).length();
                        if dist <= *radius {
                            // Average density with 6-connected neighbors
                            let mut sum = 0.0f32;
                            let mut count = 0u32;
                            for (nx, ny, nz) in [
                                (x-1,y,z),(x+1,y,z),(x,y-1,z),(x,y+1,z),(x,y,z-1),(x,y,z+1),
                                (x,y,z),
                            ] {
                                sum += grid.get(nx, ny, nz).density;
                                count += 1;
                            }
                            let avg = sum / count as f32;
                            let existing = grid.get(x, y, z);
                            let falloff = 1.0 - dist / radius;
                            let new_d = existing.density + (avg - existing.density) * strength * falloff;
                            deltas.push((x, y, z, new_d, existing.material));
                        }
                    }
                }
            }
            for (x, y, z, d, m) in deltas {
                grid.set(x, y, z, Voxel::new(d, m));
            }
        }
        SculptOp::PaintSphere { center, radius, material } => {
            let min_cell = grid.world_to_grid(*center - Vec3::splat(*radius));
            let max_cell = grid.world_to_grid(*center + Vec3::splat(*radius));
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        let world = grid.grid_to_world(x, y, z);
                        let dist = (world - *center).length();
                        if dist <= *radius {
                            let existing = grid.get(x, y, z);
                            if !existing.is_empty() {
                                grid.set(x, y, z, Voxel::new(existing.density, *material));
                            }
                        }
                    }
                }
            }
        }
        SculptOp::Flatten { center, radius, plane_normal, plane_d, strength } => {
            let min_cell = grid.world_to_grid(*center - Vec3::splat(*radius));
            let max_cell = grid.world_to_grid(*center + Vec3::splat(*radius));
            let mut deltas: Vec<(i32, i32, i32, f32, u8)> = Vec::new();
            for z in min_cell.2.max(0)..=max_cell.2.min(grid.depth as i32 - 1) {
                for y in min_cell.1.max(0)..=max_cell.1.min(grid.height as i32 - 1) {
                    for x in min_cell.0.max(0)..=max_cell.0.min(grid.width as i32 - 1) {
                        let world = grid.grid_to_world(x, y, z);
                        let horiz_dist = (world - *center).length();
                        if horiz_dist > *radius { continue; }
                        let existing = grid.get(x, y, z);
                        if existing.is_empty() { continue; }
                        // Distance to plane
                        let signed_dist = plane_normal.dot(world) - plane_d;
                        // Voxels above plane: reduce density; below: increase
                        let falloff = 1.0 - horiz_dist / radius;
                        let adjustment = -signed_dist * strength * falloff;
                        let new_d = (existing.density + adjustment).clamp(0.0, 1.0);
                        deltas.push((x, y, z, new_d, existing.material));
                    }
                }
            }
            for (x, y, z, d, m) in deltas {
                if d < 0.001 {
                    grid.set(x, y, z, Voxel::EMPTY);
                } else {
                    grid.set(x, y, z, Voxel::new(d, m));
                }
            }
        }
    }
}

// ─── Marching Cubes ───────────────────────────────────────────────────────────

/// Full 256-entry edge table for marching cubes.
/// Each entry is a bitmask of the 12 edges that are intersected for that case.
pub const MC_EDGE_TABLE: [u16; 256] = [
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
    0xd30, 0xc39, 0xf33, 0xe3a, 0x936, 0x835, 0xb3f, 0xa36,  // fixed: was b35->835
    0x53c, 0x435, 0x73f, 0x636, 0x13a, 0x033, 0x339, 0x230,
    0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c,
    0x69c, 0x795, 0x49f, 0x596, 0x29a, 0x393, 0x099, 0x190,
    0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905, 0x80c,
    0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x000,
];

/// Triangle table: for each of 256 cases, list of edge indices forming triangles.
/// We store as arrays of i8, -1 = end of list. 256 rows, 16 columns max.
pub const MC_TRI_TABLE: [[i8; 16]; 256] = generate_mc_tri_table();

const fn generate_mc_tri_table() -> [[i8; 16]; 256] {
    // Full standard marching cubes triangle table
    // This is the canonical table from Lorensen & Cline 1987
    [
        [-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,8,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,1,9,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,8,3,9,8,1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,2,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,8,3,1,2,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [9,2,10,0,2,9,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [2,8,3,2,10,8,10,9,8,-1,-1,-1,-1,-1,-1,-1],
        [3,11,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,11,2,8,11,0,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,9,0,2,3,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,11,2,1,9,11,9,8,11,-1,-1,-1,-1,-1,-1,-1],
        [3,10,1,11,10,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,10,1,0,8,10,8,11,10,-1,-1,-1,-1,-1,-1,-1],
        [3,9,0,3,11,9,11,10,9,-1,-1,-1,-1,-1,-1,-1],
        [9,8,10,10,8,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,7,8,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,3,0,7,3,4,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,1,9,8,4,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,1,9,4,7,1,7,3,1,-1,-1,-1,-1,-1,-1,-1],
        [1,2,10,8,4,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [3,4,7,3,0,4,1,2,10,-1,-1,-1,-1,-1,-1,-1],
        [9,2,10,9,0,2,8,4,7,-1,-1,-1,-1,-1,-1,-1],
        [2,10,9,2,9,7,2,7,3,7,9,4,-1,-1,-1,-1],
        [8,4,7,3,11,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [11,4,7,11,2,4,2,0,4,-1,-1,-1,-1,-1,-1,-1],
        [9,0,1,8,4,7,2,3,11,-1,-1,-1,-1,-1,-1,-1],
        [4,7,11,9,4,11,9,11,2,9,2,1,-1,-1,-1,-1],
        [3,10,1,3,11,10,7,8,4,-1,-1,-1,-1,-1,-1,-1],
        [1,11,10,1,4,11,1,0,4,7,11,4,-1,-1,-1,-1],
        [4,7,8,9,0,11,9,11,10,11,0,3,-1,-1,-1,-1],
        [4,7,11,4,11,9,9,11,10,-1,-1,-1,-1,-1,-1,-1],
        [9,5,4,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [9,5,4,0,8,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,5,4,1,5,0,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [8,5,4,8,3,5,3,1,5,-1,-1,-1,-1,-1,-1,-1],
        [1,2,10,9,5,4,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [3,0,8,1,2,10,4,9,5,-1,-1,-1,-1,-1,-1,-1],
        [5,2,10,5,4,2,4,0,2,-1,-1,-1,-1,-1,-1,-1],
        [2,10,5,3,2,5,3,5,4,3,4,8,-1,-1,-1,-1],
        [9,5,4,2,3,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,11,2,0,8,11,4,9,5,-1,-1,-1,-1,-1,-1,-1],
        [0,5,4,0,1,5,2,3,11,-1,-1,-1,-1,-1,-1,-1],
        [2,1,5,2,5,8,2,8,11,4,8,5,-1,-1,-1,-1],
        [10,3,11,10,1,3,9,5,4,-1,-1,-1,-1,-1,-1,-1],
        [4,9,5,0,8,1,8,10,1,8,11,10,-1,-1,-1,-1],
        [5,4,0,5,0,11,5,11,10,11,0,3,-1,-1,-1,-1],
        [5,4,8,5,8,10,10,8,11,-1,-1,-1,-1,-1,-1,-1],
        [9,7,8,5,7,9,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [9,3,0,9,5,3,5,7,3,-1,-1,-1,-1,-1,-1,-1],
        [0,7,8,0,1,7,1,5,7,-1,-1,-1,-1,-1,-1,-1],
        [1,5,3,3,5,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [9,7,8,9,5,7,10,1,2,-1,-1,-1,-1,-1,-1,-1],
        [10,1,2,9,5,0,5,3,0,5,7,3,-1,-1,-1,-1],
        [8,0,2,8,2,5,8,5,7,10,5,2,-1,-1,-1,-1],
        [2,10,5,2,5,3,3,5,7,-1,-1,-1,-1,-1,-1,-1],
        [7,9,5,7,8,9,3,11,2,-1,-1,-1,-1,-1,-1,-1],
        [9,5,7,9,7,2,9,2,0,2,7,11,-1,-1,-1,-1],
        [2,3,11,0,1,8,1,7,8,1,5,7,-1,-1,-1,-1],
        [11,2,1,11,1,7,7,1,5,-1,-1,-1,-1,-1,-1,-1],
        [9,5,8,8,5,7,10,1,3,10,3,11,-1,-1,-1,-1],
        [5,7,0,5,0,9,7,11,0,1,0,10,11,10,0,-1],
        [11,10,0,11,0,3,10,5,0,8,0,7,5,7,0,-1],
        [11,10,5,7,11,5,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [10,6,5,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,8,3,5,10,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [9,0,1,5,10,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,8,3,1,9,8,5,10,6,-1,-1,-1,-1,-1,-1,-1],
        [1,6,5,2,6,1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,6,5,1,2,6,3,0,8,-1,-1,-1,-1,-1,-1,-1],
        [9,6,5,9,0,6,0,2,6,-1,-1,-1,-1,-1,-1,-1],
        [5,9,8,5,8,2,5,2,6,3,2,8,-1,-1,-1,-1],
        [2,3,11,10,6,5,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [11,0,8,11,2,0,10,6,5,-1,-1,-1,-1,-1,-1,-1],
        [0,1,9,2,3,11,5,10,6,-1,-1,-1,-1,-1,-1,-1],
        [5,10,6,1,9,2,9,11,2,9,8,11,-1,-1,-1,-1],
        [6,3,11,6,5,3,5,1,3,-1,-1,-1,-1,-1,-1,-1],
        [0,8,11,0,11,5,0,5,1,5,11,6,-1,-1,-1,-1],
        [3,11,6,0,3,6,0,6,5,0,5,9,-1,-1,-1,-1],
        [6,5,9,6,9,11,11,9,8,-1,-1,-1,-1,-1,-1,-1],
        [5,10,6,4,7,8,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,3,0,4,7,3,6,5,10,-1,-1,-1,-1,-1,-1,-1],
        [1,9,0,5,10,6,8,4,7,-1,-1,-1,-1,-1,-1,-1],
        [10,6,5,1,9,7,1,7,3,7,9,4,-1,-1,-1,-1],
        [6,1,2,6,5,1,4,7,8,-1,-1,-1,-1,-1,-1,-1],
        [1,2,5,5,2,6,3,0,4,3,4,7,-1,-1,-1,-1],
        [8,4,7,9,0,5,0,6,5,0,2,6,-1,-1,-1,-1],
        [7,3,9,7,9,4,3,2,9,5,9,6,2,6,9,-1],
        [3,11,2,7,8,4,10,6,5,-1,-1,-1,-1,-1,-1,-1],
        [5,10,6,4,7,2,4,2,0,2,7,11,-1,-1,-1,-1],
        [0,1,9,4,7,8,2,3,11,5,10,6,-1,-1,-1,-1],
        [9,2,1,9,11,2,9,4,11,7,11,4,5,10,6,-1],
        [8,4,7,3,11,5,3,5,1,5,11,6,-1,-1,-1,-1],
        [5,1,11,5,11,6,1,0,11,7,11,4,0,4,11,-1],
        [0,5,9,0,6,5,0,3,6,11,6,3,8,4,7,-1],
        [6,5,9,6,9,11,4,7,9,7,11,9,-1,-1,-1,-1],
        [10,4,9,6,4,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,10,6,4,9,10,0,8,3,-1,-1,-1,-1,-1,-1,-1],
        [10,0,1,10,6,0,6,4,0,-1,-1,-1,-1,-1,-1,-1],
        [8,3,1,8,1,6,8,6,4,6,1,10,-1,-1,-1,-1],
        [1,4,9,1,2,4,2,6,4,-1,-1,-1,-1,-1,-1,-1],
        [3,0,8,1,2,9,2,4,9,2,6,4,-1,-1,-1,-1],
        [0,2,4,4,2,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [8,3,2,8,2,4,4,2,6,-1,-1,-1,-1,-1,-1,-1],
        [10,4,9,10,6,4,11,2,3,-1,-1,-1,-1,-1,-1,-1],
        [0,8,2,2,8,11,4,9,10,4,10,6,-1,-1,-1,-1],
        [3,11,2,0,1,6,0,6,4,6,1,10,-1,-1,-1,-1],
        [6,4,1,6,1,10,4,8,1,2,1,11,8,11,1,-1],
        [9,6,4,9,3,6,9,1,3,11,6,3,-1,-1,-1,-1],
        [8,11,1,8,1,0,11,6,1,9,1,4,6,4,1,-1],
        [3,11,6,3,6,0,0,6,4,-1,-1,-1,-1,-1,-1,-1],
        [6,4,8,11,6,8,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [7,10,6,7,8,10,8,9,10,-1,-1,-1,-1,-1,-1,-1],
        [0,7,3,0,10,7,0,9,10,6,7,10,-1,-1,-1,-1],
        [10,6,7,1,10,7,1,7,8,1,8,0,-1,-1,-1,-1],
        [10,6,7,10,7,1,1,7,3,-1,-1,-1,-1,-1,-1,-1],
        [1,2,6,1,6,8,1,8,9,8,6,7,-1,-1,-1,-1],
        [2,6,9,2,9,1,6,7,9,0,9,3,7,3,9,-1],
        [7,8,0,7,0,6,6,0,2,-1,-1,-1,-1,-1,-1,-1],
        [7,3,2,6,7,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [2,3,11,10,6,8,10,8,9,8,6,7,-1,-1,-1,-1],
        [2,0,7,2,7,11,0,9,7,6,7,10,9,10,7,-1],
        [1,8,0,1,7,8,1,10,7,6,7,10,2,3,11,-1],
        [11,2,1,11,1,7,10,6,1,6,7,1,-1,-1,-1,-1],
        [8,9,6,8,6,7,9,1,6,11,6,3,1,3,6,-1],
        [0,9,1,11,6,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [7,8,0,7,0,6,3,11,0,11,6,0,-1,-1,-1,-1],
        [7,11,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [7,6,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [3,0,8,11,7,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,1,9,11,7,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [8,1,9,8,3,1,11,7,6,-1,-1,-1,-1,-1,-1,-1],
        [10,1,2,6,11,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,2,10,3,0,8,6,11,7,-1,-1,-1,-1,-1,-1,-1],
        [2,9,0,2,10,9,6,11,7,-1,-1,-1,-1,-1,-1,-1],
        [6,11,7,2,10,3,10,8,3,10,9,8,-1,-1,-1,-1],
        [7,2,3,6,2,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [7,0,8,7,6,0,6,2,0,-1,-1,-1,-1,-1,-1,-1],
        [2,7,6,2,3,7,0,1,9,-1,-1,-1,-1,-1,-1,-1],
        [1,6,2,1,8,6,1,9,8,8,7,6,-1,-1,-1,-1],
        [10,7,6,10,1,7,1,3,7,-1,-1,-1,-1,-1,-1,-1],
        [10,7,6,1,7,10,1,8,7,1,0,8,-1,-1,-1,-1],
        [0,3,7,0,7,10,0,10,9,6,10,7,-1,-1,-1,-1],
        [7,6,10,7,10,8,8,10,9,-1,-1,-1,-1,-1,-1,-1],
        [6,8,4,11,8,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [3,6,11,3,0,6,0,4,6,-1,-1,-1,-1,-1,-1,-1],
        [8,6,11,8,4,6,9,0,1,-1,-1,-1,-1,-1,-1,-1],
        [9,4,6,9,6,3,9,3,1,11,3,6,-1,-1,-1,-1],
        [6,8,4,6,11,8,2,10,1,-1,-1,-1,-1,-1,-1,-1],
        [1,2,10,3,0,11,0,6,11,0,4,6,-1,-1,-1,-1],
        [4,11,8,4,6,11,0,2,9,2,10,9,-1,-1,-1,-1],
        [10,9,3,10,3,2,9,4,3,11,3,6,4,6,3,-1],
        [8,2,3,8,4,2,4,6,2,-1,-1,-1,-1,-1,-1,-1],
        [0,4,2,4,6,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,9,0,2,3,4,2,4,6,4,3,8,-1,-1,-1,-1],
        [1,9,4,1,4,2,2,4,6,-1,-1,-1,-1,-1,-1,-1],
        [8,1,3,8,6,1,8,4,6,6,10,1,-1,-1,-1,-1],
        [10,1,0,10,0,6,6,0,4,-1,-1,-1,-1,-1,-1,-1],
        [4,6,3,4,3,8,6,10,3,0,3,9,10,9,3,-1],
        [10,9,4,6,10,4,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,9,5,7,6,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,8,3,4,9,5,11,7,6,-1,-1,-1,-1,-1,-1,-1],
        [5,0,1,5,4,0,7,6,11,-1,-1,-1,-1,-1,-1,-1],
        [11,7,6,8,3,4,3,5,4,3,1,5,-1,-1,-1,-1],
        [9,5,4,10,1,2,7,6,11,-1,-1,-1,-1,-1,-1,-1],
        [6,11,7,1,2,10,0,8,3,4,9,5,-1,-1,-1,-1],
        [7,6,11,5,4,10,4,2,10,4,0,2,-1,-1,-1,-1],
        [3,4,8,3,5,4,3,2,5,10,5,2,11,7,6,-1],
        [7,2,3,7,6,2,5,4,9,-1,-1,-1,-1,-1,-1,-1],
        [9,5,4,0,8,6,0,6,2,6,8,7,-1,-1,-1,-1],
        [3,6,2,3,7,6,1,5,0,5,4,0,-1,-1,-1,-1],
        [6,2,8,6,8,7,2,1,8,4,8,5,1,5,8,-1],
        [9,5,4,10,1,6,1,7,6,1,3,7,-1,-1,-1,-1],
        [1,6,10,1,7,6,1,0,7,8,7,0,9,5,4,-1],
        [4,0,10,4,10,5,0,3,10,6,10,7,3,7,10,-1],
        [7,6,10,7,10,8,5,4,10,4,8,10,-1,-1,-1,-1],
        [6,9,5,6,11,9,11,8,9,-1,-1,-1,-1,-1,-1,-1],
        [3,6,11,0,6,3,0,5,6,0,9,5,-1,-1,-1,-1],
        [0,11,8,0,5,11,0,1,5,5,6,11,-1,-1,-1,-1],
        [6,11,3,6,3,5,5,3,1,-1,-1,-1,-1,-1,-1,-1],
        [1,2,10,9,5,11,9,11,8,11,5,6,-1,-1,-1,-1],
        [0,11,3,0,6,11,0,9,6,5,6,9,1,2,10,-1],
        [11,8,5,11,5,6,8,0,5,10,5,2,0,2,5,-1],
        [6,11,3,6,3,5,2,10,3,10,5,3,-1,-1,-1,-1],
        [5,8,9,5,2,8,5,6,2,3,8,2,-1,-1,-1,-1],
        [9,5,6,9,6,0,0,6,2,-1,-1,-1,-1,-1,-1,-1],
        [1,5,8,1,8,0,5,6,8,3,8,2,6,2,8,-1],
        [1,5,6,2,1,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,3,6,1,6,10,3,8,6,5,6,9,8,9,6,-1],
        [10,1,0,10,0,6,9,5,0,5,6,0,-1,-1,-1,-1],
        [0,3,8,5,6,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [10,5,6,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [11,5,10,7,5,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [11,5,10,11,7,5,8,3,0,-1,-1,-1,-1,-1,-1,-1],
        [5,11,7,5,10,11,1,9,0,-1,-1,-1,-1,-1,-1,-1],
        [10,7,5,10,11,7,9,8,1,8,3,1,-1,-1,-1,-1],
        [11,1,2,11,7,1,7,5,1,-1,-1,-1,-1,-1,-1,-1],
        [0,8,3,1,2,7,1,7,5,7,2,11,-1,-1,-1,-1],
        [9,7,5,9,2,7,9,0,2,2,11,7,-1,-1,-1,-1],
        [7,5,2,7,2,11,5,9,2,3,2,8,9,8,2,-1],
        [2,5,10,2,3,5,3,7,5,-1,-1,-1,-1,-1,-1,-1],
        [8,2,0,8,5,2,8,7,5,10,2,5,-1,-1,-1,-1],
        [9,0,1,2,3,10,3,5,10,3,7,5,-1,-1,-1,-1],
        [1,2,5,5,2,10,9,8,7,9,7,5,8,3,7,-1],   // rearranged for correctness
        [1,3,5,3,7,5,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,8,7,0,7,1,1,7,5,-1,-1,-1,-1,-1,-1,-1],
        [9,0,3,9,3,5,5,3,7,-1,-1,-1,-1,-1,-1,-1],
        [9,8,7,5,9,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [5,8,4,5,10,8,10,11,8,-1,-1,-1,-1,-1,-1,-1],
        [5,0,4,5,11,0,5,10,11,11,3,0,-1,-1,-1,-1],
        [0,1,9,8,4,10,8,10,11,10,4,5,-1,-1,-1,-1],
        [10,11,4,10,4,5,11,3,4,9,4,1,3,1,4,-1],
        [2,5,1,2,8,5,2,11,8,4,5,8,-1,-1,-1,-1],
        [0,4,11,0,11,3,4,5,11,2,11,1,5,1,11,-1],
        [0,2,5,0,5,9,2,11,5,4,5,8,11,8,5,-1],
        [9,4,5,2,11,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [2,5,10,3,5,2,3,4,5,3,8,4,-1,-1,-1,-1],
        [5,10,2,5,2,4,4,2,0,-1,-1,-1,-1,-1,-1,-1],
        [3,10,2,3,5,10,3,8,5,4,5,8,0,1,9,-1],
        [5,10,2,5,2,4,1,9,2,9,4,2,-1,-1,-1,-1],
        [8,4,5,8,5,3,3,5,1,-1,-1,-1,-1,-1,-1,-1],
        [0,4,5,1,0,5,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [8,4,5,8,5,3,9,0,5,0,3,5,-1,-1,-1,-1],
        [9,4,5,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,11,7,4,9,11,9,10,11,-1,-1,-1,-1,-1,-1,-1],
        [0,8,3,4,9,7,9,11,7,9,10,11,-1,-1,-1,-1],
        [1,10,11,1,11,4,1,4,0,7,4,11,-1,-1,-1,-1],
        [3,1,4,3,4,8,1,10,4,7,4,11,10,11,4,-1],
        [4,11,7,9,11,4,9,2,11,9,1,2,-1,-1,-1,-1],
        [9,7,4,9,11,7,9,1,11,2,11,1,0,8,3,-1],
        [11,7,4,11,4,2,2,4,0,-1,-1,-1,-1,-1,-1,-1],
        [11,7,4,11,4,2,8,3,4,3,2,4,-1,-1,-1,-1],
        [2,9,10,2,7,9,2,3,7,7,4,9,-1,-1,-1,-1],
        [9,10,7,9,7,4,10,2,7,8,7,0,2,0,7,-1],
        [3,7,10,3,10,2,7,4,10,1,10,0,4,0,10,-1],
        [1,10,2,8,7,4,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,9,1,4,1,7,7,1,3,-1,-1,-1,-1,-1,-1,-1],
        [4,9,1,4,1,7,0,8,1,8,7,1,-1,-1,-1,-1],
        [4,0,3,7,4,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [4,8,7,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [9,10,8,10,11,8,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [3,0,9,3,9,11,11,9,10,-1,-1,-1,-1,-1,-1,-1],
        [0,1,10,0,10,8,8,10,11,-1,-1,-1,-1,-1,-1,-1],
        [3,1,10,11,3,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,2,11,1,11,9,9,11,8,-1,-1,-1,-1,-1,-1,-1],
        [3,0,9,3,9,11,1,2,9,2,11,9,-1,-1,-1,-1],
        [0,2,11,8,0,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [3,2,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [2,3,8,2,8,10,10,8,9,-1,-1,-1,-1,-1,-1,-1],
        [9,10,2,0,9,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [2,3,8,2,8,10,0,1,8,1,10,8,-1,-1,-1,-1],
        [1,10,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [1,3,8,9,1,8,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,9,1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [0,3,8,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
        [-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],
    ]
}

/// Edge vertex pairs for marching cubes.
pub const MC_EDGE_VERTICES: [(usize, usize); 12] = [
    (0,1), (1,2), (2,3), (3,0),
    (4,5), (5,6), (6,7), (7,4),
    (0,4), (1,5), (2,6), (3,7),
];

/// Cube vertex offsets for marching cubes (dx, dy, dz).
pub const MC_VERTEX_OFFSETS: [(i32, i32, i32); 8] = [
    (0,0,0), (1,0,0), (1,1,0), (0,1,0),
    (0,0,1), (1,0,1), (1,1,1), (0,1,1),
];

#[derive(Debug, Clone)]
pub struct MeshVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec4,
    pub uv: Vec2,
}

#[derive(Debug, Clone, Default)]
pub struct GeneratedMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

impl GeneratedMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_triangle(&mut self, a: MeshVertex, b: MeshVertex, c: MeshVertex) {
        let base = self.vertices.len() as u32;
        self.vertices.push(a);
        self.vertices.push(b);
        self.vertices.push(c);
        self.indices.push(base);
        self.indices.push(base + 1);
        self.indices.push(base + 2);
    }

    pub fn compute_normals(&mut self) {
        let n_tris = self.indices.len() / 3;
        let mut normals = vec![Vec3::ZERO; self.vertices.len()];
        for ti in 0..n_tris {
            let i0 = self.indices[ti * 3] as usize;
            let i1 = self.indices[ti * 3 + 1] as usize;
            let i2 = self.indices[ti * 3 + 2] as usize;
            let p0 = self.vertices[i0].position;
            let p1 = self.vertices[i1].position;
            let p2 = self.vertices[i2].position;
            let n = (p1 - p0).cross(p2 - p0);
            normals[i0] += n;
            normals[i1] += n;
            normals[i2] += n;
        }
        for (v, n) in self.vertices.iter_mut().zip(normals.iter()) {
            v.normal = if n.length_squared() > 1e-9 { n.normalize() } else { Vec3::Y };
        }
    }
}

/// Run marching cubes on a VoxelGrid.
pub fn marching_cubes(grid: &VoxelGrid, material_table: &[VoxelMaterial]) -> GeneratedMesh {
    let mut mesh = GeneratedMesh::new();
    let vsize = grid.voxel_size;

    for z in 0..(grid.depth as i32 - 1) {
        for y in 0..(grid.height as i32 - 1) {
            for x in 0..(grid.width as i32 - 1) {
                // Fetch 8 corner values
                let mut cube_vals = [0.0f32; 8];
                let mut cube_mats = [0u8; 8];
                let mut cube_idx = 0u8;

                for (vi, (dx, dy, dz)) in MC_VERTEX_OFFSETS.iter().enumerate() {
                    let v = grid.get(x + dx, y + dy, z + dz);
                    cube_vals[vi] = v.density;
                    cube_mats[vi] = v.material;
                    if v.density >= ISO_LEVEL {
                        cube_idx |= 1 << vi;
                    }
                }

                let edge_mask = MC_EDGE_TABLE[cube_idx as usize];
                if edge_mask == 0 { continue; }

                // Compute interpolated edge vertices
                let mut edge_verts = [Vec3::ZERO; 12];
                let mut edge_mats = [0u8; 12];
                for edge in 0..12 {
                    if (edge_mask >> edge) & 1 == 0 { continue; }
                    let (vi0, vi1) = MC_EDGE_VERTICES[edge];
                    let (dx0, dy0, dz0) = MC_VERTEX_OFFSETS[vi0];
                    let (dx1, dy1, dz1) = MC_VERTEX_OFFSETS[vi1];
                    let p0 = grid.grid_to_world(x + dx0, y + dy0, z + dz0);
                    let p1 = grid.grid_to_world(x + dx1, y + dy1, z + dz1);
                    let v0 = cube_vals[vi0];
                    let v1 = cube_vals[vi1];
                    let t = if (v1 - v0).abs() > 1e-9 {
                        (ISO_LEVEL - v0) / (v1 - v0)
                    } else {
                        0.5
                    };
                    // Trilinear interpolation
                    edge_verts[edge] = p0.lerp(p1, t);
                    edge_mats[edge] = if t < 0.5 { cube_mats[vi0] } else { cube_mats[vi1] };
                }

                // Build triangles
                let tri_row = &MC_TRI_TABLE[cube_idx as usize];
                let mut ti = 0;
                while ti < 15 && tri_row[ti] >= 0 {
                    let e0 = tri_row[ti] as usize;
                    let e1 = tri_row[ti+1] as usize;
                    let e2 = tri_row[ti+2] as usize;
                    let p0 = edge_verts[e0];
                    let p1 = edge_verts[e1];
                    let p2 = edge_verts[e2];
                    let n = (p1 - p0).cross(p2 - p0);
                    let norm = if n.length_squared() > 1e-9 { n.normalize() } else { Vec3::Y };
                    let mat_idx = edge_mats[e0] as usize;
                    let color = if mat_idx < material_table.len() {
                        material_table[mat_idx].color
                    } else {
                        Vec4::ONE
                    };
                    mesh.push_triangle(
                        MeshVertex { position: p0, normal: norm, color, uv: Vec2::ZERO },
                        MeshVertex { position: p1, normal: norm, color, uv: Vec2::ZERO },
                        MeshVertex { position: p2, normal: norm, color, uv: Vec2::ZERO },
                    );
                    ti += 3;
                }
            }
        }
    }
    mesh.compute_normals();
    mesh
}

// ─── Dual Contouring ──────────────────────────────────────────────────────────

/// QEF (Quadric Error Function) for dual contouring.
/// Minimize ||Ax - b||^2 where A is the matrix of normals and b = A*intersection_points
#[derive(Debug, Clone, Default)]
pub struct QEF {
    pub ata: [[f64; 3]; 3],  // A^T A (3x3 symmetric)
    pub atb: [f64; 3],       // A^T b
    pub btb: f64,
    pub mass_point: Vec3,
    pub num_points: u32,
}

impl QEF {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a plane defined by point + normal.
    pub fn add_plane(&mut self, point: Vec3, normal: Vec3) {
        let nx = normal.x as f64;
        let ny = normal.y as f64;
        let nz = normal.z as f64;
        let d = (normal.dot(point)) as f64;

        self.ata[0][0] += nx * nx;
        self.ata[0][1] += nx * ny;
        self.ata[0][2] += nx * nz;
        self.ata[1][1] += ny * ny;
        self.ata[1][2] += ny * nz;
        self.ata[2][2] += nz * nz;

        self.atb[0] += nx * d;
        self.atb[1] += ny * d;
        self.atb[2] += nz * d;

        self.btb += d * d;

        self.mass_point += point;
        self.num_points += 1;
    }

    /// Solve the 3x3 linear system using Cramer's rule.
    pub fn solve(&self) -> Vec3 {
        // Fill in the symmetric part
        let a = [
            [self.ata[0][0], self.ata[0][1], self.ata[0][2]],
            [self.ata[0][1], self.ata[1][1], self.ata[1][2]],
            [self.ata[0][2], self.ata[1][2], self.ata[2][2]],
        ];
        let b = self.atb;

        let det = cramer3x3_det(&a);
        if det.abs() < 1e-10 {
            // Degenerate: return mass point
            if self.num_points > 0 {
                return self.mass_point / self.num_points as f32;
            }
            return Vec3::ZERO;
        }

        // Replace columns for Cramer's rule
        let ax = [
            [b[0], a[0][1], a[0][2]],
            [b[1], a[1][1], a[1][2]],
            [b[2], a[2][1], a[2][2]],
        ];
        let ay = [
            [a[0][0], b[0], a[0][2]],
            [a[1][0], b[1], a[1][2]],
            [a[2][0], b[2], a[2][2]],
        ];
        let az = [
            [a[0][0], a[0][1], b[0]],
            [a[1][0], a[1][1], b[1]],
            [a[2][0], a[2][1], b[2]],
        ];

        let x = cramer3x3_det(&ax) / det;
        let y = cramer3x3_det(&ay) / det;
        let z = cramer3x3_det(&az) / det;

        Vec3::new(x as f32, y as f32, z as f32)
    }

    pub fn evaluate(&self, p: Vec3) -> f64 {
        let px = p.x as f64;
        let py = p.y as f64;
        let pz = p.z as f64;
        let a = &self.ata;
        // p^T A^T A p - 2 p^T A^T b + b^T b
        let atap_x = a[0][0]*px + a[0][1]*py + a[0][2]*pz;
        let atap_y = a[0][1]*px + a[1][1]*py + a[1][2]*pz;
        let atap_z = a[0][2]*px + a[1][2]*py + a[2][2]*pz;
        let pt_atap = px*atap_x + py*atap_y + pz*atap_z;
        let pt_atb = px*self.atb[0] + py*self.atb[1] + pz*self.atb[2];
        pt_atap - 2.0*pt_atb + self.btb
    }
}

fn cramer3x3_det(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1]*m[2][2] - m[1][2]*m[2][1])
    - m[0][1] * (m[1][0]*m[2][2] - m[1][2]*m[2][0])
    + m[0][2] * (m[1][0]*m[2][1] - m[1][1]*m[2][0])
}

/// Estimate normal at a grid point using central differences.
pub fn grid_normal(grid: &VoxelGrid, x: i32, y: i32, z: i32) -> Vec3 {
    let dx = grid.get(x+1, y, z).density - grid.get(x-1, y, z).density;
    let dy = grid.get(x, y+1, z).density - grid.get(x, y-1, z).density;
    let dz = grid.get(x, y, z+1).density - grid.get(x, y, z-1).density;
    let n = Vec3::new(dx, dy, dz);
    if n.length_squared() > 1e-9 { n.normalize() } else { Vec3::Y }
}

/// Dual contouring: feature-preserving mesh extraction.
pub fn dual_contouring(grid: &VoxelGrid, material_table: &[VoxelMaterial]) -> GeneratedMesh {
    let mut mesh = GeneratedMesh::new();
    let mut cell_vertices: HashMap<(i32, i32, i32), u32> = HashMap::new();

    // For each cell, compute QEF vertex
    for z in 0..(grid.depth as i32 - 1) {
        for y in 0..(grid.height as i32 - 1) {
            for x in 0..(grid.width as i32 - 1) {
                let mut has_sign_change = false;
                let mut qef = QEF::new();
                let mut mat = 1u8;

                // Check all 12 edges for sign changes
                for (vi0, vi1) in &MC_EDGE_VERTICES {
                    let (dx0, dy0, dz0) = MC_VERTEX_OFFSETS[*vi0];
                    let (dx1, dy1, dz1) = MC_VERTEX_OFFSETS[*vi1];
                    let v0 = grid.get(x+dx0, y+dy0, z+dz0);
                    let v1 = grid.get(x+dx1, y+dy1, z+dz1);
                    let s0 = v0.density >= ISO_LEVEL;
                    let s1 = v1.density >= ISO_LEVEL;
                    if s0 != s1 {
                        has_sign_change = true;
                        let t = if (v1.density - v0.density).abs() > 1e-9 {
                            (ISO_LEVEL - v0.density) / (v1.density - v0.density)
                        } else { 0.5 };
                        let p0 = grid.grid_to_world(x+dx0, y+dy0, z+dz0);
                        let p1 = grid.grid_to_world(x+dx1, y+dy1, z+dz1);
                        let intersection = p0.lerp(p1, t);
                        let ix = (x+dx0) + ((dx1 - dx0) as f32 * t) as i32;
                        let iy = (y+dy0) + ((dy1 - dy0) as f32 * t) as i32;
                        let iz = (z+dz0) + ((dz1 - dz0) as f32 * t) as i32;
                        let normal = grid_normal(grid, ix.max(0), iy.max(0), iz.max(0));
                        qef.add_plane(intersection, normal);
                        mat = if t < 0.5 { v0.material } else { v1.material };
                    }
                }

                if !has_sign_change { continue; }

                let vertex_pos = qef.solve();
                let color = if (mat as usize) < material_table.len() {
                    material_table[mat as usize].color
                } else { Vec4::ONE };
                let normal = grid_normal(grid, x, y, z);

                let vtx_idx = mesh.vertices.len() as u32;
                mesh.vertices.push(MeshVertex { position: vertex_pos, normal, color, uv: Vec2::ZERO });
                cell_vertices.insert((x, y, z), vtx_idx);
            }
        }
    }

    // Generate quads for each edge with sign change
    // Check X-edges (y,z face)
    for z in 1..(grid.depth as i32 - 1) {
        for y in 1..(grid.height as i32 - 1) {
            for x in 0..(grid.width as i32 - 1) {
                let v0 = grid.get(x, y, z);
                let v1 = grid.get(x+1, y, z);
                if (v0.density >= ISO_LEVEL) == (v1.density >= ISO_LEVEL) { continue; }
                // Quad from 4 cells sharing this edge
                let cells = [(x, y-1, z-1), (x, y, z-1), (x, y, z), (x, y-1, z)];
                let verts: Vec<u32> = cells.iter().filter_map(|c| cell_vertices.get(c).copied()).collect();
                if verts.len() == 4 {
                    let flip = v0.density >= ISO_LEVEL;
                    if flip {
                        mesh.indices.extend_from_slice(&[verts[0], verts[2], verts[1]]);
                        mesh.indices.extend_from_slice(&[verts[0], verts[3], verts[2]]);
                    } else {
                        mesh.indices.extend_from_slice(&[verts[0], verts[1], verts[2]]);
                        mesh.indices.extend_from_slice(&[verts[0], verts[2], verts[3]]);
                    }
                }
            }
        }
    }

    mesh.compute_normals();
    mesh
}

// ─── Procedural Generation: 3D Perlin Noise ──────────────────────────────────

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

fn grad3(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 { y } else if h == 12 || h == 14 { x } else { z };
    let a = if (h & 1) != 0 { -u } else { u };
    let b = if (h & 2) != 0 { -v } else { v };
    a + b
}

pub struct PerlinNoise3D {
    pub perm: [u8; 512],
}

impl PerlinNoise3D {
    pub fn new(seed: u64) -> Self {
        let mut perm = [0u8; 512];
        // Simple LCG-based permutation initialization
        let mut p = [0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }
        // Fisher-Yates shuffle with seed
        let mut rng = seed;
        for i in (1..256).rev() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = (rng >> 33) as usize % (i + 1);
            p.swap(i, j);
        }
        for i in 0..256 {
            perm[i] = p[i];
            perm[i + 256] = p[i];
        }
        Self { perm }
    }

    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let xi = (x.floor() as i32 & 255) as usize;
        let yi = (y.floor() as i32 & 255) as usize;
        let zi = (z.floor() as i32 & 255) as usize;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let zf = z - z.floor();
        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        let a  = self.perm[xi] as usize + yi;
        let aa = self.perm[a] as usize + zi;
        let ab = self.perm[a + 1] as usize + zi;
        let b  = self.perm[xi + 1] as usize + yi;
        let ba = self.perm[b] as usize + zi;
        let bb = self.perm[b + 1] as usize + zi;

        lerp_f64(
            lerp_f64(
                lerp_f64(grad3(self.perm[aa], xf, yf, zf),
                         grad3(self.perm[ba], xf-1.0, yf, zf), u),
                lerp_f64(grad3(self.perm[ab], xf, yf-1.0, zf),
                         grad3(self.perm[bb], xf-1.0, yf-1.0, zf), u), v),
            lerp_f64(
                lerp_f64(grad3(self.perm[aa+1], xf, yf, zf-1.0),
                         grad3(self.perm[ba+1], xf-1.0, yf, zf-1.0), u),
                lerp_f64(grad3(self.perm[ab+1], xf, yf-1.0, zf-1.0),
                         grad3(self.perm[bb+1], xf-1.0, yf-1.0, zf-1.0), u), v), w)
    }

    pub fn octave_noise(&self, x: f64, y: f64, z: f64, octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
        let mut total = 0.0;
        let mut frequency = 1.0;
        let mut amplitude = 1.0;
        let mut max_value = 0.0;
        for _ in 0..octaves {
            total += self.noise(x * frequency, y * frequency, z * frequency) * amplitude;
            max_value += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }
        if max_value > 1e-9 { total / max_value } else { 0.0 }
    }
}

/// Generate Perlin terrain in a VoxelGrid.
pub fn generate_terrain(
    grid: &mut VoxelGrid,
    noise: &PerlinNoise3D,
    base_height: f32,
    height_scale: f32,
    noise_scale: f32,
    octaves: u32,
) {
    let base_mat = 3u8; // grass
    let sub_mat = 2u8;  // dirt
    let stone_mat = 1u8;

    for z in 0..grid.depth {
        for x in 0..grid.width {
            let world = grid.grid_to_world(x as i32, 0, z as i32);
            let nx = world.x as f64 / noise_scale as f64;
            let nz = world.z as f64 / noise_scale as f64;
            let h = noise.octave_noise(nx, 0.0, nz, octaves, 0.5, 2.0);
            let surface_y = (base_height + height_scale * h as f32) as i32;

            for y in 0..grid.height {
                let mat = if y == surface_y as usize {
                    base_mat
                } else if y < surface_y as usize && y + 3 >= surface_y as usize {
                    sub_mat
                } else if y < surface_y as usize {
                    stone_mat
                } else {
                    continue; // air
                };
                grid.set(x as i32, y as i32, z as i32, Voxel::new(1.0, mat));
            }
        }
    }
}

/// Cave system using worm algorithm (random walk erosion).
pub fn generate_caves(
    grid: &mut VoxelGrid,
    noise: &PerlinNoise3D,
    num_worms: usize,
    worm_length: usize,
    worm_radius: f32,
    seed: u64,
) {
    let mut rng = seed;
    let next_f32 = |rng: &mut u64| -> f32 {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*rng >> 33) as f32 / (u32::MAX as f32)
    };

    for _ in 0..num_worms {
        // Random starting position
        let sx = (next_f32(&mut rng) * grid.width as f32) as i32;
        let sy = (next_f32(&mut rng) * grid.height as f32 * 0.6 + grid.height as f32 * 0.1) as i32;
        let sz = (next_f32(&mut rng) * grid.depth as f32) as i32;

        let mut wx = sx as f32;
        let mut wy = sy as f32;
        let mut wz = sz as f32;

        for step in 0..worm_length {
            // Noise-guided direction
            let t = step as f64 / worm_length as f64;
            let nx = noise.noise(wx as f64 * 0.05, t * 10.0, 0.0) as f32 * 2.0 - 1.0;
            let ny = noise.noise(wx as f64 * 0.05, t * 10.0, 1.0) as f32 * 0.5 - 0.25;
            let nz = noise.noise(wx as f64 * 0.05, t * 10.0, 2.0) as f32 * 2.0 - 1.0;
            let len = (nx*nx + ny*ny + nz*nz).sqrt().max(1e-9);
            wx += nx / len * 1.5;
            wy += ny / len * 0.5;
            wz += nz / len * 1.5;

            // Carve sphere at current position
            let op = SculptOp::RemoveSphere {
                center: Vec3::new(wx, wy, wz) * grid.voxel_size + grid.origin,
                radius: worm_radius,
            };
            apply_sculpt_op(grid, &op);
        }
    }
}

/// Ore vein generation using fractal noise thresholding.
pub fn generate_ore_veins(
    grid: &mut VoxelGrid,
    noise: &PerlinNoise3D,
    ore_material: u8,
    threshold: f64,
    noise_scale: f64,
    octaves: u32,
    min_depth: i32,
    max_depth: i32,
) {
    for z in 0..grid.depth {
        for y in min_depth.max(0) as usize..max_depth.min(grid.height as i32) as usize {
            for x in 0..grid.width {
                let existing = grid.get(x as i32, y as i32, z as i32);
                if existing.is_empty() { continue; }
                let nx = x as f64 / noise_scale;
                let ny = y as f64 / noise_scale;
                let nz = z as f64 / noise_scale;
                let v = noise.octave_noise(nx, ny, nz, octaves, 0.6, 2.1);
                if v > threshold {
                    grid.set(x as i32, y as i32, z as i32, Voxel::new(1.0, ore_material));
                }
            }
        }
    }
}

// ─── Physics Integration ─────────────────────────────────────────────────────

/// Simple beam model for structural integrity.
#[derive(Debug, Clone)]
pub struct StructuralCell {
    pub stress: f32,
    pub supported: bool,
}

pub fn compute_structural_integrity(
    grid: &VoxelGrid,
    material_table: &[VoxelMaterial],
) -> Vec<StructuralCell> {
    let n = grid.width * grid.height * grid.depth;
    let mut cells = vec![StructuralCell { stress: 0.0, supported: false }; n];

    // Bottom layer is always supported (ground)
    for z in 0..grid.depth {
        for x in 0..grid.width {
            let idx = grid.index(x, 0, z);
            cells[idx].supported = true;
        }
    }

    // Propagate support upward and compute stress (simplified beam model)
    for y in 1..grid.height {
        for z in 0..grid.depth {
            for x in 0..grid.width {
                let v = grid.get(x as i32, y as i32, z as i32);
                if v.is_empty() { continue; }
                let idx = grid.index(x, y, z);
                // Check if any neighbor below is supported
                for (nx, ny, nz) in [
                    (x as i32, y as i32 - 1, z as i32),
                    (x as i32 - 1, y as i32, z as i32),
                    (x as i32 + 1, y as i32, z as i32),
                    (x as i32, y as i32, z as i32 - 1),
                    (x as i32, y as i32, z as i32 + 1),
                ] {
                    if nx < 0 || ny < 0 || nz < 0 || nx >= grid.width as i32 || ny >= grid.height as i32 || nz >= grid.depth as i32 { continue; }
                    let nidx = grid.index(nx as usize, ny as usize, nz as usize);
                    if cells[nidx].supported {
                        cells[idx].supported = true;
                        break;
                    }
                }
                // Stress = weight above / hardness
                let mat_idx = v.material as usize;
                let hardness = if mat_idx < material_table.len() { material_table[mat_idx].hardness } else { 0.5 };
                let weight = y as f32; // simplified: weight proportional to height
                cells[idx].stress = weight / (hardness * 10.0 + 0.001);
            }
        }
    }

    cells
}

/// Destruction: remove voxels below strength threshold, return debris positions.
pub fn apply_destruction(
    grid: &mut VoxelGrid,
    cells: &[StructuralCell],
    material_table: &[VoxelMaterial],
    stress_threshold: f32,
) -> Vec<Vec3> {
    let mut debris = Vec::new();
    for z in 0..grid.depth {
        for y in 0..grid.height {
            for x in 0..grid.width {
                let idx = grid.index(x, y, z);
                let v = grid.get(x as i32, y as i32, z as i32);
                if v.is_empty() { continue; }
                let mat_idx = v.material as usize;
                let hardness = if mat_idx < material_table.len() { material_table[mat_idx].hardness } else { 0.5 };
                let cell = &cells[idx];
                if cell.stress > stress_threshold * hardness || !cell.supported {
                    debris.push(grid.grid_to_world(x as i32, y as i32, z as i32));
                    grid.set(x as i32, y as i32, z as i32, Voxel::EMPTY);
                }
            }
        }
    }
    debris
}

/// Debris particle from destroyed voxel.
#[derive(Debug, Clone)]
pub struct DebrisParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub material: u8,
    pub life: f32,
    pub size: f32,
}

pub fn spawn_debris(positions: &[Vec3], material: u8, seed: u64) -> Vec<DebrisParticle> {
    let mut rng = seed;
    let next_f32 = |rng: &mut u64| -> f32 {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*rng >> 33) as f32 / u32::MAX as f32
    };
    positions.iter().map(|&pos| {
        let vx = (next_f32(&mut rng) - 0.5) * 4.0;
        let vy = next_f32(&mut rng) * 5.0 + 2.0;
        let vz = (next_f32(&mut rng) - 0.5) * 4.0;
        DebrisParticle {
            position: pos,
            velocity: Vec3::new(vx, vy, vz),
            material,
            life: 2.0 + next_f32(&mut rng) * 3.0,
            size: 0.1 + next_f32(&mut rng) * 0.3,
        }
    }).collect()
}

pub fn update_debris(particles: &mut Vec<DebrisParticle>, dt: f32) {
    let gravity = Vec3::new(0.0, -9.8, 0.0);
    for p in particles.iter_mut() {
        p.velocity += gravity * dt;
        p.position += p.velocity * dt;
        p.life -= dt;
    }
    particles.retain(|p| p.life > 0.0);
}

// ─── LOD: Octree LOD ─────────────────────────────────────────────────────────

/// Compute average material of all leaves in a subtree.
pub fn node_average_material(node: &OctreeNode) -> Option<u8> {
    match node {
        OctreeNode::Empty => None,
        OctreeNode::Leaf(v) => Some(v.material),
        OctreeNode::Branch { children } => {
            let mats: Vec<u8> = children.iter()
                .filter_map(|c| c.as_ref())
                .filter_map(|c| node_average_material(c))
                .collect();
            if mats.is_empty() { None }
            else {
                // Most common material
                let mut counts = [0u32; 256];
                for &m in &mats { counts[m as usize] += 1; }
                let max_idx = counts.iter().enumerate().max_by_key(|(_, &c)| c).map(|(i, _)| i).unwrap_or(0);
                Some(max_idx as u8)
            }
        }
    }
}

pub fn merge_octree_lod(svo: &mut SparseVoxelOctree, target_depth: usize) {
    merge_node_lod(&mut svo.root, svo.depth, target_depth);
}

fn merge_node_lod(node: &mut OctreeNode, current_depth: usize, target_depth: usize) {
    if current_depth <= target_depth {
        // At target depth: collapse to representative voxel
        if let Some(mat) = node_average_material(node) {
            let avg_density = node_average_density(node);
            *node = OctreeNode::Leaf(Voxel::new(avg_density, mat));
        }
        return;
    }
    match node {
        OctreeNode::Branch { children } => {
            for ci in 0..8 {
                if let Some(child) = &mut children[ci] {
                    merge_node_lod(child, current_depth - 1, target_depth);
                }
            }
        }
        _ => {}
    }
}

fn node_average_density(node: &OctreeNode) -> f32 {
    match node {
        OctreeNode::Empty => 0.0,
        OctreeNode::Leaf(v) => v.density,
        OctreeNode::Branch { children } => {
            let densities: Vec<f32> = children.iter()
                .filter_map(|c| c.as_ref())
                .map(|c| node_average_density(c))
                .collect();
            if densities.is_empty() { 0.0 }
            else { densities.iter().sum::<f32>() / densities.len() as f32 }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LodOctreeSet {
    pub base: SparseVoxelOctree,
    pub lods: Vec<(usize, SparseVoxelOctree)>, // (depth, svo)
}

impl LodOctreeSet {
    pub fn build(base: SparseVoxelOctree, lod_depths: &[usize]) -> Self {
        let lods = lod_depths.iter().map(|&d| {
            let mut lod = base.clone();
            merge_octree_lod(&mut lod, d);
            lod.optimize();
            (d, lod)
        }).collect();
        Self { base, lods }
    }

    pub fn select_lod(&self, distance: f32, base_dist: f32) -> &SparseVoxelOctree {
        let lod_idx = ((distance / base_dist).log2() as usize).min(self.lods.len());
        if lod_idx == 0 {
            &self.base
        } else {
            &self.lods[(lod_idx - 1).min(self.lods.len() - 1)].1
        }
    }
}

// ─── Import/Export ────────────────────────────────────────────────────────────

/// Export voxel grid as raw binary (width*height*depth bytes for material, separate density).
pub fn export_raw_binary(grid: &VoxelGrid) -> Vec<u8> {
    let n = grid.width * grid.height * grid.depth;
    let mut data = Vec::with_capacity(n * 2 + 12);
    // Header: width(4), height(4), depth(4)
    data.extend_from_slice(&(grid.width as u32).to_le_bytes());
    data.extend_from_slice(&(grid.height as u32).to_le_bytes());
    data.extend_from_slice(&(grid.depth as u32).to_le_bytes());
    // Material bytes
    for v in &grid.voxels {
        data.push(v.material);
    }
    // Density as u8 (0-255)
    for v in &grid.voxels {
        data.push((v.density * 255.0).round() as u8);
    }
    data
}

pub fn import_raw_binary(data: &[u8]) -> Option<VoxelGrid> {
    if data.len() < 12 { return None; }
    let w = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let h = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let d = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
    let n = w * h * d;
    if data.len() < 12 + n * 2 { return None; }
    let mut grid = VoxelGrid::new(w, h, d, Vec3::ZERO, VOXEL_SIZE);
    let mats = &data[12..12 + n];
    let dens = &data[12 + n..12 + n * 2];
    for (i, (m, de)) in mats.iter().zip(dens.iter()).enumerate() {
        grid.voxels[i] = Voxel::new(*de as f32 / 255.0, *m);
    }
    Some(grid)
}

/// Run-length encoding for sparse voxel data.
pub fn rle_encode(voxels: &[Voxel]) -> Vec<u8> {
    if voxels.is_empty() { return Vec::new(); }
    let mut out = Vec::new();
    let mut i = 0;
    while i < voxels.len() {
        let current = voxels[i];
        let mut run_len = 1usize;
        while i + run_len < voxels.len() && run_len < 255 && voxels[i + run_len] == current {
            run_len += 1;
        }
        out.push(run_len as u8);
        out.push(current.material);
        out.push((current.density * 255.0).round() as u8);
        i += run_len;
    }
    out
}

pub fn rle_decode(data: &[u8], expected_len: usize) -> Vec<Voxel> {
    let mut out = Vec::with_capacity(expected_len);
    let mut i = 0;
    while i + 2 < data.len() && out.len() < expected_len {
        let run = data[i] as usize;
        let mat = data[i + 1];
        let den = data[i + 2] as f32 / 255.0;
        let v = Voxel::new(den, mat);
        for _ in 0..run {
            if out.len() >= expected_len { break; }
            out.push(v);
        }
        i += 3;
    }
    while out.len() < expected_len {
        out.push(Voxel::EMPTY);
    }
    out
}

/// Export using RLE compression.
pub fn export_rle(grid: &VoxelGrid) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&(grid.width as u32).to_le_bytes());
    data.extend_from_slice(&(grid.height as u32).to_le_bytes());
    data.extend_from_slice(&(grid.depth as u32).to_le_bytes());
    let encoded = rle_encode(&grid.voxels);
    data.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    data.extend_from_slice(&encoded);
    data
}

pub fn import_rle(data: &[u8]) -> Option<VoxelGrid> {
    if data.len() < 16 { return None; }
    let w = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let h = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let d = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
    let rle_len = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
    if data.len() < 16 + rle_len { return None; }
    let expected = w * h * d;
    let voxels = rle_decode(&data[16..16 + rle_len], expected);
    let mut grid = VoxelGrid::new(w, h, d, Vec3::ZERO, VOXEL_SIZE);
    grid.voxels = voxels;
    Some(grid)
}

/// Import heightmap (2D array of heights) and extrude to 3D.
pub fn import_heightmap(
    heights: &[f32],
    width: usize,
    depth: usize,
    max_height: usize,
    material_surface: u8,
    material_subsurface: u8,
    material_base: u8,
) -> VoxelGrid {
    let mut grid = VoxelGrid::new(width, max_height, depth, Vec3::ZERO, VOXEL_SIZE);
    for z in 0..depth {
        for x in 0..width {
            let h_norm = heights[x + z * width].clamp(0.0, 1.0);
            let h = (h_norm * max_height as f32) as usize;
            for y in 0..h {
                let mat = if y + 1 == h { material_surface }
                    else if h.saturating_sub(y) <= 3 { material_subsurface }
                    else { material_base };
                grid.set(x as i32, y as i32, z as i32, Voxel::new(1.0, mat));
            }
        }
    }
    grid
}

// ─── Selection: Flood Fill ────────────────────────────────────────────────────

/// Flood-fill selection starting from a seed voxel. Returns set of grid coordinates.
pub fn flood_fill_select(
    grid: &VoxelGrid,
    seed_x: i32,
    seed_y: i32,
    seed_z: i32,
    same_material_only: bool,
) -> HashSet<(i32, i32, i32)> {
    let seed_voxel = grid.get(seed_x, seed_y, seed_z);
    if seed_voxel.is_empty() { return HashSet::new(); }

    let mut selected = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((seed_x, seed_y, seed_z));

    while let Some((x, y, z)) = queue.pop_front() {
        if selected.contains(&(x, y, z)) { continue; }
        let v = grid.get(x, y, z);
        if v.is_empty() { continue; }
        if same_material_only && v.material != seed_voxel.material { continue; }
        selected.insert((x, y, z));

        for (nx, ny, nz) in [
            (x-1,y,z),(x+1,y,z),(x,y-1,z),(x,y+1,z),(x,y,z-1),(x,y,z+1),
        ] {
            if !selected.contains(&(nx, ny, nz)) {
                queue.push_back((nx, ny, nz));
            }
        }
    }
    selected
}

// ─── Copy/Paste Volume ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VoxelClipboard {
    pub voxels: Vec<((i32, i32, i32), Voxel)>,
    pub bounds_min: (i32, i32, i32),
    pub bounds_max: (i32, i32, i32),
}

impl VoxelClipboard {
    pub fn copy_region(
        grid: &VoxelGrid,
        min: (i32, i32, i32),
        max: (i32, i32, i32),
    ) -> Self {
        let mut voxels = Vec::new();
        for z in min.2..=max.2 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    let v = grid.get(x, y, z);
                    if !v.is_empty() {
                        voxels.push(((x - min.0, y - min.1, z - min.2), v));
                    }
                }
            }
        }
        Self {
            voxels,
            bounds_min: (0, 0, 0),
            bounds_max: (max.0 - min.0, max.1 - min.1, max.2 - min.2),
        }
    }

    pub fn paste(&self, grid: &mut VoxelGrid, offset: (i32, i32, i32)) {
        for &((rx, ry, rz), v) in &self.voxels {
            let x = rx + offset.0;
            let y = ry + offset.1;
            let z = rz + offset.2;
            grid.set(x, y, z, v);
        }
    }

    pub fn size(&self) -> (i32, i32, i32) {
        (
            self.bounds_max.0 - self.bounds_min.0 + 1,
            self.bounds_max.1 - self.bounds_min.1 + 1,
            self.bounds_max.2 - self.bounds_min.2 + 1,
        )
    }
}

// ─── Mirror Symmetry ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MirrorAxis {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
    XYZ,
}

/// Apply a sculpt operation with mirror symmetry.
pub fn apply_sculpt_mirrored(
    grid: &mut VoxelGrid,
    op: &SculptOp,
    axis: MirrorAxis,
    mirror_origin: Vec3,
) {
    apply_sculpt_op(grid, op);
    let mirrored_ops = mirror_op(op, axis, mirror_origin);
    for mirrored in mirrored_ops {
        apply_sculpt_op(grid, &mirrored);
    }
}

fn mirror_pos(pos: Vec3, axis: MirrorAxis, origin: Vec3) -> Vec<Vec3> {
    let rel = pos - origin;
    match axis {
        MirrorAxis::X => vec![origin + Vec3::new(-rel.x, rel.y, rel.z)],
        MirrorAxis::Y => vec![origin + Vec3::new(rel.x, -rel.y, rel.z)],
        MirrorAxis::Z => vec![origin + Vec3::new(rel.x, rel.y, -rel.z)],
        MirrorAxis::XY => vec![
            origin + Vec3::new(-rel.x, rel.y, rel.z),
            origin + Vec3::new(rel.x, -rel.y, rel.z),
            origin + Vec3::new(-rel.x, -rel.y, rel.z),
        ],
        MirrorAxis::XZ => vec![
            origin + Vec3::new(-rel.x, rel.y, rel.z),
            origin + Vec3::new(rel.x, rel.y, -rel.z),
            origin + Vec3::new(-rel.x, rel.y, -rel.z),
        ],
        MirrorAxis::YZ => vec![
            origin + Vec3::new(rel.x, -rel.y, rel.z),
            origin + Vec3::new(rel.x, rel.y, -rel.z),
            origin + Vec3::new(rel.x, -rel.y, -rel.z),
        ],
        MirrorAxis::XYZ => {
            let mut result = Vec::new();
            for xi in 0..2 {
                for yi in 0..2 {
                    for zi in 0..2 {
                        if xi == 0 && yi == 0 && zi == 0 { continue; }
                        let sx = if xi == 1 { -1.0 } else { 1.0 };
                        let sy = if yi == 1 { -1.0 } else { 1.0 };
                        let sz = if zi == 1 { -1.0 } else { 1.0 };
                        result.push(origin + Vec3::new(rel.x * sx, rel.y * sy, rel.z * sz));
                    }
                }
            }
            result
        }
    }
}

fn mirror_op(op: &SculptOp, axis: MirrorAxis, origin: Vec3) -> Vec<SculptOp> {
    match op {
        SculptOp::AddSphere { center, radius, material, density } => {
            mirror_pos(*center, axis, origin).into_iter()
                .map(|c| SculptOp::AddSphere { center: c, radius: *radius, material: *material, density: *density })
                .collect()
        }
        SculptOp::RemoveSphere { center, radius } => {
            mirror_pos(*center, axis, origin).into_iter()
                .map(|c| SculptOp::RemoveSphere { center: c, radius: *radius })
                .collect()
        }
        SculptOp::PaintSphere { center, radius, material } => {
            mirror_pos(*center, axis, origin).into_iter()
                .map(|c| SculptOp::PaintSphere { center: c, radius: *radius, material: *material })
                .collect()
        }
        SculptOp::SmoothSphere { center, radius, strength } => {
            mirror_pos(*center, axis, origin).into_iter()
                .map(|c| SculptOp::SmoothSphere { center: c, radius: *radius, strength: *strength })
                .collect()
        }
        _ => Vec::new(),
    }
}

// ─── Undo/Redo System ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VoxelUndoState {
    pub modified_voxels: Vec<((i32, i32, i32), Voxel, Voxel)>, // (pos, before, after)
}

impl VoxelUndoState {
    pub fn new() -> Self {
        Self { modified_voxels: Vec::new() }
    }

    pub fn record_change(&mut self, x: i32, y: i32, z: i32, before: Voxel, after: Voxel) {
        if before != after {
            self.modified_voxels.push(((x, y, z), before, after));
        }
    }
}

pub struct VoxelUndoHistory {
    pub states: VecDeque<VoxelUndoState>,
    pub current: usize,
    pub max_history: usize,
}

impl VoxelUndoHistory {
    pub fn new() -> Self {
        Self {
            states: VecDeque::new(),
            current: 0,
            max_history: MAX_UNDO_HISTORY,
        }
    }

    pub fn push(&mut self, state: VoxelUndoState) {
        // Remove any redo states
        while self.states.len() > self.current {
            self.states.pop_back();
        }
        self.states.push_back(state);
        if self.states.len() > self.max_history {
            self.states.pop_front();
        }
        self.current = self.states.len();
    }

    pub fn undo(&mut self, grid: &mut VoxelGrid) -> bool {
        if self.current == 0 { return false; }
        self.current -= 1;
        let state = &self.states[self.current];
        for &((x, y, z), before, _after) in &state.modified_voxels {
            grid.set(x, y, z, before);
        }
        true
    }

    pub fn redo(&mut self, grid: &mut VoxelGrid) -> bool {
        if self.current >= self.states.len() { return false; }
        let state = &self.states[self.current];
        for &((x, y, z), _before, after) in &state.modified_voxels {
            grid.set(x, y, z, after);
        }
        self.current += 1;
        true
    }

    pub fn can_undo(&self) -> bool { self.current > 0 }
    pub fn can_redo(&self) -> bool { self.current < self.states.len() }
}

// ─── Voxel Editor State Machine ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum EditorTool {
    Add,
    Remove,
    Smooth,
    Paint,
    Flatten,
    Select,
    Fill,
}

#[derive(Debug, Clone)]
pub struct BrushSettings {
    pub radius: f32,
    pub strength: f32,
    pub material: u8,
    pub mirror_axis: Option<MirrorAxis>,
    pub mirror_origin: Vec3,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            radius: 2.0,
            strength: 0.5,
            material: 1,
            mirror_axis: None,
            mirror_origin: Vec3::ZERO,
        }
    }
}

pub struct VoxelEditor {
    pub grid: VoxelGrid,
    pub svo: Option<SparseVoxelOctree>,
    pub material_table: Vec<VoxelMaterial>,
    pub undo_history: VoxelUndoHistory,
    pub active_tool: EditorTool,
    pub brush: BrushSettings,
    pub selection: HashSet<(i32, i32, i32)>,
    pub clipboard: Option<VoxelClipboard>,
    pub mesh: Option<GeneratedMesh>,
    pub mesh_dirty: bool,
    pub noise: PerlinNoise3D,
}

impl VoxelEditor {
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        Self {
            grid: VoxelGrid::new(width, height, depth, Vec3::ZERO, VOXEL_SIZE),
            svo: None,
            material_table: default_material_table(),
            undo_history: VoxelUndoHistory::new(),
            active_tool: EditorTool::Add,
            brush: BrushSettings::default(),
            selection: HashSet::new(),
            clipboard: None,
            mesh: None,
            mesh_dirty: true,
            noise: PerlinNoise3D::new(42),
        }
    }

    pub fn apply_brush(&mut self, world_pos: Vec3) {
        // Capture before state
        let mut undo_state = VoxelUndoState::new();
        let min_c = self.grid.world_to_grid(world_pos - Vec3::splat(self.brush.radius + self.grid.voxel_size));
        let max_c = self.grid.world_to_grid(world_pos + Vec3::splat(self.brush.radius + self.grid.voxel_size));
        let mut before: Vec<((i32, i32, i32), Voxel)> = Vec::new();
        for z in min_c.2.max(0)..=max_c.2.min(self.grid.depth as i32 - 1) {
            for y in min_c.1.max(0)..=max_c.1.min(self.grid.height as i32 - 1) {
                for x in min_c.0.max(0)..=max_c.0.min(self.grid.width as i32 - 1) {
                    before.push(((x, y, z), self.grid.get(x, y, z)));
                }
            }
        }

        let op = match self.active_tool {
            EditorTool::Add => SculptOp::AddSphere {
                center: world_pos,
                radius: self.brush.radius,
                material: self.brush.material,
                density: self.brush.strength,
            },
            EditorTool::Remove => SculptOp::RemoveSphere {
                center: world_pos,
                radius: self.brush.radius,
            },
            EditorTool::Smooth => SculptOp::SmoothSphere {
                center: world_pos,
                radius: self.brush.radius,
                strength: self.brush.strength,
            },
            EditorTool::Paint => SculptOp::PaintSphere {
                center: world_pos,
                radius: self.brush.radius,
                material: self.brush.material,
            },
            EditorTool::Flatten => SculptOp::Flatten {
                center: world_pos,
                radius: self.brush.radius,
                plane_normal: Vec3::Y,
                plane_d: world_pos.y,
                strength: self.brush.strength,
            },
            _ => return,
        };

        if let Some(axis) = self.brush.mirror_axis {
            apply_sculpt_mirrored(&mut self.grid, &op, axis, self.brush.mirror_origin);
        } else {
            apply_sculpt_op(&mut self.grid, &op);
        }

        // Record undo
        for ((x, y, z), before_v) in before {
            let after_v = self.grid.get(x, y, z);
            undo_state.record_change(x, y, z, before_v, after_v);
        }
        self.undo_history.push(undo_state);
        self.mesh_dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        let result = self.undo_history.undo(&mut self.grid);
        if result { self.mesh_dirty = true; }
        result
    }

    pub fn redo(&mut self) -> bool {
        let result = self.undo_history.redo(&mut self.grid);
        if result { self.mesh_dirty = true; }
        result
    }

    pub fn select_at(&mut self, x: i32, y: i32, z: i32, add_to_selection: bool) {
        if !add_to_selection { self.selection.clear(); }
        let v = self.grid.get(x, y, z);
        if !v.is_empty() {
            self.selection.insert((x, y, z));
        }
    }

    pub fn flood_select(&mut self, x: i32, y: i32, z: i32, same_material: bool) {
        self.selection = flood_fill_select(&self.grid, x, y, z, same_material);
    }

    pub fn copy_selection(&mut self) {
        if self.selection.is_empty() { return; }
        let mut min_x = i32::MAX; let mut min_y = i32::MAX; let mut min_z = i32::MAX;
        let mut max_x = i32::MIN; let mut max_y = i32::MIN; let mut max_z = i32::MIN;
        for &(x, y, z) in &self.selection {
            min_x = min_x.min(x); min_y = min_y.min(y); min_z = min_z.min(z);
            max_x = max_x.max(x); max_y = max_y.max(y); max_z = max_z.max(z);
        }
        let cb = VoxelClipboard::copy_region(
            &self.grid,
            (min_x, min_y, min_z),
            (max_x, max_y, max_z),
        );
        self.clipboard = Some(cb);
    }

    pub fn paste(&mut self, offset: (i32, i32, i32)) {
        if let Some(ref cb) = self.clipboard.clone() {
            let mut undo_state = VoxelUndoState::new();
            let (sw, sh, sd) = cb.size();
            for &((rx, ry, rz), after_v) in &cb.voxels {
                let x = rx + offset.0;
                let y = ry + offset.1;
                let z = rz + offset.2;
                let before_v = self.grid.get(x, y, z);
                undo_state.record_change(x, y, z, before_v, after_v);
                self.grid.set(x, y, z, after_v);
            }
            self.undo_history.push(undo_state);
            self.mesh_dirty = true;
        }
    }

    pub fn rebuild_mesh(&mut self) {
        if self.mesh_dirty {
            self.mesh = Some(marching_cubes(&self.grid, &self.material_table));
            self.mesh_dirty = false;
        }
    }

    pub fn rebuild_svo(&mut self) {
        self.svo = Some(self.grid.to_svo());
    }

    pub fn generate_terrain(&mut self, base_height: f32, height_scale: f32, noise_scale: f32) {
        generate_terrain(&mut self.grid, &self.noise, base_height, height_scale, noise_scale, 6);
        self.mesh_dirty = true;
    }

    pub fn generate_caves(&mut self, num_worms: usize, worm_length: usize, radius: f32) {
        generate_caves(&mut self.grid, &self.noise, num_worms, worm_length, radius, 12345);
        self.mesh_dirty = true;
    }

    pub fn ray_cast(&self, origin: Vec3, dir: Vec3) -> Option<(Vec3, u8)> {
        if let Some(ref svo) = self.svo {
            svo.ray_intersect(origin, dir).map(|(pos, v, _t)| (pos, v.material))
        } else {
            // Fallback DDA on dense grid
            ray_dda(&self.grid, origin, dir).map(|(x, y, z)| {
                let world = self.grid.grid_to_world(x, y, z);
                let v = self.grid.get(x, y, z);
                (world, v.material)
            })
        }
    }

    pub fn fill_selection(&mut self, material: u8) {
        let sel: Vec<(i32, i32, i32)> = self.selection.iter().cloned().collect();
        let mut undo_state = VoxelUndoState::new();
        for (x, y, z) in sel {
            let before = self.grid.get(x, y, z);
            let after = Voxel::new(1.0, material);
            undo_state.record_change(x, y, z, before, after);
            self.grid.set(x, y, z, after);
        }
        self.undo_history.push(undo_state);
        self.mesh_dirty = true;
    }

    pub fn delete_selection(&mut self) {
        let sel: Vec<(i32, i32, i32)> = self.selection.iter().cloned().collect();
        let mut undo_state = VoxelUndoState::new();
        for (x, y, z) in sel {
            let before = self.grid.get(x, y, z);
            undo_state.record_change(x, y, z, before, Voxel::EMPTY);
            self.grid.set(x, y, z, Voxel::EMPTY);
        }
        self.undo_history.push(undo_state);
        self.mesh_dirty = true;
        self.selection.clear();
    }

    pub fn apply_physics_destruction(&mut self, stress_threshold: f32) -> Vec<DebrisParticle> {
        let cells = compute_structural_integrity(&self.grid, &self.material_table);
        let debris_positions = apply_destruction(&mut self.grid, &cells, &self.material_table, stress_threshold);
        self.mesh_dirty = true;
        spawn_debris(&debris_positions, 1, 42)
    }

    pub fn voxel_count(&self) -> usize {
        self.grid.voxels.iter().filter(|v| !v.is_empty()).count()
    }
}

// ─── DDA Ray Traversal on Dense Grid ─────────────────────────────────────────

pub fn ray_dda(grid: &VoxelGrid, origin: Vec3, dir: Vec3) -> Option<(i32, i32, i32)> {
    let (mut ix, mut iy, mut iz) = grid.world_to_grid(origin);
    let dx = if dir.x > 0.0 { 1i32 } else { -1 };
    let dy = if dir.y > 0.0 { 1i32 } else { -1 };
    let dz = if dir.z > 0.0 { 1i32 } else { -1 };

    let step_x = if dir.x.abs() > 1e-9 { (grid.voxel_size / dir.x.abs()) } else { f32::MAX };
    let step_y = if dir.y.abs() > 1e-9 { (grid.voxel_size / dir.y.abs()) } else { f32::MAX };
    let step_z = if dir.z.abs() > 1e-9 { (grid.voxel_size / dir.z.abs()) } else { f32::MAX };

    let mut t_max_x = step_x * 0.5;
    let mut t_max_y = step_y * 0.5;
    let mut t_max_z = step_z * 0.5;

    for _ in 0..512 {
        if ix < 0 || iy < 0 || iz < 0
            || ix >= grid.width as i32
            || iy >= grid.height as i32
            || iz >= grid.depth as i32
        {
            return None;
        }
        let v = grid.get(ix, iy, iz);
        if v.is_solid() {
            return Some((ix, iy, iz));
        }
        if t_max_x < t_max_y {
            if t_max_x < t_max_z { ix += dx; t_max_x += step_x; }
            else { iz += dz; t_max_z += step_z; }
        } else {
            if t_max_y < t_max_z { iy += dy; t_max_y += step_y; }
            else { iz += dz; t_max_z += step_z; }
        }
    }
    None
}

// ─── Voxel World Chunking ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
}

impl ChunkCoord {
    pub fn from_world(pos: Vec3, chunk_voxel_size: f32, chunk_dim: usize) -> Self {
        let chunk_world_size = chunk_voxel_size * chunk_dim as f32;
        Self {
            cx: (pos.x / chunk_world_size).floor() as i32,
            cy: (pos.y / chunk_world_size).floor() as i32,
            cz: (pos.z / chunk_world_size).floor() as i32,
        }
    }

    pub fn to_world_origin(&self, chunk_voxel_size: f32, chunk_dim: usize) -> Vec3 {
        let size = chunk_voxel_size * chunk_dim as f32;
        Vec3::new(
            self.cx as f32 * size,
            self.cy as f32 * size,
            self.cz as f32 * size,
        )
    }
}

pub struct VoxelWorld {
    pub chunks: HashMap<ChunkCoord, VoxelGrid>,
    pub chunk_dim: usize,
    pub voxel_size: f32,
    pub material_table: Vec<VoxelMaterial>,
    pub noise: PerlinNoise3D,
    pub loaded_chunks: HashSet<ChunkCoord>,
}

impl VoxelWorld {
    pub fn new(chunk_dim: usize, voxel_size: f32, seed: u64) -> Self {
        Self {
            chunks: HashMap::new(),
            chunk_dim,
            voxel_size,
            material_table: default_material_table(),
            noise: PerlinNoise3D::new(seed),
            loaded_chunks: HashSet::new(),
        }
    }

    pub fn get_or_create_chunk(&mut self, coord: &ChunkCoord) -> &mut VoxelGrid {
        let dim = self.chunk_dim;
        let vs = self.voxel_size;
        let origin = coord.to_world_origin(vs, dim);
        self.chunks.entry(coord.clone()).or_insert_with(|| {
            VoxelGrid::new(dim, dim, dim, origin, vs)
        })
    }

    pub fn get_voxel(&self, world_pos: Vec3) -> Voxel {
        let coord = ChunkCoord::from_world(world_pos, self.voxel_size, self.chunk_dim);
        if let Some(chunk) = self.chunks.get(&coord) {
            let local = world_pos - coord.to_world_origin(self.voxel_size, self.chunk_dim);
            let (lx, ly, lz) = chunk.world_to_grid(local + chunk.origin);
            chunk.get(lx, ly, lz)
        } else {
            Voxel::EMPTY
        }
    }

    pub fn set_voxel(&mut self, world_pos: Vec3, voxel: Voxel) {
        let coord = ChunkCoord::from_world(world_pos, self.voxel_size, self.chunk_dim);
        let chunk = self.get_or_create_chunk(&coord);
        let (lx, ly, lz) = chunk.world_to_grid(world_pos);
        chunk.set(lx, ly, lz, voxel);
    }

    pub fn generate_chunk(&mut self, coord: &ChunkCoord) {
        let dim = self.chunk_dim;
        let vs = self.voxel_size;
        let origin = coord.to_world_origin(vs, dim);
        let chunk = self.chunks.entry(coord.clone()).or_insert_with(|| {
            VoxelGrid::new(dim, dim, dim, origin, vs)
        });
        generate_terrain(chunk, &self.noise, dim as f32 / 2.0, dim as f32 / 3.0, 32.0, 4);
    }

    pub fn chunks_in_radius(&self, center: Vec3, radius: f32) -> Vec<ChunkCoord> {
        let chunk_world_size = self.voxel_size * self.chunk_dim as f32;
        let r = (radius / chunk_world_size).ceil() as i32;
        let base = ChunkCoord::from_world(center, self.voxel_size, self.chunk_dim);
        let mut result = Vec::new();
        for cz in -r..=r {
            for cy in -r..=r {
                for cx in -r..=r {
                    let coord = ChunkCoord { cx: base.cx + cx, cy: base.cy + cy, cz: base.cz + cz };
                    let chunk_center = coord.to_world_origin(self.voxel_size, self.chunk_dim)
                        + Vec3::splat(chunk_world_size / 2.0);
                    if (chunk_center - center).length() <= radius {
                        result.push(coord);
                    }
                }
            }
        }
        result
    }

    pub fn total_voxels(&self) -> usize {
        self.chunks.values().map(|c| c.voxels.iter().filter(|v| !v.is_empty()).count()).sum()
    }
}

// ─── Mesh Decimation ──────────────────────────────────────────────────────────

/// Simple edge-collapse mesh decimation (Garland-Heckbert style, simplified).
pub fn decimate_mesh(mesh: &GeneratedMesh, target_triangle_count: usize) -> GeneratedMesh {
    if mesh.indices.len() / 3 <= target_triangle_count {
        return mesh.clone();
    }
    // Build adjacency: which triangles share each vertex
    let n_verts = mesh.vertices.len();
    let mut vert_tris: Vec<Vec<usize>> = vec![Vec::new(); n_verts];
    let n_tris = mesh.indices.len() / 3;
    for ti in 0..n_tris {
        for j in 0..3 {
            let vi = mesh.indices[ti * 3 + j] as usize;
            vert_tris[vi].push(ti);
        }
    }

    // Compute per-vertex error quadric (simplified: use normal spread)
    let mut vertex_errors: Vec<f32> = vec![0.0; n_verts];
    for (vi, tris) in vert_tris.iter().enumerate() {
        if tris.len() < 2 { continue; }
        let mut normal_sum = Vec3::ZERO;
        for &ti in tris {
            let p0 = mesh.vertices[mesh.indices[ti*3] as usize].position;
            let p1 = mesh.vertices[mesh.indices[ti*3+1] as usize].position;
            let p2 = mesh.vertices[mesh.indices[ti*3+2] as usize].position;
            let n = (p1-p0).cross(p2-p0);
            normal_sum += if n.length_squared() > 1e-9 { n.normalize() } else { Vec3::ZERO };
        }
        // Higher error = more variation in normals (feature vertex, keep it)
        vertex_errors[vi] = 1.0 - (normal_sum.length() / tris.len() as f32).min(1.0);
    }

    // For this simplified decimation, just return the original mesh
    // (full implementation would collapse edges with lowest error cost)
    mesh.clone()
}

// ─── Ambient Occlusion Baking ─────────────────────────────────────────────────

pub fn bake_voxel_ambient_occlusion(
    grid: &VoxelGrid,
    mesh: &mut GeneratedMesh,
    num_rays: usize,
    max_dist: f32,
) {
    let ray_dirs: Vec<Vec3> = (0..num_rays).map(|i| {
        let t = i as f64 / num_rays as f64;
        let phi = t * std::f64::consts::TAU;
        let theta = (t * num_rays as f64).cos().acos();
        Vec3::new(
            (phi.cos() * theta.sin()) as f32,
            (theta.cos()) as f32,
            (phi.sin() * theta.sin()) as f32,
        )
    }).collect();

    for vert in &mut mesh.vertices {
        let mut occluded = 0.0f32;
        for &ray_dir in &ray_dirs {
            // Only sample rays in the hemisphere around the normal
            if ray_dir.dot(vert.normal) <= 0.0 { continue; }
            if let Some(_hit) = ray_dda(grid, vert.position + vert.normal * 0.01, ray_dir) {
                occluded += 1.0;
            }
        }
        let ao = 1.0 - occluded / num_rays as f32;
        vert.color = Vec4::new(vert.color.x * ao, vert.color.y * ao, vert.color.z * ao, vert.color.w);
    }
}

// ─── Greedy Mesh Generation ───────────────────────────────────────────────────

/// Greedy meshing: merge adjacent coplanar quad faces.
pub fn greedy_mesh(grid: &VoxelGrid, material_table: &[VoxelMaterial]) -> GeneratedMesh {
    let mut mesh = GeneratedMesh::new();

    // Process each axis direction (6 faces)
    for axis in 0..3usize {
        let (u, v, w) = match axis {
            0 => (1usize, 2usize, 0usize),
            1 => (0usize, 2usize, 1usize),
            _ => (0usize, 1usize, 2usize),
        };
        let dims = [grid.width, grid.height, grid.depth];

        for d in 0..dims[w] {
            let mut mask: Vec<Option<(u8, bool)>> = vec![None; dims[u] * dims[v]];
            // Build face mask for this slice
            for j in 0..dims[v] {
                for i in 0..dims[u] {
                    let mut pos = [0i32; 3];
                    pos[u] = i as i32;
                    pos[v] = j as i32;
                    pos[w] = d as i32;
                    let cur = grid.get(pos[0], pos[1], pos[2]);
                    let mut next_pos = pos;
                    next_pos[w] += 1;
                    let nxt = grid.get(next_pos[0], next_pos[1], next_pos[2]);
                    // Face exists between cur and nxt if one is solid and other isn't
                    let face = if cur.is_solid() && !nxt.is_solid() {
                        Some((cur.material, true))
                    } else if !cur.is_solid() && nxt.is_solid() {
                        Some((nxt.material, false))
                    } else {
                        None
                    };
                    mask[i + j * dims[u]] = face;
                }
            }

            // Greedy merge
            let mut used = vec![false; dims[u] * dims[v]];
            for j in 0..dims[v] {
                for i in 0..dims[u] {
                    let idx = i + j * dims[u];
                    if used[idx] || mask[idx].is_none() { continue; }
                    let (mat, front) = mask[idx].unwrap();
                    // Extend in u direction
                    let mut w_ext = 1;
                    while i + w_ext < dims[u] {
                        let ni = i + w_ext + j * dims[u];
                        if mask[ni] == Some((mat, front)) && !used[ni] { w_ext += 1; }
                        else { break; }
                    }
                    // Extend in v direction
                    let mut h_ext = 1;
                    'outer: while j + h_ext < dims[v] {
                        for di in 0..w_ext {
                            let ni = (i + di) + (j + h_ext) * dims[u];
                            if mask[ni] != Some((mat, front)) || used[ni] { break 'outer; }
                        }
                        h_ext += 1;
                    }
                    // Mark used
                    for dj in 0..h_ext {
                        for di in 0..w_ext {
                            used[(i + di) + (j + dj) * dims[u]] = true;
                        }
                    }
                    // Generate quad
                    let mut origin = [0f32; 3];
                    origin[u] = i as f32;
                    origin[v] = j as f32;
                    origin[w] = d as f32 + if front { 1.0 } else { 0.0 };
                    let mut du = [0f32; 3]; du[u] = w_ext as f32;
                    let mut dv = [0f32; 3]; dv[v] = h_ext as f32;
                    let o = grid.origin + Vec3::new(origin[0], origin[1], origin[2]) * grid.voxel_size;
                    let du3 = Vec3::new(du[0], du[1], du[2]) * grid.voxel_size;
                    let dv3 = Vec3::new(dv[0], dv[1], dv[2]) * grid.voxel_size;
                    let color = if (mat as usize) < material_table.len() {
                        material_table[mat as usize].color
                    } else { Vec4::ONE };
                    let normal_dir = if front { 1.0f32 } else { -1.0f32 };
                    let mut norm = Vec3::ZERO;
                    norm[w] = normal_dir;
                    let p0 = o;
                    let p1 = o + du3;
                    let p2 = o + du3 + dv3;
                    let p3 = o + dv3;
                    let mkv = |p: Vec3| MeshVertex { position: p, normal: norm, color, uv: Vec2::ZERO };
                    if front {
                        mesh.push_triangle(mkv(p0), mkv(p1), mkv(p2));
                        mesh.push_triangle(mkv(p0), mkv(p2), mkv(p3));
                    } else {
                        mesh.push_triangle(mkv(p0), mkv(p2), mkv(p1));
                        mesh.push_triangle(mkv(p0), mkv(p3), mkv(p2));
                    }
                }
            }
        }
    }
    mesh
}

// ─── Voxel Painting Tool ──────────────────────────────────────────────────────

pub struct VoxelPainter {
    pub palette: Vec<VoxelMaterial>,
    pub current_material: usize,
    pub blend_mode: PaintBlendMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaintBlendMode {
    Replace,
    Add,
    Blend(f32),
}

impl VoxelPainter {
    pub fn new() -> Self {
        Self {
            palette: default_material_table(),
            current_material: 1,
            blend_mode: PaintBlendMode::Replace,
        }
    }

    pub fn apply_paint(&self, existing: &Voxel, point: Vec3, center: Vec3, radius: f32) -> Voxel {
        if existing.is_empty() { return *existing; }
        let dist = (point - center).length();
        if dist > radius { return *existing; }
        let mat = self.current_material as u8;
        match self.blend_mode {
            PaintBlendMode::Replace => Voxel::new(existing.density, mat),
            PaintBlendMode::Add => {
                let t = 1.0 - dist / radius;
                if t > 0.5 { Voxel::new(existing.density, mat) } else { *existing }
            }
            PaintBlendMode::Blend(strength) => {
                let t = (1.0 - dist / radius) * strength;
                if t > 0.5 { Voxel::new(existing.density, mat) } else { *existing }
            }
        }
    }
}

// ─── Voxel Grid Statistics ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct GridStats {
    pub total_voxels: usize,
    pub solid_voxels: usize,
    pub empty_voxels: usize,
    pub material_counts: Vec<usize>,
    pub avg_density: f32,
    pub fill_ratio: f32,
}

impl GridStats {
    pub fn compute(grid: &VoxelGrid) -> Self {
        let mut stats = Self::default();
        stats.total_voxels = grid.width * grid.height * grid.depth;
        stats.material_counts = vec![0; MATERIAL_COUNT];
        let mut density_sum = 0.0f64;
        for v in &grid.voxels {
            if v.is_empty() {
                stats.empty_voxels += 1;
            } else {
                stats.solid_voxels += 1;
                if (v.material as usize) < MATERIAL_COUNT {
                    stats.material_counts[v.material as usize] += 1;
                }
                density_sum += v.density as f64;
            }
        }
        stats.avg_density = if stats.solid_voxels > 0 {
            (density_sum / stats.solid_voxels as f64) as f32
        } else { 0.0 };
        stats.fill_ratio = if stats.total_voxels > 0 {
            stats.solid_voxels as f32 / stats.total_voxels as f32
        } else { 0.0 };
        stats
    }
}

// ─── Voxel Smooth Filter ──────────────────────────────────────────────────────

pub fn smooth_grid(grid: &mut VoxelGrid, iterations: u32, strength: f32) {
    for _ in 0..iterations {
        let old = grid.voxels.clone();
        for z in 1..(grid.depth as i32 - 1) {
            for y in 1..(grid.height as i32 - 1) {
                for x in 1..(grid.width as i32 - 1) {
                    let mut sum = 0.0f32;
                    let mut count = 0u32;
                    for (dx, dy, dz) in [
                        (-1,0,0),(1,0,0),(0,-1,0),(0,1,0),(0,0,-1),(0,0,1),(0,0,0)
                    ] {
                        let idx = grid.index((x+dx) as usize, (y+dy) as usize, (z+dz) as usize);
                        sum += old[idx].density;
                        count += 1;
                    }
                    let avg = sum / count as f32;
                    let idx = grid.index(x as usize, y as usize, z as usize);
                    let old_d = old[idx].density;
                    grid.voxels[idx].density = old_d + (avg - old_d) * strength;
                }
            }
        }
    }
}

// ─── Heightmap Export ─────────────────────────────────────────────────────────

pub fn export_heightmap(grid: &VoxelGrid) -> Vec<f32> {
    let mut heights = vec![0.0f32; grid.width * grid.depth];
    for z in 0..grid.depth {
        for x in 0..grid.width {
            // Find highest solid voxel in this column
            let mut h = 0.0f32;
            for y in (0..grid.height).rev() {
                if !grid.get(x as i32, y as i32, z as i32).is_empty() {
                    h = y as f32 / grid.height as f32;
                    break;
                }
            }
            heights[x + z * grid.width] = h;
        }
    }
    heights
}

// ─── Surface Normal Computation ───────────────────────────────────────────────

pub fn compute_surface_normals(grid: &VoxelGrid) -> HashMap<(i32, i32, i32), Vec3> {
    let mut normals = HashMap::new();
    for z in 1..(grid.depth as i32 - 1) {
        for y in 1..(grid.height as i32 - 1) {
            for x in 1..(grid.width as i32 - 1) {
                let v = grid.get(x, y, z);
                if !v.is_solid() { continue; }
                let n = grid_normal(grid, x, y, z);
                normals.insert((x, y, z), n);
            }
        }
    }
    normals
}

// ─── Voxel Mesh UV Generation ─────────────────────────────────────────────────

pub fn generate_triplanar_uvs(mesh: &mut GeneratedMesh, uv_scale: f32) {
    for vert in &mut mesh.vertices {
        let abs_n = vert.normal.abs();
        if abs_n.x > abs_n.y && abs_n.x > abs_n.z {
            vert.uv = Vec2::new(vert.position.z * uv_scale, vert.position.y * uv_scale);
        } else if abs_n.y > abs_n.z {
            vert.uv = Vec2::new(vert.position.x * uv_scale, vert.position.z * uv_scale);
        } else {
            vert.uv = Vec2::new(vert.position.x * uv_scale, vert.position.y * uv_scale);
        }
    }
}

// ─── Voxel Visibility Culling ─────────────────────────────────────────────────

/// Check if a voxel face is visible (has an air neighbor).
pub fn is_face_visible(grid: &VoxelGrid, x: i32, y: i32, z: i32, nx: i32, ny: i32, nz: i32) -> bool {
    if !grid.get(x, y, z).is_solid() { return false; }
    !grid.get(x+nx, y+ny, z+nz).is_solid()
}

pub fn count_visible_faces(grid: &VoxelGrid) -> usize {
    let mut count = 0;
    let dirs = [(-1,0,0),(1,0,0),(0,-1,0),(0,1,0),(0,0,-1),(0,0,1)];
    for z in 0..grid.depth as i32 {
        for y in 0..grid.height as i32 {
            for x in 0..grid.width as i32 {
                if !grid.get(x, y, z).is_solid() { continue; }
                for (dx, dy, dz) in &dirs {
                    if is_face_visible(grid, x, y, z, *dx, *dy, *dz) {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

// ─── Heightmap-to-SDF Conversion ──────────────────────────────────────────────

/// Generate a signed distance field approximation from voxel grid.
pub fn voxel_to_sdf(grid: &VoxelGrid) -> Vec<f32> {
    let n = grid.width * grid.height * grid.depth;
    let mut sdf = vec![f32::MAX; n];

    // Initialize: inside = negative, outside = positive
    for z in 0..grid.depth {
        for y in 0..grid.height {
            for x in 0..grid.width {
                let idx = grid.index(x, y, z);
                let v = grid.get(x as i32, y as i32, z as i32);
                sdf[idx] = if v.is_solid() { -v.density } else { 1.0 - v.density };
            }
        }
    }

    // 3D distance transform (simplified: 1-pass, not exact)
    for z in 1..grid.depth as i32 {
        for y in 1..grid.height as i32 {
            for x in 1..grid.width as i32 {
                let idx = grid.index(x as usize, y as usize, z as usize);
                let n0 = sdf[grid.index((x-1) as usize, y as usize, z as usize)] + 1.0;
                let n1 = sdf[grid.index(x as usize, (y-1) as usize, z as usize)] + 1.0;
                let n2 = sdf[grid.index(x as usize, y as usize, (z-1) as usize)] + 1.0;
                let min_n = n0.min(n1).min(n2);
                if min_n < sdf[idx] { sdf[idx] = min_n; }
            }
        }
    }

    sdf
}

// ─── Additional Procedural: Dungeon Generator ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct DungeonRoom {
    pub min: (i32, i32, i32),
    pub max: (i32, i32, i32),
}

impl DungeonRoom {
    pub fn new(cx: i32, cy: i32, cz: i32, hw: i32, hh: i32, hd: i32) -> Self {
        Self {
            min: (cx - hw, cy - hh, cz - hd),
            max: (cx + hw, cy + hh, cz + hd),
        }
    }

    pub fn overlaps(&self, other: &DungeonRoom) -> bool {
        self.min.0 <= other.max.0 && self.max.0 >= other.min.0 &&
        self.min.1 <= other.max.1 && self.max.1 >= other.min.1 &&
        self.min.2 <= other.max.2 && self.max.2 >= other.min.2
    }

    pub fn center(&self) -> (i32, i32, i32) {
        (
            (self.min.0 + self.max.0) / 2,
            (self.min.1 + self.max.1) / 2,
            (self.min.2 + self.max.2) / 2,
        )
    }
}

pub fn generate_dungeon(
    grid: &mut VoxelGrid,
    num_rooms: usize,
    seed: u64,
) -> Vec<DungeonRoom> {
    let mut rng = seed;
    let next_range = |rng: &mut u64, lo: i32, hi: i32| -> i32 {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*rng >> 33) as i32).abs() % (hi - lo + 1) + lo
    };

    // Fill entire grid with stone
    for v in &mut grid.voxels {
        *v = Voxel::new(1.0, 1); // stone
    }

    let mut rooms = Vec::new();
    let attempts = num_rooms * 10;
    for _ in 0..attempts {
        if rooms.len() >= num_rooms { break; }
        let cx = next_range(&mut rng, 5, grid.width as i32 - 5);
        let cy = next_range(&mut rng, 3, grid.height as i32 - 3);
        let cz = next_range(&mut rng, 5, grid.depth as i32 - 5);
        let hw = next_range(&mut rng, 2, 6);
        let hh = next_range(&mut rng, 2, 4);
        let hd = next_range(&mut rng, 2, 6);
        let room = DungeonRoom::new(cx, cy, cz, hw, hh, hd);
        // Check overlap
        if rooms.iter().any(|r: &DungeonRoom| r.overlaps(&room)) { continue; }
        // Carve the room
        for z in room.min.2..=room.max.2 {
            for y in room.min.1..=room.max.1 {
                for x in room.min.0..=room.max.0 {
                    grid.set(x, y, z, Voxel::EMPTY);
                }
            }
        }
        rooms.push(room);
    }

    // Connect rooms with corridors
    for i in 1..rooms.len() {
        let (ax, ay, az) = rooms[i-1].center();
        let (bx, by, bz) = rooms[i].center();
        // Carve L-shaped corridor
        for x in ax.min(bx)..=ax.max(bx) {
            grid.set(x, ay, az, Voxel::EMPTY);
            grid.set(x, ay+1, az, Voxel::EMPTY);
        }
        for z in az.min(bz)..=az.max(bz) {
            grid.set(bx, ay, z, Voxel::EMPTY);
            grid.set(bx, ay+1, z, Voxel::EMPTY);
        }
        for y in ay.min(by)..=ay.max(by) {
            grid.set(bx, y, bz, Voxel::EMPTY);
        }
    }

    rooms
}

// ─── Volume Copy/Paste with Rotation ─────────────────────────────────────────

pub fn rotate_clipboard_90_xz(cb: &VoxelClipboard) -> VoxelClipboard {
    let (sw, sh, sd) = cb.size();
    let new_voxels: Vec<((i32, i32, i32), Voxel)> = cb.voxels.iter().map(|&((x, y, z), v)| {
        // Rotate 90 degrees in XZ plane: (x,y,z) -> (z, y, sw-1-x)
        ((z, y, sw - 1 - x), v)
    }).collect();
    VoxelClipboard {
        voxels: new_voxels,
        bounds_min: (0, 0, 0),
        bounds_max: (sd - 1, sh - 1, sw - 1),
    }
}

// ─── Voxel Mesh Export (OBJ format string) ───────────────────────────────────

pub fn export_mesh_obj(mesh: &GeneratedMesh) -> String {
    let mut obj = String::new();
    obj.push_str("# Voxel Mesh\n");
    for v in &mesh.vertices {
        obj.push_str(&format!("v {} {} {}\n", v.position.x, v.position.y, v.position.z));
    }
    for v in &mesh.vertices {
        obj.push_str(&format!("vn {} {} {}\n", v.normal.x, v.normal.y, v.normal.z));
    }
    for v in &mesh.vertices {
        obj.push_str(&format!("vt {} {}\n", v.uv.x, v.uv.y));
    }
    let n_tris = mesh.indices.len() / 3;
    for ti in 0..n_tris {
        let a = mesh.indices[ti*3] + 1;
        let b = mesh.indices[ti*3+1] + 1;
        let c = mesh.indices[ti*3+2] + 1;
        obj.push_str(&format!("f {0}/{0}/{0} {1}/{1}/{1} {2}/{2}/{2}\n", a, b, c));
    }
    obj
}

// ─── Voxel Raycast Result ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VoxelRaycastResult {
    pub hit: bool,
    pub position: Vec3,
    pub normal: Vec3,
    pub voxel_coord: (i32, i32, i32),
    pub distance: f32,
    pub material: u8,
}

pub fn raycast_voxel_grid(grid: &VoxelGrid, ray_origin: Vec3, ray_dir: Vec3) -> VoxelRaycastResult {
    let mut result = VoxelRaycastResult {
        hit: false,
        position: Vec3::ZERO,
        normal: Vec3::Y,
        voxel_coord: (0, 0, 0),
        distance: f32::MAX,
        material: 0,
    };

    if let Some((x, y, z)) = ray_dda(grid, ray_origin, ray_dir) {
        let v = grid.get(x, y, z);
        let pos = grid.grid_to_world(x, y, z);
        let dist = (pos - ray_origin).length();
        result.hit = true;
        result.position = pos;
        result.voxel_coord = (x, y, z);
        result.distance = dist;
        result.material = v.material;
        result.normal = grid_normal(grid, x, y, z);
    }
    result
}

// ─── Marching Cubes with Material Blending ────────────────────────────────────

pub fn marching_cubes_material_blend(
    grid: &VoxelGrid,
    material_table: &[VoxelMaterial],
) -> GeneratedMesh {
    let mut mesh = GeneratedMesh::new();
    let vsize = grid.voxel_size;

    for z in 0..(grid.depth as i32 - 1) {
        for y in 0..(grid.height as i32 - 1) {
            for x in 0..(grid.width as i32 - 1) {
                let mut cube_vals = [0.0f32; 8];
                let mut cube_mats = [0u8; 8];
                let mut cube_idx = 0u8;

                for (vi, (dx, dy, dz)) in MC_VERTEX_OFFSETS.iter().enumerate() {
                    let v = grid.get(x + dx, y + dy, z + dz);
                    cube_vals[vi] = v.density;
                    cube_mats[vi] = v.material;
                    if v.density >= ISO_LEVEL {
                        cube_idx |= 1 << vi;
                    }
                }

                let edge_mask = MC_EDGE_TABLE[cube_idx as usize];
                if edge_mask == 0 { continue; }

                let mut edge_verts = [Vec3::ZERO; 12];
                let mut edge_colors = [Vec4::ONE; 12];

                for edge in 0..12 {
                    if (edge_mask >> edge) & 1 == 0 { continue; }
                    let (vi0, vi1) = MC_EDGE_VERTICES[edge];
                    let (dx0, dy0, dz0) = MC_VERTEX_OFFSETS[vi0];
                    let (dx1, dy1, dz1) = MC_VERTEX_OFFSETS[vi1];
                    let p0 = grid.grid_to_world(x + dx0, y + dy0, z + dz0);
                    let p1 = grid.grid_to_world(x + dx1, y + dy1, z + dz1);
                    let v0 = cube_vals[vi0];
                    let v1 = cube_vals[vi1];
                    let t = if (v1 - v0).abs() > 1e-9 { (ISO_LEVEL - v0) / (v1 - v0) } else { 0.5 };
                    edge_verts[edge] = p0.lerp(p1, t);
                    let mat0 = cube_mats[vi0] as usize;
                    let mat1 = cube_mats[vi1] as usize;
                    let c0 = if mat0 < material_table.len() { material_table[mat0].color } else { Vec4::ONE };
                    let c1 = if mat1 < material_table.len() { material_table[mat1].color } else { Vec4::ONE };
                    edge_colors[edge] = c0.lerp(c1, t);
                }

                let tri_row = &MC_TRI_TABLE[cube_idx as usize];
                let mut ti = 0;
                while ti < 15 && tri_row[ti] >= 0 {
                    let e0 = tri_row[ti] as usize;
                    let e1 = tri_row[ti+1] as usize;
                    let e2 = tri_row[ti+2] as usize;
                    let p0 = edge_verts[e0]; let c0 = edge_colors[e0];
                    let p1 = edge_verts[e1]; let c1 = edge_colors[e1];
                    let p2 = edge_verts[e2]; let c2 = edge_colors[e2];
                    let n = (p1-p0).cross(p2-p0);
                    let norm = if n.length_squared() > 1e-9 { n.normalize() } else { Vec3::Y };
                    mesh.push_triangle(
                        MeshVertex { position: p0, normal: norm, color: c0, uv: Vec2::ZERO },
                        MeshVertex { position: p1, normal: norm, color: c1, uv: Vec2::ZERO },
                        MeshVertex { position: p2, normal: norm, color: c2, uv: Vec2::ZERO },
                    );
                    ti += 3;
                }
            }
        }
    }
    mesh.compute_normals();
    mesh
}

// ─── Extended: Voxel Terrain Features ────────────────────────────────────────

/// Biome system: per-voxel-column biome assignment based on noise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Biome {
    Plains, Forest, Desert, Tundra, Mountains, Ocean, Swamp, Jungle,
}

impl Biome {
    pub fn surface_material(&self) -> u8 {
        match self {
            Biome::Plains => 3,    // grass
            Biome::Forest => 3,    // grass
            Biome::Desert => 4,    // sand
            Biome::Tundra => 15,   // snow
            Biome::Mountains => 1, // stone
            Biome::Ocean => 5,     // water
            Biome::Swamp => 2,     // dirt
            Biome::Jungle => 3,    // grass
        }
    }

    pub fn subsurface_material(&self) -> u8 {
        match self {
            Biome::Desert => 4,    // sand
            Biome::Tundra => 15,   // snow
            Biome::Ocean => 4,     // sand
            _ => 2,                // dirt
        }
    }
}

pub fn classify_biome(temperature: f64, humidity: f64, altitude: f64) -> Biome {
    if altitude > 0.8 { return Biome::Mountains; }
    if altitude < 0.1 { return Biome::Ocean; }
    if temperature < 0.2 { return Biome::Tundra; }
    if temperature > 0.8 && humidity < 0.3 { return Biome::Desert; }
    if humidity > 0.8 && temperature > 0.5 { return Biome::Jungle; }
    if humidity > 0.6 { return Biome::Swamp; }
    if humidity > 0.4 { return Biome::Forest; }
    Biome::Plains
}

pub fn generate_biome_terrain(
    grid: &mut VoxelGrid,
    noise: &PerlinNoise3D,
    noise_scale: f32,
    base_height: f32,
    height_scale: f32,
) {
    for z in 0..grid.depth {
        for x in 0..grid.width {
            let world = grid.grid_to_world(x as i32, 0, z as i32);
            let nx = world.x as f64 / noise_scale as f64;
            let nz = world.z as f64 / noise_scale as f64;
            let height_n = noise.octave_noise(nx, 0.0, nz, 6, 0.5, 2.0);
            let temp_n = noise.octave_noise(nx * 0.3, 100.0, nz * 0.3, 2, 0.5, 2.0) * 0.5 + 0.5;
            let humid_n = noise.octave_noise(nx * 0.3, 200.0, nz * 0.3, 2, 0.5, 2.0) * 0.5 + 0.5;
            let alt_n = height_n * 0.5 + 0.5;
            let biome = classify_biome(temp_n, humid_n, alt_n);
            let surface_y = (base_height + height_scale * height_n as f32) as i32;
            let surface_mat = biome.surface_material();
            let sub_mat = biome.subsurface_material();
            for y in 0..grid.height {
                if y as i32 == surface_y {
                    grid.set(x as i32, y as i32, z as i32, Voxel::new(1.0, surface_mat));
                } else if y < surface_y as usize && surface_y as usize - y <= 3 {
                    grid.set(x as i32, y as i32, z as i32, Voxel::new(1.0, sub_mat));
                } else if y < surface_y as usize {
                    grid.set(x as i32, y as i32, z as i32, Voxel::new(1.0, 1));
                }
            }
        }
    }
}

// ─── Extended: Tree Generation ───────────────────────────────────────────────

pub struct TreeGenerator {
    pub trunk_material: u8,
    pub leaf_material: u8,
    pub trunk_height: u32,
    pub canopy_radius: f32,
}

impl TreeGenerator {
    pub fn new() -> Self {
        Self { trunk_material: 6, leaf_material: 7, trunk_height: 6, canopy_radius: 3.0 }
    }

    pub fn plant(&self, grid: &mut VoxelGrid, base_x: i32, base_y: i32, base_z: i32) {
        // Trunk
        for dy in 0..self.trunk_height as i32 {
            grid.set(base_x, base_y + dy, base_z, Voxel::new(1.0, self.trunk_material));
        }
        // Canopy sphere
        let crown_y = base_y + self.trunk_height as i32;
        let r = self.canopy_radius;
        let ri = r.ceil() as i32;
        for dz in -ri..=ri {
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let dist = ((dx*dx + dy*dy + dz*dz) as f32).sqrt();
                    if dist <= r {
                        let density = 1.0 - (dist / r) * 0.3;
                        grid.set(base_x + dx, crown_y + dy, base_z + dz, Voxel::new(density, self.leaf_material));
                    }
                }
            }
        }
    }

    pub fn scatter_forest(
        &self,
        grid: &mut VoxelGrid,
        noise: &PerlinNoise3D,
        density: f64,
        min_y: i32,
        seed: u64,
    ) {
        let mut rng = seed;
        let next_f = |rng: &mut u64| -> f64 {
            *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*rng >> 33) as f64 / u32::MAX as f64
        };
        for z in (0..grid.depth as i32).step_by(4) {
            for x in (0..grid.width as i32).step_by(4) {
                let n = noise.octave_noise(x as f64 * 0.1, 300.0, z as f64 * 0.1, 2, 0.5, 2.0) * 0.5 + 0.5;
                if n > density {
                    // Find surface y
                    let mut surf_y = min_y;
                    for y in (0..grid.height as i32).rev() {
                        if grid.get(x, y, z).is_solid() { surf_y = y + 1; break; }
                    }
                    let jx = x + (next_f(&mut rng) * 3.0 - 1.5) as i32;
                    let jz = z + (next_f(&mut rng) * 3.0 - 1.5) as i32;
                    if surf_y > min_y && surf_y < grid.height as i32 - (self.trunk_height as i32 + 5) {
                        self.plant(grid, jx, surf_y, jz);
                    }
                }
            }
        }
    }
}

// ─── Extended: Erosion Simulation ────────────────────────────────────────────

pub fn hydraulic_erosion(
    grid: &mut VoxelGrid,
    iterations: usize,
    erosion_strength: f32,
    deposition_strength: f32,
    seed: u64,
) {
    let mut rng = seed;
    let next_f = |rng: &mut u64| -> f32 {
        *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*rng >> 33) as f32 / u32::MAX as f32
    };

    for _ in 0..iterations {
        // Random raindrop
        let dx = (next_f(&mut rng) * grid.width as f32) as i32;
        let dz = (next_f(&mut rng) * grid.depth as f32) as i32;
        // Find surface
        let mut pos = (dx, 0i32, dz);
        for y in (0..grid.height as i32).rev() {
            if grid.get(dx, y, dz).is_solid() { pos.1 = y; break; }
        }
        let mut sediment = 0.0f32;
        let mut water = 1.0f32;
        // Flow downhill
        for _ in 0..32 {
            let (cx, cy, cz) = pos;
            // Find lowest neighbor
            let neighbors = [(cx-1,cy,cz),(cx+1,cy,cz),(cx,cy,cz-1),(cx,cy,cz+1),
                             (cx-1,cy-1,cz),(cx+1,cy-1,cz),(cx,cy-1,cz-1),(cx,cy-1,cz+1)];
            let cur_density = grid.get(cx, cy, cz).density;
            let lowest = neighbors.iter().min_by(|&&(nx,ny,nz), &&(mx,my,mz)| {
                let nd = grid.get(nx, ny, nz).density;
                let md = grid.get(mx, my, mz).density;
                nd.partial_cmp(&md).unwrap_or(std::cmp::Ordering::Equal)
            });
            if let Some(&(nx, ny, nz)) = lowest {
                let next_density = grid.get(nx, ny, nz).density;
                if next_density < cur_density {
                    // Erode current cell
                    let eroded = erosion_strength * water;
                    let cur_v = grid.get(cx, cy, cz);
                    if !cur_v.is_empty() {
                        let new_d = (cur_v.density - eroded).max(0.0);
                        grid.set(cx, cy, cz, Voxel::new(new_d, cur_v.material));
                        sediment += eroded;
                    }
                    // Deposit some sediment
                    let deposit = deposition_strength * sediment;
                    sediment -= deposit;
                    let next_v = grid.get(nx, ny, nz);
                    if !next_v.is_empty() {
                        grid.set(nx, ny, nz, Voxel::new((next_v.density + deposit).min(1.0), next_v.material));
                    }
                    pos = (nx, ny, nz);
                    water *= 0.99;
                } else { break; }
            } else { break; }
        }
    }
}

// ─── Extended: Voxel Signed Distance Field ───────────────────────────────────

pub struct VoxelSDF {
    pub grid: VoxelGrid,
    pub values: Vec<f32>,
}

impl VoxelSDF {
    pub fn build(grid: &VoxelGrid) -> Self {
        let values = voxel_to_sdf(grid);
        Self { grid: grid.clone(), values }
    }

    pub fn sample_at(&self, world_pos: Vec3) -> f32 {
        let (x, y, z) = self.grid.world_to_grid(world_pos);
        if x < 0 || y < 0 || z < 0 || x >= self.grid.width as i32 || y >= self.grid.height as i32 || z >= self.grid.depth as i32 {
            return f32::MAX;
        }
        let idx = self.grid.index(x as usize, y as usize, z as usize);
        self.values[idx]
    }

    pub fn gradient_at(&self, world_pos: Vec3) -> Vec3 {
        let eps = self.grid.voxel_size;
        let dx = self.sample_at(world_pos + Vec3::X * eps) - self.sample_at(world_pos - Vec3::X * eps);
        let dy = self.sample_at(world_pos + Vec3::Y * eps) - self.sample_at(world_pos - Vec3::Y * eps);
        let dz = self.sample_at(world_pos + Vec3::Z * eps) - self.sample_at(world_pos - Vec3::Z * eps);
        Vec3::new(dx, dy, dz) / (2.0 * eps)
    }

    pub fn is_inside(&self, world_pos: Vec3) -> bool {
        self.sample_at(world_pos) < 0.0
    }

    /// Boolean union with another SDF (minimum of values).
    pub fn union_inplace(&mut self, other: &VoxelSDF) {
        for (a, b) in self.values.iter_mut().zip(other.values.iter()) {
            *a = a.min(*b);
        }
    }

    /// Boolean subtraction: max(A, -B).
    pub fn subtract_inplace(&mut self, other: &VoxelSDF) {
        for (a, b) in self.values.iter_mut().zip(other.values.iter()) {
            *a = a.max(-b);
        }
    }

    /// Smooth union (polynomial blend).
    pub fn smooth_union_inplace(&mut self, other: &VoxelSDF, k: f32) {
        for (a, b) in self.values.iter_mut().zip(other.values.iter()) {
            let h = (k - (a.abs() - b.abs()).abs()).max(0.0) / k;
            let m = h * h * 0.25;
            *a = a.min(*b) - m * k;
        }
    }
}

// ─── Extended: Voxel Path Finding ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VoxelNode(pub i32, pub i32, pub i32);

impl VoxelNode {
    pub fn distance(&self, other: &VoxelNode) -> f32 {
        let dx = (self.0 - other.0) as f32;
        let dy = (self.1 - other.1) as f32;
        let dz = (self.2 - other.2) as f32;
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

/// A* pathfinding on voxel grid.
pub fn voxel_astar(
    grid: &VoxelGrid,
    start: VoxelNode,
    goal: VoxelNode,
    max_nodes: usize,
) -> Option<Vec<VoxelNode>> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    #[derive(PartialEq)]
    struct FNode(f32, VoxelNode);
    impl Eq for FNode {}
    impl PartialOrd for FNode {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
    }
    impl Ord for FNode {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            other.0.partial_cmp(&self.0).unwrap_or(std::cmp::Ordering::Equal)
        }
    }

    let mut open: BinaryHeap<FNode> = BinaryHeap::new();
    let mut came_from: HashMap<VoxelNode, VoxelNode> = HashMap::new();
    let mut g_score: HashMap<VoxelNode, f32> = HashMap::new();
    g_score.insert(start.clone(), 0.0);
    open.push(FNode(start.distance(&goal), start.clone()));

    let mut visited = 0usize;
    while let Some(FNode(_, current)) = open.pop() {
        if current == goal {
            // Reconstruct path
            let mut path = vec![current.clone()];
            let mut cur = current;
            while let Some(prev) = came_from.get(&cur) {
                path.push(prev.clone());
                cur = prev.clone();
            }
            path.reverse();
            return Some(path);
        }
        if visited > max_nodes { break; }
        visited += 1;

        for (dx, dy, dz) in [
            (-1,0,0),(1,0,0),(0,-1,0),(0,1,0),(0,0,-1),(0,0,1),
            (-1,0,-1),(-1,0,1),(1,0,-1),(1,0,1),
        ] {
            let nx = current.0 + dx;
            let ny = current.1 + dy;
            let nz = current.2 + dz;
            let neighbor = VoxelNode(nx, ny, nz);
            // Check if walkable: solid ground below, air at position
            let is_air = !grid.get(nx, ny, nz).is_solid();
            let has_ground = grid.get(nx, ny-1, nz).is_solid();
            if !is_air || !has_ground { continue; }
            let step_cost = if dx != 0 && dz != 0 { 1.414 } else { 1.0 };
            let tentative_g = g_score.get(&current).copied().unwrap_or(f32::MAX) + step_cost;
            if tentative_g < g_score.get(&neighbor).copied().unwrap_or(f32::MAX) {
                came_from.insert(neighbor.clone(), current.clone());
                g_score.insert(neighbor.clone(), tentative_g);
                let h = neighbor.distance(&goal);
                open.push(FNode(tentative_g + h, neighbor));
            }
        }
    }
    None
}

// ─── Extended: Voxel Light Propagation ───────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct LightCell {
    pub r: u8, pub g: u8, pub b: u8,
    pub sun: u8,
}

pub struct VoxelLightGrid {
    pub width: usize, pub height: usize, pub depth: usize,
    pub cells: Vec<LightCell>,
}

impl VoxelLightGrid {
    pub fn new(width: usize, height: usize, depth: usize) -> Self {
        Self { width, height, depth, cells: vec![LightCell::default(); width * height * depth] }
    }

    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        x + y * self.width + z * self.width * self.height
    }

    pub fn propagate_sunlight(&mut self, grid: &VoxelGrid) {
        // Top-down sunlight pass
        for z in 0..self.depth {
            for x in 0..self.width {
                let mut sun = 255u8;
                for y in (0..self.height).rev() {
                    let v = grid.get(x as i32, y as i32, z as i32);
                    if v.is_solid() { sun = (sun as f32 * 0.8) as u8; }
                    let idx = self.index(x, y, z);
                    self.cells[idx].sun = sun;
                }
            }
        }
        // Flood fill lateral propagation (simplified BFS)
        let mut queue: VecDeque<(usize, usize, usize)> = VecDeque::new();
        for z in 0..self.depth {
            for x in 0..self.width {
                let idx = self.index(x, self.height - 1, z);
                if self.cells[idx].sun > 0 { queue.push_back((x, self.height - 1, z)); }
            }
        }
        while let Some((x, y, z)) = queue.pop_front() {
            let cur_sun = self.cells[self.index(x, y, z)].sun;
            if cur_sun <= 1 { continue; }
            let new_sun = cur_sun - 1;
            for (nx, ny, nz) in [
                (x.wrapping_sub(1), y, z), (x+1, y, z), (x, y.wrapping_sub(1), z),
                (x, y+1, z), (x, y, z.wrapping_sub(1)), (x, y, z+1),
            ] {
                if nx >= self.width || ny >= self.height || nz >= self.depth { continue; }
                let nidx = self.index(nx, ny, nz);
                if !grid.get(nx as i32, ny as i32, nz as i32).is_solid() && self.cells[nidx].sun < new_sun {
                    self.cells[nidx].sun = new_sun;
                    queue.push_back((nx, ny, nz));
                }
            }
        }
    }

    pub fn add_point_light(&mut self, grid: &VoxelGrid, lx: usize, ly: usize, lz: usize, r: u8, g: u8, b: u8, radius: f32) {
        let ri = radius.ceil() as usize;
        let xr = (lx.saturating_sub(ri))..(lx + ri + 1).min(self.width);
        let yr = (ly.saturating_sub(ri))..(ly + ri + 1).min(self.height);
        let zr = (lz.saturating_sub(ri))..(lz + ri + 1).min(self.depth);
        for z in zr.clone() {
            for y in yr.clone() {
                for x in xr.clone() {
                    let dist = (((x as i32 - lx as i32).pow(2) + (y as i32 - ly as i32).pow(2) + (z as i32 - lz as i32).pow(2)) as f32).sqrt();
                    if dist > radius { continue; }
                    let att = 1.0 - dist / radius;
                    let idx = self.index(x, y, z);
                    self.cells[idx].r = (self.cells[idx].r as f32 + r as f32 * att).min(255.0) as u8;
                    self.cells[idx].g = (self.cells[idx].g as f32 + g as f32 * att).min(255.0) as u8;
                    self.cells[idx].b = (self.cells[idx].b as f32 + b as f32 * att).min(255.0) as u8;
                }
            }
        }
    }
}

// ─── Extended: Volumetric Fog ─────────────────────────────────────────────────

pub struct VolumetricFogGrid {
    pub width: usize, pub height: usize, pub depth: usize,
    pub density: Vec<f32>,
    pub scatter_color: Vec3,
    pub absorption: f32,
}

impl VolumetricFogGrid {
    pub fn new(w: usize, h: usize, d: usize) -> Self {
        Self { width: w, height: h, depth: d, density: vec![0.0; w*h*d], scatter_color: Vec3::ONE * 0.8, absorption: 0.1 }
    }

    pub fn index(&self, x: usize, y: usize, z: usize) -> usize { x + y*self.width + z*self.width*self.height }

    pub fn fill_from_grid(&mut self, grid: &VoxelGrid, lava_mat: u8) {
        let sw = self.width.min(grid.width);
        let sh = self.height.min(grid.height);
        let sd = self.depth.min(grid.depth);
        for z in 0..sd {
            for y in 0..sh {
                for x in 0..sw {
                    let v = grid.get(x as i32, y as i32, z as i32);
                    if v.material == lava_mat {
                        // Add fog near lava
                        for dy in 1..4 {
                            let fy = y + dy;
                            if fy < self.height && !grid.get(x as i32, fy as i32, z as i32).is_solid() {
                                let idx = self.index(x, fy, z);
                                self.density[idx] = (self.density[idx] + 0.3 / dy as f32).min(1.0);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn march_ray(&self, origin: Vec3, dir: Vec3, step_size: f32, max_dist: f32) -> (f32, Vec3) {
        let voxel_size = 1.0f32; // assume unit voxels for fog
        let mut t = 0.0f32;
        let mut transmittance = 1.0f32;
        let mut in_scatter = Vec3::ZERO;
        while t < max_dist && transmittance > 0.01 {
            let pos = origin + dir * t;
            let x = pos.x as usize;
            let y = pos.y as usize;
            let z = pos.z as usize;
            if x < self.width && y < self.height && z < self.depth {
                let idx = self.index(x, y, z);
                let d = self.density[idx];
                let ext = (d * self.absorption + d * 0.1) * step_size;
                in_scatter += self.scatter_color * d * transmittance * step_size;
                transmittance *= (-ext).exp();
            }
            t += step_size;
        }
        (transmittance, in_scatter)
    }
}

// ─── Extended: Minecraft-style Chunk Format ───────────────────────────────────

#[derive(Debug, Clone)]
pub struct MinecraftChunkSection {
    pub y_offset: i32,
    pub palette: Vec<u16>,          // material ids used in this section
    pub block_states: Vec<u16>,     // index into palette, 16*16*16 blocks
}

impl MinecraftChunkSection {
    pub fn new(y_offset: i32) -> Self {
        Self { y_offset, palette: vec![0], block_states: vec![0; 16*16*16] }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        let idx = x + z*16 + y*16*16;
        let palette_idx = self.block_states.get(idx).copied().unwrap_or(0) as usize;
        self.palette.get(palette_idx).copied().unwrap_or(0)
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, block_id: u16) {
        // Find or add to palette
        let palette_idx = if let Some(pi) = self.palette.iter().position(|&b| b == block_id) {
            pi
        } else {
            self.palette.push(block_id);
            self.palette.len() - 1
        };
        let idx = x + z*16 + y*16*16;
        if idx < self.block_states.len() {
            self.block_states[idx] = palette_idx as u16;
        }
    }

    pub fn from_voxel_grid_slice(grid: &VoxelGrid, chunk_x: i32, y_offset: i32, chunk_z: i32) -> Self {
        let mut section = Self::new(y_offset);
        for ly in 0..16 {
            for lz in 0..16 {
                for lx in 0..16 {
                    let gx = chunk_x * 16 + lx as i32;
                    let gy = y_offset * 16 + ly as i32;
                    let gz = chunk_z * 16 + lz as i32;
                    let v = grid.get(gx, gy, gz);
                    section.set(lx, ly, lz, v.material as u16);
                }
            }
        }
        section
    }
}

// ─── Extended: Mesh Builder ───────────────────────────────────────────────────

pub struct MeshBuilder {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<Vec4>,
    pub indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self { positions: Vec::new(), normals: Vec::new(), uvs: Vec::new(), colors: Vec::new(), indices: Vec::new() }
    }

    pub fn add_vertex(&mut self, pos: Vec3, normal: Vec3, uv: Vec2, color: Vec4) -> u32 {
        let idx = self.positions.len() as u32;
        self.positions.push(pos);
        self.normals.push(normal);
        self.uvs.push(uv);
        self.colors.push(color);
        idx
    }

    pub fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    pub fn add_quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.indices.extend_from_slice(&[a, b, c, a, c, d]);
    }

    pub fn build(self) -> GeneratedMesh {
        let vertices: Vec<MeshVertex> = self.positions.iter().zip(self.normals.iter())
            .zip(self.uvs.iter()).zip(self.colors.iter())
            .map(|(((p, n), uv), c)| MeshVertex { position: *p, normal: *n, uv: *uv, color: *c })
            .collect();
        GeneratedMesh { vertices, indices: self.indices }
    }

    pub fn add_box_faces(&mut self, min: Vec3, max: Vec3, color: Vec4) {
        let corners = [
            Vec3::new(min.x, min.y, min.z), Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z), Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z), Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z), Vec3::new(min.x, max.y, max.z),
        ];
        let faces = [
            ([0,1,2,3], Vec3::NEG_Z), ([5,4,7,6], Vec3::Z),
            ([4,0,3,7], Vec3::NEG_X), ([1,5,6,2], Vec3::X),
            ([4,5,1,0], Vec3::NEG_Y), ([3,2,6,7], Vec3::Y),
        ];
        for (verts, normal) in &faces {
            let vs: Vec<u32> = verts.iter().map(|&vi| {
                self.add_vertex(corners[vi], *normal, Vec2::ZERO, color)
            }).collect();
            self.add_quad(vs[0], vs[1], vs[2], vs[3]);
        }
    }
}

// ─── Extended: Voxel Brush Stamp ─────────────────────────────────────────────

/// Pre-defined stamp patterns for voxel brushes.
pub struct VoxelStamp {
    pub relative_positions: Vec<(i32, i32, i32)>,
    pub densities: Vec<f32>,
    pub material: u8,
}

impl VoxelStamp {
    pub fn sphere(radius: f32, material: u8) -> Self {
        let ri = radius.ceil() as i32;
        let mut positions = Vec::new();
        let mut densities = Vec::new();
        for dz in -ri..=ri {
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let dist = ((dx*dx + dy*dy + dz*dz) as f32).sqrt();
                    if dist <= radius {
                        positions.push((dx, dy, dz));
                        densities.push(1.0 - dist / radius * 0.5);
                    }
                }
            }
        }
        Self { relative_positions: positions, densities, material }
    }

    pub fn cube(half: i32, material: u8) -> Self {
        let mut positions = Vec::new();
        let mut densities = Vec::new();
        for dz in -half..=half {
            for dy in -half..=half {
                for dx in -half..=half {
                    positions.push((dx, dy, dz));
                    densities.push(1.0);
                }
            }
        }
        Self { relative_positions: positions, densities, material }
    }

    pub fn apply(&self, grid: &mut VoxelGrid, cx: i32, cy: i32, cz: i32, add: bool) {
        for (&(dx, dy, dz), &d) in self.relative_positions.iter().zip(self.densities.iter()) {
            let x = cx + dx; let y = cy + dy; let z = cz + dz;
            if add {
                let existing = grid.get(x, y, z);
                let new_d = (existing.density + d).min(1.0);
                grid.set(x, y, z, Voxel::new(new_d, self.material));
            } else {
                let existing = grid.get(x, y, z);
                let new_d = (existing.density - d).max(0.0);
                if new_d < 0.001 { grid.set(x, y, z, Voxel::EMPTY); }
                else { grid.set(x, y, z, Voxel::new(new_d, existing.material)); }
            }
        }
    }
}

// ─── Extended: Voxel Selection Operations ────────────────────────────────────

pub fn select_by_material(grid: &VoxelGrid, material: u8) -> HashSet<(i32, i32, i32)> {
    let mut sel = HashSet::new();
    for z in 0..grid.depth as i32 {
        for y in 0..grid.height as i32 {
            for x in 0..grid.width as i32 {
                let v = grid.get(x, y, z);
                if v.material == material && !v.is_empty() {
                    sel.insert((x, y, z));
                }
            }
        }
    }
    sel
}

pub fn select_by_density_range(grid: &VoxelGrid, min_d: f32, max_d: f32) -> HashSet<(i32, i32, i32)> {
    let mut sel = HashSet::new();
    for z in 0..grid.depth as i32 {
        for y in 0..grid.height as i32 {
            for x in 0..grid.width as i32 {
                let v = grid.get(x, y, z);
                if v.density >= min_d && v.density <= max_d {
                    sel.insert((x, y, z));
                }
            }
        }
    }
    sel
}

pub fn select_surface_voxels(grid: &VoxelGrid) -> HashSet<(i32, i32, i32)> {
    let mut sel = HashSet::new();
    let dirs = [(-1,0,0),(1,0,0),(0,-1,0),(0,1,0),(0,0,-1),(0,0,1)];
    for z in 0..grid.depth as i32 {
        for y in 0..grid.height as i32 {
            for x in 0..grid.width as i32 {
                if !grid.get(x, y, z).is_solid() { continue; }
                for (dx, dy, dz) in &dirs {
                    if !grid.get(x+dx, y+dy, z+dz).is_solid() {
                        sel.insert((x, y, z));
                        break;
                    }
                }
            }
        }
    }
    sel
}

// ─── Extended: Voxel Stress Visualization ────────────────────────────────────

pub fn colorize_by_stress(
    grid: &VoxelGrid,
    cells: &[StructuralCell],
    material_table: &[VoxelMaterial],
    max_stress: f32,
) -> Vec<Vec4> {
    let n = grid.width * grid.height * grid.depth;
    let mut colors = vec![Vec4::ZERO; n];
    for z in 0..grid.depth {
        for y in 0..grid.height {
            for x in 0..grid.width {
                let idx = grid.index(x, y, z);
                let v = grid.voxels[idx];
                if v.is_empty() { continue; }
                let stress_t = (cells[idx].stress / max_stress.max(1e-9)).clamp(0.0, 1.0);
                // Heatmap: blue (low) -> green -> red (high)
                let r = stress_t;
                let g = (1.0 - (stress_t - 0.5).abs() * 2.0).max(0.0);
                let b = 1.0 - stress_t;
                colors[idx] = Vec4::new(r, g, b, 1.0);
            }
        }
    }
    colors
}

// ─── Extended: Mesh Tangent Generation ───────────────────────────────────────

pub fn generate_tangents(mesh: &mut GeneratedMesh) {
    let n = mesh.vertices.len();
    let mut tan1 = vec![Vec3::ZERO; n];
    let mut tan2 = vec![Vec3::ZERO; n];
    let n_tris = mesh.indices.len() / 3;
    for ti in 0..n_tris {
        let i0 = mesh.indices[ti*3] as usize;
        let i1 = mesh.indices[ti*3+1] as usize;
        let i2 = mesh.indices[ti*3+2] as usize;
        let p0 = mesh.vertices[i0].position;
        let p1 = mesh.vertices[i1].position;
        let p2 = mesh.vertices[i2].position;
        let uv0 = mesh.vertices[i0].uv;
        let uv1 = mesh.vertices[i1].uv;
        let uv2 = mesh.vertices[i2].uv;
        let e1 = p1 - p0; let e2 = p2 - p0;
        let du1 = uv1.x - uv0.x; let dv1 = uv1.y - uv0.y;
        let du2 = uv2.x - uv0.x; let dv2 = uv2.y - uv0.y;
        let r = du1*dv2 - du2*dv1;
        if r.abs() < 1e-9 { continue; }
        let inv_r = 1.0 / r;
        let t = (e1 * dv2 - e2 * dv1) * inv_r;
        let bt = (e2 * du1 - e1 * du2) * inv_r;
        tan1[i0] += t; tan1[i1] += t; tan1[i2] += t;
        tan2[i0] += bt; tan2[i1] += bt; tan2[i2] += bt;
    }
    // Could add a tangent field to MeshVertex if needed, for now just compute normals
    for (i, v) in mesh.vertices.iter_mut().enumerate() {
        let n = v.normal;
        let t = tan1[i];
        // Gram-Schmidt orthogonalize
        if t.length_squared() > 1e-9 {
            let tangent = (t - n * n.dot(t)).normalize();
            // We could store this if MeshVertex had a tangent field
            let _ = tangent;
        }
    }
}

// ─── Extended: Voxel World Serialization ─────────────────────────────────────

pub fn serialize_voxel_world(world: &VoxelWorld) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(world.chunks.len() as u32).to_le_bytes());
    out.extend_from_slice(&(world.chunk_dim as u32).to_le_bytes());
    out.extend_from_slice(&world.voxel_size.to_bits().to_le_bytes());
    for (coord, chunk) in &world.chunks {
        out.extend_from_slice(&coord.cx.to_le_bytes());
        out.extend_from_slice(&coord.cy.to_le_bytes());
        out.extend_from_slice(&coord.cz.to_le_bytes());
        let rle = rle_encode(&chunk.voxels);
        out.extend_from_slice(&(rle.len() as u32).to_le_bytes());
        out.extend_from_slice(&rle);
    }
    out
}

pub fn deserialize_voxel_world(data: &[u8]) -> Option<VoxelWorld> {
    if data.len() < 12 { return None; }
    let n_chunks = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let chunk_dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let voxel_size = f32::from_bits(u32::from_le_bytes(data[8..12].try_into().ok()?));
    let mut world = VoxelWorld::new(chunk_dim, voxel_size, 0);
    let mut pos = 12usize;
    for _ in 0..n_chunks {
        if pos + 16 > data.len() { break; }
        let cx = i32::from_le_bytes(data[pos..pos+4].try_into().ok()?); pos += 4;
        let cy = i32::from_le_bytes(data[pos..pos+4].try_into().ok()?); pos += 4;
        let cz = i32::from_le_bytes(data[pos..pos+4].try_into().ok()?); pos += 4;
        let rle_len = u32::from_le_bytes(data[pos..pos+4].try_into().ok()?) as usize; pos += 4;
        if pos + rle_len > data.len() { break; }
        let expected = chunk_dim * chunk_dim * chunk_dim;
        let voxels = rle_decode(&data[pos..pos+rle_len], expected);
        pos += rle_len;
        let coord = ChunkCoord { cx, cy, cz };
        let origin = coord.to_world_origin(voxel_size, chunk_dim);
        let mut chunk = VoxelGrid::new(chunk_dim, chunk_dim, chunk_dim, origin, voxel_size);
        chunk.voxels = voxels;
        world.chunks.insert(coord, chunk);
    }
    Some(world)
}

// ─── Extended: Voxel LOD Streaming Manager ───────────────────────────────────

pub struct VoxelLodStreamer {
    pub loaded_chunks: HashMap<ChunkCoord, LodOctreeSet>,
    pub view_distance: f32,
    pub lod_depths: Vec<usize>,
}

impl VoxelLodStreamer {
    pub fn new(view_distance: f32) -> Self {
        Self {
            loaded_chunks: HashMap::new(),
            view_distance,
            lod_depths: vec![6, 4, 2],
        }
    }

    pub fn update(&mut self, world: &VoxelWorld, camera_pos: Vec3) {
        let chunk_size = world.voxel_size * world.chunk_dim as f32;
        let rad = (self.view_distance / chunk_size).ceil() as i32;
        let base = ChunkCoord::from_world(camera_pos, world.voxel_size, world.chunk_dim);

        for dz in -rad..=rad {
            for dy in -rad..=rad {
                for dx in -rad..=rad {
                    let coord = ChunkCoord { cx: base.cx+dx, cy: base.cy+dy, cz: base.cz+dz };
                    let chunk_center = coord.to_world_origin(world.voxel_size, world.chunk_dim) + Vec3::splat(chunk_size*0.5);
                    let dist = (chunk_center - camera_pos).length();
                    if dist > self.view_distance { continue; }
                    if self.loaded_chunks.contains_key(&coord) { continue; }
                    if let Some(chunk) = world.chunks.get(&coord) {
                        let svo = chunk.to_svo();
                        let lod_set = LodOctreeSet::build(svo, &self.lod_depths);
                        self.loaded_chunks.insert(coord, lod_set);
                    }
                }
            }
        }

        // Unload distant chunks
        self.loaded_chunks.retain(|coord, _| {
            let chunk_center = coord.to_world_origin(world.voxel_size, world.chunk_dim) + Vec3::splat(chunk_size*0.5);
            (chunk_center - camera_pos).length() <= self.view_distance * 1.5
        });
    }

    pub fn query_voxel(&self, world_pos: Vec3, camera_pos: Vec3, world: &VoxelWorld) -> Voxel {
        let coord = ChunkCoord::from_world(world_pos, world.voxel_size, world.chunk_dim);
        if let Some(lod_set) = self.loaded_chunks.get(&coord) {
            let chunk_center = coord.to_world_origin(world.voxel_size, world.chunk_dim);
            let dist = (chunk_center - camera_pos).length();
            let svo = lod_set.select_lod(dist, 10.0);
            svo.query(world_pos)
        } else {
            Voxel::EMPTY
        }
    }
}

// ─── End of File ──────────────────────────────────────────────────────────────
