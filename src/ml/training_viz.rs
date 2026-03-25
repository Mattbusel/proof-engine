//! Training visualization: loss landscapes, gradient flow, weight distributions, dashboards.

use glam::{Vec2, Vec4};

/// Statistics for a single training epoch.
#[derive(Debug, Clone)]
pub struct EpochStats {
    pub epoch: usize,
    pub loss: f32,
    pub accuracy: f32,
    pub lr: f32,
    pub grad_norm: f32,
    pub weight_norms: Vec<f32>,
}

impl EpochStats {
    pub fn new(epoch: usize, loss: f32, accuracy: f32, lr: f32) -> Self {
        Self { epoch, loss, accuracy, lr, grad_norm: 0.0, weight_norms: Vec::new() }
    }
}

/// Log of training progress over multiple epochs.
#[derive(Debug, Clone)]
pub struct TrainingLog {
    pub epochs: Vec<EpochStats>,
}

impl TrainingLog {
    pub fn new() -> Self {
        Self { epochs: Vec::new() }
    }

    pub fn push(&mut self, stats: EpochStats) {
        self.epochs.push(stats);
    }

    pub fn len(&self) -> usize {
        self.epochs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.epochs.is_empty()
    }

    pub fn best_loss(&self) -> Option<f32> {
        self.epochs.iter().map(|e| e.loss).fold(None, |acc, v| {
            Some(match acc { None => v, Some(a) => a.min(v) })
        })
    }

    pub fn best_accuracy(&self) -> Option<f32> {
        self.epochs.iter().map(|e| e.accuracy).fold(None, |acc, v| {
            Some(match acc { None => v, Some(a) => a.max(v) })
        })
    }

    /// Return the loss values as a vector for plotting.
    pub fn loss_curve(&self) -> Vec<f32> {
        self.epochs.iter().map(|e| e.loss).collect()
    }

    /// Return the accuracy values as a vector for plotting.
    pub fn accuracy_curve(&self) -> Vec<f32> {
        self.epochs.iter().map(|e| e.accuracy).collect()
    }

    /// Return learning rate schedule.
    pub fn lr_schedule(&self) -> Vec<f32> {
        self.epochs.iter().map(|e| e.lr).collect()
    }

    /// Compute smoothed loss using exponential moving average.
    pub fn smoothed_loss(&self, alpha: f32) -> Vec<f32> {
        let mut smoothed = Vec::with_capacity(self.epochs.len());
        let mut ema = 0.0f32;
        for (i, e) in self.epochs.iter().enumerate() {
            if i == 0 {
                ema = e.loss;
            } else {
                ema = alpha * ema + (1.0 - alpha) * e.loss;
            }
            smoothed.push(ema);
        }
        smoothed
    }
}

// ── Loss Landscape ──────────────────────────────────────────────────────

/// A 2D grid of loss values sampled along two parameter directions.
#[derive(Debug, Clone)]
pub struct LossLandscape {
    /// Loss values in row-major order (height x width).
    pub values: Vec<f32>,
    pub width: usize,
    pub height: usize,
    /// Range of the x-axis parameter.
    pub x_range: (f32, f32),
    /// Range of the y-axis parameter.
    pub y_range: (f32, f32),
}

impl LossLandscape {
    pub fn new(width: usize, height: usize, x_range: (f32, f32), y_range: (f32, f32)) -> Self {
        Self {
            values: vec![0.0; width * height],
            width,
            height,
            x_range,
            y_range,
        }
    }

    pub fn set(&mut self, x: usize, y: usize, val: f32) {
        if x < self.width && y < self.height {
            self.values[y * self.width + x] = val;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.values[y * self.width + x]
        } else {
            0.0
        }
    }

    pub fn min_loss(&self) -> f32 {
        self.values.iter().cloned().fold(f32::INFINITY, f32::min)
    }

    pub fn max_loss(&self) -> f32 {
        self.values.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }

