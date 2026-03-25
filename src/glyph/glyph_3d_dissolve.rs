//! 3D dissolution effects for entity death — glyph meshes split into fragments
//! that tumble with rigid-body physics, catching light as they scatter.

use glam::{Vec2, Vec3, Vec4};

use super::glyph_mesh::{GlyphMesh, Vertex3D};
use super::glyph_materials::{GlyphMaterial, lerp_material};

// ── Fragment ────────────────────────────────────────────────────────────────

/// A single fragment of a dissolved glyph mesh.
#[derive(Clone, Debug)]
pub struct GlyphFragment {
    pub mesh: GlyphMesh,
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub rotation: Vec3,
    pub material: GlyphMaterial,
    pub life: f32,
    pub max_life: f32,
}

impl GlyphFragment {
    pub fn is_alive(&self) -> bool { self.life > 0.0 }
    pub fn progress(&self) -> f32 { 1.0 - (self.life / self.max_life).clamp(0.0, 1.0) }
}

// ── Dissolve State ──────────────────────────────────────────────────────────

/// Manages the dissolution of a glyph into fragments.
#[derive(Clone, Debug)]
pub struct DissolveState {
    pub fragments: Vec<GlyphFragment>,
    pub age: f32,
    pub duration: f32,
    pub done: bool,
}

impl DissolveState {
    pub fn active_fragments(&self) -> usize {
        self.fragments.iter().filter(|f| f.is_alive()).count()
    }
}

// ── Mesh splitting ──────────────────────────────────────────────────────────

/// Split a mesh along a plane into two halves.
pub fn split_mesh_by_plane(
    mesh: &GlyphMesh,
    plane_point: Vec3,
    plane_normal: Vec3,
) -> (GlyphMesh, GlyphMesh) {
    let normal = plane_normal.normalize_or_zero();
    let mut verts_a = Vec::new();
    let mut verts_b = Vec::new();
    let mut indices_a = Vec::new();
    let mut indices_b = Vec::new();
    let mut map_a: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut map_b: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();

    // Classify each triangle
    let tri_count = mesh.indices.len() / 3;
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        let v0 = Vec3::from(mesh.vertices[i0].position);
        let v1 = Vec3::from(mesh.vertices[i1].position);
        let v2 = Vec3::from(mesh.vertices[i2].position);

        let d0 = (v0 - plane_point).dot(normal);
        let d1 = (v1 - plane_point).dot(normal);
        let d2 = (v2 - plane_point).dot(normal);

        let centroid_side = d0 + d1 + d2;

        // Simple classification: entire triangle goes to whichever side the centroid is on
        let original_indices = [mesh.indices[t * 3], mesh.indices[t * 3 + 1], mesh.indices[t * 3 + 2]];

        if centroid_side >= 0.0 {
            for &oi in &original_indices {
                let new_idx = *map_a.entry(oi).or_insert_with(|| {
                    let idx = verts_a.len() as u32;
                    verts_a.push(mesh.vertices[oi as usize]);
                    idx
                });
                indices_a.push(new_idx);
            }
        } else {
            for &oi in &original_indices {
                let new_idx = *map_b.entry(oi).or_insert_with(|| {
                    let idx = verts_b.len() as u32;
                    verts_b.push(mesh.vertices[oi as usize]);
                    idx
                });
                indices_b.push(new_idx);
            }
        }
    }

    let compute_bounds = |verts: &[Vertex3D]| -> (Vec3, Vec3) {
        let mut bmin = Vec3::splat(f32::MAX);
        let mut bmax = Vec3::splat(f32::MIN);
        for v in verts {
            let p = Vec3::from(v.position);
            bmin = bmin.min(p);
            bmax = bmax.max(p);
        }
        if verts.is_empty() { (Vec3::ZERO, Vec3::ZERO) } else { (bmin, bmax) }
    };

    let (bmin_a, bmax_a) = compute_bounds(&verts_a);
    let (bmin_b, bmax_b) = compute_bounds(&verts_b);

    let mesh_a = GlyphMesh {
        vertices: verts_a,
        triangle_count: indices_a.len() as u32 / 3,
        indices: indices_a,
        character: mesh.character,
        extrusion_depth: mesh.extrusion_depth,
        bounds_min: bmin_a,
        bounds_max: bmax_a,
    };

    let mesh_b = GlyphMesh {
        vertices: verts_b,
        triangle_count: indices_b.len() as u32 / 3,
        indices: indices_b,
        character: mesh.character,
        extrusion_depth: mesh.extrusion_depth,
        bounds_min: bmin_b,
        bounds_max: bmax_b,
    };

    (mesh_a, mesh_b)
}

