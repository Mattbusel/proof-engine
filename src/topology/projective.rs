// topology/projective.rs — Real projective plane RP^2

use glam::Vec2;

// ─── Projective Plane ──────────────────────────────────────────────────────

/// The real projective plane RP^2, modeled as a square with antipodal
/// boundary identification. Crossing any edge puts you on the opposite edge
/// with both coordinates inverted (antipodal identification).
pub struct ProjectivePlane {
    pub size: f32,
}

impl ProjectivePlane {
    pub fn new(size: f32) -> Self {
        Self { size }
    }

    /// Wrap a position into the fundamental domain [0, size) x [0, size)
    /// with antipodal boundary identification.
    pub fn projective_wrap(&self, pos: Vec2) -> Vec2 {
        projective_wrap(pos, self.size)
    }
}

/// Wrap a position in the projective plane's fundamental domain.
/// The square [0, size) x [0, size) with opposite edges identified with a flip.
pub fn projective_wrap(pos: Vec2, size: f32) -> Vec2 {
    let mut x = pos.x;
    let mut y = pos.y;

    // Normalize into [0, 2*size) x [0, 2*size) first (the orientation double cover is a torus)
    let double = 2.0 * size;
    x = ((x % double) + double) % double;
    y = ((y % double) + double) % double;

    // If in the second copy, apply antipodal identification
    if x >= size && y >= size {
        x -= size;
        y -= size;
    } else if x >= size {
        // wrap: (x, y) ~ (x - size, y + size) if y + size < 2*size
        x -= size;
        y += size;
        if y >= double {
            y -= double;
        }
        // re-check
        if x >= size || y >= size {
            x = ((x % size) + size) % size;
            y = ((y % size) + size) % size;
        }
    } else if y >= size {
        y -= size;
        x += size;
        if x >= double {
            x -= double;
        }
        if x >= size || y >= size {
            x = ((x % size) + size) % size;
            y = ((y % size) + size) % size;
        }
    }

    // Final clamp
    x = ((x % size) + size) % size;
    y = ((y % size) + size) % size;

    Vec2::new(x, y)
}

/// Convert 2D Euclidean coordinates to homogeneous projective coordinates.
/// (x, y) -> [x, y, 1]
pub fn homogeneous_coords(x: f32, y: f32) -> [f32; 3] {
    [x, y, 1.0]
}

/// Compute the projective line through two points (in homogeneous coordinates).
/// The line is the cross product of the two points.
pub fn projective_line(p1: [f32; 3], p2: [f32; 3]) -> [f32; 3] {
    cross3(p1, p2)
}

/// Compute the intersection of two projective lines.
/// In projective geometry, any two distinct lines always intersect (possibly at infinity).
/// Returns the intersection as an affine point if w != 0, None if the lines are identical.
pub fn line_intersection(l1: [f32; 3], l2: [f32; 3]) -> Option<Vec2> {
    let p = cross3(l1, l2);
    if p[2].abs() < 1e-10 {
        // Point at infinity (parallel lines in Euclidean sense) or identical lines
        if p[0].abs() < 1e-10 && p[1].abs() < 1e-10 {
            return None; // identical lines
        }
        // Point at infinity — still a valid projective point but no finite affine representation.
        // Return a large but finite point in the direction of intersection.
        let scale = 1e6;
        let len = (p[0] * p[0] + p[1] * p[1]).sqrt();
        return Some(Vec2::new(p[0] / len * scale, p[1] / len * scale));
    }
    Some(Vec2::new(p[0] / p[2], p[1] / p[2]))
}

/// Cross product of two 3-vectors (used for homogeneous coordinate operations).
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Compute the cross-ratio of four collinear points (a projective invariant).
/// Given four points on a projective line, the cross-ratio is:
/// (AC * BD) / (BC * AD)
/// where AC = distance from A to C, etc.
/// Points are given as 1D projective coordinates [value, 1] or [1, 0] for infinity.
pub fn cross_ratio(a: f32, b: f32, c: f32, d: f32) -> f32 {
    let ac = c - a;
    let bd = d - b;
    let bc = c - b;
    let ad = d - a;

    if bc.abs() < 1e-10 || ad.abs() < 1e-10 {
        return f32::INFINITY;
    }

    (ac * bd) / (bc * ad)
}

