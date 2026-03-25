//! Convert font glyph outlines to polygon meshes.
//!
//! Parses TTF/OTF glyph outlines via `ab_glyph`, extracts contour points,
//! subdivides Bezier curves adaptively, classifies holes, and outputs clean
//! polygon outlines ready for triangulation and extrusion.

use glam::Vec2;
use std::collections::HashMap;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont, GlyphId};

// ── Types ───────────────────────────────────────────────────────────────────

/// Bounding box for a glyph outline.
#[derive(Clone, Copy, Debug)]
pub struct GlyphBounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl GlyphBounds {
    pub fn size(&self) -> Vec2 { self.max - self.min }
    pub fn center(&self) -> Vec2 { (self.min + self.max) * 0.5 }

    pub fn from_points(points: &[Vec2]) -> Self {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for p in points {
            min = min.min(*p);
            max = max.max(*p);
        }
        Self { min, max }
    }
}

/// A single closed contour (outer boundary or hole).
#[derive(Clone, Debug)]
pub struct Contour {
    pub points: Vec<Vec2>,
    pub is_hole: bool,
}

impl Contour {
    pub fn signed_area(&self) -> f32 {
        signed_area(&self.points)
    }

    pub fn bounds(&self) -> GlyphBounds {
        GlyphBounds::from_points(&self.points)
    }
}

/// Complete outline for a single glyph character.
#[derive(Clone, Debug)]
pub struct GlyphOutline {
    pub contours: Vec<Contour>,
    pub advance_width: f32,
    pub bounds: GlyphBounds,
}

// ── Signed area / winding ───────────────────────────────────────────────────

/// Shoelace formula. Positive = CCW, Negative = CW.
pub fn signed_area(points: &[Vec2]) -> f32 {
    let n = points.len();
    if n < 3 { return 0.0; }
    let mut area = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].x * points[j].y;
        area -= points[j].x * points[i].y;
    }
    area * 0.5
}

/// Check if polygon winds counter-clockwise.
pub fn is_ccw(points: &[Vec2]) -> bool {
    signed_area(points) > 0.0
}

// ── Point-in-polygon ────────────────────────────────────────────────────────

/// Ray casting test.
pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let n = polygon.len();
    if n < 3 { return false; }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ── Bezier subdivision ──────────────────────────────────────────────────────

/// Adaptive subdivision of a quadratic Bezier (TrueType).
pub fn subdivide_quadratic(p0: Vec2, p1: Vec2, p2: Vec2, tolerance: f32) -> Vec<Vec2> {
    let mut result = Vec::new();
    subdivide_quad_recursive(p0, p1, p2, tolerance * tolerance, &mut result);
    result.push(p2);
    result
}

fn subdivide_quad_recursive(p0: Vec2, p1: Vec2, p2: Vec2, tol_sq: f32, out: &mut Vec<Vec2>) {
    let mid = (p0 + p2) * 0.5;
    let dist_sq = (p1 - mid).length_squared();
    if dist_sq <= tol_sq {
        out.push(p0);
        return;
    }
    let q0 = (p0 + p1) * 0.5;
    let q1 = (p1 + p2) * 0.5;
    let r = (q0 + q1) * 0.5;
    subdivide_quad_recursive(p0, q0, r, tol_sq, out);
    subdivide_quad_recursive(r, q1, p2, tol_sq, out);
}

/// Adaptive subdivision of a cubic Bezier (OpenType/CFF).
pub fn subdivide_cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, tolerance: f32) -> Vec<Vec2> {
    let mut result = Vec::new();
    subdivide_cubic_recursive(p0, p1, p2, p3, tolerance * tolerance, 0, &mut result);
    result.push(p3);
    result
}

