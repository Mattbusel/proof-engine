// physics_debug.rs — Physics visualizer for proof-engine editor
// Draws: force fields, constraint axes, cloth mesh wireframe, rigid body AABBs,
// soft body vertex normals, SDF collision shells, IK chain overlays,
// particle velocity vectors, and contact point indicators.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ─── Primitive draw commands ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DebugPrim {
    Line   { start: Vec3, end: Vec3, color: Vec4, width: f32 },
    Arrow  { from: Vec3, to: Vec3, color: Vec4, head_size: f32 },
    Sphere { center: Vec3, radius: f32, color: Vec4, filled: bool },
    Box    { center: Vec3, half_extents: Vec3, rotation: Quat, color: Vec4, filled: bool },
    Cylinder { base: Vec3, top: Vec3, radius: f32, color: Vec4 },
    Cone   { apex: Vec3, base_center: Vec3, radius: f32, color: Vec4 },
    Disc   { center: Vec3, normal: Vec3, radius: f32, color: Vec4 },
    Grid   { center: Vec3, normal: Vec3, size: f32, divisions: u32, color: Vec4 },
    Cross  { center: Vec3, size: f32, color: Vec4 },
    Axes   { transform: Mat4, size: f32 },
    Text   { pos: Vec3, text: String, color: Vec4, scale: f32 },
    Quad   { corners: [Vec3; 4], color: Vec4, filled: bool },
    Circle { center: Vec3, normal: Vec3, radius: f32, segments: u32, color: Vec4 },
    Frustum { view_proj_inv: Mat4, color: Vec4 },
    Bezier { p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, steps: u32, color: Vec4 },
    Point  { pos: Vec3, size: f32, color: Vec4 },
    Torus  { center: Vec3, normal: Vec3, major_r: f32, minor_r: f32, color: Vec4 },
    Spring { start: Vec3, end: Vec3, coils: u32, radius: f32, color: Vec4 },
}

impl DebugPrim {
    pub fn line(start: Vec3, end: Vec3, color: Vec4) -> Self {
        Self::Line { start, end, color, width: 1.5 }
    }
    pub fn thin_line(start: Vec3, end: Vec3, color: Vec4) -> Self {
        Self::Line { start, end, color, width: 1.0 }
    }
    pub fn thick_line(start: Vec3, end: Vec3, color: Vec4) -> Self {
        Self::Line { start, end, color, width: 3.0 }
    }
    pub fn arrow(from: Vec3, to: Vec3, color: Vec4) -> Self {
        Self::Arrow { from, to, color, head_size: 0.05 }
    }
    pub fn wire_sphere(center: Vec3, radius: f32, color: Vec4) -> Self {
        Self::Sphere { center, radius, color, filled: false }
    }
    pub fn solid_sphere(center: Vec3, radius: f32, color: Vec4) -> Self {
        Self::Sphere { center, radius, color, filled: true }
    }
    pub fn wire_box(center: Vec3, half: Vec3, rot: Quat, color: Vec4) -> Self {
        Self::Box { center, half_extents: half, rotation: rot, color, filled: false }
    }
    pub fn aabb(min: Vec3, max: Vec3, color: Vec4) -> Self {
        let center = (min + max) * 0.5;
        let half   = (max - min) * 0.5;
        Self::Box { center, half_extents: half, rotation: Quat::IDENTITY, color, filled: false }
    }
    pub fn axes(transform: Mat4) -> Self {
        Self::Axes { transform, size: 0.2 }
    }
    pub fn point(pos: Vec3, color: Vec4) -> Self {
        Self::Point { pos, size: 6.0, color }
    }
}

