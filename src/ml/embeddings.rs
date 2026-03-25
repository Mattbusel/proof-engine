//! Embedding space visualization: PCA, t-SNE, UMAP, nearest neighbors.

use glam::Vec2;

/// A collection of embedding vectors with optional labels.
#[derive(Debug, Clone)]
pub struct EmbeddingSpace {
    pub vectors: Vec<Vec<f32>>,
    pub labels: Vec<String>,
    pub dim: usize,
}

impl EmbeddingSpace {
    pub fn new(dim: usize) -> Self {
        Self { vectors: Vec::new(), labels: Vec::new(), dim }
    }

    pub fn add(&mut self, vector: Vec<f32>, label: String) {
        assert_eq!(vector.len(), self.dim);
        self.vectors.push(vector);
        self.labels.push(label);
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-12 { 0.0 } else { dot / denom }
}

/// Find the k nearest neighbors of a query vector, returning (index, similarity).
pub fn nearest_neighbors(space: &EmbeddingSpace, query: &[f32], k: usize) -> Vec<(usize, f32)> {
    assert_eq!(query.len(), space.dim);
    let mut scored: Vec<(usize, f32)> = space.vectors.iter().enumerate()
        .map(|(i, v)| (i, cosine_similarity(query, v)))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored
}

// ── PCA ─────────────────────────────────────────────────────────────────

/// PCA dimensionality reduction via power iteration for top-k eigenvectors.
pub fn pca(vectors: &[Vec<f32>], target_dim: usize) -> Vec<Vec<f32>> {
    if vectors.is_empty() { return vec![]; }
    let n = vectors.len();
    let d = vectors[0].len();
    let target_dim = target_dim.min(d);

    // Compute mean
    let mut mean = vec![0.0f32; d];
    for v in vectors {
        for (i, &val) in v.iter().enumerate() {
            mean[i] += val;
        }
    }
    for m in &mut mean { *m /= n as f32; }

    // Center data
    let centered: Vec<Vec<f32>> = vectors.iter()
        .map(|v| v.iter().zip(&mean).map(|(a, b)| a - b).collect())
        .collect();

    // Compute covariance matrix (d x d) — for efficiency with large d,
    // we compute X^T X / n which is d x d.
    let mut cov = vec![vec![0.0f32; d]; d];
    for v in &centered {
        for i in 0..d {
            for j in i..d {
                let val = v[i] * v[j];
                cov[i][j] += val;
                if i != j { cov[j][i] += val; }
            }
        }
    }
    let nf = n as f32;
    for row in &mut cov {
        for val in row.iter_mut() { *val /= nf; }
    }

    // Power iteration for top eigenvectors
    let mut components = Vec::with_capacity(target_dim);
    let mut deflated_cov = cov;

    for _ in 0..target_dim {
        let eigvec = power_iteration(&deflated_cov, d, 100);
        // Deflate: C = C - lambda * v * v^T
        // lambda = v^T C v
        let mut lambda = 0.0f32;
        for i in 0..d {
            let mut row_dot = 0.0f32;
            for j in 0..d {
                row_dot += deflated_cov[i][j] * eigvec[j];
            }
            lambda += eigvec[i] * row_dot;
        }
        for i in 0..d {
            for j in 0..d {
                deflated_cov[i][j] -= lambda * eigvec[i] * eigvec[j];
            }
        }
        components.push(eigvec);
    }

    // Project data onto components
    centered.iter().map(|v| {
        components.iter().map(|comp| {
            v.iter().zip(comp).map(|(a, b)| a * b).sum()
        }).collect()
    }).collect()
}

fn power_iteration(matrix: &[Vec<f32>], d: usize, iterations: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; d];
    // Initialize with [1, 0, 0, ...]
    if d > 0 { v[0] = 1.0; }
    // Add some variation to avoid degenerate cases
    for i in 0..d { v[i] = 1.0 / (1.0 + i as f32); }
    normalize(&mut v);

    for _ in 0..iterations {
        let mut new_v = vec![0.0f32; d];
        for i in 0..d {
            for j in 0..d {
                new_v[i] += matrix[i][j] * v[j];
            }
        }
        normalize(&mut new_v);
        v = new_v;
    }
    v
}

