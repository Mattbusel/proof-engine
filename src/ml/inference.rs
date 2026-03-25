//! Inference engine: run models, batch inference, ONNX loading, quantization.

use super::tensor::Tensor;
use super::model::*;
use std::io::Read;
use std::time::Instant;

/// Compute device target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Device {
    CPU,
    GPUCompute,
}

/// Inference engine wrapping a model and target device.
pub struct InferenceEngine {
    pub model: Model,
    pub device: Device,
    pub stats: InferenceStats,
}

/// Statistics from a forward pass.
#[derive(Debug, Clone, Default)]
pub struct InferenceStats {
    pub latency_ms: f64,
    pub memory_bytes: usize,
    pub flops: usize,
}

impl InferenceEngine {
    pub fn new(model: Model, device: Device) -> Self {
        Self {
            model,
            device,
            stats: InferenceStats::default(),
        }
    }

    /// Run a single inference pass.
    pub fn infer(&mut self, input: &Tensor) -> Tensor {
        let start = Instant::now();
        let result = self.model.forward(input);
        let elapsed = start.elapsed();
        self.stats.latency_ms = elapsed.as_secs_f64() * 1000.0;
        self.stats.memory_bytes = result.data.len() * 4 + input.data.len() * 4;
        self.stats.flops = self.estimate_flops(input);
        result
    }

    /// Batched inference: run each input through the model.
    pub fn batch_infer(&mut self, inputs: &[Tensor]) -> Vec<Tensor> {
        let start = Instant::now();
        let results: Vec<Tensor> = inputs.iter().map(|inp| self.model.forward(inp)).collect();
        let elapsed = start.elapsed();
        self.stats.latency_ms = elapsed.as_secs_f64() * 1000.0;
        self.stats.memory_bytes = results.iter().map(|r| r.data.len() * 4).sum::<usize>()
            + inputs.iter().map(|i| i.data.len() * 4).sum::<usize>();
        self.stats.flops = inputs.iter().map(|i| self.estimate_flops(i)).sum();
        results
    }

    /// Warm up the inference pipeline by running dummy inputs.
    pub fn warm_up(&mut self, input_shape: Vec<usize>, runs: usize) {
        let dummy = Tensor::zeros(input_shape);
        for _ in 0..runs {
            let _ = self.model.forward(&dummy);
        }
    }

    /// Rough FLOPs estimation based on layer types.
    fn estimate_flops(&self, input: &Tensor) -> usize {
        let mut flops = 0usize;
        let mut current_size: usize = input.data.len();
        for layer in &self.model.layers {
            match layer {
                Layer::Dense(d) => {
                    let m = current_size / d.weights.shape[0];
                    let k = d.weights.shape[0];
                    let n = d.weights.shape[1];
                    flops += 2 * m * k * n;
                    current_size = m * n;
                }
                Layer::Conv2D(c) => {
                    let c_out = c.filters.shape[0];
                    let c_in = c.filters.shape[1];
                    let kh = c.filters.shape[2];
                    let kw = c.filters.shape[3];
                    // rough: output_spatial * c_out * c_in * kh * kw * 2
                    flops += current_size * c_out * kh * kw * 2 / c_in.max(1);
                }
                Layer::Attention(a) => {
                    // Q,K,V projections + attention + output projection
                    flops += 4 * a.d_model * a.d_model * 2;
                }
                _ => {
                    // element-wise ops: ~N flops
                    flops += current_size;
                }
            }
        }
        flops
    }
}

// ── ONNX Loader ─────────────────────────────────────────────────────────

/// Supported ONNX operation types (simplified).
#[derive(Debug, Clone)]
enum OnnxOp {
    Gemm { transA: bool, transB: bool, alpha: f32, beta: f32 },
    Conv { strides: Vec<usize>, pads: Vec<usize> },
    Relu,
    MaxPool { kernel_shape: Vec<usize>, strides: Vec<usize> },
    BatchNorm { eps: f32 },
    Reshape,
    Softmax { axis: i32 },
    Add,
    Mul,
}

/// Minimal ONNX-like graph node.
#[derive(Debug, Clone)]
struct OnnxNode {
    op: OnnxOp,
    inputs: Vec<String>,
    outputs: Vec<String>,
}

/// ONNX model loader.
pub struct OnnxLoader;