/// Expand a DebugPrim into a flat list of line vertices for GPU upload.
/// Returns pairs (start, end) as Vec3 tuples.
impl DebugPrim {
    pub fn to_lines(&self) -> Vec<(Vec3, Vec3, Vec4)> {
        match self {
            Self::Line { start, end, color, .. } =>
                vec![(*start, *end, *color)],

            Self::Arrow { from, to, color, head_size } => {
                let dir = (*to - *from).normalize_or_zero();
                let perp = dir.any_orthogonal_vector().normalize();
                let perp2 = dir.cross(perp).normalize();
                let head_base = *to - dir * (*head_size * 3.0);
                let mut lines = vec![(*from, *to, *color)];
                for i in 0..4u32 {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_2;
                    let offset = perp * angle.cos() * *head_size + perp2 * angle.sin() * *head_size;
                    lines.push((head_base + offset, *to, *color));
                }
                lines
            }

            Self::Sphere { center, radius, color, .. } => {
                let mut lines = Vec::new();
                let segs = 16u32;
                for axis in 0..3 {
                    let prev_angle = 0.0f32;
                    let _ = prev_angle;
                    for i in 0..segs {
                        let a0 = i as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                        let a1 = (i + 1) as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                        let p0 = circle_point(*center, *radius, a0, axis);
                        let p1 = circle_point(*center, *radius, a1, axis);
                        lines.push((p0, p1, *color));
                    }
                }
                lines
            }

            Self::Box { center, half_extents, rotation, color, .. } => {
                let he = *half_extents;
                let corners_local = [
                    Vec3::new(-he.x,-he.y,-he.z), Vec3::new( he.x,-he.y,-he.z),
                    Vec3::new( he.x, he.y,-he.z), Vec3::new(-he.x, he.y,-he.z),
                    Vec3::new(-he.x,-he.y, he.z), Vec3::new( he.x,-he.y, he.z),
                    Vec3::new( he.x, he.y, he.z), Vec3::new(-he.x, he.y, he.z),
                ];
                let c: Vec<Vec3> = corners_local.iter()
                    .map(|&v| *center + rotation.mul_vec3(v))
                    .collect();
                let edges = [
                    (0,1),(1,2),(2,3),(3,0),
                    (4,5),(5,6),(6,7),(7,4),
                    (0,4),(1,5),(2,6),(3,7),
                ];
                edges.iter().map(|&(a,b)| (c[a], c[b], *color)).collect()
            }

            Self::Cylinder { base, top, radius, color } => {
                let mut lines = Vec::new();
                let segs = 12u32;
                let up = (*top - *base).normalize_or_zero();
                let right = up.any_orthogonal_vector().normalize();
                let fwd   = up.cross(right).normalize();
                for i in 0..segs {
                    let a0 = i as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                    let a1 = (i + 1) as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                    let r0 = right * a0.cos() * *radius + fwd * a0.sin() * *radius;
                    let r1 = right * a1.cos() * *radius + fwd * a1.sin() * *radius;
                    lines.push((*base + r0, *base + r1, *color));
                    lines.push((*top  + r0, *top  + r1, *color));
                    if i % 3 == 0 {
                        lines.push((*base + r0, *top + r0, *color));
                    }
                }
                lines
            }

            Self::Disc { center, normal, radius, color } => {
                let mut lines = Vec::new();
                let right = normal.any_orthogonal_vector().normalize();
                let up    = normal.cross(right).normalize();
                let segs  = 16u32;
                for i in 0..segs {
                    let a0 = i as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                    let a1 = (i + 1) as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                    let p0 = *center + right * a0.cos() * *radius + up * a0.sin() * *radius;
                    let p1 = *center + right * a1.cos() * *radius + up * a1.sin() * *radius;
                    lines.push((p0, p1, *color));
                }
                lines
            }

            Self::Circle { center, normal, radius, segments, color } => {
                let mut lines = Vec::new();
                let right = normal.any_orthogonal_vector().normalize();
                let up    = normal.cross(right).normalize();
                let s = *segments;
                for i in 0..s {
                    let a0 = i as f32 / s as f32 * 2.0 * std::f32::consts::PI;
                    let a1 = (i + 1) as f32 / s as f32 * 2.0 * std::f32::consts::PI;
                    let p0 = *center + right * a0.cos() * *radius + up * a0.sin() * *radius;
                    let p1 = *center + right * a1.cos() * *radius + up * a1.sin() * *radius;
                    lines.push((p0, p1, *color));
                }
                lines
            }

            Self::Axes { transform, size } => {
                let o = transform.col(3).truncate();
                let x = o + transform.col(0).truncate().normalize_or_zero() * *size;
                let y = o + transform.col(1).truncate().normalize_or_zero() * *size;
                let z = o + transform.col(2).truncate().normalize_or_zero() * *size;
                vec![
                    (o, x, Vec4::new(1.0, 0.2, 0.2, 1.0)),
                    (o, y, Vec4::new(0.2, 1.0, 0.2, 1.0)),
                    (o, z, Vec4::new(0.2, 0.4, 1.0, 1.0)),
                ]
            }

            Self::Cross { center, size, color } => {
                let h = *size * 0.5;
                vec![
                    (*center - Vec3::X*h, *center + Vec3::X*h, *color),
                    (*center - Vec3::Y*h, *center + Vec3::Y*h, *color),
                    (*center - Vec3::Z*h, *center + Vec3::Z*h, *color),
                ]
            }

            Self::Grid { center, normal, size, divisions, color } => {
                let mut lines = Vec::new();
                let right = normal.any_orthogonal_vector().normalize();
                let fwd   = normal.cross(right).normalize();
                let half  = *size * 0.5;
                let step  = *size / *divisions as f32;
                for i in 0..=*divisions {
                    let t = -half + i as f32 * step;
                    lines.push((
                        *center + right * t - fwd * half,
                        *center + right * t + fwd * half,
                        *color,
                    ));
                    lines.push((
                        *center - right * half + fwd * t,
                        *center + right * half + fwd * t,
                        *color,
                    ));
                }
                lines
            }

            Self::Bezier { p0, p1, p2, p3, steps, color } => {
                let mut lines = Vec::new();
                let mut prev = *p0;
                for i in 1..=*steps {
                    let t = i as f32 / *steps as f32;
                    let u = 1.0 - t;
                    let pt = *p0*(u*u*u) + *p1*(3.0*u*u*t) + *p2*(3.0*u*t*t) + *p3*(t*t*t);
                    lines.push((prev, pt, *color));
                    prev = pt;
                }
                lines
            }

            Self::Spring { start, end, coils, radius, color } => {
                let mut lines = Vec::new();
                let dir   = (*end - *start).normalize_or_zero();
                let right = dir.any_orthogonal_vector().normalize();
                let up    = dir.cross(right).normalize();
                let total = (*end - *start).length();
                let steps = *coils * 16;
                let mut prev = *start;
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let along = t * total;
                    let angle = i as f32 / 16.0 * 2.0 * std::f32::consts::PI;
                    let pos = *start + dir * along
                        + right * angle.cos() * *radius
                        + up    * angle.sin() * *radius;
                    lines.push((prev, pos, *color));
                    prev = pos;
                }
                lines
            }

            Self::Torus { center, normal, major_r, minor_r, color } => {
                let mut lines = Vec::new();
                let right = normal.any_orthogonal_vector().normalize();
                let up    = normal.cross(right).normalize();
                let major_segs = 16u32;
                let minor_segs = 8u32;
                for i in 0..major_segs {
                    let phi0 = i as f32 / major_segs as f32 * 2.0 * std::f32::consts::PI;
                    let phi1 = (i+1) as f32 / major_segs as f32 * 2.0 * std::f32::consts::PI;
                    let mc0 = *center + right * phi0.cos() * *major_r + up * phi0.sin() * *major_r;
                    let mc1 = *center + right * phi1.cos() * *major_r + up * phi1.sin() * *major_r;
                    // Draw minor circle
                    let rad0 = (right * phi0.cos() + up * phi0.sin()).normalize();
                    let _ = (mc0, mc1);
                    for j in 0..minor_segs {
                        let theta0 = j as f32 / minor_segs as f32 * 2.0 * std::f32::consts::PI;
                        let theta1 = (j+1) as f32 / minor_segs as f32 * 2.0 * std::f32::consts::PI;
                        let p0t = mc0 + rad0 * theta0.cos() * *minor_r + *normal * theta0.sin() * *minor_r;
                        let p1t = mc0 + rad0 * theta1.cos() * *minor_r + *normal * theta1.sin() * *minor_r;
                        lines.push((p0t, p1t, *color));
                    }
                }
                lines
            }

            Self::Frustum { view_proj_inv, color } => {
                let ndc_corners = [
                    Vec4::new(-1.0,-1.0,-1.0,1.0), Vec4::new( 1.0,-1.0,-1.0,1.0),
                    Vec4::new( 1.0, 1.0,-1.0,1.0), Vec4::new(-1.0, 1.0,-1.0,1.0),
                    Vec4::new(-1.0,-1.0, 1.0,1.0), Vec4::new( 1.0,-1.0, 1.0,1.0),
                    Vec4::new( 1.0, 1.0, 1.0,1.0), Vec4::new(-1.0, 1.0, 1.0,1.0),
                ];
                let ws: Vec<Vec3> = ndc_corners.iter().map(|&v| {
                    let r = *view_proj_inv * v;
                    r.truncate() / r.w
                }).collect();
                let edges = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)];
                edges.iter().map(|&(a,b)| (ws[a], ws[b], *color)).collect()
            }

            Self::Cone { apex, base_center, radius, color } => {
                let mut lines = Vec::new();
                let axis  = (*apex - *base_center).normalize_or_zero();
                let right = axis.any_orthogonal_vector().normalize();
                let fwd   = axis.cross(right).normalize();
                let segs  = 8u32;
                for i in 0..segs {
                    let a = i as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                    let a1 = (i+1) as f32 / segs as f32 * 2.0 * std::f32::consts::PI;
                    let r0 = *base_center + right * a.cos() * *radius + fwd * a.sin() * *radius;
                    let r1 = *base_center + right * a1.cos() * *radius + fwd * a1.sin() * *radius;
                    lines.push((*apex, r0, *color));
                    lines.push((r0, r1, *color));
                }
                lines
            }

            Self::Quad { corners, color, .. } => {
                vec![
                    (corners[0], corners[1], *color),
                    (corners[1], corners[2], *color),
                    (corners[2], corners[3], *color),
                    (corners[3], corners[0], *color),
                ]
            }

            Self::Point { pos, size, color } => {
                let h = *size * 0.003;
                vec![
                    (*pos - Vec3::X*h, *pos + Vec3::X*h, *color),
                    (*pos - Vec3::Y*h, *pos + Vec3::Y*h, *color),
                    (*pos - Vec3::Z*h, *pos + Vec3::Z*h, *color),
                ]
            }

            Self::Text { .. } => vec![],  // Text handled separately
        }
    }
}

