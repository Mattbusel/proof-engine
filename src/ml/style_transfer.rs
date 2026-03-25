//! Neural style transfer for applying artistic styles to content tensors.

use super::tensor::Tensor;
use super::model::{Model, Sequential, DenseLayer, Layer};

/// Pre-baked style presets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StylePreset {
    Pencil,
    Neon,
    Retro,
    Gothic,
    Watercolor,
}

impl StylePreset {
    /// Return (content_weight, style_weight, iterations) tuning for this preset.
    pub fn params(&self) -> (f32, f32, usize) {
        match self {
            StylePreset::Pencil => (1.0, 0.001, 50),
            StylePreset::Neon => (1.0, 0.01, 80),
            StylePreset::Retro => (1.0, 0.005, 60),
            StylePreset::Gothic => (1.0, 0.008, 70),
            StylePreset::Watercolor => (1.0, 0.003, 40),
        }
    }
}

/// Style transfer engine.
pub struct StyleTransfer {
    pub content_model: Model,
    pub style_model: Model,
    pub iterations: usize,
    pub content_weight: f32,
    pub style_weight: f32,
    pub learning_rate: f32,
}

impl StyleTransfer {
    pub fn new(content_model: Model, style_model: Model) -> Self {
        Self {
            content_model,
            style_model,
            iterations: 100,
            content_weight: 1.0,
            style_weight: 0.01,
            learning_rate: 0.01,
        }
    }

    /// Create a style transfer engine from a preset.
    pub fn from_preset(preset: StylePreset) -> Self {
        let (cw, sw, iters) = preset.params();
        // Simple feature extractor models
        let content_model = Sequential::new("content_extractor")
            .dense(64, 32)
            .relu()
            .build();
        let style_model = Sequential::new("style_extractor")
            .dense(64, 32)
            .relu()
            .build();
        Self {
            content_model,
            style_model,
            iterations: iters,
            content_weight: cw,
            style_weight: sw,
            learning_rate: 0.01,
        }
    }

    /// Compute the Gram matrix: G = F^T * F, where F has shape (C, N).
    /// If the input is flattened or 1-D, reshape to (sqrt, sqrt) approximately.
    pub fn gram_matrix(features: &Tensor) -> Tensor {
        let f = if features.shape.len() == 1 {
            features.reshape(vec![1, features.data.len()])
        } else if features.shape.len() == 2 {
            features.clone()
        } else {
            // (C, H, W) -> (C, H*W)
            let c = features.shape[0];
            let spatial: usize = features.shape[1..].iter().product();
            features.reshape(vec![c, spatial])
        };
        let ft = f.transpose();
        Tensor::matmul(&f, &ft)
    }

    /// Content loss: mean squared error between generated and target features.
    pub fn content_loss(generated: &Tensor, target: &Tensor) -> f32 {
        assert_eq!(generated.data.len(), target.data.len());
        let n = generated.data.len() as f32;
        generated.data.iter().zip(&target.data)
            .map(|(g, t)| (g - t) * (g - t))
            .sum::<f32>() / n
    }

    /// Style loss: MSE between Gram matrices of generated and target features.
    pub fn style_loss(generated_gram: &Tensor, target_gram: &Tensor) -> f32 {
        Self::content_loss(generated_gram, target_gram)
    }

    /// Total loss combining content and style.
    pub fn total_loss(
        &self,
        gen_content_features: &Tensor,
        target_content_features: &Tensor,
        gen_style_features: &Tensor,
        target_style_features: &Tensor,
    ) -> f32 {
        let cl = Self::content_loss(gen_content_features, target_content_features);
        let gen_gram = Self::gram_matrix(gen_style_features);
        let target_gram = Self::gram_matrix(target_style_features);
        let sl = Self::style_loss(&gen_gram, &target_gram);
        self.content_weight * cl + self.style_weight * sl
    }

