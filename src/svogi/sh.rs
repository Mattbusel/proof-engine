use glam::{Vec3, Mat4};

/// 2nd order spherical harmonics (L=0,1,2 => 9 coefficients).
#[derive(Debug, Clone, Copy)]
pub struct SH2 {
    pub coeffs: [f32; 9],
}

impl Default for SH2 {
    fn default() -> Self {
        Self { coeffs: [0.0; 9] }
    }
}

/// 3rd order spherical harmonics (L=0,1,2,3 => 16 coefficients).
#[derive(Debug, Clone, Copy)]
pub struct SH3 {
    pub coeffs: [f32; 16],
}

impl Default for SH3 {
    fn default() -> Self {
        Self { coeffs: [0.0; 16] }
    }
}

/// Factorial helper.
fn factorial(n: u32) -> f64 {
    (1..=n as u64).fold(1.0f64, |acc, x| acc * x as f64)
}

/// Double factorial: n!! = n * (n-2) * (n-4) * ...
fn double_factorial(n: i32) -> f64 {
    if n <= 0 { return 1.0; }
    let mut result = 1.0f64;
    let mut k = n;
    while k > 0 {
        result *= k as f64;
        k -= 2;
    }
    result
}

/// Associated Legendre polynomial P_l^m(x).
pub fn legendre_p(l: i32, m: i32, x: f64) -> f64 {
    let m_abs = m.abs();

    if m_abs > l {
        return 0.0;
    }

    // Compute P_m^m using the formula P_m^m(x) = (-1)^m * (2m-1)!! * (1-x^2)^(m/2)
    let mut pmm = 1.0f64;
    if m_abs > 0 {
        let somx2 = ((1.0 - x) * (1.0 + x)).sqrt();
        let mut fact = 1.0f64;
        for i in 1..=m_abs {
            pmm *= -fact * somx2;
            fact += 2.0;
        }
    }

    if l == m_abs {
        if m < 0 {
            let sign = if m_abs % 2 == 0 { 1.0 } else { -1.0 };
            return sign * factorial((l - m_abs) as u32) as f64 / factorial((l + m_abs) as u32) as f64 * pmm;
        }
        return pmm;
    }

    // P_{m+1}^m(x) = x * (2m+1) * P_m^m(x)
    let mut pmmp1 = x * (2 * m_abs + 1) as f64 * pmm;

    if l == m_abs + 1 {
        if m < 0 {
            let sign = if m_abs % 2 == 0 { 1.0 } else { -1.0 };
            return sign * factorial((l - m_abs) as u32) as f64 / factorial((l + m_abs) as u32) as f64 * pmmp1;
        }
        return pmmp1;
    }

    // Use recurrence: (l-m)*P_l^m = x*(2l-1)*P_{l-1}^m - (l+m-1)*P_{l-2}^m
    let mut pll = 0.0f64;
    for ll in (m_abs + 2)..=l {
        pll = (x * (2 * ll - 1) as f64 * pmmp1 - (ll + m_abs - 1) as f64 * pmm) / (ll - m_abs) as f64;
        pmm = pmmp1;
        pmmp1 = pll;
    }

    if m < 0 {
        let sign = if m_abs % 2 == 0 { 1.0 } else { -1.0 };
        return sign * factorial((l - m_abs) as u32) as f64 / factorial((l + m_abs) as u32) as f64 * pll;
    }
    pll
}

/// SH normalization constant K_l^m.
fn sh_k(l: i32, m: i32) -> f64 {
    let m_abs = m.abs();
    let num = (2 * l + 1) as f64 * factorial((l - m_abs) as u32) as f64;
    let den = 4.0 * std::f64::consts::PI * factorial((l + m_abs) as u32) as f64;
    (num / den).sqrt()
}

/// Evaluate the real SH basis function Y_l^m at direction (theta, phi).
fn sh_basis_real(l: i32, m: i32, theta: f64, phi: f64) -> f64 {
    let k = sh_k(l, m);
    let p = legendre_p(l, m.abs(), theta.cos());
    if m > 0 {
        std::f64::consts::SQRT_2 * k * (m as f64 * phi).cos() * p
    } else if m < 0 {
        std::f64::consts::SQRT_2 * k * ((-m) as f64 * phi).sin() * p
    } else {
        k * p
    }
}

/// Direction to spherical coordinates (theta, phi).
fn dir_to_spherical(dir: Vec3) -> (f64, f64) {
    let d = dir.normalize_or_zero();
    let theta = (d.z as f64).acos();
    let phi = (d.y as f64).atan2(d.x as f64);
    (theta, phi)
}

