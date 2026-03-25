//! Generate 3D extruded meshes from 2D glyph outlines.
//!
//! Pipeline: GlyphOutline → triangulate front face → extrude → side faces → GlyphMesh
//!
//! Triangulation uses ear clipping with hole bridging. Extrusion duplicates the
//! front face offset along Z. Side faces connect corresponding edge vertices.

use glam::{Vec2, Vec3};
use std::collections::HashMap;

use super::font_to_mesh::{GlyphOutline, Contour, OutlineCache, assign_holes_to_outers, signed_area};

// ── Vertex3D ────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex3D {
    pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self {
            position: position.to_array(),
            normal: normal.to_array(),
            uv: uv.to_array(),
        }
    }

    pub fn pos(&self) -> Vec3 { Vec3::from(self.position) }
    pub fn norm(&self) -> Vec3 { Vec3::from(self.normal) }
}

// ── GlyphMesh ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GlyphMesh {
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<u32>,
    pub character: char,
    pub extrusion_depth: f32,
    pub triangle_count: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl GlyphMesh {
    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn index_count(&self) -> usize { self.indices.len() }

    pub fn vertex_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.vertices)
    }

    pub fn index_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.indices)
    }
}

#[derive(Clone, Debug)]
pub struct MeshStats {
    pub vertices: usize,
    pub triangles: usize,
    pub bytes: usize,
}

pub fn mesh_stats(mesh: &GlyphMesh) -> MeshStats {
    MeshStats {
        vertices: mesh.vertices.len(),
        triangles: mesh.triangle_count as usize,
        bytes: mesh.vertices.len() * std::mem::size_of::<Vertex3D>()
            + mesh.indices.len() * std::mem::size_of::<u32>(),
    }
}

// ── Ear Clipping Triangulation ──────────────────────────────────────────────

/// Check if point p is inside triangle (a, b, c) using barycentric coordinates.
pub fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;

    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);

    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= 0.0 && v >= 0.0 && u + v <= 1.0
}

/// Cross product of 2D vectors (returns Z component).
fn cross2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Check if vertex at `curr` is convex (left turn) in CCW polygon.
fn is_convex(polygon: &[Vec2], prev: usize, curr: usize, next: usize) -> bool {
    let a = polygon[prev];
    let b = polygon[curr];
    let c = polygon[next];
    cross2d(b - a, c - b) > 0.0
}

/// Check if the triangle (prev, curr, next) is an ear (no other vertex inside).
fn is_ear(polygon: &[Vec2], indices: &[usize], prev_idx: usize, curr_idx: usize, next_idx: usize) -> bool {
    let a = polygon[indices[prev_idx]];
    let b = polygon[indices[curr_idx]];
    let c = polygon[indices[next_idx]];

    // Must be convex
    if cross2d(b - a, c - b) <= 0.0 {
        return false;
    }

    // No other vertex inside
    for (i, &vi) in indices.iter().enumerate() {
        if i == prev_idx || i == curr_idx || i == next_idx { continue; }
        if point_in_triangle(polygon[vi], a, b, c) {
            return false;
        }
    }

    true
}

/// Bridge a hole into the outer contour by finding a mutual visibility edge.
pub fn bridge_hole(outer: &[Vec2], hole: &[Vec2]) -> Vec<Vec2> {
    if hole.is_empty() { return outer.to_vec(); }

    // Find rightmost point in hole
    let mut rightmost_idx = 0;
    let mut max_x = f32::MIN;
    for (i, p) in hole.iter().enumerate() {
        if p.x > max_x {
            max_x = p.x;
            rightmost_idx = i;
        }
    }

    // Find closest visible point in outer contour
    let hp = hole[rightmost_idx];
    let mut best_idx = 0;
    let mut best_dist = f32::MAX;
    for (i, p) in outer.iter().enumerate() {
        let d = (p.x - hp.x).abs() + (p.y - hp.y).abs();
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }

    // Build merged polygon: outer[..best_idx+1] + hole[rightmost..] + hole[..rightmost+1] + outer[best_idx..]
    let mut result = Vec::with_capacity(outer.len() + hole.len() + 2);
    for i in 0..=best_idx {
        result.push(outer[i]);
    }
    let n_hole = hole.len();
    for i in 0..=n_hole {
        result.push(hole[(rightmost_idx + i) % n_hole]);
    }
    // Bridge back
    result.push(outer[best_idx]);
    for i in (best_idx + 1)..outer.len() {
        result.push(outer[i]);
    }

    result
}

