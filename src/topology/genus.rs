// topology/genus.rs — Genus computation and surface classification

use glam::Vec3;

// ─── Surface Classification ────────────────────────────────────────────────

/// Classification of closed surfaces by genus and orientability.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceType {
    Sphere,
    Torus,
    DoubleTorus,
    TripleTorus,
    HigherGenusTorus(i32),
    KleinBottle,
    ProjectivePlane,
    NonOrientable(i32), // genus for non-orientable
}

/// Compute the Euler characteristic: V - E + F.
pub fn euler_characteristic(vertices: usize, edges: usize, faces: usize) -> i32 {
    vertices as i32 - edges as i32 + faces as i32
}

/// Compute the genus of a closed orientable surface from a mesh.
/// genus = (2 - chi) / 2 for orientable surfaces.
pub fn genus_from_mesh(vertices: usize, edges: usize, faces: usize) -> i32 {
    let chi = euler_characteristic(vertices, edges, faces);
    (2 - chi) / 2
}

/// Classify a closed surface from its Euler characteristic and orientability.
pub fn classify_surface(chi: i32, orientable: bool) -> SurfaceType {
    if orientable {
        let genus = (2 - chi) / 2;
        match genus {
            0 => SurfaceType::Sphere,
            1 => SurfaceType::Torus,
            2 => SurfaceType::DoubleTorus,
            3 => SurfaceType::TripleTorus,
            g => SurfaceType::HigherGenusTorus(g),
        }
    } else {
        // Non-orientable: chi = 2 - k, where k is the non-orientable genus
        let k = 2 - chi;
        match k {
            1 => SurfaceType::ProjectivePlane,
            2 => SurfaceType::KleinBottle,
            n => SurfaceType::NonOrientable(n),
        }
    }
}

/// Compute the connected sum of two surfaces.
/// For orientable: genus adds.
/// For non-orientable: non-orientable genus adds.
/// Orientable + non-orientable = non-orientable.
pub fn connected_sum(a: &SurfaceType, b: &SurfaceType) -> SurfaceType {
    let (g_a, orient_a) = surface_params(a);
    let (g_b, orient_b) = surface_params(b);

    if orient_a && orient_b {
        // Both orientable: genus adds
        let total_genus = g_a + g_b;
        classify_surface(2 - 2 * total_genus, true)
    } else {
        // At least one non-orientable
        // Convert orientable genus to non-orientable: orientable genus g = non-orientable genus 2g
        let k_a = if orient_a { 2 * g_a } else { g_a };
        let k_b = if orient_b { 2 * g_b } else { g_b };
        let total_k = k_a + k_b;
        classify_surface(2 - total_k, false)
    }
}

/// Extract (genus_or_k, orientable) from a SurfaceType.
fn surface_params(s: &SurfaceType) -> (i32, bool) {
    match s {
        SurfaceType::Sphere => (0, true),
        SurfaceType::Torus => (1, true),
        SurfaceType::DoubleTorus => (2, true),
        SurfaceType::TripleTorus => (3, true),
        SurfaceType::HigherGenusTorus(g) => (*g, true),
        SurfaceType::ProjectivePlane => (1, false),
        SurfaceType::KleinBottle => (2, false),
        SurfaceType::NonOrientable(k) => (*k, false),
    }
}

// ─── Genus Renderer ────────────────────────────────────────────────────────

/// Generates mesh vertices for visualizing surfaces of various genus.
pub struct GenusRenderer;

impl GenusRenderer {
    /// Generate a mesh approximation of a surface with the given genus.
    /// Returns vertices of a deformed torus-like shape with `genus` holes.
    pub fn generate_surface(genus: i32, resolution: usize) -> Vec<Vec3> {
        if genus <= 0 {
            return Self::generate_sphere(resolution);
        }

        let mut vertices = Vec::new();

        // For genus g, create g tori connected in a row
        let spacing = 3.0;
        let major_r = 1.0;
        let minor_r = 0.4;

        for hole in 0..genus {
            let offset_x = hole as f32 * spacing - (genus as f32 - 1.0) * spacing / 2.0;

            for i in 0..resolution {
                let u = std::f32::consts::PI * 2.0 * i as f32 / resolution as f32;
                for j in 0..resolution {
                    let v = std::f32::consts::PI * 2.0 * j as f32 / resolution as f32;
                    let x = (major_r + minor_r * v.cos()) * u.cos() + offset_x;
                    let y = (major_r + minor_r * v.cos()) * u.sin();
                    let z = minor_r * v.sin();
                    vertices.push(Vec3::new(x, y, z));
                }
            }

            // Connect to next torus with a tube
            if hole < genus - 1 {
                let tube_start = offset_x + major_r + minor_r;
                let tube_end = offset_x + spacing - major_r - minor_r;
                for i in 0..resolution {
                    let angle = std::f32::consts::PI * 2.0 * i as f32 / resolution as f32;
                    for j in 0..resolution / 2 {
                        let t = j as f32 / (resolution / 2) as f32;
                        let x = tube_start + t * (tube_end - tube_start);
                        let y = minor_r * angle.cos();
                        let z = minor_r * angle.sin();
                        vertices.push(Vec3::new(x, y, z));
                    }
                }
            }
        }

        vertices
    }