fn circle_point(center: Vec3, radius: f32, angle: f32, axis: usize) -> Vec3 {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => center + Vec3::new(0.0, c * radius, s * radius),
        1 => center + Vec3::new(c * radius, 0.0, s * radius),
        _ => center + Vec3::new(c * radius, s * radius, 0.0),
    }
}

// ─── Physics body types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysBodyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Static,
    Kinematic,
    Dynamic,
    Cloth,
    SoftBody,
    Fluid,
    SdfCollider,
}

#[derive(Debug, Clone)]
pub struct PhysBodyState {
    pub id: PhysBodyId,
    pub body_type: BodyType,
    pub position: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
    pub mass: f32,
    pub asleep: bool,
    pub active: bool,
}

impl PhysBodyState {
    pub fn new(id: PhysBodyId) -> Self {
        Self {
            id,
            body_type: BodyType::Dynamic,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            aabb_min: Vec3::splat(-0.5),
            aabb_max: Vec3::splat( 0.5),
            mass: 1.0,
            asleep: false,
            active: true,
        }
    }
}

// ─── Contact / constraint ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContactPoint {
    pub position: Vec3,
    pub normal: Vec3,
    pub depth: f32,
    pub body_a: PhysBodyId,
    pub body_b: PhysBodyId,
    pub friction_impulse: f32,
    pub normal_impulse: f32,
}