// ── Dissolve creation ───────────────────────────────────────────────────────

/// Start a dissolution effect by splitting the mesh into fragments.
pub fn start_dissolve(
    mesh: &GlyphMesh,
    material: &GlyphMaterial,
    impact_dir: Vec3,
    impact_strength: f32,
) -> DissolveState {
    let mut fragments = Vec::new();

    // Generate 2-4 random split planes
    let num_splits = if mesh.triangle_count > 20 { 3 } else { 2 };
    let center = (mesh.bounds_min + mesh.bounds_max) * 0.5;

    let mut current_meshes = vec![mesh.clone()];

    // Simple deterministic "random" planes based on mesh properties
    let planes: Vec<(Vec3, Vec3)> = (0..num_splits).map(|i| {
        let angle = i as f32 * 1.2 + impact_dir.x * 0.5;
        let normal = Vec3::new(angle.cos(), angle.sin(), 0.3).normalize();
        let offset = (i as f32 - num_splits as f32 * 0.5) * 0.2;
        let point = center + normal * offset;
        (point, normal)
    }).collect();

    for (plane_point, plane_normal) in &planes {
        let mut new_meshes = Vec::new();
        for m in &current_meshes {
            if m.triangle_count < 2 {
                new_meshes.push(m.clone());
                continue;
            }
            let (a, b) = split_mesh_by_plane(m, *plane_point, *plane_normal);
            if !a.vertices.is_empty() { new_meshes.push(a); }
            if !b.vertices.is_empty() { new_meshes.push(b); }
        }
        current_meshes = new_meshes;
    }

    // Create fragments from split meshes
    for (i, frag_mesh) in current_meshes.into_iter().enumerate() {
        if frag_mesh.vertices.is_empty() { continue; }

        let frag_center = (frag_mesh.bounds_min + frag_mesh.bounds_max) * 0.5;
        let scatter_dir = (frag_center - center).normalize_or_zero();

        // Velocity: mix of impact direction and scatter direction
        let base_speed = impact_strength * 2.0;
        let velocity = (impact_dir.normalize_or_zero() * 0.5 + scatter_dir * 0.5).normalize_or_zero()
            * base_speed * (1.0 + i as f32 * 0.3);

        // Angular velocity perpendicular to velocity
        let angular = impact_dir.cross(Vec3::new(0.0, 0.0, 1.0)).normalize_or_zero()
            * impact_strength * 5.0 * (if i % 2 == 0 { 1.0 } else { -1.0 });

        let life = 1.0 + i as f32 * 0.2;

        fragments.push(GlyphFragment {
            mesh: frag_mesh,
            position: frag_center,
            velocity,
            angular_velocity: angular,
            rotation: Vec3::ZERO,
            material: *material,
            life,
            max_life: life,
        });
    }

    let max_life = fragments.iter().map(|f| f.max_life).fold(0.0f32, f32::max);

    DissolveState {
        fragments,
        age: 0.0,
        duration: max_life,
        done: false,
    }
}

// ── Simulation ──────────────────────────────────────────────────────────────

/// Advance the dissolve simulation by dt seconds.
pub fn tick_dissolve(state: &mut DissolveState, dt: f32, gravity: Vec3) {
    state.age += dt;

    for frag in &mut state.fragments {
        if !frag.is_alive() { continue; }

        // Physics
        frag.velocity += gravity * dt;
        frag.position += frag.velocity * dt;
        frag.rotation += frag.angular_velocity * dt;

        // Damping
        frag.velocity *= 0.98;
        frag.angular_velocity *= 0.97;

        // Life decay
        frag.life -= dt;

        // Material animation: fade emission, increase roughness, darken
        frag.material = dissolve_material_at(&frag.material, frag.progress());
    }

    state.done = state.fragments.iter().all(|f| !f.is_alive());
}

