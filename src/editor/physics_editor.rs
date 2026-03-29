#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// PHYSICAL CONSTANTS
// ============================================================

pub const GRAVITY: f32 = 9.80665;
pub const PI: f32 = std::f32::consts::PI;
pub const TWO_PI: f32 = 2.0 * PI;
pub const HALF_PI: f32 = PI * 0.5;
pub const DEG_TO_RAD: f32 = PI / 180.0;
pub const RAD_TO_DEG: f32 = 180.0 / PI;
pub const EPSILON: f32 = 1e-6;
pub const SLEEP_LINEAR_THRESHOLD: f32 = 0.01;
pub const SLEEP_ANGULAR_THRESHOLD: f32 = 0.01;
pub const DEFAULT_FRICTION: f32 = 0.5;
pub const DEFAULT_RESTITUTION: f32 = 0.3;
pub const AIR_DENSITY: f32 = 1.225; // kg/m^3 at sea level
pub const WATER_DENSITY: f32 = 1000.0;
pub const STEEL_DENSITY: f32 = 7850.0;
pub const WOOD_DENSITY: f32 = 700.0;
pub const CONCRETE_DENSITY: f32 = 2400.0;
pub const RUBBER_DENSITY: f32 = 1500.0;
pub const GLASS_DENSITY: f32 = 2500.0;
pub const ALUMINUM_DENSITY: f32 = 2700.0;
pub const COPPER_DENSITY: f32 = 8960.0;
pub const GOLD_DENSITY: f32 = 19300.0;
pub const ICE_DENSITY: f32 = 917.0;
pub const SAND_DENSITY: f32 = 1600.0;

// ============================================================
// INERTIA TENSOR COMPUTATION
// ============================================================

