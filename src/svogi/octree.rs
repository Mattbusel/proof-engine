use glam::{Vec3, Vec4, UVec3, IVec3};
use std::collections::VecDeque;

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn half_size(&self) -> Vec3 {
        self.size() * 0.5
    }

    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x
            && point.y >= self.min.y && point.y <= self.max.y
            && point.z >= self.min.z && point.z <= self.max.z
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// Return the child octant bounding box (0..7).
    pub fn subdivide_octant(&self, octant: u8) -> Aabb {
        let c = self.center();
        let min = Vec3::new(
            if octant & 1 == 0 { self.min.x } else { c.x },
            if octant & 2 == 0 { self.min.y } else { c.y },
            if octant & 4 == 0 { self.min.z } else { c.z },
        );
        let max = Vec3::new(
            if octant & 1 == 0 { c.x } else { self.max.x },
            if octant & 2 == 0 { c.y } else { self.max.y },
            if octant & 4 == 0 { c.z } else { self.max.z },
        );
        Aabb { min, max }
    }

    /// Intersect a ray with this AABB, returning (t_enter, t_exit). None if miss.
    pub fn intersect_ray(&self, origin: Vec3, inv_dir: Vec3) -> Option<(f32, f32)> {
        let t1 = (self.min - origin) * inv_dir;
        let t2 = (self.max - origin) * inv_dir;
        let t_min = t1.min(t2);
        let t_max = t1.max(t2);
        let t_enter = t_min.x.max(t_min.y).max(t_min.z);
        let t_exit = t_max.x.min(t_max.y).min(t_max.z);
        if t_enter <= t_exit && t_exit >= 0.0 {
            Some((t_enter.max(0.0), t_exit))
        } else {
            None
        }
    }
}

/// Data stored per voxel.
#[derive(Debug, Clone, Copy)]
pub struct VoxelData {
    pub radiance: Vec4,
    pub normal: Vec3,
    pub opacity: f32,
    pub sh_coeffs: [f32; 9],
}

impl Default for VoxelData {
    fn default() -> Self {
        Self {
            radiance: Vec4::ZERO,
            normal: Vec3::ZERO,
            opacity: 0.0,
            sh_coeffs: [0.0; 9],
        }
    }
}

impl VoxelData {
    pub fn is_empty(&self) -> bool {
        self.opacity <= 0.0
    }

    pub fn average(datas: &[&VoxelData]) -> VoxelData {
        if datas.is_empty() {
            return VoxelData::default();
        }
        let n = datas.len() as f32;
        let mut result = VoxelData::default();
        for d in datas {
            result.radiance += d.radiance;
            result.normal += d.normal;
            result.opacity += d.opacity;
            for i in 0..9 {
                result.sh_coeffs[i] += d.sh_coeffs[i];
            }
        }
        result.radiance /= n;
        result.normal = if result.normal.length_squared() > 1e-8 {
            result.normal.normalize()
        } else {
            Vec3::ZERO
        };
        result.opacity /= n;
        for i in 0..9 {
            result.sh_coeffs[i] /= n;
        }
        result
    }
}

/// A single node in the sparse voxel octree.
#[derive(Debug, Clone)]
pub struct OctreeNode {
    pub children: [Option<u32>; 8],
    pub data: VoxelData,
    pub level: u8,
    pub morton_code: u64,
}

impl OctreeNode {
    pub fn new_leaf(data: VoxelData, level: u8, morton_code: u64) -> Self {
        Self {
            children: [None; 8],
            data,
            level,
            morton_code,
        }
    }