fn normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-12 {
        for x in v.iter_mut() { *x /= norm; }
    }
}

// ── t-SNE ───────────────────────────────────────────────────────────────

/// Simplified t-SNE implementation.
pub fn tsne(vectors: &[Vec<f32>], target_dim: usize, perplexity: f32, iterations: usize) -> Vec<Vec2> {
    let n = vectors.len();
    if n == 0 { return vec![]; }
    let target_dim = target_dim.min(2); // we output Vec2 so max 2
    let _ = target_dim;

    // Compute pairwise squared distances
    let mut dist2 = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            let d: f32 = vectors[i].iter().zip(&vectors[j])
                .map(|(a, b)| (a - b) * (a - b)).sum();
            dist2[i][j] = d;
            dist2[j][i] = d;
        }
    }

    // Compute conditional probabilities P(j|i) using binary search for sigma
    let mut p = vec![vec![0.0f32; n]; n];
    let target_entropy = perplexity.ln();

    for i in 0..n {
        let mut sigma = 1.0f32;
        // Binary search for sigma that matches target perplexity
        let mut lo = 1e-10f32;
        let mut hi = 1e4f32;
        for _ in 0..50 {
            sigma = (lo + hi) / 2.0;
            let mut sum_exp = 0.0f32;
            for j in 0..n {
                if j != i {
                    sum_exp += (-dist2[i][j] / (2.0 * sigma * sigma)).exp();
                }
            }
            if sum_exp < 1e-12 { lo = sigma; continue; }
            let mut entropy = 0.0f32;
            for j in 0..n {
                if j != i {
                    let pj = (-dist2[i][j] / (2.0 * sigma * sigma)).exp() / sum_exp;
                    if pj > 1e-12 { entropy -= pj * pj.ln(); }
                }
            }
            if entropy > target_entropy { hi = sigma; } else { lo = sigma; }
        }
        // Set probabilities with this sigma
        let mut sum_exp = 0.0f32;
        for j in 0..n {
            if j != i {
                sum_exp += (-dist2[i][j] / (2.0 * sigma * sigma)).exp();
            }
        }
        if sum_exp > 1e-12 {
            for j in 0..n {
                if j != i {
                    p[i][j] = (-dist2[i][j] / (2.0 * sigma * sigma)).exp() / sum_exp;
                }
            }
        }
    }

    // Symmetrize: P = (P + P^T) / (2N)
    for i in 0..n {
        for j in i + 1..n {
            let sym = (p[i][j] + p[j][i]) / (2.0 * n as f32);
            p[i][j] = sym.max(1e-12);
            p[j][i] = sym.max(1e-12);
        }
    }

    // Initialize embedding with small random values
    let mut y: Vec<[f32; 2]> = Vec::with_capacity(n);
    let mut rng = 42u64;
    for _ in 0..n {
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
        let x = (rng as u32 as f32 / u32::MAX as f32 - 0.5) * 0.01;
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
        let y_val = (rng as u32 as f32 / u32::MAX as f32 - 0.5) * 0.01;
        y.push([x, y_val]);
    }

    let lr = 200.0f32;
    let momentum = 0.8f32;
    let mut gains = vec![[1.0f32; 2]; n];
    let mut vy = vec![[0.0f32; 2]; n];

    for _iter in 0..iterations {
        // Compute Q distribution (Student-t with 1 DOF)
        let mut q_unnorm = vec![vec![0.0f32; n]; n];
        let mut q_sum = 0.0f32;
        for i in 0..n {
            for j in i + 1..n {
                let dx = y[i][0] - y[j][0];
                let dy = y[i][1] - y[j][1];
                let val = 1.0 / (1.0 + dx * dx + dy * dy);
                q_unnorm[i][j] = val;
                q_unnorm[j][i] = val;
                q_sum += 2.0 * val;
            }
        }
        if q_sum < 1e-12 { q_sum = 1e-12; }

        // Compute gradients
        let mut grad = vec![[0.0f32; 2]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j { continue; }
                let q_ij = q_unnorm[i][j] / q_sum;
                let mult = 4.0 * (p[i][j] - q_ij) * q_unnorm[i][j];
                grad[i][0] += mult * (y[i][0] - y[j][0]);
                grad[i][1] += mult * (y[i][1] - y[j][1]);
            }
        }

        // Update
        for i in 0..n {
            for d in 0..2 {
                // Adaptive gains
                if (grad[i][d] > 0.0) != (vy[i][d] > 0.0) {
                    gains[i][d] = (gains[i][d] + 0.2).min(10.0);
                } else {
                    gains[i][d] = (gains[i][d] * 0.8).max(0.01);
                }
                vy[i][d] = momentum * vy[i][d] - lr * gains[i][d] * grad[i][d];
                y[i][d] += vy[i][d];
            }
        }
    }

    y.iter().map(|p| Vec2::new(p[0], p[1])).collect()
}