/// Compute inertia tensor for a solid box (principal moments)
/// Ixx = (1/12)*m*(h^2 + d^2), Iyy = (1/12)*m*(w^2 + d^2), Izz = (1/12)*m*(w^2 + h^2)
pub fn inertia_tensor_box(mass: f32, half_extents: Vec3) -> Mat4 {
    let w = 2.0 * half_extents.x;
    let h = 2.0 * half_extents.y;
    let d = 2.0 * half_extents.z;
    let ixx = (1.0 / 12.0) * mass * (h * h + d * d);
    let iyy = (1.0 / 12.0) * mass * (w * w + d * d);
    let izz = (1.0 / 12.0) * mass * (w * w + h * h);
    Mat4::from_cols(
        Vec4::new(ixx, 0.0, 0.0, 0.0),
        Vec4::new(0.0, iyy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, izz, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Compute inertia tensor for a solid sphere
/// I = (2/5)*m*r^2 (all principal moments equal)
pub fn inertia_tensor_sphere(mass: f32, radius: f32) -> Mat4 {
    let i = (2.0 / 5.0) * mass * radius * radius;
    Mat4::from_cols(
        Vec4::new(i, 0.0, 0.0, 0.0),
        Vec4::new(0.0, i, 0.0, 0.0),
        Vec4::new(0.0, 0.0, i, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Compute inertia tensor for a capsule (cylinder + two hemispheres)
/// Capsule aligned along Y axis
pub fn inertia_tensor_capsule(mass: f32, radius: f32, half_height: f32) -> Mat4 {
    let h = 2.0 * half_height;
    let r = radius;
    // Volume of cylinder
    let vol_cyl = PI * r * r * h;
    // Volume of sphere (two hemispheres)
    let vol_sph = (4.0 / 3.0) * PI * r * r * r;
    let total_vol = vol_cyl + vol_sph;
    let m_cyl = mass * vol_cyl / total_vol;
    let m_sph = mass * vol_sph / total_vol;
    // Cylinder inertia
    let iyy_cyl = 0.5 * m_cyl * r * r;
    let ixx_cyl = (1.0 / 12.0) * m_cyl * (3.0 * r * r + h * h);
    // Sphere inertia + parallel axis theorem for hemisphere offsets
    let i_sph_local = (2.0 / 5.0) * m_sph * r * r;
    // Each hemisphere COM is at 3r/8 from flat face
    let d = half_height + 3.0 * r / 8.0;
    let ixx_sph = i_sph_local + m_sph * d * d;
    let iyy_sph = i_sph_local;
    let ixx = ixx_cyl + ixx_sph;
    let iyy = iyy_cyl + iyy_sph;
    let izz = ixx; // symmetry
    Mat4::from_cols(
        Vec4::new(ixx, 0.0, 0.0, 0.0),
        Vec4::new(0.0, iyy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, izz, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Compute inertia tensor for a solid cylinder aligned along Y axis
/// Ixx = Izz = (1/12)*m*(3*r^2 + h^2), Iyy = (1/2)*m*r^2
pub fn inertia_tensor_cylinder(mass: f32, radius: f32, half_height: f32) -> Mat4 {
    let h = 2.0 * half_height;
    let r = radius;
    let iyy = 0.5 * mass * r * r;
    let ixx = (1.0 / 12.0) * mass * (3.0 * r * r + h * h);
    let izz = ixx;
    Mat4::from_cols(
        Vec4::new(ixx, 0.0, 0.0, 0.0),
        Vec4::new(0.0, iyy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, izz, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Compute inertia tensor for a solid cone aligned along Y axis
/// Ixx = Izz = (3/80)*m*(4*r^2 + h^2), Iyy = (3/10)*m*r^2
pub fn inertia_tensor_cone(mass: f32, radius: f32, height: f32) -> Mat4 {
    let r = radius;
    let h = height;
    let iyy = (3.0 / 10.0) * mass * r * r;
    let ixx = (3.0 / 80.0) * mass * (4.0 * r * r + h * h);
    let izz = ixx;
    Mat4::from_cols(
        Vec4::new(ixx, 0.0, 0.0, 0.0),
        Vec4::new(0.0, iyy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, izz, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Parallel axis theorem: shift inertia tensor by displacement d
/// I_new = I_cm + m*(|d|^2*I3 - d*d^T)
pub fn inertia_parallel_axis(i_cm: Mat4, mass: f32, displacement: Vec3) -> Mat4 {
    let d = displacement;
    let d2 = d.dot(d);
    // Off-diagonal products
    let dxx = d.x * d.x;
    let dyy = d.y * d.y;
    let dzz = d.z * d.z;
    let dxy = d.x * d.y;
    let dxz = d.x * d.z;
    let dyz = d.y * d.z;
    // Shift tensor (3x3 portion only)
    let shift = Mat4::from_cols(
        Vec4::new(mass * (d2 - dxx), -mass * dxy, -mass * dxz, 0.0),
        Vec4::new(-mass * dxy, mass * (d2 - dyy), -mass * dyz, 0.0),
        Vec4::new(-mass * dxz, -mass * dyz, mass * (d2 - dzz), 0.0),
        Vec4::new(0.0, 0.0, 0.0, 0.0),
    );
    // Add matrices
    let c0 = i_cm.col(0) + shift.col(0);
    let c1 = i_cm.col(1) + shift.col(1);
    let c2 = i_cm.col(2) + shift.col(2);
    let c3 = i_cm.col(3);
    Mat4::from_cols(c0, c1, c2, c3)
}

// ============================================================
// RIGID BODY DATA
// ============================================================

#[derive(Debug, Clone)]
pub struct RigidBodyInspector {
    pub id: u64,
    pub name: String,
    pub mass: f32,
    pub inertia_tensor: Mat4,
    pub center_of_mass: Vec3,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub linear_sleep_threshold: f32,
    pub angular_sleep_threshold: f32,
    pub is_kinematic: bool,
    pub is_static: bool,
    pub use_gravity: bool,
    pub gravity_scale: f32,
    pub collision_group: u32,
    pub collision_mask: u32,
    pub ccd_enabled: bool,
    pub max_linear_velocity: f32,
    pub max_angular_velocity: f32,
    pub shape_type: RigidBodyShapeType,
    pub shape_params: ShapeParameters,
    pub position: Vec3,
    pub orientation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub force_accumulator: Vec3,
    pub torque_accumulator: Vec3,
    pub sleeping: bool,
    pub sleep_timer: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RigidBodyShapeType {
    Box,
    Sphere,
    Capsule,
    Cylinder,
    Cone,
    ConvexHull,
    TriangleMesh,
    Heightfield,
    Compound,
}

#[derive(Debug, Clone)]
pub struct ShapeParameters {
    pub half_extents: Vec3,
    pub radius: f32,
    pub half_height: f32,
    pub height: f32,
}

impl Default for ShapeParameters {
    fn default() -> Self {
        Self {
            half_extents: Vec3::splat(0.5),
            radius: 0.5,
            half_height: 1.0,
            height: 2.0,
        }
    }
}

impl RigidBodyInspector {
    pub fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            mass: 1.0,
            inertia_tensor: Mat4::IDENTITY,
            center_of_mass: Vec3::ZERO,
            linear_damping: 0.01,
            angular_damping: 0.05,
            linear_sleep_threshold: SLEEP_LINEAR_THRESHOLD,
            angular_sleep_threshold: SLEEP_ANGULAR_THRESHOLD,
            is_kinematic: false,
            is_static: false,
            use_gravity: true,
            gravity_scale: 1.0,
            collision_group: 1,
            collision_mask: 0xFFFF_FFFF,
            ccd_enabled: false,
            max_linear_velocity: 500.0,
            max_angular_velocity: 50.0,
            shape_type: RigidBodyShapeType::Box,
            shape_params: ShapeParameters::default(),
            position: Vec3::ZERO,
            orientation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            force_accumulator: Vec3::ZERO,
            torque_accumulator: Vec3::ZERO,
            sleeping: false,
            sleep_timer: 0.0,
        }
    }

    /// Recompute inertia tensor based on current shape type and parameters
    pub fn recompute_inertia(&mut self) {
        self.inertia_tensor = match self.shape_type {
            RigidBodyShapeType::Box => {
                inertia_tensor_box(self.mass, self.shape_params.half_extents)
            }
            RigidBodyShapeType::Sphere => {
                inertia_tensor_sphere(self.mass, self.shape_params.radius)
            }
            RigidBodyShapeType::Capsule => {
                inertia_tensor_capsule(self.mass, self.shape_params.radius, self.shape_params.half_height)
            }
            RigidBodyShapeType::Cylinder => {
                inertia_tensor_cylinder(self.mass, self.shape_params.radius, self.shape_params.half_height)
            }
            RigidBodyShapeType::Cone => {
                inertia_tensor_cone(self.mass, self.shape_params.radius, self.shape_params.height)
            }
            _ => Mat4::IDENTITY,
        };
    }

    /// Integrate linear/angular velocity over dt using symplectic Euler
    pub fn integrate(&mut self, dt: f32) {
        if self.is_static || self.is_kinematic {
            return;
        }
        if self.sleeping {
            self.force_accumulator = Vec3::ZERO;
            self.torque_accumulator = Vec3::ZERO;
            return;
        }
        let inv_mass = if self.mass > EPSILON { 1.0 / self.mass } else { 0.0 };
        let gravity_force = if self.use_gravity {
            Vec3::new(0.0, -GRAVITY * self.gravity_scale * self.mass, 0.0)
        } else {
            Vec3::ZERO
        };
        let total_force = self.force_accumulator + gravity_force;
        // Linear integration
        let lin_accel = total_force * inv_mass;
        self.linear_velocity += lin_accel * dt;
        // Linear damping (exponential decay)
        let lin_damp_factor = (1.0 - self.linear_damping * dt).max(0.0);
        self.linear_velocity *= lin_damp_factor;
        // Clamp linear velocity
        let lv_len = self.linear_velocity.length();
        if lv_len > self.max_linear_velocity {
            self.linear_velocity *= self.max_linear_velocity / lv_len;
        }
        self.position += self.linear_velocity * dt;
        // Angular integration — simple Euler in body space
        let inv_inertia_diag = Vec3::new(
            if self.inertia_tensor.col(0).x > EPSILON { 1.0 / self.inertia_tensor.col(0).x } else { 0.0 },
            if self.inertia_tensor.col(1).y > EPSILON { 1.0 / self.inertia_tensor.col(1).y } else { 0.0 },
            if self.inertia_tensor.col(2).z > EPSILON { 1.0 / self.inertia_tensor.col(2).z } else { 0.0 },
        );
        let ang_accel = Vec3::new(
            self.torque_accumulator.x * inv_inertia_diag.x,
            self.torque_accumulator.y * inv_inertia_diag.y,
            self.torque_accumulator.z * inv_inertia_diag.z,
        );
        self.angular_velocity += ang_accel * dt;
        let ang_damp_factor = (1.0 - self.angular_damping * dt).max(0.0);
        self.angular_velocity *= ang_damp_factor;
        let av_len = self.angular_velocity.length();
        if av_len > self.max_angular_velocity {
            self.angular_velocity *= self.max_angular_velocity / av_len;
        }
        // Update orientation quaternion: dq/dt = 0.5 * omega_quat * q
        let omega = self.angular_velocity;
        let dq = Quat::from_xyzw(omega.x * 0.5, omega.y * 0.5, omega.z * 0.5, 0.0);
        let q = self.orientation;
        // Quaternion multiplication dq * q gives delta rotation
        let nq = Quat::from_xyzw(
            dq.w * q.x + dq.x * q.w + dq.y * q.z - dq.z * q.y,
            dq.w * q.y - dq.x * q.z + dq.y * q.w + dq.z * q.x,
            dq.w * q.z + dq.x * q.y - dq.y * q.x + dq.z * q.w,
            dq.w * q.w - dq.x * q.x - dq.y * q.y - dq.z * q.z,
        );
        self.orientation = Quat::from_xyzw(
            q.x + nq.x * dt,
            q.y + nq.y * dt,
            q.z + nq.z * dt,
            q.w + nq.w * dt,
        ).normalize();
        // Check sleep
        let v2 = self.linear_velocity.length_squared();
        let w2 = self.angular_velocity.length_squared();
        let lt = self.linear_sleep_threshold * self.linear_sleep_threshold;
        let at = self.angular_sleep_threshold * self.angular_sleep_threshold;
        if v2 < lt && w2 < at {
            self.sleep_timer += dt;
            if self.sleep_timer > 0.5 {
                self.sleeping = true;
            }
        } else {
            self.sleep_timer = 0.0;
            self.sleeping = false;
        }
        // Clear accumulators
        self.force_accumulator = Vec3::ZERO;
        self.torque_accumulator = Vec3::ZERO;
    }

    pub fn apply_force(&mut self, force: Vec3) {
        self.force_accumulator += force;
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        self.torque_accumulator += torque;
    }

    pub fn apply_force_at_point(&mut self, force: Vec3, world_point: Vec3) {
        self.force_accumulator += force;
        let r = world_point - (self.position + self.orientation * self.center_of_mass);
        self.torque_accumulator += r.cross(force);
    }

    pub fn wake_up(&mut self) {
        self.sleeping = false;
        self.sleep_timer = 0.0;
    }
}

// ============================================================
// CONSTRAINT TYPES — 15+ with Jacobian helpers
// ============================================================

#[derive(Debug, Clone)]
pub struct JacobianRow {
    /// Linear component for body A
    pub j_lin_a: Vec3,
    /// Angular component for body A
    pub j_ang_a: Vec3,
    /// Linear component for body B
    pub j_lin_b: Vec3,
    /// Angular component for body B
    pub j_ang_b: Vec3,
    /// Right-hand side bias (Baumgarte stabilization)
    pub bias: f32,
    /// Effective mass (diagonal element of J * M^-1 * J^T)
    pub effective_mass: f32,
    /// Accumulated impulse (warm starting)
    pub lambda: f32,
    /// Lower limit on lambda
    pub lambda_min: f32,
    /// Upper limit on lambda
    pub lambda_max: f32,
}

impl JacobianRow {
    pub fn new() -> Self {
        Self {
            j_lin_a: Vec3::ZERO,
            j_ang_a: Vec3::ZERO,
            j_lin_b: Vec3::ZERO,
            j_ang_b: Vec3::ZERO,
            bias: 0.0,
            effective_mass: 1.0,
            lambda: 0.0,
            lambda_min: f32::NEG_INFINITY,
            lambda_max: f32::INFINITY,
        }
    }

    /// Compute effective mass given inverse masses and inverse inertia tensors
    pub fn compute_effective_mass(&mut self,
        inv_mass_a: f32, inv_inertia_a: Vec3,
        inv_mass_b: f32, inv_inertia_b: Vec3) {
        let ka = inv_mass_a * self.j_lin_a.dot(self.j_lin_a)
            + (inv_inertia_a * self.j_ang_a).dot(self.j_ang_a);
        let kb = inv_mass_b * self.j_lin_b.dot(self.j_lin_b)
            + (inv_inertia_b * self.j_ang_b).dot(self.j_ang_b);
        let k = ka + kb;
        self.effective_mass = if k.abs() > EPSILON { 1.0 / k } else { 0.0 };
    }

    /// Solve one PGS iteration; returns delta lambda
    pub fn solve_velocity(&mut self,
        vel_a: Vec3, omega_a: Vec3,
        vel_b: Vec3, omega_b: Vec3) -> f32 {
        let jv = self.j_lin_a.dot(vel_a)
            + self.j_ang_a.dot(omega_a)
            + self.j_lin_b.dot(vel_b)
            + self.j_ang_b.dot(omega_b);
        let delta = self.effective_mass * (-jv - self.bias);
        let old_lambda = self.lambda;
        self.lambda = (self.lambda + delta).clamp(self.lambda_min, self.lambda_max);
        self.lambda - old_lambda
    }
}

// ---- Fixed Constraint ----

#[derive(Debug, Clone)]
pub struct FixedConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub initial_rotation_ab: Quat,
    pub breaking_force: f32,
    pub breaking_torque: f32,
    pub rows: Vec<JacobianRow>,
}

impl FixedConstraint {
    pub fn new(body_a: u64, body_b: u64, anchor_a: Vec3, anchor_b: Vec3) -> Self {
        let mut rows = Vec::new();
        for _ in 0..6 { rows.push(JacobianRow::new()); }
        Self {
            body_a, body_b, anchor_a, anchor_b,
            initial_rotation_ab: Quat::IDENTITY,
            breaking_force: f32::INFINITY,
            breaking_torque: f32::INFINITY,
            rows,
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let world_a = pos_a + ra;
        let world_b = pos_b + rb;
        let err = world_b - world_a;
        let bias_factor = baumgarte / dt;
        // 3 linear rows
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -ax;
            row.j_ang_a = -ra.cross(ax);
            row.j_lin_b = ax;
            row.j_ang_b = rb.cross(ax);
            row.bias = bias_factor * err.dot(ax);
        }
        // 3 angular rows (lock rotation difference)
        let rot_diff = rot_b * self.initial_rotation_ab.inverse() * rot_a.inverse();
        let (axis_err, angle_err) = quat_to_axis_angle(rot_diff);
        let ang_err = axis_err * angle_err;
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[3 + i];
            row.j_lin_a = Vec3::ZERO;
            row.j_ang_a = -ax;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_b = ax;
            row.bias = bias_factor * ang_err.dot(ax);
        }
    }
}

// ---- Hinge Constraint ----

#[derive(Debug, Clone)]
pub struct HingeConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub axis_a: Vec3, // hinge axis in body A local space
    pub axis_b: Vec3,
    pub lower_limit: f32,
    pub upper_limit: f32,
    pub enable_limits: bool,
    pub motor_enabled: bool,
    pub motor_target_velocity: f32,
    pub motor_max_impulse: f32,
    pub rows: Vec<JacobianRow>,
}

impl HingeConstraint {
    pub fn new(body_a: u64, body_b: u64, anchor: Vec3, axis: Vec3) -> Self {
        let mut rows = Vec::new();
        for _ in 0..7 { rows.push(JacobianRow::new()); }
        Self {
            body_a, body_b,
            anchor_a: anchor,
            anchor_b: anchor,
            axis_a: axis.normalize(),
            axis_b: axis.normalize(),
            lower_limit: -PI,
            upper_limit: PI,
            enable_limits: false,
            motor_enabled: false,
            motor_target_velocity: 0.0,
            motor_max_impulse: 10.0,
            rows,
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let world_a = pos_a + ra;
        let world_b = pos_b + rb;
        let err = world_b - world_a;
        let bias_factor = baumgarte / dt;
        // 3 linear rows (ball-socket part)
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -ax;
            row.j_ang_a = -(ra.cross(ax));
            row.j_lin_b = ax;
            row.j_ang_b = rb.cross(ax);
            row.bias = bias_factor * err.dot(ax);
        }
        // 2 angular rows to constrain perpendicular rotations
        let hinge_world_a = rot_a * self.axis_a;
        let hinge_world_b = rot_b * self.axis_b;
        // Build two vectors perpendicular to hinge axis
        let perp1 = perpendicular_to(hinge_world_a);
        let perp2 = hinge_world_a.cross(perp1).normalize();
        let ang_err_vec = hinge_world_a.cross(hinge_world_b);
        for (i, &perp) in [perp1, perp2].iter().enumerate() {
            let row = &mut self.rows[3 + i];
            row.j_lin_a = Vec3::ZERO;
            row.j_ang_a = -perp;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_b = perp;
            row.bias = bias_factor * ang_err_vec.dot(perp);
        }
        // Motor row
        if self.motor_enabled {
            let row = &mut self.rows[5];
            row.j_lin_a = Vec3::ZERO;
            row.j_ang_a = -hinge_world_a;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_b = hinge_world_a;
            row.bias = -self.motor_target_velocity;
            row.lambda_min = -self.motor_max_impulse;
            row.lambda_max = self.motor_max_impulse;
        }
        // Limit row
        if self.enable_limits {
            let angle = compute_hinge_angle(rot_a, rot_b, &self.axis_a, &self.axis_b);
            let row = &mut self.rows[6];
            row.j_lin_a = Vec3::ZERO;
            row.j_ang_a = -hinge_world_a;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_b = hinge_world_a;
            if angle < self.lower_limit {
                row.bias = bias_factor * (angle - self.lower_limit);
                row.lambda_min = 0.0;
                row.lambda_max = f32::INFINITY;
            } else if angle > self.upper_limit {
                row.bias = bias_factor * (angle - self.upper_limit);
                row.lambda_min = f32::NEG_INFINITY;
                row.lambda_max = 0.0;
            } else {
                row.lambda_min = 0.0;
                row.lambda_max = 0.0;
            }
        }
    }

    pub fn get_current_angle(&self, rot_a: Quat, rot_b: Quat) -> f32 {
        compute_hinge_angle(rot_a, rot_b, &self.axis_a, &self.axis_b)
    }
}

fn compute_hinge_angle(rot_a: Quat, rot_b: Quat, axis_a: &Vec3, axis_b: &Vec3) -> f32 {
    let wa = rot_a * *axis_a;
    let wb = rot_b * *axis_b;
    let perp = perpendicular_to(wa);
    let ref_vec = rot_a * perp;
    let cur_vec = wb - wb.dot(wa) * wa;
    let cur_len = cur_vec.length();
    if cur_len < EPSILON { return 0.0; }
    let cur_norm = cur_vec / cur_len;
    let cos_a = ref_vec.dot(cur_norm).clamp(-1.0, 1.0);
    let sin_a = wa.dot(ref_vec.cross(cur_norm));
    sin_a.atan2(cos_a)
}

// ---- Slider Constraint (Prismatic) ----

#[derive(Debug, Clone)]
pub struct SliderConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub slide_axis_a: Vec3,
    pub lower_limit: f32,
    pub upper_limit: f32,
    pub enable_limits: bool,
    pub motor_enabled: bool,
    pub motor_target_velocity: f32,
    pub motor_max_force: f32,
    pub rows: Vec<JacobianRow>,
}

impl SliderConstraint {
    pub fn new(body_a: u64, body_b: u64, anchor_a: Vec3, slide_axis: Vec3) -> Self {
        let mut rows = Vec::new();
        for _ in 0..6 { rows.push(JacobianRow::new()); }
        Self {
            body_a, body_b,
            anchor_a,
            anchor_b: anchor_a,
            slide_axis_a: slide_axis.normalize(),
            lower_limit: -1.0,
            upper_limit: 1.0,
            enable_limits: false,
            motor_enabled: false,
            motor_target_velocity: 0.0,
            motor_max_force: 100.0,
            rows,
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let slide_world = rot_a * self.slide_axis_a;
        let perp1 = perpendicular_to(slide_world);
        let perp2 = slide_world.cross(perp1).normalize();
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let diff = (pos_b + rb) - (pos_a + ra);
        let bias_factor = baumgarte / dt;
        // 2 perpendicular translation rows
        for (i, &perp) in [perp1, perp2].iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -perp;
            row.j_ang_a = -(ra.cross(perp));
            row.j_lin_b = perp;
            row.j_ang_b = rb.cross(perp);
            row.bias = bias_factor * diff.dot(perp);
        }
        // 3 angular lock rows
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        let rot_diff = rot_b * rot_a.inverse();
        let (axis_err, angle_err) = quat_to_axis_angle(rot_diff);
        let ang_err = axis_err * angle_err;
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[2 + i];
            row.j_lin_a = Vec3::ZERO;
            row.j_ang_a = -ax;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_b = ax;
            row.bias = bias_factor * ang_err.dot(ax);
        }
        // Limit / motor on slide axis handled by caller
    }

    pub fn current_position(&self, pos_a: Vec3, rot_a: Quat, pos_b: Vec3, rot_b: Quat) -> f32 {
        let slide_world = rot_a * self.slide_axis_a;
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let diff = (pos_b + rb) - (pos_a + ra);
        diff.dot(slide_world)
    }
}

// ---- Ball-Socket Constraint ----

#[derive(Debug, Clone)]
pub struct BallSocketConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub pivot_a: Vec3,
    pub pivot_b: Vec3,
    pub rows: Vec<JacobianRow>,
}

impl BallSocketConstraint {
    pub fn new(body_a: u64, body_b: u64, pivot_a: Vec3, pivot_b: Vec3) -> Self {
        let mut rows = Vec::new();
        for _ in 0..3 { rows.push(JacobianRow::new()); }
        Self { body_a, body_b, pivot_a, pivot_b, rows }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let ra = rot_a * self.pivot_a;
        let rb = rot_b * self.pivot_b;
        let err = (pos_b + rb) - (pos_a + ra);
        let bf = baumgarte / dt;
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -ax;
            row.j_ang_a = -(ra.cross(ax));
            row.j_lin_b = ax;
            row.j_ang_b = rb.cross(ax);
            row.bias = bf * err.dot(ax);
        }
    }
}

// ---- Cone-Twist Constraint ----

#[derive(Debug, Clone)]
pub struct ConeTwistConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub pivot_a: Vec3,
    pub pivot_b: Vec3,
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    pub swing_span1: f32,
    pub swing_span2: f32,
    pub twist_span: f32,
    pub softness: f32,
    pub bias_factor: f32,
    pub relaxation_factor: f32,
    pub rows: Vec<JacobianRow>,
}

impl ConeTwistConstraint {
    pub fn new(body_a: u64, body_b: u64, pivot: Vec3, axis: Vec3) -> Self {
        let mut rows = Vec::new();
        for _ in 0..6 { rows.push(JacobianRow::new()); }
        Self {
            body_a, body_b,
            pivot_a: pivot, pivot_b: pivot,
            axis_a: axis.normalize(), axis_b: axis.normalize(),
            swing_span1: HALF_PI,
            swing_span2: HALF_PI,
            twist_span: PI,
            softness: 1.0,
            bias_factor: 0.3,
            relaxation_factor: 1.0,
            rows,
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let ra = rot_a * self.pivot_a;
        let rb = rot_b * self.pivot_b;
        let err = (pos_b + rb) - (pos_a + ra);
        let bf = baumgarte / dt;
        // Ball-socket part
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -ax;
            row.j_ang_a = -(ra.cross(ax));
            row.j_lin_b = ax;
            row.j_ang_b = rb.cross(ax);
            row.bias = bf * err.dot(ax);
        }
        // Cone and twist limit rows
        let axis_world_a = rot_a * self.axis_a;
        let axis_world_b = rot_b * self.axis_b;
        // Compute swing and twist decomposition
        let (swing_q, twist_q) = decompose_swing_twist(
            rot_b * rot_a.inverse(), axis_world_a
        );
        let (swing_axis, swing_angle) = quat_to_axis_angle(swing_q);
        let (twist_axis, twist_angle) = quat_to_axis_angle(twist_q);
        // Swing limit row
        {
            let row = &mut self.rows[3];
            row.j_lin_a = Vec3::ZERO;
            row.j_lin_b = Vec3::ZERO;
            let perp = perpendicular_to(axis_world_a);
            row.j_ang_a = -perp;
            row.j_ang_b = perp;
            let limit = self.swing_span1.max(EPSILON);
            if swing_angle > limit {
                row.bias = bf * (swing_angle - limit);
                row.lambda_min = f32::NEG_INFINITY;
                row.lambda_max = 0.0;
            }
        }
        // Twist limit row
        {
            let row = &mut self.rows[4];
            row.j_lin_a = Vec3::ZERO;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_a = -axis_world_a;
            row.j_ang_b = axis_world_a;
            let limit = self.twist_span.max(EPSILON);
            if twist_angle.abs() > limit {
                let sign = twist_angle.signum();
                row.bias = bf * (twist_angle - sign * limit);
                if sign > 0.0 {
                    row.lambda_min = f32::NEG_INFINITY;
                    row.lambda_max = 0.0;
                } else {
                    row.lambda_min = 0.0;
                    row.lambda_max = f32::INFINITY;
                }
            }
        }
    }
}

// ---- Generic 6DOF Constraint ----

#[derive(Debug, Clone)]
pub struct Generic6DOFConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub frame_a: Mat4, // local frame in body A
    pub frame_b: Mat4, // local frame in body B
    pub linear_lower: Vec3,
    pub linear_upper: Vec3,
    pub angular_lower: Vec3,
    pub angular_upper: Vec3,
    pub linear_enabled: [bool; 3],
    pub angular_enabled: [bool; 3],
    pub spring_stiffness: [f32; 6],
    pub spring_damping: [f32; 6],
    pub spring_enabled: [bool; 6],
    pub rows: Vec<JacobianRow>,
}

impl Generic6DOFConstraint {
    pub fn new(body_a: u64, body_b: u64, frame_a: Mat4, frame_b: Mat4) -> Self {
        let mut rows = Vec::new();
        for _ in 0..6 { rows.push(JacobianRow::new()); }
        Self {
            body_a, body_b,
            frame_a, frame_b,
            linear_lower: Vec3::ZERO,
            linear_upper: Vec3::ZERO,
            angular_lower: Vec3::splat(-PI),
            angular_upper: Vec3::splat(PI),
            linear_enabled: [true; 3],
            angular_enabled: [true; 3],
            spring_stiffness: [0.0; 6],
            spring_damping: [0.0; 6],
            spring_enabled: [false; 6],
            rows,
        }
    }

    pub fn set_linear_free(&mut self) {
        self.linear_lower = Vec3::splat(f32::NEG_INFINITY);
        self.linear_upper = Vec3::splat(f32::INFINITY);
    }

    pub fn set_angular_locked(&mut self) {
        self.angular_lower = Vec3::ZERO;
        self.angular_upper = Vec3::ZERO;
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let bf = baumgarte / dt;
        // Extract world frames
        let r_a = Mat4::from_quat(rot_a);
        let ax_ax = Vec3::new(r_a.col(0).x, r_a.col(0).y, r_a.col(0).z);
        let ax_ay = Vec3::new(r_a.col(1).x, r_a.col(1).y, r_a.col(1).z);
        let ax_az = Vec3::new(r_a.col(2).x, r_a.col(2).y, r_a.col(2).z);
        let pivot_a = Vec3::new(self.frame_a.col(3).x, self.frame_a.col(3).y, self.frame_a.col(3).z);
        let pivot_b = Vec3::new(self.frame_b.col(3).x, self.frame_b.col(3).y, self.frame_b.col(3).z);
        let ra = rot_a * pivot_a;
        let rb = rot_b * pivot_b;
        let diff = (pos_b + rb) - (pos_a + ra);
        let lin_axes = [ax_ax, ax_ay, ax_az];
        for (i, &ax) in lin_axes.iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -ax;
            row.j_ang_a = -(ra.cross(ax));
            row.j_lin_b = ax;
            row.j_ang_b = rb.cross(ax);
            let dist = diff.dot(ax);
            let lo = match i { 0 => self.linear_lower.x, 1 => self.linear_lower.y, _ => self.linear_lower.z };
            let hi = match i { 0 => self.linear_upper.x, 1 => self.linear_upper.y, _ => self.linear_upper.z };
            if self.linear_enabled[i] {
                if lo == hi {
                    row.bias = bf * (dist - lo);
                } else if dist < lo {
                    row.bias = bf * (dist - lo);
                    row.lambda_min = 0.0;
                    row.lambda_max = f32::INFINITY;
                } else if dist > hi {
                    row.bias = bf * (dist - hi);
                    row.lambda_min = f32::NEG_INFINITY;
                    row.lambda_max = 0.0;
                } else {
                    row.lambda_min = 0.0;
                    row.lambda_max = 0.0;
                }
            }
        }
        // Angular 3 rows
        let rot_diff = rot_b * rot_a.inverse();
        let (axis_err, angle_err) = quat_to_axis_angle(rot_diff);
        let ang_err = axis_err * angle_err;
        for (i, &ax) in lin_axes.iter().enumerate() {
            let row = &mut self.rows[3 + i];
            row.j_lin_a = Vec3::ZERO;
            row.j_ang_a = -ax;
            row.j_lin_b = Vec3::ZERO;
            row.j_ang_b = ax;
            row.bias = bf * ang_err.dot(ax);
        }
    }
}

// ---- Spring Constraint ----

#[derive(Debug, Clone)]
pub struct SpringConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub min_length: f32,
    pub max_length: f32,
    pub row: JacobianRow,
}

impl SpringConstraint {
    pub fn new(body_a: u64, body_b: u64, anchor_a: Vec3, anchor_b: Vec3,
               rest_length: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            body_a, body_b, anchor_a, anchor_b,
            rest_length, stiffness, damping,
            min_length: 0.0,
            max_length: f32::INFINITY,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat, vel_a: Vec3, omega_a: Vec3,
        pos_b: Vec3, rot_b: Quat, vel_b: Vec3, omega_b: Vec3,
        dt: f32,
    ) {
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let wa = pos_a + ra;
        let wb = pos_b + rb;
        let delta = wb - wa;
        let dist = delta.length();
        if dist < EPSILON { return; }
        let n = delta / dist;
        let extension = dist - self.rest_length;
        // Spring force: F = -k*x - d*v_rel_along_n
        let v_a_pt = vel_a + omega_a.cross(ra);
        let v_b_pt = vel_b + omega_b.cross(rb);
        let v_rel = (v_b_pt - v_a_pt).dot(n);
        let spring_force = -self.stiffness * extension - self.damping * v_rel;
        let row = &mut self.row;
        row.j_lin_a = -n;
        row.j_ang_a = -(ra.cross(n));
        row.j_lin_b = n;
        row.j_ang_b = rb.cross(n);
        // Convert spring force to position constraint bias
        row.bias = spring_force / (self.stiffness.max(EPSILON) * dt);
        row.lambda_min = if dist < self.min_length { 0.0 } else { f32::NEG_INFINITY };
        row.lambda_max = if dist > self.max_length { 0.0 } else { f32::INFINITY };
    }
}

// ---- Gear Constraint ----

#[derive(Debug, Clone)]
pub struct GearConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    pub ratio: f32,      // gear ratio (omega_b = ratio * omega_a)
    pub row: JacobianRow,
}

impl GearConstraint {
    pub fn new(body_a: u64, body_b: u64, axis_a: Vec3, axis_b: Vec3, ratio: f32) -> Self {
        Self { body_a, body_b, axis_a: axis_a.normalize(), axis_b: axis_b.normalize(), ratio, row: JacobianRow::new() }
    }

    pub fn build_jacobian(&mut self, rot_a: Quat, rot_b: Quat) {
        let wa = rot_a * self.axis_a;
        let wb = rot_b * self.axis_b;
        let row = &mut self.row;
        row.j_lin_a = Vec3::ZERO;
        row.j_ang_a = wa;
        row.j_lin_b = Vec3::ZERO;
        row.j_ang_b = wb * (-self.ratio);
        row.bias = 0.0;
        row.lambda_min = f32::NEG_INFINITY;
        row.lambda_max = f32::INFINITY;
    }
}

// ---- Rack-and-Pinion Constraint ----

#[derive(Debug, Clone)]
pub struct RackAndPinionConstraint {
    pub body_pinion: u64,   // rotating gear
    pub body_rack: u64,     // translating rack
    pub pinion_axis: Vec3,  // rotation axis of pinion
    pub rack_axis: Vec3,    // translation axis of rack
    pub pitch_radius: f32,  // pinion pitch radius
    pub row: JacobianRow,
}

impl RackAndPinionConstraint {
    pub fn new(body_pinion: u64, body_rack: u64, pinion_axis: Vec3, rack_axis: Vec3, pitch_radius: f32) -> Self {
        Self {
            body_pinion, body_rack,
            pinion_axis: pinion_axis.normalize(),
            rack_axis: rack_axis.normalize(),
            pitch_radius,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(&mut self, rot_pinion: Quat, rot_rack: Quat) {
        let wa = rot_pinion * self.pinion_axis;
        let trans_ax = rot_rack * self.rack_axis;
        // v_rack = pitch_radius * omega_pinion
        let row = &mut self.row;
        row.j_lin_a = Vec3::ZERO;
        row.j_ang_a = wa * self.pitch_radius;
        row.j_lin_b = trans_ax * (-1.0);
        row.j_ang_b = Vec3::ZERO;
        row.bias = 0.0;
    }
}

// ---- Pulley Constraint ----

#[derive(Debug, Clone)]
pub struct PulleyConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub anchor_a: Vec3,         // attachment on body A
    pub anchor_b: Vec3,         // attachment on body B
    pub fixed_point_a: Vec3,    // fixed pulley wheel A world pos
    pub fixed_point_b: Vec3,    // fixed pulley wheel B world pos
    pub ratio: f32,             // pulley ratio
    pub total_length: f32,      // total rope length
    pub row: JacobianRow,
}

impl PulleyConstraint {
    pub fn new(body_a: u64, body_b: u64,
               anchor_a: Vec3, anchor_b: Vec3,
               fixed_a: Vec3, fixed_b: Vec3,
               ratio: f32, total_length: f32) -> Self {
        Self {
            body_a, body_b, anchor_a, anchor_b,
            fixed_point_a: fixed_a, fixed_point_b: fixed_b,
            ratio, total_length,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let wa = pos_a + ra;
        let wb = pos_b + rb;
        let dir_a = self.fixed_point_a - wa;
        let dist_a = dir_a.length();
        let dir_b = self.fixed_point_b - wb;
        let dist_b = dir_b.length();
        let n_a = if dist_a > EPSILON { dir_a / dist_a } else { Vec3::Y };
        let n_b = if dist_b > EPSILON { dir_b / dist_b } else { Vec3::Y };
        let constraint_err = dist_a + self.ratio * dist_b - self.total_length;
        let row = &mut self.row;
        row.j_lin_a = -n_a;
        row.j_ang_a = -(ra.cross(n_a));
        row.j_lin_b = -n_b * self.ratio;
        row.j_ang_b = -(rb.cross(n_b)) * self.ratio;
        row.bias = (baumgarte / dt) * constraint_err;
        row.lambda_min = 0.0; // rope can only pull
        row.lambda_max = f32::INFINITY;
    }
}

// ---- Motor Constraint ----

#[derive(Debug, Clone)]
pub struct MotorConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub axis: Vec3,
    pub target_velocity: f32,
    pub max_torque: f32,
    pub servo_enabled: bool,
    pub target_angle: f32,
    pub servo_stiffness: f32,
    pub row: JacobianRow,
}

impl MotorConstraint {
    pub fn new(body_a: u64, body_b: u64, axis: Vec3) -> Self {
        Self {
            body_a, body_b,
            axis: axis.normalize(),
            target_velocity: 0.0,
            max_torque: 100.0,
            servo_enabled: false,
            target_angle: 0.0,
            servo_stiffness: 10.0,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(&mut self, rot_a: Quat, rot_b: Quat, current_angle: f32, dt: f32) {
        let wa = rot_a * self.axis;
        let row = &mut self.row;
        row.j_lin_a = Vec3::ZERO;
        row.j_ang_a = -wa;
        row.j_lin_b = Vec3::ZERO;
        row.j_ang_b = wa;
        if self.servo_enabled {
            let angle_err = self.target_angle - current_angle;
            row.bias = -self.target_velocity - self.servo_stiffness * angle_err * dt;
        } else {
            row.bias = -self.target_velocity;
        }
        row.lambda_min = -self.max_torque * dt;
        row.lambda_max = self.max_torque * dt;
    }
}

// ---- Limit Constraint (generic 1-DOF with lo/hi) ----

#[derive(Debug, Clone)]
pub struct LimitConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub axis: Vec3,
    pub linear: bool, // true = linear, false = angular
    pub lower: f32,
    pub upper: f32,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub restitution: f32,
    pub row: JacobianRow,
}

impl LimitConstraint {
    pub fn new(body_a: u64, body_b: u64, axis: Vec3, lower: f32, upper: f32, linear: bool) -> Self {
        Self {
            body_a, body_b,
            axis: axis.normalize(),
            linear, lower, upper,
            anchor_a: Vec3::ZERO, anchor_b: Vec3::ZERO,
            restitution: 0.0,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat, vel_a: Vec3, omega_a: Vec3,
        pos_b: Vec3, rot_b: Quat, vel_b: Vec3, omega_b: Vec3,
        baumgarte: f32, dt: f32,
    ) {
        let bf = baumgarte / dt;
        if self.linear {
            let wa_ax = rot_a * self.axis;
            let ra = rot_a * self.anchor_a;
            let rb = rot_b * self.anchor_b;
            let diff = (pos_b + rb) - (pos_a + ra);
            let pos = diff.dot(wa_ax);
            self.row.j_lin_a = -wa_ax;
            self.row.j_ang_a = -(ra.cross(wa_ax));
            self.row.j_lin_b = wa_ax;
            self.row.j_ang_b = rb.cross(wa_ax);
            if pos < self.lower {
                self.row.bias = bf * (pos - self.lower);
                let rel_vel = self.row.j_lin_a.dot(vel_a) + self.row.j_lin_b.dot(vel_b);
                if rel_vel < 0.0 { self.row.bias += self.restitution * rel_vel; }
                self.row.lambda_min = 0.0;
                self.row.lambda_max = f32::INFINITY;
            } else if pos > self.upper {
                self.row.bias = bf * (pos - self.upper);
                self.row.lambda_min = f32::NEG_INFINITY;
                self.row.lambda_max = 0.0;
            }
        } else {
            let wa = rot_a * self.axis;
            self.row.j_lin_a = Vec3::ZERO;
            self.row.j_ang_a = -wa;
            self.row.j_lin_b = Vec3::ZERO;
            self.row.j_ang_b = wa;
            // Angular position would be computed by caller
        }
    }
}

// ---- Distance Constraint ----

#[derive(Debug, Clone)]
pub struct DistanceConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub min_distance: f32,
    pub max_distance: f32,
    pub row: JacobianRow,
}

impl DistanceConstraint {
    pub fn new(body_a: u64, body_b: u64, anchor_a: Vec3, anchor_b: Vec3, dist: f32) -> Self {
        Self {
            body_a, body_b, anchor_a, anchor_b,
            min_distance: dist, max_distance: dist,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat,
        pos_b: Vec3, rot_b: Quat,
        baumgarte: f32, dt: f32,
    ) {
        let ra = rot_a * self.anchor_a;
        let rb = rot_b * self.anchor_b;
        let wa = pos_a + ra;
        let wb = pos_b + rb;
        let delta = wb - wa;
        let dist = delta.length();
        if dist < EPSILON { return; }
        let n = delta / dist;
        let bf = baumgarte / dt;
        self.row.j_lin_a = -n;
        self.row.j_ang_a = -(ra.cross(n));
        self.row.j_lin_b = n;
        self.row.j_ang_b = rb.cross(n);
        if dist < self.min_distance {
            self.row.bias = bf * (dist - self.min_distance);
            self.row.lambda_min = 0.0;
            self.row.lambda_max = f32::INFINITY;
        } else if dist > self.max_distance {
            self.row.bias = bf * (dist - self.max_distance);
            self.row.lambda_min = f32::NEG_INFINITY;
            self.row.lambda_max = 0.0;
        } else {
            self.row.lambda_min = f32::NEG_INFINITY;
            self.row.lambda_max = f32::INFINITY;
        }
    }
}

// ---- Point-to-Point Constraint (alias of BallSocket with extra params) ----

#[derive(Debug, Clone)]
pub struct PointToPointConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub pivot_a: Vec3,
    pub pivot_b: Vec3,
    pub tau: f32,   // softness factor (0 = rigid, 1 = very soft)
    pub damping: f32,
    pub impulse_clamp: f32,
    pub rows: Vec<JacobianRow>,
}

impl PointToPointConstraint {
    pub fn new(body_a: u64, body_b: u64, pivot: Vec3) -> Self {
        let mut rows = Vec::new();
        for _ in 0..3 { rows.push(JacobianRow::new()); }
        Self {
            body_a, body_b,
            pivot_a: pivot, pivot_b: pivot,
            tau: 0.3, damping: 1.0,
            impulse_clamp: 0.0,
            rows,
        }
    }

    pub fn build_jacobian(
        &mut self,
        pos_a: Vec3, rot_a: Quat, vel_a: Vec3, omega_a: Vec3,
        pos_b: Vec3, rot_b: Quat, vel_b: Vec3, omega_b: Vec3,
        dt: f32,
    ) {
        let ra = rot_a * self.pivot_a;
        let rb = rot_b * self.pivot_b;
        let err = (pos_b + rb) - (pos_a + ra);
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        for (i, &ax) in axes.iter().enumerate() {
            let row = &mut self.rows[i];
            row.j_lin_a = -ax;
            row.j_ang_a = -(ra.cross(ax));
            row.j_lin_b = ax;
            row.j_ang_b = rb.cross(ax);
            // Soft constraint: bias = tau/dt * error + damping * rel_vel
            let v_a_pt = vel_a + omega_a.cross(ra);
            let v_b_pt = vel_b + omega_b.cross(rb);
            let rel_v = (v_b_pt - v_a_pt).dot(ax);
            row.bias = (self.tau / dt) * err.dot(ax) + self.damping * rel_v;
            if self.impulse_clamp > 0.0 {
                row.lambda_min = -self.impulse_clamp;
                row.lambda_max = self.impulse_clamp;
            }
        }
    }
}

// ---- Angular Constraint ----

#[derive(Debug, Clone)]
pub struct AngularConstraint {
    pub body_a: u64,
    pub body_b: u64,
    pub axis: Vec3,
    pub target_angle: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub row: JacobianRow,
}

impl AngularConstraint {
    pub fn new(body_a: u64, body_b: u64, axis: Vec3) -> Self {
        Self {
            body_a, body_b,
            axis: axis.normalize(),
            target_angle: 0.0,
            stiffness: 100.0,
            damping: 10.0,
            row: JacobianRow::new(),
        }
    }

    pub fn build_jacobian(
        &mut self,
        rot_a: Quat, omega_a: Vec3,
        rot_b: Quat, omega_b: Vec3,
        current_angle: f32, dt: f32,
    ) {
        let wa = rot_a * self.axis;
        let angle_err = self.target_angle - current_angle;
        let rel_omega = (omega_b - omega_a).dot(wa);
        let row = &mut self.row;
        row.j_lin_a = Vec3::ZERO;
        row.j_ang_a = -wa;
        row.j_lin_b = Vec3::ZERO;
        row.j_ang_b = wa;
        row.bias = self.stiffness * angle_err * dt - self.damping * rel_omega;
    }
}

// ---- Weld Constraint (alias of Fixed with no breaking) ----
#[derive(Debug, Clone)]
pub struct WeldConstraint {
    pub inner: FixedConstraint,
    pub allow_rotation: bool,
}

impl WeldConstraint {
    pub fn new(body_a: u64, body_b: u64, anchor_a: Vec3, anchor_b: Vec3) -> Self {
        let inner = FixedConstraint::new(body_a, body_b, anchor_a, anchor_b);
        Self { inner, allow_rotation: false }
    }
}

// ============================================================
// CONSTRAINT ENUM for editor storage
// ============================================================

#[derive(Debug, Clone)]
pub enum Constraint {
    Fixed(FixedConstraint),
    Hinge(HingeConstraint),
    Slider(SliderConstraint),
    BallSocket(BallSocketConstraint),
    ConeTwist(ConeTwistConstraint),
    Generic6DOF(Generic6DOFConstraint),
    Spring(SpringConstraint),
    Gear(GearConstraint),
    RackAndPinion(RackAndPinionConstraint),
    Pulley(PulleyConstraint),
    Motor(MotorConstraint),
    Limit(LimitConstraint),
    Distance(DistanceConstraint),
    PointToPoint(PointToPointConstraint),
    Angular(AngularConstraint),
    Weld(WeldConstraint),
}

impl Constraint {
    pub fn name(&self) -> &'static str {
        match self {
            Constraint::Fixed(_) => "Fixed",
            Constraint::Hinge(_) => "Hinge",
            Constraint::Slider(_) => "Slider",
            Constraint::BallSocket(_) => "BallSocket",
            Constraint::ConeTwist(_) => "ConeTwist",
            Constraint::Generic6DOF(_) => "Generic6DOF",
            Constraint::Spring(_) => "Spring",
            Constraint::Gear(_) => "Gear",
            Constraint::RackAndPinion(_) => "RackAndPinion",
            Constraint::Pulley(_) => "Pulley",
            Constraint::Motor(_) => "Motor",
            Constraint::Limit(_) => "Limit",
            Constraint::Distance(_) => "Distance",
            Constraint::PointToPoint(_) => "PointToPoint",
            Constraint::Angular(_) => "Angular",
            Constraint::Weld(_) => "Weld",
        }
    }
}

// ============================================================
// JOINT EDITOR — visual positioning & axis/limit arc drawing
// ============================================================

#[derive(Debug, Clone)]
pub struct JointVisualizer {
    pub constraint_id: u64,
    pub pivot_world: Vec3,
    pub axis_world: Vec3,
    pub axis_color: Vec4,
    pub arc_segments: u32,
    pub arc_radius: f32,
    pub show_limits: bool,
    pub show_drive: bool,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct ArcPoint {
    pub pos: Vec3,
    pub t: f32,     // parameter [0,1]
}

impl JointVisualizer {
    pub fn new(constraint_id: u64, pivot: Vec3, axis: Vec3) -> Self {
        Self {
            constraint_id,
            pivot_world: pivot,
            axis_world: axis.normalize(),
            axis_color: Vec4::new(1.0, 1.0, 0.0, 1.0),
            arc_segments: 32,
            arc_radius: 0.2,
            show_limits: true,
            show_drive: true,
            selected: false,
        }
    }

    /// Generate arc points for a hinge limit arc in 3D space
    pub fn compute_limit_arc(&self, lower: f32, upper: f32) -> Vec<ArcPoint> {
        let mut points = Vec::new();
        let n = self.arc_segments as usize;
        if n == 0 { return points; }
        // Build local frame: perp1, perp2, axis
        let perp1 = perpendicular_to(self.axis_world);
        let perp2 = self.axis_world.cross(perp1).normalize();
        let span = upper - lower;
        let step = span / n as f32;
        for i in 0..=n {
            let angle = lower + step * i as f32;
            let t = i as f32 / n as f32;
            let c = angle.cos();
            let s = angle.sin();
            let local = perp1 * c + perp2 * s;
            let pos = self.pivot_world + local * self.arc_radius;
            points.push(ArcPoint { pos, t });
        }
        points
    }

    /// Generate cone surface sample points
    pub fn compute_cone_surface(&self, half_angle: f32) -> Vec<Vec3> {
        let mut pts = Vec::new();
        let n = self.arc_segments as usize;
        let perp1 = perpendicular_to(self.axis_world);
        let perp2 = self.axis_world.cross(perp1).normalize();
        let tip = self.pivot_world;
        let r = self.arc_radius * half_angle.tan();
        let apex = tip + self.axis_world * self.arc_radius;
        for i in 0..n {
            let angle = TWO_PI * i as f32 / n as f32;
            let base = apex + (perp1 * angle.cos() + perp2 * angle.sin()) * r;
            pts.push(tip);
            pts.push(base);
            let angle_next = TWO_PI * (i + 1) as f32 / n as f32;
            let base_next = apex + (perp1 * angle_next.cos() + perp2 * angle_next.sin()) * r;
            pts.push(base_next);
        }
        pts
    }

    /// Axis line endpoints
    pub fn axis_line(&self, len: f32) -> (Vec3, Vec3) {
        let start = self.pivot_world - self.axis_world * len * 0.5;
        let end = self.pivot_world + self.axis_world * len * 0.5;
        (start, end)
    }

    /// Generate drive target indicator
    pub fn drive_indicator(&self, target_angle: f32) -> Vec3 {
        let perp1 = perpendicular_to(self.axis_world);
        let perp2 = self.axis_world.cross(perp1).normalize();
        let c = target_angle.cos();
        let s = target_angle.sin();
        self.pivot_world + (perp1 * c + perp2 * s) * self.arc_radius
    }
}

#[derive(Debug, Clone)]
pub struct JointEditor {
    pub visualizers: HashMap<u64, JointVisualizer>,
    pub selected_joint: Option<u64>,
    pub next_id: u64,
    pub gizmo_mode: JointGizmoMode,
    pub snap_angle_deg: f32,
    pub snap_position: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JointGizmoMode {
    Translate,
    Rotate,
    Scale,
}

impl JointEditor {
    pub fn new() -> Self {
        Self {
            visualizers: HashMap::new(),
            selected_joint: None,
            next_id: 1,
            gizmo_mode: JointGizmoMode::Translate,
            snap_angle_deg: 5.0,
            snap_position: 0.1,
        }
    }

    pub fn add_joint_visualizer(&mut self, pivot: Vec3, axis: Vec3) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.visualizers.insert(id, JointVisualizer::new(id, pivot, axis));
        id
    }

    pub fn set_selected(&mut self, id: u64) {
        if let Some(old) = self.selected_joint {
            if let Some(v) = self.visualizers.get_mut(&old) {
                v.selected = false;
            }
        }
        self.selected_joint = Some(id);
        if let Some(v) = self.visualizers.get_mut(&id) {
            v.selected = true;
        }
    }

    pub fn move_joint(&mut self, id: u64, delta: Vec3) {
        if let Some(v) = self.visualizers.get_mut(&id) {
            let snap = self.snap_position;
            let snapped = Vec3::new(
                snap_value(v.pivot_world.x + delta.x, snap),
                snap_value(v.pivot_world.y + delta.y, snap),
                snap_value(v.pivot_world.z + delta.z, snap),
            );
            v.pivot_world = snapped;
        }
    }

    pub fn rotate_axis(&mut self, id: u64, rotation: Quat) {
        if let Some(v) = self.visualizers.get_mut(&id) {
            v.axis_world = (rotation * v.axis_world).normalize();
        }
    }

    pub fn generate_all_debug_lines(&self) -> Vec<(Vec3, Vec3, Vec4)> {
        let mut lines = Vec::new();
        for (_, vis) in &self.visualizers {
            let (a, b) = vis.axis_line(0.5);
            lines.push((a, b, vis.axis_color));
        }
        lines
    }
}

// ============================================================
// CLOTH SIMULATION PARAMETERS
// ============================================================

#[derive(Debug, Clone)]
pub struct ClothParticle {
    pub position: Vec3,
    pub prev_position: Vec3,
    pub velocity: Vec3,
    pub mass: f32,
    pub inv_mass: f32,
    pub pinned: bool,
    pub normal: Vec3,
    pub tex_coord: Vec2,
}

impl ClothParticle {
    pub fn new(pos: Vec3, mass: f32) -> Self {
        Self {
            position: pos,
            prev_position: pos,
            velocity: Vec3::ZERO,
            mass,
            inv_mass: if mass > EPSILON { 1.0 / mass } else { 0.0 },
            pinned: false,
            normal: Vec3::Y,
            tex_coord: Vec2::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClothSpring {
    pub particle_a: usize,
    pub particle_b: usize,
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub spring_type: ClothSpringType,
    pub torn: bool,
    pub tear_threshold: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClothSpringType {
    Stretch,
    Shear,
    Bend,
}

#[derive(Debug, Clone)]
pub struct ClothSimulation {
    pub particles: Vec<ClothParticle>,
    pub springs: Vec<ClothSpring>,
    pub grid_width: usize,
    pub grid_height: usize,
    pub total_mass: f32,
    pub stretch_stiffness: f32,
    pub shear_stiffness: f32,
    pub bend_stiffness: f32,
    pub stretch_damping: f32,
    pub shear_damping: f32,
    pub bend_damping: f32,
    pub gravity: Vec3,
    pub wind_velocity: Vec3,
    pub wind_turbulence: f32,
    pub drag_coefficient: f32,
    pub thickness: f32,
    pub self_collision_enabled: bool,
    pub self_collision_radius: f32,
    pub tearing_enabled: bool,
    pub global_tear_threshold: f32,
    pub pinned_vertices: HashSet<usize>,
    pub iterations: u32,
}

impl ClothSimulation {
    pub fn new(width: usize, height: usize, cell_size: f32, mass_per_particle: f32) -> Self {
        let mut particles = Vec::new();
        let mut springs = Vec::new();
        // Create particle grid
        for j in 0..height {
            for i in 0..width {
                let pos = Vec3::new(i as f32 * cell_size, 0.0, j as f32 * cell_size);
                let mut p = ClothParticle::new(pos, mass_per_particle);
                p.tex_coord = Vec2::new(i as f32 / (width - 1) as f32, j as f32 / (height - 1) as f32);
                particles.push(p);
            }
        }
        let idx = |i: usize, j: usize| j * width + i;
        // Structural (stretch) springs — horizontal and vertical
        for j in 0..height {
            for i in 0..width {
                if i + 1 < width {
                    let dist = cell_size;
                    springs.push(ClothSpring {
                        particle_a: idx(i, j),
                        particle_b: idx(i + 1, j),
                        rest_length: dist,
                        stiffness: 1000.0,
                        damping: 0.1,
                        spring_type: ClothSpringType::Stretch,
                        torn: false,
                        tear_threshold: 2.0 * dist,
                    });
                }
                if j + 1 < height {
                    let dist = cell_size;
                    springs.push(ClothSpring {
                        particle_a: idx(i, j),
                        particle_b: idx(i, j + 1),
                        rest_length: dist,
                        stiffness: 1000.0,
                        damping: 0.1,
                        spring_type: ClothSpringType::Stretch,
                        torn: false,
                        tear_threshold: 2.0 * dist,
                    });
                }
            }
        }
        // Shear springs — diagonals
        for j in 0..height - 1 {
            for i in 0..width - 1 {
                let diag = cell_size * 2.0_f32.sqrt();
                springs.push(ClothSpring {
                    particle_a: idx(i, j),
                    particle_b: idx(i + 1, j + 1),
                    rest_length: diag,
                    stiffness: 500.0,
                    damping: 0.05,
                    spring_type: ClothSpringType::Shear,
                    torn: false,
                    tear_threshold: 3.0 * diag,
                });
                springs.push(ClothSpring {
                    particle_a: idx(i + 1, j),
                    particle_b: idx(i, j + 1),
                    rest_length: diag,
                    stiffness: 500.0,
                    damping: 0.05,
                    spring_type: ClothSpringType::Shear,
                    torn: false,
                    tear_threshold: 3.0 * diag,
                });
            }
        }
        // Bend springs — skip one particle
        for j in 0..height {
            for i in 0..width {
                if i + 2 < width {
                    let dist = 2.0 * cell_size;
                    springs.push(ClothSpring {
                        particle_a: idx(i, j),
                        particle_b: idx(i + 2, j),
                        rest_length: dist,
                        stiffness: 100.0,
                        damping: 0.01,
                        spring_type: ClothSpringType::Bend,
                        torn: false,
                        tear_threshold: 4.0 * dist,
                    });
                }
                if j + 2 < height {
                    let dist = 2.0 * cell_size;
                    springs.push(ClothSpring {
                        particle_a: idx(i, j),
                        particle_b: idx(i, j + 2),
                        rest_length: dist,
                        stiffness: 100.0,
                        damping: 0.01,
                        spring_type: ClothSpringType::Bend,
                        torn: false,
                        tear_threshold: 4.0 * dist,
                    });
                }
            }
        }
        let total_mass = mass_per_particle * (width * height) as f32;
        Self {
            particles, springs,
            grid_width: width, grid_height: height,
            total_mass,
            stretch_stiffness: 1000.0,
            shear_stiffness: 500.0,
            bend_stiffness: 100.0,
            stretch_damping: 0.1,
            shear_damping: 0.05,
            bend_damping: 0.01,
            gravity: Vec3::new(0.0, -GRAVITY, 0.0),
            wind_velocity: Vec3::ZERO,
            wind_turbulence: 0.1,
            drag_coefficient: 0.05,
            thickness: 0.001,
            self_collision_enabled: true,
            self_collision_radius: 0.01,
            tearing_enabled: false,
            global_tear_threshold: 3.0,
            pinned_vertices: HashSet::new(),
            iterations: 10,
        }
    }

    pub fn pin_vertex(&mut self, idx: usize) {
        if idx < self.particles.len() {
            self.particles[idx].pinned = true;
            self.particles[idx].inv_mass = 0.0;
            self.pinned_vertices.insert(idx);
        }
    }

    pub fn unpin_vertex(&mut self, idx: usize) {
        if idx < self.particles.len() {
            self.particles[idx].pinned = false;
            if self.particles[idx].mass > EPSILON {
                self.particles[idx].inv_mass = 1.0 / self.particles[idx].mass;
            }
            self.pinned_vertices.remove(&idx);
        }
    }

    /// Compute wind force on a triangle given its vertices
    pub fn wind_force_on_triangle(&self, p0: Vec3, p1: Vec3, p2: Vec3, time: f32) -> Vec3 {
        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let normal = edge1.cross(edge2);
        let area = normal.length() * 0.5;
        if area < EPSILON { return Vec3::ZERO; }
        let n = normal / (2.0 * area); // unit normal
        // Turbulence using simple hash-based noise
        let turb = Vec3::new(
            (time * 2.3 + p0.x).sin() * self.wind_turbulence,
            (time * 1.7 + p0.y).cos() * self.wind_turbulence * 0.5,
            (time * 3.1 + p0.z).sin() * self.wind_turbulence,
        );
        let effective_wind = self.wind_velocity + turb;
        let relative_wind = effective_wind; // ignore particle velocity for now
        let wind_dot_n = relative_wind.dot(n);
        // Force proportional to area * (v·n) * n (aerodynamic pressure)
        let rho_half = AIR_DENSITY * 0.5;
        let force = n * (rho_half * wind_dot_n * wind_dot_n.abs() * area);
        force
    }

    /// Verlet integrate all particles
    pub fn integrate(&mut self, dt: f32, time: f32) {
        let dt2 = dt * dt;
        for p in &mut self.particles {
            if p.pinned { continue; }
            // Save old position
            let old_pos = p.position;
            // Gravity force per unit mass (since Verlet: x += v*dt + a*dt^2)
            let acc = self.gravity;
            // Wind drag (rough approximation)
            let drag = -p.velocity * self.drag_coefficient;
            let total_acc = acc + drag * p.inv_mass;
            // Verlet step
            let new_pos = p.position * 2.0 - p.prev_position + total_acc * dt2;
            p.prev_position = old_pos;
            p.position = new_pos;
            p.velocity = (p.position - p.prev_position) / dt;
        }
    }

    /// Solve spring constraints (Gauss-Seidel)
    pub fn solve_springs(&mut self, dt: f32) {
        for _ in 0..self.iterations {
            for spring_idx in 0..self.springs.len() {
                if self.springs[spring_idx].torn { continue; }
                let a = self.springs[spring_idx].particle_a;
                let b = self.springs[spring_idx].particle_b;
                let rest = self.springs[spring_idx].rest_length;
                let stiff = self.springs[spring_idx].stiffness;
                let damp = self.springs[spring_idx].damping;
                let tear = self.springs[spring_idx].tear_threshold;
                let pa = self.particles[a].position;
                let pb = self.particles[b].position;
                let ia = self.particles[a].inv_mass;
                let ib = self.particles[b].inv_mass;
                let sum_inv = ia + ib;
                if sum_inv < EPSILON { continue; }
                let delta = pb - pa;
                let dist = delta.length();
                if dist < EPSILON { continue; }
                let stretch = (dist - rest) / dist;
                // Tearing
                if self.tearing_enabled && dist > tear {
                    self.springs[spring_idx].torn = true;
                    continue;
                }
                let correction = delta * stretch;
                let w_a = ia / sum_inv;
                let w_b = ib / sum_inv;
                self.particles[a].position += correction * w_a;
                self.particles[b].position -= correction * w_b;
            }
        }
    }

    /// Compute vertex normals from mesh connectivity
    pub fn compute_normals(&mut self) {
        for p in &mut self.particles {
            p.normal = Vec3::ZERO;
        }
        let w = self.grid_width;
        let h = self.grid_height;
        let idx = |i: usize, j: usize| j * w + i;
        for j in 0..h - 1 {
            for i in 0..w - 1 {
                let i00 = idx(i, j);
                let i10 = idx(i + 1, j);
                let i01 = idx(i, j + 1);
                let i11 = idx(i + 1, j + 1);
                let p00 = self.particles[i00].position;
                let p10 = self.particles[i10].position;
                let p01 = self.particles[i01].position;
                let p11 = self.particles[i11].position;
                // Triangle 1: 00, 10, 01
                let n1 = (p10 - p00).cross(p01 - p00);
                self.particles[i00].normal += n1;
                self.particles[i10].normal += n1;
                self.particles[i01].normal += n1;
                // Triangle 2: 10, 11, 01
                let n2 = (p11 - p10).cross(p01 - p10);
                self.particles[i10].normal += n2;
                self.particles[i11].normal += n2;
                self.particles[i01].normal += n2;
            }
        }
        for p in &mut self.particles {
            let l = p.normal.length();
            if l > EPSILON { p.normal /= l; }
        }
    }
}

// ============================================================
// FLUID SIMULATION — SPH (Smoothed Particle Hydrodynamics)
// ============================================================

pub const SPH_POLY6_COEFF: f32 = 315.0 / (64.0 * PI);
pub const SPH_SPIKY_COEFF: f32 = 15.0 / PI;
pub const SPH_VISC_COEFF: f32 = 15.0 / (2.0 * PI);

/// Poly6 kernel W(r,h) = (315/64πh^9)*(h^2-r^2)^3 for |r|<=h
pub fn sph_kernel_poly6(r_sq: f32, h: f32) -> f32 {
    let h2 = h * h;
    if r_sq > h2 { return 0.0; }
    let diff = h2 - r_sq;
    SPH_POLY6_COEFF / h.powi(9) * diff * diff * diff
}

/// Gradient of Poly6 kernel
pub fn sph_kernel_poly6_grad(r: Vec3, r_sq: f32, h: f32) -> Vec3 {
    let h2 = h * h;
    if r_sq > h2 { return Vec3::ZERO; }
    let diff = h2 - r_sq;
    let coeff = -6.0 * SPH_POLY6_COEFF / h.powi(9) * diff * diff;
    r * coeff
}

/// Laplacian of Poly6 kernel
pub fn sph_kernel_poly6_lap(r_sq: f32, h: f32) -> f32 {
    let h2 = h * h;
    if r_sq > h2 { return 0.0; }
    let diff = h2 - r_sq;
    -6.0 * SPH_POLY6_COEFF / h.powi(9) * diff * (3.0 * h2 - 7.0 * r_sq)
}

/// Spiky kernel gradient (for pressure) — W_spiky = (15/πh^6)*(h-|r|)^3
pub fn sph_kernel_spiky_grad(r: Vec3, r_len: f32, h: f32) -> Vec3 {
    if r_len > h || r_len < EPSILON { return Vec3::ZERO; }
    let diff = h - r_len;
    let coeff = -3.0 * SPH_SPIKY_COEFF / h.powi(6) * diff * diff / r_len;
    r * coeff
}

/// Viscosity kernel laplacian: W_lap = (15/(2πh^3))*(-(r^3/2h^3) + r^2/h^2 + h/(2r) - 1)
pub fn sph_kernel_viscosity_lap(r_len: f32, h: f32) -> f32 {
    if r_len > h { return 0.0; }
    (45.0 / (PI * h.powi(6))) * (h - r_len)
}

#[derive(Debug, Clone)]
pub struct SphParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub force: Vec3,
    pub density: f32,
    pub pressure: f32,
    pub mass: f32,
}

impl SphParticle {
    pub fn new(pos: Vec3, mass: f32) -> Self {
        Self {
            position: pos,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
            density: 0.0,
            pressure: 0.0,
            mass,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FluidSimulation {
    pub particles: Vec<SphParticle>,
    pub kernel_radius: f32,
    pub rest_density: f32,
    pub viscosity: f32,
    pub pressure_stiffness: f32,
    pub surface_tension: f32,
    pub gravity: Vec3,
    pub restitution: f32,
    pub particle_radius: f32,
    pub neighbor_grid: HashMap<(i32, i32, i32), Vec<usize>>,
    pub domain_min: Vec3,
    pub domain_max: Vec3,
    pub time_scale: f32,
    pub max_velocity: f32,
    pub color_field_threshold: f32,
}

impl FluidSimulation {
    pub fn new(kernel_radius: f32, rest_density: f32) -> Self {
        Self {
            particles: Vec::new(),
            kernel_radius,
            rest_density,
            viscosity: 0.1,
            pressure_stiffness: 200.0,
            surface_tension: 0.0728,
            gravity: Vec3::new(0.0, -GRAVITY, 0.0),
            restitution: 0.0,
            particle_radius: kernel_radius * 0.5,
            neighbor_grid: HashMap::new(),
            domain_min: Vec3::splat(-10.0),
            domain_max: Vec3::splat(10.0),
            time_scale: 1.0,
            max_velocity: 50.0,
            color_field_threshold: 0.6,
        }
    }

    pub fn spawn_block(&mut self, min: Vec3, max: Vec3, spacing: f32, mass: f32) {
        let h = spacing;
        let mut x = min.x;
        while x <= max.x {
            let mut y = min.y;
            while y <= max.y {
                let mut z = min.z;
                while z <= max.z {
                    self.particles.push(SphParticle::new(Vec3::new(x, y, z), mass));
                    z += h;
                }
                y += h;
            }
            x += h;
        }
    }

    fn cell_key(&self, pos: Vec3) -> (i32, i32, i32) {
        let h = self.kernel_radius;
        (
            (pos.x / h).floor() as i32,
            (pos.y / h).floor() as i32,
            (pos.z / h).floor() as i32,
        )
    }

    pub fn build_neighbor_grid(&mut self) {
        self.neighbor_grid.clear();
        for (i, p) in self.particles.iter().enumerate() {
            let key = self.cell_key(p.position);
            self.neighbor_grid.entry(key).or_default().push(i);
        }
    }

    pub fn get_neighbors(&self, pos: Vec3) -> Vec<usize> {
        let mut result = Vec::new();
        let (cx, cy, cz) = self.cell_key(pos);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = self.neighbor_grid.get(&(cx + dx, cy + dy, cz + dz)) {
                        result.extend_from_slice(list);
                    }
                }
            }
        }
        result
    }

    /// Compute density for all particles
    pub fn compute_density(&mut self) {
        let h = self.kernel_radius;
        let n = self.particles.len();
        let positions: Vec<Vec3> = self.particles.iter().map(|p| p.position).collect();
        let masses: Vec<f32> = self.particles.iter().map(|p| p.mass).collect();
        for i in 0..n {
            let mut rho = 0.0f32;
            let neighbors = self.get_neighbors(positions[i]);
            for j in neighbors {
                let r = positions[i] - positions[j];
                let r_sq = r.length_squared();
                rho += masses[j] * sph_kernel_poly6(r_sq, h);
            }
            self.particles[i].density = rho.max(self.rest_density * 0.001);
        }
    }

    /// Compute pressure from density: P = k*(rho - rho_0)
    pub fn compute_pressure(&mut self) {
        for p in &mut self.particles {
            p.pressure = self.pressure_stiffness * ((p.density / self.rest_density) - 1.0).max(0.0);
        }
    }

    /// Compute pressure, viscosity, and surface tension forces
    pub fn compute_forces(&mut self) {
        let h = self.kernel_radius;
        let n = self.particles.len();
        let positions: Vec<Vec3> = self.particles.iter().map(|p| p.position).collect();
        let velocities: Vec<Vec3> = self.particles.iter().map(|p| p.velocity).collect();
        let masses: Vec<f32> = self.particles.iter().map(|p| p.mass).collect();
        let densities: Vec<f32> = self.particles.iter().map(|p| p.density).collect();
        let pressures: Vec<f32> = self.particles.iter().map(|p| p.pressure).collect();
        for i in 0..n {
            let mut f_pressure = Vec3::ZERO;
            let mut f_viscosity = Vec3::ZERO;
            let mut f_surface = Vec3::ZERO;
            let mut color_grad = Vec3::ZERO;
            let mut color_lap = 0.0f32;
            let neighbors = self.get_neighbors(positions[i]);
            for j in neighbors {
                if i == j { continue; }
                let r = positions[i] - positions[j];
                let r_len = r.length();
                let r_sq = r_len * r_len;
                if r_sq >= h * h { continue; }
                let m_j = masses[j];
                let rho_j = densities[j];
                if rho_j < EPSILON { continue; }
                // Pressure force: -sum_j m_j * (P_i/rho_i^2 + P_j/rho_j^2) * grad_W_spiky
                let p_term = pressures[i] / (densities[i] * densities[i])
                    + pressures[j] / (rho_j * rho_j);
                let grad_spiky = sph_kernel_spiky_grad(r, r_len, h);
                f_pressure -= m_j * p_term * grad_spiky;
                // Viscosity force: mu * sum_j m_j * (v_j - v_i)/rho_j * lap_W_visc
                let lap_visc = sph_kernel_viscosity_lap(r_len, h);
                f_viscosity += (velocities[j] - velocities[i]) * (m_j / rho_j * lap_visc);
                // Surface tension (color field)
                let grad_poly6 = sph_kernel_poly6_grad(r, r_sq, h);
                let lap_poly6 = sph_kernel_poly6_lap(r_sq, h);
                color_grad += grad_poly6 * (m_j / rho_j);
                color_lap += lap_poly6 * (m_j / rho_j);
            }
            let rho_i = densities[i];
            f_pressure *= rho_i;
            f_viscosity *= self.viscosity;
            let color_grad_len = color_grad.length();
            if color_grad_len > self.color_field_threshold {
                f_surface = -self.surface_tension * color_lap / color_grad_len * color_grad;
            }
            let gravity_force = self.gravity * masses[i];
            self.particles[i].force = f_pressure + f_viscosity + f_surface + gravity_force;
        }
    }

    /// Integrate SPH particles using Euler
    pub fn integrate(&mut self, dt: f32) {
        let dt_scaled = dt * self.time_scale;
        for p in &mut self.particles {
            if p.density < EPSILON { continue; }
            let acc = p.force / p.density;
            p.velocity += acc * dt_scaled;
            // Clamp
            let v_len = p.velocity.length();
            if v_len > self.max_velocity {
                p.velocity *= self.max_velocity / v_len;
            }
            p.position += p.velocity * dt_scaled;
            // Simple domain boundary bounce
            let r = self.particle_radius;
            for dim in 0..3 {
                let lo = match dim { 0 => self.domain_min.x, 1 => self.domain_min.y, _ => self.domain_min.z };
                let hi = match dim { 0 => self.domain_max.x, 1 => self.domain_max.y, _ => self.domain_max.z };
                let pos_val = match dim { 0 => &mut p.position.x, 1 => &mut p.position.y, _ => &mut p.position.z };
                let vel_val = match dim { 0 => &mut p.velocity.x, 1 => &mut p.velocity.y, _ => &mut p.velocity.z };
                if *pos_val < lo + r {
                    *pos_val = lo + r;
                    *vel_val = vel_val.abs() * self.restitution;
                } else if *pos_val > hi - r {
                    *pos_val = hi - r;
                    *vel_val = -vel_val.abs() * self.restitution;
                }
            }
        }
    }

    /// Full simulation step
    pub fn step(&mut self, dt: f32) {
        self.build_neighbor_grid();
        self.compute_density();
        self.compute_pressure();
        self.compute_forces();
        self.integrate(dt);
    }
}

// ============================================================
// DESTRUCTION / FRACTURE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct VoronoiCell {
    pub seed: Vec3,
    pub vertices: Vec<Vec3>,
    pub mass: f32,
    pub volume: f32,
    pub intact: bool,
    pub stress: f32,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
}

impl VoronoiCell {
    pub fn new(seed: Vec3) -> Self {
        Self {
            seed,
            vertices: Vec::new(),
            mass: 0.0,
            volume: 0.0,
            intact: true,
            stress: 0.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
        }
    }

    /// Compute volume from convex hull vertices (divergence theorem approximation)
    pub fn compute_volume_from_vertices(&mut self) {
        if self.vertices.len() < 4 { self.volume = 0.0; return; }
        // Use centroid-based tetrahedra decomposition
        let centroid = self.vertices.iter().fold(Vec3::ZERO, |a, &v| a + v) / self.vertices.len() as f32;
        let mut vol = 0.0f32;
        let n = self.vertices.len();
        for i in 0..n {
            let a = self.vertices[i];
            let b = self.vertices[(i + 1) % n];
            let c = centroid;
            let d = Vec3::new(centroid.x, centroid.y + 0.01, centroid.z);
            let tet_vol = (b - a).cross(c - a).dot(d - a).abs() / 6.0;
            vol += tet_vol;
        }
        self.volume = vol;
    }
}

#[derive(Debug, Clone)]
pub struct VoronoiFracture {
    pub cells: Vec<VoronoiCell>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub num_cells: usize,
    pub material_strength: f32,     // Pa
    pub toughness: f32,             // J/m^2
    pub density: f32,
    pub cracks: Vec<(usize, usize)>, // cell pairs that are cracked
    pub debris_particles: Vec<DebrisParticle>,
    pub fractured: bool,
}

#[derive(Debug, Clone)]
pub struct DebrisParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
    pub lifetime: f32,
    pub scale: f32,
}

/// Simple LCG random number generator for reproducible fracture
pub struct FractureRng {
    state: u64,
}
impl FractureRng {
    pub fn new(seed: u64) -> Self { Self { state: seed } }
    pub fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let bits = ((self.state >> 33) as u32) | 0x3F80_0000;
        f32::from_bits(bits) - 1.0
    }
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }
}

impl VoronoiFracture {
    pub fn new(bounds_min: Vec3, bounds_max: Vec3, num_cells: usize, seed: u64) -> Self {
        let mut rng = FractureRng::new(seed);
        let mut cells = Vec::new();
        for _ in 0..num_cells {
            let seed_pt = Vec3::new(
                rng.next_range(bounds_min.x, bounds_max.x),
                rng.next_range(bounds_min.y, bounds_max.y),
                rng.next_range(bounds_min.z, bounds_max.z),
            );
            cells.push(VoronoiCell::new(seed_pt));
        }
        Self {
            cells, bounds_min, bounds_max, num_cells,
            material_strength: 1e7,
            toughness: 100.0,
            density: CONCRETE_DENSITY,
            cracks: Vec::new(),
            debris_particles: Vec::new(),
            fractured: false,
        }
    }

    /// For each point, find the nearest Voronoi seed (Lloyd-style cell assignment)
    pub fn find_cell(&self, point: Vec3) -> usize {
        let mut best = 0;
        let mut best_dist = f32::INFINITY;
        for (i, cell) in self.cells.iter().enumerate() {
            let d = (point - cell.seed).length_squared();
            if d < best_dist {
                best_dist = d;
                best = i;
            }
        }
        best
    }

    /// Accumulate stress on a cell given an impulse force
    pub fn accumulate_stress(&mut self, cell_idx: usize, force_magnitude: f32, contact_area: f32) {
        if cell_idx >= self.cells.len() { return; }
        let stress_increment = if contact_area > EPSILON { force_magnitude / contact_area } else { 0.0 };
        self.cells[cell_idx].stress += stress_increment;
    }

    /// Check if any cells exceed strength threshold and initiate fracture
    pub fn check_fracture(&mut self) -> Vec<usize> {
        let mut fractured_cells = Vec::new();
        for (i, cell) in self.cells.iter_mut().enumerate() {
            if cell.intact && cell.stress > self.material_strength {
                cell.intact = false;
                fractured_cells.push(i);
            }
        }
        fractured_cells
    }

    /// Propagate crack from a fractured cell to neighbors
    pub fn propagate_cracks(&mut self, initial_cells: &[usize]) {
        let mut queue: VecDeque<usize> = initial_cells.iter().copied().collect();
        let mut visited: HashSet<usize> = initial_cells.iter().copied().collect();
        while let Some(cell_idx) = queue.pop_front() {
            // Find neighbors (cells within 2x the average cell spacing)
            let seed = self.cells[cell_idx].seed;
            let avg_spacing = (self.bounds_max - self.bounds_min).length() / (self.num_cells as f32).cbrt();
            let crack_radius = avg_spacing * 1.5;
            let stress_here = self.cells[cell_idx].stress;
            for j in 0..self.cells.len() {
                if visited.contains(&j) { continue; }
                let dist = (self.cells[j].seed - seed).length();
                if dist < crack_radius {
                    // Stress transfer (linear fall-off with distance)
                    let transfer = stress_here * (1.0 - dist / crack_radius) * 0.5;
                    self.cells[j].stress += transfer;
                    self.cracks.push((cell_idx, j));
                    if self.cells[j].stress > self.material_strength {
                        self.cells[j].intact = false;
                        visited.insert(j);
                        queue.push_back(j);
                    }
                }
            }
        }
        self.fractured = true;
    }

    /// Spawn debris from fractured cells
    pub fn spawn_debris(&mut self, impact_velocity: Vec3, rng: &mut FractureRng) {
        for cell in &self.cells {
            if !cell.intact {
                let vel = impact_velocity + Vec3::new(
                    rng.next_range(-2.0, 2.0),
                    rng.next_range(1.0, 5.0),
                    rng.next_range(-2.0, 2.0),
                );
                let ang_vel = Vec3::new(
                    rng.next_range(-10.0, 10.0),
                    rng.next_range(-10.0, 10.0),
                    rng.next_range(-10.0, 10.0),
                );
                self.debris_particles.push(DebrisParticle {
                    position: cell.seed,
                    velocity: vel,
                    angular_velocity: ang_vel,
                    mass: cell.mass.max(0.001),
                    lifetime: rng.next_range(2.0, 8.0),
                    scale: rng.next_range(0.05, 0.3),
                });
            }
        }
    }

    /// Integrate debris particles
    pub fn integrate_debris(&mut self, dt: f32) {
        let gravity = Vec3::new(0.0, -GRAVITY, 0.0);
        self.debris_particles.retain_mut(|d| {
            d.velocity += gravity * dt;
            d.position += d.velocity * dt;
            // Simple floor bounce
            if d.position.y < 0.0 {
                d.position.y = 0.0;
                d.velocity.y = d.velocity.y.abs() * 0.4;
                d.velocity.x *= 0.8;
                d.velocity.z *= 0.8;
            }
            d.lifetime -= dt;
            d.lifetime > 0.0
        });
    }

    /// Check if two cells are still connected (no crack between them)
    pub fn are_connected(&self, a: usize, b: usize) -> bool {
        !self.cracks.contains(&(a, b)) && !self.cracks.contains(&(b, a))
    }
}

// ============================================================
// RAGDOLL EDITOR
// ============================================================

#[derive(Debug, Clone)]
pub struct RagdollBone {
    pub name: String,
    pub body_id: u64,
    pub bone_index: u32,
    pub local_offset: Vec3,
    pub local_rotation: Quat,
    pub mass: f32,
    pub shape_type: RigidBodyShapeType,
    pub shape_params: ShapeParameters,
    pub muscle_tone: f32,      // 0 = limp, 1 = fully tensed
    pub blend_weight: f32,     // blend between ragdoll and animation pose
}

#[derive(Debug, Clone)]
pub struct RagdollJoint {
    pub parent_bone: String,
    pub child_bone: String,
    pub constraint_id: u64,
    pub constraint_type: String,
    pub swing_limit: f32,
    pub twist_limit: f32,
    pub stiffness: f32,
    pub damping: f32,
}

#[derive(Debug, Clone)]
pub struct RagdollEditor {
    pub bones: HashMap<String, RagdollBone>,
    pub joints: Vec<RagdollJoint>,
    pub blend_mode: RagdollBlendMode,
    pub animation_blend: f32,   // 0 = full ragdoll, 1 = full animation
    pub blend_time: f32,
    pub current_blend_timer: f32,
    pub active: bool,
    pub total_mass: f32,
    pub com_world: Vec3,
    pub next_body_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RagdollBlendMode {
    FullRagdoll,
    FullAnimation,
    BlendedPhysics,
    KinematicDriven,
}

impl RagdollEditor {
    pub fn new() -> Self {
        Self {
            bones: HashMap::new(),
            joints: Vec::new(),
            blend_mode: RagdollBlendMode::FullRagdoll,
            animation_blend: 0.0,
            blend_time: 0.3,
            current_blend_timer: 0.0,
            active: false,
            total_mass: 0.0,
            com_world: Vec3::ZERO,
            next_body_id: 1000,
        }
    }

    pub fn build_humanoid_skeleton(&mut self) {
        // --- Torso/Spine ---
        self.add_bone("pelvis", 0, Vec3::ZERO, Quat::IDENTITY, 10.0, RigidBodyShapeType::Box,
            ShapeParameters { half_extents: Vec3::new(0.15, 0.1, 0.08), ..Default::default() });
        self.add_bone("spine", 1, Vec3::new(0.0, 0.2, 0.0), Quat::IDENTITY, 8.0, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.08, half_height: 0.1, ..Default::default() });
        self.add_bone("chest", 2, Vec3::new(0.0, 0.4, 0.0), Quat::IDENTITY, 12.0, RigidBodyShapeType::Box,
            ShapeParameters { half_extents: Vec3::new(0.18, 0.12, 0.09), ..Default::default() });
        self.add_bone("neck", 3, Vec3::new(0.0, 0.55, 0.0), Quat::IDENTITY, 1.5, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.04, half_height: 0.04, ..Default::default() });
        self.add_bone("head", 4, Vec3::new(0.0, 0.65, 0.0), Quat::IDENTITY, 5.0, RigidBodyShapeType::Sphere,
            ShapeParameters { radius: 0.12, ..Default::default() });
        // --- Left arm ---
        self.add_bone("l_shoulder", 5, Vec3::new(0.22, 0.5, 0.0), Quat::IDENTITY, 2.0, RigidBodyShapeType::Sphere,
            ShapeParameters { radius: 0.04, ..Default::default() });
        self.add_bone("l_upper_arm", 6, Vec3::new(0.3, 0.45, 0.0), Quat::IDENTITY, 2.5, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.04, half_height: 0.15, ..Default::default() });
        self.add_bone("l_forearm", 7, Vec3::new(0.45, 0.3, 0.0), Quat::IDENTITY, 1.5, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.03, half_height: 0.12, ..Default::default() });
        self.add_bone("l_hand", 8, Vec3::new(0.55, 0.18, 0.0), Quat::IDENTITY, 0.5, RigidBodyShapeType::Box,
            ShapeParameters { half_extents: Vec3::new(0.04, 0.03, 0.06), ..Default::default() });
        // --- Right arm ---
        self.add_bone("r_shoulder", 9, Vec3::new(-0.22, 0.5, 0.0), Quat::IDENTITY, 2.0, RigidBodyShapeType::Sphere,
            ShapeParameters { radius: 0.04, ..Default::default() });
        self.add_bone("r_upper_arm", 10, Vec3::new(-0.3, 0.45, 0.0), Quat::IDENTITY, 2.5, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.04, half_height: 0.15, ..Default::default() });
        self.add_bone("r_forearm", 11, Vec3::new(-0.45, 0.3, 0.0), Quat::IDENTITY, 1.5, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.03, half_height: 0.12, ..Default::default() });
        self.add_bone("r_hand", 12, Vec3::new(-0.55, 0.18, 0.0), Quat::IDENTITY, 0.5, RigidBodyShapeType::Box,
            ShapeParameters { half_extents: Vec3::new(0.04, 0.03, 0.06), ..Default::default() });
        // --- Left leg ---
        self.add_bone("l_thigh", 13, Vec3::new(0.1, -0.1, 0.0), Quat::IDENTITY, 7.0, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.06, half_height: 0.22, ..Default::default() });
        self.add_bone("l_shin", 14, Vec3::new(0.1, -0.5, 0.0), Quat::IDENTITY, 4.0, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.04, half_height: 0.2, ..Default::default() });
        self.add_bone("l_foot", 15, Vec3::new(0.1, -0.85, 0.04), Quat::IDENTITY, 1.0, RigidBodyShapeType::Box,
            ShapeParameters { half_extents: Vec3::new(0.04, 0.03, 0.1), ..Default::default() });
        // --- Right leg ---
        self.add_bone("r_thigh", 16, Vec3::new(-0.1, -0.1, 0.0), Quat::IDENTITY, 7.0, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.06, half_height: 0.22, ..Default::default() });
        self.add_bone("r_shin", 17, Vec3::new(-0.1, -0.5, 0.0), Quat::IDENTITY, 4.0, RigidBodyShapeType::Capsule,
            ShapeParameters { radius: 0.04, half_height: 0.2, ..Default::default() });
        self.add_bone("r_foot", 18, Vec3::new(-0.1, -0.85, 0.04), Quat::IDENTITY, 1.0, RigidBodyShapeType::Box,
            ShapeParameters { half_extents: Vec3::new(0.04, 0.03, 0.1), ..Default::default() });
        // Build joints
        self.build_spine_chain();
        self.build_arm_chain("l");
        self.build_arm_chain("r");
        self.build_leg_chain("l");
        self.build_leg_chain("r");
        self.compute_total_mass();
    }

    fn add_bone(&mut self, name: &str, bone_index: u32, offset: Vec3, rot: Quat,
                mass: f32, shape: RigidBodyShapeType, params: ShapeParameters) {
        let id = self.next_body_id;
        self.next_body_id += 1;
        self.bones.insert(name.to_string(), RagdollBone {
            name: name.to_string(),
            body_id: id,
            bone_index,
            local_offset: offset,
            local_rotation: rot,
            mass,
            shape_type: shape,
            shape_params: params,
            muscle_tone: 0.0,
            blend_weight: 1.0,
        });
    }

    fn build_spine_chain(&mut self) {
        self.add_joint("pelvis", "spine", 45.0_f32.to_radians(), 30.0_f32.to_radians(), 100.0, 10.0);
        self.add_joint("spine", "chest", 30.0_f32.to_radians(), 20.0_f32.to_radians(), 150.0, 15.0);
        self.add_joint("chest", "neck", 40.0_f32.to_radians(), 30.0_f32.to_radians(), 80.0, 8.0);
        self.add_joint("neck", "head", 50.0_f32.to_radians(), 60.0_f32.to_radians(), 60.0, 6.0);
    }

    fn build_arm_chain(&mut self, side: &str) {
        let s = side;
        let shoulder = format!("{}_shoulder", s);
        let upper_arm = format!("{}_upper_arm", s);
        let forearm = format!("{}_forearm", s);
        let hand = format!("{}_hand", s);
        let chest_key = "chest";
        self.add_joint(chest_key, &shoulder, 60.0_f32.to_radians(), 45.0_f32.to_radians(), 120.0, 12.0);
        self.add_joint(&shoulder, &upper_arm, 90.0_f32.to_radians(), 60.0_f32.to_radians(), 100.0, 10.0);
        self.add_joint(&upper_arm, &forearm, 120.0_f32.to_radians(), 5.0_f32.to_radians(), 80.0, 8.0);
        self.add_joint(&forearm, &hand, 70.0_f32.to_radians(), 30.0_f32.to_radians(), 40.0, 4.0);
    }

    fn build_leg_chain(&mut self, side: &str) {
        let s = side;
        let thigh = format!("{}_thigh", s);
        let shin = format!("{}_shin", s);
        let foot = format!("{}_foot", s);
        self.add_joint("pelvis", &thigh, 80.0_f32.to_radians(), 45.0_f32.to_radians(), 200.0, 20.0);
        self.add_joint(&thigh, &shin, 130.0_f32.to_radians(), 5.0_f32.to_radians(), 160.0, 16.0);
        self.add_joint(&shin, &foot, 50.0_f32.to_radians(), 20.0_f32.to_radians(), 60.0, 6.0);
    }

    fn add_joint(&mut self, parent: &str, child: &str, swing: f32, twist: f32, stiffness: f32, damping: f32) {
        let id = self.joints.len() as u64 + 2000;
        self.joints.push(RagdollJoint {
            parent_bone: parent.to_string(),
            child_bone: child.to_string(),
            constraint_id: id,
            constraint_type: "ConeTwist".to_string(),
            swing_limit: swing,
            twist_limit: twist,
            stiffness,
            damping,
        });
    }

    pub fn compute_total_mass(&mut self) {
        self.total_mass = self.bones.values().map(|b| b.mass).sum();
    }

    /// Blend bone transforms between ragdoll physics and animation
    /// Returns blended world transform for each bone
    pub fn compute_blended_transforms(&self,
        physics_poses: &HashMap<String, (Vec3, Quat)>,
        animation_poses: &HashMap<String, (Vec3, Quat)>,
    ) -> HashMap<String, (Vec3, Quat)> {
        let mut result = HashMap::new();
        let blend = self.animation_blend;
        for (name, bone) in &self.bones {
            let phys = physics_poses.get(name).copied().unwrap_or((Vec3::ZERO, Quat::IDENTITY));
            let anim = animation_poses.get(name).copied().unwrap_or((Vec3::ZERO, Quat::IDENTITY));
            let w = blend * bone.blend_weight;
            let pos = phys.0.lerp(anim.0, w);
            let rot = phys.1.slerp(anim.1, w);
            result.insert(name.clone(), (pos, rot));
        }
        result
    }

    /// Compute muscle spring torque for a joint given current angle
    pub fn muscle_torque(&self, joint: &RagdollJoint, bone_name: &str, current_angle: f32, target_angle: f32) -> f32 {
        let bone = match self.bones.get(bone_name) {
            Some(b) => b,
            None => return 0.0,
        };
        let tone = bone.muscle_tone;
        let error = target_angle - current_angle;
        tone * joint.stiffness * error
    }

    pub fn set_blend_mode(&mut self, mode: RagdollBlendMode, blend_time: f32) {
        self.blend_mode = mode.clone();
        self.blend_time = blend_time;
        self.current_blend_timer = 0.0;
        match mode {
            RagdollBlendMode::FullRagdoll => self.animation_blend = 0.0,
            RagdollBlendMode::FullAnimation => self.animation_blend = 1.0,
            _ => {}
        }
    }

    pub fn update_blend(&mut self, dt: f32) {
        self.current_blend_timer = (self.current_blend_timer + dt).min(self.blend_time);
        let t = if self.blend_time > EPSILON { self.current_blend_timer / self.blend_time } else { 1.0 };
        match self.blend_mode {
            RagdollBlendMode::BlendedPhysics => {
                self.animation_blend = smoothstep(0.0, 1.0, t) * 0.5;
            }
            RagdollBlendMode::FullAnimation => {
                self.animation_blend = smoothstep(0.0, 1.0, t);
            }
            RagdollBlendMode::FullRagdoll => {
                self.animation_blend = 1.0 - smoothstep(0.0, 1.0, t);
            }
            _ => {}
        }
    }
}

