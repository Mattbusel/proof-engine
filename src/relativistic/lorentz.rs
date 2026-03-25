//! Lorentz contraction and transformations.

use glam::{Vec2, Vec3, Vec4};

/// Compute the Lorentz factor gamma = 1 / sqrt(1 - v^2/c^2).
/// Clamps v to be strictly less than c.
pub fn lorentz_factor(v: f64, c: f64) -> f64 {
    let beta = (v / c).abs();
    if beta >= 1.0 {
        return f64::INFINITY;
    }
    1.0 / (1.0 - beta * beta).sqrt()
}

/// A four-vector in Minkowski spacetime with signature (+, -, -, -).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FourVector {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl FourVector {
    pub fn new(t: f64, x: f64, y: f64, z: f64) -> Self {
        Self { t, x, y, z }
    }

    pub fn zero() -> Self {
        Self { t: 0.0, x: 0.0, y: 0.0, z: 0.0 }
    }

    /// Minkowski dot product with signature (+, -, -, -).
    pub fn dot(&self, other: &FourVector) -> f64 {
        self.t * other.t - self.x * other.x - self.y * other.y - self.z * other.z
    }

    /// Minkowski norm squared (invariant interval).
    pub fn norm_sq(&self) -> f64 {
        self.dot(self)
    }

    /// Minkowski norm. Returns NaN for spacelike vectors if you take sqrt of negative.
    pub fn norm(&self) -> f64 {
        let ns = self.norm_sq();
        if ns >= 0.0 {
            ns.sqrt()
        } else {
            -(-ns).sqrt()
        }
    }

    /// Spatial part as a Vec3.
    pub fn spatial(&self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }

    /// Spatial magnitude.
    pub fn spatial_magnitude(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Apply a Lorentz boost along an arbitrary direction.
    pub fn boost(&self, velocity: Vec3, c: f64) -> FourVector {
        boost(self, velocity, c)
    }

    /// Scale by a scalar.
    pub fn scale(&self, s: f64) -> FourVector {
        FourVector::new(self.t * s, self.x * s, self.y * s, self.z * s)
    }

    /// Add two four-vectors.
    pub fn add(&self, other: &FourVector) -> FourVector {
        FourVector::new(
            self.t + other.t,
            self.x + other.x,
            self.y + other.y,
            self.z + other.z,
        )
    }

    /// Subtract another four-vector.
    pub fn sub(&self, other: &FourVector) -> FourVector {
        FourVector::new(
            self.t - other.t,
            self.x - other.x,
            self.y - other.y,
            self.z - other.z,
        )
    }
}

