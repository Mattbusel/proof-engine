//! Random matrix theory: GOE, GUE, Wishart matrices, eigenvalue computation,
//! and spectral distribution analysis (Wigner semicircle, Marchenko-Pastur).

use super::brownian::Rng;
use super::monte_carlo::Histogram;
use glam::Vec2;

// ---------------------------------------------------------------------------
// RandomMatrix
// ---------------------------------------------------------------------------

/// A dense matrix stored in row-major order.
pub struct RandomMatrix {
    pub rows: usize,
    pub cols: usize,
    pub entries: Vec<f64>,
}

impl RandomMatrix {
    /// Create a zero matrix.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            entries: vec![0.0; rows * cols],
        }
    }

    /// Access element (i, j).
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.entries[i * self.cols + j]
    }

    /// Set element (i, j).
    pub fn set(&mut self, i: usize, j: usize, val: f64) {
        self.entries[i * self.cols + j] = val;
    }

    /// Create an identity matrix.
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, 1.0);
        }
        m
    }

    /// Matrix multiplication.
    pub fn mul(&self, other: &RandomMatrix) -> RandomMatrix {
        assert_eq!(self.cols, other.rows);
        let mut result = RandomMatrix::zeros(self.rows, other.cols);
        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += self.get(i, k) * other.get(k, j);
                }
                result.set(i, j, sum);
            }
        }
        result
    }

    /// Transpose.
    pub fn transpose(&self) -> RandomMatrix {
        let mut result = RandomMatrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                result.set(j, i, self.get(i, j));
            }
        }
        result
    }

    /// Frobenius norm.
    pub fn frobenius_norm(&self) -> f64 {
        self.entries.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Trace (sum of diagonal elements).
    pub fn trace(&self) -> f64 {
        let n = self.rows.min(self.cols);
        (0..n).map(|i| self.get(i, i)).sum()
    }
}

// ---------------------------------------------------------------------------
// Random matrix ensembles
// ---------------------------------------------------------------------------

/// Gaussian Orthogonal Ensemble: symmetric matrix with N(0,1) off-diagonal
/// and N(0,2) diagonal entries, divided by sqrt(2n).
pub fn goe(n: usize, rng: &mut Rng) -> RandomMatrix {
    let mut m = RandomMatrix::zeros(n, n);
    let scale = 1.0 / (2.0 * n as f64).sqrt();
    for i in 0..n {
        m.set(i, i, rng.normal() * (2.0_f64).sqrt() * scale);
        for j in (i + 1)..n {
            let val = rng.normal() * scale;
            m.set(i, j, val);
            m.set(j, i, val);
        }
    }
    m
}

/// Gaussian Unitary Ensemble (real approximation): symmetric matrix similar
/// to GOE but with different normalization. For a real approximation, we
/// use the same structure as GOE with scale 1/sqrt(n).
pub fn gue(n: usize, rng: &mut Rng) -> RandomMatrix {
    let mut m = RandomMatrix::zeros(n, n);
    let scale = 1.0 / (n as f64).sqrt();
    for i in 0..n {
        m.set(i, i, rng.normal() * scale);
        for j in (i + 1)..n {
            let val = rng.normal() * scale / (2.0_f64).sqrt();
            m.set(i, j, val);
            m.set(j, i, val);
        }
    }
    m
}

/// Wishart matrix: W = X^T * X / n where X is a p x n matrix with iid N(0,1).
pub fn wishart(n: usize, p: usize, rng: &mut Rng) -> RandomMatrix {
    let mut x = RandomMatrix::zeros(p, n);
    for i in 0..p {
        for j in 0..n {
            x.set(i, j, rng.normal());
        }
    }
    let xt = x.transpose();
    let mut w = xt.mul(&x);
    // Scale by 1/n
    for v in w.entries.iter_mut() {
        *v /= n as f64;
    }
    w
}

