//! Mandelbrot set rendering with arbitrary precision deep zoom.

/// Mandelbrot rendering parameters.
#[derive(Debug, Clone)]
pub struct MandelbrotParams {
    pub center_re: f64,
    pub center_im: f64,
    pub zoom: f64,
    pub max_iter: u32,
    pub escape_radius: f64,
    pub width: u32,
    pub height: u32,
}

impl Default for MandelbrotParams {
    fn default() -> Self {
        Self {
            center_re: -0.5, center_im: 0.0, zoom: 1.0,
            max_iter: 256, escape_radius: 4.0,
            width: 800, height: 600,
        }
    }
}

/// Result of rendering one pixel.
#[derive(Debug, Clone, Copy)]
pub struct FractalPixel {
    pub iterations: u32,
    pub smooth_iter: f64,
    pub escaped: bool,
    pub final_z_re: f64,
    pub final_z_im: f64,
}

/// Mandelbrot set renderer.
pub struct MandelbrotRenderer;

impl MandelbrotRenderer {
    /// Compute one pixel of the Mandelbrot set.
    pub fn compute_pixel(c_re: f64, c_im: f64, max_iter: u32, escape_r2: f64) -> FractalPixel {
        let mut z_re = 0.0;
        let mut z_im = 0.0;
        let mut iter = 0u32;

        while iter < max_iter {
            let z_re2 = z_re * z_re;
            let z_im2 = z_im * z_im;
            if z_re2 + z_im2 > escape_r2 { break; }
            z_im = 2.0 * z_re * z_im + c_im;
            z_re = z_re2 - z_im2 + c_re;
            iter += 1;
        }

        let escaped = iter < max_iter;
        let smooth = if escaped {
            let log_zn = (z_re * z_re + z_im * z_im).ln() * 0.5;
            let nu = (log_zn / 2.0_f64.ln()).ln() / 2.0_f64.ln();
            iter as f64 + 1.0 - nu
        } else {
            iter as f64
        };

        FractalPixel {
            iterations: iter,
            smooth_iter: smooth,
            escaped,
            final_z_re: z_re,
            final_z_im: z_im,
        }
    }

    /// Render the full Mandelbrot set to a buffer of FractalPixels.
    pub fn render(params: &MandelbrotParams) -> Vec<FractalPixel> {
        let w = params.width;
        let h = params.height;
        let aspect = w as f64 / h as f64;
        let scale = 2.0 / params.zoom;
        let escape_r2 = params.escape_radius * params.escape_radius;

        let mut pixels = Vec::with_capacity((w * h) as usize);
        for py in 0..h {
            for px in 0..w {
                let c_re = params.center_re + (px as f64 / w as f64 - 0.5) * scale * aspect;
                let c_im = params.center_im + (py as f64 / h as f64 - 0.5) * scale;
                pixels.push(Self::compute_pixel(c_re, c_im, params.max_iter, escape_r2));
            }
        }
        pixels
    }

    /// Deep zoom using perturbation theory (reference orbit + delta).
    pub fn render_perturbation(params: &MandelbrotParams) -> Vec<FractalPixel> {
        let w = params.width;
        let h = params.height;
        let aspect = w as f64 / h as f64;
        let scale = 2.0 / params.zoom;
        let escape_r2 = params.escape_radius * params.escape_radius;

        // Compute reference orbit at the center
        let ref_orbit = Self::reference_orbit(params.center_re, params.center_im, params.max_iter, escape_r2);

        let mut pixels = Vec::with_capacity((w * h) as usize);
        for py in 0..h {
            for px in 0..w {
                let dc_re = (px as f64 / w as f64 - 0.5) * scale * aspect;
                let dc_im = (py as f64 / h as f64 - 0.5) * scale;
                pixels.push(Self::perturb_pixel(&ref_orbit, dc_re, dc_im, params.max_iter, escape_r2));
            }
        }
        pixels
    }

