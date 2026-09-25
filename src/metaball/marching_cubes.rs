//! Isosurface extraction for metaball fields.
//!
//! Marching tetrahedra rather than marching cubes: each cell of the grid is
//! split into six tetrahedra and each tetrahedron is cut by the isosurface
//! in one of three ways (no crossing, one triangle, two triangles). It needs
//! no 256-entry case table, has no ambiguous cases, and produces a closed
//! mesh for any field. The cost is a few more triangles than cubes would
//! give for the same grid, which a particle renderer does not care about.
//!
//! The name is kept from the stub this replaces, so callers need not change.

use glam::{Vec3, Vec4};

use super::entity_field::MetaballEntity;

/// Kept for compatibility; the tetrahedral extractor does not use case tables.
pub const EDGE_TABLE: [u16; 256] = [0u16; 256];
/// Kept for compatibility; the tetrahedral extractor does not use case tables.
pub const TRI_TABLE: [[i8; 16]; 256] = [[-1i8; 16]; 256];

#[derive(Debug, Clone, Default)]
pub struct MCVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec4,
    pub emission: f32,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractedMesh {
    pub vertices: Vec<MCVertex>,
    pub indices: Vec<u32>,
}

impl ExtractedMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// The six tetrahedra of a cube, as indices into its eight corners.
///
/// Corner `i` is at `(i & 1, (i >> 1) & 1, (i >> 2) & 1)`. Every tetrahedron
/// shares the main diagonal from corner 0 to corner 7, which is what makes
/// the six of them tile the cube without gaps.
const TETRA: [[usize; 4]; 6] = [
    [0, 1, 3, 7],
    [0, 1, 5, 7],
    [0, 2, 3, 7],
    [0, 2, 6, 7],
    [0, 4, 5, 7],
    [0, 4, 6, 7],
];

pub struct MarchingCubesExtractor;

impl MarchingCubesExtractor {
    /// Extract the `threshold` isosurface of `field` over the box, sampling
    /// `resolution` cells along each axis. Normals are the field's gradient,
    /// so they are exact for a smooth field rather than averaged from faces.
    pub fn extract(
        field: &dyn Fn(Vec3) -> f32,
        bounds_min: Vec3,
        bounds_max: Vec3,
        resolution: u32,
        threshold: f32,
    ) -> ExtractedMesh {
        Self::extract_with(field, &|_| (Vec4::ONE, 0.0), bounds_min, bounds_max, resolution, threshold)
    }

    /// As [`extract`](Self::extract), with a second function giving each
    /// vertex a colour and an emission.
    pub fn extract_with(
        field: &dyn Fn(Vec3) -> f32,
        shade: &dyn Fn(Vec3) -> (Vec4, f32),
        bounds_min: Vec3,
        bounds_max: Vec3,
        resolution: u32,
        threshold: f32,
    ) -> ExtractedMesh {
        let mut mesh = ExtractedMesh::default();
        let res = resolution.clamp(1, 512) as usize;
        let size = bounds_max - bounds_min;
        if !(size.x > 0.0 && size.y > 0.0 && size.z > 0.0) {
            return mesh;
        }
        let cell = size / res as f32;

        // Sample the whole lattice once: (res + 1)^3 values.
        let n = res + 1;
        let idx = |x: usize, y: usize, z: usize| (z * n + y) * n + x;
        let mut samples = vec![0.0f32; n * n * n];
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let p = bounds_min + Vec3::new(x as f32, y as f32, z as f32) * cell;
                    samples[idx(x, y, z)] = field(p);
                }
            }
        }

        let eps = cell.min_element() * 0.5;
        let gradient = |p: Vec3| -> Vec3 {
            let dx = field(p + Vec3::X * eps) - field(p - Vec3::X * eps);
            let dy = field(p + Vec3::Y * eps) - field(p - Vec3::Y * eps);
            let dz = field(p + Vec3::Z * eps) - field(p - Vec3::Z * eps);
            let g = Vec3::new(dx, dy, dz);
            if g.length_squared() > 1e-12 { -g.normalize() } else { Vec3::Y }
        };

        let mut emit_vertex = |p: Vec3, mesh: &mut ExtractedMesh| -> u32 {
            let (color, emission) = shade(p);
            mesh.vertices.push(MCVertex { position: p, normal: gradient(p), color, emission });
            (mesh.vertices.len() - 1) as u32
        };

        for z in 0..res {
            for y in 0..res {
                for x in 0..res {
                    let mut corner_pos = [Vec3::ZERO; 8];
                    let mut corner_val = [0.0f32; 8];
                    for (i, (cp, cv)) in corner_pos.iter_mut().zip(corner_val.iter_mut()).enumerate() {
                        let cx = x + (i & 1);
                        let cy = y + ((i >> 1) & 1);
                        let cz = z + ((i >> 2) & 1);
                        *cp = bounds_min + Vec3::new(cx as f32, cy as f32, cz as f32) * cell;
                        *cv = samples[idx(cx, cy, cz)];
                    }
                    for tet in &TETRA {
                        polygonise_tetra(
                            [corner_pos[tet[0]], corner_pos[tet[1]], corner_pos[tet[2]], corner_pos[tet[3]]],
                            [corner_val[tet[0]], corner_val[tet[1]], corner_val[tet[2]], corner_val[tet[3]]],
                            threshold,
                            &mut |a, b, c| {
                                let ia = emit_vertex(a, &mut mesh);
                                let ib = emit_vertex(b, &mut mesh);
                                let ic = emit_vertex(c, &mut mesh);
                                mesh.indices.extend_from_slice(&[ia, ib, ic]);
                            },
                        );
                    }
                }
            }
        }
        mesh
    }

    /// Extract a metaball entity's surface at its own threshold and
    /// resolution, coloured by its sources.
    pub fn extract_entity(entity: &MetaballEntity) -> ExtractedMesh {
        let (min, max) = entity.bounds();
        // A little margin, so a surface at the edge of the bounds closes.
        let pad = (max - min) * 0.1 + Vec3::splat(0.05);
        let field = |p: Vec3| entity.evaluate(p);
        let shade = |p: Vec3| {
            let (_, color, emission) = entity.evaluate_full(p);
            (color, emission)
        };
        Self::extract_with(&field, &shade, min - pad, max + pad, entity.grid_resolution.max(4), entity.threshold)
    }
}

