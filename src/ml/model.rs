//! Neural network model construction and execution.

use super::tensor::Tensor;
use std::io::{Read, Write};

/// A single layer in a neural network.
#[derive(Debug, Clone)]
pub enum Layer {
    Dense(DenseLayer),
    Conv2D(Conv2DLayer),
    MaxPool(MaxPoolLayer),
    BatchNorm(BatchNormLayer),
    Dropout(DropoutLayer),
    Flatten,
    ReLU,
    Sigmoid,
    Tanh,
    Softmax(usize), // axis
    GELU,
    Residual(ResidualBlock),
    Attention(MultiHeadAttention),
}

#[derive(Debug, Clone)]
pub struct DenseLayer {
    pub weights: Tensor, // (in_features, out_features)
    pub bias: Tensor,    // (out_features,)
}

#[derive(Debug, Clone)]
pub struct Conv2DLayer {
    pub filters: Tensor, // (c_out, c_in, kh, kw)
    pub bias: Tensor,    // (c_out,)
    pub stride: usize,
    pub padding: usize,
}

#[derive(Debug, Clone)]
pub struct MaxPoolLayer {
    pub kernel_size: usize,
    pub stride: usize,
}

#[derive(Debug, Clone)]
pub struct BatchNormLayer {
    pub gamma: Tensor,
    pub beta: Tensor,
    pub running_mean: Tensor,
    pub running_var: Tensor,
    pub eps: f32,
}

#[derive(Debug, Clone)]
pub struct DropoutLayer {
    pub p: f32,
    pub training: bool,
}

#[derive(Debug, Clone)]
pub struct ResidualBlock {
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone)]
pub struct MultiHeadAttention {
    pub heads: usize,
    pub d_model: usize,
    pub d_k: usize,
    pub w_q: Tensor, // (d_model, d_model)
    pub w_k: Tensor,
    pub w_v: Tensor,
    pub w_o: Tensor,
}

impl DenseLayer {
    pub fn new(in_features: usize, out_features: usize) -> Self {
        // Xavier initialization
        let scale = (2.0 / (in_features + out_features) as f32).sqrt();
        let w = Tensor::rand(vec![in_features, out_features], (in_features * out_features) as u64);
        let weights = Tensor {
            shape: w.shape.clone(),
            data: w.data.iter().map(|v| (v - 0.5) * 2.0 * scale).collect(),
        };
        let bias = Tensor::zeros(vec![out_features]);
        Self { weights, bias }
    }

    pub fn forward(&self, input: &Tensor) -> Tensor {
        // input: (batch, in_features) or (in_features,)
        let is_1d = input.shape.len() == 1;
        let input_2d = if is_1d {
            input.reshape(vec![1, input.shape[0]])
        } else {
            input.clone()
        };
        // out = input @ weights + bias
        let mut out = Tensor::matmul(&input_2d, &self.weights);
        // add bias to each row
        let batch = out.shape[0];
        let out_f = out.shape[1];
        for b in 0..batch {
            for j in 0..out_f {
                out.data[b * out_f + j] += self.bias.data[j];
            }
        }
        if is_1d { out.reshape(vec![out_f]) } else { out }
    }

    pub fn parameter_count(&self) -> usize {
        self.weights.data.len() + self.bias.data.len()
    }
}

impl Conv2DLayer {
    pub fn new(in_channels: usize, out_channels: usize, kernel_size: usize) -> Self {
        let n = out_channels * in_channels * kernel_size * kernel_size;
        let scale = (2.0 / (in_channels * kernel_size * kernel_size) as f32).sqrt();
        let r = Tensor::rand(vec![out_channels, in_channels, kernel_size, kernel_size], n as u64);
        let filters = Tensor {
            shape: r.shape.clone(),
            data: r.data.iter().map(|v| (v - 0.5) * 2.0 * scale).collect(),
        };
        let bias = Tensor::zeros(vec![out_channels]);
        Self { filters, bias, stride: 1, padding: 0 }
    }

    pub fn forward(&self, input: &Tensor) -> Tensor {
        let mut out = input.conv2d(&self.filters, self.stride, self.padding);
        // add bias per channel
        let c_out = out.shape[0];
        let spatial: usize = out.shape[1..].iter().product();
        for c in 0..c_out {
            for s in 0..spatial {
                out.data[c * spatial + s] += self.bias.data[c];
            }
        }
        out
    }