/// Ear clipping triangulation with hole support.
/// Returns triangle indices into the polygon array.
pub fn ear_clip_triangulate(outer: &[Vec2], holes: &[Vec<Vec2>]) -> Vec<[usize; 3]> {
    // Merge holes into outer contour
    let mut polygon = outer.to_vec();
    for hole in holes {
        polygon = bridge_hole(&polygon, hole);
    }

    if polygon.len() < 3 { return Vec::new(); }

    // Ensure CCW winding
    if signed_area(&polygon) < 0.0 {
        polygon.reverse();
    }

    let mut indices: Vec<usize> = (0..polygon.len()).collect();
    let mut triangles = Vec::new();
    let mut max_iters = polygon.len() * polygon.len();

    while indices.len() > 2 && max_iters > 0 {
        max_iters -= 1;
        let n = indices.len();
        let mut found_ear = false;

        for i in 0..n {
            let prev = (i + n - 1) % n;
            let next = (i + 1) % n;

            if is_ear(&polygon, &indices, prev, i, next) {
                triangles.push([indices[prev], indices[i], indices[next]]);
                indices.remove(i);
                found_ear = true;
                break;
            }
        }

        if !found_ear {
            // Degenerate polygon, force remove a vertex
            if indices.len() > 2 {
                let n = indices.len();
                triangles.push([indices[0], indices[1], indices[2]]);
                indices.remove(1);
            } else {
                break;
            }
        }
    }

    triangles
}

// ── Extrusion ───────────────────────────────────────────────────────────────

/// Generate a 3D extruded mesh from a glyph outline.
pub fn extrude_glyph(outline: &GlyphOutline, depth: f32, ch: char) -> GlyphMesh {
    let assignments = assign_holes_to_outers(&outline.contours);

    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();

    let bounds = outline.bounds;
    let bw = (bounds.max.x - bounds.min.x).max(0.001);
    let bh = (bounds.max.y - bounds.min.y).max(0.001);

    for (outer_idx, hole_indices) in &assignments {
        let outer = &outline.contours[*outer_idx].points;
        let holes: Vec<Vec<Vec2>> = hole_indices.iter()
            .map(|&hi| outline.contours[hi].points.clone())
            .collect();
        let hole_refs: Vec<&[Vec2]> = holes.iter().map(|h| h.as_slice()).collect();

        // Collect all contour points for this group
        let mut all_points: Vec<Vec2> = outer.clone();
        for h in &holes {
            all_points.extend_from_slice(h);
        }

        // Triangulate front face
        let hole_vecs: Vec<Vec<Vec2>> = holes.clone();
        let tris = ear_clip_triangulate(outer, &hole_vecs);

        // We need a merged polygon to map triangle indices to actual points
        let mut merged = outer.clone();
        for h in &holes {
            merged = bridge_hole(&merged, h);
        }

        let base_idx = all_vertices.len() as u32;

        // === FRONT FACE (Z = 0, normal = +Z) ===
        for p in &merged {
            let u = (p.x - bounds.min.x) / bw;
            let v = (p.y - bounds.min.y) / bh;
            all_vertices.push(Vertex3D::new(
                Vec3::new(p.x, p.y, 0.0),
                Vec3::Z,
                Vec2::new(u, v),
            ));
        }

        for tri in &tris {
            all_indices.push(base_idx + tri[0] as u32);
            all_indices.push(base_idx + tri[1] as u32);
            all_indices.push(base_idx + tri[2] as u32);
        }

        // === BACK FACE (Z = -depth, normal = -Z, reversed winding) ===
        let back_base = all_vertices.len() as u32;
        for p in &merged {
            let u = (p.x - bounds.min.x) / bw;
            let v = (p.y - bounds.min.y) / bh;
            all_vertices.push(Vertex3D::new(
                Vec3::new(p.x, p.y, -depth),
                -Vec3::Z,
                Vec2::new(u, v),
            ));
        }

        for tri in &tris {
            // Reverse winding for back face
            all_indices.push(back_base + tri[2] as u32);
            all_indices.push(back_base + tri[1] as u32);
            all_indices.push(back_base + tri[0] as u32);
        }

        // === SIDE FACES (connect front edge to back edge) ===
        // Use the original outer contour (and each hole contour) for side faces
        let mut edge_contours: Vec<&Vec<Vec2>> = vec![outer];
        for h in &holes {
            edge_contours.push(h);
        }

        for contour in edge_contours {
            let n = contour.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let p0 = contour[i];
                let p1 = contour[j];

                // Edge direction and outward normal
                let edge = p1 - p0;
                let normal_2d = Vec2::new(edge.y, -edge.x).normalize_or_zero();
                let normal = Vec3::new(normal_2d.x, normal_2d.y, 0.0);

                let edge_len = edge.length();
                let u0 = 0.0;
                let u1 = edge_len / bw;

                let side_base = all_vertices.len() as u32;

                // Front-left, Front-right, Back-right, Back-left
                all_vertices.push(Vertex3D::new(Vec3::new(p0.x, p0.y, 0.0), normal, Vec2::new(u0, 0.0)));
                all_vertices.push(Vertex3D::new(Vec3::new(p1.x, p1.y, 0.0), normal, Vec2::new(u1, 0.0)));
                all_vertices.push(Vertex3D::new(Vec3::new(p1.x, p1.y, -depth), normal, Vec2::new(u1, 1.0)));
                all_vertices.push(Vertex3D::new(Vec3::new(p0.x, p0.y, -depth), normal, Vec2::new(u0, 1.0)));

                // Two triangles for the quad
                all_indices.push(side_base);
                all_indices.push(side_base + 1);
                all_indices.push(side_base + 2);
                all_indices.push(side_base);
                all_indices.push(side_base + 2);
                all_indices.push(side_base + 3);
            }
        }
    }

    // If no assignments produced geometry, create a simple box as fallback
    if all_vertices.is_empty() {
        let b = outline.bounds;
        let min = Vec3::new(b.min.x, b.min.y, -depth);
        let max = Vec3::new(b.max.x, b.max.y, 0.0);
        return create_box_mesh(min, max, ch, depth);
    }

    // Compute bounds
    let mut bmin = Vec3::splat(f32::MAX);
    let mut bmax = Vec3::splat(f32::MIN);
    for v in &all_vertices {
        let p = v.pos();
        bmin = bmin.min(p);
        bmax = bmax.max(p);
    }

    let tri_count = all_indices.len() as u32 / 3;

    GlyphMesh {
        vertices: all_vertices,
        indices: all_indices,
        character: ch,
        extrusion_depth: depth,
        triangle_count: tri_count,
        bounds_min: bmin,
        bounds_max: bmax,
    }
}