#[derive(Debug, Clone)]
pub struct ConstraintViz {
    pub body_a: Option<PhysBodyId>,
    pub body_b: Option<PhysBodyId>,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub kind: ConstraintKind,
    pub active: bool,
    pub broken: bool,
    pub strain: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    Fixed,
    Hinge,
    BallSocket,
    Slider,
    Spring,
    Distance,
    Cone,
    Gear,
    Ik,
}

impl ConstraintKind {
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Fixed      => Vec4::new(0.8, 0.8, 0.8, 1.0),
            Self::Hinge      => Vec4::new(1.0, 0.6, 0.1, 1.0),
            Self::BallSocket => Vec4::new(0.3, 0.9, 0.5, 1.0),
            Self::Slider     => Vec4::new(0.5, 0.5, 1.0, 1.0),
            Self::Spring     => Vec4::new(1.0, 1.0, 0.2, 1.0),
            Self::Distance   => Vec4::new(0.7, 0.3, 1.0, 1.0),
            Self::Cone       => Vec4::new(1.0, 0.4, 0.4, 1.0),
            Self::Gear       => Vec4::new(0.5, 0.8, 1.0, 1.0),
            Self::Ik         => Vec4::new(0.2, 1.0, 1.0, 1.0),
        }
    }
}

// ─── Force field visualization ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ForceFieldViz {
    pub name: String,
    pub position: Vec3,
    pub radius: f32,
    pub strength: f32,
    pub field_type: ForceFieldType,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceFieldType {
    Gravity,
    Radial,
    Directional,
    Vortex,
    Turbulence,
    Attractor,
    Repulsor,
    Wind,
    Explosion,
    Magnetic,
}