    pub fn parameter_count(&self) -> usize {
        self.filters.data.len() + self.bias.data.len()
    }
}

impl MultiHeadAttention {
    pub fn new(heads: usize, d_model: usize) -> Self {
        let d_k = d_model / heads;
        let init = |seed: u64| {
            let r = Tensor::rand(vec![d_model, d_model], seed);
            let scale = (1.0 / d_model as f32).sqrt();
            Tensor {
                shape: r.shape.clone(),
                data: r.data.iter().map(|v| (v - 0.5) * 2.0 * scale).collect(),
            }
        };
        Self {
            heads,
            d_model,
            d_k,
            w_q: init(1001),
            w_k: init(2002),
            w_v: init(3003),
            w_o: init(4004),
        }
    }

    /// Forward pass. Input shape: (seq_len, d_model). Returns same shape.
    pub fn forward(&self, input: &Tensor) -> Tensor {
        assert_eq!(input.shape.len(), 2);
        let seq_len = input.shape[0];
        let d_model = input.shape[1];
        assert_eq!(d_model, self.d_model);

        let q = Tensor::matmul(input, &self.w_q);
        let k = Tensor::matmul(input, &self.w_k);
        let v = Tensor::matmul(input, &self.w_v);

        let d_k = self.d_k;
        let scale = 1.0 / (d_k as f32).sqrt();

        // accumulate multi-head output
        let mut concat_heads = vec![0.0f32; seq_len * d_model];

        for h in 0..self.heads {
            let offset = h * d_k;
            // extract head slices (seq_len, d_k) for Q, K, V
            let mut qh = vec![0.0f32; seq_len * d_k];
            let mut kh = vec![0.0f32; seq_len * d_k];
            let mut vh = vec![0.0f32; seq_len * d_k];
            for s in 0..seq_len {
                for j in 0..d_k {
                    qh[s * d_k + j] = q.data[s * d_model + offset + j];
                    kh[s * d_k + j] = k.data[s * d_model + offset + j];
                    vh[s * d_k + j] = v.data[s * d_model + offset + j];
                }
            }
            let qh = Tensor::from_vec(qh, vec![seq_len, d_k]);
            let kh_t = Tensor::from_vec(kh, vec![seq_len, d_k]).transpose();
            let vh = Tensor::from_vec(vh, vec![seq_len, d_k]);

            // scores = Q @ K^T / sqrt(d_k)
            let scores = Tensor::matmul(&qh, &kh_t).scale(scale);
            // attention weights via softmax over last axis
            let attn = scores.softmax(1);
            // context = attn @ V
            let context = Tensor::matmul(&attn, &vh);

            // write into concat buffer
            for s in 0..seq_len {
                for j in 0..d_k {
                    concat_heads[s * d_model + offset + j] = context.data[s * d_k + j];
                }
            }
        }

        let concat = Tensor::from_vec(concat_heads, vec![seq_len, d_model]);
        Tensor::matmul(&concat, &self.w_o)
    }

    pub fn parameter_count(&self) -> usize {
        self.w_q.data.len() + self.w_k.data.len() + self.w_v.data.len() + self.w_o.data.len()
    }
}

impl Layer {
    pub fn forward(&self, input: &Tensor) -> Tensor {
        match self {
            Layer::Dense(l) => l.forward(input),
            Layer::Conv2D(l) => l.forward(input),
            Layer::MaxPool(l) => input.max_pool2d(l.kernel_size, l.stride),
            Layer::BatchNorm(l) => {
                input.batch_norm(&l.running_mean, &l.running_var, &l.gamma, &l.beta, l.eps)
            }
            Layer::Dropout(l) => input.dropout(l.p, 12345, l.training),
            Layer::Flatten => input.flatten(),
            Layer::ReLU => input.relu(),
            Layer::Sigmoid => input.sigmoid(),
            Layer::Tanh => input.tanh_act(),
            Layer::Softmax(axis) => input.softmax(*axis),
            Layer::GELU => input.gelu(),
            Layer::Residual(block) => {
                let mut out = input.clone();
                for layer in &block.layers {
                    out = layer.forward(&out);
                }
                input.add(&out)
            }
            Layer::Attention(attn) => attn.forward(input),
        }
    }