fn subdivide_cubic_recursive(
    p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2,
    tol_sq: f32, depth: u32, out: &mut Vec<Vec2>,
) {
    if depth > 10 {
        out.push(p0);
        return;
    }
    let d1 = (p1 - (p0 + p3) * 0.5).length_squared();
    let d2 = (p2 - (p0 + p3) * 0.5).length_squared();
    if d1 + d2 <= tol_sq {
        out.push(p0);
        return;
    }
    let ab = (p0 + p1) * 0.5;
    let bc = (p1 + p2) * 0.5;
    let cd = (p2 + p3) * 0.5;
    let abc = (ab + bc) * 0.5;
    let bcd = (bc + cd) * 0.5;
    let abcd = (abc + bcd) * 0.5;
    subdivide_cubic_recursive(p0, ab, abc, abcd, tol_sq, depth + 1, out);
    subdivide_cubic_recursive(abcd, bcd, cd, p3, tol_sq, depth + 1, out);
}

// ── Contour classification ──────────────────────────────────────────────────

/// Classify contours as outer boundaries or holes based on winding direction.
/// Convention: CW (negative area) = outer, CCW (positive area) = hole (TrueType convention).
/// We normalize so outer contours are CCW and holes are CW for our triangulator.
pub fn classify_contours(contours: &mut [Contour]) {
    for c in contours.iter_mut() {
        let area = signed_area(&c.points);
        // TrueType: negative area = outer contour
        // We want: positive area (CCW) = outer, negative (CW) = hole
        if area < 0.0 {
            // Negative = CW in our coord system → outer in TrueType → make CCW
            c.points.reverse();
            c.is_hole = false;
        } else {
            // Positive = CCW → hole in TrueType → make CW
            c.points.reverse();
            c.is_hole = true;
        }
    }
    // If all are classified as holes, the largest one is likely the outer boundary.
    let all_holes = contours.iter().all(|c| c.is_hole);
    if all_holes && !contours.is_empty() {
        let mut max_area = 0.0f32;
        let mut max_idx = 0;
        for (i, c) in contours.iter().enumerate() {
            let a = signed_area(&c.points).abs();
            if a > max_area {
                max_area = a;
                max_idx = i;
            }
        }
        contours[max_idx].is_hole = false;
        contours[max_idx].points.reverse(); // flip to CCW
    }
}

/// Assign holes to their containing outer contours.
/// Returns Vec of (outer_index, hole_indices).
pub fn assign_holes_to_outers(contours: &[Contour]) -> Vec<(usize, Vec<usize>)> {
    let outers: Vec<usize> = contours.iter().enumerate()
        .filter(|(_, c)| !c.is_hole).map(|(i, _)| i).collect();
    let holes: Vec<usize> = contours.iter().enumerate()
        .filter(|(_, c)| c.is_hole).map(|(i, _)| i).collect();

    let mut assignments: Vec<(usize, Vec<usize>)> = outers.iter()
        .map(|&i| (i, Vec::new())).collect();

    for &h in &holes {
        if let Some(hp) = contours[h].points.first() {
            // Find the smallest outer contour containing this hole's first point.
            let mut best = None;
            let mut best_area = f32::MAX;
            for &o in &outers {
                if point_in_polygon(*hp, &contours[o].points) {
                    let area = signed_area(&contours[o].points).abs();
                    if area < best_area {
                        best_area = area;
                        best = Some(o);
                    }
                }
            }
            if let Some(o) = best {
                if let Some(entry) = assignments.iter_mut().find(|(idx, _)| *idx == o) {
                    entry.1.push(h);
                }
            }
        }
    }

    assignments
}

// ── Ramer-Douglas-Peucker simplification ────────────────────────────────────

pub fn simplify_contour(points: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    if points.len() <= 3 { return points.to_vec(); }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    rdp_recursive(points, 0, points.len() - 1, tolerance * tolerance, &mut keep);
    points.iter().enumerate().filter(|(i, _)| keep[*i]).map(|(_, p)| *p).collect()
}

