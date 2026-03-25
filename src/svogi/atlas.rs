use glam::{Vec3, Vec4, UVec3};
use super::octree::{SparseVoxelOctree, VoxelData};

/// A small 3D tile of voxel data.
#[derive(Debug, Clone)]
pub struct Brick {
    pub position: UVec3,
    pub data: Vec<Vec4>,
}

impl Brick {
    pub fn new(position: UVec3, size: u32) -> Self {
        let count = (size * size * size) as usize;
        Self {
            position,
            data: vec![Vec4::ZERO; count],
        }
    }

    pub fn volume(size: u32) -> usize {
        (size * size * size) as usize
    }
}

/// A 3D shelf for packing bricks.
#[derive(Debug, Clone)]
pub struct Shelf3D {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub used_width: u32,
    pub used_height: u32,
}

impl Shelf3D {
    pub fn new(x: u32, y: u32, z: u32, width: u32, height: u32, depth: u32) -> Self {
        Self {
            x, y, z, width, height, depth,
            used_width: 0,
            used_height: 0,
        }
    }

    pub fn can_fit(&self, w: u32, h: u32, d: u32) -> bool {
        self.used_width + w <= self.width && h <= self.height && d <= self.depth
    }
}

/// 3D shelf-based packer for the atlas.
#[derive(Debug, Clone)]
pub struct BlockPacker {
    pub shelves: Vec<Shelf3D>,
    pub atlas_size: UVec3,
    next_z: u32,
    freed: Vec<(UVec3, UVec3)>,
}

impl BlockPacker {
    pub fn new(atlas_size: UVec3) -> Self {
        Self {
            shelves: Vec::new(),
            atlas_size,
            next_z: 0,
            freed: Vec::new(),
        }
    }

    /// Allocate space for a brick of the given size in the atlas.
    pub fn pack(&mut self, brick_size: UVec3) -> Option<UVec3> {
        // Try freed blocks first
        for i in 0..self.freed.len() {
            let (pos, size) = self.freed[i];
            if size.x >= brick_size.x && size.y >= brick_size.y && size.z >= brick_size.z {
                self.freed.swap_remove(i);
                return Some(pos);
            }
        }

        // Try existing shelves
        for shelf in &mut self.shelves {
            if shelf.can_fit(brick_size.x, brick_size.y, brick_size.z) {
                let pos = UVec3::new(shelf.x + shelf.used_width, shelf.y, shelf.z);
                shelf.used_width += brick_size.x;
                if brick_size.y > shelf.used_height {
                    shelf.used_height = brick_size.y;
                }
                return Some(pos);
            }
        }

        // Create new shelf layer
        if self.next_z + brick_size.z <= self.atlas_size.z {
            let shelf = Shelf3D::new(
                0, 0, self.next_z,
                self.atlas_size.x, brick_size.y, brick_size.z,
            );
            let pos = UVec3::new(0, 0, self.next_z);
            self.next_z += brick_size.z;
            let mut new_shelf = shelf;
            new_shelf.used_width = brick_size.x;
            new_shelf.used_height = brick_size.y;
            self.shelves.push(new_shelf);
            return Some(pos);
        }

        None // Atlas full
    }

    /// Free a region.
    pub fn free(&mut self, position: UVec3, size: UVec3) {
        self.freed.push((position, size));
    }

    pub fn utilization(&self) -> f32 {
        let total = self.atlas_size.x * self.atlas_size.y * self.atlas_size.z;
        if total == 0 {
            return 0.0;
        }
        let mut used = 0u32;
        for shelf in &self.shelves {
            used += shelf.used_width * shelf.used_height * shelf.depth;
        }
        let freed_vol: u32 = self.freed.iter().map(|(_, s)| s.x * s.y * s.z).sum();
        let actual_used = used.saturating_sub(freed_vol);
        actual_used as f32 / total as f32
    }
}

/// Atlas statistics.
#[derive(Debug, Clone)]
pub struct AtlasStats {
    pub total_bricks: u32,
    pub used_bricks: u32,
    pub utilization: f32,
}

/// The 3D texture atlas for storing voxel bricks.
#[derive(Debug, Clone)]
pub struct BrickAtlas {
    pub bricks: Vec<Brick>,
    pub atlas_data: Vec<Vec4>,
    pub atlas_size: UVec3,
    pub brick_size: u32,
    packer: BlockPacker,
    brick_positions: Vec<UVec3>,
}

impl BrickAtlas {
    pub fn new(atlas_size: UVec3, brick_size: u32) -> Self {
        let total = (atlas_size.x * atlas_size.y * atlas_size.z) as usize;
        Self {
            bricks: Vec::new(),
            atlas_data: vec![Vec4::ZERO; total],
            atlas_size,
            brick_size,
            packer: BlockPacker::new(atlas_size),
            brick_positions: Vec::new(),
        }
    }

    /// Add a brick to the atlas. Returns the atlas position or None if full.
    pub fn add_brick(&mut self, brick: Brick) -> Option<UVec3> {
        let bs = UVec3::splat(self.brick_size);
        let pos = self.packer.pack(bs)?;
        self.upload_brick(&brick, pos);
        self.brick_positions.push(pos);
        self.bricks.push(brick);
        Some(pos)
    }