    /// Run iterative style transfer optimization.
    /// content and style should have the same shape.
    /// Returns a generated tensor of the same shape.
    pub fn transfer(&self, content: &Tensor, style: &Tensor) -> Tensor {
        assert_eq!(content.shape, style.shape);
        // Extract target features
        let target_content_feat = self.content_model.forward(content);
        let target_style_feat = self.style_model.forward(style);
        let target_style_gram = Self::gram_matrix(&target_style_feat);

        // Initialize generated image as content clone
        let mut generated = content.clone();
        let lr = self.learning_rate;

        for _iter in 0..self.iterations {
            let gen_content_feat = self.content_model.forward(&generated);
            let gen_style_feat = self.style_model.forward(&generated);
            let gen_style_gram = Self::gram_matrix(&gen_style_feat);

            // Compute gradients via finite differences (simplified)
            // For each element in generated, nudge and measure loss change
            let n = generated.data.len();
            let eps = 1e-4f32;

            // Content gradient: d/dx MSE = 2*(gen - target) / N propagated through model
            // Simplified: we use the feature-space gradient directly mapped back
            let content_diff = gen_content_feat.sub(&target_content_feat);
            let style_diff = gen_style_gram.sub(&target_style_gram);

            // Approximate: update generated by blending towards reducing content loss
            // and style loss. This is a simplified gradient step.
            let content_grad_scale = self.content_weight * 2.0 / n as f32;
            let style_grad_scale = self.style_weight * 2.0 / gen_style_gram.data.len().max(1) as f32;

            // Direct pixel update heuristic: blend content signal and style signal
            let content_signal = content_diff.mean();
            let style_signal = style_diff.mean();
            let total_signal = content_grad_scale * content_signal + style_grad_scale * style_signal;

            for i in 0..n {
                // Move each pixel slightly toward content value and away from loss
                let toward_content = (content.data[i] - generated.data[i]) * 0.1;
                let toward_style = (style.data[i] - generated.data[i]) * 0.05;
                generated.data[i] += lr * (toward_content * self.content_weight
                    + toward_style * self.style_weight
                    - total_signal * 0.01);
            }
        }
        generated
    }
}

/// ASCII-art style transfer: applies style modifications to glyph emission/color values.
pub struct AsciiStyleTransfer {
    pub preset: StylePreset,
}

impl AsciiStyleTransfer {
    pub fn new(preset: StylePreset) -> Self {
        Self { preset }
    }

    /// Apply style to a 1-D tensor of glyph values (brightness/emission).
    /// Returns modified values biased by the style preset.
    pub fn apply(&self, values: &Tensor) -> Tensor {
        let data: Vec<f32> = match self.preset {
            StylePreset::Pencil => {
                // High contrast, emphasize edges
                values.data.iter().map(|&v| {
                    if v > 0.5 { (v * 1.5).min(1.0) } else { (v * 0.3).max(0.0) }
                }).collect()
            }
            StylePreset::Neon => {
                // Boost bright values, saturate
                values.data.iter().map(|&v| {
                    let boosted = v * 2.0;
                    (1.0 / (1.0 + (-10.0 * (boosted - 0.5)).exp())).min(1.0)
                }).collect()
            }
            StylePreset::Retro => {
                // Quantize to 4 levels
                values.data.iter().map(|&v| {
                    ((v * 4.0).floor() / 4.0).clamp(0.0, 1.0)
                }).collect()
            }
            StylePreset::Gothic => {
                // Darken everything, high contrast
                values.data.iter().map(|&v| {
                    (v * v * 1.2).min(1.0)
                }).collect()
            }
            StylePreset::Watercolor => {
                // Soft, blurred feel via smoothing adjacent values
                let n = values.data.len();
                let mut out = vec![0.0f32; n];
                for i in 0..n {
                    let prev = if i > 0 { values.data[i - 1] } else { values.data[i] };
                    let next = if i + 1 < n { values.data[i + 1] } else { values.data[i] };
                    out[i] = (prev * 0.25 + values.data[i] * 0.5 + next * 0.25).clamp(0.0, 1.0);
                }
                out
            }
        };
        Tensor { shape: values.shape.clone(), data }
    }

