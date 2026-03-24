//! Gizmo renderer — 3D manipulation handles, measurement, annotations,
//! selection outlines, light/collider/frustum/path/vector-field visualisations.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};

// ─────────────────────────────────────────────────────────────────────────────
// Primitive geometry helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A 3D ray used for hit testing.
#[derive(Debug, Clone, Copy)]
pub struct Ray3 {
    pub origin:    Vec3,
    pub direction: Vec3,
}

impl Ray3 {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction: direction.normalize() }
    }

    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// Closest distance from the ray to a point.
    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        let v = p - self.origin;
        let t = v.dot(self.direction).max(0.0);
        (self.origin + self.direction * t - p).length()
    }

    /// Closest t on the ray to a line segment (a, b).
    pub fn closest_t_to_segment(&self, a: Vec3, b: Vec3) -> f32 {
        let ab = b - a;
        let ao = self.origin - a;
        let dab = self.direction.dot(ab);
        let dab2 = ab.dot(ab);
        let cross_len = (self.direction.cross(ab)).length();
        if cross_len < 1e-6 { return 0.0; } // parallel
        let t = ao.cross(ab).dot(self.direction.cross(ab)) / (cross_len * cross_len);
        t.max(0.0)
    }

    /// Distance from ray to a line segment.
    pub fn distance_to_segment(&self, a: Vec3, b: Vec3) -> f32 {
        let t = self.closest_t_to_segment(a, b);
        let closest_on_ray = self.at(t);
        let ab = b - a;
        let t_seg = ((closest_on_ray - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0);
        let closest_on_seg = a + ab * t_seg;
        (closest_on_ray - closest_on_seg).length()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GizmoMode & GizmoSpace
// ─────────────────────────────────────────────────────────────────────────────

/// Which manipulation gizmo is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
    Universal,
    Bounds,
    Custom,
}

impl GizmoMode {
    pub fn label(self) -> &'static str {
        match self {
            GizmoMode::Translate => "Translate",
            GizmoMode::Rotate    => "Rotate",
            GizmoMode::Scale     => "Scale",
            GizmoMode::Universal => "Universal",
            GizmoMode::Bounds    => "Bounds",
            GizmoMode::Custom    => "Custom",
        }
    }

    /// Hotkey that activates this mode.
    pub fn hotkey(self) -> char {
        match self {
            GizmoMode::Translate => 'g',
            GizmoMode::Rotate    => 'r',
            GizmoMode::Scale     => 's',
            GizmoMode::Universal => 'u',
            GizmoMode::Bounds    => 'b',
            GizmoMode::Custom    => 'c',
        }
    }
}

/// In which coordinate system the gizmo operates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GizmoSpace {
    #[default]
    World,
    Local,
}

// ─────────────────────────────────────────────────────────────────────────────
// Axis constants
// ─────────────────────────────────────────────────────────────────────────────

pub const AXIS_X: Vec3 = Vec3::X;
pub const AXIS_Y: Vec3 = Vec3::Y;
pub const AXIS_Z: Vec3 = Vec3::Z;

/// RGBA colours for each axis.
pub fn axis_color(axis: Vec3) -> Vec4 {
    if (axis - AXIS_X).length() < 1e-4 { Vec4::new(1.0, 0.2, 0.2, 1.0) }
    else if (axis - AXIS_Y).length() < 1e-4 { Vec4::new(0.2, 1.0, 0.2, 1.0) }
    else { Vec4::new(0.2, 0.4, 1.0, 1.0) }
}