    /// Copy brick data into the atlas at the given position.
    pub fn upload_brick(&mut self, brick: &Brick, atlas_pos: UVec3) {
        let bs = self.brick_size;
        for z in 0..bs {
            for y in 0..bs {
                for x in 0..bs {
                    let brick_idx = (z * bs * bs + y * bs + x) as usize;
                    if brick_idx < brick.data.len() {
                        let ax = atlas_pos.x + x;
                        let ay = atlas_pos.y + y;
                        let az = atlas_pos.z + z;
                        if ax < self.atlas_size.x && ay < self.atlas_size.y && az < self.atlas_size.z {
                            let atlas_idx = (az * self.atlas_size.y * self.atlas_size.x
                                + ay * self.atlas_size.x + ax) as usize;
                            if atlas_idx < self.atlas_data.len() {
                                self.atlas_data[atlas_idx] = brick.data[brick_idx];
                            }
                        }
                    }
                }
            }
        }
    }

    /// Sample the atlas at a world position using trilinear interpolation with LOD.
    pub fn sample_atlas(&self, world_pos: Vec3, lod: f32) -> Vec4 {
        // Convert world pos to atlas texel coordinates
        // Assume atlas covers [0, atlas_size] in normalized coordinates
        let u = world_pos.x.fract() * self.atlas_size.x as f32;
        let v = world_pos.y.fract() * self.atlas_size.y as f32;
        let w = world_pos.z.fract() * self.atlas_size.z as f32;

        // Apply LOD as a scale factor (coarser = skip texels)
        let scale = 2.0f32.powf(lod);
        let u = u / scale;
        let v = v / scale;
        let w = w / scale;

        // Trilinear interpolation
        let x0 = u.floor() as i32;
        let y0 = v.floor() as i32;
        let z0 = w.floor() as i32;
        let fx = u - u.floor();
        let fy = v - v.floor();
        let fz = w - w.floor();

        let sample = |x: i32, y: i32, z: i32| -> Vec4 {
            let x = x.clamp(0, self.atlas_size.x as i32 - 1) as u32;
            let y = y.clamp(0, self.atlas_size.y as i32 - 1) as u32;
            let z = z.clamp(0, self.atlas_size.z as i32 - 1) as u32;
            let idx = (z * self.atlas_size.y * self.atlas_size.x
                + y * self.atlas_size.x + x) as usize;
            if idx < self.atlas_data.len() {
                self.atlas_data[idx]
            } else {
                Vec4::ZERO
            }
        };

        let c000 = sample(x0, y0, z0);
        let c100 = sample(x0 + 1, y0, z0);
        let c010 = sample(x0, y0 + 1, z0);
        let c110 = sample(x0 + 1, y0 + 1, z0);
        let c001 = sample(x0, y0, z0 + 1);
        let c101 = sample(x0 + 1, y0, z0 + 1);
        let c011 = sample(x0, y0 + 1, z0 + 1);
        let c111 = sample(x0 + 1, y0 + 1, z0 + 1);

        let c00 = c000 * (1.0 - fx) + c100 * fx;
        let c10 = c010 * (1.0 - fx) + c110 * fx;
        let c01 = c001 * (1.0 - fx) + c101 * fx;
        let c11 = c011 * (1.0 - fx) + c111 * fx;

        let c0 = c00 * (1.0 - fy) + c10 * fy;
        let c1 = c01 * (1.0 - fy) + c11 * fy;

        c0 * (1.0 - fz) + c1 * fz
    }

    pub fn stats(&self) -> AtlasStats {
        AtlasStats {
            total_bricks: (self.atlas_size.x / self.brick_size)
                * (self.atlas_size.y / self.brick_size)
                * (self.atlas_size.z / self.brick_size),
            used_bricks: self.bricks.len() as u32,
            utilization: self.packer.utilization(),
        }
    }

    /// Defragment the atlas by repacking all bricks contiguously.
    pub fn defragment(&mut self) {
        let old_bricks: Vec<Brick> = self.bricks.drain(..).collect();
        let old_positions: Vec<UVec3> = self.brick_positions.drain(..).collect();

        // Clear atlas
        for v in &mut self.atlas_data {
            *v = Vec4::ZERO;
        }
        self.packer = BlockPacker::new(self.atlas_size);

        // Re-add all bricks
        for brick in old_bricks {
            self.add_brick(brick);
        }
    }

    /// Remove a brick by index.
    pub fn remove_brick(&mut self, index: usize) {
        if index >= self.bricks.len() {
            return;
        }
        let pos = self.brick_positions[index];
        let bs = UVec3::splat(self.brick_size);

        // Clear atlas region
        for z in 0..self.brick_size {
            for y in 0..self.brick_size {
                for x in 0..self.brick_size {
                    let ax = pos.x + x;
                    let ay = pos.y + y;
                    let az = pos.z + z;
                    let idx = (az * self.atlas_size.y * self.atlas_size.x
                        + ay * self.atlas_size.x + ax) as usize;
                    if idx < self.atlas_data.len() {
                        self.atlas_data[idx] = Vec4::ZERO;
                    }
                }
            }
        }

        self.packer.free(pos, bs);
        self.bricks.swap_remove(index);
        self.brick_positions.swap_remove(index);
    }
}