/// Generate a random correlation matrix of size n.
pub fn random_correlation(n: usize, rng: &mut Rng) -> RandomMatrix {
    // Use random vectors and compute their correlations
    let samples = 5 * n;
    let mut data = RandomMatrix::zeros(n, samples);
    for i in 0..n {
        for j in 0..samples {
            data.set(i, j, rng.normal());
        }
    }
    // Compute correlation matrix
    let mut corr = RandomMatrix::zeros(n, n);
    for i in 0..n {
        let mean_i: f64 = (0..samples).map(|j| data.get(i, j)).sum::<f64>() / samples as f64;
        let std_i: f64 = ((0..samples).map(|j| (data.get(i, j) - mean_i).powi(2)).sum::<f64>() / samples as f64).sqrt();
        for k in i..n {
            let mean_k: f64 = (0..samples).map(|j| data.get(k, j)).sum::<f64>() / samples as f64;
            let std_k: f64 = ((0..samples).map(|j| (data.get(k, j) - mean_k).powi(2)).sum::<f64>() / samples as f64).sqrt();
            let cov: f64 = (0..samples)
                .map(|j| (data.get(i, j) - mean_i) * (data.get(k, j) - mean_k))
                .sum::<f64>() / samples as f64;
            let r = if std_i > 1e-10 && std_k > 1e-10 { cov / (std_i * std_k) } else { 0.0 };
            corr.set(i, k, r);
            corr.set(k, i, r);
        }
    }
    corr
}

// ---------------------------------------------------------------------------
// Eigenvalue computation (QR algorithm for real symmetric matrices)
// ---------------------------------------------------------------------------

/// Compute eigenvalues of a real symmetric matrix using the QR algorithm
/// with Wilkinson shifts.
pub fn eigenvalues(matrix: &RandomMatrix) -> Vec<f64> {
    assert_eq!(matrix.rows, matrix.cols, "matrix must be square");
    let n = matrix.rows;
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![matrix.get(0, 0)];
    }

    // First, reduce to tridiagonal form via Householder reflections.
    let (diag, off_diag) = tridiagonalize(matrix);

    // Then apply implicit QR on the tridiagonal matrix.
    qr_tridiagonal(diag, off_diag)
}

/// Householder tridiagonalization of a symmetric matrix.
/// Returns (diagonal, sub-diagonal) vectors.
fn tridiagonalize(m: &RandomMatrix) -> (Vec<f64>, Vec<f64>) {
    let n = m.rows;
    let mut a = m.entries.clone();
    let cols = n;

    let get = |a: &[f64], i: usize, j: usize| a[i * cols + j];
    let set = |a: &mut [f64], i: usize, j: usize, v: f64| a[i * cols + j] = v;

    for k in 0..n.saturating_sub(2) {
        // Compute Householder vector for column k, rows k+1..n
        let mut x: Vec<f64> = (k + 1..n).map(|i| get(&a, i, k)).collect();
        let x_norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if x_norm < 1e-15 {
            continue;
        }
        let sign = if x[0] >= 0.0 { 1.0 } else { -1.0 };
        x[0] += sign * x_norm;
        let x_new_norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if x_new_norm < 1e-15 {
            continue;
        }
        for v in x.iter_mut() {
            *v /= x_new_norm;
        }

        // Apply P = I - 2*v*v^T from left and right: A <- P * A * P
        let m_size = n - k - 1;

        // Compute p = A_sub * v
        let mut p = vec![0.0; m_size];
        for i in 0..m_size {
            for j in 0..m_size {
                p[i] += get(&a, i + k + 1, j + k + 1) * x[j];
            }
        }

        // K = 2, q = 2*p - 2*(v^T * p)*v
        let vtp: f64 = x.iter().zip(p.iter()).map(|(a, b)| a * b).sum();
        let mut q = vec![0.0; m_size];
        for i in 0..m_size {
            q[i] = 2.0 * p[i] - 2.0 * vtp * x[i];
        }

        // A_sub -= v*q^T + q*v^T
        for i in 0..m_size {
            for j in 0..m_size {
                let val = get(&a, i + k + 1, j + k + 1) - 2.0 * (x[i] * q[j] + q[i] * x[j]) / 2.0;
                // Correction: A <- A - 2*v*(p - (v^T*p)*v)^T - 2*(p - (v^T*p)*v)*v^T
                // Actually let's use the simpler formula
                let new_val = get(&a, i + k + 1, j + k + 1)
                    - 2.0 * x[i] * p[j]
                    - 2.0 * p[i] * x[j]
                    + 4.0 * vtp * x[i] * x[j];
                set(&mut a, i + k + 1, j + k + 1, new_val);
            }
        }

        // Update the border elements
        // A[k, k+1..n] and A[k+1..n, k]
        let mut border: Vec<f64> = (0..m_size).map(|i| get(&a, k, i + k + 1)).collect();
        let bv: f64 = border.iter().zip(x.iter()).map(|(a, b)| a * b).sum();
        for i in 0..m_size {
            border[i] -= 2.0 * bv * x[i];
        }
        for i in 0..m_size {
            set(&mut a, k, i + k + 1, border[i]);
            set(&mut a, i + k + 1, k, border[i]);
        }
    }

    let diag: Vec<f64> = (0..n).map(|i| get(&a, i, i)).collect();
    let off_diag: Vec<f64> = (0..n.saturating_sub(1)).map(|i| get(&a, i, i + 1)).collect();
    (diag, off_diag)
}

