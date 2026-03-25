use std::f64::consts::PI;
use super::schrodinger::Complex;
use glam::Vec2;

/// Hydrogen atom energy levels: E_n = -13.6 / n^2 eV.
pub fn hydrogen_energy(n: u32) -> f64 {
    if n == 0 { return 0.0; }
    -13.6 / (n as f64 * n as f64)
}

/// Associated Legendre polynomial P_l^m(x) via recurrence.
/// Uses the convention without the Condon-Shortley phase (it's included in Y_l^m).
pub fn associated_legendre(l: u32, m: i32, x: f64) -> f64 {
    let m_abs = m.unsigned_abs();
    if m_abs > l {
        return 0.0;
    }

    // Start with P_m^m
    let mut pmm = 1.0;
    if m_abs > 0 {
        let somx2 = ((1.0 - x) * (1.0 + x)).sqrt();
        let mut fact = 1.0;
        for _i in 0..m_abs {
            pmm *= -fact * somx2;
            fact += 2.0;
        }
    }

    if l == m_abs {
        if m < 0 {
            return adjust_negative_m(l, m, pmm);
        }
        return pmm;
    }

    // P_{m+1}^m
    let mut pmmp1 = x * (2 * m_abs + 1) as f64 * pmm;
    if l == m_abs + 1 {
        if m < 0 {
            return adjust_negative_m(l, m, pmmp1);
        }
        return pmmp1;
    }

    // Recurrence for higher l
    let mut pll = 0.0;
    for ll in (m_abs + 2)..=l {
        pll = (x * (2 * ll - 1) as f64 * pmmp1 - (ll + m_abs - 1) as f64 * pmm) / (ll - m_abs) as f64;
        pmm = pmmp1;
        pmmp1 = pll;
    }

    if m < 0 {
        adjust_negative_m(l, m, pll)
    } else {
        pll
    }
}

fn adjust_negative_m(l: u32, m: i32, plm: f64) -> f64 {
    let m_abs = m.unsigned_abs();
    let sign = if m_abs % 2 == 0 { 1.0 } else { -1.0 };
    let num: f64 = (1..=(l - m_abs) as u64).map(|k| k as f64).product::<f64>().max(1.0);
    let den: f64 = (1..=(l + m_abs) as u64).map(|k| k as f64).product::<f64>().max(1.0);
    sign * (num / den) * plm
}

/// Spherical harmonic Y_l^m(theta, phi).
pub fn spherical_harmonic(l: u32, m: i32, theta: f64, phi: f64) -> Complex {
    let m_abs = m.unsigned_abs();
    if m_abs > l {
        return Complex::zero();
    }

    // Normalization factor
    let num: f64 = (1..=(l - m_abs) as u64).map(|k| k as f64).product::<f64>().max(1.0);
    let den: f64 = (1..=(l + m_abs) as u64).map(|k| k as f64).product::<f64>().max(1.0);
    let norm = ((2 * l + 1) as f64 / (4.0 * PI) * num / den).sqrt();

    let plm = associated_legendre(l, m.abs(), theta.cos());
    let phase = Complex::from_polar(1.0, m as f64 * phi);

    let cs_phase = if m > 0 && m % 2 != 0 { -1.0 } else { 1.0 };

    if m >= 0 {
        phase * (norm * plm * cs_phase)
    } else {
        let sign = if m_abs % 2 == 0 { 1.0 } else { -1.0 };
        phase * (norm * plm * sign)
    }
}

/// Factorial helper.
fn factorial(n: u64) -> f64 {
    (1..=n).map(|k| k as f64).product::<f64>().max(1.0)
}

/// Generalized Laguerre polynomial L_n^alpha(x) via recurrence.
fn generalized_laguerre(n: u32, alpha: f64, x: f64) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return 1.0 + alpha - x;
    }
    let mut l_prev = 1.0;
    let mut l_curr = 1.0 + alpha - x;
    for k in 1..n {
        let kf = k as f64;
        let l_next = ((2.0 * kf + 1.0 + alpha - x) * l_curr - (kf + alpha) * l_prev) / (kf + 1.0);
        l_prev = l_curr;
        l_curr = l_next;
    }
    l_curr
}

/// Radial wave function R_nl(r) for hydrogen atom.
/// Uses Bohr radius a0 = 1 (atomic units).
pub fn radial_wavefunction(n: u32, l: u32, r: f64) -> f64 {
    if n == 0 || l >= n {
        return 0.0;
    }
    let a0 = 1.0; // Bohr radius in atomic units
    let nf = n as f64;
    let rho = 2.0 * r / (nf * a0);

    // Normalization
    let num = factorial((n - l - 1) as u64);
    let den = 2.0 * nf * factorial((n + l) as u64);
    let norm = ((2.0 / (nf * a0)).powi(3) * num / den).sqrt();

    let laguerre = generalized_laguerre(n - l - 1, 2 * l as f64 + 1.0, rho);

    norm * (-rho / 2.0).exp() * rho.powi(l as i32) * laguerre
}

