//! Real-time surface deformation driven by force fields and math functions.

use glam::Vec3;
use super::GeoMesh;
use crate::math::{MathFunction, ForceField};

/// A deformation operation to apply to mesh vertices.
#[derive(Debug, Clone)]
pub enum DeformField {
    /// Displace each vertex along its normal by a scalar function of position.
    NormalDisplace { amplitude: f32, func: MathFunction },
    /// Radial displacement from a center point.
    Radial { center: Vec3, amplitude: f32, falloff: f32 },
    /// Twist around an axis.
    Twist { axis: Vec3, center: Vec3, angle_per_unit: f32 },
    /// Bend around an axis.
    Bend { axis: Vec3, center: Vec3, angle: f32, region_size: f32 },
    /// Noise-based displacement.
    Noise { amplitude: f32, frequency: f32, time: f32 },
    /// Sine wave along an axis.
    Wave { direction: Vec3, amplitude: f32, frequency: f32, phase: f32 },
    /// Pinch/inflate around a point.
    Pinch { center: Vec3, radius: f32, strength: f32 },
    /// Spherize: push vertices toward a sphere.
    Spherize { center: Vec3, radius: f32, strength: f32 },
    /// Taper: scale vertices based on distance along an axis.
    Taper { axis: Vec3, center: Vec3, start_scale: f32, end_scale: f32, length: f32 },
    /// Apply a force field's force as displacement.
    ForceFieldDisplace { field: ForceField, scale: f32, time: f32 },
}

/// Applies deformations to meshes.
pub struct Deformer;

impl Deformer {
    /// Apply a deformation field to a mesh in-place.
    pub fn apply(mesh: &mut GeoMesh, deform: &DeformField) {
        match deform {
            DeformField::NormalDisplace { amplitude, func } => {
                for i in 0..mesh.vertices.len() {
                    let p = mesh.vertices[i];
                    let n = mesh.normals[i];
                    let scalar = func.evaluate(p.x + p.y + p.z, p.length());
                    mesh.vertices[i] = p + n * scalar * *amplitude;
                }
            }
            DeformField::Radial { center, amplitude, falloff } => {
                for v in &mut mesh.vertices {
                    let dir = *v - *center;
                    let dist = dir.length();
                    let weight = (-dist * falloff).exp();
                    *v += dir.normalize_or_zero() * weight * *amplitude;
                }
            }
            DeformField::Twist { axis, center, angle_per_unit } => {
                let axis_n = axis.normalize_or_zero();
                for v in &mut mesh.vertices {
                    let d = (*v - *center).dot(axis_n);
                    let angle = d * angle_per_unit;
                    let (s, c) = angle.sin_cos();
                    let local = *v - *center;
                    let proj = axis_n * local.dot(axis_n);
                    let perp = local - proj;
                    if perp.length_squared() < 1e-10 { continue; }
                    let t1 = perp.normalize();
                    let t2 = axis_n.cross(t1);
                    let r = perp.length();
                    *v = *center + proj + (t1 * c + t2 * s) * r;
                }
            }
            DeformField::Wave { direction, amplitude, frequency, phase } => {
                let dir_n = direction.normalize_or_zero();
                for v in &mut mesh.vertices {
                    let d = v.dot(dir_n);
                    let offset = (d * frequency + phase).sin() * amplitude;
                    *v += Vec3::Y * offset; // displace vertically
                }
            }
            DeformField::Noise { amplitude, frequency, time } => {
                for v in &mut mesh.vertices {
                    let nx = simple_hash(v.x * frequency + time) * amplitude;
                    let ny = simple_hash(v.y * frequency + time * 1.3) * amplitude;
                    let nz = simple_hash(v.z * frequency + time * 0.7) * amplitude;
                    *v += Vec3::new(nx, ny, nz);
                }
            }
            DeformField::Pinch { center, radius, strength } => {
                for v in &mut mesh.vertices {
                    let dir = *v - *center;
                    let dist = dir.length();
                    if dist < *radius && dist > 1e-6 {
                        let t = 1.0 - dist / radius;
                        *v = *center + dir * (1.0 - t * strength);
                    }
                }
            }
            DeformField::Spherize { center, radius, strength } => {
                for v in &mut mesh.vertices {
                    let dir = *v - *center;
                    let dist = dir.length();
                    if dist > 1e-6 {
                        let sphere_pos = *center + dir.normalize() * *radius;
                        *v = v.lerp(sphere_pos, *strength);
                    }
                }
            }
            DeformField::Taper { axis, center, start_scale, end_scale, length } => {
                let axis_n = axis.normalize_or_zero();
                for v in &mut mesh.vertices {
                    let d = (*v - *center).dot(axis_n);
                    let t = (d / length.max(1e-6)).clamp(0.0, 1.0);
                    let scale = start_scale + (end_scale - start_scale) * t;
                    let proj = axis_n * d;
                    let perp = *v - *center - proj;
                    *v = *center + proj + perp * scale;
                }
            }
            DeformField::Bend { axis, center, angle, region_size } => {
                let axis_n = axis.normalize_or_zero();
                for v in &mut mesh.vertices {
                    let d = (*v - *center).dot(axis_n);
                    let t = (d / region_size.max(1e-6)).clamp(-1.0, 1.0);
                    let bend_angle = t * angle;
                    let (s, c) = bend_angle.sin_cos();
                    let local = *v - *center;
                    let proj = axis_n * local.dot(axis_n);
                    let perp = local - proj;
                    if perp.length_squared() < 1e-10 { continue; }
                    let t1 = perp.normalize();
                    let t2 = axis_n.cross(t1);
                    let r = perp.length();
                    *v = *center + proj + (t1 * c + t2 * s) * r;
                }
            }
            DeformField::ForceFieldDisplace { field, scale, time } => {
                for v in &mut mesh.vertices {
                    let force = field.force_at(*v, 1.0, 0.0, *time);
                    *v += force * *scale;
                }
            }
        }
    }