impl ForceFieldType {
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Gravity     => Vec4::new(0.5, 0.5, 1.0, 0.7),
            Self::Radial      => Vec4::new(1.0, 0.5, 0.0, 0.7),
            Self::Directional => Vec4::new(0.2, 0.8, 0.2, 0.7),
            Self::Vortex      => Vec4::new(0.9, 0.2, 0.9, 0.7),
            Self::Turbulence  => Vec4::new(0.8, 0.8, 0.0, 0.7),
            Self::Attractor   => Vec4::new(0.2, 0.8, 1.0, 0.7),
            Self::Repulsor    => Vec4::new(1.0, 0.2, 0.2, 0.7),
            Self::Wind        => Vec4::new(0.7, 0.9, 1.0, 0.7),
            Self::Explosion   => Vec4::new(1.0, 0.6, 0.0, 0.9),
            Self::Magnetic    => Vec4::new(0.6, 0.3, 1.0, 0.7),
        }
    }

    pub fn sample_at(&self, relative_pos: Vec3, strength: f32, radius: f32) -> Vec3 {
        let dist  = relative_pos.length();
        if dist > radius { return Vec3::ZERO; }
        let falloff = (1.0 - dist / radius).powi(2);
        match self {
            Self::Gravity     => Vec3::NEG_Y * strength * falloff,
            Self::Radial      => relative_pos.normalize_or_zero() * strength * falloff,
            Self::Directional => Vec3::Y * strength * falloff,
            Self::Vortex      => {
                let r = Vec3::new(relative_pos.z, 0.0, -relative_pos.x).normalize_or_zero();
                r * strength * falloff
            }
            Self::Attractor   => -relative_pos.normalize_or_zero() * strength * falloff,
            Self::Repulsor    =>  relative_pos.normalize_or_zero() * strength * falloff,
            Self::Wind        => Vec3::X * strength * falloff,
            Self::Turbulence  => Vec3::new(
                Self::hash_noise(relative_pos, 0),
                Self::hash_noise(relative_pos, 1),
                Self::hash_noise(relative_pos, 2),
            ) * strength * falloff,
            Self::Explosion   => relative_pos.normalize_or_zero() * strength * falloff * 5.0,
            Self::Magnetic    => {
                let right = Vec3::X;
                let tan = right.cross(relative_pos.normalize_or_zero()).normalize_or_zero();
                tan * strength * falloff
            }
        }
    }

    fn hash_noise(p: Vec3, seed: u32) -> f32 {
        let xi = ((p.x * 5.0 + seed as f32 * 31.0) as i32) as u32;
        let yi = ((p.y * 5.0 + seed as f32 * 67.0) as i32) as u32;
        let zi = ((p.z * 5.0 + seed as f32 * 113.0) as i32) as u32;
        let h = xi.wrapping_mul(0x9e3779b9).wrapping_add(
                yi.wrapping_mul(0x85ebca6b)).wrapping_add(
                zi.wrapping_mul(0xc2b2ae35));
        (h as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

// ─── Cloth mesh debug ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClothMeshViz {
    pub name: String,
    pub vertices: Vec<Vec3>,
    pub edges: Vec<(usize, usize)>,
    pub pinned: Vec<bool>,
    pub velocities: Vec<Vec3>,
    pub show_velocities: bool,
    pub show_normals: bool,
    pub show_pinned: bool,
    pub color: Vec4,
    pub velocity_scale: f32,
}

impl ClothMeshViz {
    pub fn new(name: String, rows: usize, cols: usize) -> Self {
        let mut verts = Vec::new();
        let mut edges = Vec::new();
        let mut pinned = Vec::new();
        // Build a simple grid mesh
        for r in 0..rows {
            for c in 0..cols {
                let x = c as f32 / (cols-1).max(1) as f32 - 0.5;
                let y = 0.0;
                let z = r as f32 / (rows-1).max(1) as f32 - 0.5;
                verts.push(Vec3::new(x, y, z));
                pinned.push(r == 0);
            }
        }
        // Horizontal
        for r in 0..rows {
            for c in 0..cols-1 {
                edges.push((r*cols+c, r*cols+c+1));
            }
        }
        // Vertical
        for r in 0..rows-1 {
            for c in 0..cols {
                edges.push((r*cols+c, (r+1)*cols+c));
            }
        }
        // Diagonal
        for r in 0..rows-1 {
            for c in 0..cols-1 {
                edges.push((r*cols+c, (r+1)*cols+c+1));
                edges.push((r*cols+c+1, (r+1)*cols+c));
            }
        }
        let velocities = vec![Vec3::ZERO; verts.len()];
        Self {
            name,
            vertices: verts,
            edges,
            pinned,
            velocities,
            show_velocities: false,
            show_normals: false,
            show_pinned: true,
            color: Vec4::new(0.4, 0.7, 1.0, 0.6),
            velocity_scale: 0.1,
        }
    }

    pub fn build_primitives(&self) -> Vec<DebugPrim> {
        let mut prims = Vec::new();
        // Edge lines
        for &(a, b) in &self.edges {
            if a < self.vertices.len() && b < self.vertices.len() {
                prims.push(DebugPrim::line(self.vertices[a], self.vertices[b], self.color));
            }
        }
        // Pinned points
        if self.show_pinned {
            let pin_color = Vec4::new(1.0, 0.3, 0.3, 1.0);
            for (i, &pinned) in self.pinned.iter().enumerate() {
                if pinned && i < self.vertices.len() {
                    prims.push(DebugPrim::Point {
                        pos: self.vertices[i],
                        size: 8.0,
                        color: pin_color,
                    });
                }
            }
        }
        // Velocity vectors
        if self.show_velocities {
            let vel_color = Vec4::new(0.2, 1.0, 0.2, 0.8);
            for (i, &vel) in self.velocities.iter().enumerate() {
                if i < self.vertices.len() && vel.length() > 0.001 {
                    prims.push(DebugPrim::arrow(
                        self.vertices[i],
                        self.vertices[i] + vel * self.velocity_scale,
                        vel_color,
                    ));
                }
            }
        }
        prims
    }
}

// ─── IK chain visualization ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IkChainViz {
    pub name: String,
    pub joints: Vec<Vec3>,
    pub orientations: Vec<Quat>,
    pub lengths: Vec<f32>,
    pub target: Vec3,
    pub target_reached: bool,
    pub iterations: u32,
    pub error: f32,
}