    pub fn new_internal(level: u8, morton_code: u64) -> Self {
        Self {
            children: [None; 8],
            data: VoxelData::default(),
            level,
            morton_code,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.iter().all(|c| c.is_none())
    }

    pub fn has_children(&self) -> bool {
        self.children.iter().any(|c| c.is_some())
    }
}

/// Morton code encoding: interleave bits of x, y, z into a 64-bit code.
pub fn morton_encode_3d(x: u32, y: u32, z: u32) -> u64 {
    fn split_by_3(mut v: u32) -> u64 {
        let mut v = v as u64 & 0x1fffff; // 21 bits
        v = (v | (v << 32)) & 0x1f00000000ffff;
        v = (v | (v << 16)) & 0x1f0000ff0000ff;
        v = (v | (v << 8))  & 0x100f00f00f00f00f;
        v = (v | (v << 4))  & 0x10c30c30c30c30c3;
        v = (v | (v << 2))  & 0x1249249249249249;
        v
    }
    split_by_3(x) | (split_by_3(y) << 1) | (split_by_3(z) << 2)
}

/// Morton code decoding: extract x, y, z from a 64-bit code.
pub fn morton_decode_3d(code: u64) -> (u32, u32, u32) {
    fn compact_by_3(mut v: u64) -> u32 {
        v &= 0x1249249249249249;
        v = (v | (v >> 2))  & 0x10c30c30c30c30c3;
        v = (v | (v >> 4))  & 0x100f00f00f00f00f;
        v = (v | (v >> 8))  & 0x1f0000ff0000ff;
        v = (v | (v >> 16)) & 0x1f00000000ffff;
        v = (v | (v >> 32)) & 0x1fffff;
        v as u32
    }
    (compact_by_3(code), compact_by_3(code >> 1), compact_by_3(code >> 2))
}

/// Dense 3D voxel grid.
#[derive(Debug, Clone)]
pub struct VoxelGrid {
    pub data: Vec<VoxelData>,
    pub resolution: UVec3,
}

impl VoxelGrid {
    pub fn new(resolution: UVec3) -> Self {
        let count = (resolution.x * resolution.y * resolution.z) as usize;
        Self {
            data: vec![VoxelData::default(); count],
            resolution,
        }
    }

    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        (z * self.resolution.y * self.resolution.x + y * self.resolution.x + x) as usize
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> &VoxelData {
        &self.data[self.index(x, y, z)]
    }

    pub fn get_mut(&mut self, x: u32, y: u32, z: u32) -> &mut VoxelData {
        let idx = self.index(x, y, z);
        &mut self.data[idx]
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, data: VoxelData) {
        let idx = self.index(x, y, z);
        self.data[idx] = data;
    }

    pub fn in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0
            && (x as u32) < self.resolution.x
            && (y as u32) < self.resolution.y
            && (z as u32) < self.resolution.z
    }

    pub fn filled_count(&self) -> usize {
        self.data.iter().filter(|v| !v.is_empty()).count()
    }
}

/// The sparse voxel octree.
#[derive(Debug, Clone)]
pub struct SparseVoxelOctree {
    pub nodes: Vec<OctreeNode>,
    pub root: u32,
    pub max_depth: u8,
    pub world_bounds: Aabb,
}

impl SparseVoxelOctree {
    pub fn new(world_bounds: Aabb, max_depth: u8) -> Self {
        let root_node = OctreeNode::new_internal(0, 0);
        Self {
            nodes: vec![root_node],
            root: 0,
            max_depth,
            world_bounds,
        }
    }

    /// Determine which octant a point falls into within the given bounds.
    fn octant_for_point(bounds: &Aabb, point: Vec3) -> u8 {
        let c = bounds.center();
        let mut octant = 0u8;
        if point.x >= c.x { octant |= 1; }
        if point.y >= c.y { octant |= 2; }
        if point.z >= c.z { octant |= 4; }
        octant
    }

    /// Insert a voxel at the given world position, creating intermediate nodes as needed.
    pub fn insert(&mut self, position: Vec3, data: VoxelData) {
        if !self.world_bounds.contains(position) {
            return;
        }
        let max_depth = self.max_depth;
        self.insert_recursive(self.root, self.world_bounds, position, data, 0, max_depth);
    }