// ============================================================
// VEHICLE PHYSICS
// ============================================================

/// Pacejka Magic Formula tire model
/// F = D * sin(C * atan(B*alpha - E*(B*alpha - atan(B*alpha))))
pub fn pacejka_magic_formula(slip_angle_rad: f32, b: f32, c: f32, d: f32, e: f32) -> f32 {
    let ba = b * slip_angle_rad;
    let inner = ba - e * (ba - ba.atan());
    d * (c * inner.atan()).sin()
}

/// Pacejka coefficients for typical dry asphalt
pub fn pacejka_dry_asphalt() -> (f32, f32, f32, f32) {
    // B=10, C=1.9, D=1.0, E=0.97 (lateral force coefficients)
    (10.0, 1.9, 1.0, 0.97)
}

/// Pacejka coefficients for wet road
pub fn pacejka_wet_road() -> (f32, f32, f32, f32) {
    (7.0, 1.7, 0.7, 0.9)
}

/// Pacejka coefficients for ice
pub fn pacejka_ice() -> (f32, f32, f32, f32) {
    (4.0, 1.5, 0.2, 0.8)
}

/// Compute Ackermann steering angles for inner and outer wheel
/// L = wheelbase, W = track width, delta = steering angle (outer)
pub fn ackermann_steering(wheelbase: f32, track_width: f32, steer_angle: f32) -> (f32, f32) {
    // Outer wheel angle = steer_angle (input)
    // Inner wheel angle = atan(L / (L/tan(delta) - W/2))
    let l = wheelbase;
    let w = track_width;
    let delta = steer_angle;
    if delta.abs() < EPSILON {
        return (0.0, 0.0);
    }
    let r_outer = l / delta.tan();
    let r_inner = r_outer - w;
    let inner_angle = (l / r_inner).atan();
    (delta, inner_angle)
}

#[derive(Debug, Clone)]
pub struct WheelState {
    pub position_local: Vec3,   // Attachment point in vehicle local space
    pub radius: f32,
    pub width: f32,
    pub suspension_rest_length: f32,
    pub suspension_max_travel: f32,
    pub suspension_stiffness: f32,
    pub suspension_damping: f32,
    pub current_compression: f32,
    pub suspension_force: f32,
    pub contact_point: Vec3,
    pub contact_normal: Vec3,
    pub is_grounded: bool,
    pub slip_angle: f32,
    pub slip_ratio: f32,
    pub lateral_force: f32,
    pub longitudinal_force: f32,
    pub steer_angle: f32,
    pub angular_velocity: f32,  // rad/s
    pub brake_torque: f32,
    pub drive_torque: f32,
    pub friction_limit: f32,
    pub rolling_resistance: f32,
    pub camber_angle: f32,
    pub road_surface: SurfaceType,
}

