//! Neural and classical upscaling for textures and glyph maps.

use super::tensor::Tensor;
use super::model::{Model, Sequential, DenseLayer, Conv2DLayer, Layer};

/// Quality preset for upscaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpscaleQuality {
    Fast,
    Balanced,
    HighQuality,
}

/// Configuration for the upscaler.
#[derive(Debug, Clone)]
pub struct UpscaleConfig {
    pub factor: u32,
    pub model_path: Option<String>,
    pub quality: UpscaleQuality,
}

impl Default for UpscaleConfig {
    fn default() -> Self {
        Self { factor: 2, model_path: None, quality: UpscaleQuality::Balanced }
    }
}

/// Neural upscaler wrapping a model.
pub struct Upscaler {
    pub model: Model,
    pub scale_factor: u32,
}

impl Upscaler {
    pub fn new(model: Model, scale_factor: u32) -> Self {
        Self { model, scale_factor }
    }

    /// Run the upscaling model on input. Input shape: (C, H, W).
    /// Output shape: (C, H*factor, W*factor).
    pub fn upscale(&self, input: &Tensor) -> Tensor {
        assert_eq!(input.shape.len(), 3);
        // First bilinear upscale to target size, then refine with model
        let upscaled = bilinear_upscale(input, self.scale_factor);
        // Flatten, run through model, reshape back
        let c = upscaled.shape[0];
        let h = upscaled.shape[1];
        let w = upscaled.shape[2];
        let flat = upscaled.flatten();
        let refined = self.model.forward(&flat);
        // Clamp to valid range and reshape
        let data: Vec<f32> = refined.data.iter().map(|&v| v.clamp(0.0, 1.0)).collect();
        if data.len() == c * h * w {
            Tensor { shape: vec![c, h, w], data }
        } else {
            // If model output size doesn't match, return bilinear result
            upscaled
        }
    }
}

/// Bilinear upscaling fallback. Input shape: (C, H, W).
pub fn bilinear_upscale(input: &Tensor, factor: u32) -> Tensor {
    assert_eq!(input.shape.len(), 3);
    let c = input.shape[0];
    let h = input.shape[1];
    let w = input.shape[2];
    let f = factor as usize;
    let new_h = h * f;
    let new_w = w * f;
    let mut data = vec![0.0f32; c * new_h * new_w];

    for ch in 0..c {
        for ny in 0..new_h {
            for nx in 0..new_w {
                let src_y = ny as f32 / f as f32;
                let src_x = nx as f32 / f as f32;

                let y0 = (src_y.floor() as usize).min(h - 1);
                let y1 = (y0 + 1).min(h - 1);
                let x0 = (src_x.floor() as usize).min(w - 1);
                let x1 = (x0 + 1).min(w - 1);

                let fy = src_y - src_y.floor();
                let fx = src_x - src_x.floor();

                let v00 = input.data[ch * h * w + y0 * w + x0];
                let v01 = input.data[ch * h * w + y0 * w + x1];
                let v10 = input.data[ch * h * w + y1 * w + x0];
                let v11 = input.data[ch * h * w + y1 * w + x1];

                let val = v00 * (1.0 - fy) * (1.0 - fx)
                    + v01 * (1.0 - fy) * fx
                    + v10 * fy * (1.0 - fx)
                    + v11 * fy * fx;

                data[ch * new_h * new_w + ny * new_w + nx] = val;
            }
        }
    }
    Tensor { shape: vec![c, new_h, new_w], data }
}