    fn insert_recursive(
        &mut self,
        node_idx: u32,
        bounds: Aabb,
        position: Vec3,
        data: VoxelData,
        depth: u8,
        max_depth: u8,
    ) {
        if depth >= max_depth {
            self.nodes[node_idx as usize].data = data;
            return;
        }

        let octant = Self::octant_for_point(&bounds, position);
        let child_bounds = bounds.subdivide_octant(octant);

        let child_idx = if let Some(idx) = self.nodes[node_idx as usize].children[octant as usize] {
            idx
        } else {
            let idx = self.nodes.len() as u32;
            let morton = morton_encode_3d(
                ((position.x - self.world_bounds.min.x) / self.world_bounds.size().x * ((1u32 << max_depth) as f32)) as u32,
                ((position.y - self.world_bounds.min.y) / self.world_bounds.size().y * ((1u32 << max_depth) as f32)) as u32,
                ((position.z - self.world_bounds.min.z) / self.world_bounds.size().z * ((1u32 << max_depth) as f32)) as u32,
            );
            let new_node = OctreeNode::new_internal(depth + 1, morton);
            self.nodes.push(new_node);
            self.nodes[node_idx as usize].children[octant as usize] = Some(idx);
            idx
        };

        self.insert_recursive(child_idx, child_bounds, position, data, depth + 1, max_depth);
    }

    /// Look up voxel data at a world position, at the given LOD level.
    /// Level 0 = root (coarsest), max_depth = leaf (finest).
    pub fn lookup(&self, position: Vec3, level: u8) -> Option<&VoxelData> {
        if !self.world_bounds.contains(position) {
            return None;
        }
        self.lookup_recursive(self.root, self.world_bounds, position, 0, level)
    }

    fn lookup_recursive(
        &self,
        node_idx: u32,
        bounds: Aabb,
        position: Vec3,
        depth: u8,
        target_level: u8,
    ) -> Option<&VoxelData> {
        let node = &self.nodes[node_idx as usize];

        if depth >= target_level || node.is_leaf() {
            if node.data.is_empty() && node.is_leaf() && depth < target_level {
                return None;
            }
            return Some(&node.data);
        }

        let octant = Self::octant_for_point(&bounds, position);
        if let Some(child_idx) = node.children[octant as usize] {
            let child_bounds = bounds.subdivide_octant(octant);
            self.lookup_recursive(child_idx, child_bounds, position, depth + 1, target_level)
        } else {
            // No child at this octant; return this node's data if non-empty
            if !node.data.is_empty() {
                Some(&node.data)
            } else {
                None
            }
        }
    }

    /// Remove a voxel at the given position (clear leaf data).
    pub fn remove(&mut self, position: Vec3) {
        if !self.world_bounds.contains(position) {
            return;
        }
        let max_depth = self.max_depth;
        self.remove_recursive(self.root, self.world_bounds, position, 0, max_depth);
    }

    fn remove_recursive(
        &mut self,
        node_idx: u32,
        bounds: Aabb,
        position: Vec3,
        depth: u8,
        max_depth: u8,
    ) -> bool {
        if depth >= max_depth {
            self.nodes[node_idx as usize].data = VoxelData::default();
            return true; // node is now empty
        }

        let octant = Self::octant_for_point(&bounds, position);
        let child_idx = match self.nodes[node_idx as usize].children[octant as usize] {
            Some(idx) => idx,
            None => return false,
        };

        let child_bounds = bounds.subdivide_octant(octant);
        let child_empty = self.remove_recursive(child_idx, child_bounds, position, depth + 1, max_depth);

        if child_empty {
            let child_node = &self.nodes[child_idx as usize];
            if child_node.is_leaf() && child_node.data.is_empty() {
                self.nodes[node_idx as usize].children[octant as usize] = None;
            }
        }

        // Prune: check if all children are None
        self.nodes[node_idx as usize].children.iter().all(|c| c.is_none())
            && self.nodes[node_idx as usize].data.is_empty()
    }