    /// Generate a synthetic loss landscape (for testing/demo).
    /// Uses a sum-of-Gaussians to create a bowl-like landscape.
    pub fn generate_synthetic(width: usize, height: usize) -> Self {
        let mut landscape = Self::new(width, height, (-2.0, 2.0), (-2.0, 2.0));
        for y in 0..height {
            for x in 0..width {
                let px = -2.0 + 4.0 * x as f32 / (width - 1).max(1) as f32;
                let py = -2.0 + 4.0 * y as f32 / (height - 1).max(1) as f32;
                // Rosenbrock-like function
                let val = (1.0 - px).powi(2) + 100.0 * (py - px * px).powi(2);
                landscape.set(x, y, val.ln().max(-5.0));
            }
        }
        landscape
    }
}

/// Render a loss landscape as colored points: (position, loss_value, color).
pub fn render_loss_landscape(landscape: &LossLandscape) -> Vec<(Vec2, f32, Vec4)> {
    let min_loss = landscape.min_loss();
    let max_loss = landscape.max_loss();
    let range = (max_loss - min_loss).max(1e-6);

    let mut points = Vec::with_capacity(landscape.width * landscape.height);
    for y in 0..landscape.height {
        for x in 0..landscape.width {
            let loss = landscape.get(x, y);
            let t = (loss - min_loss) / range; // 0..1

            let px = landscape.x_range.0
                + (landscape.x_range.1 - landscape.x_range.0) * x as f32 / (landscape.width - 1).max(1) as f32;
            let py = landscape.y_range.0
                + (landscape.y_range.1 - landscape.y_range.0) * y as f32 / (landscape.height - 1).max(1) as f32;

            // Color: blue (low loss) -> red (high loss)
            let r = t;
            let g = (1.0 - (2.0 * t - 1.0).abs()).max(0.0);
            let b = 1.0 - t;
            let color = Vec4::new(r, g, b, 1.0);

            points.push((Vec2::new(px, py), loss, color));
        }
    }
    points
}

// ── Gradient Flow Visualization ─────────────────────────────────────────

/// Gradient magnitude per layer, rendered as a horizontal bar chart.
pub struct GradientFlowViz;

impl GradientFlowViz {
    /// Render gradient norms as bar chart data: (layer_index, bar_height, color).
    pub fn render(layer_names: &[String], grad_norms: &[f32]) -> Vec<(usize, f32, Vec4)> {
        let max_norm = grad_norms.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
        layer_names.iter().enumerate().zip(grad_norms).map(|((i, _name), &norm)| {
            let t = norm / max_norm;
            // Green (healthy) -> Yellow (warning) -> Red (vanishing/exploding)
            let color = if t < 0.01 {
                // Vanishing gradient: red
                Vec4::new(1.0, 0.0, 0.0, 1.0)
            } else if t > 0.8 {
                // Potentially exploding: orange
                Vec4::new(1.0, 0.5, 0.0, 1.0)
            } else {
                // Healthy: green
                Vec4::new(0.2, 0.8, 0.2, 1.0)
            };
            (i, t, color)
        }).collect()
    }

    /// Check for vanishing gradients (any layer with norm < threshold).
    pub fn detect_vanishing(grad_norms: &[f32], threshold: f32) -> Vec<usize> {
        grad_norms.iter().enumerate()
            .filter(|(_, &n)| n < threshold)
            .map(|(i, _)| i)
            .collect()
    }

    /// Check for exploding gradients (any layer with norm > threshold).
    pub fn detect_exploding(grad_norms: &[f32], threshold: f32) -> Vec<usize> {
        grad_norms.iter().enumerate()
            .filter(|(_, &n)| n > threshold)
            .map(|(i, _)| i)
            .collect()
    }
}

// ── Weight Distribution Visualization ───────────────────────────────────

/// Render weight histograms per layer.
pub struct WeightDistViz;