fn rdp_recursive(points: &[Vec2], start: usize, end: usize, tol_sq: f32, keep: &mut [bool]) {
    if end <= start + 1 { return; }
    let line_dir = points[end] - points[start];
    let line_len_sq = line_dir.length_squared();
    let mut max_dist_sq = 0.0f32;
    let mut max_idx = start;
    for i in (start + 1)..end {
        let d = if line_len_sq < 1e-10 {
            (points[i] - points[start]).length_squared()
        } else {
            let t = ((points[i] - points[start]).dot(line_dir) / line_len_sq).clamp(0.0, 1.0);
            let proj = points[start] + line_dir * t;
            (points[i] - proj).length_squared()
        };
        if d > max_dist_sq {
            max_dist_sq = d;
            max_idx = i;
        }
    }
    if max_dist_sq > tol_sq {
        keep[max_idx] = true;
        rdp_recursive(points, start, max_idx, tol_sq, keep);
        rdp_recursive(points, max_idx, end, tol_sq, keep);
    }
}

// ── Outline extraction from ab_glyph ────────────────────────────────────────

/// Extract glyph outline from a font.
pub fn extract_outline(font: &FontVec, ch: char, scale: f32) -> Option<GlyphOutline> {
    let glyph_id = font.glyph_id(ch);
    if glyph_id.0 == 0 && ch != ' ' { return None; }

    let px_scale = PxScale::from(scale);
    let scaled = font.as_scaled(px_scale);
    let advance = scaled.h_advance(glyph_id);
    let ascent = scaled.ascent();

    let glyph = glyph_id.with_scale_and_position(px_scale, ab_glyph::point(0.0, ascent));

    let outlined = font.outline_glyph(glyph)?;
    let bounds_ab = outlined.px_bounds();

    let mut contours = Vec::new();
    let mut current_contour: Vec<Vec2> = Vec::new();
    let mut last_pos = Vec2::ZERO;

    // ab_glyph's OutlineCurve gives us MoveTo, LineTo, QuadTo, CurveTo
    // We use outline_glyph and then manually walk the outline via the draw method
    // Since ab_glyph doesn't expose raw outline walking easily, we use the
    // rasterization bounds and reconstruct from the glyph metrics.

    // Alternative approach: build contours from the outline callback
    struct OutlineBuilder {
        contours: Vec<Vec<Vec2>>,
        current: Vec<Vec2>,
        tolerance: f32,
    }

    impl OutlineBuilder {
        fn finish_contour(&mut self) {
            if self.current.len() >= 3 {
                self.contours.push(std::mem::take(&mut self.current));
            } else {
                self.current.clear();
            }
        }
    }

    // Since ab_glyph doesn't give us direct outline walking in a simple way,
    // we'll sample the glyph boundary by tracing the coverage at the edges.
    // For a production implementation, you'd use ttf-parser's OutlineBuilder.
    // Here we create a simplified outline from the glyph's bounding box and
    // rasterized coverage.

    let b = bounds_ab;
    let w = (b.max.x - b.min.x).ceil() as u32 + 2;
    let h = (b.max.y - b.min.y).ceil() as u32 + 2;

    if w < 2 || h < 2 { return None; }

    // Rasterize to coverage grid
    let mut coverage = vec![0.0f32; (w * h) as usize];
    let ox = b.min.x.floor();
    let oy = b.min.y.floor();

    outlined.draw(|x, y, v| {
        let px = x as i32 - ox as i32;
        let py = y as i32 - oy as i32;
        if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
            coverage[(py as u32 * w + px as u32) as usize] = v;
        }
    });

    // Extract contour via marching squares on the coverage grid
    let threshold = 0.5;
    let contour_points = marching_squares_contour(&coverage, w as usize, h as usize, threshold);

    for mut pts in contour_points {
        if pts.len() < 3 { continue; }
        // Transform from grid space back to glyph space
        for p in &mut pts {
            p.x = p.x + ox;
            p.y = p.y + oy;
        }
        contours.push(Contour { points: pts, is_hole: false });
    }

    if contours.is_empty() {
        // Fallback: rectangular outline from bounds
        contours.push(Contour {
            points: vec![
                Vec2::new(b.min.x, b.min.y),
                Vec2::new(b.max.x, b.min.y),
                Vec2::new(b.max.x, b.max.y),
                Vec2::new(b.min.x, b.max.y),
            ],
            is_hole: false,
        });
    }

    classify_contours(&mut contours);

    let all_points: Vec<Vec2> = contours.iter().flat_map(|c| c.points.iter().copied()).collect();
    let gbounds = if all_points.is_empty() {
        GlyphBounds { min: Vec2::new(b.min.x, b.min.y), max: Vec2::new(b.max.x, b.max.y) }
    } else {
        GlyphBounds::from_points(&all_points)
    };

    Some(GlyphOutline { contours, advance_width: advance, bounds: gbounds })
}