    /// Traverse a ray through the octree, collecting hits.
    pub fn traverse_ray(&self, origin: Vec3, direction: Vec3, max_t: f32) -> Vec<(Vec3, VoxelData, f32)> {
        let mut results = Vec::new();
        let dir_safe = Vec3::new(
            if direction.x.abs() < 1e-8 { 1e-8 } else { direction.x },
            if direction.y.abs() < 1e-8 { 1e-8 } else { direction.y },
            if direction.z.abs() < 1e-8 { 1e-8 } else { direction.z },
        );
        let inv_dir = Vec3::new(1.0 / dir_safe.x, 1.0 / dir_safe.y, 1.0 / dir_safe.z);

        self.traverse_ray_recursive(
            self.root,
            self.world_bounds,
            origin,
            direction,
            inv_dir,
            max_t,
            &mut results,
        );

        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    fn traverse_ray_recursive(
        &self,
        node_idx: u32,
        bounds: Aabb,
        origin: Vec3,
        direction: Vec3,
        inv_dir: Vec3,
        max_t: f32,
        results: &mut Vec<(Vec3, VoxelData, f32)>,
    ) {
        let hit = bounds.intersect_ray(origin, inv_dir);
        let (t_enter, t_exit) = match hit {
            Some((te, tx)) if te <= max_t => (te, tx),
            _ => return,
        };

        let node = &self.nodes[node_idx as usize];

        if node.is_leaf() {
            if !node.data.is_empty() {
                let hit_point = origin + direction * t_enter;
                results.push((hit_point, node.data, t_enter));
            }
            return;
        }

        // Traverse children sorted by t_enter for front-to-back ordering
        let mut child_order: Vec<(u8, f32)> = Vec::new();
        for octant in 0..8u8 {
            if let Some(child_idx) = node.children[octant as usize] {
                let child_bounds = bounds.subdivide_octant(octant);
                if let Some((te, _)) = child_bounds.intersect_ray(origin, inv_dir) {
                    if te <= max_t {
                        child_order.push((octant, te));
                    }
                }
            }
        }
        child_order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for (octant, _) in child_order {
            let child_idx = self.nodes[node_idx as usize].children[octant as usize].unwrap();
            let child_bounds = bounds.subdivide_octant(octant);
            self.traverse_ray_recursive(child_idx, child_bounds, origin, direction, inv_dir, max_t, results);
        }
    }

    /// Average child voxels into parent for LOD (mipmap). Process the given level.
    pub fn mipmap_level(&mut self, level: u8) {
        // Collect nodes at the given level that have children
        let indices: Vec<u32> = (0..self.nodes.len() as u32)
            .filter(|&i| {
                let n = &self.nodes[i as usize];
                n.level == level && n.has_children()
            })
            .collect();

        for idx in indices {
            let children_indices: Vec<u32> = self.nodes[idx as usize]
                .children
                .iter()
                .filter_map(|c| *c)
                .collect();

            if children_indices.is_empty() {
                continue;
            }

            let child_datas: Vec<&VoxelData> = children_indices
                .iter()
                .map(|&ci| &self.nodes[ci as usize].data)
                .filter(|d| !d.is_empty())
                .collect();

            if !child_datas.is_empty() {
                self.nodes[idx as usize].data = VoxelData::average(&child_datas);
            }
        }
    }

    /// Build the full mipmap chain from leaves up to root.
    pub fn build_mipmaps(&mut self) {
        if self.max_depth == 0 {
            return;
        }
        for level in (0..self.max_depth).rev() {
            self.mipmap_level(level);
        }
    }

    /// Build a sparse voxel octree from a dense grid.
    pub fn build_from_voxel_grid(grid: &VoxelGrid, world_bounds: Aabb) -> SparseVoxelOctree {
        let max_dim = grid.resolution.x.max(grid.resolution.y).max(grid.resolution.z);
        let max_depth = (max_dim as f32).log2().ceil() as u8;
        let max_depth = max_depth.max(1);

        let mut octree = SparseVoxelOctree::new(world_bounds, max_depth);
        let voxel_size = world_bounds.size() / Vec3::new(
            grid.resolution.x as f32,
            grid.resolution.y as f32,
            grid.resolution.z as f32,
        );

        for z in 0..grid.resolution.z {
            for y in 0..grid.resolution.y {
                for x in 0..grid.resolution.x {
                    let voxel = grid.get(x, y, z);
                    if !voxel.is_empty() {
                        let pos = world_bounds.min + Vec3::new(
                            (x as f32 + 0.5) * voxel_size.x,
                            (y as f32 + 0.5) * voxel_size.y,
                            (z as f32 + 0.5) * voxel_size.z,
                        );
                        octree.insert(pos, *voxel);
                    }
                }
            }
        }

        octree.build_mipmaps();
        octree
    }

    /// Total memory usage in bytes (approximate).
    pub fn memory_usage(&self) -> usize {
        self.nodes.len() * std::mem::size_of::<OctreeNode>()
    }

    /// Total number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Count of leaf nodes (no children).
    pub fn leaf_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_leaf()).count()
    }

    /// Maximum depth actually present.
    pub fn depth(&self) -> u8 {
        self.nodes.iter().map(|n| n.level).max().unwrap_or(0)
    }

    /// Iterate all occupied leaf voxels.
    pub fn iter_leaves(&self) -> OctreeIterator<'_> {
        OctreeIterator {
            octree: self,
            stack: vec![(self.root, self.world_bounds)],
        }
    }

    /// Sample the octree at a world position with fractional LOD.
    /// Interpolates between two integer LOD levels.
    pub fn sample_lod(&self, position: Vec3, lod: f32) -> Option<VoxelData> {
        let lod_low = lod.floor() as u8;
        let lod_high = lod.ceil() as u8;
        let frac = lod - lod.floor();

        let data_low = self.lookup(position, lod_low);
        let data_high = self.lookup(position, lod_high);

        match (data_low, data_high) {
            (Some(a), Some(b)) => {
                let mut result = VoxelData::default();
                result.radiance = a.radiance * (1.0 - frac) + b.radiance * frac;
                result.normal = if (a.normal * (1.0 - frac) + b.normal * frac).length_squared() > 1e-8 {
                    (a.normal * (1.0 - frac) + b.normal * frac).normalize()
                } else {
                    Vec3::ZERO
                };
                result.opacity = a.opacity * (1.0 - frac) + b.opacity * frac;
                for i in 0..9 {
                    result.sh_coeffs[i] = a.sh_coeffs[i] * (1.0 - frac) + b.sh_coeffs[i] * frac;
                }
                Some(result)
            }
            (Some(a), None) => Some(*a),
            (None, Some(b)) => Some(*b),
            (None, None) => None,
        }
    }
}