impl WeightDistViz {
    /// Compute a histogram of weight values.
    /// Returns (bin_centers, counts) for the given number of bins.
    pub fn histogram(weights: &[f32], num_bins: usize) -> (Vec<f32>, Vec<u32>) {
        if weights.is_empty() || num_bins == 0 {
            return (vec![], vec![]);
        }
        let min_w = weights.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_w = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = (max_w - min_w).max(1e-8);
        let bin_width = range / num_bins as f32;

        let mut counts = vec![0u32; num_bins];
        for &w in weights {
            let bin = ((w - min_w) / bin_width) as usize;
            let bin = bin.min(num_bins - 1);
            counts[bin] += 1;
        }

        let centers: Vec<f32> = (0..num_bins)
            .map(|i| min_w + (i as f32 + 0.5) * bin_width)
            .collect();

        (centers, counts)
    }

    /// Compute statistics of a weight array.
    pub fn stats(weights: &[f32]) -> WeightStats {
        if weights.is_empty() {
            return WeightStats { mean: 0.0, std: 0.0, min: 0.0, max: 0.0, sparsity: 1.0 };
        }
        let n = weights.len() as f32;
        let mean: f32 = weights.iter().sum::<f32>() / n;
        let var: f32 = weights.iter().map(|w| (w - mean) * (w - mean)).sum::<f32>() / n;
        let min = weights.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let zeros = weights.iter().filter(|&&w| w.abs() < 1e-8).count();
        WeightStats { mean, std: var.sqrt(), min, max, sparsity: zeros as f32 / n }
    }
}

#[derive(Debug, Clone)]
pub struct WeightStats {
    pub mean: f32,
    pub std: f32,
    pub min: f32,
    pub max: f32,
    pub sparsity: f32,
}

// ── Activation Map Visualization ────────────────────────────────────────

/// Render intermediate activations as colored grids.
pub struct ActivationMapViz;

impl ActivationMapViz {
    /// Render a 2-D activation map (H, W) as colored cells.
    /// Returns Vec of (position, value, color).
    pub fn render_2d(activations: &[f32], height: usize, width: usize) -> Vec<(Vec2, f32, Vec4)> {
        assert_eq!(activations.len(), height * width);
        let min_a = activations.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_a = activations.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = (max_a - min_a).max(1e-8);

        let mut points = Vec::with_capacity(height * width);
        for y in 0..height {
            for x in 0..width {
                let val = activations[y * width + x];
                let t = (val - min_a) / range;
                // Viridis-like colormap
                let r = (0.267 + 0.004 * t + 1.3 * t * t - 0.6 * t * t * t).clamp(0.0, 1.0);
                let g = (0.004 + 1.0 * t - 0.15 * t * t).clamp(0.0, 1.0);
                let b = (0.329 + 1.4 * t - 1.75 * t * t + 0.5 * t * t * t).clamp(0.0, 1.0);
                points.push((
                    Vec2::new(x as f32, y as f32),
                    val,
                    Vec4::new(r, g, b, 1.0),
                ));
            }
        }
        points
    }

    /// Render multiple channels stacked vertically.
    pub fn render_multichannel(
        activations: &[f32],
        channels: usize,
        height: usize,
        width: usize,
    ) -> Vec<(Vec2, f32, Vec4)> {
        assert_eq!(activations.len(), channels * height * width);
        let mut all_points = Vec::new();
        for c in 0..channels {
            let offset = c * height * width;
            let channel_data = &activations[offset..offset + height * width];
            let mut pts = Self::render_2d(channel_data, height, width);
            // Offset y position by channel
            let y_off = c as f32 * (height as f32 + 1.0);
            for p in &mut pts {
                p.0.y += y_off;
            }
            all_points.extend(pts);
        }
        all_points
    }
}

// ── Training Dashboard ──────────────────────────────────────────────────

/// Composite training dashboard combining multiple visualizations.
pub struct TrainingDashboard {
    pub log: TrainingLog,
    pub layer_names: Vec<String>,
}

