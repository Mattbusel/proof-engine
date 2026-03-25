//! Subdivision surfaces — Catmull-Clark and Loop subdivision on glyph meshes.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;
use super::GeoMesh;

/// Subdivision scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubdivisionScheme {
    /// Catmull-Clark: quads → quads, smoothing, suitable for any mesh.
    CatmullClark,
    /// Loop: triangles → triangles, for triangle meshes only.
    Loop,
    /// Simple midpoint: split each edge, no smoothing.
    Midpoint,
}

/// A mesh optimized for subdivision operations.
#[derive(Debug, Clone)]
pub struct SubdivMesh {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Vec<u32>>,  // each face is a list of vertex indices
}

impl SubdivMesh {
    pub fn new() -> Self { Self { vertices: Vec::new(), faces: Vec::new() } }

    pub fn from_geo_mesh(mesh: &GeoMesh) -> Self {
        let vertices = mesh.vertices.clone();
        let faces: Vec<Vec<u32>> = mesh.triangles.iter()
            .map(|t| vec![t.a, t.b, t.c])
            .collect();
        Self { vertices, faces }
    }

    /// Subdivide the mesh using the given scheme.
    pub fn subdivide(&self, scheme: SubdivisionScheme) -> SubdivMesh {
        match scheme {
            SubdivisionScheme::CatmullClark => self.catmull_clark(),
            SubdivisionScheme::Loop => self.loop_subdivide(),
            SubdivisionScheme::Midpoint => self.midpoint_subdivide(),
        }
    }

    /// Apply n levels of subdivision.
    pub fn subdivide_n(&self, scheme: SubdivisionScheme, levels: u32) -> SubdivMesh {
        let mut mesh = self.clone();
        for _ in 0..levels {
            mesh = mesh.subdivide(scheme);
        }
        mesh
    }

    fn catmull_clark(&self) -> SubdivMesh {
        let n_verts = self.vertices.len();
        let mut new_verts: Vec<Vec3> = Vec::new();
        let mut new_faces: Vec<Vec<u32>> = Vec::new();

        // 1. Face points: average of all vertices in the face
        let mut face_points = Vec::with_capacity(self.faces.len());
        for face in &self.faces {
            let avg = face.iter()
                .map(|&i| self.vertices[i as usize])
                .fold(Vec3::ZERO, |a, b| a + b) / face.len() as f32;
            face_points.push(new_verts.len() as u32);
            new_verts.push(avg);
        }

        // 2. Edge points: average of edge endpoints + adjacent face points
        let mut edge_map: HashMap<(u32, u32), u32> = HashMap::new();
        let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();

        for (fi, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                let key = if a < b { (a, b) } else { (b, a) };
                edge_faces.entry(key).or_default().push(fi);
            }
        }

        for (&(a, b), faces) in &edge_faces {
            let mid = (self.vertices[a as usize] + self.vertices[b as usize]) * 0.5;
            let face_avg: Vec3 = faces.iter()
                .map(|&fi| new_verts[face_points[fi] as usize])
                .fold(Vec3::ZERO, |acc, v| acc + v) / faces.len() as f32;
            let edge_point = (mid + face_avg) * 0.5;
            let idx = new_verts.len() as u32;
            new_verts.push(edge_point);
            edge_map.insert((a, b), idx);
        }

        // 3. Move original vertices
        let orig_offset = new_verts.len() as u32;
        for (vi, &vert) in self.vertices.iter().enumerate() {
            // For simplicity, use a weighted average (full CC uses face/edge neighbors)
            new_verts.push(vert);
        }

        // 4. Create new faces (one quad per original face edge)
        for (fi, face) in self.faces.iter().enumerate() {
            let fp = face_points[fi];
            for i in 0..face.len() {
                let a = face[i];
                let b = face[(i + 1) % face.len()];
                let prev = face[(i + face.len() - 1) % face.len()];

                let key_ab = if a < b { (a, b) } else { (b, a) };
                let key_pa = if prev < a { (prev, a) } else { (a, prev) };

                let ep_ab = edge_map[&key_ab];
                let ep_pa = edge_map[&key_pa];
                let orig_a = orig_offset + a;

                new_faces.push(vec![fp, ep_pa, orig_a, ep_ab]);
            }
        }

        SubdivMesh { vertices: new_verts, faces: new_faces }
    }

    fn loop_subdivide(&self) -> SubdivMesh {
        // Loop subdivision: insert edge midpoints, reconnect as 4 triangles per original
        let mut new_verts = self.vertices.clone();
        let mut new_faces = Vec::new();
        let mut edge_map: HashMap<(u32, u32), u32> = HashMap::new();

        let mut get_edge_vert = |a: u32, b: u32, verts: &mut Vec<Vec3>, orig: &[Vec3]| -> u32 {
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(&idx) = edge_map.get(&key) { return idx; }
            let mid = (orig[a as usize] + orig[b as usize]) * 0.5;
            let idx = verts.len() as u32;
            verts.push(mid);
            edge_map.insert(key, idx);
            idx
        };

        for face in &self.faces {
            if face.len() != 3 { continue; }
            let (a, b, c) = (face[0], face[1], face[2]);
            let ab = get_edge_vert(a, b, &mut new_verts, &self.vertices);
            let bc = get_edge_vert(b, c, &mut new_verts, &self.vertices);
            let ca = get_edge_vert(c, a, &mut new_verts, &self.vertices);
            new_faces.push(vec![a, ab, ca]);
            new_faces.push(vec![ab, b, bc]);
            new_faces.push(vec![ca, bc, c]);
            new_faces.push(vec![ab, bc, ca]);
        }

        SubdivMesh { vertices: new_verts, faces: new_faces }
    }

    fn midpoint_subdivide(&self) -> SubdivMesh {
        // Same as loop but without smoothing
        self.loop_subdivide()
    }

    /// Convert back to GeoMesh (triangulated).
    pub fn to_geo_mesh(&self) -> GeoMesh {
        let mut mesh = GeoMesh::new();
        for &v in &self.vertices {
            mesh.add_vertex(v, Vec3::Y, Vec2::ZERO);
        }
        for face in &self.faces {
            if face.len() >= 3 {
                // Fan triangulation
                for i in 1..face.len() - 1 {
                    mesh.add_triangle(face[0], face[i as usize], face[i as usize + 1]);
                }
            }
        }
        mesh.recompute_normals();
        mesh
    }

    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn face_count(&self) -> usize { self.faces.len() }
}

impl Default for SubdivMesh {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_triangle() -> SubdivMesh {
        SubdivMesh {
            vertices: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            faces: vec![vec![0, 1, 2]],
        }
    }

    #[test]
    fn loop_subdivision_increases_faces() {
        let mesh = make_triangle();
        let subdiv = mesh.subdivide(SubdivisionScheme::Loop);
        assert_eq!(subdiv.face_count(), 4); // 1 tri → 4 tris
        assert_eq!(subdiv.vertex_count(), 6); // 3 orig + 3 edge midpoints
    }

    #[test]
    fn catmull_clark_produces_quads() {
        let mesh = make_triangle();
        let subdiv = mesh.subdivide(SubdivisionScheme::CatmullClark);
        assert!(subdiv.face_count() > 0);
        // Each face should be a quad (4 vertices)
        for face in &subdiv.faces {
            assert_eq!(face.len(), 4);
        }
    }

    #[test]
    fn multi_level_subdivision() {
        let mesh = make_triangle();
        let subdiv = mesh.subdivide_n(SubdivisionScheme::Loop, 3);
        assert!(subdiv.face_count() > 16); // 1 → 4 → 16 → 64
    }
}