/// Implicit QR iteration on a tridiagonal matrix.
fn qr_tridiagonal(mut diag: Vec<f64>, mut off: Vec<f64>) -> Vec<f64> {
    let n = diag.len();
    if n <= 1 {
        return diag;
    }

    let max_iter = 100 * n;
    let mut m = n;

    for _ in 0..max_iter {
        if m <= 1 {
            break;
        }

        // Deflation: if off[m-2] is small, reduce problem size
        while m > 1 && off[m - 2].abs() < 1e-12 * (diag[m - 2].abs() + diag[m - 1].abs()).max(1e-15) {
            m -= 1;
        }
        if m <= 1 {
            break;
        }

        // Wilkinson shift
        let d = (diag[m - 2] - diag[m - 1]) / 2.0;
        let sign_d = if d >= 0.0 { 1.0 } else { -1.0 };
        let shift = diag[m - 1] - off[m - 2] * off[m - 2] / (d + sign_d * (d * d + off[m - 2] * off[m - 2]).sqrt());

        // QR step with shift
        let mut x = diag[0] - shift;
        let mut z = off[0];

        for k in 0..m - 1 {
            // Givens rotation to zero out z
            let r = (x * x + z * z).sqrt();
            let c = x / r;
            let s = z / r;

            if k > 0 {
                off[k - 1] = r;
            }

            let d1 = diag[k];
            let d2 = diag[k + 1];
            let e = off[k];

            diag[k] = c * c * d1 + 2.0 * c * s * e + s * s * d2;
            diag[k + 1] = s * s * d1 - 2.0 * c * s * e + c * c * d2;
            off[k] = c * s * (d2 - d1) + (c * c - s * s) * e;

            if k + 1 < m - 1 {
                x = off[k + 1] * c + 0.0;
                // Actually:
                let old_off_next = off[k + 1];
                x = off[k];
                z = -s * old_off_next;
                off[k + 1] = c * old_off_next;
                x = off[k]; // re-read after modification
            }
        }
    }

    diag.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    diag
}