impl TrainingDashboard {
    pub fn new(log: TrainingLog, layer_names: Vec<String>) -> Self {
        Self { log, layer_names }
    }

    /// Render the loss curve as a series of 2D points.
    pub fn render_loss_curve(&self) -> Vec<Vec2> {
        let n = self.log.len();
        if n == 0 { return vec![]; }
        let max_loss = self.log.epochs.iter().map(|e| e.loss).fold(0.0f32, f32::max).max(1e-6);
        self.log.epochs.iter().enumerate().map(|(i, e)| {
            Vec2::new(i as f32 / n as f32, e.loss / max_loss)
        }).collect()
    }

    /// Render the accuracy curve as a series of 2D points.
    pub fn render_accuracy_curve(&self) -> Vec<Vec2> {
        let n = self.log.len();
        if n == 0 { return vec![]; }
        self.log.epochs.iter().enumerate().map(|(i, e)| {
            Vec2::new(i as f32 / n as f32, e.accuracy)
        }).collect()
    }

    /// Render the learning rate schedule.
    pub fn render_lr_curve(&self) -> Vec<Vec2> {
        let n = self.log.len();
        if n == 0 { return vec![]; }
        let max_lr = self.log.epochs.iter().map(|e| e.lr).fold(0.0f32, f32::max).max(1e-8);
        self.log.epochs.iter().enumerate().map(|(i, e)| {
            Vec2::new(i as f32 / n as f32, e.lr / max_lr)
        }).collect()
    }

    /// Render gradient flow for the latest epoch.
    pub fn render_gradient_flow(&self) -> Vec<(usize, f32, Vec4)> {
        if let Some(last) = self.log.epochs.last() {
            if last.weight_norms.len() == self.layer_names.len() {
                // Use weight norms as proxy for gradient norms if grad_norm not per-layer
                return GradientFlowViz::render(&self.layer_names, &last.weight_norms);
            }
        }
        vec![]
    }