    /// Apply multiple deformations in sequence.
    pub fn apply_chain(mesh: &mut GeoMesh, deforms: &[DeformField]) {
        for d in deforms {
            Self::apply(mesh, d);
        }
        mesh.recompute_normals();
    }
}

fn simple_hash(x: f32) -> f32 {
    let x = (x * 12.9898).sin() * 43758.5453;
    x.fract() * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn flat_grid() -> GeoMesh {
        let mut mesh = GeoMesh::new();
        for z in 0..4 {
            for x in 0..4 {
                mesh.add_vertex(Vec3::new(x as f32, 0.0, z as f32), Vec3::Y, Vec2::ZERO);
            }
        }
        for z in 0..3 {
            for x in 0..3 {
                let i = (z * 4 + x) as u32;
                mesh.add_triangle(i, i + 1, i + 5);
                mesh.add_triangle(i, i + 5, i + 4);
            }
        }
        mesh
    }

    #[test]
    fn wave_deforms_vertically() {
        let mut mesh = flat_grid();
        let orig_y: Vec<f32> = mesh.vertices.iter().map(|v| v.y).collect();
        Deformer::apply(&mut mesh, &DeformField::Wave {
            direction: Vec3::X, amplitude: 1.0, frequency: 1.0, phase: 0.0,
        });
        let changed = mesh.vertices.iter().zip(orig_y.iter()).any(|(v, &oy)| (v.y - oy).abs() > 0.01);
        assert!(changed, "Wave should displace vertices");
    }

    #[test]
    fn twist_preserves_on_axis() {
        let mut mesh = GeoMesh::new();
        // Point on the twist axis should not move
        mesh.add_vertex(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(0.0, 2.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(1.0, 1.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_triangle(0, 1, 2);

        let orig = mesh.vertices[0];
        Deformer::apply(&mut mesh, &DeformField::Twist {
            axis: Vec3::Y, center: Vec3::ZERO, angle_per_unit: 1.0,
        });
        assert!((mesh.vertices[0] - orig).length() < 0.01, "Point on axis shouldn't move");
    }
}
