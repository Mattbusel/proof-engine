//! Collatz conjecture: sequences, stopping times, and tree visualization.

use glam::{Vec3, Vec4};

/// Compute the Collatz sequence starting from n until it reaches 1.
pub fn collatz_sequence(n: u64) -> Vec<u64> {
    if n == 0 {
        return vec![0];
    }
    let mut seq = vec![n];
    let mut current = n;
    while current != 1 {
        current = if current % 2 == 0 {
            current / 2
        } else {
            3 * current + 1
        };
        seq.push(current);
    }
    seq
}

/// Number of steps for n to reach 1.
pub fn collatz_stopping_time(n: u64) -> u32 {
    if n <= 1 {
        return 0;
    }
    let mut current = n;
    let mut steps = 0u32;
    while current != 1 {
        current = if current % 2 == 0 {
            current / 2
        } else {
            3 * current + 1
        };
        steps += 1;
    }
    steps
}

/// A node in the reverse Collatz tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub value: u64,
    pub children: Vec<TreeNode>,
}

/// The full reverse Collatz tree: starting from 1, build upward.
/// Each node n has child 2n (always), and child (n-1)/3 if n ≡ 1 mod 3 and (n-1)/3 is odd and > 0.
pub struct CollatzTree {
    pub root: TreeNode,
    pub limit: u64,
}

impl CollatzTree {
    /// Build the reverse Collatz tree up to a given limit on node values.
    pub fn build(limit: u64) -> Self {
        let root = Self::build_node(1, limit, 0, 40);
        CollatzTree { root, limit }
    }

    fn build_node(value: u64, limit: u64, depth: usize, max_depth: usize) -> TreeNode {
        if depth >= max_depth {
            return TreeNode { value, children: vec![] };
        }
        let mut children = Vec::new();

        // Child via n -> 2n (reverse of n/2)
        let child_even = 2 * value;
        if child_even <= limit {
            children.push(Self::build_node(child_even, limit, depth + 1, max_depth));
        }

        // Child via n -> (n-1)/3 reversed: if value came from 3k+1, then k = (value-1)/3
        // Reverse: if some odd k maps to value via 3k+1 = value, then k = (value-1)/3
        // We want k to be a valid predecessor: k must be odd and (value-1) divisible by 3
        if value > 4 && (value - 1) % 3 == 0 {
            let k = (value - 1) / 3;
            if k > 0 && k % 2 == 1 && k != value {
                // k is odd and would map to value
                if k <= limit {
                    children.push(Self::build_node(k, limit, depth + 1, max_depth));
                }
            }
        }

        TreeNode { value, children }
    }

    /// Flatten the tree into a list of (value, depth) pairs.
    pub fn flatten(&self) -> Vec<(u64, usize)> {
        let mut result = Vec::new();
        Self::flatten_node(&self.root, 0, &mut result);
        result
    }

    fn flatten_node(node: &TreeNode, depth: usize, result: &mut Vec<(u64, usize)>) {
        result.push((node.value, depth));
        for child in &node.children {
            Self::flatten_node(child, depth + 1, result);
        }
    }

    /// Count total nodes in the tree.
    pub fn node_count(&self) -> usize {
        Self::count_nodes(&self.root)
    }