impl IkChainViz {
    pub fn build_primitives(&self) -> Vec<DebugPrim> {
        let mut prims = Vec::new();
        let bone_color  = Vec4::new(0.9, 0.8, 0.2, 1.0);
        let joint_color = Vec4::new(0.3, 0.9, 0.3, 1.0);
        let target_color = if self.target_reached {
            Vec4::new(0.0, 1.0, 0.0, 1.0)
        } else {
            Vec4::new(1.0, 0.4, 0.1, 1.0)
        };

        for i in 0..self.joints.len().saturating_sub(1) {
            prims.push(DebugPrim::thick_line(self.joints[i], self.joints[i+1], bone_color));
            prims.push(DebugPrim::wire_sphere(self.joints[i], 0.02, joint_color));
            // Axes
            let m = Mat4::from_rotation_translation(self.orientations[i], self.joints[i]);
            prims.push(DebugPrim::axes(m));
        }
        if let Some(last) = self.joints.last() {
            prims.push(DebugPrim::wire_sphere(*last, 0.02, joint_color));
        }
        // Target
        prims.push(DebugPrim::Cross { center: self.target, size: 0.1, color: target_color });
        prims.push(DebugPrim::wire_sphere(self.target, 0.03, target_color));
        prims
    }
}

// ─── Particle debug visualizer ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParticleDebugViz {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub life_normalized: Vec<f32>,
    pub show_velocity: bool,
    pub show_life_color: bool,
    pub velocity_scale: f32,
    pub point_size: f32,
    pub max_shown: usize,
}

impl ParticleDebugViz {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            velocities: Vec::new(),
            life_normalized: Vec::new(),
            show_velocity: false,
            show_life_color: true,
            velocity_scale: 0.05,
            point_size: 4.0,
            max_shown: 10000,
        }
    }

    pub fn build_primitives(&self) -> Vec<DebugPrim> {
        let mut prims = Vec::new();
        let count = self.positions.len().min(self.max_shown);
        for i in 0..count {
            let life = self.life_normalized.get(i).copied().unwrap_or(1.0);
            let color = if self.show_life_color {
                lerp_color(
                    Vec4::new(1.0, 0.3, 0.1, 0.8),
                    Vec4::new(0.1, 0.5, 1.0, 0.3),
                    life,
                )
            } else {
                Vec4::new(0.8, 0.8, 0.8, 0.6)
            };
            prims.push(DebugPrim::Point { pos: self.positions[i], size: self.point_size, color });
            if self.show_velocity && i < self.velocities.len() {
                let vel = self.velocities[i];
                if vel.length() > 0.001 {
                    prims.push(DebugPrim::line(
                        self.positions[i],
                        self.positions[i] + vel * self.velocity_scale,
                        Vec4::new(0.4, 1.0, 0.4, 0.5),
                    ));
                }
            }
        }
        prims
    }
}

impl Default for ParticleDebugViz { fn default() -> Self { Self::new() } }

fn lerp_color(a: Vec4, b: Vec4, t: f32) -> Vec4 {
    a + (b - a) * t
}

// ─── Main physics debugger ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PhysicsDebugFlags {
    pub show_bodies: bool,
    pub show_aabbs: bool,
    pub show_velocities: bool,
    pub show_contacts: bool,
    pub show_constraints: bool,
    pub show_force_fields: bool,
    pub show_cloth: bool,
    pub show_ik_chains: bool,
    pub show_particles: bool,
    pub show_sleep_state: bool,
    pub show_mass_centers: bool,
    pub show_inertia_tensors: bool,
    pub show_broadphase: bool,
    pub velocity_scale: f32,
    pub body_alpha: f32,
}

impl Default for PhysicsDebugFlags {
    fn default() -> Self {
        Self {
            show_bodies: true,
            show_aabbs: false,
            show_velocities: false,
            show_contacts: true,
            show_constraints: true,
            show_force_fields: true,
            show_cloth: true,
            show_ik_chains: true,
            show_particles: false,
            show_sleep_state: true,
            show_mass_centers: false,
            show_inertia_tensors: false,
            show_broadphase: false,
            velocity_scale: 0.1,
            body_alpha: 0.5,
        }
    }
}

#[derive(Debug)]
pub struct PhysicsDebugger {
    pub flags: PhysicsDebugFlags,
    pub bodies: HashMap<PhysBodyId, PhysBodyState>,
    pub contacts: Vec<ContactPoint>,
    pub constraints: Vec<ConstraintViz>,
    pub force_fields: Vec<ForceFieldViz>,
    pub cloth_meshes: Vec<ClothMeshViz>,
    pub ik_chains: Vec<IkChainViz>,
    pub particles: ParticleDebugViz,
    pub stats: PhysicsStats,
    prim_buffer: Vec<DebugPrim>,
}

#[derive(Debug, Default, Clone)]
pub struct PhysicsStats {
    pub active_bodies: u32,
    pub sleeping_bodies: u32,
    pub contact_count: u32,
    pub constraint_count: u32,
    pub step_time_ms: f32,
    pub broadphase_pairs: u32,
    pub substeps: u32,
}

impl PhysicsStats {
    pub fn summary(&self) -> String {
        format!(
            "Bodies: {} active / {} sleeping | Contacts: {} | Constraints: {} | Step: {:.2}ms",
            self.active_bodies, self.sleeping_bodies,
            self.contact_count, self.constraint_count,
            self.step_time_ms,
        )
    }
}

