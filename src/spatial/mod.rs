//! Spatial acceleration structures for fast proximity queries.
//!
//! # Structures
//!
//! - `SpatialGrid`   — uniform 3D grid (O(1) insert, O(k) range query)
//! - `SpatialGrid2D` — 2D version for screen-space queries
//! - `BvhNode`       — bounding volume hierarchy for static geometry
//! - `KdTree`        — k-d tree for nearest-neighbor queries
//! - `SpatialIndex`  — unified trait for all spatial structures
//!
//! Used for:
//! - Fast glyph proximity (cohesion, repulsion forces)
//! - Collision detection between entities
//! - Field influence queries
//! - Particle flocking neighbor search

use glam::{Vec2, Vec3};
use std::collections::HashMap;

// ── SpatialIndex trait ────────────────────────────────────────────────────────

/// Common interface for spatial acceleration structures.
pub trait SpatialIndex<T: Clone> {
    /// Insert an item at the given position.
    fn insert(&mut self, pos: Vec3, item: T);

    /// Query all items within `radius` of `center`.
    fn query_radius(&self, center: Vec3, radius: f32) -> Vec<(T, Vec3, f32)>;

    /// Query the `k` nearest items to `center`.
    fn k_nearest(&self, center: Vec3, k: usize) -> Vec<(T, Vec3, f32)>;

    /// Remove all items.
    fn clear(&mut self);

    /// Total number of stored items.
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool { self.len() == 0 }
}

// ── SpatialGrid ───────────────────────────────────────────────────────────────

/// A uniform 3D spatial hash grid.
///
/// Space is divided into cubic cells of `cell_size`. Items are stored in
/// buckets by their cell coordinate. Queries scan all cells overlapping
/// the query sphere.
///
/// # Complexity
///
/// - Insert: O(1) average
/// - Range query: O(k + m) where k = items in range, m = cells overlapping sphere
/// - Rebuild: O(n)
pub struct SpatialGrid<T: Clone> {
    /// Cell size (world units per cell edge).
    pub cell_size: f32,
    /// Grid cells: (cx, cy, cz) → [(position, item)].
    cells: HashMap<(i32, i32, i32), Vec<(Vec3, T)>>,
    item_count: usize,
}

