//! N-dimensional tensor operations for ML workloads.

use std::ops::Range;

/// An N-dimensional tensor stored in row-major order.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl Tensor {
    // ── helpers ──────────────────────────────────────────────────────────

    /// Total number of elements implied by a shape.
    fn numel(shape: &[usize]) -> usize {
        shape.iter().product()
    }

    /// Compute strides for row-major layout.
    fn strides(shape: &[usize]) -> Vec<usize> {
        let mut s = vec![1usize; shape.len()];
        for i in (0..shape.len().saturating_sub(1)).rev() {
            s[i] = s[i + 1] * shape[i + 1];
        }
        s
    }

    /// Flat index from multi-dimensional indices.
    fn flat_index(&self, indices: &[usize]) -> usize {
        assert_eq!(indices.len(), self.shape.len(), "index rank mismatch");
        let strides = Self::strides(&self.shape);
        indices.iter().zip(strides.iter()).map(|(i, s)| i * s).sum()
    }

    // ── creation ────────────────────────────────────────────────────────

    pub fn zeros(shape: Vec<usize>) -> Self {
        let n = Self::numel(&shape);
        Self { shape, data: vec![0.0; n] }
    }

    pub fn ones(shape: Vec<usize>) -> Self {
        let n = Self::numel(&shape);
        Self { shape, data: vec![1.0; n] }
    }

    /// Pseudo-random tensor using a simple xorshift seeded from `rng`.
    pub fn rand(shape: Vec<usize>, rng: u64) -> Self {
        let n = Self::numel(&shape);
        let mut data = Vec::with_capacity(n);
        let mut state = rng.wrapping_add(1); // avoid zero
        for _ in 0..n {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            // map to 0..1
            data.push((state as u32 as f32) / (u32::MAX as f32));
        }
        Self { shape, data }
    }

    pub fn from_vec(data: Vec<f32>, shape: Vec<usize>) -> Self {
        assert_eq!(data.len(), Self::numel(&shape), "data length / shape mismatch");
        Self { shape, data }
    }

    /// Scalar tensor.
    pub fn scalar(v: f32) -> Self {
        Self { shape: vec![], data: vec![v] }
    }

    // ── indexing ─────────────────────────────────────────────────────────

    pub fn get(&self, indices: &[usize]) -> f32 {
        self.data[self.flat_index(indices)]
    }

    pub fn set(&mut self, indices: &[usize], val: f32) {
        let idx = self.flat_index(indices);
        self.data[idx] = val;
    }

    /// Slice along each axis with the given ranges. Produces a new tensor
    /// whose shape matches the range extents.
    pub fn slice(&self, ranges: &[Range<usize>]) -> Tensor {
        assert_eq!(ranges.len(), self.shape.len());
        let new_shape: Vec<usize> = ranges.iter().map(|r| r.end - r.start).collect();
        let n = Self::numel(&new_shape);
        let mut data = Vec::with_capacity(n);
        let strides = Self::strides(&self.shape);
        // recursive flattening via iterative approach
        Self::slice_recursive(&self.data, &strides, ranges, 0, 0, &mut data);
        Tensor { shape: new_shape, data }
    }

    fn slice_recursive(
        src: &[f32],
        strides: &[usize],
        ranges: &[Range<usize>],
        dim: usize,
        base: usize,
        out: &mut Vec<f32>,
    ) {
        if dim == ranges.len() {
            out.push(src[base]);
            return;
        }
        for i in ranges[dim].clone() {
            Self::slice_recursive(src, strides, ranges, dim + 1, base + i * strides[dim], out);
        }
    }

    // ── element-wise math ───────────────────────────────────────────────

    pub fn add(&self, other: &Tensor) -> Tensor {
        assert_eq!(self.shape, other.shape, "shape mismatch for add");
        let data: Vec<f32> = self.data.iter().zip(&other.data).map(|(a, b)| a + b).collect();
        Tensor { shape: self.shape.clone(), data }
    }

    pub fn sub(&self, other: &Tensor) -> Tensor {
        assert_eq!(self.shape, other.shape, "shape mismatch for sub");
        let data: Vec<f32> = self.data.iter().zip(&other.data).map(|(a, b)| a - b).collect();
        Tensor { shape: self.shape.clone(), data }
    }

    pub fn mul(&self, other: &Tensor) -> Tensor {
        assert_eq!(self.shape, other.shape, "shape mismatch for mul");
        let data: Vec<f32> = self.data.iter().zip(&other.data).map(|(a, b)| a * b).collect();
        Tensor { shape: self.shape.clone(), data }
    }

    pub fn scale(&self, s: f32) -> Tensor {
        Tensor {
            shape: self.shape.clone(),
            data: self.data.iter().map(|v| v * s).collect(),
        }
    }

    /// 2-D matrix multiply: (M, K) x (K, N) -> (M, N).
    pub fn matmul(a: &Tensor, b: &Tensor) -> Tensor {
        assert_eq!(a.shape.len(), 2, "matmul requires 2-D tensors");
        assert_eq!(b.shape.len(), 2, "matmul requires 2-D tensors");
        let m = a.shape[0];
        let k = a.shape[1];
        assert_eq!(b.shape[0], k, "inner dimensions must match");
        let n = b.shape[1];
        let mut data = vec![0.0f32; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut s = 0.0f32;
                for p in 0..k {
                    s += a.data[i * k + p] * b.data[p * n + j];
                }
                data[i * n + j] = s;
            }
        }
        Tensor { shape: vec![m, n], data }
    }

    /// Transpose the last two dimensions. For 2-D tensors this is the
    /// standard matrix transpose.
    pub fn transpose(&self) -> Tensor {
        assert!(self.shape.len() >= 2, "transpose needs rank >= 2");
        let ndim = self.shape.len();
        let rows = self.shape[ndim - 2];
        let cols = self.shape[ndim - 1];
        let batch: usize = self.shape[..ndim - 2].iter().product();
        let mut new_shape = self.shape.clone();
        new_shape[ndim - 2] = cols;
        new_shape[ndim - 1] = rows;
        let mat_size = rows * cols;
        let mut data = vec![0.0f32; self.data.len()];
        for b in 0..batch {
            let base = b * mat_size;
            for r in 0..rows {
                for c in 0..cols {
                    data[base + c * rows + r] = self.data[base + r * cols + c];
                }
            }
        }
        Tensor { shape: new_shape, data }
    }

    // ── reductions ──────────────────────────────────────────────────────

    pub fn sum(&self) -> f32 {
        self.data.iter().sum()
    }

    pub fn mean(&self) -> f32 {
        self.sum() / self.data.len() as f32
    }

    pub fn max(&self) -> f32 {
        self.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }

    pub fn min(&self) -> f32 {
        self.data.iter().cloned().fold(f32::INFINITY, f32::min)
    }

    /// Argmax along a given axis, returning a tensor with that axis removed.
    pub fn argmax(&self, axis: usize) -> Tensor {
        assert!(axis < self.shape.len());
        let axis_len = self.shape[axis];
        let mut new_shape: Vec<usize> = self.shape.clone();
        new_shape.remove(axis);
        if new_shape.is_empty() {
            new_shape.push(1);
        }
        let outer: usize = self.shape[..axis].iter().product();
        let inner: usize = self.shape[axis + 1..].iter().product();
        let mut data = Vec::with_capacity(outer * inner);
        for o in 0..outer {
            for i in 0..inner {
                let mut best_idx = 0usize;
                let mut best_val = f32::NEG_INFINITY;
                for a in 0..axis_len {
                    let flat = o * axis_len * inner + a * inner + i;
                    if self.data[flat] > best_val {
                        best_val = self.data[flat];
                        best_idx = a;
                    }
                }
                data.push(best_idx as f32);
            }
        }
        Tensor { shape: new_shape, data }
    }

    // ── reshaping ───────────────────────────────────────────────────────

    pub fn reshape(&self, new_shape: Vec<usize>) -> Tensor {
        assert_eq!(Self::numel(&new_shape), self.data.len(), "reshape size mismatch");
        Tensor { shape: new_shape, data: self.data.clone() }
    }

    pub fn flatten(&self) -> Tensor {
        Tensor { shape: vec![self.data.len()], data: self.data.clone() }
    }

    /// Remove all size-1 dimensions.
    pub fn squeeze(&self) -> Tensor {
        let new_shape: Vec<usize> = self.shape.iter().copied().filter(|&d| d != 1).collect();
        let new_shape = if new_shape.is_empty() { vec![1] } else { new_shape };
        Tensor { shape: new_shape, data: self.data.clone() }
    }

    /// Insert a size-1 dimension at `dim`.
    pub fn unsqueeze(&self, dim: usize) -> Tensor {
        let mut new_shape = self.shape.clone();
        new_shape.insert(dim, 1);
        Tensor { shape: new_shape, data: self.data.clone() }
    }

    // ── broadcasting ────────────────────────────────────────────────────

    /// Broadcast this tensor to the target shape, repeating data as needed.
    pub fn broadcast_to(&self, target: &[usize]) -> Tensor {
        assert!(target.len() >= self.shape.len());
        // left-pad shape with 1s
        let pad = target.len() - self.shape.len();
        let mut src_shape: Vec<usize> = vec![1; pad];
        src_shape.extend_from_slice(&self.shape);

        for (s, t) in src_shape.iter().zip(target.iter()) {
            assert!(*s == 1 || *s == *t, "cannot broadcast {src_shape:?} to {target:?}");
        }

        let n = Self::numel(target);
        let src_strides = Self::strides(&src_shape);
        let dst_strides = Self::strides(target);
        let mut data = vec![0.0f32; n];
        for flat in 0..n {
            let mut src_flat = 0usize;
            let mut rem = flat;
            for d in 0..target.len() {
                let coord = rem / dst_strides[d];
                rem %= dst_strides[d];
                let src_coord = if src_shape[d] == 1 { 0 } else { coord };
                src_flat += src_coord * src_strides[d];
            }
            data[flat] = self.data[src_flat];
        }
        Tensor { shape: target.to_vec(), data }
    }

    // ── activation functions ────────────────────────────────────────────

    pub fn relu(&self) -> Tensor {
        Tensor {
            shape: self.shape.clone(),
            data: self.data.iter().map(|&v| v.max(0.0)).collect(),
        }
    }

    pub fn sigmoid(&self) -> Tensor {
        Tensor {
            shape: self.shape.clone(),
            data: self.data.iter().map(|&v| 1.0 / (1.0 + (-v).exp())).collect(),
        }
    }

    pub fn tanh_act(&self) -> Tensor {
        Tensor {
            shape: self.shape.clone(),
            data: self.data.iter().map(|&v| v.tanh()).collect(),
        }
    }

    /// Softmax along `axis`.
    pub fn softmax(&self, axis: usize) -> Tensor {
        assert!(axis < self.shape.len());
        let axis_len = self.shape[axis];
        let outer: usize = self.shape[..axis].iter().product();
        let inner: usize = self.shape[axis + 1..].iter().product();
        let mut data = self.data.clone();
        for o in 0..outer {
            for i in 0..inner {
                // find max for numerical stability
                let mut mx = f32::NEG_INFINITY;
                for a in 0..axis_len {
                    let idx = o * axis_len * inner + a * inner + i;
                    mx = mx.max(data[idx]);
                }
                let mut sum = 0.0f32;
                for a in 0..axis_len {
                    let idx = o * axis_len * inner + a * inner + i;
                    let e = (data[idx] - mx).exp();
                    data[idx] = e;
                    sum += e;
                }
                for a in 0..axis_len {
                    let idx = o * axis_len * inner + a * inner + i;
                    data[idx] /= sum;
                }
            }
        }
        Tensor { shape: self.shape.clone(), data }
    }

    /// GELU activation: x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
    pub fn gelu(&self) -> Tensor {
        let sqrt_2_over_pi = (2.0f32 / std::f32::consts::PI).sqrt();
        Tensor {
            shape: self.shape.clone(),
            data: self.data.iter().map(|&x| {
                let inner = sqrt_2_over_pi * (x + 0.044715 * x * x * x);
                0.5 * x * (1.0 + inner.tanh())
            }).collect(),
        }
    }

    // ── convolution ─────────────────────────────────────────────────────

    /// 2-D convolution. Input shape: (C_in, H, W). Kernel shape: (C_out, C_in, kH, kW).
    /// Returns shape (C_out, H_out, W_out).
    pub fn conv2d(&self, kernel: &Tensor, stride: usize, padding: usize) -> Tensor {
        assert_eq!(self.shape.len(), 3, "conv2d input must be (C, H, W)");
        assert_eq!(kernel.shape.len(), 4, "conv2d kernel must be (C_out, C_in, kH, kW)");
        let c_in = self.shape[0];
        let h = self.shape[1];
        let w = self.shape[2];
        let c_out = kernel.shape[0];
        assert_eq!(kernel.shape[1], c_in);
        let kh = kernel.shape[2];
        let kw = kernel.shape[3];
        let h_out = (h + 2 * padding - kh) / stride + 1;
        let w_out = (w + 2 * padding - kw) / stride + 1;

        let mut out = vec![0.0f32; c_out * h_out * w_out];
        for co in 0..c_out {
            for oh in 0..h_out {
                for ow in 0..w_out {
                    let mut val = 0.0f32;
                    for ci in 0..c_in {
                        for fh in 0..kh {
                            for fw in 0..kw {
                                let ih = oh * stride + fh;
                                let iw = ow * stride + fw;
                                let ih = ih as isize - padding as isize;
                                let iw = iw as isize - padding as isize;
                                if ih >= 0 && ih < h as isize && iw >= 0 && iw < w as isize {
                                    let ih = ih as usize;
                                    let iw = iw as usize;
                                    let in_idx = ci * h * w + ih * w + iw;
                                    let k_idx = co * c_in * kh * kw + ci * kh * kw + fh * kw + fw;
                                    val += self.data[in_idx] * kernel.data[k_idx];
                                }
                            }
                        }
                    }
                    out[co * h_out * w_out + oh * w_out + ow] = val;
                }
            }
        }
        Tensor { shape: vec![c_out, h_out, w_out], data: out }
    }

    // ── pooling ─────────────────────────────────────────────────────────

    /// Max pooling 2-D. Input shape: (C, H, W).
    pub fn max_pool2d(&self, kernel_size: usize, stride: usize) -> Tensor {
        assert_eq!(self.shape.len(), 3);
        let c = self.shape[0];
        let h = self.shape[1];
        let w = self.shape[2];
        let h_out = (h - kernel_size) / stride + 1;
        let w_out = (w - kernel_size) / stride + 1;
        let mut out = vec![f32::NEG_INFINITY; c * h_out * w_out];
        for ch in 0..c {
            for oh in 0..h_out {
                for ow in 0..w_out {
                    let mut mx = f32::NEG_INFINITY;
                    for kh in 0..kernel_size {
                        for kw in 0..kernel_size {
                            let ih = oh * stride + kh;
                            let iw = ow * stride + kw;
                            mx = mx.max(self.data[ch * h * w + ih * w + iw]);
                        }
                    }
                    out[ch * h_out * w_out + oh * w_out + ow] = mx;
                }
            }
        }
        Tensor { shape: vec![c, h_out, w_out], data: out }
    }

    /// Average pooling 2-D. Input shape: (C, H, W).
    pub fn avg_pool2d(&self, kernel_size: usize, stride: usize) -> Tensor {
        assert_eq!(self.shape.len(), 3);
        let c = self.shape[0];
        let h = self.shape[1];
        let w = self.shape[2];
        let h_out = (h - kernel_size) / stride + 1;
        let w_out = (w - kernel_size) / stride + 1;
        let area = (kernel_size * kernel_size) as f32;
        let mut out = vec![0.0f32; c * h_out * w_out];
        for ch in 0..c {
            for oh in 0..h_out {
                for ow in 0..w_out {
                    let mut s = 0.0f32;
                    for kh in 0..kernel_size {
                        for kw in 0..kernel_size {
                            let ih = oh * stride + kh;
                            let iw = ow * stride + kw;
                            s += self.data[ch * h * w + ih * w + iw];
                        }
                    }
                    out[ch * h_out * w_out + oh * w_out + ow] = s / area;
                }
            }
        }
        Tensor { shape: vec![c, h_out, w_out], data: out }
    }

    // ── normalization ───────────────────────────────────────────────────

    /// Batch normalization: y = gamma * (x - mean) / sqrt(var + eps) + beta.
    /// All parameter tensors must have the same total length as `self`.
    pub fn batch_norm(&self, mean: &Tensor, var: &Tensor, gamma: &Tensor, beta: &Tensor, eps: f32) -> Tensor {
        assert_eq!(self.data.len(), mean.data.len());
        let data: Vec<f32> = self.data.iter().enumerate().map(|(i, &x)| {
            let m = mean.data[i];
            let v = var.data[i];
            let g = gamma.data[i];
            let b = beta.data[i];
            g * (x - m) / (v + eps).sqrt() + b
        }).collect();
        Tensor { shape: self.shape.clone(), data }
    }

    /// Layer normalization along the last `n` dimensions starting from `axis`.
    pub fn layer_norm(&self, axis: usize, eps: f32) -> Tensor {
        assert!(axis < self.shape.len());
        let outer: usize = self.shape[..axis].iter().product();
        let inner: usize = self.shape[axis..].iter().product();
        let mut data = self.data.clone();
        for o in 0..outer {
            let start = o * inner;
            let end = start + inner;
            let slice = &data[start..end];
            let mean: f32 = slice.iter().sum::<f32>() / inner as f32;
            let var: f32 = slice.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / inner as f32;
            let inv_std = 1.0 / (var + eps).sqrt();
            for i in start..end {
                data[i] = (data[i] - mean) * inv_std;
            }
        }
        Tensor { shape: self.shape.clone(), data }
    }

    // ── dropout ─────────────────────────────────────────────────────────

    /// Dropout: randomly zero elements with probability `p` during training.
    pub fn dropout(&self, p: f32, rng: u64, training: bool) -> Tensor {
        if !training || p == 0.0 {
            return self.clone();
        }
        let scale = 1.0 / (1.0 - p);
        let mut state = rng.wrapping_add(1);
        let data: Vec<f32> = self.data.iter().map(|&v| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let r = (state as u32 as f32) / (u32::MAX as f32);
            if r < p { 0.0 } else { v * scale }
        }).collect();
        Tensor { shape: self.shape.clone(), data }
    }

    // ── concatenation / stacking ────────────────────────────────────────

    /// Concatenate tensors along an axis.
    pub fn concat(tensors: &[Tensor], axis: usize) -> Tensor {
        assert!(!tensors.is_empty());
        let ndim = tensors[0].shape.len();
        assert!(axis < ndim);
        // verify all shapes match except along `axis`
        for t in &tensors[1..] {
            assert_eq!(t.shape.len(), ndim);
            for d in 0..ndim {
                if d != axis {
                    assert_eq!(t.shape[d], tensors[0].shape[d]);
                }
            }
        }
        let mut new_shape = tensors[0].shape.clone();
        new_shape[axis] = tensors.iter().map(|t| t.shape[axis]).sum();

        let outer: usize = new_shape[..axis].iter().product();
        let inner: usize = new_shape[axis + 1..].iter().product();
        let total = Self::numel(&new_shape);
        let mut data = Vec::with_capacity(total);

        for o in 0..outer {
            for t in tensors {
                let t_axis = t.shape[axis];
                let t_inner: usize = t.shape[axis + 1..].iter().product();
                for a in 0..t_axis {
                    for i in 0..inner {
                        let idx = o * t_axis * t_inner + a * t_inner + i;
                        data.push(t.data[idx]);
                    }
                }
            }
        }
        Tensor { shape: new_shape, data }
    }

    /// Stack tensors along a new axis.
    pub fn stack(tensors: &[Tensor], axis: usize) -> Tensor {
        assert!(!tensors.is_empty());
        // unsqueeze each tensor at `axis`, then concat
        let unsqueezed: Vec<Tensor> = tensors.iter().map(|t| t.unsqueeze(axis)).collect();
        Self::concat(&unsqueezed, axis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let z = Tensor::zeros(vec![2, 3]);
        assert_eq!(z.data.len(), 6);
        assert!(z.data.iter().all(|&v| v == 0.0));

        let o = Tensor::ones(vec![3, 2]);
        assert!(o.data.iter().all(|&v| v == 1.0));
    }

    #[test]
    fn test_indexing() {
        let mut t = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);
        assert_eq!(t.get(&[0, 0]), 1.0);
        assert_eq!(t.get(&[1, 2]), 6.0);
        t.set(&[0, 1], 99.0);
        assert_eq!(t.get(&[0, 1]), 99.0);
    }

    #[test]
    fn test_matmul() {
        // [[1,2],[3,4]] x [[5,6],[7,8]] = [[19,22],[43,50]]
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        let b = Tensor::from_vec(vec![5.0, 6.0, 7.0, 8.0], vec![2, 2]);
        let c = Tensor::matmul(&a, &b);
        assert_eq!(c.shape, vec![2, 2]);
        assert_eq!(c.get(&[0, 0]), 19.0);
        assert_eq!(c.get(&[0, 1]), 22.0);
        assert_eq!(c.get(&[1, 0]), 43.0);
        assert_eq!(c.get(&[1, 1]), 50.0);
    }

    #[test]
    fn test_matmul_non_square() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);
        let b = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![3, 2]);
        let c = Tensor::matmul(&a, &b);
        assert_eq!(c.shape, vec![2, 2]);
        // [1*1+2*3+3*5, 1*2+2*4+3*6] = [22, 28]
        assert_eq!(c.get(&[0, 0]), 22.0);
        assert_eq!(c.get(&[0, 1]), 28.0);
    }

    #[test]
    fn test_transpose() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]);
        let at = a.transpose();
        assert_eq!(at.shape, vec![3, 2]);
        assert_eq!(at.get(&[0, 0]), 1.0);
        assert_eq!(at.get(&[0, 1]), 4.0);
        assert_eq!(at.get(&[2, 0]), 3.0);
        assert_eq!(at.get(&[2, 1]), 6.0);
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let t = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![1, 4]);
        let s = t.softmax(1);
        let total: f32 = s.data.iter().sum();
        assert!((total - 1.0).abs() < 1e-5, "softmax sum = {total}");
        // all positive
        assert!(s.data.iter().all(|&v| v > 0.0));
    }

    #[test]
    fn test_relu_zeros_negatives() {
        let t = Tensor::from_vec(vec![-3.0, -1.0, 0.0, 1.0, 5.0], vec![5]);
        let r = t.relu();
        assert_eq!(r.data, vec![0.0, 0.0, 0.0, 1.0, 5.0]);
    }

    #[test]
    fn test_conv2d() {
        // 1 channel, 4x4 input, 1 filter 1x1x3x3, stride 1, no padding -> 2x2
        let input = Tensor::ones(vec![1, 4, 4]);
        let kernel = Tensor::ones(vec![1, 1, 3, 3]);
        let out = input.conv2d(&kernel, 1, 0);
        assert_eq!(out.shape, vec![1, 2, 2]);
        // each output element = sum of 3x3 ones = 9
        assert_eq!(out.data, vec![9.0, 9.0, 9.0, 9.0]);
    }

    #[test]
    fn test_conv2d_with_padding() {
        let input = Tensor::ones(vec![1, 3, 3]);
        let kernel = Tensor::ones(vec![1, 1, 3, 3]);
        let out = input.conv2d(&kernel, 1, 1);
        assert_eq!(out.shape, vec![1, 3, 3]);
        // center: 9, corners: 4, edges: 6
        assert_eq!(out.get(&[0, 1, 1]), 9.0);
        assert_eq!(out.get(&[0, 0, 0]), 4.0);
        assert_eq!(out.get(&[0, 0, 1]), 6.0);
    }

    #[test]
    fn test_pooling() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0];
        let t = Tensor::from_vec(data, vec![1, 4, 4]);
        let mp = t.max_pool2d(2, 2);
        assert_eq!(mp.shape, vec![1, 2, 2]);
        assert_eq!(mp.data, vec![6.0, 8.0, 14.0, 16.0]);

        let ap = t.avg_pool2d(2, 2);
        assert_eq!(ap.shape, vec![1, 2, 2]);
        assert_eq!(ap.data, vec![3.5, 5.5, 11.5, 13.5]);
    }

    #[test]
    fn test_reshape_flatten() {
        let t = Tensor::ones(vec![2, 3, 4]);
        let r = t.reshape(vec![6, 4]);
        assert_eq!(r.shape, vec![6, 4]);
        assert_eq!(r.data.len(), 24);
        let f = t.flatten();
        assert_eq!(f.shape, vec![24]);
    }

    #[test]
    fn test_squeeze_unsqueeze() {
        let t = Tensor::ones(vec![1, 3, 1, 4]);
        let s = t.squeeze();
        assert_eq!(s.shape, vec![3, 4]);
        let u = s.unsqueeze(0);
        assert_eq!(u.shape, vec![1, 3, 4]);
    }

    #[test]
    fn test_broadcast() {
        let t = Tensor::from_vec(vec![1.0, 2.0, 3.0], vec![1, 3]);
        let b = t.broadcast_to(&[2, 3]);
        assert_eq!(b.shape, vec![2, 3]);
        assert_eq!(b.data, vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_sigmoid() {
        let t = Tensor::from_vec(vec![0.0], vec![1]);
        let s = t.sigmoid();
        assert!((s.data[0] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_gelu() {
        let t = Tensor::from_vec(vec![0.0, 1.0, -1.0], vec![3]);
        let g = t.gelu();
        assert!((g.data[0]).abs() < 1e-5); // gelu(0) = 0
        assert!(g.data[1] > 0.8); // gelu(1) ~ 0.841
        assert!(g.data[2] < 0.0); // gelu(-1) ~ -0.159
    }

    #[test]
    fn test_layer_norm() {
        let t = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![1, 4]);
        let ln = t.layer_norm(1, 1e-5);
        // mean should be ~0
        let mean: f32 = ln.data.iter().sum::<f32>() / 4.0;
        assert!(mean.abs() < 1e-4);
    }

    #[test]
    fn test_concat() {
        let a = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        let b = Tensor::from_vec(vec![5.0, 6.0, 7.0, 8.0], vec![2, 2]);
        let c = Tensor::concat(&[a, b], 0);
        assert_eq!(c.shape, vec![4, 2]);
        assert_eq!(c.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_stack() {
        let a = Tensor::from_vec(vec![1.0, 2.0], vec![2]);
        let b = Tensor::from_vec(vec![3.0, 4.0], vec![2]);
        let s = Tensor::stack(&[a, b], 0);
        assert_eq!(s.shape, vec![2, 2]);
        assert_eq!(s.data, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_slice() {
        let t = Tensor::from_vec(
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            vec![3, 3],
        );
        let s = t.slice(&[0..2, 1..3]);
        assert_eq!(s.shape, vec![2, 2]);
        assert_eq!(s.data, vec![2.0, 3.0, 5.0, 6.0]);
    }

    #[test]
    fn test_dropout() {
        let t = Tensor::ones(vec![100]);
        let d = t.dropout(0.5, 42, true);
        let zeros = d.data.iter().filter(|&&v| v == 0.0).count();
        // with p=0.5 we expect roughly 50 zeros (allow wide margin)
        assert!(zeros > 10 && zeros < 90);
        // non-training should pass through
        let d2 = t.dropout(0.5, 42, false);
        assert_eq!(d2.data, t.data);
    }

    #[test]
    fn test_argmax() {
        let t = Tensor::from_vec(vec![1.0, 5.0, 3.0, 9.0, 2.0, 4.0], vec![2, 3]);
        let am = t.argmax(1);
        assert_eq!(am.shape, vec![2]);
        assert_eq!(am.data, vec![1.0, 0.0]); // argmax of [1,5,3]=1, [9,2,4]=0
    }
}