impl WheelState {
    pub fn new(position_local: Vec3, radius: f32) -> Self {
        Self {
            position_local,
            radius,
            width: 0.2,
            suspension_rest_length: 0.3,
            suspension_max_travel: 0.15,
            suspension_stiffness: 25000.0,
            suspension_damping: 2000.0,
            current_compression: 0.0,
            suspension_force: 0.0,
            contact_point: Vec3::ZERO,
            contact_normal: Vec3::Y,
            is_grounded: false,
            slip_angle: 0.0,
            slip_ratio: 0.0,
            lateral_force: 0.0,
            longitudinal_force: 0.0,
            steer_angle: 0.0,
            angular_velocity: 0.0,
            brake_torque: 0.0,
            drive_torque: 0.0,
            friction_limit: 1.0,
            rolling_resistance: 0.015,
            camber_angle: 0.0,
            road_surface: SurfaceType::Asphalt,
        }
    }

    /// Compute suspension force using spring-damper model
    /// F_susp = k * compression + c * compression_velocity
    pub fn compute_suspension_force(&mut self, compression_velocity: f32) -> f32 {
        let spring_force = self.suspension_stiffness * self.current_compression;
        let damper_force = self.suspension_damping * compression_velocity;
        let force = (spring_force + damper_force).max(0.0);
        self.suspension_force = force;
        force
    }

    /// Compute tire forces using Pacejka model
    pub fn compute_tire_forces(&mut self, normal_force: f32, surface_friction: f32) {
        if !self.is_grounded || normal_force < EPSILON {
            self.lateral_force = 0.0;
            self.longitudinal_force = 0.0;
            return;
        }
        let (b, c, d, e) = match self.road_surface {
            SurfaceType::Asphalt => pacejka_dry_asphalt(),
            SurfaceType::WetAsphalt | SurfaceType::Mud => pacejka_wet_road(),
            SurfaceType::Ice | SurfaceType::Snow => pacejka_ice(),
            _ => pacejka_dry_asphalt(),
        };
        // Scale D by normal force and surface friction
        let d_scaled = d * normal_force * surface_friction;
        // Lateral force from slip angle
        self.lateral_force = pacejka_magic_formula(self.slip_angle, b, c, d_scaled, e);
        // Longitudinal force from slip ratio (using same formula)
        let long_slip = self.slip_ratio;
        self.longitudinal_force = pacejka_magic_formula(long_slip, b * 1.2, c * 1.1, d_scaled, e);
        // Friction circle: combine longitudinal and lateral
        let combined = (self.lateral_force * self.lateral_force + self.longitudinal_force * self.longitudinal_force).sqrt();
        let limit = d_scaled * self.friction_limit;
        if combined > limit && combined > EPSILON {
            let scale = limit / combined;
            self.lateral_force *= scale;
            self.longitudinal_force *= scale;
        }
    }

    /// Compute slip angle from wheel velocity in local frame
    pub fn compute_slip_angle(&mut self, wheel_vel_local: Vec3) {
        let vx = wheel_vel_local.x;
        let vy = wheel_vel_local.z; // lateral in vehicle z
        if vx.abs() < 0.5 {
            self.slip_angle = 0.0;
            return;
        }
        self.slip_angle = (vy / vx.abs()).atan();
    }

    /// Compute longitudinal slip ratio
    pub fn compute_slip_ratio(&mut self, vehicle_speed: f32) {
        let wheel_speed = self.angular_velocity * self.radius;
        let v_ref = vehicle_speed.abs().max(0.1);
        self.slip_ratio = (wheel_speed - vehicle_speed) / v_ref;
        self.slip_ratio = self.slip_ratio.clamp(-1.0, 1.0);
    }
}

#[derive(Debug, Clone)]
pub struct EngineTorqueCurve {
    /// RPM values
    pub rpm_points: Vec<f32>,
    /// Torque values in Nm at each RPM point
    pub torque_points: Vec<f32>,
    pub idle_rpm: f32,
    pub max_rpm: f32,
    pub redline_rpm: f32,
    pub current_rpm: f32,
    pub inertia: f32,
}

impl EngineTorqueCurve {
    pub fn petrol_sport() -> Self {
        // Typical sport petrol engine: peak torque ~300Nm at 4500rpm
        let rpm = vec![0.0, 1000.0, 2000.0, 3000.0, 4000.0, 4500.0, 5500.0, 6500.0, 7000.0, 7500.0];
        let torque = vec![0.0, 180.0, 250.0, 280.0, 295.0, 300.0, 290.0, 260.0, 220.0, 0.0];
        Self {
            rpm_points: rpm,
            torque_points: torque,
            idle_rpm: 800.0,
            max_rpm: 7500.0,
            redline_rpm: 7000.0,
            current_rpm: 800.0,
            inertia: 0.15,
        }
    }

    pub fn diesel_truck() -> Self {
        let rpm = vec![0.0, 800.0, 1200.0, 1600.0, 2000.0, 2500.0, 3000.0, 3500.0, 4000.0];
        let torque = vec![0.0, 400.0, 800.0, 1000.0, 1100.0, 1100.0, 950.0, 700.0, 0.0];
        Self {
            rpm_points: rpm,
            torque_points: torque,
            idle_rpm: 600.0,
            max_rpm: 4000.0,
            redline_rpm: 3500.0,
            current_rpm: 600.0,
            inertia: 0.5,
        }
    }

    /// Evaluate torque at given RPM using linear interpolation
    pub fn evaluate(&self, rpm: f32) -> f32 {
        if self.rpm_points.is_empty() { return 0.0; }
        let rpm = rpm.clamp(0.0, *self.rpm_points.last().unwrap());
        for i in 0..self.rpm_points.len() - 1 {
            if rpm >= self.rpm_points[i] && rpm <= self.rpm_points[i + 1] {
                let t = (rpm - self.rpm_points[i]) / (self.rpm_points[i + 1] - self.rpm_points[i] + EPSILON);
                return self.torque_points[i] + t * (self.torque_points[i + 1] - self.torque_points[i]);
            }
        }
        0.0
    }

    /// Compute engine RPM from wheel angular velocity through gearbox
    pub fn update_rpm(&mut self, wheel_ang_vel: f32, gear_ratio: f32, final_drive: f32) {
        let new_rpm = (wheel_ang_vel * gear_ratio * final_drive * 60.0 / TWO_PI).abs();
        self.current_rpm = new_rpm.clamp(self.idle_rpm, self.max_rpm);
    }
}

#[derive(Debug, Clone)]
pub struct GearBox {
    pub gear_ratios: Vec<f32>,   // index 0 = reverse, 1 = 1st, 2 = 2nd...
    pub current_gear: i32,       // -1 = reverse, 0 = neutral, 1..n = forward
    pub final_drive_ratio: f32,
    pub auto_shift: bool,
    pub shift_up_rpm: f32,
    pub shift_down_rpm: f32,
    pub shift_time: f32,
    pub shifting_timer: f32,
    pub efficiency: f32,
}

impl GearBox {
    pub fn new() -> Self {
        Self {
            gear_ratios: vec![-3.5, 3.82, 2.36, 1.68, 1.31, 1.00, 0.78],
            current_gear: 1,
            final_drive_ratio: 3.73,
            auto_shift: false,
            shift_up_rpm: 6000.0,
            shift_down_rpm: 2500.0,
            shift_time: 0.2,
            shifting_timer: 0.0,
            efficiency: 0.92,
        }
    }

    pub fn current_ratio(&self) -> f32 {
        if self.current_gear < 0 {
            self.gear_ratios[0]
        } else if self.current_gear == 0 {
            0.0
        } else {
            let idx = (self.current_gear as usize).min(self.gear_ratios.len() - 1);
            self.gear_ratios[idx]
        }
    }

    pub fn output_torque(&self, engine_torque: f32) -> f32 {
        engine_torque * self.current_ratio() * self.final_drive_ratio * self.efficiency
    }

    pub fn auto_shift_logic(&mut self, current_rpm: f32) {
        if !self.auto_shift || self.shifting_timer > 0.0 { return; }
        if current_rpm > self.shift_up_rpm && self.current_gear < (self.gear_ratios.len() as i32 - 1) {
            self.current_gear += 1;
            self.shifting_timer = self.shift_time;
        } else if current_rpm < self.shift_down_rpm && self.current_gear > 1 {
            self.current_gear -= 1;
            self.shifting_timer = self.shift_time;
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.shifting_timer > 0.0 {
            self.shifting_timer = (self.shifting_timer - dt).max(0.0);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DifferentialType {
    Open,
    Locked,
    LimitedSlip { preload_torque: f32, ramp_angle_accel: f32, ramp_angle_decel: f32 },
    Torsen { worm_gear_ratio: f32 },
    Electronic { target_diff: f32, max_transfer: f32 },
}

#[derive(Debug, Clone)]
pub struct Differential {
    pub diff_type: DifferentialType,
    pub left_torque: f32,
    pub right_torque: f32,
    pub left_rpm: f32,
    pub right_rpm: f32,
}

impl Differential {
    pub fn new(diff_type: DifferentialType) -> Self {
        Self { diff_type, left_torque: 0.0, right_torque: 0.0, left_rpm: 0.0, right_rpm: 0.0 }
    }

    /// Distribute torque between two wheels based on differential type
    pub fn distribute(&mut self, input_torque: f32) {
        match &self.diff_type {
            DifferentialType::Open => {
                // Open diff: equal torque distribution
                self.left_torque = input_torque * 0.5;
                self.right_torque = input_torque * 0.5;
            }
            DifferentialType::Locked => {
                // Locked: equal torque, equal speed forced by constraint
                self.left_torque = input_torque * 0.5;
                self.right_torque = input_torque * 0.5;
            }
            DifferentialType::LimitedSlip { preload_torque, ramp_angle_accel, ramp_angle_decel } => {
                let speed_diff = (self.left_rpm - self.right_rpm).abs();
                let lock_torque = *preload_torque + speed_diff * ramp_angle_accel.tan();
                let base = input_torque * 0.5;
                let transfer = lock_torque.min(base.abs());
                if self.left_rpm > self.right_rpm {
                    self.left_torque = base - transfer;
                    self.right_torque = base + transfer;
                } else {
                    self.left_torque = base + transfer;
                    self.right_torque = base - transfer;
                }
            }
            DifferentialType::Torsen { worm_gear_ratio } => {
                // Torsen: speed-sensitive, based on worm gear ratio
                let speed_diff = (self.left_rpm - self.right_rpm).abs();
                let torque_ratio = 1.0 + speed_diff * worm_gear_ratio;
                let t = input_torque;
                self.left_torque = t / (1.0 + 1.0 / torque_ratio);
                self.right_torque = t / (1.0 + torque_ratio);
            }
            DifferentialType::Electronic { target_diff, max_transfer } => {
                let speed_diff = self.left_rpm - self.right_rpm;
                let transfer = (speed_diff * target_diff).clamp(-*max_transfer, *max_transfer);
                let base = input_torque * 0.5;
                self.left_torque = base + transfer;
                self.right_torque = base - transfer;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VehiclePhysics {
    pub body_id: u64,
    pub wheels: Vec<WheelState>,
    pub engine: EngineTorqueCurve,
    pub gearbox: GearBox,
    pub front_diff: Differential,
    pub rear_diff: Differential,
    pub center_diff: Option<Differential>,
    pub wheelbase: f32,
    pub track_width_front: f32,
    pub track_width_rear: f32,
    pub center_of_mass: Vec3,
    pub mass: f32,
    pub aerodynamic_drag: f32,   // Cd * A
    pub downforce_coefficient: f32,
    pub brake_bias_front: f32,
    pub abs_enabled: bool,
    pub tcs_enabled: bool,
    pub esc_enabled: bool,
    pub throttle: f32,
    pub brake: f32,
    pub steering: f32,
    pub handbrake: bool,
}

impl VehiclePhysics {
    pub fn new(mass: f32, wheelbase: f32, track_width: f32) -> Self {
        // 4 wheels: FL, FR, RL, RR
        let hw = track_width * 0.5;
        let hl = wheelbase * 0.5;
        let wheels = vec![
            WheelState::new(Vec3::new(hw, 0.0, hl), 0.33),   // FL
            WheelState::new(Vec3::new(-hw, 0.0, hl), 0.33),  // FR
            WheelState::new(Vec3::new(hw, 0.0, -hl), 0.33),  // RL
            WheelState::new(Vec3::new(-hw, 0.0, -hl), 0.33), // RR
        ];
        Self {
            body_id: 0,
            wheels,
            engine: EngineTorqueCurve::petrol_sport(),
            gearbox: GearBox::new(),
            front_diff: Differential::new(DifferentialType::Open),
            rear_diff: Differential::new(DifferentialType::LimitedSlip {
                preload_torque: 50.0, ramp_angle_accel: 30.0_f32.to_radians(), ramp_angle_decel: 50.0_f32.to_radians()
            }),
            center_diff: None,
            wheelbase,
            track_width_front: track_width,
            track_width_rear: track_width,
            center_of_mass: Vec3::new(0.0, 0.3, 0.0),
            mass,
            aerodynamic_drag: 0.3 * 2.2, // Cd=0.3, A=2.2m^2
            downforce_coefficient: 0.0,
            brake_bias_front: 0.6,
            abs_enabled: true,
            tcs_enabled: true,
            esc_enabled: false,
            throttle: 0.0,
            brake: 0.0,
            steering: 0.0,
            handbrake: false,
        }
    }

    pub fn apply_steering(&mut self) {
        let max_steer = 35.0_f32.to_radians();
        let steer_rad = self.steering * max_steer;
        let (delta_out, delta_in) = ackermann_steering(self.wheelbase, self.track_width_front, steer_rad);
        // Apply to FL and FR (indices 0, 1)
        if steer_rad > 0.0 {
            self.wheels[0].steer_angle = delta_out;
            self.wheels[1].steer_angle = delta_in;
        } else {
            self.wheels[0].steer_angle = delta_in;
            self.wheels[1].steer_angle = delta_out;
        }
    }

    /// Compute total aerodynamic drag force given vehicle speed (m/s)
    pub fn aerodynamic_drag_force(&self, speed: f32) -> f32 {
        0.5 * AIR_DENSITY * self.aerodynamic_drag * speed * speed
    }

    /// Compute downforce at given speed
    pub fn downforce(&self, speed: f32) -> f32 {
        0.5 * AIR_DENSITY * self.downforce_coefficient * speed * speed
    }

    /// ABS: prevent wheel lock-up
    pub fn abs_update(&mut self, vehicle_speed: f32) {
        if !self.abs_enabled { return; }
        for wheel in &mut self.wheels {
            if !wheel.is_grounded { continue; }
            // If wheel is locking (slip ratio < -0.2), reduce brake torque
            if wheel.slip_ratio < -0.2 {
                wheel.brake_torque *= 0.7;
            }
        }
    }

    /// TCS: prevent wheel spin
    pub fn tcs_update(&mut self) {
        if !self.tcs_enabled { return; }
        for wheel in &mut self.wheels {
            if !wheel.is_grounded { continue; }
            if wheel.slip_ratio > 0.2 {
                wheel.drive_torque *= 0.7;
            }
        }
    }

    pub fn update(&mut self, dt: f32, vehicle_speed: f32) {
        // Engine
        let wheel_ang_vel = if vehicle_speed.abs() > 0.1 { vehicle_speed / self.wheels[2].radius } else { 0.0 };
        self.engine.update_rpm(wheel_ang_vel, self.gearbox.current_ratio().abs(), self.gearbox.final_drive_ratio);
        self.gearbox.auto_shift_logic(self.engine.current_rpm);
        self.gearbox.update(dt);
        let engine_torque = self.engine.evaluate(self.engine.current_rpm) * self.throttle;
        let output_torque = self.gearbox.output_torque(engine_torque);
        // Distribute to rear wheels
        self.rear_diff.left_rpm = self.wheels[2].angular_velocity * 60.0 / TWO_PI;
        self.rear_diff.right_rpm = self.wheels[3].angular_velocity * 60.0 / TWO_PI;
        self.rear_diff.distribute(output_torque);
        self.wheels[2].drive_torque = self.rear_diff.left_torque;
        self.wheels[3].drive_torque = self.rear_diff.right_torque;
        // Braking
        let total_brake_torque = self.brake * 5000.0;
        let front_brake = total_brake_torque * self.brake_bias_front;
        let rear_brake = total_brake_torque * (1.0 - self.brake_bias_front);
        self.wheels[0].brake_torque = front_brake * 0.5;
        self.wheels[1].brake_torque = front_brake * 0.5;
        let rear_brake_actual = if self.handbrake { total_brake_torque } else { rear_brake * 0.5 };
        self.wheels[2].brake_torque = rear_brake_actual;
        self.wheels[3].brake_torque = rear_brake_actual;
        // Steering
        self.apply_steering();
        // Tire model
        let wheel_load = self.mass * GRAVITY / 4.0; // Simplified equal distribution
        for wheel in &mut self.wheels {
            if wheel.is_grounded {
                let surf_friction = surface_friction_coefficient(wheel.road_surface.clone(), false);
                wheel.compute_slip_ratio(vehicle_speed);
                wheel.compute_tire_forces(wheel.suspension_force.max(wheel_load), surf_friction);
            }
        }
        self.abs_update(vehicle_speed);
        self.tcs_update();
    }
}

// ============================================================
// COLLISION SHAPE EDITOR
// ============================================================

#[derive(Debug, Clone)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self { Self { min, max } }

    pub fn from_points(points: &[Vec3]) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &p in points {
            min = min.min(p);
            max = max.max(p);
        }
        Self { min, max }
    }

    pub fn center(&self) -> Vec3 { (self.min + self.max) * 0.5 }
    pub fn half_extents(&self) -> Vec3 { (self.max - self.min) * 0.5 }
    pub fn surface_area(&self) -> f32 {
        let e = self.max - self.min;
        2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }
    pub fn volume(&self) -> f32 {
        let e = self.max - self.min;
        (e.x * e.y * e.z).max(0.0)
    }
    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.max.x >= other.min.x && self.min.x <= other.max.x &&
        self.max.y >= other.min.y && self.min.y <= other.max.y &&
        self.max.z >= other.min.z && self.min.z <= other.max.z
    }
    pub fn expand(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
    pub fn merged(&self, other: &Aabb) -> Aabb {
        Aabb::new(self.min.min(other.min), self.max.max(other.max))
    }
}

#[derive(Debug, Clone)]
pub struct Obb {
    pub center: Vec3,
    pub axes: [Vec3; 3],   // orthonormal local axes
    pub half_extents: Vec3,
}

impl Obb {
    pub fn new(center: Vec3, axes: [Vec3; 3], half_extents: Vec3) -> Self {
        Self { center, axes, half_extents }
    }

    /// Project OBB onto axis n, return [min, max] interval
    pub fn project(&self, n: Vec3) -> (f32, f32) {
        let c = self.center.dot(n);
        let r = self.half_extents.x * self.axes[0].dot(n).abs()
            + self.half_extents.y * self.axes[1].dot(n).abs()
            + self.half_extents.z * self.axes[2].dot(n).abs();
        (c - r, c + r)
    }

    /// SAT-based OBB vs OBB overlap test
    pub fn overlaps(&self, other: &Obb) -> bool {
        let test_axes: Vec<Vec3> = {
            let mut v = Vec::new();
            for &a in &self.axes { v.push(a); }
            for &b in &other.axes { v.push(b); }
            for &a in &self.axes {
                for &b in &other.axes {
                    let cross = a.cross(b);
                    if cross.length_squared() > EPSILON * EPSILON {
                        v.push(cross.normalize());
                    }
                }
            }
            v
        };
        for axis in test_axes {
            let (a_min, a_max) = self.project(axis);
            let (b_min, b_max) = other.project(axis);
            if a_max < b_min || b_max < a_min { return false; }
        }
        true
    }

    pub fn to_aabb(&self) -> Aabb {
        let mut corners = Vec::new();
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let pt = self.center
                        + self.axes[0] * (self.half_extents.x * sx)
                        + self.axes[1] * (self.half_extents.y * sy)
                        + self.axes[2] * (self.half_extents.z * sz);
                    corners.push(pt);
                }
            }
        }
        Aabb::from_points(&corners)
    }
}

/// Fit an AABB to a set of triangles
pub fn fit_aabb_to_triangles(triangles: &[(Vec3, Vec3, Vec3)]) -> Aabb {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &(a, b, c) in triangles {
        min = min.min(a).min(b).min(c);
        max = max.max(a).max(b).max(c);
    }
    Aabb::new(min, max)
}

/// Fit an OBB to a point cloud using PCA approximation
/// Finds principal axes from covariance matrix (3x3 symmetric Jacobi)
pub fn fit_obb_pca(points: &[Vec3]) -> Obb {
    if points.is_empty() { return Obb::new(Vec3::ZERO, [Vec3::X, Vec3::Y, Vec3::Z], Vec3::splat(0.5)); }
    let n = points.len() as f32;
    let mean = points.iter().fold(Vec3::ZERO, |a, &b| a + b) / n;
    // Build covariance matrix
    let mut cov = [[0.0f32; 3]; 3];
    for &p in points {
        let d = p - mean;
        let dv = [d.x, d.y, d.z];
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += dv[i] * dv[j];
            }
        }
    }
    for i in 0..3 { for j in 0..3 { cov[i][j] /= n; } }
    // Jacobi eigendecomposition (symmetric 3x3)
    let (eigenvalues, eigenvectors) = jacobi_eigen_3x3(cov);
    // Sort by eigenvalue descending
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| eigenvalues[b].partial_cmp(&eigenvalues[a]).unwrap_or(std::cmp::Ordering::Equal));
    let ax0 = eigenvectors[order[0]];
    let ax1 = eigenvectors[order[1]];
    let ax2 = ax0.cross(ax1).normalize(); // ensure right-handed
    // Project points onto axes to find extents
    let mut mins = [f32::INFINITY; 3];
    let mut maxs = [f32::NEG_INFINITY; 3];
    let axes_arr = [ax0, ax1, ax2];
    for &p in points {
        for k in 0..3 {
            let proj = (p - mean).dot(axes_arr[k]);
            if proj < mins[k] { mins[k] = proj; }
            if proj > maxs[k] { maxs[k] = proj; }
        }
    }
    let he = Vec3::new(
        (maxs[0] - mins[0]) * 0.5,
        (maxs[1] - mins[1]) * 0.5,
        (maxs[2] - mins[2]) * 0.5,
    );
    let center = mean + ax0 * (mins[0] + maxs[0]) * 0.5
        + ax1 * (mins[1] + maxs[1]) * 0.5
        + ax2 * (mins[2] + maxs[2]) * 0.5;
    Obb::new(center, [ax0, ax1, ax2], he)
}

/// Simple Jacobi eigendecomposition for symmetric 3x3 matrix
/// Returns (eigenvalues, eigenvectors as Vec3)
pub fn jacobi_eigen_3x3(mut a: [[f32; 3]; 3]) -> ([f32; 3], [Vec3; 3]) {
    let mut v = [[0.0f32; 3]; 3];
    for i in 0..3 { v[i][i] = 1.0; }
    for _ in 0..50 {
        // Find largest off-diagonal element
        let mut max_val = 0.0f32;
        let (mut p, mut q) = (0, 1);
        for i in 0..3 {
            for j in i + 1..3 {
                if a[i][j].abs() > max_val {
                    max_val = a[i][j].abs();
                    p = i; q = j;
                }
            }
        }
        if max_val < 1e-7 { break; }
        // Compute rotation angle
        let theta = if (a[q][q] - a[p][p]).abs() < EPSILON {
            HALF_PI * 0.25
        } else {
            0.5 * (2.0 * a[p][q] / (a[q][q] - a[p][p])).atan()
        };
        let c = theta.cos();
        let s = theta.sin();
        // Apply Jacobi rotation
        let app = a[p][p]; let aqq = a[q][q]; let apq = a[p][q];
        a[p][p] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
        a[q][q] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
        a[p][q] = 0.0; a[q][p] = 0.0;
        for k in 0..3 {
            if k != p && k != q {
                let aip = a[k][p]; let aiq = a[k][q];
                a[k][p] = c * aip - s * aiq;
                a[p][k] = a[k][p];
                a[k][q] = s * aip + c * aiq;
                a[q][k] = a[k][q];
            }
        }
        for k in 0..3 {
            let vkp = v[k][p]; let vkq = v[k][q];
            v[k][p] = c * vkp - s * vkq;
            v[k][q] = s * vkp + c * vkq;
        }
    }
    let eigenvalues = [a[0][0], a[1][1], a[2][2]];
    let ev0 = Vec3::new(v[0][0], v[1][0], v[2][0]).normalize();
    let ev1 = Vec3::new(v[0][1], v[1][1], v[2][1]).normalize();
    let ev2 = Vec3::new(v[0][2], v[1][2], v[2][2]).normalize();
    (eigenvalues, [ev0, ev1, ev2])
}

/// Compute convex hull (gift wrapping / Jarvis march) in 2D projection
/// Then extrude in 3D for approximate convex hull vertices
pub fn compute_convex_hull_2d(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() < 3 { return points.to_vec(); }
    let mut hull = Vec::new();
    // Find leftmost point
    let start = points.iter().enumerate().min_by(|(_, a), (_, b)| {
        a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
    }).map(|(i, _)| i).unwrap_or(0);
    let mut current = start;
    loop {
        hull.push(points[current]);
        let mut next = (current + 1) % points.len();
        for i in 0..points.len() {
            let cross = cross2d(points[next] - points[current], points[i] - points[current]);
            if cross < 0.0 { next = i; }
        }
        current = next;
        if current == start { break; }
        if hull.len() > points.len() { break; } // safety
    }
    hull
}

fn cross2d(a: Vec2, b: Vec2) -> f32 { a.x * b.y - a.y * b.x }

#[derive(Debug, Clone)]
pub struct HeightfieldShape {
    pub width: usize,
    pub depth: usize,
    pub heights: Vec<f32>,
    pub scale: Vec3,
    pub min_height: f32,
    pub max_height: f32,
    pub up_axis: u8, // 0=X, 1=Y, 2=Z
}

impl HeightfieldShape {
    pub fn new(width: usize, depth: usize, scale: Vec3) -> Self {
        let heights = vec![0.0; width * depth];
        Self {
            width, depth, heights,
            scale,
            min_height: 0.0, max_height: 0.0,
            up_axis: 1,
        }
    }

    pub fn set_height(&mut self, x: usize, z: usize, h: f32) {
        if x < self.width && z < self.depth {
            self.heights[z * self.width + x] = h;
            if h < self.min_height { self.min_height = h; }
            if h > self.max_height { self.max_height = h; }
        }
    }

    pub fn get_height(&self, x: usize, z: usize) -> f32 {
        if x < self.width && z < self.depth {
            self.heights[z * self.width + x]
        } else {
            0.0
        }
    }

    pub fn height_at_world(&self, world_x: f32, world_z: f32) -> f32 {
        let lx = world_x / self.scale.x;
        let lz = world_z / self.scale.z;
        let ix = lx as usize;
        let iz = lz as usize;
        if ix + 1 >= self.width || iz + 1 >= self.depth { return 0.0; }
        let tx = lx - ix as f32;
        let tz = lz - iz as f32;
        let h00 = self.get_height(ix, iz);
        let h10 = self.get_height(ix + 1, iz);
        let h01 = self.get_height(ix, iz + 1);
        let h11 = self.get_height(ix + 1, iz + 1);
        // Bilinear interpolation
        let h0 = h00 * (1.0 - tx) + h10 * tx;
        let h1 = h01 * (1.0 - tx) + h11 * tx;
        (h0 * (1.0 - tz) + h1 * tz) * self.scale.y
    }

    pub fn compute_aabb(&self) -> Aabb {
        Aabb::new(
            Vec3::ZERO,
            Vec3::new(
                (self.width - 1) as f32 * self.scale.x,
                self.max_height * self.scale.y,
                (self.depth - 1) as f32 * self.scale.z,
            ),
        )
    }
}

#[derive(Debug, Clone)]
pub struct CollisionShapeEditor {
    pub shape_type: CollisionShapeType,
    pub box_half_extents: Vec3,
    pub sphere_radius: f32,
    pub capsule_radius: f32,
    pub capsule_half_height: f32,
    pub cylinder_radius: f32,
    pub cylinder_half_height: f32,
    pub convex_hull_points: Vec<Vec3>,
    pub triangle_mesh: Vec<(Vec3, Vec3, Vec3)>,
    pub heightfield: Option<HeightfieldShape>,
    pub aabb: Aabb,
    pub obb: Option<Obb>,
    pub mass_properties_dirty: bool,
    pub volume: f32,
    pub surface_area: f32,
    pub margin: f32,        // collision margin (GJK/EPA)
    pub local_transform: Mat4,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CollisionShapeType {
    Box,
    Sphere,
    Capsule,
    Cylinder,
    ConvexHull,
    TriangleMesh,
    Heightfield,
    Compound,
}

impl CollisionShapeEditor {
    pub fn new() -> Self {
        Self {
            shape_type: CollisionShapeType::Box,
            box_half_extents: Vec3::splat(0.5),
            sphere_radius: 0.5,
            capsule_radius: 0.25,
            capsule_half_height: 0.5,
            cylinder_radius: 0.25,
            cylinder_half_height: 0.5,
            convex_hull_points: Vec::new(),
            triangle_mesh: Vec::new(),
            heightfield: None,
            aabb: Aabb::new(Vec3::splat(-0.5), Vec3::splat(0.5)),
            obb: None,
            mass_properties_dirty: true,
            volume: 0.0,
            surface_area: 0.0,
            margin: 0.01,
            local_transform: Mat4::IDENTITY,
        }
    }

    pub fn compute_aabb(&mut self) {
        self.aabb = match self.shape_type {
            CollisionShapeType::Box => {
                let he = self.box_half_extents;
                Aabb::new(-he, he)
            }
            CollisionShapeType::Sphere => {
                let r = self.sphere_radius;
                Aabb::new(Vec3::splat(-r), Vec3::splat(r))
            }
            CollisionShapeType::Capsule => {
                let r = self.capsule_radius;
                let hh = self.capsule_half_height;
                Aabb::new(Vec3::new(-r, -(hh + r), -r), Vec3::new(r, hh + r, r))
            }
            CollisionShapeType::Cylinder => {
                let r = self.cylinder_radius;
                let hh = self.cylinder_half_height;
                Aabb::new(Vec3::new(-r, -hh, -r), Vec3::new(r, hh, r))
            }
            CollisionShapeType::ConvexHull | CollisionShapeType::TriangleMesh => {
                Aabb::from_points(&self.convex_hull_points)
            }
            CollisionShapeType::Heightfield => {
                if let Some(ref hf) = self.heightfield {
                    hf.compute_aabb()
                } else {
                    Aabb::new(Vec3::ZERO, Vec3::ZERO)
                }
            }
            CollisionShapeType::Compound => self.aabb.clone(),
        };
    }

    pub fn compute_volume(&mut self) {
        self.volume = match self.shape_type {
            CollisionShapeType::Box => {
                let e = self.box_half_extents * 2.0;
                e.x * e.y * e.z
            }
            CollisionShapeType::Sphere => {
                (4.0 / 3.0) * PI * self.sphere_radius.powi(3)
            }
            CollisionShapeType::Capsule => {
                let r = self.capsule_radius;
                let h = self.capsule_half_height * 2.0;
                PI * r * r * h + (4.0 / 3.0) * PI * r * r * r
            }
            CollisionShapeType::Cylinder => {
                let r = self.cylinder_radius;
                let h = self.cylinder_half_height * 2.0;
                PI * r * r * h
            }
            CollisionShapeType::ConvexHull => {
                // Approximate volume from AABB (convex hull is always <= AABB volume)
                self.aabb.volume() * 0.7
            }
            _ => self.aabb.volume(),
        };
    }

    pub fn compute_surface_area(&mut self) {
        self.surface_area = match self.shape_type {
            CollisionShapeType::Box => {
                let e = self.box_half_extents * 2.0;
                2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
            }
            CollisionShapeType::Sphere => {
                4.0 * PI * self.sphere_radius * self.sphere_radius
            }
            CollisionShapeType::Capsule => {
                let r = self.capsule_radius;
                let h = self.capsule_half_height * 2.0;
                TWO_PI * r * (h + 2.0 * r)
            }
            CollisionShapeType::Cylinder => {
                let r = self.cylinder_radius;
                let h = self.cylinder_half_height * 2.0;
                TWO_PI * r * (h + r)
            }
            _ => self.aabb.surface_area(),
        };
    }

    pub fn fit_to_point_cloud(&mut self, points: &[Vec3]) {
        self.convex_hull_points = points.to_vec();
        self.obb = Some(fit_obb_pca(points));
        self.compute_aabb();
    }

    pub fn generate_box_wireframe(&self) -> Vec<(Vec3, Vec3)> {
        let he = match self.shape_type {
            CollisionShapeType::Box => self.box_half_extents,
            _ => self.aabb.half_extents(),
        };
        let c = Vec3::ZERO;
        let corners = [
            Vec3::new(-he.x, -he.y, -he.z), Vec3::new(he.x, -he.y, -he.z),
            Vec3::new(he.x, he.y, -he.z),   Vec3::new(-he.x, he.y, -he.z),
            Vec3::new(-he.x, -he.y, he.z),  Vec3::new(he.x, -he.y, he.z),
            Vec3::new(he.x, he.y, he.z),    Vec3::new(-he.x, he.y, he.z),
        ];
        let edges = [
            (0,1),(1,2),(2,3),(3,0), // bottom
            (4,5),(5,6),(6,7),(7,4), // top
            (0,4),(1,5),(2,6),(3,7), // verticals
        ];
        edges.iter().map(|&(a, b)| (corners[a], corners[b])).collect()
    }