/// Full hydrogen orbital psi_nlm(r, theta, phi).
pub fn hydrogen_orbital(n: u32, l: u32, m: i32, r: f64, theta: f64, phi: f64) -> Complex {
    if l >= n || (m.unsigned_abs()) > l {
        return Complex::zero();
    }
    let radial = radial_wavefunction(n, l, r);
    let ylm = spherical_harmonic(l, m, theta, phi);
    ylm * radial
}

/// Probability density |psi|^2 at a point.
pub fn probability_density_3d(n: u32, l: u32, m: i32, r: f64, theta: f64, phi: f64) -> f64 {
    hydrogen_orbital(n, l, m, r, theta, phi).norm_sq()
}

/// Radial probability density r^2 |R_nl(r)|^2 (probability of finding electron at radius r).
pub fn radial_probability(n: u32, l: u32, r: f64) -> f64 {
    let rnl = radial_wavefunction(n, l, r);
    r * r * rnl * rnl
}

/// Human-readable orbital name.
pub fn orbital_name(n: u32, l: u32, m: i32) -> String {
    let l_char = match l {
        0 => 's',
        1 => 'p',
        2 => 'd',
        3 => 'f',
        4 => 'g',
        _ => '?',
    };
    format!("{}{}_{}", n, l_char, m)
}

/// Render a 2D slice of an orbital.
pub fn render_orbital_slice(
    n: u32,
    l: u32,
    m: i32,
    plane: SlicePlane,
    grid_size: usize,
    extent: f64,
) -> Vec<(Vec2, f64, Complex)> {
    let mut result = Vec::with_capacity(grid_size * grid_size);
    let step = 2.0 * extent / grid_size as f64;

    for iy in 0..grid_size {
        for ix in 0..grid_size {
            let u = -extent + ix as f64 * step;
            let v = -extent + iy as f64 * step;
            let (r, theta, phi) = match plane {
                SlicePlane::XY => {
                    let r = (u * u + v * v).sqrt();
                    let theta = PI / 2.0; // z=0 plane
                    let phi = v.atan2(u);
                    (r, theta, phi)
                }
                SlicePlane::XZ => {
                    let r = (u * u + v * v).sqrt();
                    let theta = if r > 1e-15 { (u / r).acos() } else { 0.0 };
                    let phi = 0.0;
                    (r, theta, phi)
                }
                SlicePlane::YZ => {
                    let r = (u * u + v * v).sqrt();
                    let theta = if r > 1e-15 { v.atan2(u) } else { 0.0 };
                    let phi = PI / 2.0;
                    (r, theta, phi)
                }
            };

            let psi = hydrogen_orbital(n, l, m, r.max(1e-10), theta, phi);
            let density = psi.norm_sq();
            result.push((Vec2::new(u as f32, v as f32), density, psi));
        }
    }
    result
}

/// Which plane to slice through for visualization.
#[derive(Clone, Copy, Debug)]
pub enum SlicePlane {
    XY,
    XZ,
    YZ,
}

/// Renderer for hydrogen orbitals as 2D glyph brightness.
pub struct HydrogenRenderer {
    pub grid_size: usize,
    pub extent: f64,
}

impl HydrogenRenderer {
    pub fn new(grid_size: usize, extent: f64) -> Self {
        Self { grid_size, extent }
    }