impl PhysicsDebugger {
    pub fn new() -> Self {
        Self {
            flags: PhysicsDebugFlags::default(),
            bodies: HashMap::new(),
            contacts: Vec::new(),
            constraints: Vec::new(),
            force_fields: Vec::new(),
            cloth_meshes: Vec::new(),
            ik_chains: Vec::new(),
            particles: ParticleDebugViz::new(),
            stats: PhysicsStats::default(),
            prim_buffer: Vec::new(),
        }
    }

    pub fn register_body(&mut self, body: PhysBodyState) {
        self.bodies.insert(body.id, body);
    }

    pub fn update_body(&mut self, id: PhysBodyId, pos: Vec3, rot: Quat, vel: Vec3) {
        if let Some(b) = self.bodies.get_mut(&id) {
            b.position = pos;
            b.rotation = rot;
            b.linear_velocity = vel;
        }
    }

    pub fn set_contacts(&mut self, contacts: Vec<ContactPoint>) {
        self.contacts = contacts;
        self.stats.contact_count = self.contacts.len() as u32;
    }

    pub fn set_constraints(&mut self, constraints: Vec<ConstraintViz>) {
        self.constraints = constraints;
        self.stats.constraint_count = self.constraints.len() as u32;
    }

    pub fn add_force_field(&mut self, ff: ForceFieldViz) {
        self.force_fields.push(ff);
    }

    pub fn add_cloth(&mut self, cloth: ClothMeshViz) {
        self.cloth_meshes.push(cloth);
    }

    pub fn add_ik_chain(&mut self, chain: IkChainViz) {
        self.ik_chains.push(chain);
    }

    /// Rebuild the debug primitive buffer
    pub fn build_primitives(&mut self) {
        self.prim_buffer.clear();
        let flags = &self.flags;

        // Bodies
        if flags.show_bodies {
            for body in self.bodies.values() {
                if !body.active { continue; }
                let col = if body.asleep && flags.show_sleep_state {
                    Vec4::new(0.4, 0.4, 0.4, flags.body_alpha)
                } else {
                    match body.body_type {
                        BodyType::Static    => Vec4::new(0.4, 0.4, 0.8, flags.body_alpha),
                        BodyType::Kinematic => Vec4::new(0.8, 0.8, 0.2, flags.body_alpha),
                        BodyType::Dynamic   => Vec4::new(0.3, 0.9, 0.3, flags.body_alpha),
                        BodyType::Cloth     => Vec4::new(0.5, 0.7, 1.0, flags.body_alpha),
                        BodyType::SoftBody  => Vec4::new(0.9, 0.5, 0.3, flags.body_alpha),
                        BodyType::Fluid     => Vec4::new(0.2, 0.6, 1.0, flags.body_alpha),
                        BodyType::SdfCollider => Vec4::new(1.0, 0.4, 0.8, flags.body_alpha),
                    }
                };
                let m = Mat4::from_rotation_translation(body.rotation, body.position);
                self.prim_buffer.push(DebugPrim::axes(m));
                self.prim_buffer.push(DebugPrim::wire_sphere(body.position, 0.05, col));

                if flags.show_mass_centers {
                    self.prim_buffer.push(DebugPrim::Cross {
                        center: body.position,
                        size: 0.08,
                        color: Vec4::new(1.0, 1.0, 0.0, 1.0),
                    });
                }
            }
        }

        // AABBs
        if flags.show_aabbs {
            for body in self.bodies.values() {
                let col = Vec4::new(0.6, 0.6, 0.6, 0.3);
                self.prim_buffer.push(DebugPrim::aabb(body.aabb_min, body.aabb_max, col));
            }
        }

        // Velocities
        if flags.show_velocities {
            for body in self.bodies.values() {
                if body.linear_velocity.length() > 0.001 {
                    self.prim_buffer.push(DebugPrim::arrow(
                        body.position,
                        body.position + body.linear_velocity * flags.velocity_scale,
                        Vec4::new(0.1, 1.0, 0.3, 0.9),
                    ));
                }
                if body.angular_velocity.length() > 0.001 {
                    self.prim_buffer.push(DebugPrim::arrow(
                        body.position,
                        body.position + body.angular_velocity.normalize() * 0.1,
                        Vec4::new(1.0, 0.6, 0.1, 0.9),
                    ));
                }
            }
        }

        // Contacts
        if flags.show_contacts {
            for contact in &self.contacts {
                let depth_color = lerp_color(
                    Vec4::new(0.0, 1.0, 0.0, 1.0),
                    Vec4::new(1.0, 0.0, 0.0, 1.0),
                    (contact.depth * 10.0).clamp(0.0, 1.0),
                );
                self.prim_buffer.push(DebugPrim::point(contact.position, depth_color));
                self.prim_buffer.push(DebugPrim::arrow(
                    contact.position,
                    contact.position + contact.normal * 0.1,
                    Vec4::new(0.8, 0.2, 0.8, 0.9),
                ));
            }
        }

        // Constraints
        if flags.show_constraints {
            for c in &self.constraints {
                let col = if c.broken {
                    Vec4::new(1.0, 0.0, 0.0, 0.9)
                } else if c.strain > 0.8 {
                    lerp_color(Vec4::new(0.0,1.0,0.0,1.0), Vec4::new(1.0,0.0,0.0,1.0), c.strain)
                } else {
                    c.kind.color()
                };
                self.prim_buffer.push(DebugPrim::line(c.anchor_a, c.anchor_b, col));
                self.prim_buffer.push(DebugPrim::wire_sphere(c.anchor_a, 0.02, col));
                self.prim_buffer.push(DebugPrim::wire_sphere(c.anchor_b, 0.02, col));

                match c.kind {
                    ConstraintKind::Hinge => {
                        let axis = (c.anchor_b - c.anchor_a).normalize_or_zero();
                        let mid = (c.anchor_a + c.anchor_b) * 0.5;
                        self.prim_buffer.push(DebugPrim::Disc {
                            center: mid, normal: axis, radius: 0.05, color: col,
                        });
                    }
                    ConstraintKind::BallSocket => {
                        self.prim_buffer.push(DebugPrim::wire_sphere(c.anchor_a, 0.06, col));
                    }
                    ConstraintKind::Spring => {
                        self.prim_buffer.push(DebugPrim::Spring {
                            start: c.anchor_a, end: c.anchor_b,
                            coils: 6, radius: 0.02, color: col,
                        });
                    }
                    ConstraintKind::Ik => {
                        self.prim_buffer.push(DebugPrim::Cross {
                            center: c.anchor_b, size: 0.08, color: col,
                        });
                    }
                    _ => {}
                }
            }
        }

        // Force fields
        if flags.show_force_fields {
            for ff in &self.force_fields {
                if !ff.active { continue; }
                let col = ff.field_type.color();
                self.prim_buffer.push(DebugPrim::wire_sphere(ff.position, ff.radius, col));
                // Sample field at grid points
                let grid = 4i32;
                for ix in -grid..=grid {
                    for iz in -grid..=grid {
                        let rel = Vec3::new(ix as f32, 0.0, iz as f32)
                            / grid as f32 * ff.radius * 0.8;
                        let force = ff.field_type.sample_at(rel, ff.strength, ff.radius);
                        if force.length() > 0.001 {
                            let from = ff.position + rel;
                            let scaled = force * 0.05;
                            self.prim_buffer.push(DebugPrim::arrow(from, from + scaled, col));
                        }
                    }
                }
            }
        }

        // Cloth
        if flags.show_cloth {
            for cloth in &self.cloth_meshes {
                let prims = cloth.build_primitives();
                self.prim_buffer.extend(prims);
            }
        }

        // IK chains
        if flags.show_ik_chains {
            for chain in &self.ik_chains {
                let prims = chain.build_primitives();
                self.prim_buffer.extend(prims);
            }
        }

        // Particles
        if flags.show_particles {
            let prims = self.particles.build_primitives();
            self.prim_buffer.extend(prims);
        }
    }