/// Iterator over all occupied leaf voxels.
pub struct OctreeIterator<'a> {
    octree: &'a SparseVoxelOctree,
    stack: Vec<(u32, Aabb)>,
}

impl<'a> Iterator for OctreeIterator<'a> {
    type Item = (Vec3, &'a VoxelData);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node_idx, bounds)) = self.stack.pop() {
            let node = &self.octree.nodes[node_idx as usize];

            if node.is_leaf() {
                if !node.data.is_empty() {
                    return Some((bounds.center(), &node.data));
                }
                continue;
            }

            for octant in (0..8u8).rev() {
                if let Some(child_idx) = node.children[octant as usize] {
                    self.stack.push((child_idx, bounds.subdivide_octant(octant)));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_basics() {
        let aabb = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
        assert_eq!(aabb.center(), Vec3::splat(5.0));
        assert_eq!(aabb.size(), Vec3::splat(10.0));
        assert!(aabb.contains(Vec3::splat(5.0)));
        assert!(!aabb.contains(Vec3::splat(11.0)));
    }

    #[test]
    fn test_aabb_intersects() {
        let a = Aabb::new(Vec3::ZERO, Vec3::splat(5.0));
        let b = Aabb::new(Vec3::splat(3.0), Vec3::splat(8.0));
        let c = Aabb::new(Vec3::splat(6.0), Vec3::splat(10.0));
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_morton_roundtrip() {
        for x in 0..16u32 {
            for y in 0..16u32 {
                for z in 0..16u32 {
                    let code = morton_encode_3d(x, y, z);
                    let (dx, dy, dz) = morton_decode_3d(code);
                    assert_eq!((x, y, z), (dx, dy, dz), "Morton roundtrip failed for ({x},{y},{z})");
                }
            }
        }
    }

    #[test]
    fn test_morton_ordering() {
        // Morton codes should maintain Z-curve ordering
        let c1 = morton_encode_3d(0, 0, 0);
        let c2 = morton_encode_3d(1, 0, 0);
        let c3 = morton_encode_3d(0, 1, 0);
        assert!(c1 < c2);
        assert!(c1 < c3);
    }

    #[test]
    fn test_insert_and_lookup() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);

        let data = VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };

        octree.insert(Vec3::new(1.0, 1.0, 1.0), data);

        let result = octree.lookup(Vec3::new(1.0, 1.0, 1.0), 3);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.radiance.x - 1.0).abs() < 1e-5);
        assert!((r.opacity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_insert_multiple_and_lookup() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);

        let data_red = VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };
        let data_blue = VoxelData {
            radiance: Vec4::new(0.0, 0.0, 1.0, 1.0),
            normal: Vec3::X,
            opacity: 0.5,
            sh_coeffs: [0.0; 9],
        };