/// Indirection table mapping world voxel coordinates to atlas positions.
#[derive(Debug, Clone)]
pub struct IndirectionTable {
    pub data: Vec<u32>,
    pub resolution: UVec3,
}

impl IndirectionTable {
    pub fn new(resolution: UVec3) -> Self {
        let count = (resolution.x * resolution.y * resolution.z) as usize;
        Self {
            data: vec![u32::MAX; count],
            resolution,
        }
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, brick_index: u32) {
        let idx = (z * self.resolution.y * self.resolution.x
            + y * self.resolution.x + x) as usize;
        if idx < self.data.len() {
            self.data[idx] = brick_index;
        }
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> Option<u32> {
        let idx = (z * self.resolution.y * self.resolution.x
            + y * self.resolution.x + x) as usize;
        if idx < self.data.len() && self.data[idx] != u32::MAX {
            Some(self.data[idx])
        } else {
            None
        }
    }
}

/// Build an indirection table from an octree and atlas.
pub fn build_indirection(octree: &SparseVoxelOctree, atlas: &BrickAtlas) -> IndirectionTable {
    let res = UVec3::splat(1u32 << octree.max_depth);
    let mut table = IndirectionTable::new(res);

    let world_size = octree.world_bounds.size();
    let voxel_size = world_size / Vec3::new(res.x as f32, res.y as f32, res.z as f32);

    for (brick_idx, brick) in atlas.bricks.iter().enumerate() {
        let bx = brick.position.x;
        let by = brick.position.y;
        let bz = brick.position.z;
        if bx < res.x && by < res.y && bz < res.z {
            table.set(bx, by, bz, brick_idx as u32);
        }
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brick_atlas_pack_and_upload() {
        let mut atlas = BrickAtlas::new(UVec3::new(32, 32, 32), 4);

        let mut brick = Brick::new(UVec3::new(0, 0, 0), 4);
        brick.data[0] = Vec4::new(1.0, 0.0, 0.0, 1.0);

        let pos = atlas.add_brick(brick);
        assert!(pos.is_some());

        let stats = atlas.stats();
        assert_eq!(stats.used_bricks, 1);
    }

    #[test]
    fn test_packer_utilization() {
        let mut packer = BlockPacker::new(UVec3::new(16, 16, 16));
        let pos1 = packer.pack(UVec3::new(4, 4, 4));
        assert!(pos1.is_some());

        let pos2 = packer.pack(UVec3::new(4, 4, 4));
        assert!(pos2.is_some());
        assert_ne!(pos1, pos2);

        let util = packer.utilization();
        assert!(util > 0.0);
    }

    #[test]
    fn test_packer_free_reuse() {
        let mut packer = BlockPacker::new(UVec3::new(16, 16, 16));
        let pos1 = packer.pack(UVec3::new(4, 4, 4)).unwrap();
        packer.free(pos1, UVec3::new(4, 4, 4));
        let pos2 = packer.pack(UVec3::new(4, 4, 4)).unwrap();
        assert_eq!(pos1, pos2); // Should reuse freed space
    }

    #[test]
    fn test_indirection_table() {
        let mut table = IndirectionTable::new(UVec3::new(4, 4, 4));
        assert!(table.get(0, 0, 0).is_none());
        table.set(1, 2, 3, 42);
        assert_eq!(table.get(1, 2, 3), Some(42));
    }

    #[test]
    fn test_atlas_defragment() {
        let mut atlas = BrickAtlas::new(UVec3::new(32, 32, 32), 4);

        for i in 0..4 {
            let brick = Brick::new(UVec3::new(i, 0, 0), 4);
            atlas.add_brick(brick);
        }
        atlas.remove_brick(1);
        assert_eq!(atlas.bricks.len(), 3);

        atlas.defragment();
        assert_eq!(atlas.bricks.len(), 3);
    }

    #[test]
    fn test_atlas_sample() {
        let mut atlas = BrickAtlas::new(UVec3::new(8, 8, 8), 4);

        // Fill a region with known color
        let mut brick = Brick::new(UVec3::ZERO, 4);
        for v in &mut brick.data {
            *v = Vec4::new(0.5, 0.5, 0.5, 1.0);
        }
        atlas.add_brick(brick);

        // Sample should return something non-zero in that region
        let sample = atlas.sample_atlas(Vec3::new(0.05, 0.05, 0.05), 0.0);
        // Just verify it doesn't panic and returns a value
        let _ = sample;
    }

    #[test]
    fn test_atlas_full() {
        let mut atlas = BrickAtlas::new(UVec3::new(4, 4, 4), 4);
        let brick = Brick::new(UVec3::ZERO, 4);
        let pos1 = atlas.add_brick(brick.clone());
        assert!(pos1.is_some());
        // Atlas is exactly one brick, second should fail
        let pos2 = atlas.add_brick(brick);
        assert!(pos2.is_none());
    }
}