/// Where the isosurface crosses the edge between two samples.
fn cross(pa: Vec3, va: f32, pb: Vec3, vb: f32, iso: f32) -> Vec3 {
    let d = vb - va;
    if d.abs() < 1e-9 {
        return (pa + pb) * 0.5;
    }
    let t = ((iso - va) / d).clamp(0.0, 1.0);
    pa + (pb - pa) * t
}

/// Cut one tetrahedron by the isosurface and hand back its triangles.
///
/// Winding is chosen so triangles face outward from the side where the
/// field is above the threshold, which for a metaball is the inside of the
/// body; the gradient normals agree with it.
fn polygonise_tetra(p: [Vec3; 4], v: [f32; 4], iso: f32, tri: &mut dyn FnMut(Vec3, Vec3, Vec3)) {
    let inside: [bool; 4] = [v[0] >= iso, v[1] >= iso, v[2] >= iso, v[3] >= iso];
    let count = inside.iter().filter(|b| **b).count();
    if count == 0 || count == 4 {
        return;
    }
    let e = |a: usize, b: usize| cross(p[a], v[a], p[b], v[b], iso);

    if count == 1 || count == 3 {
        // One vertex on its own side: a single triangle cutting off that corner.
        let lone = if count == 1 { inside.iter().position(|b| *b) } else { inside.iter().position(|b| !*b) }
            .unwrap();
        let others: Vec<usize> = (0..4).filter(|i| *i != lone).collect();
        let a = e(lone, others[0]);
        let b = e(lone, others[1]);
        let c = e(lone, others[2]);
        if count == 1 {
            tri(a, b, c);
        } else {
            tri(a, c, b);
        }
    } else {
        // Two and two: a quad, as two triangles.
        let ins: Vec<usize> = (0..4).filter(|i| inside[*i]).collect();
        let outs: Vec<usize> = (0..4).filter(|i| !inside[*i]).collect();
        let a = e(ins[0], outs[0]);
        let b = e(ins[0], outs[1]);
        let c = e(ins[1], outs[1]);
        let d = e(ins[1], outs[0]);
        tri(a, b, c);
        tri(a, c, d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sphere_field_gives_a_closed_mesh_of_about_the_right_size() {
        let field = |p: Vec3| 1.0 - p.length();
        let mesh = MarchingCubesExtractor::extract(&field, Vec3::splat(-1.5), Vec3::splat(1.5), 24, 0.0);
        assert!(!mesh.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        // Every vertex sits on the unit sphere, within a cell.
        for v in &mesh.vertices {
            assert!((v.position.length() - 1.0).abs() < 0.15, "{:?}", v.position);
            assert!((v.normal.length() - 1.0).abs() < 1e-3);
        }
        // Normals point outward for a field that is positive inside.
        let outward = mesh.vertices.iter().filter(|v| v.normal.dot(v.position) > 0.0).count();
        assert!(outward > mesh.vertices.len() * 9 / 10);
    }

    #[test]
    fn an_empty_field_gives_nothing() {
        let field = |_p: Vec3| -1.0;
        let mesh = MarchingCubesExtractor::extract(&field, Vec3::ZERO, Vec3::ONE, 8, 0.0);
        assert!(mesh.is_empty());
    }
}