/// Compute eigenvalues using a simpler Jacobi rotation method for small symmetric matrices.
/// More robust than QR for our purposes.
pub fn eigenvalues_jacobi(matrix: &RandomMatrix) -> Vec<f64> {
    assert_eq!(matrix.rows, matrix.cols);
    let n = matrix.rows;
    if n == 0 {
        return Vec::new();
    }

    let mut a = matrix.entries.clone();
    let max_iter = 100 * n * n;

    for _ in 0..max_iter {
        // Find largest off-diagonal element
        let mut max_val = 0.0;
        let mut p = 0;
        let mut q = 1;
        for i in 0..n {
            for j in (i + 1)..n {
                let v = a[i * n + j].abs();
                if v > max_val {
                    max_val = v;
                    p = i;
                    q = j;
                }
            }
        }
        if max_val < 1e-12 {
            break;
        }

        // Compute rotation angle
        let app = a[p * n + p];
        let aqq = a[q * n + q];
        let apq = a[p * n + q];

        let theta = if (app - aqq).abs() < 1e-15 {
            std::f64::consts::FRAC_PI_4
        } else {
            0.5 * (2.0 * apq / (app - aqq)).atan()
        };

        let c = theta.cos();
        let s = theta.sin();

        // Apply rotation
        let mut new_a = a.clone();
        for i in 0..n {
            // Row/col p
            let aip = a[i * n + p];
            let aiq = a[i * n + q];
            new_a[i * n + p] = c * aip + s * aiq;
            new_a[i * n + q] = -s * aip + c * aiq;
            new_a[p * n + i] = new_a[i * n + p];
            new_a[q * n + i] = new_a[i * n + q];
        }
        new_a[p * n + p] = c * c * app + 2.0 * s * c * apq + s * s * aqq;
        new_a[q * n + q] = s * s * app - 2.0 * s * c * apq + c * c * aqq;
        new_a[p * n + q] = 0.0;
        new_a[q * n + p] = 0.0;

        a = new_a;
    }

    let mut eigs: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
    eigs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    eigs
}

// ---------------------------------------------------------------------------
// Spectral analysis
// ---------------------------------------------------------------------------

/// Wigner semicircle law: eigenvalues of GOE(n) should follow
/// rho(x) = (2/(pi*R^2)) * sqrt(R^2 - x^2) for |x| <= R, where R = 2.
pub fn wigner_semicircle(eigenvalues: &[f64]) -> Histogram {
    Histogram::from_samples_bounded(eigenvalues, 50, -2.5, 2.5)
}

/// Wigner semicircle theoretical density at x with radius R.
pub fn semicircle_density(x: f64, r: f64) -> f64 {
    if x.abs() > r {
        return 0.0;
    }
    2.0 / (std::f64::consts::PI * r * r) * (r * r - x * x).sqrt()
}

/// Marchenko-Pastur distribution: eigenvalues of large Wishart matrices.
/// For ratio gamma = p/n, support is [(1-sqrt(gamma))^2, (1+sqrt(gamma))^2].
pub fn marchenko_pastur(eigenvalues: &[f64], ratio: f64) -> Histogram {
    let lambda_min = (1.0 - ratio.sqrt()).powi(2);
    let lambda_max = (1.0 + ratio.sqrt()).powi(2);
    Histogram::from_samples_bounded(eigenvalues, 50, lambda_min * 0.5, lambda_max * 1.5)
}

/// Marchenko-Pastur theoretical density.
pub fn marchenko_pastur_density(x: f64, ratio: f64) -> f64 {
    let lambda_min = (1.0 - ratio.sqrt()).powi(2);
    let lambda_max = (1.0 + ratio.sqrt()).powi(2);
    if x < lambda_min || x > lambda_max || x <= 0.0 {
        return 0.0;
    }
    let num = ((lambda_max - x) * (x - lambda_min)).sqrt();
    num / (2.0 * std::f64::consts::PI * ratio * x)
}

/// Level spacing ratio: r_i = min(s_i, s_{i+1}) / max(s_i, s_{i+1})
/// where s_i = lambda_{i+1} - lambda_i.
pub fn level_spacing_ratios(eigenvalues: &[f64]) -> Vec<f64> {
    if eigenvalues.len() < 3 {
        return Vec::new();
    }
    let spacings: Vec<f64> = eigenvalues.windows(2).map(|w| w[1] - w[0]).collect();
    spacings
        .windows(2)
        .map(|w| w[0].min(w[1]) / w[0].max(w[1]).max(1e-15))
        .collect()
}

