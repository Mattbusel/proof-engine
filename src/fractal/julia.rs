//! Julia set rendering.

use super::mandelbrot::FractalPixel;

#[derive(Debug, Clone)]
pub struct JuliaParams {
    pub c_re: f64, pub c_im: f64,
    pub center_re: f64, pub center_im: f64,
    pub zoom: f64, pub max_iter: u32,
    pub escape_radius: f64,
    pub width: u32, pub height: u32,
}
impl Default for JuliaParams {
    fn default() -> Self {
        Self { c_re: -0.7, c_im: 0.27015, center_re: 0.0, center_im: 0.0, zoom: 1.0, max_iter: 256, escape_radius: 4.0, width: 800, height: 600 }
    }
}

pub struct JuliaRenderer;
impl JuliaRenderer {
    pub fn compute_pixel(z_re: f64, z_im: f64, c_re: f64, c_im: f64, max_iter: u32, escape_r2: f64) -> FractalPixel {
        let mut zr = z_re; let mut zi = z_im;
        for i in 0..max_iter {
            let r2 = zr * zr; let i2 = zi * zi;
            if r2 + i2 > escape_r2 {
                let smooth = i as f64 + 1.0 - ((r2 + i2).ln() * 0.5 / 2.0_f64.ln()).ln() / 2.0_f64.ln();
                return FractalPixel { iterations: i, smooth_iter: smooth, escaped: true, final_z_re: zr, final_z_im: zi };
            }
            zi = 2.0 * zr * zi + c_im;
            zr = r2 - i2 + c_re;
        }
        FractalPixel { iterations: max_iter, smooth_iter: max_iter as f64, escaped: false, final_z_re: zr, final_z_im: zi }
    }

    pub fn render(params: &JuliaParams) -> Vec<FractalPixel> {
        let (w, h) = (params.width, params.height);
        let aspect = w as f64 / h as f64;
        let scale = 2.0 / params.zoom;
        let escape_r2 = params.escape_radius * params.escape_radius;
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for py in 0..h { for px in 0..w {
            let z_re = params.center_re + (px as f64 / w as f64 - 0.5) * scale * aspect;
            let z_im = params.center_im + (py as f64 / h as f64 - 0.5) * scale;
            pixels.push(Self::compute_pixel(z_re, z_im, params.c_re, params.c_im, params.max_iter, escape_r2));
        }}
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn julia_renders() {
        let params = JuliaParams { width: 10, height: 10, max_iter: 50, ..Default::default() };
        let pixels = JuliaRenderer::render(&params);
        assert_eq!(pixels.len(), 100);
    }
}