/// Evaluate 2nd order SH basis at a direction. Returns 9 values.
pub fn sh_basis_2(dir: Vec3) -> [f32; 9] {
    let d = dir.normalize_or_zero();
    let x = d.x;
    let y = d.y;
    let z = d.z;

    [
        0.282095,                            // Y_0^0
        0.488603 * y,                        // Y_1^{-1}
        0.488603 * z,                        // Y_1^0
        0.488603 * x,                        // Y_1^1
        1.092548 * x * y,                    // Y_2^{-2}
        1.092548 * y * z,                    // Y_2^{-1}
        0.315392 * (3.0 * z * z - 1.0),     // Y_2^0
        1.092548 * x * z,                    // Y_2^1
        0.546274 * (x * x - y * y),          // Y_2^2
    ]
}

/// Evaluate 3rd order SH basis at a direction. Returns 16 values.
pub fn sh_basis_3(dir: Vec3) -> [f32; 16] {
    let d = dir.normalize_or_zero();
    let x = d.x;
    let y = d.y;
    let z = d.z;

    let mut result = [0.0f32; 16];

    // L=0
    result[0] = 0.282095;

    // L=1
    result[1] = 0.488603 * y;
    result[2] = 0.488603 * z;
    result[3] = 0.488603 * x;

    // L=2
    result[4] = 1.092548 * x * y;
    result[5] = 1.092548 * y * z;
    result[6] = 0.315392 * (3.0 * z * z - 1.0);
    result[7] = 1.092548 * x * z;
    result[8] = 0.546274 * (x * x - y * y);

    // L=3
    result[9]  = 0.590044 * y * (3.0 * x * x - y * y);
    result[10] = 2.890611 * x * y * z;
    result[11] = 0.457046 * y * (5.0 * z * z - 1.0);
    result[12] = 0.373176 * z * (5.0 * z * z - 3.0);
    result[13] = 0.457046 * x * (5.0 * z * z - 1.0);
    result[14] = 1.445306 * z * (x * x - y * y);
    result[15] = 0.590044 * x * (x * x - 3.0 * y * y);

    result
}

/// Evaluate SH at a direction given coefficients.
pub fn sh_evaluate(coeffs: &[f32], dir: Vec3) -> f32 {
    if coeffs.len() >= 16 {
        let basis = sh_basis_3(dir);
        coeffs.iter().zip(basis.iter()).take(16).map(|(c, b)| c * b).sum()
    } else if coeffs.len() >= 9 {
        let basis = sh_basis_2(dir);
        coeffs.iter().zip(basis.iter()).take(9).map(|(c, b)| c * b).sum()
    } else {
        let basis = sh_basis_2(dir);
        coeffs.iter().zip(basis.iter()).map(|(c, b)| c * b).sum()
    }
}

/// Project a function into 2nd order SH via Monte Carlo sampling.
pub fn sh_project_function(
    sample_fn: impl Fn(Vec3) -> f32,
    num_samples: usize,
) -> SH2 {
    let mut result = SH2::default();
    let weight = 4.0 * std::f32::consts::PI / num_samples as f32;

    // Use stratified sampling on the sphere
    let n_sqrt = (num_samples as f32).sqrt().ceil() as usize;
    let mut count = 0;
    for i in 0..n_sqrt {
        for j in 0..n_sqrt {
            if count >= num_samples {
                break;
            }
            // Stratified spherical coordinates
            let u = (i as f32 + 0.5) / n_sqrt as f32;
            let v = (j as f32 + 0.5) / n_sqrt as f32;

            let theta = (1.0 - 2.0 * u).acos();
            let phi = 2.0 * std::f32::consts::PI * v;

            let dir = Vec3::new(
                theta.sin() * phi.cos(),
                theta.sin() * phi.sin(),
                theta.cos(),
            );

            let value = sample_fn(dir);
            let basis = sh_basis_2(dir);
            for k in 0..9 {
                result.coeffs[k] += value * basis[k] * weight;
            }
            count += 1;
        }
    }

    result
}

/// Convolve SH with a zonal harmonic kernel.
pub fn sh_convolve(a: &SH2, kernel: &[f32]) -> SH2 {
    let mut result = SH2::default();
    // Band 0: 1 coefficient
    if kernel.len() > 0 {
        result.coeffs[0] = a.coeffs[0] * kernel[0];
    }
    // Band 1: 3 coefficients
    if kernel.len() > 1 {
        for i in 1..4 {
            result.coeffs[i] = a.coeffs[i] * kernel[1];
        }
    }
    // Band 2: 5 coefficients
    if kernel.len() > 2 {
        for i in 4..9 {
            result.coeffs[i] = a.coeffs[i] * kernel[2];
        }
    }
    result
}

