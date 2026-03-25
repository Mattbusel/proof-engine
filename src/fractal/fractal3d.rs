//! 3D fractals — Mandelbulb, Mandelbox via ray marching on signed distance fields.

use glam::Vec3;

#[derive(Debug, Clone)]
pub struct RayMarchParams {
    pub max_steps: u32,
    pub max_dist: f32,
    pub min_dist: f32,
    pub power: f32,
}
impl Default for RayMarchParams {
    fn default() -> Self { Self { max_steps: 128, max_dist: 100.0, min_dist: 0.001, power: 8.0 } }
}

pub struct RayMarchResult { pub distance: f32, pub steps: u32, pub hit: bool, pub position: Vec3 }

/// Mandelbulb SDF.
pub struct Mandelbulb { pub power: f32, pub max_iter: u32 }
impl Default for Mandelbulb { fn default() -> Self { Self { power: 8.0, max_iter: 15 } } }
impl Mandelbulb {
    pub fn sdf(&self, p: Vec3) -> f32 {
        let mut z = p;
        let mut dr = 1.0f32;
        let mut r = 0.0f32;
        for _ in 0..self.max_iter {
            r = z.length();
            if r > 2.0 { break; }
            let theta = (z.z / r).acos();
            let phi = z.y.atan2(z.x);
            dr = r.powf(self.power - 1.0) * self.power * dr + 1.0;
            let zr = r.powf(self.power);
            let t = theta * self.power;
            let ph = phi * self.power;
            z = Vec3::new(t.sin() * ph.cos(), t.sin() * ph.sin(), t.cos()) * zr + p;
        }
        0.5 * r.ln() * r / dr
    }

    pub fn ray_march(&self, origin: Vec3, direction: Vec3, params: &RayMarchParams) -> RayMarchResult {
        let mut t = 0.0f32;
        for step in 0..params.max_steps {
            let p = origin + direction * t;
            let d = self.sdf(p);
            if d < params.min_dist { return RayMarchResult { distance: t, steps: step, hit: true, position: p }; }
            if t > params.max_dist { break; }
            t += d;
        }
        RayMarchResult { distance: t, steps: params.max_steps, hit: false, position: origin + direction * t }
    }
}

/// Mandelbox SDF.
pub struct Mandelbox { pub scale: f32, pub max_iter: u32, pub fold_limit: f32 }
impl Default for Mandelbox { fn default() -> Self { Self { scale: 2.0, max_iter: 15, fold_limit: 1.0 } } }
impl Mandelbox {
    pub fn sdf(&self, p: Vec3) -> f32 {
        let mut z = p;
        let mut dr = 1.0f32;
        let fl = self.fold_limit;
        for _ in 0..self.max_iter {
            // Box fold
            z = Vec3::new(
                if z.x > fl { 2.0 * fl - z.x } else if z.x < -fl { -2.0 * fl - z.x } else { z.x },
                if z.y > fl { 2.0 * fl - z.y } else if z.y < -fl { -2.0 * fl - z.y } else { z.y },
                if z.z > fl { 2.0 * fl - z.z } else if z.z < -fl { -2.0 * fl - z.z } else { z.z },
            );
            // Sphere fold
            let r2 = z.length_squared();
            if r2 < 0.25 { let t = 4.0; z *= t; dr *= t; }
            else if r2 < 1.0 { let t = 1.0 / r2; z *= t; dr *= t; }
            z = z * self.scale + p;
            dr = dr * self.scale.abs() + 1.0;
        }
        z.length() / dr.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mandelbulb_sdf_at_origin() {
        let mb = Mandelbulb::default();
        let d = mb.sdf(Vec3::ZERO);
        assert!(d < 0.1, "Origin should be near/inside the bulb, got {d}");
    }
    #[test]
    fn mandelbulb_sdf_far_away() {
        let mb = Mandelbulb::default();
        let d = mb.sdf(Vec3::new(10.0, 0.0, 0.0));
        assert!(d > 1.0, "Far point should be outside, got {d}");
    }
    #[test]
    fn mandelbox_evaluates() {
        let mb = Mandelbox::default();
        let _d = mb.sdf(Vec3::new(0.5, 0.5, 0.5));
    }
    #[test]
    fn ray_march_hits_mandelbulb() {
        let mb = Mandelbulb::default();
        let result = mb.ray_march(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0), &RayMarchParams::default());
        assert!(result.hit, "Ray toward origin should hit the Mandelbulb");
    }
}