impl<T: Clone> SpatialGrid<T> {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(0.001),
            cells: HashMap::new(),
            item_count: 0,
        }
    }

    fn cell_key(&self, pos: Vec3) -> (i32, i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
            (pos.z / self.cell_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, pos: Vec3, item: T) {
        let key = self.cell_key(pos);
        self.cells.entry(key).or_default().push((pos, item));
        self.item_count += 1;
    }

    /// Query all items within `radius` of `center`, returning (item, position, distance).
    pub fn query_radius(&self, center: Vec3, radius: f32) -> Vec<(T, Vec3, f32)> {
        let r2 = radius * radius;
        let cell_r = (radius / self.cell_size).ceil() as i32;
        let (cx, cy, cz) = self.cell_key(center);

        let mut results = Vec::new();
        for dx in -cell_r..=cell_r {
            for dy in -cell_r..=cell_r {
                for dz in -cell_r..=cell_r {
                    let key = (cx + dx, cy + dy, cz + dz);
                    if let Some(bucket) = self.cells.get(&key) {
                        for (pos, item) in bucket {
                            let d2 = (*pos - center).length_squared();
                            if d2 <= r2 {
                                results.push((item.clone(), *pos, d2.sqrt()));
                            }
                        }
                    }
                }
            }
        }
        results
    }

    /// Find the k nearest items to `center`.
    pub fn k_nearest(&self, center: Vec3, k: usize) -> Vec<(T, Vec3, f32)> {
        // Start with small radius and expand until we have k results
        let mut radius = self.cell_size;
        let mut results;
        loop {
            results = self.query_radius(center, radius);
            if results.len() >= k || radius > 1000.0 { break; }
            radius *= 2.0;
        }
        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Find the single nearest item.
    pub fn nearest(&self, center: Vec3) -> Option<(T, Vec3, f32)> {
        self.k_nearest(center, 1).into_iter().next()
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.item_count = 0;
    }

    pub fn len(&self) -> usize { self.item_count }
    pub fn is_empty(&self) -> bool { self.item_count == 0 }
    pub fn bucket_count(&self) -> usize { self.cells.len() }

    /// Average items per occupied bucket (load factor).
    pub fn avg_bucket_load(&self) -> f32 {
        if self.cells.is_empty() { return 0.0; }
        self.item_count as f32 / self.cells.len() as f32
    }

    /// Rebuild from an iterator of (position, item) pairs.
    pub fn rebuild(&mut self, items: impl Iterator<Item = (Vec3, T)>) {
        self.clear();
        for (pos, item) in items {
            self.insert(pos, item);
        }
    }

    /// Iterate over all items.
    pub fn iter(&self) -> impl Iterator<Item = (&Vec3, &T)> {
        self.cells.values().flat_map(|bucket| bucket.iter().map(|(p, t)| (p, t)))
    }
}

// ── SpatialGrid2D ─────────────────────────────────────────────────────────────

/// A uniform 2D spatial hash grid for screen-space queries.
pub struct SpatialGrid2D<T: Clone> {
    pub cell_size:  f32,
    cells: HashMap<(i32, i32), Vec<(Vec2, T)>>,
    item_count: usize,
}

impl<T: Clone> SpatialGrid2D<T> {
    pub fn new(cell_size: f32) -> Self {
        Self { cell_size: cell_size.max(0.001), cells: HashMap::new(), item_count: 0 }
    }

    fn cell_key(&self, pos: Vec2) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, pos: Vec2, item: T) {
        let key = self.cell_key(pos);
        self.cells.entry(key).or_default().push((pos, item));
        self.item_count += 1;
    }

    pub fn query_radius(&self, center: Vec2, radius: f32) -> Vec<(T, Vec2, f32)> {
        let r2 = radius * radius;
        let cell_r = (radius / self.cell_size).ceil() as i32;
        let (cx, cy) = self.cell_key(center);
        let mut results = Vec::new();
        for dx in -cell_r..=cell_r {
            for dy in -cell_r..=cell_r {
                if let Some(bucket) = self.cells.get(&(cx + dx, cy + dy)) {
                    for (pos, item) in bucket {
                        let d2 = (*pos - center).length_squared();
                        if d2 <= r2 {
                            results.push((item.clone(), *pos, d2.sqrt()));
                        }
                    }
                }
            }
        }
        results
    }

    pub fn k_nearest(&self, center: Vec2, k: usize) -> Vec<(T, Vec2, f32)> {
        let mut radius = self.cell_size;
        let mut results;
        loop {
            results = self.query_radius(center, radius);
            if results.len() >= k || radius > 10000.0 { break; }
            radius *= 2.0;
        }
        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    pub fn clear(&mut self) { self.cells.clear(); self.item_count = 0; }
    pub fn len(&self) -> usize { self.item_count }
    pub fn is_empty(&self) -> bool { self.item_count == 0 }
}

// ── AABB ──────────────────────────────────────────────────────────────────────

/// Axis-aligned bounding box in 3D.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }

    pub fn from_center_half_extents(center: Vec3, half: Vec3) -> Self {
        Self { min: center - half, max: center + half }
    }

    pub fn from_sphere(center: Vec3, radius: f32) -> Self {
        let r = Vec3::splat(radius);
        Self { min: center - r, max: center + r }
    }

    pub fn center(&self) -> Vec3 { (self.min + self.max) * 0.5 }
    pub fn half_extents(&self) -> Vec3 { (self.max - self.min) * 0.5 }
    pub fn size(&self) -> Vec3 { self.max - self.min }

    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x
            && p.y >= self.min.y && p.y <= self.max.y
            && p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        let closest = center.clamp(self.min, self.max);
        (closest - center).length_squared() <= radius * radius
    }

    /// Expand to include another AABB.
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Expand by `amount` in all directions.
    pub fn expand(&self, amount: f32) -> Aabb {
        let e = Vec3::splat(amount);
        Aabb { min: self.min - e, max: self.max + e }
    }

    pub fn surface_area(&self) -> f32 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }

    /// Ray-AABB intersection. Returns (t_min, t_max) if hit, None otherwise.
    pub fn ray_intersect(&self, origin: Vec3, dir: Vec3) -> Option<(f32, f32)> {
        let inv_dir = Vec3::new(
            if dir.x != 0.0 { 1.0 / dir.x } else { f32::INFINITY },
            if dir.y != 0.0 { 1.0 / dir.y } else { f32::INFINITY },
            if dir.z != 0.0 { 1.0 / dir.z } else { f32::INFINITY },
        );
        let t1 = (self.min - origin) * inv_dir;
        let t2 = (self.max - origin) * inv_dir;
        let t_min = t1.min(t2).max_element();
        let t_max = t1.max(t2).min_element();
        if t_max >= t_min && t_max >= 0.0 { Some((t_min, t_max)) } else { None }
    }
}

