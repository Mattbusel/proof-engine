// topology/curvature.rs — Curvature-dependent gameplay mechanics

use glam::Vec3;

// ─── Gaussian Curvature ────────────────────────────────────────────────────

/// Gaussian curvature at a point, encapsulating the curvature value and surface info.
#[derive(Clone, Copy, Debug)]
pub struct GaussianCurvature {
    pub value: f32,
}

impl GaussianCurvature {
    pub fn new(value: f32) -> Self {
        Self { value }
    }

    pub fn is_positive(&self) -> bool {
        self.value > 0.0
    }

    pub fn is_negative(&self) -> bool {
        self.value < 0.0
    }

    pub fn is_flat(&self) -> bool {
        self.value.abs() < 1e-6
    }
}

/// Surface types for curvature computation.
#[derive(Clone, Copy, Debug)]
pub enum CurvatureSurfaceType {
    /// Sphere of given radius. K = 1/r^2
    Sphere(f32),
    /// Hyperbolic surface of given "radius". K = -1/r^2
    Hyperbolic(f32),
    /// Flat plane. K = 0
    Flat,
    /// Torus at a given position. K varies: positive outside, negative inside.
    Torus { major_r: f32, minor_r: f32 },
    /// Saddle surface z = x^2 - y^2 scaled by `a`
    Saddle(f32),
}

/// Compute the Gaussian curvature at a point on a given surface type.
pub fn curvature_at(surface: CurvatureSurfaceType, pos: Vec3) -> f32 {
    match surface {
        CurvatureSurfaceType::Sphere(r) => {
            if r.abs() < 1e-10 {
                return f32::INFINITY;
            }
            1.0 / (r * r)
        }
        CurvatureSurfaceType::Hyperbolic(r) => {
            if r.abs() < 1e-10 {
                return f32::NEG_INFINITY;
            }
            -1.0 / (r * r)
        }
        CurvatureSurfaceType::Flat => 0.0,
        CurvatureSurfaceType::Torus { major_r, minor_r } => {
            // On a torus parameterized by (u, v):
            // K = cos(v) / (minor_r * (major_r + minor_r * cos(v)))
            // We need to find v from the 3D position.
            let u = pos.y.atan2(pos.x);
            let center_x = major_r * u.cos();
            let center_y = major_r * u.sin();
            let dx = pos.x - center_x;
            let dy = pos.y - center_y;
            let r_xy = (dx * dx + dy * dy).sqrt();
            let cos_v = (r_xy - 0.0) / minor_r.max(1e-6); // approximate
            let cos_v = cos_v.clamp(-1.0, 1.0);
            let v = if pos.z >= 0.0 {
                cos_v.acos()
            } else {
                -cos_v.acos()
            };
            let cv = v.cos();
            let denom = minor_r * (major_r + minor_r * cv);
            if denom.abs() < 1e-10 {
                return 0.0;
            }
            cv / denom
        }
        CurvatureSurfaceType::Saddle(a) => {
            // z = a(x^2 - y^2)
            // K = -4a^2 / (1 + 4a^2(x^2 + y^2))^2
            let x = pos.x;
            let y = pos.y;
            let denom = 1.0 + 4.0 * a * a * (x * x + y * y);
            -4.0 * a * a / (denom * denom)
        }
    }
}

// ─── Curved Gravity ────────────────────────────────────────────────────────

/// Modify gravitational acceleration based on local curvature.
/// Positive curvature focuses gravity (stronger), negative curvature defocuses (weaker).
pub fn curved_gravity(pos: Vec3, curvature: f32, mass: f32) -> Vec3 {
    let g = 9.81;
    // Curvature modifies the effective gravitational strength
    let curvature_factor = 1.0 + curvature * mass;
    let effective_g = g * curvature_factor;

    // Gravity points "down" in the local coordinate system
    Vec3::new(0.0, 0.0, -effective_g)
}

// ─── Curved Projectile ─────────────────────────────────────────────────────

/// Compute the next position of a projectile in curved space.
/// Positive curvature bends the path inward, negative bends it outward.
pub fn curved_projectile(start: Vec3, velocity: Vec3, curvature: f32, dt: f32) -> Vec3 {
    // In curved space, geodesic deviation causes trajectories to converge (K>0) or diverge (K<0).
    // Simple model: add a correction term proportional to curvature.
    let gravity = Vec3::new(0.0, 0.0, -9.81);

    // Curvature-induced deflection perpendicular to velocity
    let speed = velocity.length();
    let deflection = if speed > 1e-6 && curvature.abs() > 1e-8 {
        let v_norm = velocity / speed;
        // Deflect toward/away from origin based on curvature sign
        let to_origin = -start.normalize_or_zero();
        let perp = to_origin - v_norm * to_origin.dot(v_norm);
        perp * curvature * speed * speed * dt
    } else {
        Vec3::ZERO
    };

    start + velocity * dt + gravity * 0.5 * dt * dt + deflection
}

// ─── Curvature Field ───────────────────────────────────────────────────────

/// A 2D grid of curvature values that affect gameplay.
pub struct CurvatureField {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub values: Vec<f32>,
}