// ── UMAP (simplified) ───────────────────────────────────────────────────

/// Simplified UMAP: approximate using nearest-neighbor graph + force-directed layout.
pub fn umap(vectors: &[Vec<f32>], n_neighbors: usize, min_dist: f32, target_dim: usize) -> Vec<Vec2> {
    let n = vectors.len();
    if n == 0 { return vec![]; }
    let _ = target_dim; // we always produce 2D

    // Compute pairwise distances and build k-NN graph
    let mut knn: Vec<Vec<(usize, f32)>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut dists: Vec<(usize, f32)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| {
                let d: f32 = vectors[i].iter().zip(&vectors[j])
                    .map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
                (j, d)
            })
            .collect();
        dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        dists.truncate(n_neighbors);
        knn.push(dists);
    }

    // Build symmetric adjacency weights
    let mut weights = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        let sigma = knn[i].last().map(|&(_, d)| d.max(1e-6)).unwrap_or(1.0);
        for &(j, d) in &knn[i] {
            let w = (-d / sigma).exp();
            weights[i][j] = weights[i][j].max(w);
            weights[j][i] = weights[j][i].max(w);
        }
    }

    // Initialize layout
    let mut pos: Vec<[f32; 2]> = Vec::with_capacity(n);
    let mut rng = 123u64;
    for _ in 0..n {
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
        let x = (rng as u32 as f32 / u32::MAX as f32 - 0.5) * 10.0;
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
        let y = (rng as u32 as f32 / u32::MAX as f32 - 0.5) * 10.0;
        pos.push([x, y]);
    }

    // Optimize with attractive/repulsive forces
    let epochs = 200;
    let initial_lr = 1.0f32;

    for epoch in 0..epochs {
        let lr = initial_lr * (1.0 - epoch as f32 / epochs as f32);
        let mut forces = vec![[0.0f32; 2]; n];

        for i in 0..n {
            for &(j, _) in &knn[i] {
                let dx = pos[i][0] - pos[j][0];
                let dy = pos[i][1] - pos[j][1];
                let dist = (dx * dx + dy * dy).sqrt().max(min_dist);
                // Attractive force
                let attract = -2.0 * weights[i][j] * (dist - min_dist) / dist;
                forces[i][0] += attract * dx;
                forces[i][1] += attract * dy;
            }

            // Repulsive: sample a few random non-neighbors
            let mut neg_rng = rng.wrapping_add(i as u64);
            for _ in 0..5.min(n) {
                neg_rng ^= neg_rng << 13;
                neg_rng ^= neg_rng >> 7;
                neg_rng ^= neg_rng << 17;
                let j = neg_rng as usize % n;
                if j == i { continue; }
                let dx = pos[i][0] - pos[j][0];
                let dy = pos[i][1] - pos[j][1];
                let dist2 = dx * dx + dy * dy;
                let repel = 2.0 / (dist2 + 0.01);
                forces[i][0] += repel * dx;
                forces[i][1] += repel * dy;
            }
        }

        for i in 0..n {
            pos[i][0] += lr * forces[i][0].clamp(-4.0, 4.0);
            pos[i][1] += lr * forces[i][1].clamp(-4.0, 4.0);
        }
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    }

    pos.iter().map(|p| Vec2::new(p[0], p[1])).collect()
}