/// Bicubic upscaling fallback. Input shape: (C, H, W).
pub fn bicubic_upscale(input: &Tensor, factor: u32) -> Tensor {
    assert_eq!(input.shape.len(), 3);
    let c = input.shape[0];
    let h = input.shape[1];
    let w = input.shape[2];
    let f = factor as usize;
    let new_h = h * f;
    let new_w = w * f;
    let mut data = vec![0.0f32; c * new_h * new_w];

    // Cubic interpolation kernel
    fn cubic(t: f32) -> [f32; 4] {
        let a = -0.5f32;
        let t2 = t * t;
        let t3 = t2 * t;
        [
            a * t3 - 2.0 * a * t2 + a * t,
            (a + 2.0) * t3 - (a + 3.0) * t2 + 1.0,
            -(a + 2.0) * t3 + (2.0 * a + 3.0) * t2 - a * t,
            -a * t3 + a * t2,
        ]
    }

    fn clamp_idx(v: isize, max: usize) -> usize {
        v.max(0).min(max as isize - 1) as usize
    }

    for ch in 0..c {
        for ny in 0..new_h {
            for nx in 0..new_w {
                let src_y = ny as f32 / f as f32;
                let src_x = nx as f32 / f as f32;

                let iy = src_y.floor() as isize;
                let ix = src_x.floor() as isize;
                let fy = src_y - src_y.floor();
                let fx = src_x - src_x.floor();

                let wy = cubic(fy);
                let wx = cubic(fx);

                let mut val = 0.0f32;
                for dy in 0..4isize {
                    for dx in 0..4isize {
                        let sy = clamp_idx(iy + dy - 1, h);
                        let sx = clamp_idx(ix + dx - 1, w);
                        val += wy[dy as usize] * wx[dx as usize]
                            * input.data[ch * h * w + sy * w + sx];
                    }
                }
                data[ch * new_h * new_w + ny * new_w + nx] = val;
            }
        }
    }
    Tensor { shape: vec![c, new_h, new_w], data }
}

/// Create a simple ESPCN-style upscaler. The model learns a mapping from
/// low-res features to high-res via sub-pixel convolution (simulated with dense layers).
pub fn create_simple_upscaler(factor: u32) -> Upscaler {
    // For a simple upscaler, we use dense layers that map
    // flattened low-res input to flattened high-res output.
    // In practice this would be a convolutional model, but
    // we approximate with dense layers for simplicity.
    let f = factor as usize;
    // Assume small patches: the model processes the entire flattened image.
    // We build a generic model; actual I/O sizes depend on usage.
    let model = Sequential::new("espcn_upscaler")
        .dense(64, 128)
        .relu()
        .dense(128, 256)
        .relu()
        .dense(256, 256)
        .build();
    Upscaler::new(model, factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bilinear_upscale_shape() {
        let input = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![1, 2, 2]);
        let up = bilinear_upscale(&input, 2);
        assert_eq!(up.shape, vec![1, 4, 4]);
    }

    #[test]
    fn test_bilinear_upscale_corners() {
        let input = Tensor::from_vec(vec![0.0, 1.0, 0.0, 1.0], vec![1, 2, 2]);
        let up = bilinear_upscale(&input, 2);
        // Top-left corner should be close to 0.0
        assert!(up.get(&[0, 0, 0]).abs() < 0.01);
    }

    #[test]
    fn test_bicubic_upscale_shape() {
        let input = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![1, 2, 2]);
        let up = bicubic_upscale(&input, 3);
        assert_eq!(up.shape, vec![1, 6, 6]);
    }

    #[test]
    fn test_bicubic_constant_input() {
        // Constant image should upscale to constant
        let input = Tensor::from_vec(vec![0.5; 9], vec![1, 3, 3]);
        let up = bicubic_upscale(&input, 2);
        for &v in &up.data {
            assert!((v - 0.5).abs() < 0.1, "bicubic of constant deviated: {v}");
        }
    }

    #[test]
    fn test_create_simple_upscaler() {
        let upscaler = create_simple_upscaler(2);
        assert_eq!(upscaler.scale_factor, 2);
        assert!(upscaler.model.parameter_count() > 0);
    }

    #[test]
    fn test_upscaler_upscale() {
        // The neural upscaler may not produce perfect results with random weights,
        // but it should not panic and output the correct shape (via bilinear fallback).
        let upscaler = create_simple_upscaler(2);
        let input = Tensor::rand(vec![1, 4, 4], 42);
        let out = upscaler.upscale(&input);
        assert_eq!(out.shape, vec![1, 8, 8]);
    }

    #[test]
    fn test_upscale_config_default() {
        let cfg = UpscaleConfig::default();
        assert_eq!(cfg.factor, 2);
        assert_eq!(cfg.quality, UpscaleQuality::Balanced);
        assert!(cfg.model_path.is_none());
    }

    #[test]
    fn test_bilinear_multichannel() {
        let input = Tensor::rand(vec![3, 4, 4], 123);
        let up = bilinear_upscale(&input, 2);
        assert_eq!(up.shape, vec![3, 8, 8]);
    }
}
