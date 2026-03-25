//! Percolation theory: site and bond percolation on 2D grids,
//! cluster detection via union-find, spanning cluster identification,
//! and critical threshold estimation.

use super::brownian::Rng;
use glam::Vec2;

// ---------------------------------------------------------------------------
// PercolationGrid
// ---------------------------------------------------------------------------

/// A 2D percolation grid where each site is either open (true) or closed (false).
pub struct PercolationGrid {
    pub width: usize,
    pub height: usize,
    /// Flattened row-major grid. sites[y * width + x] = true if site (x,y) is open.
    pub sites: Vec<bool>,
}

impl PercolationGrid {
    /// Create an empty (all closed) grid.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            sites: vec![false; width * height],
        }
    }

    /// Check if site (x, y) is open.
    pub fn is_open(&self, x: usize, y: usize) -> bool {
        if x < self.width && y < self.height {
            self.sites[y * self.width + x]
        } else {
            false
        }
    }

    /// Set site (x, y).
    pub fn set(&mut self, x: usize, y: usize, open: bool) {
        if x < self.width && y < self.height {
            self.sites[y * self.width + x] = open;
        }
    }

    /// Count of open sites.
    pub fn open_count(&self) -> usize {
        self.sites.iter().filter(|&&s| s).count()
    }

    /// Fraction of open sites.
    pub fn open_fraction(&self) -> f64 {
        self.open_count() as f64 / (self.width * self.height) as f64
    }

    /// Get 4-connected neighbors of (x, y) that are within bounds.
    fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut n = Vec::with_capacity(4);
        if x > 0 {
            n.push((x - 1, y));
        }
        if x + 1 < self.width {
            n.push((x + 1, y));
        }
        if y > 0 {
            n.push((x, y - 1));
        }
        if y + 1 < self.height {
            n.push((x, y + 1));
        }
        n
    }
}

// ---------------------------------------------------------------------------
// Grid generation
// ---------------------------------------------------------------------------

/// Generate a site percolation grid: each site is open with probability p.
pub fn site_percolation(width: usize, height: usize, p: f64, rng: &mut Rng) -> PercolationGrid {
    let sites: Vec<bool> = (0..width * height).map(|_| rng.uniform() < p).collect();
    PercolationGrid { width, height, sites }
}

/// Generate a bond percolation grid.
///
/// In bond percolation, bonds (edges) are open with probability p.
/// We represent this by opening a site if at least one of its bonds is open.
/// More precisely, we track horizontal and vertical bonds, then a site is
/// considered "connected" if it has an open bond to a neighbor. For simplicity,
/// we convert to an equivalent site representation on a finer grid:
/// the effective site at (x,y) is open if the bond connecting it to a neighbor is open.
///
/// For practical purposes, we return a grid where sites represent connectivity.
pub fn bond_percolation(width: usize, height: usize, p: f64, rng: &mut Rng) -> PercolationGrid {
    // Horizontal bonds: (width-1) * height
    // Vertical bonds: width * (height-1)
    let mut h_bonds = vec![vec![false; width.saturating_sub(1)]; height];
    let mut v_bonds = vec![vec![false; width]; height.saturating_sub(1)];

    for row in h_bonds.iter_mut() {
        for bond in row.iter_mut() {
            *bond = rng.uniform() < p;
        }
    }
    for row in v_bonds.iter_mut() {
        for bond in row.iter_mut() {
            *bond = rng.uniform() < p;
        }
    }

    // A site is "open" (reachable) if it has at least one open bond
    // For percolation analysis, we use union-find on bonds directly.
    // But to return a PercolationGrid, we mark sites as open if they
    // are connected to at least one open bond.
    let mut grid = PercolationGrid::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let has_bond =
                (x > 0 && h_bonds[y][x - 1])
                    || (x < width.saturating_sub(1) && h_bonds[y][x])
                    || (y > 0 && v_bonds[y - 1][x])
                    || (y < height.saturating_sub(1) && v_bonds[y][x]);
            grid.set(x, y, has_bond);
        }
    }

    // Override: use union-find on bond connectivity for proper percolation.
    // We'll store the bond information in the grid for cluster detection.
    // Actually, let's use a union-find on all sites connected by open bonds.
    let mut uf = UnionFind::new(width * height);
    for y in 0..height {
        for x in 0..width {
            // Horizontal bond to (x+1, y)
            if x + 1 < width && x < h_bonds[y].len() && h_bonds[y][x] {
                uf.union(y * width + x, y * width + x + 1);
                grid.set(x, y, true);
                grid.set(x + 1, y, true);
            }
            // Vertical bond to (x, y+1)
            if y + 1 < height && y < v_bonds.len() && v_bonds[y][x] {
                uf.union(y * width + x, (y + 1) * width + x);
                grid.set(x, y, true);
                grid.set(x, y + 1, true);
            }
        }
    }

    grid
}