fn create_box_mesh(min: Vec3, max: Vec3, ch: char, depth: f32) -> GlyphMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let faces: [(Vec3, Vec3, Vec3, Vec3, Vec3); 6] = [
        (Vec3::new(min.x,min.y,max.z), Vec3::new(max.x,min.y,max.z), Vec3::new(max.x,max.y,max.z), Vec3::new(min.x,max.y,max.z), Vec3::Z),
        (Vec3::new(max.x,min.y,min.z), Vec3::new(min.x,min.y,min.z), Vec3::new(min.x,max.y,min.z), Vec3::new(max.x,max.y,min.z), -Vec3::Z),
        (Vec3::new(min.x,max.y,max.z), Vec3::new(max.x,max.y,max.z), Vec3::new(max.x,max.y,min.z), Vec3::new(min.x,max.y,min.z), Vec3::Y),
        (Vec3::new(min.x,min.y,min.z), Vec3::new(max.x,min.y,min.z), Vec3::new(max.x,min.y,max.z), Vec3::new(min.x,min.y,max.z), -Vec3::Y),
        (Vec3::new(max.x,min.y,max.z), Vec3::new(max.x,min.y,min.z), Vec3::new(max.x,max.y,min.z), Vec3::new(max.x,max.y,max.z), Vec3::X),
        (Vec3::new(min.x,min.y,min.z), Vec3::new(min.x,min.y,max.z), Vec3::new(min.x,max.y,max.z), Vec3::new(min.x,max.y,min.z), -Vec3::X),
    ];

    for (v0, v1, v2, v3, n) in &faces {
        let base = vertices.len() as u32;
        vertices.push(Vertex3D::new(*v0, *n, Vec2::new(0.0, 0.0)));
        vertices.push(Vertex3D::new(*v1, *n, Vec2::new(1.0, 0.0)));
        vertices.push(Vertex3D::new(*v2, *n, Vec2::new(1.0, 1.0)));
        vertices.push(Vertex3D::new(*v3, *n, Vec2::new(0.0, 1.0)));
        indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    GlyphMesh {
        vertices, indices, character: ch, extrusion_depth: depth,
        triangle_count: 12, bounds_min: min, bounds_max: max,
    }
}