    pub fn parameter_count(&self) -> usize {
        match self {
            Layer::Dense(l) => l.parameter_count(),
            Layer::Conv2D(l) => l.parameter_count(),
            Layer::BatchNorm(l) => l.gamma.data.len() + l.beta.data.len(),
            Layer::Attention(a) => a.parameter_count(),
            Layer::Residual(block) => block.layers.iter().map(|l| l.parameter_count()).sum(),
            _ => 0,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Layer::Dense(_) => "Dense",
            Layer::Conv2D(_) => "Conv2D",
            Layer::MaxPool(_) => "MaxPool",
            Layer::BatchNorm(_) => "BatchNorm",
            Layer::Dropout(_) => "Dropout",
            Layer::Flatten => "Flatten",
            Layer::ReLU => "ReLU",
            Layer::Sigmoid => "Sigmoid",
            Layer::Tanh => "Tanh",
            Layer::Softmax(_) => "Softmax",
            Layer::GELU => "GELU",
            Layer::Residual(_) => "Residual",
            Layer::Attention(_) => "Attention",
        }
    }
}

/// A sequential neural network model.
#[derive(Debug, Clone)]
pub struct Model {
    pub layers: Vec<Layer>,
    pub name: String,
}

impl Model {
    pub fn new(name: &str) -> Self {
        Self { layers: Vec::new(), name: name.to_string() }
    }

    pub fn forward(&self, input: &Tensor) -> Tensor {
        let mut x = input.clone();
        for layer in &self.layers {
            x = layer.forward(&x);
        }
        x
    }

    pub fn parameter_count(&self) -> usize {
        self.layers.iter().map(|l| l.parameter_count()).sum()
    }

    /// Collect all weight tensors from the model in order.
    fn collect_weights(&self) -> Vec<&Tensor> {
        let mut weights = Vec::new();
        for layer in &self.layers {
            match layer {
                Layer::Dense(l) => { weights.push(&l.weights); weights.push(&l.bias); }
                Layer::Conv2D(l) => { weights.push(&l.filters); weights.push(&l.bias); }
                Layer::BatchNorm(l) => {
                    weights.push(&l.gamma); weights.push(&l.beta);
                    weights.push(&l.running_mean); weights.push(&l.running_var);
                }
                Layer::Attention(a) => {
                    weights.push(&a.w_q); weights.push(&a.w_k);
                    weights.push(&a.w_v); weights.push(&a.w_o);
                }
                Layer::Residual(block) => {
                    // For simplicity, build a temp Model to reuse logic
                    let m = Model { layers: block.layers.clone(), name: String::new() };
                    // We can't easily return refs into block here without
                    // restructuring, so we skip residual sub-weights in save/load.
                    let _ = m;
                }
                _ => {}
            }
        }
        weights
    }