// ---------------------------------------------------------------------------
// Union-Find
// ---------------------------------------------------------------------------

/// Weighted union-find with path compression.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]]; // path halving
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] += 1;
        }
    }

    fn connected(&mut self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }
}

// ---------------------------------------------------------------------------
// Cluster detection
// ---------------------------------------------------------------------------

/// Find all connected clusters of open sites using union-find.
/// Returns a vector of clusters, each cluster being a vector of (x, y) coordinates.
pub fn find_clusters(grid: &PercolationGrid) -> Vec<Vec<(usize, usize)>> {
    let w = grid.width;
    let h = grid.height;
    let mut uf = UnionFind::new(w * h);

    // Union adjacent open sites
    for y in 0..h {
        for x in 0..w {
            if !grid.is_open(x, y) {
                continue;
            }
            let idx = y * w + x;
            // Right neighbor
            if x + 1 < w && grid.is_open(x + 1, y) {
                uf.union(idx, y * w + x + 1);
            }
            // Down neighbor
            if y + 1 < h && grid.is_open(x, y + 1) {
                uf.union(idx, (y + 1) * w + x);
            }
        }
    }

    // Collect clusters
    let mut cluster_map: std::collections::HashMap<usize, Vec<(usize, usize)>> =
        std::collections::HashMap::new();
    for y in 0..h {
        for x in 0..w {
            if grid.is_open(x, y) {
                let root = uf.find(y * w + x);
                cluster_map.entry(root).or_default().push((x, y));
            }
        }
    }

    let mut clusters: Vec<Vec<(usize, usize)>> = cluster_map.into_values().collect();
    clusters.sort_by(|a, b| b.len().cmp(&a.len())); // largest first
    clusters
}

/// Check if the grid percolates (there exists a cluster spanning from top to bottom).
pub fn percolates(grid: &PercolationGrid) -> bool {
    let w = grid.width;
    let h = grid.height;
    if h == 0 || w == 0 {
        return false;
    }

    let mut uf = UnionFind::new(w * h + 2); // +2 for virtual top and bottom
    let virtual_top = w * h;
    let virtual_bottom = w * h + 1;

    // Connect top row to virtual top, bottom row to virtual bottom
    for x in 0..w {
        if grid.is_open(x, 0) {
            uf.union(x, virtual_top);
        }
        if grid.is_open(x, h - 1) {
            uf.union((h - 1) * w + x, virtual_bottom);
        }
    }

    // Union adjacent open sites
    for y in 0..h {
        for x in 0..w {
            if !grid.is_open(x, y) {
                continue;
            }
            let idx = y * w + x;
            if x + 1 < w && grid.is_open(x + 1, y) {
                uf.union(idx, y * w + x + 1);
            }
            if y + 1 < h && grid.is_open(x, y + 1) {
                uf.union(idx, (y + 1) * w + x);
            }
        }
    }

    uf.connected(virtual_top, virtual_bottom)
}