/// Rotate SH by a rotation matrix. Uses the ZYZ Euler angle decomposition.
pub fn sh_rotate(coeffs: &SH2, rotation: Mat4) -> SH2 {
    let mut result = SH2::default();

    // Band 0 is rotation-invariant
    result.coeffs[0] = coeffs.coeffs[0];

    // Band 1: rotate using the upper-left 3x3 of the matrix
    // The band-1 SH transform directly as a 3-vector
    let r = rotation;
    let sh1 = [coeffs.coeffs[3], coeffs.coeffs[1], coeffs.coeffs[2]]; // (x, y, z) order

    // Apply rotation
    let rx = r.x_axis;
    let ry = r.y_axis;
    let rz = r.z_axis;

    let rotated_x = rx.x * sh1[0] + ry.x * sh1[1] + rz.x * sh1[2];
    let rotated_y = rx.y * sh1[0] + ry.y * sh1[1] + rz.y * sh1[2];
    let rotated_z = rx.z * sh1[0] + ry.z * sh1[1] + rz.z * sh1[2];

    result.coeffs[3] = rotated_x; // Y_1^1  -> x
    result.coeffs[1] = rotated_y; // Y_1^-1 -> y
    result.coeffs[2] = rotated_z; // Y_1^0  -> z

    // Band 2: approximate rotation via reprojection
    // For each of the 5 band-2 coefficients, we reproject
    // This is a simplified approach using the real SH rotation property
    let dirs = [
        Vec3::X, Vec3::Y, Vec3::Z,
        Vec3::new(1.0, 1.0, 0.0).normalize(),
        Vec3::new(1.0, 0.0, 1.0).normalize(),
        Vec3::new(0.0, 1.0, 1.0).normalize(),
        Vec3::new(1.0, -1.0, 0.0).normalize(),
        Vec3::new(-1.0, 0.0, 1.0).normalize(),
        Vec3::new(0.0, -1.0, 1.0).normalize(),
    ];

    // Compute band-2 rotation matrix using sampling
    let mut band2_coeffs = [0.0f32; 5];
    let weight = 4.0 * std::f32::consts::PI / dirs.len() as f32;
    for &dir in &dirs {
        let original_basis = sh_basis_2(dir);
        let original_val: f32 = (4..9).map(|i| coeffs.coeffs[i] * original_basis[i]).sum();

        let rot3 = glam::Mat3::from_mat4(rotation);
        let rotated_dir = rot3 * dir;
        let rotated_basis = sh_basis_2(rotated_dir);

        for i in 0..5 {
            band2_coeffs[i] += original_val * rotated_basis[i + 4] * weight;
        }
    }

    for i in 0..5 {
        result.coeffs[4 + i] = band2_coeffs[i];
    }

    result
}

/// Add two SH2.
pub fn sh_add(a: &SH2, b: &SH2) -> SH2 {
    let mut result = SH2::default();
    for i in 0..9 {
        result.coeffs[i] = a.coeffs[i] + b.coeffs[i];
    }
    result
}

/// Scale SH2.
pub fn sh_scale(a: &SH2, s: f32) -> SH2 {
    let mut result = SH2::default();
    for i in 0..9 {
        result.coeffs[i] = a.coeffs[i] * s;
    }
    result
}

/// Inner product of two SH2.
pub fn sh_dot(a: &SH2, b: &SH2) -> f32 {
    (0..9).map(|i| a.coeffs[i] * b.coeffs[i]).sum()
}

/// SH representation of a clamped cosine lobe (for diffuse transfer).
pub fn cosine_lobe_sh() -> SH2 {
    // Clamped cosine along +Z in SH
    let mut sh = SH2::default();
    sh.coeffs[0] = 0.886227;   // sqrt(pi) / 2
    sh.coeffs[2] = 1.023326;   // sqrt(pi/3)
    sh.coeffs[6] = 0.495415;   // sqrt(5*pi) / 8
    sh
}

/// SH irradiance probe.
#[derive(Debug, Clone)]
pub struct SHProbe {
    pub position: Vec3,
    pub sh_r: SH2,
    pub sh_g: SH2,
    pub sh_b: SH2,
}