// ── BvhNode ───────────────────────────────────────────────────────────────────

/// A node in a Bounding Volume Hierarchy.
///
/// BVH provides O(log n) ray queries and O(log n) sphere overlap queries.
/// Build using `Bvh::build(items)` where each item is an `(Aabb, T)` pair.
#[derive(Debug, Clone)]
pub enum BvhNode<T: Clone> {
    Leaf {
        bounds: Aabb,
        item:   T,
    },
    Branch {
        bounds: Aabb,
        left:   Box<BvhNode<T>>,
        right:  Box<BvhNode<T>>,
    },
}

impl<T: Clone> BvhNode<T> {
    pub fn bounds(&self) -> Aabb {
        match self {
            BvhNode::Leaf { bounds, .. } => *bounds,
            BvhNode::Branch { bounds, .. } => *bounds,
        }
    }

    /// Query all items whose AABB overlaps `sphere(center, radius)`.
    pub fn query_sphere(&self, center: Vec3, radius: f32, out: &mut Vec<T>) {
        if !self.bounds().intersects_sphere(center, radius) { return; }
        match self {
            BvhNode::Leaf { item, .. } => out.push(item.clone()),
            BvhNode::Branch { left, right, .. } => {
                left.query_sphere(center, radius, out);
                right.query_sphere(center, radius, out);
            }
        }
    }

    /// Ray query — returns all items whose AABB the ray intersects.
    pub fn query_ray(&self, origin: Vec3, dir: Vec3, max_t: f32, out: &mut Vec<(T, f32)>) {
        match self.bounds().ray_intersect(origin, dir) {
            None => return,
            Some((t_min, _)) if t_min > max_t => return,
            _ => {}
        }
        match self {
            BvhNode::Leaf { item, bounds } => {
                if let Some((t, _)) = bounds.ray_intersect(origin, dir) {
                    out.push((item.clone(), t));
                }
            }
            BvhNode::Branch { left, right, .. } => {
                left.query_ray(origin, dir, max_t, out);
                right.query_ray(origin, dir, max_t, out);
            }
        }
    }

    /// AABB overlap query.
    pub fn query_aabb(&self, query: &Aabb, out: &mut Vec<T>) {
        if !self.bounds().intersects(query) { return; }
        match self {
            BvhNode::Leaf { item, bounds } => {
                if bounds.intersects(query) { out.push(item.clone()); }
            }
            BvhNode::Branch { left, right, .. } => {
                left.query_aabb(query, out);
                right.query_aabb(query, out);
            }
        }
    }
}

/// A complete BVH tree.
pub struct Bvh<T: Clone> {
    root: Option<BvhNode<T>>,
    pub item_count: usize,
}

impl<T: Clone> Bvh<T> {
    /// Build a BVH from a list of (bounds, item) pairs using SAH heuristic.
    pub fn build(items: Vec<(Aabb, T)>) -> Self {
        let count = items.len();
        let root = if items.is_empty() { None } else { Some(Self::build_recursive(items)) };
        Self { root, item_count: count }
    }