    fn count_nodes(node: &TreeNode) -> usize {
        1 + node.children.iter().map(|c| Self::count_nodes(c)).sum::<usize>()
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

/// Layout and render the Collatz tree with branch colors based on path length.
pub struct CollatzTreeRenderer {
    pub origin: Vec3,
    pub scale: f32,
    pub horizontal_spread: f32,
}

pub struct CollatzGlyph {
    pub value: u64,
    pub depth: usize,
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl CollatzTreeRenderer {
    pub fn new(origin: Vec3, scale: f32, horizontal_spread: f32) -> Self {
        Self { origin, scale, horizontal_spread }
    }

    /// Render a Collatz tree.
    pub fn render(&self, tree: &CollatzTree) -> Vec<CollatzGlyph> {
        let mut glyphs = Vec::new();
        let mut counter = 0usize;
        self.render_node(&tree.root, 0, &mut counter, &mut glyphs);
        glyphs
    }

    fn render_node(
        &self,
        node: &TreeNode,
        depth: usize,
        counter: &mut usize,
        glyphs: &mut Vec<CollatzGlyph>,
    ) {
        let x = *counter as f32 * self.horizontal_spread;
        let y = -(depth as f32) * self.scale;
        let stopping = collatz_stopping_time(node.value);
        let t = (stopping as f32 / 50.0).min(1.0);
        glyphs.push(CollatzGlyph {
            value: node.value,
            depth,
            position: self.origin + Vec3::new(x, y, 0.0),
            color: Vec4::new(t, 0.5, 1.0 - t, 1.0),
            character: if node.value % 2 == 0 { 'E' } else { 'O' },
        });
        *counter += 1;
        for child in &node.children {
            self.render_node(child, depth + 1, counter, glyphs);
        }
    }

    /// Render a single Collatz sequence as a path of glyphs.
    pub fn render_sequence(&self, n: u64) -> Vec<CollatzGlyph> {
        let seq = collatz_sequence(n);
        let max_val = *seq.iter().max().unwrap_or(&1) as f32;
        seq.iter()
            .enumerate()
            .map(|(i, &v)| {
                let t = i as f32 / seq.len().max(1) as f32;
                let height = v as f32 / max_val;
                CollatzGlyph {
                    value: v,
                    depth: i,
                    position: self.origin + Vec3::new(i as f32 * self.scale, height * self.scale * 5.0, 0.0),
                    color: Vec4::new(t, height, 1.0 - t, 1.0),
                    character: if v % 2 == 0 { 'v' } else { '^' },
                }
            })
            .collect()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_basic() {
        let seq = collatz_sequence(6);
        assert_eq!(seq, vec![6, 3, 10, 5, 16, 8, 4, 2, 1]);
    }

    #[test]
    fn sequence_one() {
        assert_eq!(collatz_sequence(1), vec![1]);
    }

    #[test]
    fn sequence_27() {
        let seq = collatz_sequence(27);
        assert_eq!(*seq.last().unwrap(), 1);
        // 27 is famous for having a long sequence (111 steps)
        assert_eq!(seq.len(), 112); // 111 steps + initial value
    }

    #[test]
    fn stopping_time() {
        assert_eq!(collatz_stopping_time(1), 0);
        assert_eq!(collatz_stopping_time(2), 1);
        assert_eq!(collatz_stopping_time(6), 8);
        assert_eq!(collatz_stopping_time(27), 111);
    }

    #[test]
    fn tree_build() {
        let tree = CollatzTree::build(32);
        // Root should be 1
        assert_eq!(tree.root.value, 1);
        // 1 -> 2 is always a child
        assert!(tree.root.children.iter().any(|c| c.value == 2));
    }

    #[test]
    fn tree_contains_powers_of_2() {
        let tree = CollatzTree::build(64);
        let flat = tree.flatten();
        let values: Vec<u64> = flat.iter().map(|&(v, _)| v).collect();
        // All powers of 2 up to 64 should be in the tree
        for &p in &[1, 2, 4, 8, 16, 32, 64] {
            assert!(values.contains(&p), "tree should contain {}", p);
        }
    }

    #[test]
    fn tree_node_count() {
        let tree = CollatzTree::build(16);
        assert!(tree.node_count() >= 5); // at least 1,2,4,8,16
    }

    #[test]
    fn renderer_sequence() {
        let r = CollatzTreeRenderer::new(Vec3::ZERO, 1.0, 2.0);
        let glyphs = r.render_sequence(27);
        assert_eq!(glyphs.len(), 112);
        assert_eq!(glyphs[0].value, 27);
        assert_eq!(glyphs.last().unwrap().value, 1);
    }

    #[test]
    fn renderer_tree() {
        let tree = CollatzTree::build(32);
        let r = CollatzTreeRenderer::new(Vec3::ZERO, 1.0, 2.0);
        let glyphs = r.render(&tree);
        assert!(!glyphs.is_empty());
        assert_eq!(glyphs[0].value, 1); // root
    }

    #[test]
    fn all_reach_one() {
        // Verify Collatz for all n up to 1000
        for n in 1..=1000 {
            let seq = collatz_sequence(n);
            assert_eq!(*seq.last().unwrap(), 1, "Collatz failed for {}", n);
        }
    }
}