impl SHProbe {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            sh_r: SH2::default(),
            sh_g: SH2::default(),
            sh_b: SH2::default(),
        }
    }

    /// Evaluate irradiance in a given direction.
    pub fn evaluate(&self, direction: Vec3) -> Vec3 {
        let basis = sh_basis_2(direction);
        let r: f32 = (0..9).map(|i| self.sh_r.coeffs[i] * basis[i]).sum();
        let g: f32 = (0..9).map(|i| self.sh_g.coeffs[i] * basis[i]).sum();
        let b: f32 = (0..9).map(|i| self.sh_b.coeffs[i] * basis[i]).sum();
        Vec3::new(r.max(0.0), g.max(0.0), b.max(0.0))
    }

    /// Add a directional light sample.
    pub fn add_sample(&mut self, direction: Vec3, color: Vec3) {
        let basis = sh_basis_2(direction);
        for i in 0..9 {
            self.sh_r.coeffs[i] += color.x * basis[i];
            self.sh_g.coeffs[i] += color.y * basis[i];
            self.sh_b.coeffs[i] += color.z * basis[i];
        }
    }
}

/// Evaluate 3-channel irradiance from 3 sets of SH2 coefficients.
pub fn sh_to_color_9(sh_r: &SH2, sh_g: &SH2, sh_b: &SH2, dir: Vec3) -> Vec3 {
    let basis = sh_basis_2(dir);
    let r: f32 = (0..9).map(|i| sh_r.coeffs[i] * basis[i]).sum();
    let g: f32 = (0..9).map(|i| sh_g.coeffs[i] * basis[i]).sum();
    let b: f32 = (0..9).map(|i| sh_b.coeffs[i] * basis[i]).sum();
    Vec3::new(r.max(0.0), g.max(0.0), b.max(0.0))
}

impl SH2 {
    /// Total energy (L2 norm).
    pub fn energy(&self) -> f32 {
        self.coeffs.iter().map(|c| c * c).sum()
    }

    /// Evaluate at a direction.
    pub fn evaluate(&self, dir: Vec3) -> f32 {
        sh_evaluate(&self.coeffs, dir)
    }

    /// Project a direction with a value.
    pub fn project(&mut self, dir: Vec3, value: f32) {
        let basis = sh_basis_2(dir);
        for i in 0..9 {
            self.coeffs[i] += value * basis[i];
        }
    }
}

impl SH3 {
    pub fn energy(&self) -> f32 {
        self.coeffs.iter().map(|c| c * c).sum()
    }

    pub fn evaluate(&self, dir: Vec3) -> f32 {
        sh_evaluate(&self.coeffs, dir)
    }