    fn build_recursive(mut items: Vec<(Aabb, T)>) -> BvhNode<T> {
        if items.len() == 1 {
            let (bounds, item) = items.remove(0);
            return BvhNode::Leaf { bounds, item };
        }

        // Compute combined bounds
        let bounds = items.iter()
            .map(|(b, _)| *b)
            .reduce(|a, b| a.union(&b))
            .unwrap();

        // Split along the longest axis at the centroid median
        let size = bounds.size();
        let axis = if size.x >= size.y && size.x >= size.z { 0 }
                   else if size.y >= size.z { 1 }
                   else { 2 };

        let centroid = |b: &Aabb| -> f32 {
            match axis { 0 => b.center().x, 1 => b.center().y, _ => b.center().z }
        };

        items.sort_by(|(a, _), (b, _)| centroid(a).partial_cmp(&centroid(b))
            .unwrap_or(std::cmp::Ordering::Equal));

        let mid = items.len() / 2;
        let right_items = items.split_off(mid);
        let left_items = items;

        let left = Box::new(Self::build_recursive(left_items));
        let right = Box::new(Self::build_recursive(right_items));

        BvhNode::Branch { bounds, left, right }
    }

    pub fn query_sphere(&self, center: Vec3, radius: f32) -> Vec<T> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            root.query_sphere(center, radius, &mut out);
        }
        out
    }

    pub fn query_ray(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Vec<(T, f32)> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            root.query_ray(origin, dir.normalize_or_zero(), max_t, &mut out);
        }
        out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn query_aabb(&self, bounds: &Aabb) -> Vec<T> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            root.query_aabb(bounds, &mut out);
        }
        out
    }

    pub fn is_empty(&self) -> bool { self.root.is_none() }
    pub fn item_count(&self) -> usize { self.item_count }
}

// ── KdTree ────────────────────────────────────────────────────────────────────

/// A 3D k-d tree for efficient nearest-neighbor queries.
///
/// Best for static point clouds queried repeatedly.
/// Build with `KdTree::build(points)`.
#[derive(Debug, Clone)]
pub struct KdTree<T: Clone> {
    nodes: Vec<KdNode<T>>,
    pub item_count: usize,
}

#[derive(Debug, Clone)]
struct KdNode<T: Clone> {
    pos:   Vec3,
    item:  T,
    left:  Option<usize>,
    right: Option<usize>,
    axis:  u8,  // 0=x, 1=y, 2=z
}

impl<T: Clone> KdTree<T> {
    /// Build a k-d tree from a list of (position, item) pairs.
    pub fn build(points: Vec<(Vec3, T)>) -> Self {
        let count = points.len();
        let mut tree = Self { nodes: Vec::with_capacity(count), item_count: count };
        if !points.is_empty() {
            tree.build_recursive(points, 0);
        }
        tree
    }