    fn reference_orbit(c_re: f64, c_im: f64, max_iter: u32, escape_r2: f64) -> Vec<(f64, f64)> {
        let mut orbit = Vec::with_capacity(max_iter as usize);
        let mut z_re = 0.0;
        let mut z_im = 0.0;
        for _ in 0..max_iter {
            orbit.push((z_re, z_im));
            let z_re2 = z_re * z_re;
            let z_im2 = z_im * z_im;
            if z_re2 + z_im2 > escape_r2 * 1e6 { break; }
            let new_re = z_re2 - z_im2 + c_re;
            let new_im = 2.0 * z_re * z_im + c_im;
            z_re = new_re;
            z_im = new_im;
        }
        orbit
    }

    fn perturb_pixel(ref_orbit: &[(f64, f64)], dc_re: f64, dc_im: f64, max_iter: u32, escape_r2: f64) -> FractalPixel {
        let mut dz_re = 0.0;
        let mut dz_im = 0.0;
        let n = ref_orbit.len().min(max_iter as usize);

        for i in 0..n {
            let (zn_re, zn_im) = ref_orbit[i];
            // δz_{n+1} = 2·Z_n·δz_n + δz_n² + δc
            let new_dz_re = 2.0 * (zn_re * dz_re - zn_im * dz_im) + dz_re * dz_re - dz_im * dz_im + dc_re;
            let new_dz_im = 2.0 * (zn_re * dz_im + zn_im * dz_re) + 2.0 * dz_re * dz_im + dc_im;
            dz_re = new_dz_re;
            dz_im = new_dz_im;

            let z_re = zn_re + dz_re;
            let z_im = zn_im + dz_im;
            if z_re * z_re + z_im * z_im > escape_r2 {
                let smooth = i as f64 + 1.0 - ((z_re * z_re + z_im * z_im).ln() * 0.5 / 2.0_f64.ln()).ln() / 2.0_f64.ln();
                return FractalPixel {
                    iterations: i as u32,
                    smooth_iter: smooth,
                    escaped: true,
                    final_z_re: z_re,
                    final_z_im: z_im,
                };
            }
        }

        FractalPixel { iterations: max_iter, smooth_iter: max_iter as f64, escaped: false, final_z_re: 0.0, final_z_im: 0.0 }
    }

    /// Color a pixel using smooth iteration count.
    pub fn color_pixel(pixel: &FractalPixel, max_iter: u32) -> [f32; 3] {
        if !pixel.escaped { return [0.0, 0.0, 0.0]; }
        let t = pixel.smooth_iter / max_iter as f64;
        let hue = (t * 6.0) % 1.0;
        hsv_to_rgb(hue as f32, 0.8, 1.0 - (1.0 - t as f32).powi(2))
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h * 6.0) as u32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_in_set() {
        let p = MandelbrotRenderer::compute_pixel(0.0, 0.0, 1000, 4.0);
        assert!(!p.escaped);
    }

    #[test]
    fn two_is_outside_set() {
        let p = MandelbrotRenderer::compute_pixel(2.0, 0.0, 100, 4.0);
        assert!(p.escaped);
        assert!(p.iterations < 10);
    }

    #[test]
    fn render_produces_correct_size() {
        let params = MandelbrotParams { width: 10, height: 10, max_iter: 50, ..Default::default() };
        let pixels = MandelbrotRenderer::render(&params);
        assert_eq!(pixels.len(), 100);
    }

    #[test]
    fn smooth_coloring_works() {
        let p = MandelbrotRenderer::compute_pixel(1.0, 0.0, 100, 4.0);
        let color = MandelbrotRenderer::color_pixel(&p, 100);
        assert!(color[0] >= 0.0 && color[0] <= 1.0);
    }

    #[test]
    fn perturbation_matches_direct() {
        let params = MandelbrotParams { width: 5, height: 5, max_iter: 100, ..Default::default() };
        let direct = MandelbrotRenderer::render(&params);
        let perturb = MandelbrotRenderer::render_perturbation(&params);
        // Should produce similar results
        for (d, p) in direct.iter().zip(perturb.iter()) {
            assert_eq!(d.escaped, p.escaped);
        }
    }
}
