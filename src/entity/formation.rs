//! Target formation shapes for amorphous entities.
//!
//! Every formation is a mathematical function mapping index → 3D offset.
//! Formations are composable: scale, rotate, translate, and join operations
//! allow building complex shapes from simple primitives.

use glam::Vec3;
use std::f32::consts::{TAU, PI};

/// A formation: a list of 3D offsets from the entity center with per-slot characters.
#[derive(Clone, Debug)]
pub struct Formation {
    pub positions: Vec<Vec3>,
    pub chars: Vec<char>,
}

// ── Primitive formations ───────────────────────────────────────────────────────

impl Formation {
    /// A single glyph at the origin.
    pub fn single(ch: char) -> Self {
        Self { positions: vec![Vec3::ZERO], chars: vec![ch] }
    }

    /// A 3×3 grid (9 glyphs).
    pub fn grid_3x3() -> Self {
        Self::grid(3, 3, 1.0)
    }

    /// An N×M grid with given spacing.
    pub fn grid(cols: i32, rows: i32, spacing: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let cx = (cols - 1) as f32 * spacing * 0.5;
        let cy = (rows - 1) as f32 * spacing * 0.5;
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f32 * spacing - cx;
                let y = row as f32 * spacing - cy;
                positions.push(Vec3::new(x, y, 0.0));
                chars.push(if col == cols / 2 && row == rows / 2 { '@' } else { '#' });
            }
        }
        Self { positions, chars }
    }

    /// A circle of N glyphs at the given radius.
    pub fn circle(n: usize, radius: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        for i in 0..n {
            let angle = i as f32 / n as f32 * TAU;
            positions.push(Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0));
            chars.push('◆');
        }
        Self { positions, chars }
    }

    /// Concentric rings. Each entry is (glyph_count, radius).
    pub fn rings(spec: &[(usize, f32)]) -> Self {
        let ring_chars = ['◆', '◇', '◈', '◉', '·', '·'];
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        // Center glyph
        positions.push(Vec3::ZERO);
        chars.push('◈');
        for (ri, &(n, radius)) in spec.iter().enumerate() {
            let ch = ring_chars[ri.min(ring_chars.len() - 1)];
            for i in 0..n {
                let angle = i as f32 / n as f32 * TAU;
                positions.push(Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0));
                chars.push(ch);
            }
        }
        Self { positions, chars }
    }

    /// Diamond shape (all integer (x,y) within Manhattan distance ≤ size).
    pub fn diamond(size: i32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        for y in -size..=size {
            for x in -size..=size {
                if x.abs() + y.abs() <= size {
                    positions.push(Vec3::new(x as f32, y as f32, 0.0));
                    chars.push(if x == 0 && y == 0 { '◈' } else { '◇' });
                }
            }
        }
        Self { positions, chars }
    }

    /// Cross / plus sign with arm_length glyphs per arm (center + 4 arms).
    pub fn cross(arm_length: i32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        positions.push(Vec3::ZERO);
        chars.push('╬');
        for i in 1..=arm_length {
            for &(dx, dy, ch) in &[(1i32,0i32,'═'),(-1,0,'═'),(0,1,'║'),(0,-1,'║')] {
                positions.push(Vec3::new(dx as f32 * i as f32, dy as f32 * i as f32, 0.0));
                chars.push(ch);
            }
        }
        Self { positions, chars }
    }

    /// N-pointed star with inner_r and outer_r radii.
    pub fn star(points: usize, inner_r: f32, outer_r: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let n = points * 2;
        for i in 0..n {
            let angle = i as f32 / n as f32 * TAU - PI / 2.0;
            let r = if i % 2 == 0 { outer_r } else { inner_r };
            positions.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
            chars.push(if i % 2 == 0 { '★' } else { '·' });
        }
        Self { positions, chars }
    }

    /// Hexagonal close-pack: all hex centers within radius.
    pub fn hex_cluster(radius: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let spacing = 1.0f32;
        let h = spacing * 3.0f32.sqrt() * 0.5;
        let imax = (radius / spacing) as i32 + 2;
        for row in -imax..=imax {
            for col in -imax..=imax {
                let x = col as f32 * spacing + (row % 2) as f32 * spacing * 0.5;
                let y = row as f32 * h;
                if (x * x + y * y).sqrt() <= radius {
                    positions.push(Vec3::new(x, y, 0.0));
                    chars.push(if x.abs() < 0.01 && y.abs() < 0.01 { '⊕' } else { '◆' });
                }
            }
        }
        Self { positions, chars }
    }

    /// Archimedean spiral: r = a * θ.
    pub fn spiral(turns: f32, density: usize) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let n = (turns * density as f32) as usize;
        let spiral_chars = ['·', '·', '✦', '·', '·', '+'];
        for i in 0..n {
            let t = i as f32 / density as f32;
            let angle = t * TAU;
            let r = t * 0.6;
            positions.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
            chars.push(spiral_chars[i % spiral_chars.len()]);
        }
        Self { positions, chars }
    }

    /// Fibonacci (golden-ratio) phyllotaxis spiral — N points.
    pub fn fibonacci_spiral(n: usize, scale: f32) -> Self {
        let phi = (1.0 + 5.0f32.sqrt()) * 0.5;
        let golden_angle = TAU / (phi * phi);
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let fib_chars = ['·', '+', '✦', '·', '*', '◆'];
        for i in 0..n {
            let r = (i as f32).sqrt() * scale;
            let angle = i as f32 * golden_angle;
            positions.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
            chars.push(fib_chars[i % fib_chars.len()]);
        }
        Self { positions, chars }
    }

    /// Double helix (DNA-like): two interleaved spirals winding vertically.
    pub fn dna_helix(height: f32, turns: f32, n: usize) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let radius = 1.5f32;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let angle = t * turns * TAU;
            let y = (t - 0.5) * height;
            // Strand A
            positions.push(Vec3::new(angle.cos() * radius, y, angle.sin() * radius));
            chars.push('◆');
            // Strand B (opposite phase)
            let angle_b = angle + PI;
            positions.push(Vec3::new(angle_b.cos() * radius, y, angle_b.sin() * radius));
            chars.push('◇');
            // Rung every 4 glyphs
            if i % 4 == 0 {
                let mid_x = (angle.cos() + angle_b.cos()) * radius * 0.5;
                let mid_z = (angle.sin() + angle_b.sin()) * radius * 0.5;
                positions.push(Vec3::new(mid_x, y, mid_z));
                chars.push('═');
            }
        }
        Self { positions, chars }
    }

    /// Triangle (equilateral) of side length `size`.
    pub fn triangle(size: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let n = (size as usize).max(2);
        // Three vertices
        let verts = [
            Vec3::new(0.0, size * 0.577, 0.0),
            Vec3::new(-size * 0.5, -size * 0.289, 0.0),
            Vec3::new( size * 0.5, -size * 0.289, 0.0),
        ];
        for side in 0..3 {
            let a = verts[side];
            let b = verts[(side + 1) % 3];
            for i in 0..n {
                let t = i as f32 / n as f32;
                positions.push(a + (b - a) * t);
                chars.push(if i == 0 { '▲' } else { '△' });
            }
        }
        Self { positions, chars }
    }

    /// Lissajous figure: parametric (sin(a·t+δ), sin(b·t)) for n points.
    pub fn lissajous(n: usize, a: f32, b: f32, delta: f32, scale: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let liss_chars = ['·', '✦', '+', '·', '*'];
        for i in 0..n {
            let t = i as f32 / n as f32 * TAU;
            let x = (a * t + delta).sin() * scale;
            let y = (b * t).sin() * scale;
            positions.push(Vec3::new(x, y, 0.0));
            chars.push(liss_chars[i % liss_chars.len()]);
        }
        Self { positions, chars }
    }

    /// Lorenz attractor sample: step the attractor n times and record positions.
    pub fn lorenz_trace(n: usize, scale: f32) -> Self {
        use crate::math::attractors::{step, initial_state, AttractorType};
        let mut state = initial_state(AttractorType::Lorenz);
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let trace_chars = ['·', '·', '✦', '·', '·', '*', '·'];
        // Warm up
        for _ in 0..500 {
            let (next, _) = step(AttractorType::Lorenz, state, 0.01);
            state = next;
        }
        for i in 0..n {
            let (next, _) = step(AttractorType::Lorenz, state, 0.01);
            state = next;
            positions.push(state * scale);
            chars.push(trace_chars[i % trace_chars.len()]);
        }
        Self { positions, chars }
    }

    /// Arrow pointing in a direction (normalized), length glyphs long.
    pub fn arrow(length: usize, direction: Vec3) -> Self {
        let dir = direction.normalize_or_zero();
        let right = if dir.abs().dot(Vec3::X) < 0.9 {
            dir.cross(Vec3::X).normalize()
        } else {
            dir.cross(Vec3::Y).normalize()
        };
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        for i in 0..length {
            positions.push(dir * i as f32);
            chars.push('·');
        }
        // Arrowhead (3 glyphs)
        let tip = dir * length as f32;
        positions.push(tip);
        chars.push('►');
        positions.push(tip - dir * 0.8 + right * 0.5);
        chars.push('╱');
        positions.push(tip - dir * 0.8 - right * 0.5);
        chars.push('╲');
        Self { positions, chars }
    }

    /// Random scatter within radius, seeded for reproducibility.
    pub fn scatter(n: usize, radius: f32, seed: u64) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let scatter_chars = ['·', '+', '✦', '*', '◆', '◇'];
        let mut rng = seed;
        for i in 0..n {
            rng = rng.wrapping_mul(0x6c62272e07bb0142).wrapping_add(0x62b821756295c58d);
            let x = ((rng >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            rng = rng.wrapping_mul(0x6c62272e07bb0142).wrapping_add(0x62b821756295c58d);
            let y = ((rng >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            rng = rng.wrapping_mul(0x6c62272e07bb0142).wrapping_add(0x62b821756295c58d);
            let z = ((rng >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let v = Vec3::new(x, y, z).normalize_or_zero();
            rng = rng.wrapping_mul(0x6c62272e07bb0142).wrapping_add(0x62b821756295c58d);
            let r = ((rng >> 32) as f32 / u32::MAX as f32).sqrt() * radius;
            positions.push(v * r);
            chars.push(scatter_chars[i % scatter_chars.len()]);
        }
        Self { positions, chars }
    }

    // ── Rune formations ──────────────────────────────────────────────────────

    /// Rune: Sigma / S-curve.
    pub fn rune_sigma() -> Self {
        let pts: &[(f32, f32, char)] = &[
            (1.5,  1.0, '╗'), (0.5,  1.0, '═'), (-0.5,  1.0, '═'), (-1.5,  1.0, '╔'),
            (1.5,  0.0, '·'), (-1.5,  0.0, '·'),
            (1.5, -1.0, '╝'), (0.5, -1.0, '═'), (-0.5, -1.0, '═'), (-1.5, -1.0, '╚'),
            (0.0,  0.5, '╲'), (0.0, -0.5, '╲'),
        ];
        let positions = pts.iter().map(|&(x, y, _)| Vec3::new(x, y, 0.0)).collect();
        let chars = pts.iter().map(|&(_, _, c)| c).collect();
        Self { positions, chars }
    }

    /// Rune: Infinity / ∞ shape.
    pub fn rune_infinity() -> Self {
        let n = 32usize;
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let inf_chars = ['·', '◆', '·', '✦'];
        for i in 0..n {
            let t = i as f32 / n as f32 * TAU;
            // Lemniscate of Bernoulli
            let denom = 1.0 + (t.sin()).powi(2);
            let x = 2.5 * t.cos() / denom;
            let y = 2.5 * t.cos() * t.sin() / denom;
            positions.push(Vec3::new(x, y, 0.0));
            chars.push(inf_chars[i % inf_chars.len()]);
        }
        Self { positions, chars }
    }

    /// Rune: Chaos — a 3-armed triskelion.
    pub fn rune_chaos() -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        let arms = 3;
        let arm_len = 8;
        for arm in 0..arms {
            let base_angle = arm as f32 / arms as f32 * TAU;
            for j in 0..arm_len {
                let t = j as f32 / arm_len as f32;
                let angle = base_angle + t * PI * 0.5;
                let r = t * 2.5;
                positions.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
                chars.push(if j == arm_len - 1 { '▲' } else { '·' });
            }
        }
        // Center
        positions.push(Vec3::ZERO);
        chars.push('⊕');
        Self { positions, chars }
    }

    // ── Boss formations ──────────────────────────────────────────────────────

    /// Boss formation: Sierpinski triangle approximation (2 levels deep).
    pub fn sierpinski(depth: u32, size: f32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        sierpinski_recurse(&mut positions, &mut chars,
            Vec3::new(0.0, size * 0.577, 0.0),
            Vec3::new(-size * 0.5, -size * 0.289, 0.0),
            Vec3::new( size * 0.5, -size * 0.289, 0.0),
            depth);
        Self { positions, chars }
    }

    /// Boss formation: Mandala (multiple concentric rings with rotational symmetry).
    pub fn mandala(layers: usize, base_n: usize, base_r: f32) -> Self {
        let mandala_chars = ['◆', '◇', '◈', '✦', '★', '·'];
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        positions.push(Vec3::ZERO);
        chars.push('⊙');
        for layer in 0..layers {
            let n = base_n + layer * base_n;
            let r = base_r * (layer + 1) as f32;
            let phase_offset = if layer % 2 == 0 { 0.0 } else { PI / n as f32 };
            let ch = mandala_chars[layer.min(mandala_chars.len() - 1)];
            for i in 0..n {
                let angle = i as f32 / n as f32 * TAU + phase_offset;
                positions.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
                chars.push(ch);
            }
        }
        Self { positions, chars }
    }

    // ── Combinators ───────────────────────────────────────────────────────────

    /// Scale all positions by a scalar.
    pub fn scaled(mut self, s: f32) -> Self {
        for p in &mut self.positions { *p *= s; }
        self
    }

    /// Rotate the entire formation around the Z axis by `angle` radians.
    pub fn rotated_z(mut self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        for p in &mut self.positions {
            let x = p.x * c - p.y * s;
            let y = p.x * s + p.y * c;
            p.x = x;
            p.y = y;
        }
        self
    }

    /// Translate all positions by an offset.
    pub fn translated(mut self, offset: Vec3) -> Self {
        for p in &mut self.positions { *p += offset; }
        self
    }

    /// Reflect across the Y axis (mirror left-right).
    pub fn mirrored_x(mut self) -> Self {
        for p in &mut self.positions { p.x = -p.x; }
        self
    }

    /// Combine two formations into one.
    pub fn join(mut self, other: Formation) -> Self {
        self.positions.extend(other.positions);
        self.chars.extend(other.chars);
        self
    }

    /// Replace all characters with a single glyph.
    pub fn with_char(mut self, ch: char) -> Self {
        self.chars.iter_mut().for_each(|c| *c = ch);
        self
    }

    /// Number of glyphs.
    pub fn len(&self) -> usize { self.positions.len() }

    /// True if empty.
    pub fn is_empty(&self) -> bool { self.positions.is_empty() }

    /// Compute the centroid of all positions.
    pub fn centroid(&self) -> Vec3 {
        if self.positions.is_empty() { return Vec3::ZERO; }
        self.positions.iter().copied().sum::<Vec3>() / self.positions.len() as f32
    }

    /// Compute bounding radius (max distance from centroid).
    pub fn bounding_radius(&self) -> f32 {
        let c = self.centroid();
        self.positions.iter().map(|p| (*p - c).length()).fold(0.0f32, f32::max)
    }

    /// Normalize so bounding_radius == 1.0.
    pub fn normalized(self) -> Self {
        let r = self.bounding_radius().max(0.001);
        self.scaled(1.0 / r)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn sierpinski_recurse(
    positions: &mut Vec<Vec3>,
    chars: &mut Vec<char>,
    a: Vec3, b: Vec3, c: Vec3,
    depth: u32,
) {
    if depth == 0 {
        positions.push(a);
        positions.push(b);
        positions.push(c);
        chars.push('▲');
        chars.push('▲');
        chars.push('▲');
        return;
    }
    let ab = (a + b) * 0.5;
    let bc = (b + c) * 0.5;
    let ca = (c + a) * 0.5;
    sierpinski_recurse(positions, chars, a, ab, ca, depth - 1);
    sierpinski_recurse(positions, chars, ab, b, bc, depth - 1);
    sierpinski_recurse(positions, chars, ca, bc, c, depth - 1);
}

// Fix: The cross formation uses numeric literals for dx/dy, not negation operators
// that could conflict with the unary minus. Redefine as a proper constant array.
impl Formation {
    /// Cross formation (reimplemented cleanly without operator conflicts).
    #[allow(dead_code)]
    fn cross_inner(arm_length: i32) -> Vec<(i32, i32, char)> {
        let mut pts = vec![(0, 0, '╬')];
        for i in 1..=arm_length {
            pts.push(( i,  0, '═'));
            pts.push((-i,  0, '═'));
            pts.push(( 0,  i, '║'));
            pts.push(( 0, -i, '║'));
        }
        pts
    }
}