    fn build_recursive(&mut self, mut points: Vec<(Vec3, T)>, depth: usize) -> usize {
        let axis = (depth % 3) as u8;

        // Sort by the current axis
        points.sort_by(|a, b| {
            let va = match axis { 0 => a.0.x, 1 => a.0.y, _ => a.0.z };
            let vb = match axis { 0 => b.0.x, 1 => b.0.y, _ => b.0.z };
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = points.len() / 2;
        let mut right_points = points.split_off(mid + 1);
        let left_points = points.split_off(mid);
        let (pos, item) = points.remove(0);

        let idx = self.nodes.len();
        self.nodes.push(KdNode { pos, item, left: None, right: None, axis });

        if !left_points.is_empty() {
            let left_idx = self.build_recursive(left_points, depth + 1);
            self.nodes[idx].left = Some(left_idx);
        }
        if !right_points.is_empty() {
            let right_idx = self.build_recursive(right_points, depth + 1);
            self.nodes[idx].right = Some(right_idx);
        }

        idx
    }

    /// Find the k nearest neighbors to `query`.
    /// Returns (item, position, distance) tuples sorted by distance.
    pub fn k_nearest(&self, query: Vec3, k: usize) -> Vec<(T, Vec3, f32)> {
        if self.nodes.is_empty() { return Vec::new(); }
        let mut heap: Vec<(f32, usize)> = Vec::new(); // (dist_sq, node_idx)
        self.nn_search(0, query, k, &mut heap);
        heap.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        heap.into_iter().map(|(d2, idx)| {
            let node = &self.nodes[idx];
            (node.item.clone(), node.pos, d2.sqrt())
        }).collect()
    }

    fn nn_search(&self, node_idx: usize, query: Vec3, k: usize, heap: &mut Vec<(f32, usize)>) {
        let node = &self.nodes[node_idx];
        let d2 = (node.pos - query).length_squared();

        // Check if this node belongs in the heap
        let worst = heap.iter().map(|&(d, _)| d).fold(f32::NEG_INFINITY, f32::max);
        if heap.len() < k || d2 < worst {
            heap.push((d2, node_idx));
            if heap.len() > k {
                // Remove worst
                let worst_idx = heap.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.0.partial_cmp(&b.1.0).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i)
                    .unwrap();
                heap.swap_remove(worst_idx);
            }
        }

        // Determine which subtree to explore first
        let axis_val = match node.axis { 0 => query.x, 1 => query.y, _ => query.z };
        let node_val = match node.axis { 0 => node.pos.x, 1 => node.pos.y, _ => node.pos.z };
        let (near, far) = if axis_val <= node_val {
            (node.left, node.right)
        } else {
            (node.right, node.left)
        };

        if let Some(near_idx) = near {
            self.nn_search(near_idx, query, k, heap);
        }

        // Check if far side could have closer points
        let plane_dist_sq = (axis_val - node_val) * (axis_val - node_val);
        let current_worst = heap.iter().map(|&(d, _)| d).fold(f32::NEG_INFINITY, f32::max);
        if let Some(far_idx) = far {
            if heap.len() < k || plane_dist_sq < current_worst {
                self.nn_search(far_idx, query, k, heap);
            }
        }
    }

    /// Find all points within `radius` of `query`.
    pub fn radius_search(&self, query: Vec3, radius: f32) -> Vec<(T, Vec3, f32)> {
        if self.nodes.is_empty() { return Vec::new(); }
        let mut results = Vec::new();
        self.range_search(0, query, radius * radius, &mut results);
        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    fn range_search(&self, node_idx: usize, query: Vec3, r2: f32, out: &mut Vec<(T, Vec3, f32)>) {
        let node = &self.nodes[node_idx];
        let d2 = (node.pos - query).length_squared();
        if d2 <= r2 {
            out.push((node.item.clone(), node.pos, d2.sqrt()));
        }

        let axis_val = match node.axis { 0 => query.x, 1 => query.y, _ => query.z };
        let node_val = match node.axis { 0 => node.pos.x, 1 => node.pos.y, _ => node.pos.z };
        let plane_d2 = (axis_val - node_val) * (axis_val - node_val);

        let (near, far) = if axis_val <= node_val {
            (node.left, node.right)
        } else {
            (node.right, node.left)
        };

        if let Some(near_idx) = near {
            self.range_search(near_idx, query, r2, out);
        }
        if plane_d2 <= r2 {
            if let Some(far_idx) = far {
                self.range_search(far_idx, query, r2, out);
            }
        }
    }

    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }
    pub fn len(&self) -> usize { self.item_count }
}

// ── Frustum culling ───────────────────────────────────────────────────────────

/// A camera frustum for view culling.
///
/// Used to discard objects outside the camera view before rendering.
/// Defined by six half-space planes: left, right, top, bottom, near, far.
#[derive(Debug, Clone)]
pub struct Frustum {
    /// Six plane normals (pointing inward).
    planes: [(Vec3, f32); 6],  // (normal, d) where n·p + d >= 0 means inside
}

impl Frustum {
    /// Build a frustum from a view-projection matrix.
    ///
    /// Works with row-major matrices (as provided by `glam::Mat4`).
    pub fn from_matrix(vp: glam::Mat4) -> Self {
        let m = vp.to_cols_array_2d();
        // Gribb-Hartmann method
        let planes_raw = [
            // Left:   row3 + row0
            [m[0][3]+m[0][0], m[1][3]+m[1][0], m[2][3]+m[2][0], m[3][3]+m[3][0]],
            // Right:  row3 - row0
            [m[0][3]-m[0][0], m[1][3]-m[1][0], m[2][3]-m[2][0], m[3][3]-m[3][0]],
            // Bottom: row3 + row1
            [m[0][3]+m[0][1], m[1][3]+m[1][1], m[2][3]+m[2][1], m[3][3]+m[3][1]],
            // Top:    row3 - row1
            [m[0][3]-m[0][1], m[1][3]-m[1][1], m[2][3]-m[2][1], m[3][3]-m[3][1]],
            // Near:   row3 + row2
            [m[0][3]+m[0][2], m[1][3]+m[1][2], m[2][3]+m[2][2], m[3][3]+m[3][2]],
            // Far:    row3 - row2
            [m[0][3]-m[0][2], m[1][3]-m[1][2], m[2][3]-m[2][2], m[3][3]-m[3][2]],
        ];

        let mut planes = [(Vec3::ZERO, 0.0_f32); 6];
        for (i, raw) in planes_raw.iter().enumerate() {
            let n = Vec3::new(raw[0], raw[1], raw[2]);
            let len = n.length().max(1e-6);
            planes[i] = (n / len, raw[3] / len);
        }
        Self { planes }
    }