impl CurvatureField {
    /// Create a flat curvature field (all zeros).
    pub fn flat(width: usize, height: usize, cell_size: f32) -> Self {
        Self {
            width,
            height,
            cell_size,
            values: vec![0.0; width * height],
        }
    }

    /// Create a curvature field from a function.
    pub fn from_fn<F: Fn(f32, f32) -> f32>(width: usize, height: usize, cell_size: f32, f: F) -> Self {
        let mut values = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let wx = x as f32 * cell_size;
                let wy = y as f32 * cell_size;
                values.push(f(wx, wy));
            }
        }
        Self {
            width,
            height,
            cell_size,
            values,
        }
    }

    /// Set curvature at a grid cell.
    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.values[y * self.width + x] = value;
        }
    }

    /// Get curvature at a grid cell.
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.values[y * self.width + x]
        } else {
            0.0
        }
    }

    /// Sample the curvature field at a world position using bilinear interpolation.
    pub fn sample(&self, pos_x: f32, pos_y: f32) -> f32 {
        let gx = pos_x / self.cell_size;
        let gy = pos_y / self.cell_size;

        let x0 = gx.floor() as i32;
        let y0 = gy.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;

        let fx = gx - x0 as f32;
        let fy = gy - y0 as f32;

        let get_clamped = |x: i32, y: i32| -> f32 {
            let cx = (x.max(0) as usize).min(self.width - 1);
            let cy = (y.max(0) as usize).min(self.height - 1);
            self.values[cy * self.width + cx]
        };

        let c00 = get_clamped(x0, y0);
        let c10 = get_clamped(x1, y0);
        let c01 = get_clamped(x0, y1);
        let c11 = get_clamped(x1, y1);

        let c0 = c00 * (1.0 - fx) + c10 * fx;
        let c1 = c01 * (1.0 - fx) + c11 * fx;
        c0 * (1.0 - fy) + c1 * fy
    }

    /// Add a spherical curvature bump at a world position.
    pub fn add_bump(&mut self, center_x: f32, center_y: f32, radius: f32, magnitude: f32) {
        let r_cells = (radius / self.cell_size).ceil() as i32;
        let cx = (center_x / self.cell_size) as i32;
        let cy = (center_y / self.cell_size) as i32;

        for dy in -r_cells..=r_cells {
            for dx in -r_cells..=r_cells {
                let gx = cx + dx;
                let gy = cy + dy;
                if gx >= 0 && gx < self.width as i32 && gy >= 0 && gy < self.height as i32 {
                    let wx = gx as f32 * self.cell_size;
                    let wy = gy as f32 * self.cell_size;
                    let dist = ((wx - center_x).powi(2) + (wy - center_y).powi(2)).sqrt();
                    if dist < radius {
                        let falloff = 1.0 - dist / radius;
                        let falloff = falloff * falloff; // quadratic falloff
                        self.values[gy as usize * self.width + gx as usize] += magnitude * falloff;
                    }
                }
            }
        }
    }
}