    pub fn generate_sphere_wireframe(&self, segments: usize) -> Vec<(Vec3, Vec3)> {
        let r = self.sphere_radius;
        let mut lines = Vec::new();
        for plane in 0..3 {
            let mut prev = Vec3::ZERO;
            for i in 0..=segments {
                let t = TWO_PI * i as f32 / segments as f32;
                let c = t.cos() * r;
                let s = t.sin() * r;
                let pt = match plane {
                    0 => Vec3::new(c, s, 0.0),
                    1 => Vec3::new(c, 0.0, s),
                    _ => Vec3::new(0.0, c, s),
                };
                if i > 0 { lines.push((prev, pt)); }
                prev = pt;
            }
        }
        lines
    }
}

// ============================================================
// PHYSICS MATERIAL LIBRARY
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SurfaceType {
    Asphalt,
    WetAsphalt,
    Concrete,
    WetConcrete,
    Gravel,
    Dirt,
    Mud,
    Sand,
    Snow,
    Ice,
    Grass,
    WetGrass,
    Rubber,
    Metal,
    WetMetal,
    Wood,
    WetWood,
    Stone,
    WetStone,
    Carpet,
    Fabric,
    Leather,
    Glass,
    WetGlass,
    Ceramic,
    Marble,
    Granite,
    Foam,
    Cork,
    Plastic,
    HardPlastic,
    SoftPlastic,
    ToughRubber,
    Silicon,
    Wax,
    Oil,
    WaterSurface,
    DeepWater,
    Mud_Thick,
    Clay,
    Chalk,
    Coal,
    Sandstone,
    Limestone,
    Brick,
    WetBrick,
    TarmacRoad,
    DirtRoad,
    SnowRoad,
    Forest,
    RockFace,
    Custom,
}

#[derive(Debug, Clone)]
pub struct PhysicsMaterial {
    pub name: String,
    pub surface_type: SurfaceType,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub rolling_friction: f32,
    pub restitution: f32,        // coefficient of restitution (bounciness)
    pub density: f32,            // kg/m^3
    pub young_modulus: f32,      // Pa
    pub poisson_ratio: f32,
    pub hardness: f32,           // Vickers hardness (HV)
    pub thermal_conductivity: f32, // W/(m·K)
    pub sound_damping: f32,      // audio parameter
}

impl PhysicsMaterial {
    pub fn new(name: &str, surface_type: SurfaceType,
               sf: f32, df: f32, rf: f32, rest: f32, density: f32) -> Self {
        Self {
            name: name.to_string(),
            surface_type,
            static_friction: sf,
            dynamic_friction: df,
            rolling_friction: rf,
            restitution: rest,
            density,
            young_modulus: 1e9,
            poisson_ratio: 0.3,
            hardness: 100.0,
            thermal_conductivity: 1.0,
            sound_damping: 0.5,
        }
    }
}

pub fn surface_friction_coefficient(surface: SurfaceType, wet: bool) -> f32 {
    match (surface, wet) {
        (SurfaceType::Asphalt, false) => 0.8,
        (SurfaceType::Asphalt, true) | (SurfaceType::WetAsphalt, _) => 0.5,
        (SurfaceType::Concrete, false) => 0.75,
        (SurfaceType::Concrete, true) | (SurfaceType::WetConcrete, _) => 0.45,
        (SurfaceType::Gravel, _) => 0.6,
        (SurfaceType::Dirt, false) => 0.55,
        (SurfaceType::Dirt, true) => 0.4,
        (SurfaceType::Mud, _) | (SurfaceType::Mud_Thick, _) => 0.3,
        (SurfaceType::Sand, _) => 0.45,
        (SurfaceType::Snow, _) => 0.2,
        (SurfaceType::Ice, _) => 0.05,
        (SurfaceType::Grass, false) => 0.5,
        (SurfaceType::Grass, true) | (SurfaceType::WetGrass, _) => 0.35,
        (SurfaceType::Rubber, _) | (SurfaceType::ToughRubber, _) => 0.9,
        (SurfaceType::Metal, false) => 0.4,
        (SurfaceType::Metal, true) | (SurfaceType::WetMetal, _) => 0.2,
        (SurfaceType::Wood, false) => 0.4,
        (SurfaceType::Wood, true) | (SurfaceType::WetWood, _) => 0.25,
        (SurfaceType::Stone, false) | (SurfaceType::Granite, _) | (SurfaceType::Marble, _) => 0.6,
        (SurfaceType::Stone, true) | (SurfaceType::WetStone, _) => 0.35,
        (SurfaceType::Carpet, _) => 0.7,
        (SurfaceType::Fabric, _) => 0.6,
        (SurfaceType::Leather, _) => 0.5,
        (SurfaceType::Glass, false) => 0.4,
        (SurfaceType::Glass, true) | (SurfaceType::WetGlass, _) => 0.1,
        (SurfaceType::Ceramic, _) => 0.55,
        (SurfaceType::Foam, _) => 0.55,
        (SurfaceType::Cork, _) => 0.7,
        (SurfaceType::Plastic, _) | (SurfaceType::SoftPlastic, _) => 0.35,
        (SurfaceType::HardPlastic, _) => 0.3,
        (SurfaceType::Silicon, _) => 0.8,
        (SurfaceType::Wax, _) => 0.05,
        (SurfaceType::Oil, _) => 0.03,
        (SurfaceType::WaterSurface, _) | (SurfaceType::DeepWater, _) => 0.01,
        (SurfaceType::Clay, _) => 0.45,
        (SurfaceType::Chalk, _) => 0.65,
        (SurfaceType::Coal, _) => 0.4,
        (SurfaceType::Sandstone, _) => 0.55,
        (SurfaceType::Limestone, _) => 0.6,
        (SurfaceType::Brick, false) => 0.65,
        (SurfaceType::Brick, true) | (SurfaceType::WetBrick, _) => 0.4,
        (SurfaceType::TarmacRoad, _) => 0.75,
        (SurfaceType::DirtRoad, _) => 0.5,
        (SurfaceType::SnowRoad, _) => 0.2,
        (SurfaceType::Forest, _) => 0.5,
        (SurfaceType::RockFace, _) => 0.65,
        _ => DEFAULT_FRICTION,
    }
}

pub fn build_material_library() -> HashMap<String, PhysicsMaterial> {
    let mut lib = HashMap::new();
    let add = |lib: &mut HashMap<String, PhysicsMaterial>, m: PhysicsMaterial| {
        lib.insert(m.name.clone(), m);
    };
    // Static, dynamic, rolling, restitution, density
    add(&mut lib, PhysicsMaterial::new("Asphalt", SurfaceType::Asphalt, 0.9, 0.8, 0.02, 0.1, 2300.0));
    add(&mut lib, PhysicsMaterial::new("Wet Asphalt", SurfaceType::WetAsphalt, 0.55, 0.5, 0.03, 0.1, 2300.0));
    add(&mut lib, PhysicsMaterial::new("Concrete", SurfaceType::Concrete, 0.8, 0.75, 0.02, 0.15, CONCRETE_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Wet Concrete", SurfaceType::WetConcrete, 0.5, 0.45, 0.025, 0.15, CONCRETE_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Gravel", SurfaceType::Gravel, 0.65, 0.6, 0.04, 0.2, 1650.0));
    add(&mut lib, PhysicsMaterial::new("Dirt", SurfaceType::Dirt, 0.6, 0.55, 0.04, 0.2, 1600.0));
    add(&mut lib, PhysicsMaterial::new("Mud", SurfaceType::Mud, 0.35, 0.3, 0.05, 0.05, 1800.0));
    add(&mut lib, PhysicsMaterial::new("Sand", SurfaceType::Sand, 0.5, 0.45, 0.06, 0.1, SAND_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Snow", SurfaceType::Snow, 0.25, 0.2, 0.04, 0.05, 200.0));
    add(&mut lib, PhysicsMaterial::new("Ice", SurfaceType::Ice, 0.07, 0.05, 0.01, 0.02, ICE_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Grass", SurfaceType::Grass, 0.55, 0.5, 0.04, 0.2, 1200.0));
    add(&mut lib, PhysicsMaterial::new("Wet Grass", SurfaceType::WetGrass, 0.4, 0.35, 0.05, 0.1, 1200.0));
    add(&mut lib, PhysicsMaterial::new("Rubber", SurfaceType::Rubber, 1.0, 0.9, 0.015, 0.8, RUBBER_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Metal", SurfaceType::Metal, 0.45, 0.4, 0.01, 0.3, STEEL_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Wet Metal", SurfaceType::WetMetal, 0.25, 0.2, 0.01, 0.3, STEEL_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Steel", SurfaceType::Metal, 0.5, 0.45, 0.01, 0.35, STEEL_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Aluminum", SurfaceType::Metal, 0.42, 0.38, 0.01, 0.3, ALUMINUM_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Copper", SurfaceType::Metal, 0.48, 0.44, 0.01, 0.3, COPPER_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Gold", SurfaceType::Metal, 0.35, 0.3, 0.01, 0.3, GOLD_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Wood", SurfaceType::Wood, 0.45, 0.4, 0.02, 0.35, WOOD_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Wet Wood", SurfaceType::WetWood, 0.3, 0.25, 0.025, 0.35, WOOD_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Stone", SurfaceType::Stone, 0.65, 0.6, 0.015, 0.2, 2600.0));
    add(&mut lib, PhysicsMaterial::new("Granite", SurfaceType::Granite, 0.68, 0.62, 0.015, 0.18, 2700.0));
    add(&mut lib, PhysicsMaterial::new("Marble", SurfaceType::Marble, 0.55, 0.5, 0.015, 0.25, 2700.0));
    add(&mut lib, PhysicsMaterial::new("Glass", SurfaceType::Glass, 0.45, 0.4, 0.01, 0.65, GLASS_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Wet Glass", SurfaceType::WetGlass, 0.15, 0.1, 0.005, 0.65, GLASS_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Ceramic", SurfaceType::Ceramic, 0.6, 0.55, 0.015, 0.4, 2400.0));
    add(&mut lib, PhysicsMaterial::new("Carpet", SurfaceType::Carpet, 0.75, 0.7, 0.03, 0.05, 300.0));
    add(&mut lib, PhysicsMaterial::new("Fabric", SurfaceType::Fabric, 0.65, 0.6, 0.03, 0.1, 200.0));
    add(&mut lib, PhysicsMaterial::new("Leather", SurfaceType::Leather, 0.55, 0.5, 0.02, 0.3, 900.0));
    add(&mut lib, PhysicsMaterial::new("Foam", SurfaceType::Foam, 0.6, 0.55, 0.03, 0.05, 30.0));
    add(&mut lib, PhysicsMaterial::new("Cork", SurfaceType::Cork, 0.75, 0.7, 0.03, 0.5, 200.0));
    add(&mut lib, PhysicsMaterial::new("Plastic", SurfaceType::Plastic, 0.4, 0.35, 0.01, 0.4, 950.0));
    add(&mut lib, PhysicsMaterial::new("Hard Plastic", SurfaceType::HardPlastic, 0.35, 0.3, 0.01, 0.45, 1100.0));
    add(&mut lib, PhysicsMaterial::new("Soft Plastic", SurfaceType::SoftPlastic, 0.55, 0.5, 0.015, 0.2, 900.0));
    add(&mut lib, PhysicsMaterial::new("Silicone", SurfaceType::Silicon, 0.85, 0.8, 0.015, 0.5, 1100.0));
    add(&mut lib, PhysicsMaterial::new("Wax", SurfaceType::Wax, 0.07, 0.05, 0.005, 0.15, 900.0));
    add(&mut lib, PhysicsMaterial::new("Brick", SurfaceType::Brick, 0.7, 0.65, 0.02, 0.15, 1800.0));
    add(&mut lib, PhysicsMaterial::new("Wet Brick", SurfaceType::WetBrick, 0.45, 0.4, 0.025, 0.1, 1800.0));
    add(&mut lib, PhysicsMaterial::new("Clay", SurfaceType::Clay, 0.5, 0.45, 0.04, 0.1, 1500.0));
    add(&mut lib, PhysicsMaterial::new("Chalk", SurfaceType::Chalk, 0.7, 0.65, 0.02, 0.1, 2000.0));
    add(&mut lib, PhysicsMaterial::new("Coal", SurfaceType::Coal, 0.45, 0.4, 0.02, 0.2, 1400.0));
    add(&mut lib, PhysicsMaterial::new("Sandstone", SurfaceType::Sandstone, 0.6, 0.55, 0.02, 0.15, 2200.0));
    add(&mut lib, PhysicsMaterial::new("Limestone", SurfaceType::Limestone, 0.65, 0.6, 0.015, 0.15, 2300.0));
    add(&mut lib, PhysicsMaterial::new("Water Surface", SurfaceType::WaterSurface, 0.02, 0.01, 0.001, 0.0, WATER_DENSITY));
    add(&mut lib, PhysicsMaterial::new("Thick Mud", SurfaceType::Mud_Thick, 0.4, 0.35, 0.06, 0.02, 2000.0));
    add(&mut lib, PhysicsMaterial::new("Tarmac Road", SurfaceType::TarmacRoad, 0.8, 0.75, 0.02, 0.1, 2300.0));
    add(&mut lib, PhysicsMaterial::new("Dirt Road", SurfaceType::DirtRoad, 0.55, 0.5, 0.04, 0.15, 1600.0));
    add(&mut lib, PhysicsMaterial::new("Snow Road", SurfaceType::SnowRoad, 0.25, 0.2, 0.03, 0.05, 300.0));
    lib
}

// ============================================================
// CONTACT VISUALIZER
// ============================================================

#[derive(Debug, Clone)]
pub struct ContactPoint {
    pub world_position: Vec3,
    pub world_normal: Vec3,
    pub penetration_depth: f32,
    pub impulse: Vec3,
    pub normal_impulse: f32,
    pub tangent_impulse1: f32,
    pub tangent_impulse2: f32,
    pub body_a: u64,
    pub body_b: u64,
    pub lifetime: f32,  // frames remaining
    pub warm_impulse: f32,
}

impl ContactPoint {
    pub fn new(pos: Vec3, normal: Vec3, depth: f32, body_a: u64, body_b: u64) -> Self {
        Self {
            world_position: pos,
            world_normal: normal,
            penetration_depth: depth,
            impulse: Vec3::ZERO,
            normal_impulse: 0.0,
            tangent_impulse1: 0.0,
            tangent_impulse2: 0.0,
            body_a, body_b,
            lifetime: 3.0,
            warm_impulse: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContactVisualizer {
    pub contacts: Vec<ContactPoint>,
    pub show_normals: bool,
    pub show_impulses: bool,
    pub show_penetration: bool,
    pub normal_length: f32,
    pub impulse_scale: f32,
    pub contact_sphere_radius: f32,
    pub color_by_depth: bool,
    pub color_no_contact: Vec4,
    pub color_shallow: Vec4,
    pub color_deep: Vec4,
    pub max_display_contacts: usize,
}

impl ContactVisualizer {
    pub fn new() -> Self {
        Self {
            contacts: Vec::new(),
            show_normals: true,
            show_impulses: true,
            show_penetration: true,
            normal_length: 0.1,
            impulse_scale: 0.001,
            contact_sphere_radius: 0.02,
            color_by_depth: true,
            color_no_contact: Vec4::new(0.0, 1.0, 0.0, 1.0),
            color_shallow: Vec4::new(1.0, 1.0, 0.0, 1.0),
            color_deep: Vec4::new(1.0, 0.0, 0.0, 1.0),
            max_display_contacts: 256,
        }
    }

    pub fn add_contact(&mut self, contact: ContactPoint) {
        if self.contacts.len() < self.max_display_contacts {
            self.contacts.push(contact);
        }
    }

    pub fn update(&mut self, dt: f32) {
        for c in &mut self.contacts {
            c.lifetime -= dt * 60.0;
        }
        self.contacts.retain(|c| c.lifetime > 0.0);
    }

    pub fn clear(&mut self) {
        self.contacts.clear();
    }

    /// Compute color for a contact point based on penetration depth
    pub fn depth_color(&self, depth: f32, max_depth: f32) -> Vec4 {
        if !self.color_by_depth {
            return self.color_shallow;
        }
        let t = (depth / max_depth.max(EPSILON)).clamp(0.0, 1.0);
        Vec4::new(
            self.color_shallow.x + (self.color_deep.x - self.color_shallow.x) * t,
            self.color_shallow.y + (self.color_deep.y - self.color_shallow.y) * t,
            self.color_shallow.z + (self.color_deep.z - self.color_shallow.z) * t,
            1.0,
        )
    }

    /// Generate debug lines for all contacts
    pub fn generate_debug_lines(&self) -> Vec<(Vec3, Vec3, Vec4)> {
        let mut lines = Vec::new();
        let max_depth = self.contacts.iter().map(|c| c.penetration_depth).fold(0.0f32, f32::max).max(0.001);
        for contact in &self.contacts {
            let alpha = (contact.lifetime / 3.0).clamp(0.0, 1.0);
            let mut col = self.depth_color(contact.penetration_depth, max_depth);
            col.w = alpha;
            if self.show_normals {
                let end = contact.world_position + contact.world_normal * self.normal_length;
                lines.push((contact.world_position, end, col));
            }
            if self.show_impulses && contact.normal_impulse > EPSILON {
                let imp_end = contact.world_position + contact.world_normal * contact.normal_impulse * self.impulse_scale;
                lines.push((contact.world_position, imp_end, Vec4::new(0.0, 0.5, 1.0, alpha)));
            }
            if self.show_penetration {
                let pen_end = contact.world_position - contact.world_normal * contact.penetration_depth;
                lines.push((contact.world_position, pen_end, Vec4::new(1.0, 0.5, 0.0, alpha)));
            }
        }
        lines
    }

    /// Generate sphere positions for contact point indicators
    pub fn contact_sphere_positions(&self) -> Vec<(Vec3, Vec4, f32)> {
        let max_depth = self.contacts.iter().map(|c| c.penetration_depth).fold(0.001f32, f32::max);
        self.contacts.iter().map(|c| {
            let alpha = (c.lifetime / 3.0).clamp(0.0, 1.0);
            let mut col = self.depth_color(c.penetration_depth, max_depth);
            col.w = alpha;
            (c.world_position, col, self.contact_sphere_radius)
        }).collect()
    }

    pub fn stats(&self) -> ContactStats {
        let total = self.contacts.len();
        let max_depth = self.contacts.iter().map(|c| c.penetration_depth).fold(0.0f32, f32::max);
        let avg_depth = if total > 0 {
            self.contacts.iter().map(|c| c.penetration_depth).sum::<f32>() / total as f32
        } else { 0.0 };
        let total_impulse = self.contacts.iter().map(|c| c.normal_impulse).sum::<f32>();
        ContactStats { total, max_depth, avg_depth, total_impulse }
    }
}

#[derive(Debug, Clone)]
pub struct ContactStats {
    pub total: usize,
    pub max_depth: f32,
    pub avg_depth: f32,
    pub total_impulse: f32,
}

// ============================================================
// BROAD PHASE — Sweep and Prune (SAP)
// ============================================================

#[derive(Debug, Clone)]
pub struct SapEndpoint {
    pub value: f32,
    pub is_min: bool,
    pub body_id: u64,
}

#[derive(Debug, Clone)]
pub struct BroadPhase {
    pub x_endpoints: Vec<SapEndpoint>,
    pub y_endpoints: Vec<SapEndpoint>,
    pub z_endpoints: Vec<SapEndpoint>,
    pub active_pairs: HashSet<(u64, u64)>,
    pub aabbs: HashMap<u64, Aabb>,
}

impl BroadPhase {
    pub fn new() -> Self {
        Self {
            x_endpoints: Vec::new(),
            y_endpoints: Vec::new(),
            z_endpoints: Vec::new(),
            active_pairs: HashSet::new(),
            aabbs: HashMap::new(),
        }
    }

    pub fn update_aabb(&mut self, body_id: u64, aabb: Aabb) {
        self.aabbs.insert(body_id, aabb);
    }

    pub fn compute_overlapping_pairs(&self) -> Vec<(u64, u64)> {
        let mut result = Vec::new();
        let bodies: Vec<u64> = self.aabbs.keys().copied().collect();
        let n = bodies.len();
        for i in 0..n {
            for j in i + 1..n {
                let a = bodies[i]; let b = bodies[j];
                if let (Some(aa), Some(ab)) = (self.aabbs.get(&a), self.aabbs.get(&b)) {
                    if aa.intersects(ab) {
                        let pair = if a < b { (a, b) } else { (b, a) };
                        result.push(pair);
                    }
                }
            }
        }
        result
    }
}

// ============================================================
// NARROW PHASE — GJK / EPA helpers
// ============================================================

/// GJK support function for a convex shape (sphere)
pub fn support_sphere(center: Vec3, radius: f32, dir: Vec3) -> Vec3 {
    let d = dir.normalize();
    center + d * radius
}

/// GJK support function for a box
pub fn support_box(half_extents: Vec3, dir: Vec3) -> Vec3 {
    Vec3::new(
        half_extents.x * dir.x.signum(),
        half_extents.y * dir.y.signum(),
        half_extents.z * dir.z.signum(),
    )
}

/// Minkowski difference support function
pub fn support_minkowski(
    pa: Vec3, ha: Vec3,
    pb: Vec3, hb: Vec3,
    dir: Vec3,
) -> Vec3 {
    let sa = support_box(ha, dir) + pa;
    let sb = support_box(hb, -dir) + pb;
    sa - sb
}

/// Dot of two vectors — for clarity
fn dot(a: Vec3, b: Vec3) -> f32 { a.dot(b) }

/// Simple GJK (line simplex and triangle tests inline)
pub fn gjk_intersect_box_box(
    pos_a: Vec3, half_a: Vec3,
    pos_b: Vec3, half_b: Vec3,
) -> bool {
    // Separating Axis Theorem for two OBBs (axis-aligned in this simplified version)
    let diff = pos_b - pos_a;
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    for &ax in &axes {
        let proj_a = half_a.dot(ax.abs());
        let proj_b = half_b.dot(ax.abs());
        let dist = diff.dot(ax).abs();
        if dist > proj_a + proj_b { return false; }
    }
    true
}

/// EPA (Expanding Polytope Algorithm) — compute penetration depth and normal
/// Simplified version for sphere vs sphere
pub fn epa_sphere_sphere(
    center_a: Vec3, radius_a: f32,
    center_b: Vec3, radius_b: f32,
) -> Option<(Vec3, f32)> {
    let diff = center_b - center_a;
    let dist = diff.length();
    let overlap = radius_a + radius_b - dist;
    if overlap < 0.0 { return None; }
    let normal = if dist > EPSILON { diff / dist } else { Vec3::Y };
    Some((normal, overlap))
}

// ============================================================
// PHYSICS WORLD EDITOR — integrating everything
// ============================================================

#[derive(Debug, Clone)]
pub struct PhysicsWorldSettings {
    pub gravity: Vec3,
    pub default_linear_damping: f32,
    pub default_angular_damping: f32,
    pub default_restitution: f32,
    pub default_friction: f32,
    pub solver_iterations: u32,
    pub solver_velocity_iterations: u32,
    pub baumgarte_factor: f32,
    pub sleep_enabled: bool,
    pub fixed_timestep: f32,
    pub max_substeps: u32,
    pub broadphase_type: BroadphaseType,
    pub warm_starting: bool,
    pub continuous_collision: bool,
    pub split_impulses: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BroadphaseType {
    BruteForce,
    SweepAndPrune,
    DynamicAabbTree,
}

impl Default for PhysicsWorldSettings {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -GRAVITY, 0.0),
            default_linear_damping: 0.01,
            default_angular_damping: 0.05,
            default_restitution: DEFAULT_RESTITUTION,
            default_friction: DEFAULT_FRICTION,
            solver_iterations: 10,
            solver_velocity_iterations: 10,
            baumgarte_factor: 0.2,
            sleep_enabled: true,
            fixed_timestep: 1.0 / 60.0,
            max_substeps: 4,
            broadphase_type: BroadphaseType::SweepAndPrune,
            warm_starting: true,
            continuous_collision: false,
            split_impulses: true,
        }
    }
}

// ============================================================
// PHYSICS DEBUG OVERLAY
// ============================================================

#[derive(Debug, Clone)]
pub struct PhysicsDebugOverlay {
    pub show_colliders: bool,
    pub show_contacts: bool,
    pub show_joints: bool,
    pub show_velocities: bool,
    pub show_forces: bool,
    pub show_center_of_mass: bool,
    pub show_inertia: bool,
    pub show_sleep_state: bool,
    pub show_broadphase: bool,
    pub collider_color: Vec4,
    pub contact_color: Vec4,
    pub velocity_color: Vec4,
    pub force_color: Vec4,
    pub com_color: Vec4,
    pub sleeping_color: Vec4,
    pub velocity_scale: f32,
    pub force_scale: f32,
    pub line_width: f32,
}

impl Default for PhysicsDebugOverlay {
    fn default() -> Self {
        Self {
            show_colliders: true,
            show_contacts: true,
            show_joints: true,
            show_velocities: false,
            show_forces: false,
            show_center_of_mass: true,
            show_inertia: false,
            show_sleep_state: true,
            show_broadphase: false,
            collider_color: Vec4::new(0.0, 1.0, 0.0, 0.7),
            contact_color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            velocity_color: Vec4::new(1.0, 1.0, 0.0, 1.0),
            force_color: Vec4::new(1.0, 0.5, 0.0, 1.0),
            com_color: Vec4::new(0.0, 0.5, 1.0, 1.0),
            sleeping_color: Vec4::new(0.5, 0.5, 0.5, 0.5),
            velocity_scale: 0.1,
            force_scale: 0.0001,
            line_width: 1.0,
        }
    }
}

impl PhysicsDebugOverlay {
    pub fn generate_body_debug_lines(&self, body: &RigidBodyInspector) -> Vec<(Vec3, Vec3, Vec4)> {
        let mut lines = Vec::new();
        let col = if body.sleeping { self.sleeping_color } else { self.collider_color };
        // Velocity arrow
        if self.show_velocities && !body.is_static {
            let v_end = body.position + body.linear_velocity * self.velocity_scale;
            lines.push((body.position, v_end, self.velocity_color));
        }
        // Center of mass indicator (small cross)
        if self.show_center_of_mass {
            let com = body.position + body.orientation * body.center_of_mass;
            let d = 0.05;
            lines.push((com - Vec3::X * d, com + Vec3::X * d, self.com_color));
            lines.push((com - Vec3::Y * d, com + Vec3::Y * d, self.com_color));
            lines.push((com - Vec3::Z * d, com + Vec3::Z * d, self.com_color));
        }
        lines
    }
}

// ============================================================
// MAIN PHYSICS EDITOR STRUCT
// ============================================================

#[derive(Debug)]
pub struct PhysicsEditor {
    pub rigid_bodies: HashMap<u64, RigidBodyInspector>,
    pub constraints: HashMap<u64, Constraint>,
    pub joint_editor: JointEditor,
    pub cloth_sims: HashMap<u64, ClothSimulation>,
    pub fluid_sims: HashMap<u64, FluidSimulation>,
    pub fracture_systems: HashMap<u64, VoronoiFracture>,
    pub ragdoll_editor: RagdollEditor,
    pub vehicles: HashMap<u64, VehiclePhysics>,
    pub collision_shapes: HashMap<u64, CollisionShapeEditor>,
    pub material_library: HashMap<String, PhysicsMaterial>,
    pub contact_visualizer: ContactVisualizer,
    pub broad_phase: BroadPhase,
    pub world_settings: PhysicsWorldSettings,
    pub debug_overlay: PhysicsDebugOverlay,
    pub next_id: u64,
    pub selected_body: Option<u64>,
    pub selected_constraint: Option<u64>,
    pub simulation_running: bool,
    pub accumulated_time: f32,
    pub simulation_time: f32,
    pub step_count: u64,
    pub history: VecDeque<PhysicsSnapshot>,
    pub max_history: usize,
    pub undo_stack: VecDeque<PhysicsEditorCommand>,
    pub redo_stack: VecDeque<PhysicsEditorCommand>,
}

#[derive(Debug, Clone)]
pub struct PhysicsSnapshot {
    pub time: f32,
    pub body_states: HashMap<u64, (Vec3, Quat, Vec3, Vec3)>,
}

#[derive(Debug, Clone)]
pub enum PhysicsEditorCommand {
    AddBody { id: u64 },
    RemoveBody { id: u64, body: RigidBodyInspector },
    MoveBody { id: u64, old_pos: Vec3, new_pos: Vec3 },
    ChangeProperty { id: u64, property: String, old_value: f32, new_value: f32 },
    AddConstraint { id: u64 },
    RemoveConstraint { id: u64, constraint: Constraint },
}

impl PhysicsEditor {
    pub fn new() -> Self {
        Self {
            rigid_bodies: HashMap::new(),
            constraints: HashMap::new(),
            joint_editor: JointEditor::new(),
            cloth_sims: HashMap::new(),
            fluid_sims: HashMap::new(),
            fracture_systems: HashMap::new(),
            ragdoll_editor: RagdollEditor::new(),
            vehicles: HashMap::new(),
            collision_shapes: HashMap::new(),
            material_library: build_material_library(),
            contact_visualizer: ContactVisualizer::new(),
            broad_phase: BroadPhase::new(),
            world_settings: PhysicsWorldSettings::default(),
            debug_overlay: PhysicsDebugOverlay::default(),
            next_id: 1,
            selected_body: None,
            selected_constraint: None,
            simulation_running: false,
            accumulated_time: 0.0,
            simulation_time: 0.0,
            step_count: 0,
            history: VecDeque::new(),
            max_history: 60,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_rigid_body(&mut self, name: &str) -> u64 {
        let id = self.alloc_id();
        let body = RigidBodyInspector::new(id, name);
        self.rigid_bodies.insert(id, body);
        self.undo_stack.push_back(PhysicsEditorCommand::AddBody { id });
        id
    }

    pub fn remove_rigid_body(&mut self, id: u64) {
        if let Some(body) = self.rigid_bodies.remove(&id) {
            self.undo_stack.push_back(PhysicsEditorCommand::RemoveBody { id, body });
        }
    }

    pub fn add_constraint(&mut self, constraint: Constraint) -> u64 {
        let id = self.alloc_id();
        self.constraints.insert(id, constraint);
        id
    }

    pub fn add_cloth(&mut self, w: usize, h: usize, cell_size: f32) -> u64 {
        let id = self.alloc_id();
        self.cloth_sims.insert(id, ClothSimulation::new(w, h, cell_size, 0.01));
        id
    }

    pub fn add_fluid(&mut self, kernel_radius: f32, rest_density: f32) -> u64 {
        let id = self.alloc_id();
        self.fluid_sims.insert(id, FluidSimulation::new(kernel_radius, rest_density));
        id
    }

    pub fn add_fracture_system(&mut self, bounds_min: Vec3, bounds_max: Vec3, num_cells: usize, seed: u64) -> u64 {
        let id = self.alloc_id();
        self.fracture_systems.insert(id, VoronoiFracture::new(bounds_min, bounds_max, num_cells, seed));
        id
    }

    pub fn add_vehicle(&mut self, mass: f32, wheelbase: f32, track: f32) -> u64 {
        let id = self.alloc_id();
        let mut vehicle = VehiclePhysics::new(mass, wheelbase, track);
        vehicle.body_id = id;
        self.vehicles.insert(id, vehicle);
        id
    }

    pub fn add_collision_shape(&mut self, shape_type: CollisionShapeType) -> u64 {
        let id = self.alloc_id();
        let mut shape = CollisionShapeEditor::new();
        shape.shape_type = shape_type;
        shape.compute_aabb();
        shape.compute_volume();
        shape.compute_surface_area();
        self.collision_shapes.insert(id, shape);
        id
    }

    /// Update all broad-phase AABBs
    pub fn update_broadphase(&mut self) {
        for (id, body) in &self.rigid_bodies {
            if let Some(shape) = self.collision_shapes.get(id) {
                let aabb = shape.aabb.clone();
                // Transform AABB by body position/orientation (conservative)
                let center = body.position;
                let he = aabb.half_extents();
                let transformed = Aabb::new(center - he, center + he);
                self.broad_phase.update_aabb(*id, transformed);
            }
        }
    }

    /// Detect overlapping pairs
    pub fn broad_phase_query(&self) -> Vec<(u64, u64)> {
        self.broad_phase.compute_overlapping_pairs()
    }

    /// Step the physics simulation
    pub fn step(&mut self, dt: f32) {
        if !self.simulation_running { return; }
        self.accumulated_time += dt;
        let fixed_dt = self.world_settings.fixed_timestep;
        let mut steps = 0u32;
        while self.accumulated_time >= fixed_dt && steps < self.world_settings.max_substeps {
            self.fixed_step(fixed_dt);
            self.accumulated_time -= fixed_dt;
            steps += 1;
        }
    }

    fn fixed_step(&mut self, dt: f32) {
        // Integrate rigid bodies
        let ids: Vec<u64> = self.rigid_bodies.keys().copied().collect();
        for id in &ids {
            if let Some(body) = self.rigid_bodies.get_mut(id) {
                body.integrate(dt);
            }
        }
        // Cloth simulation
        let cloth_ids: Vec<u64> = self.cloth_sims.keys().copied().collect();
        for id in &cloth_ids {
            if let Some(cloth) = self.cloth_sims.get_mut(&id) {
                cloth.integrate(dt, self.simulation_time);
                cloth.solve_springs(dt);
                cloth.compute_normals();
            }
        }
        // Fluid simulation
        let fluid_ids: Vec<u64> = self.fluid_sims.keys().copied().collect();
        for id in &fluid_ids {
            if let Some(fluid) = self.fluid_sims.get_mut(&id) {
                fluid.step(dt);
            }
        }
        // Debris
        let fracture_ids: Vec<u64> = self.fracture_systems.keys().copied().collect();
        for id in &fracture_ids {
            if let Some(frac) = self.fracture_systems.get_mut(&id) {
                frac.integrate_debris(dt);
            }
        }
        // Contact visualizer update
        self.contact_visualizer.update(dt);
        self.simulation_time += dt;
        self.step_count += 1;
        // Snapshot
        if self.step_count % 6 == 0 {
            self.record_snapshot();
        }
    }

    fn record_snapshot(&mut self) {
        let mut states = HashMap::new();
        for (id, body) in &self.rigid_bodies {
            states.insert(*id, (body.position, body.orientation, body.linear_velocity, body.angular_velocity));
        }
        let snap = PhysicsSnapshot { time: self.simulation_time, body_states: states };
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(snap);
    }

    pub fn play(&mut self) { self.simulation_running = true; }
    pub fn pause(&mut self) { self.simulation_running = false; }

    pub fn reset(&mut self) {
        self.simulation_running = false;
        self.simulation_time = 0.0;
        self.step_count = 0;
        self.accumulated_time = 0.0;
        self.contact_visualizer.clear();
        self.history.clear();
        // Reset body states
        for body in self.rigid_bodies.values_mut() {
            body.linear_velocity = Vec3::ZERO;
            body.angular_velocity = Vec3::ZERO;
            body.force_accumulator = Vec3::ZERO;
            body.torque_accumulator = Vec3::ZERO;
            body.sleeping = false;
            body.sleep_timer = 0.0;
        }
    }

    /// Get body info string for UI display
    pub fn body_info_string(&self, id: u64) -> String {
        if let Some(body) = self.rigid_bodies.get(&id) {
            format!(
                "Body '{}' | Mass: {:.3}kg | Pos: ({:.3},{:.3},{:.3}) | V: {:.3}m/s | Sleep: {}",
                body.name,
                body.mass,
                body.position.x, body.position.y, body.position.z,
                body.linear_velocity.length(),
                body.sleeping,
            )
        } else {
            "No body selected".to_string()
        }
    }

    /// Create a standard scene with floor + stacked boxes
    pub fn create_demo_scene(&mut self) {
        // Floor (static)
        let floor_id = self.add_rigid_body("Floor");
        if let Some(body) = self.rigid_bodies.get_mut(&floor_id) {
            body.is_static = true;
            body.shape_type = RigidBodyShapeType::Box;
            body.shape_params.half_extents = Vec3::new(10.0, 0.1, 10.0);
            body.position = Vec3::new(0.0, -0.1, 0.0);
            body.recompute_inertia();
        }
        // Stack of boxes
        for i in 0..5 {
            let box_id = self.add_rigid_body(&format!("Box_{}", i));
            if let Some(body) = self.rigid_bodies.get_mut(&box_id) {
                body.mass = 1.0;
                body.shape_type = RigidBodyShapeType::Box;
                body.shape_params.half_extents = Vec3::splat(0.25);
                body.position = Vec3::new(0.0, 0.5 + i as f32 * 0.6, 0.0);
                body.recompute_inertia();
            }
        }
        // A sphere
        let sphere_id = self.add_rigid_body("Sphere");
        if let Some(body) = self.rigid_bodies.get_mut(&sphere_id) {
            body.mass = 2.0;
            body.shape_type = RigidBodyShapeType::Sphere;
            body.shape_params.radius = 0.3;
            body.position = Vec3::new(2.0, 1.5, 0.0);
            body.linear_velocity = Vec3::new(-3.0, 0.0, 0.0);
            body.recompute_inertia();
        }
    }

    /// Raycast against all rigid bodies (simplified AABB test)
    pub fn raycast(&self, ray_origin: Vec3, ray_dir: Vec3, max_dist: f32) -> Option<(u64, f32, Vec3)> {
        let mut closest: Option<(u64, f32, Vec3)> = None;
        let rd = ray_dir.normalize();
        for (id, _body) in &self.rigid_bodies {
            if let Some(shape) = self.collision_shapes.get(id) {
                if let Some(t) = ray_aabb_intersect(ray_origin, rd, &shape.aabb) {
                    if t < max_dist {
                        if closest.is_none() || t < closest.as_ref().unwrap().1 {
                            let hit_pt = ray_origin + rd * t;
                            closest = Some((*id, t, hit_pt));
                        }
                    }
                }
            }
        }
        closest
    }

    pub fn select_body_at_ray(&mut self, ray_origin: Vec3, ray_dir: Vec3) -> Option<u64> {
        if let Some((id, _, _)) = self.raycast(ray_origin, ray_dir, 1000.0) {
            self.selected_body = Some(id);
            Some(id)
        } else {
            self.selected_body = None;
            None
        }
    }

    pub fn generate_all_debug_lines(&self) -> Vec<(Vec3, Vec3, Vec4)> {
        let mut lines = Vec::new();
        // Body lines
        if self.debug_overlay.show_colliders || self.debug_overlay.show_velocities {
            for body in self.rigid_bodies.values() {
                let mut body_lines = self.debug_overlay.generate_body_debug_lines(body);
                lines.append(&mut body_lines);
            }
        }
        // Contact lines
        if self.debug_overlay.show_contacts {
            let mut contact_lines = self.contact_visualizer.generate_debug_lines();
            lines.append(&mut contact_lines);
        }
        // Joint lines
        if self.debug_overlay.show_joints {
            let mut joint_lines = self.joint_editor.generate_all_debug_lines();
            lines.append(&mut joint_lines);
        }
        lines
    }

    /// Export physics world to a simple text description
    pub fn export_description(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("PhysicsWorld: {} bodies, {} constraints\n",
            self.rigid_bodies.len(), self.constraints.len()));
        for (id, body) in &self.rigid_bodies {
            out.push_str(&format!("  Body[{}] '{}' mass={:.2} pos=({:.2},{:.2},{:.2})\n",
                id, body.name, body.mass, body.position.x, body.position.y, body.position.z));
        }
        for (id, c) in &self.constraints {
            out.push_str(&format!("  Constraint[{}] type={}\n", id, c.name()));
        }
        out
    }

    /// Force-based explosion at a point
    pub fn apply_explosion(&mut self, center: Vec3, force: f32, radius: f32) {
        for body in self.rigid_bodies.values_mut() {
            if body.is_static { continue; }
            let dir = body.position - center;
            let dist = dir.length();
            if dist < radius && dist > EPSILON {
                let falloff = 1.0 - (dist / radius);
                let impulse = (dir / dist) * force * falloff;
                body.apply_force_at_point(impulse, body.position);
                body.wake_up();
            }
        }
    }
}

// ============================================================
// UTILITY FUNCTIONS
// ============================================================

pub fn perpendicular_to(v: Vec3) -> Vec3 {
    let n = v.normalize();
    let candidate = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    n.cross(candidate).normalize()
}

pub fn quat_to_axis_angle(q: Quat) -> (Vec3, f32) {
    let w = q.w.clamp(-1.0, 1.0);
    let angle = 2.0 * w.acos();
    let s = (1.0 - w * w).sqrt();
    let axis = if s > EPSILON {
        Vec3::new(q.x / s, q.y / s, q.z / s)
    } else {
        Vec3::X
    };
    (axis, angle)
}

pub fn decompose_swing_twist(q: Quat, twist_axis: Vec3) -> (Quat, Quat) {
    let proj = Vec3::new(q.x, q.y, q.z).dot(twist_axis) * twist_axis;
    let twist = Quat::from_xyzw(proj.x, proj.y, proj.z, q.w).normalize();
    let swing = q * twist.inverse();
    (swing, twist)
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0 + EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn snap_value(v: f32, snap: f32) -> f32 {
    if snap < EPSILON { return v; }
    (v / snap).round() * snap
}

pub fn ray_aabb_intersect(origin: Vec3, dir: Vec3, aabb: &Aabb) -> Option<f32> {
    let inv_dir = Vec3::new(
        if dir.x.abs() > EPSILON { 1.0 / dir.x } else { f32::INFINITY },
        if dir.y.abs() > EPSILON { 1.0 / dir.y } else { f32::INFINITY },
        if dir.z.abs() > EPSILON { 1.0 / dir.z } else { f32::INFINITY },
    );
    let t1 = (aabb.min - origin) * inv_dir;
    let t2 = (aabb.max - origin) * inv_dir;
    let t_min = t1.min(t2);
    let t_max = t1.max(t2);
    let t_enter = t_min.x.max(t_min.y).max(t_min.z);
    let t_exit = t_max.x.min(t_max.y).min(t_max.z);
    if t_enter <= t_exit && t_exit >= 0.0 {
        Some(t_enter.max(0.0))
    } else {
        None
    }
}

/// Compute the angular impulse needed to align two coordinate frames
pub fn angular_correction_impulse(
    rot_a: Quat, rot_b: Quat,
    inv_inertia_a: Vec3, inv_inertia_b: Vec3,
    baumgarte: f32, dt: f32,
) -> Vec3 {
    let rot_diff = rot_b * rot_a.inverse();
    let (axis, angle) = quat_to_axis_angle(rot_diff);
    let error = axis * angle * baumgarte / dt;
    let inv_sum = inv_inertia_a + inv_inertia_b;
    Vec3::new(
        if inv_sum.x > EPSILON { error.x / inv_sum.x } else { 0.0 },
        if inv_sum.y > EPSILON { error.y / inv_sum.y } else { 0.0 },
        if inv_sum.z > EPSILON { error.z / inv_sum.z } else { 0.0 },
    )
}

/// Compute the center of mass of a compound body
pub fn compute_compound_com(masses: &[f32], positions: &[Vec3]) -> Vec3 {
    let total_mass: f32 = masses.iter().sum();
    if total_mass < EPSILON { return Vec3::ZERO; }
    let weighted: Vec3 = masses.iter().zip(positions.iter())
        .map(|(&m, &p)| p * m)
        .fold(Vec3::ZERO, |a, b| a + b);
    weighted / total_mass
}

/// Compute combined inertia for a compound body using parallel axis theorem
pub fn compute_compound_inertia(
    masses: &[f32],
    inertias: &[Mat4],
    positions: &[Vec3],
    com: Vec3,
) -> Mat4 {
    let mut result = Mat4::ZERO;
    for i in 0..masses.len() {
        let shifted = inertia_parallel_axis(inertias[i], masses[i], positions[i] - com);
        // Add matrices (3x3 part only)
        for col in 0..4 {
            let rc = result.col(col);
            let sc = shifted.col(col);
            // Mat4 doesn't impl AddAssign directly, reconstruct
            let _ = (rc, sc); // suppressed
        }
        // Manual element-wise add
        let rc0 = result.col(0) + shifted.col(0);
        let rc1 = result.col(1) + shifted.col(1);
        let rc2 = result.col(2) + shifted.col(2);
        let rc3 = result.col(3) + shifted.col(3);
        result = Mat4::from_cols(rc0, rc1, rc2, rc3);
    }
    result
}

/// Solve a 1D position constraint (Baumgarte): return position correction
pub fn baumgarte_position_correction(error: f32, effective_mass: f32, baumgarte: f32, dt: f32) -> f32 {
    -baumgarte / dt * error * effective_mass
}

/// Compute relative velocity of two points on two rigid bodies
pub fn relative_velocity(
    vel_a: Vec3, omega_a: Vec3, r_a: Vec3,
    vel_b: Vec3, omega_b: Vec3, r_b: Vec3,
) -> Vec3 {
    let v_a = vel_a + omega_a.cross(r_a);
    let v_b = vel_b + omega_b.cross(r_b);
    v_b - v_a
}

/// Restitution-based rebound velocity
pub fn restitution_velocity(v_rel_normal: f32, restitution: f32) -> f32 {
    if v_rel_normal >= 0.0 { return 0.0; }
    -(1.0 + restitution) * v_rel_normal
}

/// Compute friction impulse (Coulomb friction cone)
pub fn friction_impulse(
    j_normal: f32, friction_coeff: f32,
    tangent_impulse: f32,
) -> f32 {
    tangent_impulse.clamp(-friction_coeff * j_normal, friction_coeff * j_normal)
}

/// Compute angular velocity from rotation delta over dt
pub fn angular_velocity_from_delta_quat(q_prev: Quat, q_curr: Quat, dt: f32) -> Vec3 {
    let dq = q_curr * q_prev.inverse();
    let (axis, angle) = quat_to_axis_angle(dq);
    axis * (angle / dt)
}

/// Convert angular velocity vector to quaternion derivative
pub fn omega_to_quat_deriv(omega: Vec3, q: Quat) -> Quat {
    let ox = omega.x;
    let oy = omega.y;
    let oz = omega.z;
    Quat::from_xyzw(
        0.5 * (oy * q.z - oz * q.y + ox * q.w),
        0.5 * (oz * q.x - ox * q.z + oy * q.w),
        0.5 * (ox * q.y - oy * q.x + oz * q.w),
        0.5 * (-ox * q.x - oy * q.y - oz * q.z),
    )
}

/// Cross product matrix (skew symmetric) for angular dynamics
pub fn cross_matrix(v: Vec3) -> [[f32; 3]; 3] {
    [
        [0.0, -v.z, v.y],
        [v.z, 0.0, -v.x],
        [-v.y, v.x, 0.0],
    ]
}

/// Multiply 3x3 matrix by vector
pub fn mat3_mul_vec(m: [[f32; 3]; 3], v: Vec3) -> Vec3 {
    Vec3::new(
        m[0][0]*v.x + m[0][1]*v.y + m[0][2]*v.z,
        m[1][0]*v.x + m[1][1]*v.y + m[1][2]*v.z,
        m[2][0]*v.x + m[2][1]*v.y + m[2][2]*v.z,
    )
}

/// Transpose 3x3 matrix
pub fn mat3_transpose(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

/// Multiply two 3x3 matrices
pub fn mat3_mul(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut c = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    c
}

/// 3x3 determinant
pub fn mat3_det(m: [[f32; 3]; 3]) -> f32 {
    m[0][0]*(m[1][1]*m[2][2]-m[1][2]*m[2][1])
    - m[0][1]*(m[1][0]*m[2][2]-m[1][2]*m[2][0])
    + m[0][2]*(m[1][0]*m[2][1]-m[1][1]*m[2][0])
}

/// 3x3 matrix inverse
pub fn mat3_inverse(m: [[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let det = mat3_det(m);
    if det.abs() < EPSILON { return None; }
    let inv_det = 1.0 / det;
    let result = [
        [
            (m[1][1]*m[2][2]-m[1][2]*m[2][1])*inv_det,
            (m[0][2]*m[2][1]-m[0][1]*m[2][2])*inv_det,
            (m[0][1]*m[1][2]-m[0][2]*m[1][1])*inv_det,
        ],
        [
            (m[1][2]*m[2][0]-m[1][0]*m[2][2])*inv_det,
            (m[0][0]*m[2][2]-m[0][2]*m[2][0])*inv_det,
            (m[0][2]*m[1][0]-m[0][0]*m[1][2])*inv_det,
        ],
        [
            (m[1][0]*m[2][1]-m[1][1]*m[2][0])*inv_det,
            (m[0][1]*m[2][0]-m[0][0]*m[2][1])*inv_det,
            (m[0][0]*m[1][1]-m[0][1]*m[1][0])*inv_det,
        ],
    ];
    Some(result)
}

/// Gram-Schmidt orthonormalization of three vectors
pub fn gram_schmidt(v0: Vec3, v1: Vec3, v2: Vec3) -> (Vec3, Vec3, Vec3) {
    let u0 = v0.normalize();
    let u1 = (v1 - v1.dot(u0) * u0).normalize();
    let u2 = (v2 - v2.dot(u0) * u0 - v2.dot(u1) * u1).normalize();
    (u0, u1, u2)
}

// ============================================================
// ADDITIONAL PHYSICS FORMULAS AND TABLES
// ============================================================

/// Young's modulus table (Pa)
pub fn young_modulus(surface: SurfaceType) -> f32 {
    match surface {
        SurfaceType::Metal => 210e9,
        SurfaceType::Glass => 70e9,
        SurfaceType::Concrete | SurfaceType::WetConcrete => 30e9,
        SurfaceType::Wood | SurfaceType::WetWood => 10e9,
        SurfaceType::Rubber | SurfaceType::ToughRubber => 0.05e9,
        SurfaceType::Foam => 0.001e9,
        SurfaceType::Plastic | SurfaceType::HardPlastic => 3e9,
        SurfaceType::Granite => 50e9,
        SurfaceType::Marble => 50e9,
        SurfaceType::Ceramic => 100e9,
        SurfaceType::Ice => 9e9,
        SurfaceType::Leather => 0.1e9,
        SurfaceType::Asphalt | SurfaceType::TarmacRoad => 5e9,
        _ => 1e9,
    }
}

/// Poisson's ratio table
pub fn poisson_ratio(surface: SurfaceType) -> f32 {
    match surface {
        SurfaceType::Metal => 0.3,
        SurfaceType::Glass => 0.22,
        SurfaceType::Rubber | SurfaceType::ToughRubber => 0.48,
        SurfaceType::Concrete => 0.2,
        SurfaceType::Wood => 0.35,
        SurfaceType::Foam => 0.3,
        SurfaceType::Plastic | SurfaceType::HardPlastic => 0.35,
        SurfaceType::Ceramic => 0.25,
        SurfaceType::Ice => 0.33,
        _ => 0.3,
    }
}

/// Coefficient of restitution between two materials
pub fn restitution_between(a: SurfaceType, b: SurfaceType) -> f32 {
    let ra = material_restitution(a);
    let rb = material_restitution(b);
    (ra * rb).sqrt() // geometric mean
}

pub fn material_restitution(s: SurfaceType) -> f32 {
    match s {
        SurfaceType::Rubber | SurfaceType::ToughRubber => 0.9,
        SurfaceType::Metal => 0.4,
        SurfaceType::Glass => 0.7,
        SurfaceType::Wood => 0.4,
        SurfaceType::Concrete => 0.2,
        SurfaceType::Ice => 0.1,
        SurfaceType::Foam => 0.05,
        SurfaceType::Mud | SurfaceType::Mud_Thick => 0.02,
        SurfaceType::Sand => 0.1,
        SurfaceType::Carpet => 0.05,
        SurfaceType::Cork => 0.6,
        SurfaceType::Asphalt | SurfaceType::TarmacRoad => 0.15,
        SurfaceType::Marble | SurfaceType::Granite => 0.6,
        SurfaceType::Ceramic => 0.5,
        SurfaceType::Plastic | SurfaceType::HardPlastic => 0.4,
        _ => DEFAULT_RESTITUTION,
    }
}

/// Compute terminal velocity for an object falling through air
/// v_t = sqrt(2*m*g / (rho * Cd * A))
pub fn terminal_velocity(mass: f32, drag_coeff: f32, cross_section_area: f32) -> f32 {
    (2.0 * mass * GRAVITY / (AIR_DENSITY * drag_coeff * cross_section_area)).sqrt()
}

/// Compute buoyancy force on a submerged object
/// F_b = rho_fluid * V_submerged * g
pub fn buoyancy_force(fluid_density: f32, submerged_volume: f32) -> f32 {
    fluid_density * submerged_volume * GRAVITY
}

/// Drag force: F_d = 0.5 * rho * v^2 * Cd * A
pub fn drag_force(fluid_density: f32, velocity: f32, drag_coeff: f32, area: f32) -> f32 {
    0.5 * fluid_density * velocity * velocity * drag_coeff * area
}

/// Compute impact force from collision (impulse-momentum)
/// F = m * delta_v / delta_t
pub fn impact_force(mass: f32, delta_velocity: f32, contact_time: f32) -> f32 {
    mass * delta_velocity / contact_time.max(EPSILON)
}

/// Kinetic energy of a rigid body
pub fn kinetic_energy(mass: f32, vel: Vec3, inertia_diag: Vec3, omega: Vec3) -> f32 {
    let linear = 0.5 * mass * vel.length_squared();
    let angular = 0.5 * (inertia_diag.x * omega.x * omega.x
        + inertia_diag.y * omega.y * omega.y
        + inertia_diag.z * omega.z * omega.z);
    linear + angular
}

/// Potential energy (gravitational)
pub fn potential_energy(mass: f32, height: f32) -> f32 {
    mass * GRAVITY * height
}

/// Spring potential energy
pub fn spring_potential_energy(k: f32, extension: f32) -> f32 {
    0.5 * k * extension * extension
}

/// Approximate collision time from material properties (Hertz contact)
/// t_contact ~ 2.87 * (m / (E_eff * v_rel))^(2/5)
pub fn hertz_contact_time(mass: f32, eff_young: f32, rel_velocity: f32, radius: f32) -> f32 {
    let v = rel_velocity.abs().max(EPSILON);
    let factor = (mass / (eff_young * radius.sqrt() * v)).powf(0.4);
    2.87 * factor
}

/// Effective Young's modulus for two materials in contact (Hertz)
/// 1/E_eff = (1-nu_a^2)/E_a + (1-nu_b^2)/E_b
pub fn effective_young_modulus(e_a: f32, nu_a: f32, e_b: f32, nu_b: f32) -> f32 {
    let inv = (1.0 - nu_a * nu_a) / e_a + (1.0 - nu_b * nu_b) / e_b;
    1.0 / inv.max(EPSILON)
}

/// Hertz contact force: F = (4/3) * E_eff * sqrt(R_eff) * delta^(3/2)
pub fn hertz_contact_force(eff_young: f32, eff_radius: f32, penetration: f32) -> f32 {
    (4.0 / 3.0) * eff_young * eff_radius.sqrt() * penetration.powf(1.5)
}

/// Effective radius for two spheres in contact
/// 1/R_eff = 1/R_a + 1/R_b
pub fn effective_radius(r_a: f32, r_b: f32) -> f32 {
    (r_a * r_b) / (r_a + r_b + EPSILON)
}

// ============================================================
// TORQUE & FORCE ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct ForceAnalyzer {
    pub forces: Vec<(Vec3, Vec3, Vec3)>,  // (origin, direction*magnitude, color)
    pub show_resultant: bool,
    pub scale: f32,
}

impl ForceAnalyzer {
    pub fn new() -> Self {
        Self { forces: Vec::new(), show_resultant: true, scale: 0.01 }
    }

    pub fn add_force(&mut self, origin: Vec3, force: Vec3, color: Vec3) {
        self.forces.push((origin, force * self.scale, Vec3::new(color.x, color.y, color.z)));
    }

    pub fn resultant(&self) -> Vec3 {
        self.forces.iter().fold(Vec3::ZERO, |acc, (_, f, _)| acc + *f)
    }

    pub fn resultant_torque(&self, about: Vec3) -> Vec3 {
        self.forces.iter().fold(Vec3::ZERO, |acc, (origin, force, _)| {
            let r = *origin - about;
            acc + r.cross(*force)
        })
    }

    pub fn clear(&mut self) { self.forces.clear(); }

    pub fn generate_lines(&self) -> Vec<(Vec3, Vec3, Vec4)> {
        let mut lines: Vec<(Vec3, Vec3, Vec4)> = self.forces.iter().map(|(orig, f, col)| {
            (*orig, *orig + *f, Vec4::new(col.x, col.y, col.z, 1.0))
        }).collect();
        if self.show_resultant {
            let com = Vec3::ZERO;
            let res = self.resultant();
            lines.push((com, com + res, Vec4::new(1.0, 1.0, 1.0, 1.0)));
        }
        lines
    }
}

// ============================================================
// MESH TOOLS FOR PHYSICS
// ============================================================

/// Compute signed volume of a triangle mesh (divergence theorem)
pub fn mesh_signed_volume(triangles: &[(Vec3, Vec3, Vec3)]) -> f32 {
    let mut vol = 0.0f32;
    for &(a, b, c) in triangles {
        vol += a.dot(b.cross(c)) / 6.0;
    }
    vol
}

/// Compute mesh surface area
pub fn mesh_surface_area(triangles: &[(Vec3, Vec3, Vec3)]) -> f32 {
    triangles.iter().map(|&(a, b, c)| {
        (b - a).cross(c - a).length() * 0.5
    }).sum()
}

/// Compute mesh center of mass (uniform density)
pub fn mesh_center_of_mass(triangles: &[(Vec3, Vec3, Vec3)]) -> Vec3 {
    let mut weighted_sum = Vec3::ZERO;
    let mut total_vol = 0.0f32;
    for &(a, b, c) in triangles {
        let tet_vol = a.dot(b.cross(c)) / 6.0;
        let tet_com = (a + b + c) / 4.0; // (0 + a + b + c) / 4
        weighted_sum += tet_com * tet_vol;
        total_vol += tet_vol;
    }
    if total_vol.abs() > EPSILON { weighted_sum / total_vol } else { Vec3::ZERO }
}

/// Compute mesh inertia tensor (uniform density, about origin)
pub fn mesh_inertia_tensor(triangles: &[(Vec3, Vec3, Vec3)], density: f32) -> [[f32; 3]; 3] {
    let mut i = [[0.0f32; 3]; 3];
    for &(a, b, c) in triangles {
        let vol = a.dot(b.cross(c)) / 6.0;
        let pts = [a, b, c, Vec3::ZERO]; // tetrahedron with origin
        // Covariance contribution
        let mut cov = [[0.0f32; 3]; 3];
        for p in &pts {
            let pv = [p.x, p.y, p.z];
            for ii in 0..3 {
                for jj in 0..3 {
                    cov[ii][jj] += pv[ii] * pv[jj];
                }
            }
        }
        let scale = density * vol * 0.1; // simplified
        let trace = cov[0][0] + cov[1][1] + cov[2][2];
        for ii in 0..3 {
            for jj in 0..3 {
                let delta = if ii == jj { trace } else { 0.0 };
                i[ii][jj] += scale * (delta - cov[ii][jj]);
            }
        }
    }
    i
}

// ============================================================
// CONSTRAINT SOLVER (PGS — Projected Gauss-Seidel)
// ============================================================

pub struct ConstraintSolver {
    pub iterations: u32,
    pub baumgarte: f32,
    pub slop: f32,         // penetration slop (allowance before correction)
    pub warm_starting: bool,
}

impl ConstraintSolver {
    pub fn new() -> Self {
        Self { iterations: 10, baumgarte: 0.2, slop: 0.001, warm_starting: true }
    }

    /// Solve a single JacobianRow given body masses
    pub fn solve_row(
        &self, row: &mut JacobianRow,
        vel_a: &mut Vec3, omega_a: &mut Vec3, inv_mass_a: f32, inv_inertia_a: Vec3,
        vel_b: &mut Vec3, omega_b: &mut Vec3, inv_mass_b: f32, inv_inertia_b: Vec3,
    ) {
        let delta = row.solve_velocity(*vel_a, *omega_a, *vel_b, *omega_b);
        *vel_a += row.j_lin_a * (delta * inv_mass_a);
        *omega_a += row.j_ang_a * delta * inv_inertia_a;
        *vel_b += row.j_lin_b * (delta * inv_mass_b);
        *omega_b += row.j_ang_b * delta * inv_inertia_b;
    }

    /// Multi-iteration PGS for a set of rows (all against single pair)
    pub fn solve_rows(
        &self, rows: &mut Vec<JacobianRow>,
        vel_a: &mut Vec3, omega_a: &mut Vec3, inv_mass_a: f32, inv_inertia_a: Vec3,
        vel_b: &mut Vec3, omega_b: &mut Vec3, inv_mass_b: f32, inv_inertia_b: Vec3,
    ) {
        for _ in 0..self.iterations {
            for row in rows.iter_mut() {
                self.solve_row(row,
                    vel_a, omega_a, inv_mass_a, inv_inertia_a,
                    vel_b, omega_b, inv_mass_b, inv_inertia_b);
            }
        }
    }

    /// Setup and warm-start rows before solving
    pub fn warm_start(
        &self, rows: &[JacobianRow],
        vel_a: &mut Vec3, omega_a: &mut Vec3, inv_mass_a: f32, inv_inertia_a: Vec3,
        vel_b: &mut Vec3, omega_b: &mut Vec3, inv_mass_b: f32, inv_inertia_b: Vec3,
    ) {
        if !self.warm_starting { return; }
        for row in rows {
            let lambda = row.lambda;
            *vel_a += row.j_lin_a * (lambda * inv_mass_a);
            *omega_a += row.j_ang_a * lambda * inv_inertia_a;
            *vel_b += row.j_lin_b * (lambda * inv_mass_b);
            *omega_b += row.j_ang_b * lambda * inv_inertia_b;
        }
    }
}

// ============================================================
// STABILITY ANALYSIS TOOLS
// ============================================================

pub struct StabilityAnalyzer {
    pub energy_history: VecDeque<f32>,
    pub momentum_history: VecDeque<Vec3>,
    pub angular_momentum_history: VecDeque<Vec3>,
    pub history_size: usize,
}

impl StabilityAnalyzer {
    pub fn new(history_size: usize) -> Self {
        Self {
            energy_history: VecDeque::with_capacity(history_size),
            momentum_history: VecDeque::with_capacity(history_size),
            angular_momentum_history: VecDeque::with_capacity(history_size),
            history_size,
        }
    }

    pub fn record(&mut self, bodies: &HashMap<u64, RigidBodyInspector>) {
        let mut total_ke = 0.0f32;
        let mut total_mom = Vec3::ZERO;
        let mut total_ang = Vec3::ZERO;
        for body in bodies.values() {
            if body.is_static { continue; }
            let inertia_diag = Vec3::new(
                body.inertia_tensor.col(0).x,
                body.inertia_tensor.col(1).y,
                body.inertia_tensor.col(2).z,
            );
            total_ke += kinetic_energy(body.mass, body.linear_velocity, inertia_diag, body.angular_velocity);
            total_ke += potential_energy(body.mass, body.position.y);
            total_mom += body.linear_velocity * body.mass;
            let r = body.position;
            total_ang += r.cross(body.linear_velocity * body.mass);
        }
        if self.energy_history.len() >= self.history_size { self.energy_history.pop_front(); }
        if self.momentum_history.len() >= self.history_size { self.momentum_history.pop_front(); }
        if self.angular_momentum_history.len() >= self.history_size { self.angular_momentum_history.pop_front(); }
        self.energy_history.push_back(total_ke);
        self.momentum_history.push_back(total_mom);
        self.angular_momentum_history.push_back(total_ang);
    }

    pub fn energy_variance(&self) -> f32 {
        let n = self.energy_history.len();
        if n < 2 { return 0.0; }
        let mean = self.energy_history.iter().sum::<f32>() / n as f32;
        let var = self.energy_history.iter().map(|e| (e - mean) * (e - mean)).sum::<f32>() / n as f32;
        var
    }

    pub fn is_numerically_stable(&self) -> bool {
        let var = self.energy_variance();
        let mean = if self.energy_history.is_empty() { 1.0 } else {
            self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32
        };
        // Stable if variance is < 1% of mean energy
        mean.abs() < EPSILON || var / mean.abs() < 0.01
    }
}

// ============================================================
// PICKING / DRAGGING TOOLS
// ============================================================

#[derive(Debug, Clone)]
pub struct PhysicsPicker {
    pub active: bool,
    pub body_id: u64,
    pub pick_point_local: Vec3,
    pub pick_point_world: Vec3,
    pub target_point: Vec3,
    pub spring_stiffness: f32,
    pub spring_damping: f32,
    pub max_force: f32,
    pub mouse_ray_origin: Vec3,
    pub mouse_ray_dir: Vec3,
    pub pick_distance: f32,
}

impl PhysicsPicker {
    pub fn new() -> Self {
        Self {
            active: false,
            body_id: 0,
            pick_point_local: Vec3::ZERO,
            pick_point_world: Vec3::ZERO,
            target_point: Vec3::ZERO,
            spring_stiffness: 300.0,
            spring_damping: 20.0,
            max_force: 500.0,
            mouse_ray_origin: Vec3::ZERO,
            mouse_ray_dir: Vec3::Z,
            pick_distance: 5.0,
        }
    }

    pub fn begin_pick(&mut self, body_id: u64, local_pt: Vec3, world_pt: Vec3, dist: f32) {
        self.active = true;
        self.body_id = body_id;
        self.pick_point_local = local_pt;
        self.pick_point_world = world_pt;
        self.target_point = world_pt;
        self.pick_distance = dist;
    }

    pub fn end_pick(&mut self) {
        self.active = false;
    }

    pub fn update_target(&mut self, new_ray_origin: Vec3, new_ray_dir: Vec3) {
        self.mouse_ray_origin = new_ray_origin;
        self.mouse_ray_dir = new_ray_dir;
        self.target_point = new_ray_origin + new_ray_dir * self.pick_distance;
    }

    /// Compute force to apply to the body to drag the pick point to target
    pub fn compute_force(&self, body: &RigidBodyInspector) -> Vec3 {
        if !self.active { return Vec3::ZERO; }
        let current_world_pt = body.position + body.orientation * self.pick_point_local;
        let error = self.target_point - current_world_pt;
        let vel_at_pt = body.linear_velocity
            + body.angular_velocity.cross(body.orientation * self.pick_point_local);
        let force = error * self.spring_stiffness - vel_at_pt * self.spring_damping;
        let f_len = force.length();
        if f_len > self.max_force { force * (self.max_force / f_len) } else { force }
    }
}

// ============================================================
// SIMULATION REPLAY
// ============================================================

#[derive(Debug)]
pub struct SimulationReplay {
    pub snapshots: Vec<PhysicsSnapshot>,
    pub current_frame: usize,
    pub playback_speed: f32,
    pub looping: bool,
    pub playing: bool,
    pub time_accumulator: f32,
}

impl SimulationReplay {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            current_frame: 0,
            playback_speed: 1.0,
            looping: false,
            playing: false,
            time_accumulator: 0.0,
        }
    }

    pub fn record_snapshot(&mut self, snap: PhysicsSnapshot) {
        self.snapshots.push(snap);
    }

    pub fn play(&mut self) { self.playing = true; }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn stop(&mut self) { self.playing = false; self.current_frame = 0; }

    pub fn advance(&mut self, dt: f32) -> Option<&PhysicsSnapshot> {
        if !self.playing || self.snapshots.is_empty() { return None; }
        self.time_accumulator += dt * self.playback_speed;
        // Assume ~60fps snapshots
        let frame_dt = 1.0 / 60.0;
        while self.time_accumulator >= frame_dt {
            self.current_frame += 1;
            self.time_accumulator -= frame_dt;
        }
        if self.current_frame >= self.snapshots.len() {
            if self.looping {
                self.current_frame = 0;
            } else {
                self.current_frame = self.snapshots.len().saturating_sub(1);
                self.playing = false;
            }
        }
        self.snapshots.get(self.current_frame)
    }

    pub fn seek_to_time(&mut self, target_time: f32) {
        for (i, snap) in self.snapshots.iter().enumerate() {
            if snap.time >= target_time {
                self.current_frame = i;
                return;
            }
        }
        self.current_frame = self.snapshots.len().saturating_sub(1);
    }

    pub fn duration(&self) -> f32 {
        self.snapshots.last().map(|s| s.time).unwrap_or(0.0)
    }
}

// ============================================================
// FLUID PARAMETERS PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct FluidParamsPanel {
    pub kernel_radius: f32,
    pub rest_density: f32,
    pub viscosity: f32,
    pub pressure_stiffness: f32,
    pub surface_tension: f32,
    pub gravity_scale: f32,
    pub time_scale: f32,
    pub max_particles: usize,
    pub particle_mass: f32,
    pub domain_size: Vec3,
    pub domain_offset: Vec3,
    pub boundary_restitution: f32,
    pub display_radius_scale: f32,
    pub color_by_velocity: bool,
    pub color_low_vel: Vec4,
    pub color_high_vel: Vec4,
    pub color_velocity_scale: f32,
}

impl Default for FluidParamsPanel {
    fn default() -> Self {
        Self {
            kernel_radius: 0.15,
            rest_density: 1000.0,
            viscosity: 0.1,
            pressure_stiffness: 200.0,
            surface_tension: 0.0728,
            gravity_scale: 1.0,
            time_scale: 1.0,
            max_particles: 4096,
            particle_mass: 0.02,
            domain_size: Vec3::splat(5.0),
            domain_offset: Vec3::ZERO,
            boundary_restitution: 0.1,
            display_radius_scale: 1.5,
            color_by_velocity: true,
            color_low_vel: Vec4::new(0.0, 0.2, 0.8, 0.8),
            color_high_vel: Vec4::new(0.8, 0.8, 1.0, 0.9),
            color_velocity_scale: 5.0,
        }
    }
}

impl FluidParamsPanel {
    pub fn apply_to_sim(&self, sim: &mut FluidSimulation) {
        sim.kernel_radius = self.kernel_radius;
        sim.rest_density = self.rest_density;
        sim.viscosity = self.viscosity;
        sim.pressure_stiffness = self.pressure_stiffness;
        sim.surface_tension = self.surface_tension;
        sim.gravity = Vec3::new(0.0, -GRAVITY * self.gravity_scale, 0.0);
        sim.time_scale = self.time_scale;
        sim.restitution = self.boundary_restitution;
        let half = self.domain_size * 0.5;
        sim.domain_min = self.domain_offset - half;
        sim.domain_max = self.domain_offset + half;
    }

    pub fn particle_color(&self, velocity: Vec3) -> Vec4 {
        if !self.color_by_velocity {
            return self.color_low_vel;
        }
        let t = (velocity.length() / self.color_velocity_scale).clamp(0.0, 1.0);
        Vec4::new(
            self.color_low_vel.x + (self.color_high_vel.x - self.color_low_vel.x) * t,
            self.color_low_vel.y + (self.color_high_vel.y - self.color_low_vel.y) * t,
            self.color_low_vel.z + (self.color_high_vel.z - self.color_low_vel.z) * t,
            self.color_low_vel.w + (self.color_high_vel.w - self.color_low_vel.w) * t,
        )
    }
}

// ============================================================
// DESTRUCTION PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct DestructionPanel {
    pub num_fragments: usize,
    pub seed: u64,
    pub material_strength: f32,
    pub fracture_on_impact: bool,
    pub impact_threshold: f32,
    pub propagation_speed: f32,
    pub debris_lifetime: f32,
    pub debris_gravity_scale: f32,
    pub debris_air_resistance: f32,
    pub sound_on_fracture: bool,
    pub particle_effect_on_fracture: bool,
    pub fragment_density: f32,
    pub auto_remove_small_debris: bool,
    pub min_debris_mass: f32,
}

impl Default for DestructionPanel {
    fn default() -> Self {
        Self {
            num_fragments: 20,
            seed: 42,
            material_strength: 1e6,
            fracture_on_impact: true,
            impact_threshold: 1000.0,
            propagation_speed: 5000.0,
            debris_lifetime: 5.0,
            debris_gravity_scale: 1.0,
            debris_air_resistance: 0.1,
            sound_on_fracture: true,
            particle_effect_on_fracture: true,
            fragment_density: CONCRETE_DENSITY,
            auto_remove_small_debris: true,
            min_debris_mass: 0.01,
        }
    }
}

// ============================================================
// CLOTH PARAMS PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct ClothParamsPanel {
    pub grid_width: usize,
    pub grid_height: usize,
    pub cell_size: f32,
    pub total_mass: f32,
    pub stretch_stiffness: f32,
    pub shear_stiffness: f32,
    pub bend_stiffness: f32,
    pub gravity_scale: f32,
    pub wind_speed: f32,
    pub wind_direction: Vec3,
    pub wind_turbulence: f32,
    pub drag: f32,
    pub self_collision: bool,
    pub tearing: bool,
    pub tear_threshold: f32,
    pub iterations: u32,
    pub thickness: f32,
    pub visualize_springs: bool,
    pub spring_color_stretch: Vec4,
    pub spring_color_shear: Vec4,
    pub spring_color_bend: Vec4,
    pub visualize_normals: bool,
}

impl Default for ClothParamsPanel {
    fn default() -> Self {
        Self {
            grid_width: 20,
            grid_height: 20,
            cell_size: 0.1,
            total_mass: 0.5,
            stretch_stiffness: 1000.0,
            shear_stiffness: 500.0,
            bend_stiffness: 100.0,
            gravity_scale: 1.0,
            wind_speed: 0.0,
            wind_direction: Vec3::new(1.0, 0.0, 0.0),
            wind_turbulence: 0.1,
            drag: 0.05,
            self_collision: true,
            tearing: false,
            tear_threshold: 3.0,
            iterations: 10,
            thickness: 0.001,
            visualize_springs: false,
            spring_color_stretch: Vec4::new(0.2, 0.8, 0.2, 1.0),
            spring_color_shear: Vec4::new(0.8, 0.8, 0.2, 1.0),
            spring_color_bend: Vec4::new(0.8, 0.2, 0.2, 1.0),
            visualize_normals: false,
        }
    }
}

impl ClothParamsPanel {
    pub fn apply_to_sim(&self, sim: &mut ClothSimulation) {
        sim.stretch_stiffness = self.stretch_stiffness;
        sim.shear_stiffness = self.shear_stiffness;
        sim.bend_stiffness = self.bend_stiffness;
        sim.gravity = Vec3::new(0.0, -GRAVITY * self.gravity_scale, 0.0);
        sim.wind_velocity = self.wind_direction.normalize_or_zero() * self.wind_speed;
        sim.wind_turbulence = self.wind_turbulence;
        sim.drag_coefficient = self.drag;
        sim.self_collision_enabled = self.self_collision;
        sim.tearing_enabled = self.tearing;
        sim.global_tear_threshold = self.tear_threshold;
        sim.iterations = self.iterations;
        sim.thickness = self.thickness;
        // Update spring constants for all springs
        for spring in &mut sim.springs {
            match spring.spring_type {
                ClothSpringType::Stretch => {
                    spring.stiffness = self.stretch_stiffness;
                    if self.tearing {
                        spring.tear_threshold = spring.rest_length * self.tear_threshold;
                    }
                }
                ClothSpringType::Shear => spring.stiffness = self.shear_stiffness,
                ClothSpringType::Bend => spring.stiffness = self.bend_stiffness,
            }
        }
    }
}

// ============================================================
// VEHICLE PARAMS PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct VehicleParamsPanel {
    pub mass: f32,
    pub wheelbase: f32,
    pub track_width: f32,
    pub com_height: f32,
    pub engine_type: EngineType,
    pub max_steering_angle: f32,
    pub brake_force_total: f32,
    pub brake_bias_front: f32,
    pub abs: bool,
    pub tcs: bool,
    pub esc: bool,
    pub diff_type_front: String,
    pub diff_type_rear: String,
    pub aero_drag: f32,
    pub downforce: f32,
    pub show_suspension: bool,
    pub show_tire_forces: bool,
    pub show_rpm: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineType {
    PetrolSport,
    PetrolTurbo,
    DieselTruck,
    Electric,
    Custom,
}

impl Default for VehicleParamsPanel {
    fn default() -> Self {
        Self {
            mass: 1500.0,
            wheelbase: 2.7,
            track_width: 1.6,
            com_height: 0.5,
            engine_type: EngineType::PetrolSport,
            max_steering_angle: 35.0,
            brake_force_total: 8000.0,
            brake_bias_front: 0.6,
            abs: true,
            tcs: true,
            esc: false,
            diff_type_front: "Open".to_string(),
            diff_type_rear: "LSD".to_string(),
            aero_drag: 0.6,
            downforce: 0.0,
            show_suspension: true,
            show_tire_forces: false,
            show_rpm: true,
        }
    }
}

// ============================================================
// MATERIAL EDITOR PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct MaterialEditorPanel {
    pub selected_material: Option<String>,
    pub preview_sphere_radius: f32,
    pub drop_height: f32,
    pub floor_material: String,
    pub show_bounce_preview: bool,
    pub filter_text: String,
    pub sort_by: MaterialSortBy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaterialSortBy {
    Name,
    Friction,
    Restitution,
    Density,
}

impl MaterialEditorPanel {
    pub fn new() -> Self {
        Self {
            selected_material: None,
            preview_sphere_radius: 0.1,
            drop_height: 2.0,
            floor_material: "Concrete".to_string(),
            show_bounce_preview: true,
            filter_text: String::new(),
            sort_by: MaterialSortBy::Name,
        }
    }

    /// Simulate a sphere drop and compute bounce height
    pub fn compute_bounce_height(&self, restitution: f32) -> f32 {
        // h_n = e^2 * h_0
        self.drop_height * restitution * restitution
    }

    /// Filter materials by name substring
    pub fn filtered_materials<'a>(&'a self, library: &'a HashMap<String, PhysicsMaterial>) -> Vec<&'a PhysicsMaterial> {
        let mut mats: Vec<&PhysicsMaterial> = library.values()
            .filter(|m| m.name.to_lowercase().contains(&self.filter_text.to_lowercase()))
            .collect();
        match self.sort_by {
            MaterialSortBy::Name => mats.sort_by(|a, b| a.name.cmp(&b.name)),
            MaterialSortBy::Friction => mats.sort_by(|a, b| b.static_friction.partial_cmp(&a.static_friction).unwrap_or(std::cmp::Ordering::Equal)),
            MaterialSortBy::Restitution => mats.sort_by(|a, b| b.restitution.partial_cmp(&a.restitution).unwrap_or(std::cmp::Ordering::Equal)),
            MaterialSortBy::Density => mats.sort_by(|a, b| b.density.partial_cmp(&a.density).unwrap_or(std::cmp::Ordering::Equal)),
        }
        mats
    }
}

// ============================================================
// RAGDOLL PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct RagdollPanel {
    pub selected_bone: Option<String>,
    pub muscle_tone_global: f32,
    pub blend_to_animation: bool,
    pub blend_time: f32,
    pub show_bone_bodies: bool,
    pub show_joint_axes: bool,
    pub show_muscle_forces: bool,
    pub impact_threshold: f32,
    pub collapse_on_impact: bool,
    pub get_up_enabled: bool,
    pub get_up_threshold_velocity: f32,
    pub get_up_blend_time: f32,
}

impl Default for RagdollPanel {
    fn default() -> Self {
        Self {
            selected_bone: None,
            muscle_tone_global: 0.0,
            blend_to_animation: false,
            blend_time: 0.5,
            show_bone_bodies: true,
            show_joint_axes: true,
            show_muscle_forces: false,
            impact_threshold: 10.0,
            collapse_on_impact: true,
            get_up_enabled: true,
            get_up_threshold_velocity: 0.5,
            get_up_blend_time: 1.0,
        }
    }
}

// ============================================================
// DEBUG COUNTERS
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct PhysicsStats {
    pub active_bodies: usize,
    pub sleeping_bodies: usize,
    pub static_bodies: usize,
    pub active_constraints: usize,
    pub contact_pairs: usize,
    pub cloth_particles: usize,
    pub fluid_particles: usize,
    pub debris_particles: usize,
    pub solver_iterations_used: u32,
    pub step_time_us: u64,
    pub broadphase_pairs: usize,
    pub narrowphase_contacts: usize,
}

impl PhysicsStats {
    pub fn gather(editor: &PhysicsEditor) -> Self {
        let mut stats = PhysicsStats::default();
        for body in editor.rigid_bodies.values() {
            if body.is_static { stats.static_bodies += 1; }
            else if body.sleeping { stats.sleeping_bodies += 1; }
            else { stats.active_bodies += 1; }
        }
        stats.active_constraints = editor.constraints.len();
        stats.contact_pairs = editor.contact_visualizer.contacts.len();
        for cloth in editor.cloth_sims.values() {
            stats.cloth_particles += cloth.particles.len();
        }
        for fluid in editor.fluid_sims.values() {
            stats.fluid_particles += fluid.particles.len();
        }
        for frac in editor.fracture_systems.values() {
            stats.debris_particles += frac.debris_particles.len();
        }
        stats.solver_iterations_used = editor.world_settings.solver_iterations;
        stats
    }

    pub fn summary(&self) -> String {
        format!(
            "Bodies: {} active / {} sleeping / {} static | Constraints: {} | Contacts: {} | SPH: {} | Cloth: {}",
            self.active_bodies, self.sleeping_bodies, self.static_bodies,
            self.active_constraints,
            self.contact_pairs,
            self.fluid_particles,
            self.cloth_particles,
        )
    }
}

// ============================================================
// DYNAMIC AABB TREE (Bounding Volume Hierarchy)
// ============================================================

#[derive(Debug, Clone)]
pub struct BvhNode {
    pub aabb: Aabb,
    pub body_id: Option<u64>,
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub parent: Option<usize>,
    pub height: i32,
}

impl BvhNode {
    pub fn new_leaf(aabb: Aabb, body_id: u64) -> Self {
        Self { aabb, body_id: Some(body_id), left: None, right: None, parent: None, height: 0 }
    }
    pub fn is_leaf(&self) -> bool { self.left.is_none() }
}

pub struct DynamicAabbTree {
    pub nodes: Vec<BvhNode>,
    pub root: Option<usize>,
    pub free_list: Vec<usize>,
}

impl DynamicAabbTree {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), root: None, free_list: Vec::new() }
    }

    pub fn alloc_node(&mut self) -> usize {
        if let Some(idx) = self.free_list.pop() { return idx; }
        let idx = self.nodes.len();
        self.nodes.push(BvhNode {
            aabb: Aabb::new(Vec3::ZERO, Vec3::ZERO),
            body_id: None,
            left: None,
            right: None,
            parent: None,
            height: 0,
        });
        idx
    }

    pub fn insert(&mut self, body_id: u64, aabb: Aabb) {
        let leaf = self.alloc_node();
        self.nodes[leaf] = BvhNode::new_leaf(aabb, body_id);
        if self.root.is_none() {
            self.root = Some(leaf);
            return;
        }
        // Find best sibling
        let sibling = self.find_best_sibling(leaf);
        let old_parent = self.nodes[sibling].parent;
        let new_parent = self.alloc_node();
        let merged_aabb = self.nodes[sibling].aabb.merged(&self.nodes[leaf].aabb);
        self.nodes[new_parent].aabb = merged_aabb;
        self.nodes[new_parent].parent = old_parent;
        self.nodes[new_parent].left = Some(sibling);
        self.nodes[new_parent].right = Some(leaf);
        self.nodes[sibling].parent = Some(new_parent);
        self.nodes[leaf].parent = Some(new_parent);
        if let Some(op) = old_parent {
            if self.nodes[op].left == Some(sibling) {
                self.nodes[op].left = Some(new_parent);
            } else {
                self.nodes[op].right = Some(new_parent);
            }
        } else {
            self.root = Some(new_parent);
        }
        // Refit ancestors
        let mut current = Some(new_parent);
        while let Some(idx) = current {
            if let (Some(l), Some(r)) = (self.nodes[idx].left, self.nodes[idx].right) {
                let la = self.nodes[l].aabb.clone();
                let ra = self.nodes[r].aabb.clone();
                self.nodes[idx].aabb = la.merged(&ra);
                self.nodes[idx].height = 1 + self.nodes[l].height.max(self.nodes[r].height);
            }
            current = self.nodes[idx].parent;
        }
    }

    fn find_best_sibling(&self, leaf: usize) -> usize {
        // Simplified: just return root
        self.root.unwrap_or(leaf)
    }

    pub fn query_aabb(&self, query: &Aabb) -> Vec<u64> {
        let mut result = Vec::new();
        let mut stack = Vec::new();
        if let Some(root) = self.root { stack.push(root); }
        while let Some(idx) = stack.pop() {
            let node = &self.nodes[idx];
            if !node.aabb.intersects(query) { continue; }
            if node.is_leaf() {
                if let Some(id) = node.body_id { result.push(id); }
            } else {
                if let Some(l) = node.left { stack.push(l); }
                if let Some(r) = node.right { stack.push(r); }
            }
        }
        result
    }
}

// ============================================================
// FINAL INTEGRATION TEST / SMOKE TEST FUNCTION
// ============================================================

pub fn run_physics_editor_smoke_test() -> bool {
    let mut editor = PhysicsEditor::new();
    // Test rigid body creation
    let id = editor.add_rigid_body("TestBox");
    if let Some(body) = editor.rigid_bodies.get_mut(&id) {
        body.mass = 5.0;
        body.shape_type = RigidBodyShapeType::Box;
        body.shape_params.half_extents = Vec3::new(1.0, 0.5, 0.5);
        body.recompute_inertia();
    }
    // Test cloth
    let cloth_id = editor.add_cloth(10, 10, 0.1);
    if let Some(cloth) = editor.cloth_sims.get_mut(&cloth_id) {
        cloth.pin_vertex(0);
        cloth.pin_vertex(9);
    }
    // Test fluid
    let fluid_id = editor.add_fluid(0.1, 1000.0);
    if let Some(fluid) = editor.fluid_sims.get_mut(&fluid_id) {
        fluid.spawn_block(Vec3::ZERO, Vec3::splat(0.5), 0.1, 0.02);
    }
    // Test vehicle
    let _veh_id = editor.add_vehicle(1500.0, 2.7, 1.6);
    // Test fracture
    let frac_id = editor.add_fracture_system(Vec3::ZERO, Vec3::ONE, 10, 1234);
    if let Some(frac) = editor.fracture_systems.get_mut(&frac_id) {
        frac.material_strength = 1e6;
        frac.accumulate_stress(0, 2e6, 0.01);
        let fractured = frac.check_fracture();
        let frac_ref = editor.fracture_systems.get_mut(&frac_id).unwrap();
        if !fractured.is_empty() {
            let f2 = fractured.clone();
            frac_ref.propagate_cracks(&f2);
        }
    }
    // Test inertia tensor computations
    let i_box = inertia_tensor_box(1.0, Vec3::splat(0.5));
    let i_sphere = inertia_tensor_sphere(1.0, 0.5);
    let i_capsule = inertia_tensor_capsule(1.0, 0.25, 0.5);
    let _i_cylinder = inertia_tensor_cylinder(1.0, 0.25, 0.5);
    assert!(i_box.col(0).x > 0.0);
    assert!(i_sphere.col(0).x > 0.0);
    assert!(i_capsule.col(0).x > 0.0);
    // Test Pacejka
    let (b, c, d, e) = pacejka_dry_asphalt();
    let f = pacejka_magic_formula(5.0_f32.to_radians(), b, c, d, e);
    assert!(f.abs() > 0.0);
    // Test SPH kernels
    let r = Vec3::new(0.05, 0.0, 0.0);
    let h = 0.1f32;
    let k = sph_kernel_poly6(r.length_squared(), h);
    assert!(k > 0.0);
    // Test AABB
    let aabb = Aabb::from_points(&[Vec3::ZERO, Vec3::ONE]);
    assert!(aabb.contains_point(Vec3::splat(0.5)));
    // Test OBB PCA
    let pts: Vec<Vec3> = (0..20).map(|i| Vec3::new(i as f32 * 0.1, 0.0, 0.0)).collect();
    let obb = fit_obb_pca(&pts);
    assert!(obb.half_extents.length() > 0.0);
    // Test Ackermann
    let (outer, inner) = ackermann_steering(2.7, 1.6, 0.3);
    assert!(inner.abs() > outer.abs()); // inner wheel turns sharper
    // Test material library
    let lib = build_material_library();
    assert!(lib.len() >= 40);
    // Test BVH
    let mut bvh = DynamicAabbTree::new();
    bvh.insert(1, Aabb::new(Vec3::ZERO, Vec3::ONE));
    bvh.insert(2, Aabb::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5)));
    let hits = bvh.query_aabb(&Aabb::new(Vec3::splat(0.4), Vec3::splat(0.6)));
    assert!(!hits.is_empty());
    // Test stability analyzer
    let mut stability = StabilityAnalyzer::new(60);
    stability.record(&editor.rigid_bodies);
    // Test ragdoll
    editor.ragdoll_editor.build_humanoid_skeleton();
    assert!(editor.ragdoll_editor.bones.len() > 10);
    assert!(editor.ragdoll_editor.joints.len() > 5);
    // Simulate a few steps
    editor.play();
    editor.step(1.0 / 60.0);
    editor.step(1.0 / 60.0);
    editor.step(1.0 / 60.0);
    true
}

// ============================================================
// EDITOR PANEL LAYOUT DESCRIPTOR
// ============================================================

#[derive(Debug, Clone)]
pub struct PhysicsEditorLayout {
    pub show_rigid_body_panel: bool,
    pub show_constraint_panel: bool,
    pub show_joint_editor: bool,
    pub show_cloth_panel: bool,
    pub show_fluid_panel: bool,
    pub show_destruction_panel: bool,
    pub show_ragdoll_panel: bool,
    pub show_vehicle_panel: bool,
    pub show_collision_shape_panel: bool,
    pub show_material_panel: bool,
    pub show_contact_panel: bool,
    pub show_stats: bool,
    pub show_world_settings: bool,
    pub panel_width: f32,
    pub panel_height: f32,
    pub selected_tab: PhysicsTab,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PhysicsTab {
    RigidBodies,
    Constraints,
    Cloth,
    Fluid,
    Destruction,
    Ragdoll,
    Vehicles,
    Shapes,
    Materials,
    Contacts,
    Settings,
}

impl Default for PhysicsEditorLayout {
    fn default() -> Self {
        Self {
            show_rigid_body_panel: true,
            show_constraint_panel: true,
            show_joint_editor: true,
            show_cloth_panel: false,
            show_fluid_panel: false,
            show_destruction_panel: false,
            show_ragdoll_panel: false,
            show_vehicle_panel: false,
            show_collision_shape_panel: true,
            show_material_panel: true,
            show_contact_panel: true,
            show_stats: true,
            show_world_settings: true,
            panel_width: 320.0,
            panel_height: 600.0,
            selected_tab: PhysicsTab::RigidBodies,
        }
    }
}

impl PhysicsEditorLayout {
    pub fn tab_label(&self, tab: &PhysicsTab) -> &'static str {
        match tab {
            PhysicsTab::RigidBodies => "Rigid Bodies",
            PhysicsTab::Constraints => "Constraints",
            PhysicsTab::Cloth => "Cloth",
            PhysicsTab::Fluid => "Fluid",
            PhysicsTab::Destruction => "Destruction",
            PhysicsTab::Ragdoll => "Ragdoll",
            PhysicsTab::Vehicles => "Vehicles",
            PhysicsTab::Shapes => "Shapes",
            PhysicsTab::Materials => "Materials",
            PhysicsTab::Contacts => "Contacts",
            PhysicsTab::Settings => "Settings",
        }
    }
}

// ============================================================
// WIND FIELD
// ============================================================

#[derive(Debug, Clone)]
pub struct WindField {
    pub base_velocity: Vec3,
    pub gust_strength: f32,
    pub gust_frequency: f32,
    pub gust_duration: f32,
    pub turbulence_scale: f32,
    pub turbulence_frequency: f32,
    pub enabled: bool,
    pub altitude_gradient: f32, // velocity increase per meter of altitude
}

impl WindField {
    pub fn new(base: Vec3) -> Self {
        Self {
            base_velocity: base,
            gust_strength: 5.0,
            gust_frequency: 0.2,
            gust_duration: 2.0,
            turbulence_scale: 1.0,
            turbulence_frequency: 0.5,
            enabled: true,
            altitude_gradient: 0.1,
        }
    }