    pub fn project(&mut self, dir: Vec3, value: f32) {
        let basis = sh_basis_3(dir);
        for i in 0..16 {
            self.coeffs[i] += value * basis[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sh_basis_2_normalization() {
        // The L=0 coefficient should be constant 0.282095
        let b = sh_basis_2(Vec3::X);
        assert!((b[0] - 0.282095).abs() < 1e-4);

        let b2 = sh_basis_2(Vec3::Y);
        assert!((b2[0] - 0.282095).abs() < 1e-4);
    }

    #[test]
    fn test_sh_basis_3_length() {
        let b = sh_basis_3(Vec3::Z);
        assert_eq!(b.len(), 16);
    }

    #[test]
    fn test_sh_orthogonality() {
        // SH basis functions should be approximately orthogonal
        // Integrate Y_i * Y_j over the sphere using Monte Carlo
        let n = 10000;
        let n_sqrt = (n as f32).sqrt().ceil() as usize;
        let weight = 4.0 * std::f32::consts::PI / (n_sqrt * n_sqrt) as f32;

        // Test orthogonality between band 0 and band 1
        let mut dot_01 = 0.0f32;
        let mut dot_00 = 0.0f32;
        let mut dot_11 = 0.0f32;

        for i in 0..n_sqrt {
            for j in 0..n_sqrt {
                let u = (i as f32 + 0.5) / n_sqrt as f32;
                let v = (j as f32 + 0.5) / n_sqrt as f32;
                let theta = (1.0 - 2.0 * u).acos();
                let phi = 2.0 * std::f32::consts::PI * v;
                let dir = Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.sin() * phi.sin(),
                    theta.cos(),
                );
                let basis = sh_basis_2(dir);
                dot_00 += basis[0] * basis[0] * weight;
                dot_01 += basis[0] * basis[1] * weight;
                dot_11 += basis[1] * basis[1] * weight;
            }
        }

        assert!((dot_00 - 1.0).abs() < 0.15, "Y00 self-dot should be ~1, got {dot_00}");
        assert!(dot_01.abs() < 0.15, "Y00.Y1-1 should be ~0, got {dot_01}");
        assert!((dot_11 - 1.0).abs() < 0.15, "Y1-1 self-dot should be ~1, got {dot_11}");
    }

    #[test]
    fn test_sh_project_then_evaluate() {
        // Project a function that's strong in the +Y direction
        let sh = sh_project_function(
            |dir| dir.y.max(0.0),
            2500,
        );

        let val_up = sh.evaluate(Vec3::Y);
        let val_down = sh.evaluate(-Vec3::Y);

        assert!(val_up > val_down, "Projected function should be stronger in +Y: up={val_up}, down={val_down}");
        assert!(val_up > 0.0);
    }

    #[test]
    fn test_sh_rotation_preserves_energy() {
        let mut sh = SH2::default();
        sh.project(Vec3::new(1.0, 1.0, 0.0).normalize(), 1.0);
        let original_energy = sh.energy();

        let rotation = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let rotated = sh_rotate(&sh, rotation);

        let rotated_energy = rotated.energy();
        // Energy should be approximately preserved
        let ratio = rotated_energy / original_energy;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Energy should be roughly preserved: original={original_energy}, rotated={rotated_energy}"
        );
    }

    #[test]
    fn test_sh_add_scale() {
        let mut a = SH2::default();
        a.coeffs[0] = 1.0;
        a.coeffs[1] = 2.0;

        let mut b = SH2::default();
        b.coeffs[0] = 3.0;
        b.coeffs[1] = 4.0;

        let sum = sh_add(&a, &b);
        assert!((sum.coeffs[0] - 4.0).abs() < 1e-6);
        assert!((sum.coeffs[1] - 6.0).abs() < 1e-6);

        let scaled = sh_scale(&a, 2.0);
        assert!((scaled.coeffs[0] - 2.0).abs() < 1e-6);
        assert!((scaled.coeffs[1] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_sh_dot() {
        let mut a = SH2::default();
        a.coeffs[0] = 1.0;
        let mut b = SH2::default();
        b.coeffs[0] = 2.0;
        assert!((sh_dot(&a, &b) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_lobe() {
        let cosine = cosine_lobe_sh();
        let val_up = cosine.evaluate(Vec3::Z);
        let val_side = cosine.evaluate(Vec3::X);
        let val_down = cosine.evaluate(-Vec3::Z);

        assert!(val_up > val_side, "Cosine lobe should be strongest at Z");
        assert!(val_side >= val_down, "Cosine lobe should be weaker below horizon");
    }

    #[test]
    fn test_sh_probe() {
        let mut probe = SHProbe::new(Vec3::ZERO);
        probe.add_sample(Vec3::Y, Vec3::new(1.0, 0.0, 0.0));

        let color_up = probe.evaluate(Vec3::Y);
        let color_down = probe.evaluate(-Vec3::Y);
        assert!(color_up.x > color_down.x, "Probe should be brighter in sample direction");
    }

    #[test]
    fn test_sh_convolve() {
        let mut sh = SH2::default();
        sh.coeffs[0] = 1.0;
        sh.coeffs[2] = 0.5;

        let kernel = [
            std::f32::consts::PI,
            2.0 * std::f32::consts::PI / 3.0,
            std::f32::consts::PI / 4.0,
        ];
        let convolved = sh_convolve(&sh, &kernel);
        assert!((convolved.coeffs[0] - std::f32::consts::PI).abs() < 1e-4);
    }

    #[test]
    fn test_legendre_p() {
        // P_0^0(x) = 1
        assert!((legendre_p(0, 0, 0.5) - 1.0).abs() < 1e-6);
        // P_1^0(x) = x
        assert!((legendre_p(1, 0, 0.5) - 0.5).abs() < 1e-6);
        // P_2^0(x) = (3x^2 - 1)/2
        let x = 0.5;
        let expected = (3.0 * x * x - 1.0) / 2.0;
        assert!((legendre_p(2, 0, x) - expected).abs() < 1e-6);
    }

    #[test]
    fn test_sh3_project_evaluate() {
        let mut sh = SH3::default();
        sh.project(Vec3::Z, 1.0);
        let val = sh.evaluate(Vec3::Z);
        assert!(val > 0.0);
    }

    #[test]
    fn test_sh_to_color_9() {
        let mut r = SH2::default();
        r.coeffs[0] = 1.0;
        let g = SH2::default();
        let b = SH2::default();
        let color = sh_to_color_9(&r, &g, &b, Vec3::X);
        assert!(color.x > 0.0);
        assert!((color.y).abs() < 1e-6);
    }

    #[test]
    fn test_factorial() {
        assert!((factorial(0) - 1.0).abs() < 1e-10);
        assert!((factorial(5) - 120.0).abs() < 1e-10);
    }
}