/// Apply curvature field to entity movement.
/// Returns (new_position, new_velocity) after one timestep.
pub fn apply_curvature_to_movement(
    pos: Vec3,
    vel: Vec3,
    dt: f32,
    field: &CurvatureField,
) -> (Vec3, Vec3) {
    let k = field.sample(pos.x, pos.y);

    // Curvature modifies velocity direction (geodesic deviation)
    let speed = vel.length();
    let mut new_vel = vel;

    if speed > 1e-6 && k.abs() > 1e-8 {
        // Compute gradient of curvature field for deflection direction
        let eps = field.cell_size;
        let dk_dx = (field.sample(pos.x + eps, pos.y) - field.sample(pos.x - eps, pos.y)) / (2.0 * eps);
        let dk_dy = (field.sample(pos.x, pos.y + eps) - field.sample(pos.x, pos.y - eps)) / (2.0 * eps);
        let grad = Vec3::new(dk_dx, dk_dy, 0.0);

        // Deflect velocity toward higher curvature (like a lens)
        new_vel = vel + grad * speed * dt;

        // Preserve speed (curvature bends, doesn't accelerate)
        let new_speed = new_vel.length();
        if new_speed > 1e-6 {
            new_vel = new_vel * (speed / new_speed);
        }
    }

    // Apply effective gravity modification
    let gravity = Vec3::new(0.0, 0.0, -9.81 * (1.0 + k));
    new_vel = new_vel + gravity * dt;

    let new_pos = pos + new_vel * dt;
    (new_pos, new_vel)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_curvature() {
        let k = curvature_at(CurvatureSurfaceType::Sphere(2.0), Vec3::ZERO);
        assert!((k - 0.25).abs() < 1e-6, "Sphere R=2 should have K=0.25, got {}", k);
    }

    #[test]
    fn test_hyperbolic_curvature() {
        let k = curvature_at(CurvatureSurfaceType::Hyperbolic(1.0), Vec3::ZERO);
        assert!((k - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_flat_curvature() {
        let k = curvature_at(CurvatureSurfaceType::Flat, Vec3::new(5.0, 3.0, 0.0));
        assert_eq!(k, 0.0);
    }

    #[test]
    fn test_saddle_curvature_at_origin() {
        let k = curvature_at(CurvatureSurfaceType::Saddle(1.0), Vec3::ZERO);
        // At origin: K = -4a^2 / 1 = -4
        assert!((k - (-4.0)).abs() < 1e-4, "Saddle K at origin should be -4, got {}", k);
    }

    #[test]
    fn test_saddle_curvature_negative() {
        let k = curvature_at(CurvatureSurfaceType::Saddle(0.5), Vec3::new(1.0, 1.0, 0.0));
        assert!(k < 0.0, "Saddle surface should have negative curvature");
    }

    #[test]
    fn test_gaussian_curvature_classification() {
        let pos = GaussianCurvature::new(1.0);
        assert!(pos.is_positive());
        assert!(!pos.is_negative());
        assert!(!pos.is_flat());

        let neg = GaussianCurvature::new(-1.0);
        assert!(neg.is_negative());

        let flat = GaussianCurvature::new(0.0);
        assert!(flat.is_flat());
    }

    #[test]
    fn test_curved_gravity_positive_curvature() {
        let g = curved_gravity(Vec3::ZERO, 1.0, 1.0);
        // With positive curvature, gravity should be stronger
        assert!(g.z < -9.81);
    }

    #[test]
    fn test_curved_gravity_zero_curvature() {
        let g = curved_gravity(Vec3::ZERO, 0.0, 1.0);
        assert!((g.z - (-9.81)).abs() < 1e-4);
    }

    #[test]
    fn test_curved_projectile_flat() {
        let start = Vec3::new(0.0, 0.0, 10.0);
        let vel = Vec3::new(10.0, 0.0, 0.0);
        let next = curved_projectile(start, vel, 0.0, 0.1);
        // Should be normal projectile motion
        let expected_x = 0.0 + 10.0 * 0.1;
        assert!((next.x - expected_x).abs() < 1e-3);
    }

    #[test]
    fn test_curvature_field_flat() {
        let field = CurvatureField::flat(10, 10, 1.0);
        assert_eq!(field.sample(5.0, 5.0), 0.0);
    }

    #[test]
    fn test_curvature_field_set_get() {
        let mut field = CurvatureField::flat(10, 10, 1.0);
        field.set(3, 4, 1.5);
        assert!((field.get(3, 4) - 1.5).abs() < 1e-6);
        assert_eq!(field.get(0, 0), 0.0);
    }

    #[test]
    fn test_curvature_field_sample_interpolation() {
        let mut field = CurvatureField::flat(10, 10, 1.0);
        field.set(2, 2, 4.0);
        // Sampling at (2.0, 2.0) should give 4.0
        let v = field.sample(2.0, 2.0);
        assert!((v - 4.0).abs() < 1e-4, "Expected 4.0, got {}", v);
        // Sampling between cells should interpolate
        let v2 = field.sample(2.5, 2.0);
        assert!(v2 < 4.0 && v2 > 0.0);
    }

    #[test]
    fn test_curvature_field_bump() {
        let mut field = CurvatureField::flat(20, 20, 1.0);
        field.add_bump(10.0, 10.0, 5.0, 2.0);
        let center_val = field.sample(10.0, 10.0);
        let edge_val = field.sample(14.0, 10.0);
        assert!(center_val > edge_val, "Center should be stronger than edge");
        assert!(center_val > 0.0);
    }

    #[test]
    fn test_curvature_field_from_fn() {
        let field = CurvatureField::from_fn(10, 10, 1.0, |x, y| x + y);
        assert!((field.get(0, 0) - 0.0).abs() < 1e-4);
        assert!((field.get(5, 3) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn test_apply_curvature_to_movement_flat() {
        let field = CurvatureField::flat(10, 10, 1.0);
        let pos = Vec3::new(5.0, 5.0, 0.0);
        let vel = Vec3::new(1.0, 0.0, 0.0);
        let (new_pos, _new_vel) = apply_curvature_to_movement(pos, vel, 0.01, &field);
        // Should just move forward (plus tiny gravity)
        assert!((new_pos.x - 5.01).abs() < 1e-3);
    }

    #[test]
    fn test_apply_curvature_preserves_speed_approximately() {
        let mut field = CurvatureField::flat(20, 20, 1.0);
        field.add_bump(10.0, 10.0, 5.0, 1.0);
        let pos = Vec3::new(9.0, 10.0, 0.0);
        let vel = Vec3::new(5.0, 0.0, 0.0);
        let speed_before = (vel.x * vel.x + vel.y * vel.y).sqrt();
        let (_new_pos, new_vel) = apply_curvature_to_movement(pos, vel, 0.01, &field);
        let speed_after = (new_vel.x * new_vel.x + new_vel.y * new_vel.y).sqrt();
        // Horizontal speed should be roughly preserved (gravity adds z component)
        assert!((speed_before - speed_after).abs() < 1.0,
            "Speed changed too much: {} -> {}", speed_before, speed_after);
    }
}