    pub fn velocity_at(&self, position: Vec3, time: f32) -> Vec3 {
        if !self.enabled { return Vec3::ZERO; }
        let mut v = self.base_velocity;
        // Altitude gradient
        let alt_factor = 1.0 + position.y * self.altitude_gradient;
        v *= alt_factor;
        // Gust (simple sinusoidal)
        let gust_phase = time * self.gust_frequency * TWO_PI;
        let gust = self.base_velocity.normalize_or_zero() * (gust_phase.sin() * 0.5 + 0.5) * self.gust_strength;
        v += gust;
        // Turbulence
        let tx = (position.x * 0.3 + time * self.turbulence_frequency).sin();
        let ty = (position.y * 0.4 + time * self.turbulence_frequency * 1.3).cos();
        let tz = (position.z * 0.25 + time * self.turbulence_frequency * 0.7).sin();
        v += Vec3::new(tx, ty * 0.3, tz) * self.turbulence_scale;
        v
    }

    pub fn force_on_particle(&self, position: Vec3, time: f32, mass: f32, drag_area: f32) -> Vec3 {
        let wind = self.velocity_at(position, time);
        let drag = drag_force(AIR_DENSITY, wind.length(), 1.0, drag_area);
        wind.normalize_or_zero() * drag
    }
}

// ============================================================
// HEIGHTFIELD COLLISION DETECTION
// ============================================================

pub fn heightfield_closest_point(hf: &HeightfieldShape, query: Vec3) -> Vec3 {
    let h = hf.height_at_world(query.x, query.z);
    Vec3::new(query.x.clamp(0.0, (hf.width - 1) as f32 * hf.scale.x),
              h,
              query.z.clamp(0.0, (hf.depth - 1) as f32 * hf.scale.z))
}

pub fn sphere_heightfield_contact(center: Vec3, radius: f32, hf: &HeightfieldShape) -> Option<(Vec3, Vec3, f32)> {
    let closest = heightfield_closest_point(hf, center);
    let h = hf.height_at_world(center.x, center.z);
    let ground_y = h;
    let depth = radius - (center.y - ground_y);
    if depth > 0.0 {
        let contact_pt = Vec3::new(center.x, ground_y, center.z);
        Some((contact_pt, Vec3::Y, depth))
    } else {
        None
    }
}

// ============================================================
// RAG TO POSITION BODY
// ============================================================

pub fn transform_rigid_body(body: &mut RigidBodyInspector, translation: Vec3, rotation: Quat) {
    body.position = translation;
    body.orientation = rotation;
    if !body.sleeping { body.wake_up(); }
}

pub fn set_rigid_body_velocity(body: &mut RigidBodyInspector, linear: Vec3, angular: Vec3) {
    body.linear_velocity = linear;
    body.angular_velocity = angular;
    body.wake_up();
}

// ============================================================
// EDITOR GIZMO HELPERS
// ============================================================

pub fn world_to_screen(world_pos: Vec3, view_proj: Mat4, screen_width: f32, screen_height: f32) -> Vec2 {
    let clip = view_proj * Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
    if clip.w.abs() < EPSILON { return Vec2::ZERO; }
    let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    Vec2::new(
        (ndc.x * 0.5 + 0.5) * screen_width,
        (0.5 - ndc.y * 0.5) * screen_height,
    )
}

pub fn screen_to_world_ray(
    screen_x: f32, screen_y: f32,
    screen_width: f32, screen_height: f32,
    inv_view_proj: Mat4,
) -> (Vec3, Vec3) {
    let ndc_x = (screen_x / screen_width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_y / screen_height) * 2.0;
    let near = inv_view_proj * Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
    let far  = inv_view_proj * Vec4::new(ndc_x, ndc_y,  1.0, 1.0);
    let origin = Vec3::new(near.x / near.w, near.y / near.w, near.z / near.w);
    let far_pt = Vec3::new(far.x / far.w, far.y / far.w, far.z / far.w);
    let direction = (far_pt - origin).normalize();
    (origin, direction)
}

// ============================================================
// IMPULSE RESOLUTION
// ============================================================

pub fn resolve_collision_impulse(
    body_a: &mut RigidBodyInspector,
    body_b: &mut RigidBodyInspector,
    contact: &ContactPoint,
    restitution: f32,
) {
    let ra = contact.world_position - body_a.position;
    let rb = contact.world_position - body_b.position;
    let v_a = body_a.linear_velocity + body_a.angular_velocity.cross(ra);
    let v_b = body_b.linear_velocity + body_b.angular_velocity.cross(rb);
    let v_rel = v_b - v_a;
    let v_rel_n = v_rel.dot(contact.world_normal);
    if v_rel_n > 0.0 { return; } // separating
    let inv_ma = if body_a.is_static { 0.0 } else { 1.0 / body_a.mass };
    let inv_mb = if body_b.is_static { 0.0 } else { 1.0 / body_b.mass };
    let i_a = Vec3::new(
        body_a.inertia_tensor.col(0).x,
        body_a.inertia_tensor.col(1).y,
        body_a.inertia_tensor.col(2).z,
    );
    let i_b = Vec3::new(
        body_b.inertia_tensor.col(0).x,
        body_b.inertia_tensor.col(1).y,
        body_b.inertia_tensor.col(2).z,
    );
    let inv_ia = Vec3::new(
        if i_a.x > EPSILON { 1.0 / i_a.x } else { 0.0 },
        if i_a.y > EPSILON { 1.0 / i_a.y } else { 0.0 },
        if i_a.z > EPSILON { 1.0 / i_a.z } else { 0.0 },
    );
    let inv_ib = Vec3::new(
        if i_b.x > EPSILON { 1.0 / i_b.x } else { 0.0 },
        if i_b.y > EPSILON { 1.0 / i_b.y } else { 0.0 },
        if i_b.z > EPSILON { 1.0 / i_b.z } else { 0.0 },
    );
    let n = contact.world_normal;
    let ra_cross_n = ra.cross(n);
    let rb_cross_n = rb.cross(n);
    let denom = inv_ma + inv_mb
        + ra_cross_n.dot(inv_ia * ra_cross_n)
        + rb_cross_n.dot(inv_ib * rb_cross_n);
    if denom < EPSILON { return; }
    let j = -(1.0 + restitution) * v_rel_n / denom;
    let impulse = n * j;
    if !body_a.is_static {
        body_a.linear_velocity -= impulse * inv_ma;
        body_a.angular_velocity -= inv_ia * ra.cross(impulse);
        body_a.wake_up();
    }
    if !body_b.is_static {
        body_b.linear_velocity += impulse * inv_mb;
        body_b.angular_velocity += inv_ib * rb.cross(impulse);
        body_b.wake_up();
    }
}

// ============================================================
// TRIGGER VOLUMES
// ============================================================

#[derive(Debug, Clone)]
pub struct TriggerVolume {
    pub id: u64,
    pub name: String,
    pub aabb: Aabb,
    pub bodies_inside: HashSet<u64>,
    pub on_enter_events: Vec<String>,
    pub on_exit_events: Vec<String>,
    pub active: bool,
}

impl TriggerVolume {
    pub fn new(id: u64, name: &str, aabb: Aabb) -> Self {
        Self {
            id, name: name.to_string(), aabb,
            bodies_inside: HashSet::new(),
            on_enter_events: Vec::new(),
            on_exit_events: Vec::new(),
            active: true,
        }
    }