impl OnnxLoader {
    /// Load an ONNX model from a binary file. This implements a minimal
    /// subset of the protobuf format — enough for simple models.
    ///
    /// The format we support is our own simplified binary:
    /// - magic: b"ONNX" (4 bytes)
    /// - num_nodes: u32 LE
    /// - For each node:
    ///   - op_type: u8 (0=Gemm,1=Conv,2=Relu,3=MaxPool,4=BatchNorm,5=Reshape,6=Softmax,7=Add,8=Mul)
    ///   - num_weights: u32 LE
    ///   - For each weight tensor: [ndim: u32] [shape...] [data as f32 LE]
    pub fn load_onnx(path: &str) -> Result<Model, String> {
        let mut file = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let mut buf4 = [0u8; 4];
        let mut buf1 = [0u8; 1];

        // magic
        file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
        if &buf4 != b"ONNX" {
            return Err("invalid ONNX magic".into());
        }

        // num_nodes
        file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
        let num_nodes = u32::from_le_bytes(buf4) as usize;

        let mut layers = Vec::new();

        for _ in 0..num_nodes {
            file.read_exact(&mut buf1).map_err(|e| e.to_string())?;
            let op_type = buf1[0];

            file.read_exact(&mut buf4).map_err(|e| e.to_string())?;
            let num_weights = u32::from_le_bytes(buf4) as usize;

            let mut tensors = Vec::new();
            for _ in 0..num_weights {
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

            let layer = match op_type {
                0 => {
                    // Gemm -> Dense
                    if tensors.len() >= 2 {
                        Layer::Dense(DenseLayer {
                            weights: tensors[0].clone(),
                            bias: tensors[1].clone(),
                        })
                    } else {
                        return Err("Gemm requires 2 weight tensors".into());
                    }
                }
                1 => {
                    // Conv
                    if tensors.len() >= 2 {
                        Layer::Conv2D(Conv2DLayer {
                            filters: tensors[0].clone(),
                            bias: tensors[1].clone(),
                            stride: 1,
                            padding: 0,
                        })
                    } else {
                        return Err("Conv requires 2 weight tensors".into());
                    }
                }
                2 => Layer::ReLU,
                3 => Layer::MaxPool(MaxPoolLayer { kernel_size: 2, stride: 2 }),
                4 => {
                    // BatchNorm
                    if tensors.len() >= 4 {
                        Layer::BatchNorm(BatchNormLayer {
                            gamma: tensors[0].clone(),
                            beta: tensors[1].clone(),
                            running_mean: tensors[2].clone(),
                            running_var: tensors[3].clone(),
                            eps: 1e-5,
                        })
                    } else {
                        return Err("BatchNorm requires 4 tensors".into());
                    }
                }
                5 => Layer::Flatten, // Reshape treated as flatten
                6 => Layer::Softmax(0),
                7 | 8 => {
                    // Add / Mul are skip layers (element-wise with weights handled
                    // at a higher level; here we just store as identity)
                    Layer::ReLU // placeholder: identity-ish
                }
                _ => return Err(format!("unknown op type {op_type}")),
            };
            layers.push(layer);
        }

        Ok(Model { layers, name: "onnx_model".to_string() })
    }

    /// Write a model in our simplified ONNX binary format.
    pub fn save_onnx(model: &Model, path: &str) -> Result<(), String> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        file.write_all(b"ONNX").map_err(|e| e.to_string())?;
        let num_nodes = model.layers.len() as u32;
        file.write_all(&num_nodes.to_le_bytes()).map_err(|e| e.to_string())?;

        for layer in &model.layers {
            let (op_type, tensors): (u8, Vec<&Tensor>) = match layer {
                Layer::Dense(l) => (0, vec![&l.weights, &l.bias]),
                Layer::Conv2D(l) => (1, vec![&l.filters, &l.bias]),
                Layer::ReLU => (2, vec![]),
                Layer::MaxPool(_) => (3, vec![]),
                Layer::BatchNorm(l) => (4, vec![&l.gamma, &l.beta, &l.running_mean, &l.running_var]),
                Layer::Flatten => (5, vec![]),
                Layer::Softmax(_) => (6, vec![]),
                _ => (2, vec![]), // default to relu-like
            };
            file.write_all(&[op_type]).map_err(|e| e.to_string())?;
            let nw = tensors.len() as u32;
            file.write_all(&nw.to_le_bytes()).map_err(|e| e.to_string())?;
            for t in tensors {
                let ndim = t.shape.len() as u32;
                file.write_all(&ndim.to_le_bytes()).map_err(|e| e.to_string())?;
                for &d in &t.shape {
                    file.write_all(&(d as u32).to_le_bytes()).map_err(|e| e.to_string())?;
                }
                for &v in &t.data {
                    file.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
}

// ── Quantization ────────────────────────────────────────────────────────

/// Simple weight quantization: clamp weights to int8 range then dequantize.
/// This simulates the effect of lower-precision storage.
pub fn quantize_model(model: &Model, bits: u32) -> Model {
    let max_val = (1 << (bits - 1)) as f32 - 1.0;
    let min_val = -max_val - 1.0;

    let mut new_layers = Vec::new();
    for layer in &model.layers {
        let new_layer = match layer {
            Layer::Dense(l) => {
                let (qw, qb) = (quantize_tensor(&l.weights, min_val, max_val),
                                 quantize_tensor(&l.bias, min_val, max_val));
                Layer::Dense(DenseLayer { weights: qw, bias: qb })
            }
            Layer::Conv2D(l) => {
                let qf = quantize_tensor(&l.filters, min_val, max_val);
                let qb = quantize_tensor(&l.bias, min_val, max_val);
                Layer::Conv2D(Conv2DLayer { filters: qf, bias: qb, stride: l.stride, padding: l.padding })
            }
            other => other.clone(),
        };
        new_layers.push(new_layer);
    }
    Model { layers: new_layers, name: format!("{}_q{}", model.name, bits) }
}

fn quantize_tensor(t: &Tensor, min_val: f32, max_val: f32) -> Tensor {
    let abs_max = t.data.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    if abs_max == 0.0 {
        return t.clone();
    }
    let scale = max_val / abs_max;
    let inv_scale = abs_max / max_val;
    let data: Vec<f32> = t.data.iter().map(|&v| {
        let q = (v * scale).round().clamp(min_val, max_val);
        q * inv_scale
    }).collect();
    Tensor { shape: t.shape.clone(), data }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer() {
        let model = Sequential::new("test")
            .dense(4, 3)
            .relu()
            .build();
        let mut engine = InferenceEngine::new(model, Device::CPU);
        let input = Tensor::ones(vec![1, 4]);
        let out = engine.infer(&input);
        assert_eq!(out.shape, vec![1, 3]);
        assert!(engine.stats.latency_ms >= 0.0);
    }

    #[test]
    fn test_batch_infer() {
        let model = Sequential::new("test")
            .dense(3, 2)
            .build();
        let mut engine = InferenceEngine::new(model, Device::CPU);
        let inputs = vec![
            Tensor::ones(vec![1, 3]),
            Tensor::zeros(vec![1, 3]),
        ];
        let outputs = engine.batch_infer(&inputs);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].shape, vec![1, 2]);
        assert_eq!(outputs[1].shape, vec![1, 2]);
    }

    #[test]
    fn test_warm_up() {
        let model = Sequential::new("test").dense(4, 2).build();
        let mut engine = InferenceEngine::new(model, Device::CPU);
        engine.warm_up(vec![1, 4], 5);
        // just verify it doesn't panic
    }

    #[test]
    fn test_quantize_model() {
        let model = Sequential::new("test")
            .dense(4, 3)
            .relu()
            .build();
        let qmodel = quantize_model(&model, 8);
        assert!(qmodel.name.contains("q8"));
        // forward still works
        let input = Tensor::ones(vec![1, 4]);
        let out = qmodel.forward(&input);
        assert_eq!(out.shape, vec![1, 3]);
    }

    #[test]
    fn test_onnx_save_load_roundtrip() {
        let model = Sequential::new("onnx_test")
            .dense(4, 3)
            .relu()
            .dense(3, 2)
            .softmax()
            .build();

        let path = std::env::temp_dir().join("proof_engine_test.onnx");
        let path_str = path.to_str().unwrap();

        OnnxLoader::save_onnx(&model, path_str).unwrap();
        let loaded = OnnxLoader::load_onnx(path_str).unwrap();

        assert_eq!(loaded.layers.len(), model.layers.len());

        // Verify dense weights match
        if let (Layer::Dense(orig), Layer::Dense(loaded_l)) = (&model.layers[0], &loaded.layers[0]) {
            assert_eq!(orig.weights.data, loaded_l.weights.data);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_onnx_load_bad_magic() {
        let path = std::env::temp_dir().join("proof_engine_bad.onnx");
        std::fs::write(&path, b"NOPE1234").unwrap();
        let result = OnnxLoader::load_onnx(path.to_str().unwrap());
        assert!(result.is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_inference_stats() {
        let model = Sequential::new("s").dense(2, 2).build();
        let mut engine = InferenceEngine::new(model, Device::CPU);
        let _ = engine.infer(&Tensor::ones(vec![1, 2]));
        assert!(engine.stats.flops > 0);
        assert!(engine.stats.memory_bytes > 0);
    }
}
