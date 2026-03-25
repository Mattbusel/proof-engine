//! Newton fractal for arbitrary polynomials.

/// A polynomial represented as coefficients [a₀, a₁, ..., aₙ] for a₀ + a₁x + ... + aₙxⁿ.
#[derive(Debug, Clone)]
pub struct Polynomial { pub coeffs: Vec<(f64, f64)> } // complex coefficients (re, im)

impl Polynomial {
    pub fn real(coeffs: &[f64]) -> Self {
        Self { coeffs: coeffs.iter().map(|&c| (c, 0.0)).collect() }
    }

    pub fn eval(&self, z: (f64, f64)) -> (f64, f64) {
        let mut result = (0.0, 0.0);
        let mut zn = (1.0, 0.0);
        for &c in &self.coeffs {
            result = (result.0 + c.0 * zn.0 - c.1 * zn.1, result.1 + c.0 * zn.1 + c.1 * zn.0);
            zn = (zn.0 * z.0 - zn.1 * z.1, zn.0 * z.1 + zn.1 * z.0);
        }
        result
    }

    pub fn derivative(&self) -> Polynomial {
        if self.coeffs.len() <= 1 { return Polynomial { coeffs: vec![(0.0, 0.0)] }; }
        let coeffs: Vec<(f64, f64)> = self.coeffs.iter().enumerate().skip(1)
            .map(|(i, &(re, im))| (re * i as f64, im * i as f64)).collect();
        Polynomial { coeffs }
    }
}

/// Newton fractal renderer.
pub struct NewtonFractal {
    pub poly: Polynomial,
    pub max_iter: u32,
    pub tolerance: f64,
}

/// Result of one Newton pixel.
#[derive(Debug, Clone, Copy)]
pub struct NewtonPixel {
    pub root_index: i32, // -1 if not converged
    pub iterations: u32,
    pub final_z: (f64, f64),
}

impl NewtonFractal {
    pub fn new(poly: Polynomial, max_iter: u32) -> Self {
        Self { poly, max_iter, tolerance: 1e-6 }
    }

    /// z³ - 1 = 0 (classic Newton fractal with 3 roots).
    pub fn cubic_roots() -> Self {
        Self::new(Polynomial::real(&[-1.0, 0.0, 0.0, 1.0]), 64)
    }

    pub fn compute_pixel(&self, z_re: f64, z_im: f64, roots: &[(f64, f64)]) -> NewtonPixel {
        let dp = self.poly.derivative();
        let mut z = (z_re, z_im);

        for iter in 0..self.max_iter {
            let fz = self.poly.eval(z);
            let dfz = dp.eval(z);
            let denom = dfz.0 * dfz.0 + dfz.1 * dfz.1;
            if denom < 1e-20 { break; }
            z.0 -= (fz.0 * dfz.0 + fz.1 * dfz.1) / denom;
            z.1 -= (fz.1 * dfz.0 - fz.0 * dfz.1) / denom;

            // Check convergence to known roots
            for (ri, &root) in roots.iter().enumerate() {
                let dr = z.0 - root.0;
                let di = z.1 - root.1;
                if dr * dr + di * di < self.tolerance * self.tolerance {
                    return NewtonPixel { root_index: ri as i32, iterations: iter, final_z: z };
                }
            }
        }
        NewtonPixel { root_index: -1, iterations: self.max_iter, final_z: z }
    }

    /// Render the full Newton fractal.
    pub fn render(&self, width: u32, height: u32, center: (f64, f64), zoom: f64, roots: &[(f64, f64)]) -> Vec<NewtonPixel> {
        let scale = 2.0 / zoom;
        let aspect = width as f64 / height as f64;
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for py in 0..height { for px in 0..width {
            let z_re = center.0 + (px as f64 / width as f64 - 0.5) * scale * aspect;
            let z_im = center.1 + (py as f64 / height as f64 - 0.5) * scale;
            pixels.push(self.compute_pixel(z_re, z_im, roots));
        }}
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cubic_roots_converges() {
        let nf = NewtonFractal::cubic_roots();
        let roots = vec![(1.0, 0.0), (-0.5, 0.866), (-0.5, -0.866)];
        // Point near root 0
        let p = nf.compute_pixel(0.9, 0.0, &roots);
        assert_eq!(p.root_index, 0);
    }
    #[test]
    fn polynomial_eval() {
        let p = Polynomial::real(&[1.0, 0.0, 1.0]); // 1 + x²
        let (re, im) = p.eval((2.0, 0.0));
        assert!((re - 5.0).abs() < 1e-10);
    }
    #[test]
    fn derivative_correct() {
        let p = Polynomial::real(&[0.0, 0.0, 0.0, 1.0]); // x³
        let dp = p.derivative(); // 3x²
        let (re, _) = dp.eval((2.0, 0.0));
        assert!((re - 12.0).abs() < 1e-10);
    }
}