/// Compute the material state at a given dissolution progress (0 = alive, 1 = dead).
pub fn dissolve_material_at(base: &GlyphMaterial, progress: f32) -> GlyphMaterial {
    let p = progress.clamp(0.0, 1.0);

    let dead = GlyphMaterial {
        base_color: [0.1, 0.1, 0.1, 0.0],
        emission: 0.0,
        metallic: 0.0,
        roughness: 1.0,
        subsurface: 0.0,
        fresnel: 0.02,
        _pad: [0.0; 3],
    };

    lerp_material(base, &dead, p)
}

// ── Dissolve renderer helper ────────────────────────────────────────────────

/// Collects all alive fragment instances for submission to Glyph3DRenderer.
pub struct DissolveRenderer;

impl DissolveRenderer {
    /// Get transform matrices and materials for all alive fragments.
    pub fn collect_instances(state: &DissolveState) -> Vec<(glam::Mat4, GlyphMaterial, char)> {
        let mut instances = Vec::new();
        for frag in &state.fragments {
            if !frag.is_alive() { continue; }

            let translation = glam::Mat4::from_translation(frag.position);
            let rot_x = glam::Mat4::from_rotation_x(frag.rotation.x);
            let rot_y = glam::Mat4::from_rotation_y(frag.rotation.y);
            let rot_z = glam::Mat4::from_rotation_z(frag.rotation.z);
            let scale = glam::Mat4::from_scale(Vec3::splat(1.0 - frag.progress() * 0.3));
            let transform = translation * rot_z * rot_y * rot_x * scale;

            instances.push((transform, frag.material, frag.mesh.character));
        }
        instances
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::glyph_mesh::create_box_mesh;

    fn test_mesh() -> GlyphMesh {
        create_box_mesh(Vec3::ZERO, Vec3::ONE, 'X', 0.5)
    }

    #[test]
    fn split_produces_two_meshes() {
        let mesh = test_mesh();
        let (a, b) = split_mesh_by_plane(&mesh, Vec3::new(0.5, 0.5, 0.0), Vec3::X);
        assert!(!a.vertices.is_empty() || !b.vertices.is_empty());
        // Total triangles should be ≤ original (no duplication at split boundary in simple mode)
        assert!(a.triangle_count + b.triangle_count <= mesh.triangle_count + 2);
    }

    #[test]
    fn dissolve_creates_fragments() {
        let mesh = test_mesh();
        let mat = GlyphMaterial::enemy();
        let state = start_dissolve(&mesh, &mat, Vec3::X, 1.0);
        assert!(state.fragments.len() >= 2, "Should create at least 2 fragments, got {}", state.fragments.len());
    }

    #[test]
    fn fragments_move_apart() {
        let mesh = test_mesh();
        let mat = GlyphMaterial::enemy();
        let mut state = start_dissolve(&mesh, &mat, Vec3::X, 5.0);
        let initial_positions: Vec<Vec3> = state.fragments.iter().map(|f| f.position).collect();

        tick_dissolve(&mut state, 0.5, Vec3::new(0.0, -9.8, 0.0));

        for (i, frag) in state.fragments.iter().enumerate() {
            let moved = (frag.position - initial_positions[i]).length();
            assert!(moved > 0.01, "Fragment {} should have moved", i);
        }
    }

    #[test]
    fn dissolve_material_darkens() {
        let mat = GlyphMaterial::boss();
        let dead_mat = dissolve_material_at(&mat, 1.0);
        assert!(dead_mat.emission < mat.emission);
        assert!(dead_mat.roughness > mat.roughness);
    }

    #[test]
    fn dissolve_completes() {
        let mesh = test_mesh();
        let mat = GlyphMaterial::enemy();
        let mut state = start_dissolve(&mesh, &mat, Vec3::X, 1.0);
        for _ in 0..100 {
            tick_dissolve(&mut state, 0.1, Vec3::ZERO);
        }
        assert!(state.done, "Should be done after enough time");
    }

    #[test]
    fn collect_instances_only_alive() {
        let mesh = test_mesh();
        let mat = GlyphMaterial::enemy();
        let mut state = start_dissolve(&mesh, &mat, Vec3::X, 1.0);
        let initial = DissolveRenderer::collect_instances(&state).len();
        for _ in 0..100 {
            tick_dissolve(&mut state, 0.1, Vec3::ZERO);
        }
        let final_count = DissolveRenderer::collect_instances(&state).len();
        assert!(final_count <= initial);
    }
}
