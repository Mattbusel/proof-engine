// topology/tiling.rs — Fundamental domains, wallpaper groups, and tilings

use glam::Vec2;
use std::f32::consts::PI;

// ─── Wallpaper Groups ──────────────────────────────────────────────────────

/// All 17 wallpaper groups (2D crystallographic groups).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WallpaperGroup {
    P1,
    P2,
    PM,
    PG,
    CM,
    P2MM,
    P2MG,
    P2GG,
    C2MM,
    P4,
    P4MM,
    P4GM,
    P3,
    P3M1,
    P31M,
    P6,
    P6MM,
}

/// A tile instance placed in the plane.
#[derive(Clone, Debug)]
pub struct TileInstance {
    pub position: Vec2,
    pub rotation: f32,
    pub mirror: bool,
}

/// A fundamental domain defined by its vertices.
#[derive(Clone, Debug)]
pub struct FundamentalDomain {
    pub vertices: Vec<Vec2>,
}

impl FundamentalDomain {
    /// Create a rectangular fundamental domain.
    pub fn rectangle(width: f32, height: f32) -> Self {
        Self {
            vertices: vec![
                Vec2::ZERO,
                Vec2::new(width, 0.0),
                Vec2::new(width, height),
                Vec2::new(0.0, height),
            ],
        }
    }

    /// Create a rhombus fundamental domain.
    pub fn rhombus(a: f32, angle: f32) -> Self {
        let dx = a * angle.cos();
        let dy = a * angle.sin();
        Self {
            vertices: vec![
                Vec2::ZERO,
                Vec2::new(a, 0.0),
                Vec2::new(a + dx, dy),
                Vec2::new(dx, dy),
            ],
        }
    }

    /// Create an equilateral triangle fundamental domain.
    pub fn equilateral_triangle(side: f32) -> Self {
        Self {
            vertices: vec![
                Vec2::ZERO,
                Vec2::new(side, 0.0),
                Vec2::new(side / 2.0, side * (3.0_f32).sqrt() / 2.0),
            ],
        }
    }
}

/// Generate a tiling by applying the symmetries of the given wallpaper group
/// to the fundamental domain, filling the area [-extent, extent] x [-extent, extent].
pub fn generate_tiling(group: WallpaperGroup, domain: &FundamentalDomain, extent: f32) -> Vec<TileInstance> {
    let (t1, t2, symmetries) = group_generators(group, domain);

    let mut tiles = Vec::new();
    let max_n = (extent / t1.length().max(0.01)) as i32 + 2;
    let max_m = (extent / t2.length().max(0.01)) as i32 + 2;

    for n in -max_n..=max_n {
        for m in -max_m..=max_m {
            let base = t1 * n as f32 + t2 * m as f32;

            for sym in &symmetries {
                let pos = base + sym.offset;
                if pos.x.abs() <= extent && pos.y.abs() <= extent {
                    tiles.push(TileInstance {
                        position: pos,
                        rotation: sym.rotation,
                        mirror: sym.mirror,
                    });
                }
            }
        }
    }
    tiles
}

struct SymOp {
    offset: Vec2,
    rotation: f32,
    mirror: bool,
}