/// Find the spanning cluster (if it exists).
pub fn spanning_cluster(grid: &PercolationGrid) -> Option<Vec<(usize, usize)>> {
    let clusters = find_clusters(grid);
    let h = grid.height;
    for cluster in clusters {
        let has_top = cluster.iter().any(|&(_, y)| y == 0);
        let has_bottom = cluster.iter().any(|&(_, y)| y == h - 1);
        if has_top && has_bottom {
            return Some(cluster);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Critical threshold estimation
// ---------------------------------------------------------------------------

/// Estimate the critical percolation probability p_c using binary search.
///
/// For 2D site percolation on a square lattice, p_c ≈ 0.5927.
/// For bond percolation, p_c = 0.5 (exactly).
pub fn find_critical_p(
    width: usize,
    height: usize,
    trials: usize,
    rng: &mut Rng,
) -> f64 {
    let mut lo = 0.0;
    let mut hi = 1.0;

    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        let perc_count = (0..trials)
            .filter(|_| {
                let grid = site_percolation(width, height, mid, rng);
                percolates(&grid)
            })
            .count();
        let perc_fraction = perc_count as f64 / trials as f64;

        if perc_fraction < 0.5 {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    (lo + hi) / 2.0
}

/// Estimate percolation probability at a given p.
pub fn percolation_probability(
    width: usize,
    height: usize,
    p: f64,
    trials: usize,
    rng: &mut Rng,
) -> f64 {
    let count = (0..trials)
        .filter(|_| {
            let grid = site_percolation(width, height, p, rng);
            percolates(&grid)
        })
        .count();
    count as f64 / trials as f64
}

/// Compute the percolation curve: probability of percolation vs p.
pub fn percolation_curve(
    width: usize,
    height: usize,
    trials_per_point: usize,
    p_points: usize,
    rng: &mut Rng,
) -> Vec<(f64, f64)> {
    (0..=p_points)
        .map(|i| {
            let p = i as f64 / p_points as f64;
            let prob = percolation_probability(width, height, p, trials_per_point, rng);
            (p, prob)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// PercolationRenderer
// ---------------------------------------------------------------------------

/// Render a percolation grid with cluster colors and spanning cluster highlighted.
pub struct PercolationRenderer {
    pub closed_character: char,
    pub open_character: char,
    pub spanning_character: char,
    pub closed_color: [f32; 4],
    pub spanning_color: [f32; 4],
    pub cell_size: f32,
}

impl PercolationRenderer {
    pub fn new() -> Self {
        Self {
            closed_character: '░',
            open_character: '█',
            spanning_character: '▓',
            closed_color: [0.2, 0.2, 0.2, 0.5],
            spanning_color: [1.0, 0.3, 0.2, 1.0],
            cell_size: 0.5,
        }
    }

    /// Generate a deterministic color for a cluster index.
    fn cluster_color(&self, index: usize) -> [f32; 4] {
        let hues = [
            [0.2, 0.7, 1.0, 0.9],
            [0.3, 1.0, 0.4, 0.9],
            [1.0, 0.8, 0.2, 0.9],
            [0.8, 0.3, 1.0, 0.9],
            [1.0, 0.5, 0.5, 0.9],
            [0.5, 1.0, 0.8, 0.9],
            [0.9, 0.9, 0.3, 0.9],
            [0.4, 0.5, 1.0, 0.9],
        ];
        hues[index % hues.len()]
    }

    /// Render the full grid with clusters colored.
    pub fn render(&self, grid: &PercolationGrid) -> Vec<(Vec2, char, [f32; 4])> {
        let w = grid.width;
        let h = grid.height;
        let clusters = find_clusters(grid);
        let span = spanning_cluster(grid);

        // Build a lookup from (x,y) to cluster index
        let mut site_cluster = std::collections::HashMap::new();
        for (ci, cluster) in clusters.iter().enumerate() {
            for &pos in cluster {
                site_cluster.insert(pos, ci);
            }
        }

        // Build spanning set
        let spanning_set: std::collections::HashSet<(usize, usize)> = span
            .map(|c| c.into_iter().collect())
            .unwrap_or_default();

        let mut glyphs = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let pos = Vec2::new(x as f32 * self.cell_size, (h - 1 - y) as f32 * self.cell_size);
                if !grid.is_open(x, y) {
                    glyphs.push((pos, self.closed_character, self.closed_color));
                } else if spanning_set.contains(&(x, y)) {
                    glyphs.push((pos, self.spanning_character, self.spanning_color));
                } else if let Some(&ci) = site_cluster.get(&(x, y)) {
                    let color = self.cluster_color(ci);
                    glyphs.push((pos, self.open_character, color));
                } else {
                    glyphs.push((pos, self.open_character, [0.5, 0.5, 0.5, 0.7]));
                }
            }
        }
        glyphs
    }

    /// Render the percolation curve as a line chart.
    pub fn render_curve(&self, curve: &[(f64, f64)]) -> Vec<(Vec2, char, [f32; 4])> {
        let color = [0.2, 0.8, 1.0, 1.0];
        curve
            .iter()
            .map(|&(p, prob)| {
                let pos = Vec2::new(p as f32 * 10.0, prob as f32 * 10.0);
                (pos, '·', color)
            })
            .collect()
    }
}

impl Default for PercolationRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_percolation_p0() {
        let mut rng = Rng::new(42);
        let grid = site_percolation(20, 20, 0.0, &mut rng);
        assert_eq!(grid.open_count(), 0);
        assert!(!percolates(&grid));
    }

    #[test]
    fn test_site_percolation_p1() {
        let mut rng = Rng::new(42);
        let grid = site_percolation(20, 20, 1.0, &mut rng);
        assert_eq!(grid.open_count(), 400);
        assert!(percolates(&grid));
    }

    #[test]
    fn test_find_clusters() {
        let mut grid = PercolationGrid::new(5, 5);
        // Create two separate clusters
        grid.set(0, 0, true);
        grid.set(1, 0, true);
        grid.set(3, 3, true);
        grid.set(4, 3, true);
        grid.set(4, 4, true);

        let clusters = find_clusters(&grid);
        assert_eq!(clusters.len(), 2);
        // Largest cluster has 3 sites
        assert_eq!(clusters[0].len(), 3);
        assert_eq!(clusters[1].len(), 2);
    }

    #[test]
    fn test_percolates_simple() {
        // Create a grid that percolates (column of open sites)
        let mut grid = PercolationGrid::new(5, 5);
        for y in 0..5 {
            grid.set(2, y, true);
        }
        assert!(percolates(&grid));

        // Remove middle -> no longer percolates
        grid.set(2, 2, false);
        assert!(!percolates(&grid));
    }

    #[test]
    fn test_spanning_cluster() {
        let mut grid = PercolationGrid::new(5, 5);
        for y in 0..5 {
            grid.set(0, y, true);
        }
        let span = spanning_cluster(&grid);
        assert!(span.is_some());
        assert_eq!(span.unwrap().len(), 5);
    }

    #[test]
    fn test_critical_p_approximate() {
        // p_c for 2D site percolation ≈ 0.5927
        // Due to finite size effects and limited trials, we just check it's in a reasonable range
        let mut rng = Rng::new(42);
        let pc = find_critical_p(20, 20, 50, &mut rng);
        assert!(
            pc > 0.4 && pc < 0.8,
            "critical p should be roughly in [0.4, 0.8], got {}",
            pc
        );
    }

    #[test]
    fn test_percolation_monotone() {
        // Higher p should give higher percolation probability
        let mut rng = Rng::new(42);
        let prob_low = percolation_probability(20, 20, 0.3, 100, &mut rng);
        let prob_high = percolation_probability(20, 20, 0.8, 100, &mut rng);
        assert!(
            prob_high >= prob_low,
            "percolation probability should increase with p: low={}, high={}",
            prob_low,
            prob_high
        );
    }

    #[test]
    fn test_bond_percolation() {
        let mut rng = Rng::new(42);
        let grid = bond_percolation(20, 20, 0.0, &mut rng);
        assert_eq!(grid.open_count(), 0);

        let grid2 = bond_percolation(20, 20, 1.0, &mut rng);
        assert!(grid2.open_count() > 0);
    }

    #[test]
    fn test_renderer() {
        let mut rng = Rng::new(42);
        let grid = site_percolation(10, 10, 0.6, &mut rng);
        let renderer = PercolationRenderer::new();
        let glyphs = renderer.render(&grid);
        assert_eq!(glyphs.len(), 100); // 10x10 grid
    }

    #[test]
    fn test_union_find() {
        let mut uf = UnionFind::new(10);
        uf.union(0, 1);
        uf.union(1, 2);
        assert!(uf.connected(0, 2));
        assert!(!uf.connected(0, 5));
        uf.union(5, 6);
        uf.union(2, 5);
        assert!(uf.connected(0, 6));
    }

    #[test]
    fn test_percolation_curve() {
        let mut rng = Rng::new(42);
        let curve = percolation_curve(10, 10, 20, 10, &mut rng);
        assert_eq!(curve.len(), 11);
        // First point (p=0) should have prob ≈ 0
        assert!(curve[0].1 < 0.1);
        // Last point (p=1) should have prob = 1
        assert!((curve[10].1 - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_open_fraction() {
        let mut rng = Rng::new(42);
        let grid = site_percolation(100, 100, 0.5, &mut rng);
        let frac = grid.open_fraction();
        assert!(
            (frac - 0.5).abs() < 0.1,
            "open fraction should be ~0.5, got {}",
            frac
        );
    }
}