/// Generate beveled edges for softer appearance.
pub fn generate_bevel(outline_points: &[Vec2], depth: f32, bevel_size: f32) -> Vec<Vertex3D> {
    let mut verts = Vec::new();
    let n = outline_points.len();
    let steps = 3u32;

    for i in 0..n {
        let j = (i + 1) % n;
        let p0 = outline_points[i];
        let p1 = outline_points[j];
        let edge = p1 - p0;
        let normal_2d = Vec2::new(edge.y, -edge.x).normalize_or_zero();

        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let angle = t * std::f32::consts::FRAC_PI_2;
            let offset_xy = normal_2d * bevel_size * angle.cos();
            let offset_z = bevel_size * (1.0 - angle.sin());

            // Front bevel
            let pos = Vec3::new(p0.x + offset_xy.x, p0.y + offset_xy.y, offset_z);
            let norm = Vec3::new(
                normal_2d.x * angle.cos(),
                normal_2d.y * angle.cos(),
                angle.sin(),
            ).normalize_or_zero();
            verts.push(Vertex3D::new(pos, norm, Vec2::new(t, 0.0)));

            // Back bevel
            let pos_b = Vec3::new(p0.x + offset_xy.x, p0.y + offset_xy.y, -depth - offset_z);
            let norm_b = Vec3::new(
                normal_2d.x * angle.cos(),
                normal_2d.y * angle.cos(),
                -angle.sin(),
            ).normalize_or_zero();
            verts.push(Vertex3D::new(pos_b, norm_b, Vec2::new(t, 1.0)));
        }
    }

    verts
}

// ── Mesh Cache ──────────────────────────────────────────────────────────────

pub struct GlyphMeshCache {
    pub meshes: HashMap<char, GlyphMesh>,
}

impl GlyphMeshCache {
    pub fn build(outline_cache: &OutlineCache, depth: f32) -> Self {
        let mut meshes = HashMap::new();
        for (&ch, outline) in &outline_cache.outlines {
            let mesh = extrude_glyph(outline, depth, ch);
            meshes.insert(ch, mesh);
        }
        Self { meshes }
    }

    pub fn get(&self, ch: char) -> Option<&GlyphMesh> {
        self.meshes.get(&ch)
    }

    pub fn len(&self) -> usize { self.meshes.len() }
    pub fn is_empty(&self) -> bool { self.meshes.is_empty() }

    pub fn total_triangles(&self) -> u32 {
        self.meshes.values().map(|m| m.triangle_count).sum()
    }

    pub fn total_vertices(&self) -> usize {
        self.meshes.values().map(|m| m.vertices.len()).sum()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::font_to_mesh::GlyphBounds;

    fn square_outline() -> GlyphOutline {
        GlyphOutline {
            contours: vec![Contour {
                points: vec![
                    Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0),
                ],
                is_hole: false,
            }],
            advance_width: 1.0,
            bounds: GlyphBounds { min: Vec2::ZERO, max: Vec2::ONE },
        }
    }

    #[test]
    fn vertex3d_size() {
        assert_eq!(std::mem::size_of::<Vertex3D>(), 32);
    }

    #[test]
    fn point_in_triangle_basic() {
        assert!(point_in_triangle(
            Vec2::new(0.3, 0.3),
            Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0),
        ));
        assert!(!point_in_triangle(
            Vec2::new(2.0, 2.0),
            Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0),
        ));
    }

    #[test]
    fn ear_clip_square() {
        let sq = vec![
            Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0),
        ];
        let tris = ear_clip_triangulate(&sq, &[]);
        assert_eq!(tris.len(), 2, "Square should produce 2 triangles");
    }

    #[test]
    fn extrude_square_produces_geometry() {
        let outline = square_outline();
        let mesh = extrude_glyph(&outline, 0.5, '█');
        assert!(mesh.triangle_count >= 12, "Extruded square: 2 front + 2 back + 8 sides = 12, got {}", mesh.triangle_count);
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn extrude_normals_are_unit() {
        let outline = square_outline();
        let mesh = extrude_glyph(&outline, 0.5, '█');
        for v in &mesh.vertices {
            let n = v.norm();
            let len = n.length();
            assert!((len - 1.0).abs() < 0.01 || len < 0.01, "Normal should be unit or zero: {}", len);
        }
    }

    #[test]
    fn extrude_indices_valid() {
        let outline = square_outline();
        let mesh = extrude_glyph(&outline, 0.5, '█');
        let vc = mesh.vertices.len() as u32;
        for &idx in &mesh.indices {
            assert!(idx < vc, "Index {} out of range (vertex count = {})", idx, vc);
        }
    }

    #[test]
    fn bridge_hole_increases_count() {
        let outer = vec![
            Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0),
        ];
        let hole = vec![
            Vec2::new(3.0, 3.0), Vec2::new(7.0, 3.0),
            Vec2::new(7.0, 7.0), Vec2::new(3.0, 7.0),
        ];
        let merged = bridge_hole(&outer, &hole);
        assert!(merged.len() > outer.len());
        assert!(merged.len() > hole.len());
    }

    #[test]
    fn box_mesh_fallback() {
        let mesh = create_box_mesh(Vec3::ZERO, Vec3::ONE, 'X', 1.0);
        assert_eq!(mesh.triangle_count, 12);
        assert_eq!(mesh.vertices.len(), 24);
    }
}