    /// Save weights to a simple binary format:
    /// For each tensor: [ndim: u32] [shape[0]: u32] ... [shape[n-1]: u32] [data as f32 LE bytes]
    pub fn save_weights(&self, path: &str) -> Result<(), String> {
        let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let weights = self.collect_weights();
        let count = weights.len() as u32;
        file.write_all(&count.to_le_bytes()).map_err(|e| e.to_string())?;
        for w in weights {
            let ndim = w.shape.len() as u32;
            file.write_all(&ndim.to_le_bytes()).map_err(|e| e.to_string())?;
            for &d in &w.shape {
                file.write_all(&(d as u32).to_le_bytes()).map_err(|e| e.to_string())?;
            }
            for &v in &w.data {
                file.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Load weights from the binary format written by `save_weights`.
    pub fn load_weights(&mut self, path: &str) -> Result<(), String> {
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut buf4 = [0u8; 4];

        file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
        let count = u32::from_le_bytes(buf4) as usize;

        let mut tensors = Vec::with_capacity(count);
        for _ in 0..count {
            file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
            let ndim = u32::from_le_bytes(buf4) as usize;
            let mut shape = Vec::with_capacity(ndim);
            for _ in 0..ndim {
                file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
                shape.push(u32::from_le_bytes(buf4) as usize);
            }
            let n: usize = shape.iter().product();
            let mut data = Vec::with_capacity(n);
            for _ in 0..n {
                file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
                data.push(f32::from_le_bytes(buf4));
            }
            tensors.push(Tensor { shape, data });
        }

        // Assign weights back to layers
        let mut idx = 0;
        for layer in &mut self.layers {
            match layer {
                Layer::Dense(l) => {
                    if idx + 1 < tensors.len() {
                        l.weights = tensors[idx].clone();
                        l.bias = tensors[idx + 1].clone();
                        idx += 2;
                    }
                }
                Layer::Conv2D(l) => {
                    if idx + 1 < tensors.len() {
                        l.filters = tensors[idx].clone();
                        l.bias = tensors[idx + 1].clone();
                        idx += 2;
                    }
                }
                Layer::BatchNorm(l) => {
                    if idx + 3 < tensors.len() {
                        l.gamma = tensors[idx].clone();
                        l.beta = tensors[idx + 1].clone();
                        l.running_mean = tensors[idx + 2].clone();
                        l.running_var = tensors[idx + 3].clone();
                        idx += 4;
                    }
                }
                Layer::Attention(a) => {
                    if idx + 3 < tensors.len() {
                        a.w_q = tensors[idx].clone();
                        a.w_k = tensors[idx + 1].clone();
                        a.w_v = tensors[idx + 2].clone();
                        a.w_o = tensors[idx + 3].clone();
                        idx += 4;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Human-readable model summary.
pub struct ModelSummary;

impl ModelSummary {
    pub fn print(model: &Model) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Model: {}", model.name));
        lines.push(format!("{:-<60}", ""));
        lines.push(format!("{:<20} {:>20} {:>15}", "Layer", "Output Shape", "Params"));
        lines.push(format!("{:-<60}", ""));
        for (i, layer) in model.layers.iter().enumerate() {
            let params = layer.parameter_count();
            lines.push(format!("{:<20} {:>20} {:>15}", format!("{}_{}", layer.name(), i), "dynamic", params));
        }
        lines.push(format!("{:-<60}", ""));
        lines.push(format!("Total parameters: {}", model.parameter_count()));
        lines.join("\n")
    }
}

/// Builder for constructing models layer by layer.
pub struct Sequential {
    layers: Vec<Layer>,
    name: String,
}

impl Sequential {
    pub fn new(name: &str) -> Self {
        Self { layers: Vec::new(), name: name.to_string() }
    }

    pub fn dense(mut self, in_features: usize, out_features: usize) -> Self {
        self.layers.push(Layer::Dense(DenseLayer::new(in_features, out_features)));
        self
    }

    pub fn conv2d(mut self, in_channels: usize, out_channels: usize, kernel_size: usize) -> Self {
        self.layers.push(Layer::Conv2D(Conv2DLayer::new(in_channels, out_channels, kernel_size)));
        self
    }

    pub fn max_pool(mut self, kernel_size: usize, stride: usize) -> Self {
        self.layers.push(Layer::MaxPool(MaxPoolLayer { kernel_size, stride }));
        self
    }

    pub fn batch_norm(mut self, num_features: usize) -> Self {
        self.layers.push(Layer::BatchNorm(BatchNormLayer {
            gamma: Tensor::ones(vec![num_features]),
            beta: Tensor::zeros(vec![num_features]),
            running_mean: Tensor::zeros(vec![num_features]),
            running_var: Tensor::ones(vec![num_features]),
            eps: 1e-5,
        }));
        self
    }

    pub fn dropout(mut self, p: f32) -> Self {
        self.layers.push(Layer::Dropout(DropoutLayer { p, training: true }));
        self
    }

    pub fn flatten(mut self) -> Self {
        self.layers.push(Layer::Flatten);
        self
    }

    pub fn relu(mut self) -> Self {
        self.layers.push(Layer::ReLU);
        self
    }

    pub fn sigmoid(mut self) -> Self {
        self.layers.push(Layer::Sigmoid);
        self
    }

    pub fn tanh_act(mut self) -> Self {
        self.layers.push(Layer::Tanh);
        self
    }

    pub fn softmax(mut self) -> Self {
        // default: softmax over the last axis, represented as axis=0 for 1-D
        self.layers.push(Layer::Softmax(0));
        self
    }

    pub fn softmax_axis(mut self, axis: usize) -> Self {
        self.layers.push(Layer::Softmax(axis));
        self
    }

    pub fn gelu(mut self) -> Self {
        self.layers.push(Layer::GELU);
        self
    }

    pub fn residual(mut self, layers: Vec<Layer>) -> Self {
        self.layers.push(Layer::Residual(ResidualBlock { layers }));
        self
    }

    pub fn attention(mut self, heads: usize, d_model: usize) -> Self {
        self.layers.push(Layer::Attention(MultiHeadAttention::new(heads, d_model)));
        self
    }

    pub fn layer(mut self, layer: Layer) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn build(self) -> Model {
        Model { layers: self.layers, name: self.name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dense_forward_shape() {
        let layer = DenseLayer::new(4, 3);
        let input = Tensor::ones(vec![2, 4]);
        let out = layer.forward(&input);
        assert_eq!(out.shape, vec![2, 3]);
    }

    #[test]
    fn test_dense_forward_1d() {
        let layer = DenseLayer::new(3, 2);
        let input = Tensor::ones(vec![3]);
        let out = layer.forward(&input);
        assert_eq!(out.shape, vec![2]);
    }

    #[test]
    fn test_sequential_build() {
        let model = Sequential::new("test")
            .dense(10, 5)
            .relu()
            .dense(5, 2)
            .softmax()
            .build();
        assert_eq!(model.layers.len(), 4);
        assert_eq!(model.name, "test");
    }

    #[test]
    fn test_model_forward_shape() {
        let model = Sequential::new("mlp")
            .dense(4, 8)
            .relu()
            .dense(8, 3)
            .build();
        let input = Tensor::ones(vec![2, 4]);
        let out = model.forward(&input);
        assert_eq!(out.shape, vec![2, 3]);
    }

    #[test]
    fn test_parameter_count() {
        let model = Sequential::new("mlp")
            .dense(10, 5) // 10*5 + 5 = 55
            .dense(5, 2)  // 5*2 + 2 = 12
            .build();
        assert_eq!(model.parameter_count(), 55 + 12);
    }

    #[test]
    fn test_residual_connection() {
        // residual block: dense(4,4) + relu, then added to input
        let block_layers = vec![
            Layer::Dense(DenseLayer {
                weights: Tensor::zeros(vec![4, 4]),
                bias: Tensor::zeros(vec![4]),
            }),
            Layer::ReLU,
        ];
        let model = Sequential::new("res")
            .residual(block_layers)
            .build();
        let input = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![1, 4]);
        let out = model.forward(&input);
        // With zero weights, dense outputs zeros, relu(zeros)=zeros, residual = input + 0 = input
        assert_eq!(out.shape, vec![1, 4]);
        assert_eq!(out.data, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_attention_forward_shape() {
        let attn = MultiHeadAttention::new(2, 4);
        let input = Tensor::rand(vec![3, 4], 42); // seq_len=3, d_model=4
        let out = attn.forward(&input);
        assert_eq!(out.shape, vec![3, 4]);
    }

    #[test]
    fn test_model_summary() {
        let model = Sequential::new("demo")
            .dense(10, 5)
            .relu()
            .build();
        let summary = ModelSummary::print(&model);
        assert!(summary.contains("demo"));
        assert!(summary.contains("Dense"));
        assert!(summary.contains("ReLU"));
    }

    #[test]
    fn test_save_load_weights() {
        let model = Sequential::new("test")
            .dense(3, 2)
            .build();
        let path = std::env::temp_dir().join("proof_engine_test_weights.bin");
        let path_str = path.to_str().unwrap();
        model.save_weights(path_str).unwrap();

        let mut model2 = Sequential::new("test")
            .dense(3, 2)
            .build();
        model2.load_weights(path_str).unwrap();

        // weights should match
        if let (Layer::Dense(l1), Layer::Dense(l2)) = (&model.layers[0], &model2.layers[0]) {
            assert_eq!(l1.weights.data, l2.weights.data);
            assert_eq!(l1.bias.data, l2.bias.data);
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_conv2d_layer_forward() {
        let layer = Conv2DLayer::new(1, 2, 3);
        let input = Tensor::ones(vec![1, 5, 5]);
        let out = layer.forward(&input);
        assert_eq!(out.shape, vec![2, 3, 3]);
    }
}