// ---------------------------------------------------------------------------
// EigenvalueRenderer
// ---------------------------------------------------------------------------

/// Render eigenvalue distribution as a glyph histogram.
pub struct EigenvalueRenderer {
    pub bar_character: char,
    pub bar_color: [f32; 4],
    pub theory_character: char,
    pub theory_color: [f32; 4],
    pub x_scale: f32,
    pub y_scale: f32,
}

impl EigenvalueRenderer {
    pub fn new() -> Self {
        Self {
            bar_character: '█',
            bar_color: [0.4, 0.6, 1.0, 0.8],
            theory_character: '·',
            theory_color: [1.0, 0.3, 0.3, 1.0],
            x_scale: 0.3,
            y_scale: 2.0,
        }
    }

    /// Render empirical histogram.
    pub fn render_histogram(&self, hist: &Histogram) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = Vec::new();
        let max_count = hist.max_count().max(1) as f32;
        for (i, &count) in hist.bins.iter().enumerate() {
            let x = (i as f32 - hist.bin_count as f32 / 2.0) * self.x_scale;
            let height = (count as f32 / max_count * 15.0) as usize;
            for h in 0..height {
                glyphs.push((
                    Vec2::new(x, h as f32 * self.y_scale * 0.1),
                    self.bar_character,
                    self.bar_color,
                ));
            }
        }
        glyphs
    }

    /// Render with semicircle overlay.
    pub fn render_with_semicircle(&self, hist: &Histogram, radius: f64) -> Vec<(Vec2, char, [f32; 4])> {
        let mut glyphs = self.render_histogram(hist);

        // Overlay semicircle
        for i in 0..100 {
            let x = -radius + 2.0 * radius * i as f64 / 100.0;
            let density = semicircle_density(x, radius);
            let pos = Vec2::new(x as f32 * self.x_scale * (hist.bin_count as f32 / (2.0 * radius as f32)),
                                density as f32 * self.y_scale);
            glyphs.push((pos, self.theory_character, self.theory_color));
        }
        glyphs
    }
}