    /// Expand primitives into flat vertex buffer for GPU
    pub fn build_line_buffer(&self) -> Vec<(Vec3, Vec3, Vec4)> {
        self.prim_buffer.iter()
            .flat_map(|p| p.to_lines())
            .collect()
    }

    pub fn primitives(&self) -> &[DebugPrim] {
        &self.prim_buffer
    }

    pub fn clear(&mut self) {
        self.bodies.clear();
        self.contacts.clear();
        self.constraints.clear();
        self.force_fields.clear();
        self.cloth_meshes.clear();
        self.ik_chains.clear();
        self.prim_buffer.clear();
    }
}

impl Default for PhysicsDebugger {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_lines_count() {
        let p = DebugPrim::aabb(Vec3::splat(-1.0), Vec3::splat(1.0), Vec4::ONE);
        assert_eq!(p.to_lines().len(), 12);
    }

    #[test]
    fn axes_emit_three_lines() {
        let p = DebugPrim::axes(Mat4::IDENTITY);
        assert_eq!(p.to_lines().len(), 3);
    }

    #[test]
    fn force_field_attractor_points_inward() {
        let ff = ForceFieldType::Attractor;
        let rel = Vec3::new(1.0, 0.0, 0.0);
        let f = ff.sample_at(rel, 1.0, 2.0);
        assert!(f.x < 0.0, "attractor should pull x negative");
    }

    #[test]
    fn cloth_grid_has_edges() {
        let c = ClothMeshViz::new("test".into(), 4, 4);
        assert!(!c.edges.is_empty());
    }

    #[test]
    fn physics_debugger_build() {
        let mut dbg = PhysicsDebugger::new();
        let mut b = PhysBodyState::new(PhysBodyId(1));
        b.body_type = BodyType::Dynamic;
        dbg.register_body(b);
        dbg.build_primitives();
        assert!(!dbg.prim_buffer.is_empty());
    }
}