    pub fn update(&mut self, bodies: &HashMap<u64, RigidBodyInspector>) -> (Vec<u64>, Vec<u64>) {
        let mut entered = Vec::new();
        let mut exited = Vec::new();
        let mut now_inside = HashSet::new();
        for (id, body) in bodies {
            if self.aabb.contains_point(body.position) {
                now_inside.insert(*id);
                if !self.bodies_inside.contains(id) {
                    entered.push(*id);
                }
            }
        }
        for id in &self.bodies_inside {
            if !now_inside.contains(id) {
                exited.push(*id);
            }
        }
        self.bodies_inside = now_inside;
        (entered, exited)
    }
}

// ============================================================
// JOINT DRIVE PARAMETERS
// ============================================================

#[derive(Debug, Clone)]
pub struct JointDrive {
    pub target_position: f32,
    pub target_velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub max_force: f32,
    pub force_mode: DriveForceMode,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DriveForceMode { Force, Acceleration }

impl JointDrive {
    pub fn new(stiffness: f32, damping: f32, max_force: f32) -> Self {
        Self { target_position: 0.0, target_velocity: 0.0, stiffness, damping, max_force, force_mode: DriveForceMode::Force, enabled: true }
    }
    pub fn compute_force(&self, current_pos: f32, current_vel: f32) -> f32 {
        if !self.enabled { return 0.0; }
        let f = self.stiffness * (self.target_position - current_pos) + self.damping * (self.target_velocity - current_vel);
        f.clamp(-self.max_force, self.max_force)
    }
    pub fn compute_torque(&self, current_angle: f32, current_ang_vel: f32) -> f32 { self.compute_force(current_angle, current_ang_vel) }
}

#[derive(Debug, Clone)]
pub struct DOF6Drive {
    pub x_linear: JointDrive,
    pub y_linear: JointDrive,
    pub z_linear: JointDrive,
    pub x_angular: JointDrive,
    pub y_angular: JointDrive,
    pub z_angular: JointDrive,
}
impl DOF6Drive {
    pub fn new() -> Self {
        let d = || JointDrive::new(0.0, 0.0, f32::INFINITY);
        Self { x_linear: d(), y_linear: d(), z_linear: d(), x_angular: d(), y_angular: d(), z_angular: d() }
    }
    pub fn compute_linear_forces(&self, cur: Vec3, vel: Vec3) -> Vec3 {
        Vec3::new(self.x_linear.compute_force(cur.x, vel.x), self.y_linear.compute_force(cur.y, vel.y), self.z_linear.compute_force(cur.z, vel.z))
    }
    pub fn compute_angular_torques(&self, cur: Vec3, omega: Vec3) -> Vec3 {
        Vec3::new(self.x_angular.compute_torque(cur.x, omega.x), self.y_angular.compute_torque(cur.y, omega.y), self.z_angular.compute_torque(cur.z, omega.z))
    }
}

// ============================================================
// PNEUMATIC SPRING MODEL
// ============================================================

#[derive(Debug, Clone)]
pub struct PneumaticSpring {
    pub natural_length: f32,
    pub area: f32,
    pub initial_pressure: f32,
    pub polytropic_index: f32,
    pub initial_volume: f32,
    pub damping: f32,
}
impl PneumaticSpring {
    pub fn new(natural_length: f32, area: f32, pressure: f32) -> Self {
        Self { natural_length, area, initial_pressure: pressure, polytropic_index: 1.4, initial_volume: area * natural_length, damping: 500.0 }
    }
    pub fn force(&self, current_length: f32, velocity: f32) -> f32 {
        let current_volume = self.area * current_length.max(EPSILON);
        let ratio = (self.initial_volume / current_volume).powf(self.polytropic_index);
        let gauge_pressure = self.initial_pressure * ratio - self.initial_pressure;
        gauge_pressure * self.area - self.damping * velocity
    }
}

// ============================================================
// CONTACT MANIFOLD MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct ContactManifold {
    pub body_a: u64,
    pub body_b: u64,
    pub points: Vec<ContactPoint>,
    pub normal: Vec3,
    pub friction: f32,
    pub restitution: f32,
    pub age: u32,
    pub persistent_threshold: f32,
}
impl ContactManifold {
    pub fn new(body_a: u64, body_b: u64, friction: f32, restitution: f32) -> Self {
        Self { body_a, body_b, points: Vec::new(), normal: Vec3::Y, friction, restitution, age: 0, persistent_threshold: 0.02 }
    }
    pub fn add_point(&mut self, pt: ContactPoint) {
        for existing in &mut self.points {
            if (existing.world_position - pt.world_position).length_squared() < self.persistent_threshold * self.persistent_threshold {
                existing.warm_impulse = existing.normal_impulse;
                existing.normal_impulse = pt.normal_impulse;
                existing.world_position = pt.world_position;
                existing.penetration_depth = pt.penetration_depth;
                return;
            }
        }
        if self.points.len() < 4 { self.points.push(pt); }
        else {
            let min_idx = self.points.iter().enumerate()
                .min_by(|(_, a), (_, b)| a.penetration_depth.partial_cmp(&b.penetration_depth).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i).unwrap_or(0);
            if pt.penetration_depth > self.points[min_idx].penetration_depth { self.points[min_idx] = pt; }
        }
    }
    pub fn purge_invalid(&mut self) {
        self.points.retain(|p| p.penetration_depth > -0.02);
        self.age += 1;
    }
    pub fn total_impulse(&self) -> f32 { self.points.iter().map(|p| p.normal_impulse).sum() }
}

// ============================================================
// SOFT BODY
// ============================================================

#[derive(Debug, Clone)]
pub struct SoftBodyNode {
    pub position: Vec3,
    pub velocity: Vec3,
    pub force: Vec3,
    pub mass: f32,
    pub inv_mass: f32,
    pub fixed: bool,
}

#[derive(Debug, Clone)]
pub struct SoftBodySpring {
    pub node_a: usize,
    pub node_b: usize,
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
}

#[derive(Debug, Clone)]
pub struct SoftBody {
    pub nodes: Vec<SoftBodyNode>,
    pub springs: Vec<SoftBodySpring>,
    pub rest_volume: f32,
    pub volume_stiffness: f32,
    pub pressure_stiffness: f32,
    pub density: f32,
    pub total_mass: f32,
}

impl SoftBody {
    pub fn new_sphere(radius: f32, resolution: usize, mass: f32, density: f32) -> Self {
        let mut nodes = Vec::new();
        let mut springs = Vec::new();
        let stacks = resolution;
        let slices = resolution * 2;
        for i in 0..=stacks {
            let phi = PI * i as f32 / stacks as f32;
            for j in 0..slices {
                let theta = TWO_PI * j as f32 / slices as f32;
                let pos = Vec3::new(radius * phi.sin() * theta.cos(), radius * phi.cos(), radius * phi.sin() * theta.sin());
                let node_mass = mass / ((stacks + 1) * slices) as f32;
                nodes.push(SoftBodyNode { position: pos, velocity: Vec3::ZERO, force: Vec3::ZERO, mass: node_mass, inv_mass: 1.0 / node_mass, fixed: false });
            }
        }
        let n = nodes.len();
        for i in 0..n {
            for j in i + 1..n {
                let dist = (nodes[i].position - nodes[j].position).length();
                if dist < radius * 0.8 {
                    springs.push(SoftBodySpring { node_a: i, node_b: j, rest_length: dist, stiffness: 1000.0, damping: 5.0 });
                }
            }
        }
        let total_mass = nodes.iter().map(|n| n.mass).sum();
        Self { nodes, springs, rest_volume: (4.0/3.0)*PI*radius*radius*radius, volume_stiffness: 500.0, pressure_stiffness: 100.0, density, total_mass }
    }