    /// Test if a sphere overlaps the frustum.
    pub fn sphere_inside(&self, center: Vec3, radius: f32) -> bool {
        for &(n, d) in &self.planes {
            if n.dot(center) + d < -radius {
                return false;
            }
        }
        true
    }

    /// Test if an AABB overlaps the frustum (conservative test).
    pub fn aabb_inside(&self, bounds: &Aabb) -> bool {
        let center = bounds.center();
        let half = bounds.half_extents();
        for &(n, d) in &self.planes {
            // Compute positive vertex (farthest in plane normal direction)
            let r = half.x * n.x.abs() + half.y * n.y.abs() + half.z * n.z.abs();
            if n.dot(center) + d < -r {
                return false;
            }
        }
        true
    }

    /// Test if a point is inside the frustum.
    pub fn point_inside(&self, p: Vec3) -> bool {
        self.planes.iter().all(|&(n, d)| n.dot(p) + d >= 0.0)
    }
}

// ── Proximity pairs ───────────────────────────────────────────────────────────

/// Find all pairs of points closer than `max_dist`.
///
/// Returns `(i, j, distance)` for each pair where i < j.
/// Uses a spatial grid for O(n log n) performance.
pub fn find_close_pairs(positions: &[Vec3], max_dist: f32) -> Vec<(usize, usize, f32)> {
    let mut grid: SpatialGrid<usize> = SpatialGrid::new(max_dist);
    for (i, &pos) in positions.iter().enumerate() {
        grid.insert(pos, i);
    }

    let mut pairs = Vec::new();
    let r2 = max_dist * max_dist;
    for (i, &pos) in positions.iter().enumerate() {
        let nearby = grid.query_radius(pos, max_dist);
        for (j, npos, _) in nearby {
            if j > i {
                let d2 = (pos - npos).length_squared();
                if d2 <= r2 {
                    pairs.push((i, j, d2.sqrt()));
                }
            }
        }
    }
    pairs
}

/// Find all positions within `radius` of any of the given `query_points`.
///
/// Returns pairs of (query_index, position_index, distance).
pub fn batch_radius_query(
    query_points: &[Vec3],
    positions: &[Vec3],
    radius: f32,
) -> Vec<(usize, usize, f32)> {
    let mut grid: SpatialGrid<usize> = SpatialGrid::new(radius);
    for (i, &pos) in positions.iter().enumerate() {
        grid.insert(pos, i);
    }
    let mut results = Vec::new();
    for (qi, &qpos) in query_points.iter().enumerate() {
        for (idx, _, dist) in grid.query_radius(qpos, radius) {
            results.push((qi, idx, dist));
        }
    }
    results
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_grid_insert_query() {
        let mut grid: SpatialGrid<u32> = SpatialGrid::new(1.0);
        grid.insert(Vec3::new(0.5, 0.5, 0.5), 1);
        grid.insert(Vec3::new(10.0, 10.0, 10.0), 2);

        let near = grid.query_radius(Vec3::ZERO, 2.0);
        assert_eq!(near.len(), 1, "should find 1 item near origin");
        assert_eq!(*near[0].0, 1u32);
    }

    #[test]
    fn spatial_grid_k_nearest() {
        let mut grid: SpatialGrid<usize> = SpatialGrid::new(0.5);
        for i in 0..10 {
            grid.insert(Vec3::new(i as f32, 0.0, 0.0), i);
        }
        let nn = grid.k_nearest(Vec3::new(4.5, 0.0, 0.0), 2);
        assert_eq!(nn.len(), 2);
        // Nearest two should be 4 and 5
        let mut ids: Vec<usize> = nn.iter().map(|(id, _, _)| **id).collect();
        ids.sort();
        assert_eq!(ids, vec![4, 5]);
    }

    #[test]
    fn aabb_intersects() {
        let a = Aabb::new(Vec3::ZERO, Vec3::ONE);
        let b = Aabb::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5));
        let c = Aabb::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0));
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn aabb_ray_hit() {
        let aabb = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
        let hit = aabb.ray_intersect(Vec3::new(-5.0, 0.0, 0.0), Vec3::X);
        assert!(hit.is_some(), "ray should hit the AABB");
        let miss = aabb.ray_intersect(Vec3::new(-5.0, 5.0, 0.0), Vec3::X);
        assert!(miss.is_none(), "ray should miss");
    }

    #[test]
    fn bvh_sphere_query() {
        let items: Vec<(Aabb, usize)> = (0..10).map(|i| {
            let c = Vec3::new(i as f32, 0.0, 0.0);
            (Aabb::from_sphere(c, 0.4), i)
        }).collect();
        let bvh = Bvh::build(items);
        let hits = bvh.query_sphere(Vec3::new(4.5, 0.0, 0.0), 1.0);
        // Items at x=4 and x=5 should be hit
        assert!(hits.len() >= 2, "should hit at least 2 items, got {}", hits.len());
    }

    #[test]
    fn kd_tree_nearest() {
        let points: Vec<(Vec3, usize)> = (0..10).map(|i| {
            (Vec3::new(i as f32, 0.0, 0.0), i)
        }).collect();
        let tree = KdTree::build(points);
        let nn = tree.k_nearest(Vec3::new(3.1, 0.0, 0.0), 1);
        assert_eq!(nn.len(), 1);
        assert_eq!(*nn[0].0, 3usize);
    }

    #[test]
    fn kd_tree_radius_search() {
        let points: Vec<(Vec3, usize)> = (0..20).map(|i| {
            (Vec3::new(i as f32, 0.0, 0.0), i)
        }).collect();
        let tree = KdTree::build(points);
        let results = tree.radius_search(Vec3::new(10.0, 0.0, 0.0), 2.5);
        // Should find 8,9,10,11,12
        assert!(results.len() >= 4, "expected at least 4 in radius, got {}", results.len());
    }

    #[test]
    fn find_close_pairs_correct() {
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        let pairs = find_close_pairs(&positions, 1.0);
        assert_eq!(pairs.len(), 1, "should find exactly one close pair");
        assert_eq!(pairs[0].0, 0);
        assert_eq!(pairs[0].1, 1);
    }

    #[test]
    fn spatial_grid_2d_query() {
        let mut grid: SpatialGrid2D<u32> = SpatialGrid2D::new(1.0);
        grid.insert(Vec2::new(0.5, 0.5), 10);
        grid.insert(Vec2::new(5.0, 5.0), 20);
        let near = grid.query_radius(Vec2::ZERO, 1.5);
        assert_eq!(near.len(), 1);
        assert_eq!(*near[0].0, 10u32);
    }
}