/// Return translation vectors and symmetry operations for a wallpaper group.
fn group_generators(group: WallpaperGroup, domain: &FundamentalDomain) -> (Vec2, Vec2, Vec<SymOp>) {
    // Compute a bounding size from the domain
    let mut max_x = 0.0_f32;
    let mut max_y = 0.0_f32;
    for v in &domain.vertices {
        max_x = max_x.max(v.x);
        max_y = max_y.max(v.y);
    }
    let w = max_x.max(1.0);
    let h = max_y.max(1.0);

    match group {
        WallpaperGroup::P1 => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false }],
        ),
        WallpaperGroup::P2 => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, h / 2.0), rotation: PI, mirror: false },
            ],
        ),
        WallpaperGroup::PM => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::new(0.0, h), rotation: 0.0, mirror: true },
            ],
        ),
        WallpaperGroup::PG => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, 0.0), rotation: 0.0, mirror: true },
            ],
        ),
        WallpaperGroup::CM => (
            Vec2::new(w, 0.0),
            Vec2::new(w / 2.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
            ],
        ),
        WallpaperGroup::P2MM => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: true },
            ],
        ),
        WallpaperGroup::P2MG => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, 0.0), rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::new(w / 2.0, 0.0), rotation: PI, mirror: true },
            ],
        ),
        WallpaperGroup::P2GG => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, h / 2.0), rotation: PI, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, 0.0), rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::new(0.0, h / 2.0), rotation: PI, mirror: true },
            ],
        ),
        WallpaperGroup::C2MM => (
            Vec2::new(w, 0.0),
            Vec2::new(w / 2.0, h),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: true },
            ],
        ),
        WallpaperGroup::P4 => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, w), // square lattice
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI / 2.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 3.0 * PI / 2.0, mirror: false },
            ],
        ),
        WallpaperGroup::P4MM => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, w),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI / 2.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 3.0 * PI / 2.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI / 2.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 3.0 * PI / 2.0, mirror: true },
            ],
        ),
        WallpaperGroup::P4GM => (
            Vec2::new(w, 0.0),
            Vec2::new(0.0, w),
            vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, w / 2.0), rotation: PI / 2.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::new(w / 2.0, w / 2.0), rotation: 3.0 * PI / 2.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::new(w / 2.0, w / 2.0), rotation: PI / 2.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: true },
                SymOp { offset: Vec2::new(w / 2.0, w / 2.0), rotation: 3.0 * PI / 2.0, mirror: true },
            ],
        ),
        WallpaperGroup::P3 => {
            let t1 = Vec2::new(w, 0.0);
            let t2 = Vec2::new(w / 2.0, w * (3.0_f32).sqrt() / 2.0);
            (t1, t2, vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: false },
            ])
        }
        WallpaperGroup::P3M1 => {
            let t1 = Vec2::new(w, 0.0);
            let t2 = Vec2::new(w / 2.0, w * (3.0_f32).sqrt() / 2.0);
            (t1, t2, vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: true },
            ])
        }
        WallpaperGroup::P31M => {
            let t1 = Vec2::new(w, 0.0);
            let t2 = Vec2::new(w / 2.0, w * (3.0_f32).sqrt() / 2.0);
            (t1, t2, vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI / 3.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 5.0 * PI / 3.0, mirror: true },
            ])
        }
        WallpaperGroup::P6 => {
            let t1 = Vec2::new(w, 0.0);
            let t2 = Vec2::new(w / 2.0, w * (3.0_f32).sqrt() / 2.0);
            (t1, t2, vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 5.0 * PI / 3.0, mirror: false },
            ])
        }
        WallpaperGroup::P6MM => {
            let t1 = Vec2::new(w, 0.0);
            let t2 = Vec2::new(w / 2.0, w * (3.0_f32).sqrt() / 2.0);
            (t1, t2, vec![
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 5.0 * PI / 3.0, mirror: false },
                SymOp { offset: Vec2::ZERO, rotation: 0.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI / 3.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 2.0 * PI / 3.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: PI, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 4.0 * PI / 3.0, mirror: true },
                SymOp { offset: Vec2::ZERO, rotation: 5.0 * PI / 3.0, mirror: true },
            ])
        }
    }
}

// ─── Penrose Tiling ────────────────────────────────────────────────────────

/// Penrose tile types: the two rhombus shapes in P3 Penrose tiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PenroseType {
    /// Thin rhombus (36/144 degree angles)
    Thin,
    /// Thick rhombus (72/108 degree angles)
    Thick,
}

/// A single Penrose tile.
#[derive(Clone, Debug)]
pub struct PenroseTile {
    pub vertices: Vec<Vec2>,
    pub tile_type: PenroseType,
}