    pub fn render(&self, n: u32, l: u32, m: i32, plane: SlicePlane) -> Vec<Vec<(char, f64, f64, f64)>> {
        let data = render_orbital_slice(n, l, m, plane, self.grid_size, self.extent);
        let max_density = data.iter().map(|&(_, d, _)| d).fold(0.0_f64, f64::max);
        let scale = if max_density > 1e-20 { 1.0 / max_density } else { 1.0 };

        let mut grid = vec![vec![(' ', 0.0, 0.0, 0.0); self.grid_size]; self.grid_size];
        for (idx, &(_, density, psi)) in data.iter().enumerate() {
            let ix = idx % self.grid_size;
            let iy = idx / self.grid_size;
            let brightness = (density * scale).min(1.0);
            let phase = psi.arg();
            let (r, g, b) = super::wavefunction::PhaseColorMap::phase_to_rgb(phase);
            let ch = if brightness > 0.8 {
                '@'
            } else if brightness > 0.5 {
                '#'
            } else if brightness > 0.2 {
                '*'
            } else if brightness > 0.05 {
                '.'
            } else {
                ' '
            };
            grid[iy][ix] = (ch, r * brightness, g * brightness, b * brightness);
        }
        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hydrogen_energy_levels() {
        assert!((hydrogen_energy(1) - (-13.6)).abs() < 0.01);
        assert!((hydrogen_energy(2) - (-3.4)).abs() < 0.01);
        assert!((hydrogen_energy(3) - (-13.6 / 9.0)).abs() < 0.01);
    }

    #[test]
    fn test_ground_state_normalization() {
        // Integrate r^2 |R_10(r)|^2 dr from 0 to large R
        let dr = 0.01;
        let r_max = 30.0;
        let n_pts = (r_max / dr) as usize;
        let integral: f64 = (1..n_pts)
            .map(|i| {
                let r = i as f64 * dr;
                radial_probability(1, 0, r) * dr
            })
            .sum();
        assert!(
            (integral - 1.0).abs() < 0.05,
            "Ground state radial norm: {}",
            integral
        );
    }

    #[test]
    fn test_2s_normalization() {
        let dr = 0.02;
        let r_max = 50.0;
        let n_pts = (r_max / dr) as usize;
        let integral: f64 = (1..n_pts)
            .map(|i| {
                let r = i as f64 * dr;
                radial_probability(2, 0, r) * dr
            })
            .sum();
        assert!(
            (integral - 1.0).abs() < 0.1,
            "2s radial norm: {}",
            integral
        );
    }

    #[test]
    fn test_radial_orthogonality() {
        // <R_10|R_20> should be 0
        let dr = 0.01;
        let n_pts = 3000;
        let integral: f64 = (1..n_pts)
            .map(|i| {
                let r = i as f64 * dr;
                let r10 = radial_wavefunction(1, 0, r);
                let r20 = radial_wavefunction(2, 0, r);
                r * r * r10 * r20 * dr
            })
            .sum();
        assert!(integral.abs() < 0.1, "<R_10|R_20> = {}", integral);
    }

    #[test]
    fn test_spherical_harmonic_y00() {
        // Y_0^0 = 1/(2*sqrt(pi))
        let y00 = spherical_harmonic(0, 0, 0.5, 0.3);
        let expected = 1.0 / (2.0 * PI.sqrt());
        assert!((y00.re - expected).abs() < 1e-10);
        assert!(y00.im.abs() < 1e-10);
    }

    #[test]
    fn test_spherical_harmonic_normalization() {
        // Integrate |Y_1^0|^2 sin(theta) dtheta dphi over sphere
        let n_theta = 100;
        let n_phi = 100;
        let dtheta = PI / n_theta as f64;
        let dphi = 2.0 * PI / n_phi as f64;
        let integral: f64 = (0..n_theta)
            .flat_map(|it| {
                let theta = (it as f64 + 0.5) * dtheta;
                (0..n_phi).map(move |ip| {
                    let phi = (ip as f64 + 0.5) * dphi;
                    spherical_harmonic(1, 0, theta, phi).norm_sq() * theta.sin() * dtheta * dphi
                })
            })
            .sum();
        assert!(
            (integral - 1.0).abs() < 0.1,
            "Y_1^0 norm: {}",
            integral
        );
    }

    #[test]
    fn test_associated_legendre() {
        // P_0^0(x) = 1
        assert!((associated_legendre(0, 0, 0.5) - 1.0).abs() < 1e-10);
        // P_1^0(x) = x
        assert!((associated_legendre(1, 0, 0.5) - 0.5).abs() < 1e-10);
        // P_1^1(x) = -sqrt(1-x^2)
        let p11 = associated_legendre(1, 1, 0.5);
        let expected = -(1.0 - 0.25_f64).sqrt();
        assert!((p11 - expected).abs() < 1e-10, "P_1^1(0.5) = {}", p11);
    }

    #[test]
    fn test_orbital_name() {
        assert_eq!(orbital_name(1, 0, 0), "1s_0");
        assert_eq!(orbital_name(2, 1, 1), "2p_1");
        assert_eq!(orbital_name(3, 2, 0), "3d_0");
    }

    #[test]
    fn test_most_probable_radius_1s() {
        // For 1s, most probable r = a0 = 1 (atomic units)
        let dr = 0.01;
        let n_pts = 1000;
        let mut max_prob = 0.0;
        let mut max_r = 0.0;
        for i in 1..n_pts {
            let r = i as f64 * dr;
            let p = radial_probability(1, 0, r);
            if p > max_prob {
                max_prob = p;
                max_r = r;
            }
        }
        assert!((max_r - 1.0).abs() < 0.1, "Most probable r = {}", max_r);
    }

    #[test]
    fn test_renderer() {
        let renderer = HydrogenRenderer::new(10, 5.0);
        let grid = renderer.render(1, 0, 0, SlicePlane::XZ);
        assert_eq!(grid.len(), 10);
        assert_eq!(grid[0].len(), 10);
    }

    #[test]
    fn test_render_orbital_slice() {
        let data = render_orbital_slice(2, 1, 0, SlicePlane::XZ, 8, 5.0);
        assert_eq!(data.len(), 64);
    }
}