pub fn axis_hover_color(axis: Vec3) -> Vec4 {
    let c = axis_color(axis);
    Vec4::new(
        (c.x + 0.4).min(1.0),
        (c.y + 0.4).min(1.0),
        (c.z + 0.4).min(1.0),
        1.0,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// TranslateHandle
// ─────────────────────────────────────────────────────────────────────────────

/// A single axis arrow for the translation gizmo.
#[derive(Debug, Clone)]
pub struct TranslateHandle {
    pub axis:       Vec3,
    pub hovered:    bool,
    pub active:     bool,
    pub drag_start: Vec3,  // world-space drag start point
    pub value:      Vec3,  // accumulated translation
    pub length:     f32,   // visual length of the arrow
}

impl TranslateHandle {
    pub fn new(axis: Vec3) -> Self {
        Self {
            axis: axis.normalize(),
            hovered: false,
            active: false,
            drag_start: Vec3::ZERO,
            value: Vec3::ZERO,
            length: 1.0,
        }
    }

    /// Compute the start and end points of the arrow in world space.
    pub fn arrow_segment(&self, origin: Vec3, scale: f32) -> (Vec3, Vec3) {
        let end = origin + self.axis * self.length * scale;
        (origin, end)
    }

    /// Hit-test against the mouse ray. Returns true if hit.
    pub fn hit_test(&self, ray: &Ray3, origin: Vec3, scale: f32, threshold: f32) -> bool {
        let (a, b) = self.arrow_segment(origin, scale);
        ray.distance_to_segment(a, b) < threshold
    }

    pub fn begin_drag(&mut self, world_pos: Vec3) {
        self.active = true;
        self.drag_start = world_pos;
        self.value = Vec3::ZERO;
    }

    pub fn update_drag(&mut self, world_pos: Vec3) {
        let delta = world_pos - self.drag_start;
        self.value = self.axis * self.axis.dot(delta);
    }

    pub fn end_drag(&mut self) -> Vec3 {
        self.active = false;
        std::mem::take(&mut self.value)
    }

    pub fn color(&self) -> Vec4 {
        if self.hovered || self.active { axis_hover_color(self.axis) }
        else { axis_color(self.axis) }
    }

    /// ASCII representation.
    pub fn render_ascii(&self, origin: Vec3) -> String {
        let (a, b) = self.arrow_segment(origin, 1.0);
        let label = if self.axis == AXIS_X { "X" }
                    else if self.axis == AXIS_Y { "Y" }
                    else { "Z" };
        format!("→[{}] ({:.1},{:.1},{:.1})→({:.1},{:.1},{:.1})",
            label, a.x, a.y, a.z, b.x, b.y, b.z)
    }
}

/// Plane constraint handle (XY, YZ, XZ).
#[derive(Debug, Clone)]
pub struct PlaneHandle {
    pub normal:  Vec3,  // plane normal = the locked axis
    pub hovered: bool,
    pub active:  bool,
    pub value:   Vec3,
}

impl PlaneHandle {
    pub fn new(normal: Vec3) -> Self {
        Self { normal: normal.normalize(), hovered: false, active: false, value: Vec3::ZERO }
    }

    /// Returns the two tangent axes of this plane.
    pub fn tangents(&self) -> (Vec3, Vec3) {
        let u = if self.normal.abs().dot(AXIS_Y) < 0.99 {
            self.normal.cross(AXIS_Y).normalize()
        } else {
            self.normal.cross(AXIS_X).normalize()
        };
        let v = self.normal.cross(u).normalize();
        (u, v)
    }

    pub fn color(&self) -> Vec4 {
        let c = axis_color(self.normal);
        Vec4::new(c.x, c.y, c.z, if self.hovered { 0.5 } else { 0.2 })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RotateHandle
// ─────────────────────────────────────────────────────────────────────────────

/// One arc ring for the rotation gizmo.
#[derive(Debug, Clone)]
pub struct RotateHandle {
    pub axis:        Vec3,
    pub hovered:     bool,
    pub active:      bool,
    pub angle_start: f32,
    pub angle_delta: f32,
    pub radius:      f32,
    pub segments:    usize,
}

impl RotateHandle {
    pub fn new(axis: Vec3) -> Self {
        Self {
            axis: axis.normalize(),
            hovered: false,
            active: false,
            angle_start: 0.0,
            angle_delta: 0.0,
            radius: 1.0,
            segments: 32,
        }
    }

    /// Generate arc points around the axis (full circle).
    pub fn arc_points(&self, center: Vec3, scale: f32) -> Vec<Vec3> {
        let r = self.radius * scale;
        let (u, v) = {
            let n = self.axis;
            let u = if n.abs().dot(AXIS_Y) < 0.99 { n.cross(AXIS_Y).normalize() }
                    else { n.cross(AXIS_X).normalize() };
            let v = n.cross(u).normalize();
            (u, v)
        };
        let n = self.segments;
        (0..=n)
            .map(|i| {
                let theta = std::f32::consts::TAU * i as f32 / n as f32;
                center + (u * theta.cos() + v * theta.sin()) * r
            })
            .collect()
    }

    pub fn begin_drag(&mut self, angle: f32) {
        self.active = true;
        self.angle_start = angle;
        self.angle_delta = 0.0;
    }

    pub fn update_drag(&mut self, angle: f32) {
        self.angle_delta = angle - self.angle_start;
    }

    pub fn end_drag(&mut self) -> f32 {
        self.active = false;
        let delta = self.angle_delta;
        self.angle_delta = 0.0;
        delta
    }

    pub fn color(&self) -> Vec4 {
        if self.hovered || self.active { axis_hover_color(self.axis) }
        else { axis_color(self.axis) }
    }

    /// Hit-test: distance from ray to arc circle perimeter.
    pub fn hit_test(&self, ray: &Ray3, center: Vec3, scale: f32, threshold: f32) -> bool {
        let pts = self.arc_points(center, scale);
        for i in 0..pts.len().saturating_sub(1) {
            if ray.distance_to_segment(pts[i], pts[i + 1]) < threshold {
                return true;
            }
        }
        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ScaleHandle
// ─────────────────────────────────────────────────────────────────────────────

/// One per-axis scale cube handle.
#[derive(Debug, Clone)]
pub struct ScaleHandle {
    pub axis:        Vec3,
    pub hovered:     bool,
    pub active:      bool,
    pub scale_start: Vec3,
    pub scale_delta: Vec3,
    pub cube_size:   f32,
}

impl ScaleHandle {
    pub fn new(axis: Vec3) -> Self {
        Self {
            axis: axis.normalize(),
            hovered: false,
            active: false,
            scale_start: Vec3::ONE,
            scale_delta: Vec3::ZERO,
            cube_size: 0.12,
        }
    }

    /// World-space position of the cube endpoint.
    pub fn cube_center(&self, origin: Vec3, scale: f32) -> Vec3 {
        origin + self.axis * scale
    }

    pub fn begin_drag(&mut self, current_scale: Vec3) {
        self.active = true;
        self.scale_start = current_scale;
        self.scale_delta = Vec3::ZERO;
    }

    pub fn update_drag(&mut self, mouse_delta: f32) {
        let delta = self.axis * mouse_delta;
        self.scale_delta = delta;
    }

    pub fn end_drag(&mut self) -> Vec3 {
        self.active = false;
        let delta = self.scale_delta;
        self.scale_delta = Vec3::ZERO;
        delta
    }

    pub fn color(&self) -> Vec4 {
        if self.hovered || self.active { axis_hover_color(self.axis) }
        else { axis_color(self.axis) }
    }

    /// Hit-test: point-sphere approximation for the cube.
    pub fn hit_test(&self, ray: &Ray3, origin: Vec3, scale: f32, threshold: f32) -> bool {
        let center = self.cube_center(origin, scale);
        ray.distance_to_point(center) < threshold
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GizmoRaycast
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a gizmo hit test.
#[derive(Debug, Clone, PartialEq)]
pub enum GizmoHit {
    TranslateAxis(usize),      // 0=X, 1=Y, 2=Z
    TranslatePlane(usize),     // 0=XY, 1=YZ, 2=XZ
    RotateAxis(usize),
    ScaleAxis(usize),
    ScaleUniform,
    None,
}

/// Hit tests all gizmo handles against a mouse ray.
pub struct GizmoRaycast {
    pub threshold: f32,
}

impl GizmoRaycast {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    pub fn test_translate(
        &self,
        ray: &Ray3,
        handles: &[TranslateHandle; 3],
        planes: &[PlaneHandle; 3],
        origin: Vec3,
        scale: f32,
    ) -> GizmoHit {
        for (i, h) in handles.iter().enumerate() {
            if h.hit_test(ray, origin, scale, self.threshold) {
                return GizmoHit::TranslateAxis(i);
            }
        }
        for (i, p) in planes.iter().enumerate() {
            let (u, v) = p.tangents();
            let s = 0.3 * scale;
            let corners = [
                origin + u * s + v * s,
                origin + u * s - v * s,
                origin - u * s - v * s,
                origin - u * s + v * s,
            ];
            for j in 0..4 {
                if ray.distance_to_segment(corners[j], corners[(j + 1) % 4]) < self.threshold {
                    return GizmoHit::TranslatePlane(i);
                }
            }
        }
        GizmoHit::None
    }

    pub fn test_rotate(
        &self,
        ray: &Ray3,
        handles: &[RotateHandle; 3],
        center: Vec3,
        scale: f32,
    ) -> GizmoHit {
        for (i, h) in handles.iter().enumerate() {
            if h.hit_test(ray, center, scale, self.threshold) {
                return GizmoHit::RotateAxis(i);
            }
        }
        GizmoHit::None
    }

    pub fn test_scale(
        &self,
        ray: &Ray3,
        handles: &[ScaleHandle; 3],
        origin: Vec3,
        scale: f32,
    ) -> GizmoHit {
        // Uniform scale — hit near origin.
        if ray.distance_to_point(origin) < self.threshold * 1.5 {
            return GizmoHit::ScaleUniform;
        }
        for (i, h) in handles.iter().enumerate() {
            if h.hit_test(ray, origin, scale, self.threshold) {
                return GizmoHit::ScaleAxis(i);
            }
        }
        GizmoHit::None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Selection outline
// ─────────────────────────────────────────────────────────────────────────────

/// An AABB box for selection outline drawing.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl BoundingBox {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half = size * 0.5;
        Self { min: center - half, max: center + half }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// All 12 edges as (start, end) pairs.
    pub fn edges(&self) -> [(Vec3, Vec3); 12] {
        let (mn, mx) = (self.min, self.max);
        [
            // Bottom face
            (Vec3::new(mn.x, mn.y, mn.z), Vec3::new(mx.x, mn.y, mn.z)),
            (Vec3::new(mx.x, mn.y, mn.z), Vec3::new(mx.x, mn.y, mx.z)),
            (Vec3::new(mx.x, mn.y, mx.z), Vec3::new(mn.x, mn.y, mx.z)),
            (Vec3::new(mn.x, mn.y, mx.z), Vec3::new(mn.x, mn.y, mn.z)),
            // Top face
            (Vec3::new(mn.x, mx.y, mn.z), Vec3::new(mx.x, mx.y, mn.z)),
            (Vec3::new(mx.x, mx.y, mn.z), Vec3::new(mx.x, mx.y, mx.z)),
            (Vec3::new(mx.x, mx.y, mx.z), Vec3::new(mn.x, mx.y, mx.z)),
            (Vec3::new(mn.x, mx.y, mx.z), Vec3::new(mn.x, mx.y, mn.z)),
            // Verticals
            (Vec3::new(mn.x, mn.y, mn.z), Vec3::new(mn.x, mx.y, mn.z)),
            (Vec3::new(mx.x, mn.y, mn.z), Vec3::new(mx.x, mx.y, mn.z)),
            (Vec3::new(mx.x, mn.y, mx.z), Vec3::new(mx.x, mx.y, mx.z)),
            (Vec3::new(mn.x, mn.y, mx.z), Vec3::new(mn.x, mx.y, mx.z)),
        ]
    }

    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn expanded(&self, amount: f32) -> Self {
        Self {
            min: self.min - Vec3::splat(amount),
            max: self.max + Vec3::splat(amount),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AnnotationGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// World-space text label with a leader line.
#[derive(Debug, Clone)]
pub struct AnnotationGizmo {
    pub id:       u32,
    pub text:     String,
    pub world_pos: Vec3,
    pub label_offset: Vec2,  // screen-space offset from projected world_pos
    pub color:    Vec4,
    pub visible:  bool,
    pub line_width: f32,
}

impl AnnotationGizmo {
    pub fn new(id: u32, text: impl Into<String>, pos: Vec3) -> Self {
        Self {
            id,
            text: text.into(),
            world_pos: pos,
            label_offset: Vec2::new(20.0, -20.0),
            color: Vec4::new(1.0, 1.0, 0.0, 1.0),
            visible: true,
            line_width: 1.0,
        }
    }

    pub fn render_ascii(&self) -> String {
        format!(
            "[{}] \"{}\" @ ({:.1},{:.1},{:.1})",
            self.id, self.text, self.world_pos.x, self.world_pos.y, self.world_pos.z
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GridGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// Snap grid dot/line visualisation.
#[derive(Debug, Clone)]
pub struct GridGizmo {
    pub snap_size:    f32,
    pub color:        Vec4,
    pub dot_radius:   f32,
    pub show_lines:   bool,
    pub half_extent:  f32,
    pub y_plane:      f32,
}

impl GridGizmo {
    pub fn new(snap_size: f32) -> Self {
        Self {
            snap_size,
            color: Vec4::new(0.5, 0.5, 1.0, 0.4),
            dot_radius: 0.04,
            show_lines: false,
            half_extent: 20.0,
            y_plane: 0.0,
        }
    }

    /// Generate dot positions within the visible area.
    pub fn dot_positions(&self, camera_pos: Vec3) -> Vec<Vec3> {
        let s = self.snap_size;
        let cx = (camera_pos.x / s).floor() as i32;
        let cz = (camera_pos.z / s).floor() as i32;
        let n  = (self.half_extent / s).ceil() as i32 + 1;
        let mut pts = Vec::new();
        for dx in -n..=n {
            for dz in -n..=n {
                pts.push(Vec3::new(
                    (cx + dx) as f32 * s,
                    self.y_plane,
                    (cz + dz) as f32 * s,
                ));
            }
        }
        pts
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LightGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// Visualises a light source.
#[derive(Debug, Clone)]
pub struct LightGizmo {
    pub position:  Vec3,
    pub direction: Vec3,
    pub range:     f32,
    pub cone_angle: f32,  // degrees, only for spot lights
    pub color:     Vec4,
    pub kind:      LightKind,
    pub selected:  bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind { Point, Directional, Spot, Area }

impl LightGizmo {
    pub fn new_point(pos: Vec3, range: f32) -> Self {
        Self {
            position: pos, direction: Vec3::new(0.0, -1.0, 0.0),
            range, cone_angle: 45.0,
            color: Vec4::new(1.0, 1.0, 0.8, 1.0),
            kind: LightKind::Point, selected: false,
        }
    }

    /// Sphere edges for a point light.
    pub fn sphere_lines(&self, segments: usize) -> Vec<(Vec3, Vec3)> {
        let mut lines = Vec::new();
        let r = self.range;
        let n = segments;
        let tau = std::f32::consts::TAU;
        // Three great circles.
        for plane in 0..3_u8 {
            for i in 0..n {
                let a = tau * i as f32 / n as f32;
                let b = tau * (i + 1) as f32 / n as f32;
                let p = |theta: f32| -> Vec3 {
                    match plane {
                        0 => self.position + Vec3::new(r * theta.cos(), r * theta.sin(), 0.0),
                        1 => self.position + Vec3::new(0.0, r * theta.cos(), r * theta.sin()),
                        _ => self.position + Vec3::new(r * theta.cos(), 0.0, r * theta.sin()),
                    }
                };
                lines.push((p(a), p(b)));
            }
        }
        lines
    }

    /// Cone lines for a spot light.
    pub fn cone_lines(&self, segments: usize) -> Vec<(Vec3, Vec3)> {
        let mut lines = Vec::new();
        let half = self.cone_angle.to_radians() * 0.5;
        let r = self.range * half.tan();
        let tip = self.position;
        let fwd = self.direction.normalize();
        let base_center = tip + fwd * self.range;
        let right = if fwd.abs().dot(Vec3::Y) < 0.99 { fwd.cross(Vec3::Y).normalize() }
                    else { fwd.cross(Vec3::X).normalize() };
        let up = fwd.cross(right).normalize();
        let tau = std::f32::consts::TAU;
        let n = segments;
        let mut prev = base_center + right * r;
        for i in 1..=n {
            let theta = tau * i as f32 / n as f32;
            let curr = base_center + (right * theta.cos() + up * theta.sin()) * r;
            lines.push((prev, curr));
            prev = curr;
        }
        // Add lines from tip to rim.
        for i in 0..4 {
            let theta = tau * i as f32 / 4.0;
            let rim = base_center + (right * theta.cos() + up * theta.sin()) * r;
            lines.push((tip, rim));
        }
        lines
    }

    pub fn render_ascii(&self) -> String {
        format!(
            "Light[{:?}] pos=({:.1},{:.1},{:.1}) range={:.1}",
            self.kind, self.position.x, self.position.y, self.position.z, self.range
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ColliderGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// Shows a physics collider shape.
#[derive(Debug, Clone)]
pub struct ColliderGizmo {
    pub position: Vec3,
    pub rotation: Quat,
    pub shape:    ColliderShape,
    pub color:    Vec4,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub enum ColliderShape {
    Box   { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
    Cylinder { half_height: f32, radius: f32 },
    ConvexHull { points: Vec<Vec3> },
}

impl ColliderGizmo {
    pub fn new_box(pos: Vec3, half_extents: Vec3) -> Self {
        Self {
            position: pos,
            rotation: Quat::IDENTITY,
            shape: ColliderShape::Box { half_extents },
            color: Vec4::new(0.0, 1.0, 0.5, 0.8),
            selected: false,
        }
    }

    pub fn new_sphere(pos: Vec3, radius: f32) -> Self {
        Self {
            position: pos,
            rotation: Quat::IDENTITY,
            shape: ColliderShape::Sphere { radius },
            color: Vec4::new(0.0, 1.0, 0.5, 0.8),
            selected: false,
        }
    }

    pub fn wire_lines(&self, segments: usize) -> Vec<(Vec3, Vec3)> {
        match &self.shape {
            ColliderShape::Box { half_extents } => {
                let bb = BoundingBox::new(
                    self.position - *half_extents,
                    self.position + *half_extents,
                );
                bb.edges().to_vec()
            }
            ColliderShape::Sphere { radius } => {
                let lg = LightGizmo::new_point(self.position, *radius);
                lg.sphere_lines(segments)
            }
            ColliderShape::Capsule { half_height, radius } => {
                let mut lines = Vec::new();
                let r = *radius;
                let h = *half_height;
                let n = segments;
                let tau = std::f32::consts::TAU;
                // Two circles at top and bottom.
                for cap in [-1.0_f32, 1.0] {
                    let cy = self.position.y + cap * h;
                    let mut prev = Vec3::new(self.position.x + r, cy, self.position.z);
                    for i in 1..=n {
                        let theta = tau * i as f32 / n as f32;
                        let curr = Vec3::new(
                            self.position.x + r * theta.cos(),
                            cy,
                            self.position.z + r * theta.sin(),
                        );
                        lines.push((prev, curr));
                        prev = curr;
                    }
                }
                // Side lines.
                for i in 0..4 {
                    let theta = tau * i as f32 / 4.0;
                    let dx = r * theta.cos();
                    let dz = r * theta.sin();
                    let bot = Vec3::new(self.position.x + dx, self.position.y - h, self.position.z + dz);
                    let top = Vec3::new(self.position.x + dx, self.position.y + h, self.position.z + dz);
                    lines.push((bot, top));
                }
                lines
            }
            ColliderShape::Cylinder { half_height, radius } => {
                let mut lines = Vec::new();
                let r = *radius;
                let h = *half_height;
                let n = segments;
                let tau = std::f32::consts::TAU;
                for cap in [-1.0_f32, 1.0] {
                    let cy = self.position.y + cap * h;
                    let mut prev = Vec3::new(self.position.x + r, cy, self.position.z);
                    for i in 1..=n {
                        let theta = tau * i as f32 / n as f32;
                        let curr = Vec3::new(
                            self.position.x + r * theta.cos(), cy,
                            self.position.z + r * theta.sin(),
                        );
                        lines.push((prev, curr));
                        prev = curr;
                    }
                }
                for i in 0..8 {
                    let theta = tau * i as f32 / 8.0;
                    lines.push((
                        Vec3::new(self.position.x + r * theta.cos(), self.position.y - h, self.position.z + r * theta.sin()),
                        Vec3::new(self.position.x + r * theta.cos(), self.position.y + h, self.position.z + r * theta.sin()),
                    ));
                }
                lines
            }
            ColliderShape::ConvexHull { points } => {
                // Just draw lines between consecutive points as an approximation.
                let n = points.len();
                let mut lines = Vec::new();
                for i in 0..n {
                    lines.push((self.position + points[i], self.position + points[(i + 1) % n]));
                }
                lines
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CameraFrustumGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// Preview a camera's frustum in the editor viewport.
#[derive(Debug, Clone)]
pub struct CameraFrustumGizmo {
    pub position:    Vec3,
    pub rotation:    Quat,
    pub fov_degrees: f32,
    pub aspect:      f32,
    pub near:        f32,
    pub far:         f32,
    pub color:       Vec4,
    pub selected:    bool,
}

impl CameraFrustumGizmo {
    pub fn new(pos: Vec3, fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self {
            position: pos, rotation: Quat::IDENTITY,
            fov_degrees: fov, aspect, near, far,
            color: Vec4::new(0.8, 0.8, 0.0, 0.9),
            selected: false,
        }
    }

    /// Returns the 8 corners of the frustum in world space.
    pub fn frustum_corners(&self) -> [Vec3; 8] {
        let half_fov = (self.fov_degrees * 0.5).to_radians();
        let near_h = 2.0 * self.near * half_fov.tan();
        let near_w = near_h * self.aspect;
        let far_h  = 2.0 * self.far  * half_fov.tan();
        let far_w  = far_h  * self.aspect;

        let fwd   = self.rotation * Vec3::new(0.0, 0.0, -1.0);
        let right = self.rotation * Vec3::X;
        let up    = self.rotation * Vec3::Y;

        let nc = self.position + fwd * self.near;
        let fc = self.position + fwd * self.far;

        [
            nc - right * near_w * 0.5 - up * near_h * 0.5,
            nc + right * near_w * 0.5 - up * near_h * 0.5,
            nc + right * near_w * 0.5 + up * near_h * 0.5,
            nc - right * near_w * 0.5 + up * near_h * 0.5,
            fc - right * far_w * 0.5 - up * far_h * 0.5,
            fc + right * far_w * 0.5 - up * far_h * 0.5,
            fc + right * far_w * 0.5 + up * far_h * 0.5,
            fc - right * far_w * 0.5 + up * far_h * 0.5,
        ]
    }

    /// Returns the 12 edges of the frustum.
    pub fn edges(&self) -> Vec<(Vec3, Vec3)> {
        let c = self.frustum_corners();
        vec![
            // Near face
            (c[0], c[1]), (c[1], c[2]), (c[2], c[3]), (c[3], c[0]),
            // Far face
            (c[4], c[5]), (c[5], c[6]), (c[6], c[7]), (c[7], c[4]),
            // Connecting edges
            (c[0], c[4]), (c[1], c[5]), (c[2], c[6]), (c[3], c[7]),
        ]
    }

    pub fn render_ascii(&self) -> String {
        format!(
            "Camera frustum: pos=({:.1},{:.1},{:.1}) fov={:.0}° near={:.2} far={:.0}",
            self.position.x, self.position.y, self.position.z,
            self.fov_degrees, self.near, self.far
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PathGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// Draws a spline path between waypoints.
#[derive(Debug, Clone)]
pub struct PathGizmo {
    pub waypoints:    Vec<Vec3>,
    pub color:        Vec4,
    pub closed:       bool,
    pub show_handles: bool,
    pub segments_per_span: usize,
}

impl PathGizmo {
    pub fn new(waypoints: Vec<Vec3>) -> Self {
        Self {
            waypoints,
            color: Vec4::new(0.2, 0.8, 1.0, 1.0),
            closed: false,
            show_handles: true,
            segments_per_span: 16,
        }
    }

    /// Evaluate a Catmull-Rom spline at parameter t in [0, 1] between p1 and p2.
    fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * (
            p1 * 2.0
            + (p2 - p0) * t
            + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
            + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3
        )
    }

    /// Generate line segments for the spline path.
    pub fn spline_lines(&self) -> Vec<(Vec3, Vec3)> {
        let n = self.waypoints.len();
        if n < 2 { return Vec::new(); }
        let mut lines = Vec::new();
        let count = if self.closed { n } else { n - 1 };
        for i in 0..count {
            let p0 = self.waypoints[(i + n - 1) % n];
            let p1 = self.waypoints[i % n];
            let p2 = self.waypoints[(i + 1) % n];
            let p3 = self.waypoints[(i + 2) % n];
            let segs = self.segments_per_span;
            let mut prev = p1;
            for j in 1..=segs {
                let t = j as f32 / segs as f32;
                let curr = Self::catmull_rom(p0, p1, p2, p3, t);
                lines.push((prev, curr));
                prev = curr;
            }
        }
        lines
    }

    /// Straight-line segments between consecutive waypoints.
    pub fn line_segments(&self) -> Vec<(Vec3, Vec3)> {
        let n = self.waypoints.len();
        if n < 2 { return Vec::new(); }
        let count = if self.closed { n } else { n - 1 };
        (0..count)
            .map(|i| (self.waypoints[i], self.waypoints[(i + 1) % n]))
            .collect()
    }

    pub fn add_waypoint(&mut self, pt: Vec3) {
        self.waypoints.push(pt);
    }

    pub fn remove_waypoint(&mut self, index: usize) {
        if index < self.waypoints.len() {
            self.waypoints.remove(index);
        }
    }

    pub fn total_length(&self) -> f32 {
        self.line_segments().iter().map(|(a, b)| (*b - *a).length()).sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// VectorFieldGizmo
// ─────────────────────────────────────────────────────────────────────────────

/// Samples a force field and draws vectors as arrows.
#[derive(Debug, Clone)]
pub struct VectorFieldGizmo {
    pub grid_origin:    Vec3,
    pub grid_size:      Vec3,
    pub cell_count:     [u32; 3],  // samples per axis
    pub arrow_scale:    f32,
    pub color_min:      Vec4,
    pub color_max:      Vec4,
    pub max_magnitude:  f32,
    pub visible:        bool,
}

impl VectorFieldGizmo {
    pub fn new(origin: Vec3, size: Vec3, cells: [u32; 3]) -> Self {
        Self {
            grid_origin: origin,
            grid_size: size,
            cell_count: cells,
            arrow_scale: 0.5,
            color_min: Vec4::new(0.0, 0.5, 1.0, 0.8),
            color_max: Vec4::new(1.0, 0.3, 0.0, 1.0),
            max_magnitude: 10.0,
            visible: true,
        }
    }

    /// Sample positions for the vector field grid.
    pub fn sample_positions(&self) -> Vec<Vec3> {
        let [nx, ny, nz] = self.cell_count;
        let mut pts = Vec::with_capacity((nx * ny * nz) as usize);
        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let t = Vec3::new(
                        ix as f32 / nx.max(1) as f32,
                        iy as f32 / ny.max(1) as f32,
                        iz as f32 / nz.max(1) as f32,
                    );
                    pts.push(self.grid_origin + t * self.grid_size);
                }
            }
        }
        pts
    }

    /// Given a vector value at a position, produce an arrow (start, end, color).
    pub fn make_arrow(&self, pos: Vec3, vector: Vec3) -> (Vec3, Vec3, Vec4) {
        let magnitude = vector.length();
        let t = (magnitude / self.max_magnitude.max(1e-6)).clamp(0.0, 1.0);
        let color = Vec4::lerp(self.color_min, self.color_max, t);
        let end = pos + vector.normalize_or_zero() * magnitude.min(self.max_magnitude) * self.arrow_scale;
        (pos, end, color)
    }

    /// Generate arrows for a set of (position, vector) samples.
    pub fn arrows_for_samples(
        &self,
        samples: &[(Vec3, Vec3)],
    ) -> Vec<(Vec3, Vec3, Vec4)> {
        if !self.visible { return Vec::new(); }
        samples.iter()
            .map(|(pos, vec)| self.make_arrow(*pos, *vec))
            .collect()
    }

    /// Color interpolated by magnitude.
    pub fn color_for_magnitude(&self, magnitude: f32) -> Vec4 {
        let t = (magnitude / self.max_magnitude.max(1e-6)).clamp(0.0, 1.0);
        Vec4::lerp(self.color_min, self.color_max, t)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MeasurementTool
// ─────────────────────────────────────────────────────────────────────────────

/// Shows the distance between two selected points/entities.
#[derive(Debug, Clone, Default)]
pub struct MeasurementTool {
    pub point_a: Option<Vec3>,
    pub point_b: Option<Vec3>,
    pub visible: bool,
}

impl MeasurementTool {
    pub fn new() -> Self { Self { visible: true, ..Default::default() } }

    pub fn set_a(&mut self, p: Vec3) { self.point_a = Some(p); }
    pub fn set_b(&mut self, p: Vec3) { self.point_b = Some(p); }
    pub fn clear(&mut self) { self.point_a = None; self.point_b = None; }

    pub fn distance(&self) -> Option<f32> {
        Some((self.point_b? - self.point_a?).length())
    }

    pub fn midpoint(&self) -> Option<Vec3> {
        Some((self.point_a? + self.point_b?) * 0.5)
    }

    pub fn render_ascii(&self) -> String {
        match self.distance() {
            Some(d) => format!("Distance: {:.4} units", d),
            None => "Measurement: (click two points)".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GizmoRenderer — top-level struct
// ─────────────────────────────────────────────────────────────────────────────

/// Manages all gizmo state and provides hit-testing and rendering helpers.
pub struct GizmoRenderer {
    pub mode:         GizmoMode,
    pub space:        GizmoSpace,
    pub gizmo_scale:  f32,
    pub visible:      bool,

    // Translation handles: [X, Y, Z]
    pub translate:    [TranslateHandle; 3],
    pub translate_planes: [PlaneHandle; 3], // [XY, YZ, XZ]

    // Rotation handles: [X, Y, Z]
    pub rotate:       [RotateHandle; 3],

    // Scale handles: [X, Y, Z]
    pub scale:        [ScaleHandle; 3],

    pub raycast:      GizmoRaycast,
    pub annotations:  Vec<AnnotationGizmo>,
    pub grid_gizmo:   GridGizmo,
    pub measurement:  MeasurementTool,
    pub lights:       Vec<LightGizmo>,
    pub colliders:    Vec<ColliderGizmo>,
    pub cameras:      Vec<CameraFrustumGizmo>,
    pub paths:        Vec<PathGizmo>,
    pub vector_field: Option<VectorFieldGizmo>,

    pub selection_outlines: Vec<(BoundingBox, Vec4)>,

    // Axis lock state (X/Y/Z keys).
    pub locked_axis: Option<Vec3>,

    next_annotation_id: u32,
}

impl GizmoRenderer {
    pub fn new() -> Self {
        Self {
            mode:        GizmoMode::Translate,
            space:       GizmoSpace::World,
            gizmo_scale: 1.0,
            visible:     true,

            translate: [
                TranslateHandle::new(AXIS_X),
                TranslateHandle::new(AXIS_Y),
                TranslateHandle::new(AXIS_Z),
            ],
            translate_planes: [
                PlaneHandle::new(AXIS_Z), // XY plane (Z is normal)
                PlaneHandle::new(AXIS_X), // YZ plane
                PlaneHandle::new(AXIS_Y), // XZ plane
            ],
            rotate: [
                RotateHandle::new(AXIS_X),
                RotateHandle::new(AXIS_Y),
                RotateHandle::new(AXIS_Z),
            ],
            scale: [
                ScaleHandle::new(AXIS_X),
                ScaleHandle::new(AXIS_Y),
                ScaleHandle::new(AXIS_Z),
            ],

            raycast:    GizmoRaycast::new(0.1),
            annotations: Vec::new(),
            grid_gizmo: GridGizmo::new(0.5),
            measurement: MeasurementTool::new(),
            lights:      Vec::new(),
            colliders:   Vec::new(),
            cameras:     Vec::new(),
            paths:       Vec::new(),
            vector_field: None,

            selection_outlines: Vec::new(),
            locked_axis: None,

            next_annotation_id: 1,
        }
    }

    // ── Mode switching ────────────────────────────────────────────────────────

    pub fn set_mode(&mut self, mode: GizmoMode) {
        self.mode = mode;
        self.locked_axis = None;
    }

    pub fn set_space(&mut self, space: GizmoSpace) {
        self.space = space;
    }

    pub fn handle_hotkey(&mut self, key: char) {
        match key {
            'g' => self.set_mode(GizmoMode::Translate),
            'r' => self.set_mode(GizmoMode::Rotate),
            's' => self.set_mode(GizmoMode::Scale),
            'u' => self.set_mode(GizmoMode::Universal),
            'x' => self.locked_axis = Some(AXIS_X),
            'y' => self.locked_axis = Some(AXIS_Y),
            'z' => self.locked_axis = Some(AXIS_Z),
            _ => {}
        }
    }

    // ── Hit testing ───────────────────────────────────────────────────────────

    pub fn hit_test(&self, ray: &Ray3, origin: Vec3) -> GizmoHit {
        if !self.visible { return GizmoHit::None; }
        match self.mode {
            GizmoMode::Translate | GizmoMode::Universal => {
                self.raycast.test_translate(ray, &self.translate, &self.translate_planes, origin, self.gizmo_scale)
            }
            GizmoMode::Rotate => {
                self.raycast.test_rotate(ray, &self.rotate, origin, self.gizmo_scale)
            }
            GizmoMode::Scale => {
                self.raycast.test_scale(ray, &self.scale, origin, self.gizmo_scale)
            }
            _ => GizmoHit::None,
        }
    }

    // ── Hover updates ─────────────────────────────────────────────────────────

    pub fn update_hover(&mut self, hit: &GizmoHit) {
        // Clear all hover states.
        for h in self.translate.iter_mut() { h.hovered = false; }
        for p in self.translate_planes.iter_mut() { p.hovered = false; }
        for r in self.rotate.iter_mut() { r.hovered = false; }
        for s in self.scale.iter_mut() { s.hovered = false; }
        // Set new hover.
        match hit {
            GizmoHit::TranslateAxis(i) => { self.translate[*i].hovered = true; }
            GizmoHit::TranslatePlane(i) => { self.translate_planes[*i].hovered = true; }
            GizmoHit::RotateAxis(i) => { self.rotate[*i].hovered = true; }
            GizmoHit::ScaleAxis(i) => { self.scale[*i].hovered = true; }
            _ => {}
        }
    }

    // ── Drag ──────────────────────────────────────────────────────────────────

    pub fn begin_drag(&mut self, hit: &GizmoHit, world_pos: Vec3, current_scale: Vec3) {
        match hit {
            GizmoHit::TranslateAxis(i) => self.translate[*i].begin_drag(world_pos),
            GizmoHit::RotateAxis(i) => {
                let angle = world_pos.dot(self.rotate[*i].axis);
                self.rotate[*i].begin_drag(angle);
            }
            GizmoHit::ScaleAxis(i) => self.scale[*i].begin_drag(current_scale),
            _ => {}
        }
    }

    pub fn update_drag(&mut self, hit: &GizmoHit, world_pos: Vec3) {
        match hit {
            GizmoHit::TranslateAxis(i) => self.translate[*i].update_drag(world_pos),
            GizmoHit::RotateAxis(i) => {
                let angle = world_pos.dot(self.rotate[*i].axis);
                self.rotate[*i].update_drag(angle);
            }
            _ => {}
        }
    }

    pub fn end_drag(&mut self, hit: &GizmoHit) -> GizmoDelta {
        match hit {
            GizmoHit::TranslateAxis(i) => {
                let delta = self.translate[*i].end_drag();
                GizmoDelta::Translation(delta)
            }
            GizmoHit::RotateAxis(i) => {
                let delta = self.rotate[*i].end_drag();
                GizmoDelta::Rotation(self.rotate[*i].axis, delta)
            }
            GizmoHit::ScaleAxis(i) => {
                let delta = self.scale[*i].end_drag();
                GizmoDelta::Scale(delta)
            }
            GizmoHit::ScaleUniform => {
                GizmoDelta::ScaleUniform(1.0)
            }
            _ => GizmoDelta::None,
        }
    }

    // ── Selection outlines ────────────────────────────────────────────────────

    pub fn add_selection_outline(&mut self, bounds: BoundingBox, color: Vec4) {
        self.selection_outlines.push((bounds, color));
    }

    pub fn clear_selection_outlines(&mut self) {
        self.selection_outlines.clear();
    }

    // ── Annotations ───────────────────────────────────────────────────────────

    pub fn add_annotation(&mut self, text: impl Into<String>, pos: Vec3) -> u32 {
        let id = self.next_annotation_id;
        self.next_annotation_id += 1;
        self.annotations.push(AnnotationGizmo::new(id, text, pos));
        id
    }

    pub fn remove_annotation(&mut self, id: u32) {
        self.annotations.retain(|a| a.id != id);
    }

    // ── Lights / colliders / cameras ─────────────────────────────────────────

    pub fn add_light(&mut self, gizmo: LightGizmo) { self.lights.push(gizmo); }
    pub fn add_collider(&mut self, gizmo: ColliderGizmo) { self.colliders.push(gizmo); }
    pub fn add_camera_frustum(&mut self, gizmo: CameraFrustumGizmo) { self.cameras.push(gizmo); }
    pub fn add_path(&mut self, gizmo: PathGizmo) { self.paths.push(gizmo); }

    pub fn set_vector_field(&mut self, vf: VectorFieldGizmo) {
        self.vector_field = Some(vf);
    }

    // ── Rendering summary ─────────────────────────────────────────────────────

    /// Render a debug summary as ASCII text.
    pub fn render_ascii(&self, origin: Vec3) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "=== Gizmo [{}] space={:?} scale={:.2} ===\n",
            self.mode.label(), self.space, self.gizmo_scale
        ));
        match self.mode {
            GizmoMode::Translate => {
                for h in &self.translate {
                    out.push_str(&format!("  {}\n", h.render_ascii(origin)));
                }
            }
            GizmoMode::Rotate => {
                for r in &self.rotate {
                    let pts = r.arc_points(origin, self.gizmo_scale);
                    out.push_str(&format!("  Rotate[{:?}] {} arc pts\n", r.axis, pts.len()));
                }
            }
            GizmoMode::Scale => {
                for s in &self.scale {
                    out.push_str(&format!(
                        "  Scale[{:?}] cube @ {:?}\n",
                        s.axis, s.cube_center(origin, self.gizmo_scale)
                    ));
                }
            }
            _ => {}
        }
        if !self.annotations.is_empty() {
            out.push_str("Annotations:\n");
            for a in &self.annotations {
                out.push_str(&format!("  {}\n", a.render_ascii()));
            }
        }
        if !self.selection_outlines.is_empty() {
            out.push_str(&format!("{} selection outline(s)\n", self.selection_outlines.len()));
        }
        if let Some(d) = self.measurement.distance() {
            out.push_str(&format!("  {}\n", self.measurement.render_ascii()));
            let _ = d;
        }
        out
    }
}

impl Default for GizmoRenderer {
    fn default() -> Self { Self::new() }
}

/// The result of a completed drag operation.
#[derive(Debug, Clone, PartialEq)]
pub enum GizmoDelta {
    None,
    Translation(Vec3),
    Rotation(Vec3, f32),  // (axis, radians)
    Scale(Vec3),
    ScaleUniform(f32),
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_handle_hit() {
        let h = TranslateHandle::new(AXIS_X);
        let ray = Ray3::new(Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0));
        // Ray points along -Z from (0,0,1). Arrow is along +X. Should miss.
        assert!(!h.hit_test(&ray, Vec3::ZERO, 1.0, 0.1));
    }

    #[test]
    fn test_translate_handle_drag() {
        let mut h = TranslateHandle::new(AXIS_X);
        h.begin_drag(Vec3::ZERO);
        h.update_drag(Vec3::new(2.0, 0.5, 0.3));
        // Only X component should be projected.
        assert!((h.value.x - 2.0).abs() < 1e-5);
        assert!(h.value.y.abs() < 1e-5);
    }

    #[test]
    fn test_rotate_arc_points() {
        let r = RotateHandle::new(AXIS_Y);
        let pts = r.arc_points(Vec3::ZERO, 1.0);
        assert_eq!(pts.len(), r.segments + 1);
        for p in &pts {
            assert!((p.length() - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_scale_handle_hit_uniform() {
        let gizmo = GizmoRenderer::new();
        let ray = Ray3::new(Vec3::new(0.0, 0.0, 0.1), Vec3::new(0.0, 0.0, -1.0));
        let hit = gizmo.raycast.test_scale(&gizmo.scale, ray, Vec3::ZERO, 1.0);
        // A ray aimed almost directly at origin should hit ScaleUniform.
        assert_eq!(hit, GizmoHit::ScaleUniform);
    }

    #[test]
    fn test_bounding_box_edges() {
        let bb = BoundingBox::new(Vec3::ZERO, Vec3::ONE);
        let edges = bb.edges();
        assert_eq!(edges.len(), 12);
    }

    #[test]
    fn test_bounding_box_contains() {
        let bb = BoundingBox::new(Vec3::ZERO, Vec3::ONE);
        assert!(bb.contains_point(Vec3::new(0.5, 0.5, 0.5)));
        assert!(!bb.contains_point(Vec3::new(1.5, 0.5, 0.5)));
    }

    #[test]
    fn test_frustum_corners() {
        let fg = CameraFrustumGizmo::new(Vec3::ZERO, 60.0, 16.0 / 9.0, 0.1, 100.0);
        let corners = fg.frustum_corners();
        assert_eq!(corners.len(), 8);
        // Near corners should be closer to origin than far corners.
        let near_dist = corners[0].length();
        let far_dist  = corners[4].length();
        assert!(far_dist > near_dist);
    }

    #[test]
    fn test_path_gizmo_length() {
        let path = PathGizmo::new(vec![
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
        ]);
        let len = path.total_length();
        assert!((len - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_path_spline_lines() {
        let path = PathGizmo::new(vec![
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
        ]);
        let lines = path.spline_lines();
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_vector_field_samples() {
        let vf = VectorFieldGizmo::new(Vec3::ZERO, Vec3::ONE * 10.0, [4, 4, 4]);
        let pts = vf.sample_positions();
        assert_eq!(pts.len(), 64);
    }

    #[test]
    fn test_vector_field_arrow_color() {
        let vf = VectorFieldGizmo::new(Vec3::ZERO, Vec3::ONE, [2, 2, 2]);
        let c0 = vf.color_for_magnitude(0.0);
        let c1 = vf.color_for_magnitude(vf.max_magnitude);
        assert!((c0 - vf.color_min).length() < 1e-4);
        assert!((c1 - vf.color_max).length() < 1e-4);
    }

    #[test]
    fn test_light_gizmo_sphere_lines() {
        let lg = LightGizmo::new_point(Vec3::ZERO, 5.0);
        let lines = lg.sphere_lines(16);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_measurement_tool() {
        let mut m = MeasurementTool::new();
        m.set_a(Vec3::ZERO);
        m.set_b(Vec3::new(3.0, 4.0, 0.0));
        let d = m.distance().unwrap();
        assert!((d - 5.0).abs() < 1e-5);
        let mid = m.midpoint().unwrap();
        assert!((mid.x - 1.5).abs() < 1e-5);
    }

    #[test]
    fn test_gizmo_mode_hotkey() {
        let mut g = GizmoRenderer::new();
        g.handle_hotkey('r');
        assert_eq!(g.mode, GizmoMode::Rotate);
        g.handle_hotkey('g');
        assert_eq!(g.mode, GizmoMode::Translate);
        g.handle_hotkey('x');
        assert_eq!(g.locked_axis, Some(AXIS_X));
    }

    #[test]
    fn test_ray_distance_to_point() {
        let ray = Ray3::new(Vec3::ZERO, Vec3::X);
        let d = ray.distance_to_point(Vec3::new(0.0, 1.0, 0.0));
        assert!((d - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_annotation_add_remove() {
        let mut g = GizmoRenderer::new();
        let id = g.add_annotation("label", Vec3::ONE);
        assert_eq!(g.annotations.len(), 1);
        g.remove_annotation(id);
        assert!(g.annotations.is_empty());
    }

    #[test]
    fn test_collider_box_edges() {
        let c = ColliderGizmo::new_box(Vec3::ZERO, Vec3::ONE);
        let lines = c.wire_lines(16);
        assert_eq!(lines.len(), 12);
    }
}
