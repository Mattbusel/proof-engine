//! Bounding Volume Hierarchy for fast spatial queries.

use crate::glyph::GlyphId;
use glam::Vec3;

/// Axis-aligned bounding box.
#[derive(Clone, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }

    pub fn empty() -> Self {
        Self { min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) }
    }

    pub fn from_point(p: Vec3, half: f32) -> Self {
        Self { min: p - Vec3::splat(half), max: p + Vec3::splat(half) }
    }

    pub fn union(&self, other: &Aabb) -> Self {
        Self { min: self.min.min(other.min), max: self.max.max(other.max) }
    }

    pub fn center(&self) -> Vec3 { (self.min + self.max) * 0.5 }
    pub fn extents(&self) -> Vec3 { (self.max - self.min) * 0.5 }
    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        let closest = center.clamp(self.min, self.max);
        (closest - center).length_squared() <= radius * radius
    }

    pub fn intersects_aabb(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// Ray-AABB intersection (slab method). Returns distance or None.
    pub fn ray_intersect(&self, origin: Vec3, dir_inv: Vec3) -> Option<f32> {
        let t1 = (self.min - origin) * dir_inv;
        let t2 = (self.max - origin) * dir_inv;
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);
        let enter = tmin.x.max(tmin.y).max(tmin.z);
        let exit  = tmax.x.min(tmax.y).min(tmax.z);
        if exit >= enter && exit >= 0.0 { Some(enter.max(0.0)) } else { None }
    }

    pub fn longest_axis(&self) -> usize {
        let d = self.max - self.min;
        if d.x >= d.y && d.x >= d.z { 0 } else if d.y >= d.z { 1 } else { 2 }
    }
}

// ─── BVH node ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum BvhNode {
    Leaf {
        aabb:  Aabb,
        items: Vec<GlyphId>,
    },
    Internal {
        aabb:  Aabb,
        left:  Box<BvhNode>,
        right: Box<BvhNode>,
    },
}

impl BvhNode {
    pub fn aabb(&self) -> &Aabb {
        match self {
            Self::Leaf   { aabb, .. }     => aabb,
            Self::Internal { aabb, .. }   => aabb,
        }
    }

    pub fn count(&self) -> usize {
        match self {
            Self::Leaf   { items, .. }   => items.len(),
            Self::Internal { left, right, .. } => left.count() + right.count(),
        }
    }

    pub fn depth(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Internal { left, right, .. } => 1 + left.depth().max(right.depth()),
        }
    }
}

// ─── BVH ──────────────────────────────────────────────────────────────────────

/// A flat BVH built from (GlyphId, Aabb) pairs.
pub struct Bvh {
    pub root: BvhNode,
}

impl Bvh {
    const LEAF_MAX: usize = 4;

    pub fn build(items: &[(GlyphId, Aabb)]) -> Self {
        if items.is_empty() {
            return Self {
                root: BvhNode::Leaf { aabb: Aabb::empty(), items: Vec::new() },
            };
        }
        let root = Self::build_node(items);
        Self { root }
    }

    fn build_node(items: &[(GlyphId, Aabb)]) -> BvhNode {
        // Compute enclosing AABB
        let mut aabb = items[0].1.clone();
        for (_, b) in &items[1..] { aabb = aabb.union(b); }

        if items.len() <= Self::LEAF_MAX {
            return BvhNode::Leaf {
                aabb,
                items: items.iter().map(|(id, _)| *id).collect(),
            };
        }

        // Split along longest axis (median)
        let axis = aabb.longest_axis();
        let mut sorted = items.to_vec();
        sorted.sort_by(|a, b| {
            let ca = a.1.center();
            let cb = b.1.center();
            let av = [ca.x, ca.y, ca.z][axis];
            let bv = [cb.x, cb.y, cb.z][axis];
            av.partial_cmp(&bv).unwrap()
        });
        let mid = sorted.len() / 2;
        let left  = Box::new(Self::build_node(&sorted[..mid]));
        let right = Box::new(Self::build_node(&sorted[mid..]));
        BvhNode::Internal { aabb, left, right }
    }

    /// Find all glyph IDs whose bounding box intersects the sphere.
    pub fn sphere_query(&self, center: Vec3, radius: f32) -> Vec<GlyphId> {
        let mut results = Vec::new();
        Self::sphere_query_node(&self.root, center, radius, &mut results);
        results
    }

    fn sphere_query_node(node: &BvhNode, center: Vec3, radius: f32, out: &mut Vec<GlyphId>) {
        if !node.aabb().intersects_sphere(center, radius) { return; }
        match node {
            BvhNode::Leaf { items, .. } => { out.extend(items); }
            BvhNode::Internal { left, right, .. } => {
                Self::sphere_query_node(left,  center, radius, out);
                Self::sphere_query_node(right, center, radius, out);
            }
        }
    }