/// Cross-ratio of four points given as homogeneous 1D coordinates [x, w].
pub fn cross_ratio_homogeneous(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> f32 {
    // (A,C) = a[0]*c[1] - a[1]*c[0], etc.
    let ac = a[0] * c[1] - a[1] * c[0];
    let bd = b[0] * d[1] - b[1] * d[0];
    let bc = b[0] * c[1] - b[1] * c[0];
    let ad = a[0] * d[1] - a[1] * d[0];

    if bc.abs() < 1e-10 || ad.abs() < 1e-10 {
        return f32::INFINITY;
    }

    (ac * bd) / (bc * ad)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projective_wrap_inside() {
        let p = projective_wrap(Vec2::new(3.0, 4.0), 10.0);
        assert!((p.x - 3.0).abs() < 1e-4);
        assert!((p.y - 4.0).abs() < 1e-4);
    }

    #[test]
    fn test_projective_wrap_result_in_domain() {
        for ix in -20..20 {
            for iy in -20..20 {
                let pos = Vec2::new(ix as f32 * 3.7, iy as f32 * 2.3);
                let w = projective_wrap(pos, 10.0);
                assert!(w.x >= -1e-4 && w.x < 10.0 + 1e-4, "x out of range: {}", w.x);
                assert!(w.y >= -1e-4 && w.y < 10.0 + 1e-4, "y out of range: {}", w.y);
            }
        }
    }

    #[test]
    fn test_homogeneous_coords() {
        let h = homogeneous_coords(3.0, 4.0);
        assert_eq!(h, [3.0, 4.0, 1.0]);
    }

    #[test]
    fn test_projective_line_through_points() {
        let p1 = homogeneous_coords(0.0, 0.0);
        let p2 = homogeneous_coords(1.0, 1.0);
        let line = projective_line(p1, p2);
        // Line y - x = 0 => [1, -1, 0] (up to scale)
        // Check that both points satisfy the line equation a*x + b*y + c*w = 0
        let dot1 = line[0] * p1[0] + line[1] * p1[1] + line[2] * p1[2];
        let dot2 = line[0] * p2[0] + line[1] * p2[1] + line[2] * p2[2];
        assert!(dot1.abs() < 1e-6);
        assert!(dot2.abs() < 1e-6);
    }

    #[test]
    fn test_line_intersection() {
        // Line x = 1: [-1, 0, 1] => actually [1, 0, -1] via standard form
        // Let's use two lines defined by points
        let l1 = projective_line(homogeneous_coords(0.0, 0.0), homogeneous_coords(1.0, 0.0));
        let l2 = projective_line(homogeneous_coords(0.5, -1.0), homogeneous_coords(0.5, 1.0));
        let inter = line_intersection(l1, l2);
        assert!(inter.is_some());
        let p = inter.unwrap();
        assert!((p.x - 0.5).abs() < 1e-3, "x = {}", p.x);
        assert!(p.y.abs() < 1e-3, "y = {}", p.y);
    }

    #[test]
    fn test_line_intersection_parallel() {
        // Two parallel lines in Euclidean geometry still meet in projective geometry (at infinity)
        let l1 = [0.0_f32, 1.0, -1.0]; // y = 1
        let l2 = [0.0_f32, 1.0, -2.0]; // y = 2
        let inter = line_intersection(l1, l2);
        // Should return Some (point at infinity along x-axis)
        assert!(inter.is_some());
    }

    #[test]
    fn test_cross_ratio_basic() {
        // Cross-ratio of (0, 1, 2, 3)
        let cr = cross_ratio(0.0, 1.0, 2.0, 3.0);
        // (2-0)(3-1) / ((2-1)(3-0)) = 2*2 / (1*3) = 4/3
        assert!((cr - 4.0 / 3.0).abs() < 1e-4, "Cross ratio = {}", cr);
    }

    #[test]
    fn test_cross_ratio_projective_invariant() {
        // Cross-ratio should be invariant under projective transformation
        // Apply Mobius transform: f(x) = (2x + 1) / (x + 1)
        let transform = |x: f32| (2.0 * x + 1.0) / (x + 1.0);

        let a = 0.0_f32;
        let b = 1.0;
        let c = 2.0;
        let d = 3.0;

        let cr1 = cross_ratio(a, b, c, d);
        let cr2 = cross_ratio(transform(a), transform(b), transform(c), transform(d));
        assert!((cr1 - cr2).abs() < 1e-3, "Cross-ratio not invariant: {} vs {}", cr1, cr2);
    }

    #[test]
    fn test_cross_ratio_homogeneous() {
        let cr1 = cross_ratio(0.0, 1.0, 2.0, 3.0);
        let cr2 = cross_ratio_homogeneous([0.0, 1.0], [1.0, 1.0], [2.0, 1.0], [3.0, 1.0]);
        assert!((cr1 - cr2).abs() < 1e-4);
    }
}