    /// Generate a simple sphere mesh (genus 0).
    fn generate_sphere(resolution: usize) -> Vec<Vec3> {
        let mut vertices = Vec::new();
        for i in 0..=resolution {
            let theta = std::f32::consts::PI * i as f32 / resolution as f32;
            for j in 0..resolution {
                let phi = 2.0 * std::f32::consts::PI * j as f32 / resolution as f32;
                vertices.push(Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.sin() * phi.sin(),
                    theta.cos(),
                ));
            }
        }
        vertices
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euler_characteristic_tetrahedron() {
        // Tetrahedron: V=4, E=6, F=4 => chi=2 (sphere)
        assert_eq!(euler_characteristic(4, 6, 4), 2);
    }

    #[test]
    fn test_euler_characteristic_cube() {
        // Cube: V=8, E=12, F=6 => chi=2 (sphere)
        assert_eq!(euler_characteristic(8, 12, 6), 2);
    }

    #[test]
    fn test_genus_sphere() {
        // Any triangulation of sphere has chi=2, genus=0
        assert_eq!(genus_from_mesh(4, 6, 4), 0);
    }

    #[test]
    fn test_genus_torus() {
        // Torus triangulation: V=9, E=27, F=18 => chi=0, genus=1
        assert_eq!(euler_characteristic(9, 27, 18), 0);
        assert_eq!(genus_from_mesh(9, 27, 18), 1);
    }

    #[test]
    fn test_genus_double_torus() {
        // chi = -2 => genus = 2
        // V - E + F = -2 => e.g. V=10, E=30, F=18
        let chi = euler_characteristic(10, 30, 18);
        assert_eq!(chi, -2);
        assert_eq!(genus_from_mesh(10, 30, 18), 2);
    }

    #[test]
    fn test_classify_sphere() {
        assert_eq!(classify_surface(2, true), SurfaceType::Sphere);
    }

    #[test]
    fn test_classify_torus() {
        assert_eq!(classify_surface(0, true), SurfaceType::Torus);
    }

    #[test]
    fn test_classify_klein() {
        assert_eq!(classify_surface(0, false), SurfaceType::KleinBottle);
    }

    #[test]
    fn test_classify_projective_plane() {
        assert_eq!(classify_surface(1, false), SurfaceType::ProjectivePlane);
    }

    #[test]
    fn test_connected_sum_torus_torus() {
        let result = connected_sum(&SurfaceType::Torus, &SurfaceType::Torus);
        assert_eq!(result, SurfaceType::DoubleTorus);
    }

    #[test]
    fn test_connected_sum_sphere_anything() {
        // Sphere is the identity for connected sum
        let result = connected_sum(&SurfaceType::Sphere, &SurfaceType::Torus);
        assert_eq!(result, SurfaceType::Torus);
    }

    #[test]
    fn test_connected_sum_klein_projective() {
        // RP2 # KB = non-orientable genus 3
        let result = connected_sum(&SurfaceType::ProjectivePlane, &SurfaceType::KleinBottle);
        assert_eq!(result, SurfaceType::NonOrientable(3));
    }

    #[test]
    fn test_genus_renderer_sphere() {
        let verts = GenusRenderer::generate_surface(0, 10);
        assert!(!verts.is_empty());
    }

    #[test]
    fn test_genus_renderer_torus() {
        let verts = GenusRenderer::generate_surface(1, 10);
        assert!(!verts.is_empty());
    }

    #[test]
    fn test_genus_renderer_double_torus() {
        let verts = GenusRenderer::generate_surface(2, 10);
        assert!(verts.len() > GenusRenderer::generate_surface(1, 10).len());
    }
}
