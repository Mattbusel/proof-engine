//! Target formation shapes for amorphous entities.

use glam::Vec3;

/// A named formation: a list of 3D offsets from the entity center.
pub struct Formation {
    pub positions: Vec<Vec3>,
    pub chars: Vec<char>,
}

impl Formation {
    /// A simple 3x3 grid (9 glyphs).
    pub fn grid_3x3() -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        for row in -1i32..=1 {
            for col in -1i32..=1 {
                positions.push(Vec3::new(col as f32, row as f32, 0.0));
                chars.push(if row == 0 && col == 0 { '@' } else { '#' });
            }
        }
        Self { positions, chars }
    }

    /// A circle of N glyphs at radius r.
    pub fn circle(n: usize, radius: f32) -> Self {
        use std::f32::consts::TAU;
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        for i in 0..n {
            let angle = i as f32 / n as f32 * TAU;
            positions.push(Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0));
            chars.push('◆');
        }
        Self { positions, chars }
    }

    /// A diamond shape.
    pub fn diamond(size: i32) -> Self {
        let mut positions = Vec::new();
        let mut chars = Vec::new();
        for y in -size..=size {
            for x in -size..=size {
                if (x.abs() + y.abs()) <= size {
                    positions.push(Vec3::new(x as f32, y as f32, 0.0));
                    chars.push(if x == 0 && y == 0 { '◈' } else { '◇' });
                }
            }
        }
        Self { positions, chars }
    }
}