// ── Embedding Renderer ──────────────────────────────────────────────────

/// Renders embedding points as colored glyphs for 2D visualization.
pub struct EmbeddingRenderer {
    pub point_size: f32,
    pub color_by_label: bool,
}

impl EmbeddingRenderer {
    pub fn new() -> Self {
        Self { point_size: 3.0, color_by_label: true }
    }

    /// Render 2D points to a list of (position, color_rgba).
    pub fn render(&self, points: &[Vec2], labels: &[String]) -> Vec<(Vec2, [f32; 4])> {
        let unique_labels: Vec<&String> = {
            let mut u: Vec<&String> = labels.iter().collect();
            u.sort();
            u.dedup();
            u
        };

        points.iter().zip(labels).map(|(&pos, label)| {
            let color = if self.color_by_label {
                let idx = unique_labels.iter().position(|l| *l == label).unwrap_or(0);
                label_to_color(idx, unique_labels.len())
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            (pos, color)
        }).collect()
    }
}

fn label_to_color(index: usize, total: usize) -> [f32; 4] {
    if total == 0 { return [1.0, 1.0, 1.0, 1.0]; }
    let hue = index as f32 / total as f32;
    // Simple HSV -> RGB with S=0.8, V=0.9
    let h = hue * 6.0;
    let c = 0.9 * 0.8;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let m = 0.9 - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_nearest_neighbors() {
        let mut space = EmbeddingSpace::new(3);
        space.add(vec![1.0, 0.0, 0.0], "a".into());
        space.add(vec![0.9, 0.1, 0.0], "b".into());
        space.add(vec![0.0, 1.0, 0.0], "c".into());
        space.add(vec![0.0, 0.0, 1.0], "d".into());

        let query = vec![1.0, 0.0, 0.0];
        let nn = nearest_neighbors(&space, &query, 2);
        assert_eq!(nn.len(), 2);
        assert_eq!(nn[0].0, 0); // self is most similar
        assert_eq!(nn[1].0, 1); // "b" is next closest
    }

    #[test]
    fn test_pca_reduces_dimensions() {
        let vectors = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![1.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![1.0, 0.0, 1.0, 0.0],
        ];
        let reduced = pca(&vectors, 2);
        assert_eq!(reduced.len(), 5);
        assert_eq!(reduced[0].len(), 2);
    }

    #[test]
    fn test_pca_preserves_variance_ordering() {
        // Data with more variance along first dimension
        let vectors: Vec<Vec<f32>> = (0..20).map(|i| {
            vec![i as f32 * 10.0, (i % 3) as f32, 0.5]
        }).collect();
        let reduced = pca(&vectors, 2);
        // First component should capture more variance
        let var1: f32 = reduced.iter().map(|v| v[0] * v[0]).sum::<f32>() / reduced.len() as f32;
        let var2: f32 = reduced.iter().map(|v| v[1] * v[1]).sum::<f32>() / reduced.len() as f32;
        assert!(var1 > var2, "PCA first component variance ({var1}) should exceed second ({var2})");
    }

    #[test]
    fn test_tsne_output_size() {
        let vectors = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![1.0, 1.0, 0.0],
        ];
        let result = tsne(&vectors, 2, 2.0, 50);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_umap_output_size() {
        let vectors = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![1.0, 1.0, 0.0],
            vec![0.0, 1.0, 1.0],
        ];
        let result = umap(&vectors, 2, 0.1, 2);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_embedding_renderer() {
        let renderer = EmbeddingRenderer::new();
        let points = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)];
        let labels = vec!["a".to_string(), "b".to_string()];
        let result = renderer.render(&points, &labels);
        assert_eq!(result.len(), 2);
        // Colors should differ for different labels
        assert_ne!(result[0].1, result[1].1);
    }

    #[test]
    fn test_embedding_space() {
        let mut space = EmbeddingSpace::new(4);
        assert!(space.is_empty());
        space.add(vec![1.0, 2.0, 3.0, 4.0], "test".into());
        assert_eq!(space.len(), 1);
    }
}