    /// Apply color tinting based on preset. Input: (N, 4) RGBA tensor.
    pub fn tint_colors(&self, colors: &Tensor) -> Tensor {
        assert_eq!(colors.shape.len(), 2);
        assert_eq!(colors.shape[1], 4);
        let n = colors.shape[0];
        let mut data = colors.data.clone();
        let (r_mul, g_mul, b_mul) = match self.preset {
            StylePreset::Pencil => (0.9, 0.9, 0.9),
            StylePreset::Neon => (1.2, 0.3, 1.5),
            StylePreset::Retro => (1.1, 0.8, 0.5),
            StylePreset::Gothic => (0.3, 0.1, 0.3),
            StylePreset::Watercolor => (0.8, 0.9, 1.1),
        };
        for i in 0..n {
            let base = i * 4;
            data[base] = (data[base] * r_mul).clamp(0.0, 1.0);
            data[base + 1] = (data[base + 1] * g_mul).clamp(0.0, 1.0);
            data[base + 2] = (data[base + 2] * b_mul).clamp(0.0, 1.0);
            // alpha unchanged
        }
        Tensor { shape: colors.shape.clone(), data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gram_matrix() {
        // 2x3 matrix -> gram is 2x2
        let f = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);
        let g = StyleTransfer::gram_matrix(&f);
        assert_eq!(g.shape, vec![2, 2]);
        // G[0,0] = 1*1+2*2+3*3 = 14
        assert_eq!(g.get(&[0, 0]), 14.0);
        // G[0,1] = 1*4+2*5+3*6 = 32
        assert_eq!(g.get(&[0, 1]), 32.0);
    }

    #[test]
    fn test_content_loss() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0], vec![3]);
        let b = Tensor::from_vec(vec![1.0, 2.0, 3.0], vec![3]);
        assert_eq!(StyleTransfer::content_loss(&a, &b), 0.0);

        let c = Tensor::from_vec(vec![2.0, 3.0, 4.0], vec![3]);
        let loss = StyleTransfer::content_loss(&a, &c);
        assert!((loss - 1.0).abs() < 1e-5); // MSE = (1+1+1)/3 = 1
    }

    #[test]
    fn test_style_loss() {
        let a = Tensor::from_vec(vec![1.0, 0.0, 0.0, 1.0], vec![2, 2]);
        let b = Tensor::from_vec(vec![1.0, 0.0, 0.0, 1.0], vec![2, 2]);
        assert_eq!(StyleTransfer::style_loss(&a, &b), 0.0);
    }

    #[test]
    fn test_transfer_preserves_shape() {
        let st = StyleTransfer::from_preset(StylePreset::Pencil);
        // Need input matching model's expected input size (64)
        let content = Tensor::rand(vec![1, 64], 42);
        let style = Tensor::rand(vec![1, 64], 99);
        let result = st.transfer(&content, &style);
        assert_eq!(result.shape, content.shape);
    }

    #[test]
    fn test_ascii_style_pencil() {
        let ast = AsciiStyleTransfer::new(StylePreset::Pencil);
        let vals = Tensor::from_vec(vec![0.1, 0.5, 0.9], vec![3]);
        let result = ast.apply(&vals);
        assert_eq!(result.shape, vec![3]);
        // Pencil: low values get darker, high values get brighter
        assert!(result.data[0] < vals.data[0]);
        assert!(result.data[2] > vals.data[2] || (result.data[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_ascii_style_retro_quantizes() {
        let ast = AsciiStyleTransfer::new(StylePreset::Retro);
        let vals = Tensor::from_vec(vec![0.13, 0.37, 0.62, 0.88], vec![4]);
        let result = ast.apply(&vals);
        // Should be quantized to multiples of 0.25
        for &v in &result.data {
            let remainder = (v * 4.0) - (v * 4.0).floor();
            assert!(remainder.abs() < 1e-5);
        }
    }

    #[test]
    fn test_tint_colors() {
        let ast = AsciiStyleTransfer::new(StylePreset::Neon);
        let colors = Tensor::from_vec(vec![0.5, 0.5, 0.5, 1.0], vec![1, 4]);
        let tinted = ast.tint_colors(&colors);
        assert_eq!(tinted.shape, vec![1, 4]);
        // Neon boosts R and B, dims G
        assert!(tinted.data[0] > 0.5); // R boosted
        assert!(tinted.data[1] < 0.5); // G dimmed
        assert!(tinted.data[2] > 0.5); // B boosted
        assert_eq!(tinted.data[3], 1.0); // alpha unchanged
    }

    #[test]
    fn test_all_presets() {
        for preset in &[StylePreset::Pencil, StylePreset::Neon, StylePreset::Retro, StylePreset::Gothic, StylePreset::Watercolor] {
            let ast = AsciiStyleTransfer::new(*preset);
            let vals = Tensor::from_vec(vec![0.3, 0.6, 0.9], vec![3]);
            let result = ast.apply(&vals);
            assert_eq!(result.shape, vec![3]);
            // All values should be in [0, 1]
            for &v in &result.data {
                assert!(v >= 0.0 && v <= 1.0, "preset {:?} produced out-of-range value {v}", preset);
            }
        }
    }
}