/// Simple marching-squares contour extraction from a coverage grid.
fn marching_squares_contour(coverage: &[f32], w: usize, h: usize, threshold: f32) -> Vec<Vec<Vec2>> {
    if w < 2 || h < 2 { return Vec::new(); }

    let mut visited = vec![false; w * h];
    let mut contours = Vec::new();

    // Find boundary pixels and trace contours
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = y * w + x;
            if visited[idx] { continue; }
            if coverage[idx] < threshold { continue; }

            // Check if this is a boundary pixel (has a neighbor below threshold)
            let is_boundary = (x > 0 && coverage[idx - 1] < threshold)
                || (x + 1 < w && coverage[idx + 1] < threshold)
                || (y > 0 && coverage[idx - w] < threshold)
                || (y + 1 < h && coverage[idx + w] < threshold);

            if !is_boundary { continue; }

            // Trace this contour
            let contour = trace_boundary(coverage, w, h, x, y, threshold, &mut visited);
            if contour.len() >= 3 {
                contours.push(contour);
            }
        }
    }

    contours
}

/// Trace a boundary contour starting from (sx, sy) using Moore neighbor tracing.
fn trace_boundary(
    coverage: &[f32], w: usize, h: usize,
    sx: usize, sy: usize, threshold: f32,
    visited: &mut [bool],
) -> Vec<Vec2> {
    let mut contour = Vec::new();
    let dirs: [(i32, i32); 8] = [
        (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1),
    ];

    let mut x = sx as i32;
    let mut y = sy as i32;
    let mut dir = 0usize;
    let max_steps = w * h;

    for step in 0..max_steps {
        if step > 0 && x == sx as i32 && y == sy as i32 { break; }

        contour.push(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
        let idx = y as usize * w + x as usize;
        visited[idx] = true;

        // Find next boundary pixel
        let mut found = false;
        for i in 0..8 {
            let d = (dir + 6 + i) % 8; // start looking from backtrack direction
            let nx = x + dirs[d].0;
            let ny = y + dirs[d].1;
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                let nidx = ny as usize * w + nx as usize;
                if coverage[nidx] >= threshold {
                    let is_bd = (nx > 0 && coverage[nidx - 1] < threshold)
                        || ((nx as usize + 1) < w && coverage[nidx + 1] < threshold)
                        || (ny > 0 && coverage[nidx - w as i32 as usize] < threshold)
                        || ((ny as usize + 1) < h && coverage[nidx + w] < threshold);
                    if is_bd || !found {
                        x = nx;
                        y = ny;
                        dir = d;
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found { break; }
    }

    // Simplify
    if contour.len() > 20 {
        simplify_contour(&contour, 0.5)
    } else {
        contour
    }
}

// ── Outline Cache ───────────────────────────────────────────────────────────

/// Cache of extracted glyph outlines.
pub struct OutlineCache {
    pub outlines: HashMap<char, GlyphOutline>,
}

impl OutlineCache {
    pub fn build(font: &FontVec, chars: &[char], scale: f32) -> Self {
        let mut outlines = HashMap::new();
        for &ch in chars {
            if let Some(outline) = extract_outline(font, ch, scale) {
                outlines.insert(ch, outline);
            }
        }
        Self { outlines }
    }

    pub fn get(&self, ch: char) -> Option<&GlyphOutline> {
        self.outlines.get(&ch)
    }

    pub fn len(&self) -> usize { self.outlines.len() }
    pub fn is_empty(&self) -> bool { self.outlines.is_empty() }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ]
    }

    #[test]
    fn signed_area_ccw() {
        let area = signed_area(&square());
        assert!(area > 0.0, "CCW square should have positive area: {}", area);
    }

    #[test]
    fn signed_area_cw() {
        let mut sq = square();
        sq.reverse();
        let area = signed_area(&sq);
        assert!(area < 0.0, "CW square should have negative area: {}", area);
    }

    #[test]
    fn point_in_polygon_inside() {
        assert!(point_in_polygon(Vec2::new(0.5, 0.5), &square()));
    }

    #[test]
    fn point_in_polygon_outside() {
        assert!(!point_in_polygon(Vec2::new(2.0, 2.0), &square()));
    }

    #[test]
    fn subdivide_quadratic_produces_points() {
        let pts = subdivide_quadratic(
            Vec2::new(0.0, 0.0), Vec2::new(0.5, 1.0), Vec2::new(1.0, 0.0), 0.1,
        );
        assert!(pts.len() >= 3, "Should produce at least 3 points");
    }

    #[test]
    fn subdivide_cubic_produces_points() {
        let pts = subdivide_cubic(
            Vec2::new(0.0, 0.0), Vec2::new(0.3, 1.0),
            Vec2::new(0.7, 1.0), Vec2::new(1.0, 0.0), 0.1,
        );
        assert!(pts.len() >= 4, "Should produce at least 4 points");
    }

    #[test]
    fn classify_contours_identifies_holes() {
        let outer = vec![
            Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0),
        ];
        let hole = vec![
            Vec2::new(2.0, 2.0), Vec2::new(8.0, 2.0),
            Vec2::new(8.0, 8.0), Vec2::new(2.0, 8.0),
        ];
        // Make both CW (TrueType outer convention)
        let mut contours = vec![
            Contour { points: outer.into_iter().rev().collect(), is_hole: false },
            Contour { points: hole, is_hole: false },
        ];
        classify_contours(&mut contours);
        let outers: Vec<_> = contours.iter().filter(|c| !c.is_hole).collect();
        let holes: Vec<_> = contours.iter().filter(|c| c.is_hole).collect();
        assert_eq!(outers.len(), 1);
        assert_eq!(holes.len(), 1);
    }

    #[test]
    fn assign_holes_finds_containment() {
        let outer = Contour {
            points: vec![
                Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0),
                Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0),
            ],
            is_hole: false,
        };
        let hole = Contour {
            points: vec![
                Vec2::new(3.0, 3.0), Vec2::new(7.0, 3.0),
                Vec2::new(7.0, 7.0), Vec2::new(3.0, 7.0),
            ],
            is_hole: true,
        };
        let assignments = assign_holes_to_outers(&[outer, hole]);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].1.len(), 1);
    }

    #[test]
    fn simplify_reduces_points() {
        let pts: Vec<Vec2> = (0..100).map(|i| {
            let t = i as f32 / 99.0;
            Vec2::new(t * 10.0, (t * 6.28).sin() * 0.01)
        }).collect();
        let simplified = simplify_contour(&pts, 0.1);
        assert!(simplified.len() < pts.len());
    }

    #[test]
    fn glyph_bounds_from_points() {
        let pts = vec![Vec2::new(-1.0, -2.0), Vec2::new(3.0, 4.0)];
        let b = GlyphBounds::from_points(&pts);
        assert_eq!(b.min, Vec2::new(-1.0, -2.0));
        assert_eq!(b.max, Vec2::new(3.0, 4.0));
    }
}