impl Default for EigenvalueRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goe_symmetric() {
        let mut rng = Rng::new(42);
        let m = goe(10, &mut rng);
        for i in 0..10 {
            for j in 0..10 {
                assert!(
                    (m.get(i, j) - m.get(j, i)).abs() < 1e-14,
                    "GOE should be symmetric"
                );
            }
        }
    }

    #[test]
    fn test_gue_symmetric() {
        let mut rng = Rng::new(42);
        let m = gue(10, &mut rng);
        for i in 0..10 {
            for j in 0..10 {
                assert!(
                    (m.get(i, j) - m.get(j, i)).abs() < 1e-14,
                    "GUE (real approx) should be symmetric"
                );
            }
        }
    }

    #[test]
    fn test_wishart_positive_semidefinite() {
        // Wishart matrices should have non-negative eigenvalues
        let mut rng = Rng::new(42);
        let w = wishart(20, 10, &mut rng);
        let eigs = eigenvalues_jacobi(&w);
        for &e in &eigs {
            assert!(
                e > -0.1,
                "Wishart eigenvalue should be >= 0, got {}",
                e
            );
        }
    }

    #[test]
    fn test_eigenvalues_identity() {
        let id = RandomMatrix::identity(5);
        let eigs = eigenvalues_jacobi(&id);
        for &e in &eigs {
            assert!((e - 1.0).abs() < 1e-6, "identity eigenvalue should be 1, got {}", e);
        }
    }

    #[test]
    fn test_eigenvalues_diagonal() {
        let mut m = RandomMatrix::zeros(3, 3);
        m.set(0, 0, 1.0);
        m.set(1, 1, 3.0);
        m.set(2, 2, 5.0);
        let eigs = eigenvalues_jacobi(&m);
        assert!((eigs[0] - 1.0).abs() < 1e-6);
        assert!((eigs[1] - 3.0).abs() < 1e-6);
        assert!((eigs[2] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_eigenvalues_2x2() {
        // [[2, 1], [1, 2]] has eigenvalues 1 and 3
        let mut m = RandomMatrix::zeros(2, 2);
        m.set(0, 0, 2.0);
        m.set(0, 1, 1.0);
        m.set(1, 0, 1.0);
        m.set(1, 1, 2.0);
        let eigs = eigenvalues_jacobi(&m);
        assert!((eigs[0] - 1.0).abs() < 1e-6, "got {}", eigs[0]);
        assert!((eigs[1] - 3.0).abs() < 1e-6, "got {}", eigs[1]);
    }

    #[test]
    fn test_wigner_semicircle_approximate() {
        // Generate many GOE matrices, collect eigenvalues
        let mut rng = Rng::new(42);
        let mut all_eigs = Vec::new();
        let n = 50;
        let samples = 100;
        for _ in 0..samples {
            let m = goe(n, &mut rng);
            let eigs = eigenvalues_jacobi(&m);
            all_eigs.extend(eigs);
        }

        // Most eigenvalues should be within [-2, 2] (approximately)
        let in_range = all_eigs.iter().filter(|&&e| e.abs() < 2.5).count();
        let fraction = in_range as f64 / all_eigs.len() as f64;
        assert!(
            fraction > 0.9,
            "most GOE eigenvalues should be in [-2.5, 2.5], fraction = {}",
            fraction
        );

        let hist = wigner_semicircle(&all_eigs);
        assert_eq!(hist.bins.len(), 50);
    }

    #[test]
    fn test_semicircle_density() {
        // Density at 0 should be 2/(pi*R^2) * R = 2/(pi*R)
        let r = 2.0;
        let d0 = semicircle_density(0.0, r);
        let expected = 2.0 / (std::f64::consts::PI * r);
        assert!((d0 - expected).abs() < 1e-10);

        // Density outside support should be 0
        assert_eq!(semicircle_density(3.0, 2.0), 0.0);
    }

    #[test]
    fn test_matrix_operations() {
        let mut a = RandomMatrix::zeros(2, 2);
        a.set(0, 0, 1.0);
        a.set(0, 1, 2.0);
        a.set(1, 0, 3.0);
        a.set(1, 1, 4.0);

        assert!((a.trace() - 5.0).abs() < 1e-10);

        let at = a.transpose();
        assert!((at.get(0, 1) - 3.0).abs() < 1e-10);
        assert!((at.get(1, 0) - 2.0).abs() < 1e-10);

        let prod = a.mul(&RandomMatrix::identity(2));
        assert!((prod.get(0, 0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_marchenko_pastur_density() {
        let ratio = 0.5;
        let lambda_min = (1.0 - ratio.sqrt()).powi(2);
        let lambda_max = (1.0 + ratio.sqrt()).powi(2);
        // Density should be 0 outside support
        assert_eq!(marchenko_pastur_density(0.0, ratio), 0.0);
        assert_eq!(marchenko_pastur_density(lambda_max + 1.0, ratio), 0.0);
        // Density should be positive inside support
        let mid = (lambda_min + lambda_max) / 2.0;
        assert!(marchenko_pastur_density(mid, ratio) > 0.0);
    }

    #[test]
    fn test_renderer() {
        let mut rng = Rng::new(42);
        let m = goe(10, &mut rng);
        let eigs = eigenvalues_jacobi(&m);
        let hist = wigner_semicircle(&eigs);
        let renderer = EigenvalueRenderer::new();
        let glyphs = renderer.render_histogram(&hist);
        assert!(!glyphs.is_empty());
    }
}