    /// Summary string for the current training state.
    pub fn summary(&self) -> String {
        let n = self.log.len();
        if n == 0 { return "No training data".to_string(); }
        let last = &self.log.epochs[n - 1];
        let best_loss = self.log.best_loss().unwrap_or(0.0);
        let best_acc = self.log.best_accuracy().unwrap_or(0.0);
        format!(
            "Epoch {}/{}: loss={:.4} acc={:.2}% lr={:.6} | best_loss={:.4} best_acc={:.2}%",
            last.epoch, n, last.loss, last.accuracy * 100.0, last.lr,
            best_loss, best_acc * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_log() {
        let mut log = TrainingLog::new();
        assert!(log.is_empty());
        log.push(EpochStats::new(0, 2.5, 0.1, 0.01));
        log.push(EpochStats::new(1, 1.5, 0.5, 0.01));
        log.push(EpochStats::new(2, 0.8, 0.8, 0.005));
        assert_eq!(log.len(), 3);
        assert!((log.best_loss().unwrap() - 0.8).abs() < 1e-5);
        assert!((log.best_accuracy().unwrap() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_loss_curve() {
        let mut log = TrainingLog::new();
        log.push(EpochStats::new(0, 2.0, 0.1, 0.01));
        log.push(EpochStats::new(1, 1.0, 0.5, 0.01));
        let curve = log.loss_curve();
        assert_eq!(curve, vec![2.0, 1.0]);
    }

    #[test]
    fn test_smoothed_loss() {
        let mut log = TrainingLog::new();
        for i in 0..10 {
            log.push(EpochStats::new(i, 10.0 - i as f32, 0.0, 0.01));
        }
        let smoothed = log.smoothed_loss(0.9);
        assert_eq!(smoothed.len(), 10);
        // Smoothed should lag behind actual
        assert!(smoothed[9] > log.epochs[9].loss);
    }

    #[test]
    fn test_loss_landscape() {
        let landscape = LossLandscape::generate_synthetic(10, 10);
        assert_eq!(landscape.values.len(), 100);
        assert!(landscape.min_loss() < landscape.max_loss());
    }

    #[test]
    fn test_render_loss_landscape() {
        let landscape = LossLandscape::generate_synthetic(5, 5);
        let points = render_loss_landscape(&landscape);
        assert_eq!(points.len(), 25);
        for (pos, loss, color) in &points {
            assert!(pos.x >= -2.0 && pos.x <= 2.0);
            assert!(pos.y >= -2.0 && pos.y <= 2.0);
            assert!(color.w == 1.0); // full alpha
            let _ = loss;
        }
    }

    #[test]
    fn test_gradient_flow_viz() {
        let names = vec!["dense_0".into(), "dense_1".into(), "dense_2".into()];
        let norms = vec![0.5, 0.001, 0.3];
        let bars = GradientFlowViz::render(&names, &norms);
        assert_eq!(bars.len(), 3);
    }

    #[test]
    fn test_detect_vanishing() {
        let norms = vec![0.5, 0.001, 0.0001, 0.3];
        let vanishing = GradientFlowViz::detect_vanishing(&norms, 0.01);
        assert_eq!(vanishing, vec![1, 2]);
    }

    #[test]
    fn test_detect_exploding() {
        let norms = vec![0.5, 100.0, 0.3, 200.0];
        let exploding = GradientFlowViz::detect_exploding(&norms, 50.0);
        assert_eq!(exploding, vec![1, 3]);
    }

    #[test]
    fn test_weight_histogram() {
        let weights = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let (centers, counts) = WeightDistViz::histogram(&weights, 5);
        assert_eq!(centers.len(), 5);
        assert_eq!(counts.len(), 5);
        let total: u32 = counts.iter().sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn test_weight_stats() {
        let weights = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = WeightDistViz::stats(&weights);
        assert!((stats.mean - 3.0).abs() < 1e-5);
        assert!(stats.std > 0.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.sparsity, 0.0);
    }

    #[test]
    fn test_activation_map_2d() {
        let acts = vec![0.0, 0.5, 1.0, 0.25, 0.75, 0.5, 0.1, 0.9, 0.4];
        let points = ActivationMapViz::render_2d(&acts, 3, 3);
        assert_eq!(points.len(), 9);
    }

    #[test]
    fn test_activation_map_multichannel() {
        let acts = vec![0.0; 2 * 3 * 3]; // 2 channels, 3x3
        let points = ActivationMapViz::render_multichannel(&acts, 2, 3, 3);
        assert_eq!(points.len(), 18);
    }

    #[test]
    fn test_training_dashboard() {
        let mut log = TrainingLog::new();
        log.push(EpochStats { epoch: 0, loss: 2.0, accuracy: 0.1, lr: 0.01, grad_norm: 0.5, weight_norms: vec![0.5, 0.3] });
        log.push(EpochStats { epoch: 1, loss: 1.0, accuracy: 0.5, lr: 0.01, grad_norm: 0.4, weight_norms: vec![0.4, 0.3] });

        let dashboard = TrainingDashboard::new(log, vec!["dense_0".into(), "dense_1".into()]);
        let loss_pts = dashboard.render_loss_curve();
        assert_eq!(loss_pts.len(), 2);
        let acc_pts = dashboard.render_accuracy_curve();
        assert_eq!(acc_pts.len(), 2);
        let summary = dashboard.summary();
        assert!(summary.contains("Epoch"));
        assert!(summary.contains("loss="));
    }

    #[test]
    fn test_dashboard_gradient_flow() {
        let mut log = TrainingLog::new();
        log.push(EpochStats {
            epoch: 0, loss: 1.0, accuracy: 0.5, lr: 0.01, grad_norm: 0.5,
            weight_norms: vec![0.5, 0.3, 0.1],
        });
        let names = vec!["a".into(), "b".into(), "c".into()];
        let dashboard = TrainingDashboard::new(log, names);
        let flow = dashboard.render_gradient_flow();
        assert_eq!(flow.len(), 3);
    }
}