    pub fn integrate(&mut self, dt: f32, gravity: Vec3) {
        for node in &mut self.nodes {
            if node.fixed { continue; }
            let acc = (node.force + gravity * node.mass) * node.inv_mass;
            node.velocity += acc * dt;
            node.velocity *= 0.99;
            node.position += node.velocity * dt;
            node.force = Vec3::ZERO;
        }
    }

    pub fn solve_springs(&mut self) {
        for spring in &self.springs {
            let a = spring.node_a;
            let b = spring.node_b;
            let pa = self.nodes[a].position;
            let pb = self.nodes[b].position;
            let delta = pb - pa;
            let dist = delta.length();
            if dist < EPSILON { continue; }
            let stretch = dist - spring.rest_length;
            let n = delta / dist;
            let va = self.nodes[a].velocity;
            let vb = self.nodes[b].velocity;
            let damp = (vb - va).dot(n) * spring.damping;
            let force = n * (stretch * spring.stiffness + damp);
            let ia = self.nodes[a].inv_mass;
            let ib = self.nodes[b].inv_mass;
            let total = ia + ib;
            if total < EPSILON { continue; }
            self.nodes[a].force += force * ia / total;
            self.nodes[b].force -= force * ib / total;
        }
    }

    pub fn compute_volume(&self) -> f32 {
        let center = self.nodes.iter().fold(Vec3::ZERO, |a, n| a + n.position) / self.nodes.len() as f32;
        let max_r = self.nodes.iter().map(|n| (n.position - center).length()).fold(0.0f32, f32::max);
        (4.0 / 3.0) * PI * max_r * max_r * max_r
    }

    pub fn apply_pressure(&mut self) {
        let vol = self.compute_volume();
        let pressure = self.pressure_stiffness * (self.rest_volume - vol);
        let center = self.nodes.iter().fold(Vec3::ZERO, |a, n| a + n.position) / self.nodes.len() as f32;
        for node in &mut self.nodes {
            node.force += (node.position - center).normalize_or_zero() * pressure;
        }
    }
}

// ============================================================
// MOTOR PID CONTROLLER
// ============================================================

#[derive(Debug, Clone)]
pub struct MotorVelocityController {
    pub kp: f32, pub ki: f32, pub kd: f32,
    pub integral: f32,
    pub prev_error: f32,
    pub output_min: f32,
    pub output_max: f32,
    pub integral_limit: f32,
    pub target: f32,
}
impl MotorVelocityController {
    pub fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self { kp, ki, kd, integral: 0.0, prev_error: 0.0, output_min: -1000.0, output_max: 1000.0, integral_limit: 100.0, target: 0.0 }
    }
    pub fn update(&mut self, current: f32, dt: f32) -> f32 {
        let error = self.target - current;
        self.integral = (self.integral + error * dt).clamp(-self.integral_limit, self.integral_limit);
        let derivative = (error - self.prev_error) / dt.max(EPSILON);
        self.prev_error = error;
        (self.kp * error + self.ki * self.integral + self.kd * derivative).clamp(self.output_min, self.output_max)
    }
    pub fn reset(&mut self) { self.integral = 0.0; self.prev_error = 0.0; }
}

// ============================================================
// INVERSE KINEMATICS
// ============================================================

/// 2-bone analytical IK solve
pub fn two_bone_ik(root: Vec3, bone1_length: f32, bone2_length: f32, target: Vec3, hint: Vec3) -> (Vec3, Vec3) {
    let total_len = bone1_length + bone2_length;
    let rtt = target - root;
    let dist = rtt.length().min(total_len * 0.9999);
    let a = bone2_length; let b = bone1_length; let c = dist;
    let cos_b = ((b*b + c*c - a*a) / (2.0*b*c)).clamp(-1.0, 1.0);
    let angle_b = cos_b.acos();
    let forward = rtt.normalize_or_zero();
    let right = forward.cross(hint).normalize_or_zero();
    let up = right.cross(forward);
    let joint1 = root + forward * b * angle_b.cos() + up * b * angle_b.sin();
    (joint1, target)
}

/// FABRIK N-bone chain solver
pub fn fabrik_solve(positions: &mut Vec<Vec3>, bone_lengths: &[f32], target: Vec3, iterations: u32, tolerance: f32) {
    if positions.len() < 2 || bone_lengths.len() != positions.len() - 1 { return; }
    let root = positions[0];
    let n = positions.len();
    let total_length: f32 = bone_lengths.iter().sum();
    if (target - root).length() >= total_length {
        let dir = (target - root).normalize_or_zero();
        for i in 1..n { positions[i] = positions[i-1] + dir * bone_lengths[i-1]; }
        return;
    }
    for _ in 0..iterations {
        *positions.last_mut().unwrap() = target;
        for i in (0..n-1).rev() {
            let dir = (positions[i] - positions[i+1]).normalize_or_zero();
            positions[i] = positions[i+1] + dir * bone_lengths[i];
        }
        positions[0] = root;
        for i in 0..n-1 {
            let dir = (positions[i+1] - positions[i]).normalize_or_zero();
            positions[i+1] = positions[i] + dir * bone_lengths[i];
        }
        if (*positions.last().unwrap() - target).length() < tolerance { break; }
    }
}

// ============================================================
// FORCE PLATE SENSOR
// ============================================================

#[derive(Debug, Clone)]
pub struct ForcePlate {
    pub id: u64,
    pub position: Vec3,
    pub size: Vec2,
    pub total_force: Vec3,
    pub total_torque: Vec3,
    pub cop: Vec3,
    pub active: bool,
    pub history: VecDeque<Vec3>,
    pub history_max: usize,
}
impl ForcePlate {
    pub fn new(id: u64, position: Vec3, size: Vec2) -> Self {
        Self { id, position, size, total_force: Vec3::ZERO, total_torque: Vec3::ZERO, cop: Vec3::ZERO, active: true, history: VecDeque::new(), history_max: 256 }
    }
    pub fn record_contact(&mut self, contact: &ContactPoint) {
        let local_pt = contact.world_position - self.position;
        let hx = self.size.x * 0.5; let hz = self.size.y * 0.5;
        if local_pt.x.abs() <= hx && local_pt.z.abs() <= hz {
            let f = contact.world_normal * contact.normal_impulse;
            self.total_force += f;
            self.total_torque += local_pt.cross(f);
        }
    }
    pub fn update(&mut self) {
        if self.history.len() >= self.history_max { self.history.pop_front(); }
        self.history.push_back(self.total_force);
        let fy = self.total_force.y.abs();
        if fy > EPSILON { self.cop = Vec3::new(-self.total_torque.z / fy, 0.0, self.total_torque.x / fy); }
        self.total_force = Vec3::ZERO;
        self.total_torque = Vec3::ZERO;
    }
}

// ============================================================
// EXPLOSION WAVE
// ============================================================

#[derive(Debug, Clone)]
pub struct ExplosionWave {
    pub center: Vec3,
    pub initial_radius: f32,
    pub current_radius: f32,
    pub max_radius: f32,
    pub propagation_speed: f32,
    pub peak_pressure: f32,
    pub decay_exponent: f32,
    pub active: bool,
    pub time: f32,
}
impl ExplosionWave {
    pub fn new(center: Vec3, peak_pressure: f32, max_radius: f32) -> Self {
        Self { center, initial_radius: 0.01, current_radius: 0.01, max_radius, propagation_speed: 340.0, peak_pressure, decay_exponent: 2.0, active: true, time: 0.0 }
    }
    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.current_radius = self.initial_radius + self.propagation_speed * self.time;
        if self.current_radius > self.max_radius { self.active = false; }
    }
    pub fn pressure_at(&self, dist: f32) -> f32 {
        if !self.active || dist < EPSILON { return 0.0; }
        if (dist - self.current_radius).abs() > 0.5 { return 0.0; }
        self.peak_pressure / (dist / self.initial_radius).powf(self.decay_exponent)
    }
    pub fn force_on_body(&self, body_pos: Vec3, body_cross_section: f32) -> Vec3 {
        let diff = body_pos - self.center;
        let dist = diff.length();
        let p = self.pressure_at(dist);
        if p < EPSILON { return Vec3::ZERO; }
        diff.normalize_or_zero() * p * body_cross_section
    }
}

// ============================================================
// PHYSICS UNIT CONVERSIONS
// ============================================================

pub fn kg_to_pounds(kg: f32) -> f32 { kg * 2.20462 }
pub fn pounds_to_kg(lbs: f32) -> f32 { lbs / 2.20462 }
pub fn meters_to_feet(m: f32) -> f32 { m * 3.28084 }
pub fn feet_to_meters(ft: f32) -> f32 { ft / 3.28084 }
pub fn nm_to_ftlb(nm: f32) -> f32 { nm * 0.737562 }
pub fn ftlb_to_nm(ftlb: f32) -> f32 { ftlb / 0.737562 }
pub fn kph_to_ms(kph: f32) -> f32 { kph / 3.6 }
pub fn ms_to_kph(ms: f32) -> f32 { ms * 3.6 }
pub fn mph_to_ms(mph: f32) -> f32 { mph * 0.44704 }
pub fn ms_to_mph(ms: f32) -> f32 { ms / 0.44704 }
pub fn pa_to_psi(pa: f32) -> f32 { pa * 0.000145038 }
pub fn psi_to_pa(psi: f32) -> f32 { psi / 0.000145038 }
pub fn rpm_to_rads(rpm: f32) -> f32 { rpm * TWO_PI / 60.0 }
pub fn rads_to_rpm(rads: f32) -> f32 { rads * 60.0 / TWO_PI }
pub fn horsepower_to_watts(hp: f32) -> f32 { hp * 745.7 }
pub fn watts_to_horsepower(w: f32) -> f32 { w / 745.7 }
pub fn kwh_to_joules(kwh: f32) -> f32 { kwh * 3_600_000.0 }
pub fn joules_to_kwh(j: f32) -> f32 { j / 3_600_000.0 }

// ============================================================
// EXTENDED FRICTION MODELS
// ============================================================

pub fn anisotropic_friction(relative_velocity: Vec3, normal: Vec3, friction_x: f32, friction_z: f32, local_x: Vec3) -> Vec3 {
    let local_z = normal.cross(local_x).normalize_or_zero();
    let vx = relative_velocity.dot(local_x);
    let vz = relative_velocity.dot(local_z);
    -(local_x * vx * friction_x + local_z * vz * friction_z)
}

pub fn rolling_friction_torque(normal_force: f32, roll_radius: f32, mu_r: f32) -> f32 {
    mu_r * normal_force * roll_radius
}

pub fn stribeck_friction(relative_speed: f32, mu_static: f32, mu_kinetic: f32, stribeck_speed: f32) -> f32 {
    let delta = mu_static - mu_kinetic;
    mu_kinetic + delta * (-relative_speed / stribeck_speed.max(EPSILON)).exp()
}

// ============================================================
// GJK SIMPLEX SOLVER
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct GjkSimplex {
    pub points: Vec<Vec3>,
}

impl GjkSimplex {
    pub fn new() -> Self { Self { points: Vec::new() } }
    pub fn add(&mut self, p: Vec3) { self.points.push(p); }
    pub fn len(&self) -> usize { self.points.len() }

    pub fn nearest_simplex(&mut self) -> Option<Vec3> {
        match self.len() {
            1 => Some(-self.points[0]),
            2 => self.line_case(),
            3 => self.triangle_case(),
            4 => self.tetrahedron_case(),
            _ => None,
        }
    }

    fn line_case(&mut self) -> Option<Vec3> {
        let a = self.points[1]; let b = self.points[0];
        let ab = b - a; let ao = -a;
        if ab.dot(ao) > 0.0 {
            Some(ab.cross(ao).cross(ab))
        } else {
            self.points = vec![a];
            Some(ao)
        }
    }

    fn triangle_case(&mut self) -> Option<Vec3> {
        let a = self.points[2]; let b = self.points[1]; let c = self.points[0];
        let ab = b - a; let ac = c - a; let ao = -a;
        let abc = ab.cross(ac);
        if abc.cross(ac).dot(ao) > 0.0 {
            if ac.dot(ao) > 0.0 {
                self.points = vec![c, a];
                return Some(ac.cross(ao).cross(ac));
            }
            self.points = vec![b, a];
            return self.line_case();
        }
        if ab.cross(abc).dot(ao) > 0.0 {
            self.points = vec![b, a];
            return self.line_case();
        }
        if abc.dot(ao) > 0.0 { Some(abc) } else { self.points = vec![b, c, a]; Some(-abc) }
    }

    fn tetrahedron_case(&mut self) -> Option<Vec3> {
        let a = self.points[3]; let b = self.points[2]; let c = self.points[1]; let d = self.points[0];
        let ab = b - a; let ac = c - a; let ad = d - a; let ao = -a;
        if ab.cross(ac).dot(ao) > 0.0 {
            self.points = vec![c, b, a];
            return self.triangle_case();
        }
        if ac.cross(ad).dot(ao) > 0.0 {
            self.points = vec![d, c, a];
            return self.triangle_case();
        }
        if ad.cross(ab).dot(ao) > 0.0 {
            self.points = vec![b, d, a];
            return self.triangle_case();
        }
        None
    }
}

pub fn gjk_intersect(
    support_a: impl Fn(Vec3) -> Vec3,
    support_b: impl Fn(Vec3) -> Vec3,
    initial_dir: Vec3, max_iter: u32,
) -> bool {
    let mut dir = if initial_dir.length_squared() > EPSILON { initial_dir.normalize() } else { Vec3::X };
    let mut simplex = GjkSimplex::new();
    let p0 = support_a(dir) - support_b(-dir);
    simplex.add(p0);
    let mut next_dir = -p0;
    for _ in 0..max_iter {
        if next_dir.length_squared() < EPSILON { return true; }
        let p = support_a(next_dir) - support_b(-next_dir);
        if p.dot(next_dir) < 0.0 { return false; }
        simplex.add(p);
        match simplex.nearest_simplex() {
            None => return true,
            Some(d) => next_dir = d,
        }
    }
    false
}

// ============================================================
// CONVEX HULL 3D
// ============================================================

#[derive(Debug, Clone)]
pub struct ConvexHullFace {
    pub vertices: [usize; 3],
    pub normal: Vec3,
}

pub struct ConvexHull3D {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<ConvexHullFace>,
}

impl ConvexHull3D {
    pub fn from_points(points: &[Vec3]) -> Self {
        if points.len() < 4 { return Self { vertices: points.to_vec(), faces: Vec::new() }; }
        let mut hull = Self { vertices: points.to_vec(), faces: Vec::new() };
        let (i0, i1, i2, i3) = find_initial_tetrahedron(points);
        let p0 = points[i0]; let p1 = points[i1]; let p2 = points[i2]; let p3 = points[i3];
        let n = (p1 - p0).cross(p2 - p0);
        let triplets: [(usize,usize,usize); 4] = if n.dot(p3 - p0) < 0.0 {
            [(i0,i1,i2),(i0,i3,i1),(i0,i2,i3),(i1,i3,i2)]
        } else {
            [(i0,i2,i1),(i0,i1,i3),(i0,i3,i2),(i1,i2,i3)]
        };
        for (a,b,c) in &triplets {
            let normal = (points[*b]-points[*a]).cross(points[*c]-points[*a]).normalize_or_zero();
            hull.faces.push(ConvexHullFace { vertices: [*a,*b,*c], normal });
        }
        hull
    }

    pub fn volume(&self) -> f32 {
        let mut vol = 0.0f32;
        for face in &self.faces {
            let a = self.vertices[face.vertices[0]];
            let b = self.vertices[face.vertices[1]];
            let c = self.vertices[face.vertices[2]];
            vol += a.dot(b.cross(c));
        }
        vol.abs() / 6.0
    }

    pub fn support(&self, dir: Vec3) -> Vec3 {
        self.vertices.iter().copied()
            .max_by(|a, b| a.dot(dir).partial_cmp(&b.dot(dir)).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(Vec3::ZERO)
    }

    pub fn aabb(&self) -> Aabb { Aabb::from_points(&self.vertices) }
}

fn find_initial_tetrahedron(points: &[Vec3]) -> (usize, usize, usize, usize) {
    let n = points.len();
    if n < 4 { return (0, 1.min(n-1), 2.min(n-1), 3.min(n-1)); }
    let i0 = 0usize;
    let i1 = (0..n).max_by(|&a,&b| (points[a]-points[i0]).length_squared().partial_cmp(&(points[b]-points[i0]).length_squared()).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(1);
    let i2 = (0..n).max_by(|&a,&b| {
        let d = (points[i1]-points[i0]).normalize_or_zero();
        ((points[a]-points[i0]).cross(d).length_squared()).partial_cmp(&(points[b]-points[i0]).cross(d).length_squared()).unwrap_or(std::cmp::Ordering::Equal)
    }).unwrap_or(2);
    let n012 = (points[i1]-points[i0]).cross(points[i2]-points[i0]).normalize_or_zero();
    let i3 = (0..n).max_by(|&a,&b| (points[a]-points[i0]).dot(n012).abs().partial_cmp(&(points[b]-points[i0]).dot(n012).abs()).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(3);
    (i0, i1, i2, i3)
}

// ============================================================
// PENDULUM CHAIN
// ============================================================

#[derive(Debug, Clone)]
pub struct PendulumChain {
    pub nodes: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub masses: Vec<f32>,
    pub lengths: Vec<f32>,
    pub fixed: Vec<bool>,
    pub gravity: Vec3,
    pub iterations: u32,
    pub damping: f32,
}
impl PendulumChain {
    pub fn new_simple(num_links: usize, link_length: f32, mass_per_link: f32, pivot: Vec3) -> Self {
        let mut nodes = Vec::new();
        let mut velocities = Vec::new();
        let mut masses = Vec::new();
        let mut lengths = Vec::new();
        let mut fixed = Vec::new();
        for i in 0..=num_links {
            nodes.push(pivot + Vec3::new(0.0, -(i as f32 * link_length), 0.0));
            velocities.push(Vec3::ZERO);
            masses.push(if i == 0 { 0.0 } else { mass_per_link });
            fixed.push(i == 0);
            if i > 0 { lengths.push(link_length); }
        }
        Self { nodes, velocities, masses, lengths, fixed, gravity: Vec3::new(0.0, -GRAVITY, 0.0), iterations: 20, damping: 0.999 }
    }

    pub fn integrate(&mut self, dt: f32) {
        let dt2 = dt * dt;
        for i in 0..self.nodes.len() {
            if self.fixed[i] { continue; }
            let prev = self.nodes[i] - self.velocities[i] * dt;
            let new_pos = self.nodes[i] * 2.0 - prev + self.gravity * dt2;
            self.velocities[i] = (new_pos - self.nodes[i]) / dt;
            self.velocities[i] *= self.damping;
            self.nodes[i] = new_pos;
        }
        for _ in 0..self.iterations {
            for j in 0..self.lengths.len() {
                let a = j; let b = j + 1;
                let delta = self.nodes[b] - self.nodes[a];
                let dist = delta.length();
                if dist < EPSILON { continue; }
                let diff = (dist - self.lengths[j]) / dist;
                let ma = if self.fixed[a] { f32::INFINITY } else { self.masses[a] };
                let mb = if self.fixed[b] { f32::INFINITY } else { self.masses[b] };
                let total = 1.0/ma + 1.0/mb;
                if total < EPSILON { continue; }
                let correction = delta * diff;
                if !self.fixed[a] { self.nodes[a] += correction * (1.0/ma) / total; }
                if !self.fixed[b] { self.nodes[b] -= correction * (1.0/mb) / total; }
            }
        }
    }

    pub fn total_energy(&self) -> f32 {
        let mut e = 0.0f32;
        for i in 0..self.nodes.len() {
            if self.fixed[i] { continue; }
            e += 0.5 * self.masses[i] * self.velocities[i].length_squared();
            e += self.masses[i] * (-self.gravity.y) * self.nodes[i].y;
        }
        e
    }
}

// ============================================================
// SHALLOW WATER EQUATIONS
// ============================================================

#[derive(Debug, Clone)]
pub struct ShallowWaterSim {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub depth: Vec<f32>,
    pub velocity_x: Vec<f32>,
    pub velocity_z: Vec<f32>,
    pub base_height: f32,
    pub gravity: f32,
    pub damping: f32,
}
impl ShallowWaterSim {
    pub fn new(width: usize, height: usize, cell_size: f32, base_h: f32) -> Self {
        let n = width * height;
        Self { width, height, cell_size, depth: vec![base_h; n], velocity_x: vec![0.0; (width+1)*height], velocity_z: vec![0.0; width*(height+1)], base_height: base_h, gravity: GRAVITY, damping: 0.999 }
    }
    pub fn perturb(&mut self, cx: usize, cz: usize, amplitude: f32) {
        for dz in -2i32..=2 {
            for dx in -2i32..=2 {
                let x = cx as i32 + dx; let z = cz as i32 + dz;
                if x >= 0 && x < self.width as i32 && z >= 0 && z < self.height as i32 {
                    let r = ((dx*dx+dz*dz) as f32).sqrt();
                    let w = (1.0 - r/3.0).max(0.0);
                    self.depth[z as usize * self.width + x as usize] += amplitude * w;
                }
            }
        }
    }
    pub fn step(&mut self, dt: f32) {
        let w = self.width; let h = self.height;
        let dx = self.cell_size; let g = self.gravity; let damp = self.damping;
        for z in 0..h {
            for x in 1..w {
                let grad = (self.depth[z*w+x] - self.depth[z*w+(x-1)]) / dx;
                let vi = z*(w+1)+x;
                self.velocity_x[vi] -= g * grad * dt;
                self.velocity_x[vi] *= damp;
            }
        }
        for z in 1..h {
            for x in 0..w {
                let grad = (self.depth[z*w+x] - self.depth[(z-1)*w+x]) / dx;
                let vi = z*w+x;
                self.velocity_z[vi] -= g * grad * dt;
                self.velocity_z[vi] *= damp;
            }
        }
        let mut new_depth = self.depth.clone();
        for z in 0..h {
            for x in 0..w {
                let vx_r = self.velocity_x[z*(w+1)+x+1];
                let vx_l = self.velocity_x[z*(w+1)+x];
                let vz_t = if z+1 < h { self.velocity_z[(z+1)*w+x] } else { 0.0 };
                let vz_b = self.velocity_z[z*w+x];
                new_depth[z*w+x] -= self.depth[z*w+x] * (vx_r - vx_l + vz_t - vz_b) / dx * dt;
            }
        }
        self.depth = new_depth;
    }
    pub fn height_at(&self, x: usize, z: usize) -> f32 {
        if x < self.width && z < self.height { self.depth[z*self.width+x] } else { self.base_height }
    }
}

// ============================================================
// PHYSICS ANIMATION CURVES
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CurveLoopMode { Once, Loop, PingPong }

#[derive(Debug, Clone)]
pub struct PhysicsPropertyCurve {
    pub name: String,
    pub keys: Vec<(f32, f32)>,
    pub loop_mode: CurveLoopMode,
}
impl PhysicsPropertyCurve {
    pub fn new(name: &str) -> Self { Self { name: name.to_string(), keys: Vec::new(), loop_mode: CurveLoopMode::Once } }
    pub fn add_key(&mut self, t: f32, v: f32) {
        self.keys.push((t, v));
        self.keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }
    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 0.0; }
        let dur = self.keys.last().map(|k| k.0).unwrap_or(0.0);
        let tw = match self.loop_mode {
            CurveLoopMode::Once => t.clamp(0.0, dur),
            CurveLoopMode::Loop => if dur > EPSILON { t % dur } else { 0.0 },
            CurveLoopMode::PingPong => {
                if dur > EPSILON { let c = t % (2.0*dur); if c <= dur { c } else { 2.0*dur-c } } else { 0.0 }
            }
        };
        for i in 0..self.keys.len()-1 {
            if tw >= self.keys[i].0 && tw <= self.keys[i+1].0 {
                let dt = self.keys[i+1].0 - self.keys[i].0;
                let frac = if dt > EPSILON { (tw - self.keys[i].0) / dt } else { 0.0 };
                return self.keys[i].1 + (self.keys[i+1].1 - self.keys[i].1) * frac;
            }
        }
        self.keys.last().map(|k| k.1).unwrap_or(0.0)
    }
    pub fn duration(&self) -> f32 { self.keys.last().map(|k| k.0).unwrap_or(0.0) }
}

// ============================================================
// PHYSICS EDITOR COMPLETE
// ============================================================