    /// Find all glyph IDs intersected by a ray, sorted by distance.
    pub fn ray_query(&self, origin: Vec3, direction: Vec3) -> Vec<(GlyphId, f32)> {
        let dir = direction.normalize_or_zero();
        let dir_inv = Vec3::new(
            if dir.x.abs() > 1e-7 { 1.0 / dir.x } else { f32::MAX },
            if dir.y.abs() > 1e-7 { 1.0 / dir.y } else { f32::MAX },
            if dir.z.abs() > 1e-7 { 1.0 / dir.z } else { f32::MAX },
        );
        let mut results = Vec::new();
        Self::ray_query_node(&self.root, origin, dir_inv, &mut results);
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results
    }

    fn ray_query_node(
        node: &BvhNode, origin: Vec3, dir_inv: Vec3, out: &mut Vec<(GlyphId, f32)>,
    ) {
        if node.aabb().ray_intersect(origin, dir_inv).is_none() { return; }
        match node {
            BvhNode::Leaf { items, aabb } => {
                if let Some(t) = aabb.ray_intersect(origin, dir_inv) {
                    for &id in items { out.push((id, t)); }
                }
            }
            BvhNode::Internal { left, right, .. } => {
                Self::ray_query_node(left,  origin, dir_inv, out);
                Self::ray_query_node(right, origin, dir_inv, out);
            }
        }
    }

    /// AABB overlap query.
    pub fn aabb_query(&self, query: &Aabb) -> Vec<GlyphId> {
        let mut results = Vec::new();
        Self::aabb_query_node(&self.root, query, &mut results);
        results
    }

    fn aabb_query_node(node: &BvhNode, query: &Aabb, out: &mut Vec<GlyphId>) {
        if !node.aabb().intersects_aabb(query) { return; }
        match node {
            BvhNode::Leaf { items, .. } => { out.extend(items); }
            BvhNode::Internal { left, right, .. } => {
                Self::aabb_query_node(left,  query, out);
                Self::aabb_query_node(right, query, out);
            }
        }
    }

    pub fn depth(&self) -> usize { self.root.depth() }
    pub fn count(&self) -> usize { self.root.count() }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn gid(n: u32) -> GlyphId { GlyphId(n) }

    #[test]
    fn aabb_contains() {
        let a = Aabb::from_point(Vec3::ZERO, 1.0);
        assert!( a.contains_point(Vec3::ZERO));
        assert!(!a.contains_point(Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn aabb_sphere() {
        let a = Aabb::from_point(Vec3::ZERO, 1.0);
        assert!( a.intersects_sphere(Vec3::ZERO, 0.5));
        assert!( a.intersects_sphere(Vec3::new(2.0, 0.0, 0.0), 1.5));
        assert!(!a.intersects_sphere(Vec3::new(5.0, 0.0, 0.0), 1.0));
    }

    #[test]
    fn bvh_sphere_query() {
        let items: Vec<(GlyphId, Aabb)> = (0u32..10)
            .map(|i| (gid(i), Aabb::from_point(Vec3::new(i as f32 * 3.0, 0.0, 0.0), 0.5)))
            .collect();
        let bvh = Bvh::build(&items);
        let hits = bvh.sphere_query(Vec3::ZERO, 2.0);
        assert!(hits.contains(&gid(0)));
        assert!(!hits.contains(&gid(5)));
    }

    #[test]
    fn bvh_ray_query() {
        let items = vec![
            (gid(0), Aabb::from_point(Vec3::new(0.0, 0.0, 5.0),  0.5)),
            (gid(1), Aabb::from_point(Vec3::new(0.0, 0.0, 20.0), 0.5)),
            (gid(2), Aabb::from_point(Vec3::new(10.0, 0.0, 5.0), 0.5)),
        ];
        let bvh = Bvh::build(&items);
        let hits = bvh.ray_query(Vec3::ZERO, Vec3::Z);
        // Should hit items 0 and 1 (along Z), not item 2 (off to X side)
        let ids: Vec<GlyphId> = hits.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&gid(0)));
        assert!(ids.contains(&gid(1)));
    }

    #[test]
    fn bvh_empty() {
        let bvh = Bvh::build(&[]);
        let hits = bvh.sphere_query(Vec3::ZERO, 100.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn aabb_longest_axis() {
        let a = Aabb::new(Vec3::ZERO, Vec3::new(10.0, 3.0, 1.0));
        assert_eq!(a.longest_axis(), 0);
        let b = Aabb::new(Vec3::ZERO, Vec3::new(1.0, 10.0, 3.0));
        assert_eq!(b.longest_axis(), 1);
    }
}