/// Generate a Penrose tiling using the Robinson triangle decomposition.
/// `depth` controls the number of subdivision iterations.
pub fn generate_penrose(depth: usize) -> Vec<PenroseTile> {
    let golden = (1.0 + 5.0_f32.sqrt()) / 2.0;

    // Start with a wheel of 10 "half-kite" triangles
    let mut triangles: Vec<(bool, Vec2, Vec2, Vec2)> = Vec::new(); // (is_type_a, A, B, C)

    for i in 0..10 {
        let angle1 = 2.0 * PI * i as f32 / 10.0;
        let angle2 = 2.0 * PI * (i + 1) as f32 / 10.0;

        let b = Vec2::new(angle1.cos(), angle1.sin());
        let c = Vec2::new(angle2.cos(), angle2.sin());

        if i % 2 == 0 {
            triangles.push((true, Vec2::ZERO, b, c));
        } else {
            triangles.push((true, Vec2::ZERO, c, b));
        }
    }

    // Subdivide
    for _ in 0..depth {
        let mut new_triangles = Vec::new();
        for &(is_a, a, b, c) in &triangles {
            if is_a {
                // Subdivide acute triangle
                let p = a + (b - a) / golden;
                new_triangles.push((true, c, p, b));
                new_triangles.push((false, p, c, a));
            } else {
                // Subdivide obtuse triangle
                let q = b + (a - b) / golden;
                let r = b + (c - b) / golden;
                new_triangles.push((false, r, c, a));
                new_triangles.push((false, q, r, b));
                new_triangles.push((true, r, q, a));
            }
        }
        triangles = new_triangles;
    }

    // Convert pairs of triangles into rhombus tiles
    // For simplicity, output each triangle pair as a tile
    let mut tiles = Vec::new();
    let mut used = vec![false; triangles.len()];

    for i in 0..triangles.len() {
        if used[i] {
            continue;
        }

        let (is_a_i, a_i, b_i, c_i) = triangles[i];

        // Try to find a matching triangle to form a rhombus
        let mut found = false;
        for j in (i + 1)..triangles.len() {
            if used[j] {
                continue;
            }
            let (is_a_j, a_j, b_j, c_j) = triangles[j];
            if is_a_i != is_a_j {
                continue;
            }

            // Check if they share an edge (B-C)
            let share = (b_i - b_j).length() < 0.01 && (c_i - c_j).length() < 0.01
                || (b_i - c_j).length() < 0.01 && (c_i - b_j).length() < 0.01;

            if share {
                let tile_type = if is_a_i { PenroseType::Thin } else { PenroseType::Thick };
                tiles.push(PenroseTile {
                    vertices: vec![a_i, b_i, a_j, c_i],
                    tile_type,
                });
                used[i] = true;
                used[j] = true;
                found = true;
                break;
            }
        }

        if !found {
            // Unpaired triangle — output as a degenerate tile
            let tile_type = if is_a_i { PenroseType::Thin } else { PenroseType::Thick };
            tiles.push(PenroseTile {
                vertices: vec![a_i, b_i, c_i],
                tile_type,
            });
            used[i] = true;
        }
    }

    tiles
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p1_tiling() {
        let domain = FundamentalDomain::rectangle(1.0, 1.0);
        let tiles = generate_tiling(WallpaperGroup::P1, &domain, 5.0);
        assert!(!tiles.is_empty());
        // P1 has one copy per unit cell
        for tile in &tiles {
            assert!(!tile.mirror);
            assert!((tile.rotation).abs() < 1e-6);
        }
    }

    #[test]
    fn test_p4_tiling() {
        let domain = FundamentalDomain::rectangle(1.0, 1.0);
        let tiles = generate_tiling(WallpaperGroup::P4, &domain, 3.0);
        assert!(!tiles.is_empty());
        // P4 has 4 rotations per unit cell
    }

    #[test]
    fn test_p6mm_tiling() {
        let domain = FundamentalDomain::equilateral_triangle(1.0);
        let tiles = generate_tiling(WallpaperGroup::P6MM, &domain, 3.0);
        assert!(!tiles.is_empty());
        // P6MM has 12 symmetry operations
    }

    #[test]
    fn test_all_17_groups_produce_tiles() {
        let domain = FundamentalDomain::rectangle(1.0, 1.0);
        let groups = [
            WallpaperGroup::P1, WallpaperGroup::P2, WallpaperGroup::PM,
            WallpaperGroup::PG, WallpaperGroup::CM, WallpaperGroup::P2MM,
            WallpaperGroup::P2MG, WallpaperGroup::P2GG, WallpaperGroup::C2MM,
            WallpaperGroup::P4, WallpaperGroup::P4MM, WallpaperGroup::P4GM,
            WallpaperGroup::P3, WallpaperGroup::P3M1, WallpaperGroup::P31M,
            WallpaperGroup::P6, WallpaperGroup::P6MM,
        ];
        for group in groups {
            let tiles = generate_tiling(group, &domain, 2.0);
            assert!(!tiles.is_empty(), "Group {:?} produced no tiles", group);
        }
    }

    #[test]
    fn test_fundamental_domain_rectangle() {
        let d = FundamentalDomain::rectangle(2.0, 3.0);
        assert_eq!(d.vertices.len(), 4);
    }

    #[test]
    fn test_fundamental_domain_rhombus() {
        let d = FundamentalDomain::rhombus(1.0, PI / 3.0);
        assert_eq!(d.vertices.len(), 4);
    }

    #[test]
    fn test_fundamental_domain_triangle() {
        let d = FundamentalDomain::equilateral_triangle(1.0);
        assert_eq!(d.vertices.len(), 3);
    }

    #[test]
    fn test_penrose_tiling_depth_0() {
        let tiles = generate_penrose(0);
        assert!(!tiles.is_empty());
    }

    #[test]
    fn test_penrose_tiling_depth_3() {
        let tiles = generate_penrose(3);
        assert!(tiles.len() > 10, "Depth-3 Penrose should have many tiles, got {}", tiles.len());
    }

    #[test]
    fn test_penrose_has_both_types() {
        let tiles = generate_penrose(3);
        let has_thin = tiles.iter().any(|t| t.tile_type == PenroseType::Thin);
        let has_thick = tiles.iter().any(|t| t.tile_type == PenroseType::Thick);
        assert!(has_thin, "Should have thin tiles");
        assert!(has_thick, "Should have thick tiles");
    }

    #[test]
    fn test_penrose_tile_vertices() {
        let tiles = generate_penrose(2);
        for tile in &tiles {
            assert!(tile.vertices.len() >= 3);
            for v in &tile.vertices {
                // All vertices should be within a reasonable range of the origin
                assert!(v.length() < 5.0, "Vertex too far: {:?}", v);
            }
        }
    }
}