impl std::ops::Add for FourVector {
    type Output = FourVector;
    fn add(self, rhs: FourVector) -> FourVector {
        FourVector::new(self.t + rhs.t, self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for FourVector {
    type Output = FourVector;
    fn sub(self, rhs: FourVector) -> FourVector {
        FourVector::new(self.t - rhs.t, self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f64> for FourVector {
    type Output = FourVector;
    fn mul(self, rhs: f64) -> FourVector {
        FourVector::new(self.t * rhs, self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl std::ops::Neg for FourVector {
    type Output = FourVector;
    fn neg(self) -> FourVector {
        FourVector::new(-self.t, -self.x, -self.y, -self.z)
    }
}

/// Lorentz boost parameters.
#[derive(Debug, Clone)]
pub struct LorentzBoost {
    pub velocity: Vec3,
    pub gamma: f64,
}

impl LorentzBoost {
    pub fn new(velocity: Vec3, c: f64) -> Self {
        let v = (velocity.length() as f64).min(c * 0.9999999);
        Self {
            velocity,
            gamma: lorentz_factor(v, c),
        }
    }

    pub fn beta(&self, c: f64) -> f64 {
        self.velocity.length() as f64 / c
    }

    /// Apply this boost to a four-vector.
    pub fn apply(&self, fv: &FourVector, c: f64) -> FourVector {
        boost(fv, self.velocity, c)
    }

    /// Inverse boost (negate velocity).
    pub fn inverse(&self) -> LorentzBoost {
        LorentzBoost {
            velocity: -self.velocity,
            gamma: self.gamma,
        }
    }
}

/// General Lorentz boost of a four-vector along an arbitrary velocity direction.
///
/// Uses the standard formula for a boost along direction n = v/|v|:
///   t' = gamma (t - (v . r) / c^2)
///   r' = r + (gamma - 1)(r . n)n - gamma v t
///
/// where r = (x, y, z) spatial part.
pub fn boost(four_vec: &FourVector, velocity: Vec3, c: f64) -> FourVector {
    let vx = velocity.x as f64;
    let vy = velocity.y as f64;
    let vz = velocity.z as f64;
    let v_mag = (vx * vx + vy * vy + vz * vz).sqrt();

    if v_mag < 1e-15 {
        return *four_vec;
    }

    let gamma = lorentz_factor(v_mag, c);
    let nx = vx / v_mag;
    let ny = vy / v_mag;
    let nz = vz / v_mag;

    // r dot n
    let r_dot_n = four_vec.x * nx + four_vec.y * ny + four_vec.z * nz;
    // r dot v
    let r_dot_v = four_vec.x * vx + four_vec.y * vy + four_vec.z * vz;

    let t_prime = gamma * (four_vec.t - r_dot_v / (c * c));
    let coeff = (gamma - 1.0) * r_dot_n;
    let x_prime = four_vec.x + coeff * nx - gamma * vx * four_vec.t;
    let y_prime = four_vec.y + coeff * ny - gamma * vy * four_vec.t;
    let z_prime = four_vec.z + coeff * nz - gamma * vz * four_vec.t;

    FourVector::new(t_prime, x_prime, y_prime, z_prime)
}

/// Length contraction: L = L_0 / gamma.
pub fn contract_length(proper_length: f64, v: f64, c: f64) -> f64 {
    let gamma = lorentz_factor(v, c);
    proper_length / gamma
}

/// Proper time: tau = t / gamma (coordinate time to proper time).
pub fn proper_time(coordinate_time: f64, v: f64, c: f64) -> f64 {
    let gamma = lorentz_factor(v, c);
    coordinate_time / gamma
}

/// Relativistic velocity addition: w = (v1 + v2) / (1 + v1*v2/c^2).
pub fn velocity_addition(v1: f64, v2: f64, c: f64) -> f64 {
    (v1 + v2) / (1.0 + v1 * v2 / (c * c))
}

/// Rapidity: phi = atanh(v/c).
pub fn rapidity(v: f64, c: f64) -> f64 {
    (v / c).atanh()
}

/// Four-momentum from mass and 3-velocity.
/// p^mu = (gamma*m*c, gamma*m*vx, gamma*m*vy, gamma*m*vz)
pub fn four_momentum(mass: f64, velocity: Vec3, c: f64) -> FourVector {
    let vx = velocity.x as f64;
    let vy = velocity.y as f64;
    let vz = velocity.z as f64;
    let v_mag = (vx * vx + vy * vy + vz * vz).sqrt();
    let gamma = lorentz_factor(v_mag, c);
    FourVector::new(
        gamma * mass * c,
        gamma * mass * vx,
        gamma * mass * vy,
        gamma * mass * vz,
    )
}

/// Relativistic energy: E = gamma * m * c^2.
pub fn relativistic_energy(mass: f64, v: f64, c: f64) -> f64 {
    lorentz_factor(v, c) * mass * c * c
}

/// Relativistic momentum magnitude: p = gamma * m * v.
pub fn relativistic_momentum(mass: f64, v: f64, c: f64) -> f64 {
    lorentz_factor(v, c) * mass * v
}

/// Invariant mass from energy and momentum: m^2 = E^2/c^4 - p^2/c^2.
/// Returns the mass (taking sqrt, returns 0 for tachyonic cases).
pub fn invariant_mass(energy: f64, momentum: f64, c: f64) -> f64 {
    let m_sq = energy * energy / (c * c * c * c) - momentum * momentum / (c * c);
    if m_sq >= 0.0 {
        m_sq.sqrt()
    } else {
        0.0
    }
}

/// Modifies entity visual scale based on velocity relative to observer.
/// Contracts along the direction of motion by 1/gamma.
#[derive(Debug, Clone)]
pub struct LorentzContractor {
    pub c: f64,
}

impl LorentzContractor {
    pub fn new(c: f64) -> Self {
        Self { c }
    }

    /// Compute contracted scale for an entity moving at `velocity` relative to observer.
    /// Returns (scale_x, scale_y, scale_z) where the motion direction is contracted.
    pub fn contracted_scale(&self, velocity: Vec3) -> Vec3 {
        let v = velocity.length() as f64;
        if v < 1e-12 {
            return Vec3::ONE;
        }
        let gamma = lorentz_factor(v, self.c);
        let contraction = (1.0 / gamma) as f32;
        let dir = velocity.normalize();

        // Contract along the motion direction, keep perpendicular unchanged.
        // scale = I + (contraction - 1) * (dir outer dir)
        // For a unit vector, the contracted component = contraction, others = 1
        let sx = 1.0 + (contraction - 1.0) * dir.x * dir.x;
        let sy = 1.0 + (contraction - 1.0) * dir.y * dir.y;
        let sz = 1.0 + (contraction - 1.0) * dir.z * dir.z;

        Vec3::new(sx, sy, sz)
    }

    /// Apply contraction to a list of vertex positions around a center.
    pub fn contract_vertices(&self, vertices: &[Vec3], center: Vec3, velocity: Vec3) -> Vec<Vec3> {
        let v = velocity.length() as f64;
        if v < 1e-12 {
            return vertices.to_vec();
        }
        let gamma = lorentz_factor(v, self.c);
        let contraction = (1.0 / gamma) as f32;
        let dir = velocity.normalize();

        vertices.iter().map(|vert| {
            let rel = *vert - center;
            let along = rel.dot(dir) * dir;
            let perp = rel - along;
            center + along * contraction + perp
        }).collect()
    }
}

/// Render objects with velocity-dependent squish along motion direction.
#[derive(Debug, Clone)]
pub struct LorentzRenderer {
    pub c: f64,
    pub contraction_enabled: bool,
    pub color_shift_enabled: bool,
}

impl LorentzRenderer {
    pub fn new(c: f64) -> Self {
        Self {
            c,
            contraction_enabled: true,
            color_shift_enabled: false,
        }
    }

    /// Compute the apparent position of a vertex given object velocity.
    /// Contracts along the direction of motion.
    pub fn apparent_vertex(
        &self,
        vertex: Vec3,
        object_center: Vec3,
        velocity: Vec3,
    ) -> Vec3 {
        if !self.contraction_enabled {
            return vertex;
        }
        let v = velocity.length() as f64;
        if v < 1e-12 {
            return vertex;
        }
        let gamma = lorentz_factor(v, self.c);
        let contraction = (1.0 / gamma) as f32;
        let dir = velocity.normalize();
        let rel = vertex - object_center;
        let along = rel.dot(dir) * dir;
        let perp = rel - along;
        object_center + along * contraction + perp
    }

    /// Render a set of entity glyphs with Lorentz contraction applied.
    pub fn render_glyphs(
        &self,
        positions: &[Vec3],
        center: Vec3,
        velocity: Vec3,
    ) -> Vec<Vec3> {
        positions.iter().map(|p| self.apparent_vertex(*p, center, velocity)).collect()
    }

    /// Compute velocity-dependent brightness factor.
    /// Objects moving toward observer appear brighter due to beaming.
    pub fn brightness_factor(&self, velocity: Vec3, observer_dir: Vec3) -> f32 {
        let v = velocity.length() as f64;
        if v < 1e-12 {
            return 1.0;
        }
        let gamma = lorentz_factor(v, self.c);
        let beta = v / self.c;
        let dir = velocity.normalize();
        let cos_theta = dir.dot(observer_dir.normalize()) as f64;
        let doppler = gamma * (1.0 - beta * cos_theta);
        if doppler > 1e-10 {
            (1.0 / doppler).powi(3) as f32
        } else {
            1.0
        }
    }

    /// Apply full Lorentz rendering transform to a set of glyph data.
    /// Returns (contracted_positions, brightness_factors).
    pub fn transform_entity(
        &self,
        positions: &[Vec3],
        center: Vec3,
        velocity: Vec3,
        observer_pos: Vec3,
    ) -> (Vec<Vec3>, Vec<f32>) {
        let new_positions = self.render_glyphs(positions, center, velocity);
        let observer_dir = (observer_pos - center).normalize_or_zero();
        let brightness = positions.iter().map(|_| self.brightness_factor(velocity, observer_dir)).collect();
        (new_positions, brightness)
    }
}

/// Compute the kinetic energy: KE = (gamma - 1) * m * c^2.
pub fn kinetic_energy(mass: f64, v: f64, c: f64) -> f64 {
    (lorentz_factor(v, c) - 1.0) * mass * c * c
}

/// Convert between rapidity and velocity.
pub fn velocity_from_rapidity(phi: f64, c: f64) -> f64 {
    c * phi.tanh()
}

/// Compose two boosts by adding rapidities (for collinear boosts).
pub fn compose_collinear_boosts(v1: f64, v2: f64, c: f64) -> f64 {
    let phi1 = rapidity(v1, c);
    let phi2 = rapidity(v2, c);
    velocity_from_rapidity(phi1 + phi2, c)
}

/// Relativistic Doppler factor for radial motion.
pub fn doppler_factor(v: f64, c: f64, approaching: bool) -> f64 {
    let beta = v / c;
    if approaching {
        ((1.0 + beta) / (1.0 - beta)).sqrt()
    } else {
        ((1.0 - beta) / (1.0 + beta)).sqrt()
    }
}

/// Four-velocity from 3-velocity.
pub fn four_velocity(velocity: Vec3, c: f64) -> FourVector {
    let vx = velocity.x as f64;
    let vy = velocity.y as f64;
    let vz = velocity.z as f64;
    let v_mag = (vx * vx + vy * vy + vz * vz).sqrt();
    let gamma = lorentz_factor(v_mag, c);
    FourVector::new(gamma * c, gamma * vx, gamma * vy, gamma * vz)
}

/// Check if a four-vector is timelike (norm_sq > 0).
pub fn is_timelike(fv: &FourVector) -> bool {
    fv.norm_sq() > 0.0
}

/// Check if a four-vector is spacelike (norm_sq < 0).
pub fn is_spacelike(fv: &FourVector) -> bool {
    fv.norm_sq() < 0.0
}

/// Check if a four-vector is lightlike/null (norm_sq ~ 0).
pub fn is_lightlike(fv: &FourVector, tolerance: f64) -> bool {
    fv.norm_sq().abs() < tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: f64 = 299_792_458.0; // speed of light in m/s

    #[test]
    fn test_lorentz_factor_zero() {
        let g = lorentz_factor(0.0, C);
        assert!((g - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_lorentz_factor_high_v() {
        let g = lorentz_factor(0.99 * C, C);
        let expected = 1.0 / (1.0 - 0.99 * 0.99_f64).sqrt();
        assert!((g - expected).abs() < 1e-6);
    }

    #[test]
    fn test_lorentz_factor_approaches_infinity() {
        let g = lorentz_factor(0.9999999 * C, C);
        assert!(g > 1000.0);
        let g2 = lorentz_factor(C, C);
        assert!(g2.is_infinite());
    }

    #[test]
    fn test_four_vector_minkowski_dot() {
        let a = FourVector::new(5.0, 1.0, 2.0, 3.0);
        let b = FourVector::new(3.0, 1.0, 1.0, 1.0);
        // dot = 5*3 - 1*1 - 2*1 - 3*1 = 15 - 6 = 9
        assert!((a.dot(&b) - 9.0).abs() < 1e-10);
    }

    #[test]
    fn test_four_vector_norm_sq() {
        let v = FourVector::new(5.0, 3.0, 0.0, 0.0);
        // norm_sq = 25 - 9 = 16
        assert!((v.norm_sq() - 16.0).abs() < 1e-10);
    }

    #[test]
    fn test_boost_zero_velocity() {
        let fv = FourVector::new(1.0, 2.0, 3.0, 4.0);
        let boosted = boost(&fv, Vec3::ZERO, C);
        assert!((boosted.t - fv.t).abs() < 1e-10);
        assert!((boosted.x - fv.x).abs() < 1e-10);
    }

    #[test]
    fn test_boost_preserves_interval() {
        let fv = FourVector::new(10.0, 1.0, 2.0, 0.0);
        let original_interval = fv.norm_sq();
        let boosted = boost(&fv, Vec3::new(0.5 * C as f32, 0.0, 0.0), C);
        let boosted_interval = boosted.norm_sq();
        assert!(
            (original_interval - boosted_interval).abs() < 1e-3,
            "Interval not preserved: {} vs {}",
            original_interval,
            boosted_interval
        );
    }

    #[test]
    fn test_contract_length() {
        let L0 = 10.0;
        let L = contract_length(L0, 0.0, C);
        assert!((L - 10.0).abs() < 1e-10);

        let L2 = contract_length(L0, 0.866 * C, C);
        // gamma ~ 2, so L ~ 5
        assert!((L2 - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_velocity_addition_subluminal() {
        let w = velocity_addition(0.5 * C, 0.5 * C, C);
        assert!(w < C);
        let expected = (0.5 * C + 0.5 * C) / (1.0 + 0.25);
        assert!((w - expected).abs() < 1e-6);
    }

    #[test]
    fn test_velocity_addition_light_speed() {
        // Adding c to anything should give c
        let w = velocity_addition(C, 0.5 * C, C);
        assert!((w - C).abs() < 1e-6);
    }

    #[test]
    fn test_rapidity() {
        let phi = rapidity(0.0, C);
        assert!(phi.abs() < 1e-10);

        let phi2 = rapidity(0.5 * C, C);
        assert!((phi2 - (0.5_f64).atanh()).abs() < 1e-10);
    }

    #[test]
    fn test_energy_momentum_relation() {
        // E^2 = p^2 c^2 + m^2 c^4
        let mass = 1.0;
        let v = 0.8 * C;
        let E = relativistic_energy(mass, v, C);
        let p = relativistic_momentum(mass, v, C);
        let lhs = E * E;
        let rhs = p * p * C * C + mass * mass * C * C * C * C;
        assert!(
            (lhs - rhs).abs() / lhs < 1e-10,
            "E^2 = p^2 c^2 + m^2 c^4 failed: {} vs {}",
            lhs, rhs
        );
    }

    #[test]
    fn test_invariant_mass_recovery() {
        let mass = 2.5;
        let v = 0.6 * C;
        let E = relativistic_energy(mass, v, C);
        let p = relativistic_momentum(mass, v, C);
        let m_recovered = invariant_mass(E, p, C);
        assert!((m_recovered - mass).abs() < 1e-6);
    }

    #[test]
    fn test_four_momentum_norm() {
        let mass = 1.0;
        let vel = Vec3::new(0.3 * C as f32, 0.4 * C as f32, 0.0);
        let pm = four_momentum(mass, vel, C);
        // p^mu p_mu = m^2 c^2 (with our convention p0 = gamma*m*c)
        let norm = pm.norm_sq();
        let expected = mass * mass * C * C;
        assert!(
            (norm - expected).abs() / expected < 1e-6,
            "Four-momentum norm: {} vs {}",
            norm, expected
        );
    }

    #[test]
    fn test_proper_time() {
        let t = 10.0;
        let tau = proper_time(t, 0.866 * C, C);
        // gamma ~ 2, tau ~ 5
        assert!((tau - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_lorentz_contraction_renderer() {
        let contactor = LorentzContractor::new(C);
        let scale = contactor.contracted_scale(Vec3::new(0.866 * C as f32, 0.0, 0.0));
        // gamma ~ 2, contraction along x ~ 0.5
        assert!((scale.x - 0.5).abs() < 0.05);
        assert!((scale.y - 1.0).abs() < 0.01);
        assert!((scale.z - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_compose_collinear_boosts() {
        let w = compose_collinear_boosts(0.5 * C, 0.5 * C, C);
        let w2 = velocity_addition(0.5 * C, 0.5 * C, C);
        assert!((w - w2).abs() < 1e-6);
    }

    #[test]
    fn test_kinetic_energy_low_v() {
        // At low velocities, KE ~ 0.5 * m * v^2
        let mass = 1.0;
        let v = 0.001 * C;
        let ke_rel = kinetic_energy(mass, v, C);
        let ke_classical = 0.5 * mass * v * v;
        assert!(
            (ke_rel - ke_classical).abs() / ke_classical < 0.01,
            "Low v KE: rel={} classical={}",
            ke_rel, ke_classical
        );
    }

    #[test]
    fn test_four_velocity_norm() {
        let vel = Vec3::new(0.5 * C as f32, 0.0, 0.0);
        let u = four_velocity(vel, C);
        // u^mu u_mu = c^2
        let norm = u.norm_sq();
        assert!(
            (norm - C * C).abs() / (C * C) < 1e-6,
            "Four-velocity norm: {} vs {}",
            norm, C * C
        );
    }
}