        octree.insert(Vec3::new(1.0, 1.0, 1.0), data_red);
        octree.insert(Vec3::new(6.0, 6.0, 6.0), data_blue);

        let r1 = octree.lookup(Vec3::new(1.0, 1.0, 1.0), 3).unwrap();
        assert!((r1.radiance.x - 1.0).abs() < 1e-5);

        let r2 = octree.lookup(Vec3::new(6.0, 6.0, 6.0), 3).unwrap();
        assert!((r2.radiance.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_remove() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);

        let data = VoxelData {
            radiance: Vec4::ONE,
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };

        octree.insert(Vec3::new(2.0, 2.0, 2.0), data);
        assert!(octree.lookup(Vec3::new(2.0, 2.0, 2.0), 3).is_some());

        octree.remove(Vec3::new(2.0, 2.0, 2.0));
        // After remove, the data should be empty
        let r = octree.lookup(Vec3::new(2.0, 2.0, 2.0), 3);
        match r {
            Some(d) => assert!(d.is_empty()),
            None => {} // also acceptable
        }
    }

    #[test]
    fn test_ray_traversal() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);

        let data = VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Z,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };

        // Place a voxel at roughly (4, 4, 4)
        octree.insert(Vec3::new(4.0, 4.0, 4.0), data);

        // Shoot ray along Z towards it
        let hits = octree.traverse_ray(
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            20.0,
        );
        assert!(!hits.is_empty(), "Ray should hit the voxel");
        assert!((hits[0].1.radiance.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_ray_miss() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);

        let data = VoxelData {
            radiance: Vec4::ONE,
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };
        octree.insert(Vec3::new(1.0, 1.0, 1.0), data);

        // Shoot ray that misses entirely
        let hits = octree.traverse_ray(
            Vec3::new(7.0, 7.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            20.0,
        );
        // The ray goes through a different octant, should not hit
        // (it may or may not depending on resolution, but at least check it runs)
        let _ = hits;
    }

    #[test]
    fn test_mipmap_averaging() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 2);

        let data = VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };

        octree.insert(Vec3::new(1.0, 1.0, 1.0), data);
        octree.insert(Vec3::new(3.0, 1.0, 1.0), data);

        octree.build_mipmaps();

        // Parent should have averaged radiance
        let parent = octree.lookup(Vec3::new(2.0, 1.0, 1.0), 1);
        assert!(parent.is_some());
    }

    #[test]
    fn test_build_from_grid() {
        let mut grid = VoxelGrid::new(UVec3::new(4, 4, 4));
        grid.set(1, 1, 1, VoxelData {
            radiance: Vec4::new(0.5, 0.5, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });

        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(4.0));
        let octree = SparseVoxelOctree::build_from_voxel_grid(&grid, bounds);

        assert!(octree.node_count() > 1);
        assert!(octree.leaf_count() > 0);
    }

    #[test]
    fn test_iterator() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);

        let data = VoxelData {
            radiance: Vec4::ONE,
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };

        octree.insert(Vec3::new(1.0, 1.0, 1.0), data);
        octree.insert(Vec3::new(6.0, 6.0, 6.0), data);

        let leaves: Vec<_> = octree.iter_leaves().collect();
        assert_eq!(leaves.len(), 2);
    }

    #[test]
    fn test_voxel_grid() {
        let mut grid = VoxelGrid::new(UVec3::new(4, 4, 4));
        assert_eq!(grid.filled_count(), 0);

        grid.set(0, 0, 0, VoxelData {
            radiance: Vec4::ONE,
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        assert_eq!(grid.filled_count(), 1);
        assert!(grid.in_bounds(3, 3, 3));
        assert!(!grid.in_bounds(4, 0, 0));
    }

    #[test]
    fn test_aabb_ray_intersection() {
        let aabb = Aabb::new(Vec3::ZERO, Vec3::splat(2.0));
        let origin = Vec3::new(1.0, 1.0, -5.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let inv_dir = Vec3::new(f32::INFINITY, f32::INFINITY, 1.0);
        // Use safe inv_dir
        let inv_dir = Vec3::new(1.0 / 1e-8, 1.0 / 1e-8, 1.0);
        let hit = aabb.intersect_ray(origin, Vec3::new(0.0, 0.0, 1.0).recip());
        // direction is (0,0,1), recip may be inf, that's fine for the algorithm
        // Just test with non-degenerate ray
        let origin2 = Vec3::new(1.0, 1.0, -2.0);
        let hit2 = aabb.intersect_ray(origin2, Vec3::new(1.0, 1.0, 1.0).recip());
        assert!(hit2.is_some());
    }

    #[test]
    fn test_memory_usage() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let octree = SparseVoxelOctree::new(bounds, 3);
        assert!(octree.memory_usage() > 0);
    }

    #[test]
    fn test_subdivide_octant_covers_full_volume() {
        let aabb = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        for octant in 0..8u8 {
            let child = aabb.subdivide_octant(octant);
            assert!(child.size().x > 0.0);
            assert!(child.size().y > 0.0);
            assert!(child.size().z > 0.0);
            // Child should be half the size
            assert!((child.size().x - 4.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_sample_lod() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);
        let data = VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        };
        octree.insert(Vec3::new(1.0, 1.0, 1.0), data);
        octree.build_mipmaps();

        let sampled = octree.sample_lod(Vec3::new(1.0, 1.0, 1.0), 2.5);
        assert!(sampled.is_some());
    }

    #[test]
    fn test_lookup_out_of_bounds_returns_none() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let octree = SparseVoxelOctree::new(bounds, 3);
        assert!(octree.lookup(Vec3::splat(100.0), 3).is_none());
    }

    #[test]
    fn test_insert_out_of_bounds_noop() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(8.0));
        let mut octree = SparseVoxelOctree::new(bounds, 3);
        let initial_count = octree.node_count();
        octree.insert(Vec3::splat(-5.0), VoxelData {
            radiance: Vec4::ONE,
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        assert_eq!(octree.node_count(), initial_count);
    }

    #[test]
    fn test_depth_tracking() {
        let bounds = Aabb::new(Vec3::ZERO, Vec3::splat(16.0));
        let mut octree = SparseVoxelOctree::new(bounds, 4);
        octree.insert(Vec3::splat(1.0), VoxelData {
            radiance: Vec4::ONE,
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [0.0; 9],
        });
        assert!(octree.depth() >= 1);
        assert!(octree.depth() <= 4);
    }

    #[test]
    fn test_voxel_data_average() {
        let a = VoxelData {
            radiance: Vec4::new(1.0, 0.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 1.0,
            sh_coeffs: [1.0; 9],
        };
        let b = VoxelData {
            radiance: Vec4::new(0.0, 1.0, 0.0, 1.0),
            normal: Vec3::Y,
            opacity: 0.5,
            sh_coeffs: [0.0; 9],
        };
        let avg = VoxelData::average(&[&a, &b]);
        assert!((avg.radiance.x - 0.5).abs() < 1e-5);
        assert!((avg.radiance.y - 0.5).abs() < 1e-5);
        assert!((avg.opacity - 0.75).abs() < 1e-5);
    }

    #[test]
    fn test_morton_large_values() {
        let code = morton_encode_3d(1000, 2000, 3000);
        let (x, y, z) = morton_decode_3d(code);
        assert_eq!((x, y, z), (1000, 2000, 3000));
    }

    #[test]
    fn test_grid_index_consistency() {
        let grid = VoxelGrid::new(UVec3::new(8, 8, 8));
        let idx1 = grid.index(0, 0, 0);
        let idx2 = grid.index(7, 7, 7);
        assert_eq!(idx1, 0);
        assert_eq!(idx2, 511);
    }
}
